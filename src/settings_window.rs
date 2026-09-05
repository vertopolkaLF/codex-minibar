//! Settings-window entry point.
//!
//! The host is exposed here so callers do not depend on popup implementation
//! details; both surfaces share tokens from [`crate::theme`].

use crate::settings::{
    AccentColor, AppTheme, AutoActivationPause, BottomBarSize, LimitRefreshInterval, LimitValue,
    OpenRouterAccount, PopupBackgroundMaterial, PopupCornerRadius, PopupVisibility,
    PopupWidgetKind, ProviderKind,
    ScheduledActivation, Settings, TimeFormat, TotalSpendPresentation, TrayColorMode,
    TrayFixedColor, TrayIndicator, TrayPresentation, TrayWidget, TrayWidgetKind,
};
use crate::settings_controls::{
    SETTINGS_CARD_PADDING, settings_action_card, settings_brick_body_height, settings_brick_row,
    settings_brick_table_header, settings_card_padding, settings_checkbox_expander,
    settings_content_expander, settings_content_expander_with_trailing, settings_control_card,
    settings_info_card, settings_slider_content, settings_toggle_card,
    settings_toggle_card_with_description, settings_toggle_expander, update_available_nav_card,
};
use crate::theme::{CONTROL_FAST_ANIMATION, CONTROL_NORMAL_ANIMATION, duration};
use crate::updater::{
    ISSUES_URL, RELEASES_URL, REPO_URL, UpdateController, UpdatePhase, current_version,
};
use anyhow::Context;
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::Sender,
    },
    thread,
    time::Duration,
};
use windows_reactor::*;

mod about;
mod activation;
mod advanced;
mod appearance;
mod customize;
mod general;
mod log;
mod navigation;
mod notifications;
mod onboarding;
mod pages;
mod persistence;
mod platform;
mod providers;
mod shared;
mod state;
#[cfg(test)]
mod tests;
mod tray;

use navigation::{
    RenderedPage, SettingsNavMode, Tab, fade_to_rendered_page, first_provider_in_order,
    providers_nav_items, providers_pane_add_footer, root_nav_items,
};
use onboarding::{detected_providers, onboarding_render};
use pages::render as render_page;
use platform::{
    install_settings_close_hide, load_settings_for_window, set_settings_window_icon,
    sync_settings_caption_button_theme,
};
use providers::{ProviderInstallStatus, provider_install_status, provider_page_content};
use shared::enabled_providers;
use state::{SettingsPageContext, SettingsWindowState};
use tray::tray_indicator_edit_overlay;

pub(crate) use persistence::persist_update;
pub(crate) use platform::is_open;

const WINDOW_WIDTH: f64 = 760.0;
const WINDOW_HEIGHT: f64 = 520.0;
const SETTINGS_WINDOW_TITLE: &str = "Codex Minibar Settings";
const ONBOARDING_WINDOW_TITLE: &str = "Welcome to Codex Minibar";

/// Debounces filesystem/registry provider detection while a path is edited.
static PROVIDER_STATUS_GEN: AtomicU64 = AtomicU64::new(0);
static DISCOVERED_POPUP_BRICKS: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());

thread_local! {
    static HOST: RefCell<Option<Rc<ReactorHost>>> = const { RefCell::new(None) };
    static LIVE_SETTINGS_STATE: RefCell<Option<SettingsWindowState>> = const { RefCell::new(None) };
}

pub fn sync_open_window(settings: Settings, ui_dispatcher: UiMarshaller) {
    if !is_open() {
        return;
    }
    ui_dispatcher.dispatch(move || {
        LIVE_SETTINGS_STATE.with(|state| {
            if let Some(state) = state.borrow().as_ref() {
                state.apply(&settings);
            }
        });
    });
}

fn cached_discovered_popup_bricks() -> BTreeMap<String, String> {
    DISCOVERED_POPUP_BRICKS
        .lock()
        .map(|labels| labels.clone())
        .unwrap_or_default()
}

fn discovered_popup_brick_labels(
    limits: &crate::limits::ProviderLimits,
) -> BTreeMap<String, String> {
    let mut labels = BTreeMap::new();
    for (provider, snapshot) in limits.iter() {
        for (brick_id, title) in
            crate::provider_registry::discovered_additional_brick_labels(provider, snapshot)
        {
            labels.insert(brick_id, title);
        }
    }
    labels
}

/// Publishes API-discovered additional windows so Settings can list them
/// immediately, using the provider-supplied titles rather than a hardcoded catalog.
pub fn publish_discovered_popup_bricks(
    limits: &crate::limits::ProviderLimits,
    ui_dispatcher: UiMarshaller,
) {
    let labels = discovered_popup_brick_labels(limits);
    if let Ok(mut slot) = DISCOVERED_POPUP_BRICKS.lock() {
        *slot = labels.clone();
    }
    if !is_open() {
        return;
    }
    ui_dispatcher.dispatch(move || {
        LIVE_SETTINGS_STATE.with(|state| {
            if let Some(state) = state.borrow().as_ref() {
                state.discovered_popup_bricks.call(labels);
            }
        });
    });
}

