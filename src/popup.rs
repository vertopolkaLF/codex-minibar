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
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicIsize, AtomicU8, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::settings::{BottomBarSize, PopupCornerRadius};
use windows_reactor::{ReactorHost, Rendering, on_rendering};
use windows_sys::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::{
        Dwm::{
            DWMSBT_NONE, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_EXTENDED_FRAME_BOUNDS,
            DWMWA_SYSTEMBACKDROP_TYPE, DwmExtendFrameIntoClientArea, DwmFlush,
            DwmGetWindowAttribute, DwmSetWindowAttribute,
        },
        Gdi::{
            CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, GetMonitorInfoW, HMONITOR,
            MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, RGN_AND, SetWindowRgn,
        },
    },
    UI::{
        Controls::MARGINS,
        HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, SetFocus, VK_ESCAPE, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON,
        },
        WindowsAndMessaging::{
            DispatchMessageW, FindWindowW, GWL_EXSTYLE, GWL_STYLE, GetCursorPos, GetWindowLongW,
            GetWindowRect, HWND_TOPMOST, MSG, PM_REMOVE, PeekMessageW, SPI_GETCLIENTAREAANIMATION,
            SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOREDRAW, SWP_NOSIZE, SWP_NOZORDER,
            SWP_SHOWWINDOW, SetForegroundWindow, SetWindowLongW, SetWindowPos,
            SystemParametersInfoW, TranslateMessage, WS_CAPTION, WS_EX_APPWINDOW, WS_EX_LAYERED,
            WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
            WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
        },
    },
};

const WINDOW_TITLE: &str = "Codex Minibar";
const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
/// Do not let DWM add its own rounded non-client frame; the visible capsule is
/// clipped and painted by our XAML surface.
const DWMWCP_DONOTROUND: u32 = 1;
const DWMWA_NCRENDERING_POLICY: u32 = 2;
const DWMNCRP_DISABLED: u32 = 1;
/// Ignore outside presses briefly after open so the tray click that showed us
/// cannot immediately dismiss. Counted from the moment the slide starts.
const SHOW_GRACE_MS: i64 = 200;
/// Popup client width in DIP — fixed; height adapts to content.
pub const POPUP_WIDTH: i32 = 380;
/// Layout pieces used by [`height_for`] (must stay in sync with `popup_window`).
const LIMIT_CARD_HEIGHT: i32 = 82;
const BODY_PAD_Y: i32 = 36; // top 16 + bottom 20
const BODY_SPACING: i32 = 12;
const DEFAULT_FOOTER_HEIGHT: i32 = BottomBarSize::Comfortable.footer_height_dip();
const CHROME_HEIGHT: i32 = 4; // outer border + inset
/// Baseline height when the two standard limit cards and footer are shown.
pub const POPUP_HEIGHT: i32 =
    BODY_PAD_Y + LIMIT_CARD_HEIGHT * 2 + BODY_SPACING + DEFAULT_FOOTER_HEIGHT + CHROME_HEIGHT;
/// Smallest popup: two limit cards plus the footer.
pub const POPUP_HEIGHT_MIN: i32 = POPUP_HEIGHT;
/// Temporary safety ceiling before the popup is assigned to a monitor.
///
/// This is deliberately larger than any supported desktop. The real maximum
/// is set from the target monitor immediately before every show; using the old
/// 640 DIP value here made that bootstrap constraint leak into the live popup.
pub const FALLBACK_CLIENT_HEIGHT_LIMIT: i32 = 4_096;
/// Popup height as a share of the monitor it is opened on.
const POPUP_SCREEN_HEIGHT_FRACTION: f64 = 0.80;
/// Historical popup radius used before runtime appearance settings are loaded.
pub const WINDOW_CORNER_RADIUS_DIP: i32 = PopupCornerRadius::Small.dip();
/// Inner card radius. Keep cards visibly nested without turning the whole
/// popup into a stack of identical rounded rectangles.
pub const CARD_CORNER_RADIUS_DIP: i32 = 10;
const PARKED_X: i32 = -32_000;
const PARKED_Y: i32 = -32_000;
/// Fluent motion tokens used by Windows edge panels.
const OPEN_ANIMATION_DURATION: Duration = Duration::from_millis(250);
const CLOSE_ANIMATION_DURATION: Duration = Duration::from_millis(167);
/// Critical-damping frequency for the popup shell height spring. This is
/// intentionally a physics-based settle rather than a fixed-duration ease:
/// DesiredSize can change while a page is still rendering, and retargeting a
/// spring preserves velocity instead of making the bottom edge restart/bob.
const HEIGHT_SPRING_OMEGA: f64 = 24.0;
/// Gap from the monitor edge.
const EDGE_MARGIN: i32 = 20;

static HWND_BITS: AtomicIsize = AtomicIsize::new(0);
static CONFIGURED: AtomicBool = AtomicBool::new(false);
static POPUP_VISIBLE: AtomicBool = AtomicBool::new(false);
static BUTTON_WAS_DOWN: AtomicBool = AtomicBool::new(false);
static ESCAPE_WAS_DOWN: AtomicBool = AtomicBool::new(false);
static IGNORE_OUTSIDE_UNTIL_MS: AtomicI64 = AtomicI64::new(0);
/// Current client height in DIP (updated when content changes).
static CLIENT_HEIGHT_DIP: AtomicI32 = AtomicI32::new(POPUP_HEIGHT);
/// Height used by the XAML surface layout. It changes once per new target,
/// never once per spring frame.
static LAYOUT_SURFACE_HEIGHT_DIP: AtomicI32 = AtomicI32::new(POPUP_HEIGHT);
/// Current height of the clipped visible surface while its spring is running.
static ANIMATED_SURFACE_HEIGHT_DIP: AtomicI32 = AtomicI32::new(POPUP_HEIGHT);
/// Fixed native host height while the visible capsule animates inside it.
static HOST_CLIENT_HEIGHT_DIP: AtomicI32 = AtomicI32::new(POPUP_HEIGHT);
static BOTTOM_BAR_SIZE: AtomicU8 = AtomicU8::new(BottomBarSize::Comfortable.index() as u8);
static CORNER_RADIUS_DIP: AtomicI32 = AtomicI32::new(WINDOW_CORNER_RADIUS_DIP);
/// Natural height of the body stack, including its padding, in DIPs.
static BODY_CONTENT_HEIGHT_DIP: AtomicI32 = AtomicI32::new(0);
/// Dynamic client-height limit for the monitor that owns the current popup.
static MAX_CLIENT_HEIGHT_DIP: AtomicI32 = AtomicI32::new(FALLBACK_CLIENT_HEIGHT_LIMIT);
/// Physical monitor bounds (not work area) — right edge is the seam to the next display.
static MONITOR_LEFT: AtomicI32 = AtomicI32::new(0);
static MONITOR_TOP: AtomicI32 = AtomicI32::new(0);
static MONITOR_RIGHT: AtomicI32 = AtomicI32::new(0);
static MONITOR_BOTTOM: AtomicI32 = AtomicI32::new(0);
static WORK_BOTTOM: AtomicI32 = AtomicI32::new(0);
static CORNER_RADIUS_PX: AtomicI32 = AtomicI32::new(WINDOW_CORNER_RADIUS_DIP);
/// `GetWindowRect().bottom` locked when the popup settles. Height animation
/// keeps this exact pixel fixed — never retarget from DWM frame bounds.
static PINNED_WIN_BOTTOM_PX: AtomicI32 = AtomicI32::new(0);

