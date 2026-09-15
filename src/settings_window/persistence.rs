use super::platform::choose_settings_file;
use super::*;

pub(super) fn persist_bool(
    setter: SetState<bool>,
    settings_tx: Sender<Settings>,
    value: bool,
    update: impl FnOnce(&mut Settings, bool),
) {
    setter.call(value);
    persist_update(settings_tx, |settings| update(settings, value));
}

pub(super) fn persist_u8(
    setter: SetState<u8>,
    settings_tx: Sender<Settings>,
    value: u8,
    update: impl FnOnce(&mut Settings, u8),
) {
    setter.call(value);
    persist_update(settings_tx, |settings| update(settings, value));
}

pub(crate) fn persist_update(settings_tx: Sender<Settings>, update: impl FnOnce(&mut Settings)) {
    if let Err(error) = try_persist_update(settings_tx, update) {
        eprintln!("failed to save settings: {error:#}");
    }
}

/// Credential dialogs must report persistence failures instead of displaying
/// a success notice when the account list could not be saved.
pub(crate) fn try_persist_update(
    settings_tx: Sender<Settings>,
    update: impl FnOnce(&mut Settings),
) -> anyhow::Result<()> {
    try_persist_update_fallible(settings_tx, |settings| {
        update(settings);
        Ok(())
    })
}

pub(crate) fn try_persist_update_fallible(
    settings_tx: Sender<Settings>,
    update: impl FnOnce(&mut Settings) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    Settings::default_path().and_then(|path| {
        let mut settings = Settings::load_or_create(&path)?;
        update(&mut settings)?;
        settings.normalize_tray_widgets();
        settings.normalize_popup_visibility();
        // Persist first so a flaky side effect cannot block live UI updates.
        settings.save(&path)?;
        if let Err(error) = settings.apply_runtime_effects() {
            eprintln!("failed to apply runtime settings effects: {error:#}");
        }
        // Disk persistence is the transaction boundary. A disconnected live
        // listener means the UI is shutting down; it must not make callers
        // roll back secrets after the settings file has already committed.
        if let Err(error) = settings_tx.send(settings) {
            eprintln!("failed to notify live settings listeners: {error}");
        }
        Ok(())
    })
}

pub(super) fn replace_settings(
    settings_tx: Sender<Settings>,
    mut settings: Settings,
) -> anyhow::Result<()> {
    let path = Settings::default_path()?;
    settings.normalize_tray_widgets();
    settings.save(&path)?;
    if let Err(error) = settings.apply_runtime_effects() {
        eprintln!("failed to apply runtime settings effects: {error:#}");
    }
    settings_tx
        .send(settings)
        .context("notify live settings listeners")?;
    Ok(())
}

pub(super) fn export_settings() -> anyhow::Result<()> {
    let Some(path) = choose_settings_file(true)? else {
        return Ok(());
    };
    let current_path = Settings::default_path()?;
    Settings::load_or_create(&current_path)?.save(&path)
}

pub(super) fn import_settings() -> anyhow::Result<Option<Settings>> {
    let Some(path) = choose_settings_file(false)? else {
        return Ok(None);
    };
    Settings::load_or_create(&path).map(Some)
}
