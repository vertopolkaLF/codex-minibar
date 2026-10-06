//! Integrations: the Stream Deck companion installer.

use futures::StreamExt;
use gpui::{AnyElement, Context, IntoElement};

use super::kit::{self, Button, Kit, Row};
use super::window::SettingsWindow;
use crate::streamdeck::InstallPhase;

fn install_description(phase: &InstallPhase) -> &'static str {
    match phase {
        InstallPhase::Idle => {
            "Download the latest Stream Deck companion from GitHub and open its installer."
        }
        InstallPhase::Downloading => "Downloading the latest Stream Deck companion from GitHub...",
        InstallPhase::Launching => "Opening the Stream Deck installer...",
        InstallPhase::Launched => "The installer is open. Finish the installation in Stream Deck.",
        InstallPhase::Failed(_) => "The plugin could not be downloaded or opened. Try again.",
    }
}

fn install_button_label(phase: &InstallPhase) -> &'static str {
    match phase {
        InstallPhase::Downloading => "Downloading...",
        InstallPhase::Launching => "Opening...",
        _ => "Install plugin",
    }
}

impl SettingsWindow {
    pub(super) fn integrations_page(
        &mut self,
        k: &mut Kit,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let phase = &self.streamdeck_phase;
        let busy = matches!(phase, InstallPhase::Downloading | InstallPhase::Launching);
        let install = Self::h(cx, |_, (), _, cx| {
            let (tx, mut rx) = futures::channel::mpsc::unbounded();
            crate::streamdeck::install_latest_plugin_async(move |phase| {
                let _ = tx.unbounded_send(phase);
            });
            cx.spawn(async move |this, cx| {
                while let Some(phase) = rx.next().await {
                    if this
                        .update(cx, |this, cx| {
                            this.streamdeck_phase = phase;
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .detach();
        });
        let mut row = Row::new("integrations-streamdeck", "Stream Deck companion")
            .icon(kit::row_icon(k, "package-fill"))
            .description(k, install_description(phase));
        if matches!(phase, InstallPhase::Failed(_)) {
            row = row.detail(
                kit::text(
                    "Check your connection, then try again.",
                    12.0,
                    k.theme.caution,
                )
                .into_any_element(),
            );
        }
        let row = row
            .trailing(
                Button::new("integrations-streamdeck", install_button_label(phase))
                    .accent()
                    .disabled(busy)
                    .on_click(install)
                    .render(k),
            )
            .render(k);
        vec![kit::card(k, vec![row])]
    }
}
