//! Integrations: the Stream Deck companion installer.

use futures::StreamExt;
use gpui::{AnyElement, Context, IntoElement};

use super::kit::{self, Button, Kit, Row};
use super::window::SettingsWindow;
use crate::streamdeck::InstallPhase;

fn install_description(phase: &InstallPhase) -> &'static str {
    match phase {
        InstallPhase::Idle => {
            crate::i18n::tr("download-the-latest-stream-deck-companion-from-github-and-open-it")
        }
        InstallPhase::Downloading => {
            crate::i18n::tr("downloading-the-latest-stream-deck-companion-from-github")
        }
        InstallPhase::Launching => crate::i18n::tr("opening-the-stream-deck-installer"),
        InstallPhase::Launched => {
            crate::i18n::tr("the-installer-is-open-finish-the-installation-in-stream-deck")
        }
        InstallPhase::Failed(_) => {
            crate::i18n::tr("the-plugin-could-not-be-downloaded-or-opened-try-again")
        }
    }
}

fn install_button_label(phase: &InstallPhase) -> &'static str {
    match phase {
        InstallPhase::Downloading => crate::i18n::tr("downloading"),
        InstallPhase::Launching => crate::i18n::tr("opening"),
        _ => crate::i18n::tr("install-plugin"),
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
        let mut row = Row::new(
            "integrations-streamdeck",
            crate::i18n::tr("stream-deck-companion"),
        )
        .icon(kit::row_icon(k, "package-fill"))
        .description(k, install_description(phase));
        if matches!(phase, InstallPhase::Failed(_)) {
            row = row.detail(
                kit::text(
                    crate::i18n::tr("check-your-connection-then-try-again"),
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
