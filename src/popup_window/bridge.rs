use super::*;

use std::collections::HashSet;

/// Rejects queued output from a worker that was replaced, and from instances
/// that were disabled or removed since the event was sent.
pub(super) fn provider_worker_event_is_current(
    ui: &UiState,
    provider: ProviderId,
    worker_revision: u64,
) -> bool {
    ui.provider_enabled(provider) && worker_revision == ui.credentials_revision(provider)
}

fn forced_reset_info_body(reset: &crate::reset_feed::ForcedReset) -> String {
    let local = reset.reset_at.with_timezone(&Local);
    let when = format!(
        "{}, {}",
        crate::i18n::month_day(local),
        TimeFormat::current().format_hm(local)
    );
    let countdown = format_reset_in(Some(reset.reset_at));
    reset.label.as_deref().map_or_else(
        || {
            crate::i18n::format(
                "a-possible-codex-reset-is-scheduled-for-when-in-countdown",
                &[
                    ("when", when.to_string()),
                    ("countdown", countdown.to_string()),
                ],
            )
        },
        |label| {
            crate::i18n::format(
                "label-possible-reset-on-when-in-countdown",
                &[
                    ("label", label.to_string()),
                    ("when", when.to_string()),
                    ("countdown", countdown.to_string()),
                ],
            )
        },
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
            notifications::show(
                crate::i18n::tr("new-codex-reset-info"),
                &forced_reset_info_body(reset),
            );
            state.mark_forced_reset_info_notified(reset.id.clone());
        }
    }
}

/// The notification tracker of one instance, named so its toasts say which
/// instance they are about when a driver has several.
/// Notifies once per login when it enters Claude Code's three-day warning
/// before it stops renewing. A new sign-in moves the deadline and re-arms it.
fn warn_login_expiry(provider: ProviderId, limits: &RateLimits) {
    static WARNED: Mutex<Vec<(ProviderId, DateTime<Utc>)>> = Mutex::new(Vec::new());
    let Some(expires_at) = limits.login_expires_at else {
        return;
    };
    let left = expires_at - Utc::now();
    if left <= chrono::Duration::zero() || left > chrono::Duration::days(3) {
        return;
    }
    let Ok(mut warned) = WARNED.lock() else {
        return;
    };
    if warned.contains(&(provider, expires_at)) {
        return;
    }
    warned.retain(|(warned, _)| *warned != provider);
    warned.push((provider, expires_at));
    let local = expires_at.with_timezone(&chrono::Local);
    crate::notifications::show(
        &crate::i18n::format(
            "login-expires-soon",
            &[("v0", provider.qualified_name().to_string())],
        ),
        &crate::i18n::format(
            "it-stops-renewing-on-open-minibar-and-choose-sign-in-again",
            &[(
                "v0",
                (format!(
                    "{}, {}",
                    crate::i18n::month_day(local),
                    TimeFormat::current().format_hm(local)
                ))
                .to_string(),
            )],
        ),
    );
}

fn instance_tracker(
    trackers: &mut HashMap<String, LimitNotificationTracker>,
    provider: ProviderId,
) -> &mut LimitNotificationTracker {
    trackers
        .entry(provider.id().to_owned())
        .or_default()
        .named(provider.qualified_name())
}

