//! Native Win32 helpers that make the WinUI window behave like a tray popup.
//!
//! The WinUI window is parked off-screen instead of being closed. Closing it
//! would trigger `windows-reactor`'s `Closed -> process::exit` handler.
//!
//! Sizing is owned by WinUI (`inner_size` / constraints). This module only
//! moves the HWND — never resizes it — so DPI and layout stay in sync.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicIsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use windows_sys::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::{
        Dwm::{
            DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_EXTENDED_FRAME_BOUNDS,
            DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMSBT_NONE,
            DWMSBT_TRANSIENTWINDOW, DwmExtendFrameIntoClientArea, DwmFlush,
            DwmGetWindowAttribute, DwmSetWindowAttribute,
        },
        Gdi::{
            CreateRectRgn, GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO,
            MonitorFromPoint, SetWindowRgn,
        },
    },
    UI::{
        Controls::MARGINS,
        HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON},
        WindowsAndMessaging::{
            DispatchMessageW, FindWindowW, GWL_EXSTYLE, GWL_STYLE, GetCursorPos, GetWindowLongW,
            GetWindowRect, HWND_TOPMOST, MSG, PM_REMOVE, PeekMessageW, SetForegroundWindow,
            SetWindowLongW, SetWindowPos, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
            SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, TranslateMessage, WS_CAPTION,
            WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
            WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
        },
    },
};

const WINDOW_TITLE: &str = "Codex Minibar";
const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
const DWMWCP_ROUND: u32 = 2;
const SHOW_GRACE_MS: i64 = 450;
/// Popup client size in DIP — must match `App::inner_size` / content stack.
pub const POPUP_WIDTH: i32 = 380;
pub const POPUP_HEIGHT: i32 = 434;
const PARKED_X: i32 = -32_000;
const PARKED_Y: i32 = -32_000;
/// 30 compositor-synchronised frames at 60 Hz ≈ 500 ms.
const ANIMATION_STEPS: i32 = 30;
/// Final inset from the target monitor's right/bottom work-area edges.
const EDGE_MARGIN: i32 = 20;

static HWND_BITS: AtomicIsize = AtomicIsize::new(0);
static CONFIGURED: AtomicBool = AtomicBool::new(false);
static POPUP_VISIBLE: AtomicBool = AtomicBool::new(false);
static BUTTON_WAS_DOWN: AtomicBool = AtomicBool::new(false);
static IGNORE_OUTSIDE_UNTIL_MS: AtomicI64 = AtomicI64::new(0);
static WORK_LEFT: AtomicI32 = AtomicI32::new(0);
static WORK_TOP: AtomicI32 = AtomicI32::new(0);
static WORK_RIGHT: AtomicI32 = AtomicI32::new(0);
static WORK_BOTTOM: AtomicI32 = AtomicI32::new(0);

fn store_work_area(work: RECT) {
    WORK_LEFT.store(work.left, Ordering::SeqCst);
    WORK_TOP.store(work.top, Ordering::SeqCst);
    WORK_RIGHT.store(work.right, Ordering::SeqCst);
    WORK_BOTTOM.store(work.bottom, Ordering::SeqCst);
}