pub fn open(
    settings_tx: Sender<Settings>,
    updates: Arc<UpdateController>,
) -> windows_core::Result<()> {
    HOST.with(|slot| {
        if is_open() {
            if let Some(host) = slot.borrow().as_ref() {
                return host.activate();
            }
        }

        // A user can close the settings window using the title-bar button.
        // ReactorHost then remains allocated but its HWND is gone, so discard
        // that stale host before creating the next settings window.
        slot.borrow_mut().take();

        // Always reload from disk so tray/popup open paths share the same live
        // values after an earlier toggle, without depending on a stale snapshot.
        let view_settings = Arc::new(load_settings_for_window());
        let host = Rc::new(ReactorHost::new_with_window_options(
            SETTINGS_WINDOW_TITLE,
            Some(WindowSize {
                width: WINDOW_WIDTH,
                height: WINDOW_HEIGHT,
            }),
            InnerConstraints {
                min_width: Some(560.0),
                min_height: Some(400.0),
                max_width: None,
                max_height: None,
            },
            Box::new(move |_: &(), cx: &mut RenderCx| {
                render(
                    cx,
                    Arc::clone(&view_settings),
                    settings_tx.clone(),
                    Arc::clone(&updates),
                )
            }),
            |recon| {
                // Realize NavigationView/templates on the first paint so the
                // window does not appear and then fill in controls afterward.
                recon.eager_templated_realization = true;
            },
        )?);
        set_settings_window_icon();
        // Hide the HWND before WinUI tears content down so close does not flash
        // empty black chrome (default title bar + no Mica/content).
        install_settings_close_hide();
        host.activate()?;
        *slot.borrow_mut() = Some(host);
        Ok(())
    })
}

/// Opens the two-step first-launch flow. Choices stay local until Done so a
/// dismissed onboarding window never half-configures provider workers.
pub fn open_onboarding(settings_tx: Sender<Settings>) -> windows_core::Result<()> {
    HOST.with(|slot| {
        if is_open() {
            if let Some(host) = slot.borrow().as_ref() {
                return host.activate();
            }
        }
        slot.borrow_mut().take();

        let settings = Arc::new(load_settings_for_window());
        let detected = detected_providers(&settings);
        let host = Rc::new(ReactorHost::new_with_window_options(
            ONBOARDING_WINDOW_TITLE,
            Some(WindowSize {
                width: WINDOW_WIDTH,
                height: WINDOW_HEIGHT,
            }),
            InnerConstraints {
                min_width: Some(560.0),
                min_height: Some(400.0),
                max_width: None,
                max_height: None,
            },
            Box::new(move |_: &(), cx: &mut RenderCx| {
                onboarding_render(cx, Arc::clone(&settings), detected, settings_tx.clone())
            }),
            |recon| recon.eager_templated_realization = true,
        )?);
        set_settings_window_icon();
        install_settings_close_hide();
        host.activate()?;
        *slot.borrow_mut() = Some(host);
        Ok(())
    })
}

