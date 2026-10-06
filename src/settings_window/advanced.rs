//! Advanced: settings import/export, clearing Usage data, factory reset.

use gpui::{AnyElement, Context, PathPromptOptions};

use super::kit::{self, Button, Kit, Row};
use super::persistence::replace_settings;
use super::window::SettingsWindow;
use crate::settings::Settings;
use crate::worker::UsageAction;

fn action_row(
    k: &Kit,
    id: &str,
    icon: &str,
    title: &str,
    description: &str,
    button: Button,
) -> AnyElement {
    Row::new(format!("row-{id}"), title.to_owned())
        .icon(kit::row_icon(k, icon))
        .description(k, description.to_owned())
        .trailing(button.render(k))
        .render(k)
}

impl SettingsWindow {
    pub(super) fn advanced_page(&mut self, k: &mut Kit, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let backup = kit::card_of(k, |k| {
            vec![
                action_row(
                    k,
                    "advanced-export",
                    "export-fill",
                    "Export settings",
                    "Save every setting to a .toml file. Saved keys stay in Windows user storage.",
                    Button::new("advanced-export", "Export")
                        .on_click(Self::h(cx, |this, (), _, cx| this.export_settings(cx))),
                ),
                action_row(
                    k,
                    "advanced-import",
                    "upload-simple-fill",
                    "Import settings",
                    "Replace the current settings with a previously exported file.",
                    Button::new("advanced-import", "Import")
                        .on_click(Self::h(cx, |this, (), _, cx| this.import_settings(cx))),
                ),
            ]
        });
        let data = kit::card_of(k, |k| {
            vec![
                action_row(
                    k,
                    "advanced-clear",
                    "broom-fill",
                    "Clear Usage data",
                    "Delete the collected Usage history. It is rebuilt from local provider logs on the next scan.",
                    Button::new("advanced-clear", "Clear").on_click(Self::h(
                        cx,
                        |this, (), _, cx| {
                            if let Err(error) =
                                this.state.usage_actions_tx.send(UsageAction::ClearData)
                            {
                                eprintln!("failed to queue usage data clear: {error}");
                                crate::notifications::show(
                                    "Usage data clear failed",
                                    "The background worker is unavailable.",
                                );
                            } else {
                                this.show_notice("Usage data cleared.", cx);
                            }
                        },
                    )),
                ),
                action_row(
                    k,
                    "advanced-reset",
                    "arrow-counter-clockwise-bold",
                    "Reset all settings",
                    "Restore every default and start the welcome flow again.",
                    Button::new("advanced-reset", "Reset")
                        .danger()
                        .on_click(Self::h(cx, |this, (), _, cx| {
                            this.confirm_reset = true;
                            cx.notify();
                        })),
                ),
            ]
        });
        vec![
            kit::section_heading(k, "Backup"),
            backup,
            kit::section_heading(k, "Data"),
            data,
        ]
    }

    fn export_settings(&mut self, cx: &mut Context<Self>) {
        let directory = directories::UserDirs::new()
            .and_then(|dirs| dirs.document_dir().map(|dir| dir.to_path_buf()))
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        let prompt = cx.prompt_for_new_path(&directory, Some("codex-minibar-settings.toml"));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = prompt.await else {
                return;
            };
            let result = Settings::default_path()
                .and_then(|current| Settings::load_or_create(&current))
                .and_then(|settings| settings.save(&path));
            let _ = this.update(cx, |this, cx| match result {
                Ok(()) => this.show_notice("Settings exported.", cx),
                Err(error) => {
                    eprintln!("failed to export settings: {error:#}");
                    crate::notifications::show("Settings export failed", &format!("{error:#}"));
                }
            });
        })
        .detach();
    }

    fn import_settings(&mut self, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = prompt.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                let result = Settings::load_or_create(&path).and_then(|settings| {
                    replace_settings(this.settings_tx(), settings.clone())?;
                    Ok(settings)
                });
                match result {
                    Ok(settings) => {
                        this.settings = settings;
                        this.show_notice("Settings imported.", cx);
                    }
                    Err(error) => {
                        eprintln!("failed to import settings: {error:#}");
                        crate::notifications::show("Settings import failed", &format!("{error:#}"));
                    }
                }
            });
        })
        .detach();
    }

    pub(super) fn reset_confirm_overlay(
        &self,
        k: &Kit,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.confirm_reset {
            return None;
        }
        let cancel = Self::h(cx, |this, (), _, cx| {
            this.confirm_reset = false;
            cx.notify();
        });
        let reset = Self::h(cx, |this, (), window, cx| {
            this.confirm_reset = false;
            let settings = Settings::default();
            match replace_settings(this.settings_tx(), settings.clone()) {
                Ok(()) => {
                    this.settings = settings;
                    // Resetting returns to the same first-launch path as a
                    // new install; onboarding replaces this window.
                    window.remove_window();
                    super::window_closed(false);
                    super::open_onboarding();
                }
                Err(error) => {
                    eprintln!("failed to reset settings: {error:#}");
                    crate::notifications::show("Settings reset failed", &format!("{error:#}"));
                }
            }
            cx.notify();
        });
        Some(kit::dialog(
            k,
            "reset",
            420.0,
            vec![
                kit::dialog_title(k, "Reset all settings?"),
                kit::caption(
                    k,
                    "Every setting returns to its default and the welcome flow opens again. Saved keys and Usage data are kept.",
                ),
            ],
            vec![
                Button::new("reset-confirm", "Reset")
                    .accent()
                    .full_width()
                    .on_click(reset)
                    .render(k),
                Button::new("reset-cancel", "Cancel")
                    .full_width()
                    .on_click(cancel.clone())
                    .render(k),
            ],
            Some(cancel),
        ))
    }
}
