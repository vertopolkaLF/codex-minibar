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
    pub events: Receiver<WorkerEvent>,
    join: Option<JoinHandle<()>>,
}

impl WorkerHandle {
    pub fn refresh(&self) {
        let _ = self.commands.send(WorkerCommand::Refresh);
    }

    pub fn shutdown(mut self) {
        let _ = self.commands.send(WorkerCommand::Shutdown);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }

    /// Split into command sender, event receiver, and join handle.
    pub fn into_parts(mut self) -> (Sender<WorkerCommand>, Receiver<WorkerEvent>, JoinHandle<()>) {
        let join = self
            .join
            .take()
            .expect("worker join handle missing");
        (self.commands, self.events, join)
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
                Ok(WorkerCommand::Refresh) | Err(RecvTimeoutError::Timeout) => {}
            }
        }
        let _ = event_sender.send(WorkerEvent::Stopped);
    });
    WorkerHandle {
        commands: command_sender,
        events: event_receiver,
        join: Some(join),
    }
}

fn tick(
    provider: &mut impl LimitProvider,
    activator: &mut impl Activator,
    state: &mut ActivationState,
    automatic_activation: bool,
) -> Result<Vec<WorkerEvent>> {
    let limits = provider.read_limits()?;
    let mut events = vec![WorkerEvent::LimitsUpdated(limits.clone())];
    if automatic_activation && state.decide(&limits.primary, Utc::now()) == Decision::ActivateNow {
        let now = Utc::now();
        state.record_attempt(&limits.primary, now);
        match activator.activate() {
            Ok(()) => {
                state.record_success(&limits.primary, now);
                events.push(WorkerEvent::ActivationSucceeded);
            }
            Err(error) => events.push(WorkerEvent::ActivationFailed(error.to_string())),
        }
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;

    use super::*;
    use crate::limits::LimitWindow;

    struct Provider(RateLimits);
    impl LimitProvider for Provider {
        fn read_limits(&mut self) -> Result<RateLimits> {
            Ok(self.0.clone())
        }
    }

    struct CountingActivator(usize);
    impl Activator for CountingActivator {
        fn activate(&mut self) -> Result<()> {
            self.0 += 1;
            Ok(())
        }
    }

    #[test]
    fn tick_activates_missing_reset_only_once() {
        let limits = RateLimits {
            primary: LimitWindow::default(),
            secondary: LimitWindow::default(),
            sampled_at: Utc::now(),
            ..RateLimits::default()
        };
        let mut provider = Provider(limits);
        let mut activator = CountingActivator(0);
        let mut state = ActivationState::default();
        tick(&mut provider, &mut activator, &mut state, true).unwrap();
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
        let limits = RateLimits {
            sampled_at: Utc::now(),
            ..RateLimits::default()
        };
        let events = tick(
            &mut Provider(limits),
            &mut FailingActivator,
            &mut ActivationState::default(),
            true,
        )
        .unwrap();
        assert!(matches!(
            events.last(),
            Some(WorkerEvent::ActivationFailed(_))
        ));
    }
}
