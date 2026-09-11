//! GPUI application, shared model, and tray/window routing.
mod popup_view;
mod runtime;
mod settings_view;
mod state;

use std::{borrow::Cow, sync::Arc, time::Duration};

use gpui::{prelude::*, *};
use gpui_component::{Root, Theme, ThemeMode};

use crate::{
    settings::{AppTheme, Settings},
    tray::{TrayManager, TrayMenuAction},
};
use runtime::{Command, Runtime, Snapshot};

pub use state::AppState;

struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(asset(path).map(Cow::Borrowed))
    }

    fn list(&self, _: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

fn asset(path: &str) -> Option<&'static [u8]> {
    match path {
        "chatgpt" => Some(include_bytes!("../../assets/icons/chatgpt-iconify.svg")),
        "claude" => Some(include_bytes!("../../assets/icons/claude-iconify.svg")),
        "cursor" => Some(include_bytes!("../../assets/icons/cursor-iconify.svg")),
        "refresh" => Some(include_bytes!(
            "../../assets/icons/fluent-arrow-sync-20-filled.svg"
        )),
        "home" => Some(include_bytes!("../../assets/icons/fluent-home-24-filled.svg")),
        "usage" => Some(include_bytes!(
            "../../assets/icons/fluent-data-histogram-24-filled.svg"
        )),
        "settings" => Some(include_bytes!(
            "../../assets/icons/fluent-settings-20-filled.svg"
        )),
        "quit" => Some(include_bytes!("../../assets/icons/fluent-power-20-filled.svg")),
        "codex" => Some(include_bytes!("../../assets/icons/openai-iconify.svg")),
        "opencode" | "opencode-go" => {
            Some(include_bytes!("../../assets/icons/opencode-iconify.svg"))
        }
        "openrouter" => Some(include_bytes!("../../assets/icons/openrouter-iconify.svg")),
        "nav-general" => Some(include_bytes!("../../assets/icons/ph-house-fill.svg")),
        "nav-appearance" => Some(include_bytes!("../../assets/icons/ph-paint-brush-fill.svg")),
        "nav-providers" => Some(include_bytes!(
            "../../assets/icons/ph-plugs-connected-fill.svg"
        )),
        "nav-popup" => Some(include_bytes!("../../assets/icons/ph-squares-four-fill.svg")),
        "nav-schedule" => Some(include_bytes!("../../assets/icons/ph-clock-fill.svg")),
        "nav-tray" => Some(include_bytes!("../../assets/icons/ph-package-fill.svg")),
        "nav-notifications" => Some(include_bytes!("../../assets/icons/ph-bell-fill.svg")),
        "nav-advanced" => Some(include_bytes!("../../assets/icons/ph-flag-fill.svg")),
        "nav-log" => Some(include_bytes!("../../assets/icons/ph-scroll-fill.svg")),
        "nav-about" => Some(include_bytes!("../../assets/icons/ph-info-fill.svg")),
        _ => None,
    }
}

struct Model {
    runtime: Runtime,
    snapshot: Snapshot,
    settings_generation: u64,
    popup: Option<WindowHandle<popup_view::PopupView>>,
    settings: Option<WindowHandle<Root>>,
}

impl Model {
    fn edit(&mut self, edit: impl FnOnce(&mut Settings), cx: &mut Context<Self>) {
        edit(&mut self.snapshot.settings);
        self.snapshot.settings.normalize_popup_order();
        self.snapshot.settings.normalize_tray_widgets();
        crate::theme::set_animations_enabled(self.snapshot.settings.animations_enabled);
        apply_theme(&self.snapshot.settings, cx);
        self.settings_generation += 1;
        self.runtime.send(Command::Settings(
            Box::new(self.snapshot.settings.clone()),
            self.settings_generation,
        ));
        cx.notify();
    }
}

