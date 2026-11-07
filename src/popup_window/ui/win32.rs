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
            MONITOR_DEFAULTTONULL, MONITORINFO, MONITORINFOEXW, MonitorFromPoint,
            MonitorFromWindow, SetWindowRgn,
        },
    },
    UI::{
        Accessibility::{HWINEVENTHOOK, SetWinEventHook},
        HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
        Input::KeyboardAndMouse::{ReleaseCapture, SetCapture},
        Shell::{DefSubclassProc, SetWindowSubclass},
        WindowsAndMessaging::{
            EVENT_SYSTEM_FOREGROUND, FindWindowExW, GWL_EXSTYLE, GWL_STYLE, GetClassNameW,
            GetCursorPos, GetWindowLongPtrW, GetWindowRect, HTCAPTION, HTCLIENT, HWND_TOPMOST,
            IDC_SIZENS, LoadCursorW, MA_NOACTIVATE, SW_HIDE, SW_SHOWNOACTIVATE, SWP_FRAMECHANGED,
            SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER, SetCursor,
            SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow,
            WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_CAPTURECHANGED, WM_DISPLAYCHANGE,
            WM_DPICHANGED, WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_LBUTTONDOWN, WM_LBUTTONUP,
            WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_NCCALCSIZE, WM_NCHITTEST, WM_NCLBUTTONDBLCLK,
            WM_SETCURSOR, WM_SETTINGCHANGE, WS_CAPTION, WS_EX_APPWINDOW, WS_EX_LAYERED,
            WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
            WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
        },
    },
};

const SUBCLASS_ID: usize = 0x434D_4250; // "CMBP"
const DWMWCP_DONOTROUND: u32 = 1;
const DWMNCRP_DISABLED: u32 = 1;

thread_local! {
    static POPUP_HWND: std::cell::Cell<HWND> = const { std::cell::Cell::new(std::ptr::null_mut()) };
    static DRAG_HEADER: std::cell::Cell<Option<Rect>> = const { std::cell::Cell::new(None) };
    static MOVING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static RESIZE_AREA: std::cell::Cell<Option<ResizeArea>> = const { std::cell::Cell::new(None) };
    static HEIGHT_DRAG: std::cell::Cell<Option<HeightDrag>> = const { std::cell::Cell::new(None) };
}

#[derive(Clone, Copy)]
pub(crate) struct ResizeArea {
    pub(crate) surface: Rect,
    pub(crate) top_anchored: bool,
    pub(crate) floating: bool,
    pub(crate) scale: f32,
    pub(crate) minimum: f32,
    pub(crate) maximum: f32,
}

impl ResizeArea {
    fn edge_at(self, x: i32, y: i32) -> Option<bool> {
        let band = (5.0 * self.scale).ceil() as i32;
        let r = self.surface;
        if x < r.left + band || x >= r.right - band {
            return None;
        }
        if (self.floating || !self.top_anchored) && y >= r.top && y < r.top + band {
            Some(true)
        } else if (self.floating || self.top_anchored) && y >= r.bottom - band && y < r.bottom {
            Some(false)
        } else {
            None
        }
    }

    fn contains_edge(self, x: i32, y: i32) -> bool {
        self.edge_at(x, y).is_some()
    }
}

#[derive(Clone, Copy)]
struct HeightDrag {
    area: ResizeArea,
    start_y: i32,
    last_height: f32,
    top_edge: bool,
    fixed_bottom: Option<i32>,
}

impl HeightDrag {
    fn height_at(self, y: i32) -> f32 {
        let delta = (y - self.start_y) as f32 / self.area.scale;
        let initial = self.area.surface.height() as f32 / self.area.scale;
        (initial + if self.top_edge { -delta } else { delta })
            .clamp(self.area.minimum, self.area.maximum)
    }
}

pub(crate) fn set_resize_area(area: Option<ResizeArea>) {
    RESIZE_AREA.set(area);
}

fn pointer(hwnd: HWND) -> (i32, i32, i32) {
    let mut point = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut point) };
    let rect = window_rect(hwnd);
    (point.x - rect.left, point.y - rect.top, point.y)
}

fn publish_height(drag: HeightDrag, finished: bool) {
    crate::popup_window::send_command(crate::popup_window::PopupCommand::ResizeHeight {
        height_dip: drag.last_height,
        finished,
        fixed_bottom: drag.fixed_bottom,
    });
}

fn finish_height_drag() {
    if let Some(drag) = HEIGHT_DRAG.take() {
        crate::popup::set_dragging(false);
        publish_height(drag, true);
    }
}

