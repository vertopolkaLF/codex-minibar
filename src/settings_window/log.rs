use super::shared::log_view_card;
use super::*;

pub(super) fn render(ctx: &SettingsPageContext<'_>) -> (&'static str, Vec<Element>) {
    let log_content = ctx.log_content;
    let hovered_card_id = ctx.hovered_card_id;
    let set_hovered_card_id = ctx.set_hovered_card_id.clone();
    (
        "Log",
        vec![
            settings_action_card(
                "Application log",
                "Open log.txt",
                || {
                    if let Err(error) = crate::logger::open() {
                        eprintln!("failed to open log.txt: {error:#}");
                        crate::notifications::show("Could not open log.txt", &error.to_string());
                    }
                },
                "log-open-file",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("log-open-file"),
            settings_action_card(
                "Logs folder",
                "Open folder",
                || {
                    if let Err(error) = crate::logger::open_folder() {
                        eprintln!("failed to open logs folder: {error:#}");
                        crate::notifications::show(
                            "Could not open logs folder",
                            &error.to_string(),
                        );
                    }
                },
                "log-open-folder",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("log-open-folder"),
            log_view_card(log_content).with_key("log-live-tail"),
        ],
    )
}
