use super::*;

pub fn app(cx: &mut RenderCx, state: Arc<AppState>) -> Element {
    let dpi = cx.use_dpi().max(1);
    // Pin the root to the live client size. Stretch alone is not enough: during
    // shell-height animation the tree otherwise keeps its content DesiredSize
    // and sits top-aligned in a taller HWND, leaving a black band under the footer.
    let window_size = cx.use_inner_size();
    let color_scheme = cx.use_color_scheme();
    let bottom_bar_size = popup::bottom_bar_size();
    let window_corner_radius = f64::from(popup::corner_radius_dip());
    // Keep the content one physical pixel inside the selected backdrop stroke so GDI's
    // aliased region cannot trim its anti-aliased outer corner pixels.
    let border_inset = 96.0 / f64::from(dpi);
    let (ui, set_ui) = cx.use_async_state(UiState {
        theme: state.settings.theme,
        accent_color: state.settings.accent_color,
        animations_enabled: state.settings.animations_enabled,
        time_format: state.settings.time_format,
        provider_errors: state.startup_provider_errors.iter().cloned().collect(),
        last_activation: format_last_activation(&RateLimits::default(), state.last_activation_at),
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
        update_version: state
            .updates
            .available_update()
            .map(|update| update.version),
        ..UiState::default()
    });
    cx.use_effect(
        (
            ui.theme,
            ui.accent_color,
            ui.animations_enabled,
            ui.time_format,
        ),
        move || {
            crate::theme::set_animations_enabled(ui.animations_enabled);
            crate::theme::apply_appearance(ui.theme, ui.accent_color);
            ui.time_format.apply();
        },
    );
    // Rendering observes the same snapshot that the tray consumes; UiState
    // deliberately contains only view metadata, never a second copy of limits.
    let limits = state.current_limits();
    let commands = state.worker_commands();
    let ui_dispatcher = cx.use_ui_marshaller();
    let settings_tx = state.settings_tx.clone();
    let (hovered_action, set_hovered_action) = cx.use_state(Option::<String>::None);
    let (tab_scroll_x, set_tab_scroll_x) = cx.use_state(0.0_f64);
    let (widget_drag, set_widget_drag) = cx.use_state(None::<WidgetDragState>);
    let (overview_metric, set_overview_metric) = cx.use_state(OverviewMetric::default());
    let (overview_range, set_overview_range) = cx.use_state(OverviewRange::default());
    let (overview_breakdown, set_overview_breakdown) = cx.use_state(BreakdownMode::default());
    let (overview_chart_hover, set_overview_chart_hover) = cx.use_state(None::<usize>);
    let (pager, pager_dispatch) = cx.use_reducer_fn(reduce_pager, PagerState::default());
    let (hovered_combined_usage_period, set_hovered_combined_usage_period) =
        cx.use_state(None::<TotalSpendPeriod>);
    let (hovered_usage_stats, set_hovered_usage_stats) =
        cx.use_state(None::<UsageStatsHover>);
    // Relative timestamps need an occasional render tick while the popup is
    // visible. `prepare_show_on_ui_thread` requests an immediate render on
    // every open, so there is no reason to reconcile the entire hidden WinUI
    // tree once per second for the lifetime of the process.
    let (clock_tick, set_clock_tick) = cx.use_async_state(0_u64);
    let page_animations_enabled = ui.animations_enabled && popup::system_animations_enabled();
    let (refresh_rotation, set_refresh_rotation) = cx.use_async_state(0.0_f64);

    cx.use_effect_with_cleanup((ui.refreshing, page_animations_enabled), {
        let set_refresh_rotation = set_refresh_rotation.clone();
        move || {
            if !ui.refreshing || !page_animations_enabled {
                set_refresh_rotation.call(0.0);
                return None;
            }

            set_refresh_rotation.call(0.0);
            let started_at = Instant::now();
            let timer = DispatcherTimer::new(Duration::from_millis(16), move || {
                if popup::is_visible() {
                    set_refresh_rotation.call(refresh_rotation_at(started_at.elapsed()));
                }
            })
            .ok();
            Some(move || drop(timer))
        }
    });

    cx.use_effect_with_cleanup(
        (
            pager.animation_id,
            pager.outgoing.is_some(),
            page_animations_enabled,
        ),
        {
            let pager_dispatch = pager_dispatch.clone();
            move || {
                let timer = pager.outgoing.and_then(|_| {
                    let duration = if page_animations_enabled {
                        PAGER_ANIMATION_DURATION
                    } else {
                        Duration::from_millis(1)
                    };
                    DispatcherTimer::new_one_shot(duration, move || {
                        pager_dispatch.call(PagerAction::AnimationFinished(pager.animation_id));
                    })
                    .ok()
                });
                Some(move || drop(timer))
            }
        },
    );

    cx.use_effect(
        (
            pager.current,
            ui.codex_enabled,
            ui.claude_enabled,
            ui.cursor_enabled,
            ui.opencode_zen_enabled,
            ui.opencode_go_enabled,
            ui.openrouter_enabled,
            popup_order_key(&ui.popup_order),
        ),
        {
            let pager_dispatch = pager_dispatch.clone();
            let order = provider_order_from_popup(&ui.popup_order);
            move || {
                pager_dispatch.call(PagerAction::SetProviderOrder(order.clone()));
                let available = match pager.current {
                    PopupView::Home | PopupView::Usage => true,
                    PopupView::Codex => ui.codex_enabled,
                    PopupView::Claude => ui.claude_enabled,
                    PopupView::Cursor => ui.cursor_enabled,
                    PopupView::OpenCodeZen => ui.opencode_zen_enabled,
                    PopupView::OpenCodeGo => ui.opencode_go_enabled,
                    PopupView::OpenRouter => ui.openrouter_enabled,
                };
                if !available {
                    pager_dispatch.call(PagerAction::Select(PopupView::Home));
                }
            }
        },
    );

    cx.use_effect((), {
        let state = Arc::clone(&state);
        let set_ui = set_ui.clone();
        let ui_dispatcher = ui_dispatcher.clone();
        move || {
            // Convert the WinUI window into a hidden tray popup as soon as it exists.
            let _ = popup::ensure_configured();
            popup::sync_host_constraints();
            // SystemBackdrop paints square + shadow past SetWindowRgn — keep it off.
            set_backdrop(None);
            start_background_bridge(state, set_ui, ui_dispatcher);
        }
    });

    cx.use_effect((), {
        let set_clock_tick = set_clock_tick.clone();
        move || {
            thread::spawn(move || {
                let mut tick = 0_u64;
                loop {
                    thread::sleep(Duration::from_secs(60));
                    if popup::is_visible() {
                        tick = tick.wrapping_add(1);
                        set_clock_tick.call(tick);
                    }
                }
            });
        }
    });

    let refresh = {
        let commands = commands.clone();
        let set_ui = set_ui.clone();
        let ui = ui.clone();
        move || {
            if refresh_all_workers(&commands) {
                let mut ui = ui.clone();
                ui.refreshing = true;
                set_ui.call(ui);
            }
        }
    };
    // A selector only earns its keep when it can actually switch between
    // providers. With zero or one enabled provider the familiar compact
    // footer remains, sparing us some very professional-looking empty UI.
    let enabled_provider_order = provider_order_from_popup(&ui.popup_order)
        .into_iter()
        .filter(|provider| {
            provider_is_enabled(
                *provider,
                ui.codex_enabled,
                ui.claude_enabled,
                ui.cursor_enabled,
                ui.opencode_zen_enabled,
                ui.opencode_go_enabled,
                ui.openrouter_enabled,
            )
        })
        .collect::<Vec<_>>();

    let enabled_provider_count = enabled_provider_order.len();
    let show_provider_icon_tabs = enabled_provider_count > 1;
    let show_provider_tabs = show_provider_icon_tabs;
    let show_footer_tabs = true;
    let selected_view = pager.current;
    let show_total_spend = ui.show_total_spend_on_all_tab
        && total_spend_provider_count(
            ui.codex_enabled,
            ui.claude_enabled,
            ui.cursor_enabled,
            ui.opencode_zen_enabled,
            ui.opencode_go_enabled,
        ) > 1;
    let all_tab_widgets = visible_popup_widgets(
        &ui.popup_order,
        show_total_spend && show_provider_icon_tabs,
        &ui.popup_visibility,
        ui.codex_enabled,
        ui.claude_enabled,
        ui.cursor_enabled,
        ui.opencode_zen_enabled,
        ui.opencode_go_enabled,
        ui.openrouter_enabled,
    );
    let can_reorder_widgets = selected_view == PopupView::Home && all_tab_widgets.len() > 1;
    let build_body = |view: PopupView, retain_disabled_detail: bool| {
        let surface = if view == PopupView::Home {
            PopupSurface::HomeTab
        } else {
            PopupSurface::ProviderTab
        };
        let show_total_spend = show_total_spend && view == PopupView::Home;

        let mut body: Vec<Element> = Vec::new();
        let mut has_preceding_section = false;
        if let Some(error) = ui.error.clone() {
            body.push(
                InfoBar::new("Something went wrong")
                    .message(error)
                    .error()
                    .is_closable(false)
                    .with_key("popup-error")
                    .into(),
            );
        }
        if let Some(provider) = view.provider()
            && let Some(error) = ui.provider_error(provider)
        {
            body.push(
                InfoBar::new(format!("{} error", provider.display_name()))
                    .message(error)
                    .error()
                    .is_closable(false)
                    .with_key(format!("popup-provider-error-{}", provider.id()))
                    .into(),
            );
        }

        if view == PopupView::Home {
            let widgets = visible_popup_widgets(
                &ui.popup_order,
                show_total_spend,
                &ui.popup_visibility,
                ui.codex_enabled,
                ui.claude_enabled,
                ui.cursor_enabled,
                ui.opencode_zen_enabled,
                ui.opencode_go_enabled,
                ui.openrouter_enabled,
            );
            for (index, widget) in widgets.into_iter().enumerate() {
                let is_first = index == 0 && !has_preceding_section;
                let section = match widget {
                    PopupWidgetKind::TotalSpend => {
                        let on_period = {
                            let settings_tx = settings_tx.clone();
                            let set_ui = set_ui.clone();
                            let ui = ui.clone();
                            move |period| {
                                persist_total_spend_period(
                                    settings_tx.clone(),
                                    set_ui.clone(),
                                    ui.clone(),
                                    period,
                                );
                            }
                        };
                        combined_usage_card(
                            &limits,
                            is_first,
                            ui.codex_enabled,
                            ui.claude_enabled,
                            ui.cursor_enabled,
                            ui.opencode_zen_enabled,
                            ui.opencode_go_enabled,
                            ui.openrouter_enabled,
                            ui.total_spend_period,
                            on_period,
                            hovered_combined_usage_period,
                            set_hovered_combined_usage_period.clone(),
                            color_scheme,
                            ui.use_colored_provider_icons,
                            ui.total_spend_presentation,
                            can_reorder_widgets.then(|| {
                                drag_handle(
                                    PopupWidgetKind::TotalSpend,
                                    color_scheme,
                                    &widget_drag,
                                    set_widget_drag.clone(),
                                )
                            }),
                            {
                                let pager_dispatch = pager_dispatch.clone();
                                move || pager_dispatch.call(PagerAction::Select(PopupView::Usage))
                            },
                            hovered_usage_stats,
                            set_hovered_usage_stats.clone(),
                        )
                        .with_key(format!(
                            "all-combined-usage-{}-{:?}-{}-{}-{}",
                            ui.total_spend_period.key(),
                            ui.total_spend_presentation,
                            ui.use_colored_provider_icons,
                            color_scheme as i32,
                            if is_first { "first" } else { "rest" }
                        ))
                    }
                    provider_widget => {
                        let provider = provider_widget.as_provider().expect("provider widget");
                        let limits_for_provider = limits.get(provider);
                        let provider_error = ui.provider_error(provider).map(|error| {
                            let pager_dispatch = pager_dispatch.clone();
                            (
                                error,
                                Callback::new(move |()| {
                                    pager_dispatch.call(PagerAction::Select(
                                        PopupView::from_provider(provider),
                                    ));
                                }),
                            )
                        });
                        let handle = can_reorder_widgets.then(|| {
                            drag_handle(
                                provider_widget,
                                color_scheme,
                                &widget_drag,
                                set_widget_drag.clone(),
                            )
                        });
                        vstack(provider_cards(
                            provider,
                            is_first,
                            limits_for_provider,
                            ui.show_used_percentage,
                            ui.show_usage_pace,
                            ui.compact_usage_cards,
                            &ui.popup_visibility,
                            PopupSurface::HomeTab,
                            show_provider_tabs,
                            ui.show_account_name,
                            color_scheme,
                            handle,
                            (provider == ProviderKind::OpenRouter).then(|| {
                                OpenRouterPopupActions {
                                    settings_tx: settings_tx.clone(),
                                    hovered_action: hovered_action.clone(),
                                    set_hovered_action: set_hovered_action.clone(),
                                    now: Utc::now(),
                                }
                            }),
                            provider_error,
                        ))
                        .spacing(6.0)
                        .with_key(format!(
                            "provider-{}-{}-{}",
                            provider.id(),
                            if is_first { "first" } else { "rest" },
                            if provider == ProviderKind::OpenRouter {
                                openrouter_accounts_strip_key(limits_for_provider)
                            } else {
                                String::new()
                            }
                        ))
                        .into()
                    }
                };
                let section = if can_reorder_widgets {
                    with_widget_drop_target(
                        widget,
                        section,
                        &widget_drag,
                        set_widget_drag.clone(),
                        settings_tx.clone(),
                        set_ui.clone(),
                        ui.clone(),
                    )
                } else {
                    section
                };
                body.push(section);
                has_preceding_section = true;
            }
        } else if view == PopupView::Usage {
            let enabled_spend: Vec<ProviderKind> = enabled_provider_order
                .iter()
                .copied()
                .filter(|provider| {
                    crate::provider_registry::PROVIDERS
                        .iter()
                        .any(|descriptor| {
                            descriptor.kind == *provider && descriptor.include_in_total_spend
                        })
                })
                .collect();
            let snapshot =
                build_overview_snapshot(&limits, &enabled_spend, overview_metric, overview_range);
            body.push(crate::popup_usage::overview_page(
                &snapshot,
                overview_metric,
                overview_range,
                overview_breakdown,
                overview_chart_hover,
                color_scheme,
                ui.use_colored_provider_icons,
                set_overview_metric.clone(),
                set_overview_range.clone(),
                set_overview_breakdown.clone(),
                set_overview_chart_hover.clone(),
            ));
        } else {
            let providers_for_view: Vec<ProviderKind> = view
                .provider()
                .filter(|provider| {
                    provider_is_enabled(
                        *provider,
                        ui.codex_enabled,
                        ui.claude_enabled,
                        ui.cursor_enabled,
                        ui.opencode_zen_enabled,
                        ui.opencode_go_enabled,
                        ui.openrouter_enabled,
                    ) || retain_disabled_detail
                })
                .into_iter()
                .collect();
            for provider in providers_for_view {
                let limits_for_provider = limits.get(provider);
                let provider_error = ui.provider_error(provider).map(|error| {
                    let pager_dispatch = pager_dispatch.clone();
                    (
                        error,
                        Callback::new(move |()| {
                            pager_dispatch
                                .call(PagerAction::Select(PopupView::from_provider(provider)));
                        }),
                    )
                });
                body.push(
                    vstack(provider_cards(
                        provider,
                        !has_preceding_section,
                        limits_for_provider,
                        ui.show_used_percentage,
                        ui.show_usage_pace,
                        ui.compact_usage_cards,
                        &ui.popup_visibility,
                        surface,
                        show_provider_tabs,
                        ui.show_account_name,
                        color_scheme,
                        None,
                        (provider == ProviderKind::OpenRouter).then(|| OpenRouterPopupActions {
                            settings_tx: settings_tx.clone(),
                            hovered_action: hovered_action.clone(),
                            set_hovered_action: set_hovered_action.clone(),
                            now: Utc::now(),
                        }),
                        provider_error,
                    ))
                    .spacing(6.0)
                    .with_key(format!(
                        "provider-{}-{}",
                        provider.id(),
                        if provider == ProviderKind::OpenRouter {
                            openrouter_accounts_strip_key(limits_for_provider)
                        } else {
                            String::new()
                        }
                    ))
                    .into(),
                );
                has_preceding_section = true;
            }
        }
        if !ui.codex_enabled
            && !ui.claude_enabled
            && !ui.cursor_enabled
            && !ui.opencode_zen_enabled
            && !ui.opencode_go_enabled
            && !ui.openrouter_enabled
        {
            body.push(
                InfoBar::new("No providers enabled")
                    .message("Turn one on in Settings > Providers.")
                    .is_closable(false)
                    .with_key("popup-no-providers")
                    .into(),
            );
        }
        body
    };

    let body = build_body(selected_view, false);
    let outgoing_body = pager.outgoing.map(|view| build_body(view, true));

    let footer_background = match color_scheme {
        // Low-alpha overlay keeps the selected material visible beneath chrome.
        ColorScheme::Dark => Color {
            a: 0x24,
            r: 0,
            g: 0,
            b: 0,
        },
        ColorScheme::Light => Color {
            a: 0x0d,
            r: 0,
            g: 0,
            b: 0,
        },
    };

    let footer_identity: Element = if show_footer_tabs {
        // Build only live tabs — never pad with Element::Empty. Empty siblings
        // collapse during reconcile and let swap-chain hosts keep a prior
        // provider's pixels in another tab's slot.
        let provider_tab_count = if show_provider_icon_tabs {
            enabled_provider_order.len()
        } else {
            0
        };
        let tab_content_width = provider_tab_strip_content_width(provider_tab_count);
        let tab_viewport_width = provider_tab_strip_viewport_width();
        let tab_max_offset = (tab_content_width - tab_viewport_width).max(0.0);
        let tab_scroll_x = tab_scroll_x.clamp(0.0, tab_max_offset);
        let on_tab_wheel = Callback::new({
            let set_tab_scroll_x = set_tab_scroll_x.clone();
            move |info: PointerEventInfo| {
                if info.wheel_delta == 0 {
                    return;
                }
                let step = f64::from(info.wheel_delta) / 120.0 * 48.0;
                let dx = if info.wheel_is_horizontal {
                    step
                } else {
                    -step
                };
                let next = (tab_scroll_x + dx).clamp(0.0, tab_max_offset);
                if (next - tab_scroll_x).abs() > 0.5 {
                    set_tab_scroll_x.call(next);
                }
            }
        });
        let mut provider_tabs = vec![popup_tab_button(
            "provider-tab-home",
            Some("fluent-home"),
            None,
            "Home",
            selected_view == PopupView::Home,
            false,
            ui.use_colored_provider_icons,
            color_scheme,
            &hovered_action,
            set_hovered_action.clone(),
            on_tab_wheel.clone(),
            {
                let pager_dispatch = pager_dispatch.clone();
                move || pager_dispatch.call(PagerAction::Select(PopupView::Home))
            },
        )];
        provider_tabs.push(popup_tab_button(
            "provider-tab-usage",
            Some("fluent-chart"),
            None,
            "Usage",
            selected_view == PopupView::Usage,
            false,
            ui.use_colored_provider_icons,
            color_scheme,
            &hovered_action,
            set_hovered_action.clone(),
            on_tab_wheel.clone(),
            {
                let pager_dispatch = pager_dispatch.clone();
                move || pager_dispatch.call(PagerAction::Select(PopupView::Usage))
            },
        ));
        if show_provider_icon_tabs {
            for provider in &enabled_provider_order {
                let (tab_id, icon_name, tip, view) = match provider {
                    ProviderKind::Codex => (
                        "provider-tab-codex",
                        if ui.replace_chatgpt_logo_with_codex {
                            "codex"
                        } else {
                            "chatgpt"
                        },
                        "Codex",
                        PopupView::Codex,
                    ),
                    ProviderKind::Claude => {
                        ("provider-tab-claude", "claude", "Claude", PopupView::Claude)
                    }
                    ProviderKind::Cursor => {
                        ("provider-tab-cursor", "cursor", "Cursor", PopupView::Cursor)
                    }
                    ProviderKind::OpenCodeZen => (
                        "provider-tab-opencode-zen",
                        "opencode",
                        "OpenCode Zen",
                        PopupView::OpenCodeZen,
                    ),
                    ProviderKind::OpenCodeGo => (
                        "provider-tab-opencode-go",
                        "opencode",
                        "OpenCode Go",
                        PopupView::OpenCodeGo,
                    ),
                    ProviderKind::OpenRouter => (
                        "provider-tab-openrouter",
                        "openrouter",
                        "OpenRouter",
                        PopupView::OpenRouter,
                    ),
                };
                provider_tabs.push(popup_tab_button(
                    tab_id,
                    Some(icon_name),
                    None,
                    tip,
                    selected_view == view,
                    ui.has_provider_error(*provider),
                    ui.use_colored_provider_icons,
                    color_scheme,
                    &hovered_action,
                    set_hovered_action.clone(),
                    on_tab_wheel.clone(),
                    {
                        let pager_dispatch = pager_dispatch.clone();
                        move || pager_dispatch.call(PagerAction::Select(view))
                    },
                ));
            }
        }
        let tabs_key = provider_tabs_key(
            &enabled_provider_order,
            show_provider_icon_tabs,
            ui.use_colored_provider_icons,
            color_scheme,
        );
        horizontal_wheel_strip(
            hstack(provider_tabs)
                .spacing(bottom_bar_size.tab_spacing())
                .horizontal_alignment(HorizontalAlignment::Left)
                .vertical_alignment(VerticalAlignment::Center)
                .margin(Thickness {
                    left: -tab_scroll_x,
                    top: 0.0,
                    right: 0.0,
                    bottom: 0.0,
                })
                // Provider marks are native swap-chain children. Recreate
                // the whole selector when membership, order, tint mode, or
                // theme changes; otherwise WinUI reconciliation can retain
                // a prior tab's text/icon. Scroll offset stays out of the
                // key so panning cannot recycle swap-chain hosts.
                .with_key(tabs_key.clone()),
            bottom_bar_size.icon_button_size(),
            tabs_key,
            on_tab_wheel,
        )
    } else {
        vstack((
            body_strong("Codex Minibar").foreground(ThemeRef::SecondaryText),
            caption(if ui.refreshing {
                "Refreshing…".into()
            } else {
                format_last_updated(latest_sampled_at(&limits), clock_tick)
            })
            .foreground(ThemeRef::TertiaryText),
        ))
        .spacing(0.0)
        .vertical_alignment(VerticalAlignment::Center)
        .horizontal_alignment(HorizontalAlignment::Left)
        .into()
    };
    let refresh_tooltip = if ui.refreshing {
        "Refreshing limits and usage…".into()
    } else if show_footer_tabs {
        let last_updated = format_last_updated(latest_sampled_at(&limits), clock_tick);
        let relative_time = last_updated
            .strip_prefix("Updated ")
            .unwrap_or(&last_updated);
        format!("Refresh | Last updated {relative_time}")
    } else {
        "Refresh".into()
    };

    let footer = border(
        grid((
            footer_identity.grid_column(0),
            hstack({
                // Build only live actions — never pad with Element::Empty.
                // Empty siblings collapse during reconcile and let swap-chain
                // hosts keep a neighbor's painted icon in another slot.
                let mut actions = vec![
                    icon_button(
                        "refresh",
                        "fluent-refresh",
                        "fluent-refresh",
                        &refresh_tooltip,
                        ui.refreshing,
                        refresh_rotation,
                        color_scheme,
                        &hovered_action,
                        set_hovered_action.clone(),
                        refresh,
                    ),
                    icon_button(
                        "settings",
                        "fluent-settings",
                        "fluent-settings",
                        "Settings",
                        false,
                        0.0,
                        color_scheme,
                        &hovered_action,
                        set_hovered_action.clone(),
                        {
                            let settings_tx = settings_tx.clone();
                            let updates = Arc::clone(&state.updates);
                            move || {
                                if let Err(error) = crate::settings_window::open(
                                    settings_tx.clone(),
                                    updates.clone(),
                                ) {
                                    eprintln!("Could not open settings window: {error:?}");
                                }
                            }
                        },
                    ),
                ];
                if ui.update_version.is_some() {
                    actions.push(
                        update_accent_button("Update", || {
                            if let Err(error) = crate::updater::apply_pending_update() {
                                eprintln!("failed to apply update: {error:#}");
                                notifications::show("Update failed", &format!("{error:#}"));
                            }
                        })
                        .height(bottom_bar_size.icon_button_size())
                        .min_height(bottom_bar_size.icon_button_size())
                        .max_height(bottom_bar_size.icon_button_size())
                        .padding(Thickness {
                            left: bottom_bar_size.update_button_padding(),
                            top: 0.0,
                            right: bottom_bar_size.update_button_padding(),
                            bottom: 0.0,
                        })
                        .vertical_alignment(VerticalAlignment::Center)
                        .with_key("footer-update")
                        .into(),
                    );
                }
                actions
            })
            .spacing(bottom_bar_size.action_spacing())
            .horizontal_alignment(HorizontalAlignment::Right)
            .vertical_alignment(VerticalAlignment::Center)
            // Update membership swaps control kinds; key the whole strip so
            // action swap-chain hosts never inherit a neighbor's painted icon.
            .with_key(footer_actions_key(
                ui.update_version.is_some(),
                color_scheme,
            ))
            .canvas_z_index(1)
            .grid_column(1),
        ))
        .rows([GridLength::Auto])
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .column_spacing(bottom_bar_size.column_spacing())
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(Thickness {
        left: if show_footer_tabs {
            bottom_bar_size.tab_padding_left()
        } else {
            bottom_bar_size.no_tabs_padding_left()
        },
        top: bottom_bar_size.padding_top(),
        right: bottom_bar_size.padding_right(),
        // Extra bottom padding so content clears the rounded window corners.
        bottom: bottom_bar_size.padding_bottom(),
    })
    .border_thickness(Thickness {
        left: 0.0,
        top: 1.0,
        right: 0.0,
        bottom: 0.0,
    })
    .background(footer_background)
    .border_brush(ThemeRef::CardStroke)
    .horizontal_alignment(HorizontalAlignment::Stretch);

    // The body can outgrow the popup when both providers, statistics, and an
    // error are visible. Give it the flexible row and keep the footer in a
    // separate Auto row so it remains fixed to the bottom edge.
    let build_page = |body: Vec<Element>,
                      view: PopupView,
                      role: &'static str,
                      from_x: f32,
                      to_x: f32,
                      measure_height: bool| {
        // Limit snapshots update for every provider poll. They must update the
        // existing reactive tree rather than remount this entire page: doing
        // so also recreates its unmanaged SwapChainPanel/XAML children and
        // steadily grows the WinUI compositor's retained allocation.
        //
        // Key only error presence, not the message text: a poll can emit a new
        // string every minute and would otherwise remount the whole page.
        //
        // Structural OpenRouter membership belongs here: adding/removing keys
        // or flipping expired chrome changes DesiredSize without a viewport
        // SizeChanged. Remounting the page is what tab switches already do so
        // the queued on_resize measure can shrink the HWND.
        let body_layout_key = format!(
            "popup-page-{role}-{}-{}-{}-{}-{:?}-{}-{}-{}-{}-{}-{}-{}-{}-{}-{}-{:?}-{:?}",
            ui.error.is_some(),
            view.provider()
                .is_some_and(|provider| ui.has_provider_error(provider)),
            popup_visibility_key(&ui.popup_visibility),
            ui.show_total_spend_on_all_tab,
            ui.total_spend_presentation,
            ui.total_spend_period.key(),
            ui.show_account_name,
            ui.codex_enabled,
            ui.claude_enabled,
            ui.cursor_enabled,
            ui.openrouter_enabled,
            popup_order_key(&ui.popup_order),
            popup_body_height_key(&limits, view),
            ui.compact_usage_cards,
            ui.settings_revision,
            color_scheme as i32,
            view,
        );
        let mut content = vstack(body)
            .spacing(6.0)
            .padding(Thickness {
                left: 16.0,
                top: 16.0,
                right: 16.0,
                bottom: 16.0,
            })
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Top);
        if widget_drag.is_some() {
            let set_drag = set_widget_drag.clone();
            let drag = widget_drag.clone();
            let settings_tx = settings_tx.clone();
            let set_ui = set_ui.clone();
            let ui = ui.clone();
            content = content.on_pointer_released(move |_: PointerEventInfo| {
                let Some(current) = drag.clone() else {
                    return;
                };
                commit_widget_drag(
                    settings_tx.clone(),
                    set_ui.clone(),
                    ui.clone(),
                    current,
                    set_drag.clone(),
                );
            });
        }
        if from_x != to_x {
            content.mounted = Some(Callback::new(move |native: Option<_>| {
                if let Some(native) = native
                    && let Err(error) = animate_translation_x(
                        native,
                        from_x,
                        to_x,
                        PAGER_ANIMATION_DURATION,
                        Easing::Fluent,
                    )
                {
                    eprintln!("Could not animate popup page: {error:?}");
                }
            }));
        }
        if measure_height {
            content = content.on_resize(|_width, height| {
                popup::set_client_height_from_body_content(height);
            });
        }
        scroll_viewer(content.with_key(body_layout_key))
            .horizontal_scroll_bar_visibility(ScrollBarVisibility::Disabled)
            .vertical_scroll_bar_visibility(if measure_height {
                ScrollBarVisibility::Auto
            } else {
                ScrollBarVisibility::Hidden
            })
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch)
            .grid_row(0)
            .into()
    };

    let incoming_from = if page_animations_enabled {
        pager
            .outgoing
            .map_or(0.0, |_| pager.direction.incoming_offset())
    } else {
        0.0
    };
    let current_page = build_page(body, selected_view, "current", incoming_from, 0.0, true);
    let outgoing_page = match (pager.outgoing, outgoing_body) {
        (Some(view), Some(body)) => build_page(
            body,
            view,
            "outgoing",
            0.0,
            if page_animations_enabled {
                pager.direction.outgoing_offset()
            } else {
                0.0
            },
            false,
        ),
        _ => Element::Empty,
    };
    let page_viewport = grid((outgoing_page, current_page))
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Stretch)
        .grid_row(0);

    let body_panel = border(
        grid((page_viewport, footer.grid_row(1)))
            .rows([GridLength::Star(1.0), GridLength::Auto])
            .columns([GridLength::Star(1.0)])
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch)
            .background(Color::transparent()),
    )
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Stretch);

    // The selected material sits behind the fixed native host; reconciler does
    // not manage this panel's children. Element-level backdrops are used
    // rather than `Window.SystemBackdrop`: the latter ignores the popup's
    // Win32 rounded region and paints past its edges. Keeping this layer at
    // host height is important: during a shrink, the old GDI clip can briefly
    // expose space above the new bottom-aligned content, and that space must
    // stay painted.
    // Height is owned solely by the body's desired-size callback above. Using
    // this layer's arranged height as a second source fed ResizeClient back
    // into layout and caused a resize loop / spurious scrollbars.
    let background_material = popup::background_material();
    let background_material_key = background_material.index();
    let background = {
        let mut host = swap_chain_panel()
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch);
        host.mounted = Some(Callback::new(move |native: Option<_>| {
            if let Some(native) = native {
                let result = match background_material {
                    crate::settings::PopupBackgroundMaterial::Acrylic => {
                        crate::acrylic::install_acrylic_into(native)
                    }
                    crate::settings::PopupBackgroundMaterial::Mica => {
                        crate::acrylic::install_popup_mica_into(native)
                    }
                };
                if let Err(error) = result {
                    eprintln!("Could not install popup background material: {error:?}");
                }
            }
        }));
        host.unmounted = Some(Callback::new(|native: Option<_>| {
            if let Some(native) = native {
                let _ = crate::acrylic::clear_children(native);
            }
        }));
        let background: Element = host.into();
        background.with_key(format!(
            "popup-background-{background_material_key}-{}",
            popup::corner_radius_dip()
        ))
    };

    let surface_height =
        f64::from(popup::surface_height_dip()).clamp(1.0, window_size.height.max(1.0));
    let popup_surface = border(
        grid((body_panel,))
            .rows([GridLength::Star(1.0)])
            .columns([GridLength::Star(1.0)])
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch)
            // Keep the content shell transparent so the host-level backdrop
            // remains visible between cards and during the animated settle.
            .background(Color::transparent()),
    )
    .padding(Thickness::uniform(border_inset))
    .corner_radius(window_corner_radius)
    .background(Color::transparent())
    .width(window_size.width.max(1.0))
    .height(surface_height)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Bottom);

    // The native host is deliberately fixed-height while the visible capsule
    // changes size. This gives the footer a real bottom-aligned parent and
    // keeps any transition-only host area painted instead of exposing a black
    // client clear. The GDI region still limits what reaches the desktop.
    border(
        grid((background, popup_surface))
            .rows([GridLength::Star(1.0)])
            .columns([GridLength::Star(1.0)])
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch)
            .background(Color::transparent()),
    )
    .corner_radius(window_corner_radius)
    .background(Color::transparent())
    .width(window_size.width.max(1.0))
    .height(window_size.height.max(1.0))
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Stretch)
    .into()
}
