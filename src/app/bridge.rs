//! The tray bridge: one thread owns the tray icons and the shortcut, folds
//! worker events and settings commits into [`UiState`], and repaints every
//! surface from the shared snapshots when they change.

use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError},
    },
    thread,
    time::Duration,
};

use chrono::{DateTime, Local, Utc};

use super::{
    auto_reset::AutoReset,
    state::{AppState, Changes},
};
use crate::{
    app::event::{AppEvent, ProviderEvent},
    format::format_reset_in,
    providers::instances::{ProviderId, ProviderInstance},
    providers::registry::ProviderKind,
    providers::worker::{UsageAction, WorkerCommand, WorkerEvent},
    quota::limits::{ProviderLimits, RateLimits},
    settings::{NotificationSettings, Settings, TimeFormat, TrayWidget},
    surfaces::notifications::{self, LimitNotificationTracker},
    surfaces::tray::{MenuCommand, TrayManager},
    ui::popup::{self, Origin},
    ui::popup_window::{
        PendingPopupView, UiState,
        formatting::{format_activation_at, format_last_activation},
        publish_ui,
    },
    update::UpdatePhase,
};

/// Rejects queued output from a worker that was replaced, and from instances
/// that were disabled or removed since the event was sent.
fn provider_worker_event_is_current(
    ui: &UiState,
    provider: ProviderId,
    worker_revision: u64,
) -> bool {
    ui.settings.is_enabled(provider) && worker_revision == ui.credentials_revision(provider)
}

/// Flatten provider usage errors, including per-account errors exposed by
/// OpenRouter, into the provider-scoped presentation used by the popup.
fn usage_error_message(statistics: &crate::usage::UsageStatistics) -> Option<String> {
    let mut errors = Vec::new();
    let mut add_error = |error: Option<&str>| {
        let Some(error) = error else {
            return;
        };
        let error = error.trim();
        if !error.is_empty() && !errors.iter().any(|seen| seen == error) {
            errors.push(error.to_owned());
        }
    };
    add_error(statistics.error.as_deref());
    for account in statistics.accounts.values() {
        add_error(account.error.as_deref());
    }
    (!errors.is_empty()).then(|| errors.join("; "))
}

fn forced_reset_info_body(reset: &crate::quota::reset_detection::AnnouncedReset) -> String {
    let local = reset.reset_at.with_timezone(&Local);
    let when = format!(
        "{}, {}",
        crate::i18n::month_day(local),
        TimeFormat::current().format_hm(local)
    );
    let countdown = format_reset_in(Some(reset.reset_at));
    let before = reset.timing == crate::quota::reset_detection::ResetTiming::Before;
    let mut args = vec![("when", when), ("countdown", countdown)];
    match reset.label.as_deref() {
        Some(label) => {
            args.push(("label", label.to_owned()));
            crate::i18n::format(
                if before {
                    "label-possible-reset-before-when-within-countdown"
                } else {
                    "label-possible-reset-on-when-in-countdown"
                },
                &args,
            )
        }
        None => crate::i18n::format(
            if before {
                "a-possible-codex-reset-is-expected-before-when-within-countdown"
            } else {
                "a-possible-codex-reset-is-scheduled-for-when-in-countdown"
            },
            &args,
        ),
    }
}

/// Notifies only that new feed information arrived. This is intentionally
/// unrelated to the API-driven `limits_changed` notification: the feed never
/// proves that a reset actually happened at `reset_at`.
fn notify_new_forced_reset_info(
    resets: &[crate::quota::reset_detection::AnnouncedReset],
    notified_ids: &mut HashSet<String>,
    settings: &NotificationSettings,
    state: &AppState,
) {
    if !settings.forced_reset_feed_enabled || !settings.forced_reset_notifications {
        return;
    }
    let now = Utc::now();
    for reset in resets.iter().filter(|reset| reset.reset_at > now) {
        if notified_ids.insert(reset.id.clone()) {
            notifications::show(
                match reset.kind {
                    crate::quota::reset_detection::ResetKind::Forced => {
                        crate::i18n::tr("new-codex-reset-info")
                    }
                    crate::quota::reset_detection::ResetKind::Banked => {
                        crate::i18n::tr("new-codex-banked-reset-info")
                    }
                },
                &forced_reset_info_body(reset),
            );
            state.mark_forced_reset_info_notified(reset.id.clone());
        }
    }
}

/// Notifies once per incident (never for maintenance) of a watched driver.
fn notify_new_provider_incidents(
    statuses: &[crate::providers::status::ProviderStatus],
    notified_ids: &mut HashSet<String>,
    settings: &NotificationSettings,
    state: &AppState,
) {
    if !settings.provider_status_enabled || !settings.provider_status_notifications {
        return;
    }
    for status in statuses {
        for incident in status
            .incidents
            .iter()
            .filter(|incident| !incident.is_maintenance())
        {
            if notified_ids.insert(incident.id.clone()) {
                notifications::show_provider_incident(status.kind, incident);
                state.mark_provider_incident_notified(incident.id.clone());
            }
        }
    }
}

