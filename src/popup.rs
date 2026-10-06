//! Thread-safe façade over the GPUI tray popup.
//!
//! The popup is rendered by GPUI on its own thread (see
//! [`crate::popup_window::ui`]). The tray bridge and the Stream Deck server
//! only post commands and read the shared flags below; none of them ever
//! touches GPUI state directly.
//!
//! Geometry model: the native window is a fixed, monitor-sized technical host
//! anchored to the monitor's right edge above the taskbar. The visible capsule
//! is drawn bottom/right aligned inside it and the HWND region is clipped to
//! that capsule on every animation frame. Opening, closing, height and width
//! motion therefore never move or resize the HWND (and never reallocate the
//! swap chain); they only change what GPUI paints and what the region exposes.
//! After hiding, the host parks at 1x1 pixels to release its large DirectX
//! surfaces. Its full monitor geometry is restored before the next show.

use std::{
    sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicU8, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::settings::{BottomBarSize, PopupBackgroundMaterial, PopupCornerRadius};

/// Popup client width in DIP — height adapts to content.
pub const POPUP_WIDTH: i32 = 380;
/// Two compact content columns, shared outer padding, and a 12 DIP gutter.
pub const POPUP_WIDE_WIDTH: i32 = POPUP_WIDTH * 2 - 34 + 12;
/// Inner card radius. Keep cards visibly nested without turning the whole
/// popup into a stack of identical rounded rectangles.
pub const CARD_CORNER_RADIUS_DIP: i32 = 10;
/// Historical popup radius used before runtime appearance settings are loaded.
pub const WINDOW_CORNER_RADIUS_DIP: i32 = PopupCornerRadius::Small.dip();
/// Gap between the capsule and the monitor/taskbar edges, in physical pixels.
pub const EDGE_MARGIN_PX: i32 = 20;
/// Popup height as a share of the monitor it is opened on.
pub const POPUP_SCREEN_HEIGHT_FRACTION: f64 = 0.80;
/// Ignore outside presses briefly after open so the tray click that showed us
/// cannot immediately dismiss.
const SHOW_GRACE_MS: i64 = 200;

static POPUP_VISIBLE: AtomicBool = AtomicBool::new(false);
static POPUP_CLOSING: AtomicBool = AtomicBool::new(false);
static BUTTON_WAS_DOWN: AtomicBool = AtomicBool::new(false);
static ESCAPE_WAS_DOWN: AtomicBool = AtomicBool::new(false);
static IGNORE_OUTSIDE_UNTIL_MS: AtomicI64 = AtomicI64::new(0);
static BOTTOM_BAR_SIZE: AtomicU8 = AtomicU8::new(BottomBarSize::Comfortable.index() as u8);
static CORNER_RADIUS_DIP: AtomicI32 = AtomicI32::new(WINDOW_CORNER_RADIUS_DIP);
static POPUP_BACKGROUND_MATERIAL: AtomicU8 =
    AtomicU8::new(PopupBackgroundMaterial::Acrylic.index() as u8);
/// Width of the visible capsule (DIP); tracked for helpers outside GPUI.
static CLIENT_WIDTH_DIP: AtomicI32 = AtomicI32::new(POPUP_WIDTH);
/// Visible capsule bounds in screen pixels, published by the GPUI thread on
/// every geometry change. Outside-click detection must use the capsule and
/// not the transparent technical host around it.
static SURFACE_LEFT: AtomicI32 = AtomicI32::new(0);
static SURFACE_TOP: AtomicI32 = AtomicI32::new(0);
static SURFACE_RIGHT: AtomicI32 = AtomicI32::new(0);
static SURFACE_BOTTOM: AtomicI32 = AtomicI32::new(0);

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

pub fn is_visible() -> bool {
    POPUP_VISIBLE.load(Ordering::SeqCst)
}

pub fn is_closing() -> bool {
    POPUP_CLOSING.load(Ordering::SeqCst)
}

/// GPUI thread only: records the lifecycle reached by the native window.
pub(crate) fn set_lifecycle(visible: bool, closing: bool) {
    POPUP_VISIBLE.store(visible, Ordering::SeqCst);
    POPUP_CLOSING.store(closing, Ordering::SeqCst);
}

/// GPUI thread only: the capsule is now visible, so the press that opened it
/// must not count as an outside click.
pub(crate) fn arm_outside_click_grace() {
    IGNORE_OUTSIDE_UNTIL_MS.store(now_ms() + SHOW_GRACE_MS, Ordering::SeqCst);
    BUTTON_WAS_DOWN.store(true, Ordering::SeqCst);
}

/// GPUI thread only: publishes the capsule rectangle in screen pixels.
pub(crate) fn publish_surface_bounds(left: i32, top: i32, right: i32, bottom: i32) {
    SURFACE_LEFT.store(left, Ordering::SeqCst);
    SURFACE_TOP.store(top, Ordering::SeqCst);
    SURFACE_RIGHT.store(right, Ordering::SeqCst);
    SURFACE_BOTTOM.store(bottom, Ordering::SeqCst);
}

pub(crate) fn set_client_width_dip(width: i32) {
    CLIENT_WIDTH_DIP.store(width, Ordering::SeqCst);
}

pub fn client_width_dip() -> i32 {
    CLIENT_WIDTH_DIP.load(Ordering::SeqCst)
}

pub fn bottom_bar_size() -> BottomBarSize {
    BottomBarSize::from_index(i32::from(BOTTOM_BAR_SIZE.load(Ordering::SeqCst)))
}

pub fn corner_radius_dip() -> i32 {
    CORNER_RADIUS_DIP.load(Ordering::SeqCst)
}

pub fn background_material() -> PopupBackgroundMaterial {
    PopupBackgroundMaterial::from_index(i32::from(POPUP_BACKGROUND_MATERIAL.load(Ordering::SeqCst)))
}

/// Apply appearance values that affect the popup chrome. Startup calls this
/// before the GPUI thread exists; later calls repaint the live popup at once.
pub fn apply_popup_appearance(
    size: BottomBarSize,
    radius: PopupCornerRadius,
    background_material: PopupBackgroundMaterial,
) {
    let size_changed =
        BOTTOM_BAR_SIZE.swap(size.index() as u8, Ordering::SeqCst) != size.index() as u8;
    let radius_changed = CORNER_RADIUS_DIP.swap(radius.dip(), Ordering::SeqCst) != radius.dip();
    let material = background_material.index() as u8;
    let material_changed = POPUP_BACKGROUND_MATERIAL.swap(material, Ordering::SeqCst) != material;
    if size_changed || radius_changed || material_changed {
        crate::popup_window::send_command(crate::popup_window::PopupCommand::AppearanceChanged);
    }
}

/// Show the popup near a tray click (physical screen pixels).
pub fn show_near(anchor_x: i32, anchor_y: i32) {
    // Optimistic: the tray bridge reads this flag before the GPUI thread has
    // processed the command, and a second tray click must toggle it closed.
    POPUP_VISIBLE.store(true, Ordering::SeqCst);
    POPUP_CLOSING.store(false, Ordering::SeqCst);
    arm_outside_click_grace();
    crate::popup_window::send_command(crate::popup_window::PopupCommand::Show {
        anchor: Some((anchor_x, anchor_y)),
    });
}

/// Show the popup beside the current pointer location.
///
/// Settings opened from the tray menu do not carry a tray-click position, but
/// the pointer still gives the expected monitor and taskbar anchor.
pub fn show_near_cursor() {
    let (x, y) = cursor_position();
    show_near(x, y);
}

/// Show the popup on the primary monitor's tray corner.
///
/// Stream Deck keys have no display of their own; following the cursor would
/// drop the flyout on whichever screen the mouse happens to occupy.
pub fn show_on_primary() {
    let (x, y) = primary_monitor_center();
    show_near(x, y);
}

pub fn hide() {
    if !is_visible() || is_closing() {
        return;
    }
    POPUP_CLOSING.store(true, Ordering::SeqCst);
    crate::popup_window::send_command(crate::popup_window::PopupCommand::Hide);
}

pub fn toggle_near(anchor_x: i32, anchor_y: i32) {
    if is_visible() && !is_closing() {
        hide();
    } else {
        show_near(anchor_x, anchor_y);
    }
}

/// Respect the Windows "Animation effects" accessibility preference.
#[cfg(windows)]
pub fn system_animations_enabled() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
    };
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

