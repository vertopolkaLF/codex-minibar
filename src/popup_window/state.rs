use super::*;

pub(super) fn refresh_all_workers(commands: &[(ProviderKind, Sender<WorkerCommand>)]) -> bool {
    let mut requested = false;
    for (_, commands) in commands {
        requested |= commands.send(WorkerCommand::Refresh).is_ok();
    }
    requested
}

/// Hidden tray popups must not rebuild their WinUI tree on every provider poll.
/// Remounting unmanaged SwapChainPanel/XAML children steadily grows the
/// compositor working set (observed multi-GB after long idle runs).
pub(super) fn popup_ui_should_publish() -> bool {
    popup::is_visible() || crate::settings_window::is_open()
}

pub(super) fn publish_popup_ui(set_ui: &AsyncSetState<UiState>, ui: &UiState) {
    if popup_ui_should_publish() {
        set_ui.call(ui.clone());
    }
}

/// Push the latest view state before a show so the first frame is current even
/// after a stretch of suppressed background polls.
pub(super) fn flush_popup_ui(set_ui: &AsyncSetState<UiState>, ui: &UiState) {
    set_ui.call(ui.clone());
}

/// Shared startup state handed from `main` into the reactor render tree.
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
    pub startup_error: Option<String>,
    /// Last activation attempt loaded from persisted activation state.
    pub last_activation_at: Option<DateTime<Utc>>,
    /// Live settings pushes from the settings window; drained by the tray bridge.
    pub settings_rx: Mutex<Option<Receiver<Settings>>>,
    pub settings_tx: Sender<Settings>,
    pub updates: Arc<UpdateController>,
}

impl AppState {
    pub(super) fn current_limits(&self) -> ProviderLimits {
        self.limits
            .lock()
            .map(|limits| limits.clone())
            .unwrap_or_default()
    }

    pub(super) fn replace_limits(&self, provider: ProviderKind, mut limits: RateLimits) {
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

    pub(super) fn replace_usage(
        &self,
        provider: ProviderKind,
        usage: crate::usage::UsageStatistics,
    ) {
        if let Ok(mut current) = self.limits.lock() {
            current.get_mut(provider).usage = usage;
        }
    }

    pub(super) fn take_worker_events(&self) -> Option<Receiver<WorkerEvent>> {
        self.worker_events_rx.lock().ok()?.take()
    }

    pub(super) fn worker_commands(&self) -> Vec<(ProviderKind, Sender<WorkerCommand>)> {
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
    pub(super) fn sync_provider_workers(
        &self,
        settings: &Settings,
        restart: &[ProviderKind],
    ) -> Vec<String> {
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
                Err(error) => errors.push(format!("{}: {error:#}", provider.display_name())),
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

#[derive(Clone, Debug, PartialEq)]
pub(super) struct UiState {
    pub(super) theme: AppTheme,
    pub(super) accent_color: AccentColor,
    pub(super) animations_enabled: bool,
    pub(super) time_format: TimeFormat,
    pub(super) last_activation: String,
    pub(super) error: Option<String>,
    /// Changes for every successful worker sample.  Rate-limit data lives only
    /// in `AppState`, but this revision makes that external snapshot observable
    /// to the reactive render loop even when all other view metadata is equal.
    pub(super) limits_revision: u64,
    /// Provider limit/usage requests currently in flight. The refresh icon
    /// stays active until every operation started by the workers has finished.
    pub(super) active_requests: Vec<(ProviderKind, RequestKind)>,
    pub(super) refreshing: bool,
    pub(super) show_used_percentage: bool,
    pub(super) show_usage_pace: bool,
    pub(super) popup_visibility: PopupVisibility,
    pub(super) show_total_spend_on_all_tab: bool,
    pub(super) total_spend_presentation: TotalSpendPresentation,
    pub(super) total_spend_period: TotalSpendPeriod,
    pub(super) show_account_name: bool,
    pub(super) codex_enabled: bool,
    pub(super) claude_enabled: bool,
    pub(super) cursor_enabled: bool,
    pub(super) opencode_zen_enabled: bool,
    pub(super) opencode_go_enabled: bool,
    pub(super) openrouter_enabled: bool,
    pub(super) opencode_zen_credentials_revision: u64,
    pub(super) opencode_go_credentials_revision: u64,
    pub(super) openrouter_credentials_revision: u64,
    pub(super) popup_order: Vec<PopupWidgetKind>,
    pub(super) use_colored_provider_icons: bool,
    pub(super) replace_chatgpt_logo_with_codex: bool,
    pub(super) codex_path: Option<std::path::PathBuf>,
    pub(super) claude_path: Option<std::path::PathBuf>,
    pub(super) cursor_path: Option<std::path::PathBuf>,
    pub(super) update_version: Option<String>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            theme: AppTheme::Auto,
            accent_color: AccentColor::Windows,
            animations_enabled: true,
            time_format: TimeFormat::from_windows(),
            last_activation: "Never".into(),
            error: None,
            limits_revision: 0,
            active_requests: Vec::new(),
            refreshing: false,
            show_used_percentage: false,
            show_usage_pace: true,
            popup_visibility: PopupVisibility::build_defaults(),
            show_total_spend_on_all_tab: true,
            total_spend_presentation: TotalSpendPresentation::default(),
            total_spend_period: TotalSpendPeriod::default(),
            show_account_name: false,
            codex_enabled: true,
            claude_enabled: false,
            cursor_enabled: false,
            opencode_zen_enabled: false,
            opencode_go_enabled: false,
            opencode_zen_credentials_revision: 0,
            opencode_go_credentials_revision: 0,
            openrouter_enabled: false,
            openrouter_credentials_revision: 0,
            popup_order: PopupWidgetKind::default_order(),
            use_colored_provider_icons: false,
            replace_chatgpt_logo_with_codex: false,
            codex_path: None,
            claude_path: None,
            cursor_path: None,
            update_version: None,
        }
    }
}

impl UiState {
    /// Shows an error in the popup and records it once per distinct message.
    /// Polling can retry many times a second; duplicate popup errors would make
    /// the diagnostic log unreadable.
    pub(super) fn set_popup_error(&mut self, error: impl Into<String>) {
        let error = error.into();
        if self.error.as_deref() != Some(error.as_str()) {
            crate::logger::info(format!("Popup error: {error}"));
        }
        self.error = Some(error);
    }

    pub(super) fn request_started(&mut self, provider: ProviderKind, kind: RequestKind) {
        self.active_requests.push((provider, kind));
        self.refreshing = true;
    }

    pub(super) fn request_finished(&mut self, provider: ProviderKind, kind: RequestKind) {
        if let Some(index) = self
            .active_requests
            .iter()
            .position(|active| *active == (provider, kind))
        {
            self.active_requests.remove(index);
        }
        self.refreshing = !self.active_requests.is_empty();
    }
}

impl UiState {
    /// Marks the shared rate-limit snapshot as changed so `AsyncSetState` does
    /// not discard an otherwise identical UI state as a no-op.
    pub(super) fn observe_limits_update(&mut self) {
        self.limits_revision = self.limits_revision.wrapping_add(1);
    }
}
