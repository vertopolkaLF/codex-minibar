use std::{collections::HashMap, path::PathBuf, sync::mpsc::Sender, thread, time::Duration};

use anyhow::{Result, anyhow};

use crate::{
    claude::{ClaudeActivator, ClaudeClient},
    codex::{CodexActivator, CodexClient, first_available},
    cursor::{CursorActivator, CursorClient},
    settings::{ProviderKind, Settings},
    worker::{self, WorkerEvent, WorkerHandle},
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
    for provider in crate::provider_registry::PROVIDERS
        .iter()
        .map(|descriptor| descriptor.kind)
    {
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
    let activation_path = provider_activation_path(provider, activation_path);
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
                Duration::from_secs(settings.limit_refresh_interval.seconds()),
            )
        }
        ProviderKind::Claude => worker::start_worker(
            ClaudeClient::new(),
            ClaudeClient::new(),
            ClaudeActivator::new(),
            activation_path,
            settings.automatic_activation,
            settings.history_retention_days,
            Duration::from_secs(settings.limit_refresh_interval.seconds()),
        ),
        ProviderKind::Cursor => worker::start_worker(
            CursorClient::new(),
            CursorClient::new(),
            CursorActivator,
            activation_path,
            false,
            settings.history_retention_days,
            Duration::from_secs(settings.limit_refresh_interval.seconds()),
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
                WorkerEvent::ActivationSucceeded => {
                    Some(WorkerEvent::ProviderActivationSucceeded(provider))
                }
                WorkerEvent::ActivationFailed(error) => {
                    Some(WorkerEvent::ProviderActivationFailed(provider, error))
                }
                WorkerEvent::PollFailed(error) => {
                    Some(WorkerEvent::ProviderPollFailed(provider, error))
                }
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

fn provider_activation_path(provider: ProviderKind, base_path: PathBuf) -> PathBuf {
    match provider {
        // Preserve the existing Codex state file so current users retain their
        // established activation baseline after updating.
        ProviderKind::Codex => base_path,
        // Claude has an independent five-hour clock; sharing Codex's baseline
        // would suppress or duplicate an activation whenever both are enabled.
        ProviderKind::Claude => base_path.with_file_name("activation-claude.toml"),
        ProviderKind::Cursor => base_path.with_file_name("activation-cursor.toml"),
    }
}
