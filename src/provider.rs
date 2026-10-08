use std::{collections::HashMap, path::PathBuf, sync::mpsc::Sender, thread, time::Duration};

use anyhow::{Context, Result, anyhow};

use crate::{
    claude::{ClaudeActivator, ClaudeClient},
    codex::{CodexActivator, CodexClient, first_available},
    cursor::{CursorActivator, CursorClient},
    instances::{Capabilities, ProviderId, ProviderInstance},
    openrouter::OpenRouterClient,
    settings::{ProviderKind, Settings},
    worker::{self, WorkerEvent, WorkerHandle},
};

pub type ProviderWorkers = HashMap<ProviderId, WorkerHandle>;

/// Starts every enabled instance independently. Each worker has its own poll
/// loop, then forwards into the shared UI stream with the instance identity.
pub fn start_enabled_workers(
    settings: &Settings,
    activation_path: PathBuf,
    events: Sender<WorkerEvent>,
) -> (ProviderWorkers, Vec<(ProviderId, String)>) {
    start_enabled_workers_with_limits(
        settings,
        activation_path,
        events,
        &crate::limits::ProviderLimits::default(),
    )
}

pub fn start_enabled_workers_with_limits(
    settings: &Settings,
    activation_path: PathBuf,
    events: Sender<WorkerEvent>,
    cached_limits: &crate::limits::ProviderLimits,
) -> (ProviderWorkers, Vec<(ProviderId, String)>) {
    let mut workers = ProviderWorkers::new();
    let mut errors = Vec::new();
    for provider in settings.enabled_providers() {
        match start_provider_worker_with_limits(
            provider,
            settings,
            activation_path.clone(),
            events.clone(),
            Some(cached_limits.get(provider)),
        ) {
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
    provider: ProviderId,
    settings: &Settings,
    activation_path: PathBuf,
    events: Sender<WorkerEvent>,
) -> Result<WorkerHandle> {
    start_provider_worker_with_limits(provider, settings, activation_path, events, None)
}

/// Settings shared by every worker regardless of driver.
struct WorkerOptions {
    activation_path: PathBuf,
    automatic_activation: bool,
    schedules: Vec<crate::settings::ScheduledActivation>,
    pauses: Vec<crate::settings::AutoActivationPause>,
    history_retention_days: u16,
    usage_refresh_interval: Duration,
    usage_collection_enabled: bool,
    limit_refresh_interval: Duration,
}

pub(crate) fn start_provider_worker_with_limits(
    provider: ProviderId,
    settings: &Settings,
    activation_path: PathBuf,
    events: Sender<WorkerEvent>,
    _cached_limits: Option<&crate::limits::RateLimits>,
) -> Result<WorkerHandle> {
    let instance = settings
        .instance(provider)
        .with_context(|| format!("{} is not configured", provider.display_name()))?;
    let activates = crate::provider_registry::descriptor(provider.kind()).supports_activation;
    let options = WorkerOptions {
        activation_path: provider_activation_path(provider, activation_path),
        automatic_activation: automatic_activation(provider, settings),
        schedules: if activates {
            schedules_for(provider, settings)
        } else {
            Vec::new()
        },
        pauses: if activates {
            auto_activation_pauses_for(provider, settings)
        } else {
            Vec::new()
        },
        history_retention_days: settings.history_retention_days,
        usage_refresh_interval: Duration::from_secs(settings.usage_refresh_interval.seconds()),
        usage_collection_enabled: settings.usage_stats_collection_enabled(provider),
        limit_refresh_interval: Duration::from_secs(instance.refresh_interval().seconds()),
    };
    // A replaced worker can still have queued output. Tag every forwarded
    // event so the bridge rejects anything from an older credential.
    let worker_revision = instance.credentials_revision;
    let mut worker = start_driver_worker(instance, options)?;
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
                    WorkerEvent::UsageLoadedFromCache(usage) => Some(
                        WorkerEvent::ProviderUsageLoadedFromCache(provider, worker_revision, usage),
                    ),
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

fn start_driver_worker(
    instance: &ProviderInstance,
    options: WorkerOptions,
) -> Result<WorkerHandle> {
    let provider = instance.provider_id();
    macro_rules! start_worker {
        ($limits:expr, $usage:expr, $activator:expr, $activation:expr) => {
            worker::start_worker(
                $limits,
                $usage,
                $activator,
                options.activation_path,
                $activation && options.automatic_activation,
                options.schedules,
                options.pauses,
                options.history_retention_days,
                options.usage_refresh_interval,
                options.usage_collection_enabled,
                options.limit_refresh_interval,
            )
        };
    }
    Ok(match instance.driver {
        ProviderKind::Codex => {
            let executable = first_available(instance.binary_path.as_deref())
                .unwrap_or_else(|_| PathBuf::from("codex"));
            crate::logger::info(format!(
                "{} executable: {}",
                provider.display_name(),
                executable.display()
            ));
            start_worker!(
                CodexClient::for_instance(&executable, instance),
                CodexClient::for_instance(&executable, instance),
                CodexActivator::new(executable).with_home(instance.config_folder()),
                true
            )
        }
        ProviderKind::Claude => {
            let folder = instance.config_folder();
            let executable =
                crate::claude::executable_for(instance.binary_path.as_deref(), folder.as_deref());
            crate::logger::info(format!(
                "{} executable: {}",
                provider.display_name(),
                executable
                    .as_deref()
                    .map_or("none".into(), |path| path.display().to_string())
            ));
            start_worker!(
                ClaudeClient::for_instance(instance),
                ClaudeClient::for_instance(instance),
                ClaudeActivator::new(executable).with_config_folder(folder),
                true
            )
        }
        ProviderKind::Cursor => {
            let executable = crate::cursor::installation_path(instance.binary_path.as_deref())
                .unwrap_or_else(|| PathBuf::from("Cursor.exe"));
            crate::logger::info(format!("Cursor executable: {}", executable.display()));
            start_worker!(
                CursorClient::new(),
                CursorClient::new(),
                CursorActivator,
                false
            )
        }
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => start_worker!(
            crate::opencode::OpenCodeClient::new(provider)?,
            crate::opencode::OpenCodeClient::new(provider)?,
            crate::opencode::OpenCodeClient::new(provider)?,
            false
        ),
        ProviderKind::OpenRouter => start_worker!(
            OpenRouterClient::new(instance)?,
            OpenRouterClient::new(instance)?,
            crate::openrouter::OpenRouterActivator,
            false
        ),
        ProviderKind::Antigravity => {
            let path = instance.binary_path.clone();
            if let Some(executable) = crate::antigravity::cli_available(path.as_deref()) {
                crate::logger::info(format!("Antigravity executable: {}", executable.display()));
            }
            start_worker!(
                crate::antigravity::AntigravityClient::new(path.clone()),
                crate::antigravity::AntigravityClient::new(path),
                crate::antigravity::AntigravityActivator,
                false
            )
        }
        ProviderKind::Grok => {
            let path = instance.binary_path.clone();
            if let Some(executable) = crate::grok::cli_available(path.as_deref()) {
                crate::logger::info(format!("Grok executable: {}", executable.display()));
            }
            start_worker!(
                crate::grok::GrokClient::new(path.clone()),
                crate::grok::GrokClient::new(path),
                crate::grok::GrokActivator,
                false
            )
        }
        ProviderKind::Kiro => {
            let client = || {
                crate::kiro::KiroClient::with_paths(
                    instance.binary_path.as_deref(),
                    instance.kiro_crew_path.as_deref(),
                    instance.kiro_cli_path.as_deref(),
                )
            };
            start_worker!(client(), client(), crate::kiro::KiroActivator, false)
        }
    })
}

/// Whether an instance's worker may start sessions on its own. Shared by
/// worker start-up and live settings changes so the two cannot disagree.
pub fn automatic_activation(provider: ProviderId, settings: &Settings) -> bool {
    settings.instance(provider).is_some_and(|instance| {
        instance.auto_activation && Capabilities::of(instance).auto_activation
    })
}

pub(crate) fn schedules_for(
    provider: ProviderId,
    settings: &Settings,
) -> Vec<crate::settings::ScheduledActivation> {
    if !settings
        .instance(provider)
        .is_some_and(|instance| Capabilities::of(instance).auto_activation)
    {
        return Vec::new();
    }
    settings
        .scheduled_activations
        .iter()
        .filter(|rule| rule.targets(provider))
        .cloned()
        .collect()
}

pub(crate) fn auto_activation_pauses_for(
    provider: ProviderId,
    settings: &Settings,
) -> Vec<crate::settings::AutoActivationPause> {
    if !settings
        .instance(provider)
        .is_some_and(|instance| Capabilities::of(instance).auto_activation)
    {
        return Vec::new();
    }
    settings
        .auto_activation_pauses
        .iter()
        .filter(|pause| pause.targets(provider))
        .cloned()
        .collect()
}

/// Every instance has an independent five-hour clock. Primary instances keep
/// the files they used before instances existed.
fn provider_activation_path(provider: ProviderId, base_path: PathBuf) -> PathBuf {
    if !provider.is_primary() {
        let id = provider
            .id()
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .collect::<String>();
        return base_path.with_file_name(format!("activation-{id}.toml"));
    }
    match provider.kind() {
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
        ProviderKind::Kiro => base_path.with_file_name("activation-kiro.toml"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instances::InstanceSource;

    #[test]
    fn activation_is_a_per_instance_setting() {
        let mut settings = Settings::default();
        let codex = ProviderId::primary(ProviderKind::Codex);
        assert!(!automatic_activation(codex, &settings));
        settings.instance_mut(codex).unwrap().auto_activation = true;
        assert!(automatic_activation(codex, &settings));
        let mut work = ProviderInstance::new(ProviderKind::Claude, "Work");
        work.auto_activation = true;
        let work = settings.add_instance(work);
        assert!(automatic_activation(work, &settings));
        assert!(!automatic_activation(
            ProviderId::primary(ProviderKind::Claude),
            &settings
        ));
        // A pasted credential has no CLI login to start a session with.
        settings.instance_mut(work).unwrap().source = InstanceSource::Manual;
        assert!(!automatic_activation(work, &settings));
    }

    #[test]
    fn every_instance_has_its_own_activation_state() {
        let base = PathBuf::from("state").join("activation.toml");
        assert_eq!(
            provider_activation_path(ProviderId::primary(ProviderKind::Codex), base.clone()),
            base
        );
        assert_eq!(
            provider_activation_path(ProviderId::primary(ProviderKind::Claude), base.clone()),
            base.with_file_name("activation-claude.toml")
        );
        assert_eq!(
            provider_activation_path(
                ProviderId::new(ProviderKind::Claude, "claude-work"),
                base.clone()
            ),
            base.with_file_name("activation-claude-work.toml")
        );
    }
}
