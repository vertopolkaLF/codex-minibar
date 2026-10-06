use super::*;

#[derive(Clone)]
pub(super) struct SettingsWindowState {
    pub(super) theme: SetState<AppTheme>,
    pub(super) accent_color: SetState<AccentColor>,
    pub(super) animations_enabled: SetState<bool>,
    pub(super) bottom_bar_size: SetState<BottomBarSize>,
    pub(super) popup_corner_radius: SetState<PopupCornerRadius>,
    pub(super) popup_background_material: SetState<PopupBackgroundMaterial>,
    pub(super) time_format: SetState<TimeFormat>,
    pub(super) instances: SetState<Vec<ProviderInstance>>,
    pub(super) popup_tab_mode: SetState<PopupTabMode>,
    pub(super) use_colored_provider_icons: SetState<bool>,
    pub(super) use_colored_sidebar_icons: SetState<bool>,
    pub(super) replace_chatgpt_logo_with_codex: SetState<bool>,
    pub(super) scheduled_activations: SetState<Vec<ScheduledActivation>>,
    pub(super) auto_activation_pauses: SetState<Vec<AutoActivationPause>>,
    pub(super) usage_stats_enabled: SetState<bool>,
    pub(super) limit_refresh_interval: SetState<LimitRefreshInterval>,
    pub(super) usage_refresh_interval: SetState<UsageRefreshInterval>,
    pub(super) reset_announcement_refresh_interval: SetState<ResetAnnouncementRefreshInterval>,
    pub(super) start_at_login: SetState<bool>,
    pub(super) show_used_percentage: SetState<bool>,
    pub(super) show_usage_values: SetState<bool>,
    pub(super) show_usage_pace: SetState<bool>,
    pub(super) compact_usage_cards: SetState<bool>,
    pub(super) popup_two_columns: SetState<bool>,
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
    pub(super) forced_reset_feed_enabled: SetState<bool>,
    pub(super) forced_reset_notifications: SetState<bool>,
}

impl SettingsWindowState {
    pub(super) fn apply(&self, settings: &Settings) {
        self.theme.call(settings.theme);
        self.accent_color.call(settings.accent_color);
        self.animations_enabled.call(settings.animations_enabled);
        self.bottom_bar_size.call(settings.bottom_bar_size);
        self.popup_corner_radius.call(settings.popup_corner_radius);
        self.popup_background_material
            .call(settings.popup_background_material);
        self.time_format.call(settings.time_format);
        self.instances.call(settings.instances.clone());
        self.popup_tab_mode.call(settings.popup_tab_mode);
        self.use_colored_provider_icons
            .call(settings.use_colored_provider_icons);
        self.use_colored_sidebar_icons
            .call(settings.use_colored_sidebar_icons);
        self.replace_chatgpt_logo_with_codex
            .call(settings.replace_chatgpt_logo_with_codex);
        self.scheduled_activations
            .call(settings.scheduled_activations.clone());
        self.auto_activation_pauses
            .call(settings.auto_activation_pauses.clone());
        self.usage_stats_enabled.call(settings.usage_stats_enabled);
        self.limit_refresh_interval
            .call(settings.limit_refresh_interval);
        self.usage_refresh_interval
            .call(settings.usage_refresh_interval);
        self.reset_announcement_refresh_interval
            .call(settings.reset_announcement_refresh_interval);
        self.start_at_login.call(settings.start_at_login);
        self.show_used_percentage
            .call(settings.show_used_percentage);
        self.show_usage_values.call(settings.show_usage_values);
        self.show_usage_pace.call(settings.show_usage_pace);
        self.compact_usage_cards.call(settings.compact_usage_cards);
        self.popup_two_columns.call(settings.popup_two_columns);
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
        self.forced_reset_feed_enabled
            .call(settings.notifications.forced_reset_feed_enabled);
        self.forced_reset_notifications
            .call(settings.notifications.forced_reset_notifications);
    }
}

