//! Settings-window entry point.
//!
//! The host is exposed here so callers do not depend on popup implementation
//! details; both surfaces share tokens from [`crate::theme`].

use crate::notifications;
#[cfg(any())]
use crate::settings::TraySource;
use crate::settings::{
    AccentColor, AppTheme, LimitRefreshInterval, LimitValue, OpenRouterAccount, PopupVisibility,
    PopupWidgetKind, ProviderKind, ScheduledActivation, Settings, TimeFormat,
    TotalSpendPresentation, TrayColorMode, TrayFixedColor, TrayIndicator, TrayPresentation,
    TrayWidget, TrayWidgetKind,
};
use crate::settings_controls::{
    SETTINGS_CARD_PADDING, settings_action_card, settings_brick_body_height, settings_brick_row,
    settings_brick_table_header,
    settings_card_padding, settings_checkbox_expander, settings_content_expander,
    settings_control_card, settings_info_card, settings_slider_content, settings_toggle_card,
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
        Arc,
        Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::Sender,
    },
    thread,
    time::Duration,
};
use windows_reactor::*;

const WINDOW_WIDTH: f64 = 760.0;
const WINDOW_HEIGHT: f64 = 520.0;
const SETTINGS_WINDOW_TITLE: &str = "Codex Minibar Settings";
const ONBOARDING_WINDOW_TITLE: &str = "Welcome to Codex Minibar";

/// Generation counter so overlapping indicator-modal open/close animations don't race.
static INDICATOR_MODAL_ANIM_GEN: AtomicU64 = AtomicU64::new(0);
/// Debounces filesystem/registry provider detection while a path is edited.
static PROVIDER_STATUS_GEN: AtomicU64 = AtomicU64::new(0);
static CODEX_PATH_SAVE_GEN: AtomicU64 = AtomicU64::new(0);
static CLAUDE_PATH_SAVE_GEN: AtomicU64 = AtomicU64::new(0);
static CURSOR_PATH_SAVE_GEN: AtomicU64 = AtomicU64::new(0);
/// CSS `#000a` → `#000000aa` backdrop behind the indicator edit card.
const INDICATOR_MODAL_SCRIM: Color = Color {
    a: 0xaa,
    r: 0,
    g: 0,
    b: 0,
};
const INDICATOR_MODAL_WIDTH: f64 = 520.0;
const INDICATOR_MODAL_RADIUS: f64 = 12.0;

static DISCOVERED_POPUP_BRICKS: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());

thread_local! {
    static HOST: RefCell<Option<Rc<ReactorHost>>> = const { RefCell::new(None) };
    static LIVE_SETTINGS_STATE: RefCell<Option<SettingsWindowState>> = const { RefCell::new(None) };
    static TRAY_PREVIEW_MOUNTS: RefCell<HashMap<String, windows_core::IInspectable>> =
        RefCell::new(HashMap::new());
    static TRAY_PREVIEW_CACHE: RefCell<HashMap<String, TrayPreviewCacheEntry>> =
        RefCell::new(HashMap::new());
}

