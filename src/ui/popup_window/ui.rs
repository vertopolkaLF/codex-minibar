//! GPUI implementation of the tray popup.
//!
//! GPUI runs on a dedicated thread with its own message loop. It hosts the
//! popup and the Settings/onboarding windows; other threads talk to it only
//! through [`super::PopupCommand`].

mod activity;
mod cards;
pub(crate) mod components;
pub(crate) mod controls;
mod footer;
mod home;
mod keys;
#[cfg(test)]
pub(crate) use home::donut_segments;
#[cfg(windows)]
mod backdrop;
#[cfg(windows)]
mod blur_effect;
mod root;
pub(crate) mod theme;
mod toast;
mod toast_summary;
mod tooltip;
mod usage;
#[cfg(windows)]
pub(crate) mod win32;

use std::sync::Arc;

use futures::StreamExt;
use gpui::{
    AppContext, Application, Bounds, WindowBackgroundAppearance, WindowBounds, WindowKind,
    WindowOptions, point, px, size,
};

pub(crate) use root::PopupRoot;
#[cfg(windows)]
pub(crate) use win32::Monitor;

use super::{AppState, PopupCommand, UiState};

#[cfg(not(windows))]
#[derive(Clone, Copy, Debug)]
pub(crate) struct Monitor {
    pub(crate) bounds: Rect,
    pub(crate) work: Rect,
    pub(crate) dpi: u32,
}

#[cfg(not(windows))]
impl Monitor {
    pub(crate) fn scale(self) -> f64 {
        f64::from(self.dpi.max(1)) / 96.0
    }
}

#[cfg(not(windows))]
#[derive(Clone, Copy, Debug)]
pub(crate) struct Rect {
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) right: i32,
    pub(crate) bottom: i32,
}

#[cfg(not(windows))]
impl Rect {
    pub(crate) fn width(self) -> i32 {
        (self.right - self.left).max(0)
    }

    pub(crate) fn height(self) -> i32 {
        (self.bottom - self.top).max(0)
    }
}

/// Native work to do after an update that prepared a show.
#[cfg(windows)]
pub(crate) struct ShowPlan {
    pub(crate) hwnd: windows_sys::Win32::Foundation::HWND,
    pub(crate) rect: win32::Rect,
    pub(crate) first_show: bool,
}

/// Which corner of the work area the popup stands in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Placement {
    pub(crate) left: bool,
    pub(crate) top: bool,
}

