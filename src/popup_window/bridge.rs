use super::*;

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
    set_ui: AsyncSetState<UiState>,
    ui_dispatcher: UiMarshaller,
) {
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
    let settings_tx = state.settings_tx.clone();
    let updates = Arc::clone(&state.updates);
    let mut check_for_updates = state.settings.check_for_updates;
    let mut notify_on_update = state.settings.notifications.update_available;

    thread::spawn(move || {
        let mut tray = TrayManager::new();
        let fallback_attempt = state.last_activation_at;
        let mut notification_settings = state.settings.notifications.clone();
        let mut limit_notifications = HashMap::<ProviderKind, LimitNotificationTracker>::new();
        let mut update_phase = updates.snapshot();
        let mut ui = UiState {
            theme: state.settings.theme,
            accent_color: state.settings.accent_color,
            animations_enabled: state.settings.animations_enabled,
            popup_background_material: state.settings.popup_background_material,
            provider_errors: state.startup_provider_errors.iter().cloned().collect(),
            last_activation: format_last_activation(&RateLimits::default(), fallback_attempt),
            show_used_percentage: state.settings.show_used_percentage,
            show_usage_pace: state.settings.show_usage_pace,
            compact_usage_cards: state.settings.compact_usage_cards,
            popup_visibility: state.settings.popup_visibility.clone(),
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
            openrouter_credentials_revision: state.settings.openrouter_credentials_revision,
            popup_order: state.settings.popup_order.clone(),
            use_colored_provider_icons: state.settings.use_colored_provider_icons,
            replace_chatgpt_logo_with_codex: state.settings.replace_chatgpt_logo_with_codex,
            codex_path: state.settings.codex_path.clone(),
            claude_path: state.settings.claude_path.clone(),
            cursor_path: state.settings.cursor_path.clone(),
            update_version: update_version_from_phase(&update_phase),
            ..UiState::default()
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
            flush_popup_ui(&set_ui, &ui);
        }

        // Keep trying until the WinUI window exists, then park it as a popup.
        for _ in 0..50 {
            if popup::ensure_configured().is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        let apply_settings = |ui: &mut UiState,
                              set_ui: &AsyncSetState<UiState>,
                              notification_settings: &mut NotificationSettings,
                              widgets: &mut Vec<TrayWidget>,
                              tray: &mut TrayManager,
                              settings: Settings| {
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
                || ui.openrouter_enabled != settings.providers.is_enabled(ProviderKind::OpenRouter);
            let opencode_zen_credentials_changed =
                ui.opencode_zen_credentials_revision != settings.opencode_zen_credentials_revision;
            let opencode_go_credentials_changed =
                ui.opencode_go_credentials_revision != settings.opencode_go_credentials_revision;
            let openrouter_credentials_changed =
                ui.openrouter_credentials_revision != settings.openrouter_credentials_revision;
            ui.theme = settings.theme;
            ui.accent_color = settings.accent_color;
            ui.animations_enabled = settings.animations_enabled;
            ui.popup_background_material = settings.popup_background_material;
            ui.time_format = settings.time_format;
            ui.show_used_percentage = settings.show_used_percentage;
            ui.show_usage_pace = settings.show_usage_pace;
            ui.compact_usage_cards = settings.compact_usage_cards;
            ui.popup_visibility = settings.popup_visibility.clone();
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
            ui.openrouter_credentials_revision = settings.openrouter_credentials_revision;
            // Keep the previous OpenRouter snapshot visible while the worker
            // restarts. Wiping to Default made the tab go blank for the full
            // sequential /key poll (15s timeout × each key) after adding a key.
            ui.popup_order = settings.popup_order.clone();
            ui.use_colored_provider_icons = settings.use_colored_provider_icons;
            ui.replace_chatgpt_logo_with_codex = settings.replace_chatgpt_logo_with_codex;
            *notification_settings = settings.notifications.clone();
            *widgets = settings
                .tray_widgets
                .iter()
                .filter(|widget| widget.is_visible_for(&settings.providers))
                .cloned()
                .collect();
            ui.update_version = update_version_from_phase(&phase);
            // Presentation settings must visibly apply before any background
            // work. In particular, changing provider icons must never wait on
            // a worker lock, network request, or provider lifecycle change.
            flush_popup_ui(set_ui, ui);
            let restart = [
                (ProviderKind::Codex, settings.codex_path != ui.codex_path),
                (ProviderKind::Claude, settings.claude_path != ui.claude_path),
                (ProviderKind::Cursor, settings.cursor_path != ui.cursor_path),
                (ProviderKind::OpenRouter, openrouter_credentials_changed),
            ]
            .into_iter()
            .filter_map(|(provider, changed)| changed.then_some(provider))
            .collect::<Vec<_>>();
            ui.codex_path = settings.codex_path.clone();
            ui.claude_path = settings.claude_path.clone();
            ui.cursor_path = settings.cursor_path.clone();
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
                    settings.automatic_activation
                        && crate::provider_registry::descriptor(provider).supports_activation,
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
                // The worker refreshes immediately after receiving this command,
                // so the selected history range is reflected in the open popup
                // without asking the user to restart the application.
                let _ = commands.send(WorkerCommand::SetHistoryRetentionDays(
                    settings.history_retention_days,
                ));
                if (provider == ProviderKind::OpenCodeZen && opencode_zen_credentials_changed)
                    || (provider == ProviderKind::OpenCodeGo && opencode_go_credentials_changed)
                    || (provider == ProviderKind::OpenRouter && openrouter_credentials_changed)
                {
                    let _ = commands.send(WorkerCommand::Refresh);
                }
            }
            flush_popup_ui(set_ui, ui);
        };

        let drain_settings = |ui: &mut UiState,
                              set_ui: &AsyncSetState<UiState>,
                              notification_settings: &mut NotificationSettings,
                              widgets: &mut Vec<TrayWidget>,
                              tray: &mut TrayManager,
                              check_for_updates: &mut bool,
                              notify_on_update: &mut bool| {
            let Some(settings_rx) = settings_rx.as_ref() else {
                return;
            };
            while let Ok(settings) = settings_rx.try_recv() {
                if settings.check_for_updates && !*check_for_updates {
                    updates.check_async(false, settings.notifications.update_available);
                }
                *check_for_updates = settings.check_for_updates;
                *notify_on_update = settings.notifications.update_available;
                apply_settings(ui, set_ui, notification_settings, widgets, tray, settings);
            }
        };

        let drain_updates = |ui: &mut UiState,
                             set_ui: &AsyncSetState<UiState>,
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
            publish_popup_ui(set_ui, ui);
        };

        let drain_toast_update = || {
            if crate::notifications::take_toast_update_request()
                && let Err(error) = crate::updater::apply_pending_update()
            {
                eprintln!("failed to apply update from toast: {error:#}");
                notifications::show("Update failed", &format!("{error:#}"));
            }
        };

        let Some(events) = events else {
            publish_popup_ui(&set_ui, &ui);
            loop {
                popup::pump_messages();
                drain_toast_update();
                if let Err(error) = tray.refresh_system_theme(&widgets, &state.current_limits()) {
                    ui.set_popup_error(error.to_string());
                    publish_popup_ui(&set_ui, &ui);
                }
                drain_settings(
                    &mut ui,
                    &set_ui,
                    &mut notification_settings,
                    &mut widgets,
                    &mut tray,
                    &mut check_for_updates,
                    &mut notify_on_update,
                );
                drain_updates(&mut ui, &set_ui, &mut tray, &mut update_phase, &mut widgets);
                if pump_tray_and_dismiss(
                    &tray,
                    &ui_dispatcher,
                    &settings_tx,
                    &state,
                    &mut ui,
                    &set_ui,
                ) {
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
            if let Err(error) = tray.refresh_system_theme(&widgets, &state.current_limits()) {
                ui.set_popup_error(error.to_string());
                publish_popup_ui(&set_ui, &ui);
            }
            drain_settings(
                &mut ui,
                &set_ui,
                &mut notification_settings,
                &mut widgets,
                &mut tray,
                &mut check_for_updates,
                &mut notify_on_update,
            );
            drain_updates(&mut ui, &set_ui, &mut tray, &mut update_phase, &mut widgets);
            if pump_tray_and_dismiss(
                &tray,
                &ui_dispatcher,
                &settings_tx,
                &state,
                &mut ui,
                &set_ui,
            ) {
                drop(tray);
                state.shutdown_worker();
                std::process::exit(0);
            }
            match events.recv_timeout(Duration::from_millis(16)) {
                Ok(WorkerEvent::ProviderRequestStarted(provider, kind)) => {
                    ui.request_started(provider, kind);
                    publish_popup_ui(&set_ui, &ui);
                }
                Ok(WorkerEvent::ProviderRequestFinished(provider, kind)) => {
                    ui.request_finished(provider, kind);
                    publish_popup_ui(&set_ui, &ui);
                }
                Ok(WorkerEvent::ProviderLimitsUpdated(provider, limits)) => {
                    if (provider == ProviderKind::Codex && !ui.codex_enabled)
                        || (provider == ProviderKind::Claude && !ui.claude_enabled)
                        || (provider == ProviderKind::Cursor && !ui.cursor_enabled)
                        || (provider == ProviderKind::OpenCodeZen && !ui.opencode_zen_enabled)
                        || (provider == ProviderKind::OpenCodeGo && !ui.opencode_go_enabled)
                        || (provider == ProviderKind::OpenRouter && !ui.openrouter_enabled)
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
                    // Publish once, then let both native tray and WinUI render
                    // from that exact snapshot.
                    state.replace_limits(provider, limits);
                    ui.clear_provider_error(provider);
                    let limits = state.current_limits();
                    crate::settings_window::publish_discovered_popup_bricks(
                        &limits,
                        ui_dispatcher.clone(),
                    );
                    if ui.popup_visibility.absorb_discovered_bricks(&limits) {
                        let limits_for_settings = limits.clone();
                        crate::settings_window::persist_update(
                            settings_tx.clone(),
                            move |settings| {
                                settings.absorb_discovered_popup_bricks(&limits_for_settings);
                            },
                        );
                    }
                    limit_notifications.entry(provider).or_default().observe(
                        limits.get(provider),
                        &notification_settings,
                        provider,
                    );
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
                    publish_popup_ui(&set_ui, &ui);
                }
                Ok(WorkerEvent::ProviderUsageUpdated(provider, usage)) => {
                    if (provider == ProviderKind::Codex && !ui.codex_enabled)
                        || (provider == ProviderKind::Claude && !ui.claude_enabled)
                        || (provider == ProviderKind::Cursor && !ui.cursor_enabled)
                        || (provider == ProviderKind::OpenCodeZen && !ui.opencode_zen_enabled)
                        || (provider == ProviderKind::OpenCodeGo && !ui.opencode_go_enabled)
                        || (provider == ProviderKind::OpenRouter && !ui.openrouter_enabled)
                    {
                        continue;
                    }
                    crate::logger::info(format!(
                        "{} usage received: today={} tokens, history={} tokens",
                        provider.display_name(),
                        usage.today.total_tokens(),
                        usage.history.total_tokens()
                    ));
                    state.replace_usage(provider, usage);
                    // Usage stats affect only the popup, but they share the
                    // reactive snapshot revision with quota updates.
                    ui.observe_limits_update();
                    publish_popup_ui(&set_ui, &ui);
                }
                Ok(WorkerEvent::ProviderActivationStarted(provider)) => {
                    crate::logger::info(format!("{} activation started", provider.display_name()));
                }
                Ok(WorkerEvent::ProviderActivationSucceeded(provider)) => {
                    crate::logger::info(format!(
                        "{} activation succeeded",
                        provider.display_name()
                    ));
                    ui.last_activation = format!(
                        "{} succeeded at {}",
                        provider.display_name(),
                        format_activation_at(Utc::now())
                    );
                    if notification_settings.activation_success {
                        notifications::show_activation_succeeded(provider);
                    }
                    publish_popup_ui(&set_ui, &ui);
                }
                Ok(WorkerEvent::ProviderActivationFailed(provider, error)) => {
                    crate::logger::info(format!(
                        "{} activation failed: {error}",
                        provider.display_name()
                    ));
                    ui.last_activation = format!(
                        "{} failed at {}: {error}",
                        provider.display_name(),
                        format_activation_at(Utc::now())
                    );
                    publish_popup_ui(&set_ui, &ui);
                }
                Ok(WorkerEvent::ProviderPollFailed(provider, error)) => {
                    crate::logger::info(format!(
                        "{} polling failed: {error}",
                        provider.display_name()
                    ));
                    ui.set_provider_error(provider, error);
                    publish_popup_ui(&set_ui, &ui);
                }
                // All live provider workers are forwarded as scoped events.
                Ok(
                    WorkerEvent::RequestStarted(_)
                    | WorkerEvent::RequestFinished(_)
                    | WorkerEvent::LimitsUpdated(_)
                    | WorkerEvent::UsageUpdated(_)
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
    ui_dispatcher: &UiMarshaller,
    settings_tx: &Sender<Settings>,
    state: &AppState,
    ui: &mut UiState,
    set_ui: &AsyncSetState<UiState>,
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
            let x = position.x as i32;
            let y = position.y as i32;
            if popup::is_visible() {
                // While Settings is open the popup is a live preview, not a
                // transient tray flyout. Keep it available until Settings closes.
                if !crate::settings_window::is_open() {
                    ui_dispatcher.dispatch(popup::hide);
                }
            } else {
                // Activation and motion publication both belong to WinUI's
                // thread. Publishing the animation from this tray worker used
                // to strand the HWND just beyond the monitor edge forever.
                // Flush suppressed background UiState so the first frame sees
                // the latest limits/error/activation text.
                flush_popup_ui(set_ui, ui);
                let (ready_tx, ready_rx) = std::sync::mpsc::channel();
                ui_dispatcher.dispatch(move || {
                    let ready = popup::prepare_show_on_ui_thread();
                    if ready {
                        popup::show_near(x, y);
                    }
                    let _ = ready_tx.send(ready);
                });
                match ready_rx.recv_timeout(std::time::Duration::from_millis(500)) {
                    Ok(true) => {}
                    Ok(false) => {
                        eprintln!("popup host was unavailable during synchronous reactivation");
                    }
                    Err(error) => eprintln!("popup reactivation timed out: {error}"),
                }
            }
            ui_dispatcher.dispatch(popup::hide_from_switchers);
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
                let updates = Arc::clone(&state.updates);
                flush_popup_ui(set_ui, ui);
                ui_dispatcher.dispatch(move || {
                    // Opening Settings from the tray menu should provide the
                    // same always-visible live preview as opening it from the
                    // popup footer.
                    if !popup::is_visible() && popup::prepare_show_on_ui_thread() {
                        popup::show_near_cursor();
                    }
                    if let Err(error) = crate::settings_window::open(settings_tx, updates) {
                        eprintln!("Could not open settings window: {error:?}");
                    }
                });
            }
            TrayMenuAction::Exit => return true,
        }
    }

    // HWND geometry belongs to the WinUI thread. Coalesce the 60 Hz tray pump
    // into at most one pending UI task so a busy dispatcher cannot accumulate
    // an unbounded tail of stale SetWindowPos calls.
    if popup::is_visible() && !KEEP_ON_MONITOR_QUEUED.swap(true, Ordering::SeqCst) {
        ui_dispatcher.dispatch(|| {
            popup::keep_on_monitor();
            KEEP_ON_MONITOR_QUEUED.store(false, Ordering::SeqCst);
        });
    }

    // Settings are a live editor for this surface. Treat the separate settings
    // window as part of the popup interaction so navigating or toggling a
    // setting cannot dismiss the preview beneath it.
    if !crate::settings_window::is_open()
        && !popup::is_closing()
        && (popup::clicked_outside() || popup::escape_pressed())
    {
        ui_dispatcher.dispatch(popup::hide);
    }
    false
}

#[cfg(not(windows))]
pub(super) fn pump_tray_and_dismiss(
    _tray: &TrayManager,
    _ui_dispatcher: &UiMarshaller,
    _settings_tx: &Sender<Settings>,
    _state: &AppState,
    _ui: &mut UiState,
    _set_ui: &AsyncSetState<UiState>,
) -> bool {
    false
}
