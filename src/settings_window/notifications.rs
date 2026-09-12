use super::persistence::{persist_bool, persist_u8};
use super::*;

pub(super) fn render(ctx: &SettingsPageContext<'_>) -> (&'static str, Vec<Element>) {
    let activation_success = ctx.activation_success;
    let activation_failure = ctx.activation_failure;
    let limits_reset = ctx.limits_reset;
    let low_usage_enabled = ctx.low_usage_enabled;
    let low_usage_threshold = ctx.low_usage_threshold;
    let low_usage_expanded = ctx.low_usage_expanded;
    let low_usage_expand_progress = ctx.low_usage_expand_progress;
    let weekly_low_usage_enabled = ctx.weekly_low_usage_enabled;
    let weekly_low_usage_threshold = ctx.weekly_low_usage_threshold;
    let weekly_low_usage_expanded = ctx.weekly_low_usage_expanded;
    let weekly_low_usage_expand_progress = ctx.weekly_low_usage_expand_progress;
    let set_activation_success = ctx.set_activation_success.clone();
    let set_activation_failure = ctx.set_activation_failure.clone();
    let set_limits_reset = ctx.set_limits_reset.clone();
    let set_low_usage_enabled = ctx.set_low_usage_enabled.clone();
    let set_low_usage_threshold = ctx.set_low_usage_threshold.clone();
    let set_low_usage_expanded = ctx.set_low_usage_expanded.clone();
    let set_low_usage_expand_progress = ctx.set_low_usage_expand_progress.clone();
    let set_weekly_low_usage_enabled = ctx.set_weekly_low_usage_enabled.clone();
    let set_weekly_low_usage_threshold = ctx.set_weekly_low_usage_threshold.clone();
    let set_weekly_low_usage_expanded = ctx.set_weekly_low_usage_expanded.clone();
    let set_weekly_low_usage_expand_progress = ctx.set_weekly_low_usage_expand_progress.clone();
    let hovered_card_id = ctx.hovered_card_id;
    let set_hovered_card_id = ctx.set_hovered_card_id.clone();
    let settings_tx = ctx.settings_tx.clone();
    let apply_activation_success = settings_tx.clone();
    let apply_activation_failure = settings_tx.clone();
    let apply_limits_reset = settings_tx.clone();
    let apply_low_usage_enabled = settings_tx.clone();
    let apply_low_usage_threshold = settings_tx.clone();
    let apply_weekly_low_usage_enabled = settings_tx.clone();
    let apply_weekly_low_usage_threshold = settings_tx.clone();
    (
        "Notifications",
        vec![
            settings_toggle_card(
                "Successful activations",
                activation_success,
                move |value| {
                    persist_bool(
                        set_activation_success.clone(),
                        apply_activation_success.clone(),
                        value,
                        |settings, value| {
                            settings.notifications.activation_success = value;
                        },
                    );
                },
                "notif-activation-success",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("notif-activation-success"),
            settings_toggle_card(
                "Failed activations",
                activation_failure,
                move |value| {
                    persist_bool(
                        set_activation_failure.clone(),
                        apply_activation_failure.clone(),
                        value,
                        |settings, value| {
                            settings.notifications.activation_failure = value;
                        },
                    );
                },
                "notif-activation-failure",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("notif-activation-failure"),
            settings_toggle_card(
                "When limits reset",
                limits_reset,
                move |value| {
                    persist_bool(
                        set_limits_reset.clone(),
                        apply_limits_reset.clone(),
                        value,
                        |settings, value| {
                            settings.notifications.limits_changed = value;
                        },
                    );
                },
                "notif-limits-reset",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("notif-limits-reset"),
            settings_toggle_expander(
                format!("When 5-hour remaining hits {low_usage_threshold}%"),
                None,
                low_usage_enabled,
                move |value| {
                    persist_bool(
                        set_low_usage_enabled.clone(),
                        apply_low_usage_enabled.clone(),
                        value,
                        |settings, value| {
                            settings.notifications.low_usage_enabled = value;
                        },
                    );
                },
                low_usage_expanded,
                low_usage_expand_progress,
                Some(78.0),
                set_low_usage_expanded,
                set_low_usage_expand_progress,
                "notif-low-usage",
                hovered_card_id,
                set_hovered_card_id.clone(),
                settings_slider_content("Threshold", low_usage_threshold, 5, 50, 5, {
                    let set_low_usage_threshold = set_low_usage_threshold.clone();
                    let apply_low_usage_threshold = apply_low_usage_threshold.clone();
                    move |value: f64| {
                        let percent = value.round().clamp(5.0, 50.0) as u8;
                        if percent == low_usage_threshold {
                            return;
                        }
                        persist_u8(
                            set_low_usage_threshold.clone(),
                            apply_low_usage_threshold.clone(),
                            percent,
                            |settings, value| {
                                settings.notifications.low_usage_threshold_percent = value;
                            },
                        );
                    }
                }),
            )
            .with_key("notif-low-usage"),
            settings_toggle_expander(
                format!("When weekly remaining hits {weekly_low_usage_threshold}%"),
                None,
                weekly_low_usage_enabled,
                move |value| {
                    persist_bool(
                        set_weekly_low_usage_enabled.clone(),
                        apply_weekly_low_usage_enabled.clone(),
                        value,
                        |settings, value| {
                            settings.notifications.weekly_low_usage_enabled = value;
                        },
                    );
                },
                weekly_low_usage_expanded,
                weekly_low_usage_expand_progress,
                Some(78.0),
                set_weekly_low_usage_expanded,
                set_weekly_low_usage_expand_progress,
                "notif-weekly-low-usage",
                hovered_card_id,
                set_hovered_card_id.clone(),
                settings_slider_content("Threshold", weekly_low_usage_threshold, 5, 50, 5, {
                    let set_weekly_low_usage_threshold = set_weekly_low_usage_threshold.clone();
                    let apply_weekly_low_usage_threshold = apply_weekly_low_usage_threshold.clone();
                    move |value: f64| {
                        let percent = value.round().clamp(5.0, 50.0) as u8;
                        if percent == weekly_low_usage_threshold {
                            return;
                        }
                        persist_u8(
                            set_weekly_low_usage_threshold.clone(),
                            apply_weekly_low_usage_threshold.clone(),
                            percent,
                            |settings, value| {
                                settings.notifications.weekly_low_usage_threshold_percent = value;
                            },
                        );
                    }
                }),
            )
            .with_key("notif-weekly-low-usage"),
        ],
    )
}
