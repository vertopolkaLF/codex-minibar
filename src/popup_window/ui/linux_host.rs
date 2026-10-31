//! Linux popup host.
//!
//! Windows keeps one technical host window alive and clips it to the
//! capsule. GPUI cannot hide windows or make their transparent parts
//! click-through on Linux, so here each show opens a window exactly the size
//! of the capsule and each hide removes it. The [`PopupRoot`] entity outlives
//! those windows, so pages, measurements and cached snapshots survive a
//! close just like they do on Windows.
//!
//! On X11 the window opens next to the tray click. Wayland clients cannot
//! position windows; the compositor places it. Floating panels stay open
//! on focus loss and use compositor-driven header dragging. Regular popups
//! close on focus loss unless pinned.

use std::sync::Arc;

use futures::StreamExt;
use gpui::{
    App, AppContext, AsyncApp, Bounds, Entity, WindowAppearance, WindowBackgroundAppearance,
    WindowBounds, WindowDecorations, WindowHandle, WindowKind, WindowOptions, point, px, size,
};

use super::{Monitor, Placement, PopupRoot, Rect};
use crate::popup_window::{AppState, PopupCommand};

/// Room left for top and bottom panels, which GPUI does not report.
const PANEL_RESERVE: i32 = 40;

struct Host {
    root: Entity<PopupRoot>,
    window: Option<WindowHandle<PopupRoot>>,
}

pub(super) fn start(
    state: Arc<AppState>,
    default_font: gpui::SharedString,
    mut commands: futures::channel::mpsc::UnboundedReceiver<PopupCommand>,
    cx: &mut App,
) {
    let dark = matches!(
        cx.window_appearance(),
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    );
    let settings_state = Arc::clone(&state);
    let root = cx.new(|cx| PopupRoot::detached(state, default_font, dark, cx));
    cx.spawn(async move |cx| {
        let mut host = Host { root, window: None };
        while let Some(command) = commands.next().await {
            if let PopupCommand::Settings(command) = command {
                crate::settings_window::handle(command, &settings_state, cx);
                continue;
            }
            host.handle(command, cx);
            host.close_if_hidden(cx);
        }
    })
    .detach();
}

impl Host {
    fn handle(&mut self, command: PopupCommand, cx: &mut AsyncApp) {
        match command {
            PopupCommand::Publish(ui) => match self.window {
                Some(window) => {
                    let _ =
                        window.update(cx, |root, window, cx| root.apply_ui(*ui, Some(window), cx));
                }
                None => {
                    let _ = self
                        .root
                        .update(cx, |root, cx| root.apply_ui(*ui, None, cx));
                }
            },
            PopupCommand::Show { anchor, .. } => self.show(anchor, cx),
            PopupCommand::Hide => {
                let _ = self.root.update(cx, |root, cx| root.begin_hide(cx));
            }
            PopupCommand::SelectView(view) => {
                let _ = self.root.update(cx, |root, cx| root.select_view(view, cx));
            }
            PopupCommand::SelectNearest(provider) => {
                let _ = self.root.update(cx, |root, cx| {
                    if let Some(view) = root.nearest_view(provider, cx) {
                        root.select_view(view, cx);
                    }
                });
            }
            PopupCommand::StartHomeEditing => {
                let _ = self
                    .root
                    .update(cx, |root, cx| root.set_home_editing(true, cx));
            }
            PopupCommand::AppearanceChanged => {
                if let Some(window) = self.window {
                    let _ =
                        window.update(cx, |root, window, cx| root.appearance_changed(window, cx));
                }
            }
            // The window follows the capsule by itself.
            PopupCommand::Reposition | PopupCommand::Settings(_) => {}
        }
    }

