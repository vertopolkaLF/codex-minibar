#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::time::Duration;

use anyhow::{Result, anyhow};
use codex_minibar::{
    app::MinibarApp,
    codex::{CodexActivator, CodexClient, first_available},
    settings::Settings,
    worker::start_worker,
};
use eframe::egui;

fn run() -> Result<()> {
    let path = Settings::default_path()?;
    let settings = Settings::load_or_create(&path)?;
    let executable = first_available(settings.codex_path.as_deref());
    let (worker, startup_error) = match executable {
        Ok(executable) => {
            let state_path = path.with_file_name("activation.toml");
            let worker = start_worker(
                CodexClient::new(&executable),
                CodexActivator::new(executable),
                state_path,
                settings.automatic_activation,
                Duration::from_secs(60),
            );
            (Some(worker), None)
        }
        Err(error) => (None, Some(error.to_string())),
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Codex Minibar")
            .with_inner_size([380.0, 420.0])
            .with_min_inner_size([340.0, 380.0])
            .with_resizable(false)
            // The popup supplies its own rounded chrome, so do not let Windows
            // draw a title bar or frame around it.
            .with_decorations(false)
            .with_transparent(true)
            .with_taskbar(false)
            .with_active(false)
            .with_always_on_top()
            .with_visible(false),
        ..Default::default()
    };
    eframe::run_native(
        "Codex Minibar",
        native_options,
        Box::new(move |creation_context| {
            Ok(Box::new(MinibarApp::new(
                creation_context,
                settings,
                worker,
                startup_error,
            )))
        }),
    )
    .map_err(|error| anyhow!(error.to_string()))
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Codex Minibar failed: {error:#}");
    }
}
