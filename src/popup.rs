//! Native Win32 helpers that make the WinUI window behave like a tray popup.
//!
//! The WinUI window is parked off-screen instead of being closed. Closing it
//! would trigger `windows-reactor`'s `Closed -> process::exit` handler.
//!
//! Width is fixed; height is updated via [`set_client_height_dip`] when popup
//! content changes. Positioning still never fights WinUI layout — we only
//! move (and occasionally resize) the HWND.

use std::{
    cell::RefCell,
    rc::Rc,
    sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicIsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use windows_reactor::ReactorHost;
use windows_sys::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::{
        Dwm::{
            DWMSBT_NONE, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_EXTENDED_FRAME_BOUNDS,
            DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE, DwmExtendFrameIntoClientArea,
            DwmFlush, DwmGetWindowAttribute, DwmSetWindowAttribute,
        },
        Gdi::{
            CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, GetMonitorInfoW, HMONITOR,
            MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, RGN_AND, SetWindowRgn,
        },
    },
    UI::{
        Controls::MARGINS,
        HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON},
        WindowsAndMessaging::{
            CS_DROPSHADOW, DispatchMessageW, FindWindowW, GCL_STYLE, GWL_EXSTYLE, GWL_STYLE,
            GetClassLongPtrW, GetCursorPos, GetWindowLongW, GetWindowRect, HWND_TOPMOST, MSG,
            PM_REMOVE, PeekMessageW, SWP_FRAMECHANGED, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE,
            SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetClassLongPtrW, SetForegroundWindow,
            SetWindowLongW, SetWindowPos, TranslateMessage, WS_CAPTION, WS_EX_APPWINDOW,
            WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_MAXIMIZEBOX,
            WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
        },
    },
};

const WINDOW_TITLE: &str = "Codex Minibar";
const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
/// No DWM rounded-frame (its drop shadow always spills past the monitor seam).
/// Shape comes from `CreateRoundRectRgn` + matching XAML `corner_radius`.
const DWMWCP_DONOTROUND: u32 = 1;
const SHOW_GRACE_MS: i64 = 450;
/// Popup client width in DIP — fixed; height adapts to content.
pub const POPUP_WIDTH: i32 = 380;
/// Layout pieces used by [`height_for`] (must stay in sync with `popup_window`).
const LIMIT_CARD_HEIGHT: i32 = 82;
const META_ROW_HEIGHT: i32 = 22;
const BODY_PAD_Y: i32 = 36; // top 16 + bottom 20
const BODY_SPACING: i32 = 12;
const FOOTER_HEIGHT: i32 = 61; // padding + icon row + top border
const CHROME_HEIGHT: i32 = 4; // outer border + inset
/// Baseline height when both limit cards, plan/credits, and footer are shown.
pub const POPUP_HEIGHT: i32 =
    BODY_PAD_Y + LIMIT_CARD_HEIGHT * 2 + META_ROW_HEIGHT + BODY_SPACING * 2 + FOOTER_HEIGHT + CHROME_HEIGHT;
/// Smallest popup: two limit cards + footer (plan/credits row hidden).
pub const POPUP_HEIGHT_MIN: i32 =
    BODY_PAD_Y + LIMIT_CARD_HEIGHT * 2 + BODY_SPACING + FOOTER_HEIGHT + CHROME_HEIGHT;
/// Upper bound for long error InfoBars; keeps OverlappedPresenter flexible.
pub const POPUP_HEIGHT_MAX: i32 = 640;
/// Must match the root XAML `corner_radius` in `app.rs`.
pub const WINDOW_CORNER_RADIUS_DIP: i32 = 8;
const PARKED_X: i32 = -32_000;
const PARKED_Y: i32 = -32_000;
/// 24 compositor-synchronised frames at 60 Hz ≈ 400 ms.
const ANIMATION_STEPS: i32 = 24;
/// Gap from the monitor edge.
const EDGE_MARGIN: i32 = 20;

