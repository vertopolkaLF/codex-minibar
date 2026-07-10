//! Windows-specific behavior for the transient tray popup.

#[cfg(windows)]
use std::ffi::c_void;

#[cfg(windows)]
use eframe::CreationContext;
#[cfg(windows)]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::Dwm::{DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DwmSetWindowAttribute},
    UI::{
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON},
        WindowsAndMessaging::{
            GWL_EXSTYLE, GetCursorPos, GetWindowLongW, GetWindowRect, SWP_FRAMECHANGED,
            SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowLongW, SetWindowPos, WS_EX_APPWINDOW,
            WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        },
    },
};

/// Native state used to make the eframe window behave like a tray popup.
#[cfg(windows)]
pub struct TrayPopup {
    hwnd: HWND,
    button_was_down: bool,
}

#[cfg(windows)]
impl TrayPopup {
    pub fn configure(creation_context: &CreationContext<'_>) -> Option<Self> {
        let RawWindowHandle::Win32(handle) = creation_context.window_handle().ok()?.as_raw() else {
            return None;
        };
        let hwnd = handle.hwnd.get();

        // Keep the popup out of Alt+Tab/taskbar and ensure clicks do not activate it.
        unsafe {
            let style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            let style = (style & !WS_EX_APPWINDOW) | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
            SetWindowLongW(hwnd, GWL_EXSTYLE, style as i32);
            SetWindowPos(
                hwnd,
                0,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );

            // Windows 11 otherwise paints an accent-coloured DWM border around
            // transparent undecorated windows.
            let border_color = DWMWA_COLOR_NONE;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR as u32,
                &border_color as *const u32 as *const c_void,
                size_of::<u32>() as u32,
            );
        }

        Some(Self {
            hwnd,
            button_was_down: false,
        })
    }

    /// Detect a new mouse press that lands outside the popup without stealing it.
    pub fn clicked_outside(&mut self) -> bool {
        let button_is_down = unsafe {
            [VK_LBUTTON, VK_MBUTTON, VK_RBUTTON]
                .into_iter()
                .any(|button| GetAsyncKeyState(button as i32) < 0)
        };
        let is_new_click = button_is_down && !self.button_was_down;
        self.button_was_down = button_is_down;
        if !is_new_click {
            return false;
        }

        unsafe {
            let mut cursor = POINT { x: 0, y: 0 };
            let mut popup = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            GetCursorPos(&mut cursor);
            GetWindowRect(self.hwnd, &mut popup);
            cursor.x < popup.left
                || cursor.x >= popup.right
                || cursor.y < popup.top
                || cursor.y >= popup.bottom
        }
    }
}

#[cfg(not(windows))]
pub struct TrayPopup;

#[cfg(not(windows))]
impl TrayPopup {
    pub fn clicked_outside(&mut self) -> bool {
        false
    }
}