pub(crate) fn set_drag_header(rect: Option<Rect>) {
    DRAG_HEADER.set(rect);
}

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
    /// The Windows taskbar on this monitor, when it has one. Other appbars
    /// (e.g. the PowerToys Command Palette dock) also shrink `work`, so only
    /// this rectangle tells which edge the taskbar is on.
    pub(crate) taskbar: Option<Rect>,
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
        WM_SETCURSOR => {
            let (x, y, _) = pointer(hwnd);
            if HEIGHT_DRAG.get().is_some()
                || RESIZE_AREA
                    .get()
                    .is_some_and(|area| area.contains_edge(x, y))
            {
                unsafe { SetCursor(LoadCursorW(std::ptr::null_mut(), IDC_SIZENS)) };
                return 1;
            }
            unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
        }
        WM_LBUTTONDOWN => {
            let (x, y, screen_y) = pointer(hwnd);
            if let Some(area) = RESIZE_AREA.get().filter(|area| area.contains_edge(x, y)) {
                let top_edge = area.edge_at(x, y) == Some(true);
                HEIGHT_DRAG.set(Some(HeightDrag {
                    area,
                    start_y: screen_y,
                    last_height: area.surface.height() as f32 / area.scale,
                    top_edge,
                    fixed_bottom: (area.floating && top_edge)
                        .then(|| window_rect(hwnd).top + area.surface.bottom),
                }));
                crate::popup::set_dragging(true);
                unsafe { SetCapture(hwnd) };
                return 0;
            }
            unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
        }
        WM_MOUSEMOVE if HEIGHT_DRAG.get().is_some() => {
            let (_, _, y) = pointer(hwnd);
            if let Some(mut drag) = HEIGHT_DRAG.get() {
                drag.last_height = drag.height_at(y);
                HEIGHT_DRAG.set(Some(drag));
                publish_height(drag, false);
            }
            0
        }
        WM_LBUTTONUP if HEIGHT_DRAG.get().is_some() => {
            let (_, _, y) = pointer(hwnd);
            if let Some(mut drag) = HEIGHT_DRAG.get() {
                drag.last_height = drag.height_at(y);
                HEIGHT_DRAG.set(Some(drag));
            }
            finish_height_drag();
            unsafe { ReleaseCapture() };
            0
        }
        WM_CAPTURECHANGED => {
            finish_height_drag();
            unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
        }
        crate::app_ids::SHOW_MESSAGE => {
            crate::popup::show_near_cursor();
            0
        }
        // The client area is the entire window: no frame, no resize border,
        // no 1px Win11 top inset. GPUI then lays out over the exact HWND
        // rectangle that the region and outside-click math use.
        WM_NCCALCSIZE => 0,
        WM_NCHITTEST => {
            let origin = window_rect(hwnd);
            let x = (lparam as u32 as u16 as i16) as i32 - origin.left;
            let y = ((lparam as u32 >> 16) as u16 as i16) as i32 - origin.top;
            if RESIZE_AREA
                .get()
                .is_some_and(|area| area.contains_edge(x, y))
            {
                return HTCLIENT as LRESULT;
            }
            if DRAG_HEADER
                .get()
                .is_some_and(|r| x >= r.left && x < r.right && y >= r.top && y < r.bottom)
            {
                HTCAPTION as LRESULT
            } else {
                HTCLIENT as LRESULT
            }
        }
        // A double click on the drag grip must never maximize a tray panel.
        WM_NCLBUTTONDBLCLK => 0,
        WM_ENTERSIZEMOVE => {
            MOVING.set(true);
            crate::popup::set_dragging(true);
            unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
        }
        WM_EXITSIZEMOVE => {
            MOVING.set(false);
            crate::popup::set_dragging(false);
            crate::popup_window::send_command(crate::popup_window::PopupCommand::Moved);
            unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
        }
        WM_DPICHANGED => {
            let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
            if !MOVING.get() {
                crate::popup_window::send_command(crate::popup_window::PopupCommand::Reposition);
            }
            result
        }
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
    POPUP_HWND.set(hwnd);
    unsafe {
        SetWindowSubclass(hwnd, Some(popup_subclass_proc), SUBCLASS_ID, 0);
        let title = crate::win32::wide(crate::app_ids::POPUP_TITLE);
        SetWindowTextW(hwnd, title.as_ptr());
    }
    restyle(hwnd);
    unsafe {
        // The popup can stay open for a long time (e.g. as the live preview
        // beside Settings). Another topmost window that takes the foreground
        // lands above it, so re-raise whenever a foreign window activates.
        // Out-of-context hooks run on this (GPUI) thread's message loop.
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            std::ptr::null_mut(),
            Some(foreground_changed),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
    }
}

unsafe extern "system" fn toast_subclass_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    match message {
        WM_NCCALCSIZE => 0,
        WM_NCHITTEST => HTCLIENT as LRESULT,
        // Clicking a notification never steals focus from the user's app.
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        _ => unsafe { DefSubclassProc(hwnd, message, wparam, lparam) },
    }
}

