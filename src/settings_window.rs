//! Settings-window entry point.
//!
//! The host is exposed here so callers do not depend on popup implementation
//! details; both surfaces share tokens from [`crate::theme`].

use crate::notifications;
#[cfg(any())]
use crate::settings::TraySource;
use crate::settings::{
    AccentColor, AppTheme, LimitRefreshInterval, LimitValue, PopupWidgetKind, ProviderKind,
    Settings, TotalSpendPresentation, TrayColorMode, TrayFixedColor, TrayIndicator,
    TrayPresentation, TrayWidget, TrayWidgetKind,
};
use crate::settings_controls::{
    settings_action_card, settings_content_expander, settings_control_card, settings_info_card,
    settings_slider_content, settings_toggle_card, settings_toggle_card_with_description,
    settings_toggle_expander, update_available_nav_card,
};
use crate::theme::{CONTROL_FAST_ANIMATION, duration};
use crate::updater::{
    ISSUES_URL, RELEASES_URL, REPO_URL, UpdateController, UpdatePhase, current_version,
};
use anyhow::Context;
use std::{
    cell::RefCell,
    collections::HashMap,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, mpsc::Sender},
    thread,
    time::Duration,
};
use windows_reactor::*;

const WINDOW_WIDTH: f64 = 760.0;
const WINDOW_HEIGHT: f64 = 520.0;
const SETTINGS_WINDOW_TITLE: &str = "Codex Minibar Settings";
const ONBOARDING_WINDOW_TITLE: &str = "Welcome to Codex Minibar";

thread_local! {
    static HOST: RefCell<Option<Rc<ReactorHost>>> = const { RefCell::new(None) };
    static LIVE_SETTINGS_STATE: RefCell<Option<SettingsWindowState>> = const { RefCell::new(None) };
    static TRAY_PREVIEW_MOUNTS: RefCell<HashMap<String, windows_core::IInspectable>> =
        RefCell::new(HashMap::new());
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

#[derive(Clone)]
struct SettingsWindowState {
    theme: SetState<AppTheme>,
    accent_color: SetState<AccentColor>,
    animations_enabled: SetState<bool>,
    codex_enabled: SetState<bool>,
    claude_enabled: SetState<bool>,
    cursor_enabled: SetState<bool>,
    popup_order: SetState<Vec<PopupWidgetKind>>,
    use_colored_provider_icons: SetState<bool>,
    replace_chatgpt_logo_with_codex: SetState<bool>,
    automatic_activation: SetState<bool>,
    limit_refresh_interval: SetState<LimitRefreshInterval>,
    start_at_login: SetState<bool>,
    show_used_percentage: SetState<bool>,
    show_usage_pace: SetState<bool>,
    show_banked_resets: SetState<bool>,
    show_usage_stats: SetState<bool>,
    show_total_spend_on_all_tab: SetState<bool>,
    total_spend_presentation: SetState<TotalSpendPresentation>,
    show_account_name: SetState<bool>,
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
        self.codex_enabled
            .call(settings.providers.is_enabled(ProviderKind::Codex));
        self.claude_enabled
            .call(settings.providers.is_enabled(ProviderKind::Claude));
        self.cursor_enabled
            .call(settings.providers.is_enabled(ProviderKind::Cursor));
        self.popup_order.call(settings.popup_order.clone());
        self.use_colored_provider_icons
            .call(settings.use_colored_provider_icons);
        self.replace_chatgpt_logo_with_codex
            .call(settings.replace_chatgpt_logo_with_codex);
        self.automatic_activation
            .call(settings.automatic_activation);
        self.limit_refresh_interval
            .call(settings.limit_refresh_interval);
        self.start_at_login.call(settings.start_at_login);
        self.show_used_percentage
            .call(settings.show_used_percentage);
        self.show_usage_pace.call(settings.show_usage_pace);
        self.show_banked_resets.call(settings.show_banked_resets);
        self.show_usage_stats.call(settings.show_usage_stats);
        self.show_total_spend_on_all_tab
            .call(settings.show_total_spend_on_all_tab);
        self.total_spend_presentation
            .call(settings.total_spend_presentation);
        self.show_account_name.call(settings.show_account_name);
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
                onboarding_render(
                    cx,
                    Arc::clone(&settings),
                    detected.clone(),
                    settings_tx.clone(),
                )
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

fn detected_providers(settings: &Settings) -> [bool; 3] {
    [
        crate::codex::is_installed(settings.codex_path.as_deref()),
        crate::claude::is_installed(),
        crate::cursor::is_installed(),
    ]
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
    detected: [bool; 3],
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
    let (start_at_login, set_start_at_login) = cx.use_state(settings.start_at_login);
    let (automatic_activation, set_automatic_activation) =
        cx.use_state(settings.automatic_activation);
    let (limit_refresh_interval, set_limit_refresh_interval) =
        cx.use_state(settings.limit_refresh_interval);
    let (show_used_percentage, set_show_used_percentage) =
        cx.use_state(settings.show_used_percentage);
    let (show_usage_pace, set_show_usage_pace) = cx.use_state(settings.show_usage_pace);
    let (show_banked_resets, set_show_banked_resets) = cx.use_state(settings.show_banked_resets);
    let (show_usage_stats, set_show_usage_stats) = cx.use_state(settings.show_usage_stats);
    let (show_total_spend_on_all_tab, set_show_total_spend_on_all_tab) =
        cx.use_state(settings.show_total_spend_on_all_tab);
    let (total_spend_presentation, set_total_spend_presentation) =
        cx.use_state(settings.total_spend_presentation);
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
                    "Show banked resets",
                    Some("Shows available banked reset credits in the popup."),
                    show_banked_resets,
                    move |value| set_show_banked_resets.call(value),
                    "onboarding-show-banked-resets",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-show-banked-resets"),
                settings_toggle_card_with_description(
                    "Show usage stats",
                    Some("Shows local token activity and the usage chart in the popup."),
                    show_usage_stats,
                    move |value| set_show_usage_stats.call(value),
                    "onboarding-show-usage-stats",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-show-usage-stats"),
                if [codex_enabled, claude_enabled, cursor_enabled]
                    .into_iter()
                    .filter(|enabled| *enabled)
                    .count()
                    > 1
                {
                    settings_toggle_card_with_description(
                        "Show total spend in All tab",
                        Some("Shows the provider spend breakdown when All is selected."),
                        show_total_spend_on_all_tab,
                        move |value| set_show_total_spend_on_all_tab.call(value),
                        "onboarding-show-total-spend",
                        &hovered_card_id,
                        set_hovered_card_id.clone(),
                    )
                    .with_key("onboarding-show-total-spend")
                } else {
                    Element::Empty
                },
                if [codex_enabled, claude_enabled, cursor_enabled]
                    .into_iter()
                    .filter(|enabled| *enabled)
                    .count()
                    > 1
                {
                    settings_control_card(
                        "Total spend layout",
                        Some("Choose how provider totals are arranged in the All tab."),
                        ComboBox::new(["Donut", "Progress bar"])
                            .selected_index(total_spend_presentation.index())
                            .on_selection_changed(move |choice| {
                                set_total_spend_presentation
                                    .call(TotalSpendPresentation::from_index(choice));
                            }),
                        "onboarding-total-spend-layout",
                        &hovered_card_id,
                        set_hovered_card_id.clone(),
                    )
                    .with_key("onboarding-total-spend-layout")
                } else {
                    Element::Empty
                },
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
                            })
                            .map(|provider| provider.kind),
                    );
                    completed.tray_widgets = crate::provider_registry::PROVIDERS
                        .iter()
                        .filter(|provider| completed.providers.is_enabled(provider.kind))
                        .map(|provider| TrayWidget::for_provider(provider.kind))
                        .collect();
                    completed.start_at_login = start_at_login;
                    completed.automatic_activation = automatic_activation;
                    completed.limit_refresh_interval = limit_refresh_interval;
                    completed.show_used_percentage = show_used_percentage;
                    completed.show_usage_pace = show_usage_pace;
                    completed.show_banked_resets = show_banked_resets;
                    completed.show_usage_stats = show_usage_stats;
                    completed.show_total_spend_on_all_tab = show_total_spend_on_all_tab;
                    completed.total_spend_presentation = total_spend_presentation;
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
    Tray,
    Notifications,
    Advanced,
    About,
}

