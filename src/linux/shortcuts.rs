use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use ashpd::desktop::{
    Session,
    global_shortcuts::{GlobalShortcuts, NewShortcut, Shortcut},
};
use futures::{FutureExt, StreamExt, channel::mpsc, pin_mut, select};

const SHORTCUT_ID: &str = "toggle-popup";
const PREFERRED_TRIGGER: &str = "CTRL+ALT+m";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub pending: bool,
    pub trigger: Option<String>,
    pub failed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Enable(bool),
    Configure,
}

struct Controller {
    sender: mpsc::UnboundedSender<Command>,
    requested: Mutex<bool>,
}

static CONTROLLER: OnceLock<Controller> = OnceLock::new();
static SNAPSHOT: Mutex<Snapshot> = Mutex::new(Snapshot {
    pending: false,
    trigger: None,
    failed: false,
});
static ACTIVATION_TOKEN: Mutex<Option<(String, Instant)>> = Mutex::new(None);

pub fn snapshot() -> Snapshot {
    SNAPSHOT
        .lock()
        .map(|value| value.clone())
        .unwrap_or_default()
}

fn publish(value: Snapshot) {
    if let Ok(mut snapshot) = SNAPSHOT.lock() {
        *snapshot = value;
    }
    crate::settings_window::refresh_hotkey_status();
    super::wake::wake();
}

fn report_error(error: anyhow::Error) {
    crate::logger::info(format!("Linux popup shortcut: {error:#}"));
    publish(Snapshot {
        pending: false,
        failed: true,
        ..snapshot()
    });
}

fn controller() -> &'static Controller {
    CONTROLLER.get_or_init(|| {
        let (sender, receiver) = mpsc::unbounded();
        if let Err(error) = std::thread::Builder::new()
            .name("popup-shortcut".into())
            .spawn(move || futures::executor::block_on(run(receiver)))
        {
            report_error(error.into());
        }
        Controller {
            sender,
            requested: Mutex::new(false),
        }
    })
}

pub fn sync(enabled: bool) {
    if !enabled && CONTROLLER.get().is_none() {
        return;
    }
    let controller = controller();
    if let Ok(mut requested) = controller.requested.lock()
        && *requested != enabled
    {
        *requested = enabled;
        let _ = controller.sender.unbounded_send(Command::Enable(enabled));
    }
}

pub fn configure() {
    let _ = controller().sender.unbounded_send(Command::Configure);
}

pub fn take_activation_token() -> Option<String> {
    ACTIVATION_TOKEN
        .lock()
        .ok()?
        .take()
        .filter(|(_, received)| received.elapsed() < Duration::from_secs(5))
        .map(|(token, _)| token)
}

async fn run(mut commands: mpsc::UnboundedReceiver<Command>) {
    let mut command = commands.next().await;
    while let Some(next) = command {
        if next == Command::Enable(false) {
            let _ = take_activation_token();
            publish(Snapshot::default());
            command = commands.next().await;
            continue;
        }
        publish(Snapshot {
            pending: true,
            ..Snapshot::default()
        });
        match run_session(&mut commands).await {
            Ok(next) => command = next,
            Err(error) => {
                let _ = take_activation_token();
                publish(Snapshot::default());
                report_error(error);
                command = commands.next().await;
            }
        }
    }
}

async fn run_session(commands: &mut mpsc::UnboundedReceiver<Command>) -> Result<Option<Command>> {
    super::identity::register_portal().await?;
    let portal = GlobalShortcuts::new().await?;
    let session = portal.create_session().await?;
    let result = serve_session(&portal, &session, commands).await;
    if let Err(error) = session.close().await {
        crate::logger::info(format!("Close Linux popup shortcut session: {error}"));
    }
    result
}

fn trigger(shortcuts: &[Shortcut]) -> Option<String> {
    shortcuts
        .iter()
        .find(|shortcut| shortcut.id() == SHORTCUT_ID)
        .map(|shortcut| shortcut.trigger_description().to_owned())
        .filter(|description| !description.trim().is_empty())
}

fn matches_activation(session: &str, expected_session: &str, shortcut_id: &str) -> bool {
    session == expected_session && shortcut_id == SHORTCUT_ID
}