    fn show(&mut self, anchor: Option<(i32, i32)>, cx: &mut AsyncApp) {
        if self.window.is_some() {
            self.activate(cx);
            return;
        }
        let Ok(monitor) = cx.update(|cx| primary_monitor(cx)) else {
            return;
        };
        let Ok((width, height)) = self
            .root
            .update(cx, |root, cx| root.prepare_linux_open(monitor, cx))
        else {
            return;
        };
        let origin = window_origin(monitor, anchor, (width, height));
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(origin.0 as f32), px(origin.1 as f32)),
                size: size(px(width.round()), px(height.round())),
            })),
            titlebar: None,
            focus: true,
            show: true,
            // X11 never focuses PopUp (notification-type) windows, and
            // focus is how the popup learns about outside clicks.
            kind: WindowKind::Normal,
            is_movable: true,
            is_resizable: false,
            is_minimizable: false,
            display_id: None,
            window_background: WindowBackgroundAppearance::Transparent,
            app_id: Some(crate::linux::identity::APP_ID.into()),
            window_min_size: None,
            window_max_size: None,
            // The capsule draws its own rounded frame.
            window_decorations: Some(WindowDecorations::Client),
            tabbing_identifier: None,
        };
        let root = self.root.clone();
        let opened = cx.update(|cx| {
            cx.open_window(options, move |window, cx| {
                root.update(cx, |root, cx| root.attach_window(window, cx));
                root
            })
        });
        let window = match opened {
            Ok(Ok(window)) => window,
            Ok(Err(error)) | Err(error) => {
                eprintln!("could not open the popup window: {error:#}");
                crate::popup::set_lifecycle(false, false);
                return;
            }
        };
        self.window = Some(window);
        let placement = Placement {
            left: true,
            top: true,
        };
        let _ = window.update(cx, |root, window, cx| {
            root.begin_show(monitor, placement, window, cx);
            window.set_always_on_top(root.ui.popup_pinned);
            activate_window(window);
        });
    }

    fn activate(&self, cx: &mut AsyncApp) {
        if let Some(window) = self.window {
            let _ = window.update(cx, |_, window, _| activate_window(window));
        }
    }

    /// Removes the window once the popup reached `Hidden`.
    fn close_if_hidden(&mut self, cx: &mut AsyncApp) {
        let Some(window) = self.window else {
            return;
        };
        let visible = self
            .root
            .read_with(cx, |root, _| root.host.visible())
            .unwrap_or(false);
        if !visible {
            self.window = None;
            let _ = window.update(cx, |_, window, _| window.remove_window());
        }
    }
}

fn activate_window(window: &gpui::Window) {
    if let Some(token) = crate::linux::shortcuts::take_activation_token() {
        window.activate_window_with_token(token);
    } else {
        window.activate_window();
    }
}

/// The primary display in GPUI's logical pixels, minus panel room.
fn primary_monitor(cx: &App) -> Monitor {
    let bounds = cx
        .primary_display()
        .map(|display| display.bounds())
        .unwrap_or_else(|| Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(1920.0), px(1080.0)),
        });
    let left = f32::from(bounds.origin.x).round() as i32;
    let top = f32::from(bounds.origin.y).round() as i32;
    let right = left + f32::from(bounds.size.width).round() as i32;
    let bottom = top + f32::from(bounds.size.height).round() as i32;
    Monitor {
        bounds: Rect {
            left,
            top,
            right,
            bottom,
        },
        work: Rect {
            left,
            top: top + PANEL_RESERVE,
            right,
            bottom: bottom - PANEL_RESERVE,
        },
        // Display bounds are already logical pixels.
        dpi: 96,
    }
}

/// Where the window opens on X11 (Wayland compositors ignore it): beside the
/// tray click, on the half of the screen the click came from, or in the
/// top-right corner where most Linux panels keep their tray.
fn window_origin(monitor: Monitor, anchor: Option<(i32, i32)>, size: (f32, f32)) -> (i32, i32) {
    let margin = crate::popup::EDGE_MARGIN_PX;
    let work = monitor.work;
    let (width, height) = (size.0.round() as i32, size.1.round() as i32);
    let max_left = (work.right - margin - width).max(work.left);
    let max_top = (work.bottom - margin - height).max(work.top);
    let Some((x, y)) = anchor.filter(|anchor| *anchor != (0, 0)) else {
        return (max_left, work.top + margin);
    };
    let left = (x - width / 2).clamp((work.left + margin).min(max_left), max_left);
    let center_y = monitor.bounds.top + monitor.bounds.height() / 2;
    let top = if y < center_y {
        work.top + margin
    } else {
        max_top
    };
    (left, top)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor() -> Monitor {
        Monitor {
            bounds: Rect {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            },
            work: Rect {
                left: 0,
                top: 40,
                right: 1920,
                bottom: 1040,
            },
            dpi: 96,
        }
    }

    #[test]
    fn opens_top_right_without_a_click_position() {
        assert_eq!(window_origin(monitor(), None, (380.0, 600.0)), (1520, 60));
        assert_eq!(
            window_origin(monitor(), Some((0, 0)), (380.0, 600.0)),
            (1520, 60)
        );
    }

    #[test]
    fn follows_the_tray_click_and_stays_on_screen() {
        // A top-panel tray near the right edge.
        assert_eq!(
            window_origin(monitor(), Some((1900, 10)), (380.0, 600.0)),
            (1520, 60)
        );
        // A bottom-panel tray near the left edge.
        assert_eq!(
            window_origin(monitor(), Some((30, 1070)), (380.0, 600.0)),
            (20, 420)
        );
    }
}