/// Notifies once per login when it enters Claude Code's three-day warning
/// before it stops renewing. A new sign-in moves the deadline and re-arms it.
fn warn_login_expiry(provider: ProviderId, limits: &RateLimits) {
    static WARNED: Mutex<Vec<(ProviderId, DateTime<Utc>)>> = Mutex::new(Vec::new());
    let Some(expires_at) = limits.login_expires_at else {
        return;
    };
    let left = expires_at - Utc::now();
    if left <= chrono::Duration::zero() || left > chrono::Duration::days(3) {
        return;
    }
    let Ok(mut warned) = WARNED.lock() else {
        return;
    };
    if warned.contains(&(provider, expires_at)) {
        return;
    }
    warned.retain(|(warned, _)| *warned != provider);
    warned.push((provider, expires_at));
    let local = expires_at.with_timezone(&chrono::Local);
    crate::surfaces::notifications::show_kind(
        crate::surfaces::notifications::NotificationKind::Warning,
        &crate::i18n::format(
            "login-expires-soon",
            &[("v0", provider.qualified_name().to_string())],
        ),
        &crate::i18n::format(
            "it-stops-renewing-on-open-minibar-and-choose-sign-in-again",
            &[(
                "v0",
                (format!(
                    "{}, {}",
                    crate::i18n::month_day(local),
                    TimeFormat::current().format_hm(local)
                ))
                .to_string(),
            )],
        ),
    );
}

/// Instances whose worker must be replaced: their read-relevant settings
/// (paths, credential source, keys, revision) changed.
fn instances_needing_restart(
    before: &[ProviderInstance],
    after: &[ProviderInstance],
) -> Vec<ProviderId> {
    after
        .iter()
        .filter(|instance| instance.enabled)
        .filter(|instance| {
            before
                .iter()
                .find(|old| old.id == instance.id)
                .is_some_and(|old| old.enabled && old.runtime_key() != instance.runtime_key())
        })
        .map(ProviderInstance::provider_id)
        .collect()
}

/// Sends a worker the poll intervals it should use right now. Workers ignore
/// an unchanged interval, so resending after any change is safe.
fn send_refresh_intervals(
    provider: ProviderId,
    commands: &Sender<WorkerCommand>,
    settings: &Settings,
) {
    if let Some(instance) = settings.instance(provider) {
        let _ = commands.send(WorkerCommand::SetLimitRefreshInterval(
            crate::providers::limit_refresh_interval(instance),
        ));
    }
    let _ = commands.send(WorkerCommand::SetUsageRefreshInterval(
        crate::providers::usage_refresh_interval(settings),
    ));
}

/// Transient popups need frame-rate dismissal polling; pinned panels use the
/// idle cadence. GPUI drives their animations without polling this bridge.
fn bridge_surface_active() -> bool {
    (popup::is_visible() && !popup::is_pinned())
        || popup::is_closing()
        || crate::ui::settings_window::is_open()
}

/// Idle tick is short enough for the 250 ms system-theme check, stream deck
/// and update toasts; window messages (tray) wake the idle wait immediately.
fn bridge_poll_interval(active: bool) -> Duration {
    Duration::from_millis(if active {
        16
    } else if crate::app::game_mode::is_active() {
        // Still meets the 250 ms theme check while waking less often.
        250
    } else {
        100
    })
}

fn visible_widgets(settings: &Settings) -> Vec<TrayWidget> {
    settings
        .tray_widgets
        .iter()
        .filter(|widget| widget.is_visible_for(&settings.instances))
        .cloned()
        .collect()
}

fn take<T>(slot: &Mutex<Option<T>>) -> Option<T> {
    slot.lock().ok().and_then(|mut slot| slot.take())
}

/// What one flush repaints for the snapshots that changed.
#[derive(Debug, PartialEq, Eq)]
struct FanOut {
    /// Tray, taskbar and Settings take the live quota snapshot.
    sync_limits: bool,
    /// The popup takes `ui`.
    publish: bool,
}

impl FanOut {
    /// Folds usage changes into `ui` and decides the rest.
    fn plan(changes: Changes, ui: &mut UiState, ui_changed: bool) -> Self {
        if changes.contains(Changes::USAGE) {
            ui.usage_revision = ui.usage_revision.wrapping_add(1);
        }
        Self {
            sync_limits: changes.contains(Changes::LIMITS),
            publish: ui_changed || !changes.is_empty(),
        }
    }
}

pub(super) fn start(state: Arc<AppState>) {
    thread::spawn(move || Bridge::new(state).run());
}

struct Bridge {
    state: Arc<AppState>,
    /// The popup's view state; `settings` in it is the live settings.
    ui: UiState,
    /// `ui` changed since it was last published.
    ui_changed: bool,
    tray: TrayManager,
    widgets: Vec<TrayWidget>,
    #[cfg(windows)]
    hotkey: crate::app::hotkey::Registration,
    update_phase: UpdatePhase,
    events: Option<Receiver<AppEvent>>,
    /// The last wait delivered an event, so the next one does not wait.
    made_progress: bool,
    settings_rx: Option<Receiver<Settings>>,
    usage_actions_rx: Option<Receiver<UsageAction>>,
    streamdeck_rx: Receiver<crate::surfaces::streamdeck::Command>,
    auto_reset: AutoReset,
    /// Keyed by instance id.
    limit_notifications: HashMap<String, LimitNotificationTracker>,
    /// Credential revision each tracker was primed with. A tracker primed
    /// on one login must not compare its reset time with another's.
    notified_revisions: HashMap<String, u64>,
    pending_auto_activation_successes: HashSet<ProviderId>,
    forced_reset_notified_ids: HashSet<String>,
    incident_notified_ids: HashSet<String>,
    usage_clear_generation: u64,
    pending_usage_clear: Option<(u64, Vec<ProviderId>)>,
    /// Display currency or rates last painted; see [`crate::format::currency`].
    currency_generation: u64,
}