#[cfg(windows)]
impl Placement {
    /// The corner by the taskbar: the tray's end by default, or the end
    /// nearest `anchor` when the popup opens `beside` a taskbar widget.
    pub(crate) fn for_anchor(monitor: Monitor, anchor: (i32, i32), beside: bool) -> Self {
        let bounds = monitor.bounds;
        let edge = taskbar_edge(monitor);
        let vertical = matches!(edge, Edge::Left | Edge::Right);
        let center_x = bounds.left + bounds.width() / 2;
        let center_y = bounds.top + bounds.height() / 2;
        Self {
            left: edge == Edge::Left || (edge != Edge::Right && beside && anchor.0 < center_x),
            top: edge == Edge::Top || (vertical && beside && anchor.1 < center_y),
        }
    }
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Edge {
    Left,
    Top,
    Right,
    Bottom,
}

/// The monitor edge the taskbar is docked to. Its own frame decides, because
/// third-party appbars shrink the work area too; the work-area insets are only
/// a fallback when no taskbar lives on this monitor.
#[cfg(windows)]
fn taskbar_edge(monitor: Monitor) -> Edge {
    let (bounds, work) = (monitor.bounds, monitor.work);
    if let Some(bar) = monitor.taskbar {
        return if bar.height() > bar.width() {
            if bar.left + bar.right < bounds.left + bounds.right {
                Edge::Left
            } else {
                Edge::Right
            }
        } else if bar.top + bar.bottom < bounds.top + bounds.bottom {
            Edge::Top
        } else {
            Edge::Bottom
        };
    }
    if work.left > bounds.left {
        Edge::Left
    } else if work.right < bounds.right {
        Edge::Right
    } else if work.top > bounds.top {
        Edge::Top
    } else {
        Edge::Bottom
    }
}

/// Host width and height in pixels: the capsule plus its margin, as tall as
/// the monitor.
#[cfg(windows)]
fn host_size(monitor: Monitor, wide: bool) -> (i32, i32) {
    let width_dip = if wide {
        crate::ui::popup::POPUP_WIDE_WIDTH
    } else {
        crate::ui::popup::POPUP_WIDTH
    };
    let width = (f64::from(width_dip) * monitor.scale()).round() as i32;
    (
        width + crate::ui::popup::EDGE_MARGIN_PX,
        monitor.bounds.height(),
    )
}

/// The fixed technical host: anchored to the work area's edge on the
/// placement's side, standing on its bottom (or top) margin, wide and tall
/// enough for every layout.
#[cfg(windows)]
pub(crate) fn host_rect(monitor: Monitor, placement: Placement) -> win32::Rect {
    let margin = crate::ui::popup::EDGE_MARGIN_PX;
    let wide = f64::from(monitor.bounds.width() - margin * 2) / monitor.scale()
        >= f64::from(crate::ui::popup::POPUP_WIDE_WIDTH);
    let (width, height) = host_size(monitor, wide);
    let work = monitor.work;
    let (left, right) = if placement.left {
        (work.left, work.left + width)
    } else {
        (work.right - width, work.right)
    };
    let (top, bottom) = if placement.top {
        let top = work.top + margin;
        (top, top + height)
    } else {
        let bottom = work.bottom - margin;
        (bottom - height, bottom)
    };
    win32::Rect {
        left,
        top,
        right,
        bottom,
    }
}

/// Keep the visible origin exactly where the user placed it, even off-screen.
/// Width and height limits must never snap a floating window back into an appbar.
#[cfg(windows)]
pub(crate) fn floating_host_rect(
    monitor: Monitor,
    x: i32,
    y: i32,
    two_columns: bool,
) -> win32::Rect {
    let (width, height) = host_size(monitor, two_columns);
    let left = x.saturating_sub(crate::ui::popup::EDGE_MARGIN_PX);
    win32::Rect {
        left,
        top: y,
        right: left.saturating_add(width),
        bottom: y.saturating_add(height),
    }
}

pub(crate) fn start(
    state: Arc<AppState>,
    commands: futures::channel::mpsc::UnboundedReceiver<PopupCommand>,
) {
    let spawned = std::thread::Builder::new()
        .name("popup-gpui".into())
        .spawn(move || run(state, commands));
    if let Err(error) = spawned {
        eprintln!("could not start the popup UI thread: {error}");
    }
}

fn run(
    state: Arc<AppState>,
    mut commands: futures::channel::mpsc::UnboundedReceiver<PopupCommand>,
) {
    Application::new()
        .with_assets(crate::ui::gui::assets::Assets)
        .run(move |cx| {
            crate::ui::gui::theme::register_bundled_fonts(cx);
            crate::ui::gui::input::bind_keys(cx);
            let default_font = crate::ui::gui::theme::default_font_family(cx);
            let settings_state = Arc::clone(&state);
            // The tray starts hidden. Do not allocate large swap-chain/MSAA
            // surfaces before the user has even opened the popup.
            let initial = size(px(1.0), px(1.0));
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(0.0), px(0.0)),
                    size: initial,
                })),
                titlebar: None,
                focus: false,
                show: false,
                kind: WindowKind::PopUp,
                is_movable: false,
                is_resizable: false,
                is_minimizable: false,
                display_id: None,
                // Never enable HWND-wide Acrylic: it ignores the capsule's
                // region. A separate clipped composition visual supplies blur.
                window_background: WindowBackgroundAppearance::Transparent,
                app_id: None,
                window_min_size: None,
                window_max_size: None,
                window_decorations: None,
                tabbing_identifier: None,
            };
            let window = match cx.open_window(options, |window, cx| {
                cx.new(|cx| PopupRoot::new(state, default_font, window, cx))
            }) {
                Ok(window) => window,
                Err(error) => {
                    eprintln!("could not create the popup window: {error:#}");
                    return;
                }
            };

            cx.spawn(async move |cx| {
                #[cfg(windows)]
                {
                    let hwnd = window
                        .update(cx, |_, window, _| crate::ui::gui::hwnd_from_window(window))
                        .ok()
                        .flatten();
                    if let Some(hwnd) = hwnd {
                        win32::configure(hwnd);
                        let _ = window.update(cx, |root, _, _| {
                            root.attach_native_host(hwnd);
                        });
                        win32::park_hidden(hwnd);
                        cx.spawn(async move |cx| prewarm(window, cx).await).detach();
                    }
                }
                while let Some(command) = commands.next().await {
                    if let PopupCommand::Settings(command) = command {
                        crate::ui::settings_window::handle(command, &settings_state, cx);
                        continue;
                    }
                    handle_command(command, &window, cx);
                }
            })
            .detach();
        });
}