/// Immutable values and reactive setters shared by the settings page renderers.
/// The shell creates this once per render; page modules only consume it.
#[derive(Clone)]
pub(super) struct SettingsPageContext<'a> {
    pub(super) theme: AppTheme,
    pub(super) accent_color: AccentColor,
    pub(super) animations_enabled: bool,
    pub(super) bottom_bar_size: BottomBarSize,
    pub(super) popup_corner_radius: PopupCornerRadius,
    pub(super) popup_background_material: PopupBackgroundMaterial,
    pub(super) time_format: TimeFormat,
    /// Every configured provider instance in display order.
    pub(super) instances: &'a [ProviderInstance],
    /// Detection results keyed by instance id.
    pub(super) install_statuses: &'a HashMap<String, ProviderInstallStatus>,
    pub(super) popup_tab_mode: PopupTabMode,
    pub(super) openrouter_snapshot: &'a OpenRouterSettingsSnapshot,
    pub(super) expanded_provider_cards: &'a [String],
    pub(super) provider_notice: &'a Option<String>,
    pub(super) color_scheme: ColorScheme,
    pub(super) use_colored_provider_icons: bool,
    pub(super) use_colored_sidebar_icons: bool,
    pub(super) replace_chatgpt_logo_with_codex: bool,
    pub(super) scheduled_activations: &'a [ScheduledActivation],
    pub(super) auto_activation_pauses: &'a [AutoActivationPause],
    pub(super) expanded_scheduled_activation: &'a Option<String>,
    pub(super) expanded_auto_activation_pause: &'a Option<String>,
    pub(super) usage_stats_enabled: bool,
    pub(super) limit_refresh_interval: LimitRefreshInterval,
    pub(super) usage_refresh_interval: UsageRefreshInterval,
    pub(super) reset_announcement_refresh_interval: ResetAnnouncementRefreshInterval,
    pub(super) start_at_login: bool,
    pub(super) show_used_percentage: bool,
    pub(super) show_usage_values: bool,
    pub(super) show_usage_pace: bool,
    pub(super) compact_usage_cards: bool,
    pub(super) popup_two_columns: bool,
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
    pub(super) collapsed_popup_provider: &'a Option<String>,
    pub(super) check_for_updates: bool,
    pub(super) notify_on_update: bool,
    pub(super) forced_reset_feed_enabled: bool,
    pub(super) forced_reset_notifications: bool,
    pub(super) update_phase: &'a UpdatePhase,
    pub(super) log_content: &'a str,
    pub(super) streamdeck_install_phase: &'a crate::streamdeck::InstallPhase,
    pub(super) set_theme: SetState<AppTheme>,
    pub(super) set_accent_color: SetState<AccentColor>,
    pub(super) set_animations_enabled: SetState<bool>,
    pub(super) set_bottom_bar_size: SetState<BottomBarSize>,
    pub(super) set_popup_corner_radius: SetState<PopupCornerRadius>,
    pub(super) set_popup_background_material: SetState<PopupBackgroundMaterial>,
    pub(super) set_time_format: SetState<TimeFormat>,
    pub(super) set_instances: SetState<Vec<ProviderInstance>>,
    pub(super) set_popup_tab_mode: SetState<PopupTabMode>,
    pub(super) set_expanded_provider_cards: AsyncSetState<Vec<String>>,
    pub(super) set_provider_dialog: AsyncSetState<Option<ProviderDialog>>,
    pub(super) set_provider_notice: AsyncSetState<Option<String>>,
    pub(super) set_use_colored_provider_icons: SetState<bool>,
    pub(super) set_use_colored_sidebar_icons: SetState<bool>,
    pub(super) set_replace_chatgpt_logo_with_codex: SetState<bool>,
    pub(super) set_scheduled_activations: SetState<Vec<ScheduledActivation>>,
    pub(super) set_auto_activation_pauses: SetState<Vec<AutoActivationPause>>,
    pub(super) set_expanded_scheduled_activation: SetState<Option<String>>,
    pub(super) set_expanded_auto_activation_pause: SetState<Option<String>>,
    pub(super) set_usage_stats_enabled: SetState<bool>,
    pub(super) set_limit_refresh_interval: SetState<LimitRefreshInterval>,
    pub(super) set_usage_refresh_interval: SetState<UsageRefreshInterval>,
    pub(super) set_reset_announcement_refresh_interval: SetState<ResetAnnouncementRefreshInterval>,
    pub(super) set_start_at_login: SetState<bool>,
    pub(super) set_show_used_percentage: SetState<bool>,
    pub(super) set_show_usage_values: SetState<bool>,
    pub(super) set_show_usage_pace: SetState<bool>,
    pub(super) set_compact_usage_cards: SetState<bool>,
    pub(super) set_popup_two_columns: SetState<bool>,
    pub(super) set_popup_visibility: SetState<PopupVisibility>,
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
    pub(super) set_collapsed_popup_provider: SetState<Option<String>>,
    pub(super) set_hovered_card_id: SetState<Option<String>>,
    pub(super) set_check_for_updates: SetState<bool>,
    pub(super) set_notify_on_update: SetState<bool>,
    pub(super) set_forced_reset_feed_enabled: SetState<bool>,
    pub(super) set_forced_reset_notifications: SetState<bool>,
    pub(super) set_troubleshoot_picker: AsyncSetState<Option<crate::troubleshoot::ToolPickerState>>,
    pub(super) set_streamdeck_install_phase: AsyncSetState<crate::streamdeck::InstallPhase>,
    pub(super) theme_navigation_guard: HookRef<bool>,
    pub(super) theme_navigation_guard_timer: HookRef<Option<DispatcherTimer>>,
    pub(super) settings_tx: Sender<Settings>,
    pub(super) usage_actions_tx: Sender<UsageAction>,
    pub(super) ui_dispatcher: UiMarshaller,
    pub(super) updates: Arc<UpdateController>,
}

impl SettingsPageContext<'_> {
    pub(super) fn instance(&self, provider: ProviderId) -> Option<&ProviderInstance> {
        self.instances
            .iter()
            .find(|instance| instance.id == provider.id())
    }

    pub(super) fn install_status(&self, provider: ProviderId) -> ProviderInstallStatus {
        self.install_statuses
            .get(provider.id())
            .cloned()
            .unwrap_or_else(|| ProviderInstallStatus::checking_for(provider.kind()))
    }
}
