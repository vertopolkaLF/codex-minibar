//! Native HWND plumbing for the GPUI popup host.
//!
//! GPUI owns the window procedure; these helpers restyle its HWND into a
//! borderless, topmost tool popup and clip
//! it to the visible capsule. Anything that moves, sizes or shows the HWND
//! sends synchronous messages that GPUI answers by borrowing the app, so those
//! calls must run from a foreground task, never inside a render or update.

#![cfg(windows)]

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::{
        Dwm::{
            DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_NCRENDERING_POLICY,
            DWMWA_TRANSITIONS_FORCEDISABLED, DWMWA_WINDOW_CORNER_PREFERENCE, DwmSetWindowAttribute,
        },
        Gdi::{
            CreateRectRgn, CreateRoundRectRgn, GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST,
            MONITORINFO, MonitorFromPoint, SetWindowRgn,
        },
    },
    UI::{
        HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
        Shell::{DefSubclassProc, SetWindowSubclass},
        WindowsAndMessaging::{
            GWL_EXSTYLE, GWL_STYLE, GetWindowLongPtrW, GetWindowRect, HTCLIENT, HWND_TOPMOST,
            SW_HIDE, SW_SHOWNOACTIVATE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
            SWP_NOZORDER, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow,
            WM_DISPLAYCHANGE, WM_NCCALCSIZE, WM_NCHITTEST, WM_SETTINGCHANGE, WS_CAPTION,
            WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW,
            WS_EX_TOPMOST, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
        },
    },
};

const SUBCLASS_ID: usize = 0x434D_4250; // "CMBP"
const DWMWCP_DONOTROUND: u32 = 1;
const DWMNCRP_DISABLED: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rect {
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) right: i32,
    pub(crate) bottom: i32,
}

impl Rect {
    pub(crate) fn width(self) -> i32 {
        (self.right - self.left).max(0)
    }

    pub(crate) fn height(self) -> i32 {
        (self.bottom - self.top).max(0)
    }

    fn from_win32(rect: RECT) -> Self {
        Self {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Monitor {
    pub(crate) bounds: Rect,
    pub(crate) work: Rect,
    pub(crate) dpi: u32,
}

impl Monitor {
    pub(crate) fn scale(self) -> f64 {
        f64::from(self.dpi.max(1)) / 96.0
    }
}

unsafe extern "system" fn popup_subclass_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    match message {
        // The client area is the entire window: no frame, no resize border,
        // no 1px Win11 top inset. GPUI then lays out over the exact HWND
        // rectangle that the region and outside-click math use.
        WM_NCCALCSIZE => 0,
        WM_NCHITTEST => HTCLIENT as LRESULT,
        WM_DISPLAYCHANGE | WM_SETTINGCHANGE => {
            crate::popup_window::send_command(crate::popup_window::PopupCommand::Reposition);
            unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
        }
        _ => unsafe { DefSubclassProc(hwnd, message, wparam, lparam) },
    }
}

/// One-time restyle of GPUI's PopUp window into the tray flyout host.
///
/// Must be called outside any GPUI borrow: `SWP_FRAMECHANGED` makes the
/// window recompute its client area, which GPUI observes via `WM_SIZE`.
pub(crate) fn configure(hwnd: HWND) {
    unsafe {
        SetWindowSubclass(hwnd, Some(popup_subclass_proc), SUBCLASS_ID, 0);
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let style = (style
            & !(WS_CAPTION | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU))
            | WS_POPUP;
        SetWindowLongPtrW(hwnd, GWL_STYLE, style as isize);
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let ex_style = (ex_style & !(WS_EX_APPWINDOW | WS_EX_LAYERED))
            | WS_EX_TOOLWINDOW
            | WS_EX_TOPMOST
            | WS_EX_NOREDIRECTIONBITMAP;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style as isize);

        let set_u32 = |attribute: i32, value: u32| {
            let _ = DwmSetWindowAttribute(
                hwnd,
                attribute as u32,
                &value as *const u32 as *const _,
                size_of::<u32>() as u32,
            );
        };
        // DWM contributes no rounding, border or show/hide animation: the
        // capsule shape, stroke and motion are all painted by GPUI.
        set_u32(DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND);
        set_u32(DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE);
        set_u32(DWMWA_NCRENDERING_POLICY, DWMNCRP_DISABLED);
        set_u32(DWMWA_TRANSITIONS_FORCEDISABLED, 1);

        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
        );
    }
}

