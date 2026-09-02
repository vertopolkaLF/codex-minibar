use super::*;

#[derive(Clone)]
pub(super) struct SettingsWindowState {
    pub(super) theme: SetState<AppTheme>,
    pub(super) accent_color: SetState<AccentColor>,
    pub(super) animations_enabled: SetState<bool>,
    pub(super) time_format: SetState<TimeFormat>,
    pub(super) codex_enabled: SetState<bool>,
    pub(super) claude_enabled: SetState<bool>,
    pub(super) cursor_enabled: SetState<bool>,
    pub(super) opencode_zen_enabled: SetState<bool>,
    pub(super) opencode_go_enabled: SetState<bool>,
    pub(super) openrouter_enabled: SetState<bool>,
    pub(super) openrouter_accounts: SetState<Vec<OpenRouterAccount>>,
    pub(super) codex_path: SetState<String>,
    pub(super) claude_path: SetState<String>,
    pub(super) cursor_path: SetState<String>,
    pub(super) popup_order: SetState<Vec<PopupWidgetKind>>,
    pub(super) use_colored_provider_icons: SetState<bool>,
    pub(super) use_colored_sidebar_icons: SetState<bool>,
    pub(super) replace_chatgpt_logo_with_codex: SetState<bool>,
    pub(super) automatic_activation: SetState<bool>,
    pub(super) scheduled_activations: SetState<Vec<ScheduledActivation>>,
    pub(super) auto_activation_pauses: SetState<Vec<AutoActivationPause>>,
    pub(super) limit_refresh_interval: SetState<LimitRefreshInterval>,
    pub(super) start_at_login: SetState<bool>,
    pub(super) show_used_percentage: SetState<bool>,
    pub(super) show_usage_pace: SetState<bool>,
    pub(super) compact_usage_cards: SetState<bool>,
    pub(super) popup_visibility: SetState<PopupVisibility>,
    pub(super) discovered_popup_bricks: SetState<BTreeMap<String, String>>,
    pub(super) show_total_spend_on_all_tab: SetState<bool>,
    pub(super) total_spend_presentation: SetState<TotalSpendPresentation>,
    pub(super) show_account_name: SetState<bool>,
    pub(super) activation_success: SetState<bool>,
    pub(super) activation_failure: SetState<bool>,
    pub(super) limits_reset: SetState<bool>,
    pub(super) low_usage_enabled: SetState<bool>,
    pub(super) low_usage_threshold: SetState<u8>,
    pub(super) weekly_low_usage_enabled: SetState<bool>,
    pub(super) weekly_low_usage_threshold: SetState<u8>,
    pub(super) tray_widgets: SetState<Vec<TrayWidget>>,
    pub(super) check_for_updates: SetState<bool>,
    pub(super) notify_on_update: SetState<bool>,
}

impl SettingsWindowState {
    pub(super) fn apply(&self, settings: &Settings) {
        self.theme.call(settings.theme);
        self.accent_color.call(settings.accent_color);
        self.animations_enabled.call(settings.animations_enabled);
        self.time_format.call(settings.time_format);
        self.codex_enabled
            .call(settings.providers.is_enabled(ProviderKind::Codex));
        self.claude_enabled
            .call(settings.providers.is_enabled(ProviderKind::Claude));
        self.cursor_enabled
            .call(settings.providers.is_enabled(ProviderKind::Cursor));
        self.opencode_zen_enabled
            .call(settings.providers.is_enabled(ProviderKind::OpenCodeZen));
        self.opencode_go_enabled
            .call(settings.providers.is_enabled(ProviderKind::OpenCodeGo));
        self.openrouter_enabled
            .call(settings.providers.is_enabled(ProviderKind::OpenRouter));
        self.openrouter_accounts
            .call(crate::openrouter::accounts_for_settings(settings));
        self.codex_path.call(
            settings
                .codex_path
                .as_ref()
                .map_or_else(String::new, |path| path.to_string_lossy().into_owned()),
        );
        self.claude_path.call(
            settings
                .claude_path
                .as_ref()
                .map_or_else(String::new, |path| path.to_string_lossy().into_owned()),
        );
        self.cursor_path.call(
            settings
                .cursor_path
                .as_ref()
                .map_or_else(String::new, |path| path.to_string_lossy().into_owned()),
        );
        self.popup_order.call(settings.popup_order.clone());
        self.use_colored_provider_icons
            .call(settings.use_colored_provider_icons);
        self.use_colored_sidebar_icons
            .call(settings.use_colored_sidebar_icons);
        self.replace_chatgpt_logo_with_codex
            .call(settings.replace_chatgpt_logo_with_codex);
        self.automatic_activation
            .call(settings.automatic_activation);
        self.scheduled_activations
            .call(settings.scheduled_activations.clone());
        self.auto_activation_pauses
            .call(settings.auto_activation_pauses.clone());
        self.limit_refresh_interval
            .call(settings.limit_refresh_interval);
        self.start_at_login.call(settings.start_at_login);
        self.show_used_percentage
            .call(settings.show_used_percentage);
        self.show_usage_pace.call(settings.show_usage_pace);
        self.compact_usage_cards.call(settings.compact_usage_cards);
        self.popup_visibility
            .call(settings.popup_visibility.clone());
        self.show_total_spend_on_all_tab
            .call(settings.show_total_spend_on_all_tab);
        self.total_spend_presentation
            .call(settings.total_spend_presentation);
        self.show_account_name.call(settings.show_account_name);
        self.activation_success
            .call(settings.notifications.activation_success);
        self.activation_failure
            .call(settings.notifications.activation_failure);
        self.limits_reset
            .call(settings.notifications.limits_changed);
        self.low_usage_enabled
            .call(settings.notifications.low_usage_enabled);
        self.low_usage_threshold
            .call(settings.notifications.low_usage_threshold_percent);
        self.weekly_low_usage_enabled
            .call(settings.notifications.weekly_low_usage_enabled);
        self.weekly_low_usage_threshold
            .call(settings.notifications.weekly_low_usage_threshold_percent);
        self.tray_widgets.call(settings.tray_widgets.clone());
        self.check_for_updates.call(settings.check_for_updates);
        self.notify_on_update
            .call(settings.notifications.update_available);
    }
}

