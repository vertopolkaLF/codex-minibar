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
    tray::TrayManager,
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
        (Some(reset), Some(minutes)) => {
            Some(reset - ChronoDuration::minutes(i64::from(minutes)))
        }
        _ => None,
    }
}

fn format_last_activation(
    limits: &RateLimits,
    fallback_attempt: Option<DateTime<Utc>>,
) -> String {
    window_started_at(&limits.primary)
        .or(fallback_attempt)
        .map(format_activation_at)
        .unwrap_or_else(|| "Never".into())
}

/// Soft translucent fill so Acrylic shows through cards and the footer.
const SURFACE_FILL: Color = Color {
    a: 10,
    r: 255,
    g: 255,
    b: 255,
};
/// Soft translucent fill so Acrylic shows through cards and the footer.
const DARK_SURFACE_FILL: Color = Color {
    a: 70,
    r: 0,
    g: 0,
    b: 0,
};
/// Window outline: CSS `#fff4` → `#ffffff44`.
const WINDOW_BORDER: Color = Color {
    a: 10,
    r: 255,
    g: 255,
    b: 255,
};

/// Shared startup state handed from `main` into the reactor render tree.
pub struct AppState {
    pub settings: Settings,
    pub commands: Option<Sender<WorkerCommand>>,
    pub events: Mutex<Option<Receiver<WorkerEvent>>>,
    pub startup_error: Option<String>,
    /// Last activation attempt loaded from persisted activation state.
    pub last_activation_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq)]
struct UiState {
    limits: RateLimits,
    last_activation: String,
    error: Option<String>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            limits: RateLimits::default(),
            last_activation: "Never".into(),
            error: None,
        }
    }
}

