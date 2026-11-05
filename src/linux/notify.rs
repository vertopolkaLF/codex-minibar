//! In-app notification cards as freedesktop desktop notifications.
//!
//! The Windows build stacks its own animated cards in a click-through
//! overlay. Linux clients cannot place windows on Wayland or make parts of a
//! window click-through, so cards go to the desktop's notification server
//! (`org.freedesktop.Notifications`) instead. Keyed cards replace their
//! previous bubble, buttons become notification actions, and clicking the
//! bubble itself opens the popup.

use std::{
    collections::HashMap,
    sync::{
        Mutex, OnceLock,
        mpsc::{self, Sender},
    },
};

use zbus::zvariant::Value;

use crate::notifications::{Notification, NotificationAction, NotificationKind, Progress};

const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
const DEFAULT_ACTION: &str = "default";

#[derive(Debug, PartialEq, Eq)]
enum Invocation {
    Show(Option<String>),
    Run(NotificationAction, Option<String>),
}

#[derive(Default)]
struct State {
    server: Option<String>,
    /// Server ids of keyed cards, so an update replaces the same bubble.
    keyed: HashMap<&'static str, u32>,
    /// Buttons of bubbles that may still be clicked.
    actions: HashMap<u32, Vec<NotificationAction>>,
    /// Focus grants sent immediately before a notification is invoked.
    activation_tokens: HashMap<u32, String>,
}

impl State {
    fn use_server(&mut self, server: Option<&str>) {
        if server.is_none() || self.server.as_deref() != server {
            self.keyed.clear();
            self.actions.clear();
            self.activation_tokens.clear();
            self.server = server.map(str::to_owned);
        }
    }

    fn remember_activation_token(&mut self, id: u32, token: String) {
        if self.actions.contains_key(&id) {
            self.activation_tokens.insert(id, token);
        }
    }

    fn close(&mut self, id: u32) {
        self.actions.remove(&id);
        self.activation_tokens.remove(&id);
        self.keyed.retain(|_, keyed| *keyed != id);
    }

    fn invocation(&mut self, id: u32, key: &str) -> Option<Invocation> {
        let actions = self.actions.get(&id)?;
        if key == DEFAULT_ACTION {
            return Some(Invocation::Show(self.activation_tokens.remove(&id)));
        }
        let index = key.strip_prefix('a')?.parse::<usize>().ok()?;
        // Action keys are opaque: only the exact keys sent in Notify count.
        if key != format!("a{index}") {
            return None;
        }
        let action = actions.get(index)?.clone();
        // Invalid IDs or keys must not consume a valid later focus grant.
        let token = self.activation_tokens.remove(&id);
        Some(Invocation::Run(action, token))
    }
}

fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(Default::default)
}

/// Shows `notification` as a desktop notification, without its sound when
/// `silent`. Runs the D-Bus call on a worker thread, because callers include
/// the GPUI thread. One thread keeps the cards in order: a keyed card can only
/// replace an earlier one once that one's server id has arrived.
pub fn post(notification: Notification, silent: bool) {
    static QUEUE: OnceLock<Sender<(Notification, bool)>> = OnceLock::new();
    let queue = QUEUE.get_or_init(|| {
        let (sender, receiver) = mpsc::channel::<(Notification, bool)>();
        std::thread::spawn(move || {
            for (notification, silent) in receiver {
                if let Err(error) = send(&notification, silent) {
                    crate::logger::info(format!("desktop notification failed: {error:#}"));
                }
            }
        });
        sender
    });
    let _ = queue.send((notification, silent));
}