fn apply_theme(settings: &Settings, cx: &mut App) {
    crate::theme::set_animations_enabled(settings.animations_enabled);
    let dark = match settings.theme {
        AppTheme::Dark => true,
        AppTheme::Light => false,
        AppTheme::Auto => matches!(
            cx.window_appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
    };
    crate::theme::apply_appearance(settings.theme, settings.accent_color);
    Theme::change(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        None,
        cx,
    );
    let [r, g, b] = crate::theme::current_accent_rgb();
    let theme = Theme::global_mut(cx);
    theme.colors.primary = rgb((r as u32) << 16 | (g as u32) << 8 | b as u32).into();
    theme.font_family = "Segoe UI".into();
    theme.font_size = px(13.0);
    theme.radius = px(6.0);
}

fn open_settings(model: Entity<Model>, cx: &mut App) {
    if let Some(handle) = model.read(cx).settings {
        if handle
            .update(cx, |_, window, _| {
                window.activate_window();
            })
            .is_ok()
        {
            return;
        }
    }
    let result = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(760.), px(560.)),
                cx,
            ))),
            window_min_size: Some(size(px(650.), px(460.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Codex Minibar Settings".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(|cx| settings_view::SettingsView::new(model.clone(), window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        },
    );
    match result {
        Ok(handle) => model.update(cx, |model, _| model.settings = Some(handle)),
        Err(error) => crate::logger::info(format!("Settings window failed: {error:#}")),
    }
}

fn tray_widgets(snapshot: &Snapshot) -> Vec<crate::settings::TrayWidget> {
    snapshot
        .settings
        .tray_widgets
        .iter()
        .filter(|widget| widget.is_visible_for(&snapshot.settings.providers))
        .cloned()
        .collect()
}

fn sync_tray(tray: &mut TrayManager, snapshot: &Snapshot) {
    let widgets = tray_widgets(snapshot);
    if let Err(error) = tray.sync(
        &widgets,
        &snapshot.limits,
        matches!(snapshot.update, crate::updater::UpdatePhase::Available(_)),
    ) {
        crate::logger::info(error.to_string());
    }
}

pub fn run(state: Arc<AppState>) -> anyhow::Result<()> {
    let runtime = Runtime::start(state.clone());
    Application::new().with_assets(Assets).run(move |cx| {
        gpui_component::init(cx);
        let initial = runtime.read();
        apply_theme(&initial.settings, cx);
        let onboarding = !initial.settings.onboarding_completed;
        let model = cx.new(|_| Model {
            runtime: runtime.clone(),
            snapshot: initial,
            settings_generation: 0,
            popup: None,
            settings: None,
        });
        let popup_model = model.clone();
        let popup = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(380.), px(520.)),
                        cx,
                    ))),
                    titlebar: None,
                    kind: WindowKind::PopUp,
                    show: false,
                    focus: false,
                    is_resizable: false,
                    is_minimizable: false,
                    is_movable: false,
                    ..Default::default()
                },
                move |window, cx| {
                    window.set_window_title("Codex Minibar");
                    cx.new(|cx| popup_view::PopupView::new(popup_model, window, cx))
                },
            )
            .expect("create GPUI popup");
        model.update(cx, |model, _| model.popup = Some(popup));
        if onboarding {
            open_settings(model.clone(), cx);
        }
        let shutdown = runtime.clone();
        cx.on_app_quit(move |_| {
            shutdown.send(Command::Shutdown);
            async {}
        })
        .detach();
        let mut tray = TrayManager::new();
        let mut last_revision = 0u64;
        let mut last_appearance = cx.window_appearance();
        cx.spawn(async move |cx| {
            let _ = cx.update(|cx| {
                sync_tray(&mut tray, &model.read(cx).snapshot);
                last_revision = model.read(cx).snapshot.revision;
            });
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let result = cx.update(|cx| {
                    let revision = runtime
                        .snapshot
                        .lock()
                        .map(|snapshot| snapshot.revision)
                        .unwrap_or(last_revision);
                    if revision != last_revision {
                        let mut snapshot = runtime.read();
                        apply_theme(&snapshot.settings, cx);
                        model.update(cx, |model, cx| {
                            if snapshot.settings_generation < model.settings_generation {
                                let mut settings = model.snapshot.settings.clone();
                                let _ = settings.absorb_discovered_popup_bricks(&snapshot.limits);
                                snapshot.settings = settings;
                            }
                            model.snapshot = snapshot;
                            cx.notify();
                        });
                        last_revision = revision;
                        sync_tray(&mut tray, &model.read(cx).snapshot);
                    }
                    if cx.window_appearance() != last_appearance {
                        last_appearance = cx.window_appearance();
                        let settings = model.read(cx).snapshot.settings.clone();
                        apply_theme(&settings, cx);
                        model.update(cx, |_, cx| cx.notify());
                    }
                    let snapshot = &model.read(cx).snapshot;
                    let widgets = tray_widgets(snapshot);
                    let _ = tray.refresh_system_theme(&widgets, &snapshot.limits);
                    if let Some(handle) = model.read(cx).settings {
                        if handle.update(cx, |_, _, _| {}).is_err() {
                            model.update(cx, |model, _| model.settings = None);
                        }
                    }
                    while let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
                        if let tray_icon::TrayIconEvent::Click {
                            id,
                            position,
                            button: tray_icon::MouseButton::Left,
                            button_state: tray_icon::MouseButtonState::Up,
                            ..
                        } = event
                        {
                            if tray.contains(&id) {
                                let _ = popup.update(cx, |_, window, _| {
                                    if crate::popup::visible(window) {
                                        crate::popup::hide(window);
                                    } else {
                                        crate::popup::show_near(
                                            window,
                                            position.x as i32,
                                            position.y as i32,
                                        );
                                    }
                                });
                            }
                        }
                    }
                    for action in tray.drain_menu_actions() {
                        match action {
                            TrayMenuAction::Settings => open_settings(model.clone(), cx),
                            TrayMenuAction::Exit => cx.quit(),
                            TrayMenuAction::Update => {
                                let _ = crate::updater::apply_pending_update();
                            }
                        }
                    }
                });
                if result.is_err() {
                    break;
                }
            }
        })
        .detach();
    });
    Ok(())
}
