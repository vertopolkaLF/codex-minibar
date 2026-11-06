//! Visual check of the taskbar widgets with sample quotas.
//!
//! `cargo run --example taskbar_probe -- [seconds] [style...]` draws widgets
//! into the real taskbar, changes the values every few seconds and exits.

#[cfg(windows)]
fn main() {
    use chrono::{Duration, Utc};
    use codex_minibar::{
        instances::ProviderId,
        limits::{LimitWindow, ProviderLimits, RateLimits},
        provider_registry::ProviderKind,
        settings::{Settings, TaskbarWidget, TaskbarWidgetStyle, TrayIndicator},
    };

    unsafe {
        windows_sys::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
            windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
    }
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let seconds = args
        .first()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(20);
    let styles = args
        .iter()
        .skip(1)
        .filter_map(|value| match value.as_str() {
            "glance" => Some(TaskbarWidgetStyle::Glance),
            "rings" => Some(TaskbarWidgetStyle::Rings),
            "bars" => Some(TaskbarWidgetStyle::Bars),
            "nested" => Some(TaskbarWidgetStyle::Nested),
            "dials" => Some(TaskbarWidgetStyle::Dials),
            "meters" => Some(TaskbarWidgetStyle::Meters),
            _ => None,
        })
        .collect::<Vec<_>>();
    let styles = if styles.is_empty() {
        TaskbarWidgetStyle::ALL.to_vec()
    } else {
        styles
    };

    let mut settings = Settings::default();
    for instance in &mut settings.instances {
        instance.enabled = matches!(instance.driver, ProviderKind::Codex | ProviderKind::Claude);
    }
    // Not `apply_runtime_effects`: that would register this binary at login.
    codex_minibar::instances::publish(&settings.instances);
    let codex = ProviderId::primary(ProviderKind::Codex);
    let claude = ProviderId::primary(ProviderKind::Claude);
    settings.taskbar.enabled = true;
    settings.taskbar.widgets = styles
        .iter()
        .enumerate()
        .map(|(index, style)| {
            let provider = if index % 2 == 0 { codex } else { claude };
            let mut widget = TaskbarWidget::for_provider(provider);
            widget.style = *style;
            if *style == TaskbarWidgetStyle::Glance && index > 0 {
                widget.indicators.truncate(1);
            }
            if *style == TaskbarWidgetStyle::Bars {
                widget
                    .indicators
                    .push(TrayIndicator::new(codex, "codex.session"));
            }
            widget
        })
        .collect();

    let limits = |step: u8| {
        let window = |used: u8, minutes: i64| LimitWindow {
            used_percent: Some(used),
            resets_at: Some(Utc::now() + Duration::minutes(minutes)),
            duration_minutes: Some(300),
        };
        ProviderLimits::from_entries([
            (
                codex,
                RateLimits {
                    primary: window(58u8.saturating_add(step * 9) % 101, 133),
                    secondary: window(30 + step * 4, 3 * 1440 + 240),
                    ..RateLimits::default()
                },
            ),
            (
                claude,
                RateLimits {
                    primary: window(12 + step * 20, 47),
                    secondary: window(88u8.saturating_sub(step * 30), 5 * 1440),
                    ..RateLimits::default()
                },
            ),
        ])
    };

    codex_minibar::taskbar::publish_limits(&limits(0));
    codex_minibar::taskbar::publish_settings(&settings);
    let started = std::time::Instant::now();
    let mut step = 0u8;
    while started.elapsed().as_secs() < seconds {
        std::thread::sleep(std::time::Duration::from_secs(4));
        step = (step + 1) % 4;
        codex_minibar::taskbar::publish_limits(&limits(step));
        for action in codex_minibar::taskbar::take_actions() {
            eprintln!("action: {action:?}");
        }
    }
    settings.taskbar.enabled = false;
    codex_minibar::taskbar::publish_settings(&settings);
    std::thread::sleep(std::time::Duration::from_millis(400));
}

#[cfg(not(windows))]
fn main() {}