thread_local! {
    static POPUP_HOST: RefCell<Option<Rc<ReactorHost>>> = const { RefCell::new(None) };
    static POPUP_RENDERING: RefCell<Option<Rendering>> = const { RefCell::new(None) };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowMotionKind {
    Opening,
    Closing,
}

#[derive(Clone, Debug)]
struct WindowMotion {
    kind: WindowMotionKind,
    from_x: i32,
    to_x: i32,
    started_at: Instant,
    duration: Duration,
}

#[derive(Clone, Debug)]
struct HeightMotion {
    position_dip: f64,
    velocity_dip_per_second: f64,
    target_dip: f64,
    last_sample: Instant,
}

struct PopupMotion {
    window: Option<WindowMotion>,
    height: Option<HeightMotion>,
}

// Tray events are pumped on a worker thread while CompositionTarget.Rendering
// fires on the WinUI thread. Motion therefore cannot be thread-local: doing so
// leaves the real animation loop staring at a different, permanently empty
// state while the HWND remains parked beyond the monitor edge.
static POPUP_MOTION: Mutex<PopupMotion> = Mutex::new(PopupMotion {
    window: None,
    height: None,
});

fn popup_motion() -> MutexGuard<'static, PopupMotion> {
    POPUP_MOTION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
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

fn pinned_bottom_px() -> i32 {
    let locked = PINNED_WIN_BOTTOM_PX.load(Ordering::SeqCst);
    if locked != 0 {
        locked
    } else {
        loaded_work_bottom().saturating_sub(EDGE_MARGIN)
    }
}

/// Maximum client height for the monitor selected when the popup was opened.
pub fn max_client_height_dip() -> i32 {
    MAX_CLIENT_HEIGHT_DIP.load(Ordering::SeqCst)
}

pub fn bottom_bar_size() -> BottomBarSize {
    BottomBarSize::from_index(i32::from(BOTTOM_BAR_SIZE.load(Ordering::SeqCst)))
}

pub fn bottom_bar_icon_size() -> f64 {
    bottom_bar_size().icon_button_size()
}

pub fn bottom_bar_icon_glyph_size() -> f64 {
    bottom_bar_size().icon_glyph_size()
}

fn footer_height_dip() -> i32 {
    bottom_bar_size().footer_height_dip()
}

pub fn corner_radius_dip() -> i32 {
    CORNER_RADIUS_DIP.load(Ordering::SeqCst)
}

/// Apply appearance values that affect the popup outside the reactive tree.
/// Startup calls this before the host exists; later calls update the visible
/// region immediately and the next render picks up the new XAML geometry.
pub fn apply_popup_appearance(size: BottomBarSize, radius: PopupCornerRadius) {
    let size_index = size.index() as u8;
    let size_changed = BOTTOM_BAR_SIZE.swap(size_index, Ordering::SeqCst) != size_index;
    let radius_dip = radius.dip();
    let radius_changed = CORNER_RADIUS_DIP.swap(radius_dip, Ordering::SeqCst) != radius_dip;

    if !size_changed && !radius_changed {
        return;
    }

    if size_changed {
        let body_height = BODY_CONTENT_HEIGHT_DIP.load(Ordering::SeqCst);
        if body_height > 0 {
            set_client_height_dip(body_height + footer_height_dip() + CHROME_HEIGHT);
        }
    }

    if radius_changed {
        let radius_px = (i64::from(radius_dip) * i64::from(host_dpi(96)) / 96) as i32;
        CORNER_RADIUS_PX.store(radius_px.max(0), Ordering::SeqCst);
        if is_visible()
            && let Some(hwnd) = current_hwnd()
        {
            apply_surface_window_region(hwnd, f64::from(animated_surface_height_dip()), None);
        }
    }

    if current_hwnd().is_some()
        && (!size_changed || BODY_CONTENT_HEIGHT_DIP.load(Ordering::SeqCst) == 0)
    {
        windows_reactor::request_ui_rerender_on_ui_thread();
    }
}

/// Visible body height in DIP (popup client minus footer and chrome).
pub fn body_viewport_height_dip() -> f64 {
    let layout_height = LAYOUT_SURFACE_HEIGHT_DIP.load(Ordering::SeqCst);
    f64::from((layout_height - footer_height_dip() - CHROME_HEIGHT).max(80))
}

/// Current visible capsule height in DIPs. The native host can be larger while
/// a popup height transition is running; layout must follow the capsule, not
/// the clipped host rectangle.
pub fn surface_height_dip() -> i32 {
    LAYOUT_SURFACE_HEIGHT_DIP
        .load(Ordering::SeqCst)
        .clamp(80, HOST_CLIENT_HEIGHT_DIP.load(Ordering::SeqCst).max(80))
}

fn animated_surface_height_dip() -> i32 {
    ANIMATED_SURFACE_HEIGHT_DIP
        .load(Ordering::SeqCst)
        .clamp(80, HOST_CLIENT_HEIGHT_DIP.load(Ordering::SeqCst).max(80))
}

/// The same DPI used by `ReactorHost::resize_client`; mixing it with the
/// monitor DPI makes a nominal 80% window visibly much shorter on scaled
/// displays.
fn host_dpi(fallback: u32) -> u32 {
    POPUP_HOST.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or(fallback.max(1), |host| host.dpi())
    })
}

