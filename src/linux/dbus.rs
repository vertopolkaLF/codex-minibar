//! Session D-Bus service for panel widgets.
//!
//! `io.github.vertopolkalf.CodexMinibar` at `/io/github/vertopolkalf/CodexMinibar`
//! implements `io.github.vertopolkalf.CodexMinibar1`:
//!
//! - `Status() -> s`: the JSON [`crate::status::Report`] that
//!   `codex-minibar status --json` prints (without its `ok` wrapper).
//! - `Changed(s status)` signal: the same JSON whenever any quota, label or
//!   language-dependent text in it changes, so widgets need not poll.
//! - `Toggle(s activation_token)`: like a tray click. The token is an
//!   `XDG_ACTIVATION_TOKEN` that lets the popup take focus; pass `""` without one.
//! - `ShowProvider(s id)`: opens the popup on that provider's tab.
//! - `Refresh()`: asks every provider to refresh now.
//!
//! The GNOME Shell extension and the KDE Plasma widget in `integrations/` are
//! its clients. Any other client can use `gdbus` or `busctl`, e.g.
//! `gdbus call --session --dest io.github.vertopolkalf.CodexMinibar
//! --object-path /io/github/vertopolkalf/CodexMinibar
//! --method io.github.vertopolkalf.CodexMinibar1.Status`.

use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

use anyhow::{Context, Result};
use zbus::{fdo, interface, object_server::SignalEmitter};

use crate::{instances::ProviderId, popup_window::AppState, streamdeck::Command};

pub const BUS_NAME: &str = "io.github.vertopolkalf.CodexMinibar";
pub const OBJECT_PATH: &str = "/io/github/vertopolkalf/CodexMinibar";
/// How often the report is rebuilt to detect a change worth a signal. The
/// report is a few kilobytes and built from in-memory limits.
const CHANGE_POLL: Duration = Duration::from_secs(2);

struct Service {
    state: Arc<AppState>,
    commands: Mutex<mpsc::Sender<Command>>,
}

impl Service {
    fn send(&self, command: Command) -> fdo::Result<()> {
        self.commands
            .lock()
            .map_err(|_| fdo::Error::Failed("command queue unavailable".into()))?
            .send(command)
            .map_err(|_| fdo::Error::Failed("Codex Minibar is shutting down".into()))?;
        super::wake::wake();
        Ok(())
    }
}

#[interface(name = "io.github.vertopolkalf.CodexMinibar1")]
impl Service {
    fn status(&self) -> String {
        status_json(&self.state)
    }

    fn toggle(&self, activation_token: &str) {
        let token = (!activation_token.is_empty()).then(|| activation_token.to_owned());
        super::instance::toggle_with_token(token);
    }

    fn show_provider(&self, id: &str) -> fdo::Result<()> {
        let provider = ProviderId::lookup(id)
            .ok_or_else(|| fdo::Error::InvalidArgs(format!("unknown provider: {id}")))?;
        self.send(Command::ShowProvider { provider })
    }

    fn refresh(&self) -> fdo::Result<()> {
        self.send(Command::RefreshData)
    }

    #[zbus(signal)]
    async fn changed(emitter: &SignalEmitter<'_>, status: &str) -> zbus::Result<()>;
}

fn status_json(state: &AppState) -> String {
    serde_json::to_string(&crate::streamdeck::status_report(state)).unwrap_or_default()
}

/// The report without its timestamp, which changes on every build.
fn change_key(state: &AppState) -> String {
    let mut report = crate::streamdeck::status_report(state);
    report.observed_at = Default::default();
    serde_json::to_string(&report).unwrap_or_default()
}

/// Claims the bus name and serves the interface for the life of the
/// process. Failures (no session bus, name taken) only lose the widgets.
pub fn start(state: Arc<AppState>, commands: mpsc::Sender<Command>) {
    let spawned = thread::Builder::new()
        .name("minibar-dbus".into())
        .spawn(move || {
            if let Err(error) = serve(state, commands) {
                crate::logger::info(format!("D-Bus status service unavailable: {error:#}"));
            }
        });
    if let Err(error) = spawned {
        crate::logger::info(format!("D-Bus status service did not start: {error}"));
    }
}

fn serve(state: Arc<AppState>, commands: mpsc::Sender<Command>) -> Result<()> {
    let service = Service {
        state: Arc::clone(&state),
        commands: Mutex::new(commands),
    };
    let name = if crate::fake::enabled() {
        format!("{BUS_NAME}.Fake")
    } else {
        BUS_NAME.to_owned()
    };
    let connection = zbus::blocking::connection::Builder::session()
        .context("connect to the session bus")?
        .name(name)
        .context("claim the bus name")?
        .serve_at(OBJECT_PATH, service)
        .context("export the status object")?
        .build()
        .context("start the D-Bus service")?;
    let emitter =
        SignalEmitter::new(connection.inner(), OBJECT_PATH).context("create the signal emitter")?;
    let mut last = change_key(&state);
    loop {
        thread::sleep(CHANGE_POLL);
        let key = change_key(&state);
        if key == last {
            continue;
        }
        last = key;
        let status = status_json(&state);
        if let Err(error) = futures::executor::block_on(Service::changed(&emitter, &status)) {
            crate::logger::info(format!("D-Bus status signal failed: {error}"));
        }
    }
}