impl Bridge {
    fn new(state: Arc<AppState>) -> Self {
        let ui = UiState::initial(&state);
        let mut bridge = Self {
            streamdeck_rx: crate::surfaces::streamdeck::start_server(Arc::clone(&state)),
            tray: TrayManager::new(),
            // Registered from this thread so WM_HOTKEY reaches its message pump.
            #[cfg(windows)]
            hotkey: crate::app::hotkey::Registration::default(),
            widgets: visible_widgets(&ui.settings),
            update_phase: state.updates.snapshot(),
            events: state.take_worker_events(),
            made_progress: false,
            settings_rx: take(&state.settings_rx),
            usage_actions_rx: take(&state.usage_actions_rx),
            auto_reset: AutoReset::new(),
            limit_notifications: HashMap::new(),
            notified_revisions: HashMap::new(),
            pending_auto_activation_successes: HashSet::new(),
            forced_reset_notified_ids: HashSet::new(),
            incident_notified_ids: HashSet::new(),
            usage_clear_generation: 0,
            pending_usage_clear: None,
            currency_generation: crate::format::currency::generation(),
            ui,
            ui_changed: true,
            state,
        };
        crate::surfaces::taskbar::register_bridge_thread();
        // Start from the hydrated store while the first network refresh is
        // in flight. Account names live in settings; overlay them before the
        // first paint.
        bridge
            .state
            .apply_openrouter_account_names(&bridge.ui.settings);
        bridge.state.mark(Changes::LIMITS);
        bridge.flush();
        bridge
    }

    fn run(mut self) {
        loop {
            if self.tick() {
                self.exit();
            }
            self.flush();
            if let Some(event) = self.next_event() {
                self.handle_event(event);
                self.flush();
            }
        }
    }

    fn exit(self) -> ! {
        drop(self.tray);
        AppState::flush_limits_persistence();
        self.state.shutdown_worker();
        std::process::exit(0);
    }

    /// One pass over every input except worker events. Returns whether the
    /// app should exit.
    fn tick(&mut self) -> bool {
        popup::pump_messages();
        self.drain_streamdeck();
        self.drain_usage_actions();
        if let Err(error) = self
            .tray
            .refresh_system_theme(&self.widgets, &self.state.current_limits())
        {
            self.ui.set_popup_error(error.to_string());
            self.ui_changed = true;
        }
        self.drain_settings();
        self.drain_updates();
        self.sync_currency();
        self.auto_reset.poll(&self.ui.settings, &self.state);
        if crate::app::game_mode::take_transition() {
            for (provider, commands) in self.state.worker_commands() {
                send_refresh_intervals(provider, &commands, &self.ui.settings);
            }
        }
        // A popup opened below shows the state gathered so far.
        self.flush();
        self.pump_hotkey();
        let exit = self.pump_taskbar_actions();
        exit || self.pump_tray()
    }

    /// Active surfaces poll at frame rate and wake on worker events. Idle
    /// (popup hidden, Settings closed) the thread sleeps in the message wait,
    /// which tray and popup window messages interrupt, and only looks at the
    /// channels on a coarse tick. After an event the next wait is skipped, so
    /// bursts drain at full speed.
    fn next_event(&mut self) -> Option<AppEvent> {
        let active = bridge_surface_active();
        let Some(events) = &self.events else {
            popup::wait_for_messages(bridge_poll_interval(active));
            return None;
        };
        let next = if self.made_progress {
            events
                .try_recv()
                .map_err(|error| error == TryRecvError::Disconnected)
        } else if active {
            events
                .recv_timeout(bridge_poll_interval(true))
                .map_err(|error| error == RecvTimeoutError::Disconnected)
        } else {
            popup::wait_for_messages(bridge_poll_interval(false));
            events
                .try_recv()
                .map_err(|error| error == TryRecvError::Disconnected)
        };
        self.made_progress = next.is_ok();
        match next {
            Ok(event) => Some(event),
            Err(disconnected) => {
                if disconnected {
                    self.events = None;
                }
                None
            }
        }
    }

    /// Repaints every surface from the shared snapshots that changed, then
    /// publishes the popup state if anything did.
    fn flush(&mut self) {
        let fan_out = FanOut::plan(self.state.take_changes(), &mut self.ui, self.ui_changed);
        if fan_out.sync_limits {
            self.sync_limits();
        }
        if fan_out.publish {
            self.ui_changed = false;
            publish_ui(&self.ui);
        }
    }

    /// Hands the live quota snapshot to every surface besides the popup,
    /// which re-reads it with each publish.
    fn sync_limits(&mut self) {
        let limits = self.state.current_limits();
        self.sync_tray(&limits);
        crate::surfaces::taskbar::publish_limits(&limits);
        crate::ui::settings_window::publish_limits(&limits);
        if self
            .ui
            .settings
            .popup_visibility
            .clone()
            .absorb_discovered_bricks(&limits)
        {
            self.ui.edit_settings(move |settings| {
                settings.popup_visibility.absorb_discovered_bricks(&limits);
            });
        }
        self.ui_changed = true;
    }

