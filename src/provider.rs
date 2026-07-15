use std::{
    collections::HashMap,
    path::PathBuf,
    sync::mpsc::Sender,
    thread,
    time::Duration,
};

use anyhow::{anyhow, Result};

use crate::{
    claude::ClaudeClient,
    codex::{first_available, CodexActivator, CodexClient},
    settings::{ProviderKind, Settings},
    usage::UsageStatistics,
    worker::{self, Activator, UsageProvider, WorkerEvent, WorkerHandle},
};

pub type ProviderWorkers = HashMap<ProviderKind, WorkerHandle>;

/// Starts every enabled provider independently. Each worker has its own poll
/// loop, then forwards into the shared UI stream with a provider identity.
pub fn start_enabled_workers(
    settings: &Settings,
    activation_path: PathBuf,
    events: Sender<WorkerEvent>,
) -> (ProviderWorkers, Vec<String>) {
    let mut workers = ProviderWorkers::new();
    let mut errors = Vec::new();
    for provider in [ProviderKind::Codex, ProviderKind::Claude] {
        if !settings.providers.is_enabled(provider) {
            continue;
        }
        match start_provider_worker(provider, settings, activation_path.clone(), events.clone()) {
            Ok(worker) => {
                workers.insert(provider, worker);
            }
            Err(error) => errors.push(format!("{}: {error:#}", provider.display_name())),
        }
    }
    (workers, errors)
}

pub fn start_provider_worker(
    provider: ProviderKind,
    settings: &Settings,
    activation_path: PathBuf,
    events: Sender<WorkerEvent>,
) -> Result<WorkerHandle> {
    let mut worker = match provider {
        ProviderKind::Codex => {
            let executable = first_available(settings.codex_path.as_deref())?;
            worker::start_worker(
                CodexClient::new(&executable),
                CodexClient::new(&executable),
                CodexActivator::new(executable),
                activation_path,
                settings.automatic_activation,
                settings.history_retention_days,
                Duration::from_secs(60),
            )
        }
        ProviderKind::Claude => worker::start_worker(
            ClaudeClient::new(),
            EmptyUsageProvider,
            NoopActivator,
            activation_path,
            false,
            settings.history_retention_days,
            Duration::from_secs(60),
        ),
    };
    let source_events = worker
        .take_events()
        .ok_or_else(|| anyhow!("provider worker did not expose an event stream"))?;
    thread::spawn(move || {
        while let Ok(event) = source_events.recv() {
            let mapped = match event {
                WorkerEvent::LimitsUpdated(limits) => {
                    Some(WorkerEvent::ProviderLimitsUpdated(provider, limits))
                }
                WorkerEvent::UsageUpdated(usage) => {
                    Some(WorkerEvent::ProviderUsageUpdated(provider, usage))
                }
                WorkerEvent::ActivationSucceeded => Some(WorkerEvent::ProviderActivationSucceeded(provider)),
                WorkerEvent::ActivationFailed(error) => {
                    Some(WorkerEvent::ProviderActivationFailed(provider, error))
                }
                WorkerEvent::PollFailed(error) => Some(WorkerEvent::ProviderPollFailed(provider, error)),
                WorkerEvent::Stopped => None,
                // Only the worker itself emits unscoped events. Passing any
                // already-scoped value through avoids silently losing data if
                // a future provider delegates another coordinator.
                event => Some(event),
            };
            if let Some(event) = mapped {
                if events.send(event).is_err() {
                    break;
                }
            }
        }
    });
    Ok(worker)
}

struct EmptyUsageProvider;

impl UsageProvider for EmptyUsageProvider {
    fn load_cached_usage_statistics(&mut self, _: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }

    fn refresh_usage_statistics(&mut self, _: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }
}

struct NoopActivator;

impl Activator for NoopActivator {
    fn activate(&mut self) -> Result<()> {
        Err(anyhow!("automatic activation is only available for Codex"))
    }
}
