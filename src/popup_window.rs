use std::{
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender},
    },
    thread,
    time::Duration,
};

use chrono::{DateTime, Duration as ChronoDuration, Local, Utc};
use windows_reactor::*;

use crate::{
    limits::{LimitWindow, RateLimits},
    popup,
    settings::Settings,
    theme::{DARK_SURFACE_FILL, SURFACE_FILL, WINDOW_BORDER, WINDOW_FILL},
    tray::{TrayManager, TrayMenuAction},
    worker::{WorkerCommand, WorkerEvent},
};

fn format_activation_at(at: DateTime<Utc>) -> String {
    at.with_timezone(&Local)
        .format("%H:%M:%S %d.%m.%Y")
        .to_string()
}

/// Start of the current 5h window: resets_at minus duration.
fn window_started_at(window: &LimitWindow) -> Option<DateTime<Utc>> {
    match (window.resets_at, window.duration_minutes) {
        (Some(reset), Some(minutes)) => Some(reset - ChronoDuration::minutes(i64::from(minutes))),
        _ => None,
    }
}

fn format_last_activation(limits: &RateLimits, fallback_attempt: Option<DateTime<Utc>>) -> String {
    window_started_at(&limits.primary)
        .or(fallback_attempt)
        .map(format_activation_at)
        .unwrap_or_else(|| "Never".into())
}

/// `#FFFFFF05`: a barely perceptible content wash over Mica, matching the
/// settings-window appearance without making the page look like a card.
use crate::theme::SETTINGS_CONTENT_FILL;
/// Shared startup state handed from `main` into the reactor render tree.
pub struct AppState {
    pub settings: Settings,
    pub commands: Option<Sender<WorkerCommand>>,
    pub worker: Mutex<Option<crate::worker::WorkerHandle>>,
    pub startup_error: Option<String>,
    /// Last activation attempt loaded from persisted activation state.
    pub last_activation_at: Option<DateTime<Utc>>,
    /// Live settings pushes from the settings window; drained by the tray bridge.
    pub settings_rx: Mutex<Option<Receiver<Settings>>>,
    pub settings_tx: Sender<Settings>,
}

impl AppState {
    fn take_worker_events(&self) -> Option<Receiver<WorkerEvent>> {
        self.worker.lock().ok()?.as_mut()?.take_events()
    }

    fn shutdown_worker(&self) {
        if let Ok(mut worker) = self.worker.lock()
            && let Some(worker) = worker.take()
        {
            worker.shutdown();
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct UiState {
    limits: RateLimits,
    last_activation: String,
    error: Option<String>,
    show_used_percentage: bool,
    hide_plan_credits: bool,
}

/// Sections of the settings window. Keeping this as a small enum makes the
/// sidebar stable while each page grows independently.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SettingsTab {
    #[default]
    General,
    Tray,
    Notifications,
    Advanced,
}

#[allow(dead_code)]
impl SettingsTab {
    fn index(self) -> u8 {
        match self {
            Self::General => 0,
            Self::Tray => 1,
            Self::Notifications => 2,
            Self::Advanced => 3,
        }
    }

    fn tag(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Tray => "tray",
            Self::Notifications => "notifications",
            Self::Advanced => "advanced",
        }
    }

    fn from_tag(tag: &str) -> Self {
        match tag {
            "tray" => Self::Tray,
            "notifications" => Self::Notifications,
            "advanced" => Self::Advanced,
            _ => Self::General,
        }
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            limits: RateLimits::default(),
            last_activation: "Never".into(),
            error: None,
            show_used_percentage: false,
            hide_plan_credits: false,
        }
    }
}

