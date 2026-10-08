use super::*;

pub(super) fn refresh_all_workers(commands: &[(ProviderId, Sender<WorkerCommand>)]) -> bool {
    let mut requested = false;
    for (_, commands) in commands {
        requested |= commands.send(WorkerCommand::Refresh).is_ok();
    }
    requested
}

/// Publish view state to the GPUI popup. Hidden popups only store it; the
/// renderer repaints when it is shown, so background polls stay cheap.
pub(super) fn publish_popup_ui(ui: &UiState) {
    super::publish_ui(ui);
}

/// Same as [`publish_popup_ui`]; kept separate to mark paths that must reach
/// the renderer before the next show.
pub(super) fn flush_popup_ui(ui: &UiState) {
    super::publish_ui(ui);
}

/// Flatten provider usage errors, including per-account errors exposed by
/// OpenRouter, into the provider-scoped presentation used by the popup.
pub(super) fn usage_error_message(statistics: &crate::usage::UsageStatistics) -> Option<String> {
    let mut errors = Vec::new();
    let mut add_error = |error: Option<&str>| {
        let Some(error) = error else {
            return;
        };
        let error = error.trim();
        if !error.is_empty() && !errors.iter().any(|seen| seen == error) {
            errors.push(error.to_owned());
        }
    };
    add_error(statistics.error.as_deref());
    for account in statistics.accounts.values() {
        add_error(account.error.as_deref());
    }
    (!errors.is_empty()).then(|| errors.join("; "))
}

/// Shared startup state handed from `main` into the reactor render tree.
pub struct AppState {
    pub settings: Settings,
    /// The sole live rate-limit snapshot. Both the tray and popup read this
    /// store, and worker results replace it atomically before either surface
    /// is repainted.
    pub limits: Mutex<ProviderLimits>,
    /// Latest valid public forced-reset announcements. Kept outside UiState so
    /// hidden popups avoid a full native-tree publish on every feed poll.
    pub forced_resets: Mutex<Vec<crate::reset_feed::ForcedReset>>,
    pub commands: Mutex<HashMap<ProviderId, Sender<WorkerCommand>>>,
    pub workers: Mutex<crate::provider::ProviderWorkers>,
    pub worker_events_rx: Mutex<Option<Receiver<WorkerEvent>>>,
    pub worker_events_tx: Sender<WorkerEvent>,
    /// Independent public-feed worker for announced Codex forced resets.
    pub reset_feed_worker: Mutex<Option<crate::reset_feed::ResetFeedWorker>>,
    pub activation_path: std::path::PathBuf,
    /// Provider-scoped errors from workers that could not be created at startup.
    /// They remain visible until that provider returns a successful limits
    /// response, just like errors received from a running worker.
    pub startup_provider_errors: Vec<(ProviderId, String)>,
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
    pub(super) fn current_limits(&self) -> ProviderLimits {
        self.limits
            .lock()
            .map(|limits| limits.clone())
            .unwrap_or_default()
    }

    /// Overlay locally chosen OpenRouter account names onto the live quota
    /// snapshot. A rename is a settings-only change and must not wait for the
    /// next worker poll or an app restart.
    pub(super) fn apply_openrouter_account_names(&self, settings: &Settings) -> bool {
        let mut changed = false;
        for instance in settings
            .instances
            .iter()
            .filter(|instance| instance.driver == ProviderKind::OpenRouter)
        {
            let provider = instance.provider_id();
            let mut limits = self.current_limits().get(provider).clone();
            if crate::openrouter::apply_account_names(&mut limits, instance) {
                self.replace_limits(provider, limits);
                changed = true;
            }
        }
        changed
    }