fn send(notification: &Notification, silent: bool) -> anyhow::Result<()> {
    let bus =
        super::desktop::session_bus().ok_or_else(|| anyhow::anyhow!("no D-Bus session bus"))?;
    // Capabilities belong to the current server, which can change when the
    // desktop restarts or the user switches notification daemons.
    let (server, markup) = server_capabilities(bus);
    if let Err(error) = listen_for_actions(bus) {
        crate::logger::info(format!("notification actions unavailable: {error:#}"));
    }
    let mut actions = Vec::new();
    actions.push(DEFAULT_ACTION.to_owned());
    actions.push(String::new());
    for (index, (label, _)) in notification.actions.iter().enumerate() {
        actions.push(format!("a{index}"));
        actions.push(label.clone());
    }
    let mut hints: HashMap<&str, Value> = HashMap::new();
    let urgency: u8 = match notification.kind {
        NotificationKind::Error => 2,
        _ => 1,
    };
    hints.insert("urgency", Value::from(urgency));
    hints.insert("desktop-entry", Value::from(super::identity::APP_ID));
    // Progress ticks replace the bubble many times; never chime for them.
    if silent || notification.progress.is_some() {
        hints.insert("suppress-sound", Value::from(true));
    }
    if let Some(Progress::Fraction(fraction)) = notification.progress {
        hints.insert(
            "value",
            Value::from((fraction.clamp(0.0, 1.0) * 100.0).round() as i32),
        );
    }
    // Progress cards stay until the result replaces them.
    let expire_timeout: i32 = if notification.progress.is_some() {
        0
    } else {
        -1
    };
    // A daemon may emit a close/action signal immediately after replying.
    // Keep registration and signal handling serialized until the ID and its
    // actions are stored, so an early signal cannot outrun registration.
    let mut state = state()
        .lock()
        .map_err(|_| anyhow::anyhow!("notification state unavailable"))?;
    state.use_server(server.as_deref());
    let replaces = notification
        .key
        .and_then(|key| state.keyed.get(key).copied())
        .unwrap_or(0);
    let reply = bus.call_method(
        // Pin the request to the owner whose capabilities and IDs we read.
        // If it exits, fail rather than replace an unrelated new server's ID.
        Some(server.as_deref().unwrap_or(NAME)),
        PATH,
        Some(NAME),
        "Notify",
        &(
            "Codex Minibar",
            replaces,
            "codex-minibar",
            notification.title.as_str(),
            format_body(&body(notification), markup),
            actions,
            hints,
            expire_timeout,
        ),
    )?;
    let id: u32 = reply.body().deserialize()?;
    let header = reply.header();
    state.use_server(header.sender().map(|sender| sender.as_str()));
    if let Some(key) = notification.key {
        state.keyed.insert(key, id);
    }
    state.actions.insert(
        id,
        notification
            .actions
            .iter()
            .map(|(_, action)| action.clone())
            .collect(),
    );
    Ok(())
}

/// The card text, with the quotas a limit card draws as bars spelled out.
fn body(notification: &Notification) -> String {
    let Some(alert) = notification.limits.as_deref() else {
        return notification.body.clone();
    };
    let quotas = crate::tray::tooltip(&alert.limits);
    if notification.body.is_empty() {
        quotas
    } else {
        format!("{}\n{quotas}", notification.body)
    }
}

fn server_capabilities(bus: &zbus::blocking::Connection) -> (Option<String>, bool) {
    let Ok(reply) = bus.call_method(Some(NAME), PATH, Some(NAME), "GetCapabilities", &()) else {
        // Without an identified owner, never reuse cached notification IDs.
        return (None, true);
    };
    let server = reply.header().sender().map(|sender| sender.to_string());
    let markup = reply
        .body()
        .deserialize::<Vec<String>>()
        .ok()
        // On a failed capability query retain conservative escaping, so a
        // markup-capable server never interprets provider text as HTML.
        .is_none_or(|capabilities| capabilities.iter().any(|cap| cap == "body-markup"));
    (server, markup)
}

fn format_body(text: &str, markup: bool) -> String {
    if markup {
        escape_markup(text)
    } else {
        text.to_owned()
    }
}

/// Servers may interpret a subset of HTML in the body.
fn escape_markup(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Establishes the subscription before Notify and retries failed initialization
/// on the next notification. A finished listener can also be restarted.
fn listen_for_actions(bus: &zbus::blocking::Connection) -> anyhow::Result<()> {
    static LISTENER: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);
    let mut listener = LISTENER
        .lock()
        .map_err(|_| anyhow::anyhow!("notification listener unavailable"))?;
    if listener
        .as_ref()
        .is_some_and(|thread| !thread.is_finished())
    {
        return Ok(());
    }
    if let Some(thread) = listener.take() {
        let _ = thread.join();
    }
    let proxy = zbus::blocking::Proxy::new(bus, NAME, PATH, NAME)?;
    let signals = proxy.receive_all_signals()?;
    *listener = Some(
        std::thread::Builder::new()
            .name("notification-actions".into())
            .spawn(move || run_action_listener(signals))?,
    );
    Ok(())
}

fn run_action_listener(signals: zbus::blocking::proxy::SignalIterator<'static>) {
    // A server commonly emits ActionInvoked immediately before
    // NotificationClosed. One ordered stream must clone the action before
    // processing the close; separate threads can discard it first.
    for message in signals {
        let header = message.header();
        // Signals queued by the previous daemon must not remove or invoke
        // records whose numeric IDs have already been reused by a new one.
        let Ok(mut state) = state().lock() else {
            continue;
        };
        if state.server.is_none()
            || state.server.as_deref() != header.sender().map(|sender| sender.as_str())
        {
            continue;
        }
        match header.member().map(|member| member.as_str()) {
            Some("NotificationClosed") => {
                if let Ok((id, _reason)) = message.body().deserialize::<(u32, u32)>() {
                    state.close(id);
                }
                continue;
            }
            Some("ActivationToken") => {
                if let Ok((id, token)) = message.body().deserialize::<(u32, String)>() {
                    state.remember_activation_token(id, token);
                }
                continue;
            }
            Some("ActionInvoked") => {}
            _ => continue,
        }
        let Ok((id, key)) = message.body().deserialize::<(u32, String)>() else {
            continue;
        };
        let invocation = state.invocation(id, &key);
        drop(state);
        match invocation {
            Some(Invocation::Show(token)) => super::instance::show_with_token(token),
            Some(Invocation::Run(NotificationAction::OpenUrl(url), token)) => open_url(url, token),
            Some(Invocation::Run(action, _)) => action.run(),
            None => {}
        }
    }
}

