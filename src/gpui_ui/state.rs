use std::{collections::HashMap, sync::{Arc, Mutex, mpsc::{Receiver, Sender}}};
use chrono::{DateTime, Utc};
use crate::{settings::{Settings, ProviderKind}, limits::{ProviderLimits, RateLimits}, updater::UpdateController, worker::{WorkerCommand, WorkerEvent, UsageAction}};
pub struct AppState {
    pub settings: Settings,
    /// The sole live rate-limit snapshot. Both the tray and popup read this
    /// store, and worker results replace it atomically before either surface
    /// is repainted.
    pub limits: Mutex<ProviderLimits>,
    pub commands: Mutex<HashMap<ProviderKind, Sender<WorkerCommand>>>,
    pub workers: Mutex<crate::provider::ProviderWorkers>,
    pub worker_events_rx: Mutex<Option<Receiver<WorkerEvent>>>,
    pub worker_events_tx: Sender<WorkerEvent>,
    pub activation_path: std::path::PathBuf,
    /// Provider-scoped errors from workers that could not be created at startup.
    /// They remain visible until that provider returns a successful limits
    /// response, just like errors received from a running worker.
    pub startup_provider_errors: Vec<(ProviderKind, String)>,
    /// Last activation attempt loaded from persisted activation state.
    pub last_activation_at: Option<DateTime<Utc>>,
    /// Live settings pushes from the settings window; drained by the tray bridge.
    pub settings_rx: Mutex<Option<Receiver<Settings>>>,
    pub settings_tx: Sender<Settings>,
    /// Destructive usage actions are serialized by the tray bridge and then
    /// fanned out to every provider usage worker.
    pub usage_actions_rx: Mutex<Option<Receiver<UsageAction>>>,
    pub usage_actions_tx: Sender<UsageAction>,
    pub updates: Arc<UpdateController>,
}

impl AppState {
    pub(crate) fn current_limits(&self) -> ProviderLimits {
        self.limits
            .lock()
            .map(|limits| limits.clone())
            .unwrap_or_default()
    }

    pub(crate) fn replace_limits(&self, provider: ProviderKind, mut limits: RateLimits) {
        let persisted = if let Ok(mut current) = self.limits.lock() {
            // Quota polling must not erase the independently refreshed usage
            // history between its ten-minute scans.
            limits.usage = current.get(provider).usage.clone();
            *current.get_mut(provider) = limits.clone();
            Some(limits)
        } else {
            None
        };
        // Never hold the live UI snapshot while waiting for storage. Usage
        // refreshes can legitimately keep the SQLite writer busy briefly.
        if let Some(limits) = persisted
            && let Err(error) =
                crate::store::with_store(|store| store.save_limits(provider, &limits))
        {
            eprintln!(
                "failed to persist {} limits: {error:#}",
                provider.display_name()
            );
        }
    }

    pub(crate) fn replace_usage(
        &self,
        provider: ProviderKind,
        usage: crate::usage::UsageStatistics,
    ) {
        if let Ok(mut current) = self.limits.lock() {
            current.get_mut(provider).usage = usage;
        }
    }

    pub(crate) fn clear_usage_snapshot(&self) {
        if let Ok(mut limits) = self.limits.lock() {
            for provider in ProviderKind::ALL {
                limits.get_mut(provider).usage = crate::usage::UsageStatistics::default();
            }
        }
    }

    pub(crate) fn take_worker_events(&self) -> Option<Receiver<WorkerEvent>> {
        self.worker_events_rx.lock().ok()?.take()
    }

    pub(crate) fn worker_commands(&self) -> Vec<(ProviderKind, Sender<WorkerCommand>)> {
        self.commands
            .lock()
            .map(|commands| {
                commands
                    .iter()
                    .map(|(provider, commands)| (*provider, commands.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Applies provider toggles without disturbing workers that remain enabled.
    pub(crate) fn sync_provider_workers(
        &self,
        settings: &Settings,
        restart: &[ProviderKind],
    ) -> Vec<(ProviderKind, String)> {
        let disabled = crate::provider_registry::PROVIDERS
            .iter()
            .map(|descriptor| descriptor.kind)
            .filter(|provider| !settings.providers.is_enabled(*provider))
            .collect::<Vec<_>>();
        let stopped = self.workers.lock().map_or_else(
            |_| Vec::new(),
            |mut workers| {
                disabled
                    .iter()
                    .chain(
                        restart
                            .iter()
                            .filter(|provider| settings.providers.is_enabled(**provider)),
                    )
                    .filter_map(|provider| workers.remove(provider))
                    .collect()
            },
        );
        for worker in stopped {
            worker.shutdown();
        }
        if let Ok(mut commands) = self.commands.lock() {
            commands.retain(|provider, _| {
                settings.providers.is_enabled(*provider) && !restart.contains(provider)
            });
        }
        if let Ok(mut limits) = self.limits.lock() {
            for provider in &disabled {
                *limits.get_mut(*provider) = RateLimits::default();
            }
        }

        let mut errors = Vec::new();
        for provider in crate::provider_registry::PROVIDERS
            .iter()
            .map(|descriptor| descriptor.kind)
        {
            if !settings.providers.is_enabled(provider)
                || self
                    .workers
                    .lock()
                    .is_ok_and(|workers| workers.contains_key(&provider))
            {
                continue;
            }
            match crate::provider::start_provider_worker(
                provider,
                settings,
                self.activation_path.clone(),
                self.worker_events_tx.clone(),
            ) {
                Ok(worker) => {
                    if let Ok(mut commands) = self.commands.lock() {
                        commands.insert(provider, worker.commands.clone());
                    }
                    if let Ok(mut workers) = self.workers.lock() {
                        workers.insert(provider, worker);
                    }
                }
                Err(error) => errors.push((provider, format!("{error:#}"))),
            }
        }
        errors
    }

    pub fn shutdown_worker(&self) {
        if let Ok(mut workers) = self.workers.lock() {
            for (_, worker) in std::mem::take(&mut *workers) {
                worker.shutdown();
            }
        }
        if let Ok(mut commands) = self.commands.lock() {
            commands.clear();
        }
    }
}