/// Root WinUI view for Codex Minibar (hosted in a tray popup shell).
pub fn app(cx: &mut RenderCx, state: Arc<AppState>) -> Element {
    let dpi = cx.use_dpi().max(1);
    let window_corner_radius = f64::from(popup::WINDOW_CORNER_RADIUS_DIP);
    // Keep the visual stroke one physical pixel inside the HWND clip so GDI's
    // aliased region cannot trim its anti-aliased XAML corner pixels.
    let border_inset = 96.0 / f64::from(dpi);
    let inner_corner_radius = (window_corner_radius - border_inset).max(0.0);
    let (ui, set_ui) = cx.use_async_state(UiState {
        error: state.startup_error.clone(),
        last_activation: format_last_activation(&RateLimits::default(), state.last_activation_at),
        show_used_percentage: state.settings.show_used_percentage,
        hide_plan_credits: state.settings.hide_plan_credits,
        ..UiState::default()
    });
    let commands = state.commands.clone();
    let ui_dispatcher = cx.use_ui_marshaller();
    let settings_tx = state.settings_tx.clone();

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

    let refresh = {
        let commands = commands.clone();
        move || {
            if let Some(commands) = &commands {
                let _ = commands.send(WorkerCommand::Refresh);
            }
        }
    };
    let quit = move || std::process::exit(0);

    let mut body: Vec<Element> = vec![
        limit_card("5h Session", &ui.limits.primary, ui.show_used_percentage),
        limit_card("Weekly", &ui.limits.secondary, ui.show_used_percentage),
    ];

    if !ui.hide_plan_credits {
        body.push(meta_row(&ui.limits));
    }

    if let Some(error) = &ui.error {
        body.insert(
            0,
            InfoBar::new("Something went wrong")
                .message(error.clone())
                .error()
                .is_closable(false)
                .into(),
        );
    }

    let footer = border(
        grid((
            vstack((
                body_strong("Codex Minibar").foreground(ThemeRef::SecondaryText),
                caption(format!(
                    "Updated {}",
                    sample_freshness(ui.limits.sampled_at).to_lowercase()
                ))
                .foreground(ThemeRef::TertiaryText),
            ))
            .spacing(0.0)
            .vertical_alignment(VerticalAlignment::Center)
            .horizontal_alignment(HorizontalAlignment::Left)
            .grid_column(0),
            hstack((
                icon_button("\u{E72C}", "Refresh", 16.0, refresh),
                icon_button("\u{E713}", "Settings", 16.0, {
                    let settings_tx = settings_tx.clone();
                    move || {
                        if let Err(error) = crate::settings_window::open(settings_tx.clone()) {
                            eprintln!("Could not open settings window: {error:?}");
                        }
                    }
                }),
                icon_button("\u{E7E8}", "Quit", 16.0, quit),
            ))
            .spacing(4.0)
            .horizontal_alignment(HorizontalAlignment::Right)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(1),
        ))
        .rows([GridLength::Auto])
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(Thickness {
        left: 24.0,
        top: 10.0,
        right: 18.0,
        // Extra bottom padding so content clears the rounded window corners.
        bottom: 14.0,
    })
    .background(DARK_SURFACE_FILL)
    .border_thickness(Thickness {
        left: 0.0,
        top: 1.0,
        right: 0.0,
        bottom: 0.0,
    })
    .border_brush(ThemeRef::CardStroke)
    .horizontal_alignment(HorizontalAlignment::Stretch);

    // Content + footer are Auto-sized. Acrylic shares that Auto row so its
    // SizeChanged reports the real content height — then we ResizeClient to match.
    // Guessing DIP constants fought WinUI and left a beer gut under the footer.
    let body_panel = border(
        grid((
            vstack(body)
                .spacing(12.0)
                .padding(Thickness {
                    left: 16.0,
                    top: 16.0,
                    right: 16.0,
                    bottom: 20.0,
                })
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .vertical_alignment(VerticalAlignment::Top)
                .grid_row(0),
            footer.grid_row(1),
        ))
        .rows([GridLength::Auto, GridLength::Auto])
        .columns([GridLength::Star(1.0)])
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Top)
        .background(Color::transparent()),
    )
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(WINDOW_BORDER)
    .corner_radius(inner_corner_radius)
    .background(WINDOW_FILL)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Top);

    // Acrylic behind content; reconciler does not manage this panel's children.
    // on_resize reports the Auto-row height (body + border). Add chrome padding
    // (border_inset on top and bottom) so the HWND does not clip the bottom stroke.
    let acrylic = {
        let mut host = swap_chain_panel()
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch);
        host.mounted = Some(Callback::new(|native: Option<_>| {
            if let Some(native) = native {
                crate::acrylic::install_into(native);
            }
        }));
        host.on_resize(move |_width, height| {
            popup::set_client_height_from_content(height + 2.0 * border_inset);
        })
    };

    let chrome = border(
        grid((acrylic, body_panel))
            .rows([GridLength::Auto])
            .columns([GridLength::Star(1.0)])
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Top)
            .background(Color::transparent()),
    )
    .padding(Thickness::uniform(border_inset))
    .corner_radius(window_corner_radius)
    .background(WINDOW_FILL)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Top);

    chrome.into()
}