fn open_url(url: String, token: Option<String>) {
    // Desktop launch helpers can stay alive with the browser. Keep the ordered
    // notification signal listener free to process subsequent actions/closes.
    let spawned = std::thread::Builder::new()
        .name("notification-open".into())
        .spawn(move || {
            if let Err(error) = super::desktop::open_with_activation_token(
                std::ffi::OsStr::new(&url),
                token.as_deref(),
            ) {
                crate::logger::info(format!("notification URL could not open: {error:#}"));
            }
        });
    if let Err(error) = spawned {
        crate::logger::info(format!("notification URL worker could not start: {error}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_actions_only_open_live_registered_notifications() {
        let mut state = State::default();
        state.actions.insert(10, Vec::new());
        state.remember_activation_token(10, "focus-grant".into());
        assert_eq!(state.invocation(20, DEFAULT_ACTION), None);
        assert_eq!(
            state.activation_tokens.get(&10).map(String::as_str),
            Some("focus-grant")
        );
        assert_eq!(
            state.invocation(10, DEFAULT_ACTION),
            Some(Invocation::Show(Some("focus-grant".into())))
        );
        assert_eq!(
            state.invocation(10, DEFAULT_ACTION),
            Some(Invocation::Show(None))
        );
        state.close(10);
        assert_eq!(state.invocation(10, DEFAULT_ACTION), None);
    }

    #[test]
    fn action_keys_must_match_registered_keys_without_consuming_other_focus_grants() {
        let mut state = State::default();
        let action = NotificationAction::OpenUrl("https://example.com/release".into());
        state.actions.insert(10, vec![action.clone()]);
        state.remember_activation_token(10, "focus-grant".into());
        for key in ["", "unknown", "a+0", "a00", "a1", "a-1"] {
            assert_eq!(state.invocation(10, key), None, "{key}");
            assert_eq!(
                state.activation_tokens.get(&10).map(String::as_str),
                Some("focus-grant")
            );
        }
        assert_eq!(state.invocation(20, "a0"), None);
        assert_eq!(
            state.invocation(10, "a0"),
            Some(Invocation::Run(action, Some("focus-grant".into())))
        );
        assert!(!state.activation_tokens.contains_key(&10));
    }

    #[test]
    fn daemon_changes_invalidate_ids_actions_and_focus_grants() {
        let mut state = State::default();
        state.use_server(Some(":1.10"));
        state.keyed.insert("progress", 1);
        state.actions.insert(1, Vec::new());
        state.remember_activation_token(1, "old-token".into());
        state.use_server(Some(":1.10"));
        assert_eq!(state.keyed.get("progress"), Some(&1));
        assert!(state.actions.contains_key(&1));
        state.use_server(Some(":1.11"));
        assert!(state.keyed.is_empty());
        assert!(state.actions.is_empty());
        assert!(state.activation_tokens.is_empty());
        state.keyed.insert("progress", 1);
        state.use_server(None);
        assert!(state.keyed.is_empty());
        assert_eq!(state.server, None);
    }

    #[test]
    fn notification_focus_tokens_are_scoped_to_live_notifications() {
        let mut state = State::default();
        state.actions.insert(1, Vec::new());
        state.actions.insert(2, Vec::new());
        state.keyed.insert("progress", 1);
        state.remember_activation_token(1, "first".into());
        state.remember_activation_token(2, "second".into());
        state.remember_activation_token(3, "unknown".into());
        assert!(!state.activation_tokens.contains_key(&3));
        assert_eq!(state.activation_tokens.remove(&1).as_deref(), Some("first"));
        assert_eq!(state.activation_tokens.remove(&1), None);
        state.close(2);
        assert_eq!(state.activation_tokens.remove(&2), None);
        assert!(!state.actions.contains_key(&2));
        state.close(1);
        assert!(!state.keyed.contains_key("progress"));
    }

    #[test]
    fn plain_text_servers_receive_original_text_without_visible_entities() {
        let text = "Usage < 10% & reset > 1h";
        assert_eq!(format_body(text, false), text);
        assert_eq!(
            format_body(text, true),
            "Usage &lt; 10% &amp; reset &gt; 1h"
        );
    }

    #[test]
    fn markup_is_escaped() {
        assert_eq!(escape_markup("a < b & c > d"), "a &lt; b &amp; c &gt; d");
    }
}
