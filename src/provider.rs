use std::{collections::HashMap, path::PathBuf, sync::mpsc::Sender, thread, time::Duration};

use anyhow::{Result, anyhow};

use crate::{
    claude::{ClaudeActivator, ClaudeClient},
    codex::{CodexActivator, CodexClient, first_available},
    cursor::{CursorActivator, CursorClient},
    openrouter::OpenRouterClient,
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
) -> (ProviderWorkers, Vec<(ProviderKind, String)>) {
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
            Err(error) => {
                crate::logger::info(format!(
                    "{} worker failed to start: {error:#}",
                    provider.display_name()
                ));
                errors.push((provider, format!("{error:#}")));
            }
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
    // The OpenRouter worker is replaced whenever this revision changes. Tag
    // every forwarded event so queued output from the old worker can never
    // overwrite the replacement worker's state.
    let worker_revision = match provider {
        ProviderKind::OpenRouter => settings.openrouter_credentials_revision,
        _ => 0,
    };
    let automatic_activation = settings.automatic_activation
        && crate::provider_registry::descriptor(provider).supports_activation;
    let mut worker = match provider {
        ProviderKind::Codex => {
            let executable = first_available(settings.codex_path.as_deref())?;
            crate::logger::info(format!("Codex executable: {}", executable.display()));
            worker::start_worker(
                CodexClient::new(&executable),
                CodexClient::new(&executable),
                CodexActivator::new(executable),
                activation_path,
                automatic_activation,
                schedules_for(provider, settings),
                auto_activation_pauses_for(provider, settings),
                settings.history_retention_days,
                Duration::from_secs(settings.usage_refresh_interval.seconds()),
                settings.usage_stats_enabled && settings.usage_stats_provider_enabled(provider),
                Duration::from_secs(settings.limit_refresh_interval.seconds()),
            )
        }
        ProviderKind::Claude => {
            let executable = crate::claude::first_available(settings.claude_path.as_deref())
                .unwrap_or_else(|| PathBuf::from("claude"));
            crate::logger::info(format!("Claude executable: {}", executable.display()));
            worker::start_worker(
                ClaudeClient::new(),
                ClaudeClient::new(),
                ClaudeActivator::new(Some(executable)),
                activation_path,
                automatic_activation,
                schedules_for(provider, settings),
                auto_activation_pauses_for(provider, settings),
                settings.history_retention_days,
                Duration::from_secs(settings.usage_refresh_interval.seconds()),
                settings.usage_stats_enabled && settings.usage_stats_provider_enabled(provider),
                Duration::from_secs(settings.limit_refresh_interval.seconds()),
            )
        }
        ProviderKind::Cursor => {
            let executable = crate::cursor::installation_path(settings.cursor_path.as_deref())
                .unwrap_or_else(|| PathBuf::from("Cursor.exe"));
            crate::logger::info(format!("Cursor executable: {}", executable.display()));
            worker::start_worker(
                CursorClient::new(),
                CursorClient::new(),
                CursorActivator,
                activation_path,
                false,
                Vec::new(),
                Vec::new(),
                settings.history_retention_days,
                Duration::from_secs(settings.usage_refresh_interval.seconds()),
                settings.usage_stats_enabled && settings.usage_stats_provider_enabled(provider),
                Duration::from_secs(settings.limit_refresh_interval.seconds()),
            )
        }
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => worker::start_worker(
            crate::opencode::OpenCodeClient::new(provider)?,
            crate::opencode::OpenCodeClient::new(provider)?,
            crate::opencode::OpenCodeClient::new(provider)?,
            activation_path,
            false,
            Vec::new(),
            Vec::new(),
            settings.history_retention_days,
            Duration::from_secs(settings.usage_refresh_interval.seconds()),
            settings.usage_stats_enabled && settings.usage_stats_provider_enabled(provider),
            Duration::from_secs(settings.limit_refresh_interval.seconds()),
        ),
        ProviderKind::OpenRouter => worker::start_worker(
            OpenRouterClient::new(settings)?,
            OpenRouterClient::new(settings)?,
            crate::openrouter::OpenRouterActivator,
            activation_path,
            false,
            Vec::new(),
            Vec::new(),
            settings.history_retention_days,
            Duration::from_secs(settings.usage_refresh_interval.seconds()),
            settings.usage_stats_enabled && settings.usage_stats_provider_enabled(provider),
            Duration::from_secs(settings.limit_refresh_interval.seconds()),
        ),
        ProviderKind::Antigravity => {
            let executable =
                crate::antigravity::cli_available(settings.antigravity_path.as_deref());
            if let Some(executable) = &executable {
                crate::logger::info(format!("Antigravity executable: {}", executable.display()));
            }
            worker::start_worker(
                crate::antigravity::AntigravityClient::new(settings.antigravity_path.clone()),
                crate::antigravity::AntigravityClient::new(settings.antigravity_path.clone()),
                crate::antigravity::AntigravityActivator,
                activation_path,
                false,
                Vec::new(),
                Vec::new(),
                settings.history_retention_days,
                Duration::from_secs(settings.usage_refresh_interval.seconds()),
                false,
                Duration::from_secs(settings.limit_refresh_interval.seconds()),
            )
        }
        ProviderKind::Grok => {
            let executable = crate::grok::cli_available(settings.grok_path.as_deref());
            if let Some(executable) = &executable {
                crate::logger::info(format!("Grok executable: {}", executable.display()));
            }
            worker::start_worker(
                crate::grok::GrokClient::new(settings.grok_path.clone()),
                crate::grok::GrokClient::new(settings.grok_path.clone()),
                crate::grok::GrokActivator,
                activation_path,
                false,
                Vec::new(),
                Vec::new(),
                settings.history_retention_days,
                Duration::from_secs(settings.usage_refresh_interval.seconds()),
                false,
                Duration::from_secs(settings.limit_refresh_interval.seconds()),
            )
        }
    };
    let source_events = worker
        .take_events()
        .ok_or_else(|| anyhow!("provider worker did not expose an event stream"))?;
    thread::spawn(move || {
        while let Ok(event) = source_events.recv() {
            let mapped =
                match event {
                    WorkerEvent::RequestStarted(kind) => Some(WorkerEvent::ProviderRequestStarted(
                        provider,
                        worker_revision,
                        kind,
                    )),
                    WorkerEvent::RequestFinished(kind) => Some(
                        WorkerEvent::ProviderRequestFinished(provider, worker_revision, kind),
                    ),
                    WorkerEvent::LimitsUpdated(limits) => Some(WorkerEvent::ProviderLimitsUpdated(
                        provider,
                        worker_revision,
                        limits,
                    )),
                    WorkerEvent::UsageUpdated(usage) => Some(WorkerEvent::ProviderUsageUpdated(
                        provider,
                        worker_revision,
                        usage,
                    )),
                    WorkerEvent::UsageDataCleared(generation) => {
                        // This acknowledges a clear command sent to this exact
                        // worker. Keep the clear generation across a worker
                        // replacement so the bridge can finish its barrier.
                        Some(WorkerEvent::ProviderUsageDataCleared(provider, generation))
                    }
                    WorkerEvent::UsageRefreshFailed(error) => Some(
                        WorkerEvent::ProviderUsageRefreshFailed(provider, worker_revision, error),
                    ),
                    WorkerEvent::ActivationStarted => Some(WorkerEvent::ProviderActivationStarted(
                        provider,
                        worker_revision,
                    )),
                    WorkerEvent::ActivationSucceeded => Some(
                        WorkerEvent::ProviderActivationSucceeded(provider, worker_revision),
                    ),
                    WorkerEvent::ActivationFailed(error) => Some(
                        WorkerEvent::ProviderActivationFailed(provider, worker_revision, error),
                    ),
                    WorkerEvent::PollFailed(error) => Some(WorkerEvent::ProviderPollFailed(
                        provider,
                        worker_revision,
                        error,
                    )),
                    WorkerEvent::Stopped => None,
                    // Only the worker itself emits unscoped events. Passing any
                    // already-scoped value through avoids silently losing data if
                    // a future provider delegates another coordinator.
                    event => Some(event),
                };
            if let Some(event) = mapped
                && events.send(event).is_err()
            {
                break;
            }
        }
    });
    Ok(worker)
}

fn schedules_for(
    provider: ProviderKind,
    settings: &Settings,
) -> Vec<crate::settings::ScheduledActivation> {
    if !crate::provider_registry::descriptor(provider).supports_activation {
        return Vec::new();
    }
    settings
        .scheduled_activations
        .iter()
        .filter(|rule| rule.provider() == Some(provider))
        .cloned()
        .collect()
}

fn auto_activation_pauses_for(
    provider: ProviderKind,
    settings: &Settings,
) -> Vec<crate::settings::AutoActivationPause> {
    if !crate::provider_registry::descriptor(provider).supports_activation {
        return Vec::new();
    }
    settings
        .auto_activation_pauses
        .iter()
        .filter(|pause| pause.provider() == Some(provider))
        .cloned()
        .collect()
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
        ProviderKind::OpenCodeZen => base_path.with_file_name("activation-opencode-zen.toml"),
        ProviderKind::OpenCodeGo => base_path.with_file_name("activation-opencode-go.toml"),
        ProviderKind::OpenRouter => base_path.with_file_name("activation-openrouter.toml"),
        ProviderKind::Antigravity => base_path.with_file_name("activation-antigravity.toml"),
        ProviderKind::Grok => base_path.with_file_name("activation-grok.toml"),
    }
}
