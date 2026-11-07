//! Tray popup: view state and the GPUI renderer.
//!
//! The bridge in [`crate::app`] folds worker events into [`UiState`] on the
//! tray thread and publishes snapshots to the GPUI thread through
//! [`PopupCommand`]s. The renderer in [`ui`] owns everything visual;
//! [`model`] keeps the card visibility rules framework-free and unit-tested.

use std::sync::{Arc, LazyLock, Mutex};

use chrono::{DateTime, Duration as ChronoDuration, Local, Utc};

use crate::{
    app::AppState,
    instances::{ProviderId, ProviderInstance},
    limits::{LimitWindow, ProviderLimits, RateLimits, SpendingSummary, UsageAmount},
    popup,
    provider_registry::{
        ProviderKind, UsageStats, additional_limit_brick_id, credits_brick_id, resets_brick_id,
        spending_brick_id, usage_brick_id,
    },
    settings::{
        HomeCardLayout, HomeWidgetId, PopupSurface, PopupTabMode, PopupVisibility, Settings,
        TimeFormat, TotalSpendPeriod, TotalSpendPresentation,
    },
};

pub(crate) mod formatting;
pub(crate) mod model;
mod navigation;
mod state;
pub(crate) mod ui;

#[cfg(test)]
pub(crate) mod tests;

pub(crate) use state::UiState;

use crate::format::{
    capitalize_plan_name, format_micro_usd, format_reset_in, format_time_ago, format_time_late,
    format_token_count, format_usd,
};
use formatting::*;
pub(crate) use navigation::PopupView;
use navigation::*;

#[derive(Clone, Copy, Debug)]
pub(crate) enum PendingPopupView {
    Home,
    Provider(ProviderId),
    /// Home when it shows the instance, otherwise the tab that does.
    Nearest(ProviderId),
}

static PENDING_POPUP_VIEW: Mutex<Option<PendingPopupView>> = Mutex::new(None);

/// Commands posted to the GPUI thread. Every variant is cheap to send from
/// any thread; the GPUI side wakes through its foreground executor.
pub(crate) enum PopupCommand {
    Publish(Box<UiState>),
    Show {
        anchor: Option<(i32, i32)>,
        /// Open toward the anchor's end of the taskbar, not the tray's.
        beside: bool,
    },
    Hide,
    SelectView(PopupView),
    /// Go to the page nearest an instance: Home when it shows the instance,
    /// otherwise the tab that does.
    SelectNearest(ProviderId),
    /// Enter Home edit mode, opening Home.
    StartHomeEditing,
    AppearanceChanged,
    Reposition,
    /// Persist a completed native move, then rebuild the host for its new DPI.
    Moved,
    /// Height drag updates stay in memory; persist only on release/capture loss.
    ResizeHeight {
        height_dip: f32,
        finished: bool,
        /// Floating top-edge resizing preserves the original visible bottom.
        fixed_bottom: Option<i32>,
    },
    /// Another window took the foreground; restore the popup to the top of
    /// the topmost band if it is visible.
    Raise,
    /// Settings/onboarding window requests, executed on the GPUI thread.
    Settings(crate::settings_window::Command),
    /// An in-app notification for the toast host.
    Toast(Box<crate::notifications::Notification>),
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

/// Publish the latest view state to the renderer. Hidden popups only store
/// it; the renderer repaints when it is shown, so background polls stay cheap.
pub(crate) fn publish_ui(ui: &UiState) {
    send_command(PopupCommand::Publish(Box::new(ui.clone())));
}

/// Start the GPUI renderer thread. Returns false when it already runs.
pub(crate) fn start(state: Arc<AppState>) -> bool {
    let Some(receiver) = COMMANDS.1.lock().ok().and_then(|mut slot| slot.take()) else {
        return false;
    };
    ui::start(state, receiver);
    true
}

/// What a tray click, a taskbar widget, the shortcut and a Stream Deck key
/// do: hide a popup that is up, unless Settings or onboarding previews in it,
/// otherwise open it on `view` (the last page when `None`).
pub(crate) fn toggle(origin: popup::Origin, view: Option<PendingPopupView>) {
    if popup::is_up_for(origin) {
        if !crate::settings_window::is_open() {
            popup::hide();
        }
    } else {
        if let Some(view) = view {
            request_view(view);
        }
        popup::show(origin);
    }
}

/// Shows the popup unless it is up already. Returns whether it was shown.
pub(crate) fn ensure_shown(origin: popup::Origin) -> bool {
    let show = !popup::is_up_for(origin);
    if show {
        popup::show(origin);
    }
    show
}

/// Requests a page for the next popup show; an open popup switches at once.
/// The request is intentionally ephemeral, like the clicks that make it.
pub(crate) fn request_view(view: PendingPopupView) {
    if popup::is_open() {
        // An open popup switches pages right away, with its usual slide.
        send_command(match view {
            PendingPopupView::Home => PopupCommand::SelectView(PopupView::Home),
            PendingPopupView::Provider(provider) => {
                PopupCommand::SelectView(PopupView::Provider(provider))
            }
            PendingPopupView::Nearest(provider) => PopupCommand::SelectNearest(provider),
        });
    } else if let Ok(mut pending) = PENDING_POPUP_VIEW.lock() {
        *pending = Some(view);
    }
}

/// Shows the popup in Home edit mode, wherever the request comes from.
pub(crate) fn start_home_editing() {
    ensure_shown(popup::Origin::Cursor);
    send_command(PopupCommand::StartHomeEditing);
}

fn take_popup_view_request() -> Option<PendingPopupView> {
    PENDING_POPUP_VIEW
        .lock()
        .ok()
        .and_then(|mut pending| pending.take())
}
