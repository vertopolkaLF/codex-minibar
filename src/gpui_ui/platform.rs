//! Small native integration boundary. GPUI owns rendering and the event loop.
use crate::settings::{PopupBackgroundMaterial};
pub const POPUP_WIDTH: i32 = 380;
pub fn system_animations_enabled() -> bool {
    #[cfg(windows)] unsafe {
        let mut enabled = 1i32;
        windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(0x1042, 0, (&mut enabled as *mut i32).cast(), 0);
        return enabled != 0;
    }
    #[cfg(not(windows))] { true }
}
#[cfg(windows)]
pub fn hwnd(window: &gpui::Window) -> windows_sys::Win32::Foundation::HWND {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match HasWindowHandle::window_handle(window).ok().map(|h| h.as_raw()) {
        Some(RawWindowHandle::Win32(h)) => h.hwnd.get() as _,
        _ => std::ptr::null_mut(),
    }
}
#[cfg(windows)]
pub fn hide(window: &gpui::Window) { unsafe { windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(hwnd(window), 0); } }
#[cfg(windows)]
pub fn visible(window: &gpui::Window) -> bool { unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd(window)) != 0 } }
#[cfg(windows)]
pub fn show_near(window: &mut gpui::Window, x: i32, y: i32) {
    use windows_sys::Win32::{Foundation::{POINT,RECT}, Graphics::Gdi::*, UI::WindowsAndMessaging::*};
    unsafe {
        let handle = hwnd(window);
        let monitor = MonitorFromPoint(POINT{x,y}, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed(); info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 { return; }
        let mut rect: RECT = std::mem::zeroed(); GetWindowRect(handle, &mut rect);
        let work = info.rcWork; let width = rect.right-rect.left; let height = rect.bottom-rect.top;
        let left = (x-width/2).clamp(work.left, (work.right-width).max(work.left));
        let top = (y-height-12).clamp(work.top, (work.bottom-height).max(work.top));
        let style = GetWindowLongW(handle,GWL_EXSTYLE);
        SetWindowLongW(handle,GWL_EXSTYLE,(style | WS_EX_TOOLWINDOW as i32) & !(WS_EX_APPWINDOW as i32));
        SetWindowPos(handle, HWND_TOPMOST,left,top,0,0,SWP_NOSIZE | SWP_SHOWWINDOW);
        SetForegroundWindow(handle);
    }
}

#[cfg(windows)]
pub fn max_height(window: &gpui::Window) -> f32 {
    use windows_sys::Win32::Graphics::Gdi::*;
    unsafe {
        let monitor=MonitorFromWindow(hwnd(window),MONITOR_DEFAULTTONEAREST);
        let mut info:MONITORINFO=std::mem::zeroed();info.cbSize=std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor,&mut info)==0 {return 640.;}
        ((info.rcWork.bottom-info.rcWork.top) as f32 / window.scale_factor()*0.8).max(100.)
    }
}
#[cfg(windows)]
pub fn resize_pinned(window:&mut gpui::Window,height:f32) {
    use windows_sys::Win32::{Foundation::RECT,UI::WindowsAndMessaging::*};
    unsafe {
        let handle=hwnd(window);let mut rect:RECT=std::mem::zeroed();GetWindowRect(handle,&mut rect);
        let old_height=rect.bottom-rect.top;
        window.resize(gpui::size(gpui::px(380.),gpui::px(height)));
        let new_height=(height*window.scale_factor()).round() as i32;
        SetWindowPos(handle,std::ptr::null_mut(),rect.left,rect.top+old_height-new_height,0,0,SWP_NOSIZE|SWP_NOZORDER|SWP_NOACTIVATE);
    }
}
#[cfg(windows)]
pub fn appearance(window:&gpui::Window,settings:&crate::settings::Settings,dark:bool) {
    use windows_sys::Win32::{Graphics::{Dwm::*,Gdi::*},UI::WindowsAndMessaging::*};
    let handle=hwnd(window);
    unsafe {
        let immersive=dark as i32;
        DwmSetWindowAttribute(handle,20,(&immersive as *const i32).cast(),4);
        let kind=match settings.popup_background_material {PopupBackgroundMaterial::Mica=>2i32,PopupBackgroundMaterial::Acrylic=>3};
        window.set_background_appearance(if kind==3 {gpui::WindowBackgroundAppearance::Blurred}else{gpui::WindowBackgroundAppearance::Transparent});
        DwmSetWindowAttribute(handle,38,(&kind as *const i32).cast(),4);
        let radius=(settings.popup_corner_radius.dip() as f32*window.scale_factor()).round() as i32;
        let bounds=window.bounds();
        let region=CreateRoundRectRgn(0,0,(f32::from(bounds.size.width)*window.scale_factor()) as i32+1,(f32::from(bounds.size.height)*window.scale_factor()) as i32+1,radius*2,radius*2);
        if !region.is_null() && SetWindowRgn(handle,region,1)==0 {DeleteObject(region);}
    }
}

