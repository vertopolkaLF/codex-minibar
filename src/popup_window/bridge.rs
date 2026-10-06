use super::*;

use std::collections::HashSet;

pub(super) fn provider_worker_event_is_current(
    ui: &UiState,
    provider: ProviderKind,
    worker_revision: u64,
) -> bool {
    let current_revision = match provider {
        ProviderKind::OpenRouter => ui.openrouter_credentials_revision,
        ProviderKind::Claude => ui.claude_credentials_revision,
        ProviderKind::Codex => ui.codex_credentials_revision,
        _ => 0,
    };
    worker_revision == current_revision
}

fn forced_reset_info_body(reset: &crate::reset_feed::ForcedReset) -> String {
    let local = reset.reset_at.with_timezone(&Local);
    let when = format!(
        "{}, {}",
        local.format("%b %-d"),
        TimeFormat::current().format_hm(local)
    );
    let countdown = format_reset_in(Some(reset.reset_at));
    reset.label.as_deref().map_or_else(
        || format!("A possible Codex reset is scheduled for {when} (in {countdown})"),
        |label| format!("{label}: possible reset on {when} (in {countdown})"),
    )
}

/// Notifies only that new feed information arrived. This is intentionally
/// unrelated to the API-driven `limits_changed` notification: the feed never
/// proves that a reset actually happened at `reset_at`.
fn notify_new_forced_reset_info(
    resets: &[crate::reset_feed::ForcedReset],
    notified_ids: &mut HashSet<String>,
    settings: &NotificationSettings,
    state: &AppState,
) {
    if !settings.forced_reset_feed_enabled || !settings.forced_reset_notifications {
        return;
    }
    let now = Utc::now();
    for reset in resets.iter().filter(|reset| reset.reset_at > now) {
        if notified_ids.insert(reset.id.clone()) {
            notifications::show("New Codex reset info", &forced_reset_info_body(reset));
            state.mark_forced_reset_info_notified(reset.id.clone());
        }
    }
}

/// The tracker for one provider account, named so its toasts say which account
/// they are about.
fn account_profile_tracker<'a>(
    trackers: &'a mut HashMap<String, LimitNotificationTracker>,
    profile: &crate::limits::ClaudeProfileSnapshot,
    provider: ProviderKind,
) -> &'a mut LimitNotificationTracker {
    trackers
        .entry(format!("{}:{}", provider.id(), profile.id))
        .or_default()
        .named(format!(
            "{} \u{00b7} {}",
            provider.display_name(),
            profile.name
        ))
}

pub(super) fn primary_notification_profile(
    profiles: &[crate::limits::AccountProfileSnapshot],
    activation_succeeded: bool,
) -> Option<&crate::limits::AccountProfileSnapshot> {
    if activation_succeeded {
        profiles.iter().find(|profile| profile.id == "default")
    } else {
        profiles.first()
    }
}

pub(super) fn update_available_from_phase(phase: &UpdatePhase) -> bool {
    matches!(phase, UpdatePhase::Available(_))
}

pub(super) fn update_version_from_phase(phase: &UpdatePhase) -> Option<String> {
    match phase {
        UpdatePhase::Available(update) => Some(update.version.clone()),
        _ => None,
    }
}