/// The first settings surface is deliberately a native WinUI shell: persistent
/// sidebar on the left, focused tab content on the right. Persistence wiring
/// follows once every setting has its final interaction model.
#[allow(dead_code)]
pub(crate) fn open_settings_window(settings_tx: Sender<Settings>) -> windows_core::Result<()> {
    crate::settings_window::open(settings_tx)
}

/// A transparent WinUI/Mica window can retain a stale white DWM redirection
/// bitmap, particularly after moving across monitors. The visual symptom is a
/// real window that is lighter than screenshots despite the same XAML tree.
/// Disabling that legacy backing surface preserves Mica and lets the intended
/// `#FFFFFF05` page wash composite correctly.
#[cfg(windows)]
fn disable_settings_redirection_bitmap() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GWL_EXSTYLE, GetWindowLongW, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOSIZE, SWP_NOZORDER, SetWindowLongW, SetWindowPos, WS_EX_NOREDIRECTIONBITMAP,
    };

    let title: Vec<u16> = "Codex Minibar Settings"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    if hwnd.is_null() {
        return;
    }

    unsafe {
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        SetWindowLongW(
            hwnd,
            GWL_EXSTYLE,
            (ex_style | WS_EX_NOREDIRECTIONBITMAP) as i32,
        );
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        );
    }
}

#[cfg(not(windows))]
fn disable_settings_redirection_bitmap() {}

#[allow(dead_code)]
fn settings_window(cx: &mut RenderCx, settings: Arc<Settings>) -> Element {
    // Run after the WinUI tree has mounted as well as immediately after window
    // activation. The second pass covers the first-frame compositor path.
    cx.use_effect((), disable_settings_redirection_bitmap);
    let (selected, set_selected) = cx.use_state(SettingsTab::default());
    let content = settings_tab_content(&settings, selected);

    let menu = [
        NavViewItem::new("General")
            .tag(SettingsTab::General.tag())
            .icon(Symbol::Home),
        NavViewItem::new("Tray")
            .tag(SettingsTab::Tray.tag())
            .icon(Symbol::More),
        NavViewItem::new("Notifications")
            .tag(SettingsTab::Notifications.tag())
            .icon(Symbol::Flag),
        NavViewItem::new("Advanced")
            .tag(SettingsTab::Advanced.tag())
            .icon(Symbol::Edit),
    ];
    // NavigationView owns the sidebar only. Its generated content presenter
    // is opaque in the current WinUI template, so rendering the page inside it
    // would blend our `#FFFFFF05` wash over white instead of Mica.
    let navigation = NavigationView::new(menu, Element::Empty)
        .selected_tag(selected.tag())
        .on_selection_changed({
            move |tag: String| {
                let next = SettingsTab::from_tag(&tag);
                if next != selected {
                    set_selected.call(next);
                }
            }
        })
        .pane_display_mode(NavigationViewPaneDisplayMode::Left)
        .pane_open(true)
        .open_pane_length(220.0)
        .pane_title("Settings")
        .settings_visible(false)
        .back_button_visible(false)
        .pane_toggle_button_visible(false)
        .background(Color::transparent())
        .width(220.0)
        .horizontal_alignment(HorizontalAlignment::Left)
        .vertical_alignment(VerticalAlignment::Stretch);

    let page = border(
        border(content)
            .with_key(format!("settings-page-{}", selected.tag()))
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch),
    )
    .padding(Thickness {
        left: 32.0,
        top: 24.0,
        right: 32.0,
        bottom: 32.0,
    })
    .background(SETTINGS_CONTENT_FILL)
    .corner_radius(12.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Stretch);

    let title_bar = TitleBar::new("Codex Minibar Settings")
        .back_button_visible(false)
        .pane_toggle_button_visible(false);

    let shell = grid((navigation.grid_column(0), page.grid_column(1)))
        .columns([GridLength::Pixel(220.0), GridLength::Star(1.0)])
        .rows([GridLength::Star(1.0)])
        .background(Color::transparent());

    grid((title_bar.grid_row(0), shell.grid_row(1)))
        .rows([GridLength::Auto, GridLength::Star(1.0)])
        .columns([GridLength::Star(1.0)])
        .background(Color::transparent())
        .into()
}