fn update_height_limit_for_monitor(monitor: RECT, fallback_dpi: u32) {
    let monitor_height_px = (monitor.bottom - monitor.top).max(1);
    let height_dip = (f64::from(monitor_height_px) * 96.0 / f64::from(host_dpi(fallback_dpi))
        * POPUP_SCREEN_HEIGHT_FRACTION)
        .round() as i32;
    let max_height_dip = height_dip.max(80);

    let changed = MAX_CLIENT_HEIGHT_DIP.swap(max_height_dip, Ordering::SeqCst) != max_height_dip;

    POPUP_HOST.with(|slot| {
        if let Some(host) = slot.borrow().as_ref() {
            let _ = host.relax_height_constraints(f64::from(max_height_dip));
        }
    });

    if changed {
        resize_for_body_content();
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

/// Normalized critically-damped spring curve used for the short window slide.
/// It is monotonic, so the popup never overshoots across a monitor seam.
fn critical_spring_curve(progress: f64, omega: f64) -> f64 {
    let t = progress.clamp(0.0, 1.0);
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    (1.0 - (1.0 + omega * t) * (-omega * t).exp()).clamp(0.0, 1.0)
}

/// Return and remember the stable logical bottom for the current work area.
/// DWM's extended frame includes a shadow that can vary by a pixel while the
/// popup is moving; using it as a vertical anchor makes the footer drift.
fn refresh_bottom_anchor() -> i32 {
    let seam = loaded_work_bottom().saturating_sub(EDGE_MARGIN);
    PINNED_WIN_BOTTOM_PX.store(seam, Ordering::SeqCst);
    seam
}

/// Spring-based entrance: fast response with a soft landing.
fn ease_entrance(progress: f64) -> f64 {
    critical_spring_curve(progress, 8.0)
}

/// Respect the Windows "Animation effects" accessibility preference.
pub fn system_animations_enabled() -> bool {
    let mut enabled = 1i32;
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            &mut enabled as *mut i32 as *mut _,
            0,
        )
    };
    ok == 0 || enabled != 0
}

pub fn animations_enabled() -> bool {
    crate::theme::animations_enabled()
}

/// Spring-based gentle exit: the surface gains speed as it leaves.
fn ease_exit(progress: f64) -> f64 {
    critical_spring_curve(progress, 7.0)
}

fn elapsed_progress(started_at: Instant, duration: Duration, now: Instant) -> f64 {
    if duration.is_zero() {
        return 1.0;
    }
    now.saturating_duration_since(started_at).as_secs_f64() / duration.as_secs_f64()
}

fn lerp_i32(from: i32, to: i32, progress: f64) -> i32 {
    (f64::from(from) + f64::from(to - from) * progress).round() as i32
}

/// Advance a critical-damping spring without resetting its velocity when the
/// target changes. The closed-form step is stable across uneven compositor
/// frame intervals and avoids the accumulated jitter of a hand-written Euler
/// integrator.
fn step_height_spring(motion: &mut HeightMotion, now: Instant) -> (i32, bool) {
    let dt = now
        .saturating_duration_since(motion.last_sample)
        .as_secs_f64()
        .clamp(0.0, 0.05);
    motion.last_sample = now;

    let offset = motion.position_dip - motion.target_dip;
    let omega = HEIGHT_SPRING_OMEGA;
    let decay = (-omega * dt).exp();
    let next_offset = (offset + (motion.velocity_dip_per_second + omega * offset) * dt) * decay;
    let next_velocity = (motion.velocity_dip_per_second
        - omega * (motion.velocity_dip_per_second + omega * offset) * dt)
        * decay;

    motion.position_dip = motion.target_dip + next_offset;
    motion.velocity_dip_per_second = next_velocity;
    let settled = next_offset.abs() < 0.25 && next_velocity.abs() < 1.0;
    if settled {
        motion.position_dip = motion.target_dip;
        motion.velocity_dip_per_second = 0.0;
    }

    (motion.position_dip.round() as i32, settled)
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
    // Every HWND mutation is dispatched to the WinUI thread. Update shared
    // state first, then release the mutex before calling Win32: SetWindowPos
    // can synchronously re-enter XAML layout, which may publish a height motion
    // and must never wait on a lock held by this same call stack.
    POPUP_VISIBLE.store(false, Ordering::SeqCst);
    {
        let mut motion = popup_motion();
        motion.window = None;
        motion.height = None;
    }
    PINNED_WIN_BOTTOM_PX.store(0, Ordering::SeqCst);
    stop_rendering_if_idle();
    unsafe {
        // Preserve the HWND/XAML compositor between shows. An empty region and
        // an off-screen position are invisible without `SWP_HIDEWINDOW`, which
        // can suspend composition until an unrelated WinUI window activates.
        hide_window_pixels(hwnd);
        // Park off-screen. Never touch size — WinUI owns width/height.
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            PARKED_X,
            PARKED_Y,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER,
        );
    }
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
pub fn height_for(error: Option<&str>) -> i32 {
    let mut blocks = vec![LIMIT_CARD_HEIGHT, LIMIT_CARD_HEIGHT];
    if let Some(message) = error {
        blocks.insert(0, info_bar_height(message));
    }
    let spacings = BODY_SPACING * (blocks.len().saturating_sub(1) as i32);
    let height =
        BODY_PAD_Y + blocks.iter().sum::<i32>() + spacings + footer_height_dip() + CHROME_HEIGHT;
    let max_height = max_client_height_dip();
    height.clamp(minimum_popup_height_dip().min(max_height), max_height)
}

fn minimum_popup_height_dip() -> i32 {
    BODY_PAD_Y
        + LIMIT_CARD_HEIGHT * 2
        + BODY_SPACING
        + footer_height_dip()
        + CHROME_HEIGHT
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
    let _ = host.relax_height_constraints(f64::from(max_client_height_dip()));
    // Pin taskbar exclusion as early as possible — before the first show.
    let _ = host.set_shown_in_switchers(false);
    POPUP_HOST.with(|slot| *slot.borrow_mut() = Some(host));
    // Do not subscribe CompositionTarget.Rendering here. A permanent
    // subscription keeps the compositor spinning for the whole process life
    // and was a major contributor to multi-hour working-set growth. Motion
    // paths call [`ensure_rendering`] only while an animation is active.
}