/// Instances whose worker must be replaced: their read-relevant settings
/// (paths, credential source, keys, revision) changed.
pub(super) fn instances_needing_restart(
    before: &[ProviderInstance],
    after: &[ProviderInstance],
) -> Vec<ProviderId> {
    after
        .iter()
        .filter(|instance| instance.enabled)
        .filter(|instance| {
            before
                .iter()
                .find(|old| old.id == instance.id)
                .is_some_and(|old| old.enabled && old.runtime_key() != instance.runtime_key())
        })
        .map(ProviderInstance::provider_id)
        .collect()
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

pub(super) fn start_background_bridge(state: Arc<AppState>) {
    // Use the already hydrated persistent snapshot while the first network
    // refresh is in flight. Opening Settings never starts another poll.
    // Account names live in settings, so overlay them before the first paint.
    let startup_settings = state.settings.clone();
    let _ = state.apply_openrouter_account_names(&startup_settings);
    crate::settings_window::publish_openrouter_snapshot(&state.current_limits());
    let events = state.take_worker_events();
    let mut widgets = state
        .settings
        .tray_widgets
        .iter()
        .filter(|widget| widget.is_visible_for(&state.settings.instances))
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
        // Keyed by instance id.
        let mut limit_notifications = HashMap::<String, LimitNotificationTracker>::new();
        // Credential revision each tracker was primed with. A tracker primed
        // on one login must not compare its reset time with another's.
        let mut notified_revisions = HashMap::<String, u64>::new();
        let mut pending_auto_activation_successes = HashSet::<ProviderId>::new();
        let mut forced_reset_notified_ids = HashSet::<String>::new();
        let mut usage_clear_generation = 0_u64;
        let mut pending_usage_clear: Option<(u64, Vec<ProviderId>)> = None;
        let mut update_phase = updates.snapshot();
        let mut live_settings = state.settings.clone();
        let mut ui = UiState {
            provider_errors: state.startup_provider_errors.iter().cloned().collect(),
            last_activation: format_last_activation(&RateLimits::default(), fallback_attempt),
            update_version: update_version_from_phase(&update_phase),
            ..UiState::popup_layout_from_settings(&state.settings)
        };
        ui.apply_settings(&state.settings);
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
            if !crate::settings_window::has_pending_edits() {
                settings.language.apply();
            }
            crate::settings_window::sync_open_window(settings.clone());
            let phase = updates.snapshot();
            ui.settings_revision = ui.settings_revision.wrapping_add(1);
            // Names and badges are read by every surface; publish them
            // before anything repaints.
            crate::instances::publish(&settings.instances);
            let providers_changed = ui.enabled_providers() != settings.enabled_providers()
                || ui.instances.len() != settings.instances.len();
            let restart = instances_needing_restart(&ui.instances, &settings.instances);
            for provider in &restart {
                // A replaced credential or path must not present the old
                // login's quota as the new one's. The matching worker
                // revision also rejects output already queued by the worker
                // being replaced.
                state.replace_limits(*provider, RateLimits::default());
                ui.observe_limits_update();
            }
            if state.apply_openrouter_account_names(&settings) || !restart.is_empty() {
                // Rename is settings-only. Overlay the new names before the
                // first paint so the popup does not keep the previous label.
                ui.observe_limits_update();
                crate::settings_window::publish_openrouter_snapshot(&state.current_limits());
            }
            if ui.theme != settings.theme || ui.accent_color != settings.accent_color {
                crate::theme::apply_appearance(settings.theme, settings.accent_color);
            }
            ui.apply_settings(&settings);
            *notification_settings = settings.notifications.clone();
            state.sync_reset_feed(&settings);
            if !settings.notifications.forced_reset_feed_enabled {
                state.replace_forced_resets(Vec::new());
                ui.observe_forced_resets_update();
            }
            *widgets = settings
                .tray_widgets
                .iter()
                .filter(|widget| widget.is_visible_for(&settings.instances))
                .cloned()
                .collect();
            ui.update_version = update_version_from_phase(&phase);
            for provider in ui
                .active_requests
                .iter()
                .map(|(provider, _)| *provider)
                .collect::<Vec<_>>()
            {
                if restart.contains(&provider) || !settings.is_enabled(provider) {
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
            ui.provider_errors
                .retain(|provider, _| settings.is_enabled(*provider));
            ui.usage_errors
                .retain(|provider, _| settings.usage_stats_collection_enabled(*provider));
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
                let _ = commands.send(WorkerCommand::SetScheduledActivations(
                    crate::provider::schedules_for(provider, &settings),
                ));
                let _ = commands.send(WorkerCommand::SetAutoActivationPauses(
                    crate::provider::auto_activation_pauses_for(provider, &settings),
                ));
                if let Some(instance) = settings.instance(provider) {
                    let _ = commands.send(WorkerCommand::SetLimitRefreshInterval(
                        Duration::from_secs(instance.refresh_interval().seconds()),
                    ));
                }
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
                    settings.usage_stats_collection_enabled(provider),
                ));
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
             pending: &mut Option<(u64, Vec<ProviderId>)>| {
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
                            ui.set_popup_error(crate::i18n::format(
                                "could-not-clear-usage-data-error",
                                &[("error", format!("{:#}", error))],
                            ));
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
                notifications::show(crate::i18n::tr("update-failed"), &format!("{error:#}"));
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
                if pump_tray_and_dismiss(&tray, &settings_tx, &state, &mut ui) {
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
            if pump_tray_and_dismiss(&tray, &settings_tx, &state, &mut ui) {
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
                    crate::logger::info(format!(
                        "{} limits received: session used={:?}%, reset={:?}; weekly used={:?}%, reset={:?}",
                        provider.display_name(),
                        limits.primary.used_percent,
                        limits.primary.resets_at,
                        limits.secondary.used_percent,
                        limits.secondary.resets_at,
                    ));
                    let mut limits = limits;
                    // The running reader still has the names it started with.
                    if provider.kind() == ProviderKind::OpenRouter
                        && let Some(instance) = live_settings.instance(provider)
                    {
                        crate::openrouter::apply_account_names(&mut limits, instance);
                    }
                    // Publish once, then let both native tray and GPUI render
                    // from that exact snapshot.
                    warn_login_expiry(provider, &limits);
                    state.replace_limits(provider, limits);
                    ui.clear_provider_error(provider);
                    let limits = state.current_limits();
                    crate::settings_window::publish_discovered_popup_bricks(&limits);
                    crate::settings_window::publish_openrouter_snapshot(&limits);
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
                    let revision = ui.credentials_revision(provider);
                    if notified_revisions.insert(provider.id().to_owned(), revision)
                        != Some(revision)
                    {
                        limit_notifications.remove(provider.id());
                    }
                    let tracker = instance_tracker(&mut limit_notifications, provider);
                    let notification_result = if combine_activation_notification {
                        tracker.observe_with_primary_reset_deferred(
                            limits.get(provider),
                            &notification_settings,
                            provider,
                        )
                    } else {
                        tracker.observe(limits.get(provider), &notification_settings, provider)
                    };
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
                    if provider == ProviderId::primary(ProviderKind::Codex) {
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
                    if provider == ProviderId::primary(ProviderKind::Codex)
                        && usage
                            .account_id
                            .as_deref()
                            .is_some_and(|id| id != crate::store::codex_accounts::current_id())
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
                    ui.last_activation = crate::i18n::format(
                        "succeeded-at",
                        &[
                            ("v0", provider.qualified_name().to_string()),
                            ("v1", (format_activation_at(Utc::now())).to_string()),
                        ],
                    );
                    let combine_activation_notification =
                        crate::provider::automatic_activation(provider, &live_settings)
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
                    ui.last_activation = crate::i18n::format(
                        "failed-at-error",
                        &[
                            ("v0", provider.qualified_name().to_string()),
                            ("v1", (format_activation_at(Utc::now())).to_string()),
                            ("error", error.to_string()),
                        ],
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
    _settings_tx: &Sender<Settings>,
    _state: &AppState,
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
                    notifications::show(crate::i18n::tr("update-failed"), &format!("{error:#}"));
                }
            }
            TrayMenuAction::Settings => {
                flush_popup_ui(ui);
                // Opening Settings from the tray menu provides the same
                // always-visible live preview as opening it from the footer.
                if !popup::is_visible() || popup::is_closing() {
                    popup::show_near_cursor();
                }
                crate::settings_window::open();
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
    _settings_tx: &Sender<Settings>,
    _state: &AppState,
    _ui: &mut UiState,
) -> bool {
    false
}
