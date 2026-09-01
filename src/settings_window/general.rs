use super::persistence::{persist_bool, persist_update};
use super::*;

pub(super) fn render(ctx: &SettingsPageContext<'_>) -> (&'static str, Vec<Element>) {
    let start_at_login = ctx.start_at_login;
    let limit_refresh_interval = ctx.limit_refresh_interval;
    let set_start_at_login = ctx.set_start_at_login.clone();
    let set_limit_refresh_interval = ctx.set_limit_refresh_interval.clone();
    let hovered_card_id = ctx.hovered_card_id;
    let set_hovered_card_id = ctx.set_hovered_card_id.clone();
    let settings_tx = ctx.settings_tx.clone();
    let apply_start_at_login = settings_tx.clone();
    let apply_limit_refresh_interval = settings_tx.clone();
    (
        "General",
        vec![
            settings_toggle_card(
                "Start with Windows",
                start_at_login,
                move |value| {
                    persist_bool(
                        set_start_at_login.clone(),
                        apply_start_at_login.clone(),
                        value,
                        |settings, value| {
                            settings.start_at_login = value;
                        },
                    );
                },
                "general-startup",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("general-startup"),
            settings_control_card(
                "Refresh limits",
                None,
                ComboBox::new([
                    "30 seconds",
                    "1 minute",
                    "5 minutes",
                    "10 minutes",
                    "15 minutes",
                ])
                .selected_index(limit_refresh_interval.index())
                .on_selection_changed(move |choice: i32| {
                    let value = LimitRefreshInterval::from_index(choice);
                    set_limit_refresh_interval.call(value);
                    persist_update(apply_limit_refresh_interval.clone(), move |settings| {
                        settings.limit_refresh_interval = value;
                    });
                }),
                "general-limit-refresh-interval",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("general-limit-refresh-interval"),
        ],
    )
}
