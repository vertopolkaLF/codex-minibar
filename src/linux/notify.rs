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
    sync::{Mutex, OnceLock},
};

use zbus::zvariant::Value;

use crate::notifications::{Notification, NotificationAction, NotificationKind, Progress};

const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
const DEFAULT_ACTION: &str = "default";

#[derive(Default)]
struct State {
    /// Server ids of keyed cards, so an update replaces the same bubble.
    keyed: HashMap<&'static str, u32>,
    /// Buttons of bubbles that may still be clicked.
    actions: HashMap<u32, Vec<NotificationAction>>,
}

fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(Default::default)
}

/// Shows `notification` as a desktop notification, without its sound when
/// `silent`. Runs the D-Bus call on a short-lived thread: callers include the
/// GPUI thread.
pub fn post(notification: Notification, silent: bool) {
    std::thread::spawn(move || {
        if let Err(error) = send(&notification, silent) {
            crate::logger::info(format!("desktop notification failed: {error:#}"));
        }
    });
}

fn send(notification: &Notification, silent: bool) -> anyhow::Result<()> {
    let bus =
        super::desktop::session_bus().ok_or_else(|| anyhow::anyhow!("no D-Bus session bus"))?;
    listen_for_actions();
    let replaces = notification
        .key
        .and_then(|key| state().lock().ok()?.keyed.get(key).copied())
        .unwrap_or(0);
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
    hints.insert("desktop-entry", Value::from("codex-minibar"));
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
    let reply = bus.call_method(
        Some(NAME),
        PATH,
        Some(NAME),
        "Notify",
        &(
            "Codex Minibar",
            replaces,
            "codex-minibar",
            notification.title.as_str(),
            escape_markup(&body(notification)),
            actions,
            hints,
            expire_timeout,
        ),
    )?;
    let id: u32 = reply.body().deserialize()?;
    if let Ok(mut state) = state().lock() {
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
    }
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

/// Servers may interpret a subset of HTML in the body.
fn escape_markup(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Starts one thread that runs the actions of clicked bubbles.
fn listen_for_actions() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let spawned = std::thread::Builder::new()
            .name("notification-actions".into())
            .spawn(|| {
                if let Err(error) = run_action_listener() {
                    crate::logger::info(format!("notification actions unavailable: {error:#}"));
                }
            });
        if let Err(error) = spawned {
            crate::logger::info(format!("notification actions unavailable: {error}"));
        }
    });
}

fn run_action_listener() -> anyhow::Result<()> {
    let bus =
        super::desktop::session_bus().ok_or_else(|| anyhow::anyhow!("no D-Bus session bus"))?;
    let proxy = zbus::blocking::Proxy::new(bus, NAME, PATH, NAME)?;
    let mut closed = proxy.receive_signal("NotificationClosed")?;
    let invoked = proxy.receive_signal("ActionInvoked")?;
    // Forget bubbles the server closed so their actions are not retained.
    std::thread::spawn(move || {
        for message in &mut closed {
            if let Ok((id, _reason)) = message.body().deserialize::<(u32, u32)>()
                && let Ok(mut state) = state().lock()
            {
                state.actions.remove(&id);
                state.keyed.retain(|_, keyed| *keyed != id);
            }
        }
    });
    for message in invoked {
        let Ok((id, key)) = message.body().deserialize::<(u32, String)>() else {
            continue;
        };
        if key == DEFAULT_ACTION {
            crate::popup::show_near_cursor();
            super::wake::wake();
            continue;
        }
        let action = key
            .strip_prefix('a')
            .and_then(|index| index.parse::<usize>().ok())
            .and_then(|index| {
                state()
                    .lock()
                    .ok()?
                    .actions
                    .get(&id)
                    .and_then(|actions| actions.get(index).cloned())
            });
        if let Some(action) = action {
            action.run();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_is_escaped() {
        assert_eq!(escape_markup("a < b & c > d"), "a &lt; b &amp; c &gt; d");
    }
}