impl Tab {
    fn tag(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Appearance => "appearance",
            Self::Providers => "providers",
            Self::Tray => "tray",
            Self::Notifications => "notifications",
            Self::Advanced => "advanced",
            Self::About => "about",
        }
    }

    fn from_tag(tag: &str) -> Self {
        match tag {
            "appearance" => Self::Appearance,
            "tray" => Self::Tray,
            "providers" => Self::Providers,
            "notifications" => Self::Notifications,
            "advanced" => Self::Advanced,
            "about" => Self::About,
            _ => Self::General,
        }
    }
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
    let (selected, set_selected) = cx.use_state(Tab::default());
    let (rendered_tab, set_rendered_tab) = cx.use_async_state(Tab::default());
    let (page_visible, set_page_visible) = cx.use_async_state(true);
    let theme_navigation_guard = cx.use_ref(false);
    let theme_navigation_guard_timer = cx.use_ref(None::<DispatcherTimer>);

    let nav_icon_color = match color_scheme {
        ColorScheme::Dark => "#E6E6E6",
        ColorScheme::Light => "#3A3A3A",
    };
    let mut navigation = NavigationView::new(
        [
            NavViewItem::new("General")
                .tag("general")
                .icon_path(crate::icons::data("house"), nav_icon_color),
            NavViewItem::new("Providers")
                .tag("providers")
                .icon_path(crate::icons::data("plugs-connected"), nav_icon_color),
            NavViewItem::new("Tray")
                .tag("tray")
                .icon_path(crate::icons::data("chat-centered-text"), nav_icon_color),
            NavViewItem::new("Notifications")
                .tag("notifications")
                .icon_path(crate::icons::data("bell"), nav_icon_color),
            NavViewItem::new("Appearance")
                .tag("appearance")
                .icon_path(crate::icons::data("paint-brush"), nav_icon_color),
            NavViewItem::new("Advanced")
                .tag("advanced")
                .icon_path(crate::icons::data("sliders"), nav_icon_color),
            NavViewItem::new("About & Updates")
                .tag("about")
                .icon_path(crate::icons::data("info"), nav_icon_color),
        ],
        Element::Empty,
    )
    .selected_tag(selected.tag())
    .on_selection_changed({
        let set_rendered_tab = set_rendered_tab.clone();
        let set_page_visible = set_page_visible.clone();
        let theme_navigation_guard = theme_navigation_guard.clone();
        move |tag: String| {
            let next = Tab::from_tag(&tag);
            if theme_navigation_guard.get_cloned() && next != selected {
                return;
            }
            if next != selected {
                set_page_visible.call(false);
                set_selected.call(next);
                let set_rendered_tab = set_rendered_tab.clone();
                let set_page_visible = set_page_visible.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(duration(Duration::from_millis(180)));
                    set_rendered_tab.call(next);
                    set_page_visible.call(true);
                });
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
    if let UpdatePhase::Available(update) = &update_phase {
        let version = update.version.clone();
        navigation = navigation.pane_footer(
            border(update_available_nav_card(version, || {
                if let Err(error) = crate::updater::apply_pending_update() {
                    eprintln!("failed to apply update: {error:#}");
                    notifications::show("Update failed", &format!("{error:#}"));
                }
            }))
            // L/R inset is ours; PaneFooter already reserves bottom chrome,
            // so keep bottom padding at 0 or the card floats too high off the edge.
            .padding(Thickness {
                left: 12.0,
                top: 0.0,
                right: 12.0,
                bottom: 2.0,
            })
            .background(Color::transparent()),
        );
    }

    let (codex_enabled, set_codex_enabled) =
        cx.use_state(settings.providers.is_enabled(ProviderKind::Codex));
    let (theme, set_theme) = cx.use_state(settings.theme);
    let (accent_color, set_accent_color) = cx.use_state(settings.accent_color);
    let (animations_enabled, set_animations_enabled) = cx.use_state(settings.animations_enabled);
    cx.use_effect((theme, accent_color, animations_enabled), move || {
        crate::theme::set_animations_enabled(animations_enabled);
        crate::theme::apply_appearance(theme, accent_color);
    });
    let (claude_enabled, set_claude_enabled) =
        cx.use_state(settings.providers.is_enabled(ProviderKind::Claude));
    let (cursor_enabled, set_cursor_enabled) =
        cx.use_state(settings.providers.is_enabled(ProviderKind::Cursor));
    let (popup_order, set_popup_order) = cx.use_state(settings.popup_order.clone());
    let (use_colored_provider_icons, set_use_colored_provider_icons) =
        cx.use_state(settings.use_colored_provider_icons);
    let (replace_chatgpt_logo_with_codex, set_replace_chatgpt_logo_with_codex) =
        cx.use_state(settings.replace_chatgpt_logo_with_codex);
    let (start_at_login, set_start_at_login) = cx.use_state(settings.start_at_login);
    let (automatic_activation, set_automatic_activation) =
        cx.use_state(settings.automatic_activation);
    let (limit_refresh_interval, set_limit_refresh_interval) =
        cx.use_state(settings.limit_refresh_interval);
    let (show_used_percentage, set_show_used_percentage) =
        cx.use_state(settings.show_used_percentage);
    let (show_usage_pace, set_show_usage_pace) = cx.use_state(settings.show_usage_pace);
    let (show_banked_resets, set_show_banked_resets) = cx.use_state(settings.show_banked_resets);
    let (show_usage_stats, set_show_usage_stats) = cx.use_state(settings.show_usage_stats);
    let (show_total_spend_on_all_tab, set_show_total_spend_on_all_tab) =
        cx.use_state(settings.show_total_spend_on_all_tab);
    let (total_spend_presentation, set_total_spend_presentation) =
        cx.use_state(settings.total_spend_presentation);
    let (show_account_name, set_show_account_name) = cx.use_state(settings.show_account_name);
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
    let (removed_tray_widget, set_removed_tray_widget) = cx.use_state(None::<(usize, TrayWidget)>);
    let (check_for_updates, set_check_for_updates) = cx.use_state(settings.check_for_updates);
    let (notify_on_update, set_notify_on_update) =
        cx.use_state(settings.notifications.update_available);

    LIVE_SETTINGS_STATE.with(|state| {
        *state.borrow_mut() = Some(SettingsWindowState {
            theme: set_theme.clone(),
            accent_color: set_accent_color.clone(),
            animations_enabled: set_animations_enabled.clone(),
            codex_enabled: set_codex_enabled.clone(),
            claude_enabled: set_claude_enabled.clone(),
            cursor_enabled: set_cursor_enabled.clone(),
            popup_order: set_popup_order.clone(),
            use_colored_provider_icons: set_use_colored_provider_icons.clone(),
            replace_chatgpt_logo_with_codex: set_replace_chatgpt_logo_with_codex.clone(),
            automatic_activation: set_automatic_activation.clone(),
            limit_refresh_interval: set_limit_refresh_interval.clone(),
            start_at_login: set_start_at_login.clone(),
            show_used_percentage: set_show_used_percentage.clone(),
            show_usage_pace: set_show_usage_pace.clone(),
            show_banked_resets: set_show_banked_resets.clone(),
            show_usage_stats: set_show_usage_stats.clone(),
            show_total_spend_on_all_tab: set_show_total_spend_on_all_tab.clone(),
            total_spend_presentation: set_total_spend_presentation.clone(),
            show_account_name: set_show_account_name.clone(),
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

    // Padding lives on tab content (inside the scroller), not on this pane, so
    // LayerFill crops flush to the window edge while long tabs stay scrollable.
    let page_scroller = scroll_viewer(
        border(tab_content(
            rendered_tab,
            theme,
            accent_color,
            animations_enabled,
            codex_enabled,
            claude_enabled,
            cursor_enabled,
            &popup_order,
            use_colored_provider_icons,
            replace_chatgpt_logo_with_codex,
            automatic_activation,
            limit_refresh_interval,
            start_at_login,
            show_used_percentage,
            show_usage_pace,
            show_banked_resets,
            show_usage_stats,
            show_total_spend_on_all_tab,
            total_spend_presentation,
            show_account_name,
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
            &removed_tray_widget,
            &hovered_card_id,
            check_for_updates,
            notify_on_update,
            &update_phase,
            set_codex_enabled,
            set_theme,
            set_accent_color,
            set_animations_enabled,
            set_claude_enabled,
            set_cursor_enabled,
            set_popup_order,
            set_use_colored_provider_icons,
            set_replace_chatgpt_logo_with_codex,
            set_automatic_activation,
            set_limit_refresh_interval,
            set_start_at_login,
            set_show_used_percentage,
            set_show_usage_pace,
            set_show_banked_resets,
            set_show_usage_stats,
            set_show_total_spend_on_all_tab,
            set_total_spend_presentation,
            set_show_account_name,
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
            set_tray_widgets,
            set_expanded_tray_widget,
            set_removed_tray_widget,
            set_hovered_card_id,
            set_check_for_updates,
            set_notify_on_update,
            theme_navigation_guard,
            theme_navigation_guard_timer,
            settings_tx.clone(),
            ui_dispatcher.clone(),
            updates.clone(),
        ))
        .padding(Thickness {
            left: 32.0,
            top: 24.0,
            right: 32.0,
            bottom: 32.0,
        })
        .with_key(format!("settings-page-{}", rendered_tab.tag()))
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Top),
    )
    // Keys are honored only in multi-child containers by windows-reactor.
    // The Grid below therefore remounts this native ScrollViewer on every
    // rendered-tab change, guaranteeing a fresh zero scroll offset.
    .with_key(format!("settings-scroll-{}", rendered_tab.tag()))
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
    let title_bar = TitleBar::new("Codex Minibar Settings")
        .content(title_bar_icon)
        .back_button_visible(false)
        .pane_toggle_button_visible(false)
        // Tall caption buttons so min/max/close fill the TitleBar height.
        .tall(true);
    let shell = grid((navigation.grid_column(0), page.grid_column(1)))
        .columns([GridLength::Pixel(220.0), GridLength::Star(1.0)])
        .rows([GridLength::Star(1.0)])
        .background(Color::transparent());
    let mica = {
        let mut host = swap_chain_panel()
            .grid_row_span(2)
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
    grid((mica, title_bar.grid_row(0), shell.grid_row(1)))
        .rows([GridLength::Auto, GridLength::Star(1.0)])
        .columns([GridLength::Star(1.0)])
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

fn tab_content(
    tab: Tab,
    theme: AppTheme,
    accent_color: AccentColor,
    animations_enabled: bool,
    codex_enabled: bool,
    claude_enabled: bool,
    cursor_enabled: bool,
    popup_order: &[PopupWidgetKind],
    use_colored_provider_icons: bool,
    replace_chatgpt_logo_with_codex: bool,
    automatic_activation: bool,
    limit_refresh_interval: LimitRefreshInterval,
    start_at_login: bool,
    show_used_percentage: bool,
    show_usage_pace: bool,
    show_banked_resets: bool,
    show_usage_stats: bool,
    show_total_spend_on_all_tab: bool,
    total_spend_presentation: TotalSpendPresentation,
    show_account_name: bool,
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
    removed_tray_widget: &Option<(usize, TrayWidget)>,
    hovered_card_id: &Option<String>,
    check_for_updates: bool,
    notify_on_update: bool,
    update_phase: &UpdatePhase,
    set_codex_enabled: SetState<bool>,
    set_theme: SetState<AppTheme>,
    set_accent_color: SetState<AccentColor>,
    set_animations_enabled: SetState<bool>,
    set_claude_enabled: SetState<bool>,
    set_cursor_enabled: SetState<bool>,
    set_popup_order: SetState<Vec<PopupWidgetKind>>,
    set_use_colored_provider_icons: SetState<bool>,
    set_replace_chatgpt_logo_with_codex: SetState<bool>,
    set_automatic_activation: SetState<bool>,
    set_limit_refresh_interval: SetState<LimitRefreshInterval>,
    set_start_at_login: SetState<bool>,
    set_show_used_percentage: SetState<bool>,
    set_show_usage_pace: SetState<bool>,
    set_show_banked_resets: SetState<bool>,
    set_show_usage_stats: SetState<bool>,
    set_show_total_spend_on_all_tab: SetState<bool>,
    set_total_spend_presentation: SetState<TotalSpendPresentation>,
    set_show_account_name: SetState<bool>,
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
    set_removed_tray_widget: SetState<Option<(usize, TrayWidget)>>,
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
    let apply_codex_enabled = settings_tx.clone();
    let apply_claude_enabled = settings_tx.clone();
    let apply_cursor_enabled = settings_tx.clone();
    let apply_use_colored_provider_icons = settings_tx.clone();
    let apply_replace_chatgpt_logo_with_codex = settings_tx.clone();
    let apply_automatic_activation = settings_tx.clone();
    let apply_limit_refresh_interval = settings_tx.clone();
    let apply_start_at_login = settings_tx.clone();
    let apply_show_used_percentage = settings_tx.clone();
    let apply_show_usage_pace = settings_tx.clone();
    let apply_show_banked_resets = settings_tx.clone();
    let apply_show_usage_stats = settings_tx.clone();
    let apply_show_total_spend_on_all_tab = settings_tx.clone();
    let apply_total_spend_presentation = settings_tx.clone();
    let apply_show_account_name = settings_tx.clone();
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
    let tray_widgets_for_codex_toggle = tray_widgets.to_vec();
    let tray_widgets_for_claude_toggle = tray_widgets.to_vec();
    let tray_widgets_for_cursor_toggle = tray_widgets.to_vec();
    let tray_widget_setter_for_codex_toggle = set_tray_widgets.clone();
    let tray_widget_setter_for_claude_toggle = set_tray_widgets.clone();
    let tray_widget_setter_for_cursor_toggle = set_tray_widgets.clone();
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
                settings_section_heading("Customization").with_key("general-customization-heading"),
                settings_toggle_card_with_description(
                    "Replace \"amount left\" with \"amount used\"",
                    Some("Shows consumed usage instead of the remaining amount."),
                    show_used_percentage,
                    move |value| {
                        persist_bool(
                            set_show_used_percentage.clone(),
                            apply_show_used_percentage.clone(),
                            value,
                            |settings, value| {
                                settings.show_used_percentage = value;
                            },
                        );
                    },
                    "general-show-used",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("general-show-used"),
                settings_toggle_card_with_description(
                    "Show usage pace",
                    Some(
                        "Shows the expected-use marker and whether consumption is ahead of or behind schedule.",
                    ),
                    show_usage_pace,
                    move |value| {
                        persist_bool(
                            set_show_usage_pace.clone(),
                            apply_show_usage_pace.clone(),
                            value,
                            |settings, value| {
                                settings.show_usage_pace = value;
                            },
                        );
                    },
                    "general-show-usage-pace",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("general-show-usage-pace"),
                settings_toggle_card_with_description(
                    "Show banked resets",
                    Some("Shows available banked reset credits in the popup."),
                    show_banked_resets,
                    move |value| {
                        persist_bool(
                            set_show_banked_resets.clone(),
                            apply_show_banked_resets.clone(),
                            value,
                            |settings, value| {
                                settings.show_banked_resets = value;
                            },
                        );
                    },
                    "general-show-banked-resets",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("general-show-banked-resets"),
                settings_toggle_card_with_description(
                    "Show usage stats",
                    Some("Shows local token activity and the usage chart in the popup."),
                    show_usage_stats,
                    move |value| {
                        persist_bool(
                            set_show_usage_stats.clone(),
                            apply_show_usage_stats.clone(),
                            value,
                            |settings, value| {
                                settings.show_usage_stats = value;
                            },
                        );
                    },
                    "general-show-usage-stats",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("general-show-usage-stats"),
                if [codex_enabled, claude_enabled, cursor_enabled]
                    .into_iter()
                    .filter(|enabled| *enabled)
                    .count()
                    > 1
                {
                    settings_toggle_card_with_description(
                        "Show total spend in All tab",
                        Some("Shows the provider spend breakdown when All is selected."),
                        show_total_spend_on_all_tab,
                        move |value| {
                            persist_bool(
                                set_show_total_spend_on_all_tab.clone(),
                                apply_show_total_spend_on_all_tab.clone(),
                                value,
                                |settings, value| {
                                    settings.show_total_spend_on_all_tab = value;
                                },
                            );
                        },
                        "general-show-total-spend",
                        hovered_card_id,
                        set_hovered_card_id.clone(),
                    )
                    .with_key("general-show-total-spend")
                } else {
                    Element::Empty
                },
                if [codex_enabled, claude_enabled, cursor_enabled]
                    .into_iter()
                    .filter(|enabled| *enabled)
                    .count()
                    > 1
                {
                    settings_control_card(
                        "Total spend layout",
                        Some("Choose how provider totals are arranged in the All tab."),
                        ComboBox::new(["Donut", "Progress bar"])
                            .selected_index(total_spend_presentation.index())
                            .on_selection_changed(move |choice| {
                                let value = TotalSpendPresentation::from_index(choice);
                                set_total_spend_presentation.call(value);
                                persist_update(apply_total_spend_presentation.clone(), move |settings| {
                                    settings.total_spend_presentation = value;
                                });
                            }),
                        "general-total-spend-layout",
                        hovered_card_id,
                        set_hovered_card_id.clone(),
                    )
                    .with_key("general-total-spend-layout")
                } else {
                    Element::Empty
                },
                settings_toggle_card_with_description(
                    "Show account name",
                    Some("Shows your Codex name or Claude organization beside the provider heading."),
                    show_account_name,
                    move |value| {
                        persist_bool(
                            set_show_account_name.clone(),
                            apply_show_account_name.clone(),
                            value,
                            |settings, value| {
                                settings.show_account_name = value;
                            },
                        );
                    },
                    "general-show-account-name",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("general-show-account-name"),
            ],
        ),
        Tab::Appearance => (
            "Appearance",
            vec![
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
            ],
        ),
        Tab::Providers => {
            let provider_order: Vec<ProviderKind> = popup_order
                .iter()
                .filter_map(|widget| widget.as_provider())
                .collect();
            let mut rows = Vec::new();
            for provider in provider_order.iter().copied() {
                let (title, description, enabled, setter, apply_tx, other_a, other_b, tray_snapshot, tray_setter) =
                    match provider {
                        ProviderKind::Codex => (
                            "Codex",
                            Some("Reads limits from the locally signed-in Codex CLI or desktop app."),
                            codex_enabled,
                            set_codex_enabled.clone(),
                            apply_codex_enabled.clone(),
                            claude_enabled,
                            cursor_enabled,
                            tray_widgets_for_codex_toggle.clone(),
                            tray_widget_setter_for_codex_toggle.clone(),
                        ),
                        ProviderKind::Claude => (
                            "Claude",
                            Some("Reads limits with the existing signed-in Claude Code OAuth session."),
                            claude_enabled,
                            set_claude_enabled.clone(),
                            apply_claude_enabled.clone(),
                            codex_enabled,
                            cursor_enabled,
                            tray_widgets_for_claude_toggle.clone(),
                            tray_widget_setter_for_claude_toggle.clone(),
                        ),
                        ProviderKind::Cursor => (
                            "Cursor",
                            Some("Reads your signed-in Cursor desktop app session and shows the current billing-cycle usage."),
                            cursor_enabled,
                            set_cursor_enabled.clone(),
                            apply_cursor_enabled.clone(),
                            codex_enabled,
                            claude_enabled,
                            tray_widgets_for_cursor_toggle.clone(),
                            tray_widget_setter_for_cursor_toggle.clone(),
                        ),
                    };
                let toggle = match provider {
                    ProviderKind::Codex => settings_toggle_card_with_description(
                        title,
                        description,
                        enabled,
                        move |value| {
                            persist_provider_enabled(
                                setter.clone(),
                                tray_setter.clone(),
                                apply_tx.clone(),
                                ProviderKind::Codex,
                                value,
                                other_a,
                                other_b,
                                tray_snapshot.clone(),
                            );
                        },
                        "providers-codex",
                        hovered_card_id,
                        set_hovered_card_id.clone(),
                    ),
                    ProviderKind::Claude => settings_toggle_card_with_description(
                        title,
                        description,
                        enabled,
                        move |value| {
                            persist_provider_enabled(
                                setter.clone(),
                                tray_setter.clone(),
                                apply_tx.clone(),
                                ProviderKind::Claude,
                                value,
                                other_a,
                                other_b,
                                tray_snapshot.clone(),
                            );
                        },
                        "providers-claude",
                        hovered_card_id,
                        set_hovered_card_id.clone(),
                    ),
                    ProviderKind::Cursor => settings_toggle_card_with_description(
                        title,
                        description,
                        enabled,
                        move |value| {
                            persist_cursor_enabled(
                                setter.clone(),
                                tray_setter.clone(),
                                apply_tx.clone(),
                                value,
                                other_a,
                                other_b,
                                tray_snapshot.clone(),
                            );
                        },
                        "providers-cursor",
                        hovered_card_id,
                        set_hovered_card_id.clone(),
                    ),
                };
                rows.push(toggle.with_key(format!("providers-{}", provider.id())));
            }
            rows.push(
                settings_section_heading("Customization")
                    .with_key("providers-customization-heading"),
            );
            rows.push(
                settings_toggle_card(
                    "Use colored provider icons",
                    use_colored_provider_icons,
                    move |value| {
                        persist_bool(
                            set_use_colored_provider_icons.clone(),
                            apply_use_colored_provider_icons.clone(),
                            value,
                            |settings, value| {
                                settings.use_colored_provider_icons = value;
                            },
                        );
                    },
                    "providers-colored-icons",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("providers-colored-icons"),
            );
            if codex_enabled {
                rows.push(
                    settings_toggle_card(
                        "Replace ChatGPT logo with Codex",
                        replace_chatgpt_logo_with_codex,
                        move |value| {
                            persist_bool(
                                set_replace_chatgpt_logo_with_codex.clone(),
                                apply_replace_chatgpt_logo_with_codex.clone(),
                                value,
                                |settings, value| {
                                    settings.replace_chatgpt_logo_with_codex = value;
                                },
                            );
                        },
                        "providers-codex-logo",
                        hovered_card_id,
                        set_hovered_card_id.clone(),
                    )
                    .with_key("providers-codex-logo"),
                );
            }
            ("Providers", rows)
        }
        Tab::Tray => {
            let providers: Vec<ProviderKind> = popup_order
                .iter()
                .filter_map(|widget| widget.as_provider())
                .collect();
            let enabled_providers =
                enabled_providers(&providers, codex_enabled, claude_enabled, cursor_enabled);
            (
                "Tray",
                tray_settings_cards(
                    tray_widgets,
                    &enabled_providers,
                    expanded_tray_widget,
                    removed_tray_widget,
                    set_tray_widgets,
                    set_expanded_tray_widget,
                    set_removed_tray_widget,
                    settings_tx.clone(),
                ),
            )
        }
        Tab::Notifications => (
            "Notifications",
            vec![
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
                codex_enabled: set_codex_enabled,
                claude_enabled: set_claude_enabled,
                cursor_enabled: set_cursor_enabled,
                popup_order: set_popup_order,
                use_colored_provider_icons: set_use_colored_provider_icons,
                replace_chatgpt_logo_with_codex: set_replace_chatgpt_logo_with_codex,
                automatic_activation: set_automatic_activation,
                limit_refresh_interval: set_limit_refresh_interval,
                start_at_login: set_start_at_login,
                show_used_percentage: set_show_used_percentage,
                show_usage_pace: set_show_usage_pace,
                show_banked_resets: set_show_banked_resets,
                show_usage_stats: set_show_usage_stats,
                show_total_spend_on_all_tab: set_show_total_spend_on_all_tab,
                total_spend_presentation: set_total_spend_presentation,
                show_account_name: set_show_account_name,
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
    .padding(Thickness::uniform(18.0))
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
            left: 14.0,
            top: 12.0,
            right: 14.0,
            bottom: 12.0,
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

fn tray_widget_summary(widget: &TrayWidget) -> String {
    if widget.kind == TrayWidgetKind::AppIcon {
        return "App icon".into();
    }
    let labels = widget
        .indicators
        .iter()
        .map(|indicator| {
            let Some(provider) = indicator.provider() else {
                return format!("Unsupported {}", indicator.provider_id);
            };
            let metric = crate::provider_registry::metric(provider, &indicator.metric_id)
                .map(|metric| metric.label.to_owned())
                .unwrap_or_else(|| indicator.metric_id.clone());
            format!("{} {metric}", provider.display_name())
        })
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
    let pixels = crate::tray::render_widget(widget, tray_preview_limits());
    let preview_id = widget.id.clone();

    // Swap-chain preview painters normally run only on mount. Settings need
    // true live feedback, so repaint the retained native panel in place while
    // keeping the surrounding card identity stable.
    TRAY_PREVIEW_MOUNTS.with(|mounts| {
        if let Some(native) = mounts.borrow().get(&preview_id).cloned()
            && let Err(error) = crate::acrylic::install_tray_pixels_into(native, &pixels)
        {
            eprintln!("Could not update tray preview: {error:?}");
        }
    });

    let pixels_for_mount = pixels.clone();
    let id_for_mount = preview_id.clone();
    let id_for_unmount = preview_id.clone();
    let mut host = swap_chain_panel().width(32.0).height(32.0);
    host.mounted = Some(Callback::new(
        move |native: Option<windows_core::IInspectable>| {
            if let Some(native) = native {
                if let Err(error) =
                    crate::acrylic::install_tray_pixels_into(native.clone(), &pixels_for_mount)
                {
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
/// Match ComboBox / input control height; trash glyph is intentionally smaller.
const TRAY_REMOVE_BUTTON_SIZE: f64 = 32.0;
const TRAY_REMOVE_ICON_SIZE: f64 = 14.0;

fn tray_settings_cards(
    widgets: &[TrayWidget],
    enabled_providers: &[ProviderKind],
    expanded_widget: &Option<String>,
    removed_widget: &Option<(usize, TrayWidget)>,
    set_widgets: SetState<Vec<TrayWidget>>,
    set_expanded_widget: SetState<Option<String>>,
    set_removed_widget: SetState<Option<(usize, TrayWidget)>>,
    settings_tx: Sender<Settings>,
) -> Vec<Element> {
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
            .padding(Thickness::uniform(10.0))
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

        let content: Element = if widget.kind == TrayWidgetKind::AppIcon {
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
            let mut appearance_controls = Vec::<Element>::new();

            let widgets_for_presentation = widgets.to_vec();
            let presentation_setter = set_widgets.clone();
            let presentation_tx = settings_tx.clone();
            let providers_for_presentation = enabled_providers.to_vec();
            appearance_controls.push(
                ComboBox::new([
                    "Numbers",
                    "Progress bars",
                    "Rings",
                    "Reset time",
                    "Countdown",
                ])
                .header("Appearance")
                .grid_column(0)
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .selected_index(tray_presentation_index(widget.presentation))
                .on_selection_changed(move |choice| {
                    let mut next = widgets_for_presentation.clone();
                    next[index].presentation = tray_presentation_from_index(choice);
                    persist_tray_widgets(
                        presentation_setter.clone(),
                        presentation_tx.clone(),
                        next,
                        &providers_for_presentation,
                    );
                })
                .into(),
            );

            let widgets_for_color = widgets.to_vec();
            let color_setter = set_widgets.clone();
            let color_tx = settings_tx.clone();
            let providers_for_color = enabled_providers.to_vec();
            appearance_controls.push(
                ComboBox::new(["Status", "Fixed", "Provider", "App accent", "Monochrome"])
                    .header("Color")
                    .grid_column(1)
                    .horizontal_alignment(HorizontalAlignment::Stretch)
                    .selected_index(tray_color_mode_index(widget.color_mode))
                    .on_selection_changed(move |choice| {
                        let mut next = widgets_for_color.clone();
                        next[index].color_mode = tray_color_mode_from_index(choice);
                        persist_tray_widgets(
                            color_setter.clone(),
                            color_tx.clone(),
                            next,
                            &providers_for_color,
                        );
                    })
                    .into(),
            );
            fields.push(
                grid(appearance_controls)
                    .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
                    .column_spacing(12.0)
                    .horizontal_alignment(HorizontalAlignment::Stretch)
                    .into(),
            );

            if widget.color_mode == TrayColorMode::Fixed {
                let widgets_for_picker = widgets.to_vec();
                let picker_setter = set_widgets.clone();
                let picker_tx = settings_tx.clone();
                let providers_for_picker = enabled_providers.to_vec();
                fields.push(
                    ColorPicker::new(ColorArgb::new(
                        widget.fixed_color.red,
                        widget.fixed_color.green,
                        widget.fixed_color.blue,
                    ))
                    .alpha_enabled(false)
                    .on_color_changed(move |(_, red, green, blue)| {
                        let mut next = widgets_for_picker.clone();
                        next[index].fixed_color = TrayFixedColor { red, green, blue };
                        persist_tray_widgets(
                            picker_setter.clone(),
                            picker_tx.clone(),
                            next,
                            &providers_for_picker,
                        );
                    })
                    .into(),
                );
            }

            fields.push(settings_section_heading("Indicators"));
            for (indicator_index, indicator) in widget.indicators.iter().cloned().enumerate() {
                let provider_options = crate::provider_registry::PROVIDERS;
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
                let widgets_for_metric = widgets.to_vec();
                let metric_setter = set_widgets.clone();
                let metric_tx = settings_tx.clone();
                let enabled_for_metric = enabled_providers.to_vec();
                let widgets_for_value = widgets.to_vec();
                let value_setter = set_widgets.clone();
                let value_tx = settings_tx.clone();
                let enabled_for_value = enabled_providers.to_vec();
                let widgets_for_remove = widgets.to_vec();
                let remove_setter = set_widgets.clone();
                let removed_setter = set_removed_widget.clone();
                let remove_tx = settings_tx.clone();
                let enabled_for_remove = enabled_providers.to_vec();
                let widgets_for_indicator_up = widgets.to_vec();
                let indicator_up_setter = set_widgets.clone();
                let indicator_up_tx = settings_tx.clone();
                let enabled_for_indicator_up = enabled_providers.to_vec();
                let widgets_for_indicator_down = widgets.to_vec();
                let indicator_down_setter = set_widgets.clone();
                let indicator_down_tx = settings_tx.clone();
                let enabled_for_indicator_down = enabled_providers.to_vec();

                let indicator_reorder = vstack((
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
                            let mut next = widgets_for_indicator_up.clone();
                            next[index]
                                .indicators
                                .swap(indicator_index, indicator_index - 1);
                            persist_tray_widgets(
                                indicator_up_setter.clone(),
                                indicator_up_tx.clone(),
                                next,
                                &enabled_for_indicator_up,
                            );
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
                            let mut next = widgets_for_indicator_down.clone();
                            if indicator_index + 1 >= next[index].indicators.len() {
                                return;
                            }
                            next[index]
                                .indicators
                                .swap(indicator_index, indicator_index + 1);
                            persist_tray_widgets(
                                indicator_down_setter.clone(),
                                indicator_down_tx.clone(),
                                next,
                                &enabled_for_indicator_down,
                            );
                        }),
                ))
                .spacing(0.0)
                .horizontal_alignment(HorizontalAlignment::Center);

                let indicator_fields = vec![
                    indicator_reorder
                        .grid_column(0)
                        .vertical_alignment(VerticalAlignment::Center)
                        .into(),
                    ComboBox::new(provider_labels)
                        .header("Provider")
                        .grid_column(1)
                        .horizontal_alignment(HorizontalAlignment::Stretch)
                        .selected_index(provider_index)
                        .on_selection_changed(move |choice: i32| {
                            let Some(descriptor) =
                                crate::provider_registry::PROVIDERS.get(choice.max(0) as usize)
                            else {
                                return;
                            };
                            let mut next = widgets_for_provider.clone();
                            next[index].indicators[indicator_index].provider_id =
                                descriptor.id.into();
                            next[index].indicators[indicator_index].metric_id = descriptor
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
                        })
                        .into(),
                    ComboBox::new(metric_labels)
                        .header("Metric")
                        .grid_column(2)
                        .horizontal_alignment(HorizontalAlignment::Stretch)
                        .selected_index(metric_index)
                        .with_key(format!(
                            "tray-metric-{}-{indicator_index}-{}",
                            widget.id, indicator.provider_id
                        ))
                        .on_selection_changed(move |choice: i32| {
                            let Some(metric) = metrics.get(choice.max(0) as usize) else {
                                return;
                            };
                            let mut next = widgets_for_metric.clone();
                            next[index].indicators[indicator_index].metric_id = metric.id.into();
                            persist_tray_widgets(
                                metric_setter.clone(),
                                metric_tx.clone(),
                                next,
                                &enabled_for_metric,
                            );
                        })
                        .into(),
                    ComboBox::new(["Remaining", "Used"])
                        .header("Value")
                        .grid_column(3)
                        .horizontal_alignment(HorizontalAlignment::Stretch)
                        .selected_index(if indicator.limit_value == LimitValue::Remaining {
                            0
                        } else {
                            1
                        })
                        .on_selection_changed(move |choice| {
                            let mut next = widgets_for_value.clone();
                            next[index].indicators[indicator_index].limit_value = if choice == 1 {
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
                        })
                        .into(),
                    Button::new("")
                        .icon(Symbol::Delete)
                        .grid_column(4)
                        .font_size(TRAY_REMOVE_ICON_SIZE)
                        .width(TRAY_REMOVE_BUTTON_SIZE)
                        .height(TRAY_REMOVE_BUTTON_SIZE)
                        .min_width(TRAY_REMOVE_BUTTON_SIZE)
                        .min_height(TRAY_REMOVE_BUTTON_SIZE)
                        .max_width(TRAY_REMOVE_BUTTON_SIZE)
                        .max_height(TRAY_REMOVE_BUTTON_SIZE)
                        .padding(Thickness::uniform(0.0))
                        .tooltip("Remove indicator")
                        .vertical_alignment(VerticalAlignment::Bottom)
                        .on_click(move || {
                            let mut next = widgets_for_remove.clone();
                            next[index].indicators.remove(indicator_index);
                            if next[index].indicators.is_empty() {
                                let removed = next.remove(index);
                                removed_setter.call(Some((index, removed)));
                            }
                            persist_tray_widgets(
                                remove_setter.clone(),
                                remove_tx.clone(),
                                next,
                                &enabled_for_remove,
                            );
                        })
                        .into(),
                ];
                fields.push(
                    border(
                        grid(indicator_fields)
                            .columns([
                                GridLength::Pixel(TRAY_REORDER_BUTTON_SIZE),
                                GridLength::Star(1.0),
                                GridLength::Star(1.5),
                                GridLength::Star(1.0),
                                GridLength::Pixel(TRAY_REMOVE_BUTTON_SIZE),
                            ])
                            .column_spacing(12.0)
                            .horizontal_alignment(HorizontalAlignment::Stretch),
                    )
                    .padding(Thickness::uniform(12.0))
                    .background(ThemeRef::CardBackground)
                    .corner_radius(8.0)
                    .border_thickness(Thickness::uniform(1.0))
                    .border_brush(ThemeRef::CardStroke)
                    .horizontal_alignment(HorizontalAlignment::Stretch)
                    .with_key(format!("tray-indicator-{}-{indicator_index}", widget.id))
                    .with_translation_transition(duration(CONTROL_FAST_ANIMATION))
                    .into(),
                );
            }

            let mut widget_actions = Vec::<Element>::new();
            if widget.indicators.len() < 3 {
                let widgets_for_add = widgets.to_vec();
                let add_setter = set_widgets.clone();
                let add_tx = settings_tx.clone();
                let enabled_for_add = enabled_providers.to_vec();
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
                            next[index]
                                .indicators
                                .push(TrayIndicator::new(fallback_provider, metric));
                            persist_tray_widgets(
                                add_setter.clone(),
                                add_tx.clone(),
                                next,
                                &enabled_for_add,
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

        let row: Element = settings_content_expander(header, is_expanded, move |expanded: bool| {
            expand_setter.call(expanded.then(|| expand_id.clone()));
        }, content)
        .with_translation_transition(duration(CONTROL_FAST_ANIMATION))
        .with_opacity_transition(duration(CONTROL_FAST_ANIMATION))
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
) -> Vec<ProviderKind> {
    order
        .iter()
        .copied()
        .filter(|provider| match provider {
            ProviderKind::Codex => codex_enabled,
            ProviderKind::Claude => claude_enabled,
            ProviderKind::Cursor => cursor_enabled,
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