/// Attach the compositor clock while shell motion is running.
fn ensure_rendering() {
    POPUP_RENDERING.with(|slot| {
        if slot.borrow().is_none() {
            *slot.borrow_mut() = on_rendering(animation_frame).ok();
        }
    });
}

/// Drop the compositor clock once every shell animation has finished.
fn stop_rendering_if_idle() {
    let idle = {
        let motion = popup_motion();
        motion.window.is_none() && motion.height.is_none()
    };
    if idle {
        POPUP_RENDERING.with(|slot| {
            *slot.borrow_mut() = None;
        });
    }
}

/// Resize to the body's natural height plus the fixed footer and popup chrome.
/// The final size is always capped by the active monitor's height limit.
pub fn set_client_height_from_body_content(body_height_dip: f64) {
    if !body_height_dip.is_finite() || body_height_dip < 1.0 {
        return;
    }
    BODY_CONTENT_HEIGHT_DIP.store(body_height_dip.ceil() as i32, Ordering::SeqCst);
    resize_for_body_content();
}

fn resize_for_body_content() {
    let body_height = BODY_CONTENT_HEIGHT_DIP.load(Ordering::SeqCst);
    if body_height < 1 {
        return;
    }
    set_client_height_dip(body_height + footer_height_dip() + CHROME_HEIGHT);
}

/// Retarget the visible capsule and re-pin it if the popup is open.
///
/// The XAML surface receives one layout update for the new target. The spring
/// then animates only the clipped native surface, so a heavy page is not
/// reconciled again on every compositor frame.
pub fn set_client_height_dip(height_dip: i32) {
    let height_dip = height_dip.clamp(80, max_client_height_dip());
    let previous_target = CLIENT_HEIGHT_DIP.swap(height_dip, Ordering::SeqCst);
    if previous_target == height_dip {
        return;
    }
    LAYOUT_SURFACE_HEIGHT_DIP.store(height_dip, Ordering::SeqCst);
    windows_reactor::request_ui_rerender_on_ui_thread();

    if !is_visible() || !animations_enabled() {
        apply_client_height_immediately(height_dip);
        return;
    }

    let applied = ANIMATED_SURFACE_HEIGHT_DIP.load(Ordering::SeqCst);
    let mut motion = popup_motion();
    if let Some(height) = motion.height.as_mut() {
        // Already springing — only move the destination. Keeping position and
        // velocity makes a DesiredSize blip retarget smoothly instead of
        // restarting from a stale integer frame.
        if (height.target_dip - f64::from(height_dip)).abs() <= 0.5 {
            return;
        }
        height.target_dip = f64::from(height_dip);
        drop(motion);
        ensure_rendering();
        return;
    }

    if applied == height_dip {
        return;
    }
    motion.height = Some(HeightMotion {
        position_dip: f64::from(applied),
        velocity_dip_per_second: 0.0,
        target_dip: f64::from(height_dip),
        last_sample: Instant::now(),
    });
    drop(motion);
    ensure_rendering();
}

/// Re-apply size constraints after Win32 chrome is stripped (stale NC metrics).
pub fn sync_host_constraints() {
    let height = CLIENT_HEIGHT_DIP
        .load(Ordering::SeqCst)
        .clamp(80, max_client_height_dip());
    CLIENT_HEIGHT_DIP.store(height, Ordering::SeqCst);
    LAYOUT_SURFACE_HEIGHT_DIP.store(height, Ordering::SeqCst);
    HOST_CLIENT_HEIGHT_DIP.store(height, Ordering::SeqCst);
    ANIMATED_SURFACE_HEIGHT_DIP.store(height, Ordering::SeqCst);
    POPUP_HOST.with(|slot| {
        if let Some(host) = slot.borrow().as_ref() {
            let _ = host.relax_height_constraints(f64::from(max_client_height_dip()));
            let _ = host.resize_client(f64::from(POPUP_WIDTH), f64::from(height));
        }
    });
}

fn apply_client_height_immediately(height_dip: i32) {
    LAYOUT_SURFACE_HEIGHT_DIP.store(height_dip, Ordering::SeqCst);
    ANIMATED_SURFACE_HEIGHT_DIP.store(height_dip, Ordering::SeqCst);

    if !is_visible() {
        HOST_CLIENT_HEIGHT_DIP.store(height_dip, Ordering::SeqCst);
        POPUP_HOST.with(|slot| {
            if let Some(host) = slot.borrow().as_ref() {
                let _ = host.relax_height_constraints(f64::from(max_client_height_dip()));
                let _ = host.resize_client(f64::from(POPUP_WIDTH), f64::from(height_dip));
            }
        });
        return;
    }
    let Some(hwnd) = current_hwnd().or_else(find_hwnd) else {
        return;
    };
    HWND_BITS.store(hwnd as isize, Ordering::SeqCst);
    let monitor = loaded_monitor();
    if monitor.right > monitor.left {
        // ResizeClient re-anchors the top-left corner. Re-apply the stable
        // bottom seam after an immediate settings/content resize as well.
        pin_right_edge(hwnd);
        pin_win_bottom_to_seam(hwnd, refresh_bottom_anchor());
        windows_reactor::request_ui_rerender_on_ui_thread();
        apply_surface_window_region(hwnd, f64::from(height_dip), Some(monitor));
    }
}

