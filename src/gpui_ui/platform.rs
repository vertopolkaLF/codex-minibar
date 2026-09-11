//! Native window chrome around the GPUI popup. GPUI owns rendering.
use crate::settings::PopupBackgroundMaterial;

pub const POPUP_WIDTH: f32 = 380.0;

pub fn system_animations_enabled() -> bool {
    #[cfg(windows)]
    unsafe {
        let mut enabled = 1i32;
        windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(
            0x1042,
            0,
            (&mut enabled as *mut i32).cast(),
            0,
        );
        return enabled != 0;
    }
    #[cfg(not(windows))]
    {
        true
    }
}

#[cfg(windows)]
pub fn hwnd(window: &gpui::Window) -> windows_sys::Win32::Foundation::HWND {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match HasWindowHandle::window_handle(window)
        .ok()
        .map(|handle| handle.as_raw())
    {
        Some(RawWindowHandle::Win32(handle)) => handle.hwnd.get() as _,
        _ => std::ptr::null_mut(),
    }
}

pub fn hide(window: &gpui::Window) {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(hwnd(window), 0);
    }
    #[cfg(not(windows))]
    {
        let _ = window;
    }
}

pub fn visible(window: &gpui::Window) -> bool {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd(window)) != 0
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        false
    }
}

pub fn show_near(window: &mut gpui::Window, x: i32, y: i32) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            Foundation::{POINT, RECT},
            Graphics::Gdi::*,
            UI::WindowsAndMessaging::*,
        };
        unsafe {
            let handle = hwnd(window);
            let monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
            let mut info: MONITORINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if GetMonitorInfoW(monitor, &mut info) == 0 {
                return;
            }
            let mut rect: RECT = std::mem::zeroed();
            GetWindowRect(handle, &mut rect);
            let work = info.rcWork;
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            let left = (x - width / 2).clamp(work.left, (work.right - width).max(work.left));
            let top = (y - height - 12).clamp(work.top, (work.bottom - height).max(work.top));
            let style = GetWindowLongW(handle, GWL_EXSTYLE);
            SetWindowLongW(
                handle,
                GWL_EXSTYLE,
                (style | WS_EX_TOOLWINDOW as i32) & !(WS_EX_APPWINDOW as i32),
            );
            SetWindowPos(
                handle,
                HWND_TOPMOST,
                left,
                top,
                0,
                0,
                SWP_NOSIZE | SWP_SHOWWINDOW,
            );
            SetForegroundWindow(handle);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (window, x, y);
    }
}

pub fn max_height(window: &gpui::Window) -> f32 {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Graphics::Gdi::*;
        let monitor = MonitorFromWindow(hwnd(window), MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return 640.0;
        }
        ((info.rcWork.bottom - info.rcWork.top) as f32 / window.scale_factor() * 0.8).max(100.0)
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        640.0
    }
}

pub fn resize_pinned(window: &mut gpui::Window, height: f32) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::{Foundation::RECT, UI::WindowsAndMessaging::*};
        unsafe {
            let handle = hwnd(window);
            let mut rect: RECT = std::mem::zeroed();
            GetWindowRect(handle, &mut rect);
            let old_height = rect.bottom - rect.top;
            window.resize(gpui::size(gpui::px(POPUP_WIDTH), gpui::px(height)));
            let new_height = (height * window.scale_factor()).round() as i32;
            SetWindowPos(
                handle,
                std::ptr::null_mut(),
                rect.left,
                rect.top + old_height - new_height,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
    #[cfg(not(windows))]
    {
        window.resize(gpui::size(gpui::px(POPUP_WIDTH), gpui::px(height)));
    }
}

pub fn appearance(window: &gpui::Window, settings: &crate::settings::Settings, dark: bool) {
    #[cfg(windows)]
    {
        use std::cell::Cell;
        use windows_sys::Win32::Graphics::{Dwm::*, Gdi::*};
        thread_local! {
            static LAST: Cell<(isize, bool, u8, i32, u32, u32)> = const { Cell::new((0, false, 255, -1, 0, 0)) };
        }
        let handle = hwnd(window);
        let kind = match settings.popup_background_material {
            PopupBackgroundMaterial::Mica => 2i32,
            PopupBackgroundMaterial::Acrylic => 3,
        };
        let radius =
            (settings.popup_corner_radius.dip() as f32 * window.scale_factor()).round() as i32;
        let bounds = window.bounds();
        let width = (f32::from(bounds.size.width) * window.scale_factor()) as u32;
        let height = (f32::from(bounds.size.height) * window.scale_factor()) as u32;
        let fingerprint = (handle as isize, dark, kind as u8, radius, width, height);
        if LAST.get() == fingerprint {
            return;
        }
        LAST.set(fingerprint);
        unsafe {
            let immersive = dark as i32;
            DwmSetWindowAttribute(handle, 20, (&immersive as *const i32).cast(), 4);
            window.set_background_appearance(if kind == 3 {
                gpui::WindowBackgroundAppearance::Blurred
            } else {
                gpui::WindowBackgroundAppearance::Transparent
            });
            DwmSetWindowAttribute(handle, 38, (&kind as *const i32).cast(), 4);
            let region = CreateRoundRectRgn(
                0,
                0,
                width as i32 + 1,
                height as i32 + 1,
                radius * 2,
                radius * 2,
            );
            if !region.is_null() && SetWindowRgn(handle, region, 1) == 0 {
                DeleteObject(region);
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (window, settings, dark);
    }
}
