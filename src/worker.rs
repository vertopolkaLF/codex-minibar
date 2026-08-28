use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::Result;
use chrono::Utc;

use crate::{
    limits::RateLimits,
    scheduler::{self, ActivationState, Decision},
    settings::ScheduledActivation,
    usage::UsageStatistics,
};

pub const USAGE_STATS_INTERVAL: Duration = Duration::from_secs(10 * 60);

pub trait LimitProvider: Send + 'static {
    fn read_limits(&mut self) -> Result<RateLimits>;
}

pub trait UsageProvider: Send + 'static {
    fn load_cached_usage_statistics(&mut self, history_days: u16) -> Result<UsageStatistics>;
    fn refresh_usage_statistics(&mut self, history_days: u16) -> Result<UsageStatistics>;

    /// Some providers have useful local history even when their remote quota
    /// endpoint is unavailable. The default keeps the existing lazy behavior
    /// for Codex, Claude, and Cursor.
    fn refresh_without_limits(&self) -> bool {
        false
    }
}

pub trait Activator: Send + 'static {
    fn activate(&mut self) -> Result<()>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkerCommand {
    Refresh,
    SetLimitRefreshInterval(Duration),
    SetAutomaticActivation(bool),
    SetScheduledActivations(Vec<ScheduledActivation>),
    SetHistoryRetentionDays(u16),
    Shutdown,
}

#[derive(Clone, Debug)]
pub enum WorkerEvent {
    LimitsUpdated(RateLimits),
    UsageUpdated(UsageStatistics),
    ActivationStarted,
    ActivationSucceeded,
    ActivationFailed(String),
    PollFailed(String),
    Stopped,
    /// A provider-scoped event emitted by the multi-provider coordinator.
    ProviderLimitsUpdated(crate::settings::ProviderKind, RateLimits),
    ProviderUsageUpdated(crate::settings::ProviderKind, UsageStatistics),
    ProviderActivationStarted(crate::settings::ProviderKind),
    ProviderActivationSucceeded(crate::settings::ProviderKind),
    ProviderActivationFailed(crate::settings::ProviderKind, String),
    ProviderPollFailed(crate::settings::ProviderKind, String),
}

pub struct WorkerHandle {
    pub commands: Sender<WorkerCommand>,
    events: Option<Receiver<WorkerEvent>>,
    join: Option<JoinHandle<()>>,
}

impl WorkerHandle {
    pub fn refresh(&self) {
        let _ = self.commands.send(WorkerCommand::Refresh);
    }

    pub fn shutdown(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        let _ = self.commands.send(WorkerCommand::Shutdown);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }

