//! Settings persistence shared by the Settings window, onboarding and popup.
//!
//! Every write reloads the file, applies one change, saves, applies runtime
//! effects and broadcasts the result. ALL writers (Settings window, onboarding,
//! popup, bridge, credential dialogs) go through one serial writer thread so
//! load-modify-save cycles never interleave and never drop each other's edits,
//! and so no UI or tray thread waits on disk I/O.
//!
//! Three entry points, by caller:
//! - [`queue`] / [`persist_update`]: fire-and-forget edits.
//! - [`queue_fallible`] / [`queue_replace`]: return a [`Receiver`] with the
//!   outcome. UI-thread callers wait for it on the background executor;
//!   they must never block on it.
//! - [`try_persist_update_fallible`]: blocking convenience for callers that
//!   are already on a worker thread (never the GPUI or bridge thread).

use std::sync::{
    LazyLock, Mutex,
    atomic::{AtomicUsize, Ordering},
    mpsc::{self, Receiver, Sender},
};

use anyhow::Context as _;

use crate::settings::Settings;

type Job = Box<dyn FnOnce() + Send>;

/// Jobs queued but not yet committed. While any are pending, live syncs of
/// intermediate results are ignored so the window never flickers back.
static PENDING: AtomicUsize = AtomicUsize::new(0);

static WRITER: LazyLock<Mutex<Sender<Job>>> = LazyLock::new(|| {
    let (tx, rx) = mpsc::channel::<Job>();
    let spawned = std::thread::Builder::new()
        .name("settings-writer".into())
        .spawn(move || {
            while let Ok(job) = rx.recv() {
                // A panicking edit must not kill the writer for the session.
                // Its reply sender is dropped, which the caller sees as an
                // error.
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
                if outcome.is_err() {
                    eprintln!("a settings write panicked; the writer keeps running");
                }
                PENDING.fetch_sub(1, Ordering::SeqCst);
            }
        });
    if let Err(error) = spawned {
        eprintln!("could not start the settings writer: {error}");
    }
    Mutex::new(tx)
});

/// Run `job` on the serial writer. If the writer is unavailable the job is
/// dropped, which closes any reply channel it owned.
fn submit(job: impl FnOnce() + Send + 'static) {
    PENDING.fetch_add(1, Ordering::SeqCst);
    let sent = WRITER
        .lock()
        .map(|writer| writer.send(Box::new(job)).is_ok())
        .unwrap_or(false);
    if !sent {
        PENDING.fetch_sub(1, Ordering::SeqCst);
        eprintln!("failed to queue a settings change: writer unavailable");
    }
}

/// Queue an edit on the serial writer without waiting for it.
pub(crate) fn queue(
    settings_tx: Sender<Settings>,
    edit: impl FnOnce(&mut Settings) + Send + 'static,
) {
    submit(move || {
        let result = commit_update(settings_tx, |settings| {
            edit(settings);
            Ok(())
        });
        if let Err(error) = result {
            eprintln!("failed to save settings: {error:#}");
        }
    });
}

/// Queue an edit that can fail and may return a value. The receiver yields the
/// outcome once the change is committed (or failed).
pub(crate) fn queue_fallible<T: Send + 'static>(
    settings_tx: Sender<Settings>,
    update: impl FnOnce(&mut Settings) -> anyhow::Result<T> + Send + 'static,
) -> Receiver<anyhow::Result<T>> {
    let (reply, outcome) = mpsc::channel();
    submit(move || {
        let _ = reply.send(commit_update(settings_tx, update));
    });
    outcome
}

/// Queue a whole-snapshot replacement (reset, import, onboarding completion).
pub(crate) fn queue_replace(
    settings_tx: Sender<Settings>,
    settings: Settings,
) -> Receiver<anyhow::Result<()>> {
    let (reply, outcome) = mpsc::channel();
    submit(move || {
        let _ = reply.send(commit_replace(settings_tx, settings));
    });
    outcome
}

/// Block until a queued write finishes. Worker threads only.
pub(crate) fn wait<T>(outcome: Receiver<anyhow::Result<T>>) -> anyhow::Result<T> {
    outcome
        .recv()
        .map_err(|_| anyhow::anyhow!("the settings writer stopped before saving"))?
}

pub(crate) fn has_pending() -> bool {
    PENDING.load(Ordering::SeqCst) > 0
}

/// Fire-and-forget persisted edit; safe from any thread.
pub(crate) fn persist_update(
    settings_tx: Sender<Settings>,
    update: impl FnOnce(&mut Settings) + Send + 'static,
) {
    queue(settings_tx, update);
}

/// Credential flows must report persistence failures instead of displaying a
/// success notice when the settings could not be saved. Blocks until the
/// serial writer has committed, so call it from worker threads only.
pub(crate) fn try_persist_update_fallible<T: Send + 'static>(
    settings_tx: Sender<Settings>,
    update: impl FnOnce(&mut Settings) -> anyhow::Result<T> + Send + 'static,
) -> anyhow::Result<T> {
    wait(queue_fallible(settings_tx, update))
}

/// Runs on the writer thread only.
fn commit_update<T>(
    settings_tx: Sender<Settings>,
    update: impl FnOnce(&mut Settings) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let path = Settings::default_path()?;
    let mut settings = Settings::load_or_create(&path)?;
    let value = update(&mut settings)?;
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
    Ok(value)
}

/// Runs on the writer thread only.
fn commit_replace(settings_tx: Sender<Settings>, mut settings: Settings) -> anyhow::Result<()> {
    let path = Settings::default_path()?;
    settings.normalize_tray_widgets();
    settings.normalize_popup_visibility();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallible_jobs_run_serially_in_submission_order() {
        let (tx, _rx) = mpsc::channel::<Settings>();
        let order = std::sync::Arc::new(Mutex::new(Vec::new()));
        let mut receivers = Vec::new();
        for n in 0..20 {
            let order = order.clone();
            // Exercise only the writer; the settings-touching commit is
            // bypassed by submitting plain jobs.
            let (reply, outcome) = mpsc::channel();
            submit(move || {
                order.lock().unwrap().push(n);
                let _ = reply.send(Ok::<usize, anyhow::Error>(n));
            });
            receivers.push(outcome);
        }
        let _ = tx;
        for (n, outcome) in receivers.into_iter().enumerate() {
            assert_eq!(wait(outcome).unwrap(), n);
        }
        assert_eq!(*order.lock().unwrap(), (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn a_panicking_job_reports_an_error_and_keeps_the_writer_alive() {
        let (reply, outcome) = mpsc::channel::<anyhow::Result<()>>();
        submit(move || {
            let _reply = reply;
            panic!("boom");
        });
        assert!(wait(outcome).is_err());
        let (reply, outcome) = mpsc::channel();
        submit(move || {
            let _ = reply.send(Ok(1));
        });
        assert_eq!(wait(outcome).unwrap(), 1);
    }
}