fn loaded_work_area() -> RECT {
    RECT {
        left: WORK_LEFT.load(Ordering::SeqCst),
        top: WORK_TOP.load(Ordering::SeqCst),
        right: WORK_RIGHT.load(Ordering::SeqCst),
        bottom: WORK_BOTTOM.load(Ordering::SeqCst),
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

/// WinUI-like ease-in-out: soft departure, quick middle, gentle arrival.
fn ease_in_out_cubic(progress: f64) -> f64 {
    if progress < 0.5 {
        4.0 * progress.powi(3)
    } else {
        1.0 - (-2.0 * progress + 2.0).powi(3) / 2.0
    }
}

fn encode_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn find_hwnd() -> Option<HWND> {
    let title = encode_wide(WINDOW_TITLE);
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    if hwnd.is_null() {
        None
    } else {
        Some(hwnd)
    }
}

fn current_hwnd() -> Option<HWND> {
    let bits = HWND_BITS.load(Ordering::SeqCst);
    if bits == 0 {
        find_hwnd()
    } else {
        Some(bits as HWND)
    }
}

fn park(hwnd: HWND) {
    unsafe {
        SetWindowRgn(hwnd, std::ptr::null_mut(), 0);
        // Park off-screen. Never touch size — WinUI owns width/height.
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            PARKED_X,
            PARKED_Y,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW,
        );
    }
    POPUP_VISIBLE.store(false, Ordering::SeqCst);
}

fn window_rect(hwnd: HWND) -> RECT {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe {
        GetWindowRect(hwnd, &mut rect);
    }
    rect
}

/// Visible frame according to DWM — more reliable than GetWindowRect when
/// WinUI/DWM disagree about the painted bounds.
fn frame_bounds(hwnd: HWND) -> RECT {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let hr = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS as u32,
            &mut rect as *mut RECT as *mut _,
            size_of::<RECT>() as u32,
        )
    };
    if hr == 0 && rect.right > rect.left && rect.bottom > rect.top {
        rect
    } else {
        window_rect(hwnd)
    }
}

fn monitor_dpi(monitor: HMONITOR) -> u32 {
    let mut dpi_x = 0u32;
    let mut dpi_y = 0u32;
    let ok = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    if ok == 0 && dpi_x > 0 {
        dpi_x
    } else {
        96
    }
}

/// Estimate outer size without ever writing it back through SetWindowPos.
fn popup_pixel_size(hwnd: HWND, monitor: HMONITOR) -> (i32, i32) {
    let rect = frame_bounds(hwnd);
    let measured_w = (rect.right - rect.left).abs();
    let measured_h = (rect.bottom - rect.top).abs();

    let dpi = monitor_dpi(monitor);
    let expected_w = (i64::from(POPUP_WIDTH) * i64::from(dpi) / 96) as i32;
    let expected_h = (i64::from(POPUP_HEIGHT) * i64::from(dpi) / 96) as i32;

    (measured_w.max(expected_w).max(1), measured_h.max(expected_h).max(1))
}

fn move_hwnd(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_SHOWWINDOW,
        );
    }
}

/// Shift the HWND so its visible frame sits inside `work` with EDGE_MARGIN.
/// Never resizes — WinUI owns width/height; resizing here caused the window to
/// grow back past the monitor edge.
fn clamp_frame_to_work(hwnd: HWND, work: RECT) {
    let frame = frame_bounds(hwnd);
    let width = (frame.right - frame.left).max(1);
    let height = (frame.bottom - frame.top).max(1);

    let mut x = work.right - width - EDGE_MARGIN;
    let mut y = work.bottom - height - EDGE_MARGIN;
    x = x.max(work.left + EDGE_MARGIN);
    y = y.max(work.top + EDGE_MARGIN);

    // Prefer keeping the right/bottom edges on-monitor even if that means
    // overlapping the left/top margin on a tiny work area.
    if x + width > work.right - EDGE_MARGIN {
        x = work.right - width - EDGE_MARGIN;
    }
    if y + height > work.bottom - EDGE_MARGIN {
        y = work.bottom - height - EDGE_MARGIN;
    }

    let current = window_rect(hwnd);
    // frame vs window origin can differ; move by the frame delta.
    let dx = x - frame.left;
    let dy = y - frame.top;
    if dx != 0 || dy != 0 {
        move_hwnd(hwnd, current.left + dx, current.top + dy);
    }
}

/// Clip painted pixels to the work area. DWM Acrylic ignores this — disable
/// the backdrop while the clip is active.
fn clip_to_work_area(hwnd: HWND, work: RECT) {
    let window = window_rect(hwnd);
    let width = (window.right - window.left).max(0);
    let height = (window.bottom - window.top).max(0);

    let left = (work.left - window.left).clamp(0, width);
    let top = (work.top - window.top).clamp(0, height);
    let right = (work.right - window.left).clamp(left, width);
    let bottom = (work.bottom - window.top).clamp(top, height);

    unsafe {
        let region = CreateRectRgn(left, top, right, bottom);
        SetWindowRgn(hwnd, region, 1);
    }
}

fn clear_window_clip(hwnd: HWND) {
    unsafe {
        SetWindowRgn(hwnd, std::ptr::null_mut(), 1);
    }
}