fn handle_command(
    command: PopupCommand,
    window: &gpui::WindowHandle<PopupRoot>,
    cx: &mut gpui::AsyncApp,
) {
    match command {
        PopupCommand::Publish(ui) => {
            #[cfg(windows)]
            let geometry_changed = window
                .update(cx, |root, _, _| {
                    root.ui.settings.popup_floating != ui.settings.popup_floating
                        || root.ui.settings.popup_position != ui.settings.popup_position
                        || (ui.settings.popup_floating
                            && root.ui.settings.popup_two_columns != ui.settings.popup_two_columns)
                })
                .unwrap_or(false);
            let _ = window.update(cx, |root, window, cx| root.apply_ui(*ui, window, cx));
            #[cfg(windows)]
            {
                if geometry_changed {
                    let animated = window
                        .update(cx, |root, _, cx| root.animate_reposition(cx))
                        .unwrap_or(false);
                    if !animated {
                        handle_command(PopupCommand::Reposition, window, cx);
                    }
                }
            }
        }
        PopupCommand::Show { anchor, beside } => {
            #[cfg(windows)]
            {
                let plan = window
                    .update(cx, |root, window, cx| {
                        root.begin_show(anchor, beside, window, cx)
                    })
                    .ok()
                    .flatten();
                let Some(plan) = plan else {
                    crate::ui::popup::set_lifecycle(false, false);
                    return;
                };
                if plan.first_show {
                    // Hide every pixel before the first on-screen frame; the
                    // capsule then slides in from beyond the monitor edge.
                    win32::set_region(plan.hwnd, None, 0);
                }
                win32::place(plan.hwnd, plan.rect);
                // A DPI change during placement makes GPUI apply Windows'
                // suggested rect; place again with the final geometry.
                if win32::window_rect(plan.hwnd) != plan.rect {
                    win32::place(plan.hwnd, plan.rect);
                }
                let _ = window.update(cx, |root, window, cx| {
                    root.host.last_region = None;
                    window.refresh();
                    cx.notify();
                });
                if plan.first_show {
                    win32::show(plan.hwnd);
                }
                if !crate::ui::theme::animations_enabled() {
                    win32::activate(plan.hwnd);
                }
            }
            #[cfg(not(windows))]
            {
                let _ = (anchor, beside);
            }
        }
        PopupCommand::Hide => {
            let _ = window.update(cx, |root, _, cx| root.begin_hide(cx));
        }
        PopupCommand::SelectView(view) => {
            let _ = window.update(cx, |root, _, cx| root.select_view(view, cx));
        }
        PopupCommand::SelectNearest(provider) => {
            let _ = window.update(cx, |root, _, cx| {
                if let Some(view) = root.nearest_view(provider, cx) {
                    root.select_view(view, cx);
                }
            });
        }
        PopupCommand::StartHomeEditing => {
            let _ = window.update(cx, |root, _, cx| root.set_home_editing(true, cx));
        }
        PopupCommand::AppearanceChanged => {
            let _ = window.update(cx, |root, window, cx| root.appearance_changed(window, cx));
        }
        PopupCommand::Settings(_) => {}
        PopupCommand::Toast(notification) => toast::post(*notification, window, cx),
        PopupCommand::Raise => {
            #[cfg(windows)]
            {
                let hwnd = window
                    .update(cx, |root, _, _| {
                        root.host.hwnd.filter(|_| root.host.visible())
                    })
                    .ok()
                    .flatten();
                if let Some(hwnd) = hwnd {
                    win32::raise(hwnd);
                }
            }
        }
        PopupCommand::Reposition => {
            #[cfg(windows)]
            {
                let plan = window
                    .update(cx, |root, _, cx| {
                        let hwnd = root.host.hwnd?;
                        if !root.host.visible() {
                            return None;
                        }
                        let rect = root.prepare_geometry(None, false);
                        root.remember_floating_origin(rect, cx);
                        root.host.last_region = None;
                        cx.notify();
                        Some((hwnd, rect))
                    })
                    .ok()
                    .flatten();
                if let Some((hwnd, rect)) = plan
                    && win32::window_rect(hwnd) != rect
                {
                    win32::place_keep_order(hwnd, rect);
                    let _ = window.update(cx, |root, window, cx| {
                        root.host.last_region = None;
                        window.refresh();
                        cx.notify();
                    });
                }
            }
        }
        PopupCommand::Moved => {
            #[cfg(windows)]
            {
                let _ = window.update(cx, |root, _, cx| root.save_floating_position(cx));
                handle_command(PopupCommand::Reposition, window, cx);
            }
        }
        PopupCommand::ResizeHeight {
            height_dip,
            finished,
            fixed_bottom,
        } => {
            let _ = window.update(cx, |root, _, cx| {
                root.resize_height(height_dip, finished, cx)
            });
            #[cfg(windows)]
            if let Some(bottom) = fixed_bottom {
                let plan = window
                    .update(cx, |root, window, _| {
                        if !root.ui.settings.popup_floating || root.host.phase != root::Phase::Open
                        {
                            return None;
                        }
                        let hwnd = root.host.hwnd?;
                        let old = win32::window_rect(hwnd);
                        let top = bottom
                            - (root.host.height.position * f64::from(window.scale_factor())).round()
                                as i32;
                        let rect = win32::Rect {
                            left: old.left,
                            right: old.right,
                            top,
                            bottom: top + old.height(),
                        };
                        root.host.floating_rect = Some(rect);
                        root.host.last_region = None;
                        Some((hwnd, rect))
                    })
                    .ok()
                    .flatten();
                if let Some((hwnd, rect)) = plan {
                    if win32::window_rect(hwnd) != rect {
                        win32::place_keep_order(hwnd, rect);
                    }
                    if finished {
                        let _ = window.update(cx, |root, _, cx| root.save_floating_position(cx));
                    }
                }
            }
            #[cfg(not(windows))]
            let _ = fixed_bottom;
        }
    }
}