struct TrayPreviewCacheEntry {
    widget: TrayWidget,
    accent: [u8; 3],
    uses_light_theme: bool,
    time_format: TimeFormat,
    minute_bucket: u64,
    pixels: Arc<Vec<u8>>,
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

#[derive(Clone)]
struct SettingsWindowState {
    theme: SetState<AppTheme>,
    accent_color: SetState<AccentColor>,
    animations_enabled: SetState<bool>,
    time_format: SetState<TimeFormat>,
    codex_enabled: SetState<bool>,
    claude_enabled: SetState<bool>,
    cursor_enabled: SetState<bool>,
    opencode_zen_enabled: SetState<bool>,
    opencode_go_enabled: SetState<bool>,
    openrouter_enabled: SetState<bool>,
    openrouter_accounts: SetState<Vec<OpenRouterAccount>>,
    codex_path: SetState<String>,
    claude_path: SetState<String>,
    cursor_path: SetState<String>,
    popup_order: SetState<Vec<PopupWidgetKind>>,
    use_colored_provider_icons: SetState<bool>,
    use_colored_sidebar_icons: SetState<bool>,
    replace_chatgpt_logo_with_codex: SetState<bool>,
    automatic_activation: SetState<bool>,
    scheduled_activations: SetState<Vec<ScheduledActivation>>,
    limit_refresh_interval: SetState<LimitRefreshInterval>,
    start_at_login: SetState<bool>,
    show_used_percentage: SetState<bool>,
    show_usage_pace: SetState<bool>,
    popup_visibility: SetState<PopupVisibility>,
    discovered_popup_bricks: SetState<BTreeMap<String, String>>,
    show_total_spend_on_all_tab: SetState<bool>,
    total_spend_presentation: SetState<TotalSpendPresentation>,
    show_account_name: SetState<bool>,
    activation_success: SetState<bool>,
    activation_failure: SetState<bool>,
    limits_reset: SetState<bool>,
    low_usage_enabled: SetState<bool>,
    low_usage_threshold: SetState<u8>,
    weekly_low_usage_enabled: SetState<bool>,
    weekly_low_usage_threshold: SetState<u8>,
    tray_widgets: SetState<Vec<TrayWidget>>,
    check_for_updates: SetState<bool>,
    notify_on_update: SetState<bool>,
}

impl SettingsWindowState {
    fn apply(&self, settings: &Settings) {
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
        self.limit_refresh_interval
            .call(settings.limit_refresh_interval);
        self.start_at_login.call(settings.start_at_login);
        self.show_used_percentage
            .call(settings.show_used_percentage);
        self.show_usage_pace.call(settings.show_usage_pace);
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

fn detected_providers(settings: &Settings) -> [bool; 6] {
    [
        crate::codex::is_installed(settings.codex_path.as_deref()),
        crate::claude::is_installed(settings.claude_path.as_deref()),
        crate::cursor::is_installed(settings.cursor_path.as_deref()),
        crate::opencode::is_installed(ProviderKind::OpenCodeZen),
        crate::opencode::is_installed(ProviderKind::OpenCodeGo),
        crate::openrouter::is_installed_for_accounts(&crate::openrouter::accounts_for_settings(
            settings,
        )),
    ]
}

#[derive(Clone, PartialEq)]
struct ProviderInstallStatus {
    app: Option<String>,
    cli: Option<String>,
    used: Option<ProviderInstallSource>,
    cli_applicable: bool,
    checking: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum ProviderInstallSource {
    App,
    Cli,
}

impl ProviderInstallStatus {
    fn checking() -> Self {
        Self {
            app: None,
            cli: None,
            used: None,
            cli_applicable: true,
            checking: true,
        }
    }
}

fn provider_install_status(
    provider: ProviderKind,
    configured_folder: &str,
) -> ProviderInstallStatus {
    let configured_folder = (!configured_folder.trim().is_empty())
        .then(|| std::path::Path::new(configured_folder.trim()));
    let (app, cli, used) = match provider {
        ProviderKind::Codex => {
            let candidates = crate::discovery::discover(configured_folder);
            let app = candidates
                .iter()
                .find(|candidate| candidate.source == crate::discovery::CandidateSource::DesktopApp)
                .map(|candidate| candidate.path.as_path());
            let cli = candidates
                .iter()
                .find(|candidate| candidate.source != crate::discovery::CandidateSource::DesktopApp)
                .map(|candidate| candidate.path.as_path());
            let used = candidates.first().map(|candidate| match candidate.source {
                crate::discovery::CandidateSource::DesktopApp => ProviderInstallSource::App,
                _ => ProviderInstallSource::Cli,
            });
            (
                app.map(|path| path.display().to_string()),
                cli.map(|path| path.display().to_string()),
                used,
            )
        }
        ProviderKind::Claude => {
            let app = crate::claude_desktop::bundled_cli();
            let cli = crate::claude::cli_available(configured_folder);
            let used = if app.is_some() {
                Some(ProviderInstallSource::App)
            } else {
                cli.as_ref().map(|_| ProviderInstallSource::Cli)
            };
            (
                app.map(|path| path.display().to_string()),
                cli.map(|path| path.display().to_string()),
                used,
            )
        }
        ProviderKind::Cursor => {
            let app = crate::cursor::installation_path(configured_folder);
            let used = app.as_ref().map(|_| ProviderInstallSource::App);
            (app.map(|path| path.display().to_string()), None, used)
        }
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => {
            let detected = crate::opencode::is_installed(provider);
            let detail = detected.then(|| "OpenCode auth.json or local database".into());
            (detail, None, detected.then_some(ProviderInstallSource::App))
        }
        ProviderKind::OpenRouter => {
            let detected = crate::openrouter::is_installed();
            let detail = detected.then(|| "OpenRouter account credentials are configured".into());
            (detail, None, detected.then_some(ProviderInstallSource::App))
        }
    };
    ProviderInstallStatus {
        app,
        cli,
        used,
        cli_applicable: matches!(provider, ProviderKind::Codex | ProviderKind::Claude),
        checking: false,
    }
}

fn provider_install_status_card(status: &ProviderInstallStatus) -> Element {
    if status.checking {
        return border(
            text_block("Checking installed app and CLI…")
                .font_size(12.0)
                .opacity(0.72),
        )
        .padding(settings_card_padding())
        .background(ThemeRef::SubtleFill)
        .corner_radius(6.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into();
    }
    let status_line =
        |label: &str, path: Option<&String>, used: bool, unavailable: bool| -> Element {
            let mut title = Vec::<Element>::new();
            if used {
                title.push(
                    crate::icons::element("check-circle-fill", 15.0, Color::rgb(65, 184, 131))
                        .vertical_alignment(VerticalAlignment::Center),
                );
            }
            title.push(
                text_block(format!("{label}:"))
                    .font_size(12.0)
                    .bold()
                    .into(),
            );
            let detail = if unavailable {
                "Not applicable".into()
            } else {
                path.cloned().unwrap_or_else(|| "Not found".into())
            };
            vstack((
                hstack(title).spacing(5.0),
                text_block(detail).font_size(12.0).opacity(0.72).wrap(),
            ))
            .spacing(2.0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .into()
        };
    border(
        vstack((
            status_line(
                "Desktop App",
                status.app.as_ref(),
                status.used == Some(ProviderInstallSource::App),
                false,
            ),
            status_line(
                "CLI",
                status.cli.as_ref(),
                status.used == Some(ProviderInstallSource::Cli),
                !status.cli_applicable,
            ),
        ))
        .spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(settings_card_padding())
    .background(ThemeRef::SubtleFill)
    .corner_radius(6.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn opencode_credentials_card(
    provider: ProviderKind,
    key_input: &str,
    set_key_input: SetState<String>,
    settings_tx: Sender<Settings>,
) -> Element {
    let provider_name = provider.display_name();
    let manual_key_saved = crate::opencode::key_is_configured(provider);
    let detected = crate::opencode::is_installed(provider);
    let source = if manual_key_saved {
        "Manual key saved in protected Windows user storage."
    } else if detected {
        "OpenCode auth.json or local history detected; automatic discovery is active."
    } else {
        "No key or local history detected yet."
    };
    let save_input = key_input.to_owned();
    let save_setter = set_key_input.clone();
    let save_tx = settings_tx.clone();
    let clear_setter = set_key_input.clone();
    let clear_tx = settings_tx;
    border(
        vstack((
            text_block(format!("{provider_name} API key"))
                .font_size(12.0)
                .bold(),
            text_block(source).font_size(11.0).opacity(0.72).wrap(),
            PasswordBox::new()
                .placeholder_text("Paste a manual API key (optional)")
                .on_password_changed(set_key_input)
                .height(32.0),
            hstack((
                Button::new("Save key").on_click(move || {
                    let value = save_input.trim().to_owned();
                    if value.is_empty() {
                        notifications::show(
                            "OpenCode key not saved",
                            "Paste an API key before saving it.",
                        );
                        return;
                    }
                    persist_opencode_manual_key(
                        provider,
                        Some(value),
                        save_setter.clone(),
                        save_tx.clone(),
                    );
                }),
                Button::new("Clear key").on_click(move || {
                    persist_opencode_manual_key(
                        provider,
                        None,
                        clear_setter.clone(),
                        clear_tx.clone(),
                    );
                }),
            ))
            .spacing(8.0),
        ))
        .spacing(6.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(settings_card_padding())
    .background(ThemeRef::SubtleFill)
    .corner_radius(6.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn opencode_detection_card(provider: ProviderKind) -> Element {
    let detected = crate::opencode::is_installed(provider);
    let status = if detected {
        "Detected from OpenCode auth.json, environment, manual key, or local history."
    } else {
        "No OpenCode credential or local history detected yet."
    };
    border(
        vstack((
            text_block("OpenCode local source").font_size(12.0).bold(),
            text_block(status).font_size(11.0).opacity(0.72).wrap(),
        ))
        .spacing(2.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(settings_card_padding())
    .background(ThemeRef::SubtleFill)
    .corner_radius(6.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn openrouter_accounts_card(
    accounts: &[OpenRouterAccount],
    set_accounts: SetState<Vec<OpenRouterAccount>>,
    key_inputs: &HashMap<String, String>,
    set_key_inputs: SetState<HashMap<String, String>>,
    management_inputs: &HashMap<String, String>,
    set_management_inputs: SetState<HashMap<String, String>>,
    settings_tx: Sender<Settings>,
) -> Element {
    let mut account_cards: Vec<Element> = Vec::new();
    for account in accounts {
        let account_id = account.id.clone();
        let account_name = account.name.clone();
        let mut api_key_cards: Vec<Element> = Vec::new();
        for (key_index, key_id) in account.api_key_ids.iter().enumerate() {
            let input_id = format!("{}:{key_id}", account.id);
            let input_value = key_inputs.get(&input_id).cloned().unwrap_or_default();
            let key_saved = crate::openrouter::api_key_is_configured(&account.id, key_id);
            let save_account_id = account.id.clone();
            let save_key_id = key_id.clone();
            let save_input = input_value.clone();
            let save_input_id = input_id.clone();
            let save_inputs = key_inputs.clone();
            let save_input_setter = set_key_inputs.clone();
            let save_tx = settings_tx.clone();
            let clear_account_id = account.id.clone();
            let clear_key_id = key_id.clone();
            let clear_input_id = input_id.clone();
            let clear_inputs = key_inputs.clone();
            let clear_input_setter = set_key_inputs.clone();
            let clear_tx = settings_tx.clone();
            let remove_account_id = account.id.clone();
            let remove_key_id = key_id.clone();
            let remove_setter = set_accounts.clone();
            let remove_tx = settings_tx.clone();
            api_key_cards.push(
                border(
                    vstack((
                        text_block(format!("API key {}", key_index + 1))
                            .font_size(12.0)
                            .bold(),
                        text_block(if key_saved {
                            "Saved in protected Windows user storage."
                        } else {
                            "No API key configured yet."
                        })
                        .font_size(11.0)
                        .opacity(0.72)
                        .wrap(),
                        PasswordBox::new()
                            .value(input_value)
                            .placeholder_text(if key_saved {
                                "Enter a replacement API key"
                            } else {
                                "Paste an OpenRouter API key"
                            })
                            .on_password_changed({
                                let input_id = input_id.clone();
                                let inputs = key_inputs.clone();
                                let setter = set_key_inputs.clone();
                                move |value: String| {
                                    let mut next = inputs.clone();
                                    next.insert(input_id.clone(), value);
                                    setter.call(next);
                                }
                            })
                            .height(32.0),
                        hstack((
                            Button::new("Save key").on_click(move || {
                                let value = save_input.trim().to_owned();
                                if value.is_empty() {
                                    notifications::show(
                                        "OpenRouter key not saved",
                                        "Paste an API key before saving it.",
                                    );
                                    return;
                                }
                                persist_openrouter_api_key(
                                    save_account_id.clone(),
                                    save_key_id.clone(),
                                    Some(value),
                                    save_input_id.clone(),
                                    save_inputs.clone(),
                                    save_input_setter.clone(),
                                    save_tx.clone(),
                                );
                            }),
                            Button::new("Clear").on_click(move || {
                                persist_openrouter_api_key(
                                    clear_account_id.clone(),
                                    clear_key_id.clone(),
                                    None,
                                    clear_input_id.clone(),
                                    clear_inputs.clone(),
                                    clear_input_setter.clone(),
                                    clear_tx.clone(),
                                );
                            }),
                            Button::new("Remove").on_click(move || {
                                if let Err(error) = crate::openrouter::save_account_api_key(
                                    &remove_account_id,
                                    &remove_key_id,
                                    None,
                                ) {
                                    notifications::show(
                                        "OpenRouter key not removed",
                                        &format!("{error:#}"),
                                    );
                                    return;
                                }
                                // Mutate the persisted account by stable id so a
                                // stale UI snapshot cannot reassign the key row
                                // to a neighboring account after list shifts.
                                let account_id = remove_account_id.clone();
                                let key_id = remove_key_id.clone();
                                mutate_openrouter_accounts(
                                    remove_setter.clone(),
                                    remove_tx.clone(),
                                    move |accounts| {
                                        let Some(account) = accounts
                                            .iter_mut()
                                            .find(|account| account.id == account_id)
                                        else {
                                            return false;
                                        };
                                        let before = account.api_key_ids.len();
                                        account.api_key_ids.retain(|id| id != &key_id);
                                        account.api_key_ids.len() != before
                                    },
                                );
                            }),
                        ))
                        .spacing(8.0),
                    ))
                    .spacing(6.0)
                    .horizontal_alignment(HorizontalAlignment::Stretch),
                )
                .padding(settings_card_padding())
                .background(ThemeRef::SubtleFill)
                .corner_radius(6.0)
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .with_key(format!("openrouter-api-card-{}-{key_id}", account.id))
                .into(),
            );
        }

        let management_input = management_inputs
            .get(&account.id)
            .cloned()
            .unwrap_or_default();
        let management_saved = crate::openrouter::management_key_is_configured(&account.id);
        let save_management_account_id = account.id.clone();
        let save_management_input = management_input.clone();
        let save_management_inputs = management_inputs.clone();
        let save_management_input_setter = set_management_inputs.clone();
        let save_management_tx = settings_tx.clone();
        let clear_management_account_id = account.id.clone();
        let clear_management_inputs = management_inputs.clone();
        let clear_management_input_setter = set_management_inputs.clone();
        let clear_management_tx = settings_tx.clone();
        let add_key_account_id = account.id.clone();
        let add_key_setter = set_accounts.clone();
        let add_key_tx = settings_tx.clone();
        let remove_account = account.clone();
        let remove_account_setter = set_accounts.clone();
        let remove_account_tx = settings_tx.clone();
        let rename_account_id = account.id.clone();
        let rename_setter = set_accounts.clone();
        let rename_tx = settings_tx.clone();
        account_cards.push(
            border(
                vstack((
                    hstack((
                        text_block("Account name").font_size(12.0).bold(),
                        Button::new("Remove account").on_click(move || {
                            for key_id in &remove_account.api_key_ids {
                                if let Err(error) = crate::openrouter::save_account_api_key(
                                    &remove_account.id,
                                    key_id,
                                    None,
                                ) {
                                    notifications::show(
                                        "OpenRouter account not removed",
                                        &format!("{error:#}"),
                                    );
                                    return;
                                }
                            }
                            if let Err(error) =
                                crate::openrouter::save_management_key(&remove_account.id, None)
                            {
                                notifications::show(
                                    "OpenRouter account not removed",
                                    &format!("{error:#}"),
                                );
                                return;
                            }
                            let removed_id = remove_account.id.clone();
                            mutate_openrouter_accounts(
                                remove_account_setter.clone(),
                                remove_account_tx.clone(),
                                move |accounts| {
                                    let before = accounts.len();
                                    accounts.retain(|account| account.id != removed_id);
                                    accounts.len() != before
                                },
                            );
                        }),
                    ))
                    .spacing(8.0)
                    .horizontal_alignment(HorizontalAlignment::Stretch),
                    text_box(account_name)
                        .placeholder_text("OpenRouter account name")
                        .on_commit(move |value: String| {
                            let account_id = rename_account_id.clone();
                            mutate_openrouter_accounts(
                                rename_setter.clone(),
                                rename_tx.clone(),
                                move |accounts| {
                                    let Some(account) = accounts
                                        .iter_mut()
                                        .find(|account| account.id == account_id)
                                    else {
                                        return false;
                                    };
                                    if account.name == value {
                                        return false;
                                    }
                                    account.name = value;
                                    true
                                },
                            );
                        })
                        .height(32.0),
                    text_block(if management_saved {
                        "Management key saved; it is used for the account credit balance."
                    } else {
                        "Add a management key to read the account credit balance."
                    })
                    .font_size(11.0)
                    .opacity(0.72)
                    .wrap(),
                    PasswordBox::new()
                        .value(management_input)
                        .placeholder_text(if management_saved {
                            "Enter a replacement management key"
                        } else {
                            "Paste a management key (optional)"
                        })
                        .on_password_changed({
                            let account_id = account.id.clone();
                            let inputs = management_inputs.clone();
                            let setter = set_management_inputs.clone();
                            move |value: String| {
                                let mut next = inputs.clone();
                                next.insert(account_id.clone(), value);
                                setter.call(next);
                            }
                        })
                        .height(32.0),
                    hstack((
                        Button::new("Save management key").on_click(move || {
                            let value = save_management_input.trim().to_owned();
                            if value.is_empty() {
                                notifications::show(
                                    "OpenRouter management key not saved",
                                    "Paste a management key before saving it.",
                                );
                                return;
                            }
                            persist_openrouter_management_key(
                                save_management_account_id.clone(),
                                Some(value),
                                save_management_inputs.clone(),
                                save_management_input_setter.clone(),
                                save_management_tx.clone(),
                            );
                        }),
                        Button::new("Clear management key").on_click(move || {
                            persist_openrouter_management_key(
                                clear_management_account_id.clone(),
                                None,
                                clear_management_inputs.clone(),
                                clear_management_input_setter.clone(),
                                clear_management_tx.clone(),
                            );
                        }),
                    ))
                    .spacing(8.0),
                    vstack(api_key_cards)
                        .spacing(8.0)
                        .with_layout_animation(
                            LayoutAnimationConfig::linear(duration(CONTROL_NORMAL_ANIMATION))
                                .animate_size(true),
                        ),
                    Button::new("Add API key").on_click(move || {
                        let account_id = add_key_account_id.clone();
                        mutate_openrouter_accounts(
                            add_key_setter.clone(),
                            add_key_tx.clone(),
                            move |accounts| {
                                let Some(account) = accounts
                                    .iter_mut()
                                    .find(|account| account.id == account_id)
                                else {
                                    return false;
                                };
                                account
                                    .api_key_ids
                                    .push(OpenRouterAccount::new_api_key_id());
                                true
                            },
                        );
                    }),
                ))
                .spacing(8.0)
                .horizontal_alignment(HorizontalAlignment::Stretch),
            )
            .padding(settings_card_padding())
            .background(ThemeRef::CardBackground)
            .corner_radius(8.0)
            .border_thickness(Thickness::uniform(1.0))
            .border_brush(ThemeRef::CardStroke)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .with_key(format!("openrouter-account-card-{account_id}"))
            .into(),
        );
    }

    let add_account_setter = set_accounts;
    let add_account_tx = settings_tx;
    border(
        vstack((
            text_block("OpenRouter accounts").font_size(12.0).bold(),
            text_block(
                "Each account can contain any number of API keys and one management key. API keys show individual usage; the management key provides the shared account balance.",
            )
            .font_size(11.0)
            .opacity(0.72)
            .wrap(),
            vstack(account_cards)
                .spacing(10.0)
                .with_layout_animation(
                    LayoutAnimationConfig::linear(duration(CONTROL_NORMAL_ANIMATION))
                        .animate_size(true),
                ),
            Button::new("Add account").on_click(move || {
                mutate_openrouter_accounts(
                    add_account_setter.clone(),
                    add_account_tx.clone(),
                    move |accounts| {
                        let next_index = accounts.len() + 1;
                        accounts.push(OpenRouterAccount::new(format!("Account {next_index}")));
                        true
                    },
                );
            }),
        ))
        .spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(settings_card_padding())
    .background(ThemeRef::SubtleFill)
    .corner_radius(6.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .with_layout_animation(
        LayoutAnimationConfig::linear(duration(CONTROL_NORMAL_ANIMATION)).animate_size(true),
    )
    .into()
}

/// Apply an OpenRouter account-list mutation against the on-disk settings, never
/// against a stale UI snapshot. Account membership is always addressed by
/// stable account id so list shifts cannot move API keys between accounts.
///
/// The mutator returns `true` when it changed anything; no-ops skip the
/// credentials revision bump so workers are not restarted for free.
fn mutate_openrouter_accounts(
    setter: SetState<Vec<OpenRouterAccount>>,
    settings_tx: Sender<Settings>,
    mutate: impl FnOnce(&mut Vec<OpenRouterAccount>) -> bool + 'static,
) {
    persist_update(settings_tx, move |settings| {
        // Include the synthetic legacy account when present so edits land on the
        // same identities the Settings UI is showing.
        let mut accounts = crate::openrouter::accounts_for_settings(settings);
        if !mutate(&mut accounts) {
            return;
        }
        settings.openrouter_accounts = accounts;
        settings.openrouter_credentials_revision =
            settings.openrouter_credentials_revision.wrapping_add(1);
        setter.call(crate::openrouter::accounts_for_settings(settings));
    });
}

fn persist_openrouter_api_key(
    account_id: String,
    key_id: String,
    value: Option<String>,
    input_id: String,
    mut inputs: HashMap<String, String>,
    input_setter: SetState<HashMap<String, String>>,
    settings_tx: Sender<Settings>,
) {
    if let Err(error) =
        crate::openrouter::save_account_api_key(&account_id, &key_id, value.as_deref())
    {
        eprintln!("failed to save OpenRouter API key: {error:#}");
        notifications::show("OpenRouter key not saved", &format!("{error:#}"));
        return;
    }
    inputs.remove(&input_id);
    input_setter.call(inputs);
    persist_update(settings_tx, |settings| {
        settings.openrouter_credentials_revision =
            settings.openrouter_credentials_revision.wrapping_add(1);
    });
}

fn persist_openrouter_management_key(
    account_id: String,
    value: Option<String>,
    mut inputs: HashMap<String, String>,
    input_setter: SetState<HashMap<String, String>>,
    settings_tx: Sender<Settings>,
) {
    if let Err(error) = crate::openrouter::save_management_key(&account_id, value.as_deref()) {
        eprintln!("failed to save OpenRouter management key: {error:#}");
        notifications::show("OpenRouter management key not saved", &format!("{error:#}"));
        return;
    }
    inputs.remove(&account_id);
    input_setter.call(inputs);
    persist_update(settings_tx, |settings| {
        settings.openrouter_credentials_revision =
            settings.openrouter_credentials_revision.wrapping_add(1);
    });
}

fn persist_opencode_manual_key(
    provider: ProviderKind,
    value: Option<String>,
    input_setter: SetState<String>,
    settings_tx: Sender<Settings>,
) {
    let result = crate::opencode::save_manual_key(provider, value.as_deref());
    if let Err(error) = result {
        eprintln!(
            "failed to save {} manual key: {error:#}",
            provider.display_name()
        );
        notifications::show("OpenCode key not saved", &format!("{error:#}"));
        return;
    }
    input_setter.call(String::new());
    persist_update(settings_tx, move |settings| match provider {
        ProviderKind::OpenCodeZen => {
            settings.opencode_zen_credentials_revision =
                settings.opencode_zen_credentials_revision.wrapping_add(1);
        }
        ProviderKind::OpenCodeGo => {
            settings.opencode_go_credentials_revision =
                settings.opencode_go_credentials_revision.wrapping_add(1);
        }
        _ => {}
    });
}

fn persist_provider_folder(provider: ProviderKind, value: String, settings_tx: Sender<Settings>) {
    let generation = match provider {
        ProviderKind::Codex => &CODEX_PATH_SAVE_GEN,
        ProviderKind::Claude => &CLAUDE_PATH_SAVE_GEN,
        ProviderKind::Cursor => &CURSOR_PATH_SAVE_GEN,
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo | ProviderKind::OpenRouter => return,
    };
    let revision = generation.fetch_add(1, Ordering::Relaxed) + 1;
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        let generation = match provider {
            ProviderKind::Codex => &CODEX_PATH_SAVE_GEN,
            ProviderKind::Claude => &CLAUDE_PATH_SAVE_GEN,
            ProviderKind::Cursor => &CURSOR_PATH_SAVE_GEN,
            ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo | ProviderKind::OpenRouter => {
                return;
            }
        };
        if generation.load(Ordering::Relaxed) != revision {
            return;
        }
        let folder = (!value.trim().is_empty()).then(|| PathBuf::from(value.trim()));
        persist_update(settings_tx, move |settings| match provider {
            ProviderKind::Codex => settings.codex_path = folder,
            ProviderKind::Claude => settings.claude_path = folder,
            ProviderKind::Cursor => settings.cursor_path = folder,
            ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo | ProviderKind::OpenRouter => {}
        });
    });
}

/// On WM_CLOSE / SC_CLOSE, hide the window while it still looks correct, then
/// let the default close path destroy it. Without this, content is dismantled
/// while the HWND is still visible → black flash with OS chrome.
#[cfg(windows)]
fn install_settings_close_hide() {
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SC_CLOSE, SW_HIDE, ShowWindow, WM_CLOSE, WM_NCDESTROY, WM_SYSCOMMAND,
    };

    const SUBCLASS_ID: usize = 0xC0DE_5E77;

    let hwnd = find_settings_window();
    if hwnd.is_null() {
        return;
    }

    unsafe extern "system" fn subclass_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _uid: usize,
        _data: usize,
    ) -> LRESULT {
        let is_close =
            msg == WM_CLOSE || (msg == WM_SYSCOMMAND && (wparam & 0xFFF0) as u32 == SC_CLOSE);
        if is_close {
            // Hide while fully painted; default processing then destroys.
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
        if msg == WM_NCDESTROY {
            unsafe {
                RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
            }
        }
        unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
    }

    unsafe {
        let _ = SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, 0);
    }
}

#[cfg(windows)]
fn set_settings_window_icon() {
    use windows_sys::Win32::{
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{ICON_BIG, ICON_SMALL, LoadIconW, SendMessageW, WM_SETICON},
    };

    let hwnd = find_settings_window();
    if hwnd.is_null() {
        return;
    }

    // `winresource` embeds the application icon as resource 1.
    let module = unsafe { GetModuleHandleW(std::ptr::null()) };
    let icon = unsafe { LoadIconW(module, 1usize as *const u16) };
    if !icon.is_null() {
        unsafe {
            SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, icon as isize);
            SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, icon as isize);
        }
    }
}

/// The caption buttons are painted by DWM, outside the XAML `TitleBar` tree.
/// Keep their light/dark glyphs in lockstep with the live WinUI theme.
#[cfg(windows)]
fn sync_settings_caption_button_theme(color_scheme: ColorScheme) {
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;

    const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
    let hwnd = find_settings_window();
    if hwnd.is_null() {
        return;
    }

    let use_dark_caption_buttons = i32::from(matches!(color_scheme, ColorScheme::Dark));
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &use_dark_caption_buttons as *const i32 as *const _,
            size_of::<i32>() as u32,
        );
    }
}

#[cfg(windows)]
fn find_settings_window() -> windows_sys::Win32::Foundation::HWND {
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;

    [SETTINGS_WINDOW_TITLE, ONBOARDING_WINDOW_TITLE]
        .into_iter()
        .map(|title| {
            let title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
            unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) }
        })
        .find(|hwnd| !hwnd.is_null())
        .unwrap_or(std::ptr::null_mut())
}

#[cfg(not(windows))]
fn sync_settings_caption_button_theme(_color_scheme: ColorScheme) {}

#[cfg(not(windows))]
fn set_settings_window_icon() {}

#[cfg(not(windows))]
fn install_settings_close_hide() {}

fn load_settings_for_window() -> Settings {
    match Settings::default_path().and_then(|path| Settings::load_or_create(&path)) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("failed to load settings for window: {error:#}");
            Settings::default()
        }
    }
}

/// Whether the independent settings surface is currently alive.
///
/// The tray popup uses this to stay visible as a live preview while a user
/// navigates settings and changes popup-related options.
#[cfg(windows)]
pub(crate) fn is_open() -> bool {
    !find_settings_window().is_null()
}

#[cfg(not(windows))]
pub(crate) fn is_open() -> bool {
    false
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum OnboardingStep {
    #[default]
    Providers,
    General,
}

/// Compact first-launch surface. It deliberately reuses the same setting
/// controls as the full editor, but persists exactly once on Done.
fn onboarding_render(
    cx: &mut RenderCx,
    settings: Arc<Settings>,
    detected: [bool; 6],
    settings_tx: Sender<Settings>,
) -> Element {
    let color_scheme = cx.use_color_scheme();
    cx.use_effect(color_scheme, move || {
        sync_settings_caption_button_theme(color_scheme);
    });
    let (step, set_step) = cx.use_state(OnboardingStep::default());
    let (codex_enabled, set_codex_enabled) = cx.use_state(detected[0]);
    let (claude_enabled, set_claude_enabled) = cx.use_state(detected[1]);
    let (cursor_enabled, set_cursor_enabled) = cx.use_state(detected[2]);
    let (opencode_zen_enabled, set_opencode_zen_enabled) = cx.use_state(detected[3]);
    let (opencode_go_enabled, set_opencode_go_enabled) = cx.use_state(detected[4]);
    let (openrouter_enabled, set_openrouter_enabled) = cx.use_state(detected[5]);
    let (start_at_login, set_start_at_login) = cx.use_state(settings.start_at_login);
    let (automatic_activation, set_automatic_activation) =
        cx.use_state(settings.automatic_activation);
    let (limit_refresh_interval, set_limit_refresh_interval) =
        cx.use_state(settings.limit_refresh_interval);
    let (show_used_percentage, set_show_used_percentage) =
        cx.use_state(settings.show_used_percentage);
    let (show_usage_pace, set_show_usage_pace) = cx.use_state(settings.show_usage_pace);
    let (show_account_name, set_show_account_name) = cx.use_state(settings.show_account_name);
    let (hovered_card_id, set_hovered_card_id) = cx.use_state(None::<String>);

    let (heading, description, cards): (&str, &str, Vec<Element>) = match step {
        OnboardingStep::Providers => (
            "Choose providers",
            "We found the providers installed on this PC and selected them for you. You can change these choices now or later in Settings.",
            vec![
                settings_toggle_card_with_description(
                    "Codex",
                    Some(if detected[0] {
                        "Detected on this PC."
                    } else {
                        "Not detected — enable it if it is installed elsewhere."
                    }),
                    codex_enabled,
                    move |value| set_codex_enabled.call(value),
                    "onboarding-codex",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-codex"),
                settings_toggle_card_with_description(
                    "Claude",
                    Some(if detected[1] {
                        "Detected on this PC."
                    } else {
                        "Not detected — enable it if it is installed elsewhere."
                    }),
                    claude_enabled,
                    move |value| set_claude_enabled.call(value),
                    "onboarding-claude",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-claude"),
                settings_toggle_card_with_description(
                    "Cursor",
                    Some(if detected[2] {
                        "Detected on this PC."
                    } else {
                        "Not detected — enable it if it is installed elsewhere."
                    }),
                    cursor_enabled,
                    move |value| set_cursor_enabled.call(value),
                    "onboarding-cursor",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-cursor"),
                settings_toggle_card_with_description(
                    "OpenCode Zen",
                    Some(if detected[3] {
                        "Detected from OpenCode auth or local history."
                    } else {
                        "Not detected — enable it if OpenCode Zen is configured elsewhere."
                    }),
                    opencode_zen_enabled,
                    move |value| set_opencode_zen_enabled.call(value),
                    "onboarding-opencode-zen",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-opencode-zen"),
                settings_toggle_card_with_description(
                    "OpenCode Go",
                    Some(if detected[4] {
                        "Detected from OpenCode auth or local history."
                    } else {
                        "Not detected — enable it if OpenCode Go is configured elsewhere."
                    }),
                    opencode_go_enabled,
                    move |value| set_opencode_go_enabled.call(value),
                    "onboarding-opencode-go",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-opencode-go"),
                settings_toggle_card_with_description(
                    "OpenRouter",
                    Some(if detected[5] {
                        "OpenRouter account credentials are configured."
                    } else {
                        "Optional — configure accounts later in Settings > Providers."
                    }),
                    openrouter_enabled,
                    move |value| set_openrouter_enabled.call(value),
                    "onboarding-openrouter",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-openrouter"),
            ],
        ),
        OnboardingStep::General => (
            "General settings",
            "Set the basics for Codex Minibar. Every option can be changed later in Settings.",
            vec![
                settings_section_heading("Startup").with_key("onboarding-startup-heading"),
                settings_toggle_card_with_description(
                    "Start at login",
                    Some("Opens Codex Minibar automatically after you sign in."),
                    start_at_login,
                    move |value| set_start_at_login.call(value),
                    "onboarding-start-at-login",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-start-at-login"),
                settings_section_heading("Features").with_key("onboarding-features-heading"),
                settings_toggle_card_with_description(
                    "Activate limits automatically",
                    Some("Starts a supported provider's 5-hour window when needed."),
                    automatic_activation,
                    move |value| set_automatic_activation.call(value),
                    "onboarding-automatic-activation",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-automatic-activation"),
                settings_control_card(
                    "Refresh limits period",
                    Some("How often enabled providers fetch their current limits."),
                    ComboBox::new([
                        "30 seconds",
                        "1 minute",
                        "5 minutes",
                        "10 minutes",
                        "15 minutes",
                    ])
                    .selected_index(limit_refresh_interval.index())
                    .on_selection_changed(move |choice| {
                        set_limit_refresh_interval.call(LimitRefreshInterval::from_index(choice));
                    }),
                    "onboarding-limit-refresh-interval",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-limit-refresh-interval"),
                settings_section_heading("Customization")
                    .with_key("onboarding-customization-heading"),
                settings_toggle_card_with_description(
                    "Replace amount left with amount used",
                    Some("Shows consumed usage instead of the remaining amount."),
                    show_used_percentage,
                    move |value| set_show_used_percentage.call(value),
                    "onboarding-show-used",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-show-used"),
                settings_toggle_card_with_description(
                    "Show usage pace",
                    Some("Shows expected use and whether consumption is on pace."),
                    show_usage_pace,
                    move |value| set_show_usage_pace.call(value),
                    "onboarding-show-usage-pace",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-show-usage-pace"),
                settings_toggle_card_with_description(
                    "Show account name",
                    Some("Shows your Codex name or Claude organization in the popup."),
                    show_account_name,
                    move |value| set_show_account_name.call(value),
                    "onboarding-show-account-name",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-show-account-name"),
            ],
        ),
    };

    let back_or_spacer: Element = match step {
        OnboardingStep::Providers => border(Element::Empty).width(72.0).into(),
        OnboardingStep::General => {
            let set_step = set_step.clone();
            Button::new("Back")
                .on_click(move || set_step.call(OnboardingStep::Providers))
                .into()
        }
    };
    let action: Element = match step {
        OnboardingStep::Providers => {
            let set_step = set_step.clone();
            Button::new("Continue")
                .accent()
                .on_click(move || set_step.call(OnboardingStep::General))
                .into()
        }
        OnboardingStep::General => {
            let settings_tx = settings_tx.clone();
            let settings = Arc::clone(&settings);
            Button::new("Done")
                .accent()
                .on_click(move || {
                    let mut completed = (*settings).clone();
                    completed.onboarding_completed = true;
                    completed.providers = crate::settings::ProviderSettings::from_enabled(
                        crate::provider_registry::PROVIDERS
                            .iter()
                            .filter(|provider| match provider.kind {
                                ProviderKind::Codex => codex_enabled,
                                ProviderKind::Claude => claude_enabled,
                                ProviderKind::Cursor => cursor_enabled,
                                ProviderKind::OpenCodeZen => opencode_zen_enabled,
                                ProviderKind::OpenCodeGo => opencode_go_enabled,
                                ProviderKind::OpenRouter => openrouter_enabled,
                            })
                            .map(|provider| provider.kind),
                    );
                    completed.tray_widgets = crate::provider_registry::PROVIDERS
                        .iter()
                        .filter(|provider| completed.providers.is_enabled(provider.kind))
                        .filter(|provider| !provider.default_tray_metrics.is_empty())
                        .map(|provider| TrayWidget::for_provider(provider.kind))
                        .collect();
                    completed.start_at_login = start_at_login;
                    completed.automatic_activation = automatic_activation;
                    completed.limit_refresh_interval = limit_refresh_interval;
                    completed.show_used_percentage = show_used_percentage;
                    completed.show_usage_pace = show_usage_pace;
                    completed.show_account_name = show_account_name;
                    if let Err(error) = replace_settings(settings_tx.clone(), completed) {
                        eprintln!("failed to complete onboarding: {error:#}");
                        return;
                    }
                    // The popup host shares this UI thread. Prepare it before
                    // dismissing onboarding so Done always lands on the popup.
                    if crate::popup::prepare_show_on_ui_thread() {
                        crate::popup::show_near_cursor();
                    }
                    close_open_window();
                })
                .into()
        }
    };

    let content = scroll_viewer(
        vstack((
            text_block(heading).font_size(28.0).font_weight(600),
            text_block(description).font_size(14.0).opacity(0.72).wrap(),
            vstack(cards).spacing(10.0),
        ))
        .spacing(16.0)
        .padding(Thickness {
            left: 32.0,
            top: 28.0,
            right: 32.0,
            bottom: 20.0,
        })
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .horizontal_scroll_bar_visibility(ScrollBarVisibility::Disabled)
    .vertical_scroll_bar_visibility(ScrollBarVisibility::Auto)
    .grid_row(0);
    let footer = border(
        hstack((back_or_spacer, action))
            .spacing(8.0)
            .horizontal_alignment(HorizontalAlignment::Right),
    )
    .padding(Thickness {
        left: 32.0,
        top: 14.0,
        right: 32.0,
        bottom: 18.0,
    })
    .border_thickness(Thickness {
        left: 0.0,
        top: 1.0,
        right: 0.0,
        bottom: 0.0,
    })
    .border_brush(ThemeRef::CardStroke)
    .grid_row(1);
    let title_bar = TitleBar::new(ONBOARDING_WINDOW_TITLE)
        .back_button_visible(false)
        .pane_toggle_button_visible(false)
        .tall(true);
    let body = grid((content, footer))
        .rows([GridLength::Star(1.0), GridLength::Auto])
        .columns([GridLength::Star(1.0)])
        .background(ThemeRef::LayerFill)
        .grid_row(1);
    grid((title_bar.grid_row(0), body))
        .rows([GridLength::Auto, GridLength::Star(1.0)])
        .columns([GridLength::Star(1.0)])
        .background(ThemeRef::LayerFill)
        .into()
}

#[cfg(windows)]
fn close_open_window() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
    let hwnd = find_settings_window();
    if !hwnd.is_null() {
        unsafe {
            let _ = PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
    }
}

#[cfg(not(windows))]
fn close_open_window() {}

/// Resetting returns to the same first-launch path as a new install. Wait for
/// the current native host to close before creating the onboarding host so the
/// two settings surfaces can never overlap or fight over the host slot.
fn restart_onboarding_after_reset(settings_tx: Sender<Settings>, ui_dispatcher: UiMarshaller) {
    close_open_window();
    thread::spawn(move || {
        for _ in 0..20 {
            if !is_open() {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        ui_dispatcher.dispatch(move || {
            if let Err(error) = open_onboarding(settings_tx) {
                eprintln!("failed to reopen onboarding after settings reset: {error:?}");
            }
        });
    });
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Tab {
    #[default]
    General,
    Appearance,
    Providers,
    Popup,
    Schedule,
    Tray,
    Notifications,
    Advanced,
    Log,
    About,
}

impl Tab {
    fn tag(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Appearance => "appearance",
            Self::Providers => "providers",
            Self::Popup => "customize",
            Self::Schedule => "schedule",
            Self::Tray => "tray",
            Self::Notifications => "notifications",
            Self::Advanced => "advanced",
            Self::Log => "log",
            Self::About => "about",
        }
    }

    fn from_tag(tag: &str) -> Self {
        match tag {
            "appearance" => Self::Appearance,
            "tray" => Self::Tray,
            "providers" => Self::Providers,
            "popup" | "customize" => Self::Popup,
            "schedule" => Self::Schedule,
            "notifications" => Self::Notifications,
            "advanced" => Self::Advanced,
            "log" => Self::Log,
            "about" => Self::About,
            _ => Self::General,
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum SettingsNavMode {
    #[default]
    Root,
    Providers,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RenderedPage {
    Root(Tab),
    Provider(ProviderKind),
}

impl Default for RenderedPage {
    fn default() -> Self {
        Self::Root(Tab::default())
    }
}

impl RenderedPage {
    fn scroll_key(self) -> String {
        match self {
            Self::Root(tab) => format!("settings-scroll-{}", tab.tag()),
            Self::Provider(provider) => format!("settings-scroll-provider-{}", provider.id()),
        }
    }

    fn page_key(self) -> String {
        match self {
            Self::Root(tab) => format!("settings-page-{}", tab.tag()),
            Self::Provider(provider) => format!("settings-page-provider-{}", provider.id()),
        }
    }
}

fn provider_order_from_popup(popup_order: &[PopupWidgetKind]) -> Vec<ProviderKind> {
    popup_order
        .iter()
        .filter_map(|widget| widget.as_provider())
        .collect()
}

fn first_provider_in_order(popup_order: &[PopupWidgetKind]) -> ProviderKind {
    provider_order_from_popup(popup_order)
        .into_iter()
        .next()
        .unwrap_or(ProviderKind::Codex)
}

fn fade_to_rendered_page(
    set_page_visible: AsyncSetState<bool>,
    set_rendered_page: AsyncSetState<RenderedPage>,
    page: RenderedPage,
) {
    set_page_visible.call(false);
    std::thread::spawn(move || {
        std::thread::sleep(duration(Duration::from_millis(180)));
        set_rendered_page.call(page);
        set_page_visible.call(true);
    });
}

fn root_nav_items(nav_icon_color: &str, use_colored: bool) -> [NavViewItem; 10] {
    let item = |label: &str, tag: &str| {
        let mut nav = NavViewItem::new(label).tag(tag);
        if use_colored {
            nav = nav.icon_image_uri(crate::icons::fluent_color_uri(tag));
        } else {
            nav = nav.icon_path(
                crate::icons::data(crate::icons::sidebar_mono_icon(tag)),
                nav_icon_color,
            );
        }
        nav
    };
    [
        item("General", "general"),
        item("Providers", "providers")
            .trailing_icon_path(crate::icons::data("caret-right"), nav_icon_color),
        item("Customize", "customize"),
        item("Schedule", "schedule"),
        item("Tray", "tray"),
        item("Notifications", "notifications"),
        item("Appearance", "appearance"),
        item("Advanced", "advanced"),
        item("Log", "log"),
        item("About & Updates", "about"),
    ]
}

fn providers_nav_items(popup_order: &[PopupWidgetKind], nav_icon_color: &str) -> Vec<NavViewItem> {
    let mut items = vec![NavViewItem::header("Providers")];
    for provider in provider_order_from_popup(popup_order) {
        let descriptor = crate::provider_registry::descriptor(provider);
        items.push(
            NavViewItem::new(descriptor.display_name)
                .tag(provider.id())
                .icon_path(crate::icons::data(descriptor.icon), nav_icon_color),
        );
    }
    items
}

fn providers_pane_add_footer(
    set_openrouter_accounts: SetState<Vec<OpenRouterAccount>>,
    settings_tx: Sender<Settings>,
    set_selected_provider: SetState<ProviderKind>,
    set_page_visible: AsyncSetState<bool>,
    set_rendered_page: AsyncSetState<RenderedPage>,
) -> Element {
    let add_account_setter = set_openrouter_accounts;
    let add_account_tx = settings_tx;
    let select_openrouter = set_selected_provider;
    let page_visible = set_page_visible;
    let rendered_page = set_rendered_page;
    border(
        vstack((
            text_block("Add OpenRouter accounts here. Other providers use a single signed-in session.")
                .font_size(11.0)
                .opacity(0.72)
                .wrap(),
            Button::new("Add")
                .icon(Symbol::Add)
                .menu_flyout(vec![
                    menu_item("OpenRouter account"),
                    menu_separator(),
                    menu_item("Codex (single session)"),
                    menu_item("Claude (single session)"),
                    menu_item("Cursor (single session)"),
                    menu_item("OpenCode Zen (single session)"),
                    menu_item("OpenCode Go (single session)"),
                ])
                .on_item_clicked(move |choice: String| match choice.as_str() {
                    "OpenRouter account" => {
                        mutate_openrouter_accounts(
                            add_account_setter.clone(),
                            add_account_tx.clone(),
                            move |accounts| {
                                let next_index = accounts.len() + 1;
                                accounts
                                    .push(OpenRouterAccount::new(format!("Account {next_index}")));
                                true
                            },
                        );
                        select_openrouter.call(ProviderKind::OpenRouter);
                        fade_to_rendered_page(
                            page_visible.clone(),
                            rendered_page.clone(),
                            RenderedPage::Provider(ProviderKind::OpenRouter),
                        );
                    }
                    _ => {
                        notifications::show(
                            "Single session",
                            "This provider uses one signed-in session. Open its page to configure it.",
                        );
                    }
                }),
        ))
        .spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(Thickness {
        left: 12.0,
        top: 0.0,
        right: 12.0,
        bottom: 2.0,
    })
    .background(Color::transparent())
    .into()
}

/// Root content for the independent WinUI settings window.
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
    let (time_format, set_time_format) = cx.use_state(settings.time_format);
    cx.use_effect((theme, accent_color, animations_enabled, time_format), move || {
        crate::theme::set_animations_enabled(animations_enabled);
        crate::theme::apply_appearance(theme, accent_color);
        time_format.apply();
    });
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
                            notifications::show("Update failed", &format!("{error:#}"));
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
    let (limit_refresh_interval, set_limit_refresh_interval) =
        cx.use_state(settings.limit_refresh_interval);
    let (show_used_percentage, set_show_used_percentage) =
        cx.use_state(settings.show_used_percentage);
    let (show_usage_pace, set_show_usage_pace) = cx.use_state(settings.show_usage_pace);
    let (popup_visibility, set_popup_visibility) =
        cx.use_state(settings.popup_visibility.clone());
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
            limit_refresh_interval: set_limit_refresh_interval.clone(),
            start_at_login: set_start_at_login.clone(),
            show_used_percentage: set_show_used_percentage.clone(),
            show_usage_pace: set_show_usage_pace.clone(),
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

    let settings_page_body = match rendered_page {
        RenderedPage::Root(tab) => tab_content(
            tab,
            theme,
            accent_color,
            animations_enabled,
            time_format,
            codex_enabled,
            claude_enabled,
            cursor_enabled,
            opencode_zen_enabled,
            opencode_go_enabled,
            openrouter_enabled,
            &codex_path,
            &claude_path,
            &cursor_path,
            &codex_install_status,
            &claude_install_status,
            &cursor_install_status,
            &opencode_zen_install_status,
            &opencode_go_install_status,
            &openrouter_install_status,
            &opencode_zen_key_input,
            &opencode_go_key_input,
            &openrouter_accounts,
            &openrouter_key_inputs,
            &openrouter_management_inputs,
            &popup_order,
            use_colored_provider_icons,
            use_colored_sidebar_icons,
            replace_chatgpt_logo_with_codex,
            automatic_activation,
            &scheduled_activations,
            limit_refresh_interval,
            start_at_login,
            show_used_percentage,
            show_usage_pace,
            &popup_visibility,
            &discovered_popup_bricks,
            show_total_spend_on_all_tab,
            total_spend_presentation,
            show_account_name,
            activation_success,
            activation_failure,
            limits_reset,
            low_usage_enabled,
            low_usage_threshold,
            low_usage_expanded,
            low_usage_expand_progress,
            weekly_low_usage_enabled,
            weekly_low_usage_threshold,
            weekly_low_usage_expanded,
            weekly_low_usage_expand_progress,
            &tray_widgets,
            &expanded_tray_widget,
            &editing_tray_indicator,
            &removed_tray_widget,
            &hovered_card_id,
            &expanded_popup_provider,
            check_for_updates,
            notify_on_update,
            &update_phase,
            &log_content,
            set_codex_enabled,
            set_theme,
            set_accent_color,
            set_animations_enabled,
            set_time_format,
            set_claude_enabled,
            set_cursor_enabled,
            set_opencode_zen_enabled,
            set_opencode_go_enabled,
            set_openrouter_enabled,
            set_opencode_zen_key_input,
            set_opencode_go_key_input,
            set_openrouter_accounts.clone(),
            set_openrouter_key_inputs.clone(),
            set_openrouter_management_inputs.clone(),
            set_codex_path,
            set_claude_path,
            set_cursor_path,
            set_popup_order,
            set_use_colored_provider_icons,
            set_use_colored_sidebar_icons,
            set_replace_chatgpt_logo_with_codex,
            set_automatic_activation,
            set_scheduled_activations.clone(),
            set_limit_refresh_interval,
            set_start_at_login,
            set_show_used_percentage,
            set_show_usage_pace,
            set_popup_visibility,
            set_discovered_popup_bricks,
            set_show_total_spend_on_all_tab,
            set_total_spend_presentation,
            set_show_account_name,
            set_activation_success,
            set_activation_failure,
            set_limits_reset,
            set_low_usage_enabled,
            set_low_usage_threshold,
            set_low_usage_expanded,
            set_low_usage_expand_progress,
            set_weekly_low_usage_enabled,
            set_weekly_low_usage_threshold,
            set_weekly_low_usage_expanded,
            set_weekly_low_usage_expand_progress,
            set_tray_widgets.clone(),
            set_expanded_tray_widget,
            set_editing_tray_indicator.clone(),
            set_indicator_modal_visible.clone(),
            set_removed_tray_widget,
            set_expanded_popup_provider,
            set_hovered_card_id,
            set_check_for_updates,
            set_notify_on_update,
            theme_navigation_guard,
            theme_navigation_guard_timer,
            settings_tx.clone(),
            ui_dispatcher.clone(),
            updates.clone(),
        ),
        RenderedPage::Provider(provider) => provider_page_content(
            provider,
            codex_enabled,
            claude_enabled,
            cursor_enabled,
            opencode_zen_enabled,
            opencode_go_enabled,
            openrouter_enabled,
            &codex_path,
            &claude_path,
            &cursor_path,
            &codex_install_status,
            &claude_install_status,
            &cursor_install_status,
            &opencode_zen_install_status,
            &opencode_go_install_status,
            &openrouter_install_status,
            &opencode_zen_key_input,
            &opencode_go_key_input,
            &openrouter_accounts,
            &openrouter_key_inputs,
            &openrouter_management_inputs,
            &tray_widgets,
            &hovered_card_id,
            set_codex_enabled,
            set_claude_enabled,
            set_cursor_enabled,
            set_opencode_zen_enabled,
            set_opencode_go_enabled,
            set_openrouter_enabled,
            set_opencode_zen_key_input,
            set_opencode_go_key_input,
            set_openrouter_accounts.clone(),
            set_openrouter_key_inputs.clone(),
            set_openrouter_management_inputs.clone(),
            set_codex_path,
            set_claude_path,
            set_cursor_path,
            set_tray_widgets.clone(),
            set_hovered_card_id,
            settings_tx.clone(),
        ),
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

/// About mirrors the README hero with the high-resolution app icon including
/// its rounded background.
fn settings_about_icon_uri() -> String {
    let packaged = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .map(|parent| parent.join("assets/app-icon.png"))
        })
        .filter(|path| path.exists());
    let path = packaged.unwrap_or_else(|| {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/app-icon.png")
    });
    format!("file:///{}", path.to_string_lossy().replace('\\', "/"))
}

#[allow(clippy::too_many_arguments)]
fn provider_page_content(
    provider: ProviderKind,
    codex_enabled: bool,
    claude_enabled: bool,
    cursor_enabled: bool,
    opencode_zen_enabled: bool,
    opencode_go_enabled: bool,
    openrouter_enabled: bool,
    codex_path: &str,
    claude_path: &str,
    cursor_path: &str,
    codex_install_status: &ProviderInstallStatus,
    claude_install_status: &ProviderInstallStatus,
    cursor_install_status: &ProviderInstallStatus,
    opencode_zen_install_status: &ProviderInstallStatus,
    opencode_go_install_status: &ProviderInstallStatus,
    openrouter_install_status: &ProviderInstallStatus,
    opencode_zen_key_input: &str,
    opencode_go_key_input: &str,
    openrouter_accounts: &[OpenRouterAccount],
    openrouter_key_inputs: &HashMap<String, String>,
    openrouter_management_inputs: &HashMap<String, String>,
    tray_widgets: &[TrayWidget],
    hovered_card_id: &Option<String>,
    set_codex_enabled: SetState<bool>,
    set_claude_enabled: SetState<bool>,
    set_cursor_enabled: SetState<bool>,
    set_opencode_zen_enabled: SetState<bool>,
    set_opencode_go_enabled: SetState<bool>,
    set_openrouter_enabled: SetState<bool>,
    set_opencode_zen_key_input: SetState<String>,
    set_opencode_go_key_input: SetState<String>,
    set_openrouter_accounts: SetState<Vec<OpenRouterAccount>>,
    set_openrouter_key_inputs: SetState<HashMap<String, String>>,
    set_openrouter_management_inputs: SetState<HashMap<String, String>>,
    set_codex_path: SetState<String>,
    set_claude_path: SetState<String>,
    set_cursor_path: SetState<String>,
    set_tray_widgets: SetState<Vec<TrayWidget>>,
    set_hovered_card_id: SetState<Option<String>>,
    settings_tx: Sender<Settings>,
) -> Element {
    let apply_codex_enabled = settings_tx.clone();
    let apply_claude_enabled = settings_tx.clone();
    let apply_cursor_enabled = settings_tx.clone();
    let apply_codex_path = settings_tx.clone();
    let apply_claude_path = settings_tx.clone();
    let apply_cursor_path = settings_tx.clone();
    let tray_widgets_for_codex_toggle = tray_widgets.to_vec();
    let tray_widgets_for_claude_toggle = tray_widgets.to_vec();
    let tray_widgets_for_cursor_toggle = tray_widgets.to_vec();
    let tray_widgets_for_opencode_toggle = tray_widgets.to_vec();
    let tray_widget_setter_for_codex_toggle = set_tray_widgets.clone();
    let tray_widget_setter_for_claude_toggle = set_tray_widgets.clone();
    let tray_widget_setter_for_cursor_toggle = set_tray_widgets.clone();
    let tray_widget_setter_for_opencode_toggle = set_tray_widgets.clone();
    let apply_opencode_zen_enabled = settings_tx.clone();
    let apply_opencode_go_enabled = settings_tx.clone();
    let apply_openrouter_enabled = settings_tx.clone();
    let settings_tx_for_details = settings_tx.clone();

    let enable_card = match provider {
        ProviderKind::Codex => settings_toggle_card_with_description(
                "Enabled",
                Some("Reads limits from the locally signed-in Codex CLI or desktop app."),
                codex_enabled,
                move |value| {
                    persist_provider_enabled(
                        set_codex_enabled.clone(),
                        tray_widget_setter_for_codex_toggle.clone(),
                        apply_codex_enabled.clone(),
                        ProviderKind::Codex,
                        value,
                        claude_enabled,
                        cursor_enabled,
                        tray_widgets_for_codex_toggle.clone(),
                    )
                },
                "provider-codex-enabled",
                hovered_card_id,
                set_hovered_card_id.clone(),
            ),
        ProviderKind::Claude => settings_toggle_card_with_description(
                "Enabled",
                Some("Reads limits with the existing signed-in Claude Code OAuth session."),
                claude_enabled,
                move |value| {
                    persist_provider_enabled(
                        set_claude_enabled.clone(),
                        tray_widget_setter_for_claude_toggle.clone(),
                        apply_claude_enabled.clone(),
                        ProviderKind::Claude,
                        value,
                        codex_enabled,
                        cursor_enabled,
                        tray_widgets_for_claude_toggle.clone(),
                    )
                },
                "provider-claude-enabled",
                hovered_card_id,
                set_hovered_card_id.clone(),
            ),
        ProviderKind::Cursor => settings_toggle_card_with_description(
                "Enabled",
                Some("Reads your signed-in Cursor desktop app session and shows the current billing-cycle usage."),
                cursor_enabled,
                move |value| {
                    persist_cursor_enabled(
                        set_cursor_enabled.clone(),
                        tray_widget_setter_for_cursor_toggle.clone(),
                        apply_cursor_enabled.clone(),
                        value,
                        codex_enabled,
                        claude_enabled,
                        tray_widgets_for_cursor_toggle.clone(),
                    )
                },
                "provider-cursor-enabled",
                hovered_card_id,
                set_hovered_card_id.clone(),
            ),
        ProviderKind::OpenCodeZen => settings_toggle_card_with_description(
                "Enabled",
                Some("Reads Zen authentication/models and local OpenCode usage history."),
                opencode_zen_enabled,
                move |value| {
                    persist_provider_enabled(
                        set_opencode_zen_enabled.clone(),
                        tray_widget_setter_for_opencode_toggle.clone(),
                        apply_opencode_zen_enabled.clone(),
                        ProviderKind::OpenCodeZen,
                        value,
                        opencode_go_enabled,
                        false,
                        tray_widgets_for_opencode_toggle.clone(),
                    )
                },
                "provider-opencode-zen-enabled",
                hovered_card_id,
                set_hovered_card_id.clone(),
            ),
        ProviderKind::OpenCodeGo => settings_toggle_card_with_description(
                "Enabled",
                Some("Reads account-wide Go quota windows and local OpenCode usage history."),
                opencode_go_enabled,
                move |value| {
                    persist_provider_enabled(
                        set_opencode_go_enabled.clone(),
                        tray_widget_setter_for_opencode_toggle.clone(),
                        apply_opencode_go_enabled.clone(),
                        ProviderKind::OpenCodeGo,
                        value,
                        opencode_zen_enabled,
                        false,
                        tray_widgets_for_opencode_toggle.clone(),
                    )
                },
                "provider-opencode-go-enabled",
                hovered_card_id,
                set_hovered_card_id.clone(),
            ),
        ProviderKind::OpenRouter => settings_toggle_card_with_description(
                "Enabled",
                Some("Reads API-key usage and spending limits; a management key also provides the account credit balance."),
                openrouter_enabled,
                move |value| {
                    persist_provider_enabled(
                        set_openrouter_enabled.clone(),
                        tray_widget_setter_for_opencode_toggle.clone(),
                        apply_openrouter_enabled.clone(),
                        ProviderKind::OpenRouter,
                        value,
                        opencode_zen_enabled,
                        opencode_go_enabled,
                        tray_widgets_for_opencode_toggle.clone(),
                    )
                },
                "provider-openrouter-enabled",
                hovered_card_id,
                set_hovered_card_id.clone(),
            ),
    };

    let install_status = match provider {
        ProviderKind::Codex => codex_install_status,
        ProviderKind::Claude => claude_install_status,
        ProviderKind::Cursor => cursor_install_status,
        ProviderKind::OpenCodeZen => opencode_zen_install_status,
        ProviderKind::OpenCodeGo => opencode_go_install_status,
        ProviderKind::OpenRouter => openrouter_install_status,
    };

    let (path, path_label, path_description, placeholder) = match provider {
        ProviderKind::Codex => (
            codex_path,
            "Codex CLI folder (optional)",
            "Choose the folder containing codex.exe, codex.cmd, or codex.ps1. Leave it empty for automatic scanning.",
            r"C:\\Users\\you\\AppData\\Roaming\\npm",
        ),
        ProviderKind::Claude => (
            claude_path,
            "Claude Code CLI folder (optional)",
            "Choose the folder containing claude.exe, claude.cmd, or claude.ps1. Leave it empty for automatic scanning.",
            r"C:\\Users\\you\\AppData\\Roaming\\npm",
        ),
        ProviderKind::Cursor => (
            cursor_path,
            "Cursor app folder (optional)",
            "Choose the folder containing Cursor.exe. Leave it empty for automatic scanning; usage still comes from Cursor's signed-in local profile.",
            r"C:\\Users\\you\\AppData\\Local\\Programs\\Cursor",
        ),
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo | ProviderKind::OpenRouter => {
            ("", "", "", "")
        }
    };

    let codex_path_setter = set_codex_path.clone();
    let claude_path_setter = set_claude_path.clone();
    let cursor_path_setter = set_cursor_path.clone();
    let codex_path_tx = apply_codex_path.clone();
    let claude_path_tx = apply_claude_path.clone();
    let cursor_path_tx = apply_cursor_path.clone();

    let path_input: Element = match provider {
        ProviderKind::Codex => {
            let picker_setter = set_codex_path.clone();
            let picker_tx = apply_codex_path.clone();
            grid((
                text_box(path)
                    .placeholder_text(placeholder)
                    .on_commit(move |value: String| {
                        codex_path_setter.call(value.clone());
                        persist_provider_folder(ProviderKind::Codex, value, codex_path_tx.clone());
                    })
                    .height(32.0)
                    .grid_column(0),
                Button::new("")
                    .icon_path(crate::icons::data("fluent-folder"), "#E6E6E6")
                    .width(44.0)
                    .height(32.0)
                    .on_click(move || match choose_provider_folder() {
                        Ok(Some(folder)) => {
                            let value = folder.display().to_string();
                            picker_setter.call(value.clone());
                            persist_provider_folder(ProviderKind::Codex, value, picker_tx.clone());
                        }
                        Ok(None) => {}
                        Err(error) => eprintln!("failed to choose Codex folder: {error:#}"),
                    })
                    .grid_column(1),
            ))
            .columns([GridLength::Star(1.0), GridLength::Auto])
            .column_spacing(8.0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .into()
        }
        ProviderKind::Claude => {
            let picker_setter = set_claude_path.clone();
            let picker_tx = apply_claude_path.clone();
            grid((
                text_box(path)
                    .placeholder_text(placeholder)
                    .on_commit(move |value: String| {
                        claude_path_setter.call(value.clone());
                        persist_provider_folder(ProviderKind::Claude, value, claude_path_tx.clone());
                    })
                    .height(32.0)
                    .grid_column(0),
                Button::new("")
                    .icon_path(crate::icons::data("fluent-folder"), "#E6E6E6")
                    .width(44.0)
                    .height(32.0)
                    .on_click(move || match choose_provider_folder() {
                        Ok(Some(folder)) => {
                            let value = folder.display().to_string();
                            picker_setter.call(value.clone());
                            persist_provider_folder(ProviderKind::Claude, value, picker_tx.clone());
                        }
                        Ok(None) => {}
                        Err(error) => eprintln!("failed to choose Claude folder: {error:#}"),
                    })
                    .grid_column(1),
            ))
            .columns([GridLength::Star(1.0), GridLength::Auto])
            .column_spacing(8.0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .into()
        }
        ProviderKind::Cursor => {
            let picker_setter = set_cursor_path.clone();
            let picker_tx = apply_cursor_path.clone();
            grid((
                text_box(path)
                    .placeholder_text(placeholder)
                    .on_commit(move |value: String| {
                        cursor_path_setter.call(value.clone());
                        persist_provider_folder(ProviderKind::Cursor, value, cursor_path_tx.clone());
                    })
                    .height(32.0)
                    .grid_column(0),
                Button::new("")
                    .icon_path(crate::icons::data("fluent-folder"), "#E6E6E6")
                    .width(44.0)
                    .height(32.0)
                    .on_click(move || match choose_provider_folder() {
                        Ok(Some(folder)) => {
                            let value = folder.display().to_string();
                            picker_setter.call(value.clone());
                            persist_provider_folder(ProviderKind::Cursor, value, picker_tx.clone());
                        }
                        Ok(None) => {}
                        Err(error) => eprintln!("failed to choose Cursor folder: {error:#}"),
                    })
                    .grid_column(1),
            ))
            .columns([GridLength::Star(1.0), GridLength::Auto])
            .column_spacing(8.0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .into()
        }
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo | ProviderKind::OpenRouter => {
            Element::Empty
        }
    };

    let details: Element = if matches!(provider, ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo) {
        let (key_input, set_key_input) = match provider {
            ProviderKind::OpenCodeZen => (opencode_zen_key_input, set_opencode_zen_key_input.clone()),
            ProviderKind::OpenCodeGo => (opencode_go_key_input, set_opencode_go_key_input.clone()),
            _ => unreachable!("OpenCode credentials branch"),
        };
        vstack((
            opencode_detection_card(provider),
            opencode_credentials_card(provider, key_input, set_key_input, settings_tx_for_details.clone()),
        ))
        .spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Top)
        .into()
    } else if provider == ProviderKind::OpenRouter {
        vstack((
            settings_info_card(
                "OpenRouter source",
                if crate::openrouter::is_installed_for_accounts(openrouter_accounts) {
                    "Protected account credentials"
                } else {
                    "No account credentials configured"
                },
            ),
            openrouter_accounts_card(
                openrouter_accounts,
                set_openrouter_accounts.clone(),
                openrouter_key_inputs,
                set_openrouter_key_inputs.clone(),
                openrouter_management_inputs,
                set_openrouter_management_inputs.clone(),
                settings_tx_for_details.clone(),
            ),
        ))
        .spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Top)
        .into()
    } else {
        vstack((
            provider_install_status_card(install_status),
            vstack((
                text_block(path_label).font_size(12.0),
                text_block(path_description)
                    .font_size(11.0)
                    .opacity(0.72)
                    .wrap(),
                path_input,
            ))
            .spacing(3.0)
            .horizontal_alignment(HorizontalAlignment::Stretch),
        ))
        .spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Top)
        .into()
    };

    let mut rows = vec![enable_card.with_key(format!("provider-{}-enabled", provider.id()))];
    rows.push(
        details.with_key(format!("provider-{}-details", provider.id())),
    );

    let row_count = rows.len();
    let cards = vstack(rows)
        .spacing(4.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .with_key(format!("provider-{}-cards-{row_count}", provider.id()));
    grid((
        text_block(provider.display_name())
            .font_size(28.0)
            .bold()
            .grid_row(0),
        cards.grid_row(1),
    ))
    .columns([GridLength::Star(1.0)])
    .rows([GridLength::Auto, GridLength::Auto])
    .row_spacing(10.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Top)
    .into()
}

fn tab_content(
    tab: Tab,
    theme: AppTheme,
    accent_color: AccentColor,
    animations_enabled: bool,
    time_format: TimeFormat,
    codex_enabled: bool,
    claude_enabled: bool,
    cursor_enabled: bool,
    opencode_zen_enabled: bool,
    opencode_go_enabled: bool,
    openrouter_enabled: bool,
    codex_path: &str,
    claude_path: &str,
    cursor_path: &str,
    codex_install_status: &ProviderInstallStatus,
    claude_install_status: &ProviderInstallStatus,
    cursor_install_status: &ProviderInstallStatus,
    opencode_zen_install_status: &ProviderInstallStatus,
    opencode_go_install_status: &ProviderInstallStatus,
    openrouter_install_status: &ProviderInstallStatus,
    opencode_zen_key_input: &str,
    opencode_go_key_input: &str,
    openrouter_accounts: &[OpenRouterAccount],
    openrouter_key_inputs: &HashMap<String, String>,
    openrouter_management_inputs: &HashMap<String, String>,
    popup_order: &[PopupWidgetKind],
    use_colored_provider_icons: bool,
    use_colored_sidebar_icons: bool,
    replace_chatgpt_logo_with_codex: bool,
    automatic_activation: bool,
    scheduled_activations: &[ScheduledActivation],
    limit_refresh_interval: LimitRefreshInterval,
    start_at_login: bool,
    show_used_percentage: bool,
    show_usage_pace: bool,
    popup_visibility: &PopupVisibility,
    discovered_popup_bricks: &BTreeMap<String, String>,
    show_total_spend_on_all_tab: bool,
    total_spend_presentation: TotalSpendPresentation,
    show_account_name: bool,
    activation_success: bool,
    activation_failure: bool,
    limits_reset: bool,
    low_usage_enabled: bool,
    low_usage_threshold: u8,
    low_usage_expanded: bool,
    low_usage_expand_progress: f64,
    weekly_low_usage_enabled: bool,
    weekly_low_usage_threshold: u8,
    weekly_low_usage_expanded: bool,
    weekly_low_usage_expand_progress: f64,
    tray_widgets: &[TrayWidget],
    expanded_tray_widget: &Option<String>,
    editing_tray_indicator: &Option<(String, usize)>,
    removed_tray_widget: &Option<(usize, TrayWidget)>,
    hovered_card_id: &Option<String>,
    expanded_popup_provider: &Option<String>,
    check_for_updates: bool,
    notify_on_update: bool,
    update_phase: &UpdatePhase,
    log_content: &str,
    set_codex_enabled: SetState<bool>,
    set_theme: SetState<AppTheme>,
    set_accent_color: SetState<AccentColor>,
    set_animations_enabled: SetState<bool>,
    set_time_format: SetState<TimeFormat>,
    set_claude_enabled: SetState<bool>,
    set_cursor_enabled: SetState<bool>,
    set_opencode_zen_enabled: SetState<bool>,
    set_opencode_go_enabled: SetState<bool>,
    set_openrouter_enabled: SetState<bool>,
    set_opencode_zen_key_input: SetState<String>,
    set_opencode_go_key_input: SetState<String>,
    set_openrouter_accounts: SetState<Vec<OpenRouterAccount>>,
    set_openrouter_key_inputs: SetState<HashMap<String, String>>,
    set_openrouter_management_inputs: SetState<HashMap<String, String>>,
    set_codex_path: SetState<String>,
    set_claude_path: SetState<String>,
    set_cursor_path: SetState<String>,
    set_popup_order: SetState<Vec<PopupWidgetKind>>,
    set_use_colored_provider_icons: SetState<bool>,
    set_use_colored_sidebar_icons: SetState<bool>,
    set_replace_chatgpt_logo_with_codex: SetState<bool>,
    set_automatic_activation: SetState<bool>,
    set_scheduled_activations: SetState<Vec<ScheduledActivation>>,
    set_limit_refresh_interval: SetState<LimitRefreshInterval>,
    set_start_at_login: SetState<bool>,
    set_show_used_percentage: SetState<bool>,
    set_show_usage_pace: SetState<bool>,
    set_popup_visibility: SetState<PopupVisibility>,
    set_discovered_popup_bricks: SetState<BTreeMap<String, String>>,
    set_show_total_spend_on_all_tab: SetState<bool>,
    set_total_spend_presentation: SetState<TotalSpendPresentation>,
    set_show_account_name: SetState<bool>,
    set_activation_success: SetState<bool>,
    set_activation_failure: SetState<bool>,
    set_limits_reset: SetState<bool>,
    set_low_usage_enabled: SetState<bool>,
    set_low_usage_threshold: SetState<u8>,
    set_low_usage_expanded: SetState<bool>,
    set_low_usage_expand_progress: AsyncSetState<f64>,
    set_weekly_low_usage_enabled: SetState<bool>,
    set_weekly_low_usage_threshold: SetState<u8>,
    set_weekly_low_usage_expanded: SetState<bool>,
    set_weekly_low_usage_expand_progress: AsyncSetState<f64>,
    set_tray_widgets: SetState<Vec<TrayWidget>>,
    set_expanded_tray_widget: SetState<Option<String>>,
    set_editing_tray_indicator: AsyncSetState<Option<(String, usize)>>,
    set_indicator_modal_visible: AsyncSetState<bool>,
    set_removed_tray_widget: SetState<Option<(usize, TrayWidget)>>,
    set_expanded_popup_provider: SetState<Option<String>>,
    set_hovered_card_id: SetState<Option<String>>,
    set_check_for_updates: SetState<bool>,
    set_notify_on_update: SetState<bool>,
    theme_navigation_guard: HookRef<bool>,
    theme_navigation_guard_timer: HookRef<Option<DispatcherTimer>>,
    settings_tx: Sender<Settings>,
    ui_dispatcher: UiMarshaller,
    updates: Arc<UpdateController>,
) -> Element {
    let apply_theme = settings_tx.clone();
    let apply_accent_color = settings_tx.clone();
    let apply_animations_enabled = settings_tx.clone();
    let apply_time_format = settings_tx.clone();
    let apply_use_colored_provider_icons = settings_tx.clone();
    let apply_use_colored_sidebar_icons = settings_tx.clone();
    let apply_replace_chatgpt_logo_with_codex = settings_tx.clone();
    let apply_automatic_activation = settings_tx.clone();
    let apply_limit_refresh_interval = settings_tx.clone();
    let apply_start_at_login = settings_tx.clone();
    let apply_show_used_percentage = settings_tx.clone();
    let apply_show_usage_pace = settings_tx.clone();
    let apply_show_account_name = settings_tx.clone();
    let apply_activation_success = settings_tx.clone();
    let apply_activation_failure = settings_tx.clone();
    let apply_limits_reset = settings_tx.clone();
    let apply_low_usage_enabled = settings_tx.clone();
    let apply_low_usage_threshold = settings_tx.clone();
    let apply_weekly_low_usage_enabled = settings_tx.clone();
    let apply_weekly_low_usage_threshold = settings_tx.clone();
    let apply_check_for_updates = settings_tx.clone();
    let apply_notify_on_update = settings_tx.clone();
    let apply_settings_import = settings_tx.clone();
    let apply_settings_reset = settings_tx.clone();
    let (title, rows) = match tab {
        Tab::General => (
            "General",
            vec![
                settings_toggle_card_with_description(
                    "Start with Windows",
                    Some("Opens Codex Minibar automatically after you sign in."),
                    start_at_login,
                    move |value| {
                        persist_bool(
                            set_start_at_login.clone(),
                            apply_start_at_login.clone(),
                            value,
                            |settings, value| {
                                settings.start_at_login = value;
                            },
                        );
                    },
                    "general-startup",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("general-startup"),
                settings_section_heading("Features").with_key("general-features-heading"),
                settings_toggle_card_with_description(
                    "Activate limits automatically",
                    Some("Sends a short low-effort prompt through each enabled provider when needed to begin its 5-hour usage window."),
                    automatic_activation,
                    move |value| {
                        persist_bool(
                            set_automatic_activation.clone(),
                            apply_automatic_activation.clone(),
                            value,
                            |settings, value| {
                                settings.automatic_activation = value;
                            },
                        );
                    },
                    "general-automatic-activation",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("general-automatic-activation"),
                settings_control_card(
                    "Refresh limits period",
                    Some("How often enabled providers fetch the current limits."),
                    ComboBox::new(["30 seconds", "1 minute", "5 minutes", "10 minutes", "15 minutes"])
                        .selected_index(limit_refresh_interval.index())
                        .on_selection_changed(move |choice: i32| {
                            let value = LimitRefreshInterval::from_index(choice);
                            set_limit_refresh_interval.call(value);
                            persist_update(apply_limit_refresh_interval.clone(), move |settings| {
                                settings.limit_refresh_interval = value;
                            });
                        }),
                    "general-limit-refresh-interval",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("general-limit-refresh-interval"),
            ],
        ),
        Tab::Appearance => {
            let appearance_rows = vec![
                settings_control_card(
                    "Color theme",
                    Some("Follow Windows or keep Codex Minibar light or dark."),
                    ComboBox::new(["Use Windows setting", "Light", "Dark"])
                        .selected_index(theme.index())
                        .on_selection_changed(move |choice| {
                            let value = AppTheme::from_index(choice);
                            theme_navigation_guard.set(true);
                            let guard = theme_navigation_guard.clone();
                            match DispatcherTimer::new_one_shot(
                                Duration::from_millis(350),
                                move || guard.set(false),
                            ) {
                                Ok(timer) => theme_navigation_guard_timer.set(Some(timer)),
                                Err(_) => theme_navigation_guard.set(false),
                            }
                            set_theme.call(value);
                            crate::theme::apply_appearance(value, accent_color);
                            persist_update(apply_theme.clone(), move |settings| {
                                settings.theme = value;
                            });
                        }),
                    "appearance-theme",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("appearance-theme"),
                settings_control_card(
                    "Accent color",
                    Some("Use the Windows accent or choose a color for highlighted controls."),
                    ComboBox::new([
                        "Windows default",
                        "Blue",
                        "Purple",
                        "Pink",
                        "Red",
                        "Orange",
                        "Green",
                        "Teal",
                    ])
                    .selected_index(accent_color.index())
                    .on_selection_changed(move |choice| {
                        let value = AccentColor::from_index(choice);
                        set_accent_color.call(value);
                        crate::theme::apply_appearance(theme, value);
                        persist_update(apply_accent_color.clone(), move |settings| {
                            settings.accent_color = value;
                        });
                    }),
                    "appearance-accent",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("appearance-accent"),
                settings_toggle_card_with_description(
                    "Colored sidebar icons",
                    Some("Use Fluent Color glyphs in the Settings sidebar. Turn off for monochrome theme icons."),
                    use_colored_sidebar_icons,
                    {
                        let set_use_colored_sidebar_icons =
                            set_use_colored_sidebar_icons.clone();
                        let apply_use_colored_sidebar_icons =
                            apply_use_colored_sidebar_icons.clone();
                        move |value| {
                            persist_bool(
                                set_use_colored_sidebar_icons.clone(),
                                apply_use_colored_sidebar_icons.clone(),
                                value,
                                |settings, value| {
                                    settings.use_colored_sidebar_icons = value;
                                },
                            );
                        }
                    },
                    "appearance-colored-sidebar-icons",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("appearance-colored-sidebar-icons"),
                settings_control_card(
                    "Time format",
                    Some("12-hour or 24-hour clocks in the popup and tray."),
                    ComboBox::new(["12-hour", "24-hour"])
                        .selected_index(time_format.index())
                        .on_selection_changed(move |choice| {
                            let value = TimeFormat::from_index(choice);
                            set_time_format.call(value);
                            value.apply();
                            persist_update(apply_time_format.clone(), move |settings| {
                                settings.time_format = value;
                            });
                        }),
                    "appearance-time-format",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("appearance-time-format"),
                settings_section_heading("Motion").with_key("appearance-motion-heading"),
                settings_toggle_card_with_description(
                    "Animation effects",
                    Some("Turn this off for the same reduced-motion behavior as disabling Animation effects in Windows."),
                    animations_enabled,
                    move |value| {
                        crate::theme::set_animations_enabled(value);
                        persist_bool(
                            set_animations_enabled.clone(),
                            apply_animations_enabled.clone(),
                            value,
                            |settings, value| settings.animations_enabled = value,
                        );
                    },
                    "appearance-animations",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("appearance-animations"),
            ];
            ("Appearance", appearance_rows)
        }
        Tab::Popup => {
            let providers: Vec<ProviderKind> = popup_order
                .iter()
                .filter_map(|widget| widget.as_provider())
                .collect();
            let enabled = enabled_providers(
                &providers,
                codex_enabled,
                claude_enabled,
                cursor_enabled,
                opencode_zen_enabled,
                opencode_go_enabled,
                openrouter_enabled,
            );
            let mut rows = vec![
                settings_section_heading("Tabs").with_key("customize-tabs-heading"),
                settings_toggle_card(
                    "Use colored provider icons",
                    use_colored_provider_icons,
                    {
                        let set_use_colored_provider_icons =
                            set_use_colored_provider_icons.clone();
                        let apply_use_colored_provider_icons =
                            apply_use_colored_provider_icons.clone();
                        move |value| {
                            persist_bool(
                                set_use_colored_provider_icons.clone(),
                                apply_use_colored_provider_icons.clone(),
                                value,
                                |settings, value| {
                                    settings.use_colored_provider_icons = value;
                                },
                            );
                        }
                    },
                    "customize-colored-icons",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("customize-colored-icons"),
            ];
            if codex_enabled {
                rows.push(
                    settings_toggle_card(
                        "Replace ChatGPT logo with Codex",
                        replace_chatgpt_logo_with_codex,
                        {
                            let set_replace_chatgpt_logo_with_codex =
                                set_replace_chatgpt_logo_with_codex.clone();
                            let apply_replace_chatgpt_logo_with_codex =
                                apply_replace_chatgpt_logo_with_codex.clone();
                            move |value| {
                                persist_bool(
                                    set_replace_chatgpt_logo_with_codex.clone(),
                                    apply_replace_chatgpt_logo_with_codex.clone(),
                                    value,
                                    |settings, value| {
                                        settings.replace_chatgpt_logo_with_codex = value;
                                    },
                                );
                            }
                        },
                        "customize-codex-logo",
                        hovered_card_id,
                        set_hovered_card_id.clone(),
                    )
                    .with_key("customize-codex-logo"),
                );
            }
            rows.extend([
                settings_section_heading("Cards").with_key("customize-cards-heading"),
                settings_toggle_card_with_description(
                    "Replace \"amount left\" with \"amount used\"",
                    Some("Shows consumed usage instead of the remaining amount."),
                    show_used_percentage,
                    {
                        let set_show_used_percentage = set_show_used_percentage.clone();
                        let apply_show_used_percentage = apply_show_used_percentage.clone();
                        move |value| {
                            persist_bool(
                                set_show_used_percentage.clone(),
                                apply_show_used_percentage.clone(),
                                value,
                                |settings, value| {
                                    settings.show_used_percentage = value;
                                },
                            );
                        }
                    },
                    "customize-show-used",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("customize-show-used"),
                settings_toggle_card_with_description(
                    "Show usage pace",
                    Some(
                        "Shows the expected-use marker and whether consumption is ahead of or behind schedule.",
                    ),
                    show_usage_pace,
                    {
                        let set_show_usage_pace = set_show_usage_pace.clone();
                        let apply_show_usage_pace = apply_show_usage_pace.clone();
                        move |value| {
                            persist_bool(
                                set_show_usage_pace.clone(),
                                apply_show_usage_pace.clone(),
                                value,
                                |settings, value| {
                                    settings.show_usage_pace = value;
                                },
                            );
                        }
                    },
                    "customize-show-usage-pace",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("customize-show-usage-pace"),
                settings_toggle_card_with_description(
                    "Show account name",
                    Some("Shows your Codex name or Claude organization beside the provider heading."),
                    show_account_name,
                    {
                        let set_show_account_name = set_show_account_name.clone();
                        let apply_show_account_name = apply_show_account_name.clone();
                        move |value| {
                            persist_bool(
                                set_show_account_name.clone(),
                                apply_show_account_name.clone(),
                                value,
                                |settings, value| {
                                    settings.show_account_name = value;
                                },
                            );
                        }
                    },
                    "customize-show-account-name",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("customize-show-account-name"),
            ]);
            rows.extend(popup_settings_cards(
                popup_visibility,
                discovered_popup_bricks,
                popup_order,
                &enabled,
                show_total_spend_on_all_tab,
                total_spend_presentation,
                expanded_popup_provider,
                set_expanded_popup_provider,
                set_popup_visibility,
                set_show_total_spend_on_all_tab,
                set_total_spend_presentation,
                hovered_card_id,
                set_hovered_card_id.clone(),
                settings_tx.clone(),
            ));
            ("Customize", rows)
        }
        Tab::Providers => unreachable!("Providers drill-in uses provider_page_content"),
        Tab::Schedule => (
            "Schedule",
            scheduled_activation_cards(
                scheduled_activations,
                &[
                    codex_enabled,
                    claude_enabled,
                    cursor_enabled,
                    opencode_zen_enabled,
                    opencode_go_enabled,
                    openrouter_enabled,
                ],
                set_scheduled_activations.clone(),
                settings_tx.clone(),
            ),
        ),
        Tab::Tray => {
            let providers: Vec<ProviderKind> = popup_order
                .iter()
                .filter_map(|widget| widget.as_provider())
                .collect();
            let enabled_providers =
                enabled_providers(
                    &providers,
                    codex_enabled,
                    claude_enabled,
                    cursor_enabled,
                    opencode_zen_enabled,
                    opencode_go_enabled,
                    openrouter_enabled,
                );
            (
                "Tray",
                tray_settings_cards(
                    tray_widgets,
                    &enabled_providers,
                    expanded_tray_widget,
                    editing_tray_indicator,
                    removed_tray_widget,
                    set_tray_widgets,
                    set_expanded_tray_widget,
                    set_editing_tray_indicator,
                    set_indicator_modal_visible,
                    set_removed_tray_widget,
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                    settings_tx.clone(),
                ),
            )
        }
        Tab::Notifications => (
            "Notifications",
            vec![
                settings_toggle_card(
                    "Activation successes",
                    activation_success,
                    move |value| {
                        persist_bool(
                            set_activation_success.clone(),
                            apply_activation_success.clone(),
                            value,
                            |settings, value| {
                                settings.notifications.activation_success = value;
                            },
                        );
                    },
                    "notif-activation-success",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("notif-activation-success"),
                settings_toggle_card(
                    "Activation failures",
                    activation_failure,
                    move |value| {
                        persist_bool(
                            set_activation_failure.clone(),
                            apply_activation_failure.clone(),
                            value,
                            |settings, value| {
                                settings.notifications.activation_failure = value;
                            },
                        );
                    },
                    "notif-activation-failure",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("notif-activation-failure"),
                settings_toggle_card(
                    "When limits got reset",
                    limits_reset,
                    move |value| {
                        persist_bool(
                            set_limits_reset.clone(),
                            apply_limits_reset.clone(),
                            value,
                            |settings, value| {
                                settings.notifications.limits_changed = value;
                            },
                        );
                    },
                    "notif-limits-reset",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("notif-limits-reset"),
                settings_toggle_expander(
                    format!("When session usage is down to {low_usage_threshold}%"),
                    None,
                    low_usage_enabled,
                    move |value| {
                        persist_bool(
                            set_low_usage_enabled.clone(),
                            apply_low_usage_enabled.clone(),
                            value,
                            |settings, value| {
                                settings.notifications.low_usage_enabled = value;
                            },
                        );
                    },
                    low_usage_expanded,
                    low_usage_expand_progress,
                    Some(78.0),
                    set_low_usage_expanded,
                    set_low_usage_expand_progress,
                    "notif-low-usage",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                    settings_slider_content(
                        "Notify when remaining session usage reaches",
                        low_usage_threshold,
                        5,
                        50,
                        5,
                        move |value: f64| {
                            let percent = value.round().clamp(5.0, 50.0) as u8;
                            persist_u8(
                                set_low_usage_threshold.clone(),
                                apply_low_usage_threshold.clone(),
                                percent,
                                |settings, value| {
                                    settings.notifications.low_usage_threshold_percent = value;
                                },
                            );
                        },
                    ),
                )
                .with_key("notif-low-usage"),
                settings_toggle_expander(
                    format!(
                        "When weekly usage is down to {weekly_low_usage_threshold}%"
                    ),
                    None,
                    weekly_low_usage_enabled,
                    move |value| {
                        persist_bool(
                            set_weekly_low_usage_enabled.clone(),
                            apply_weekly_low_usage_enabled.clone(),
                            value,
                            |settings, value| {
                                settings.notifications.weekly_low_usage_enabled = value;
                            },
                        );
                    },
                    weekly_low_usage_expanded,
                    weekly_low_usage_expand_progress,
                    Some(78.0),
                    set_weekly_low_usage_expanded,
                    set_weekly_low_usage_expand_progress,
                    "notif-weekly-low-usage",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                    settings_slider_content(
                        "Notify when remaining weekly usage reaches",
                        weekly_low_usage_threshold,
                        5,
                        50,
                        5,
                        move |value: f64| {
                            let percent = value.round().clamp(5.0, 50.0) as u8;
                            persist_u8(
                                set_weekly_low_usage_threshold.clone(),
                                apply_weekly_low_usage_threshold.clone(),
                                percent,
                                |settings, value| {
                                    settings.notifications.weekly_low_usage_threshold_percent =
                                        value;
                                },
                            );
                        },
                    ),
                )
                .with_key("notif-weekly-low-usage"),
            ],
        ),
        Tab::Advanced => {
            let import_state = SettingsWindowState {
                theme: set_theme,
                accent_color: set_accent_color,
                animations_enabled: set_animations_enabled,
                time_format: set_time_format,
                codex_enabled: set_codex_enabled,
                claude_enabled: set_claude_enabled,
                cursor_enabled: set_cursor_enabled,
                opencode_zen_enabled: set_opencode_zen_enabled,
                opencode_go_enabled: set_opencode_go_enabled,
                openrouter_enabled: set_openrouter_enabled,
                openrouter_accounts: set_openrouter_accounts,
                codex_path: set_codex_path,
                claude_path: set_claude_path,
                cursor_path: set_cursor_path,
                popup_order: set_popup_order,
                use_colored_provider_icons: set_use_colored_provider_icons,
                use_colored_sidebar_icons: set_use_colored_sidebar_icons,
                replace_chatgpt_logo_with_codex: set_replace_chatgpt_logo_with_codex,
                automatic_activation: set_automatic_activation,
                scheduled_activations: set_scheduled_activations.clone(),
                limit_refresh_interval: set_limit_refresh_interval,
                start_at_login: set_start_at_login,
                show_used_percentage: set_show_used_percentage,
                show_usage_pace: set_show_usage_pace,
                popup_visibility: set_popup_visibility,
                discovered_popup_bricks: set_discovered_popup_bricks,
                show_total_spend_on_all_tab: set_show_total_spend_on_all_tab,
                total_spend_presentation: set_total_spend_presentation,
                show_account_name: set_show_account_name,
                activation_success: set_activation_success,
                activation_failure: set_activation_failure,
                limits_reset: set_limits_reset,
                low_usage_enabled: set_low_usage_enabled,
                low_usage_threshold: set_low_usage_threshold,
                weekly_low_usage_enabled: set_weekly_low_usage_enabled,
                weekly_low_usage_threshold: set_weekly_low_usage_threshold,
                tray_widgets: set_tray_widgets,
                check_for_updates: set_check_for_updates,
                notify_on_update: set_notify_on_update,
            };
            let reset_state = import_state.clone();
            let reset_dispatcher = ui_dispatcher.clone();
            (
            "Advanced",
            vec![
                settings_action_card(
                    "Export settings",
                    "Export",
                    || {
                        if let Err(error) = export_settings() {
                            eprintln!("failed to export settings: {error:#}");
                            notifications::show("Settings export failed", &format!("{error:#}"));
                        }
                    },
                    "advanced-export",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("advanced-export"),
                settings_action_card(
                    "Import settings",
                    "Import",
                    move || {
                        let result = import_settings().and_then(|settings| match settings {
                            Some(settings) => {
                                replace_settings(apply_settings_import.clone(), settings.clone())?;
                                import_state.apply(&settings);
                                Ok(())
                            }
                            None => Ok(()),
                        });
                        if let Err(error) = result {
                            eprintln!("failed to import settings: {error:#}");
                            notifications::show("Settings import failed", &format!("{error:#}"));
                        }
                    },
                    "advanced-import",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("advanced-import"),
                settings_action_card(
                    "Reset all settings",
                    "Reset",
                    move || {
                        if !confirm_settings_reset() {
                            return;
                        }
                        let settings = Settings::default();
                        if let Err(error) = replace_settings(apply_settings_reset.clone(), settings.clone()) {
                            eprintln!("failed to reset settings: {error:#}");
                            notifications::show("Settings reset failed", &format!("{error:#}"));
                        } else {
                            reset_state.apply(&settings);
                            restart_onboarding_after_reset(
                                apply_settings_reset.clone(),
                                reset_dispatcher.clone(),
                            );
                        }
                    },
                    "advanced-reset",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("advanced-reset"),
            ],
        )
        }
        Tab::Log => (
            "Log",
            vec![
                settings_action_card(
                    "Application log",
                    "Open log.txt",
                    || {
                        if let Err(error) = crate::logger::open() {
                            eprintln!("failed to open log.txt: {error:#}");
                            notifications::show("Could not open log.txt", &error.to_string());
                        }
                    },
                    "log-open-file",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("log-open-file"),
                settings_action_card(
                    "Current and archived application logs",
                    "Open logs folder",
                    || {
                        if let Err(error) = crate::logger::open_folder() {
                            eprintln!("failed to open logs folder: {error:#}");
                            notifications::show(
                                "Could not open logs folder",
                                &error.to_string(),
                            );
                        }
                    },
                    "log-open-folder",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("log-open-folder"),
                log_view_card(log_content).with_key("log-live-tail"),
            ],
        ),
        Tab::About => (
            "About & Updates",
            about_settings_cards(
                check_for_updates,
                notify_on_update,
                update_phase,
                set_check_for_updates,
                set_notify_on_update,
                apply_check_for_updates,
                apply_notify_on_update,
                hovered_card_id,
                set_hovered_card_id.clone(),
                settings_tx.clone(),
                updates,
            ),
        ),
    };
    let row_count = rows.len();
    let cards = vstack(rows)
        .spacing(if tab == Tab::Tray { 12.0 } else { 4.0 })
        .grid_row(1)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .with_key(format!("{}-cards-{row_count}", tab.tag()));

    let heading: Element = if tab == Tab::About {
        Element::Empty
    } else if tab == Tab::Schedule {
        let provider = if codex_enabled {
            Some(ProviderKind::Codex)
        } else if claude_enabled {
            Some(ProviderKind::Claude)
        } else {
            None
        };
        if let Some(provider) = provider {
            let existing = scheduled_activations.to_vec();
            let setter = set_scheduled_activations.clone();
            let tx = settings_tx.clone();
            grid((
                text_block(title)
                    .font_size(28.0)
                    .bold()
                    .grid_column(0)
                    .vertical_alignment(VerticalAlignment::Center),
                Button::new("Add activation")
                    .accent()
                    .on_click(move || {
                        let mut next = existing.clone();
                        next.push(ScheduledActivation::new(provider));
                        persist_schedules(setter.clone(), tx.clone(), next);
                    })
                    .grid_column(1)
                    .vertical_alignment(VerticalAlignment::Center),
            ))
            .columns([GridLength::Star(1.0), GridLength::Auto])
            .rows([GridLength::Auto])
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .grid_row(0)
            .into()
        } else {
            text_block(title).font_size(28.0).bold().grid_row(0).into()
        }
    } else {
        text_block(title).font_size(28.0).bold().grid_row(0).into()
    };

    grid((heading, cards))
        .columns([GridLength::Star(1.0)])
        .rows([GridLength::Auto, GridLength::Auto])
        .row_spacing(if tab == Tab::About { 0.0 } else { 10.0 })
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Top)
        .into()
}

fn scheduled_activation_cards(
    schedules: &[ScheduledActivation],
    provider_enabled: &[bool; 6],
    set_schedules: SetState<Vec<ScheduledActivation>>,
    settings_tx: Sender<Settings>,
) -> Vec<Element> {
    // A schedule can only target a currently enabled provider. Keeping the
    // available choices in the card also avoids separate, provider-specific
    // add buttons that made the narrow settings content overflow.
    let available_providers: Vec<ProviderKind> = ProviderKind::ALL
        .into_iter()
        .filter(|provider| crate::provider_registry::descriptor(*provider).supports_activation)
        .filter(|provider| match provider {
            ProviderKind::Codex => provider_enabled[0],
            ProviderKind::Claude => provider_enabled[1],
            ProviderKind::Cursor => false,
            ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => false,
            ProviderKind::OpenRouter => false,
        })
        .collect();
    let provider_labels: Vec<String> = available_providers
        .iter()
        .map(|provider| provider.display_name().to_string())
        .collect();
    const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let mut rows: Vec<Element> = vec![
        text_block("Start a provider's 5-hour limit window at a chosen local time. Automatic activation is paused for the six hours before each scheduled run.")
            .foreground(ThemeRef::SecondaryText)
            .wrap()
            .margin(Thickness {
                left: 0.0,
                top: 0.0,
                right: 0.0,
                bottom: 8.0,
            })
            .with_key("schedule-description")
            .into(),
    ];
    for (index, schedule) in schedules.iter().enumerate() {
        let schedule_id = schedule.id.clone();
        let schedule_id_for_enabled = schedule_id.clone();
        let schedules_for_enabled = schedules.to_vec();
        let enabled_setter = set_schedules.clone();
        let enabled_tx = settings_tx.clone();
        let schedules_for_provider = schedules.to_vec();
        let provider_setter = set_schedules.clone();
        let provider_tx = settings_tx.clone();
        let schedules_for_time = schedules.to_vec();
        let time_setter = set_schedules.clone();
        let time_tx = settings_tx.clone();
        let schedules_for_remove = schedules.to_vec();
        let remove_setter = set_schedules.clone();
        let remove_tx = settings_tx.clone();
        let selected_provider = schedule
            .provider()
            .and_then(|provider| {
                available_providers
                    .iter()
                    .position(|candidate| *candidate == provider)
            })
            .unwrap_or(0) as i32;
        let provider_choices = available_providers.clone();
        let weekday_buttons: Vec<Element> = WEEKDAYS
            .iter()
            .enumerate()
            .map(|(weekday, label)| {
                let schedules = schedules.to_vec();
                let setter = set_schedules.clone();
                let tx = settings_tx.clone();
                let schedule_id = schedule.id.clone();
                ToggleButton::new(*label, schedule.occurs_on(weekday as u8))
                    .on_checked(move |checked| {
                        let mut next = schedules.clone();
                        let Some(rule) = next.iter_mut().find(|rule| rule.id == schedule_id) else {
                            return;
                        };
                        let day = weekday as u8;
                        if checked {
                            if rule.weekdays.contains(&day) {
                                return;
                            }
                            rule.weekdays.push(day);
                            rule.weekdays.sort_unstable();
                        } else {
                            // A rule with no selected days could never fire. Keep
                            // its last day selected rather than saving a dead rule.
                            if rule.weekdays.len() == 1 && rule.weekdays[0] == day {
                                return;
                            }
                            rule.weekdays.retain(|candidate| *candidate != day);
                        }
                        rule.weekday = *rule.weekdays.first().unwrap_or(&0);
                        persist_schedules(setter.clone(), tx.clone(), next);
                    })
                    .grid_column(weekday as i32)
                    .min_width(0.0)
                    .horizontal_alignment(HorizontalAlignment::Stretch)
                    .with_key(format!("schedule-{}-weekday-{weekday}", schedule.id))
                    .into()
            })
            .collect();
        let weekday_selector = grid(weekday_buttons)
            .columns(vec![GridLength::Star(1.0); 7])
            .rows([GridLength::Auto])
            .column_spacing(4.0)
            .margin(Thickness {
                left: 0.0,
                top: 10.0,
                right: 0.0,
                bottom: 0.0,
            })
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .grid_row(1)
            .grid_column_span(3);
        // Match the proven settings-card header layout: RelativePanel pins the
        // switch to the card edge, and the explicit 50px width removes WinUI's
        // invisible content slot from the switch template.
        let header_children: Vec<Element> = vec![
            text_block("Activate limit")
                .font_size(14.0)
                .margin(Thickness {
                    left: SETTINGS_CARD_PADDING,
                    top: SETTINGS_CARD_PADDING,
                    right: 82.0,
                    bottom: SETTINGS_CARD_PADDING,
                })
                .relative_align_left()
                .relative_align_v_center()
                .into(),
            ToggleSwitch::new(schedule.enabled)
                .on_content("")
                .off_content("")
                .on_toggled(move |enabled| {
                    let mut next = schedules_for_enabled.clone();
                    if let Some(rule) = next
                        .iter_mut()
                        .find(|rule| rule.id == schedule_id_for_enabled)
                    {
                        if rule.enabled == enabled {
                            return;
                        }
                        rule.enabled = enabled;
                    } else {
                        return;
                    }
                    persist_schedules(enabled_setter.clone(), enabled_tx.clone(), next);
                })
                .min_width(0.0)
                .max_width(50.0)
                .width(50.0)
                .margin(Thickness {
                    left: 0.0,
                    top: 0.0,
                    // Compensate for the WinUI ToggleSwitch template's trailing
                    // slot so the visible track, not merely its layout box,
                    // shares the delete button's right edge.
                    right: 7.0,
                    bottom: 0.0,
                })
                .relative_align_right()
                .relative_align_v_center()
                .into(),
        ];
        let header = relative_panel(header_children)
            .min_height(60.0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .background(Color::transparent());
        let action_row = grid((
            ComboBox::new(provider_labels.clone())
                .selected_index(selected_provider)
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .on_selection_changed(move |value: i32| {
                    let Some(provider) = provider_choices.get(value.max(0) as usize).copied()
                    else {
                        return;
                    };
                    let mut next = schedules_for_provider.clone();
                    if let Some(rule) = next.get_mut(index) {
                        if rule.provider_id == provider.id() {
                            return;
                        }
                        rule.provider_id = provider.id().into();
                    } else {
                        return;
                    }
                    persist_schedules(provider_setter.clone(), provider_tx.clone(), next);
                })
                .grid_column(0),
            TimePicker::new()
                .clock_identifier("24HourClock")
                .minute_increment(5)
                .time_minutes(schedule.time_minutes)
                .height(40.0)
                .min_height(40.0)
                .max_height(40.0)
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .on_selected_time_changed(move |time: TimeSpan| {
                    let mut next = schedules_for_time.clone();
                    if let Some(rule) = next.get_mut(index) {
                        let time_minutes = (time.duration / (60 * 10_000_000))
                            .clamp(0, i64::from(23 * 60 + 59))
                            as u16;
                        if rule.time_minutes == time_minutes {
                            return;
                        }
                        rule.time_minutes = time_minutes;
                    } else {
                        return;
                    }
                    persist_schedules(time_setter.clone(), time_tx.clone(), next);
                })
                .grid_column(1),
            Button::new("\u{E74D}")
                .font_family("Segoe Fluent Icons")
                .font_size(14.0)
                .width(32.0)
                .height(32.0)
                .min_width(32.0)
                .min_height(32.0)
                .padding(Thickness::uniform(0.0))
                .tooltip("Remove activation")
                .on_click(move || {
                    let next = schedules_for_remove
                        .iter()
                        .filter(|rule| rule.id != schedule_id)
                        .cloned()
                        .collect();
                    persist_schedules(remove_setter.clone(), remove_tx.clone(), next);
                })
                .grid_column(2),
            weekday_selector,
        ))
        .columns([
            GridLength::Star(1.0),
            GridLength::Star(2.0),
            GridLength::Auto,
        ])
        .rows([GridLength::Auto, GridLength::Auto])
        .column_spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch);
        let body = border(action_row)
            .padding(Thickness {
                left: SETTINGS_CARD_PADDING,
                top: 0.0,
                right: SETTINGS_CARD_PADDING,
                bottom: SETTINGS_CARD_PADDING,
            })
            .horizontal_alignment(HorizontalAlignment::Stretch);
        let card_content: Element = vstack((header, body))
            .spacing(0.0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .into();
        let shell_children: Vec<Element> = vec![
            border(Element::Empty)
                .background(ThemeRef::CardBackground)
                .corner_radius(8.0)
                .border_thickness(Thickness::uniform(1.0))
                .border_brush(ThemeRef::CardStroke)
                .relative_align_left()
                .relative_align_right()
                .relative_align_top()
                .relative_align_bottom()
                .into(),
            card_content,
        ];
        rows.push(
            relative_panel(shell_children)
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .with_translation_transition(duration(CONTROL_NORMAL_ANIMATION))
                .with_opacity_transition(duration(CONTROL_NORMAL_ANIMATION))
                .with_key(format!("schedule-rule-{}", schedule.id))
                .into(),
        );
    }

    if available_providers.is_empty() {
        rows.push(
            text_block("Enable a provider in Providers to add a limit activation schedule.")
                .foreground(ThemeRef::SecondaryText)
                .wrap()
                .into(),
        );
    }
    rows
}

fn persist_schedules(
    setter: SetState<Vec<ScheduledActivation>>,
    settings_tx: Sender<Settings>,
    schedules: Vec<ScheduledActivation>,
) {
    setter.call(schedules.clone());
    persist_update(settings_tx, move |settings| {
        settings.scheduled_activations = schedules
    });
}

fn log_view_card(log_content: &str) -> Element {
    border(
        scroll_viewer(
            border(
                text_block(if log_content.is_empty() {
                    "No log events yet."
                } else {
                    log_content
                })
                .font_size(12.0)
                .wrap(),
            )
            .padding(settings_card_padding()),
        )
        .horizontal_scroll_bar_visibility(ScrollBarVisibility::Disabled)
        .vertical_scroll_bar_visibility(ScrollBarVisibility::Auto)
        .height(340.0),
    )
    .background(ThemeRef::CardBackground)
    .corner_radius(8.0)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn settings_section_heading(title: impl Into<String>) -> Element {
    text_block(title)
        .font_size(16.0)
        .semibold()
        .margin(Thickness {
            left: 0.0,
            top: 16.0,
            right: 0.0,
            bottom: 4.0,
        })
        .into()
}

fn update_status_label(phase: &UpdatePhase) -> String {
    match phase {
        UpdatePhase::Idle => "Look for the latest release on GitHub".into(),
        UpdatePhase::Checking => "Checking updates".into(),
        UpdatePhase::UpToDate => "No updates found".into(),
        UpdatePhase::Available(update) => format!("Update {} available", update.version),
        UpdatePhase::Applying => "Installing update...".into(),
        // Never surface raw transport errors (e.g. "GET https://...").
        UpdatePhase::Failed(_) => "Couldn't check for updates".into(),
    }
}

fn about_settings_cards(
    check_for_updates: bool,
    notify_on_update: bool,
    update_phase: &UpdatePhase,
    set_check_for_updates: SetState<bool>,
    set_notify_on_update: SetState<bool>,
    apply_check_for_updates: Sender<Settings>,
    apply_notify_on_update: Sender<Settings>,
    hovered_card_id: &Option<String>,
    set_hovered_card_id: SetState<Option<String>>,
    settings_tx: Sender<Settings>,
    updates: Arc<UpdateController>,
) -> Vec<Element> {
    let version = current_version().to_string();
    let updates_for_check = updates.clone();
    let notify_for_check = notify_on_update;

    let hero = border(
        vstack((
            Image::new_with_uri(settings_about_icon_uri())
                .width(112.0)
                .height(112.0)
                .horizontal_alignment(HorizontalAlignment::Center)
                .margin(Thickness {
                    left: 0.0,
                    top: 0.0,
                    right: 0.0,
                    bottom: 10.0,
                }),
            vstack((
                text_block("Codex Minibar")
                    .font_size(26.0)
                    .bold()
                    .horizontal_alignment(HorizontalAlignment::Center),
                text_block(format!("Version {version}"))
                    .font_size(13.0)
                    .foreground(ThemeRef::SecondaryText)
                    .horizontal_alignment(HorizontalAlignment::Center),
            ))
            .spacing(2.0)
            .horizontal_alignment(HorizontalAlignment::Center),
            text_block("A lightweight Windows tray companion for Codex rate limits.")
                .font_size(15.0)
                .wrap()
                .foreground(ThemeRef::SecondaryText)
                .horizontal_alignment(HorizontalAlignment::Center)
                .margin(Thickness {
                    left: 0.0,
                    top: 10.0,
                    right: 0.0,
                    bottom: 0.0,
                }),
        ))
        .spacing(0.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(Thickness {
        left: 0.0,
        top: 8.0,
        right: 0.0,
        bottom: 22.0,
    })
    .background(Color::transparent())
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .with_key("about-hero")
    .into();

    let update_options = vstack((
        settings_toggle_card(
            "Check for updates on startup",
            check_for_updates,
            move |value| {
                persist_bool(
                    set_check_for_updates.clone(),
                    apply_check_for_updates.clone(),
                    value,
                    |settings, value| {
                        settings.check_for_updates = value;
                    },
                );
            },
            "about-check-updates",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("about-check-updates"),
        settings_toggle_card(
            "Notify when a new version is found",
            notify_on_update,
            move |value| {
                persist_bool(
                    set_notify_on_update.clone(),
                    apply_notify_on_update.clone(),
                    value,
                    |settings, value| {
                        settings.notifications.update_available = value;
                    },
                );
            },
            "about-notify-updates",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("about-notify-updates"),
    ))
    .spacing(4.0);

    let update_settings_separator = border(Element::Empty)
        .height(1.0)
        .background(ThemeRef::DividerStroke)
        .margin(Thickness {
            left: 0.0,
            top: 4.0,
            right: 0.0,
            bottom: 4.0,
        })
        .horizontal_alignment(HorizontalAlignment::Stretch);

    let update_actions: Element = if matches!(update_phase, UpdatePhase::Available(_)) {
        vstack((
            settings_action_card(
                "Download and install the latest release",
                "Update",
                || {
                    if let Err(error) = crate::updater::apply_pending_update() {
                        eprintln!("failed to apply update: {error:#}");
                        notifications::show("Update failed", &format!("{error:#}"));
                    }
                },
                "about-update-apply",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("about-update-apply"),
            settings_action_card(
                "Read the release notes on GitHub",
                "What's New",
                || {
                    if let Err(error) = crate::updater::open_release_notes() {
                        eprintln!("failed to open release notes: {error:#}");
                    }
                },
                "about-whats-new",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("about-whats-new"),
        ))
        .spacing(4.0)
        .into()
    } else {
        settings_action_card(
            update_status_label(update_phase),
            "Check for updates",
            move || {
                updates_for_check.check_async(false, notify_for_check);
            },
            "about-check-now",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("about-check-now")
    };

    let updates_card = about_section(
        "Updates",
        vstack((update_actions, update_settings_separator, update_options)).spacing(4.0),
    )
    .with_key("about-updates");

    let resources = about_section(
        "Resources",
        grid((
            about_action_card(
                "GitHub",
                "Browse the source code",
                AboutCardIcon::Phosphor("github-logo"),
                || {
                    let _ = crate::updater::open_url(REPO_URL);
                },
                "about-github",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .grid_row(0)
            .grid_column(0),
            about_action_card(
                "Releases",
                "See what's new",
                AboutCardIcon::Phosphor("download-simple"),
                || {
                    let _ = crate::updater::open_url(RELEASES_URL);
                },
                "about-releases",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .grid_row(0)
            .grid_column(1),
            about_action_card(
                "Report an issue",
                "Found a bug?",
                AboutCardIcon::Phosphor("flag"),
                || {
                    let _ = crate::updater::open_url(ISSUES_URL);
                },
                "about-issues",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .grid_row(1)
            .grid_column(0),
            about_action_card(
                "Author",
                "@vertopolkaLF",
                AboutCardIcon::Phosphor("at"),
                || {
                    let _ = crate::updater::open_url("https://github.com/vertopolkaLF");
                },
                "about-author",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .grid_row(1)
            .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
        .rows([GridLength::Auto, GridLength::Auto])
        .column_spacing(12.0)
        .row_spacing(12.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .with_key("about-resources");

    let cards = vec![hero, updates_card.into(), resources.into()];

    let _ = settings_tx;
    cards
}

fn about_section(title: impl Into<String>, content: impl Into<Element>) -> Element {
    about_section_with_header(text_block(title).font_size(18.0).bold(), content)
}

fn about_section_with_header(header: impl Into<Element>, content: impl Into<Element>) -> Element {
    border(
        vstack((header.into(), content.into()))
            .spacing(14.0)
            .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(settings_card_padding())
    .background(ThemeRef::CardBackground)
    .corner_radius(14.0)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

/// Full-surface action card used by the About page.  The panel, not a nested
/// button, owns the click target so it feels like one intentional control.
#[derive(Clone, Copy)]
enum AboutCardIcon {
    Phosphor(&'static str),
}

fn about_action_card(
    title: impl Into<String>,
    description: impl Into<String>,
    icon: AboutCardIcon,
    on_click: impl IntoUnitCallback,
    card_id: &'static str,
    hovered_id: &Option<String>,
    set_hovered_id: SetState<Option<String>>,
) -> Element {
    let hovered = hovered_id.as_deref() == Some(card_id);
    let on_click = on_click.into_unit_callback();
    let on_enter = {
        let set_hovered_id = set_hovered_id.clone();
        move |_: PointerEventInfo| set_hovered_id.call(Some(card_id.to_string()))
    };
    let on_exit = move || set_hovered_id.call(None);

    let base: Element = border(Element::Empty)
        .background(ThemeRef::AccentTertiary)
        // Accent resources can be fully opaque on some Windows palettes.
        // Keep only a gentle tint, comparable to the previous card fill.
        .opacity(0.18)
        .corner_radius(10.0)
        .relative_align_left()
        .relative_align_right()
        .relative_align_top()
        .relative_align_bottom()
        .into();
    let hover: Element = border(Element::Empty)
        .background(ThemeRef::AccentSecondary)
        .opacity(if hovered { 0.28 } else { 0.0 })
        .with_opacity_transition(duration(CONTROL_FAST_ANIMATION))
        .corner_radius(10.0)
        .relative_align_left()
        .relative_align_right()
        .relative_align_top()
        .relative_align_bottom()
        .into();
    let AboutCardIcon::Phosphor(name) = icon;
    let icon: Element = crate::icons::element(name, 16.0, Color::rgb(226, 151, 78))
        .vertical_alignment(VerticalAlignment::Center)
        .into();
    let heading = grid((
        icon.grid_column(0),
        text_block(title)
            .font_size(15.0)
            .semibold()
            .grid_column(1)
            .vertical_alignment(VerticalAlignment::Center),
    ))
    .columns([GridLength::Pixel(16.0), GridLength::Star(1.0)])
    .column_spacing(8.0)
    .rows([GridLength::Auto]);

    relative_panel(vec![
        base,
        hover,
        vstack((
            heading,
            text_block(description)
                .font_size(13.0)
                .foreground(ThemeRef::SecondaryText),
        ))
        .spacing(5.0)
        .margin(Thickness {
            left: SETTINGS_CARD_PADDING,
            top: SETTINGS_CARD_PADDING,
            right: SETTINGS_CARD_PADDING,
            bottom: SETTINGS_CARD_PADDING,
        })
        .relative_align_left()
        .relative_align_top()
        .into(),
    ])
    .min_height(82.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .background(Color::transparent())
    .on_pointer_entered(on_enter)
    .on_pointer_exited(on_exit)
    .on_tapped(move || on_click.invoke(()))
    .with_key(card_id)
    .into()
}

fn tray_color_mode_label(mode: TrayColorMode) -> &'static str {
    match mode {
        TrayColorMode::Status => "Status color",
        TrayColorMode::Fixed => "Fixed color",
        TrayColorMode::Provider => "Provider color",
        TrayColorMode::Accent => "App accent",
        TrayColorMode::Monochrome => "Monochrome",
    }
}

fn tray_indicator_summary(indicator: &TrayIndicator) -> String {
    let Some(provider) = indicator.provider() else {
        return format!("Unsupported {}", indicator.provider_id);
    };
    let metric = crate::provider_registry::metric(provider, &indicator.metric_id)
        .map(|metric| metric.label.to_owned())
        .unwrap_or_else(|| indicator.metric_id.clone());
    let value = match indicator.limit_value {
        LimitValue::Used => "Used",
        LimitValue::Remaining => "Remaining",
    };
    format!(
        "{} · {metric} · {value} · {}",
        provider.display_name(),
        tray_color_mode_label(indicator.color_mode)
    )
}

fn tray_widget_summary(widget: &TrayWidget) -> String {
    if widget.kind == TrayWidgetKind::AppIcon {
        return "App icon".into();
    }
    let labels = widget
        .indicators
        .iter()
        .map(tray_indicator_summary)
        .collect::<Vec<_>>();
    if labels.is_empty() {
        "Empty widget".into()
    } else {
        labels.join(" · ")
    }
}

fn tray_preview_limits() -> &'static crate::limits::ProviderLimits {
    static LIMITS: std::sync::OnceLock<crate::limits::ProviderLimits> = std::sync::OnceLock::new();
    LIMITS.get_or_init(|| {
        let window = |used_percent| crate::limits::LimitWindow {
            used_percent: Some(used_percent),
            resets_at: None,
            duration_minutes: Some(300),
        };
        crate::limits::ProviderLimits::from_entries([
            (
                ProviderKind::Codex,
                crate::limits::RateLimits {
                    primary: window(38),
                    secondary: window(70),
                    ..Default::default()
                },
            ),
            (
                ProviderKind::Claude,
                crate::limits::RateLimits {
                    primary: window(55),
                    secondary: window(12),
                    ..Default::default()
                },
            ),
            (
                ProviderKind::Cursor,
                crate::limits::RateLimits {
                    secondary: window(18),
                    additional_limits: vec![crate::limits::AdditionalLimit {
                        id: "cursor-api".into(),
                        title: "API".into(),
                        window: window(47),
                    }],
                    ..Default::default()
                },
            ),
        ])
    })
}

fn tray_widget_preview(widget: &TrayWidget) -> Element {
    let preview_id = widget.id.clone();
    let accent = crate::theme::current_accent_rgb();
    let uses_light_theme = crate::tray::system_uses_light_theme();
    let time_format = TimeFormat::current();
    let minute_bucket = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() / 60);
    let (pixels, pixels_changed) = TRAY_PREVIEW_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(entry) = cache.get(&preview_id)
            && entry.widget == *widget
            && entry.accent == accent
            && entry.uses_light_theme == uses_light_theme
            && entry.time_format == time_format
            && entry.minute_bucket == minute_bucket
        {
            return (Arc::clone(&entry.pixels), false);
        }

        let pixels = Arc::new(crate::tray::render_widget_with_accent(
            widget,
            tray_preview_limits(),
            accent,
        ));
        cache.insert(
            preview_id.clone(),
            TrayPreviewCacheEntry {
                widget: widget.clone(),
                accent,
                uses_light_theme,
                time_format,
                minute_bucket,
                pixels: Arc::clone(&pixels),
            },
        );
        (pixels, true)
    });

    // Swap-chain preview painters normally run only on mount. Settings need
    // true live feedback. Repaint the retained native panel only when preview
    // inputs changed; root settings rerenders must not reinstall identical pixels.
    if pixels_changed {
        TRAY_PREVIEW_MOUNTS.with(|mounts| {
            if let Some(native) = mounts.borrow().get(&preview_id).cloned()
                && let Err(error) =
                    crate::acrylic::install_tray_pixels_into(native, pixels.as_slice())
            {
                eprintln!("Could not update tray preview: {error:?}");
            }
        });
    }

    let pixels_for_mount = Arc::clone(&pixels);
    let id_for_mount = preview_id.clone();
    let id_for_unmount = preview_id.clone();
    let mut host = swap_chain_panel().width(32.0).height(32.0);
    host.mounted = Some(Callback::new(
        move |native: Option<windows_core::IInspectable>| {
            if let Some(native) = native {
                if let Err(error) = crate::acrylic::install_tray_pixels_into(
                    native.clone(),
                    pixels_for_mount.as_slice(),
                ) {
                    eprintln!("Could not install tray preview: {error:?}");
                }
                TRAY_PREVIEW_MOUNTS.with(|mounts| {
                    mounts.borrow_mut().insert(id_for_mount.clone(), native);
                });
            }
        },
    ));
    host.unmounted = Some(Callback::new(
        move |_: Option<windows_core::IInspectable>| {
            TRAY_PREVIEW_MOUNTS.with(|mounts| {
                mounts.borrow_mut().remove(&id_for_unmount);
            });
            TRAY_PREVIEW_CACHE.with(|cache| {
                cache.borrow_mut().remove(&id_for_unmount);
            });
        },
    ));
    let preview: Element = host.into();
    preview.with_key(format!("tray-preview-{preview_id}"))
}

fn tray_presentation_index(presentation: TrayPresentation) -> i32 {
    match presentation.canonical_percentage() {
        TrayPresentation::StackedBars => 1,
        TrayPresentation::NestedRings => 2,
        TrayPresentation::ResetTime => 3,
        TrayPresentation::ResetCountdown => 4,
        _ => 0,
    }
}

fn tray_presentation_from_index(index: i32) -> TrayPresentation {
    match index {
        1 => TrayPresentation::StackedBars,
        2 => TrayPresentation::NestedRings,
        3 => TrayPresentation::ResetTime,
        4 => TrayPresentation::ResetCountdown,
        _ => TrayPresentation::StackedNumbers,
    }
}

fn tray_color_mode_index(mode: TrayColorMode) -> i32 {
    match mode {
        TrayColorMode::Status => 0,
        TrayColorMode::Fixed => 1,
        TrayColorMode::Provider => 2,
        TrayColorMode::Accent => 3,
        TrayColorMode::Monochrome => 4,
    }
}

fn tray_color_mode_from_index(index: i32) -> TrayColorMode {
    match index {
        1 => TrayColorMode::Fixed,
        2 => TrayColorMode::Provider,
        3 => TrayColorMode::Accent,
        4 => TrayColorMode::Monochrome,
        _ => TrayColorMode::Status,
    }
}

// Segoe Fluent chevron glyphs — same family/size as the settings card chevron.
const CHEVRON_UP_GLYPH: &str = "\u{E70E}";
const CHEVRON_DOWN_GLYPH: &str = "\u{E70D}";
const TRAY_REORDER_ICON_FONT: &str = "Segoe Fluent Icons";
/// Match the settings card chevron glyph size.
const TRAY_REORDER_ICON_SIZE: f64 = 12.0;
const TRAY_REORDER_BUTTON_SIZE: f64 = 18.0;

fn open_indicator_edit_modal(
    widget_id: String,
    indicator_index: usize,
    set_editing: AsyncSetState<Option<(String, usize)>>,
    set_visible: AsyncSetState<bool>,
) {
    let anim_id = INDICATOR_MODAL_ANIM_GEN.fetch_add(1, Ordering::Relaxed) + 1;
    set_editing.call(Some((widget_id, indicator_index)));
    set_visible.call(false);
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(16));
        if INDICATOR_MODAL_ANIM_GEN.load(Ordering::Relaxed) != anim_id {
            return;
        }
        set_visible.call(true);
    });
}

fn close_indicator_edit_modal(
    set_editing: AsyncSetState<Option<(String, usize)>>,
    set_visible: AsyncSetState<bool>,
) {
    let anim_id = INDICATOR_MODAL_ANIM_GEN.fetch_add(1, Ordering::Relaxed) + 1;
    set_visible.call(false);
    let wait = duration(CONTROL_NORMAL_ANIMATION);
    if wait.is_zero() {
        set_editing.call(None);
        return;
    }
    thread::spawn(move || {
        thread::sleep(wait);
        if INDICATOR_MODAL_ANIM_GEN.load(Ordering::Relaxed) != anim_id {
            return;
        }
        set_editing.call(None);
    });
}

fn clear_indicator_edit_modal(
    set_editing: AsyncSetState<Option<(String, usize)>>,
    set_visible: AsyncSetState<bool>,
) {
    INDICATOR_MODAL_ANIM_GEN.fetch_add(1, Ordering::Relaxed);
    set_visible.call(false);
    set_editing.call(None);
}

fn tray_settings_cards(
    widgets: &[TrayWidget],
    enabled_providers: &[ProviderKind],
    expanded_widget: &Option<String>,
    editing_indicator: &Option<(String, usize)>,
    removed_widget: &Option<(usize, TrayWidget)>,
    set_widgets: SetState<Vec<TrayWidget>>,
    set_expanded_widget: SetState<Option<String>>,
    set_editing_indicator: AsyncSetState<Option<(String, usize)>>,
    set_indicator_modal_visible: AsyncSetState<bool>,
    set_removed_widget: SetState<Option<(usize, TrayWidget)>>,
    hovered_card_id: &Option<String>,
    set_hovered_card_id: SetState<Option<String>>,
    settings_tx: Sender<Settings>,
) -> Vec<Element> {
    let _ = editing_indicator;
    let mut rows = Vec::new();
    if let Some((removed_index, removed)) = removed_widget.clone() {
        let widgets_for_undo = widgets.to_vec();
        let undo_setter = set_widgets.clone();
        let clear_removed = set_removed_widget.clone();
        let undo_tx = settings_tx.clone();
        let providers_for_undo = enabled_providers.to_vec();
        rows.push(
            border(
                hstack((
                    text_block("Widget removed")
                        .font_size(13.0)
                        .vertical_alignment(VerticalAlignment::Center),
                    Button::new("Undo").on_click(move || {
                        let mut next = widgets_for_undo.clone();
                        next.insert(removed_index.min(next.len()), removed.clone());
                        persist_tray_widgets(
                            undo_setter.clone(),
                            undo_tx.clone(),
                            next,
                            &providers_for_undo,
                        );
                        clear_removed.call(None);
                    }),
                ))
                .spacing(10.0),
            )
            .padding(settings_card_padding())
            .background(ThemeRef::LayerFill)
            .corner_radius(6.0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .with_opacity_transition(duration(CONTROL_FAST_ANIMATION))
            .with_key("tray-widget-undo")
            .into(),
        );
    }
    if widgets.is_empty() {
        rows.push(settings_info_card("Tray icon", "App icon").with_key("tray-empty"));
    }

    for (index, widget) in widgets.iter().cloned().enumerate() {
        let widget_id = widget.id.clone();
        let is_expanded = expanded_widget.as_deref() == Some(widget_id.as_str());
        let expand_id = widget_id.clone();
        let expand_setter = set_expanded_widget.clone();
        let header_id = widget_id.clone();
        let widgets_for_up = widgets.to_vec();
        let up_setter = set_widgets.clone();
        let up_tx = settings_tx.clone();
        let providers_for_up = enabled_providers.to_vec();
        let widgets_for_down = widgets.to_vec();
        let down_setter = set_widgets.clone();
        let down_tx = settings_tx.clone();
        let providers_for_down = enabled_providers.to_vec();

        let reorder_buttons = vstack((
            Button::new(CHEVRON_UP_GLYPH)
                .subtle()
                .font_family(TRAY_REORDER_ICON_FONT)
                .font_size(TRAY_REORDER_ICON_SIZE)
                .width(TRAY_REORDER_BUTTON_SIZE)
                .height(TRAY_REORDER_BUTTON_SIZE)
                .min_width(TRAY_REORDER_BUTTON_SIZE)
                .min_height(TRAY_REORDER_BUTTON_SIZE)
                .max_width(TRAY_REORDER_BUTTON_SIZE)
                .max_height(TRAY_REORDER_BUTTON_SIZE)
                .padding(Thickness::uniform(0.0))
                .enabled(index > 0)
                .tooltip("Move widget up")
                .on_click(move || {
                    if index == 0 {
                        return;
                    }
                    let mut next = widgets_for_up.clone();
                    next.swap(index, index - 1);
                    persist_tray_widgets(up_setter.clone(), up_tx.clone(), next, &providers_for_up);
                }),
            Button::new(CHEVRON_DOWN_GLYPH)
                .subtle()
                .font_family(TRAY_REORDER_ICON_FONT)
                .font_size(TRAY_REORDER_ICON_SIZE)
                .width(TRAY_REORDER_BUTTON_SIZE)
                .height(TRAY_REORDER_BUTTON_SIZE)
                .min_width(TRAY_REORDER_BUTTON_SIZE)
                .min_height(TRAY_REORDER_BUTTON_SIZE)
                .max_width(TRAY_REORDER_BUTTON_SIZE)
                .max_height(TRAY_REORDER_BUTTON_SIZE)
                .padding(Thickness::uniform(0.0))
                .enabled(index + 1 < widgets.len())
                .tooltip("Move widget down")
                .on_click(move || {
                    if index + 1 >= widgets_for_down.len() {
                        return;
                    }
                    let mut next = widgets_for_down.clone();
                    next.swap(index, index + 1);
                    persist_tray_widgets(
                        down_setter.clone(),
                        down_tx.clone(),
                        next,
                        &providers_for_down,
                    );
                }),
        ))
        .spacing(0.0)
        .horizontal_alignment(HorizontalAlignment::Center)
        .vertical_alignment(VerticalAlignment::Center);
        let header = grid((
            reorder_buttons
                .grid_column(0)
                .vertical_alignment(VerticalAlignment::Center),
            tray_widget_preview(&widget)
                .grid_column(1)
                .vertical_alignment(VerticalAlignment::Center),
            vstack((
                text_block(format!("Widget {}", index + 1))
                    .font_size(14.0)
                    .semibold(),
                text_block(tray_widget_summary(&widget))
                    .font_size(12.0)
                    .foreground(ThemeRef::SecondaryText),
            ))
            .spacing(2.0)
            .vertical_alignment(VerticalAlignment::Center)
            .on_tapped({
                let expand_setter = set_expanded_widget.clone();
                let expand_id = widget_id.clone();
                move || {
                    expand_setter.call(if is_expanded {
                        None
                    } else {
                        Some(expand_id.clone())
                    });
                }
            })
            .grid_column(2),
        ))
        .columns([
            GridLength::Pixel(TRAY_REORDER_BUTTON_SIZE),
            GridLength::Pixel(32.0),
            GridLength::Star(1.0),
        ])
        .column_spacing(8.0)
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .with_key(format!("tray-header-{header_id}"));

        // Collapsed cards need headers only. Building their declarative editor trees
        // eagerly allocates many callbacks and clones the complete widget list per action.
        let content: Element = if !is_expanded {
            Element::Empty
        } else if widget.kind == TrayWidgetKind::AppIcon {
            let widgets_for_duplicate = widgets.to_vec();
            let duplicate_setter = set_widgets.clone();
            let duplicate_tx = settings_tx.clone();
            let providers_for_duplicate = enabled_providers.to_vec();
            let widgets_for_remove = widgets.to_vec();
            let remove_setter = set_widgets.clone();
            let removed_setter = set_removed_widget.clone();
            let remove_tx = settings_tx.clone();
            let providers_for_remove = enabled_providers.to_vec();
            hstack((
                Button::new("Duplicate").on_click(move || {
                    let mut next = widgets_for_duplicate.clone();
                    next.insert(index + 1, TrayWidget::app_icon());
                    persist_tray_widgets(
                        duplicate_setter.clone(),
                        duplicate_tx.clone(),
                        next,
                        &providers_for_duplicate,
                    );
                }),
                Button::new("Remove").on_click(move || {
                    let mut next = widgets_for_remove.clone();
                    let removed = next.remove(index);
                    removed_setter.call(Some((index, removed)));
                    persist_tray_widgets(
                        remove_setter.clone(),
                        remove_tx.clone(),
                        next,
                        &providers_for_remove,
                    );
                }),
            ))
            .spacing(8.0)
            .into()
        } else {
            let mut fields = Vec::<Element>::new();

            let widgets_for_presentation = widgets.to_vec();
            let presentation_setter = set_widgets.clone();
            let presentation_tx = settings_tx.clone();
            let providers_for_presentation = enabled_providers.to_vec();
            fields.push(
                ComboBox::new([
                    "Numbers",
                    "Progress bars",
                    "Rings",
                    "Reset time",
                    "Countdown",
                ])
                .header("Appearance")
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .selected_index(tray_presentation_index(widget.presentation))
                .on_selection_changed({
                    let providers_for_empty = enabled_providers.to_vec();
                    move |choice| {
                        let mut next = widgets_for_presentation.clone();
                        next[index].presentation = tray_presentation_from_index(choice);
                        if next[index].presentation.is_reset_clock() {
                            next[index].indicators.truncate(1);
                            if next[index].indicators.is_empty() {
                                let provider = providers_for_empty
                                    .first()
                                    .copied()
                                    .unwrap_or(ProviderKind::Codex);
                                let descriptor = crate::provider_registry::descriptor(provider);
                                let metric = descriptor
                                    .default_tray_metrics
                                    .first()
                                    .copied()
                                    .unwrap_or("unknown");
                                next[index]
                                    .indicators
                                    .push(TrayIndicator::new(provider, metric));
                            }
                        }
                        persist_tray_widgets(
                            presentation_setter.clone(),
                            presentation_tx.clone(),
                            next,
                            &providers_for_presentation,
                        );
                    }
                })
                .into(),
            );

            if widget.presentation.is_reset_clock() {
                fields.push(tray_time_parameter_fields(
                    index,
                    &widget,
                    widgets,
                    enabled_providers,
                    set_widgets.clone(),
                    settings_tx.clone(),
                ));
            } else {
                fields.push(settings_section_heading("Indicators"));
                for (indicator_index, indicator) in widget.indicators.iter().cloned().enumerate() {
                    fields.push(tray_indicator_summary_card(
                        index,
                        indicator_index,
                        &indicator,
                        &widget,
                        widgets,
                        enabled_providers,
                        set_widgets.clone(),
                        set_editing_indicator.clone(),
                        set_indicator_modal_visible.clone(),
                        set_removed_widget.clone(),
                        settings_tx.clone(),
                    ));
                }
            }

            let mut widget_actions = Vec::<Element>::new();
            if !widget.presentation.is_reset_clock() && widget.indicators.len() < 3 {
                let widgets_for_add = widgets.to_vec();
                let add_setter = set_widgets.clone();
                let add_tx = settings_tx.clone();
                let enabled_for_add = enabled_providers.to_vec();
                let edit_added = set_editing_indicator.clone();
                let show_added = set_indicator_modal_visible.clone();
                let widget_id_for_add = widget.id.clone();
                let fallback_provider = widget
                    .indicators
                    .last()
                    .and_then(TrayIndicator::provider)
                    .or_else(|| enabled_providers.first().copied())
                    .unwrap_or(ProviderKind::Codex);
                widget_actions.push(
                    Button::new("Add indicator")
                        .on_click(move || {
                            let descriptor =
                                crate::provider_registry::descriptor(fallback_provider);
                            let metric = descriptor
                                .default_tray_metrics
                                .first()
                                .copied()
                                .unwrap_or("unknown");
                            let mut next = widgets_for_add.clone();
                            let new_index = next[index].indicators.len();
                            next[index]
                                .indicators
                                .push(TrayIndicator::new(fallback_provider, metric));
                            persist_tray_widgets(
                                add_setter.clone(),
                                add_tx.clone(),
                                next,
                                &enabled_for_add,
                            );
                            open_indicator_edit_modal(
                                widget_id_for_add.clone(),
                                new_index,
                                edit_added.clone(),
                                show_added.clone(),
                            );
                        })
                        .into(),
                );
            }

            let widgets_for_duplicate = widgets.to_vec();
            let duplicate_setter = set_widgets.clone();
            let duplicate_tx = settings_tx.clone();
            let enabled_for_duplicate = enabled_providers.to_vec();
            let widgets_for_remove = widgets.to_vec();
            let remove_setter = set_widgets.clone();
            let removed_setter = set_removed_widget.clone();
            let remove_tx = settings_tx.clone();
            let enabled_for_remove = enabled_providers.to_vec();
            let clear_editing = set_editing_indicator.clone();
            let clear_visible = set_indicator_modal_visible.clone();
            let removed_widget_id = widget.id.clone();
            widget_actions.push(
                Button::new("Duplicate")
                    .on_click(move || {
                        let mut next = widgets_for_duplicate.clone();
                        let copy = next[index].duplicate_with_new_id();
                        next.insert(index + 1, copy);
                        persist_tray_widgets(
                            duplicate_setter.clone(),
                            duplicate_tx.clone(),
                            next,
                            &enabled_for_duplicate,
                        );
                    })
                    .into(),
            );
            widget_actions.push(
                Button::new("Remove")
                    .on_click(move || {
                        let mut next = widgets_for_remove.clone();
                        let removed = next.remove(index);
                        clear_indicator_edit_modal(clear_editing.clone(), clear_visible.clone());
                        let _ = removed_widget_id;
                        removed_setter.call(Some((index, removed)));
                        persist_tray_widgets(
                            remove_setter.clone(),
                            remove_tx.clone(),
                            next,
                            &enabled_for_remove,
                        );
                    })
                    .into(),
            );
            fields.push(
                hstack(widget_actions)
                    .spacing(8.0)
                    .horizontal_alignment(HorizontalAlignment::Left)
                    .into(),
            );
            vstack(fields).spacing(10.0).into()
        };

        let row: Element = settings_content_expander(
            header,
            is_expanded,
            move |expanded: bool| {
                expand_setter.call(expanded.then(|| expand_id.clone()));
            },
            format!("tray-widget-{widget_id}"),
            hovered_card_id,
            set_hovered_card_id.clone(),
            content,
        )
        .with_key(format!("tray-widget-{widget_id}"));
        rows.push(row);
    }

    let first_enabled = enabled_providers
        .first()
        .copied()
        .unwrap_or(ProviderKind::Codex);
    let mut add_actions = Vec::<Element>::new();
    let widgets_for_custom = widgets.to_vec();
    let custom_setter = set_widgets.clone();
    let custom_tx = settings_tx.clone();
    let providers_for_custom = enabled_providers.to_vec();
    let expanded_for_custom = set_expanded_widget.clone();
    let widgets_for_app = widgets.to_vec();
    let app_setter = set_widgets;
    let providers_for_app = enabled_providers.to_vec();
    add_actions.push(
        Button::new("Add widget")
            .accent()
            .enabled(!enabled_providers.is_empty())
            .on_click(move || {
                let mut next = widgets_for_custom.clone();
                let widget = TrayWidget::custom_for_provider(first_enabled);
                let id = widget.id.clone();
                next.push(widget);
                persist_tray_widgets(
                    custom_setter.clone(),
                    custom_tx.clone(),
                    next,
                    &providers_for_custom,
                );
                expanded_for_custom.call(Some(id));
            })
            .into(),
    );
    add_actions.push(
        Button::new("Add app icon")
            .on_click(move || {
                let mut next = widgets_for_app.clone();
                next.push(TrayWidget::app_icon());
                persist_tray_widgets(
                    app_setter.clone(),
                    settings_tx.clone(),
                    next,
                    &providers_for_app,
                );
            })
            .into(),
    );
    rows.push(
        hstack(add_actions)
            .spacing(8.0)
            .horizontal_alignment(HorizontalAlignment::Left)
            .with_key("tray-add-actions")
            .into(),
    );
    rows
}

fn tray_time_parameter_fields(
    widget_index: usize,
    widget: &TrayWidget,
    widgets: &[TrayWidget],
    enabled_providers: &[ProviderKind],
    set_widgets: SetState<Vec<TrayWidget>>,
    settings_tx: Sender<Settings>,
) -> Element {
    let indicator = widget.indicators.first().cloned().unwrap_or_else(|| {
        let provider = enabled_providers
            .first()
            .copied()
            .unwrap_or(ProviderKind::Codex);
        let descriptor = crate::provider_registry::descriptor(provider);
        let metric = descriptor
            .default_tray_metrics
            .first()
            .copied()
            .unwrap_or("unknown");
        TrayIndicator::new(provider, metric)
    });
    let indicator_index = 0usize;

    let provider_options: Vec<_> = crate::provider_registry::PROVIDERS
        .iter()
        .filter(|provider| !provider.default_tray_metrics.is_empty())
        .collect();
    let known_provider = indicator.provider();
    let mut provider_labels = provider_options
        .iter()
        .map(|provider| provider.display_name.to_owned())
        .collect::<Vec<_>>();
    let provider_index = known_provider
        .and_then(|provider| {
            provider_options
                .iter()
                .position(|descriptor| descriptor.kind == provider)
        })
        .unwrap_or_else(|| {
            provider_labels.push(format!("Unsupported ({})", indicator.provider_id));
            provider_labels.len() - 1
        }) as i32;
    let metric_provider = known_provider.unwrap_or(ProviderKind::Codex);
    let metrics = crate::provider_registry::descriptor(metric_provider).metrics;
    let mut metric_labels = metrics
        .iter()
        .map(|metric| metric.label.to_owned())
        .collect::<Vec<_>>();
    let metric_index = metrics
        .iter()
        .position(|metric| metric.id == indicator.metric_id)
        .unwrap_or_else(|| {
            metric_labels.push(format!("Unavailable ({})", indicator.metric_id));
            metric_labels.len() - 1
        }) as i32;

    let widgets_for_provider = widgets.to_vec();
    let provider_setter = set_widgets.clone();
    let provider_tx = settings_tx.clone();
    let enabled_for_provider = enabled_providers.to_vec();
    let provider_box = ComboBox::new(provider_labels)
        .header("Provider")
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .selected_index(provider_index)
        .on_selection_changed(move |choice: i32| {
            let Some(descriptor) = crate::provider_registry::PROVIDERS.get(choice.max(0) as usize)
            else {
                return;
            };
            let mut next = widgets_for_provider.clone();
            if next[widget_index].indicators.is_empty() {
                next[widget_index]
                    .indicators
                    .push(TrayIndicator::new(descriptor.kind, "unknown"));
            }
            next[widget_index].indicators[indicator_index].provider_id = descriptor.id.into();
            next[widget_index].indicators[indicator_index].metric_id = descriptor
                .default_tray_metrics
                .first()
                .copied()
                .unwrap_or("unknown")
                .into();
            persist_tray_widgets(
                provider_setter.clone(),
                provider_tx.clone(),
                next,
                &enabled_for_provider,
            );
        });

    let widgets_for_metric = widgets.to_vec();
    let metric_setter = set_widgets.clone();
    let metric_tx = settings_tx.clone();
    let enabled_for_metric = enabled_providers.to_vec();
    let metric_box = ComboBox::new(metric_labels)
        .header("Metric")
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .selected_index(metric_index)
        .with_key(format!(
            "tray-time-metric-{}-{}",
            widget.id, indicator.provider_id
        ))
        .on_selection_changed(move |choice: i32| {
            let Some(metric) = metrics.get(choice.max(0) as usize) else {
                return;
            };
            let mut next = widgets_for_metric.clone();
            if next[widget_index].indicators.is_empty() {
                return;
            }
            next[widget_index].indicators[indicator_index].metric_id = metric.id.into();
            persist_tray_widgets(
                metric_setter.clone(),
                metric_tx.clone(),
                next,
                &enabled_for_metric,
            );
        });

    let widgets_for_color = widgets.to_vec();
    let color_setter = set_widgets.clone();
    let color_tx = settings_tx.clone();
    let providers_for_color = enabled_providers.to_vec();
    let color_box = ComboBox::new(["Status", "Fixed", "Provider", "App accent", "Monochrome"])
        .header("Color")
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .selected_index(tray_color_mode_index(indicator.color_mode))
        .on_selection_changed(move |choice| {
            let mut next = widgets_for_color.clone();
            if next[widget_index].indicators.is_empty() {
                return;
            }
            next[widget_index].indicators[indicator_index].color_mode =
                tray_color_mode_from_index(choice);
            persist_tray_widgets(
                color_setter.clone(),
                color_tx.clone(),
                next,
                &providers_for_color,
            );
        });

    let mut fields = Vec::<Element>::new();
    fields.push(
        grid((
            provider_box.grid_column(0).grid_row(0),
            metric_box.grid_column(1).grid_row(0),
            color_box.grid_column(0).grid_row(1).grid_column_span(2),
        ))
        .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
        .rows([GridLength::Auto, GridLength::Auto])
        .column_spacing(12.0)
        .row_spacing(12.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into(),
    );

    if indicator.color_mode == TrayColorMode::Fixed {
        let widgets_for_picker = widgets.to_vec();
        let picker_setter = set_widgets;
        let picker_tx = settings_tx;
        let providers_for_picker = enabled_providers.to_vec();
        fields.push(
            border(
                ColorPicker::new(ColorArgb::new(
                    indicator.fixed_color.red,
                    indicator.fixed_color.green,
                    indicator.fixed_color.blue,
                ))
                .alpha_enabled(false)
                .hex_input_visible(true)
                .color_slider_visible(true)
                .color_channel_text_input_visible(false)
                .on_color_changed(move |(_, red, green, blue)| {
                    let mut next = widgets_for_picker.clone();
                    if next[widget_index].indicators.is_empty() {
                        return;
                    }
                    next[widget_index].indicators[indicator_index].fixed_color =
                        TrayFixedColor { red, green, blue };
                    persist_tray_widgets(
                        picker_setter.clone(),
                        picker_tx.clone(),
                        next,
                        &providers_for_picker,
                    );
                }),
            )
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .min_height(180.0)
            .into(),
        );
    }

    vstack(fields)
        .spacing(12.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .with_key(format!("tray-time-fields-{}", widget.id))
        .into()
}

fn tray_indicator_summary_card(
    widget_index: usize,
    indicator_index: usize,
    indicator: &TrayIndicator,
    widget: &TrayWidget,
    widgets: &[TrayWidget],
    enabled_providers: &[ProviderKind],
    set_widgets: SetState<Vec<TrayWidget>>,
    set_editing_indicator: AsyncSetState<Option<(String, usize)>>,
    set_indicator_modal_visible: AsyncSetState<bool>,
    set_removed_widget: SetState<Option<(usize, TrayWidget)>>,
    settings_tx: Sender<Settings>,
) -> Element {
    let widget_id = widget.id.clone();
    let widgets_for_up = widgets.to_vec();
    let up_setter = set_widgets.clone();
    let up_tx = settings_tx.clone();
    let enabled_for_up = enabled_providers.to_vec();
    let widgets_for_down = widgets.to_vec();
    let down_setter = set_widgets.clone();
    let down_tx = settings_tx.clone();
    let enabled_for_down = enabled_providers.to_vec();
    let widgets_for_remove = widgets.to_vec();
    let remove_setter = set_widgets;
    let removed_setter = set_removed_widget;
    let remove_tx = settings_tx;
    let enabled_for_remove = enabled_providers.to_vec();
    let clear_editing = set_editing_indicator.clone();
    let clear_visible = set_indicator_modal_visible.clone();
    let open_editor = set_editing_indicator;
    let show_editor = set_indicator_modal_visible;
    let edit_widget_id = widget_id.clone();

    let reorder = vstack((
        Button::new(CHEVRON_UP_GLYPH)
            .subtle()
            .font_family(TRAY_REORDER_ICON_FONT)
            .font_size(TRAY_REORDER_ICON_SIZE)
            .width(TRAY_REORDER_BUTTON_SIZE)
            .height(TRAY_REORDER_BUTTON_SIZE)
            .min_width(TRAY_REORDER_BUTTON_SIZE)
            .min_height(TRAY_REORDER_BUTTON_SIZE)
            .max_width(TRAY_REORDER_BUTTON_SIZE)
            .max_height(TRAY_REORDER_BUTTON_SIZE)
            .padding(Thickness::uniform(0.0))
            .enabled(indicator_index > 0)
            .tooltip("Move indicator up")
            .on_click(move || {
                if indicator_index == 0 {
                    return;
                }
                let mut next = widgets_for_up.clone();
                next[widget_index]
                    .indicators
                    .swap(indicator_index, indicator_index - 1);
                persist_tray_widgets(up_setter.clone(), up_tx.clone(), next, &enabled_for_up);
            }),
        Button::new(CHEVRON_DOWN_GLYPH)
            .subtle()
            .font_family(TRAY_REORDER_ICON_FONT)
            .font_size(TRAY_REORDER_ICON_SIZE)
            .width(TRAY_REORDER_BUTTON_SIZE)
            .height(TRAY_REORDER_BUTTON_SIZE)
            .min_width(TRAY_REORDER_BUTTON_SIZE)
            .min_height(TRAY_REORDER_BUTTON_SIZE)
            .max_width(TRAY_REORDER_BUTTON_SIZE)
            .max_height(TRAY_REORDER_BUTTON_SIZE)
            .padding(Thickness::uniform(0.0))
            .enabled(indicator_index + 1 < widget.indicators.len())
            .tooltip("Move indicator down")
            .on_click(move || {
                let mut next = widgets_for_down.clone();
                if indicator_index + 1 >= next[widget_index].indicators.len() {
                    return;
                }
                next[widget_index]
                    .indicators
                    .swap(indicator_index, indicator_index + 1);
                persist_tray_widgets(
                    down_setter.clone(),
                    down_tx.clone(),
                    next,
                    &enabled_for_down,
                );
            }),
    ))
    .spacing(0.0)
    .vertical_alignment(VerticalAlignment::Center);

    border(
        grid((
            reorder
                .grid_column(0)
                .vertical_alignment(VerticalAlignment::Center),
            vstack((
                text_block(tray_indicator_summary(indicator))
                    .font_size(14.0)
                    .semibold()
                    .wrap(),
                text_block(format!("Indicator {}", indicator_index + 1))
                    .font_size(12.0)
                    .foreground(ThemeRef::SecondaryText),
            ))
            .spacing(2.0)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(1),
            hstack((
                Button::new("Edit").on_click(move || {
                    open_indicator_edit_modal(
                        edit_widget_id.clone(),
                        indicator_index,
                        open_editor.clone(),
                        show_editor.clone(),
                    );
                }),
                Button::new("Remove").on_click(move || {
                    let mut next = widgets_for_remove.clone();
                    next[widget_index].indicators.remove(indicator_index);
                    if next[widget_index].indicators.is_empty() {
                        let removed = next.remove(widget_index);
                        clear_indicator_edit_modal(clear_editing.clone(), clear_visible.clone());
                        removed_setter.call(Some((widget_index, removed)));
                    } else {
                        clear_indicator_edit_modal(clear_editing.clone(), clear_visible.clone());
                    }
                    persist_tray_widgets(
                        remove_setter.clone(),
                        remove_tx.clone(),
                        next,
                        &enabled_for_remove,
                    );
                }),
            ))
            .spacing(8.0)
            .vertical_alignment(VerticalAlignment::Center)
            .horizontal_alignment(HorizontalAlignment::Right)
            .grid_column(2),
        ))
        .columns([
            GridLength::Pixel(TRAY_REORDER_BUTTON_SIZE),
            GridLength::Star(1.0),
            GridLength::Auto,
        ])
        .column_spacing(12.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(settings_card_padding())
    .background(ThemeRef::CardBackground)
    .corner_radius(8.0)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .min_height(60.0)
    .with_key(format!("tray-indicator-{}-{indicator_index}", widget.id))
    .with_translation_transition(duration(CONTROL_FAST_ANIMATION))
    .into()
}

fn tray_indicator_edit_overlay(
    widgets: &[TrayWidget],
    enabled_providers: &[ProviderKind],
    editing: &(String, usize),
    visible: bool,
    set_widgets: SetState<Vec<TrayWidget>>,
    set_editing_indicator: AsyncSetState<Option<(String, usize)>>,
    set_indicator_modal_visible: AsyncSetState<bool>,
    settings_tx: Sender<Settings>,
) -> Option<Element> {
    let (widget_id, indicator_index) = editing;
    let (widget_index, widget) = widgets
        .iter()
        .cloned()
        .enumerate()
        .find(|(_, widget)| widget.id == *widget_id)?;
    let indicator = widget.indicators.get(*indicator_index)?.clone();

    let dismiss_editing = set_editing_indicator.clone();
    let dismiss_visible = set_indicator_modal_visible.clone();
    let form = tray_indicator_edit_form(
        widget_index,
        *indicator_index,
        &indicator,
        &widget,
        widgets,
        enabled_providers,
        set_widgets,
        set_editing_indicator,
        set_indicator_modal_visible,
        settings_tx,
    );

    let anim = duration(CONTROL_NORMAL_ANIMATION);
    let card = border(
        scroll_viewer(border(form).padding(Thickness {
            left: 24.0,
            top: 24.0,
            right: 24.0,
            bottom: 24.0,
        }))
        .horizontal_scroll_bar_visibility(ScrollBarVisibility::Disabled)
        .vertical_scroll_bar_visibility(ScrollBarVisibility::Auto)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Stretch),
    )
    .background(ThemeRef::SolidBackground)
    .corner_radius(INDICATOR_MODAL_RADIUS)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .width(INDICATOR_MODAL_WIDTH)
    .horizontal_alignment(HorizontalAlignment::Center)
    .vertical_alignment(VerticalAlignment::Stretch)
    .on_tapped(|| {})
    .with_key(format!(
        "tray-indicator-modal-{}-{indicator_index}",
        widget.id
    ));

    // 1+18+1 star rows → middle band is 90% of the window height.
    Some(
        relative_panel::<Vec<Element>>(vec![
            border(Element::Empty)
                .background(INDICATOR_MODAL_SCRIM)
                .opacity(if visible { 1.0 } else { 0.0 })
                .relative_align_left()
                .relative_align_right()
                .relative_align_top()
                .relative_align_bottom()
                .on_tapped(move || {
                    close_indicator_edit_modal(dismiss_editing.clone(), dismiss_visible.clone());
                })
                .with_opacity_transition(anim)
                .into(),
            grid((
                border(Element::Empty).grid_row(0),
                card.opacity(if visible { 1.0 } else { 0.0 })
                    .with_opacity_transition(anim)
                    .grid_row(1),
                border(Element::Empty).grid_row(2),
            ))
            .columns([GridLength::Star(1.0)])
            .rows([
                GridLength::Star(1.0),
                GridLength::Star(18.0),
                GridLength::Star(1.0),
            ])
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch)
            .relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .relative_align_bottom()
            .into(),
        ])
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Stretch)
        .with_key("tray-indicator-edit-overlay")
        .into(),
    )
}

fn tray_indicator_edit_form(
    widget_index: usize,
    indicator_index: usize,
    indicator: &TrayIndicator,
    widget: &TrayWidget,
    widgets: &[TrayWidget],
    enabled_providers: &[ProviderKind],
    set_widgets: SetState<Vec<TrayWidget>>,
    set_editing_indicator: AsyncSetState<Option<(String, usize)>>,
    set_indicator_modal_visible: AsyncSetState<bool>,
    settings_tx: Sender<Settings>,
) -> Element {
    let provider_options: Vec<_> = crate::provider_registry::PROVIDERS
        .iter()
        .filter(|provider| !provider.default_tray_metrics.is_empty())
        .collect();
    let known_provider = indicator.provider();
    let mut provider_labels = provider_options
        .iter()
        .map(|provider| provider.display_name.to_owned())
        .collect::<Vec<_>>();
    let provider_index = known_provider
        .and_then(|provider| {
            provider_options
                .iter()
                .position(|descriptor| descriptor.kind == provider)
        })
        .unwrap_or_else(|| {
            provider_labels.push(format!("Unsupported ({})", indicator.provider_id));
            provider_labels.len() - 1
        }) as i32;
    let metric_provider = known_provider.unwrap_or(ProviderKind::Codex);
    let metrics = crate::provider_registry::descriptor(metric_provider).metrics;
    let mut metric_labels = metrics
        .iter()
        .map(|metric| metric.label.to_owned())
        .collect::<Vec<_>>();
    let metric_index = metrics
        .iter()
        .position(|metric| metric.id == indicator.metric_id)
        .unwrap_or_else(|| {
            metric_labels.push(format!("Unavailable ({})", indicator.metric_id));
            metric_labels.len() - 1
        }) as i32;

    let mut fields = Vec::<Element>::new();
    let close_editing = set_editing_indicator;
    let close_visible = set_indicator_modal_visible;
    fields.push(
        grid((
            hstack((
                tray_widget_preview(widget).vertical_alignment(VerticalAlignment::Center),
                text_block(format!("Edit indicator {}", indicator_index + 1))
                    .font_size(24.0)
                    .bold()
                    .vertical_alignment(VerticalAlignment::Center),
            ))
            .spacing(12.0)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(0),
            Button::new("Done")
                .accent()
                .on_click(move || {
                    close_indicator_edit_modal(close_editing.clone(), close_visible.clone());
                })
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .column_spacing(12.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into(),
    );

    fields.push(
        text_block(tray_indicator_summary(indicator))
            .font_size(14.0)
            .foreground(ThemeRef::SecondaryText)
            .into(),
    );

    let widgets_for_provider = widgets.to_vec();
    let provider_setter = set_widgets.clone();
    let provider_tx = settings_tx.clone();
    let enabled_for_provider = enabled_providers.to_vec();
    let provider_box = ComboBox::new(provider_labels)
        .header("Provider")
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .selected_index(provider_index)
        .on_selection_changed(move |choice: i32| {
            let Some(descriptor) = crate::provider_registry::PROVIDERS.get(choice.max(0) as usize)
            else {
                return;
            };
            let mut next = widgets_for_provider.clone();
            next[widget_index].indicators[indicator_index].provider_id = descriptor.id.into();
            next[widget_index].indicators[indicator_index].metric_id = descriptor
                .default_tray_metrics
                .first()
                .copied()
                .unwrap_or("unknown")
                .into();
            persist_tray_widgets(
                provider_setter.clone(),
                provider_tx.clone(),
                next,
                &enabled_for_provider,
            );
        });

    let widgets_for_metric = widgets.to_vec();
    let metric_setter = set_widgets.clone();
    let metric_tx = settings_tx.clone();
    let enabled_for_metric = enabled_providers.to_vec();
    let metric_box = ComboBox::new(metric_labels)
        .header("Metric")
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .selected_index(metric_index)
        .with_key(format!(
            "tray-metric-modal-{}-{indicator_index}-{}",
            widget.id, indicator.provider_id
        ))
        .on_selection_changed(move |choice: i32| {
            let Some(metric) = metrics.get(choice.max(0) as usize) else {
                return;
            };
            let mut next = widgets_for_metric.clone();
            next[widget_index].indicators[indicator_index].metric_id = metric.id.into();
            persist_tray_widgets(
                metric_setter.clone(),
                metric_tx.clone(),
                next,
                &enabled_for_metric,
            );
        });

    let widgets_for_value = widgets.to_vec();
    let value_setter = set_widgets.clone();
    let value_tx = settings_tx.clone();
    let enabled_for_value = enabled_providers.to_vec();
    let value_box = ComboBox::new(["Remaining", "Used"])
        .header("Value")
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .selected_index(if indicator.limit_value == LimitValue::Remaining {
            0
        } else {
            1
        })
        .on_selection_changed(move |choice| {
            let mut next = widgets_for_value.clone();
            next[widget_index].indicators[indicator_index].limit_value = if choice == 1 {
                LimitValue::Used
            } else {
                LimitValue::Remaining
            };
            persist_tray_widgets(
                value_setter.clone(),
                value_tx.clone(),
                next,
                &enabled_for_value,
            );
        });

    // Color is per-indicator so stacked tray glyphs can use different palettes.
    let widgets_for_color = widgets.to_vec();
    let color_setter = set_widgets.clone();
    let color_tx = settings_tx.clone();
    let providers_for_color = enabled_providers.to_vec();
    let color_box = ComboBox::new(["Status", "Fixed", "Provider", "App accent", "Monochrome"])
        .header("Color")
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .selected_index(tray_color_mode_index(indicator.color_mode))
        .on_selection_changed(move |choice| {
            let mut next = widgets_for_color.clone();
            next[widget_index].indicators[indicator_index].color_mode =
                tray_color_mode_from_index(choice);
            persist_tray_widgets(
                color_setter.clone(),
                color_tx.clone(),
                next,
                &providers_for_color,
            );
        });

    fields.push(
        grid((
            provider_box.grid_column(0).grid_row(0),
            metric_box.grid_column(1).grid_row(0),
            value_box.grid_column(0).grid_row(1),
            color_box.grid_column(1).grid_row(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
        .rows([GridLength::Auto, GridLength::Auto])
        .column_spacing(12.0)
        .row_spacing(12.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into(),
    );

    if indicator.color_mode == TrayColorMode::Fixed {
        let widgets_for_picker = widgets.to_vec();
        let picker_setter = set_widgets;
        let picker_tx = settings_tx;
        let providers_for_picker = enabled_providers.to_vec();
        fields.push(
            border(
                ColorPicker::new(ColorArgb::new(
                    indicator.fixed_color.red,
                    indicator.fixed_color.green,
                    indicator.fixed_color.blue,
                ))
                .alpha_enabled(false)
                .hex_input_visible(true)
                .color_slider_visible(true)
                .color_channel_text_input_visible(false)
                .on_color_changed(move |(_, red, green, blue)| {
                    let mut next = widgets_for_picker.clone();
                    next[widget_index].indicators[indicator_index].fixed_color =
                        TrayFixedColor { red, green, blue };
                    persist_tray_widgets(
                        picker_setter.clone(),
                        picker_tx.clone(),
                        next,
                        &providers_for_picker,
                    );
                }),
            )
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .min_height(220.0)
            .into(),
        );
    }

    vstack(fields)
        .spacing(16.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .with_key(format!(
            "tray-indicator-form-{}-{indicator_index}",
            widget.id
        ))
        .into()
}

#[cfg(any())]
fn legacy_tray_settings_cards(
    widgets: &[TrayWidget],
    enabled_providers: &[ProviderKind],
    set_widgets: SetState<Vec<TrayWidget>>,
    settings_tx: Sender<Settings>,
) -> Vec<Element> {
    let mut cards = Vec::new();
    if widgets.is_empty() {
        cards.push(
            settings_info_card("Tray icon", "App icon (add a widget to replace it)")
                .with_key("tray-empty"),
        );
    }
    for (index, widget) in widgets.iter().cloned().enumerate() {
        let source_items = vec!["5h + week", "5h limit", "Weekly limit", "5h reset"];
        let source_index = source_index(&widget.source);
        let presentation_items = presentation_options(&widget.source);
        let presentation_index = presentation_items
            .iter()
            .position(|(_, presentation)| *presentation == widget.presentation)
            .unwrap_or(0) as i32;
        let widget_for_source = widget.clone();
        let widgets_for_provider = widgets.to_vec();
        let provider_setter = set_widgets.clone();
        let provider_tx = settings_tx.clone();
        let providers_for_provider = enabled_providers.to_vec();
        let widgets_for_source = widgets.to_vec();
        let source_setter = set_widgets.clone();
        let source_tx = settings_tx.clone();
        let providers_for_source = enabled_providers.to_vec();
        let widget_for_presentation = widget.clone();
        let widgets_for_presentation = widgets.to_vec();
        let presentation_setter = set_widgets.clone();
        let presentation_tx = settings_tx.clone();
        let providers_for_presentation = enabled_providers.to_vec();
        let widgets_for_value = widgets.to_vec();
        let value_setter = set_widgets.clone();
        let value_tx = settings_tx.clone();
        let providers_for_value = enabled_providers.to_vec();
        let widgets_for_remove = widgets.to_vec();
        let remove_setter = set_widgets.clone();
        let remove_tx = settings_tx.clone();
        let providers_for_remove = enabled_providers.to_vec();
        let widgets_for_left = widgets.to_vec();
        let left_setter = set_widgets.clone();
        let left_tx = settings_tx.clone();
        let providers_for_left = enabled_providers.to_vec();
        let widgets_for_right = widgets.to_vec();
        let right_setter = set_widgets.clone();
        let right_tx = settings_tx.clone();
        let providers_for_right = enabled_providers.to_vec();

        let mut fields: Vec<Element> = vec![
            text_block(format!("Tray widget {}", index + 1))
                .font_size(16.0)
                .bold()
                .into(),
            ComboBox::new(source_items)
                .header("Information")
                .selected_index(source_index)
                .on_selection_changed(move |choice: i32| {
                    let mut next = widgets_for_source.clone();
                    let source = source_from_index(choice);
                    next[index] = TrayWidget {
                        provider: widget_for_source.provider,
                        source: source.clone(),
                        presentation: default_presentation(&source),
                        limit_value: widget_for_source.limit_value,
                    };
                    persist_tray_widgets(
                        source_setter.clone(),
                        source_tx.clone(),
                        next,
                        &providers_for_source,
                    );
                })
                .into(),
            ComboBox::new(presentation_items.iter().map(|(label, _)| *label))
                .header("Appearance")
                .selected_index(presentation_index)
                // Remount when Information changes so item labels cannot stick
                // to a stale ComboBox selection header.
                .with_key(format!("tray-appearance-{index}-{source_index}"))
                .on_selection_changed(move |choice: i32| {
                    let mut next = widgets_for_presentation.clone();
                    if let Some((_, presentation)) =
                        presentation_options(&widget_for_presentation.source)
                            .get(choice.max(0) as usize)
                    {
                        next[index].presentation = presentation.clone();
                        persist_tray_widgets(
                            presentation_setter.clone(),
                            presentation_tx.clone(),
                            next,
                            &providers_for_presentation,
                        );
                    }
                })
                .into(),
        ];
        match enabled_providers {
            [] => fields.push(
                text_block("Enable a provider to choose what this widget displays.")
                    .font_size(12.0)
                    .foreground(ThemeRef::SecondaryText)
                    .wrap()
                    .into(),
            ),
            [_] => {}
            providers => {
                let provider_index = providers
                    .iter()
                    .position(|provider| *provider == widget.provider)
                    .unwrap_or(0) as i32;
                fields.insert(
                    1,
                    ComboBox::new(providers.iter().map(|provider| provider.display_name()))
                        .header("Provider")
                        .selected_index(provider_index)
                        .on_selection_changed(move |choice: i32| {
                            let Some(provider) =
                                providers_for_provider.get(choice.max(0) as usize).copied()
                            else {
                                return;
                            };
                            let mut next = widgets_for_provider.clone();
                            next[index].provider = provider;
                            persist_tray_widgets(
                                provider_setter.clone(),
                                provider_tx.clone(),
                                next,
                                &providers_for_provider,
                            );
                        })
                        .into(),
                );
            }
        }
        if widget.uses_limit_value() {
            fields.push(
                ComboBox::new(["Remaining", "Used"])
                    .header("Limit value")
                    .selected_index(if widget.limit_value == LimitValue::Remaining {
                        0
                    } else {
                        1
                    })
                    .on_selection_changed(move |choice| {
                        let mut next = widgets_for_value.clone();
                        next[index].limit_value = if choice == 1 {
                            LimitValue::Used
                        } else {
                            LimitValue::Remaining
                        };
                        persist_tray_widgets(
                            value_setter.clone(),
                            value_tx.clone(),
                            next,
                            &providers_for_value,
                        );
                    })
                    .into(),
            );
        }
        fields.push(
            hstack((
                Button::new("Move left")
                    .enabled(index > 0)
                    .on_click(move || {
                        if index == 0 {
                            return;
                        }
                        let mut next = widgets_for_left.clone();
                        next.swap(index, index - 1);
                        persist_tray_widgets(
                            left_setter.clone(),
                            left_tx.clone(),
                            next,
                            &providers_for_left,
                        );
                    }),
                Button::new("Move right")
                    .enabled(index + 1 < widgets_for_right.len())
                    .on_click(move || {
                        if index + 1 >= widgets_for_right.len() {
                            return;
                        }
                        let mut next = widgets_for_right.clone();
                        next.swap(index, index + 1);
                        persist_tray_widgets(
                            right_setter.clone(),
                            right_tx.clone(),
                            next,
                            &providers_for_right,
                        );
                    }),
            ))
            .spacing(8.0)
            .into(),
        );
        fields.push(
            Button::new(format!("Remove widget {}", index + 1))
                .on_click(move || {
                    let mut next = widgets_for_remove.clone();
                    next.remove(index);
                    persist_tray_widgets(
                        remove_setter.clone(),
                        remove_tx.clone(),
                        next,
                        &providers_for_remove,
                    );
                })
                .into(),
        );
        cards.push(
            border(vstack(fields).spacing(8.0))
                .padding(Thickness::uniform(16.0))
                .background(ThemeRef::CardBackground)
                .corner_radius(8.0)
                .border_thickness(Thickness::uniform(1.0))
                .border_brush(ThemeRef::CardStroke)
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .with_key(format!("tray-widget-{index}"))
                .into(),
        );
    }
    let add_setter = set_widgets;
    let widgets_for_add = widgets.to_vec();
    let providers_for_add = enabled_providers.to_vec();
    cards.push(
        Button::new("Add tray widget")
            .accent()
            .on_click(move || {
                let mut next = widgets_for_add.clone();
                next.push(TrayWidget::default_user_widget());
                persist_tray_widgets(
                    add_setter.clone(),
                    settings_tx.clone(),
                    next,
                    &providers_for_add,
                );
            })
            .with_key("tray-add-widget")
            .into(),
    );
    cards
}

#[cfg(any())]
fn source_index(source: &TraySource) -> i32 {
    match source {
        TraySource::Combined => 0,
        TraySource::Primary => 1,
        TraySource::Secondary => 2,
        TraySource::PrimaryReset => 3,
    }
}

#[cfg(any())]
fn source_from_index(index: i32) -> TraySource {
    match index {
        1 => TraySource::Primary,
        2 => TraySource::Secondary,
        3 => TraySource::PrimaryReset,
        _ => TraySource::Combined,
    }
}

#[cfg(any())]
fn presentation_options(source: &TraySource) -> Vec<(&'static str, TrayPresentation)> {
    match source {
        TraySource::Combined => vec![
            ("Two numbers", TrayPresentation::StackedNumbers),
            ("Two progress bars", TrayPresentation::StackedBars),
            ("Nested rings", TrayPresentation::NestedRings),
        ],
        TraySource::Primary | TraySource::Secondary => vec![
            ("Number", TrayPresentation::Number),
            ("Progress bar", TrayPresentation::Bar),
            ("Ring", TrayPresentation::Ring),
        ],
        TraySource::PrimaryReset => vec![
            ("Reset time", TrayPresentation::ResetTime),
            ("Time remaining", TrayPresentation::ResetCountdown),
        ],
    }
}

#[cfg(any())]
fn default_presentation(source: &TraySource) -> TrayPresentation {
    presentation_options(source)[0].1.clone()
}

fn persist_tray_widgets(
    setter: SetState<Vec<TrayWidget>>,
    settings_tx: Sender<Settings>,
    widgets: Vec<TrayWidget>,
    _enabled_providers: &[ProviderKind],
) {
    let mut widgets = widgets;
    for widget in &mut widgets {
        widget.normalize();
    }
    setter.call(widgets.clone());
    persist_update(settings_tx, move |settings| settings.tray_widgets = widgets);
}

fn enabled_providers(
    order: &[ProviderKind],
    codex_enabled: bool,
    claude_enabled: bool,
    cursor_enabled: bool,
    opencode_zen_enabled: bool,
    opencode_go_enabled: bool,
    openrouter_enabled: bool,
) -> Vec<ProviderKind> {
    order
        .iter()
        .copied()
        .filter(|provider| match provider {
            ProviderKind::Codex => codex_enabled,
            ProviderKind::Claude => claude_enabled,
            ProviderKind::Cursor => cursor_enabled,
            ProviderKind::OpenCodeZen => opencode_zen_enabled,
            ProviderKind::OpenCodeGo => opencode_go_enabled,
            ProviderKind::OpenRouter => openrouter_enabled,
        })
        .filter(|provider| {
            !crate::provider_registry::descriptor(*provider)
                .default_tray_metrics
                .is_empty()
        })
        .collect()
}

#[cfg(any())]
fn normalize_tray_widget_providers(
    mut widgets: Vec<TrayWidget>,
    enabled_providers: &[ProviderKind],
) -> Vec<TrayWidget> {
    if let [provider] = enabled_providers {
        for widget in &mut widgets {
            widget.provider = *provider;
        }
    }
    widgets
}

fn persist_provider_enabled(
    setter: SetState<bool>,
    widgets_setter: SetState<Vec<TrayWidget>>,
    settings_tx: Sender<Settings>,
    provider: ProviderKind,
    enabled: bool,
    other_provider_enabled: bool,
    cursor_enabled: bool,
    widgets: Vec<TrayWidget>,
) {
    setter.call(enabled);
    let _ = (other_provider_enabled, cursor_enabled);
    widgets_setter.call(widgets);
    persist_update(settings_tx, move |settings| {
        settings.providers.set_enabled(provider, enabled);
    });
}

fn persist_cursor_enabled(
    setter: SetState<bool>,
    widgets_setter: SetState<Vec<TrayWidget>>,
    settings_tx: Sender<Settings>,
    enabled: bool,
    codex_enabled: bool,
    claude_enabled: bool,
    widgets: Vec<TrayWidget>,
) {
    setter.call(enabled);
    let _ = (codex_enabled, claude_enabled);
    widgets_setter.call(widgets);
    persist_update(settings_tx, move |settings| {
        settings
            .providers
            .set_enabled(ProviderKind::Cursor, enabled);
    });
}

fn persist_popup_brick(
    current: &PopupVisibility,
    set_popup_visibility: SetState<PopupVisibility>,
    settings_tx: Sender<Settings>,
    brick_id: String,
    all_tab: bool,
    provider_tab: bool,
) {
    let mut next = current.clone();
    next.set_brick(brick_id.clone(), all_tab, provider_tab);
    set_popup_visibility.call(next);
    persist_update(settings_tx, move |settings| {
        settings.popup_visibility.set_brick(brick_id, all_tab, provider_tab);
    });
}

fn persist_popup_provider_all(
    current: &PopupVisibility,
    set_popup_visibility: SetState<PopupVisibility>,
    settings_tx: Sender<Settings>,
    provider: ProviderKind,
    show_on_all: bool,
) {
    let mut next = current.clone();
    next.set_provider_all_tab(provider, show_on_all);
    set_popup_visibility.call(next);
    persist_update(settings_tx, move |settings| {
        settings
            .popup_visibility
            .set_provider_all_tab(provider, show_on_all);
    });
}

fn popup_settings_cards(
    popup_visibility: &PopupVisibility,
    discovered_popup_bricks: &BTreeMap<String, String>,
    popup_order: &[PopupWidgetKind],
    enabled_providers: &[ProviderKind],
    show_total_spend_on_all_tab: bool,
    total_spend_presentation: TotalSpendPresentation,
    expanded_popup_provider: &Option<String>,
    set_expanded_popup_provider: SetState<Option<String>>,
    set_popup_visibility: SetState<PopupVisibility>,
    set_show_total_spend_on_all_tab: SetState<bool>,
    set_total_spend_presentation: SetState<TotalSpendPresentation>,
    hovered_card_id: &Option<String>,
    set_hovered_card_id: SetState<Option<String>>,
    settings_tx: Sender<Settings>,
) -> Vec<Element> {
    let apply_show_total_spend = settings_tx.clone();
    let apply_total_spend_presentation = settings_tx.clone();
    let mut rows = vec![
        settings_section_heading("Home tab").with_key("popup-home-tab-heading"),
        settings_toggle_card_with_description(
            "Show total spend",
            Some("Shows the provider spend breakdown when Home is selected."),
            show_total_spend_on_all_tab,
            move |value| {
                persist_bool(
                    set_show_total_spend_on_all_tab.clone(),
                    apply_show_total_spend.clone(),
                    value,
                    |settings, value| {
                        settings.show_total_spend_on_all_tab = value;
                    },
                );
            },
            "popup-show-total-spend",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("popup-show-total-spend"),
        settings_control_card(
            "Total spend layout",
            Some("Choose how provider totals are arranged in the Home tab."),
            ComboBox::new(["Donut", "Progress bar"])
                .selected_index(total_spend_presentation.index())
                .on_selection_changed({
                    let apply_total_spend_presentation = apply_total_spend_presentation.clone();
                    move |choice| {
                        let value = TotalSpendPresentation::from_index(choice);
                        set_total_spend_presentation.call(value);
                        persist_update(apply_total_spend_presentation.clone(), move |settings| {
                            settings.total_spend_presentation = value;
                        });
                    }
                }),
            "popup-total-spend-layout",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("popup-total-spend-layout"),
        settings_section_heading("Provider cards").with_key("popup-provider-cards-heading"),
    ];

    let ordered_enabled: Vec<ProviderKind> = popup_order
        .iter()
        .filter_map(|widget| widget.as_provider())
        .filter(|provider| enabled_providers.contains(provider))
        .collect();

    if ordered_enabled.is_empty() {
        rows.push(
            settings_info_card(
                "Popup cards",
                "Enable a provider to configure its popup cards.",
            )
            .with_key("popup-empty"),
        );
    }

    for provider in ordered_enabled {
        let descriptor = crate::provider_registry::descriptor(provider);
        let provider_id = provider.id().to_string();
        let is_expanded = expanded_popup_provider.as_deref() == Some(provider_id.as_str());
        let expand_id = provider_id.clone();
        let expand_setter = set_expanded_popup_provider.clone();
        let section_all = popup_visibility.provider_shown_on_all(provider);
        let mut brick_rows = vec![settings_brick_table_header(provider.id())];

        let extra_ids = popup_visibility
            .bricks
            .keys()
            .chain(discovered_popup_bricks.keys())
            .cloned()
            .collect::<Vec<_>>();
        for brick_id in crate::provider_registry::settings_brick_ids(provider, &extra_ids) {
            let snapshot_all = popup_visibility.clone();
            let snapshot_tab = popup_visibility.clone();
            let visibility = snapshot_all.visibility_for(&brick_id);
            let label = crate::provider_registry::settings_brick_label(
                provider,
                &brick_id,
                discovered_popup_bricks,
            );
            let brick_id_for_all = brick_id.clone();
            let brick_id_for_tab = brick_id.clone();
            let set_visibility_all = set_popup_visibility.clone();
            let set_visibility_tab = set_popup_visibility.clone();
            let settings_tx_all = settings_tx.clone();
            let settings_tx_tab = settings_tx.clone();
            brick_rows.push(settings_brick_row(
                label,
                visibility.all_tab,
                visibility.provider_tab,
                section_all,
                move |all_tab| {
                    let provider_tab = snapshot_all.visibility_for(&brick_id_for_all).provider_tab;
                    persist_popup_brick(
                        &snapshot_all,
                        set_visibility_all.clone(),
                        settings_tx_all.clone(),
                        brick_id_for_all.clone(),
                        all_tab,
                        provider_tab,
                    );
                },
                move |provider_tab| {
                    let all_tab = snapshot_tab.visibility_for(&brick_id_for_tab).all_tab;
                    persist_popup_brick(
                        &snapshot_tab,
                        set_visibility_tab.clone(),
                        settings_tx_tab.clone(),
                        brick_id_for_tab.clone(),
                        all_tab,
                        provider_tab,
                    );
                },
                &format!("{}-{}", provider.id(), brick_id),
            ));
        }

        let section_snapshot = popup_visibility.clone();
        let set_section = set_popup_visibility.clone();
        let section_tx = settings_tx.clone();
        let expanded_body_height = Some(settings_brick_body_height(brick_rows.len()));
        rows.push(
            settings_checkbox_expander(
                descriptor.display_name,
                section_all,
                move |show_on_all| {
                    persist_popup_provider_all(
                        &section_snapshot,
                        set_section.clone(),
                        section_tx.clone(),
                        provider,
                        show_on_all,
                    );
                },
                is_expanded,
                move |expanded| {
                    if expanded {
                        expand_setter.call(Some(expand_id.clone()));
                    } else {
                        expand_setter.call(None);
                    }
                },
                expanded_body_height,
                format!("popup-provider-{}", provider.id()),
                hovered_card_id,
                set_hovered_card_id.clone(),
                vstack(brick_rows).spacing(0.0),
            )
            .with_key(format!("popup-provider-{}", provider.id())),
        );
    }

    rows
}

fn persist_bool(
    setter: SetState<bool>,
    settings_tx: Sender<Settings>,
    value: bool,
    update: impl FnOnce(&mut Settings, bool),
) {
    setter.call(value);
    persist_update(settings_tx, |settings| update(settings, value));
}

fn persist_u8(
    setter: SetState<u8>,
    settings_tx: Sender<Settings>,
    value: u8,
    update: impl FnOnce(&mut Settings, u8),
) {
    setter.call(value);
    persist_update(settings_tx, |settings| update(settings, value));
}

pub(crate) fn persist_update(settings_tx: Sender<Settings>, update: impl FnOnce(&mut Settings)) {
    let result = Settings::default_path().and_then(|path| {
        let mut settings = Settings::load_or_create(&path)?;
        update(&mut settings);
        settings.normalize_tray_widgets();
        settings.normalize_popup_visibility();
        // Persist first so a flaky side effect cannot block live UI updates.
        settings.save(&path)?;
        if let Err(error) = settings.apply_runtime_effects() {
            eprintln!("failed to apply runtime settings effects: {error:#}");
        }
        settings_tx
            .send(settings)
            .context("notify live settings listeners")?;
        Ok(())
    });
    if let Err(error) = result {
        eprintln!("failed to save settings: {error:#}");
    }
}

fn replace_settings(settings_tx: Sender<Settings>, mut settings: Settings) -> anyhow::Result<()> {
    let path = Settings::default_path()?;
    settings.normalize_tray_widgets();
    settings.save(&path)?;
    if let Err(error) = settings.apply_runtime_effects() {
        eprintln!("failed to apply runtime settings effects: {error:#}");
    }
    settings_tx
        .send(settings)
        .context("notify live settings listeners")?;
    Ok(())
}

fn export_settings() -> anyhow::Result<()> {
    let Some(path) = choose_settings_file(true)? else {
        return Ok(());
    };
    let current_path = Settings::default_path()?;
    Settings::load_or_create(&current_path)?.save(&path)
}

fn import_settings() -> anyhow::Result<Option<Settings>> {
    let Some(path) = choose_settings_file(false)? else {
        return Ok(None);
    };
    Settings::load_or_create(&path).map(Some)
}

#[cfg(windows)]
fn choose_settings_file(save: bool) -> anyhow::Result<Option<PathBuf>> {
    use windows_sys::Win32::UI::Controls::Dialogs::{
        GetOpenFileNameW, GetSaveFileNameW, OFN_FILEMUSTEXIST, OFN_OVERWRITEPROMPT,
        OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };

    let mut filename = "codex-minibar-settings.toml"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    filename.resize(32_768, 0);
    let filter = "Codex Minibar settings (*.toml)\0*.toml\0\0"
        .encode_utf16()
        .collect::<Vec<_>>();
    let default_extension = "toml\0".encode_utf16().collect::<Vec<_>>();
    let title = if save {
        "Export settings\0"
    } else {
        "Import settings\0"
    }
    .encode_utf16()
    .collect::<Vec<_>>();
    let mut dialog: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    dialog.lpstrFilter = filter.as_ptr();
    dialog.lpstrFile = filename.as_mut_ptr();
    dialog.nMaxFile = filename.len() as u32;
    dialog.lpstrTitle = title.as_ptr();
    dialog.lpstrDefExt = default_extension.as_ptr();
    dialog.Flags = OFN_PATHMUSTEXIST
        | if save {
            OFN_OVERWRITEPROMPT
        } else {
            OFN_FILEMUSTEXIST
        };

    let accepted = unsafe {
        if save {
            GetSaveFileNameW(&mut dialog)
        } else {
            GetOpenFileNameW(&mut dialog)
        }
    } != 0;
    if !accepted {
        return Ok(None);
    }
    let length = filename.iter().position(|&unit| unit == 0).unwrap_or(0);
    Ok(Some(PathBuf::from(String::from_utf16(
        &filename[..length],
    )?)))
}

#[cfg(not(windows))]
fn choose_settings_file(_save: bool) -> anyhow::Result<Option<PathBuf>> {
    anyhow::bail!("settings import and export are only available on Windows")
}

#[cfg(windows)]
fn choose_provider_folder() -> anyhow::Result<Option<PathBuf>> {
    use windows_sys::Win32::UI::Shell::{
        BIF_EDITBOX, BIF_NEWDIALOGSTYLE, BIF_RETURNONLYFSDIRS, BROWSEINFOW, ILFree,
        SHBrowseForFolderW, SHGetPathFromIDListW,
    };

    let title = "Choose the folder containing the provider files\0"
        .encode_utf16()
        .collect::<Vec<_>>();
    let mut display_name = [0_u16; 260];
    let mut dialog: BROWSEINFOW = unsafe { std::mem::zeroed() };
    dialog.pszDisplayName = display_name.as_mut_ptr();
    dialog.lpszTitle = title.as_ptr();
    dialog.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE | BIF_EDITBOX;

    let item_id_list = unsafe { SHBrowseForFolderW(&dialog) };
    if item_id_list.is_null() {
        return Ok(None);
    }

    let mut path = [0_u16; 32_768];
    let found_path = unsafe { SHGetPathFromIDListW(item_id_list, path.as_mut_ptr()) } != 0;
    unsafe { ILFree(item_id_list) };
    if !found_path {
        return Ok(None);
    }

    let length = path.iter().position(|&unit| unit == 0).unwrap_or(0);
    Ok(Some(PathBuf::from(String::from_utf16(&path[..length])?)))
}

#[cfg(not(windows))]
fn choose_provider_folder() -> anyhow::Result<Option<PathBuf>> {
    anyhow::bail!("provider folder selection is only available on Windows")
}

#[cfg(windows)]
fn confirm_settings_reset() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IDYES, MB_ICONWARNING, MB_YESNO, MessageBoxW,
    };

    let message = "Reset all Codex Minibar settings to their defaults?\0"
        .encode_utf16()
        .collect::<Vec<_>>();
    let title = "Reset settings\0".encode_utf16().collect::<Vec<_>>();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_YESNO | MB_ICONWARNING,
        ) == IDYES
    }
}

#[cfg(not(windows))]
fn confirm_settings_reset() -> bool {
    false
}
