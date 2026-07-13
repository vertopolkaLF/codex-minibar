//! Settings-window entry point.
//!
//! The host is exposed here so callers do not depend on popup implementation
//! details; both surfaces share tokens from [`crate::theme`].

use crate::notifications;
use crate::settings::{LimitValue, Settings, TrayPresentation, TraySource, TrayWidget};
use crate::settings_controls::{
    UPDATE_SYMBOL, settings_action_card, settings_info_card, settings_slider_content,
    settings_toggle_card, settings_toggle_card_with_description, settings_toggle_expander,
    update_available_nav_card,
};
use crate::theme::CONTROL_FAST_ANIMATION;
use crate::updater::{
    ISSUES_URL, RELEASES_URL, REPO_URL, UpdateController, UpdatePhase, current_version,
};
use anyhow::Context;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, mpsc::Sender},
    time::Duration,
};
use windows_reactor::*;

const WINDOW_WIDTH: f64 = 760.0;
const WINDOW_HEIGHT: f64 = 520.0;

thread_local! {
    static HOST: RefCell<Option<Rc<ReactorHost>>> = const { RefCell::new(None) };
}

pub fn open(
    settings_tx: Sender<Settings>,
    updates: Arc<UpdateController>,
) -> windows_core::Result<()> {
    HOST.with(|slot| {
        if settings_window_is_open() {
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
            "Codex Minibar Settings",
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
        // Force Dark so the XAML island clear color is black, not white.
        set_requested_theme(RequestedTheme::Dark);
        set_settings_window_icon();
        apply_settings_dark_chrome();
        // Hide the HWND before WinUI tears content down so close does not flash
        // empty black chrome (default title bar + no Mica/content).
        install_settings_close_hide();
        host.activate()?;
        *slot.borrow_mut() = Some(host);
        Ok(())
    })
}

/// On WM_CLOSE / SC_CLOSE, hide the window while it still looks correct, then
/// let the default close path destroy it. Without this, content is dismantled
/// while the HWND is still visible → black flash with OS chrome.
#[cfg(windows)]
fn install_settings_close_hide() {
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SC_CLOSE, SW_HIDE, ShowWindow, WM_CLOSE, WM_NCDESTROY, WM_SYSCOMMAND,
    };

    const SUBCLASS_ID: usize = 0xC0DE_5E77;

    let title: Vec<u16> = "Codex Minibar Settings"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
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
        UI::WindowsAndMessaging::{
            FindWindowW, ICON_BIG, ICON_SMALL, LoadIconW, SendMessageW, WM_SETICON,
        },
    };

    let title: Vec<u16> = "Codex Minibar Settings"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
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

#[cfg(not(windows))]
fn set_settings_window_icon() {}

#[cfg(not(windows))]
fn install_settings_close_hide() {}

/// Immersive dark mode + Dark requested theme for the settings HWND.
#[cfg(windows)]
fn apply_settings_dark_chrome() {
    use windows_sys::Win32::{
        Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute},
        UI::WindowsAndMessaging::FindWindowW,
    };

    set_requested_theme(RequestedTheme::Dark);

    let title: Vec<u16> = "Codex Minibar Settings"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    if hwnd.is_null() {
        return;
    }

    let dark_mode = 1u32;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            &dark_mode as *const u32 as *const _,
            size_of::<u32>() as u32,
        );
    }
}

#[cfg(not(windows))]
fn apply_settings_dark_chrome() {
    set_requested_theme(RequestedTheme::Dark);
}

fn load_settings_for_window() -> Settings {
    match Settings::default_path().and_then(|path| Settings::load_or_create(&path)) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("failed to load settings for window: {error:#}");
            Settings::default()
        }
    }
}

#[cfg(windows)]
fn settings_window_is_open() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;

    let title: Vec<u16> = "Codex Minibar Settings"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    !unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) }.is_null()
}

