use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::Result;
use chrono::Utc;

use crate::{
    limits::RateLimits,
    scheduler::{ActivationState, Decision},
};

pub trait LimitProvider: Send + 'static {
    fn read_limits(&mut self) -> Result<RateLimits>;
}

pub trait Activator: Send + 'static {
    fn activate(&mut self) -> Result<()>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerCommand {
    Refresh,
    SetAutomaticActivation(bool),
    Shutdown,
}

#[derive(Clone, Debug)]
pub enum WorkerEvent {
    LimitsUpdated(RateLimits),
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
    /// the owner responsible for joining the worker during shutdown.
    pub fn take_events(&mut self) -> Option<Receiver<WorkerEvent>> {
        self.events.take()
    }
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn start_worker(
    mut provider: impl LimitProvider,
    mut activator: impl Activator,
    state_path: PathBuf,
    automatic_activation: bool,
    poll_interval: Duration,
) -> WorkerHandle {
    let (command_sender, command_receiver) = mpsc::channel();
    let (event_sender, event_receiver) = mpsc::channel();
    let join = thread::spawn(move || {
        let mut state = ActivationState::load_or_default(&state_path).unwrap_or_default();
        let mut automatic_activation = automatic_activation;
        loop {
            match tick(
                &mut provider,
                &mut activator,
                &mut state,
                automatic_activation,
            ) {
                Ok(events) => {
                    let _ = state.save(&state_path);
                    for event in events {
                        let _ = event_sender.send(event);
                    }
                }
                Err(error) => {
                    let _ = event_sender.send(WorkerEvent::PollFailed(error.to_string()));
                }
            }
            match command_receiver.recv_timeout(poll_interval) {
                Ok(WorkerCommand::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(WorkerCommand::SetAutomaticActivation(enabled)) => {
                    automatic_activation = enabled;
                }
                Ok(WorkerCommand::Refresh) | Err(RecvTimeoutError::Timeout) => {}
            }
        }
        let _ = event_sender.send(WorkerEvent::Stopped);
    });
    WorkerHandle {
        commands: command_sender,
        events: Some(event_receiver),
        join: Some(join),
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
                // Re-read so `observe` baselines the post-activation reset time
                // and we do not immediately fire again on the next tick.
                if let Ok(fresh) = provider.read_limits() {
                    limits = fresh;
                }
                state.observe(&limits.primary);
                events.push(WorkerEvent::ActivationSucceeded);
            }
            Err(error) => {
                // Keep the previous baseline so a stable post-reset timestamp
                // still retries on the next poll instead of being forgotten.
                events.push(WorkerEvent::ActivationFailed(error.to_string()));
            }
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
            // Post-activation refresh inside the activating tick.
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
        assert_eq!(
            state.last_seen_resets_at,
            limits_at(20, 1).primary.resets_at
        );

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
        // Keep the old baseline so the next poll retries the same change.
        assert_eq!(
            state.last_seen_resets_at,
            limits_at(15, 0).primary.resets_at
        );
    }
}