    /// Repaints the existing native icons in place. Recreating them makes
    /// Explorer animate a remove/add sequence and causes a visible flash.
    fn sync_tray(&mut self, limits: &ProviderLimits) {
        let update_available = matches!(self.update_phase, UpdatePhase::Available(_));
        crate::surfaces::taskbar::set_update_available(update_available);
        if let Err(error) = self.tray.sync(&self.widgets, limits, update_available) {
            self.ui.set_popup_error(error.to_string());
        }
        self.ui_changed = true;
    }

    /// Rates arrive on their own thread; every surface that shows a usage
    /// cost repaints once they or the chosen currency change.
    fn sync_currency(&mut self) {
        let generation = crate::format::currency::generation();
        if generation != self.currency_generation {
            self.currency_generation = generation;
            self.state.mark(Changes::LIMITS);
            self.state.mark(Changes::USAGE);
        }
    }

    fn drain_settings(&mut self) {
        while let Some(settings) = self
            .settings_rx
            .as_ref()
            .and_then(|settings_rx| settings_rx.try_recv().ok())
        {
            self.apply_settings(settings);
        }
    }

    fn apply_settings(&mut self, settings: Settings) {
        let state = Arc::clone(&self.state);
        // Also affects checks already in flight, whose result may arrive later.
        state.updates.set_notifications_enabled(
            settings.check_for_updates && settings.notifications.update_available,
        );
        if settings.check_for_updates && !self.ui.settings.check_for_updates {
            state.updates.check_async(false);
        }
        let previous = Arc::clone(&self.ui.settings);
        let providers_changed = previous.enabled_providers() != settings.enabled_providers()
            || previous.instances.len() != settings.instances.len();
        let restart = instances_needing_restart(&previous.instances, &settings.instances);
        for provider in &restart {
            // A replaced credential or path must not present the old login's
            // quota as the new one's. The matching worker revision also
            // rejects output already queued by the worker being replaced.
            state.replace_limits(*provider, RateLimits::default());
        }
        state.apply_openrouter_account_names(&settings);
        state.sync_reset_feed(&settings);
        state.sync_provider_status(&settings);
        if !settings.notifications.provider_status_enabled {
            state.replace_provider_statuses(Vec::new());
        }
        if !settings.notifications.forced_reset_feed_enabled {
            state.replace_forced_resets(Vec::new());
        }
        self.widgets = visible_widgets(&settings);
        for provider in self
            .ui
            .active_requests
            .iter()
            .map(|(provider, _)| *provider)
            .collect::<Vec<_>>()
        {
            if restart.contains(&provider) || !settings.is_enabled(provider) {
                self.ui.clear_provider_requests(provider);
            }
        }
        self.ui.set_settings(settings);
        // Presentation settings must visibly apply before any background
        // work. In particular, changing provider icons must never wait on a
        // worker lock, network request, or provider lifecycle change. The
        // tray and taskbar repaint once, in the flush that ends this tick.
        publish_ui(&self.ui);
        self.ui_changed = true;
        let settings = Arc::clone(&self.ui.settings);
        if providers_changed || !restart.is_empty() {
            for (provider, error) in state.sync_provider_workers(&settings, &restart) {
                self.ui.set_provider_error(provider, error);
            }
        }
        self.ui
            .provider_errors
            .retain(|provider, _| settings.is_enabled(*provider));
        self.ui
            .usage_errors
            .retain(|provider, _| settings.usage_stats_collection_enabled(*provider));
        // The visible tray widgets may have changed.
        state.mark(Changes::LIMITS);
        for (provider, commands) in state.worker_commands() {
            let _ = commands.send(WorkerCommand::SetAutomaticActivation(
                crate::providers::automatic_activation(provider, &settings),
            ));
            let _ = commands.send(WorkerCommand::SetScheduledActivations(
                crate::providers::schedules_for(provider, &settings),
            ));
            let _ = commands.send(WorkerCommand::SetAutoActivationPauses(
                crate::providers::auto_activation_pauses_for(provider, &settings),
            ));
            send_refresh_intervals(provider, &commands, &settings);
            // The worker reloads the selected history range immediately, so
            // the open popup reflects the change.
            let _ = commands.send(WorkerCommand::SetHistoryRetentionDays(
                settings.history_retention_days,
            ));
            let _ = commands.send(WorkerCommand::SetUsageCollectionEnabled(
                settings.usage_stats_collection_enabled(provider),
            ));
        }
        notify_new_forced_reset_info(
            &state.current_forced_resets(),
            &mut self.forced_reset_notified_ids,
            &settings.notifications,
            &state,
        );
    }

    fn drain_usage_actions(&mut self) {
        while let Some(UsageAction::ClearData) = self
            .usage_actions_rx
            .as_ref()
            .and_then(|actions| actions.try_recv().ok())
        {
            // One clear operation is enough. The button remains safe to click
            // while a previous provider barrier is draining.
            if self.pending_usage_clear.is_some() {
                continue;
            }
            self.usage_clear_generation = self.usage_clear_generation.wrapping_add(1);
            let generation = self.usage_clear_generation;
            let targets = self
                .state
                .worker_commands()
                .into_iter()
                .filter_map(|(provider, commands)| {
                    commands
                        .send(WorkerCommand::ClearUsageData(generation))
                        .is_ok()
                        .then_some(provider)
                })
                .collect::<Vec<_>>();
            self.state.clear_usage_snapshot();
            // The wipe itself runs once, off this thread, after the barrier
            // completes (immediately when nobody can ack).
            if targets.is_empty() {
                spawn_usage_wipe(self.state.worker_events_tx.clone(), generation);
            }
            self.pending_usage_clear = Some((generation, targets));
        }
    }

