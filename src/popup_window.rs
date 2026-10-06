//! Tray popup: data bridge, view state and the GPUI renderer.
//!
//! `bridge` folds worker events into [`UiState`] on the tray thread and
//! publishes snapshots to the GPUI thread through [`PopupCommand`]s. The
//! renderer in [`ui`] owns everything visual; [`model`] keeps the card
//! visibility rules framework-free and unit-tested.

use std::{
    collections::HashMap,
    sync::{
        Arc, LazyLock, Mutex,
        mpsc::{Receiver, Sender},
    },
    thread,
    time::Duration,
};

use chrono::{DateTime, Duration as ChronoDuration, Local, Utc};

use crate::{
    limits::{LimitWindow, ProviderLimits, RateLimits, SpendingSummary, UsageAmount},
    notifications,
    notifications::LimitNotificationTracker,
    popup,
    provider_registry::{
        LimitSectionKind, additional_limit_brick_id, credits_brick_id, limit_section_brick_id,
        resets_brick_id, spending_brick_id, usage_brick_id,
    },
    settings::{
        AccentColor, AppTheme, HomeWidgetId, NotificationSettings, PopupBackgroundMaterial,
        PopupSurface, PopupVisibility, PopupWidgetKind, ProviderKind, Settings, TimeFormat,
        TotalSpendPeriod, TotalSpendPresentation, TrayWidget,
    },
    tray::{TrayManager, TrayMenuAction},
    updater::{UpdateController, UpdatePhase},
    worker::{RequestKind, UsageAction, WorkerCommand, WorkerEvent},
};

mod actions;
mod bridge;
mod chrome;
mod formatting;
pub(crate) mod model;
mod navigation;
mod state;
pub(crate) mod ui;

#[cfg(test)]
mod tests;

pub use state::AppState;
pub(crate) use state::UiState;

pub(crate) use actions::*;
use bridge::*;
use chrome::*;
use formatting::*;
pub(crate) use navigation::PopupView;
use navigation::*;
use state::*;

#[derive(Clone, Copy, Debug)]
pub(crate) enum PendingPopupView {
    Home,
    Provider(ProviderKind),
}

static PENDING_POPUP_VIEW: Mutex<Option<PendingPopupView>> = Mutex::new(None);

/// Commands posted to the GPUI thread. Every variant is cheap to send from
/// any thread; the GPUI side wakes through its foreground executor.
pub(crate) enum PopupCommand {
    Publish(Box<UiState>),
    Show { anchor: Option<(i32, i32)> },
    Hide,
    SelectView(PopupView),
    AppearanceChanged,
    Reposition,
}

type CommandChannel = (
    futures::channel::mpsc::UnboundedSender<PopupCommand>,
    Mutex<Option<futures::channel::mpsc::UnboundedReceiver<PopupCommand>>>,
);

/// Commands sent before the GPUI thread starts simply queue up here.
static COMMANDS: LazyLock<CommandChannel> = LazyLock::new(|| {
    let (sender, receiver) = futures::channel::mpsc::unbounded();
    (sender, Mutex::new(Some(receiver)))
});

pub(crate) fn send_command(command: PopupCommand) {
    let _ = COMMANDS.0.unbounded_send(command);
}

/// Publish the latest view state to the renderer.
pub(crate) fn publish_ui(ui: &UiState) {
    send_command(PopupCommand::Publish(Box::new(ui.clone())));
}

/// Start the popup: the GPUI renderer thread and the tray/worker bridge.
///
/// `ui_dispatcher` targets the main STA; it forwards to WinUI only after the
/// first Settings/onboarding request starts the XAML application.
pub fn start(state: Arc<AppState>, ui_dispatcher: windows_reactor::UiMarshaller) {
    let Some(receiver) = COMMANDS.1.lock().ok().and_then(|mut slot| slot.take()) else {
        return;
    };
    ui::start(Arc::clone(&state), receiver);
    start_background_bridge(state, ui_dispatcher);
}

/// Requests a provider tab for the next popup show. The request is
/// intentionally ephemeral, matching clicks from the tray and Stream Deck.
pub fn request_provider_view(provider: ProviderKind) {
    request_view(PendingPopupView::Provider(provider));
}

pub fn request_home_view() {
    request_view(PendingPopupView::Home);
}

fn request_view(view: PendingPopupView) {
    if popup::is_visible() && !popup::is_closing() {
        // An open popup switches pages right away, with its usual slide.
        send_command(PopupCommand::SelectView(match view {
            PendingPopupView::Home => PopupView::Home,
            PendingPopupView::Provider(provider) => PopupView::from_provider(provider),
        }));
    } else if let Ok(mut pending) = PENDING_POPUP_VIEW.lock() {
        *pending = Some(view);
    }
}

fn take_popup_view_request() -> Option<PendingPopupView> {
    PENDING_POPUP_VIEW
        .lock()
        .ok()
        .and_then(|mut pending| pending.take())
}
