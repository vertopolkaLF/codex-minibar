use super::shared::log_view_card;
use super::*;

pub(super) fn render(ctx: &SettingsPageContext<'_>) -> (&'static str, Vec<Element>) {
    let log_content = ctx.log_content;
    let hovered_card_id = ctx.hovered_card_id;
    let set_hovered_card_id = ctx.set_hovered_card_id.clone();
    // Troubleshooting uses the CLIs configured on the first instances.
    let binary = |driver: ProviderKind| {
        ctx.instances
            .iter()
            .find(|instance| instance.driver == driver)
            .and_then(|instance| instance.binary_path.clone())
    };
    let codex_path = binary(ProviderKind::Codex);
    let claude_path = binary(ProviderKind::Claude);
    let set_troubleshoot_picker = ctx.set_troubleshoot_picker.clone();
    (
        "Log",
        vec![
            settings_action_card(
                "Run Troubleshoot with AI",
                "Choose tool",
                move || {
                    let tools = crate::troubleshoot::available_tools(
                        codex_path.as_deref(),
                        claude_path.as_deref(),
                    );
                    if tools.is_empty() {
                        crate::notifications::show(
                            "No supported AI tool found",
                            "Install Codex or Claude Code and make it available to Minibar.",
                        );
                    } else {
                        set_troubleshoot_picker
                            .call(Some(crate::troubleshoot::ToolPickerState::new(tools)));
                    }
                },
                "log-run-troubleshoot",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("log-run-troubleshoot"),
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
