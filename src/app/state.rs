use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU8, Ordering},
        mpsc::{Receiver, Sender},
    },
    time::Duration,
};

use chrono::{DateTime, Utc};

use super::limits_persist::LimitsPersister;
use crate::{
    app_event::AppEvent,
    instances::ProviderId,
    limits::{ProviderLimits, RateLimits},
    provider_registry::ProviderKind,
    provider_status::{ProviderStatus, ProviderStatusCommand, ProviderStatusWorker},
    reset_detection::AnnouncedReset,
    reset_feed::{ResetFeedCommand, ResetFeedWorker},
    settings::Settings,
    updater::UpdateController,
    worker::{UsageAction, WorkerCommand},
};

fn limits_persister() -> &'static LimitsPersister {
    static PERSISTER: OnceLock<LimitsPersister> = OnceLock::new();
    PERSISTER.get_or_init(|| {
        LimitsPersister::spawn(|provider, limits| {
            if let Err(error) =
                crate::store::with_store(|store| store.save_limits(provider, limits))
            {
                eprintln!(
                    "failed to persist {} limits: {error:#}",
                    provider.display_name()
                );
            }
        })
    })
}

/// Which shared snapshots changed since the bridge last fanned them out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Changes(u8);

impl Changes {
    pub(crate) const LIMITS: Self = Self(1);
    pub(crate) const USAGE: Self = Self(2);
    pub(crate) const FEEDS: Self = Self(4);

    pub(crate) fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    pub(crate) fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Shared startup state handed from `main` into the reactor render tree.
pub struct AppState {
    /// The settings the app started with. Live settings reach the bridge
    /// through `settings_rx`.
    pub settings: Settings,
    /// The sole live rate-limit snapshot. Both the tray and popup read this
    /// store, and worker results replace it atomically before either surface
    /// is repainted. Write it only through the `replace_*` methods, which
    /// tell the bridge to repaint every surface.
    pub limits: Mutex<ProviderLimits>,
    /// Latest valid public forced-reset announcements. Kept outside UiState so
    /// hidden popups avoid a full native-tree publish on every feed poll.
    pub forced_resets: Mutex<Vec<AnnouncedReset>>,
    /// Ongoing status-page incidents of watched drivers.
    pub provider_statuses: Mutex<Vec<ProviderStatus>>,
    /// Bits of [`Changes`] not yet fanned out by the bridge.
    pub changes: AtomicU8,
    pub commands: Mutex<HashMap<ProviderId, Sender<WorkerCommand>>>,
    pub workers: Mutex<crate::provider::ProviderWorkers>,
    pub worker_events_rx: Mutex<Option<Receiver<AppEvent>>>,
    pub worker_events_tx: Sender<AppEvent>,
    /// Independent public-feed worker for announced Codex forced resets.
    pub reset_feed_worker: Mutex<Option<ResetFeedWorker>>,
    /// Public status-page poller for provider incidents.
    pub provider_status_worker: Mutex<Option<ProviderStatusWorker>>,
    pub activation_path: PathBuf,
    /// Provider-scoped errors from workers that could not be created at startup.
    /// They remain visible until that provider returns a successful limits
    /// response, just like errors received from a running worker.
    pub startup_provider_errors: Vec<(ProviderId, String)>,
    /// Last activation attempt loaded from persisted activation state.
    pub last_activation_at: Option<DateTime<Utc>>,
    /// Live settings pushes from the settings window; drained by the tray bridge.
    pub settings_rx: Mutex<Option<Receiver<Settings>>>,
    /// Destructive usage actions are serialized by the tray bridge and then
    /// fanned out to every provider usage worker.
    pub usage_actions_rx: Mutex<Option<Receiver<UsageAction>>>,
    pub usage_actions_tx: Sender<UsageAction>,
    pub updates: Arc<UpdateController>,
}

impl AppState {
    pub(super) fn mark(&self, changes: Changes) {
        self.changes.fetch_or(changes.0, Ordering::SeqCst);
    }

    /// Snapshots changed since the last call.
    pub(crate) fn take_changes(&self) -> Changes {
        Changes(self.changes.swap(0, Ordering::SeqCst))
    }