fn set_system_backdrop(hwnd: HWND, backdrop: i32) {
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE as u32,
            &backdrop as *const i32 as *const _,
            size_of::<i32>() as u32,
        );
    }
}

fn set_frame_margins(hwnd: HWND, fill: bool) {
    let margins = if fill {
        MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        }
    } else {
        MARGINS {
            cxLeftWidth: 0,
            cxRightWidth: 0,
            cyTopHeight: 0,
            cyBottomHeight: 0,
        }
    };
    unsafe {
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
    }
}

/// Prefer the monitor that owns the tray click; if that point sits on a shared
/// edge, keep the primary-display work area so the popup never opens on the
/// neighbor.
fn resolve_work_area(anchor_x: i32, anchor_y: i32) -> (HMONITOR, RECT) {
    unsafe {
        let monitor = MonitorFromPoint(
            POINT {
                x: anchor_x,
                y: anchor_y,
            },
            MONITOR_DEFAULTTONEAREST,
        );
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            rcMonitor: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            rcWork: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            dwFlags: 0,
        };
        GetMonitorInfoW(monitor, &mut info);

        // If the click is exactly on the right edge, NEAREST can pick the
        // display to the right. Pull back one pixel and resolve again.
        if anchor_x >= info.rcMonitor.right.saturating_sub(1) {
            let inward = MonitorFromPoint(
                POINT {
                    x: anchor_x.saturating_sub(2),
                    y: anchor_y,
                },
                MONITOR_DEFAULTTONEAREST,
            );
            let mut inward_info = info;
            inward_info.cbSize = size_of::<MONITORINFO>() as u32;
            GetMonitorInfoW(inward, &mut inward_info);
            return (inward, inward_info.rcWork);
        }

        (monitor, info.rcWork)
    }
}

/// Apply Acrylic so the backdrop blurs whatever is behind the popup.
/// WinUI `DesktopAcrylicBackdrop` owns the tint; DWM uses TRANSIENTWINDOW Acrylic.
fn apply_acrylic(hwnd: HWND) {
    unsafe {
        let dark_mode = 1u32;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            &dark_mode as *const u32 as *const _,
            size_of::<u32>() as u32,
        );
    }
    set_system_backdrop(hwnd, DWMSBT_TRANSIENTWINDOW);
    set_frame_margins(hwnd, true);
}

/// Find the WinUI window, restyle it as a tool popup, and park it off-screen.
pub fn ensure_configured() -> Option<HWND> {
    let hwnd = find_hwnd()?;
    HWND_BITS.store(hwnd as isize, Ordering::SeqCst);
    if CONFIGURED.swap(true, Ordering::SeqCst) {
        return Some(hwnd);
    }

    unsafe {
        // Strip chrome, but do NOT force WS_POPUP — that breaks WinUI SystemBackdrop.
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        let style =
            style & !(WS_CAPTION | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU);
        SetWindowLongW(hwnd, GWL_STYLE, style as i32);

        // Tool/topmost popup shell. Avoid layered alpha and permanent no-activate:
        // both force solid backdrop fallbacks.
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        let ex_style = (ex_style & !(WS_EX_APPWINDOW | WS_EX_LAYERED | WS_EX_NOACTIVATE))
            | WS_EX_TOOLWINDOW
            | WS_EX_TOPMOST;
        SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style as i32);

        let corner = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const u32 as *const _,
            size_of::<u32>() as u32,
        );

        let border_color = DWMWA_COLOR_NONE;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR as u32,
            &border_color as *const u32 as *const _,
            size_of::<u32>() as u32,
        );

        apply_acrylic(hwnd);

        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOSIZE | SWP_NOMOVE,
        );
    }

    park(hwnd);
    Some(hwnd)
}