    fn drain_updates(&mut self) {
        let next = self.state.updates.snapshot();
        if next == self.update_phase {
            return;
        }
        self.update_phase = next;
        self.ui.update_version = match &self.update_phase {
            UpdatePhase::Available(update) => Some(update.version.clone()),
            _ => None,
        };
        self.sync_tray(&self.state.current_limits());
    }

    fn drain_streamdeck(&mut self) {
        while let Ok(command) = self.streamdeck_rx.try_recv() {
            match command {
                crate::surfaces::streamdeck::Command::RefreshData => {
                    self.state.refresh_all();
                }
                #[cfg(not(windows))]
                crate::surfaces::streamdeck::Command::RefreshAfterResume => {
                    for (_, commands) in self.state.worker_commands() {
                        let _ = commands.send(WorkerCommand::RefreshAutomatic);
                    }
                }
                // Panel widgets ask for one provider; unlike a key press this
                // never hides a popup that is already up.
                #[cfg(not(windows))]
                crate::surfaces::streamdeck::Command::ShowProvider { provider } => {
                    crate::ui::popup_window::request_view(PendingPopupView::Provider(provider));
                    crate::ui::popup_window::ensure_shown(Origin::PrimaryMonitor);
                }
                crate::surfaces::streamdeck::Command::OpenPopup { provider } => {
                    crate::ui::popup_window::toggle(
                        Origin::PrimaryMonitor,
                        Some(provider.map_or(PendingPopupView::Home, PendingPopupView::Provider)),
                    );
                }
                crate::surfaces::streamdeck::Command::OpenCommandPaletteSettings => {
                    crate::ui::settings_window::open_command_palette();
                }
            }
        }
    }

    /// Linux has no in-process hotkey: the desktop runs
    /// `codex-minibar --toggle`, which the single-instance socket delivers
    /// (`platform::linux::instance`).
    #[cfg(not(windows))]
    fn pump_hotkey(&mut self) {}

    /// Keeps the popup shortcut registered as configured and toggles the
    /// popup when it is pressed, the same way a tray click does.
    #[cfg(windows)]
    fn pump_hotkey(&mut self) {
        // Settings releases the shortcut while it records a new one; the flag
        // cannot outlive the window that set it.
        let recording = crate::app::hotkey::is_suspended() && crate::ui::settings_window::is_open();
        if self
            .hotkey
            .sync(self.ui.settings.popup_hotkey.filter(|_| !recording))
        {
            crate::ui::settings_window::refresh_hotkey_status();
        }
        if crate::app::hotkey::take_pressed() && !recording {
            crate::ui::popup_window::toggle(Origin::Cursor, None);
        }
    }