    pub(crate) fn current_limits(&self) -> ProviderLimits {
        self.limits
            .lock()
            .map(|limits| limits.clone())
            .unwrap_or_default()
    }

    /// Overlay locally chosen OpenRouter account names onto the live quota
    /// snapshot. A rename is a settings-only change and must not wait for the
    /// next worker poll or an app restart.
    pub(crate) fn apply_openrouter_account_names(&self, settings: &Settings) {
        let Ok(mut current) = self.limits.lock() else {
            return;
        };
        let mut renamed = Vec::new();
        for instance in settings
            .instances
            .iter()
            .filter(|instance| instance.driver == ProviderKind::OpenRouter)
        {
            let provider = instance.provider_id();
            if crate::openrouter::apply_account_names(current.get_mut(provider), instance) {
                renamed.push((provider, current.get(provider).clone()));
            }
        }
        drop(current);
        if renamed.is_empty() {
            return;
        }
        self.mark(Changes::LIMITS);
        for (provider, limits) in renamed {
            limits_persister().save(provider, limits);
        }
    }

    pub(crate) fn replace_limits(&self, provider: ProviderId, mut limits: RateLimits) {
        if let Ok(mut current) = self.limits.lock() {
            // Quota polling must not erase the independently refreshed usage
            // history between its ten-minute scans.
            limits.usage = current.get(provider).usage.clone();
            *current.get_mut(provider) = limits.clone();
        } else {
            return;
        }
        self.mark(Changes::LIMITS);
        // Never wait for storage on the caller (the tray thread): usage scans
        // hold the store for seconds. The persister coalesces to the latest
        // snapshot per provider and writes in the background.
        limits_persister().save(provider, limits);
    }

    /// Blocks (bounded) until queued limit snapshots reach storage.
    pub fn flush_limits_persistence() {
        limits_persister().flush(Duration::from_secs(3));
    }

    pub(crate) fn replace_usage(&self, provider: ProviderId, usage: crate::usage::UsageStatistics) {
        if let Ok(mut current) = self.limits.lock() {
            current.get_mut(provider).usage = usage;
            self.mark(Changes::USAGE);
        }
    }

    pub(crate) fn clear_usage_snapshot(&self) {
        if let Ok(mut limits) = self.limits.lock() {
            let providers = limits
                .iter()
                .map(|(provider, _)| provider)
                .collect::<Vec<_>>();
            for provider in providers {
                limits.get_mut(provider).usage = crate::usage::UsageStatistics::default();
            }
            self.mark(Changes::USAGE);
        }
    }

    pub(crate) fn take_worker_events(&self) -> Option<Receiver<AppEvent>> {
        self.worker_events_rx.lock().ok()?.take()
    }