async fn serve_session(
    portal: &GlobalShortcuts<'_>,
    session: &Session<'_, GlobalShortcuts<'_>>,
    commands: &mut mpsc::UnboundedReceiver<Command>,
) -> Result<Option<Command>> {
    let session_value = serde_json::to_value(session)?;
    let session_path = session_value.as_str().context("portal session path")?;
    let mut activated = portal.receive_activated().await?.fuse();
    let mut changed = portal.receive_shortcuts_changed().await?.fuse();
    let closed = session.receive_closed().await?.fuse();
    pin_mut!(closed);
    let shortcuts = [NewShortcut::new(
        SHORTCUT_ID,
        format!("Codex Minibar — {}", crate::i18n::tr("popup-shortcut")),
    )
    .preferred_trigger(Some(PREFERRED_TRIGGER))];
    let binding = portal.bind_shortcuts(session, &shortcuts, None).fuse();
    pin_mut!(binding);
    let response = select! {
        response = binding => response?.response()?,
        command = commands.next().fuse() => return Ok(command),
        _ = closed.next().fuse() => return Err(anyhow!("shortcut session closed during setup")),
    };
    publish(Snapshot {
        trigger: trigger(response.shortcuts()),
        ..Snapshot::default()
    });
    loop {
        select! {
            command = commands.next().fuse() => match command {
                Some(Command::Configure) => {
                    let options = HashMap::<&str, zbus::zvariant::OwnedValue>::new();
                    if let Err(error) = portal.call_method("ConfigureShortcuts", &(session, "", options)).await {
                        report_error(error.into());
                    }
                }
                Some(Command::Enable(true)) => {}
                command => return Ok(command),
            },
            event = activated.next() => {
                let event = event.context("shortcut activation stream ended")?;
                if matches_activation(event.session_handle().as_str(), session_path, event.shortcut_id()) {
                    let token = event.options().get("activation_token")
                        .and_then(|value| <&str>::try_from(value).ok())
                        .map(|token| (token.to_owned(), Instant::now()));
                    if let Ok(mut pending) = ACTIVATION_TOKEN.lock() {
                        *pending = token;
                    }
                    crate::hotkey::notify_pressed();
                }
            },
            event = changed.next() => {
                let event = event.context("shortcut configuration stream ended")?;
                if event.session_handle().as_str() == session_path {
                    publish(Snapshot {
                        trigger: trigger(event.shortcuts()),
                        ..Snapshot::default()
                    });
                }
            },
            _ = closed.next().fuse() => return Err(anyhow!("shortcut session closed by desktop")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activations_must_match_both_session_and_shortcut() {
        assert!(matches_activation(
            "/session/current",
            "/session/current",
            SHORTCUT_ID
        ));
        assert!(!matches_activation(
            "/session/old",
            "/session/current",
            SHORTCUT_ID
        ));
        assert!(!matches_activation(
            "/session/current",
            "/session/current",
            "other"
        ));
    }

    #[test]
    fn portal_trigger_description_is_used_and_can_be_cleared() {
        let encoded = serde_json::json!([
            ["other", {
                "description": ["s", "Other"],
                "trigger_description": ["s", "Ctrl+Q"]
            }],
            [SHORTCUT_ID, {
                "description": ["s", "Popup"],
                "trigger_description": ["s", "Super+M"]
            }]
        ])
        .to_string();
        let shortcuts: Vec<Shortcut> = serde_json::from_str(&encoded).unwrap();
        assert_eq!(trigger(&shortcuts).as_deref(), Some("Super+M"));
        assert_eq!(trigger(&shortcuts[..1]), None);
        assert_eq!(trigger(&[]), None);
    }

    #[test]
    fn activation_tokens_are_single_use_and_expire() {
        *ACTIVATION_TOKEN.lock().unwrap() = Some(("fresh".into(), Instant::now()));
        assert_eq!(take_activation_token().as_deref(), Some("fresh"));
        assert_eq!(take_activation_token(), None);
        *ACTIVATION_TOKEN.lock().unwrap() =
            Some(("expired".into(), Instant::now() - Duration::from_secs(6)));
        assert_eq!(take_activation_token(), None);
        assert!(ACTIVATION_TOKEN.lock().unwrap().is_none());
    }
}