    /// Hands the UI bridge the sole event receiver while this handle remains
    /// the owner responsible for joining both background tasks during shutdown.
    pub fn take_events(&mut self) -> Option<Receiver<WorkerEvent>> {
        self.events.take()
    }
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Starts independent background tasks. Rate limits remain on the fast polling
/// path; scanning local session logs starts only after the first successful
/// limit response and then runs at the much slower stats interval.
pub fn start_worker(
    provider: impl LimitProvider,
    usage_provider: impl UsageProvider,
    activator: impl Activator,
    state_path: PathBuf,
    automatic_activation: bool,
    scheduled_activations: Vec<ScheduledActivation>,
    history_retention_days: u16,
    poll_interval: Duration,
) -> WorkerHandle {
    let (command_sender, command_receiver) = mpsc::channel();
    let (event_sender, event_receiver) = mpsc::channel();
    let mut handle = start_worker_with_channels(
        provider,
        usage_provider,
        activator,
        state_path,
        automatic_activation,
        scheduled_activations,
        history_retention_days,
        poll_interval,
        command_sender,
        command_receiver,
        event_sender,
        true,
    );
    handle.events = Some(event_receiver);
    handle
}

/// Starts a worker which publishes to an existing event stream. This lets the
/// selected provider change without recreating the UI's event bridge.
#[allow(clippy::too_many_arguments)]
pub fn start_worker_with_event_sender(
    provider: impl LimitProvider,
    usage_provider: impl UsageProvider,
    activator: impl Activator,
    state_path: PathBuf,
    automatic_activation: bool,
    scheduled_activations: Vec<ScheduledActivation>,
    history_retention_days: u16,
    poll_interval: Duration,
    event_sender: Sender<WorkerEvent>,
) -> WorkerHandle {
    let (command_sender, command_receiver) = mpsc::channel();
    start_worker_with_channels(
        provider,
        usage_provider,
        activator,
        state_path,
        automatic_activation,
        scheduled_activations,
        history_retention_days,
        poll_interval,
        command_sender,
        command_receiver,
        event_sender,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn start_worker_with_channels(
    provider: impl LimitProvider,
    usage_provider: impl UsageProvider,
    activator: impl Activator,
    state_path: PathBuf,
    automatic_activation: bool,
    scheduled_activations: Vec<ScheduledActivation>,
    history_retention_days: u16,
    poll_interval: Duration,
    command_sender: Sender<WorkerCommand>,
    command_receiver: Receiver<WorkerCommand>,
    event_sender: Sender<WorkerEvent>,
    publish_stopped: bool,
) -> WorkerHandle {
    let (limit_commands, limit_commands_rx) = mpsc::channel();
    let (usage_commands, usage_commands_rx) = mpsc::channel();
    let limits_ready = Arc::new(AtomicBool::new(usage_provider.refresh_without_limits()));

    let limit_join = {
        let event_sender = event_sender.clone();
        let limits_ready = Arc::clone(&limits_ready);
        thread::spawn(move || {
            run_limit_task(
                provider,
                activator,
                state_path,
                automatic_activation,
                scheduled_activations,
                poll_interval,
                limit_commands_rx,
                event_sender,
                limits_ready,
            )
        })
    };
    let usage_join = {
        let event_sender = event_sender.clone();
        thread::spawn(move || {
            enter_background_processing_mode();
            run_usage_task(
                usage_provider,
                history_retention_days,
                usage_commands_rx,
                event_sender,
                limits_ready,
            )
        })
    };

    let join = thread::spawn(move || {
        while let Ok(command) = command_receiver.recv() {
            match command {
                WorkerCommand::Shutdown => {
                    let _ = limit_commands.send(WorkerCommand::Shutdown);
                    let _ = usage_commands.send(WorkerCommand::Shutdown);
                    break;
                }
                WorkerCommand::Refresh => {
                    let _ = limit_commands.send(WorkerCommand::Refresh);
                    let _ = usage_commands.send(WorkerCommand::Refresh);
                }
                WorkerCommand::SetLimitRefreshInterval(interval) => {
                    let _ = limit_commands.send(WorkerCommand::SetLimitRefreshInterval(interval));
                }
                WorkerCommand::SetAutomaticActivation(enabled) => {
                    let _ = limit_commands.send(WorkerCommand::SetAutomaticActivation(enabled));
                }
                WorkerCommand::SetScheduledActivations(schedules) => {
                    let _ = limit_commands.send(WorkerCommand::SetScheduledActivations(schedules));
                }
                WorkerCommand::SetHistoryRetentionDays(days) => {
                    let _ = usage_commands.send(WorkerCommand::SetHistoryRetentionDays(days));
                }
            }
        }
        let _ = limit_commands.send(WorkerCommand::Shutdown);
        let _ = usage_commands.send(WorkerCommand::Shutdown);
        let _ = limit_join.join();
        let _ = usage_join.join();
        if publish_stopped {
            let _ = event_sender.send(WorkerEvent::Stopped);
        }
    });

    WorkerHandle {
        commands: command_sender,
        events: None,
        join: Some(join),
    }
}

/// Usage scans are maintenance work. On Windows, background mode lowers CPU,
/// memory, and disk-I/O priority so a cold multi-gigabyte history rebuild does
/// not compete with input handling or the foreground application.
fn enter_background_processing_mode() {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN,
        };

        let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN);
    }
}

