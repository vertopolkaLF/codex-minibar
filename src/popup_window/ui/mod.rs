//! GPUI implementation of the tray popup.
//!
//! GPUI runs on a dedicated thread with its own message loop; WinUI keeps the
//! process main thread for the Settings windows. The two only talk through
//! [`super::PopupCommand`] (into GPUI) and the `UiMarshaller` (into WinUI).

mod activity;
mod assets;
mod cards;
mod components;
mod controls;
mod footer;
mod fx;
mod home;
#[cfg(test)]
pub(crate) use home::donut_segments;
#[cfg(windows)]
mod backdrop;
#[cfg(windows)]
mod blur_effect;
mod root;
mod theme;
mod tooltip;
mod usage;
#[cfg(windows)]
mod win32;

use std::sync::Arc;

use futures::StreamExt;
use gpui::{
    AppContext, Application, Bounds, SharedString, WindowBackgroundAppearance, WindowBounds,
    WindowKind, WindowOptions, point, px, size,
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

/// Whether popup motion runs: the app toggle and Windows "Animation effects".
pub(crate) fn animations_enabled(ui: &UiState) -> bool {
    ui.animations_enabled && crate::popup::system_animations_enabled()
}

/// The fixed technical host: anchored to the monitor's right edge, standing
/// on the work area's bottom margin, wide and tall enough for every layout.
#[cfg(windows)]
pub(crate) fn host_rect(monitor: Monitor) -> win32::Rect {
    let scale = monitor.scale();
    let margin = crate::popup::EDGE_MARGIN_PX;
    let wide = f64::from(monitor.bounds.width() - margin * 2) / scale
        >= f64::from(crate::popup::POPUP_WIDE_WIDTH);
    let width_dip = if wide {
        crate::popup::POPUP_WIDE_WIDTH
    } else {
        crate::popup::POPUP_WIDTH
    };
    let width = (f64::from(width_dip) * scale).round() as i32 + margin;
    let height = (f64::from(monitor.bounds.height()) * crate::popup::POPUP_SCREEN_HEIGHT_FRACTION)
        .round() as i32;
    let bottom = monitor.work.bottom - margin;
    win32::Rect {
        left: monitor.bounds.right - width,
        top: (bottom - height).max(monitor.work.top),
        right: monitor.bounds.right,
        bottom,
    }
}

/// Resolve a font family once: Segoe UI Variable on Windows 11, Segoe UI on
/// older systems that do not ship the variable font.
fn pick_font_family(cx: &mut gpui::App) -> SharedString {
    let names = cx.text_system().all_font_names();
    for candidate in ["Segoe UI Variable Text", "Segoe UI Variable", "Segoe UI"] {
        if names.iter().any(|name| name == candidate) {
            return SharedString::from(candidate);
        }
    }
    SharedString::from("Segoe UI")
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
        .with_assets(assets::PopupAssets)
        .run(move |cx| {
            let font_family = pick_font_family(cx);
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
                window_decorations: None,
                tabbing_identifier: None,
            };
            let window = match cx.open_window(options, |window, cx| {
                cx.new(|cx| PopupRoot::new(state, font_family, window, cx))
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
                        .update(cx, |_, window, _| win32::hwnd_from_window(window))
                        .ok()
                        .flatten();
                    if let Some(hwnd) = hwnd {
                        win32::configure(hwnd);
                        let _ = window.update(cx, |root, _, _| {
                            root.attach_native_host(hwnd);
                        });
                        win32::park_hidden(hwnd);
                    }
                }
                while let Some(command) = commands.next().await {
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
            let _ = window.update(cx, |root, window, cx| root.apply_ui(*ui, window, cx));
        }
        PopupCommand::Show { anchor } => {
            #[cfg(windows)]
            {
                let plan = window
                    .update(cx, |root, window, cx| root.begin_show(anchor, window, cx))
                    .ok()
                    .flatten();
                let Some(plan) = plan else {
                    crate::popup::set_lifecycle(false, false);
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
                if !root_animations(window, cx) {
                    win32::activate(plan.hwnd);
                }
            }
            #[cfg(not(windows))]
            {
                let _ = anchor;
            }
        }
        PopupCommand::Hide => {
            let _ = window.update(cx, |root, _, cx| root.begin_hide(cx));
        }
        PopupCommand::SelectView(view) => {
            let _ = window.update(cx, |root, _, cx| root.select_view(view, cx));
        }
        PopupCommand::AppearanceChanged => {
            let _ = window.update(cx, |root, window, cx| root.appearance_changed(window, cx));
        }
        PopupCommand::Reposition => {
            #[cfg(windows)]
            {
                let plan = window
                    .update(cx, |root, _, _| {
                        let hwnd = root.host.hwnd?;
                        if !root.host.visible() {
                            return None;
                        }
                        let rect = win32::window_rect(hwnd);
                        let monitor = win32::monitor_for_point(rect.right - 1, rect.bottom - 1);
                        root.host.monitor = Some(monitor);
                        root.host.scale = monitor.scale() as f32;
                        Some((hwnd, host_rect(monitor)))
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
    }
}

fn root_animations(window: &gpui::WindowHandle<PopupRoot>, cx: &mut gpui::AsyncApp) -> bool {
    window
        .update(cx, |root, _, _| animations_enabled(&root.ui))
        .unwrap_or(false)
}
