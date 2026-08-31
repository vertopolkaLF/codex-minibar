//! Compact limit surface embedded into the Windows taskbar.
//!
//! The host follows the same shell-integration model as FluentFlyout: a normal
//! app window is converted to `WS_CHILD`, parented to Explorer's taskbar HWND,
//! expanded to the taskbar client, and clipped to the widget's rounded hit
//! region. The XAML content remains owned by windows-reactor.

use std::{
    cell::{Cell, RefCell},
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicIsize, Ordering},
    },
    time::Duration,
};

use windows_reactor::*;

use crate::{
    limits::ProviderLimits,
    popup_window::AppState,
    settings::{Settings, TrayWidget, TrayWidgetKind},
};

pub const WINDOW_TITLE: &str = "Codex Minibar Taskbar Widget";

const WIDGET_HEIGHT_DIP: f64 = 40.0;
const PREVIEW_SIZE_DIP: f64 = 32.0;
const CONTENT_PADDING_DIP: f64 = 4.0;
const CONTENT_GAP_DIP: f64 = 2.0;
const LAYOUT_REFRESH: Duration = Duration::from_millis(1500);

thread_local! {
    static HOST: RefCell<Option<Rc<ReactorHost>>> = const { RefCell::new(None) };
    static HOST_ACTIVATED: Cell<bool> = const { Cell::new(false) };
}
static HWND_BITS: AtomicIsize = AtomicIsize::new(0);

#[derive(Clone, Debug, PartialEq)]
pub struct TaskbarWidgetSnapshot {
    pub enabled: bool,
    widgets: Vec<TrayWidget>,
    limits: ProviderLimits,
    accent: [u8; 3],
    system_uses_light_theme: bool,
}

impl TaskbarWidgetSnapshot {
    pub fn from_settings(settings: &Settings, limits: ProviderLimits) -> Self {
        Self {
            enabled: settings.taskbar_widget_enabled,
            widgets: visible_taskbar_widgets(settings),
            limits,
            accent: settings
                .accent_color
                .rgb()
                .map_or_else(crate::theme::current_accent_rgb, |(red, green, blue)| {
                    [red, green, blue]
                }),
            system_uses_light_theme: crate::tray::system_uses_light_theme(),
        }
    }

    pub fn replace_limits(&mut self, limits: ProviderLimits) {
        self.limits = limits;
    }

    pub fn refresh_system_theme(&mut self) -> bool {
        let next = crate::tray::system_uses_light_theme();
        if next == self.system_uses_light_theme {
            return false;
        }
        self.system_uses_light_theme = next;
        true
    }
}