/// Resize the fixed native host while preserving the logical bottom seam.
///
/// The visible capsule is clipped inside this host and owns the animated
/// height; the host itself should only change when the monitor-sized shell is
/// prepared or the monitor constraints change.
fn resize_host_height_now(hwnd: HWND, height_dip: i32) {
    let height_dip = height_dip.clamp(80, max_client_height_dip());
    if HOST_CLIENT_HEIGHT_DIP.load(Ordering::SeqCst) == height_dip {
        return;
    }
    if PINNED_WIN_BOTTOM_PX.load(Ordering::SeqCst) == 0 {
        refresh_bottom_anchor();
    }
    let seam = pinned_bottom_px();
    let win = window_rect(hwnd);

    POPUP_HOST.with(|slot| {
        if let Some(host) = slot.borrow().as_ref() {
            let _ = host.relax_height_constraints(f64::from(max_client_height_dip()));
            // Set the island target before changing the HWND. Both operations
            // run on the UI thread, so the next compositor frame sees one
            // geometry target instead of exposing a black clear while XAML
            // catches up with native window resizing.
            host.sync_render_size(f64::from(POPUP_WIDTH), f64::from(height_dip));
            let _ = host.move_and_resize_bottom_pinned(
                win.left,
                seam,
                f64::from(POPUP_WIDTH),
                f64::from(height_dip),
            );
        }
    });
    HOST_CLIENT_HEIGHT_DIP.store(height_dip, Ordering::SeqCst);
    pin_win_bottom_to_seam(hwnd, seam);
}

/// Prepare the larger off-screen host before the popup is shown.
fn prepare_hidden_host_height(height_dip: i32) {
    let height_dip = height_dip.clamp(80, max_client_height_dip());
    HOST_CLIENT_HEIGHT_DIP.store(height_dip, Ordering::SeqCst);
    POPUP_HOST.with(|slot| {
        if let Some(host) = slot.borrow().as_ref() {
            host.sync_render_size(f64::from(POPUP_WIDTH), f64::from(height_dip));
            let _ = host.resize_client(f64::from(POPUP_WIDTH), f64::from(height_dip));
        }
    });
}

/// One compositor tick of shell-surface spring motion.
///
/// The native host remains fixed while only the bottom-aligned capsule changes
/// height. This prevents Window.ContentPresenter from exposing its opaque
/// client background below a still-rendering root.
fn apply_animated_client_height(hwnd: HWND, height_dip: i32) {
    let host_height = HOST_CLIENT_HEIGHT_DIP.load(Ordering::SeqCst);
    if height_dip > host_height {
        resize_host_height_now(hwnd, height_dip);
    }
    ANIMATED_SURFACE_HEIGHT_DIP.store(height_dip, Ordering::SeqCst);
    apply_surface_window_region(hwnd, f64::from(height_dip), Some(loaded_monitor()));
}

fn finish_height_animation(hwnd: HWND, height_dip: i32, pin: bool) {
    ANIMATED_SURFACE_HEIGHT_DIP.store(height_dip, Ordering::SeqCst);
    apply_surface_window_region(hwnd, f64::from(height_dip), None);
    if pin {
        // Horizontal settle only — full DWM-frame-based pinning can yank the
        // locked vertical seam by a pixel or two.
        pin_right_edge(hwnd);
        pin_win_bottom_to_seam(hwnd, pinned_bottom_px());
    }
}

/// Keep the right edge on the monitor seam without touching Y.
fn pin_right_edge(hwnd: HWND) {
    let monitor = loaded_monitor();
    if monitor.right <= monitor.left {
        return;
    }
    let win = window_rect(hwnd);
    let frame = frame_bounds(hwnd);
    let left = frame.left.min(win.left);
    let right = frame.right.max(win.right);
    let width = (right - left).max(1);
    let target_x = monitor.right - width - EDGE_MARGIN;
    let dx = target_x - left;
    if dx != 0 {
        move_hwnd(hwnd, win.left + dx, win.top);
    }
}

/// Pin using `GetWindowRect` only — DWM extended bounds include a jittery shadow.
fn pin_win_bottom_to_seam(hwnd: HWND, seam: i32) {
    let win = window_rect(hwnd);
    let dy = seam - win.bottom;
    if dy == 0 {
        return;
    }
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            win.left,
            win.top + dy,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_NOREDRAW,
        );
    }
}

/// Estimate outer size without ever writing it back through SetWindowPos.
fn popup_pixel_size(hwnd: HWND, monitor: HMONITOR) -> (i32, i32) {
    let rect = frame_bounds(hwnd);
    let measured_w = (rect.right - rect.left).abs();
    let measured_h = (rect.bottom - rect.top).abs();

    let dpi = host_dpi(monitor_dpi(monitor));
    let height_dip = HOST_CLIENT_HEIGHT_DIP.load(Ordering::SeqCst);
    let expected_w = (i64::from(POPUP_WIDTH) * i64::from(dpi) / 96) as i32;
    let expected_h = (i64::from(height_dip) * i64::from(dpi) / 96) as i32;

    (
        measured_w.max(expected_w).max(1),
        measured_h.max(expected_h).max(1),
    )
}

fn move_hwnd(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        // Never activate while moving — WinUI would focus the first button.
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_SHOWWINDOW | SWP_NOACTIVATE,
        );
    }
}

/// Clip the fixed native host to the visible bottom-aligned capsule.
fn apply_surface_window_region(hwnd: HWND, surface_height_dip: f64, monitor_clip: Option<RECT>) {
    let window = window_rect(hwnd);
    apply_surface_window_region_for_rect(hwnd, window, surface_height_dip, monitor_clip);
}

fn monitor_clip_bounds(window: RECT, monitor: RECT) -> (i32, i32, i32, i32) {
    let width = (window.right - window.left).max(1);
    let height = (window.bottom - window.top).max(1);
    let left = (monitor.left - window.left).clamp(0, width);
    let top = (monitor.top - window.top).clamp(0, height);
    let right = (monitor.right - window.left).clamp(left, width);
    let bottom = (monitor.bottom - window.top).clamp(top, height);
    (left, top, right, bottom)
}