/// The popup is the app's main surface: render it once invisibly at launch so
/// the first tray click gets a warm renderer and a fully played open slide.
/// A real Show during the prewarm simply takes over the already shown host.
#[cfg(windows)]
async fn prewarm(window: gpui::WindowHandle<PopupRoot>, cx: &mut gpui::AsyncApp) {
    use std::time::{Duration, Instant};
    const LIMIT: Duration = Duration::from_secs(3);
    let Some(plan) = window
        .update(cx, |root, window, cx| root.begin_prewarm(window, cx))
        .ok()
        .flatten()
    else {
        return;
    };
    win32::set_region(plan.hwnd, None, 0);
    win32::place(plan.hwnd, plan.rect);
    if win32::window_rect(plan.hwnd) != plan.rect {
        win32::place(plan.hwnd, plan.rect);
    }
    let _ = window.update(cx, |root, window, cx| {
        root.host.last_region = None;
        window.refresh();
        cx.notify();
    });
    win32::show(plan.hwnd);
    let started = Instant::now();
    loop {
        cx.background_executor()
            .timer(Duration::from_millis(50))
            .await;
        let settled = window
            .update(cx, |root, _, _| root.prewarm_settled())
            .unwrap_or(true);
        if settled || started.elapsed() >= LIMIT {
            break;
        }
    }
    let _ = window.update(cx, |root, _, cx| root.end_prewarm(cx));
}

#[cfg(all(test, windows))]
mod tests {
    use super::{Monitor, Placement, win32::Rect};

