//! Provider coordination is independent of either window and never blocks GPUI.
use std::{
    collections::HashMap,
    sync::{
        mpsc::{self, Sender},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use crate::{
    limits::ProviderLimits,
    notifications::{self, LimitNotificationTracker},
    settings::{ProviderKind, Settings},
    updater::UpdatePhase,
    worker::{RequestKind, WorkerCommand, WorkerEvent},
};

use super::state::AppState;

#[derive(Clone)]
pub struct Snapshot {
    pub settings: Settings,
    pub limits: ProviderLimits,
    pub errors: HashMap<ProviderKind, String>,
    pub usage_errors: HashMap<ProviderKind, String>,
    pub error: Option<String>,
    pub active: Vec<(ProviderKind, RequestKind)>,
    pub update: UpdatePhase,
    pub revision: u64,
    pub settings_generation: u64,
}

pub enum Command {
    Settings(Box<Settings>, u64),
    Refresh,
    ClearUsage,
    Shutdown,
}

#[derive(Clone)]
pub struct Runtime {
    pub snapshot: Arc<Mutex<Snapshot>>,
    pub commands: Sender<Command>,
    pub state: Arc<AppState>,
}

impl Runtime {
    pub fn start(state: Arc<AppState>) -> Self {
        let mut initial = Snapshot {
            settings: state.settings.clone(),
            limits: state.current_limits(),
            errors: state.startup_provider_errors.iter().cloned().collect(),
            usage_errors: HashMap::new(),
            error: None,
            active: Vec::new(),
            update: state.updates.snapshot(),
            revision: 1,
            settings_generation: 0,
        };
        if initial
            .settings
            .absorb_discovered_popup_bricks(&initial.limits)
        {
            if let Ok(path) = Settings::default_path() {
                let _ = initial.settings.save(&path);
            }
        }
        let snapshot = Arc::new(Mutex::new(initial.clone()));
        let (commands, incoming) = mpsc::channel();
        let runtime = Self {
            snapshot: snapshot.clone(),
            commands,
            state: state.clone(),
        };
        thread::spawn(move || {
            let Some(events) = state.take_worker_events() else {
                return;
            };
            let mut current = initial;
            let mut trackers = HashMap::<ProviderKind, LimitNotificationTracker>::new();
            let mut clear_generation = 0u64;
            let mut pending_clear: Option<(u64, Vec<ProviderKind>)> = None;
            let mut next_update = std::time::Instant::now() + Duration::from_secs(21600);
            loop {
                let mut changed = false;
                while let Ok(command) = incoming.try_recv() {
                    match command {
                        Command::Shutdown => {
                            state.shutdown_worker();
                            return;
                        }
                        Command::Refresh => {
                            for (_, sender) in state.worker_commands() {
                                let _ = sender.send(WorkerCommand::Refresh);
                            }
                        }
                        Command::Settings(next, generation) => {
                            current.settings_generation = generation;
                            let result = apply_settings(&state, &current.settings, &next);
                            match result {
                                Ok(()) => {
                                    current.settings = *next;
                                    current.error = None;
                                }
                                Err(error) => {
                                    current.error =
                                        Some(format!("Could not save settings: {error:#}"))
                                }
                            }
                            current
                                .errors
                                .retain(|provider, _| current.settings.providers.is_enabled(*provider));
                            current.usage_errors.retain(|provider, _| {
                                current.settings.providers.is_enabled(*provider)
                            });
                            current
                                .active
                                .retain(|(provider, _)| current.settings.providers.is_enabled(*provider));
                            trackers
                                .retain(|provider, _| current.settings.providers.is_enabled(*provider));
                            changed = true;
                        }
                        Command::ClearUsage if pending_clear.is_none() => {
                            clear_generation += 1;
                            let targets = state
                                .worker_commands()
                                .into_iter()
                                .filter_map(|(provider, sender)| {
                                    sender
                                        .send(WorkerCommand::ClearUsageData(clear_generation))
                                        .is_ok()
                                        .then_some(provider)
                                })
                                .collect::<Vec<_>>();
                            state.clear_usage_snapshot();
                            if targets.is_empty() {
                                if let Err(error) =
                                    crate::store::with_store(|store| store.clear_usage_data())
                                {
                                    current.error = Some(error.to_string());
                                }
                            } else {
                                pending_clear = Some((clear_generation, targets));
                            }
                            changed = true;
                        }
                        Command::ClearUsage => {}
                    }
                }
                while let Ok(event) = events.try_recv() {
                    changed = true;
                    match event {
                        WorkerEvent::ProviderLimitsUpdated(provider, limits) => {
                            if !current.settings.providers.is_enabled(provider) {
                                continue;
                            }
                            trackers
                                .entry(provider)
                                .or_default()
                                .observe(&limits, &current.settings.notifications, provider);
                            state.replace_limits(provider, limits);
                            current.errors.remove(&provider);
                        }
                        WorkerEvent::ProviderUsageUpdated(provider, usage) => {
                            if pending_clear.is_none()
                                && current.settings.providers.is_enabled(provider)
                            {
                                state.replace_usage(provider, usage);
                                current.usage_errors.remove(&provider);
                            }
                        }
                        WorkerEvent::ProviderPollFailed(provider, error) => {
                            crate::logger::info(format!("{}: {error}", provider.display_name()));
                            current.errors.insert(provider, error);
                        }
                        WorkerEvent::ProviderUsageRefreshFailed(provider, error) => {
                            current.usage_errors.insert(provider, error);
                        }
                        WorkerEvent::ProviderRequestStarted(provider, kind) => {
                            current.active.push((provider, kind))
                        }
                        WorkerEvent::ProviderRequestFinished(provider, kind) => {
                            if let Some(index) =
                                current.active.iter().position(|entry| *entry == (provider, kind))
                            {
                                current.active.remove(index);
                            }
                        }
                        WorkerEvent::ProviderActivationSucceeded(provider) => {
                            if current.settings.notifications.activation_success {
                                notifications::show_activation_succeeded(provider);
                            }
                        }
                        WorkerEvent::ProviderActivationFailed(provider, error) => {
                            current.errors.insert(provider, error);
                        }
                        WorkerEvent::ProviderUsageDataCleared(provider, generation) => {
                            if let Some((expected, targets)) = pending_clear.as_mut() {
                                if *expected == generation {
                                    targets.retain(|target| *target != provider);
                                }
                                if targets.is_empty() {
                                    for (_, sender) in state.worker_commands() {
                                        let _ = sender
                                            .send(WorkerCommand::ResumeUsageRefresh(generation));
                                    }
                                    pending_clear = None;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                let update = state.updates.snapshot();
                if current.update != update {
                    current.update = update;
                    changed = true;
                }
                if notifications::take_toast_update_request() {
                    if let Err(error) = crate::updater::apply_pending_update() {
                        current.error = Some(error.to_string());
                        changed = true;
                    }
                }
                if current.settings.check_for_updates && std::time::Instant::now() >= next_update {
                    state
                        .updates
                        .check_async(true, current.settings.notifications.update_available);
                    next_update = std::time::Instant::now() + Duration::from_secs(21600);
                }
                current.limits = state.current_limits();
                if current
                    .settings
                    .absorb_discovered_popup_bricks(&current.limits)
                {
                    if let Ok(path) = Settings::default_path() {
                        let _ = current.settings.save(&path);
                    }
                    changed = true;
                }
                if changed {
                    current.revision = current.revision.wrapping_add(1);
                    if let Ok(mut target) = snapshot.lock() {
                        *target = current.clone();
                    }
                }
                thread::sleep(Duration::from_millis(50));
            }
        });
        runtime
    }

    pub fn read(&self) -> Snapshot {
        self.snapshot.lock().expect("runtime snapshot").clone()
    }

    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }
}

fn apply_settings(state: &AppState, previous: &Settings, next: &Settings) -> anyhow::Result<()> {
    next.validate()?;
    next.save(&Settings::default_path()?)?;
    next.apply_runtime_effects()?;
    crate::theme::apply_appearance(next.theme, next.accent_color);
    if next.check_for_updates && !previous.check_for_updates {
        state
            .updates
            .check_async(false, next.notifications.update_available);
    }
    let mut restart = Vec::new();
    for (provider, changed) in [
        (ProviderKind::Codex, previous.codex_path != next.codex_path),
        (ProviderKind::Claude, previous.claude_path != next.claude_path),
        (ProviderKind::Cursor, previous.cursor_path != next.cursor_path),
        (
            ProviderKind::OpenCodeZen,
            previous.opencode_zen_credentials_revision != next.opencode_zen_credentials_revision,
        ),
        (
            ProviderKind::OpenCodeGo,
            previous.opencode_go_credentials_revision != next.opencode_go_credentials_revision,
        ),
        (
            ProviderKind::OpenRouter,
            previous.openrouter_credentials_revision != next.openrouter_credentials_revision
                || previous.openrouter_accounts != next.openrouter_accounts,
        ),
    ] {
        if changed {
            restart.push(provider);
        }
    }
    let errors = state.sync_provider_workers(next, &restart);
    for (provider, error) in errors {
        crate::logger::info(format!("{}: {error}", provider.display_name()));
    }
    for (provider, sender) in state.worker_commands() {
        let activation = crate::provider_registry::descriptor(provider).supports_activation;
        for command in [
            WorkerCommand::SetAutomaticActivation(next.automatic_activation && activation),
            WorkerCommand::SetScheduledActivations(
                next.scheduled_activations
                    .iter()
                    .filter(|rule| activation && rule.provider() == Some(provider))
                    .cloned()
                    .collect(),
            ),
            WorkerCommand::SetAutoActivationPauses(
                next.auto_activation_pauses
                    .iter()
                    .filter(|rule| activation && rule.provider() == Some(provider))
                    .cloned()
                    .collect(),
            ),
            WorkerCommand::SetLimitRefreshInterval(Duration::from_secs(
                next.limit_refresh_interval.seconds(),
            )),
            WorkerCommand::SetUsageRefreshInterval(Duration::from_secs(
                next.usage_refresh_interval.seconds(),
            )),
            WorkerCommand::SetHistoryRetentionDays(next.history_retention_days),
            WorkerCommand::SetUsageCollectionEnabled(next.usage_stats_enabled),
        ] {
            let _ = sender.send(command);
        }
    }
    Ok(())
}
