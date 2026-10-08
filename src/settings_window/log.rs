//! Log: troubleshooting entry points and a live tail of log.txt.

use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};

use super::kit::{self, Button, Kit, Row};
use super::window::SettingsWindow;
use crate::settings::ProviderKind;

impl SettingsWindow {
    pub(super) fn log_page(&mut self, k: &mut Kit, cx: &mut Context<Self>) -> Vec<AnyElement> {
        if self.log.is_empty() {
            self.log = crate::logger::tail_lines(100)
                .unwrap_or_else(|error| error.to_string())
                .into();
        }
        let actions = kit::card_of(k, |k| {
            vec![
                Row::new(
                    "log-troubleshoot",
                    crate::i18n::tr("run-troubleshoot-with-ai"),
                )
                .icon(kit::row_icon(k, "sparkle-fill"))
                .description(
                    k,
                    crate::i18n::tr(
                        "let-an-installed-ai-cli-read-the-log-and-investigate-a-problem",
                    ),
                )
                .trailing(
                    Button::new("log-troubleshoot", crate::i18n::tr("choose-tool"))
                        .accent()
                        .on_click(Self::h(cx, |this, (), _, cx| this.open_troubleshoot(cx)))
                        .render(k),
                )
                .render(k),
                Row::new("log-file", crate::i18n::tr("application-log"))
                    .icon(kit::row_icon(k, "file-text-fill"))
                    .description(k, crate::i18n::tr("log-txt-in-the-app-data-folder"))
                    .trailing(
                        Button::new("log-open-file", crate::i18n::tr("open-log-txt"))
                            .on_click(kit::handler(|(), _, _| {
                                if let Err(error) = crate::logger::open() {
                                    eprintln!("failed to open log.txt: {error:#}");
                                    crate::notifications::show(
                                        crate::i18n::tr("could-not-open-log-txt"),
                                        &error.to_string(),
                                    );
                                }
                            }))
                            .render(k),
                    )
                    .trailing(
                        Button::icon_only("log-open-folder", "folder-open-fill")
                            .tooltip(crate::i18n::tr("open-logs-folder"))
                            .on_click(kit::handler(|(), _, _| {
                                if let Err(error) = crate::logger::open_folder() {
                                    eprintln!("failed to open logs folder: {error:#}");
                                    crate::notifications::show(
                                        crate::i18n::tr("could-not-open-logs-folder"),
                                        &error.to_string(),
                                    );
                                }
                            }))
                            .render(k),
                    )
                    .render(k),
            ]
        });
        let theme = &k.theme;
        let text = if self.log.is_empty() {
            crate::i18n::tr("no-log-events-yet").into()
        } else {
            self.log.clone()
        };
        let tail = div()
            .id("log-tail")
            .h(px(380.0))
            .w_full()
            .overflow_y_scroll()
            .rounded(px(kit::CARD_RADIUS))
            .bg(theme.card)
            .p(px(14.0))
            .font_family(theme.mono_font.clone())
            .text_size(px(12.0))
            .line_height(px(18.0))
            .text_color(theme.text_secondary)
            .child(text)
            .into_any_element();
        vec![
            actions,
            kit::section_heading(k, crate::i18n::tr("live-tail")),
            tail,
        ]
    }

    fn open_troubleshoot(&mut self, cx: &mut gpui::Context<Self>) {
        // Troubleshooting uses the CLIs configured on the first instances.
        let binary = |driver: ProviderKind| {
            self.settings
                .instances
                .iter()
                .find(|instance| instance.driver == driver)
                .and_then(|instance| instance.binary_path.clone())
        };
        let tools = crate::troubleshoot::available_tools(
            binary(ProviderKind::Codex).as_deref(),
            binary(ProviderKind::Claude).as_deref(),
        );
        if tools.is_empty() {
            crate::notifications::show(
                crate::i18n::tr("no-supported-ai-tool-found"),
                crate::i18n::tr("install-codex-or-claude-code-and-make-it-available-to-minibar"),
            );
        } else {
            self.troubleshoot = Some(crate::troubleshoot::ToolPickerState::new(tools));
            cx.notify();
        }
    }
}
