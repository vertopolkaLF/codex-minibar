//! Settings persistence shared by the Settings window, onboarding and popup.
//!
//! Every write reloads the file, applies one change, saves, applies runtime
//! effects and broadcasts the result. Settings-window edits go through one
//! serial writer thread so rapid toggles never interleave their
//! load-modify-save cycles and never block the UI thread on disk I/O.

use std::sync::{
    LazyLock, Mutex,
    atomic::{AtomicUsize, Ordering},
    mpsc::{self, Sender},
};

use anyhow::Context as _;

use crate::settings::Settings;

type Edit = Box<dyn FnOnce(&mut Settings) + Send>;
type Job = (Sender<Settings>, Edit);

/// Edits queued but not yet committed. While any are pending, live syncs of
/// intermediate results are ignored so the window never flickers back.
static PENDING: AtomicUsize = AtomicUsize::new(0);

static WRITER: LazyLock<Mutex<Sender<Job>>> = LazyLock::new(|| {
    let (tx, rx) = mpsc::channel::<Job>();
    let spawned = std::thread::Builder::new()
        .name("settings-writer".into())
        .spawn(move || {
            while let Ok((settings_tx, edit)) = rx.recv() {
                persist_update(settings_tx, edit);
                PENDING.fetch_sub(1, Ordering::SeqCst);
            }
        });
    if let Err(error) = spawned {
        eprintln!("could not start the settings writer: {error}");
    }
    Mutex::new(tx)
});

/// Queue an edit on the serial writer.
pub(crate) fn queue(
    settings_tx: Sender<Settings>,
    edit: impl FnOnce(&mut Settings) + Send + 'static,
) {
    PENDING.fetch_add(1, Ordering::SeqCst);
    let sent = WRITER
        .lock()
        .map(|writer| writer.send((settings_tx, Box::new(edit))).is_ok())
        .unwrap_or(false);
    if !sent {
        PENDING.fetch_sub(1, Ordering::SeqCst);
        eprintln!("failed to queue a settings change: writer unavailable");
    }
}

pub(crate) fn has_pending() -> bool {
    PENDING.load(Ordering::SeqCst) > 0
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

pub(crate) fn replace_settings(
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

pub(crate) fn load_settings_for_window() -> Settings {
    match Settings::default_path().and_then(|path| Settings::load_or_create(&path)) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("failed to load settings for window: {error:#}");
            Settings::default()
        }
    }
}