pub(super) fn start_background_bridge(
    state: Arc<AppState>,
    ui_dispatcher: windows_reactor::UiMarshaller,
) {
    // Use the already hydrated persistent snapshot while the first network
    // refresh is in flight. Opening Settings never starts another poll.
    // Account names live in settings, so overlay them before the first paint.
    let startup_settings = state.settings.clone();
    let _ = state.apply_openrouter_account_names(&startup_settings);
    let retained = crate::codex::prepare_startup_limits(
        state.current_limits().get(ProviderKind::Codex),
        &startup_settings,
    );
    state.replace_limits(ProviderKind::Codex, retained);
    let _ = state.apply_codex_profile_names(&startup_settings);
    crate::settings_window::publish_openrouter_snapshot(
        state.current_limits().get(ProviderKind::OpenRouter),
        ui_dispatcher.clone(),
    );
    let events = state.take_worker_events();
    let mut widgets = state
        .settings
        .tray_widgets
        .iter()
        .filter(|widget| widget.is_visible_for(&state.settings.providers))
        .cloned()
        .collect::<Vec<_>>();
    let settings_rx = state
        .settings_rx
        .lock()
        .ok()
        .and_then(|mut slot| slot.take());
    let usage_actions_rx = state
        .usage_actions_rx
        .lock()
        .ok()
        .and_then(|mut slot| slot.take());
    let settings_tx = state.settings_tx.clone();
    let updates = Arc::clone(&state.updates);
    let mut check_for_updates = state.settings.check_for_updates;
    let mut notify_on_update = state.settings.notifications.update_available;

    thread::spawn(move || {
        let streamdeck_rx = crate::streamdeck::start_server(Arc::clone(&state));
        let mut tray = TrayManager::new();
        let fallback_attempt = state.last_activation_at;
        let mut notification_settings = state.settings.notifications.clone();
        // Keyed by provider id, or by profile for a multi-profile provider.
        let mut limit_notifications = HashMap::<String, LimitNotificationTracker>::new();
        let mut notified_codex_profiles = state.settings.codex_profiles.clone();
        let mut notified_claude_profiles = state.settings.claude_profiles.clone();
        let mut notified_codex_revision = state.settings.codex_credentials_revision;
        let mut notified_claude_revision = state.settings.claude_credentials_revision;
        let mut pending_auto_activation_successes = HashSet::<ProviderKind>::new();
        let mut forced_reset_notified_ids = HashSet::<String>::new();
        let mut usage_clear_generation = 0_u64;
        let mut pending_usage_clear: Option<(u64, Vec<ProviderKind>)> = None;
        let mut update_phase = updates.snapshot();
        let mut live_settings = state.settings.clone();
        let mut ui = UiState {
            theme: state.settings.theme,
            accent_color: state.settings.accent_color,
            animations_enabled: state.settings.animations_enabled,
            popup_background_material: state.settings.popup_background_material,
            provider_errors: state.startup_provider_errors.iter().cloned().collect(),
            last_activation: format_last_activation(&RateLimits::default(), fallback_attempt),
            show_used_percentage: state.settings.show_used_percentage,
            show_usage_values: state.settings.show_usage_values,
            show_usage_pace: state.settings.show_usage_pace,
            compact_usage_cards: state.settings.compact_usage_cards,
            popup_visibility: state.settings.popup_visibility.clone(),
            usage_stats_enabled: state.settings.usage_stats_enabled,
            show_total_spend_on_all_tab: state.settings.show_total_spend_on_all_tab,
            total_spend_presentation: state.settings.total_spend_presentation,
            total_spend_period: state.settings.total_spend_period,
            show_account_name: state.settings.show_account_name,
            codex_enabled: state.settings.providers.is_enabled(ProviderKind::Codex),
            claude_enabled: state.settings.providers.is_enabled(ProviderKind::Claude),
            cursor_enabled: state.settings.providers.is_enabled(ProviderKind::Cursor),
            opencode_zen_enabled: state
                .settings
                .providers
                .is_enabled(ProviderKind::OpenCodeZen),
            opencode_go_enabled: state
                .settings
                .providers
                .is_enabled(ProviderKind::OpenCodeGo),
            opencode_zen_credentials_revision: state.settings.opencode_zen_credentials_revision,
            opencode_go_credentials_revision: state.settings.opencode_go_credentials_revision,
            openrouter_enabled: state
                .settings
                .providers
                .is_enabled(ProviderKind::OpenRouter),
            antigravity_enabled: state
                .settings
                .providers
                .is_enabled(ProviderKind::Antigravity),
            grok_enabled: state.settings.providers.is_enabled(ProviderKind::Grok),
            kiro_enabled: state.settings.providers.is_enabled(ProviderKind::Kiro),
            openrouter_credentials_revision: state.settings.openrouter_credentials_revision,
            popup_order: state.settings.popup_order.clone(),
            use_colored_provider_icons: state.settings.use_colored_provider_icons,
            show_accounts_as_tabs: state.settings.show_accounts_as_tabs,
            replace_chatgpt_logo_with_codex: state.settings.replace_chatgpt_logo_with_codex,
            codex_path: state.settings.codex_path.clone(),
            claude_path: state.settings.claude_path.clone(),
            codex_profiles: state.settings.codex_profiles.clone(),
            claude_profiles: state.settings.claude_profiles.clone(),
            codex_home_excluded_profiles: state.settings.codex_home_excluded_profiles.clone(),
            claude_home_excluded_profiles: state.settings.claude_home_excluded_profiles.clone(),
            codex_credentials_revision: state.settings.codex_credentials_revision,
            claude_credentials_revision: state.settings.claude_credentials_revision,
            cursor_path: state.settings.cursor_path.clone(),
            antigravity_path: state.settings.antigravity_path.clone(),
            grok_path: state.settings.grok_path.clone(),
            kiro_path: state.settings.kiro_path.clone(),
            kiro_crew_path: state.settings.kiro_crew_path.clone(),
            kiro_cli_path: state.settings.kiro_cli_path.clone(),
            update_version: update_version_from_phase(&update_phase),
            ..UiState::popup_layout_from_settings(&state.settings)
        };
        if let Some(error) = ui.error.as_deref() {
            crate::logger::info(format!("Popup error: {error}"));
        }

        if let Err(error) = tray.sync(
            &widgets,
            &state.current_limits(),
            update_available_from_phase(&update_phase),
        ) {
            ui.set_popup_error(error.to_string());
            flush_popup_ui(&ui);
        }

        // First frame: the renderer seeds itself from settings, then takes
        // this complete snapshot (errors, activation text, update state).
        publish_popup_ui(&ui);

        let apply_settings = |ui: &mut UiState,
                              notification_settings: &mut NotificationSettings,
                              widgets: &mut Vec<TrayWidget>,
                              tray: &mut TrayManager,
                              settings: Settings,
                              live_settings: &mut Settings| {
            crate::settings_window::sync_open_window(settings.clone(), ui_dispatcher.clone());
            let phase = updates.snapshot();
            ui.settings_revision = ui.settings_revision.wrapping_add(1);
            let providers_changed = ui.codex_enabled
                != settings.providers.is_enabled(ProviderKind::Codex)
                || ui.claude_enabled != settings.providers.is_enabled(ProviderKind::Claude)
                || ui.cursor_enabled != settings.providers.is_enabled(ProviderKind::Cursor)
                || ui.opencode_zen_enabled
                    != settings.providers.is_enabled(ProviderKind::OpenCodeZen)
                || ui.opencode_go_enabled
                    != settings.providers.is_enabled(ProviderKind::OpenCodeGo)
                || ui.openrouter_enabled != settings.providers.is_enabled(ProviderKind::OpenRouter)
                || ui.antigravity_enabled
                    != settings.providers.is_enabled(ProviderKind::Antigravity)
                || ui.grok_enabled != settings.providers.is_enabled(ProviderKind::Grok)
                || ui.kiro_enabled != settings.providers.is_enabled(ProviderKind::Kiro);
            let opencode_zen_credentials_changed =
                ui.opencode_zen_credentials_revision != settings.opencode_zen_credentials_revision;
            let opencode_go_credentials_changed =
                ui.opencode_go_credentials_revision != settings.opencode_go_credentials_revision;
            let openrouter_credentials_changed =
                ui.openrouter_credentials_revision != settings.openrouter_credentials_revision;
            if openrouter_credentials_changed {
                // Account ids can survive a key replacement. Do not present
                // the old key's balance/label as data for its replacement.
                // The matching worker revision also rejects output which was
                // already queued by the worker being replaced.
                state.replace_limits(ProviderKind::OpenRouter, RateLimits::default());
                ui.observe_limits_update();
                crate::settings_window::publish_openrouter_snapshot(
                    state.current_limits().get(ProviderKind::OpenRouter),
                    ui_dispatcher.clone(),
                );
            } else if state.apply_openrouter_account_names(&settings) {
                // Rename is settings-only. Overlay the new names before the
                // first paint so the popup does not keep the previous label.
                ui.observe_limits_update();
                crate::settings_window::publish_openrouter_snapshot(
                    state.current_limits().get(ProviderKind::OpenRouter),
                    ui_dispatcher.clone(),
                );
            }
            // A rename only relabels the cards. Adding, removing or toggling
            // a profile changes what is read, so the reader restarts.
            let read_set = |profiles: &[crate::settings::ClaudeProfile]| {
                profiles
                    .iter()
                    .map(|profile| (profile.id.clone(), profile.enabled))
                    .collect::<Vec<_>>()
            };
            let claude_profiles_changed =
                read_set(&ui.claude_profiles) != read_set(&settings.claude_profiles);
            let claude_credentials_changed =
                ui.claude_credentials_revision != settings.claude_credentials_revision;
            if claude_profiles_changed
                || claude_credentials_changed
                || ui.claude_path != settings.claude_path
            {
                // Keep samples for unchanged profiles through reader replacement.
                // Removed, disabled and credential-replaced profiles lose theirs.
                let retained = crate::claude::prepare_profile_refresh(
                    state.current_limits().get(ProviderKind::Claude),
                    live_settings,
                    &settings,
                );
                state.replace_limits(ProviderKind::Claude, retained);
                ui.observe_limits_update();
            } else if state.apply_claude_profile_names(&settings) {
                ui.observe_limits_update();
            }
            let codex_profiles_changed =
                read_set(&ui.codex_profiles) != read_set(&settings.codex_profiles);
            let codex_credentials_changed =
                ui.codex_credentials_revision != settings.codex_credentials_revision;
            if codex_profiles_changed
                || codex_credentials_changed
                || ui.codex_path != settings.codex_path
            {
                // Keep samples for unchanged profiles through reader replacement.
                // Removed, disabled and credential-replaced profiles lose theirs.
                let retained = crate::codex::prepare_profile_refresh(
                    state.current_limits().get(ProviderKind::Codex),
                    live_settings,
                    &settings,
                );
                state.replace_limits(ProviderKind::Codex, retained);
                ui.observe_limits_update();
            } else if state.apply_codex_profile_names(&settings) {
                ui.observe_limits_update();
            }
            if ui.theme != settings.theme || ui.accent_color != settings.accent_color {
                // Settings windows keep using WinUI's theme resources.
                let (theme, accent) = (settings.theme, settings.accent_color);
                ui_dispatcher.dispatch(move || crate::theme::apply_appearance(theme, accent));
            }
            ui.theme = settings.theme;
            ui.accent_color = settings.accent_color;
            ui.animations_enabled = settings.animations_enabled;
            ui.popup_background_material = settings.popup_background_material;
            ui.time_format = settings.time_format;
            ui.show_used_percentage = settings.show_used_percentage;
            ui.show_usage_values = settings.show_usage_values;
            ui.show_usage_pace = settings.show_usage_pace;
            ui.compact_usage_cards = settings.compact_usage_cards;
            ui.popup_visibility = settings.popup_visibility.clone();
            ui.usage_stats_enabled = settings.usage_stats_enabled;
            ui.usage_stats_excluded_providers = settings.effective_usage_stats_excluded_providers();
            ui.show_total_spend_on_all_tab = settings.show_total_spend_on_all_tab;
            ui.total_spend_presentation = settings.total_spend_presentation;
            ui.total_spend_period = settings.total_spend_period;
            ui.show_account_name = settings.show_account_name;
            ui.codex_enabled = settings.providers.is_enabled(ProviderKind::Codex);
            ui.claude_enabled = settings.providers.is_enabled(ProviderKind::Claude);
            ui.cursor_enabled = settings.providers.is_enabled(ProviderKind::Cursor);
            ui.opencode_zen_enabled = settings.providers.is_enabled(ProviderKind::OpenCodeZen);
            ui.opencode_go_enabled = settings.providers.is_enabled(ProviderKind::OpenCodeGo);
            ui.opencode_zen_credentials_revision = settings.opencode_zen_credentials_revision;
            ui.opencode_go_credentials_revision = settings.opencode_go_credentials_revision;
            ui.openrouter_enabled = settings.providers.is_enabled(ProviderKind::OpenRouter);
            ui.antigravity_enabled = settings.providers.is_enabled(ProviderKind::Antigravity);
            ui.grok_enabled = settings.providers.is_enabled(ProviderKind::Grok);
            ui.kiro_enabled = settings.providers.is_enabled(ProviderKind::Kiro);
            ui.openrouter_credentials_revision = settings.openrouter_credentials_revision;
            ui.popup_order = settings.popup_order.clone();
            ui.popup_two_columns = settings.popup_two_columns;
            ui.popup_right_column = settings.popup_right_column.clone();
            ui.popup_home_order = settings.popup_home_order.clone();
            ui.popup_home_right_column = settings.popup_home_right_column.clone();
            ui.use_colored_provider_icons = settings.use_colored_provider_icons;
            ui.show_accounts_as_tabs = settings.show_accounts_as_tabs;
            ui.replace_chatgpt_logo_with_codex = settings.replace_chatgpt_logo_with_codex;
            *notification_settings = settings.notifications.clone();
            state.sync_reset_feed(&settings);
            if !settings.notifications.forced_reset_feed_enabled {
                state.replace_forced_resets(Vec::new());
                ui.observe_forced_resets_update();
            }
            *widgets = settings
                .tray_widgets
                .iter()
                .filter(|widget| widget.is_visible_for(&settings.providers))
                .cloned()
                .collect();
            ui.update_version = update_version_from_phase(&phase);
            let restart = [
                (
                    ProviderKind::Codex,
                    settings.codex_path != ui.codex_path
                        || codex_profiles_changed
                        || codex_credentials_changed,
                ),
                (
                    ProviderKind::Claude,
                    settings.claude_path != ui.claude_path
                        || claude_profiles_changed
                        || claude_credentials_changed,
                ),
                (ProviderKind::Cursor, settings.cursor_path != ui.cursor_path),
                (
                    ProviderKind::Antigravity,
                    settings.antigravity_path != ui.antigravity_path,
                ),
                (ProviderKind::Grok, settings.grok_path != ui.grok_path),
                (
                    ProviderKind::Kiro,
                    settings.kiro_path != ui.kiro_path
                        || settings.kiro_crew_path != ui.kiro_crew_path
                        || settings.kiro_cli_path != ui.kiro_cli_path,
                ),
                (ProviderKind::OpenRouter, openrouter_credentials_changed),
            ]
            .into_iter()
            .filter_map(|(provider, changed)| changed.then_some(provider))
            .collect::<Vec<_>>();
            ui.codex_path = settings.codex_path.clone();
            ui.claude_path = settings.claude_path.clone();
            ui.codex_profiles = settings.codex_profiles.clone();
            ui.claude_profiles = settings.claude_profiles.clone();
            ui.codex_home_excluded_profiles = settings.codex_home_excluded_profiles.clone();
            ui.claude_home_excluded_profiles = settings.claude_home_excluded_profiles.clone();
            ui.codex_credentials_revision = settings.codex_credentials_revision;
            ui.claude_credentials_revision = settings.claude_credentials_revision;
            ui.cursor_path = settings.cursor_path.clone();
            ui.antigravity_path = settings.antigravity_path.clone();
            ui.grok_path = settings.grok_path.clone();
            ui.kiro_path = settings.kiro_path.clone();
            ui.kiro_crew_path = settings.kiro_crew_path.clone();
            ui.kiro_cli_path = settings.kiro_cli_path.clone();
            for provider in ProviderKind::ALL {
                if restart.contains(&provider) || !settings.providers.is_enabled(provider) {
                    ui.clear_provider_requests(provider);
                }
            }
            // Presentation settings must visibly apply before any background
            // work. In particular, changing provider icons must never wait on
            // a worker lock, network request, or provider lifecycle change.
            flush_popup_ui(ui);
            if providers_changed || !restart.is_empty() {
                let provider_errors = state.sync_provider_workers(&settings, &restart);
                for (provider, error) in provider_errors {
                    ui.set_provider_error(provider, error);
                }
            }
            for provider in ProviderKind::ALL {
                if !settings.providers.is_enabled(provider) {
                    ui.clear_provider_error(provider);
                }
                if !settings.usage_stats_enabled
                    || !crate::provider_registry::supports_usage_stats(provider)
                    || !settings.usage_stats_collection_enabled(provider)
                {
                    ui.clear_usage_error(provider);
                }
            }
            // Repaint the existing native icons in place. Recreating them makes
            // Explorer animate a remove/add sequence and causes a visible flash.
            if let Err(error) = tray.sync(
                widgets,
                &state.current_limits(),
                update_available_from_phase(&phase),
            ) {
                ui.set_popup_error(error.to_string());
            }
            for (provider, commands) in state.worker_commands() {
                let _ = commands.send(WorkerCommand::SetAutomaticActivation(
                    crate::provider::automatic_activation(provider, &settings),
                ));
                let schedules = settings
                    .scheduled_activations
                    .iter()
                    .filter(|rule| {
                        crate::provider_registry::descriptor(provider).supports_activation
                            && rule.provider() == Some(provider)
                    })
                    .cloned()
                    .collect();
                let _ = commands.send(WorkerCommand::SetScheduledActivations(schedules));
                let auto_activation_pauses = settings
                    .auto_activation_pauses
                    .iter()
                    .filter(|pause| {
                        crate::provider_registry::descriptor(provider).supports_activation
                            && pause.provider() == Some(provider)
                    })
                    .cloned()
                    .collect();
                let _ = commands.send(WorkerCommand::SetAutoActivationPauses(
                    auto_activation_pauses,
                ));
                let _ = commands.send(WorkerCommand::SetLimitRefreshInterval(Duration::from_secs(
                    settings.limit_refresh_interval.seconds(),
                )));
                let _ = commands.send(WorkerCommand::SetUsageRefreshInterval(Duration::from_secs(
                    settings.usage_refresh_interval.seconds(),
                )));
                // The worker reloads the selected history range immediately,
                // so changes are reflected in the open popup without asking the
                // user to restart the application.
                let _ = commands.send(WorkerCommand::SetHistoryRetentionDays(
                    settings.history_retention_days,
                ));
                let _ = commands.send(WorkerCommand::SetUsageCollectionEnabled(
                    crate::provider_registry::supports_usage_stats(provider)
                        && settings.usage_stats_collection_enabled(provider),
                ));
                if (provider == ProviderKind::OpenCodeZen && opencode_zen_credentials_changed)
                    || (provider == ProviderKind::OpenCodeGo && opencode_go_credentials_changed)
                    || (provider == ProviderKind::OpenRouter && openrouter_credentials_changed)
                {
                    let _ = commands.send(WorkerCommand::Refresh);
                }
            }
            *live_settings = settings;
            flush_popup_ui(ui);
        };

        let drain_settings = |ui: &mut UiState,
                              notification_settings: &mut NotificationSettings,
                              widgets: &mut Vec<TrayWidget>,
                              tray: &mut TrayManager,
                              check_for_updates: &mut bool,
                              notify_on_update: &mut bool,
                              forced_reset_notified_ids: &mut HashSet<String>,
                              live_settings: &mut Settings| {
            let Some(settings_rx) = settings_rx.as_ref() else {
                return;
            };
            while let Ok(settings) = settings_rx.try_recv() {
                if settings.check_for_updates && !*check_for_updates {
                    updates.check_async(false, settings.notifications.update_available);
                }
                *check_for_updates = settings.check_for_updates;
                *notify_on_update = settings.notifications.update_available;
                apply_settings(
                    ui,
                    notification_settings,
                    widgets,
                    tray,
                    settings,
                    live_settings,
                );
                notify_new_forced_reset_info(
                    &state.current_forced_resets(),
                    forced_reset_notified_ids,
                    notification_settings,
                    &state,
                );
            }
        };

        let drain_usage_actions =
            |ui: &mut UiState,
             generation: &mut u64,
             pending: &mut Option<(u64, Vec<ProviderKind>)>| {
                let Some(actions) = usage_actions_rx.as_ref() else {
                    return;
                };
                while let Ok(UsageAction::ClearData) = actions.try_recv() {
                    // One clear operation is enough. The button remains safe to
                    // click while a previous provider barrier is draining.
                    if pending.is_some() {
                        continue;
                    }
                    *generation = generation.wrapping_add(1);
                    let clear_generation = *generation;
                    let targets = state
                        .worker_commands()
                        .into_iter()
                        .filter_map(|(provider, commands)| {
                            commands
                                .send(WorkerCommand::ClearUsageData(clear_generation))
                                .is_ok()
                                .then_some(provider)
                        })
                        .collect::<Vec<_>>();
                    state.clear_usage_snapshot();
                    ui.observe_usage_update();
                    publish_popup_ui(ui);

                    if targets.is_empty() {
                        if let Err(error) =
                            crate::store::with_store(|store| store.clear_usage_data())
                        {
                            ui.set_popup_error(format!("Could not clear usage data: {error:#}"));
                            publish_popup_ui(ui);
                        }
                    } else {
                        *pending = Some((clear_generation, targets));
                    }
                }
            };

        let drain_updates = |ui: &mut UiState,
                             tray: &mut TrayManager,
                             update_phase: &mut UpdatePhase,
                             widgets: &mut Vec<TrayWidget>| {
            let next = updates.snapshot();
            if next == *update_phase {
                return;
            }
            *update_phase = next;
            ui.update_version = update_version_from_phase(update_phase);
            if let Err(error) = tray.sync(
                widgets,
                &state.current_limits(),
                update_available_from_phase(update_phase),
            ) {
                ui.set_popup_error(error.to_string());
            }
            publish_popup_ui(ui);
        };

        let drain_toast_update = || {
            if crate::notifications::take_toast_update_request()
                && let Err(error) = crate::updater::apply_pending_update()
            {
                eprintln!("failed to apply update from toast: {error:#}");
                notifications::show("Update failed", &format!("{error:#}"));
            }
        };

        let drain_streamdeck = || {
            while let Ok(command) = streamdeck_rx.try_recv() {
                match command {
                    crate::streamdeck::Command::RefreshData => {
                        for (_, commands) in state.worker_commands() {
                            let _ = commands.send(WorkerCommand::Refresh);
                        }
                    }
                    crate::streamdeck::Command::OpenPopup { provider } => {
                        if popup::is_visible() && !popup::is_closing() {
                            // Match tray-click toggle: a second press dismisses
                            // the flyout. Keep it when Settings is using it as
                            // a live preview.
                            if !crate::settings_window::is_open() {
                                popup::hide();
                            }
                            continue;
                        }
                        match provider {
                            Some(provider) => crate::popup_window::request_provider_view(provider),
                            None => crate::popup_window::request_home_view(),
                        }
                        popup::show_on_primary();
                    }
                }
            }
        };

        let Some(events) = events else {
            publish_popup_ui(&ui);
            loop {
                popup::pump_messages();
                drain_toast_update();
                drain_streamdeck();
                drain_usage_actions(
                    &mut ui,
                    &mut usage_clear_generation,
                    &mut pending_usage_clear,
                );
                if let Err(error) = tray.refresh_system_theme(&widgets, &state.current_limits()) {
                    ui.set_popup_error(error.to_string());
                    publish_popup_ui(&ui);
                }
                drain_settings(
                    &mut ui,
                    &mut notification_settings,
                    &mut widgets,
                    &mut tray,
                    &mut check_for_updates,
                    &mut notify_on_update,
                    &mut forced_reset_notified_ids,
                    &mut live_settings,
                );
                drain_updates(&mut ui, &mut tray, &mut update_phase, &mut widgets);
                if pump_tray_and_dismiss(&tray, &ui_dispatcher, &settings_tx, &state, &mut ui) {
                    drop(tray);
                    state.shutdown_worker();
                    std::process::exit(0);
                }
                thread::sleep(Duration::from_millis(16));
            }
        };

        loop {
            popup::pump_messages();
            drain_toast_update();
            drain_streamdeck();
            drain_usage_actions(
                &mut ui,
                &mut usage_clear_generation,
                &mut pending_usage_clear,
            );
            if let Err(error) = tray.refresh_system_theme(&widgets, &state.current_limits()) {
                ui.set_popup_error(error.to_string());
                publish_popup_ui(&ui);
            }
            drain_settings(
                &mut ui,
                &mut notification_settings,
                &mut widgets,
                &mut tray,
                &mut check_for_updates,
                &mut notify_on_update,
                &mut forced_reset_notified_ids,
                &mut live_settings,
            );
            drain_updates(&mut ui, &mut tray, &mut update_phase, &mut widgets);
            if pump_tray_and_dismiss(&tray, &ui_dispatcher, &settings_tx, &state, &mut ui) {
                drop(tray);
                state.shutdown_worker();
                std::process::exit(0);
            }
            match events.recv_timeout(Duration::from_millis(16)) {
                Ok(WorkerEvent::ForcedResetsUpdated(snapshot)) => {
                    forced_reset_notified_ids.extend(snapshot.notified_ids);
                    if notification_settings.forced_reset_feed_enabled {
                        state.replace_forced_resets(snapshot.resets.clone());
                    } else {
                        state.replace_forced_resets(Vec::new());
                    }
                    ui.observe_forced_resets_update();
                    notify_new_forced_reset_info(
                        &snapshot.resets,
                        &mut forced_reset_notified_ids,
                        &notification_settings,
                        &state,
                    );
                    publish_popup_ui(&ui);
                }
                Ok(WorkerEvent::ForcedResetsRefreshFailed(error)) => {
                    crate::logger::info(format!("Codex reset feed refresh failed: {error}"));
                }
                Ok(WorkerEvent::ProviderRequestStarted(provider, worker_revision, kind)) => {
                    if !provider_worker_event_is_current(&ui, provider, worker_revision) {
                        continue;
                    }
                    ui.request_started(provider, kind);
                    publish_popup_ui(&ui);
                }
                Ok(WorkerEvent::ProviderRequestFinished(provider, worker_revision, kind)) => {
                    if !provider_worker_event_is_current(&ui, provider, worker_revision) {
                        continue;
                    }
                    ui.request_finished(provider, kind);
                    publish_popup_ui(&ui);
                }
                Ok(WorkerEvent::ProviderLimitsUpdated(provider, worker_revision, limits)) => {
                    if !provider_worker_event_is_current(&ui, provider, worker_revision) {
                        continue;
                    }
                    if (provider == ProviderKind::Codex && !ui.codex_enabled)
                        || (provider == ProviderKind::Claude && !ui.claude_enabled)
                        || (provider == ProviderKind::Cursor && !ui.cursor_enabled)
                        || (provider == ProviderKind::OpenCodeZen && !ui.opencode_zen_enabled)
                        || (provider == ProviderKind::OpenCodeGo && !ui.opencode_go_enabled)
                        || (provider == ProviderKind::OpenRouter && !ui.openrouter_enabled)
                        || (provider == ProviderKind::Antigravity && !ui.antigravity_enabled)
                        || (provider == ProviderKind::Grok && !ui.grok_enabled)
                        || (provider == ProviderKind::Kiro && !ui.kiro_enabled)
                    {
                        continue;
                    }
                    crate::logger::info(format!(
                        "{} limits received: session used={:?}%, reset={:?}; weekly used={:?}%, reset={:?}",
                        provider.display_name(),
                        limits.primary.used_percent,
                        limits.primary.resets_at,
                        limits.secondary.used_percent,
                        limits.secondary.resets_at,
                    ));
                    let mut limits = limits;
                    if provider == ProviderKind::OpenRouter {
                        crate::openrouter::apply_account_names(&mut limits, &live_settings);
                    }
                    // The running reader still has the names it started with.
                    if provider == ProviderKind::Claude {
                        crate::claude::apply_profile_names(&mut limits, &live_settings);
                    }
                    // Publish once, then let both native tray and WinUI render
                    // from that exact snapshot.
                    state.replace_limits(provider, limits);
                    ui.clear_provider_error(provider);
                    let limits = state.current_limits();
                    crate::settings_window::publish_discovered_popup_bricks(
                        &limits,
                        ui_dispatcher.clone(),
                    );
                    if provider == ProviderKind::OpenRouter {
                        crate::settings_window::publish_openrouter_snapshot(
                            limits.get(ProviderKind::OpenRouter),
                            ui_dispatcher.clone(),
                        );
                    }
                    if ui.popup_visibility.absorb_discovered_bricks(&limits) {
                        let limits_for_settings = limits.clone();
                        crate::settings_window::persist_update(
                            settings_tx.clone(),
                            move |settings| {
                                settings.absorb_discovered_popup_bricks(&limits_for_settings);
                            },
                        );
                    }
                    let combine_activation_notification =
                        pending_auto_activation_successes.remove(&provider);
                    // With several Claude profiles each one is tracked by
                    // itself and named in its toasts. The first profile is
                    // also the provider-level snapshot observed here.
                    if provider == ProviderKind::Claude
                        && (notified_claude_profiles != ui.claude_profiles
                            || notified_claude_revision != ui.claude_credentials_revision)
                    {
                        // A tracker primed on one account must not compare
                        // its reset time with a different account's.
                        notified_claude_profiles = ui.claude_profiles.clone();
                        notified_claude_revision = ui.claude_credentials_revision;
                        limit_notifications
                            .retain(|key, _| !key.starts_with(ProviderKind::Claude.id()));
                    }
                    if provider == ProviderKind::Codex
                        && (notified_codex_profiles != ui.codex_profiles
                            || notified_codex_revision != ui.codex_credentials_revision)
                    {
                        // A tracker primed on one account must not compare
                        // its reset time with a different account's.
                        notified_codex_profiles = ui.codex_profiles.clone();
                        notified_codex_revision = ui.codex_credentials_revision;
                        limit_notifications
                            .retain(|key, _| !key.starts_with(ProviderKind::Codex.id()));
                    }
                    let profiles = limits.get(provider).account_profiles(provider);
                    let primary_profile =
                        primary_notification_profile(profiles, combine_activation_notification);
                    let tracker = match primary_profile {
                        Some(profile) => {
                            account_profile_tracker(&mut limit_notifications, profile, provider)
                        }
                        None => limit_notifications
                            .entry(provider.id().to_owned())
                            .or_default(),
                    };
                    let notification_result = if combine_activation_notification {
                        tracker.observe_with_primary_reset_deferred(
                            primary_profile.map_or(limits.get(provider), |profile| &profile.limits),
                            &notification_settings,
                            provider,
                        )
                    } else {
                        tracker.observe(
                            primary_profile.map_or(limits.get(provider), |profile| &profile.limits),
                            &notification_settings,
                            provider,
                        )
                    };
                    for profile in profiles.iter().filter(|profile| {
                        primary_profile.is_none_or(|primary| primary.id != profile.id)
                    }) {
                        account_profile_tracker(&mut limit_notifications, profile, provider)
                            .observe(&profile.limits, &notification_settings, provider);
                    }
                    if combine_activation_notification {
                        if notification_result.primary_reset {
                            notifications::show_activation_succeeded_after_reset(provider);
                        } else if notification_settings.activation_success {
                            notifications::show_activation_succeeded(provider);
                        }
                    }
                    if let Err(error) = tray.sync(
                        &widgets,
                        &limits,
                        update_available_from_phase(&update_phase),
                    ) {
                        ui.set_popup_error(error.to_string());
                    } else {
                        ui.error = None;
                    }
                    if provider == ProviderKind::Codex {
                        ui.last_activation =
                            format_last_activation(limits.get(provider), fallback_attempt);
                    }
                    ui.observe_limits_update();
                    publish_popup_ui(&ui);
                }
                Ok(WorkerEvent::ProviderUsageUpdated(provider, worker_revision, usage)) => {
                    if !provider_worker_event_is_current(&ui, provider, worker_revision) {
                        continue;
                    }
                    if provider == ProviderKind::Codex
                        && usage
                            .account_id
                            .as_deref()
                            .is_some_and(|id| id != crate::store::codex_accounts::current_id())
                    {
                        continue;
                    }
                    if (provider == ProviderKind::Codex && !ui.codex_enabled)
                        || (provider == ProviderKind::Claude && !ui.claude_enabled)
                        || (provider == ProviderKind::Cursor && !ui.cursor_enabled)
                        || (provider == ProviderKind::OpenCodeZen && !ui.opencode_zen_enabled)
                        || (provider == ProviderKind::OpenCodeGo && !ui.opencode_go_enabled)
                        || (provider == ProviderKind::OpenRouter && !ui.openrouter_enabled)
                        || (provider == ProviderKind::Antigravity && !ui.antigravity_enabled)
                        || (provider == ProviderKind::Grok && !ui.grok_enabled)
                        || (provider == ProviderKind::Kiro && !ui.kiro_enabled)
                    {
                        continue;
                    }
                    crate::logger::info(format!(
                        "{} usage received: today={} tokens, history={} tokens",
                        provider.display_name(),
                        usage.today.total_tokens(),
                        usage.history.total_tokens()
                    ));
                    let usage_error = live_settings
                        .usage_stats_collection_enabled(provider)
                        .then(|| usage_error_message(&usage))
                        .flatten();
                    state.replace_usage(provider, usage);
                    if let Some(error) = usage_error {
                        ui.set_usage_error(provider, error);
                    } else {
                        ui.clear_usage_error(provider);
                    }
                    ui.observe_usage_update();
                    publish_popup_ui(&ui);
                }
                Ok(WorkerEvent::ProviderUsageLoadedFromCache(provider, worker_revision, usage)) => {
                    if !provider_worker_event_is_current(&ui, provider, worker_revision) {
                        continue;
                    }
                    if (provider == ProviderKind::Codex && !ui.codex_enabled)
                        || (provider == ProviderKind::Claude && !ui.claude_enabled)
                        || (provider == ProviderKind::Cursor && !ui.cursor_enabled)
                        || (provider == ProviderKind::OpenCodeZen && !ui.opencode_zen_enabled)
                        || (provider == ProviderKind::OpenCodeGo && !ui.opencode_go_enabled)
                        || (provider == ProviderKind::OpenRouter && !ui.openrouter_enabled)
                        || (provider == ProviderKind::Antigravity && !ui.antigravity_enabled)
                        || (provider == ProviderKind::Grok && !ui.grok_enabled)
                        || (provider == ProviderKind::Kiro && !ui.kiro_enabled)
                    {
                        continue;
                    }
                    crate::logger::info(format!(
                        "{} usage cache loaded: today={} tokens, history={} tokens",
                        provider.display_name(),
                        usage.today.total_tokens(),
                        usage.history.total_tokens()
                    ));
                    state.replace_usage(provider, usage);
                    // Cached account.error values are historical diagnostics,
                    // not proof that the provider is failing now.
                    ui.observe_usage_update();
                    publish_popup_ui(&ui);
                }
                Ok(WorkerEvent::ProviderUsageRefreshFailed(provider, worker_revision, error)) => {
                    if !provider_worker_event_is_current(&ui, provider, worker_revision)
                        || !live_settings.usage_stats_collection_enabled(provider)
                    {
                        continue;
                    }
                    crate::logger::info(format!(
                        "{} usage refresh failed: {error}",
                        provider.display_name()
                    ));
                    // Usage/analytics failures use the same provider-scoped
                    // presentation as quota failures. The state keeps the
                    // source separate so a successful quota poll does not
                    // clear an active analytics error.
                    ui.set_usage_error(provider, error);
                    publish_popup_ui(&ui);
                }
                Ok(WorkerEvent::ProviderUsageDataCleared(provider, generation)) => {
                    let completed = pending_usage_clear.as_mut().is_some_and(
                        |(pending_generation, providers)| {
                            if *pending_generation != generation {
                                return false;
                            }
                            providers.retain(|pending_provider| *pending_provider != provider);
                            providers.is_empty()
                        },
                    );
                    if completed {
                        for (_, commands) in state.worker_commands() {
                            let _ = commands.send(WorkerCommand::ResumeUsageRefresh(generation));
                        }
                        pending_usage_clear = None;
                    }
                }
                Ok(WorkerEvent::ProviderActivationStarted(provider, worker_revision)) => {
                    if !provider_worker_event_is_current(&ui, provider, worker_revision) {
                        continue;
                    }
                    crate::logger::info(format!("{} activation started", provider.display_name()));
                }
                Ok(WorkerEvent::ProviderActivationSucceeded(provider, worker_revision)) => {
                    if !provider_worker_event_is_current(&ui, provider, worker_revision) {
                        continue;
                    }
                    crate::logger::info(format!(
                        "{} activation succeeded",
                        provider.display_name()
                    ));
                    ui.last_activation = format!(
                        "{} succeeded at {}",
                        provider.display_name(),
                        format_activation_at(Utc::now())
                    );
                    let combine_activation_notification = live_settings.automatic_activation
                        && notification_settings.limits_changed
                        && notification_settings.activation_success;
                    if combine_activation_notification {
                        pending_auto_activation_successes.insert(provider);
                    } else if notification_settings.activation_success {
                        notifications::show_activation_succeeded(provider);
                    }
                    publish_popup_ui(&ui);
                }
                Ok(WorkerEvent::ProviderActivationFailed(provider, worker_revision, error)) => {
                    if !provider_worker_event_is_current(&ui, provider, worker_revision) {
                        continue;
                    }
                    pending_auto_activation_successes.remove(&provider);
                    crate::logger::info(format!(
                        "{} activation failed: {error}",
                        provider.display_name()
                    ));
                    ui.last_activation = format!(
                        "{} failed at {}: {error}",
                        provider.display_name(),
                        format_activation_at(Utc::now())
                    );
                    publish_popup_ui(&ui);
                }
                Ok(WorkerEvent::ProviderPollFailed(provider, worker_revision, error)) => {
                    if !provider_worker_event_is_current(&ui, provider, worker_revision) {
                        continue;
                    }
                    crate::logger::info(format!(
                        "{} polling failed: {error}",
                        provider.display_name()
                    ));
                    ui.set_provider_error(provider, error);
                    publish_popup_ui(&ui);
                }
                // All live provider workers are forwarded as scoped events.
                Ok(
                    WorkerEvent::RequestStarted(_)
                    | WorkerEvent::RequestFinished(_)
                    | WorkerEvent::LimitsUpdated(_)
                    | WorkerEvent::UsageUpdated(_)
                    | WorkerEvent::UsageLoadedFromCache(_)
                    | WorkerEvent::UsageDataCleared(_)
                    | WorkerEvent::UsageRefreshFailed(_)
                    | WorkerEvent::ActivationStarted
                    | WorkerEvent::ActivationSucceeded
                    | WorkerEvent::ActivationFailed(_)
                    | WorkerEvent::PollFailed(_),
                ) => {}
                Ok(WorkerEvent::Stopped) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

#[cfg(windows)]
pub(super) fn pump_tray_and_dismiss(
    tray: &TrayManager,
    _ui_dispatcher: &windows_reactor::UiMarshaller,
    settings_tx: &Sender<Settings>,
    state: &AppState,
    ui: &mut UiState,
) -> bool {
    use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};

    while let Ok(event) = TrayIconEvent::receiver().try_recv() {
        if let TrayIconEvent::Click {
            id,
            position,
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
            && tray.contains(&id)
        {
            if popup::is_visible() {
                // While Settings is open the popup is a live preview, not a
                // transient tray flyout. Keep it available until Settings closes.
                // A click while it is already closing (the press dismissed it)
                // must not reopen it.
                if !crate::settings_window::is_open() {
                    popup::hide();
                }
            } else {
                // Flush suppressed background state so the first frame sees
                // the latest limits/error/activation text.
                flush_popup_ui(ui);
                popup::show_near(position.x as i32, position.y as i32);
            }
        }
    }

    for action in tray.drain_menu_actions() {
        match action {
            TrayMenuAction::Update => {
                if let Err(error) = crate::updater::apply_pending_update() {
                    eprintln!("failed to apply update: {error:#}");
                    notifications::show("Update failed", &format!("{error:#}"));
                }
            }
            TrayMenuAction::Settings => {
                let settings_tx = settings_tx.clone();
                let usage_actions_tx = state.usage_actions_tx.clone();
                let updates = Arc::clone(&state.updates);
                flush_popup_ui(ui);
                // Opening Settings from the tray menu provides the same
                // always-visible live preview as opening it from the footer.
                if !popup::is_visible() || popup::is_closing() {
                    popup::show_near_cursor();
                }
                crate::settings_runtime::dispatch_window(move || {
                    if let Err(error) =
                        crate::settings_window::open(settings_tx, usage_actions_tx, updates)
                    {
                        eprintln!("Could not open settings window: {error:?}");
                    }
                });
            }
            TrayMenuAction::Exit => return true,
        }
    }

    // Settings are a live editor for this surface. Treat the separate settings
    // window as part of the popup interaction so navigating or toggling a
    // setting cannot dismiss the preview beneath it.
    if !crate::settings_window::is_open()
        && !popup::is_closing()
        && (popup::clicked_outside() || popup::escape_pressed())
    {
        popup::hide();
    }
    false
}

#[cfg(not(windows))]
pub(super) fn pump_tray_and_dismiss(
    _tray: &TrayManager,
    _ui_dispatcher: &windows_reactor::UiMarshaller,
    _settings_tx: &Sender<Settings>,
    _state: &AppState,
    _ui: &mut UiState,
) -> bool {
    false
}