pub fn render(
    cx: &mut RenderCx,
    settings: Arc<Settings>,
    settings_tx: Sender<Settings>,
    updates: Arc<UpdateController>,
) -> Element {
    let color_scheme = cx.use_color_scheme();
    let ui_dispatcher = cx.use_ui_marshaller();
    cx.use_effect(color_scheme, move || {
        sync_settings_caption_button_theme(color_scheme);
    });
    let (update_phase, set_update_phase) = cx.use_async_state(updates.snapshot());
    let updates_for_poll = updates.clone();
    cx.use_effect((), move || {
        let updates = updates_for_poll.clone();
        let set_update_phase = set_update_phase.clone();
        std::thread::spawn(move || {
            loop {
                set_update_phase.call(updates.snapshot());
                std::thread::sleep(Duration::from_millis(500));
            }
        });
    });
    let (root_selected, set_root_selected) = cx.use_state(Tab::default());
    let (nav_mode, set_nav_mode) = cx.use_state(SettingsNavMode::Root);
    let (return_root_tab, set_return_root_tab) = cx.use_state(Tab::General);
    let (selected_provider, set_selected_provider) =
        cx.use_state(first_provider_in_order(&settings.popup_order));
    let (rendered_page, set_rendered_page) = cx.use_async_state(RenderedPage::default());
    let (page_visible, set_page_visible) = cx.use_async_state(true);
    let (log_content, set_log_content) = cx
        .use_async_state(crate::logger::tail_lines(100).unwrap_or_else(|error| error.to_string()));
    cx.use_effect((), move || {
        let set_log_content = set_log_content.clone();
        std::thread::spawn(move || {
            loop {
                set_log_content
                    .call(crate::logger::tail_lines(100).unwrap_or_else(|error| error.to_string()));
                std::thread::sleep(Duration::from_millis(500));
            }
        });
    });
    let theme_navigation_guard = cx.use_ref(false);
    let theme_navigation_guard_timer = cx.use_ref(None::<DispatcherTimer>);

    let (codex_enabled, set_codex_enabled) =
        cx.use_state(settings.providers.is_enabled(ProviderKind::Codex));
    let (theme, set_theme) = cx.use_state(settings.theme);
    let (accent_color, set_accent_color) = cx.use_state(settings.accent_color);
    let (animations_enabled, set_animations_enabled) = cx.use_state(settings.animations_enabled);
    let (bottom_bar_size, set_bottom_bar_size) = cx.use_state(settings.bottom_bar_size);
    let (popup_corner_radius, set_popup_corner_radius) =
        cx.use_state(settings.popup_corner_radius);
    let (popup_background_material, set_popup_background_material) =
        cx.use_state(settings.popup_background_material);
    let (time_format, set_time_format) = cx.use_state(settings.time_format);
    cx.use_effect(
        (theme, accent_color, animations_enabled, time_format),
        move || {
            crate::theme::set_animations_enabled(animations_enabled);
            crate::theme::apply_appearance(theme, accent_color);
            time_format.apply();
        },
    );
    let (claude_enabled, set_claude_enabled) =
        cx.use_state(settings.providers.is_enabled(ProviderKind::Claude));
    let (cursor_enabled, set_cursor_enabled) =
        cx.use_state(settings.providers.is_enabled(ProviderKind::Cursor));
    let (opencode_zen_enabled, set_opencode_zen_enabled) =
        cx.use_state(settings.providers.is_enabled(ProviderKind::OpenCodeZen));
    let (opencode_go_enabled, set_opencode_go_enabled) =
        cx.use_state(settings.providers.is_enabled(ProviderKind::OpenCodeGo));
    let (openrouter_enabled, set_openrouter_enabled) =
        cx.use_state(settings.providers.is_enabled(ProviderKind::OpenRouter));
    let (opencode_zen_key_input, set_opencode_zen_key_input) = cx.use_state(String::new());
    let (opencode_go_key_input, set_opencode_go_key_input) = cx.use_state(String::new());
    let (openrouter_accounts, set_openrouter_accounts) =
        cx.use_state(crate::openrouter::accounts_for_settings(&settings));
    let (openrouter_key_inputs, set_openrouter_key_inputs) =
        cx.use_state(HashMap::<String, String>::new());
    let (openrouter_management_inputs, set_openrouter_management_inputs) =
        cx.use_state(HashMap::<String, String>::new());
    let (codex_path, set_codex_path) = cx.use_state(
        settings
            .codex_path
            .as_ref()
            .map_or_else(String::new, |path| path.to_string_lossy().into_owned()),
    );
    let (claude_path, set_claude_path) = cx.use_state(
        settings
            .claude_path
            .as_ref()
            .map_or_else(String::new, |path| path.to_string_lossy().into_owned()),
    );
    let (cursor_path, set_cursor_path) = cx.use_state(
        settings
            .cursor_path
            .as_ref()
            .map_or_else(String::new, |path| path.to_string_lossy().into_owned()),
    );
    let (popup_order, set_popup_order) = cx.use_state(settings.popup_order.clone());
    let (use_colored_sidebar_icons, set_use_colored_sidebar_icons) =
        cx.use_state(settings.use_colored_sidebar_icons);

    let nav_icon_color = match color_scheme {
        ColorScheme::Dark => "#E6E6E6",
        ColorScheme::Light => "#3A3A3A",
    };
    let nav_selected_tag = match nav_mode {
        SettingsNavMode::Root => root_selected.tag().to_string(),
        SettingsNavMode::Providers => selected_provider.id().to_string(),
    };
    let nav_menu_items: Vec<NavViewItem> = match nav_mode {
        SettingsNavMode::Root => root_nav_items(nav_icon_color, use_colored_sidebar_icons).into(),
        SettingsNavMode::Providers => providers_nav_items(&popup_order, nav_icon_color),
    };
    let nav_key = match nav_mode {
        SettingsNavMode::Root => format!(
            "settings-nav-root-{}-{nav_icon_color}",
            if use_colored_sidebar_icons {
                "color"
            } else {
                "mono"
            }
        ),
        SettingsNavMode::Providers => format!("settings-nav-providers-{nav_icon_color}"),
    };
    let mut navigation = NavigationView::new(nav_menu_items, Element::Empty)
        .with_key(nav_key)
        .selected_tag(nav_selected_tag)
        .on_selection_changed({
            let set_rendered_page = set_rendered_page.clone();
            let set_page_visible = set_page_visible.clone();
            let theme_navigation_guard = theme_navigation_guard.clone();
            let set_nav_mode = set_nav_mode.clone();
            let set_return_root_tab = set_return_root_tab.clone();
            let set_root_selected = set_root_selected.clone();
            let set_selected_provider = set_selected_provider.clone();
            let popup_order = popup_order.clone();
            move |tag: String| {
                if theme_navigation_guard.get_cloned() {
                    return;
                }
                match nav_mode {
                    SettingsNavMode::Root => {
                        if tag == "providers" {
                            let first = first_provider_in_order(&popup_order);
                            let restore = if root_selected != Tab::Providers {
                                root_selected
                            } else {
                                Tab::General
                            };
                            set_return_root_tab.call(restore);
                            set_nav_mode.call(SettingsNavMode::Providers);
                            set_selected_provider.call(first);
                            fade_to_rendered_page(
                                set_page_visible.clone(),
                                set_rendered_page.clone(),
                                RenderedPage::Provider(first),
                            );
                            return;
                        }
                        let next = Tab::from_tag(&tag);
                        if next != root_selected {
                            set_root_selected.call(next);
                            fade_to_rendered_page(
                                set_page_visible.clone(),
                                set_rendered_page.clone(),
                                RenderedPage::Root(next),
                            );
                        }
                    }
                    SettingsNavMode::Providers => {
                        if let Some(provider) = ProviderKind::from_id(&tag)
                            && provider != selected_provider
                        {
                            set_selected_provider.call(provider);
                            fade_to_rendered_page(
                                set_page_visible.clone(),
                                set_rendered_page.clone(),
                                RenderedPage::Provider(provider),
                            );
                        }
                    }
                }
            }
        })
        .pane_display_mode(NavigationViewPaneDisplayMode::Left)
        .pane_open(true)
        .open_pane_length(220.0)
        .settings_visible(false)
        .back_button_visible(false)
        .pane_toggle_button_visible(false)
        .background(Color::transparent())
        .width(220.0)
        .horizontal_alignment(HorizontalAlignment::Left)
        .vertical_alignment(VerticalAlignment::Stretch);
    navigation = match nav_mode {
        SettingsNavMode::Root => {
            if let UpdatePhase::Available(update) = &update_phase {
                let version = update.version.clone();
                navigation.pane_footer(
                    border(update_available_nav_card(version, || {
                        if let Err(error) = crate::updater::apply_pending_update() {
                            eprintln!("failed to apply update: {error:#}");
                            crate::notifications::show("Update failed", &format!("{error:#}"));
                        }
                    }))
                    .padding(Thickness {
                        left: 12.0,
                        top: 0.0,
                        right: 12.0,
                        bottom: 2.0,
                    })
                    .background(Color::transparent()),
                )
            } else {
                navigation
            }
        }
        SettingsNavMode::Providers => navigation.pane_footer(providers_pane_add_footer(
            set_openrouter_accounts.clone(),
            settings_tx.clone(),
            set_selected_provider.clone(),
            set_page_visible.clone(),
            set_rendered_page.clone(),
        )),
    };

    let (codex_install_status, set_codex_install_status) =
        cx.use_async_state(ProviderInstallStatus::checking());
    let (claude_install_status, set_claude_install_status) =
        cx.use_async_state(ProviderInstallStatus::checking());
    let (cursor_install_status, set_cursor_install_status) =
        cx.use_async_state(ProviderInstallStatus::checking());
    let (opencode_zen_install_status, set_opencode_zen_install_status) =
        cx.use_async_state(ProviderInstallStatus::checking());
    let (opencode_go_install_status, set_opencode_go_install_status) =
        cx.use_async_state(ProviderInstallStatus::checking());
    let (openrouter_install_status, set_openrouter_install_status) =
        cx.use_async_state(ProviderInstallStatus::checking());
    let status_codex_path = codex_path.clone();
    let status_claude_path = claude_path.clone();
    let status_cursor_path = cursor_path.clone();
    cx.use_effect(
        (codex_path.clone(), claude_path.clone(), cursor_path.clone()),
        move || {
            let generation = PROVIDER_STATUS_GEN.fetch_add(1, Ordering::Relaxed) + 1;
            set_codex_install_status.call(ProviderInstallStatus::checking());
            set_claude_install_status.call(ProviderInstallStatus::checking());
            set_cursor_install_status.call(ProviderInstallStatus::checking());
            set_opencode_zen_install_status.call(ProviderInstallStatus::checking());
            set_opencode_go_install_status.call(ProviderInstallStatus::checking());
            set_openrouter_install_status.call(ProviderInstallStatus::checking());
            let codex_status = set_codex_install_status.clone();
            let claude_status = set_claude_install_status.clone();
            let cursor_status = set_cursor_install_status.clone();
            let opencode_zen_status = set_opencode_zen_install_status.clone();
            let opencode_go_status = set_opencode_go_install_status.clone();
            let openrouter_status = set_openrouter_install_status.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(250));
                if PROVIDER_STATUS_GEN.load(Ordering::Relaxed) != generation {
                    return;
                }
                let codex = provider_install_status(ProviderKind::Codex, &status_codex_path);
                let claude = provider_install_status(ProviderKind::Claude, &status_claude_path);
                let cursor = provider_install_status(ProviderKind::Cursor, &status_cursor_path);
                let opencode_zen = provider_install_status(ProviderKind::OpenCodeZen, "");
                let opencode_go = provider_install_status(ProviderKind::OpenCodeGo, "");
                let openrouter = provider_install_status(ProviderKind::OpenRouter, "");
                if PROVIDER_STATUS_GEN.load(Ordering::Relaxed) == generation {
                    codex_status.call(codex);
                    claude_status.call(claude);
                    cursor_status.call(cursor);
                    opencode_zen_status.call(opencode_zen);
                    opencode_go_status.call(opencode_go);
                    openrouter_status.call(openrouter);
                }
            });
        },
    );
    let (use_colored_provider_icons, set_use_colored_provider_icons) =
        cx.use_state(settings.use_colored_provider_icons);
    let (replace_chatgpt_logo_with_codex, set_replace_chatgpt_logo_with_codex) =
        cx.use_state(settings.replace_chatgpt_logo_with_codex);
    let (start_at_login, set_start_at_login) = cx.use_state(settings.start_at_login);
    let (automatic_activation, set_automatic_activation) =
        cx.use_state(settings.automatic_activation);
    let (scheduled_activations, set_scheduled_activations) =
        cx.use_state(settings.scheduled_activations.clone());
    let (auto_activation_pauses, set_auto_activation_pauses) =
        cx.use_state(settings.auto_activation_pauses.clone());
    let (expanded_scheduled_activation, set_expanded_scheduled_activation) =
        cx.use_state(None::<String>);
    let (expanded_auto_activation_pause, set_expanded_auto_activation_pause) =
        cx.use_state(None::<String>);
    let (limit_refresh_interval, set_limit_refresh_interval) =
        cx.use_state(settings.limit_refresh_interval);
    let (show_used_percentage, set_show_used_percentage) =
        cx.use_state(settings.show_used_percentage);
    let (show_usage_pace, set_show_usage_pace) = cx.use_state(settings.show_usage_pace);
    let (compact_usage_cards, set_compact_usage_cards) = cx.use_state(settings.compact_usage_cards);
    let (popup_visibility, set_popup_visibility) = cx.use_state(settings.popup_visibility.clone());
    let (show_total_spend_on_all_tab, set_show_total_spend_on_all_tab) =
        cx.use_state(settings.show_total_spend_on_all_tab);
    let (total_spend_presentation, set_total_spend_presentation) =
        cx.use_state(settings.total_spend_presentation);
    let (show_account_name, set_show_account_name) = cx.use_state(settings.show_account_name);
    let (activation_success, set_activation_success) =
        cx.use_state(settings.notifications.activation_success);
    let (activation_failure, set_activation_failure) =
        cx.use_state(settings.notifications.activation_failure);
    let (limits_reset, set_limits_reset) = cx.use_state(settings.notifications.limits_changed);
    let (low_usage_enabled, set_low_usage_enabled) =
        cx.use_state(settings.notifications.low_usage_enabled);
    let (low_usage_threshold, set_low_usage_threshold) =
        cx.use_state(settings.notifications.low_usage_threshold_percent);
    let (low_usage_expanded, set_low_usage_expanded) = cx.use_state(true);
    let (low_usage_expand_progress, set_low_usage_expand_progress) = cx.use_async_state(1.0_f64);
    let (weekly_low_usage_enabled, set_weekly_low_usage_enabled) =
        cx.use_state(settings.notifications.weekly_low_usage_enabled);
    let (weekly_low_usage_threshold, set_weekly_low_usage_threshold) =
        cx.use_state(settings.notifications.weekly_low_usage_threshold_percent);
    let (weekly_low_usage_expanded, set_weekly_low_usage_expanded) = cx.use_state(false);
    let (weekly_low_usage_expand_progress, set_weekly_low_usage_expand_progress) =
        cx.use_async_state(0.0_f64);
    let (hovered_card_id, set_hovered_card_id) = cx.use_state(None::<String>);
    let (tray_widgets, set_tray_widgets) = cx.use_state(settings.tray_widgets.clone());
    let (expanded_tray_widget, set_expanded_tray_widget) = cx.use_state(None::<String>);
    let (editing_tray_indicator, set_editing_tray_indicator) =
        cx.use_async_state(None::<(String, usize)>);
    let (indicator_modal_visible, set_indicator_modal_visible) = cx.use_async_state(false);
    let (removed_tray_widget, set_removed_tray_widget) = cx.use_state(None::<(usize, TrayWidget)>);
    let (expanded_popup_provider, set_expanded_popup_provider) = cx.use_state(None::<String>);
    let (discovered_popup_bricks, set_discovered_popup_bricks) =
        cx.use_state(cached_discovered_popup_bricks());
    let (check_for_updates, set_check_for_updates) = cx.use_state(settings.check_for_updates);
    let (notify_on_update, set_notify_on_update) =
        cx.use_state(settings.notifications.update_available);

    LIVE_SETTINGS_STATE.with(|state| {
        *state.borrow_mut() = Some(SettingsWindowState {
            theme: set_theme.clone(),
            accent_color: set_accent_color.clone(),
            animations_enabled: set_animations_enabled.clone(),
            bottom_bar_size: set_bottom_bar_size.clone(),
            popup_corner_radius: set_popup_corner_radius.clone(),
            popup_background_material: set_popup_background_material.clone(),
            time_format: set_time_format.clone(),
            codex_enabled: set_codex_enabled.clone(),
            claude_enabled: set_claude_enabled.clone(),
            cursor_enabled: set_cursor_enabled.clone(),
            opencode_zen_enabled: set_opencode_zen_enabled.clone(),
            opencode_go_enabled: set_opencode_go_enabled.clone(),
            openrouter_enabled: set_openrouter_enabled.clone(),
            openrouter_accounts: set_openrouter_accounts.clone(),
            codex_path: set_codex_path.clone(),
            claude_path: set_claude_path.clone(),
            cursor_path: set_cursor_path.clone(),
            popup_order: set_popup_order.clone(),
            use_colored_provider_icons: set_use_colored_provider_icons.clone(),
            use_colored_sidebar_icons: set_use_colored_sidebar_icons.clone(),
            replace_chatgpt_logo_with_codex: set_replace_chatgpt_logo_with_codex.clone(),
            automatic_activation: set_automatic_activation.clone(),
            scheduled_activations: set_scheduled_activations.clone(),
            auto_activation_pauses: set_auto_activation_pauses.clone(),
            limit_refresh_interval: set_limit_refresh_interval.clone(),
            start_at_login: set_start_at_login.clone(),
            show_used_percentage: set_show_used_percentage.clone(),
            show_usage_pace: set_show_usage_pace.clone(),
            compact_usage_cards: set_compact_usage_cards.clone(),
            popup_visibility: set_popup_visibility.clone(),
            discovered_popup_bricks: set_discovered_popup_bricks.clone(),
            show_total_spend_on_all_tab: set_show_total_spend_on_all_tab.clone(),
            total_spend_presentation: set_total_spend_presentation.clone(),
            show_account_name: set_show_account_name.clone(),
            activation_success: set_activation_success.clone(),
            activation_failure: set_activation_failure.clone(),
            limits_reset: set_limits_reset.clone(),
            low_usage_enabled: set_low_usage_enabled.clone(),
            low_usage_threshold: set_low_usage_threshold.clone(),
            weekly_low_usage_enabled: set_weekly_low_usage_enabled.clone(),
            weekly_low_usage_threshold: set_weekly_low_usage_threshold.clone(),
            tray_widgets: set_tray_widgets.clone(),
            check_for_updates: set_check_for_updates.clone(),
            notify_on_update: set_notify_on_update.clone(),
        });
    });

    let page_context = SettingsPageContext {
        theme: theme,
        accent_color: accent_color,
        animations_enabled: animations_enabled,
        bottom_bar_size: bottom_bar_size,
        popup_corner_radius: popup_corner_radius,
        popup_background_material: popup_background_material,
        time_format: time_format,
        codex_enabled: codex_enabled,
        claude_enabled: claude_enabled,
        cursor_enabled: cursor_enabled,
        opencode_zen_enabled: opencode_zen_enabled,
        opencode_go_enabled: opencode_go_enabled,
        openrouter_enabled: openrouter_enabled,
        codex_path: &codex_path,
        claude_path: &claude_path,
        cursor_path: &cursor_path,
        codex_install_status: &codex_install_status,
        claude_install_status: &claude_install_status,
        cursor_install_status: &cursor_install_status,
        opencode_zen_install_status: &opencode_zen_install_status,
        opencode_go_install_status: &opencode_go_install_status,
        openrouter_install_status: &openrouter_install_status,
        opencode_zen_key_input: &opencode_zen_key_input,
        opencode_go_key_input: &opencode_go_key_input,
        openrouter_accounts: &openrouter_accounts,
        openrouter_key_inputs: &openrouter_key_inputs,
        openrouter_management_inputs: &openrouter_management_inputs,
        popup_order: &popup_order,
        use_colored_provider_icons: use_colored_provider_icons,
        use_colored_sidebar_icons: use_colored_sidebar_icons,
        replace_chatgpt_logo_with_codex: replace_chatgpt_logo_with_codex,
        automatic_activation: automatic_activation,
        scheduled_activations: &scheduled_activations,
        auto_activation_pauses: &auto_activation_pauses,
        expanded_scheduled_activation: &expanded_scheduled_activation,
        expanded_auto_activation_pause: &expanded_auto_activation_pause,
        limit_refresh_interval: limit_refresh_interval,
        start_at_login: start_at_login,
        show_used_percentage: show_used_percentage,
        show_usage_pace: show_usage_pace,
        compact_usage_cards: compact_usage_cards,
        popup_visibility: &popup_visibility,
        discovered_popup_bricks: &discovered_popup_bricks,
        show_total_spend_on_all_tab: show_total_spend_on_all_tab,
        total_spend_presentation: total_spend_presentation,
        show_account_name: show_account_name,
        activation_success: activation_success,
        activation_failure: activation_failure,
        limits_reset: limits_reset,
        low_usage_enabled: low_usage_enabled,
        low_usage_threshold: low_usage_threshold,
        low_usage_expanded: low_usage_expanded,
        low_usage_expand_progress: low_usage_expand_progress,
        weekly_low_usage_enabled: weekly_low_usage_enabled,
        weekly_low_usage_threshold: weekly_low_usage_threshold,
        weekly_low_usage_expanded: weekly_low_usage_expanded,
        weekly_low_usage_expand_progress: weekly_low_usage_expand_progress,
        tray_widgets: &tray_widgets,
        expanded_tray_widget: &expanded_tray_widget,
        editing_tray_indicator: &editing_tray_indicator,
        removed_tray_widget: &removed_tray_widget,
        hovered_card_id: &hovered_card_id,
        expanded_popup_provider: &expanded_popup_provider,
        check_for_updates: check_for_updates,
        notify_on_update: notify_on_update,
        update_phase: &update_phase,
        log_content: &log_content,
        set_codex_enabled: set_codex_enabled.clone(),
        set_theme: set_theme.clone(),
        set_accent_color: set_accent_color.clone(),
        set_animations_enabled: set_animations_enabled.clone(),
        set_bottom_bar_size: set_bottom_bar_size.clone(),
        set_popup_corner_radius: set_popup_corner_radius.clone(),
        set_popup_background_material: set_popup_background_material.clone(),
        set_time_format: set_time_format.clone(),
        set_claude_enabled: set_claude_enabled.clone(),
        set_cursor_enabled: set_cursor_enabled.clone(),
        set_opencode_zen_enabled: set_opencode_zen_enabled.clone(),
        set_opencode_go_enabled: set_opencode_go_enabled.clone(),
        set_openrouter_enabled: set_openrouter_enabled.clone(),
        set_opencode_zen_key_input: set_opencode_zen_key_input.clone(),
        set_opencode_go_key_input: set_opencode_go_key_input.clone(),
        set_openrouter_accounts: set_openrouter_accounts.clone(),
        set_openrouter_key_inputs: set_openrouter_key_inputs.clone(),
        set_openrouter_management_inputs: set_openrouter_management_inputs.clone(),
        set_codex_path: set_codex_path.clone(),
        set_claude_path: set_claude_path.clone(),
        set_cursor_path: set_cursor_path.clone(),
        set_popup_order: set_popup_order.clone(),
        set_use_colored_provider_icons: set_use_colored_provider_icons.clone(),
        set_use_colored_sidebar_icons: set_use_colored_sidebar_icons.clone(),
        set_replace_chatgpt_logo_with_codex: set_replace_chatgpt_logo_with_codex.clone(),
        set_automatic_activation: set_automatic_activation.clone(),
        set_scheduled_activations: set_scheduled_activations.clone(),
        set_auto_activation_pauses: set_auto_activation_pauses.clone(),
        set_expanded_scheduled_activation: set_expanded_scheduled_activation.clone(),
        set_expanded_auto_activation_pause: set_expanded_auto_activation_pause.clone(),
        set_limit_refresh_interval: set_limit_refresh_interval.clone(),
        set_start_at_login: set_start_at_login.clone(),
        set_show_used_percentage: set_show_used_percentage.clone(),
        set_show_usage_pace: set_show_usage_pace.clone(),
        set_compact_usage_cards: set_compact_usage_cards.clone(),
        set_popup_visibility: set_popup_visibility.clone(),
        set_discovered_popup_bricks: set_discovered_popup_bricks.clone(),
        set_show_total_spend_on_all_tab: set_show_total_spend_on_all_tab.clone(),
        set_total_spend_presentation: set_total_spend_presentation.clone(),
        set_show_account_name: set_show_account_name.clone(),
        set_activation_success: set_activation_success.clone(),
        set_activation_failure: set_activation_failure.clone(),
        set_limits_reset: set_limits_reset.clone(),
        set_low_usage_enabled: set_low_usage_enabled.clone(),
        set_low_usage_threshold: set_low_usage_threshold.clone(),
        set_low_usage_expanded: set_low_usage_expanded.clone(),
        set_low_usage_expand_progress: set_low_usage_expand_progress.clone(),
        set_weekly_low_usage_enabled: set_weekly_low_usage_enabled.clone(),
        set_weekly_low_usage_threshold: set_weekly_low_usage_threshold.clone(),
        set_weekly_low_usage_expanded: set_weekly_low_usage_expanded.clone(),
        set_weekly_low_usage_expand_progress: set_weekly_low_usage_expand_progress.clone(),
        set_tray_widgets: set_tray_widgets.clone(),
        set_expanded_tray_widget: set_expanded_tray_widget.clone(),
        set_editing_tray_indicator: set_editing_tray_indicator.clone(),
        set_indicator_modal_visible: set_indicator_modal_visible.clone(),
        set_removed_tray_widget: set_removed_tray_widget.clone(),
        set_expanded_popup_provider: set_expanded_popup_provider.clone(),
        set_hovered_card_id: set_hovered_card_id.clone(),
        set_check_for_updates: set_check_for_updates.clone(),
        set_notify_on_update: set_notify_on_update.clone(),
        theme_navigation_guard: theme_navigation_guard.clone(),
        theme_navigation_guard_timer: theme_navigation_guard_timer.clone(),
        settings_tx: settings_tx.clone(),
        ui_dispatcher: ui_dispatcher.clone(),
        updates: updates.clone(),
    };
    let settings_page_body = match rendered_page {
        RenderedPage::Root(tab) => render_page(tab, &page_context),
        RenderedPage::Provider(provider) => provider_page_content(provider, &page_context),
    };

    // Padding lives on tab content (inside the scroller), not on this pane, so
    // LayerFill crops flush to the window edge while long tabs stay scrollable.
    let page_scroller = scroll_viewer(
        border(settings_page_body)
            .padding(Thickness {
                left: 32.0,
                top: 24.0,
                right: 32.0,
                bottom: 32.0,
            })
            .with_key(rendered_page.page_key())
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Top),
    )
    // Keys are honored only in multi-child containers by windows-reactor.
    // The Grid below therefore remounts this native ScrollViewer on every
    // rendered-page change, guaranteeing a fresh zero scroll offset.
    .with_key(rendered_page.scroll_key())
    .horizontal_scroll_bar_visibility(ScrollBarVisibility::Disabled)
    .vertical_scroll_bar_visibility(ScrollBarVisibility::Auto)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Stretch)
    .grid_row(0)
    .grid_column(0);

    let page_content = border(
        grid((page_scroller,))
            .columns([GridLength::Star(1.0)])
            .rows([GridLength::Star(1.0)])
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch),
    )
    .opacity(if page_visible { 1.0 } else { 0.0 })
    .with_opacity_transition(duration(CONTROL_FAST_ANIMATION))
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Stretch);

    let page = border(page_content)
        // Standard Fluent content layer over the element-level Mica base.
        .background(ThemeRef::LayerFill)
        .corner_radii(CornerRadii {
            top_left: 12.0,
            ..Default::default()
        })
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Stretch);

    // Match NavigationView item icons: 16px glyph centered in the 48px leading column.
    let title_bar_icon = hstack((Image::new_with_uri(settings_title_icon_uri())
        .width(16.0)
        .height(16.0),))
    .margin(Thickness {
        left: 16.0,
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
    })
    .vertical_alignment(VerticalAlignment::Center);
    let providers_drill_in = nav_mode == SettingsNavMode::Providers;
    let title_bar = TitleBar::new("Codex Minibar Settings")
        .content(title_bar_icon)
        .back_button_visible(providers_drill_in)
        .back_button_enabled(providers_drill_in)
        .on_back_requested({
            let set_nav_mode = set_nav_mode.clone();
            let set_root_selected = set_root_selected.clone();
            let set_page_visible = set_page_visible.clone();
            let set_rendered_page = set_rendered_page.clone();
            move || {
                let restore = return_root_tab;
                set_nav_mode.call(SettingsNavMode::Root);
                set_root_selected.call(restore);
                fade_to_rendered_page(
                    set_page_visible.clone(),
                    set_rendered_page.clone(),
                    RenderedPage::Root(restore),
                );
            }
        })
        .pane_toggle_button_visible(false)
        // Tall caption buttons so min/max/close fill the TitleBar height.
        .tall(true);
    let shell = grid((navigation.grid_column(0), page.grid_column(1)))
        .columns([GridLength::Pixel(220.0), GridLength::Star(1.0)])
        .rows([GridLength::Star(1.0)])
        .background(Color::transparent());

    let window_body = grid((title_bar.grid_row(0), shell.grid_row(1)))
        .rows([GridLength::Auto, GridLength::Star(1.0)])
        .columns([GridLength::Star(1.0)])
        .background(Color::transparent());

    let tray_providers: Vec<ProviderKind> = popup_order
        .iter()
        .filter_map(|widget| widget.as_provider())
        .collect();
    let tray_enabled_providers = enabled_providers(
        &tray_providers,
        codex_enabled,
        claude_enabled,
        cursor_enabled,
        opencode_zen_enabled,
        opencode_go_enabled,
        openrouter_enabled,
    );
    let window_body: Element = if let Some(editing) = editing_tray_indicator.as_ref() {
        let overlay = tray_indicator_edit_overlay(
            &tray_widgets,
            &tray_enabled_providers,
            editing,
            indicator_modal_visible,
            set_tray_widgets.clone(),
            set_editing_tray_indicator.clone(),
            set_indicator_modal_visible.clone(),
            settings_tx.clone(),
        );
        match overlay {
            Some(overlay) => relative_panel::<Vec<Element>>(vec![
                window_body
                    .relative_align_left()
                    .relative_align_right()
                    .relative_align_top()
                    .relative_align_bottom()
                    .into(),
                overlay
                    .relative_align_left()
                    .relative_align_right()
                    .relative_align_top()
                    .relative_align_bottom(),
            ])
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch)
            .into(),
            None => window_body.into(),
        }
    } else {
        window_body.into()
    };

    let mica = {
        let mut host = swap_chain_panel()
            .grid_row_span(1)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch);
        host.mounted = Some(Callback::new(|native: Option<_>| {
            if let Some(native) = native
                && let Err(error) = crate::acrylic::install_mica_into(native)
            {
                eprintln!("Could not install settings Mica element: {error:?}");
            }
        }));
        host
    };
    relative_panel::<Vec<Element>>(vec![
        mica.relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .relative_align_bottom()
            .into(),
        window_body
            .relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .relative_align_bottom(),
    ])
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Stretch)
    .background(Color::transparent())
    .into()
}

fn settings_title_icon_uri() -> String {
    let packaged = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .map(|parent| parent.join("assets/icons/app-icon-32.png"))
        })
        .filter(|path| path.exists());
    let path = packaged.unwrap_or_else(|| {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons/app-icon-32.png")
    });
    format!("file:///{}", path.to_string_lossy().replace('\\', "/"))
}