/// One-time restyle of the notification host: the popup's borderless,
/// topmost tool window, which additionally never activates.
pub(crate) fn configure_toast(hwnd: HWND) {
    unsafe {
        SetWindowSubclass(hwnd, Some(toast_subclass_proc), SUBCLASS_ID, 0);
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, (ex_style | WS_EX_NOACTIVATE) as _);
    }
    restyle(hwnd);
}

/// Borderless, topmost tool window that DWM neither rounds nor animates.
/// Outside any GPUI borrow only (`SWP_FRAMECHANGED` reaches GPUI's WM_SIZE).
fn restyle(hwnd: HWND) {
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let style = (style
            & !(WS_CAPTION | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU))
            | WS_POPUP;
        SetWindowLongPtrW(hwnd, GWL_STYLE, style as _);
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let ex_style = (ex_style & !(WS_EX_APPWINDOW | WS_EX_LAYERED))
            | WS_EX_TOOLWINDOW
            | WS_EX_TOPMOST
            | WS_EX_NOREDIRECTIONBITMAP;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style as _);

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

unsafe extern "system" fn foreground_changed(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    // SetWindowPos re-enters GPUI; defer to the foreground command task.
    if crate::popup::is_visible() {
        if !crate::popup::is_pinned() {
            // The tray owns its toggle. Closing here first can make the tray
            // click interpret the closing popup as a request to reopen it.
            let mut class = [0u16; 64];
            let len = unsafe { GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32) }.max(0)
                as usize;
            if crate::win32::TASKBAR_CLASSES
                .iter()
                .any(|name| name.encode_utf16().eq(class[..len].iter().copied()))
            {
                return;
            }
            crate::popup::hide();
        } else {
            crate::popup_window::send_command(crate::popup_window::PopupCommand::Raise);
        }
    }
}

/// Restore the visible popup to the top of the topmost band. Outside any GPUI
/// borrow only.
pub(crate) fn raise(hwnd: HWND) {
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOOWNERZORDER,
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
            taskbar: taskbar_on(monitor),
            dpi: monitor_dpi(monitor),
        }
    }
}

fn monitor_name(handle: HMONITOR) -> String {
    let mut info: MONITORINFOEXW = unsafe { std::mem::zeroed() };
    info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    unsafe { GetMonitorInfoW(handle, &mut info as *mut _ as *mut MONITORINFO) };
    let len = info
        .szDevice
        .iter()
        .position(|c| *c == 0)
        .unwrap_or(info.szDevice.len());
    String::from_utf16_lossy(&info.szDevice[..len])
}

pub(crate) fn save_position(x: i32, y: i32) -> crate::settings::PopupPosition {
    let monitor = monitor_for_point(x, y);
    let handle = unsafe { MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST) };
    crate::settings::PopupPosition {
        monitor: monitor_name(handle),
        x_dip: (f64::from(x - monitor.work.left) / monitor.scale()).round() as i32,
        y_dip: (f64::from(y - monitor.work.top) / monitor.scale()).round() as i32,
        screen_x: x,
        screen_y: y,
    }
}

pub(crate) fn restore_position(position: &crate::settings::PopupPosition) -> (Monitor, i32, i32) {
    // Restore the exact saved screen origin. Monitor/work-area changes must
    // not silently reposition a normal floating window.
    (
        monitor_for_point(position.screen_x, position.screen_y),
        position.screen_x,
        position.screen_y,
    )
}

/// The frame of the primary or secondary taskbar that lives on `monitor`.
fn taskbar_on(monitor: HMONITOR) -> Option<Rect> {
    for class in crate::win32::TASKBAR_CLASSES {
        let class = crate::win32::wide(class);
        let mut after: HWND = std::ptr::null_mut();
        loop {
            let taskbar = unsafe {
                FindWindowExW(
                    std::ptr::null_mut(),
                    after,
                    class.as_ptr(),
                    std::ptr::null(),
                )
            };
            if taskbar.is_null() {
                break;
            }
            after = taskbar;
            if unsafe { MonitorFromWindow(taskbar, MONITOR_DEFAULTTONULL) } != monitor {
                continue;
            }
            let rect = window_rect(taskbar);
            if rect.width() > 0 && rect.height() > 0 {
                return Some(rect);
            }
        }
    }
    None
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
    set_resize_area(None);
    if HEIGHT_DRAG.get().is_some() {
        finish_height_drag();
        unsafe { ReleaseCapture() };
    }
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }
}

/// Release the full-size swap-chain and path/MSAA surfaces while hidden.
/// Show restores host_rect before the first visible frame. Resize outside
/// any GPUI borrow: WM_SIZE synchronously enters its renderer.
pub(crate) fn park_hidden(hwnd: HWND) {
    let origin = window_rect(hwnd);
    place_keep_order(
        hwnd,
        Rect {
            left: origin.left,
            top: origin.top,
            right: origin.left + 1,
            bottom: origin.top + 1,
        },
    );
}

