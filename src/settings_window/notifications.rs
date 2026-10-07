//! Notifications: activation results, resets and low-usage thresholds.

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
                    "Successful activations",
                    None,
                    n.activation_success,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |s| s.notifications.activation_success = value)
                    }),
                ),
                kit::toggle_row(
                    k,
                    "notif-activation-failure",
                    "Failed activations",
                    None,
                    n.activation_failure,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |s| s.notifications.activation_failure = value)
                    }),
                ),
                kit::toggle_row(
                    k,
                    "notif-limits-reset",
                    "When limits reset",
                    None,
                    n.limits_changed,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |s| s.notifications.limits_changed = value)
                    }),
                ),
            ]
        });
        let low = self.threshold_card(
            k,
            cx,
            "notif-low-usage",
            format!(
                "When 5-hour remaining hits {}%",
                n.low_usage_threshold_percent
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
            format!(
                "When weekly remaining hits {}%",
                n.weekly_low_usage_threshold_percent
            ),
            n.weekly_low_usage_enabled,
            n.weekly_low_usage_threshold_percent,
            |s, v| s.notifications.weekly_low_usage_enabled = v,
            |s, v| s.notifications.weekly_low_usage_threshold_percent = v,
        );
        vec![activity, kit::section_heading(k, "Low usage"), low, weekly]
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
            Some("Shows a notification once per window when the remaining share drops to the threshold.".into()),
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
        let threshold_row = Row::new(format!("{id}-threshold-row"), "Threshold")
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