/// Immutable values and reactive setters shared by the settings page renderers.
/// The shell creates this once per render; page modules only consume it.
#[derive(Clone)]
pub(super) struct SettingsPageContext<'a> {
    pub(super) theme: AppTheme,
    pub(super) accent_color: AccentColor,
    pub(super) animations_enabled: bool,
    pub(super) time_format: TimeFormat,
    pub(super) codex_enabled: bool,
    pub(super) claude_enabled: bool,
    pub(super) cursor_enabled: bool,
    pub(super) opencode_zen_enabled: bool,
    pub(super) opencode_go_enabled: bool,
    pub(super) openrouter_enabled: bool,
    pub(super) codex_path: &'a str,
    pub(super) claude_path: &'a str,
    pub(super) cursor_path: &'a str,
    pub(super) codex_install_status: &'a ProviderInstallStatus,
    pub(super) claude_install_status: &'a ProviderInstallStatus,
    pub(super) cursor_install_status: &'a ProviderInstallStatus,
    pub(super) opencode_zen_install_status: &'a ProviderInstallStatus,
    pub(super) opencode_go_install_status: &'a ProviderInstallStatus,
    pub(super) openrouter_install_status: &'a ProviderInstallStatus,
    pub(super) opencode_zen_key_input: &'a str,
    pub(super) opencode_go_key_input: &'a str,
    pub(super) openrouter_accounts: &'a [OpenRouterAccount],
    pub(super) openrouter_key_inputs: &'a HashMap<String, String>,
    pub(super) openrouter_management_inputs: &'a HashMap<String, String>,
    pub(super) popup_order: &'a [PopupWidgetKind],
    pub(super) use_colored_provider_icons: bool,
    pub(super) use_colored_sidebar_icons: bool,
    pub(super) replace_chatgpt_logo_with_codex: bool,
    pub(super) automatic_activation: bool,
    pub(super) scheduled_activations: &'a [ScheduledActivation],
    pub(super) auto_activation_pauses: &'a [AutoActivationPause],
    pub(super) expanded_scheduled_activation: &'a Option<String>,
    pub(super) expanded_auto_activation_pause: &'a Option<String>,
    pub(super) limit_refresh_interval: LimitRefreshInterval,
    pub(super) start_at_login: bool,
    pub(super) show_used_percentage: bool,
    pub(super) show_usage_pace: bool,
    pub(super) compact_usage_cards: bool,
    pub(super) popup_visibility: &'a PopupVisibility,
    pub(super) discovered_popup_bricks: &'a BTreeMap<String, String>,
    pub(super) show_total_spend_on_all_tab: bool,
    pub(super) total_spend_presentation: TotalSpendPresentation,
    pub(super) show_account_name: bool,
    pub(super) activation_success: bool,
    pub(super) activation_failure: bool,
    pub(super) limits_reset: bool,
    pub(super) low_usage_enabled: bool,
    pub(super) low_usage_threshold: u8,
    pub(super) low_usage_expanded: bool,
    pub(super) low_usage_expand_progress: f64,
    pub(super) weekly_low_usage_enabled: bool,
    pub(super) weekly_low_usage_threshold: u8,
    pub(super) weekly_low_usage_expanded: bool,
    pub(super) weekly_low_usage_expand_progress: f64,
    pub(super) tray_widgets: &'a [TrayWidget],
    pub(super) expanded_tray_widget: &'a Option<String>,
    pub(super) editing_tray_indicator: &'a Option<(String, usize)>,
    pub(super) removed_tray_widget: &'a Option<(usize, TrayWidget)>,
    pub(super) hovered_card_id: &'a Option<String>,
    pub(super) expanded_popup_provider: &'a Option<String>,
    pub(super) check_for_updates: bool,
    pub(super) notify_on_update: bool,
    pub(super) update_phase: &'a UpdatePhase,
    pub(super) log_content: &'a str,
    pub(super) set_codex_enabled: SetState<bool>,
    pub(super) set_theme: SetState<AppTheme>,
    pub(super) set_accent_color: SetState<AccentColor>,
    pub(super) set_animations_enabled: SetState<bool>,
    pub(super) set_time_format: SetState<TimeFormat>,
    pub(super) set_claude_enabled: SetState<bool>,
    pub(super) set_cursor_enabled: SetState<bool>,
    pub(super) set_opencode_zen_enabled: SetState<bool>,
    pub(super) set_opencode_go_enabled: SetState<bool>,
    pub(super) set_openrouter_enabled: SetState<bool>,
    pub(super) set_opencode_zen_key_input: SetState<String>,
    pub(super) set_opencode_go_key_input: SetState<String>,
    pub(super) set_openrouter_accounts: SetState<Vec<OpenRouterAccount>>,
    pub(super) set_openrouter_key_inputs: SetState<HashMap<String, String>>,
    pub(super) set_openrouter_management_inputs: SetState<HashMap<String, String>>,
    pub(super) set_codex_path: SetState<String>,
    pub(super) set_claude_path: SetState<String>,
    pub(super) set_cursor_path: SetState<String>,
    pub(super) set_popup_order: SetState<Vec<PopupWidgetKind>>,
    pub(super) set_use_colored_provider_icons: SetState<bool>,
    pub(super) set_use_colored_sidebar_icons: SetState<bool>,
    pub(super) set_replace_chatgpt_logo_with_codex: SetState<bool>,
    pub(super) set_automatic_activation: SetState<bool>,
    pub(super) set_scheduled_activations: SetState<Vec<ScheduledActivation>>,
    pub(super) set_auto_activation_pauses: SetState<Vec<AutoActivationPause>>,
    pub(super) set_expanded_scheduled_activation: SetState<Option<String>>,
    pub(super) set_expanded_auto_activation_pause: SetState<Option<String>>,
    pub(super) set_limit_refresh_interval: SetState<LimitRefreshInterval>,
    pub(super) set_start_at_login: SetState<bool>,
    pub(super) set_show_used_percentage: SetState<bool>,
    pub(super) set_show_usage_pace: SetState<bool>,
    pub(super) set_compact_usage_cards: SetState<bool>,
    pub(super) set_popup_visibility: SetState<PopupVisibility>,
    pub(super) set_discovered_popup_bricks: SetState<BTreeMap<String, String>>,
    pub(super) set_show_total_spend_on_all_tab: SetState<bool>,
    pub(super) set_total_spend_presentation: SetState<TotalSpendPresentation>,
    pub(super) set_show_account_name: SetState<bool>,
    pub(super) set_activation_success: SetState<bool>,
    pub(super) set_activation_failure: SetState<bool>,
    pub(super) set_limits_reset: SetState<bool>,
    pub(super) set_low_usage_enabled: SetState<bool>,
    pub(super) set_low_usage_threshold: SetState<u8>,
    pub(super) set_low_usage_expanded: SetState<bool>,
    pub(super) set_low_usage_expand_progress: AsyncSetState<f64>,
    pub(super) set_weekly_low_usage_enabled: SetState<bool>,
    pub(super) set_weekly_low_usage_threshold: SetState<u8>,
    pub(super) set_weekly_low_usage_expanded: SetState<bool>,
    pub(super) set_weekly_low_usage_expand_progress: AsyncSetState<f64>,
    pub(super) set_tray_widgets: SetState<Vec<TrayWidget>>,
    pub(super) set_expanded_tray_widget: SetState<Option<String>>,
    pub(super) set_editing_tray_indicator: AsyncSetState<Option<(String, usize)>>,
    pub(super) set_indicator_modal_visible: AsyncSetState<bool>,
    pub(super) set_removed_tray_widget: SetState<Option<(usize, TrayWidget)>>,
    pub(super) set_expanded_popup_provider: SetState<Option<String>>,
    pub(super) set_hovered_card_id: SetState<Option<String>>,
    pub(super) set_check_for_updates: SetState<bool>,
    pub(super) set_notify_on_update: SetState<bool>,
    pub(super) theme_navigation_guard: HookRef<bool>,
    pub(super) theme_navigation_guard_timer: HookRef<Option<DispatcherTimer>>,
    pub(super) settings_tx: Sender<Settings>,
    pub(super) ui_dispatcher: UiMarshaller,
    pub(super) updates: Arc<UpdateController>,
}
