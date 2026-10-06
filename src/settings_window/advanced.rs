use super::onboarding::restart_onboarding_after_reset;
use super::persistence::{export_settings, import_settings, replace_settings};
use super::platform::confirm_settings_reset;
use super::*;

pub(super) fn render(ctx: &SettingsPageContext<'_>) -> (&'static str, Vec<Element>) {
    let hovered_card_id = ctx.hovered_card_id;
    let set_hovered_card_id = ctx.set_hovered_card_id.clone();
    let settings_tx = ctx.settings_tx.clone();
    let usage_actions_tx = ctx.usage_actions_tx.clone();
    let ui_dispatcher = ctx.ui_dispatcher.clone();
    let apply_settings_import = settings_tx.clone();
    let apply_settings_reset = settings_tx.clone();
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
                            super::apply_live_settings(&settings);
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
                "Clear Usage data",
                "Clear",
                move || {
                    if let Err(error) = usage_actions_tx.send(UsageAction::ClearData) {
                        eprintln!("failed to queue usage data clear: {error}");
                        crate::notifications::show(
                            "Usage data clear failed",
                            "The background worker is unavailable.",
                        );
                    }
                },
                "advanced-clear-usage",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("advanced-clear-usage"),
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
                        super::apply_live_settings(&settings);
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