#[allow(clippy::too_many_arguments)]
fn run_limit_task(
    mut provider: impl LimitProvider,
    mut activator: impl Activator,
    state_path: PathBuf,
    mut automatic_activation: bool,
    mut scheduled_activations: Vec<ScheduledActivation>,
    mut poll_interval: Duration,
    commands: Receiver<WorkerCommand>,
    events: Sender<WorkerEvent>,
    limits_ready: Arc<AtomicBool>,
) {
    let mut state = ActivationState::load_or_default(&state_path).unwrap_or_default();
    // This worker belongs to one provider, so its deadline and any retry stay
    // provider-local. A failing provider cannot wake another provider's loop.
    let mut next_poll = Instant::now();
    loop {
        if next_poll <= Instant::now() {
            // Schedule from the end of each request. A manual refresh replaces
            // the previous deadline instead of leaving a stale timer behind.
            match tick(
                &mut provider,
                &mut activator,
                &mut state,
                automatic_activation,
                &scheduled_activations,
            ) {
                Ok(worker_events) => {
                    let _ = state.save(&state_path);
                    for event in worker_events {
                        let is_limits_update = matches!(event, WorkerEvent::LimitsUpdated(_));
                        let _ = events.send(event);
                        // The usage task may start only after the first quota event
                        // has been placed in the UI queue.
                        if is_limits_update {
                            limits_ready.store(true, Ordering::Release);
                        }
                    }
                }
                Err(error) => {
                    let _ = events.send(WorkerEvent::PollFailed(error.to_string()));
                }
            }
            next_poll = Instant::now() + poll_interval;
            continue;
        }

        match commands.recv_timeout(next_poll.saturating_duration_since(Instant::now())) {
            Ok(WorkerCommand::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            Ok(WorkerCommand::SetAutomaticActivation(enabled)) => {
                automatic_activation = enabled;
            }
            Ok(WorkerCommand::SetScheduledActivations(schedules)) => {
                if scheduled_activations == schedules {
                    continue;
                }
                scheduled_activations = schedules;
                next_poll = Instant::now();
            }
            Ok(WorkerCommand::SetLimitRefreshInterval(interval)) => {
                poll_interval = interval;
                // Apply the setting immediately without an extra request.
                next_poll = Instant::now() + poll_interval;
            }
            Ok(WorkerCommand::Refresh) | Err(RecvTimeoutError::Timeout) => {
                next_poll = Instant::now();
            }
            Ok(WorkerCommand::SetHistoryRetentionDays(_)) => {}
        }
    }
}

fn run_usage_task(
    mut provider: impl UsageProvider,
    mut history_retention_days: u16,
    commands: Receiver<WorkerCommand>,
    events: Sender<WorkerEvent>,
    limits_ready: Arc<AtomicBool>,
) {
    // Cached aggregates are a fast local read and make the popup useful before
    // a provider's first quota request completes. Keep the potentially
    // expensive refresh scan behind `limits_ready` so it does not compete with
    // that first network request.
    if let Ok(usage) = provider.load_cached_usage_statistics(history_retention_days) {
        let _ = events.send(WorkerEvent::UsageUpdated(usage));
    }
    while !limits_ready.load(Ordering::Acquire) {
        match commands.recv_timeout(Duration::from_millis(100)) {
            Ok(WorkerCommand::Shutdown) | Err(RecvTimeoutError::Disconnected) => return,
            Ok(WorkerCommand::SetHistoryRetentionDays(days)) => {
                let days = days.clamp(1, 365);
                if days != history_retention_days {
                    history_retention_days = days;
                    if let Ok(usage) = provider.load_cached_usage_statistics(history_retention_days)
                    {
                        let _ = events.send(WorkerEvent::UsageUpdated(usage));
                    }
                }
            }
            Ok(WorkerCommand::Refresh)
            | Ok(WorkerCommand::SetLimitRefreshInterval(_))
            | Ok(WorkerCommand::SetAutomaticActivation(_))
            | Ok(WorkerCommand::SetScheduledActivations(_)) => {}
            Err(RecvTimeoutError::Timeout) => {}
        }
    }

    // Preserve this deadline while processing commands that belong to the
    // limit task. Otherwise every settings update wakes this task and turns a
    // ten-minute maintenance scan into a tight loop.
    let mut next_refresh = Instant::now();
    loop {
        if next_refresh <= Instant::now() {
            if let Ok(usage) = provider.refresh_usage_statistics(history_retention_days) {
                let _ = events.send(WorkerEvent::UsageUpdated(usage));
            }
            next_refresh = Instant::now() + USAGE_STATS_INTERVAL;
            continue;
        }

        match commands.recv_timeout(next_refresh.saturating_duration_since(Instant::now())) {
            Ok(WorkerCommand::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            Ok(WorkerCommand::SetHistoryRetentionDays(days)) => {
                let days = days.clamp(1, 365);
                if days != history_retention_days {
                    history_retention_days = days;
                    if let Ok(usage) = provider.load_cached_usage_statistics(history_retention_days)
                    {
                        let _ = events.send(WorkerEvent::UsageUpdated(usage));
                    }
                    next_refresh = Instant::now();
                }
            }
            Ok(WorkerCommand::Refresh) | Err(RecvTimeoutError::Timeout) => {
                next_refresh = Instant::now();
            }
            Ok(WorkerCommand::SetLimitRefreshInterval(_))
            | Ok(WorkerCommand::SetAutomaticActivation(_))
            | Ok(WorkerCommand::SetScheduledActivations(_)) => {}
        }
    }
}

fn tick(
    provider: &mut impl LimitProvider,
    activator: &mut impl Activator,
    state: &mut ActivationState,
    automatic_activation: bool,
    scheduled_activations: &[ScheduledActivation],
) -> Result<Vec<WorkerEvent>> {
    let mut limits = provider.read_limits()?;
    let mut events = Vec::new();

    let now = Utc::now();
    // The UI hides an absent 5h window via this same predicate. Never turn
    // "this account has no session window" into a request to activate one.
    let session_window_available = !limits.five_hour_disabled();
    let scheduled_due = session_window_available
        .then(|| scheduler::due_scheduled_activation(scheduled_activations, state, now))
        .flatten();
    let automatic_due = session_window_available
        && automatic_activation
        && !scheduler::scheduled_activation_within(
            scheduled_activations,
            now,
            scheduler::AUTO_ACTIVATION_SCHEDULE_GUARD,
        )
        && state.decide_with_unactivated(&limits.primary, limits.primary_window_is_unactivated)
            == Decision::ActivateNow;
    if scheduled_due.is_some() || automatic_due {
        state.record_attempt(Utc::now());
        events.push(WorkerEvent::ActivationStarted);
        match activator.activate() {
            Ok(()) => {
                if let Ok(fresh) = provider.read_limits() {
                    limits = fresh;
                }
                state.observe_with_unactivated(
                    &limits.primary,
                    limits.primary_window_is_unactivated,
                );
                if let Some((rule, occurrence)) = scheduled_due {
                    state.record_scheduled_activation(&rule.id, occurrence);
                }
                events.push(WorkerEvent::ActivationSucceeded);
            }
            Err(error) => events.push(WorkerEvent::ActivationFailed(error.to_string())),
        }
    } else {
        state.observe_with_unactivated(&limits.primary, limits.primary_window_is_unactivated);
    }

    events.insert(0, WorkerEvent::LimitsUpdated(limits));
    Ok(events)
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;
    use chrono::{Datelike, Duration as ChronoDuration, Local, TimeZone, Timelike};

    use super::*;
    use crate::limits::LimitWindow;

    struct ScriptedProvider {
        samples: Vec<RateLimits>,
        index: usize,
    }

    impl ScriptedProvider {
        fn new(samples: Vec<RateLimits>) -> Self {
            Self { samples, index: 0 }
        }
    }

    impl LimitProvider for ScriptedProvider {
        fn read_limits(&mut self) -> Result<RateLimits> {
            let sample = self.samples[self.index.min(self.samples.len() - 1)].clone();
            self.index += 1;
            Ok(sample)
        }
    }

    struct CountingActivator(usize);
    impl Activator for CountingActivator {
        fn activate(&mut self) -> Result<()> {
            self.0 += 1;
            Ok(())
        }
    }

    struct CountingUsageProvider {
        refreshes: Arc<std::sync::atomic::AtomicUsize>,
        refresh_without_limits: bool,
    }

    impl UsageProvider for CountingUsageProvider {
        fn load_cached_usage_statistics(&mut self, _history_days: u16) -> Result<UsageStatistics> {
            Ok(UsageStatistics::default())
        }

        fn refresh_usage_statistics(&mut self, _history_days: u16) -> Result<UsageStatistics> {
            let requests = self.refreshes.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(UsageStatistics {
                history: crate::usage::TokenUsage {
                    requests: requests as u64,
                    ..Default::default()
                },
                ..Default::default()
            })
        }

        fn refresh_without_limits(&self) -> bool {
            self.refresh_without_limits
        }
    }

    fn limits_at(hour: u32, minute: u32) -> RateLimits {
        RateLimits {
            primary: LimitWindow {
                used_percent: Some(0),
                resets_at: Some(Utc.with_ymd_and_hms(2026, 7, 10, hour, minute, 0).unwrap()),
                duration_minutes: Some(300),
            },
            secondary: LimitWindow::default(),
            sampled_at: Utc::now(),
            ..RateLimits::default()
        }
    }

    #[test]
    fn tick_baselines_then_activates_only_when_reset_changes() {
        let mut provider = ScriptedProvider::new(vec![
            limits_at(15, 0),
            limits_at(15, 0),
            limits_at(20, 1),
            limits_at(20, 1),
            limits_at(20, 1),
        ]);
        let mut activator = CountingActivator(0);
        let mut state = ActivationState::default();

        tick(&mut provider, &mut activator, &mut state, true, &[]).unwrap();
        assert_eq!(activator.0, 0);
        tick(&mut provider, &mut activator, &mut state, true, &[]).unwrap();
        assert_eq!(activator.0, 0);
        tick(&mut provider, &mut activator, &mut state, true, &[]).unwrap();
        assert_eq!(activator.0, 1);
        assert_eq!(
            state.last_seen_resets_at,
            limits_at(20, 1).primary.resets_at
        );
        tick(&mut provider, &mut activator, &mut state, true, &[]).unwrap();
        assert_eq!(activator.0, 1);
    }

    #[test]
    fn tick_activates_codex_unactivated_window_once() {
        let sampled_at = Utc.with_ymd_and_hms(2026, 7, 10, 15, 0, 0).unwrap();
        let unactivated = RateLimits {
            primary: LimitWindow {
                used_percent: Some(0),
                resets_at: Some(sampled_at + ChronoDuration::hours(5)),
                duration_minutes: Some(300),
            },
            secondary: LimitWindow {
                used_percent: Some(0),
                resets_at: Some(sampled_at + ChronoDuration::days(7)),
                duration_minutes: Some(10_080),
            },
            sampled_at,
            primary_window_is_unactivated: true,
            ..RateLimits::default()
        };
        let mut provider = ScriptedProvider::new(vec![unactivated.clone(), unactivated]);
        let mut activator = CountingActivator(0);
        let mut state = ActivationState::default();

        tick(&mut provider, &mut activator, &mut state, true, &[]).unwrap();
        assert_eq!(activator.0, 1);
        tick(&mut provider, &mut activator, &mut state, true, &[]).unwrap();
        assert_eq!(activator.0, 1);
    }

    #[test]
    fn tick_ignores_unmarked_five_hour_placeholder_shape() {
        let sampled_at = Utc.with_ymd_and_hms(2026, 7, 10, 15, 0, 0).unwrap();
        let sample = RateLimits {
            primary: LimitWindow {
                used_percent: Some(0),
                resets_at: Some(sampled_at + ChronoDuration::hours(5)),
                duration_minutes: Some(300),
            },
            sampled_at,
            ..RateLimits::default()
        };
        let mut activator = CountingActivator(0);
        let mut state = ActivationState::default();

        tick(
            &mut ScriptedProvider::new(vec![sample]),
            &mut activator,
            &mut state,
            true,
            &[],
        )
        .unwrap();

        assert_eq!(activator.0, 0);
    }

    #[test]
    fn delayed_reset_deadline_does_not_repeat_successful_activation() {
        let without_deadline = RateLimits {
            primary: LimitWindow {
                used_percent: Some(0),
                resets_at: None,
                duration_minutes: Some(300),
            },
            sampled_at: Utc::now(),
            ..RateLimits::default()
        };
        let mut provider = ScriptedProvider::new(vec![
            limits_at(15, 0),
            without_deadline.clone(),
            without_deadline,
            limits_at(20, 0),
        ]);
        let mut activator = CountingActivator(0);
        let mut state = ActivationState::default();

        tick(&mut provider, &mut activator, &mut state, true, &[]).unwrap();
        tick(&mut provider, &mut activator, &mut state, true, &[]).unwrap();
        assert_eq!(activator.0, 1);

        tick(&mut provider, &mut activator, &mut state, true, &[]).unwrap();
        assert_eq!(activator.0, 1);
        assert_eq!(
            state.last_seen_resets_at,
            limits_at(20, 0).primary.resets_at
        );
    }

    #[test]
    fn absent_session_window_blocks_automatic_and_scheduled_activation() {
        let local_now = Local::now();
        let schedule = ScheduledActivation {
            id: "due-now".into(),
            provider_id: crate::settings::ProviderKind::Codex.id().into(),
            weekday: local_now.weekday().num_days_from_monday() as u8,
            weekdays: vec![local_now.weekday().num_days_from_monday() as u8],
            time_minutes: (local_now.hour() * 60 + local_now.minute()) as u16,
            enabled: true,
        };
        let mut activator = CountingActivator(0);
        let mut state = ActivationState::default();
        let events = tick(
            &mut ScriptedProvider::new(vec![RateLimits::default()]),
            &mut activator,
            &mut state,
            true,
            &[schedule],
        )
        .unwrap();

        assert_eq!(activator.0, 0);
        assert!(!events.iter().any(|event| matches!(
            event,
            WorkerEvent::ActivationStarted
                | WorkerEvent::ActivationSucceeded
                | WorkerEvent::ActivationFailed(_)
        )));
    }

    struct FailingActivator;
    impl Activator for FailingActivator {
        fn activate(&mut self) -> Result<()> {
            Err(anyhow!("nope"))
        }
    }

    #[test]
    fn activation_failure_becomes_an_event() {
        let mut state = ActivationState {
            last_seen_resets_at: limits_at(15, 0).primary.resets_at,
            ..ActivationState::default()
        };
        let events = tick(
            &mut ScriptedProvider::new(vec![limits_at(15, 5)]),
            &mut FailingActivator,
            &mut state,
            true,
            &[],
        )
        .unwrap();
        assert!(matches!(
            events
                .iter()
                .find(|event| matches!(event, WorkerEvent::ActivationFailed(_))),
            Some(WorkerEvent::ActivationFailed(_))
        ));
        assert_eq!(
            state.last_seen_resets_at,
            limits_at(15, 0).primary.resets_at
        );
    }

    #[test]
    fn usage_statistics_interval_is_ten_minutes() {
        assert_eq!(USAGE_STATS_INTERVAL, Duration::from_secs(600));
    }

    #[test]
    fn manual_refresh_immediately_scans_for_missing_usage() {
        let (commands_tx, commands_rx) = mpsc::channel();
        let (events_tx, events_rx) = mpsc::channel();
        let limits_ready = Arc::new(AtomicBool::new(true));
        let refreshes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let provider = CountingUsageProvider {
            refreshes: Arc::clone(&refreshes),
            refresh_without_limits: false,
        };
        let task = thread::spawn(move || {
            run_usage_task(provider, 30, commands_rx, events_tx, limits_ready);
        });

        // Cached snapshot, then the initial local-log scan.
        assert!(matches!(
            events_rx.recv_timeout(Duration::from_secs(1)),
            Ok(WorkerEvent::UsageUpdated(_))
        ));
        assert!(matches!(
            events_rx.recv_timeout(Duration::from_secs(1)),
            Ok(WorkerEvent::UsageUpdated(_))
        ));
        assert_eq!(refreshes.load(Ordering::SeqCst), 1);

        commands_tx.send(WorkerCommand::Refresh).unwrap();
        assert!(matches!(
            events_rx.recv_timeout(Duration::from_secs(1)),
            Ok(WorkerEvent::UsageUpdated(_))
        ));
        assert_eq!(refreshes.load(Ordering::SeqCst), 2);

        commands_tx.send(WorkerCommand::Shutdown).unwrap();
        task.join().unwrap();
    }

    #[test]
    fn providers_with_local_usage_can_scan_before_remote_limits() {
        let (commands_tx, commands_rx) = mpsc::channel();
        let (events_tx, events_rx) = mpsc::channel();
        let refreshes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let provider = CountingUsageProvider {
            refreshes: Arc::clone(&refreshes),
            refresh_without_limits: true,
        };
        let limits_ready = Arc::new(AtomicBool::new(provider.refresh_without_limits()));
        let task = thread::spawn(move || {
            run_usage_task(provider, 30, commands_rx, events_tx, limits_ready);
        });

        assert!(matches!(
            events_rx.recv_timeout(Duration::from_secs(1)),
            Ok(WorkerEvent::UsageUpdated(_))
        ));
        assert!(matches!(
            events_rx.recv_timeout(Duration::from_secs(1)),
            Ok(WorkerEvent::UsageUpdated(_))
        ));
        assert_eq!(refreshes.load(Ordering::SeqCst), 1);

        commands_tx.send(WorkerCommand::Shutdown).unwrap();
        task.join().unwrap();
    }

    #[test]
    fn unrelated_commands_do_not_restart_usage_scan_or_republish_cache() {
        let (commands_tx, commands_rx) = mpsc::channel();
        let (events_tx, events_rx) = mpsc::channel();
        let limits_ready = Arc::new(AtomicBool::new(true));
        let refreshes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let provider = CountingUsageProvider {
            refreshes: Arc::clone(&refreshes),
            refresh_without_limits: false,
        };
        let task = thread::spawn(move || {
            run_usage_task(provider, 30, commands_rx, events_tx, limits_ready);
        });

        assert!(matches!(
            events_rx.recv_timeout(Duration::from_secs(1)),
            Ok(WorkerEvent::UsageUpdated(_))
        ));
        assert!(matches!(
            events_rx.recv_timeout(Duration::from_secs(1)),
            Ok(WorkerEvent::UsageUpdated(_))
        ));
        assert_eq!(refreshes.load(Ordering::SeqCst), 1);

        commands_tx
            .send(WorkerCommand::SetAutomaticActivation(true))
            .unwrap();
        commands_tx
            .send(WorkerCommand::SetScheduledActivations(Vec::new()))
            .unwrap();
        commands_tx
            .send(WorkerCommand::SetLimitRefreshInterval(Duration::from_secs(
                60,
            )))
            .unwrap();
        commands_tx
            .send(WorkerCommand::SetHistoryRetentionDays(30))
            .unwrap();

        assert!(matches!(
            events_rx.recv_timeout(Duration::from_millis(200)),
            Err(RecvTimeoutError::Timeout)
        ));
        assert_eq!(refreshes.load(Ordering::SeqCst), 1);

        commands_tx.send(WorkerCommand::Shutdown).unwrap();
        task.join().unwrap();
    }

    #[test]
    fn changed_history_range_reloads_cache_and_scans_once() {
        let (commands_tx, commands_rx) = mpsc::channel();
        let (events_tx, events_rx) = mpsc::channel();
        let limits_ready = Arc::new(AtomicBool::new(true));
        let refreshes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let provider = CountingUsageProvider {
            refreshes: Arc::clone(&refreshes),
            refresh_without_limits: false,
        };
        let task = thread::spawn(move || {
            run_usage_task(provider, 30, commands_rx, events_tx, limits_ready);
        });

        assert!(events_rx.recv_timeout(Duration::from_secs(1)).is_ok());
        assert!(events_rx.recv_timeout(Duration::from_secs(1)).is_ok());
        assert_eq!(refreshes.load(Ordering::SeqCst), 1);

        commands_tx
            .send(WorkerCommand::SetHistoryRetentionDays(90))
            .unwrap();
        assert!(events_rx.recv_timeout(Duration::from_secs(1)).is_ok());
        assert!(events_rx.recv_timeout(Duration::from_secs(1)).is_ok());
        assert_eq!(refreshes.load(Ordering::SeqCst), 2);
        assert!(matches!(
            events_rx.recv_timeout(Duration::from_millis(200)),
            Err(RecvTimeoutError::Timeout)
        ));

        commands_tx.send(WorkerCommand::Shutdown).unwrap();
        task.join().unwrap();
    }

    #[test]
    fn cached_usage_is_published_before_first_limits_update() {
        let (commands_tx, commands_rx) = mpsc::channel();
        let (events_tx, events_rx) = mpsc::channel();
        let limits_ready = Arc::new(AtomicBool::new(false));
        let refreshes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let provider = CountingUsageProvider {
            refreshes: Arc::clone(&refreshes),
            refresh_without_limits: false,
        };
        let task_limits_ready = Arc::clone(&limits_ready);
        let task = thread::spawn(move || {
            run_usage_task(provider, 30, commands_rx, events_tx, task_limits_ready);
        });

        // The cache arrives even while the first rate-limit request is still
        // pending; no filesystem refresh has happened yet.
        assert!(matches!(
            events_rx.recv_timeout(Duration::from_secs(1)),
            Ok(WorkerEvent::UsageUpdated(_))
        ));
        assert_eq!(refreshes.load(Ordering::SeqCst), 0);

        commands_tx
            .send(WorkerCommand::SetHistoryRetentionDays(30))
            .unwrap();
        commands_tx
            .send(WorkerCommand::SetAutomaticActivation(true))
            .unwrap();
        assert!(matches!(
            events_rx.recv_timeout(Duration::from_millis(200)),
            Err(RecvTimeoutError::Timeout)
        ));
        assert_eq!(refreshes.load(Ordering::SeqCst), 0);

        limits_ready.store(true, Ordering::Release);
        assert!(matches!(
            events_rx.recv_timeout(Duration::from_secs(1)),
            Ok(WorkerEvent::UsageUpdated(_))
        ));
        assert_eq!(refreshes.load(Ordering::SeqCst), 1);

        commands_tx.send(WorkerCommand::Shutdown).unwrap();
        task.join().unwrap();
    }
}