    pub(super) fn replace_limits(&self, provider: ProviderId, mut limits: RateLimits) {
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

    pub(super) fn replace_usage(&self, provider: ProviderId, usage: crate::usage::UsageStatistics) {
        if let Ok(mut current) = self.limits.lock() {
            current.get_mut(provider).usage = usage;
        }
    }

    pub(super) fn clear_usage_snapshot(&self) {
        if let Ok(mut limits) = self.limits.lock() {
            let providers = limits
                .iter()
                .map(|(provider, _)| provider)
                .collect::<Vec<_>>();
            for provider in providers {
                limits.get_mut(provider).usage = crate::usage::UsageStatistics::default();
            }
        }
    }

    pub(super) fn take_worker_events(&self) -> Option<Receiver<WorkerEvent>> {
        self.worker_events_rx.lock().ok()?.take()
    }

    pub(super) fn worker_commands(&self) -> Vec<(ProviderId, Sender<WorkerCommand>)> {
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

    pub(super) fn current_forced_resets(&self) -> Vec<crate::reset_feed::ForcedReset> {
        self.forced_resets
            .lock()
            .map(|resets| resets.clone())
            .unwrap_or_default()
    }

    pub(super) fn replace_forced_resets(&self, resets: Vec<crate::reset_feed::ForcedReset>) {
        if let Ok(mut current) = self.forced_resets.lock() {
            *current = resets;
        }
    }

    pub(super) fn sync_reset_feed(&self, settings: &Settings) {
        let Ok(worker) = self.reset_feed_worker.lock() else {
            return;
        };
        let Some(worker) = worker.as_ref() else {
            return;
        };
        let _ = worker
            .commands
            .send(crate::reset_feed::ResetFeedCommand::SetEnabled(
                settings.notifications.forced_reset_feed_enabled,
            ));
        let _ = worker
            .commands
            .send(crate::reset_feed::ResetFeedCommand::SetRefreshInterval(
                Duration::from_secs(settings.reset_announcement_refresh_interval.seconds()),
            ));
    }

    pub(super) fn mark_forced_reset_info_notified(&self, id: String) {
        let Ok(worker) = self.reset_feed_worker.lock() else {
            return;
        };
        if let Some(worker) = worker.as_ref() {
            let _ = worker
                .commands
                .send(crate::reset_feed::ResetFeedCommand::MarkNotified(id));
        }
    }

    /// Applies instance toggles without disturbing workers that remain
    /// enabled. Workers of removed or disabled instances stop, and instances
    /// listed in `restart` get a fresh worker.
    pub(super) fn sync_provider_workers(
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
            let cached_limits = self.current_limits();
            match crate::provider::start_provider_worker_with_limits(
                provider,
                settings,
                self.activation_path.clone(),
                self.worker_events_tx.clone(),
                Some(cached_limits.get(provider)),
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
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiState {
    pub(super) theme: AppTheme,
    pub(super) accent_color: AccentColor,
    pub(super) font_family: Option<String>,
    pub(super) animations_enabled: bool,
    pub(super) popup_background_material: PopupBackgroundMaterial,
    pub(super) popup_theme: PopupTheme,
    pub(super) time_format: TimeFormat,
    pub(super) last_activation: String,
    pub(super) provider_errors: HashMap<ProviderId, String>,
    pub(super) usage_errors: HashMap<ProviderId, String>,
    pub(super) error: Option<String>,
    /// Changes for every settings transaction so layout-sensitive toggles
    /// force a fresh body measurement and popup resize.
    pub(super) settings_revision: u64,
    /// Changes for every successful worker sample.  Rate-limit data lives only
    /// in `AppState`, but this revision makes that external snapshot observable
    /// to the reactive render loop even when all other view metadata is equal.
    pub(super) limits_revision: u64,
    /// Changes for every successful usage refresh or usage-data clear. Usage
    /// snapshot memoization keys off this revision rather than quota polls.
    pub(super) usage_revision: u64,
    /// Same bridge for the independently polled forced-reset feed.
    pub(super) forced_resets_revision: u64,
    /// Provider limit/usage requests currently in flight. The refresh icon
    /// stays active until every operation started by the workers has finished.
    pub(super) active_requests: Vec<(ProviderId, RequestKind)>,
    pub(super) refreshing: bool,
    pub(super) show_used_percentage: bool,
    pub(super) show_usage_values: bool,
    pub(super) show_usage_pace: bool,
    pub(super) compact_usage_cards: bool,
    pub(super) popup_visibility: PopupVisibility,
    pub(super) usage_stats_enabled: bool,
    /// Instances whose usage statistics are collected and shown.
    pub(super) usage_stats_providers: Vec<ProviderId>,
    pub(super) show_total_spend_on_all_tab: bool,
    pub(super) total_spend_presentation: TotalSpendPresentation,
    pub(super) total_spend_period: TotalSpendPeriod,
    pub(super) show_account_name: bool,
    /// Every configured instance in display order, enabled or not.
    pub(super) instances: Vec<ProviderInstance>,
    pub(super) popup_tab_mode: PopupTabMode,
    /// Driver id -> instance id selected in a grouped tab's switcher.
    pub(super) grouped_tab_selection: std::collections::BTreeMap<String, String>,
    pub(super) popup_two_columns: bool,
    pub(super) popup_home_order: Vec<HomeWidgetId>,
    pub(super) popup_home_right_column: Option<Vec<HomeWidgetId>>,
    pub(super) popup_home_card_layouts: std::collections::BTreeMap<String, HomeCardLayout>,
    pub(super) use_colored_provider_icons: bool,
    pub(super) replace_chatgpt_logo_with_codex: bool,
    pub(super) update_version: Option<String>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            theme: AppTheme::Auto,
            accent_color: AccentColor::Windows,
            font_family: None,
            animations_enabled: true,
            popup_background_material: PopupBackgroundMaterial::Mica,
            popup_theme: PopupTheme::Fluent,
            time_format: TimeFormat::from_windows(),
            last_activation: crate::i18n::tr("never").into(),
            provider_errors: HashMap::new(),
            usage_errors: HashMap::new(),
            error: None,
            settings_revision: 0,
            limits_revision: 0,
            usage_revision: 0,
            forced_resets_revision: 0,
            active_requests: Vec::new(),
            refreshing: false,
            show_used_percentage: false,
            show_usage_values: true,
            show_usage_pace: true,
            compact_usage_cards: true,
            popup_visibility: PopupVisibility::build_defaults(),
            usage_stats_enabled: true,
            usage_stats_providers: Vec::new(),
            show_total_spend_on_all_tab: true,
            total_spend_presentation: TotalSpendPresentation::default(),
            total_spend_period: TotalSpendPeriod::default(),
            show_account_name: false,
            instances: Settings::default().instances,
            popup_tab_mode: PopupTabMode::default(),
            grouped_tab_selection: Default::default(),
            popup_two_columns: false,
            popup_home_order: Vec::new(),
            popup_home_right_column: None,
            popup_home_card_layouts: Default::default(),
            use_colored_provider_icons: true,
            replace_chatgpt_logo_with_codex: false,
            update_version: None,
        }
    }
}

impl UiState {
    /// Shared seed for both the render tree and the background bridge. The
    /// bridge publishes a complete snapshot, so it must restore layout and
    /// effective usage eligibility before either path can produce a first frame.
    pub(super) fn popup_layout_from_settings(settings: &Settings) -> Self {
        Self {
            usage_stats_providers: settings.usage_stats_providers(),
            instances: settings.instances.clone(),
            popup_tab_mode: settings.popup_tab_mode,
            grouped_tab_selection: settings.grouped_tab_selection.clone(),
            popup_two_columns: settings.popup_two_columns,
            popup_home_order: settings.popup_home_order.clone(),
            popup_home_right_column: settings.popup_home_right_column.clone(),
            popup_home_card_layouts: settings.popup_home_card_layouts.clone(),
            ..Self::default()
        }
    }

    /// Copies every settings-owned popup field. Runtime state (errors,
    /// requests, revisions, update banner) is left untouched.
    pub(super) fn apply_settings(&mut self, settings: &Settings) {
        self.theme = settings.theme;
        self.accent_color = settings.accent_color;
        self.font_family = settings.font_family.clone();
        self.animations_enabled = settings.animations_enabled;
        self.popup_background_material = settings.popup_background_material;
        self.popup_theme = settings.popup_theme;
        self.time_format = settings.time_format;
        self.show_used_percentage = settings.show_used_percentage;
        self.show_usage_values = settings.show_usage_values;
        self.show_usage_pace = settings.show_usage_pace;
        self.compact_usage_cards = settings.compact_usage_cards;
        self.popup_visibility = settings.popup_visibility.clone();
        self.usage_stats_enabled = settings.usage_stats_enabled;
        self.usage_stats_providers = settings.usage_stats_providers();
        self.show_total_spend_on_all_tab = settings.show_total_spend_on_all_tab;
        self.total_spend_presentation = settings.total_spend_presentation;
        self.total_spend_period = settings.total_spend_period;
        self.show_account_name = settings.show_account_name;
        self.instances = settings.instances.clone();
        self.popup_tab_mode = settings.popup_tab_mode;
        self.grouped_tab_selection = settings.grouped_tab_selection.clone();
        self.popup_two_columns = settings.popup_two_columns;
        self.popup_home_order = settings.popup_home_order.clone();
        self.popup_home_right_column = settings.popup_home_right_column.clone();
        self.popup_home_card_layouts = settings.popup_home_card_layouts.clone();
        self.use_colored_provider_icons = settings.use_colored_provider_icons;
        self.replace_chatgpt_logo_with_codex = settings.replace_chatgpt_logo_with_codex;
    }

    pub(super) fn home_card_layout(&self, provider: ProviderId) -> HomeCardLayout {
        self.popup_home_card_layouts
            .get(provider.id())
            .copied()
            .unwrap_or_default()
    }

    pub(super) fn instance(&self, provider: ProviderId) -> Option<&ProviderInstance> {
        self.instances
            .iter()
            .find(|instance| instance.id == provider.id())
    }

    /// Enabled instances in display order.
    pub(super) fn enabled_providers(&self) -> Vec<ProviderId> {
        self.instances
            .iter()
            .filter(|instance| instance.enabled)
            .map(ProviderInstance::provider_id)
            .collect()
    }

    pub(super) fn provider_enabled(&self, provider: ProviderId) -> bool {
        self.instance(provider)
            .is_some_and(|instance| instance.enabled)
    }

    /// Enabled instances of one driver, in display order.
    pub(super) fn enabled_instances_of(&self, driver: ProviderKind) -> Vec<ProviderId> {
        self.instances
            .iter()
            .filter(|instance| instance.enabled && instance.driver == driver)
            .map(ProviderInstance::provider_id)
            .collect()
    }

    /// Credential revision of the running worker's instance.
    pub(super) fn credentials_revision(&self, provider: ProviderId) -> u64 {
        self.instance(provider)
            .map_or(0, |instance| instance.credentials_revision)
    }

    /// Turn transport/status chains into short product-facing messages. The
    /// original diagnostic is retained by the logging paths, never in this UI.
    pub(super) fn error_for_ui(error: &str) -> String {
        let lower = error.to_ascii_lowercase();
        let tokens = lower
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|token| !token.is_empty())
            .collect::<Vec<_>>();
        let has_status = |code: &str| {
            tokens.windows(2).any(|pair| {
                (pair[1] == code && ["status", "code", "http"].contains(&pair[0]))
                    || (pair[0] == code && ["forbidden", "unauthorized"].contains(&pair[1]))
            })
        };
        let mut messages = Vec::new();
        let mut add = |message: &'static str| {
            if !messages.contains(&message) {
                messages.push(message);
            }
        };
        let timeout = lower.contains("timed out")
            || lower.contains("timeout")
            || lower.contains("os error 10060")
            || has_status("408")
            || has_status("504");
        let certificate = lower.contains("certificate") || lower.contains("unknownissuer");
        let closed = lower.contains("os error 10054")
            || lower.contains("connection reset")
            || lower.contains("forcibly closed")
            || lower.contains("connection closed");
        let dns = lower.contains("dns")
            || lower.contains("no such host")
            || lower.contains("name resolution")
            || lower.contains("os error 11001");
        if timeout {
            add(crate::i18n::english(
                "the-request-timed-out-try-refreshing-again",
            ));
        }
        if certificate {
            add(crate::i18n::english(
                "the-secure-connection-could-not-be-verified-see-log-for-details",
            ));
        }
        if closed && !timeout {
            add(crate::i18n::english(
                "the-provider-closed-the-connection-try-refreshing-again",
            ));
        }
        if dns {
            add(crate::i18n::english(
                "the-provider-s-address-could-not-be-resolved-check-your-connectio",
            ));
        }
        if !timeout
            && !certificate
            && !closed
            && !dns
            && (lower.contains("connection failed")
                || lower.contains("connect error")
                || lower.contains("network error")
                || lower.contains("connection refused")
                || lower.contains("tls connection init failed"))
        {
            add(crate::i18n::english(
                "could-not-connect-to-the-provider-check-your-connection-and-try-a",
            ));
        }
        if lower.contains("save a valid management key") {
            add(crate::i18n::english(
                "the-management-key-was-rejected-update-it-in-settings",
            ));
        } else if has_status("401") {
            add(crate::i18n::english(
                "authentication-failed-sign-in-again-or-update-the-provider-key",
            ));
        }
        if has_status("403") {
            add(crate::i18n::english(
                "access-denied-by-the-provider-http-403",
            ));
        }
        if has_status("429")
            || lower.contains("rate limited")
            || lower.contains("too many requests")
        {
            add(crate::i18n::english(
                "too-many-requests-wait-a-few-minutes-before-refreshing-again",
            ));
        }
        if ["500", "502", "503"].iter().any(|code| has_status(code)) {
            add(crate::i18n::english(
                "the-provider-is-temporarily-unavailable-try-again-later",
            ));
        }
        if has_status("400") {
            add(crate::i18n::english(
                "the-provider-rejected-the-request-see-log-for-details",
            ));
        }
        if has_status("404") {
            add(crate::i18n::english(
                "the-requested-resource-was-not-found-see-log-for-details",
            ));
        }
        if lower.contains("parse ")
            || lower.contains("invalid json")
            || lower.contains("missing rows")
            || lower.contains("incomplete results")
        {
            add(crate::i18n::english(
                "the-provider-returned-an-unexpected-response-try-refreshing-again",
            ));
        }
        if !messages.is_empty() {
            return messages.join("\n");
        }
        // Keep short actionable domain messages. Unknown technical dumps need
        // a useful fallback instead of another URL/stack-trace wall of text.
        if error.chars().count() > 220
            || lower.contains("https://")
            || lower.contains("http://")
            || lower.contains("os error")
            || lower.contains("<html")
        {
            return crate::i18n::english("the-request-failed-see-log-for-details").into();
        }
        error.trim().into()
    }

    /// Shows an app-level error in the popup and records it once per distinct
    /// message. Provider polling uses [`set_provider_error`] instead so one
    /// provider cannot overwrite another provider's diagnostic state.
    pub(super) fn set_popup_error(&mut self, error: impl Into<String>) {
        let raw_error = error.into();
        let display_error = Self::error_for_ui(&raw_error);
        if self.error.as_deref() != Some(display_error.as_str()) {
            crate::logger::info(format!("Popup error: {raw_error}"));
        }
        self.error = Some(display_error);
    }

    /// Retain the latest provider error until that provider produces a
    /// successful limits response. A changing message updates the detail while
    /// keeping the red marker continuously visible.
    pub(super) fn set_provider_error(&mut self, provider: ProviderId, error: impl Into<String>) {
        let raw_error = error.into();
        let display_error = Self::error_for_ui(&raw_error);
        if self.provider_errors.get(&provider) != Some(&display_error) {
            crate::logger::info(format!(
                "{} provider error: {raw_error}",
                provider.display_name()
            ));
        }
        self.provider_errors.insert(provider, display_error);
    }

    pub(super) fn clear_provider_error(&mut self, provider: ProviderId) {
        self.provider_errors.remove(&provider);
    }

    /// Return the latest error from either the provider's quota poll or its
    /// usage/analytics refresh. Both are provider failures in the popup UI,
    /// but they have independent lifetimes so a healthy quota poll cannot
    /// hide a still-failing analytics request.
    pub(super) fn provider_error(&self, provider: ProviderId) -> Option<&str> {
        if let Some(error) = self.provider_errors.get(&provider) {
            Some(error.as_str())
        } else {
            self.usage_errors.get(&provider).map(String::as_str)
        }
    }

    pub(super) fn has_provider_error(&self, provider: ProviderId) -> bool {
        self.provider_errors.contains_key(&provider) || self.usage_errors.contains_key(&provider)
    }

    pub(super) fn set_usage_error(&mut self, provider: ProviderId, error: impl Into<String>) {
        let raw_error = error.into();
        let display_error = Self::error_for_ui(&raw_error);
        if self.usage_errors.get(&provider) != Some(&display_error) {
            crate::logger::info(format!(
                "{} provider usage error: {raw_error}",
                provider.display_name()
            ));
        }
        self.usage_errors.insert(provider, display_error);
    }

    pub(super) fn clear_usage_error(&mut self, provider: ProviderId) {
        self.usage_errors.remove(&provider);
    }

    pub(super) fn usage_stats_provider_enabled(&self, provider: ProviderId) -> bool {
        self.usage_stats_providers.contains(&provider)
    }

    /// Collected and counted toward the Usage tab and Home total spend.
    pub(super) fn usage_overview_included(&self, provider: ProviderId) -> bool {
        self.usage_stats_provider_enabled(provider)
            && self
                .instances
                .iter()
                .any(|instance| instance.provider_id() == provider && instance.in_usage_overview)
    }

    pub(super) fn request_started(&mut self, provider: ProviderId, kind: RequestKind) {
        self.active_requests.push((provider, kind));
        self.refreshing = true;
    }

    pub(super) fn request_finished(&mut self, provider: ProviderId, kind: RequestKind) {
        if let Some(index) = self
            .active_requests
            .iter()
            .position(|active| *active == (provider, kind))
        {
            self.active_requests.remove(index);
        }
        self.refreshing = !self.active_requests.is_empty();
    }

    pub(super) fn clear_provider_requests(&mut self, provider: ProviderId) {
        self.active_requests
            .retain(|(active_provider, _)| *active_provider != provider);
        self.refreshing = !self.active_requests.is_empty();
    }
}

impl UiState {
    /// Marks the shared rate-limit snapshot as changed so `AsyncSetState` does
    /// not discard an otherwise identical UI state as a no-op.
    pub(super) fn observe_limits_update(&mut self) {
        self.limits_revision = self.limits_revision.wrapping_add(1);
    }

    pub(super) fn observe_usage_update(&mut self) {
        self.usage_revision = self.usage_revision.wrapping_add(1);
    }

    pub(super) fn observe_forced_resets_update(&mut self) {
        self.forced_resets_revision = self.forced_resets_revision.wrapping_add(1);
    }
}