static HWND_BITS: AtomicIsize = AtomicIsize::new(0);
static CONFIGURED: AtomicBool = AtomicBool::new(false);
static POPUP_VISIBLE: AtomicBool = AtomicBool::new(false);
static BUTTON_WAS_DOWN: AtomicBool = AtomicBool::new(false);
static IGNORE_OUTSIDE_UNTIL_MS: AtomicI64 = AtomicI64::new(0);
/// Current client height in DIP (updated when content changes).
static CLIENT_HEIGHT_DIP: AtomicI32 = AtomicI32::new(POPUP_HEIGHT);
/// Physical monitor bounds (not work area) — right edge is the seam to the next display.
static MONITOR_LEFT: AtomicI32 = AtomicI32::new(0);
static MONITOR_TOP: AtomicI32 = AtomicI32::new(0);
static MONITOR_RIGHT: AtomicI32 = AtomicI32::new(0);
static MONITOR_BOTTOM: AtomicI32 = AtomicI32::new(0);
static WORK_BOTTOM: AtomicI32 = AtomicI32::new(0);
static CORNER_RADIUS_PX: AtomicI32 = AtomicI32::new(WINDOW_CORNER_RADIUS_DIP);

thread_local! {
    static POPUP_HOST: RefCell<Option<Rc<ReactorHost>>> = const { RefCell::new(None) };
}

fn store_bounds(monitor: RECT, work: RECT) {
    MONITOR_LEFT.store(monitor.left, Ordering::SeqCst);
    MONITOR_TOP.store(monitor.top, Ordering::SeqCst);
    MONITOR_RIGHT.store(monitor.right, Ordering::SeqCst);
    MONITOR_BOTTOM.store(monitor.bottom, Ordering::SeqCst);
    // Taskbar sits on the bottom of `work`; keep that for vertical pinning.
    WORK_BOTTOM.store(work.bottom, Ordering::SeqCst);
}

fn loaded_monitor() -> RECT {
    RECT {
        left: MONITOR_LEFT.load(Ordering::SeqCst),
        top: MONITOR_TOP.load(Ordering::SeqCst),
        right: MONITOR_RIGHT.load(Ordering::SeqCst),
        bottom: MONITOR_BOTTOM.load(Ordering::SeqCst),
    }
}

fn loaded_work_bottom() -> i32 {
    WORK_BOTTOM.load(Ordering::SeqCst)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

/// CSS `cubic-bezier(x1, y1, x2, y2)` sample for unit progress in `[0, 1]`.
fn cubic_bezier(x1: f64, y1: f64, x2: f64, y2: f64, progress: f64) -> f64 {
    let x = progress.clamp(0.0, 1.0);
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }

    // Solve cubic Bezier x(t) = x for t via Newton, then sample y(t).
    let mut t = x;
    for _ in 0..8 {
        let u = 1.0 - t;
        let x_t = 3.0 * u * u * t * x1 + 3.0 * u * t * t * x2 + t * t * t;
        let dx = 3.0 * u * u * x1 + 6.0 * u * t * (x2 - x1) + 3.0 * t * t * (1.0 - x2);
        if dx.abs() < 1e-9 {
            break;
        }
        t = (t - (x_t - x) / dx).clamp(0.0, 1.0);
    }

    let u = 1.0 - t;
    3.0 * u * u * t * y1 + 3.0 * u * t * t * y2 + t * t * t
}

/// Popup slide-in easing: `cubic-bezier(1, 0, 0, 1)`.
fn ease_appear(progress: f64) -> f64 {
    cubic_bezier(0.0, 0.5, 0.0, 1.0, progress)
}

fn encode_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn find_hwnd() -> Option<HWND> {
    let title = encode_wide(WINDOW_TITLE);
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    if hwnd.is_null() { None } else { Some(hwnd) }
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
            SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER | SWP_HIDEWINDOW,
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
    if ok == 0 && dpi_x > 0 { dpi_x } else { 96 }
}

