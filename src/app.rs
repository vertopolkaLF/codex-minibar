use std::{
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender},
    },
    thread,
    time::Duration,
};

use chrono::{DateTime, Local, Utc};
use windows_reactor::*;

use crate::{
    limits::{LimitWindow, RateLimits},
    popup,
    settings::Settings,
    tray::TrayManager,
    worker::{WorkerCommand, WorkerEvent},
};

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
            last_activation: "No activation attempt in this session".into(),
            error: None,
        }
    }
}

/// Root WinUI view for Codex Minibar (hosted in a tray popup shell).
pub fn app(cx: &mut RenderCx, state: Arc<AppState>) -> Element {
    let (ui, set_ui) = cx.use_async_state(UiState {
        error: state.startup_error.clone(),
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
        top: 12.0,
        right: 18.0,
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

    border(
        grid((
            vstack(body)
                .spacing(12.0)
                .padding(Thickness::uniform(16.0))
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .grid_row(0),
            footer.grid_row(1),
        ))
        .rows([GridLength::Star(1.0), GridLength::Auto])
        .columns([GridLength::Star(1.0)])
        .background(Color::transparent())
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Stretch),
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
        let mut ui = UiState {
            error: state.startup_error.clone(),
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
                    ui.limits = limits;
                    set_ui.call(ui.clone());
                }
                Ok(WorkerEvent::ActivationSucceeded) => {
                    ui.last_activation =
                        format!("Succeeded at {}", Local::now().format("%H:%M:%S %d.%m.%Y"));
                    set_ui.call(ui.clone());
                }
                Ok(WorkerEvent::ActivationFailed(error)) => {
                    ui.last_activation = format!("Failed: {error}");
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

const ICON_BUTTON_SIZE: f64 = 32.0;

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
                    .foreground(color)
                    .horizontal_alignment(HorizontalAlignment::Right),
            ))
            .spacing(8.0),
            ProgressBar::new(progress).range(0.0, 100.0),
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
