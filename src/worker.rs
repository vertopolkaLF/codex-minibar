use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::Result;
use chrono::Utc;

use crate::{
    limits::RateLimits,
    scheduler::{ActivationState, Decision},
    usage::UsageStatistics,
};

pub const USAGE_STATS_INTERVAL: Duration = Duration::from_secs(10 * 60);

pub trait LimitProvider: Send + 'static {
    fn read_limits(&mut self) -> Result<RateLimits>;
}

pub trait UsageProvider: Send + 'static {
    fn load_cached_usage_statistics(&mut self, history_days: u16) -> Result<UsageStatistics>;
    fn refresh_usage_statistics(&mut self, history_days: u16) -> Result<UsageStatistics>;
}

pub trait Activator: Send + 'static {
    fn activate(&mut self) -> Result<()>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerCommand {
    Refresh,
    SetAutomaticActivation(bool),
    SetHistoryRetentionDays(u16),
    Shutdown,
}

#[derive(Clone, Debug)]
pub enum WorkerEvent {
    LimitsUpdated(RateLimits),
    UsageUpdated(UsageStatistics),
    ActivationSucceeded,
    ActivationFailed(String),
    PollFailed(String),
    Stopped,
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
    history_retention_days: u16,
    poll_interval: Duration,
) -> WorkerHandle {
    let (command_sender, command_receiver) = mpsc::channel();
    let (limit_commands, limit_commands_rx) = mpsc::channel();
    let (usage_commands, usage_commands_rx) = mpsc::channel();
    let (event_sender, event_receiver) = mpsc::channel();
    let limits_ready = Arc::new(AtomicBool::new(false));

    let limit_join = {
        let event_sender = event_sender.clone();
        let limits_ready = Arc::clone(&limits_ready);
        thread::spawn(move || {
            run_limit_task(
                provider,
                activator,
                state_path,
                automatic_activation,
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
                WorkerCommand::SetAutomaticActivation(enabled) => {
                    let _ = limit_commands.send(WorkerCommand::SetAutomaticActivation(enabled));
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
        let _ = event_sender.send(WorkerEvent::Stopped);
    });

    WorkerHandle {
        commands: command_sender,
        events: Some(event_receiver),
        join: Some(join),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_limit_task(
    mut provider: impl LimitProvider,
    mut activator: impl Activator,
    state_path: PathBuf,
    mut automatic_activation: bool,
    poll_interval: Duration,
    commands: Receiver<WorkerCommand>,
    events: Sender<WorkerEvent>,
    limits_ready: Arc<AtomicBool>,
) {
    let mut state = ActivationState::load_or_default(&state_path).unwrap_or_default();
    loop {
        match tick(&mut provider, &mut activator, &mut state, automatic_activation) {
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
        match commands.recv_timeout(poll_interval) {
            Ok(WorkerCommand::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            Ok(WorkerCommand::SetAutomaticActivation(enabled)) => {
                automatic_activation = enabled;
            }
            Ok(WorkerCommand::Refresh)
            | Ok(WorkerCommand::SetHistoryRetentionDays(_))
            | Err(RecvTimeoutError::Timeout) => {}
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
    // Do not compete with the first rate-limit request. The popup receives its
    // actionable quota snapshot before any potentially expensive disk scan.
    while !limits_ready.load(Ordering::Acquire) {
        match commands.recv_timeout(Duration::from_millis(100)) {
            Ok(WorkerCommand::Shutdown) | Err(RecvTimeoutError::Disconnected) => return,
            Ok(WorkerCommand::SetHistoryRetentionDays(days)) => {
                history_retention_days = days.clamp(1, 365);
            }
            Ok(WorkerCommand::Refresh) | Ok(WorkerCommand::SetAutomaticActivation(_)) => {}
            Err(RecvTimeoutError::Timeout) => {}
        }
    }

    if let Ok(usage) = provider.load_cached_usage_statistics(history_retention_days) {
        let _ = events.send(WorkerEvent::UsageUpdated(usage));
    }
    loop {
        if let Ok(usage) = provider.refresh_usage_statistics(history_retention_days) {
            let _ = events.send(WorkerEvent::UsageUpdated(usage));
        }
        match commands.recv_timeout(USAGE_STATS_INTERVAL) {
            Ok(WorkerCommand::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            Ok(WorkerCommand::SetHistoryRetentionDays(days)) => {
                history_retention_days = days.clamp(1, 365);
                if let Ok(usage) = provider.load_cached_usage_statistics(history_retention_days) {
                    let _ = events.send(WorkerEvent::UsageUpdated(usage));
                }
            }
            Ok(WorkerCommand::Refresh)
            | Ok(WorkerCommand::SetAutomaticActivation(_))
            | Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

fn tick(
    provider: &mut impl LimitProvider,
    activator: &mut impl Activator,
    state: &mut ActivationState,
    automatic_activation: bool,
) -> Result<Vec<WorkerEvent>> {
    let mut limits = provider.read_limits()?;
    let mut events = Vec::new();

    if automatic_activation && state.decide(&limits.primary) == Decision::ActivateNow {
        state.record_attempt(Utc::now());
        match activator.activate() {
            Ok(()) => {
                if let Ok(fresh) = provider.read_limits() {
                    limits = fresh;
                }
                state.observe(&limits.primary);
                events.push(WorkerEvent::ActivationSucceeded);
            }
            Err(error) => events.push(WorkerEvent::ActivationFailed(error.to_string())),
        }
    } else {
        state.observe(&limits.primary);
    }

    events.insert(0, WorkerEvent::LimitsUpdated(limits));
    Ok(events)
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;
    use chrono::TimeZone;

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
            limits_at(15, 1),
            limits_at(20, 1),
            limits_at(20, 1),
        ]);
        let mut activator = CountingActivator(0);
        let mut state = ActivationState::default();

        tick(&mut provider, &mut activator, &mut state, true).unwrap();
        assert_eq!(activator.0, 0);
        tick(&mut provider, &mut activator, &mut state, true).unwrap();
        assert_eq!(activator.0, 0);
        tick(&mut provider, &mut activator, &mut state, true).unwrap();
        assert_eq!(activator.0, 1);
        assert_eq!(state.last_seen_resets_at, limits_at(20, 1).primary.resets_at);
        tick(&mut provider, &mut activator, &mut state, true).unwrap();
        assert_eq!(activator.0, 1);
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
            &mut ScriptedProvider::new(vec![limits_at(15, 1)]),
            &mut FailingActivator,
            &mut state,
            true,
        )
        .unwrap();
        assert!(matches!(
            events
                .iter()
                .find(|event| matches!(event, WorkerEvent::ActivationFailed(_))),
            Some(WorkerEvent::ActivationFailed(_))
        ));
        assert_eq!(state.last_seen_resets_at, limits_at(15, 0).primary.resets_at);
    }

    #[test]
    fn usage_statistics_interval_is_ten_minutes() {
        assert_eq!(USAGE_STATS_INTERVAL, Duration::from_secs(600));
    }
}