/// Client height in DIP for the current popup contents.
pub fn height_for(hide_plan_credits: bool, error: Option<&str>) -> i32 {
    let mut blocks = vec![LIMIT_CARD_HEIGHT, LIMIT_CARD_HEIGHT];
    if !hide_plan_credits {
        blocks.push(META_ROW_HEIGHT);
    }
    if let Some(message) = error {
        blocks.insert(0, info_bar_height(message));
    }
    let spacings = BODY_SPACING * (blocks.len().saturating_sub(1) as i32);
    let height = BODY_PAD_Y + blocks.iter().sum::<i32>() + spacings + FOOTER_HEIGHT + CHROME_HEIGHT;
    height.clamp(POPUP_HEIGHT_MIN, POPUP_HEIGHT_MAX)
}

fn info_bar_height(message: &str) -> i32 {
    // Body padding leaves ~348 DIP; InfoBar chrome eats more, so wrap early.
    const CHARS_PER_LINE: usize = 42;
    let lines = message.chars().count().div_ceil(CHARS_PER_LINE).max(1);
    const BASE: i32 = 48;
    const LINE: i32 = 18;
    BASE + LINE * lines as i32
}

/// Keep the WinUI host so content-driven resizes can call `AppWindow.ResizeClient`.
pub fn register_host(host: Rc<ReactorHost>) {
    let _ = host.relax_height_constraints(f64::from(POPUP_HEIGHT_MAX));
    POPUP_HOST.with(|slot| *slot.borrow_mut() = Some(host));
}

/// Resize from a measured content height (DIPs). Ignores bogus zero-size layout passes.
pub fn set_client_height_from_content(height_dip: f64) {
    if !height_dip.is_finite() || height_dip < 32.0 {
        return;
    }
    set_client_height_dip(height_dip.ceil() as i32);
}

/// Resize the WinUI client to `height_dip` and re-pin if the popup is open.
pub fn set_client_height_dip(height_dip: i32) {
    let height_dip = height_dip.clamp(80, POPUP_HEIGHT_MAX);
    if CLIENT_HEIGHT_DIP.swap(height_dip, Ordering::SeqCst) == height_dip {
        return;
    }
    apply_client_height(height_dip);
}

/// Re-apply size constraints after Win32 chrome is stripped (stale NC metrics).
pub fn sync_host_constraints() {
    POPUP_HOST.with(|slot| {
        if let Some(host) = slot.borrow().as_ref() {
            let _ = host.relax_height_constraints(f64::from(POPUP_HEIGHT_MAX));
            let height = CLIENT_HEIGHT_DIP.load(Ordering::SeqCst);
            let _ = host.resize_client(f64::from(POPUP_WIDTH), f64::from(height));
        }
    });
}

fn apply_client_height(height_dip: i32) {
    POPUP_HOST.with(|slot| {
        if let Some(host) = slot.borrow().as_ref() {
            let _ = host.relax_height_constraints(f64::from(POPUP_HEIGHT_MAX));
            let _ = host.resize_client(f64::from(POPUP_WIDTH), f64::from(height_dip));
        }
    });

    if !is_visible() {
        return;
    }
    let Some(hwnd) = current_hwnd().or_else(find_hwnd) else {
        return;
    };
    HWND_BITS.store(hwnd as isize, Ordering::SeqCst);
    let monitor = loaded_monitor();
    if monitor.right > monitor.left {
        pin_bottom_right(hwnd, monitor, loaded_work_bottom());
    }
}