#[cfg(not(windows))]
pub fn system_animations_enabled() -> bool {
    true
}

pub fn animations_enabled() -> bool {
    crate::theme::animations_enabled()
}

/// Dispatch messages for the thread that owns the tray icon.
#[cfg(windows)]
pub fn pump_messages() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage,
    };
    unsafe {
        let mut message = std::mem::zeroed::<MSG>();
        while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

#[cfg(not(windows))]
pub fn pump_messages() {}

#[cfg(windows)]
fn cursor_position() -> (i32, i32) {
    use windows_sys::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};
    let mut cursor = POINT { x: 0, y: 0 };
    unsafe {
        GetCursorPos(&mut cursor);
    }
    (cursor.x, cursor.y)
}

#[cfg(not(windows))]
fn cursor_position() -> (i32, i32) {
    (0, 0)
}

#[cfg(windows)]
fn primary_monitor_center() -> (i32, i32) {
    use windows_sys::Win32::{
        Foundation::{POINT, RECT},
        Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint},
    };
    unsafe {
        let monitor = MonitorFromPoint(
            POINT {
                x: i32::MIN,
                y: i32::MIN,
            },
            MONITOR_DEFAULTTOPRIMARY,
        );
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
        GetMonitorInfoW(monitor, &mut info);
        let bounds = info.rcMonitor;
        (
            bounds.left + (bounds.right - bounds.left) / 2,
            bounds.top + (bounds.bottom - bounds.top) / 2,
        )
    }
}

