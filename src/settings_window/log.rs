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
                Row::new("log-troubleshoot", "Run Troubleshoot with AI")
                    .icon(kit::row_icon(k, "sparkle-fill"))
                    .description(
                        k,
                        "Let an installed AI CLI read the log and investigate a problem.",
                    )
                    .trailing(
                        Button::new("log-troubleshoot", "Choose tool")
                            .accent()
                            .on_click(Self::h(cx, |this, (), _, cx| this.open_troubleshoot(cx)))
                            .render(k),
                    )
                    .render(k),
                Row::new("log-file", "Application log")
                    .icon(kit::row_icon(k, "file-text-fill"))
                    .description(k, "log.txt in the app data folder.")
                    .trailing(
                        Button::new("log-open-file", "Open log.txt")
                            .on_click(kit::handler(|(), _, _| {
                                if let Err(error) = crate::logger::open() {
                                    eprintln!("failed to open log.txt: {error:#}");
                                    crate::notifications::show(
                                        "Could not open log.txt",
                                        &error.to_string(),
                                    );
                                }
                            }))
                            .render(k),
                    )
                    .trailing(
                        Button::icon_only("log-open-folder", "folder-open-fill")
                            .tooltip("Open logs folder")
                            .on_click(kit::handler(|(), _, _| {
                                if let Err(error) = crate::logger::open_folder() {
                                    eprintln!("failed to open logs folder: {error:#}");
                                    crate::notifications::show(
                                        "Could not open logs folder",
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
            "No log events yet.".into()
        } else {
            self.log.clone()
        };
        let tail = div()
            .id("log-tail")
            .h(px(380.0))
            .w_full()
            .overflow_y_scroll()
            .rounded(px(kit::CARD_RADIUS))
            .border_1()
            .border_color(theme.card_stroke)
            .bg(theme.card)
            .p(px(14.0))
            .font_family(theme.mono_font.clone())
            .text_size(px(12.0))
            .line_height(px(18.0))
            .text_color(theme.text_secondary)
            .child(text)
            .into_any_element();
        vec![actions, kit::section_heading(k, "Live tail"), tail]
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
                "No supported AI tool found",
                "Install Codex or Claude Code and make it available to Minibar.",
            );
        } else {
            self.troubleshoot = Some(crate::troubleshoot::ToolPickerState::new(tools));
            cx.notify();
        }
    }
}