/// Dispatch messages for the thread that owns the tray icon.
pub fn pump_messages() {
    unsafe {
        let mut message = std::mem::zeroed::<MSG>();
        while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

pub fn is_visible() -> bool {
    POPUP_VISIBLE.load(Ordering::SeqCst)
}

pub fn hide() {
    let Some(hwnd) = current_hwnd() else {
        return;
    };
    park(hwnd);
}

/// Re-clamp if WinUI grows/moves the HWND past the stored work area.
pub fn keep_on_monitor() {
    if !is_visible() {
        return;
    }
    let Some(hwnd) = current_hwnd() else {
        return;
    };

    let work = loaded_work_area();
    if work.right <= work.left || work.bottom <= work.top {
        return;
    }

    let frame = frame_bounds(hwnd);
    let overflow_x = frame.right - (work.right - EDGE_MARGIN);
    let overflow_y = frame.bottom - (work.bottom - EDGE_MARGIN);
    if overflow_x <= 0 && overflow_y <= 0 {
        return;
    }

    let current = window_rect(hwnd);
    move_hwnd(
        hwnd,
        current.left - overflow_x.max(0),
        current.top - overflow_y.max(0),
    );
}

/// Show the popup near the tray click, anchored above the taskbar.
pub fn show_near(anchor_x: i32, anchor_y: i32) {
    let Some(hwnd) = ensure_configured() else {
        return;
    };

    let (monitor, work) = resolve_work_area(anchor_x, anchor_y);
    store_work_area(work);

    unsafe {
        // Acrylic ignores SetWindowRgn — kill it for the slide, restore after.
        set_system_backdrop(hwnd, DWMSBT_NONE);
        set_frame_margins(hwnd, false);

        let (width, height) = popup_pixel_size(hwnd, monitor);
        let target_x = work.right - width - EDGE_MARGIN;
        let target_y = work.bottom - height - EDGE_MARGIN;
        let start_x = work.right;

        move_hwnd(hwnd, start_x, target_y);
        clip_to_work_area(hwnd, work);
        let _ = SetForegroundWindow(hwnd);

        for step in 1..=ANIMATION_STEPS {
            let progress = f64::from(step) / f64::from(ANIMATION_STEPS);
            let eased = ease_in_out_cubic(progress);
            let animated_x =
                start_x - ((f64::from(start_x - target_x) * eased).round() as i32);
            move_hwnd(hwnd, animated_x, target_y);
            clip_to_work_area(hwnd, work);
            let _ = DwmFlush();
        }

        clear_window_clip(hwnd);
        apply_acrylic(hwnd);
        let _ = DwmFlush();
    }

    // WinUI may still be settling size — clamp from the live DWM frame, twice.
    clamp_frame_to_work(hwnd, work);
    unsafe {
        let _ = DwmFlush();
    }
    clamp_frame_to_work(hwnd, work);

    POPUP_VISIBLE.store(true, Ordering::SeqCst);
    IGNORE_OUTSIDE_UNTIL_MS.store(now_ms() + SHOW_GRACE_MS, Ordering::SeqCst);
    BUTTON_WAS_DOWN.store(true, Ordering::SeqCst);
}

pub fn toggle_near(anchor_x: i32, anchor_y: i32) {
    if is_visible() {
        hide();
    } else {
        show_near(anchor_x, anchor_y);
    }
}

/// Detect a new mouse press that lands outside the popup.
pub fn clicked_outside() -> bool {
    if !is_visible() || now_ms() < IGNORE_OUTSIDE_UNTIL_MS.load(Ordering::SeqCst) {
        let button_is_down = unsafe {
            [VK_LBUTTON, VK_MBUTTON, VK_RBUTTON]
                .into_iter()
                .any(|button| GetAsyncKeyState(button as i32) < 0)
        };
        BUTTON_WAS_DOWN.store(button_is_down, Ordering::SeqCst);
        return false;
    }
    let Some(hwnd) = current_hwnd() else {
        return false;
    };

    let button_is_down = unsafe {
        [VK_LBUTTON, VK_MBUTTON, VK_RBUTTON]
            .into_iter()
            .any(|button| GetAsyncKeyState(button as i32) < 0)
    };
    let was_down = BUTTON_WAS_DOWN.swap(button_is_down, Ordering::SeqCst);
    let is_new_click = button_is_down && !was_down;
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
        GetWindowRect(hwnd, &mut popup);
        cursor.x < popup.left
            || cursor.x >= popup.right
            || cursor.y < popup.top
            || cursor.y >= popup.bottom
    }
}