    fn monitor(work: Rect) -> Monitor {
        Monitor {
            bounds: Rect {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            },
            work,
            taskbar: None,
            dpi: 96,
        }
    }

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
        Rect {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn floating_panel_never_adjusts_user_origin_for_edges_or_column_count() {
        for dpi in [96, 144, 192] {
            let monitor = Monitor {
                dpi,
                ..monitor(rect(40, 0, 1920, 1032))
            };
            let width = |two_columns| super::floating_host_rect(monitor, 0, 0, two_columns).width();
            assert!(width(true) > width(false));
            for (x, y) in [(-8000, -3000), (8000, 3000), (1900, 1000), (700, 300)] {
                for two_columns in [false, true] {
                    let host = super::floating_host_rect(monitor, x, y, two_columns);
                    assert_eq!(host.left + crate::ui::popup::EDGE_MARGIN_PX, x);
                    assert_eq!(host.top, y);
                    // Edges never squeeze the panel.
                    assert_eq!(host.width(), width(two_columns));
                }
            }
        }
    }

    #[test]
    fn floating_panel_height_does_not_change_near_work_area_bottom() {
        let monitor = monitor(rect(0, 0, 1920, 1032));
        let top = super::floating_host_rect(monitor, 800, 100, true);
        let bottom = super::floating_host_rect(monitor, 800, 1000, true);
        assert_eq!(top.height(), bottom.height());
        assert_eq!(bottom.top, 1000);
    }

    #[test]
    fn tray_popup_reserves_full_screen_height_on_both_taskbar_edges() {
        for top in [false, true] {
            let monitor = monitor(if top {
                rect(0, 48, 1920, 1080)
            } else {
                rect(0, 0, 1920, 1032)
            });
            let host = super::host_rect(monitor, Placement { left: false, top });
            assert_eq!(host.height(), monitor.bounds.height());
            if top {
                assert_eq!(
                    host.top,
                    monitor.work.top + crate::ui::popup::EDGE_MARGIN_PX
                );
            } else {
                assert_eq!(
                    host.bottom,
                    monitor.work.bottom - crate::ui::popup::EDGE_MARGIN_PX
                );
            }
            let floating = super::floating_host_rect(monitor, 400, 200, false);
            assert_eq!(floating.height(), monitor.bounds.height());
        }
    }

    #[test]
    fn popup_opens_by_the_taskbar_and_beside_start_widgets() {
        let bottom = monitor(rect(0, 0, 1920, 1032));
        let tray = Placement::default();
        assert_eq!(Placement::for_anchor(bottom, (1800, 1050), false), tray);
        // A tray click never moves the popup, wherever the anchor is.
        assert_eq!(Placement::for_anchor(bottom, (100, 1050), false), tray);
        let left = Placement {
            left: true,
            top: false,
        };
        assert_eq!(Placement::for_anchor(bottom, (100, 1050), true), left);
        assert_eq!(Placement::for_anchor(bottom, (1500, 1050), true), tray);

        let top = monitor(rect(0, 48, 1920, 1080));
        assert_eq!(
            Placement::for_anchor(top, (100, 20), true),
            Placement {
                left: true,
                top: true,
            }
        );
        let side = monitor(rect(62, 0, 1920, 1080));
        assert_eq!(Placement::for_anchor(side, (30, 1000), false), left);
        assert_eq!(
            Placement::for_anchor(side, (30, 100), true),
            Placement {
                left: true,
                top: true,
            }
        );
    }

    #[test]
    fn other_appbars_do_not_move_the_popup_off_the_taskbar() {
        let tray = Placement::default();
        // A top dock plus the bottom taskbar.
        let mut docked = monitor(rect(0, 40, 1920, 1032));
        docked.taskbar = Some(rect(0, 1032, 1920, 1080));
        assert_eq!(Placement::for_anchor(docked, (1800, 1050), false), tray);
        // A left dock plus the bottom taskbar.
        docked.work = rect(100, 0, 1920, 1032);
        assert_eq!(Placement::for_anchor(docked, (1800, 1050), false), tray);
        // A right dock plus the bottom taskbar, beside a Start-side widget.
        docked.work = rect(0, 0, 1820, 1032);
        assert_eq!(
            Placement::for_anchor(docked, (100, 1050), true),
            Placement {
                left: true,
                top: false,
            }
        );
        // A bottom dock plus a top taskbar.
        docked.work = rect(0, 48, 1920, 1000);
        docked.taskbar = Some(rect(0, 0, 1920, 48));
        assert_eq!(
            Placement::for_anchor(docked, (1800, 20), false),
            Placement {
                left: false,
                top: true,
            }
        );
    }
}
