use super::*;

fn install_description(phase: &crate::streamdeck::InstallPhase) -> &'static str {
    match phase {
        crate::streamdeck::InstallPhase::Idle => {
            "Download the latest Stream Deck companion from GitHub and open its installer."
        }
        crate::streamdeck::InstallPhase::Downloading => {
            "Downloading the latest Stream Deck companion from GitHub..."
        }
        crate::streamdeck::InstallPhase::Launching => "Opening the Stream Deck installer...",
        crate::streamdeck::InstallPhase::Launched => {
            "The installer is open. Finish the installation in Stream Deck."
        }
        crate::streamdeck::InstallPhase::Failed(_) => {
            "The plugin could not be downloaded or opened. Try again."
        }
    }
}

fn install_button_label(phase: &crate::streamdeck::InstallPhase) -> &'static str {
    match phase {
        crate::streamdeck::InstallPhase::Downloading => "Downloading...",
        crate::streamdeck::InstallPhase::Launching => "Opening...",
        _ => "Install plugin",
    }
}

pub(super) fn render(ctx: &SettingsPageContext<'_>) -> (&'static str, Vec<Element>) {
    let phase = ctx.streamdeck_install_phase;
    let busy = matches!(
        phase,
        crate::streamdeck::InstallPhase::Downloading | crate::streamdeck::InstallPhase::Launching
    );
    let set_phase = ctx.set_streamdeck_install_phase.clone();
    let action = Button::new(install_button_label(phase))
        .accent()
        .enabled(!busy)
        .on_click(move || {
            let set_phase = set_phase.clone();
            crate::streamdeck::install_latest_plugin_async(move |phase| set_phase.call(phase));
        });

    (
        "Integrations",
        vec![
            settings_control_card(
                "Stream Deck companion",
                Some(install_description(phase)),
                action,
                "integrations-streamdeck",
                ctx.hovered_card_id,
                ctx.set_hovered_card_id.clone(),
            )
            .with_key("integrations-streamdeck"),
        ],
    )
}
