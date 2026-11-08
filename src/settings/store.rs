//! The serial settings writer.
//!
//! Every change to the settings file, from any window or thread, goes through
//! one writer thread: each job reloads the file, applies its change,
//! normalizes and saves. Load-modify-save cycles never interleave or drop each
//! other's edits, and no UI or tray thread waits on disk I/O. Once the queue
//! drains, the committed snapshot goes to the listener installed at startup.
//!
//! [`queue`] is fire-and-forget. [`queue_fallible`] returns a [`Receiver`]
//! with the outcome, answered after the listener has seen the commit; UI
//! threads wait for it on a background executor. [`update_blocking`] waits in
//! place, for worker threads only.

use std::sync::{
    LazyLock, Mutex, OnceLock,
    atomic::{AtomicUsize, Ordering},
    mpsc::{self, Receiver, Sender},
};

use anyhow::Result;

use super::Settings;

/// Runs after the writer thread finished a job and announced its commit.
type Reply = Box<dyn FnOnce() + Send>;
type Job = Box<dyn FnOnce() -> Reply + Send>;
type Listener = Box<dyn Fn(Settings) + Send + Sync>;

/// Jobs queued but not yet finished. While any are pending, open windows keep
/// their own snapshot so they never flicker back to an intermediate commit.
static PENDING: AtomicUsize = AtomicUsize::new(0);
static LISTENER: OnceLock<Listener> = OnceLock::new();
/// The newest commit the listener has not seen yet.
static UNANNOUNCED: Mutex<Option<Settings>> = Mutex::new(None);

static WRITER: LazyLock<Mutex<Sender<Job>>> = LazyLock::new(|| {
    let (tx, rx) = mpsc::channel::<Job>();
    let spawned = std::thread::Builder::new()
        .name("settings-writer".into())
        .spawn(move || {
            while let Ok(job) = rx.recv() {
                // A panicking edit must not kill the writer for the session.
                // Its reply sender is dropped, which the caller sees as an
                // error.
                let reply = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job))
                    .unwrap_or_else(|_| {
                        crate::logger::info("a settings write panicked; the writer keeps running");
                        Box::new(|| {})
                    });
                if PENDING.fetch_sub(1, Ordering::SeqCst) == 1 {
                    announce();
                }
                reply();
            }
        });
    if let Err(error) = spawned {
        crate::logger::info(format!("could not start the settings writer: {error}"));
    }
    Mutex::new(tx)
});

/// Installs the one listener that receives every committed snapshot.
pub fn listen(listener: impl Fn(Settings) + Send + Sync + 'static) {
    if LISTENER.set(Box::new(listener)).is_err() {
        crate::logger::info("the settings listener was already installed");
    }
}

fn announce() {
    let committed = UNANNOUNCED
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .take();
    if let (Some(settings), Some(listener)) = (committed, LISTENER.get()) {
        listener(settings);
    }
}

/// Runs `job` on the serial writer. If the writer is unavailable the job is
/// dropped, which closes any reply channel it owned.
fn submit(job: impl FnOnce() -> Reply + Send + 'static) {
    PENDING.fetch_add(1, Ordering::SeqCst);
    let sent = WRITER
        .lock()
        .map(|writer| writer.send(Box::new(job)).is_ok())
        .unwrap_or(false);
    if !sent {
        PENDING.fetch_sub(1, Ordering::SeqCst);
        crate::logger::info("failed to queue a settings change: writer unavailable");
    }
}

/// Queues an edit without waiting for it.
pub fn queue(edit: impl FnOnce(&mut Settings) + Send + 'static) {
    queue_result(move |settings| {
        edit(settings);
        Ok(())
    });
}

fn queue_result(update: impl FnOnce(&mut Settings) -> Result<()> + Send + 'static) {
    submit(move || {
        if let Err(error) = commit(update) {
            crate::logger::info(format!("failed to save settings: {error:#}"));
        }
        Box::new(|| {})
    });
}

/// Applies `edit` to a window's own snapshot at once, so the change shows
/// before it commits, and queues the settings it changed as absolute values.
/// The edit runs only here, so a move or a toggle lands in the file exactly
/// as the window shows it. Returns the snapshot as it was before the edit.
pub fn edit_snapshot(local: &mut Settings, edit: impl FnOnce(&mut Settings)) -> Settings {
    let before = local.clone();
    edit(local);
    local.normalize_all();
    match Patch::between(&before, local) {
        Ok(patch) if patch.0.is_empty() => {}
        Ok(patch) => queue_result(move |settings| patch.apply(settings)),
        Err(error) => crate::logger::info(format!("failed to queue a settings edit: {error:#}")),
    }
    before
}

/// Top-level settings an edit changed, with their new values (`None` when
/// the value is no longer written).
struct Patch(Vec<(String, Option<toml::Value>)>);

impl Patch {
    fn between(before: &Settings, after: &Settings) -> Result<Self> {
        let (before, after) = (document(before)?, document(after)?);
        let mut changes = after
            .iter()
            .filter(|(key, value)| before.get(*key) != Some(*value))
            .map(|(key, value)| (key.clone(), Some(value.clone())))
            .collect::<Vec<_>>();
        changes.extend(
            before
                .keys()
                .filter(|key| !after.contains_key(*key))
                .map(|key| (key.clone(), None)),
        );
        Ok(Self(changes))
    }