#[cfg(not(windows))]
fn settings_window_is_open() -> bool {
    false
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Tab {
    #[default]
    General,
    Tray,
    Notifications,
    Advanced,
    About,
}

impl Tab {
    fn tag(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Tray => "tray",
            Self::Notifications => "notifications",
            Self::Advanced => "advanced",
            Self::About => "about",
        }
    }

    fn from_tag(tag: &str) -> Self {
        match tag {
            "tray" => Self::Tray,
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
    // After the tree mounts, pin Dark so the island clear color stays black
    // (white clear washes every translucent Fluent layer on a real display).
    cx.use_effect((), apply_settings_dark_chrome);
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

    let mut navigation = NavigationView::new(
        [
            NavViewItem::new("General")
                .tag("general")
                .icon(Symbol::Home),
            NavViewItem::new("Tray").tag("tray").icon(Symbol::More),
            NavViewItem::new("Notifications")
                .tag("notifications")
                .icon(Symbol::Flag),
            NavViewItem::new("Advanced")
                .tag("advanced")
                .icon(Symbol::Edit),
            NavViewItem::new("About & Updates")
                .tag("about")
                .icon(Symbol::Important),
        ],
        Element::Empty,
    )
    .selected_tag(selected.tag())
    .on_selection_changed({
        let set_rendered_tab = set_rendered_tab.clone();
        let set_page_visible = set_page_visible.clone();
        move |tag: String| {
            let next = Tab::from_tag(&tag);
            if next != selected {
                set_page_visible.call(false);
                set_selected.call(next);
                let set_rendered_tab = set_rendered_tab.clone();
                let set_page_visible = set_page_visible.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(180));
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

    let (start_at_login, set_start_at_login) = cx.use_state(settings.start_at_login);
    let (automatic_activation, set_automatic_activation) =
        cx.use_state(settings.automatic_activation);
    let (show_used_percentage, set_show_used_percentage) =
        cx.use_state(settings.show_used_percentage);
    let (show_usage_pace, set_show_usage_pace) = cx.use_state(settings.show_usage_pace);
    let (hide_plan_credits, set_hide_plan_credits) = cx.use_state(settings.hide_plan_credits);
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
    let (check_for_updates, set_check_for_updates) = cx.use_state(settings.check_for_updates);
    let (notify_on_update, set_notify_on_update) =
        cx.use_state(settings.notifications.update_available);

    // Padding lives on tab content (inside the scroller), not on this pane, so
    // LayerFill crops flush to the window edge while long tabs stay scrollable.
    let page_scroller = scroll_viewer(
        border(tab_content(
            &settings,
            rendered_tab,
            automatic_activation,
            start_at_login,
            show_used_percentage,
            show_usage_pace,
            hide_plan_credits,
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
            &hovered_card_id,
            check_for_updates,
            notify_on_update,
            &update_phase,
            set_automatic_activation,
            set_start_at_login,
            set_show_used_percentage,
            set_show_usage_pace,
            set_hide_plan_credits,
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
            set_hovered_card_id,
            set_check_for_updates,
            set_notify_on_update,
            settings_tx.clone(),
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
    .with_opacity_transition(CONTROL_FAST_ANIMATION)
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

fn settings_github_icon_uri() -> String {
    let packaged = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .map(|parent| parent.join("assets/icons/github-iconify.svg"))
        })
        .filter(|path| path.exists());
    let path = packaged.unwrap_or_else(|| {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons/github-iconify.svg")
    });
    format!("file:///{}", path.to_string_lossy().replace('\\', "/"))
}

fn tab_content(
    settings: &Settings,
    tab: Tab,
    automatic_activation: bool,
    start_at_login: bool,
    show_used_percentage: bool,
    show_usage_pace: bool,
    hide_plan_credits: bool,
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
    hovered_card_id: &Option<String>,
    check_for_updates: bool,
    notify_on_update: bool,
    update_phase: &UpdatePhase,
    set_automatic_activation: SetState<bool>,
    set_start_at_login: SetState<bool>,
    set_show_used_percentage: SetState<bool>,
    set_show_usage_pace: SetState<bool>,
    set_hide_plan_credits: SetState<bool>,
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
    set_hovered_card_id: SetState<Option<String>>,
    set_check_for_updates: SetState<bool>,
    set_notify_on_update: SetState<bool>,
    settings_tx: Sender<Settings>,
    updates: Arc<UpdateController>,
) -> Element {
    let apply_automatic_activation = settings_tx.clone();
    let apply_start_at_login = settings_tx.clone();
    let apply_show_used_percentage = settings_tx.clone();
    let apply_show_usage_pace = settings_tx.clone();
    let apply_hide_plan_credits = settings_tx.clone();
    let apply_activation_failure = settings_tx.clone();
    let apply_limits_reset = settings_tx.clone();
    let apply_low_usage_enabled = settings_tx.clone();
    let apply_low_usage_threshold = settings_tx.clone();
    let apply_weekly_low_usage_enabled = settings_tx.clone();
    let apply_weekly_low_usage_threshold = settings_tx.clone();
    let apply_check_for_updates = settings_tx.clone();
    let apply_notify_on_update = settings_tx.clone();
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
                    Some("Sends a short message when needed to begin the 5-hour usage window."),
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
                settings_section_heading("Customization").with_key("general-customization-heading"),
                settings_toggle_card_with_description(
                    "Show used percentage",
                    Some("Reports consumed usage instead of the amount remaining."),
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
                settings_toggle_card(
                    "Hide plan and credits from popup",
                    hide_plan_credits,
                    move |value| {
                        persist_bool(
                            set_hide_plan_credits.clone(),
                            apply_hide_plan_credits.clone(),
                            value,
                            |settings, value| {
                                settings.hide_plan_credits = value;
                            },
                        );
                    },
                    "general-hide-credits",
                    hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("general-hide-credits"),
            ],
        ),
        Tab::Tray => (
            "Tray",
            tray_settings_cards(tray_widgets, set_tray_widgets, settings_tx.clone()),
        ),
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
        Tab::Advanced => (
            "Advanced",
            vec![
                settings_info_card(
                    "History retention",
                    format!("{} days", settings.history_retention_days),
                )
                .with_key("advanced-retention"),
                settings_info_card(
                    "Codex executable",
                    settings
                        .codex_path
                        .as_ref()
                        .map_or("Automatic".into(), |path| path.display().to_string()),
                )
                .with_key("advanced-codex-path"),
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
        .background(Color {
            a: 17,
            r: 255,
            g: 255,
            b: 255,
        })
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
                Some(UPDATE_SYMBOL),
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
                None,
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
            None,
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
                AboutCardIcon::GitHub,
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
                AboutCardIcon::Glyph("▤"),
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
                AboutCardIcon::Glyph("⚑"),
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
                AboutCardIcon::Glyph("@"),
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
    GitHub,
    Glyph(&'static str),
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
        .background(Color {
            a: 48,
            r: 255,
            g: 137,
            b: 83,
        })
        .corner_radius(10.0)
        .relative_align_left()
        .relative_align_right()
        .relative_align_top()
        .relative_align_bottom()
        .into();
    let hover: Element = border(Element::Empty)
        .background(Color {
            a: 78,
            r: 255,
            g: 137,
            b: 83,
        })
        .opacity(if hovered { 1.0 } else { 0.0 })
        .with_opacity_transition(CONTROL_FAST_ANIMATION)
        .corner_radius(10.0)
        .relative_align_left()
        .relative_align_right()
        .relative_align_top()
        .relative_align_bottom()
        .into();
    let icon: Element = match icon {
        AboutCardIcon::GitHub => Image::new_with_uri(settings_github_icon_uri())
            .width(16.0)
            .height(16.0)
            .vertical_alignment(VerticalAlignment::Center)
            .into(),
        AboutCardIcon::Glyph(value) => text_block(value)
            .font_size(16.0)
            .foreground(Color {
                a: 255,
                r: 255,
                g: 170,
                b: 89,
            })
            .width(16.0)
            .vertical_alignment(VerticalAlignment::Center)
            .into(),
    };
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

fn tray_settings_cards(
    widgets: &[TrayWidget],
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
        let widgets_for_source = widgets.to_vec();
        let source_setter = set_widgets.clone();
        let source_tx = settings_tx.clone();
        let widget_for_presentation = widget.clone();
        let widgets_for_presentation = widgets.to_vec();
        let presentation_setter = set_widgets.clone();
        let presentation_tx = settings_tx.clone();
        let widgets_for_value = widgets.to_vec();
        let value_setter = set_widgets.clone();
        let value_tx = settings_tx.clone();
        let widgets_for_remove = widgets.to_vec();
        let remove_setter = set_widgets.clone();
        let remove_tx = settings_tx.clone();
        let widgets_for_left = widgets.to_vec();
        let left_setter = set_widgets.clone();
        let left_tx = settings_tx.clone();
        let widgets_for_right = widgets.to_vec();
        let right_setter = set_widgets.clone();
        let right_tx = settings_tx.clone();

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
                        source: source.clone(),
                        presentation: default_presentation(&source),
                        limit_value: widget_for_source.limit_value,
                    };
                    persist_tray_widgets(source_setter.clone(), source_tx.clone(), next);
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
                        );
                    }
                })
                .into(),
        ];
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
                        persist_tray_widgets(value_setter.clone(), value_tx.clone(), next);
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
                        persist_tray_widgets(left_setter.clone(), left_tx.clone(), next);
                    }),
                Button::new("Move right")
                    .enabled(index + 1 < widgets_for_right.len())
                    .on_click(move || {
                        if index + 1 >= widgets_for_right.len() {
                            return;
                        }
                        let mut next = widgets_for_right.clone();
                        next.swap(index, index + 1);
                        persist_tray_widgets(right_setter.clone(), right_tx.clone(), next);
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
                    persist_tray_widgets(remove_setter.clone(), remove_tx.clone(), next);
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
    cards.push(
        Button::new("Add tray widget")
            .accent()
            .on_click(move || {
                let mut next = widgets_for_add.clone();
                next.push(TrayWidget::default_user_widget());
                persist_tray_widgets(add_setter.clone(), settings_tx.clone(), next);
            })
            .with_key("tray-add-widget")
            .into(),
    );
    cards
}

fn source_index(source: &TraySource) -> i32 {
    match source {
        TraySource::Combined => 0,
        TraySource::Primary => 1,
        TraySource::Secondary => 2,
        TraySource::PrimaryReset => 3,
    }
}

fn source_from_index(index: i32) -> TraySource {
    match index {
        1 => TraySource::Primary,
        2 => TraySource::Secondary,
        3 => TraySource::PrimaryReset,
        _ => TraySource::Combined,
    }
}

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

fn default_presentation(source: &TraySource) -> TrayPresentation {
    presentation_options(source)[0].1.clone()
}

fn persist_tray_widgets(
    setter: SetState<Vec<TrayWidget>>,
    settings_tx: Sender<Settings>,
    widgets: Vec<TrayWidget>,
) {
    setter.call(widgets.clone());
    persist_update(settings_tx, move |settings| settings.tray_widgets = widgets);
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

fn persist_update(settings_tx: Sender<Settings>, update: impl FnOnce(&mut Settings)) {
    let result = Settings::default_path().and_then(|path| {
        let mut settings = Settings::load_or_create(&path)?;
        update(&mut settings);
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