/// Root WinUI view for Codex Minibar (hosted in a tray popup shell).
pub fn app(cx: &mut RenderCx, state: Arc<AppState>) -> Element {
    let (ui, set_ui) = cx.use_async_state(UiState {
        error: state.startup_error.clone(),
        last_activation: format_last_activation(&RateLimits::default(), state.last_activation_at),
        ..UiState::default()
    });
    let commands = state.commands.clone();

    cx.use_effect((), {
        let state = Arc::clone(&state);
        let set_ui = set_ui.clone();
        move || {
            // Convert the WinUI window into a hidden tray popup as soon as it exists.
            let _ = popup::ensure_configured();
            // Restyling can detach SystemBackdrop — re-apply Acrylic on the UI thread.
            set_backdrop(Some(Backdrop::Acrylic));
            start_background_bridge(state, set_ui);
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
        limit_card("5 hour window", &ui.limits.primary),
        limit_card("7 day window", &ui.limits.secondary),
        meta_row(&ui.limits),
        status_card(&ui),
    ];

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
            body_strong("Codex Minibar")
                .foreground(ThemeRef::SecondaryText)
                .vertical_alignment(VerticalAlignment::Center)
                .horizontal_alignment(HorizontalAlignment::Left)
                .grid_column(0),
            hstack((
                icon_button("\u{E72C}", "Refresh", 16.0, refresh),
                icon_button("\u{E713}", "Settings", 16.0, || {}),
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
        bottom: 12.0,
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

    // Content on top (Auto), footer pinned to the bottom. Any leftover height
    // stays between them only if the window is taller than the stack — keep
    // POPUP_HEIGHT matched to content so that gap stays ~0.
    border(
        grid((
            vstack(body)
                .spacing(12.0)
                .padding(Thickness {
                    left: 16.0,
                    top: 16.0,
                    right: 16.0,
                    bottom: 16.0,
                })
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .vertical_alignment(VerticalAlignment::Top)
                .grid_row(0),
            footer.grid_row(1),
        ))
        .rows([GridLength::Auto, GridLength::Auto])
        .columns([GridLength::Star(1.0)])
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Stretch)
        .background(Color::transparent()),
    )
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(WINDOW_BORDER)
    .corner_radius(9.0)
    .background(Color::transparent())
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Stretch)
    .into()
}

fn start_background_bridge(state: Arc<AppState>, set_ui: AsyncSetState<UiState>) {
    let events = state.events.lock().ok().and_then(|mut slot| slot.take());
    let widgets = state.settings.tray_widgets.clone();

    thread::spawn(move || {
        let mut tray = TrayManager::new();
        let fallback_attempt = state.last_activation_at;
        let mut ui = UiState {
            error: state.startup_error.clone(),
            last_activation: format_last_activation(&RateLimits::default(), fallback_attempt),
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

        let Some(events) = events else {
            set_ui.call(ui);
            loop {
                popup::pump_messages();
                pump_tray_and_dismiss(&tray);
                thread::sleep(Duration::from_millis(16));
            }
        };

        loop {
            popup::pump_messages();
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
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    pump_tray_and_dismiss(&tray);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

#[cfg(windows)]
fn pump_tray_and_dismiss(tray: &TrayManager) {
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

    if popup::clicked_outside() {
        popup::hide();
    }
}

#[cfg(not(windows))]
fn pump_tray_and_dismiss(_tray: &TrayManager) {}

const ICON_BUTTON_SIZE: f64 = 36.0;

/// Icon-only button using Segoe Fluent Icons glyphs.
/// `font_size` is tuned per glyph so they look optically equal.
fn icon_button(
    glyph: &str,
    tip: &str,
    font_size: f64,
    on_click: impl IntoUnitCallback,
) -> Button {
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
        grid((
            border(Element::Empty)
                .background(fill)
                .corner_radius(radius)
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .vertical_alignment(VerticalAlignment::Stretch)
                .grid_column(0),
        ))
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

fn limit_card(title: &str, window: &LimitWindow) -> Element {
    let remaining = window.remaining_percent();
    let color = remaining_theme(remaining);
    let remaining_label = remaining
        .map(|value| format!("{value}% left"))
        .unwrap_or_else(|| "Unavailable".into());
    let progress = f64::from(remaining.unwrap_or(0));
    let used = window
        .used_percent
        .map(|value| format!("{value}%"))
        .unwrap_or_else(|| "?".into());
    let reset = format_reset(window.resets_at);
    let duration = window
        .duration_minutes
        .map(format_duration)
        .unwrap_or_default();

    border(
        vstack((
            hstack((
                caption(title.to_uppercase()).foreground(ThemeRef::SecondaryText),
                text_block(remaining_label)
                    .bold()
                    .foreground(color.clone())
                    .horizontal_alignment(HorizontalAlignment::Right),
            ))
            .spacing(8.0),
            rounded_progress(progress, color),
            hstack((
                caption("Used").foreground(ThemeRef::TertiaryText),
                text_block(used),
                text_block("·").foreground(ThemeRef::TertiaryText),
                caption("Resets").foreground(ThemeRef::TertiaryText),
                text_block(reset),
                text_block(duration).foreground(ThemeRef::SecondaryText),
            ))
            .spacing(6.0),
        ))
        .spacing(8.0),
    )
    .corner_radius(8.0)
    .padding(Thickness::uniform(12.0))
    .background(SURFACE_FILL)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .into()
}

fn meta_row(limits: &RateLimits) -> Element {
    hstack((
        caption("PLAN").foreground(ThemeRef::TertiaryText),
        text_block(
            limits
                .plan_type
                .as_deref()
                .unwrap_or("Unavailable")
                .to_uppercase(),
        )
        .bold(),
        text_block("·").foreground(ThemeRef::TertiaryText),
        caption("CREDITS").foreground(ThemeRef::TertiaryText),
        text_block(credits_label(limits)).bold(),
    ))
    .spacing(8.0)
    .into()
}

fn status_card(ui: &UiState) -> Element {
    border(
        vstack((
            caption("LATEST SAMPLE").foreground(ThemeRef::TertiaryText),
            text_block(sample_freshness(ui.limits.sampled_at)),
            caption("LAST ACTIVATION").foreground(ThemeRef::TertiaryText),
            text_block(ui.last_activation.clone()),
        ))
        .spacing(6.0),
    )
    .corner_radius(8.0)
    .padding(Thickness::uniform(12.0))
    .background(SURFACE_FILL)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
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

fn format_duration(minutes: u32) -> String {
    if minutes.is_multiple_of(1_440) {
        format!("{}d window", minutes / 1_440)
    } else if minutes.is_multiple_of(60) {
        format!("{}h window", minutes / 60)
    } else {
        format!("{minutes}m window")
    }
}

fn remaining_theme(remaining: Option<u8>) -> ThemeRef {
    match remaining {
        Some(0..=15) => ThemeRef::SystemCritical,
        Some(16..=50) => ThemeRef::SystemCaution,
        Some(_) => ThemeRef::SystemSuccess,
        None => ThemeRef::TertiaryText,
    }
}

fn format_reset(reset: Option<DateTime<Utc>>) -> String {
    reset
        .map(|value| {
            value
                .with_timezone(&Local)
                .format("%H:%M, %d %b")
                .to_string()
        })
        .unwrap_or_else(|| "Unavailable".into())
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
        assert_eq!(format_reset(None), "Unavailable");
    }

    #[test]
    fn themes_follow_remaining_thresholds() {
        assert_eq!(remaining_theme(Some(10)), ThemeRef::SystemCritical);
        assert_eq!(remaining_theme(Some(30)), ThemeRef::SystemCaution);
        assert_eq!(remaining_theme(Some(80)), ThemeRef::SystemSuccess);
    }
}