fn apply_surface_window_region_for_rect(
    hwnd: HWND,
    window: RECT,
    surface_height_dip: f64,
    monitor_clip: Option<RECT>,
) {
    let width = (window.right - window.left).max(1);
    let height = (window.bottom - window.top).max(1);
    let surface_height = (surface_height_dip * f64::from(host_dpi(96)) / 96.0)
        .round()
        .clamp(1.0, f64::from(height)) as i32;
    let surface_top = height.saturating_sub(surface_height);
    let radius = CORNER_RADIUS_PX.load(Ordering::SeqCst).max(0);
    let arc = radius.saturating_mul(2);

    unsafe {
        let shape = CreateRoundRectRgn(0, surface_top, width + 1, height + 1, arc, arc);
        if shape.is_null() {
            return;
        }

        if let Some(mon) = monitor_clip {
            let (left, top, right, bottom) = monitor_clip_bounds(window, mon);
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

fn clear_window_region(hwnd: HWND) {
    unsafe {
        SetWindowRgn(hwnd, std::ptr::null_mut(), 1);
    }
}

/// Drive every popup movement from the compositor clock. This keeps window
/// geometry, XAML layout, and the DWM frame on the same cadence instead of
/// blocking the UI thread in a hand-written `DwmFlush` loop.
fn animation_frame() {
    let Some(hwnd) = current_hwnd() else {
        return;
    };
    let now = Instant::now();
    let mut height_frame = None;
    let mut height_finished = None;
    let mut window_frame = None;
    let mut window_finished = None;
    let mut active_window_kind = None;

    {
        let mut motion = popup_motion();
        let mut finish_height = None;
        if let Some(height) = motion.height.as_mut() {
            let (value, settled) = step_height_spring(height, now);
            height_frame = Some(value);
            if settled {
                finish_height = Some(height.target_dip.round() as i32);
            }
        }
        if finish_height.is_some() {
            motion.height = None;
            height_finished = finish_height;
        }

        if let Some(window) = motion.window.as_ref() {
            active_window_kind = Some(window.kind);
            let progress =
                elapsed_progress(window.started_at, window.duration, now).clamp(0.0, 1.0);
            let eased = match window.kind {
                WindowMotionKind::Opening => ease_entrance(progress),
                WindowMotionKind::Closing => ease_exit(progress),
            };
            window_frame = Some((window.kind, lerp_i32(window.from_x, window.to_x, eased)));
            if progress >= 1.0 {
                window_finished = Some(window.kind);
                motion.window = None;
            }
        }
    }

    if let Some(height) = height_frame {
        apply_animated_client_height(hwnd, height);
    }
    if let Some((kind, x)) = window_frame {
        let rect = window_rect(hwnd);
        if kind == WindowMotionKind::Closing {
            // Clip for the upcoming position *before* moving. This prevents a
            // DWM frame from being presented for one refresh on a monitor to
            // the right of the popup's owning monitor.
            let width = (rect.right - rect.left).max(1);
            let height = (rect.bottom - rect.top).max(1);
            let next_rect = RECT {
                left: x,
                top: rect.top,
                right: x.saturating_add(width),
                bottom: rect.top.saturating_add(height),
            };
            apply_surface_window_region_for_rect(
                hwnd,
                next_rect,
                f64::from(animated_surface_height_dip()),
                Some(loaded_monitor()),
            );
            move_hwnd(hwnd, x, rect.top);
        } else {
            move_hwnd(hwnd, x, rect.top);
            apply_surface_window_region(
                hwnd,
                f64::from(animated_surface_height_dip()),
                Some(loaded_monitor()),
            );
        }
    }

    if let Some(height) = height_finished {
        finish_height_animation(hwnd, height, active_window_kind.is_none());
    }

    match window_finished {
        Some(WindowMotionKind::Opening) => finish_opening(hwnd),
        Some(WindowMotionKind::Closing) => park(hwnd),
        None => {}
    }
    stop_rendering_if_idle();
}

fn finish_opening(hwnd: HWND) {
    apply_popup_chrome(hwnd);
    hide_from_taskbar(hwnd);
    unsafe {
        // A tray click authorizes foreground activation. Clear keyboard focus
        // immediately so the first footer action never receives a focus ring.
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(std::ptr::null_mut());
    }
    let monitor = loaded_monitor();
    if monitor.right > monitor.left {
        // Keep the logical window bottom fixed to the work-area seam from the
        // first visible frame. The old DWM-union correction ran only at the
        // end of the slide, so a height change during opening could establish
        // a different bottom and make the footer drift.
        pin_right_edge(hwnd);
        pin_win_bottom_to_seam(hwnd, refresh_bottom_anchor());
    }
    IGNORE_OUTSIDE_UNTIL_MS.store(now_ms() + SHOW_GRACE_MS, Ordering::SeqCst);
    BUTTON_WAS_DOWN.store(true, Ordering::SeqCst);
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

/// Extend the DWM glass surface through the entire technical host. Without
/// this, transparent XAML outside the capsule falls back to the HWND's opaque
/// client clear color — the infamous black rectangle.
fn set_frame_margins(hwnd: HWND, full_glass: bool) {
    let margins = if full_glass {
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

/// Settled shell chrome: the XAML capsule supplies the material, border, and
/// radius; DWM contributes no visible non-client frame or shadow.
fn apply_popup_chrome(hwnd: HWND) {
    unsafe {
        let no_border = DWMWA_COLOR_NONE;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR as u32,
            &no_border as *const u32 as *const _,
            size_of::<u32>() as u32,
        );
    }
    disable_native_frame(hwnd);
    set_system_backdrop(hwnd, DWMSBT_NONE);
    if is_visible() {
        apply_surface_window_region(hwnd, f64::from(animated_surface_height_dip()), None);
    } else {
        clear_window_region(hwnd);
    }
}

/// Disable DWM non-client rendering. The HWND is only a transparent technical
/// host; border, radius, shadow, and backdrop belong to the XAML capsule.
fn disable_native_frame(hwnd: HWND) {
    unsafe {
        let policy = DWMNCRP_DISABLED;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY,
            &policy as *const u32 as *const _,
            size_of::<u32>() as u32,
        );
    }
    set_corner_preference(hwnd);
    set_frame_margins(hwnd, true);
}

/// Keep the popup out of the taskbar and Alt+Tab forever.
///
/// WinUI's AppWindow still defaults to `IsShownInSwitchers = true`, which puts a
/// normal taskbar button even when `WS_EX_TOOLWINDOW` is set — so both paths are
/// applied, and re-applied on every show in case AppWindow / Explorer reset them.
fn hide_from_taskbar(hwnd: HWND) {
    hide_from_switchers();

    unsafe {
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        let ex_style = (ex_style & !(WS_EX_APPWINDOW | WS_EX_LAYERED | WS_EX_NOACTIVATE))
            | WS_EX_TOOLWINDOW
            | WS_EX_TOPMOST
            | WS_EX_NOREDIRECTIONBITMAP;
        SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style as i32);
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
}

/// AppWindow taskbar exclusion — must run on the UI thread that owns the host.
pub fn hide_from_switchers() {
    POPUP_HOST.with(|slot| {
        if let Some(host) = slot.borrow().as_ref() {
            let _ = host.set_shown_in_switchers(false);
        }
    });
}

/// Find the WinUI window, restyle it as a tool popup, and park it off-screen.
pub fn ensure_configured() -> Option<HWND> {
    let hwnd = find_hwnd()?;
    HWND_BITS.store(hwnd as isize, Ordering::SeqCst);
    if CONFIGURED.swap(true, Ordering::SeqCst) {
        // Styles can be reset by AppWindow / shell — keep taskbar exclusion sticky.
        hide_from_taskbar(hwnd);
        return Some(hwnd);
    }

    unsafe {
        // Strip chrome, but do NOT force WS_POPUP — that breaks WinUI SystemBackdrop.
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        let style =
            style & !(WS_CAPTION | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU);
        SetWindowLongW(hwnd, GWL_STYLE, style as i32);

        // Tool/topmost popup shell. Avoid layered alpha and permanent no-activate:
        // both force solid backdrop fallbacks. Drop the GDI redirection bitmap so
        // acrylic / soft white strokes do not AA against a stale white surface
        // (bright fringes that eyes see and screenshots often miss).
        hide_from_taskbar(hwnd);

        apply_popup_chrome(hwnd);
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

pub fn is_closing() -> bool {
    popup_motion()
        .window
        .as_ref()
        .is_some_and(|window| window.kind == WindowMotionKind::Closing)
}

pub fn hide() {
    let Some(hwnd) = current_hwnd() else {
        // Still clear the flag — a missing HWND must not sticky-lock toggle.
        POPUP_VISIBLE.store(false, Ordering::SeqCst);
        return;
    };
    if !is_visible() {
        return;
    }
    if !animations_enabled() {
        park(hwnd);
        return;
    }
    if is_closing() {
        return;
    }
    let rect = window_rect(hwnd);
    let monitor = loaded_monitor();
    if monitor.right <= monitor.left {
        park(hwnd);
        return;
    }
    // Keep the technical host glass-transparent while the monitor-clipped
    // region owns the entering surface shape.
    set_frame_margins(hwnd, true);
    apply_surface_window_region(
        hwnd,
        f64::from(animated_surface_height_dip()),
        Some(monitor),
    );
    popup_motion().window = Some(WindowMotion {
        kind: WindowMotionKind::Closing,
        from_x: rect.left,
        to_x: monitor.right,
        started_at: Instant::now(),
        duration: CLOSE_ANIMATION_DURATION,
    });
    ensure_rendering();
}

/// Synchronize XAML composition before a native show. Must run on the UI thread.
///
/// This path must stay synchronous: acknowledging a queued activation before
/// it actually ran caused the popup to remain dormant until Settings happened
/// to activate another WinUI window.
pub fn prepare_show_on_ui_thread() -> bool {
    let activated = POPUP_HOST.with(|slot| {
        slot.borrow().as_ref().is_some_and(|host| {
            // The hidden popup no longer burns a full reconciliation every
            // second merely to age relative timestamps. Reconcile once on
            // every show so the first presented frame is current.
            windows_reactor::request_ui_rerender_on_ui_thread();
            host.activate_now().is_ok()
        })
    });
    hide_from_switchers();
    activated
}

/// Re-clamp if WinUI grows/moves the HWND past the stored monitor.
pub fn keep_on_monitor() {
    // This function runs only on the WinUI dispatcher. Inspect motion under the
    // mutex, release it, and only then touch HWND geometry to avoid Win32/XAML
    // re-entrancy deadlocks.
    let animating = {
        let motion = popup_motion();
        motion.window.is_some() || motion.height.is_some()
    };
    if !is_visible() || animating {
        return;
    }
    let Some(hwnd) = current_hwnd() else {
        return;
    };

    let monitor = loaded_monitor();
    if monitor.right <= monitor.left {
        return;
    }
    pin_right_edge(hwnd);
    pin_win_bottom_to_seam(hwnd, refresh_bottom_anchor());
}

/// Show the popup near the tray click, anchored above the taskbar.
pub fn show_near(anchor_x: i32, anchor_y: i32) {
    let Some(hwnd) = ensure_configured() else {
        return;
    };
    // Re-assert before SWP_SHOWWINDOW — Explorer / AppWindow can resurrect the button.
    hide_from_taskbar(hwnd);

    let (hmonitor, monitor, work) = resolve_monitor(anchor_x, anchor_y);
    store_bounds(monitor, work);

    let dpi = monitor_dpi(hmonitor);
    update_height_limit_for_monitor(monitor, dpi);
    // WinUI can leave a hidden AppWindow at the presenter's maximum height while
    // the reactor still holds the correct content target. Reapply that target
    // before reading native bounds; otherwise popup_pixel_size preserves the stale
    // oversized HWND until a tab switch happens to publish a different height.
    sync_host_constraints();
    prepare_hidden_host_height(max_client_height_dip());
    let corner_px = (i64::from(corner_radius_dip()) * i64::from(dpi) / 96) as i32;
    CORNER_RADIUS_PX.store(corner_px.max(0), Ordering::SeqCst);

    unsafe {
        // Element-level Acrylic stays clipped by XAML; full-host glass keeps the
        // technical area outside the capsule transparent during the slide-in.
        set_system_backdrop(hwnd, DWMSBT_NONE);
        set_frame_margins(hwnd, true);

        let (width, height) = popup_pixel_size(hwnd, hmonitor);
        let target_x = monitor.right - width - EDGE_MARGIN;
        let target_y = work.bottom - height - EDGE_MARGIN;
        let start_x = monitor.right;

        // Hide pixels *before* the first on-screen move so one frame can't
        // flash the full window onto the neighboring monitor.
        hide_window_pixels(hwnd);
        move_hwnd(hwnd, start_x, target_y);
        // Establish the vertical seam before the opening spring starts. From
        // this point onward every height frame uses the same bottom anchor.
        let bottom_seam = work.bottom.saturating_sub(EDGE_MARGIN);
        PINNED_WIN_BOTTOM_PX.store(bottom_seam, Ordering::SeqCst);
        pin_win_bottom_to_seam(hwnd, bottom_seam);
        apply_surface_window_region(
            hwnd,
            f64::from(animated_surface_height_dip()),
            Some(monitor),
        );
        let _ = DwmFlush();
        // Keep native moves non-activating. We activate once the final clipped
        // frame is in place so element-level Acrylic uses its active wallpaper
        // material instead of the dimmed inactive fallback.

        // Mark visible before scheduling the non-blocking compositor motion.
        POPUP_VISIBLE.store(true, Ordering::SeqCst);
        IGNORE_OUTSIDE_UNTIL_MS.store(now_ms() + SHOW_GRACE_MS, Ordering::SeqCst);
        BUTTON_WAS_DOWN.store(true, Ordering::SeqCst);
        if !animations_enabled() {
            move_hwnd(hwnd, target_x, target_y);
            finish_opening(hwnd);
            return;
        }

        popup_motion().window = Some(WindowMotion {
            kind: WindowMotionKind::Opening,
            from_x: start_x,
            to_x: target_x,
            started_at: Instant::now(),
            duration: OPEN_ANIMATION_DURATION,
        });
        ensure_rendering();
    }
}

/// Show the popup beside the current pointer location.
///
/// Settings opened from the tray menu do not carry a tray-click position, but
/// the pointer still gives the expected monitor and taskbar anchor.
pub fn show_near_cursor() {
    let mut cursor = POINT { x: 0, y: 0 };
    unsafe {
        GetCursorPos(&mut cursor);
    }
    show_near(cursor.x, cursor.y);
}

pub fn toggle_near(anchor_x: i32, anchor_y: i32) {
    if is_visible() {
        hide();
    } else {
        show_near(anchor_x, anchor_y);
    }
}

fn any_mouse_button_down() -> bool {
    unsafe {
        [VK_LBUTTON, VK_MBUTTON, VK_RBUTTON]
            .into_iter()
            .any(|button| GetAsyncKeyState(button as i32) < 0)
    }
}

fn cursor_outside_hwnd(hwnd: HWND) -> bool {
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

/// Rising edge of a mouse button while the cursor is outside the popup.
fn new_press_outside(hwnd: HWND) -> bool {
    let button_is_down = any_mouse_button_down();
    let was_down = BUTTON_WAS_DOWN.swap(button_is_down, Ordering::SeqCst);
    button_is_down && !was_down && cursor_outside_hwnd(hwnd)
}

/// Detect a new mouse press that lands outside the popup.
pub fn clicked_outside() -> bool {
    if !is_visible() || now_ms() < IGNORE_OUTSIDE_UNTIL_MS.load(Ordering::SeqCst) {
        BUTTON_WAS_DOWN.store(any_mouse_button_down(), Ordering::SeqCst);
        return false;
    }
    let Some(hwnd) = current_hwnd() else {
        return false;
    };
    new_press_outside(hwnd)
}

/// Rising edge of Escape while the transient popup is active.
pub fn escape_pressed() -> bool {
    if !is_visible() {
        ESCAPE_WAS_DOWN.store(false, Ordering::SeqCst);
        return false;
    }
    let down = unsafe { GetAsyncKeyState(VK_ESCAPE as i32) < 0 };
    let was_down = ESCAPE_WAS_DOWN.swap(down, Ordering::SeqCst);
    down && !was_down
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_curves_keep_exact_endpoints() {
        for easing in [ease_entrance, ease_exit] {
            assert_eq!(easing(0.0), 0.0);
            assert_eq!(easing(1.0), 1.0);
        }
    }

    #[test]
    fn spring_curves_are_monotonic() {
        for easing in [ease_entrance, ease_exit] {
            let samples = (0..=100)
                .map(|step| easing(f64::from(step) / 100.0))
                .collect::<Vec<_>>();
            assert!(samples.windows(2).all(|pair| pair[0] <= pair[1]));
        }
    }

    #[test]
    fn integer_interpolation_reaches_both_geometry_targets() {
        assert_eq!(lerp_i32(300, 700, 0.0), 300);
        assert_eq!(lerp_i32(300, 700, 1.0), 700);
        assert_eq!(lerp_i32(700, 300, 0.5), 500);
    }

    #[test]
    fn height_spring_converges_without_overshooting() {
        let started = Instant::now();
        let mut motion = HeightMotion {
            position_dip: 300.0,
            velocity_dip_per_second: 0.0,
            target_dip: 700.0,
            last_sample: started,
        };
        let mut previous = 300;
        let mut settled = false;
        for frame in 1..=120 {
            let (value, done) =
                step_height_spring(&mut motion, started + Duration::from_millis(frame * 16));
            assert!(value >= previous && value <= 700);
            previous = value;
            settled |= done;
        }
        assert!(settled);
        assert_eq!(previous, 700);
        assert_eq!(motion.velocity_dip_per_second, 0.0);
    }

    #[test]
    fn worker_published_motion_is_visible_to_the_render_thread() {
        popup_motion().window = None;
        std::thread::spawn(|| {
            popup_motion().window = Some(WindowMotion {
                kind: WindowMotionKind::Opening,
                from_x: 1_920,
                to_x: 1_520,
                started_at: Instant::now(),
                duration: OPEN_ANIMATION_DURATION,
            });
        })
        .join()
        .unwrap();

        assert_eq!(
            popup_motion().window.as_ref().map(|motion| motion.kind),
            Some(WindowMotionKind::Opening)
        );
        popup_motion().window = None;
    }

    #[test]
    fn exit_clip_shrinks_at_the_owning_monitor_edge() {
        let monitor = RECT {
            left: 0,
            top: 0,
            right: 1_920,
            bottom: 1_080,
        };
        let window = |left| RECT {
            left,
            top: 400,
            right: left + 380,
            bottom: 700,
        };

        assert_eq!(
            monitor_clip_bounds(window(1_520), monitor),
            (0, 0, 380, 300)
        );
        assert_eq!(
            monitor_clip_bounds(window(1_720), monitor),
            (0, 0, 200, 300)
        );
        assert_eq!(monitor_clip_bounds(window(1_920), monitor), (0, 0, 0, 300));
    }
}