    pub(crate) fn worker_commands(&self) -> Vec<(ProviderId, Sender<WorkerCommand>)> {
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

    /// Asks every worker to poll now. Returns whether any worker took it.
    pub(crate) fn refresh_all(&self) -> bool {
        let mut requested = false;
        for (_, commands) in self.worker_commands() {
            requested |= commands.send(WorkerCommand::Refresh).is_ok();
        }
        requested
    }

    pub(crate) fn current_forced_resets(&self) -> Vec<AnnouncedReset> {
        self.forced_resets
            .lock()
            .map(|resets| resets.clone())
            .unwrap_or_default()
    }

    pub(crate) fn replace_forced_resets(&self, resets: Vec<AnnouncedReset>) {
        if let Ok(mut current) = self.forced_resets.lock() {
            *current = resets;
            self.mark(Changes::FEEDS);
        }
    }

    pub(crate) fn sync_reset_feed(&self, settings: &Settings) {
        self.send_reset_feed(ResetFeedCommand::SetEnabled(
            settings.notifications.forced_reset_feed_enabled,
        ));
        self.send_reset_feed(ResetFeedCommand::SetRefreshInterval(Duration::from_secs(
            settings.reset_announcement_refresh_interval.seconds(),
        )));
    }

    pub(crate) fn current_provider_statuses(&self) -> Vec<ProviderStatus> {
        self.provider_statuses
            .lock()
            .map(|statuses| statuses.clone())
            .unwrap_or_default()
    }

    pub(crate) fn replace_provider_statuses(&self, statuses: Vec<ProviderStatus>) {
        if let Ok(mut current) = self.provider_statuses.lock() {
            *current = statuses;
            self.mark(Changes::FEEDS);
        }
    }

    fn send_provider_status(&self, command: ProviderStatusCommand) {
        if let Ok(worker) = self.provider_status_worker.lock()
            && let Some(worker) = worker.as_ref()
        {
            let _ = worker.commands.send(command);
        }
    }

    pub(crate) fn sync_provider_status(&self, settings: &Settings) {
        self.send_provider_status(ProviderStatusCommand::Watch(
            crate::provider_status::watched_kinds(settings),
        ));
    }

    pub(crate) fn mark_provider_incident_notified(&self, id: String) {
        self.send_provider_status(ProviderStatusCommand::MarkNotified(id));
    }

    fn send_reset_feed(&self, command: ResetFeedCommand) {
        if let Ok(worker) = self.reset_feed_worker.lock()
            && let Some(worker) = worker.as_ref()
        {
            let _ = worker.commands.send(command);
        }
    }

    pub(crate) fn mark_forced_reset_info_notified(&self, id: String) {
        self.send_reset_feed(ResetFeedCommand::MarkNotified(id));
    }

    /// Feeds a Codex quota sample to announced-reset detection. The previous
    /// snapshot goes first: on the very first run it is the persisted sample
    /// from the last session, which lets a reset that landed while the app
    /// was closed be recognized. Samples the detector already has are ignored.
    pub(crate) fn observe_reset_limits(&self, provider: ProviderId, current: &RateLimits) {
        if provider.kind() != ProviderKind::Codex {
            return;
        }
        let previous = self.current_limits().get(provider).clone();
        for limits in [&previous, current] {
            if let Some(observation) = crate::reset_detection::Observation::from_limits(limits) {
                self.send_reset_feed(ResetFeedCommand::Observe(
                    provider.id().to_owned(),
                    Box::new(observation),
                ));
            }
        }
    }

    /// A reset credit spent from the popup, so detection does not mistake
    /// its effect for an announced forced reset.
    pub(crate) fn record_banked_reset_use(&self, provider: ProviderId) {
        if provider.kind() == ProviderKind::Codex {
            self.send_reset_feed(ResetFeedCommand::RecordBankedUse(
                provider.id().to_owned(),
                Utc::now(),
            ));
        }
    }

    /// Applies instance toggles without disturbing workers that remain
    /// enabled. Workers of removed or disabled instances stop, and instances
    /// listed in `restart` get a fresh worker.
    pub(crate) fn sync_provider_workers(
        &self,
        settings: &Settings,
        restart: &[ProviderId],
    ) -> Vec<(ProviderId, String)> {
        let enabled = settings.enabled_providers();
        let stopped = self.workers.lock().map_or_else(
            |_| Vec::new(),
            |mut workers| {
                let remove = workers
                    .keys()
                    .copied()
                    .filter(|provider| !enabled.contains(provider) || restart.contains(provider))
                    .collect::<Vec<_>>();
                remove
                    .into_iter()
                    .filter_map(|provider| workers.remove(&provider))
                    .collect::<Vec<_>>()
            },
        );
        for worker in stopped {
            worker.shutdown();
        }
        if let Ok(mut commands) = self.commands.lock() {
            commands
                .retain(|provider, _| enabled.contains(provider) && !restart.contains(provider));
        }
        if let Ok(mut limits) = self.limits.lock() {
            // Disabled instances drop their live sample; removed ones vanish.
            limits.retain(|provider| settings.instance(provider).is_some());
            for provider in settings.provider_ids() {
                if !enabled.contains(&provider) {
                    *limits.get_mut(provider) = RateLimits::default();
                }
            }
            self.mark(Changes::LIMITS);
        }

        let mut errors = Vec::new();
        for provider in enabled {
            if self
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
        if let Ok(mut worker) = self.reset_feed_worker.lock()
            && let Some(worker) = worker.take()
        {
            worker.shutdown();
        }
        if let Ok(mut worker) = self.provider_status_worker.lock()
            && let Some(worker) = worker.take()
        {
            worker.shutdown();
        }
    }
}