#[allow(dead_code)]
fn settings_tab_content(settings: &Settings, tab: SettingsTab) -> Element {
    let (title, subtitle, rows): (&str, &str, Vec<Element>) = match tab {
        SettingsTab::General => (
            "General",
            "Core behavior for Codex Minibar.",
            vec![
                settings_row(
                    "Automatic activation",
                    if settings.automatic_activation {
                        "On"
                    } else {
                        "Off"
                    },
                ),
                settings_row(
                    "Start at sign-in",
                    if settings.start_at_login { "On" } else { "Off" },
                ),
                settings_row(
                    "Check for updates",
                    if settings.check_for_updates {
                        "On"
                    } else {
                        "Off"
                    },
                ),
            ],
        ),
        SettingsTab::Tray => (
            "Tray",
            "Choose what Codex Minibar shows in the notification area.",
            vec![settings_row(
                "Active tray widgets",
                format!("{} configured", settings.tray_widgets.len()),
            )],
        ),
        SettingsTab::Notifications => (
            "Notifications",
            "Decide which important events deserve your attention.",
            vec![
                settings_row(
                    "Activation failures",
                    if settings.notifications.activation_failure {
                        "On"
                    } else {
                        "Off"
                    },
                ),
                settings_row(
                    "Codex unavailable",
                    if settings.notifications.codex_unavailable {
                        "On"
                    } else {
                        "Off"
                    },
                ),
                settings_row(
                    "Activation successes",
                    if settings.notifications.activation_success {
                        "On"
                    } else {
                        "Off"
                    },
                ),
            ],
        ),
        SettingsTab::Advanced => (
            "Advanced",
            "Storage and integration settings that should stay out of the way.",
            vec![
                settings_row(
                    "History retention",
                    format!("{} days", settings.history_retention_days),
                ),
                settings_row(
                    "Codex executable",
                    settings
                        .codex_path
                        .as_ref()
                        .map_or("Automatic".into(), |path| path.display().to_string()),
                ),
            ],
        ),
    };

    vstack((
        text_block(title).font_size(28.0).bold(),
        text_block(subtitle).foreground(ThemeRef::SecondaryText),
        vstack(rows).spacing(8.0),
    ))
    .spacing(10.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Top)
    .into()
}

