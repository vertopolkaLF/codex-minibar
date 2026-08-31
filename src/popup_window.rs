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
        AccentColor, AppTheme, NotificationSettings, PopupSurface, PopupVisibility,
        PopupWidgetKind, ProviderKind, Settings, TimeFormat, TotalSpendPeriod,
        TotalSpendPresentation, TrayWidget,
    },
    settings_controls::update_accent_button,
    tray::{TrayManager, TrayMenuAction},
    updater::{UpdateController, UpdatePhase},
    usage_overview::{BreakdownMode, OverviewMetric, OverviewRange, build_overview_snapshot},
    worker::{RequestKind, WorkerCommand, WorkerEvent},
};

#[cfg(windows)]
static KEEP_ON_MONITOR_QUEUED: AtomicBool = AtomicBool::new(false);

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

use bridge::*;
use cards::*;
use chrome::*;
use formatting::*;
use interactions::*;
use navigation::*;
use state::*;
use usage_cards::*;
