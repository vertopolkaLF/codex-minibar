//! Native Win32 helpers that make the WinUI window behave like a tray popup.
//!
//! The WinUI window is parked off-screen instead of being closed. Closing it
//! would trigger `windows-reactor`'s `Closed -> process::exit` handler.

use std::sync::atomic::{AtomicBool, AtomicI64, AtomicIsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use windows_sys::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::{
        Dwm::{
            DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_SYSTEMBACKDROP_TYPE,
            DWMWA_USE_IMMERSIVE_DARK_MODE, DWMSBT_TRANSIENTWINDOW, DwmExtendFrameIntoClientArea,
            DwmFlush, DwmSetWindowAttribute,
        },
        Gdi::{
            GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
        },
    },
    UI::{
        Controls::MARGINS,
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON},
        WindowsAndMessaging::{
            DispatchMessageW, FindWindowW, GWL_EXSTYLE, GWL_STYLE, GetCursorPos, GetWindowLongW,
            GetWindowRect, HWND_TOPMOST, MSG, PM_REMOVE, PeekMessageW, SetForegroundWindow,
            SetWindowLongW, SetWindowPos, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOZORDER,
            SWP_SHOWWINDOW, TranslateMessage, WS_CAPTION, WS_EX_APPWINDOW, WS_EX_LAYERED,
            WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
            WS_SYSMENU, WS_THICKFRAME,
        },
    },
};

const WINDOW_TITLE: &str = "Codex Minibar";
const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
const DWMWCP_ROUND: u32 = 2;
const SHOW_GRACE_MS: i64 = 450;
const POPUP_WIDTH: i32 = 380;
const POPUP_HEIGHT: i32 = 460;
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
        // Park off-screen. Do not use layered alpha — it breaks system backdrops.
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            PARKED_X,
            PARKED_Y,
            POPUP_WIDTH,
            POPUP_HEIGHT,
            SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW,
        );
    }
    POPUP_VISIBLE.store(false, Ordering::SeqCst);
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

        // TRANSIENTWINDOW = Acrylic blur for popups/flyouts.
        let backdrop = DWMSBT_TRANSIENTWINDOW;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE as u32,
            &backdrop as *const i32 as *const _,
            size_of::<i32>() as u32,
        );

        // Let DWM draw the backdrop into the full client area.
        let margins = MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        };
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
    }
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
            SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOZORDER,
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

/// Show the popup near the tray click, anchored above the cursor.
pub fn show_near(anchor_x: i32, anchor_y: i32) {
    let Some(hwnd) = ensure_configured() else {
        return;
    };

    unsafe {
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        GetWindowRect(hwnd, &mut rect);
        let width = (rect.right - rect.left).abs().max(POPUP_WIDTH);
        let height = (rect.bottom - rect.top).abs().max(POPUP_HEIGHT);
        let monitor = MonitorFromPoint(
            POINT {
                x: anchor_x,
                y: anchor_y,
            },
            MONITOR_DEFAULTTONEAREST,
        );
        let mut monitor_info = MONITORINFO {
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
        GetMonitorInfoW(monitor, &mut monitor_info);

        // Anchor to the bottom-right of the monitor's work area. `rcWork`
        // excludes the taskbar, so the popup always sits directly above it.
        let y = (monitor_info.rcWork.bottom - height - EDGE_MARGIN)
            .max(monitor_info.rcWork.top + EDGE_MARGIN);

        // Start beyond the monitor's right edge, then slide left.
        // Activate so Acrylic uses the live blur path, not the solid fallback.
        let target_x = (monitor_info.rcWork.right - width - EDGE_MARGIN)
            .max(monitor_info.rcWork.left + EDGE_MARGIN);
        let start_x = monitor_info.rcWork.right;
        apply_acrylic(hwnd);
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            start_x,
            y,
            width,
            height,
            SWP_SHOWWINDOW,
        );
        let _ = SetForegroundWindow(hwnd);

        for step in 1..=ANIMATION_STEPS {
            let progress = f64::from(step) / f64::from(ANIMATION_STEPS);
            let eased = ease_in_out_cubic(progress);
            let animated_x =
                start_x - ((f64::from(start_x - target_x) * eased).round() as i32);
            SetWindowPos(hwnd, HWND_TOPMOST, animated_x, y, width, height, 0);
            // Synchronize each property update with the DWM compositor rather
            // than relying on a coarse `thread::sleep` timer.
            let _ = DwmFlush();
        }
    }

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
