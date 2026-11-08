//! Puts settings into effect outside the settings file: every process-wide
//! mirror, OS registration and live surface.
//!
//! [`start`] runs once at startup, before any window is shown, and routes
//! every later commit of [`crate::settings::store`] through [`apply`], then to
//! the open Settings window and the bridge.

use std::{path::Path, sync::mpsc::Sender};

use anyhow::Result;

use crate::providers::registry::ProviderKind;
use crate::settings::Settings;

/// Loads the settings file at startup, finishing what a plain load cannot do
/// on its own: the credential moves of the instances migration and adopting
/// an existing login registration.
pub fn load_at_startup(path: &Path) -> Result<Settings> {
    let mut settings = Settings::load_or_create_with(path, finish_instance_migration)?;
    // If the installer (or another tool) registered us in HKCU Run while
    // settings still say off, adopt that before applying settings, which
    // would otherwise delete the Run value on launch.
    if !crate::platform::paths::fake_sandbox()
        && !settings.start_at_login
        && startup_registration_present()
    {
        settings.start_at_login = true;
        settings.save(path)?;
    }
    Ok(settings)
}

/// Applies `settings` now and every later commit as it lands.
pub fn start(settings: &Settings, bridge: Sender<Settings>) {
    apply(settings);
    crate::settings::store::listen(move |settings| {
        apply(&settings);
        crate::ui::settings_window::sync_open_window(settings.clone());
        // A closed bridge means the app is shutting down; the file is saved.
        let _ = bridge.send(settings);
    });
}

/// The process-wide values the windows read while painting or dismissing
/// that a window's own edit changed, so the edit shows before it commits.
/// Only changed values apply: a window's snapshot can be older than another
/// window's pending edit, and must not put that edit's value back.
pub fn apply_presentation_changes(before: &Settings, after: &Settings) {
    apply_presentation(Some(before), after);
}

fn apply_presentation(before: Option<&Settings>, settings: &Settings) {
    let changed = |same: fn(&Settings, &Settings) -> bool| {
        before.is_none_or(|before| !same(before, settings))
    };
    if changed(|a, b| a.language == b.language) {
        settings.language.apply();
    }
    if changed(|a, b| a.animations_enabled == b.animations_enabled) {
        crate::ui::theme::set_animations_enabled(settings.animations_enabled);
    }
    if changed(|a, b| a.time_format == b.time_format) {
        settings.time_format.apply();
    }
    if changed(|a, b| a.replace_chatgpt_logo_with_codex == b.replace_chatgpt_logo_with_codex) {
        crate::providers::registry::apply_logo_settings(settings.replace_chatgpt_logo_with_codex);
    }
    if changed(|a, b| a.popup_pinned == b.popup_pinned) {
        crate::ui::popup::set_user_pinned(settings.popup_pinned);
    }
}

/// Everything settings drive outside the windows' own snapshots.
pub fn apply(settings: &Settings) {
    apply_presentation(None, settings);
    crate::ui::theme::apply_appearance(settings.accent_color);
    // Names and badges are read by every surface; publish them first.
    crate::providers::instances::publish(&settings.instances);
    #[cfg(windows)]
    gpui::set_software_rendering(settings.software_rendering);
    crate::surfaces::notifications::set_sound_enabled(settings.notifications.sound);
    crate::surfaces::notifications::set_follow_do_not_disturb(
        settings.notifications.follow_do_not_disturb,
    );
    crate::app::game_mode::apply(
        settings.game_mode,
        settings.notifications.game_mode_changes,
        &settings.game_mode_games,
        &settings.game_mode_ignored,
    );
    crate::ui::popup::apply_popup_appearance(
        settings.bottom_bar_size,
        settings.popup_corner_radius,
        settings.popup_background_material,
    );
    crate::providers::codex::discovery::protect_configured_paths(
        settings
            .instances
            .iter()
            .filter(|instance| instance.driver == ProviderKind::Codex)
            .filter_map(|instance| instance.binary_path.as_deref()),
    );
    crate::surfaces::taskbar::publish_settings(settings);
    crate::surfaces::cmdpal::publish_settings(settings);
    // The sandbox must never rewrite the real install's login entry.
    if !crate::platform::paths::fake_sandbox()
        && let Err(error) = apply_startup_registration(settings.start_at_login)
    {
        crate::platform::logger::info(format!("failed to apply startup registration: {error:#}"));
    }
}

/// Moves saved sign-in sessions out of protected storage into the instances'
/// config folders, and keeps a lone pre-accounts OpenRouter key.
fn finish_instance_migration(settings: &mut Settings) -> bool {
    crate::settings::finish_instance_migration(
        settings,
        |name| crate::platform::secrets::load(name).ok().flatten(),
        |name| {
            if let Err(error) = crate::platform::secrets::save(name, None) {
                crate::platform::logger::info(format!(
                    "failed to remove migrated secret {name}: {error:#}"
                ));
            }
        },
        crate::providers::openrouter::key_is_configured(),
        |instance| crate::providers::instances::managed_folder(&instance.id),
    )
}

#[cfg(windows)]
const STARTUP_RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const STARTUP_VALUE: &str = "Codex Minibar";

#[cfg(windows)]
fn startup_registration_present() -> bool {
    use windows_sys::Win32::System::Registry::HKEY_CURRENT_USER;
    crate::platform::win32::RegKey::open(HKEY_CURRENT_USER, STARTUP_RUN_KEY)
        .is_some_and(|key| key.string(STARTUP_VALUE).is_some())
}

#[cfg(not(windows))]
fn startup_registration_present() -> bool {
    false
}

#[cfg(windows)]
fn apply_startup_registration(enabled: bool) -> Result<()> {
    use anyhow::Context as _;
    let key = crate::platform::win32::RegKey::create_hkcu(STARTUP_RUN_KEY)
        .context("open Windows startup registry key")?;
    let result = if enabled {
        let executable =
            std::env::current_exe().context("resolve current executable for startup")?;
        key.set_string(STARTUP_VALUE, format!("\"{}\"", executable.display()))
    } else {
        key.delete_value(STARTUP_VALUE)
    };
    // Deleting an already-absent Run value is success: the desired end state is
    // "not registered", whether we removed it now or it was never there.
    const ERROR_FILE_NOT_FOUND: u32 = 2;
    match result {
        Ok(()) => Ok(()),
        Err(ERROR_FILE_NOT_FOUND) if !enabled => Ok(()),
        Err(status) => anyhow::bail!("update Windows startup registration: {status}"),
    }
}

#[cfg(not(windows))]
fn apply_startup_registration(_enabled: bool) -> Result<()> {
    Ok(())
}