#[allow(dead_code)]
fn settings_row(label: impl Into<String>, value: impl Into<String>) -> Element {
    border(
        grid((
            text_block(label)
                .grid_column(0)
                .vertical_alignment(VerticalAlignment::Center),
            text_block(value)
                .foreground(ThemeRef::SecondaryText)
                .grid_column(1)
                .horizontal_alignment(HorizontalAlignment::Right)
                .vertical_alignment(VerticalAlignment::Center),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(Thickness {
        left: 12.0,
        top: 10.0,
        right: 12.0,
        bottom: 10.0,
    })
    .background(ThemeRef::CardBackground)
    .corner_radius(6.0)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn start_background_bridge(
    state: Arc<AppState>,
    set_ui: AsyncSetState<UiState>,
    ui_dispatcher: UiMarshaller,
) {
    let events = state.take_worker_events();
    let widgets = state.settings.tray_widgets.clone();
    let settings_rx = state
        .settings_rx
        .lock()
        .ok()
        .and_then(|mut slot| slot.take());
    let settings_tx = state.settings_tx.clone();

    thread::spawn(move || {
        let mut tray = TrayManager::new();
        let fallback_attempt = state.last_activation_at;
        let mut ui = UiState {
            error: state.startup_error.clone(),
            last_activation: format_last_activation(&RateLimits::default(), fallback_attempt),
            show_used_percentage: state.settings.show_used_percentage,
            hide_plan_credits: state.settings.hide_plan_credits,
            ..UiState::default()
        };

        if let Err(error) = tray.sync(&widgets, &ui.limits) {
            ui.error = Some(error.to_string());
            set_ui.call(ui.clone());
        }

        // Keep trying until the WinUI window exists, then park it as a popup.
        for _ in 0..50 {
            if popup::ensure_configured().is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        let apply_settings =
            |ui: &mut UiState, set_ui: &AsyncSetState<UiState>, settings: Settings| {
                ui.show_used_percentage = settings.show_used_percentage;
                ui.hide_plan_credits = settings.hide_plan_credits;
                set_ui.call(ui.clone());
            };

        let drain_settings = |ui: &mut UiState, set_ui: &AsyncSetState<UiState>| {
            let Some(settings_rx) = settings_rx.as_ref() else {
                return;
            };
            while let Ok(settings) = settings_rx.try_recv() {
                apply_settings(ui, set_ui, settings);
            }
        };

        let Some(events) = events else {
            set_ui.call(ui.clone());
            loop {
                popup::pump_messages();
                drain_settings(&mut ui, &set_ui);
                if pump_tray_and_dismiss(&tray, &ui_dispatcher, &settings_tx) {
                    drop(tray);
                    state.shutdown_worker();
                    std::process::exit(0);
                }
                thread::sleep(Duration::from_millis(16));
            }
        };

        loop {
            popup::pump_messages();
            drain_settings(&mut ui, &set_ui);
            if pump_tray_and_dismiss(&tray, &ui_dispatcher, &settings_tx) {
                drop(tray);
                state.shutdown_worker();
                std::process::exit(0);
            }
            match events.recv_timeout(Duration::from_millis(16)) {
                Ok(WorkerEvent::LimitsUpdated(limits)) => {
                    if let Err(error) = tray.sync(&widgets, &limits) {
                        ui.error = Some(error.to_string());
                    } else {
                        ui.error = None;
                    }
                    ui.last_activation = format_last_activation(&limits, fallback_attempt);
                    ui.limits = limits;
                    set_ui.call(ui.clone());
                }
                Ok(WorkerEvent::ActivationSucceeded) => {
                    ui.last_activation =
                        format!("Succeeded at {}", format_activation_at(Utc::now()));
                    set_ui.call(ui.clone());
                }
                Ok(WorkerEvent::ActivationFailed(error)) => {
                    ui.last_activation =
                        format!("Failed at {}: {error}", format_activation_at(Utc::now()));
                    set_ui.call(ui.clone());
                }
                Ok(WorkerEvent::PollFailed(error)) => {
                    ui.error = Some(error);
                    set_ui.call(ui.clone());
                }
                Ok(WorkerEvent::Stopped) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

#[cfg(windows)]
fn pump_tray_and_dismiss(
    tray: &TrayManager,
    ui_dispatcher: &UiMarshaller,
    settings_tx: &Sender<Settings>,
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
            popup::toggle_near(position.x as i32, position.y as i32);
        }
    }

    for action in tray.drain_menu_actions() {
        match action {
            TrayMenuAction::Settings => {
                let settings_tx = settings_tx.clone();
                ui_dispatcher.dispatch(move || {
                    if let Err(error) = crate::settings_window::open(settings_tx) {
                        eprintln!("Could not open settings window: {error:?}");
                    }
                });
            }
            TrayMenuAction::Exit => return true,
        }
    }

    popup::keep_on_monitor();

    if popup::clicked_outside() {
        popup::hide();
    }
    false
}

#[cfg(not(windows))]
fn pump_tray_and_dismiss(
    _tray: &TrayManager,
    _ui_dispatcher: &UiMarshaller,
    _settings_tx: &Sender<Settings>,
) -> bool {
    false
}

const ICON_BUTTON_SIZE: f64 = 36.0;

/// Icon-only button using Segoe Fluent Icons glyphs.
/// `font_size` is tuned per glyph so they look optically equal.
fn icon_button(glyph: &str, tip: &str, font_size: f64, on_click: impl IntoUnitCallback) -> Button {
    button(glyph)
        .subtle()
        .tooltip(tip)
        .font_family("Segoe Fluent Icons")
        .font_size(font_size)
        .width(ICON_BUTTON_SIZE)
        .height(ICON_BUTTON_SIZE)
        .min_width(ICON_BUTTON_SIZE)
        .min_height(ICON_BUTTON_SIZE)
        .max_width(ICON_BUTTON_SIZE)
        .max_height(ICON_BUTTON_SIZE)
        .padding(Thickness::uniform(0.0))
        .on_click(on_click)
}

/// Thin pill progress track with a rounded fill (no thumb).
fn rounded_progress(value: f64, fill: ThemeRef) -> Element {
    const HEIGHT: f64 = 6.0;
    let radius = HEIGHT / 2.0;
    let filled = value.clamp(0.0, 100.0);
    let (fill_star, rest_star) = if filled <= 0.0 {
        (0.0001, 100.0)
    } else if filled >= 100.0 {
        (100.0, 0.0001)
    } else {
        (filled, 100.0 - filled)
    };

    border(
        grid((border(Element::Empty)
            .background(fill)
            .corner_radius(radius)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch)
            .grid_column(0),))
        .columns([GridLength::Star(fill_star), GridLength::Star(rest_star)])
        .rows([GridLength::Star(1.0)])
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Stretch),
    )
    .background(Color {
        a: 70,
        r: 255,
        g: 255,
        b: 255,
    })
    .corner_radius(radius)
    .height(HEIGHT)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn limit_card(title: &str, window: &LimitWindow, show_used_percentage: bool) -> Element {
    let remaining = window.remaining_percent();
    let accent = ThemeRef::SystemAttention;
    let percentage = if show_used_percentage {
        window.used_percent
    } else {
        remaining
    };
    let suffix = if show_used_percentage { "used" } else { "left" };
    let remaining_label = percentage
        .map(|value| format!("{value}% {suffix}"))
        .unwrap_or_else(|| "Unavailable".into());
    let progress = f64::from(percentage.unwrap_or(0));
    let reset = format_reset_in(window.resets_at);

    border(
        vstack((
            grid((caption(title.to_uppercase()).foreground(ThemeRef::SecondaryText),))
                .columns([GridLength::Star(1.0)])
                .rows([GridLength::Auto])
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .vertical_alignment(VerticalAlignment::Center),
            rounded_progress(progress, accent.clone()),
            grid((
                hstack((text_block(remaining_label)
                    .font_weight(600)
                    .foreground(accent)
                    .vertical_alignment(VerticalAlignment::Center),))
                .vertical_alignment(VerticalAlignment::Center),
                hstack((
                    text_block("Resets in")
                        .foreground(ThemeRef::TertiaryText)
                        .vertical_alignment(VerticalAlignment::Center),
                    text_block(reset).vertical_alignment(VerticalAlignment::Center),
                ))
                .spacing(6.0)
                .horizontal_alignment(HorizontalAlignment::Right)
                .vertical_alignment(VerticalAlignment::Center),
            ))
            .columns([GridLength::Star(1.0), GridLength::Auto])
            .rows([GridLength::Auto])
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Center),
        ))
        .spacing(8.0),
    )
    .corner_radius(f64::from(popup::WINDOW_CORNER_RADIUS_DIP))
    .padding(Thickness::uniform(12.0))
    .background(SURFACE_FILL)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .into()
}

fn meta_row(limits: &RateLimits) -> Element {
    grid((
        hstack((
            text_block("PLAN")
                .foreground(ThemeRef::TertiaryText)
                .vertical_alignment(VerticalAlignment::Center),
            text_block(
                limits
                    .plan_type
                    .as_deref()
                    .unwrap_or("Unavailable")
                    .to_uppercase(),
            )
            .bold()
            .vertical_alignment(VerticalAlignment::Center),
        ))
        .spacing(8.0)
        .vertical_alignment(VerticalAlignment::Center),
        hstack((
            text_block("CREDITS")
                .foreground(ThemeRef::TertiaryText)
                .vertical_alignment(VerticalAlignment::Center),
            text_block(credits_label(limits))
                .bold()
                .vertical_alignment(VerticalAlignment::Center),
        ))
        .spacing(8.0)
        .vertical_alignment(VerticalAlignment::Center)
        .horizontal_alignment(HorizontalAlignment::Right),
    ))
    .columns([GridLength::Star(1.0), GridLength::Auto])
    .rows([GridLength::Auto])
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Center)
    .into()
}

fn credits_label(limits: &RateLimits) -> String {
    if limits.credits.unlimited {
        "Unlimited".into()
    } else if limits.credits.has_credits {
        limits
            .credits
            .balance
            .clone()
            .unwrap_or_else(|| "Available".into())
    } else {
        "None".into()
    }
}

fn format_reset_in(reset: Option<DateTime<Utc>>) -> String {
    let Some(reset) = reset else {
        return "Unavailable".into();
    };

    let remaining_minutes = (reset - Utc::now()).num_minutes().max(0);
    let days = remaining_minutes / 1_440;
    let hours = (remaining_minutes % 1_440) / 60;
    let minutes = remaining_minutes % 60;

    if days > 0 {
        if hours > 0 {
            format!("{days}d {hours}h")
        } else {
            format!("{days}d")
        }
    } else if hours > 0 {
        if minutes > 0 {
            format!("{hours}h {minutes}m")
        } else {
            format!("{hours}h")
        }
    } else {
        format!("{minutes}m")
    }
}

fn sample_freshness(sampled_at: DateTime<Utc>) -> String {
    if sampled_at.timestamp() == 0 {
        return "Waiting for Codex...".into();
    }
    let seconds = (Utc::now() - sampled_at).num_seconds().max(0);
    match seconds {
        0..=4 => "Just now".into(),
        5..=59 => format!("{seconds} seconds ago"),
        _ => format!("{} minutes ago", seconds / 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_activation_uses_window_start() {
        let primary = LimitWindow {
            used_percent: Some(1),
            resets_at: Some(
                chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 7, 10, 16, 8, 0).unwrap(),
            ),
            duration_minutes: Some(300),
        };
        assert_eq!(
            window_started_at(&primary),
            Some(chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 7, 10, 11, 8, 0).unwrap())
        );
        assert_eq!(
            format_last_activation(&RateLimits::default(), None),
            "Never"
        );
    }

    #[test]
    fn unavailable_sample_has_clear_copy() {
        assert_eq!(
            sample_freshness(DateTime::default()),
            "Waiting for Codex..."
        );
        assert_eq!(format_reset_in(None), "Unavailable");
    }
}