    /// Clicks and menu choices from the taskbar widgets. Returns whether the
    /// app should exit.
    fn pump_taskbar_actions(&mut self) -> bool {
        for action in crate::surfaces::taskbar::take_actions() {
            match action {
                crate::surfaces::taskbar::Action::Open {
                    provider,
                    anchor,
                    page,
                } => {
                    use crate::settings::TaskbarClick;
                    let view = match (provider, page) {
                        (Some(provider), TaskbarClick::Nearest) => {
                            PendingPopupView::Nearest(provider)
                        }
                        (Some(provider), TaskbarClick::Tab) => PendingPopupView::Provider(provider),
                        _ => PendingPopupView::Home,
                    };
                    crate::ui::popup_window::toggle(
                        Origin::TaskbarWidget(anchor.0, anchor.1),
                        Some(view),
                    );
                }
                crate::surfaces::taskbar::Action::Edit(id) => {
                    show_settings_preview();
                    crate::ui::settings_window::open_taskbar_widget(id);
                }
                crate::surfaces::taskbar::Action::Settings => {
                    show_settings_preview();
                    crate::ui::settings_window::open_taskbar();
                }
                crate::surfaces::taskbar::Action::Hide => {
                    crate::settings::store::queue(|settings| {
                        settings.taskbar.enabled = false;
                    });
                }
                crate::surfaces::taskbar::Action::App(command) => {
                    if run_menu_command(command) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Tray clicks and menu choices, and outside-click/Escape dismissal.
    /// Returns whether the app should exit.
    #[cfg(windows)]
    fn pump_tray(&mut self) -> bool {
        use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};

        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click {
                id,
                position,
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
                && self.tray.contains(&id)
            {
                crate::ui::popup_window::toggle(
                    Origin::Tray(position.x as i32, position.y as i32),
                    None,
                );
            }
        }

        for command in self.tray.drain_menu_actions() {
            if run_menu_command(command) {
                return true;
            }
        }

        // Settings are a live editor for this surface. Treat the separate
        // settings window as part of the popup interaction so navigating or
        // toggling a setting cannot dismiss the preview beneath it.
        if !crate::ui::settings_window::is_open()
            && !popup::is_closing()
            && (popup::clicked_outside() || popup::escape_pressed())
        {
            popup::hide();
        }
        false
    }

    /// Tray input arrives from the StatusNotifierItem threads. Outside clicks
    /// and Escape are handled by the popup window itself (focus loss and key
    /// events): Linux clients cannot observe global input on Wayland.
    #[cfg(not(windows))]
    fn pump_tray(&mut self) -> bool {
        use crate::surfaces::tray::TrayEvent;

        for event in self.tray.drain_events() {
            match event {
                TrayEvent::Activate(x, y) => {
                    if popup::just_dismissed_by_focus_loss()
                        && (!popup::is_visible() || popup::is_closing())
                    {
                        // The click that closed the popup by taking its focus.
                        continue;
                    }
                    let origin = if (x, y) == (0, 0) {
                        Origin::Cursor
                    } else {
                        Origin::Tray(x, y)
                    };
                    crate::ui::popup_window::toggle(origin, None);
                }
                TrayEvent::ShowPopup => {
                    crate::ui::popup_window::ensure_shown(Origin::Cursor);
                }
                TrayEvent::Menu(command) => {
                    if run_menu_command(command) {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::ForcedResetsUpdated(snapshot) => {
                self.forced_reset_notified_ids.extend(snapshot.notified_ids);
                let notifications = &self.ui.settings.notifications;
                self.state
                    .replace_forced_resets(if notifications.forced_reset_feed_enabled {
                        snapshot.resets.clone()
                    } else {
                        Vec::new()
                    });
                notify_new_forced_reset_info(
                    &snapshot.resets,
                    &mut self.forced_reset_notified_ids,
                    notifications,
                    &self.state,
                );
            }
            AppEvent::ProviderStatusUpdated(snapshot) => {
                self.incident_notified_ids.extend(snapshot.notified_ids);
                let notifications = &self.ui.settings.notifications;
                let statuses = if notifications.provider_status_enabled {
                    snapshot.statuses
                } else {
                    Vec::new()
                };
                notify_new_provider_incidents(
                    &statuses,
                    &mut self.incident_notified_ids,
                    notifications,
                    &self.state,
                );
                self.state.replace_provider_statuses(statuses);
            }
            AppEvent::UsageClearFinished(generation, error) => {
                if self
                    .pending_usage_clear
                    .as_ref()
                    .is_some_and(|(pending, _)| *pending == generation)
                {
                    self.pending_usage_clear = None;
                    if let Some(error) = error {
                        self.ui.set_popup_error(crate::i18n::format(
                            "could-not-clear-usage-data-error",
                            &[("error", error)],
                        ));
                        self.ui_changed = true;
                    }
                    for (_, commands) in self.state.worker_commands() {
                        let _ = commands.send(WorkerCommand::ResumeUsageRefresh(generation));
                    }
                }
            }
            AppEvent::Provider(ProviderEvent {
                provider,
                revision,
                event,
            }) => self.handle_provider_event(provider, revision, event),
        }
    }

    fn handle_provider_event(&mut self, provider: ProviderId, revision: u64, event: WorkerEvent) {
        // The clear-barrier ack carries a clear generation instead of a
        // credential revision, so it is never stale.
        if !matches!(event, WorkerEvent::UsageDataCleared(_))
            && !provider_worker_event_is_current(&self.ui, provider, revision)
        {
            return;
        }
        let name = provider.display_name();
        match event {
            WorkerEvent::RequestStarted(kind) => self.ui.request_started(provider, kind),
            WorkerEvent::RequestFinished(kind) => self.ui.request_finished(provider, kind),
            WorkerEvent::LimitsUpdated(limits) => self.limits_updated(provider, limits),
            WorkerEvent::UsageUpdated(usage) => {
                if provider == ProviderId::primary(ProviderKind::Codex)
                    && usage
                        .account_id
                        .as_deref()
                        .is_some_and(|id| id != crate::store::codex_accounts::current_id())
                {
                    return;
                }
                crate::platform::logger::info(format!(
                    "{name} usage received: today={} tokens, history={} tokens",
                    usage.today.total_tokens(),
                    usage.history.total_tokens()
                ));
                let usage_error = self
                    .ui
                    .settings
                    .usage_stats_collection_enabled(provider)
                    .then(|| usage_error_message(&usage))
                    .flatten();
                self.state.replace_usage(provider, usage);
                match usage_error {
                    Some(error) => self.ui.set_usage_error(provider, error),
                    None => self.ui.clear_usage_error(provider),
                }
            }
            WorkerEvent::UsageLoadedFromCache(usage) => {
                crate::platform::logger::info(format!(
                    "{name} usage cache loaded: today={} tokens, history={} tokens",
                    usage.today.total_tokens(),
                    usage.history.total_tokens()
                ));
                // Cached account.error values are historical diagnostics, not
                // proof that the provider is failing now.
                self.state.replace_usage(provider, usage);
            }
            WorkerEvent::UsageRefreshFailed(error) => {
                if !self.ui.settings.usage_stats_collection_enabled(provider) {
                    return;
                }
                crate::platform::logger::info(format!("{name} usage refresh failed: {error}"));
                // Usage/analytics failures use the same provider-scoped
                // presentation as quota failures. The state keeps the source
                // separate so a successful quota poll does not clear an
                // active analytics error.
                self.ui.set_usage_error(provider, error);
            }
            WorkerEvent::UsageDataCleared(generation) => {
                let completed = self.pending_usage_clear.as_mut().is_some_and(
                    |(pending_generation, providers)| {
                        if *pending_generation != generation {
                            return false;
                        }
                        let before = providers.len();
                        providers.retain(|pending_provider| *pending_provider != provider);
                        before != providers.len() && providers.is_empty()
                    },
                );
                if completed {
                    // Every worker is idle and acknowledged, so any stale
                    // in-flight snapshot has already been delivered. Drop it,
                    // then wipe the store exactly once off-thread.
                    self.state.clear_usage_snapshot();
                    spawn_usage_wipe(self.state.worker_events_tx.clone(), generation);
                }
                return;
            }
            WorkerEvent::ActivationStarted => {
                crate::platform::logger::info(format!("{name} activation started"));
                return;
            }
            WorkerEvent::ActivationSucceeded => {
                crate::platform::logger::info(format!("{name} activation succeeded"));
                self.ui.last_activation = crate::i18n::format(
                    "succeeded-at",
                    &[
                        ("v0", provider.qualified_name().to_string()),
                        ("v1", format_activation_at(Utc::now())),
                    ],
                );
                let settings = &self.ui.settings;
                let combine_activation_notification =
                    crate::providers::automatic_activation(provider, settings)
                        && settings.notifications.limits_changed
                        && settings.notifications.activation_success;
                if combine_activation_notification {
                    self.pending_auto_activation_successes.insert(provider);
                } else if settings.notifications.activation_success {
                    notifications::show_activation_succeeded(provider, None);
                }
            }
            WorkerEvent::ActivationFailed(error) => {
                self.pending_auto_activation_successes.remove(&provider);
                crate::platform::logger::info(format!("{name} activation failed: {error}"));
                self.ui.last_activation = crate::i18n::format(
                    "failed-at-error",
                    &[
                        ("v0", provider.qualified_name().to_string()),
                        ("v1", format_activation_at(Utc::now())),
                        ("error", error.to_string()),
                    ],
                );
                if self.ui.settings.notifications.activation_failure {
                    notifications::show_error(&provider.qualified_name(), &self.ui.last_activation);
                }
            }
            WorkerEvent::PollFailed(error) => {
                crate::platform::logger::info(format!("{name} polling failed: {error}"));
                self.ui.set_provider_error(provider, error);
            }
        }
        self.ui_changed = true;
    }

    fn limits_updated(&mut self, provider: ProviderId, limits: RateLimits) {
        crate::platform::logger::info(format!(
            "{} limits received: session used={:?}%, reset={:?}; weekly used={:?}%, reset={:?}",
            provider.display_name(),
            limits.primary.used_percent,
            limits.primary.resets_at,
            limits.secondary.used_percent,
            limits.secondary.resets_at,
        ));
        warn_login_expiry(provider, &limits);
        self.state.observe_reset_limits(provider, &limits);
        let mut limits = limits;
        // The running reader still has the names it started with.
        if let Some(instance) = self
            .ui
            .settings
            .instance(provider)
            .filter(|instance| instance.driver == ProviderKind::OpenRouter)
        {
            crate::providers::openrouter::apply_account_names(&mut limits, instance);
        }
        self.state.replace_limits(provider, limits);
        // The tray repaint in this pass's flush reports its own failure.
        self.ui.error = None;
        self.ui.clear_provider_error(provider);
        let limits = self.state.current_limits();
        let limits = limits.get(provider);
        let combine_activation_notification =
            self.pending_auto_activation_successes.remove(&provider);
        let revision = self.ui.credentials_revision(provider);
        if self
            .notified_revisions
            .insert(provider.id().to_owned(), revision)
            != Some(revision)
        {
            self.limit_notifications.remove(provider.id());
        }
        let settings = &self.ui.settings.notifications;
        let tracker = self
            .limit_notifications
            .entry(provider.id().to_owned())
            .or_default()
            .named(provider.qualified_name());
        if combine_activation_notification {
            let result = tracker.observe_with_primary_reset_deferred(limits, settings, provider);
            if result.primary_reset {
                notifications::show_activation_succeeded_after_reset(provider, limits);
            } else if settings.activation_success {
                notifications::show_activation_succeeded(provider, Some(limits));
            }
        } else {
            tracker.observe(limits, settings, provider);
        }
        if provider == ProviderId::primary(ProviderKind::Codex) {
            self.ui.last_activation = format_last_activation(limits, self.state.last_activation_at);
        }
    }
}

/// Settings opened from the tray menu or a widget keep the popup up as a
/// live preview, like Settings opened from the popup's footer.
fn show_settings_preview() {
    crate::ui::popup_window::ensure_shown(Origin::Cursor);
}

/// The app's own menu commands, from a tray icon or a taskbar widget.
/// Returns whether the app should exit.
fn run_menu_command(command: MenuCommand) -> bool {
    match command {
        MenuCommand::Update => {
            // Failures are reported on the update card.
            let _ = crate::update::apply_pending_update();
        }
        MenuCommand::Settings => {
            show_settings_preview();
            crate::ui::settings_window::open();
        }
        MenuCommand::Exit => return true,
    }
    false
}

/// Runs the single usage-data wipe on a dedicated thread so the bridge never
/// blocks on the store mutex, then reports back through the worker event
/// channel so the bridge can surface errors and resume the workers.
fn spawn_usage_wipe(events: Sender<AppEvent>, generation: u64) {
    let fallback = events.clone();
    let spawned = thread::Builder::new()
        .name("usage-wipe".into())
        .spawn(move || {
            let error = crate::store::with_store(|store| store.clear_usage_data())
                .err()
                .map(|error| format!("{error:#}"));
            let _ = events.send(AppEvent::UsageClearFinished(generation, error));
        });
    if let Err(error) = spawned {
        let _ = fallback.send(AppEvent::UsageClearFinished(
            generation,
            Some(error.to_string()),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(kind: ProviderKind) -> ProviderId {
        ProviderId::primary(kind)
    }

    fn idle_state() -> AppState {
        let (worker_events_tx, worker_events_rx) = std::sync::mpsc::channel();
        let (usage_actions_tx, usage_actions_rx) = std::sync::mpsc::channel();
        AppState {
            settings: Settings::default(),
            limits: Mutex::new(ProviderLimits::default()),
            forced_resets: Mutex::new(Vec::new()),
            provider_statuses: Mutex::new(Vec::new()),
            changes: Default::default(),
            commands: Mutex::new(HashMap::new()),
            workers: Mutex::new(HashMap::new()),
            worker_events_rx: Mutex::new(Some(worker_events_rx)),
            worker_events_tx,
            reset_feed_worker: Mutex::new(None),
            provider_status_worker: Mutex::new(None),
            activation_path: std::path::PathBuf::new(),
            startup_provider_errors: Vec::new(),
            last_activation_at: None,
            settings_rx: Mutex::new(None),
            usage_actions_rx: Mutex::new(Some(usage_actions_rx)),
            usage_actions_tx,
            updates: crate::update::UpdateController::new(),
        }
    }

    #[test]
    fn changes_marked_within_a_tick_fan_out_once() {
        let state = idle_state();
        let mut ui = UiState::initial(&state);

        // A settings commit resets a provider and then marks the widgets.
        state.mark(Changes::LIMITS);
        state.mark(Changes::LIMITS);
        assert_eq!(
            FanOut::plan(state.take_changes(), &mut ui, false),
            FanOut {
                sync_limits: true,
                publish: true,
            }
        );
        assert_eq!(ui.usage_revision, 0);
        assert_eq!(
            FanOut::plan(state.take_changes(), &mut ui, false),
            FanOut {
                sync_limits: false,
                publish: false,
            }
        );

        state.replace_usage(id(ProviderKind::Codex), Default::default());
        assert_eq!(
            FanOut::plan(state.take_changes(), &mut ui, false),
            FanOut {
                sync_limits: false,
                publish: true,
            }
        );
        assert_eq!(ui.usage_revision, 1);

        assert_eq!(
            FanOut::plan(state.take_changes(), &mut ui, true),
            FanOut {
                sync_limits: false,
                publish: true,
            }
        );
        assert_eq!(ui.usage_revision, 1);
    }

    #[test]
    fn credential_worker_events_are_accepted_only_for_the_current_revision() {
        let mut settings = Settings::default();
        for instance in &mut settings.instances {
            instance.enabled = true;
        }
        settings
            .instance_mut(id(ProviderKind::OpenRouter))
            .unwrap()
            .credentials_revision = 7;
        settings
            .instance_mut(id(ProviderKind::Claude))
            .unwrap()
            .credentials_revision = 3;
        let work = settings.add_instance(ProviderInstance::new(ProviderKind::Claude, "Work"));
        let ui = UiState::new(settings.clone());

        let current =
            |provider, revision| provider_worker_event_is_current(&ui, provider, revision);
        assert!(current(id(ProviderKind::OpenRouter), 7));
        assert!(!current(id(ProviderKind::OpenRouter), 6));
        assert!(current(id(ProviderKind::Claude), 3));
        assert!(!current(id(ProviderKind::Claude), 2));
        // Another instance of the same driver has its own revision.
        assert!(current(work, 0));
        assert!(!current(work, 3));
        assert!(current(id(ProviderKind::OpenCodeZen), 0));
        // Disabled and removed instances drop queued output.
        let mut disabled = settings.clone();
        disabled.set_enabled(id(ProviderKind::Codex), false);
        let ui = UiState::new(disabled);
        assert!(!provider_worker_event_is_current(
            &ui,
            id(ProviderKind::Codex),
            0
        ));
        assert!(!provider_worker_event_is_current(
            &ui,
            ProviderId::new(ProviderKind::Claude, "claude-removed"),
            0
        ));
    }

    #[test]
    fn nested_usage_errors_are_promoted_to_provider_error_state() {
        let mut statistics = crate::usage::UsageStatistics::default();
        statistics.accounts.insert(
            "account".into(),
            crate::usage::UsageStatistics {
                error: Some("analytics request failed".into()),
                ..Default::default()
            },
        );

        assert_eq!(
            usage_error_message(&statistics).as_deref(),
            Some("analytics request failed")
        );
    }

    #[test]
    fn only_read_relevant_changes_restart_an_instance_worker() {
        let before = crate::ui::popup_window::tests::instance_fixture().instances;
        let mut after = before.clone();
        for instance in after.iter_mut().chain(&mut before.clone()) {
            instance.enabled = true;
        }
        let before = after.clone();
        let work = after
            .iter_mut()
            .find(|instance| instance.id == "claude-work")
            .unwrap();
        work.name = "Renamed".into();
        work.badge = "RN".into();
        work.show_on_home = false;
        assert!(instances_needing_restart(&before, &after).is_empty());
        let work = after
            .iter_mut()
            .find(|instance| instance.id == "claude-work")
            .unwrap();
        work.binary_path = Some("C:/tools".into());
        assert_eq!(
            instances_needing_restart(&before, &after),
            vec![ProviderId::new(ProviderKind::Claude, "claude-work")]
        );
        let codex = after
            .iter_mut()
            .find(|instance| instance.id == "codex")
            .unwrap();
        codex.credentials_revision += 1;
        assert_eq!(instances_needing_restart(&before, &after).len(), 2);
    }
}