/// A tray click authorizes foreground activation for this process.
pub(crate) fn activate(hwnd: HWND) {
    unsafe {
        let _ = SetForegroundWindow(hwnd);
    }
}

/// Coarse visibility/input envelope around `rect` (window-relative pixels).
/// GPUI and the composition backdrop paint the actual antialiased outline.
/// Keep this binary GDI clip outside their edge coverage, otherwise it chops
/// partially transparent corner pixels into a staircase.
/// `None` hides every pixel while keeping the compositor surface alive.
///
/// `SetWindowRgn` only sends position-change notifications without moving or
/// sizing the window, which GPUI ignores, so it is safe during a frame.
pub(crate) fn set_region(hwnd: HWND, rect: Option<Rect>, radius_px: i32) {
    // Physical pixels, independent of display scaling. Two cover the renderer's
    // antialias fringe plus the half-pixel rounding of animated capsule bounds.
    const AA_PADDING_PX: i32 = 2;
    unsafe {
        let region = match rect.filter(|rect| rect.width() > 0 && rect.height() > 0) {
            None => CreateRectRgn(0, 0, 0, 0),
            Some(rect) if radius_px <= 0 => CreateRectRgn(
                rect.left - AA_PADDING_PX,
                rect.top - AA_PADDING_PX,
                rect.right + AA_PADDING_PX,
                rect.bottom + AA_PADDING_PX,
            ),
            Some(rect) => {
                // Grow radius with bounds to keep the corner centers fixed.
                let arc = radius_px.saturating_add(AA_PADDING_PX).saturating_mul(2);
                // GDI round regions exclude the right/bottom edge; extend by
                // one pixel so the capsule keeps its full size.
                CreateRoundRectRgn(
                    rect.left - AA_PADDING_PX,
                    rect.top - AA_PADDING_PX,
                    rect.right + AA_PADDING_PX + 1,
                    rect.bottom + AA_PADDING_PX + 1,
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

#[cfg(test)]
mod resize_tests {
    use super::*;

    fn area(top_anchored: bool, scale: f32) -> ResizeArea {
        ResizeArea {
            surface: Rect {
                left: 10,
                top: 100,
                right: 410,
                bottom: 500,
            },
            top_anchored,
            floating: false,
            scale,
            minimum: 160.0,
            maximum: 1080.0 / scale,
        }
    }

    #[test]
    fn resize_hit_band_follows_the_free_edge_for_top_and_bottom_popups() {
        for top_anchored in [false, true] {
            let area = area(top_anchored, 1.0);
            assert_eq!(area.contains_edge(100, 102), !top_anchored);
            assert_eq!(area.contains_edge(100, 498), top_anchored);
            assert!(!area.contains_edge(100, 300));
            assert!(!area.contains_edge(10, if top_anchored { 498 } else { 102 }));
        }
    }

    #[test]
    fn resize_direction_matches_the_fixed_edge() {
        for top_anchored in [false, true] {
            let drag = HeightDrag {
                area: area(top_anchored, 1.0),
                start_y: 400,
                last_height: 400.0,
                top_edge: !top_anchored,
                fixed_bottom: None,
            };
            assert_eq!(
                drag.height_at(500),
                if top_anchored { 500.0 } else { 300.0 }
            );
            assert_eq!(
                drag.height_at(300),
                if top_anchored { 300.0 } else { 500.0 }
            );
        }
    }

    #[test]
    fn resize_uses_dips_and_caps_at_full_screen_height() {
        let drag = HeightDrag {
            area: area(true, 2.0),
            start_y: -500,
            last_height: 200.0,
            top_edge: false,
            fixed_bottom: None,
        };
        assert_eq!(drag.height_at(-300), 300.0);
        assert_eq!(drag.height_at(5000), 540.0);
        assert_eq!(drag.height_at(-5000), 160.0);
    }

    #[test]
    fn floating_panel_exposes_both_resize_edges_and_respects_content_cap() {
        let area = ResizeArea {
            floating: true,
            maximum: 450.0,
            ..area(true, 1.0)
        };
        assert_eq!(area.edge_at(100, 102), Some(true));
        assert_eq!(area.edge_at(100, 498), Some(false));
        for top_edge in [false, true] {
            let drag = HeightDrag {
                area,
                start_y: 400,
                last_height: 400.0,
                top_edge,
                fixed_bottom: top_edge.then_some(700),
            };
            assert_eq!(drag.height_at(if top_edge { -5000 } else { 5000 }), 450.0);
            assert_eq!(drag.height_at(if top_edge { 5000 } else { -5000 }), 160.0);
        }
    }
}
