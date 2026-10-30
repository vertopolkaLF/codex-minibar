//! Notifications: activation results, resets, low-usage thresholds and
//! every news/status toast, so any notification can be silenced here even
//! when its feature lives on another page.

use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};

use super::kit::{self, Kit, Row, SliderRange};
use super::window::SettingsWindow;
use crate::popup_window::ui::fx;
use crate::settings::Settings;

impl SettingsWindow {
    pub(super) fn notifications_page(
        &mut self,
        k: &mut Kit,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let n = self.settings.notifications.clone();
        let activity = kit::card_of(k, |k| {
            vec![
                kit::toggle_row(
                    k,
                    "notif-activation-success",
                    crate::i18n::tr("successful-activations"),
                    None,
                    n.activation_success,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |s| s.notifications.activation_success = value)
                    }),
                ),
                kit::toggle_row(
                    k,
                    "notif-activation-failure",
                    crate::i18n::tr("failed-activations"),
                    None,
                    n.activation_failure,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |s| s.notifications.activation_failure = value)
                    }),
                ),
                kit::toggle_row(
                    k,
                    "notif-limits-reset",
                    crate::i18n::tr("when-limits-reset"),
                    None,
                    n.limits_changed,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |s| s.notifications.limits_changed = value)
                    }),
                ),
                kit::toggle_row(
                    k,
                    "notif-auto-reset-upcoming",
                    crate::i18n::tr("before-an-automatic-reset"),
                    Some(
                        crate::i18n::tr("warns-before-a-banked-reset-is-applied-automatically")
                            .into(),
                    ),
                    n.auto_reset_upcoming,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |s| s.notifications.auto_reset_upcoming = value)
                    }),
                ),
                kit::toggle_row(
                    k,
                    "notif-game-mode-changes",
                    crate::i18n::tr("notify-game-mode-changes"),
                    Some(crate::i18n::tr("notify-game-mode-changes-description").into()),
                    n.game_mode_changes,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |s| s.notifications.game_mode_changes = value)
                    }),
                ),
            ]
        });
        let news = kit::card_of(k, |k| {
            vec![
                kit::toggle_row(
                    k,
                    "notif-provider-incidents",
                    crate::i18n::tr("notify-about-new-incidents"),
                    Some(crate::i18n::tr("notify-about-new-incidents-description").into()),
                    n.provider_status_notifications,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |s| {
                            s.notifications.provider_status_notifications = value
                        })
                    }),
                ),
                kit::toggle_row(
                    k,
                    "notif-tibo-resets",
                    crate::i18n::tr("notify-when-new-reset-info-arrives"),
                    Some(
                        crate::i18n::tr(
                            "shows-a-notification-when-the-feed-reports-a-possible-reset-never",
                        )
                        .into(),
                    ),
                    n.forced_reset_notifications,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |s| {
                            s.notifications.forced_reset_notifications = value
                        })
                    }),
                ),
                kit::toggle_row(
                    k,
                    "notif-update-available",
                    crate::i18n::tr("notify-when-a-new-version-is-found"),
                    None,
                    n.update_available,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |s| s.notifications.update_available = value)
                    }),
                ),
            ]
        });
        let delivery = kit::card_of(k, |k| {
            vec![
                kit::toggle_row(
                    k,
                    "notif-sound",
                    crate::i18n::tr("notification-sounds"),
                    Some(crate::i18n::tr("plays-a-short-sound-with-each-notification").into()),
                    n.sound,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |s| s.notifications.sound = value)
                    }),
                ),
                kit::toggle_row(
                    k,
                    "notif-follow-do-not-disturb",
                    crate::i18n::tr("follow-do-not-disturb"),
                    Some(crate::i18n::tr("follow-do-not-disturb-description").into()),
                    n.follow_do_not_disturb,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |s| s.notifications.follow_do_not_disturb = value)
                    }),
                ),
            ]
        });
        let low = self.threshold_card(
            k,
            cx,
            "notif-low-usage",
            crate::i18n::format(
                "when-5-hour-remaining-hits",
                &[("v0", n.low_usage_threshold_percent.to_string())],
            ),
            n.low_usage_enabled,
            n.low_usage_threshold_percent,
            |s, v| s.notifications.low_usage_enabled = v,
            |s, v| s.notifications.low_usage_threshold_percent = v,
        );
        let weekly = self.threshold_card(
            k,
            cx,
            "notif-weekly-low-usage",
            crate::i18n::format(
                "when-weekly-remaining-hits",
                &[("v0", n.weekly_low_usage_threshold_percent.to_string())],
            ),
            n.weekly_low_usage_enabled,
            n.weekly_low_usage_threshold_percent,
            |s, v| s.notifications.weekly_low_usage_enabled = v,
            |s, v| s.notifications.weekly_low_usage_threshold_percent = v,
        );
        // How notifications behave comes first, then what they are about.
        vec![
            delivery,
            kit::section_heading(k, crate::i18n::tr("activity")),
            activity,
            kit::section_heading(k, crate::i18n::tr("low-usage")),
            low,
            weekly,
            kit::section_heading(k, crate::i18n::tr("news-and-status")),
            news,
        ]
    }

    #[allow(clippy::too_many_arguments)]
    fn threshold_card(
        &mut self,
        k: &mut Kit,
        cx: &mut Context<Self>,
        id: &'static str,
        title: String,
        enabled: bool,
        threshold: u8,
        write_enabled: fn(&mut Settings, bool),
        write_threshold: fn(&mut Settings, u8),
    ) -> AnyElement {
        let toggle = kit::toggle_row(
            k,
            id,
            title,
            Some(
                crate::i18n::tr(
                    "shows-a-notification-once-per-window-when-the-remaining-share-dro",
                )
                .into(),
            ),
            enabled,
            Self::h(cx, move |this, value: bool, _, cx| {
                this.edit(cx, move |s| write_enabled(s, value))
            }),
        );
        let divider = kit::divider(k);
        let slider = kit::slider(
            k,
            format!("{id}-threshold"),
            f32::from(threshold),
            SliderRange {
                min: 5.0,
                max: 50.0,
                step: 5.0,
            },
            220.0,
            Self::h(cx, move |this, value: f32, _, cx| {
                let percent = value.round().clamp(5.0, 50.0) as u8;
                if percent != threshold {
                    this.edit(cx, move |s| write_threshold(s, percent))
                }
            }),
        );
        let value_label = kit::text(format!("{threshold}%"), 13.0, k.theme.text_secondary)
            .w(px(40.0))
            .text_right()
            .into_any_element();
        let threshold_row = Row::new(format!("{id}-threshold-row"), crate::i18n::tr("threshold"))
            .trailing(slider)
            .trailing(value_label)
            .render(k);
        let reveal = kit::collapsible(k, fx::key(("threshold", id)), enabled, move |_| {
            div()
                .flex()
                .flex_col()
                .child(div().px(px(kit::ROW_PADDING_X)).child(divider))
                .child(threshold_row)
                .into_any_element()
        });
        div()
            .pb(px(4.0))
            .child(kit::card_surface(k, std::iter::once(toggle).chain(reveal)))
            .into_any_element()
    }
}