/// Estimate outer size without ever writing it back through SetWindowPos.
fn popup_pixel_size(hwnd: HWND, monitor: HMONITOR) -> (i32, i32) {
    let rect = frame_bounds(hwnd);
    let measured_w = (rect.right - rect.left).abs();
    let measured_h = (rect.bottom - rect.top).abs();

    let dpi = monitor_dpi(monitor);
    let height_dip = CLIENT_HEIGHT_DIP.load(Ordering::SeqCst);
    let expected_w = (i64::from(POPUP_WIDTH) * i64::from(dpi) / 96) as i32;
    let expected_h = (i64::from(height_dip) * i64::from(dpi) / 96) as i32;

    (
        measured_w.max(expected_w).max(1),
        measured_h.max(expected_h).max(1),
    )
}

fn move_hwnd(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        SetWindowPos(hwnd, HWND_TOPMOST, x, y, 0, 0, SWP_NOSIZE | SWP_SHOWWINDOW);
    }
}

/// Pin the HWND to the bottom-right of the target monitor.
///
/// Uses the union of `GetWindowRect` and DWM extended bounds so we never
/// underestimate size, and clamps against `rcMonitor` (the real seam to the
/// next display) — not just `rcWork`.
fn pin_bottom_right(hwnd: HWND, monitor: RECT, work_bottom: i32) {
    for _ in 0..6 {
        let frame = frame_bounds(hwnd);
        let win = window_rect(hwnd);
        let left = frame.left.min(win.left);
        let top = frame.top.min(win.top);
        let right = frame.right.max(win.right);
        let bottom = frame.bottom.max(win.bottom);
        let width = (right - left).max(1);
        let height = (bottom - top).max(1);

        let target_x = monitor.right - width - EDGE_MARGIN;
        let target_y = work_bottom - height - EDGE_MARGIN;

        let dx = target_x - left;
        let dy = target_y - top;
        if dx == 0 && dy == 0 {
            break;
        }
        move_hwnd(hwnd, win.left + dx, win.top + dy);
        unsafe {
            let _ = DwmFlush();
        }
    }

    // Absolute hard stop: nothing past the monitor seam.
    let win = window_rect(hwnd);
    let frame = frame_bounds(hwnd);
    let right = frame.right.max(win.right);
    if right > monitor.right - EDGE_MARGIN {
        let dx = right - (monitor.right - EDGE_MARGIN);
        move_hwnd(hwnd, win.left - dx, win.top);
    }

    // Rounded region = window shape without the Win11 DWM frame shadow.
    apply_window_region(hwnd, None);
}

/// Rounded HWND shape (and optional monitor clip during slide-in).
///
/// `DWMWCP_ROUND` looks better but always paints a drop shadow past the
/// monitor seam; a GDI round-rect region has no shadow and matches XAML.
fn apply_window_region(hwnd: HWND, monitor_clip: Option<RECT>) {
    let window = window_rect(hwnd);
    let width = (window.right - window.left).max(1);
    let height = (window.bottom - window.top).max(1);
    let radius = CORNER_RADIUS_PX.load(Ordering::SeqCst).max(1);
    let arc = radius.saturating_mul(2);

    unsafe {
        let shape = CreateRoundRectRgn(0, 0, width + 1, height + 1, arc, arc);
        if shape.is_null() {
            return;
        }

        if let Some(mon) = monitor_clip {
            let left = (mon.left - window.left).clamp(0, width);
            let top = (mon.top - window.top).clamp(0, height);
            let right = (mon.right - window.left).clamp(left, width);
            let bottom = (mon.bottom - window.top).clamp(top, height);
            let clip = CreateRectRgn(left, top, right, bottom);
            if !clip.is_null() {
                let _ = CombineRgn(shape, shape, clip, RGN_AND);
                let _ = DeleteObject(clip as _);
            }
        }

        SetWindowRgn(hwnd, shape, 1);
    }
}

