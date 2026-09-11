//! Provider coordination is independent of either window and never blocks GPUI.
use std::{collections::HashMap, sync::{Arc, Mutex, mpsc::{self, Sender}}, thread, time::Duration};
use crate::{limits::ProviderLimits, notifications::{self, LimitNotificationTracker}, settings::{Settings, ProviderKind}, worker::{WorkerEvent, WorkerCommand, RequestKind}, updater::UpdatePhase};
use super::state::AppState;
#[derive(Clone)]
pub struct Snapshot {
    pub settings: Settings,
    pub limits: ProviderLimits,
    pub errors: HashMap<ProviderKind,String>,
    pub usage_errors: HashMap<ProviderKind,String>,
    pub error: Option<String>,
    pub active: Vec<(ProviderKind,RequestKind)>,
    pub update: UpdatePhase,
    pub revision: u64,
    pub settings_generation: u64,
}
pub enum Command { Settings(Box<Settings>, u64), Refresh, ClearUsage, Shutdown }
#[derive(Clone)]
pub struct Runtime { pub snapshot: Arc<Mutex<Snapshot>>, pub commands: Sender<Command>, pub state: Arc<AppState> }
impl Runtime {
    pub fn start(state: Arc<AppState>) -> Self {
        let initial = Snapshot { settings: state.settings.clone(), limits: state.current_limits(), errors: state.startup_provider_errors.iter().cloned().collect(), usage_errors: HashMap::new(), error: None, active: Vec::new(), update: state.updates.snapshot(), revision: 1, settings_generation: 0 };
        let snapshot = Arc::new(Mutex::new(initial.clone()));
        let (commands, incoming) = mpsc::channel();
        let runtime = Self { snapshot: snapshot.clone(), commands, state: state.clone() };
        thread::spawn(move || {
            let Some(events) = state.take_worker_events() else { return; };
            let mut current = initial;
            let mut trackers = HashMap::<ProviderKind,LimitNotificationTracker>::new();
            let mut clear_generation = 0u64;
            let mut pending_clear: Option<(u64, Vec<ProviderKind>)> = None;
            let mut next_update = std::time::Instant::now() + Duration::from_secs(21600);
            loop {
                let mut changed = false;
                while let Ok(command) = incoming.try_recv() {
                    match command {
                        Command::Shutdown => { state.shutdown_worker(); return; }
                        Command::Refresh => for (_, sender) in state.worker_commands() { let _ = sender.send(WorkerCommand::Refresh); },
                        Command::Settings(next, generation) => {
                            current.settings_generation = generation;
                            let result = apply_settings(&state, &current.settings, &next);
                            match result {
                                Ok(()) => { current.settings = *next; current.error = None; },
                                Err(error) => current.error = Some(format!("Could not save settings: {error:#}")),
                            }
                            current.errors.retain(|p,_| current.settings.providers.is_enabled(*p));
                            current.usage_errors.retain(|p,_| current.settings.providers.is_enabled(*p));
                            current.active.retain(|(p,_)| current.settings.providers.is_enabled(*p));
                            trackers.retain(|p,_| current.settings.providers.is_enabled(*p));
                            changed = true;
                        }
                        Command::ClearUsage if pending_clear.is_none() => {
                            clear_generation += 1;
                            let targets = state.worker_commands().into_iter().filter_map(|(p,s)| s.send(WorkerCommand::ClearUsageData(clear_generation)).is_ok().then_some(p)).collect::<Vec<_>>();
                            state.clear_usage_snapshot();
                            if targets.is_empty() {
                                if let Err(error) = crate::store::with_store(|store| store.clear_usage_data()) { current.error = Some(error.to_string()); }
                            } else { pending_clear = Some((clear_generation,targets)); }
                            changed = true;
                        }
                        Command::ClearUsage => {}
                    }
                }
                while let Ok(event) = events.try_recv() {
                    changed = true;
                    match event {
                        WorkerEvent::ProviderLimitsUpdated(p, limits) => {
                            if !current.settings.providers.is_enabled(p) { continue; }
                            trackers.entry(p).or_default().observe(&limits, &current.settings.notifications,p);
                            state.replace_limits(p,limits); current.errors.remove(&p);
                        }
                        WorkerEvent::ProviderUsageUpdated(p,usage) => {
                            if pending_clear.is_none() && current.settings.providers.is_enabled(p) { state.replace_usage(p,usage); current.usage_errors.remove(&p); }
                        }
                        WorkerEvent::ProviderPollFailed(p,error) => { crate::logger::info(format!("{}: {error}",p.display_name())); current.errors.insert(p,error); }
                        WorkerEvent::ProviderUsageRefreshFailed(p,error) => { current.usage_errors.insert(p,error); }
                        WorkerEvent::ProviderRequestStarted(p,kind) => current.active.push((p,kind)),
                        WorkerEvent::ProviderRequestFinished(p,kind) => { if let Some(i) = current.active.iter().position(|e| *e == (p,kind)) { current.active.remove(i); } }
                        WorkerEvent::ProviderActivationSucceeded(p) => if current.settings.notifications.activation_success { notifications::show_activation_succeeded(p); },
                        WorkerEvent::ProviderActivationFailed(p,error) => { current.errors.insert(p,error); }
                        WorkerEvent::ProviderUsageDataCleared(p,generation) => {
                            if let Some((expected,targets)) = pending_clear.as_mut() {
                                if *expected == generation { targets.retain(|target| *target != p); }
                                if targets.is_empty() {
                                    for (_,sender) in state.worker_commands() { let _ = sender.send(WorkerCommand::ResumeUsageRefresh(generation)); }
                                    pending_clear = None;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                let update = state.updates.snapshot();
                if current.update != update { current.update = update; changed = true; }
                if notifications::take_toast_update_request() { if let Err(e) = crate::updater::apply_pending_update() { current.error = Some(e.to_string()); changed = true; } }
                if current.settings.check_for_updates && std::time::Instant::now() >= next_update { state.updates.check_async(true,current.settings.notifications.update_available); next_update = std::time::Instant::now() + Duration::from_secs(21600); }
                if changed {
                    current.limits = state.current_limits(); current.revision = current.revision.wrapping_add(1);
                    if let Ok(mut target) = snapshot.lock() { *target = current.clone(); }
                }
                thread::sleep(Duration::from_millis(50));
            }
        });
        runtime
    }
    pub fn read(&self) -> Snapshot { self.snapshot.lock().expect("runtime snapshot").clone() }
    pub fn send(&self, command: Command) { let _ = self.commands.send(command); }
}
fn apply_settings(state: &AppState, previous: &Settings, next: &Settings) -> anyhow::Result<()> {
    next.validate()?;
    next.save(&Settings::default_path()?)?;
    next.apply_runtime_effects()?;
    crate::theme::apply_appearance(next.theme,next.accent_color);
    if next.check_for_updates && !previous.check_for_updates { state.updates.check_async(false,next.notifications.update_available); }
    let mut restart = Vec::new();
    for (p,changed) in [
        (ProviderKind::Codex,previous.codex_path != next.codex_path),
        (ProviderKind::Claude,previous.claude_path != next.claude_path),
        (ProviderKind::Cursor,previous.cursor_path != next.cursor_path),
        (ProviderKind::OpenCodeZen, previous.opencode_zen_credentials_revision != next.opencode_zen_credentials_revision),
        (ProviderKind::OpenCodeGo, previous.opencode_go_credentials_revision != next.opencode_go_credentials_revision),
        (ProviderKind::OpenRouter,previous.openrouter_credentials_revision != next.openrouter_credentials_revision || previous.openrouter_accounts != next.openrouter_accounts),
    ] { if changed { restart.push(p); } }
    let errors = state.sync_provider_workers(next,&restart);
    for (p,error) in errors { crate::logger::info(format!("{}: {error}",p.display_name())); }
    for (p,sender) in state.worker_commands() {
        let activation = crate::provider_registry::descriptor(p).supports_activation;
        for command in [
            WorkerCommand::SetAutomaticActivation(next.automatic_activation && activation),
            WorkerCommand::SetScheduledActivations(next.scheduled_activations.iter().filter(|r| activation && r.provider() == Some(p)).cloned().collect()),
            WorkerCommand::SetAutoActivationPauses(next.auto_activation_pauses.iter().filter(|r| activation && r.provider() == Some(p)).cloned().collect()),
            WorkerCommand::SetLimitRefreshInterval(Duration::from_secs(next.limit_refresh_interval.seconds())),
            WorkerCommand::SetUsageRefreshInterval(Duration::from_secs(next.usage_refresh_interval.seconds())),
            WorkerCommand::SetHistoryRetentionDays(next.history_retention_days),
            WorkerCommand::SetUsageCollectionEnabled(next.usage_stats_enabled),
        ] { let _ = sender.send(command); }
    }
    Ok(())
}

