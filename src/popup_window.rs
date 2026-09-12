use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender},
    },
    thread,
    time::{Duration, Instant},
};

use chrono::{DateTime, Duration as ChronoDuration, Local, Utc};
use windows_reactor::*;

use crate::{
    limits::{
        LimitWindow, OpenRouterAccountSnapshot, PaceTip, ProviderLimits, RateLimits,
        SpendingSummary,
    },
    notifications,
    notifications::LimitNotificationTracker,
    popup,
    provider_registry::{
        LimitSectionKind, additional_limit_brick_id, credits_brick_id, limit_section_brick_id,
        resets_brick_id, spending_brick_id, usage_brick_id,
    },
    settings::{
        AccentColor, AppTheme, NotificationSettings, PopupBackgroundMaterial, PopupSurface,
        PopupVisibility,
        PopupWidgetKind, ProviderKind, Settings, TimeFormat, TotalSpendPeriod,
        TotalSpendPresentation, TrayWidget,
    },
    tray::{TrayManager, TrayMenuAction},
    updater::{UpdateController, UpdatePhase},
    usage_overview::{BreakdownMode, OverviewMetric, OverviewRange, build_overview_snapshot},
    worker::{RequestKind, UsageAction, WorkerCommand, WorkerEvent},
};

#[cfg(windows)]
static KEEP_ON_MONITOR_QUEUED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
enum PendingPopupView {
    Home,
    Provider(ProviderKind),
}

static PENDING_POPUP_VIEW: Mutex<Option<PendingPopupView>> = Mutex::new(None);

mod bridge;
mod cards;
mod chrome;
mod formatting;
mod interactions;
mod navigation;
mod shell;
mod state;
mod usage_cards;

#[cfg(test)]
mod tests;

pub use shell::app;
pub use state::AppState;

/// Requests a provider tab for the next popup render. The request is
/// intentionally ephemeral, matching clicks from the tray and Stream Deck.
pub fn request_provider_view(provider: ProviderKind) {
    if let Ok(mut pending) = PENDING_POPUP_VIEW.lock() {
        *pending = Some(PendingPopupView::Provider(provider));
    }
    windows_reactor::request_ui_rerender_on_ui_thread();
}

pub fn request_home_view() {
    if let Ok(mut pending) = PENDING_POPUP_VIEW.lock() {
        *pending = Some(PendingPopupView::Home);
    }
    windows_reactor::request_ui_rerender_on_ui_thread();
}

fn take_popup_view_request() -> Option<PendingPopupView> {
    PENDING_POPUP_VIEW
        .lock()
        .ok()
        .and_then(|mut pending| pending.take())
}

use bridge::*;
use cards::*;
use chrome::*;
use formatting::*;
use interactions::*;
use navigation::*;
use state::*;
use usage_cards::*;