#[cfg(not(windows))]
fn primary_monitor_center() -> (i32, i32) {
    (0, 0)
}

#[cfg(windows)]
fn any_mouse_button_down() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON,
    };
    unsafe {
        [VK_LBUTTON, VK_MBUTTON, VK_RBUTTON]
            .into_iter()
            .any(|button| GetAsyncKeyState(button as i32) < 0)
    }
}

#[cfg(not(windows))]
fn any_mouse_button_down() -> bool {
    false
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SurfaceRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl SurfaceRect {
    pub(crate) fn contains(self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

fn surface_rect() -> SurfaceRect {
    SurfaceRect {
        left: SURFACE_LEFT.load(Ordering::SeqCst),
        top: SURFACE_TOP.load(Ordering::SeqCst),
        right: SURFACE_RIGHT.load(Ordering::SeqCst),
        bottom: SURFACE_BOTTOM.load(Ordering::SeqCst),
    }
}

/// Detect a new mouse press that lands outside the visible capsule.
pub fn clicked_outside() -> bool {
    if !is_visible() || now_ms() < IGNORE_OUTSIDE_UNTIL_MS.load(Ordering::SeqCst) {
        BUTTON_WAS_DOWN.store(any_mouse_button_down(), Ordering::SeqCst);
        return false;
    }
    let button_is_down = any_mouse_button_down();
    let was_down = BUTTON_WAS_DOWN.swap(button_is_down, Ordering::SeqCst);
    if !button_is_down || was_down {
        return false;
    }
    let surface = surface_rect();
    if surface.right <= surface.left || surface.bottom <= surface.top {
        return false;
    }
    let (x, y) = cursor_position();
    !surface.contains(x, y)
}

/// Rising edge of Escape while the transient popup is visible.
#[cfg(windows)]
pub fn escape_pressed() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE};
    if !is_visible() {
        ESCAPE_WAS_DOWN.store(false, Ordering::SeqCst);
        return false;
    }
    let down = unsafe { GetAsyncKeyState(VK_ESCAPE as i32) < 0 };
    let was_down = ESCAPE_WAS_DOWN.swap(down, Ordering::SeqCst);
    down && !was_down
}

#[cfg(not(windows))]
pub fn escape_pressed() -> bool {
    false
}

/// Normalized critically-damped spring curve used for the short open/close
/// slide. It is monotonic, so the capsule never overshoots the monitor seam.
pub(crate) fn critical_spring_curve(progress: f64, omega: f64) -> f64 {
    let t = progress.clamp(0.0, 1.0);
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    (1.0 - (1.0 + omega * t) * (-omega * t).exp()).clamp(0.0, 1.0)
}

/// Spring-based entrance: fast response with a soft landing.
pub(crate) fn ease_entrance(progress: f64) -> f64 {
    critical_spring_curve(progress, 8.0)
}

/// Spring-based gentle exit: the surface gains speed as it leaves.
pub(crate) fn ease_exit(progress: f64) -> f64 {
    critical_spring_curve(progress, 7.0)
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
    fn surface_hit_test_excludes_the_right_and_bottom_edges() {
        let surface = SurfaceRect {
            left: 100,
            top: 200,
            right: 480,
            bottom: 700,
        };
        assert!(surface.contains(100, 200));
        assert!(surface.contains(479, 699));
        assert!(!surface.contains(480, 300));
        assert!(!surface.contains(300, 700));
        assert!(!surface.contains(99, 300));
    }
}