    fn apply(self, settings: &mut Settings) -> Result<()> {
        let mut document = document(settings)?;
        for (key, value) in self.0 {
            match value {
                Some(value) => document.insert(key, value),
                None => document.remove(&key),
            };
        }
        let unknown = std::mem::take(&mut settings.unknown_entries);
        *settings = toml::Value::Table(document).try_into()?;
        settings.unknown_entries = unknown;
        Ok(())
    }
}

fn document(settings: &Settings) -> Result<toml::map::Map<String, toml::Value>> {
    match toml::Value::try_from(settings)? {
        toml::Value::Table(table) => Ok(table),
        _ => anyhow::bail!("settings did not serialize to a table"),
    }
}

/// Queues an edit that can fail and may return a value. The receiver yields
/// the outcome once the change is committed (or failed).
pub fn queue_fallible<T: Send + 'static>(
    update: impl FnOnce(&mut Settings) -> Result<T> + Send + 'static,
) -> Receiver<Result<T>> {
    let (reply, outcome) = mpsc::channel();
    submit(move || {
        let result = commit(update);
        Box::new(move || {
            let _ = reply.send(result);
        })
    });
    outcome
}

/// Blocks until a queued write finishes. Never on the GPUI or bridge thread.
pub fn wait<T>(outcome: Receiver<Result<T>>) -> Result<T> {
    outcome
        .recv()
        .map_err(|_| anyhow::anyhow!("the settings writer stopped before saving"))?
}

/// Commits `update` and waits for it. Never on the GPUI or bridge thread.
pub fn update_blocking<T: Send + 'static>(
    update: impl FnOnce(&mut Settings) -> Result<T> + Send + 'static,
) -> Result<T> {
    wait(queue_fallible(update))
}

pub fn has_pending() -> bool {
    PENDING.load(Ordering::SeqCst) > 0
}

/// The settings file as it is now, for a window that opens. Never writes.
pub fn read() -> Settings {
    match crate::app_paths::settings().and_then(|path| Settings::read(&path)) {
        Ok(settings) => settings,
        Err(error) => {
            crate::logger::info(format!("failed to read settings: {error:#}"));
            Settings::default()
        }
    }
}

/// Runs on the writer thread only. Saving the file is the transaction
/// boundary: once it succeeds the change counts, whatever its listeners do.
fn commit<T>(update: impl FnOnce(&mut Settings) -> Result<T>) -> Result<T> {
    let path = crate::app_paths::settings()?;
    let mut settings = Settings::load_or_create(&path)?;
    let value = update(&mut settings)?;
    settings.normalize_all();
    settings.save(&path)?;
    *UNANNOUNCED
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = Some(settings);
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallible_jobs_run_serially_in_submission_order() {
        let order = std::sync::Arc::new(Mutex::new(Vec::new()));
        let mut receivers = Vec::new();
        for n in 0..20 {
            let order = order.clone();
            // Exercise only the writer; the settings-touching commit is
            // bypassed by submitting plain jobs.
            let (reply, outcome) = mpsc::channel();
            submit(move || {
                order.lock().unwrap().push(n);
                Box::new(move || {
                    let _ = reply.send(Ok::<usize, anyhow::Error>(n));
                })
            });
            receivers.push(outcome);
        }
        for (n, outcome) in receivers.into_iter().enumerate() {
            assert_eq!(wait(outcome).unwrap(), n);
        }
        assert_eq!(*order.lock().unwrap(), (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn a_patch_sets_only_what_the_edit_changed() {
        let before = Settings::default();
        let mut window = before.clone();
        window
            .tray_widgets
            .insert(0, crate::settings::TrayWidget::app_icon());
        window.popup_two_columns = !before.popup_two_columns;
        let patch = Patch::between(&before, &window).unwrap();
        let mut committed = before.clone();
        committed.show_usage_pace = !before.show_usage_pace;
        patch.apply(&mut committed).unwrap();
        assert_eq!(committed.tray_widgets, window.tray_widgets);
        assert_eq!(committed.popup_two_columns, window.popup_two_columns);
        assert_eq!(committed.show_usage_pace, !before.show_usage_pace);
    }

    #[test]
    fn a_toggle_lands_as_shown_even_when_the_commit_already_has_it() {
        let before = Settings::default();
        let mut window = before.clone();
        window.popup_two_columns = !window.popup_two_columns;
        let patch = Patch::between(&before, &window).unwrap();
        // Another surface already committed the same toggle; re-running the
        // edit would flip it back, the patch must not.
        let mut committed = window.clone();
        patch.apply(&mut committed).unwrap();
        assert_eq!(committed.popup_two_columns, window.popup_two_columns);
    }

    #[test]
    fn a_panicking_job_reports_an_error_and_keeps_the_writer_alive() {
        let (reply, outcome) = mpsc::channel::<Result<()>>();
        submit(move || {
            let _reply = reply;
            panic!("boom");
        });
        assert!(wait(outcome).is_err());
        let (reply, outcome) = mpsc::channel();
        submit(move || {
            Box::new(move || {
                let _ = reply.send(Ok(1));
            })
        });
        assert_eq!(wait(outcome).unwrap(), 1);
    }
}