fn visible_taskbar_widgets(settings: &Settings) -> Vec<TrayWidget> {
    let configured = settings
        .tray_widgets
        .iter()
        .filter(|widget| widget.is_visible_for(&settings.providers))
        .cloned()
        .collect::<Vec<_>>();
    if !configured.is_empty() {
        return configured;
    }

    settings
        .ordered_enabled_providers()
        .into_iter()
        .map(TrayWidget::for_provider)
        .find(|widget| widget.kind == TrayWidgetKind::Limits)
        .map_or_else(|| vec![TrayWidget::app_icon()], |widget| vec![widget])
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Placement {
    root_width_dip: f64,
    root_height_dip: f64,
    left_dip: f64,
    top_dip: f64,
    vertical: bool,
}

impl Placement {
    fn bootstrap(widget_width_dip: f64) -> Self {
        Self {
            root_width_dip: widget_width_dip,
            root_height_dip: WIDGET_HEIGHT_DIP,
            left_dip: 0.0,
            top_dip: 0.0,
            vertical: false,
        }
    }
}

pub fn register_host(host: Rc<ReactorHost>) {
    HWND_BITS.store(0, Ordering::Release);
    platform::reset_layout_cache();
    HOST.with(|slot| *slot.borrow_mut() = Some(host));
}

/// Create a fresh hidden reactor host when Explorer has destroyed the previous
/// child window. Safe to call repeatedly from the WinUI dispatcher thread.
pub fn ensure_host(state: Arc<AppState>) -> windows_core::Result<()> {
    if platform::window_exists() {
        return Ok(());
    }
    HOST_ACTIVATED.with(|activated| activated.set(false));
    let render_state = Arc::clone(&state);
    let host = Rc::new(ReactorHost::new_with_window_options(
        WINDOW_TITLE,
        Some(WindowSize {
            width: WIDGET_HEIGHT_DIP,
            height: WIDGET_HEIGHT_DIP,
        }),
        InnerConstraints {
            min_width: Some(1.0),
            min_height: Some(1.0),
            max_width: None,
            max_height: None,
        },
        Box::new(move |_: &(), cx: &mut RenderCx| view(cx, Arc::clone(&render_state))),
        |_| {},
    )?);
    register_host(host);
    Ok(())
}

pub fn view(cx: &mut RenderCx, state: Arc<AppState>) -> Element {
    let initial = state.current_taskbar_widget_snapshot();
    let (snapshot, set_snapshot) = cx.use_async_state(initial);
    let initial_width = content_width(snapshot.widgets.len());
    let (placement, set_placement) = cx.use_async_state(Placement::bootstrap(initial_width));
    let (hovered, set_hovered) = cx.use_state(false);

    cx.use_effect_with_cleanup((), {
        let state = Arc::clone(&state);
        let set_snapshot = set_snapshot.clone();
        move || {
            let seen = Rc::new(Cell::new(
                state.taskbar_widget_revision.load(Ordering::Acquire),
            ));
            let timer = DispatcherTimer::new(Duration::from_millis(50), move || {
                let revision = state.taskbar_widget_revision.load(Ordering::Acquire);
                if revision != seen.get() {
                    seen.set(revision);
                    set_snapshot.call(state.current_taskbar_widget_snapshot());
                }
            })
            .ok();
            Some(move || drop(timer))
        }
    });

    // A vertical legacy taskbar cannot fit a horizontal strip. Keep the first
    // configured metric visible there; horizontal taskbars show the full set.
    let visible_count = if placement.vertical {
        snapshot.widgets.len().min(1)
    } else {
        snapshot.widgets.len()
    };
    let widget_width = content_width(visible_count);

    cx.use_effect_with_cleanup((snapshot.enabled, widget_width.to_bits()), {
        let set_placement = set_placement.clone();
        move || {
            let refresh = {
                let set_placement = set_placement.clone();
                move || {
                    if let Some(next) = platform::sync(snapshot.enabled, widget_width) {
                        set_placement.call(next);
                    }
                }
            };
            refresh();
            let timer = DispatcherTimer::new(LAYOUT_REFRESH, refresh).ok();
            Some(move || drop(timer))
        }
    });

    let previews = snapshot
        .widgets
        .iter()
        .take(visible_count)
        .enumerate()
        .map(|(index, widget)| {
            tray_pixels_element(
                crate::tray::render_widget_with_accent(widget, &snapshot.limits, snapshot.accent),
                format!("{}-{index}", widget.id),
            )
        })
        .collect::<Vec<_>>();
    let strip_identity = snapshot
        .widgets
        .iter()
        .take(visible_count)
        .map(|widget| widget.id.as_str())
        .collect::<Vec<_>>()
        .join("-");
    let strip = hstack(previews)
        .spacing(CONTENT_GAP_DIP)
        .padding(Thickness::uniform(CONTENT_PADDING_DIP))
        .width(widget_width)
        .height(WIDGET_HEIGHT_DIP)
        .horizontal_alignment(HorizontalAlignment::Left)
        .vertical_alignment(VerticalAlignment::Top);

    let idle = border(Element::Empty)
        .background(ThemeRef::ControlFillSecondary)
        .corner_radius(6.0)
        .border_thickness(Thickness::uniform(1.0))
        .border_brush(ThemeRef::ControlStroke)
        .width(widget_width)
        .height(WIDGET_HEIGHT_DIP);
    let hover = border(Element::Empty)
        .background(ThemeRef::SubtleFill)
        .corner_radius(6.0)
        .width(widget_width)
        .height(WIDGET_HEIGHT_DIP)
        .opacity(if hovered { 1.0 } else { 0.0 })
        .with_opacity_transition(crate::theme::duration(
            crate::theme::CONTROL_FASTER_ANIMATION,
        ));
    // SwapChainPanel can consume pointer input. Keep one transparent ordinary
    // XAML layer above every preview so the whole clipped surface is clickable.
    let set_hovered_on_enter = set_hovered.clone();
    let hit_target = border(Element::Empty)
        .background(Color::transparent())
        .width(widget_width)
        .height(WIDGET_HEIGHT_DIP)
        .tooltip("Open Codex Minibar")
        .on_pointer_entered(move |_: PointerEventInfo| set_hovered_on_enter.call(true))
        .on_pointer_exited(move || set_hovered.call(false))
        .on_tapped(crate::popup::show_near_cursor);

    let surface_layers: Vec<Element> =
        vec![idle.into(), hover.into(), strip.into(), hit_target.into()];
    let surface: Element = relative_panel(surface_layers)
        .width(widget_width)
        .height(WIDGET_HEIGHT_DIP)
        .canvas_left(placement.left_dip)
        .canvas_top(placement.top_dip)
        .with_key(format!(
            "taskbar-strip-{strip_identity}-{}",
            snapshot.system_uses_light_theme
        ))
        .into();

    Canvas::new([surface])
        .width(placement.root_width_dip.max(1.0))
        .height(placement.root_height_dip.max(1.0))
        .background(Color::transparent())
        .into()
}

fn content_width(widget_count: usize) -> f64 {
    let count = widget_count.max(1) as f64;
    CONTENT_PADDING_DIP * 2.0 + PREVIEW_SIZE_DIP * count + CONTENT_GAP_DIP * (count - 1.0)
}

fn tray_pixels_element(pixels: Vec<u8>, slot_identity: String) -> Element {
    let mut hasher = DefaultHasher::new();
    pixels.hash(&mut hasher);
    let identity = hasher.finish();
    let mounted_pixels = pixels;
    let mut host = swap_chain_panel()
        .width(PREVIEW_SIZE_DIP)
        .height(PREVIEW_SIZE_DIP);
    host.mounted = Some(Callback::new(move |native: Option<_>| {
        if let Some(native) = native
            && let Err(error) = crate::acrylic::install_tray_pixels_into(native, &mounted_pixels)
        {
            eprintln!("Could not install taskbar metric preview: {error:?}");
        }
    }));
    host.unmounted = Some(Callback::new(move |native: Option<_>| {
        if let Some(native) = native {
            let _ = crate::acrylic::clear_children(native);
        }
    }));
    let element: Element = host.into();
    element.with_key(format!("taskbar-preview-{slot_identity}-{identity:016x}"))
}

fn primary_axis_origin(container: i32, anchor: Option<i32>, extent: i32) -> i32 {
    let max = (container - extent - 4).max(4);
    anchor
        .map(|start| start - extent - 6)
        .unwrap_or(max)
        .clamp(4, max)
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::{cell::RefCell, ffi::c_void, ptr};
    use windows_sys::Win32::{
        Foundation::{HWND, POINT, RECT},
        Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, ScreenToClient, SetWindowRgn},
        UI::{
            HiDpi::GetDpiForWindow,
            WindowsAndMessaging::{
                FindWindowExW, FindWindowW, GWL_EXSTYLE, GWL_STYLE, GetParent, GetWindowLongW,
                GetWindowRect, IsWindow, SW_HIDE, SW_SHOWNOACTIVATE, SWP_ASYNCWINDOWPOS,
                SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
                SetParent, SetWindowLongW, SetWindowPos, ShowWindow, WS_CAPTION, WS_CHILD,
                WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX,
                WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
            },
        },
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct NativeLayout {
        hwnd: isize,
        taskbar: isize,
        container_x: i32,
        container_y: i32,
        container_width: i32,
        container_height: i32,
        region_left: i32,
        region_top: i32,
        region_width: i32,
        region_height: i32,
        dpi: u32,
    }

    thread_local! {
        static LAST_LAYOUT: RefCell<Option<NativeLayout>> = const { RefCell::new(None) };
    }

    pub(super) fn sync(enabled: bool, widget_width_dip: f64) -> Option<Placement> {
        let hwnd = current_hwnd()?;
        if !enabled {
            hide(hwnd);
            return None;
        }
        let taskbar = find_window_by_class("Shell_TrayWnd")?;
        if taskbar.is_null() {
            hide(hwnd);
            return None;
        }

        let needs_parent = unsafe { GetParent(hwnd) } != taskbar;
        let needs_activation = HOST_ACTIVATED.with(|activated| !activated.get());
        if needs_parent || needs_activation {
            configure(hwnd, taskbar);
        }
        if activate_host_once(hwnd, taskbar) {
            // Window::Activate can restore top-level presenter styles. Reassert
            // the Explorer child identity once, not on every layout pulse.
            configure(hwnd, taskbar);
        }

        let mut taskbar_rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if unsafe { GetWindowRect(taskbar, &mut taskbar_rect) } == 0 {
            hide(hwnd);
            return None;
        }
        let width_px = taskbar_rect.right - taskbar_rect.left;
        let height_px = taskbar_rect.bottom - taskbar_rect.top;
        if width_px <= 0 || height_px <= 0 {
            hide(hwnd);
            return None;
        }
        let dpi = unsafe { GetDpiForWindow(taskbar) }.max(96);
        let dip_to_px = |dip: f64| (dip * f64::from(dpi) / 96.0).round() as i32;
        let widget_width_px = dip_to_px(widget_width_dip).max(1);
        let widget_height_px = dip_to_px(WIDGET_HEIGHT_DIP).max(1);
        let vertical = height_px > width_px;

        let tray = unsafe {
            FindWindowExW(
                taskbar,
                ptr::null_mut(),
                wide("TrayNotifyWnd").as_ptr(),
                ptr::null(),
            )
        };
        let tray_rect = (!tray.is_null()).then(|| {
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            (unsafe { GetWindowRect(tray, &mut rect) } != 0).then_some(rect)
        });
        let tray_rect = tray_rect.flatten();

        let (left_px, top_px, region_width_px, region_height_px) = if vertical {
            let anchor = tray_rect.map(|rect| rect.top - taskbar_rect.top);
            (
                ((width_px - widget_width_px) / 2).max(0),
                primary_axis_origin(height_px, anchor, widget_height_px),
                widget_width_px,
                widget_height_px,
            )
        } else {
            let anchor = tray_rect.map(|rect| rect.left - taskbar_rect.left);
            (
                primary_axis_origin(width_px, anchor, widget_width_px),
                ((height_px - widget_height_px) / 2).max(0),
                widget_width_px,
                widget_height_px,
            )
        };

        let mut container_origin = POINT {
            x: taskbar_rect.left,
            y: taskbar_rect.top,
        };
        unsafe {
            ScreenToClient(taskbar, &mut container_origin);
        }
        let layout = NativeLayout {
            hwnd: hwnd as isize,
            taskbar: taskbar as isize,
            container_x: container_origin.x,
            container_y: container_origin.y,
            container_width: width_px,
            container_height: height_px,
            region_left: left_px,
            region_top: top_px,
            region_width: region_width_px,
            region_height: region_height_px,
            dpi,
        };
        let unchanged = LAST_LAYOUT.with(|cached| cached.borrow().as_ref() == Some(&layout));
        if unchanged {
            return Some(placement_from_layout(layout, vertical));
        }

        unsafe {
            SetWindowPos(
                hwnd,
                ptr::null_mut(),
                container_origin.x,
                container_origin.y,
                width_px,
                height_px,
                SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS | SWP_SHOWWINDOW,
            );
        }
        HOST.with(|slot| {
            if let Some(host) = slot.borrow().as_ref() {
                host.sync_render_size(
                    f64::from(width_px) * 96.0 / f64::from(dpi),
                    f64::from(height_px) * 96.0 / f64::from(dpi),
                );
            }
        });

        let radius = dip_to_px(6.0).max(1) * 2;
        let region = unsafe {
            CreateRoundRectRgn(
                left_px,
                top_px,
                left_px + region_width_px + 1,
                top_px + region_height_px + 1,
                radius,
                radius,
            )
        };
        if region.is_null() {
            hide(hwnd);
            return None;
        }
        if unsafe { SetWindowRgn(hwnd, region, 1) } == 0 {
            unsafe {
                DeleteObject(region);
            }
            hide(hwnd);
            return None;
        }
        unsafe {
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        LAST_LAYOUT.with(|cached| *cached.borrow_mut() = Some(layout));

        Some(placement_from_layout(layout, vertical))
    }

    fn placement_from_layout(layout: NativeLayout, vertical: bool) -> Placement {
        Placement {
            root_width_dip: f64::from(layout.container_width) * 96.0 / f64::from(layout.dpi),
            root_height_dip: f64::from(layout.container_height) * 96.0 / f64::from(layout.dpi),
            left_dip: f64::from(layout.region_left) * 96.0 / f64::from(layout.dpi),
            top_dip: f64::from(layout.region_top) * 96.0 / f64::from(layout.dpi),
            vertical,
        }
    }

    pub(super) fn window_exists() -> bool {
        current_hwnd().is_some()
    }

    fn activate_host_once(hwnd: HWND, taskbar: HWND) -> bool {
        HOST_ACTIVATED.with(|activated| {
            if activated.get() {
                return false;
            }
            // Keep the first activation clipped away so WinUI cannot flash a
            // normal top-level surface before Explorer owns it.
            hide_pixels(hwnd);
            configure(hwnd, taskbar);
            let activated_now = HOST.with(|slot| {
                slot.borrow().as_ref().is_some_and(|host| {
                    let _ = host.set_shown_in_switchers(false);
                    host.activate_now().is_ok()
                })
            });
            activated.set(activated_now);
            activated_now
        })
    }

    fn configure(hwnd: HWND, taskbar: HWND) {
        unsafe {
            let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
            let style = (style
                & !(WS_POPUP
                    | WS_CAPTION
                    | WS_THICKFRAME
                    | WS_MINIMIZEBOX
                    | WS_MAXIMIZEBOX
                    | WS_SYSMENU))
                | WS_CHILD;
            SetWindowLongW(hwnd, GWL_STYLE, style as i32);

            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            let ex_style = (ex_style | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) & !WS_EX_APPWINDOW;
            SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style as i32);
            if GetParent(hwnd) != taskbar {
                SetParent(hwnd, taskbar);
            }
            SetWindowPos(
                hwnd,
                ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE,
            );
        }
    }

    fn hide(hwnd: HWND) {
        reset_layout_cache();
        hide_pixels(hwnd);
        unsafe {
            ShowWindow(hwnd, SW_HIDE);
        }
    }

    fn hide_pixels(hwnd: HWND) {
        let empty = unsafe { CreateRoundRectRgn(0, 0, 0, 0, 0, 0) };
        if !empty.is_null() {
            if unsafe { SetWindowRgn(hwnd, empty, 0) } == 0 {
                unsafe {
                    DeleteObject(empty);
                }
            }
        }
    }

    fn current_hwnd() -> Option<HWND> {
        let cached = HWND_BITS.load(Ordering::Acquire) as HWND;
        if !cached.is_null() && unsafe { IsWindow(cached) } != 0 {
            return Some(cached);
        }
        HWND_BITS.store(0, Ordering::Release);
        let hwnd = find_window(WINDOW_TITLE)?;
        HWND_BITS.store(hwnd as isize, Ordering::Release);
        Some(hwnd)
    }

    fn find_window(title: &str) -> Option<HWND> {
        let hwnd = unsafe { FindWindowW(ptr::null(), wide(title).as_ptr()) };
        (!hwnd.is_null()).then_some(hwnd)
    }

    fn find_window_by_class(class_name: &str) -> Option<HWND> {
        let hwnd = unsafe { FindWindowW(wide(class_name).as_ptr(), ptr::null()) };
        (!hwnd.is_null()).then_some(hwnd)
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub(super) fn reset_layout_cache() {
        LAST_LAYOUT.with(|cached| *cached.borrow_mut() = None);
    }

    #[allow(dead_code)]
    fn _assert_hwnd_pointer_shape(_: *mut c_void) {}
}

#[cfg(not(windows))]
mod platform {
    use super::*;

    pub(super) fn sync(_enabled: bool, _widget_width_dip: f64) -> Option<Placement> {
        None
    }

    pub(super) fn window_exists() -> bool {
        false
    }

    pub(super) fn reset_layout_cache() {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{ProviderKind, ProviderSettings};

    #[test]
    fn taskbar_falls_back_to_first_enabled_provider_with_metrics() {
        let settings = Settings {
            providers: ProviderSettings::from_enabled([ProviderKind::Claude]),
            ..Settings::default()
        };
        let widgets = visible_taskbar_widgets(&settings);
        assert_eq!(widgets.len(), 1);
        assert_eq!(widgets[0].kind, TrayWidgetKind::Limits);
        assert!(
            widgets[0]
                .indicators
                .iter()
                .all(|indicator| indicator.provider() == Some(ProviderKind::Claude))
        );
    }

    #[test]
    fn taskbar_mirrors_explicit_visible_tray_widgets() {
        let explicit = TrayWidget::for_provider(ProviderKind::Codex);
        let settings = Settings {
            providers: ProviderSettings::from_enabled([ProviderKind::Codex]),
            tray_widgets: vec![explicit.clone()],
            ..Settings::default()
        };
        assert_eq!(visible_taskbar_widgets(&settings), vec![explicit]);
    }

    #[test]
    fn origin_prefers_the_space_immediately_before_the_tray() {
        assert_eq!(primary_axis_origin(1920, Some(1700), 120), 1574);
        assert_eq!(primary_axis_origin(100, Some(20), 80), 4);
        assert_eq!(primary_axis_origin(1920, None, 120), 1796);
    }
}
