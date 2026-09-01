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
    let result = Settings::default_path().and_then(|path| {
        let mut settings = Settings::load_or_create(&path)?;
        update(&mut settings);
        settings.normalize_tray_widgets();
        settings.normalize_popup_visibility();
        // Persist first so a flaky side effect cannot block live UI updates.
        settings.save(&path)?;
        if let Err(error) = settings.apply_runtime_effects() {
            eprintln!("failed to apply runtime settings effects: {error:#}");
        }
        settings_tx
            .send(settings)
            .context("notify live settings listeners")?;
        Ok(())
    });
    if let Err(error) = result {
        eprintln!("failed to save settings: {error:#}");
    }
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