fn hide_window_pixels(hwnd: HWND) {
    unsafe {
        let empty = CreateRectRgn(0, 0, 0, 0);
        SetWindowRgn(hwnd, empty, 1);
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

fn set_corner_preference(hwnd: HWND) {
    unsafe {
        let corner = DWMWCP_DONOTROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const u32 as *const _,
            size_of::<u32>() as u32,
        );
    }
}

/// Prefer the monitor that owns the tray click; if that point sits on a shared
/// edge, pull inward so we don't open on the neighbor.
fn resolve_monitor(anchor_x: i32, anchor_y: i32) -> (HMONITOR, RECT, RECT) {
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
            return (inward, inward_info.rcMonitor, inward_info.rcWork);
        }

        (monitor, info.rcMonitor, info.rcWork)
    }
}

/// Shell chrome without SystemBackdrop (Acrylic ignores SetWindowRgn and
/// paints square corners + a drop shadow onto the neighboring monitor).
fn apply_popup_chrome(hwnd: HWND) {
    unsafe {
        let dark_mode = 1u32;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            &dark_mode as *const u32 as *const _,
            size_of::<u32>() as u32,
        );
        let no_border = DWMWA_COLOR_NONE;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR as u32,
            &no_border as *const u32 as *const _,
            size_of::<u32>() as u32,
        );
    }
    set_corner_preference(hwnd);
    set_system_backdrop(hwnd, DWMSBT_NONE);
    set_frame_margins(hwnd, false);
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

        let class_style = GetClassLongPtrW(hwnd, GCL_STYLE) as u32;
        SetClassLongPtrW(hwnd, GCL_STYLE, (class_style & !CS_DROPSHADOW) as isize);

        apply_popup_chrome(hwnd);

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

/// Re-clamp if WinUI grows/moves the HWND past the stored monitor.
pub fn keep_on_monitor() {
    if !is_visible() {
        return;
    }
    let Some(hwnd) = current_hwnd() else {
        return;
    };

    let monitor = loaded_monitor();
    if monitor.right <= monitor.left {
        return;
    }
    pin_bottom_right(hwnd, monitor, loaded_work_bottom());
}

/// Show the popup near the tray click, anchored above the taskbar.
pub fn show_near(anchor_x: i32, anchor_y: i32) {
    let Some(hwnd) = ensure_configured() else {
        return;
    };

    let (hmonitor, monitor, work) = resolve_monitor(anchor_x, anchor_y);
    store_bounds(monitor, work);

    let dpi = monitor_dpi(hmonitor);
    let corner_px = (i64::from(WINDOW_CORNER_RADIUS_DIP) * i64::from(dpi) / 96) as i32;
    CORNER_RADIUS_PX.store(corner_px.max(1), Ordering::SeqCst);

    unsafe {
        // Acrylic ignores SetWindowRgn — kill it for the slide, restore after.
        set_system_backdrop(hwnd, DWMSBT_NONE);
        set_frame_margins(hwnd, false);

        let (width, height) = popup_pixel_size(hwnd, hmonitor);
        let target_x = monitor.right - width - EDGE_MARGIN;
        let target_y = work.bottom - height - EDGE_MARGIN;
        let start_x = monitor.right;

        // Hide pixels *before* the first on-screen move so one frame can't
        // flash the full window onto the neighboring monitor.
        hide_window_pixels(hwnd);
        move_hwnd(hwnd, start_x, target_y);
        apply_window_region(hwnd, Some(monitor));
        let _ = DwmFlush();
        let _ = SetForegroundWindow(hwnd);

        for step in 1..=ANIMATION_STEPS {
            let progress = f64::from(step) / f64::from(ANIMATION_STEPS);
            let eased = ease_appear(progress);
            let animated_x = start_x - ((f64::from(start_x - target_x) * eased).round() as i32);
            move_hwnd(hwnd, animated_x, target_y);
            apply_window_region(hwnd, Some(monitor));
            let _ = DwmFlush();
        }

        apply_popup_chrome(hwnd);
        let _ = DwmFlush();
    }

    pin_bottom_right(hwnd, monitor, work.bottom);

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
