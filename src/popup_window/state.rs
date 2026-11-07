//! The popup's view state: the settings snapshot it renders plus what the
//! bridge learned at runtime (errors, requests, activation text).

use std::{collections::HashMap, sync::Arc};

use crate::{
    app::AppState,
    instances::ProviderId,
    limits::RateLimits,
    provider_registry::ProviderKind,
    settings::{HomeCardLayout, Settings},
    worker::RequestKind,
};

use super::formatting::format_last_activation;

#[derive(Clone, Debug)]
pub(crate) struct UiState {
    /// What the popup renders. Popup edits land here first and reach the
    /// file through the settings writer; every commit replaces it.
    pub(crate) settings: Arc<Settings>,
    /// Derived from `settings`; computing it reads the credential vault.
    usage_stats_providers: Vec<ProviderId>,
    pub(crate) last_activation: String,
    pub(crate) provider_errors: HashMap<ProviderId, String>,
    pub(crate) usage_errors: HashMap<ProviderId, String>,
    pub(crate) error: Option<String>,
    /// Changes for every successful usage refresh or usage-data clear. Usage
    /// snapshot memoization keys off this revision rather than quota polls.
    pub(crate) usage_revision: u64,
    /// Provider limit/usage requests currently in flight. The refresh icon
    /// stays active until every operation started by the workers has finished.
    pub(crate) active_requests: Vec<(ProviderId, RequestKind)>,
    pub(crate) refreshing: bool,
    pub(crate) update_version: Option<String>,
}

impl Default for UiState {
    fn default() -> Self {
        Self::new(Settings::default())
    }
}

impl UiState {
    pub(crate) fn new(settings: Settings) -> Self {
        Self {
            usage_stats_providers: settings.usage_stats_providers(),
            settings: Arc::new(settings),
            last_activation: crate::i18n::tr("never").into(),
            provider_errors: HashMap::new(),
            usage_errors: HashMap::new(),
            error: None,
            usage_revision: 0,
            active_requests: Vec::new(),
            refreshing: false,
            update_version: None,
        }
    }

    /// The state both the popup's first frame and the bridge start from.
    pub(crate) fn initial(state: &AppState) -> Self {
        let mut ui = Self::new(state.settings.clone());
        ui.provider_errors = state
            .startup_provider_errors
            .iter()
            .map(|(provider, error)| (*provider, Self::error_for_ui(error)))
            .collect();
        ui.last_activation =
            format_last_activation(&RateLimits::default(), state.last_activation_at);
        ui.update_version = state
            .updates
            .available_update()
            .map(|update| update.version);
        ui
    }

    /// Replaces the snapshot with a committed one.
    pub(crate) fn set_settings(&mut self, settings: Settings) {
        self.usage_stats_providers = settings.usage_stats_providers();
        self.settings = Arc::new(settings);
    }

    /// Applies an edit to the snapshot at once and queues what it changed
    /// for the settings file. Returns the snapshot before the edit.
    pub(crate) fn edit_settings(&mut self, edit: impl FnOnce(&mut Settings)) -> Settings {
        let before = crate::settings::store::edit_snapshot(Arc::make_mut(&mut self.settings), edit);
        if self.settings.instances != before.instances {
            self.usage_stats_providers = self.settings.usage_stats_providers();
        }
        before
    }

    pub(super) fn compact_bar_visible(&self) -> bool {
        self.settings.popup_floating || self.settings.show_popup_action_bar
    }

    pub(super) fn home_card_layout(&self, provider: ProviderId) -> HomeCardLayout {
        self.settings
            .popup_home_card_layouts
            .get(provider.id())
            .copied()
            .unwrap_or_default()
    }

    /// Enabled instances of one driver, in display order.
    pub(super) fn enabled_instances_of(&self, driver: ProviderKind) -> Vec<ProviderId> {
        self.settings
            .instances
            .iter()
            .filter(|instance| instance.enabled && instance.driver == driver)
            .map(|instance| instance.provider_id())
            .collect()
    }

    /// Credential revision of the running worker's instance.
    pub(crate) fn credentials_revision(&self, provider: ProviderId) -> u64 {
        self.settings
            .instance(provider)
            .map_or(0, |instance| instance.credentials_revision)
    }

    /// Turn transport/status chains into short product-facing messages. The
    /// original diagnostic is retained by the logging paths, never in this UI.
    pub(crate) fn error_for_ui(error: &str) -> String {
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
    pub(crate) fn set_popup_error(&mut self, error: impl Into<String>) {
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
    pub(crate) fn set_provider_error(&mut self, provider: ProviderId, error: impl Into<String>) {
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

    pub(crate) fn clear_provider_error(&mut self, provider: ProviderId) {
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

    pub(crate) fn set_usage_error(&mut self, provider: ProviderId, error: impl Into<String>) {
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

    pub(crate) fn clear_usage_error(&mut self, provider: ProviderId) {
        self.usage_errors.remove(&provider);
    }

    pub(super) fn usage_stats_provider_enabled(&self, provider: ProviderId) -> bool {
        self.usage_stats_providers.contains(&provider)
    }

    /// Collected and counted toward the Usage tab and Home total spend.
    pub(super) fn usage_overview_included(&self, provider: ProviderId) -> bool {
        self.usage_stats_provider_enabled(provider)
            && self
                .settings
                .instances
                .iter()
                .any(|instance| instance.provider_id() == provider && instance.in_usage_overview)
    }

    pub(crate) fn request_started(&mut self, provider: ProviderId, kind: RequestKind) {
        self.active_requests.push((provider, kind));
        self.refreshing = true;
    }

    pub(crate) fn request_finished(&mut self, provider: ProviderId, kind: RequestKind) {
        if let Some(index) = self
            .active_requests
            .iter()
            .position(|active| *active == (provider, kind))
        {
            self.active_requests.remove(index);
        }
        self.refreshing = !self.active_requests.is_empty();
    }

    pub(crate) fn clear_provider_requests(&mut self, provider: ProviderId) {
        self.active_requests
            .retain(|(active_provider, _)| *active_provider != provider);
        self.refreshing = !self.active_requests.is_empty();
    }
}
