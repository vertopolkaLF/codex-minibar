//! About & Updates: hero, update status and project links.

use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px, relative,
};

use super::kit::{self, Button, Kit, Row, eid};
use super::window::{SettingsWindow, install_update, open_url};
use crate::updater::{ISSUES_URL, RELEASES_URL, REPO_URL, UpdatePhase, current_version};

fn update_status_label(phase: &UpdatePhase) -> String {
    match phase {
        UpdatePhase::Idle => crate::i18n::tr("check-github-for-a-new-version").into(),
        UpdatePhase::Checking => crate::i18n::tr("checking-for-updates").into(),
        UpdatePhase::UpToDate => crate::i18n::tr("you-re-up-to-date").into(),
        UpdatePhase::Available(update) => {
            crate::i18n::format("update-available", &[("v0", update.version.to_string())])
        }
        UpdatePhase::Applying => crate::i18n::tr("installing-update").into(),
        // Never surface raw transport errors (e.g. "GET https://...").
        UpdatePhase::Failed(_) => crate::i18n::tr("couldn-t-check-for-updates").into(),
    }
}

impl SettingsWindow {
    pub(super) fn about_page(&mut self, k: &mut Kit, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let theme = k.theme.clone();
        let hero = div()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(8.0))
            .pb(px(24.0))
            .child(kit::image("color/app-icon.png", 104.0).mb(px(14.0)))
            .child(
                div()
                    .text_size(px(28.0))
                    .line_height(px(34.0))
                    .font_weight(FontWeight::BOLD)
                    .child("Codex Minibar"),
            )
            .child(
                div()
                    .mt(px(8.0))
                    .max_w(px(420.0))
                    .text_center()
                    .text_size(px(14.0))
                    .line_height(px(20.0))
                    .text_color(theme.text_secondary)
                    .child(crate::i18n::tr("about-tagline")),
            )
            .child(
                div()
                    .mt(px(14.0))
                    .h(px(22.0))
                    .px(px(10.0))
                    .flex()
                    .items_center()
                    .rounded_full()
                    .bg(theme.accent_soft)
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.accent_text)
                    .child(crate::i18n::format(
                        "version",
                        &[("v0", current_version().to_string())],
                    )),
            )
            .into_any_element();

        let phase = self.update_phase.clone();
        let check_for_updates = self.settings.check_for_updates;
        let notify = self.settings.notifications.update_available;
        let mut update_rows = Vec::new();
        let status_icon = match phase {
            UpdatePhase::UpToDate => kit::icon("check-circle-fill", 18.0, theme.success),
            UpdatePhase::Available(_) => kit::icon("download-simple-fill", 18.0, theme.accent_text),
            UpdatePhase::Failed(_) => kit::icon("warning-fill", 18.0, theme.caution),
            _ => kit::icon("arrow-clockwise-bold", 18.0, theme.glyph()),
        };
        let status = Row::new("about-status", update_status_label(&phase)).icon(
            div()
                .size(px(20.0))
                .flex()
                .items_center()
                .justify_center()
                .child(status_icon)
                .into_any_element(),
        );
        if matches!(phase, UpdatePhase::Available(_)) {
            update_rows.push(
                status
                    .description(k, crate::i18n::tr("a-new-release-is-ready-to-install"))
                    .trailing(
                        Button::new("about-whats-new", crate::i18n::tr("what-s-new"))
                            .on_click(kit::handler(|(), _, _| {
                                if let Err(error) = crate::updater::open_release_notes() {
                                    eprintln!("failed to open release notes: {error:#}");
                                }
                            }))
                            .render(k),
                    )
                    .trailing(
                        Button::new("about-update", crate::i18n::tr("update"))
                            .accent()
                            .on_click(kit::handler(|(), _, _| install_update()))
                            .render(k),
                    )
                    .render(k),
            );
        } else {
            let busy = matches!(phase, UpdatePhase::Checking | UpdatePhase::Applying);
            update_rows.push(
                status
                    .trailing(
                        Button::new("about-check", crate::i18n::tr("check-for-updates"))
                            .accent()
                            .disabled(busy)
                            .on_click(Self::h(cx, |this, (), _, _| {
                                this.state.updates.check_async(
                                    false,
                                    this.settings.notifications.update_available,
                                );
                            }))
                            .render(k),
                    )
                    .render(k),
            );
        }
        update_rows.push(kit::toggle_row(
            k,
            "about-check-startup",
            crate::i18n::tr("check-for-updates-on-startup"),
            None,
            check_for_updates,
            Self::h(cx, |this, value: bool, _, cx| {
                this.edit(cx, move |s| s.check_for_updates = value)
            }),
        ));
        update_rows.push(kit::toggle_row(
            k,
            "about-notify",
            crate::i18n::tr("notify-when-a-new-version-is-found"),
            None,
            notify,
            Self::h(cx, |this, value: bool, _, cx| {
                this.edit(cx, move |s| s.notifications.update_available = value)
            }),
        ));
        let updates = kit::card(k, update_rows);

        let tile = |id: &'static str,
                    title: &'static str,
                    detail: &'static str,
                    glyph: &'static str,
                    url: &'static str| {
            let (tile, t) = kit::hoverable(k, div().id(eid(id)), kit::hover_key(id));
            tile.w(relative(0.5))
                .p(px(6.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(14.0))
                        .p(px(16.0))
                        .rounded(px(kit::CARD_RADIUS))
                        .bg(kit::blend(theme.card, theme.card_hover, t))
                        .shadow(kit::card_shadow(&theme))
                        .child(
                            div()
                                .size(px(36.0))
                                .rounded(px(8.0))
                                .bg(theme.accent_soft)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(kit::icon(glyph, 18.0, theme.accent_text)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .gap(px(1.0))
                                .child(
                                    div()
                                        .text_size(px(14.0))
                                        .line_height(px(20.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(title),
                                )
                                .child(
                                    kit::text(detail, 12.0, theme.text_secondary)
                                        .line_height(px(16.0)),
                                ),
                        )
                        .child(kit::icon("arrow-square-out", 14.0, theme.text_tertiary)),
                )
                .cursor_pointer()
                .on_click(move |_, _, _| open_url(url))
                .into_any_element()
        };
        let resources = div()
            .flex()
            .flex_wrap()
            .mx(px(-6.0))
            .child(tile(
                "about-github",
                crate::i18n::tr("github"),
                crate::i18n::tr("source-code"),
                "github-logo-fill",
                REPO_URL,
            ))
            .child(tile(
                "about-releases",
                crate::i18n::tr("releases"),
                crate::i18n::tr("see-what-s-new"),
                "download-simple-fill",
                RELEASES_URL,
            ))
            .child(tile(
                "about-issues",
                crate::i18n::tr("report-an-issue"),
                crate::i18n::tr("found-a-bug"),
                "flag-fill",
                ISSUES_URL,
            ))
            .child(tile(
                "about-author",
                crate::i18n::tr("author"),
                "@vertopolkaLF",
                "at-fill",
                "https://github.com/vertopolkaLF",
            ))
            .into_any_element();

        vec![
            hero,
            kit::section_heading(k, crate::i18n::tr("updates")),
            updates,
            kit::section_heading(k, crate::i18n::tr("resources")),
            resources,
        ]
    }
}
