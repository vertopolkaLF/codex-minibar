use super::onboarding::restart_onboarding_after_reset;
use super::persistence::{export_settings, import_settings, replace_settings};
use super::platform::confirm_settings_reset;
use super::*;

pub(super) fn render(ctx: &SettingsPageContext<'_>) -> (&'static str, Vec<Element>) {
    let set_codex_enabled = ctx.set_codex_enabled.clone();
    let set_theme = ctx.set_theme.clone();
    let set_accent_color = ctx.set_accent_color.clone();
    let set_animations_enabled = ctx.set_animations_enabled.clone();
    let set_bottom_bar_size = ctx.set_bottom_bar_size.clone();
    let set_popup_corner_radius = ctx.set_popup_corner_radius.clone();
    let set_popup_background_material = ctx.set_popup_background_material.clone();
    let set_time_format = ctx.set_time_format.clone();
    let set_claude_enabled = ctx.set_claude_enabled.clone();
    let set_cursor_enabled = ctx.set_cursor_enabled.clone();
    let set_opencode_zen_enabled = ctx.set_opencode_zen_enabled.clone();
    let set_opencode_go_enabled = ctx.set_opencode_go_enabled.clone();
    let set_openrouter_enabled = ctx.set_openrouter_enabled.clone();
    let set_openrouter_accounts = ctx.set_openrouter_accounts.clone();
    let set_codex_path = ctx.set_codex_path.clone();
    let set_claude_path = ctx.set_claude_path.clone();
    let set_cursor_path = ctx.set_cursor_path.clone();
    let set_popup_order = ctx.set_popup_order.clone();
    let set_use_colored_provider_icons = ctx.set_use_colored_provider_icons.clone();
    let set_use_colored_sidebar_icons = ctx.set_use_colored_sidebar_icons.clone();
    let set_replace_chatgpt_logo_with_codex = ctx.set_replace_chatgpt_logo_with_codex.clone();
    let set_automatic_activation = ctx.set_automatic_activation.clone();
    let set_scheduled_activations = ctx.set_scheduled_activations.clone();
    let set_auto_activation_pauses = ctx.set_auto_activation_pauses.clone();
    let set_limit_refresh_interval = ctx.set_limit_refresh_interval.clone();
    let set_start_at_login = ctx.set_start_at_login.clone();
    let set_show_used_percentage = ctx.set_show_used_percentage.clone();
    let set_show_usage_pace = ctx.set_show_usage_pace.clone();
    let set_compact_usage_cards = ctx.set_compact_usage_cards.clone();
    let set_popup_visibility = ctx.set_popup_visibility.clone();
    let set_discovered_popup_bricks = ctx.set_discovered_popup_bricks.clone();
    let set_show_total_spend_on_all_tab = ctx.set_show_total_spend_on_all_tab.clone();
    let set_total_spend_presentation = ctx.set_total_spend_presentation.clone();
    let set_show_account_name = ctx.set_show_account_name.clone();
    let set_activation_success = ctx.set_activation_success.clone();
    let set_activation_failure = ctx.set_activation_failure.clone();
    let set_limits_reset = ctx.set_limits_reset.clone();
    let set_low_usage_enabled = ctx.set_low_usage_enabled.clone();
    let set_low_usage_threshold = ctx.set_low_usage_threshold.clone();
    let set_weekly_low_usage_enabled = ctx.set_weekly_low_usage_enabled.clone();
    let set_weekly_low_usage_threshold = ctx.set_weekly_low_usage_threshold.clone();
    let set_tray_widgets = ctx.set_tray_widgets.clone();
    let set_check_for_updates = ctx.set_check_for_updates.clone();
    let set_notify_on_update = ctx.set_notify_on_update.clone();
    let hovered_card_id = ctx.hovered_card_id;
    let set_hovered_card_id = ctx.set_hovered_card_id.clone();
    let settings_tx = ctx.settings_tx.clone();
    let ui_dispatcher = ctx.ui_dispatcher.clone();
    let apply_settings_import = settings_tx.clone();
    let apply_settings_reset = settings_tx.clone();
    let import_state = SettingsWindowState {
        theme: set_theme,
        accent_color: set_accent_color,
        animations_enabled: set_animations_enabled,
        bottom_bar_size: set_bottom_bar_size,
        popup_corner_radius: set_popup_corner_radius,
        popup_background_material: set_popup_background_material,
        time_format: set_time_format,
        codex_enabled: set_codex_enabled,
        claude_enabled: set_claude_enabled,
        cursor_enabled: set_cursor_enabled,
        opencode_zen_enabled: set_opencode_zen_enabled,
        opencode_go_enabled: set_opencode_go_enabled,
        openrouter_enabled: set_openrouter_enabled,
        openrouter_accounts: set_openrouter_accounts,
        codex_path: set_codex_path,
        claude_path: set_claude_path,
        cursor_path: set_cursor_path,
        popup_order: set_popup_order,
        use_colored_provider_icons: set_use_colored_provider_icons,
        use_colored_sidebar_icons: set_use_colored_sidebar_icons,
        replace_chatgpt_logo_with_codex: set_replace_chatgpt_logo_with_codex,
        automatic_activation: set_automatic_activation,
        scheduled_activations: set_scheduled_activations.clone(),
        auto_activation_pauses: set_auto_activation_pauses.clone(),
        limit_refresh_interval: set_limit_refresh_interval,
        start_at_login: set_start_at_login,
        show_used_percentage: set_show_used_percentage,
        show_usage_pace: set_show_usage_pace,
        compact_usage_cards: set_compact_usage_cards,
        popup_visibility: set_popup_visibility,
        discovered_popup_bricks: set_discovered_popup_bricks,
        show_total_spend_on_all_tab: set_show_total_spend_on_all_tab,
        total_spend_presentation: set_total_spend_presentation,
        show_account_name: set_show_account_name,
        activation_success: set_activation_success,
        activation_failure: set_activation_failure,
        limits_reset: set_limits_reset,
        low_usage_enabled: set_low_usage_enabled,
        low_usage_threshold: set_low_usage_threshold,
        weekly_low_usage_enabled: set_weekly_low_usage_enabled,
        weekly_low_usage_threshold: set_weekly_low_usage_threshold,
        tray_widgets: set_tray_widgets,
        check_for_updates: set_check_for_updates,
        notify_on_update: set_notify_on_update,
    };
    let reset_state = import_state.clone();
    let reset_dispatcher = ui_dispatcher.clone();
    (
        "Advanced",
        vec![
            settings_action_card(
                "Export settings",
                "Export",
                || {
                    if let Err(error) = export_settings() {
                        eprintln!("failed to export settings: {error:#}");
                        crate::notifications::show("Settings export failed", &format!("{error:#}"));
                    }
                },
                "advanced-export",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("advanced-export"),
            settings_action_card(
                "Import settings",
                "Import",
                move || {
                    let result = import_settings().and_then(|settings| match settings {
                        Some(settings) => {
                            replace_settings(apply_settings_import.clone(), settings.clone())?;
                            import_state.apply(&settings);
                            Ok(())
                        }
                        None => Ok(()),
                    });
                    if let Err(error) = result {
                        eprintln!("failed to import settings: {error:#}");
                        crate::notifications::show("Settings import failed", &format!("{error:#}"));
                    }
                },
                "advanced-import",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("advanced-import"),
            settings_action_card(
                "Reset all settings",
                "Reset",
                move || {
                    if !confirm_settings_reset() {
                        return;
                    }
                    let settings = Settings::default();
                    if let Err(error) =
                        replace_settings(apply_settings_reset.clone(), settings.clone())
                    {
                        eprintln!("failed to reset settings: {error:#}");
                        crate::notifications::show("Settings reset failed", &format!("{error:#}"));
                    } else {
                        reset_state.apply(&settings);
                        restart_onboarding_after_reset(
                            apply_settings_reset.clone(),
                            reset_dispatcher.clone(),
                        );
                    }
                },
                "advanced-reset",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("advanced-reset"),
        ],
    )
}