pub(crate) fn monitor_for_point(x: i32, y: i32) -> Monitor {
    unsafe {
        let mut monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        let mut info = monitor_info(monitor);
        // A point on the shared edge between monitors belongs to the
        // neighbor; pull inward so the flyout opens where the tray is.
        if x >= info.rcMonitor.right.saturating_sub(1) {
            monitor = MonitorFromPoint(
                POINT {
                    x: x.saturating_sub(2),
                    y,
                },
                MONITOR_DEFAULTTONEAREST,
            );
            info = monitor_info(monitor);
        }
        Monitor {
            bounds: Rect::from_win32(info.rcMonitor),
            work: Rect::from_win32(info.rcWork),
            dpi: monitor_dpi(monitor),
        }
    }
}

unsafe fn monitor_info(monitor: HMONITOR) -> MONITORINFO {
    let empty = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        rcMonitor: empty,
        rcWork: empty,
        dwFlags: 0,
    };
    unsafe { GetMonitorInfoW(monitor, &mut info) };
    info
}

fn monitor_dpi(monitor: HMONITOR) -> u32 {
    let mut dpi_x = 0u32;
    let mut dpi_y = 0u32;
    let ok = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    if ok == 0 && dpi_x > 0 { dpi_x } else { 96 }
}

pub(crate) fn window_rect(hwnd: HWND) -> Rect {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe {
        GetWindowRect(hwnd, &mut rect);
    }
    Rect::from_win32(rect)
}

/// Move and size the technical host. Outside any GPUI borrow only.
pub(crate) fn place(hwnd: HWND, rect: Rect) {
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            rect.left,
            rect.top,
            rect.width().max(1),
            rect.height().max(1),
            SWP_NOACTIVATE,
        );
    }
}

/// Show without activation. GPUI paints its first frame synchronously while
/// handling `WM_SHOWWINDOW`, so this must also run outside any GPUI borrow.
pub(crate) fn show(hwnd: HWND) {
    unsafe {
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
        );
    }
}

pub(crate) fn hide(hwnd: HWND) {
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }
}

/// A tray click authorizes foreground activation for this process.
pub(crate) fn activate(hwnd: HWND) {
    unsafe {
        let _ = SetForegroundWindow(hwnd);
    }
}

/// Clip the HWND to `rect` (window-relative pixels) with rounded corners.
/// `None` hides every pixel while keeping the compositor surface alive.
///
/// `SetWindowRgn` only sends position-change notifications without moving or
/// sizing the window, which GPUI ignores, so it is safe during a frame.
pub(crate) fn set_region(hwnd: HWND, rect: Option<Rect>, radius_px: i32) {
    unsafe {
        let region = match rect.filter(|rect| rect.width() > 0 && rect.height() > 0) {
            None => CreateRectRgn(0, 0, 0, 0),
            Some(rect) if radius_px <= 0 => {
                CreateRectRgn(rect.left, rect.top, rect.right, rect.bottom)
            }
            Some(rect) => {
                let arc = radius_px.saturating_mul(2);
                // GDI round regions exclude the right/bottom edge; extend by
                // one pixel so the capsule keeps its full size.
                CreateRoundRectRgn(
                    rect.left,
                    rect.top,
                    rect.right + 1,
                    rect.bottom + 1,
                    arc,
                    arc,
                )
            }
        };
        if region.is_null() {
            return;
        }
        // Ownership of the region passes to the system.
        // GPUI already paints the next frame. Requesting a native redraw here
        // invalidates the host again for every pixel of capsule motion.
        SetWindowRgn(hwnd, region, 0);
    }
}

pub(crate) fn hwnd_from_window(window: &gpui::Window) -> Option<HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as HWND),
        _ => None,
    }
}

/// `SWP_NOZORDER` placement used when only the host geometry changes while
/// the popup is already visible.
pub(crate) fn place_keep_order(hwnd: HWND, rect: Rect) {
    unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            rect.left,
            rect.top,
            rect.width().max(1),
            rect.height().max(1),
            SWP_NOACTIVATE | SWP_NOZORDER,
        );
    }
}
