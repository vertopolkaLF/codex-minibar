use std::{collections::HashMap, path::PathBuf, sync::mpsc::Sender, thread, time::Duration};

use anyhow::{Context, Result, anyhow};

use crate::{
    app_event::{AppEvent, ProviderEvent},
    instances::{Capabilities, ProviderId, ProviderInstance},
    settings::Settings,
    worker::{WorkerEvent, WorkerHandle, WorkerOptions},
};

pub type ProviderWorkers = HashMap<ProviderId, WorkerHandle>;

/// Starts every enabled instance independently. Each worker has its own poll
/// loop, then forwards into the shared UI stream with the instance identity.
pub fn start_enabled_workers(
    settings: &Settings,
    activation_path: PathBuf,
    events: Sender<AppEvent>,
) -> (ProviderWorkers, Vec<(ProviderId, String)>) {
    let mut workers = ProviderWorkers::new();
    let mut errors = Vec::new();
    for provider in settings.enabled_providers() {
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
    provider: ProviderId,
    settings: &Settings,
    activation_path: PathBuf,
    events: Sender<AppEvent>,
) -> Result<WorkerHandle> {
    let instance = settings
        .instance(provider)
        .with_context(|| format!("{} is not configured", provider.display_name()))?;
    let options = WorkerOptions {
        activation_path: provider_activation_path(provider, activation_path),
        automatic_activation: automatic_activation(provider, settings),
        schedules: schedules_for(provider, settings),
        pauses: auto_activation_pauses_for(provider, settings),
        history_retention_days: settings.history_retention_days,
        usage_refresh_interval: usage_refresh_interval(settings),
        usage_collection_enabled: settings.usage_stats_collection_enabled(provider),
        limit_refresh_interval: limit_refresh_interval(instance),
    };
    // A replaced worker can still have queued output. Tag every forwarded
    // event so the bridge rejects anything from an older credential.
    let worker_revision = instance.credentials_revision;
    let mut worker = crate::drivers::start_worker(instance, options)?;
    let source_events = worker
        .take_events()
        .ok_or_else(|| anyhow!("provider worker did not expose an event stream"))?;
    let retired = worker.retired_flag();
    thread::spawn(move || {
        while let Ok(event) = source_events.recv() {
            // A shut-down worker may still finish a long scan or request.
            // Its output belongs to a replaced or disabled instance; only the
            // clear-barrier ack must survive so a pending clear can finish.
            if retired.load(std::sync::atomic::Ordering::Acquire)
                && !matches!(event, WorkerEvent::UsageDataCleared(_))
            {
                continue;
            }
            let event = AppEvent::Provider(ProviderEvent {
                provider,
                revision: worker_revision,
                event,
            });
            if events.send(event).is_err() {
                break;
            }
        }
    });
    Ok(worker)
}

/// The quota poll interval an instance's worker uses right now: the
/// configured one, stretched while game mode is active.
pub fn limit_refresh_interval(instance: &ProviderInstance) -> Duration {
    crate::game_mode::stretch_limit_interval(Duration::from_secs(
        instance.refresh_interval().seconds(),
    ))
}

/// The usage scan interval every worker uses right now.
pub fn usage_refresh_interval(settings: &Settings) -> Duration {
    crate::game_mode::stretch_usage_interval(Duration::from_secs(
        settings.usage_refresh_interval.seconds(),
    ))
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
    base_path
        .with_file_name(crate::provider_registry::descriptor(provider.kind()).activation_state_file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instances::InstanceSource;
    use crate::provider_registry::ProviderKind;

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
