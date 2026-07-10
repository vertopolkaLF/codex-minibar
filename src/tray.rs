use chrono::{DateTime, Local, Utc};
use font8x8::{BASIC_FONTS, UnicodeFonts};

use crate::{
    limits::{LimitWindow, RateLimits},
    settings::{TrayMetric, TrayWidget},
};

const ICON_SIZE: usize = 32;

pub fn tooltip(limits: &RateLimits) -> String {
    format!(
        "5h  |  {}  |  {}\n7d  |  {}  |  {}",
        format_remaining(&limits.primary),
        format_reset(limits.primary.resets_at),
        format_remaining(&limits.secondary),
        format_reset(limits.secondary.resets_at),
    )
}

fn format_remaining(window: &LimitWindow) -> String {
    window
        .remaining_percent()
        .map(|value| format!("{value}%"))
        .unwrap_or_else(|| "?".into())
}

fn format_reset(reset: Option<DateTime<Utc>>) -> String {
    reset
        .map(|value| {
            value
                .with_timezone(&Local)
                .format("%H:%M %d.%m")
                .to_string()
        })
        .unwrap_or_else(|| "?".into())
}

fn label(widget: &TrayWidget, limits: &RateLimits) -> String {
    match widget.metric {
        TrayMetric::PrimaryRemaining => limits
            .primary
            .remaining_percent()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "?".into()),
        TrayMetric::PrimaryReset => reset_label(limits.primary.resets_at),
        TrayMetric::SecondaryRemaining => limits
            .secondary
            .remaining_percent()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "?".into()),
        TrayMetric::SecondaryReset => reset_label(limits.secondary.resets_at),
        TrayMetric::Combined => format!(
            "{}|{}",
            limits
                .primary
                .remaining_percent()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".into()),
            limits
                .secondary
                .remaining_percent()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".into())
        ),
    }
}

fn reset_label(reset: Option<DateTime<Utc>>) -> String {
    reset
        .map(|value| value.with_timezone(&Local).format("%H%M").to_string())
        .unwrap_or_else(|| "?".into())
}

fn color(widget: &TrayWidget, limits: &RateLimits) -> [u8; 4] {
    let remaining = match widget.metric {
        TrayMetric::PrimaryRemaining | TrayMetric::PrimaryReset => {
            limits.primary.remaining_percent()
        }
        TrayMetric::SecondaryRemaining | TrayMetric::SecondaryReset => {
            limits.secondary.remaining_percent()
        }
        TrayMetric::Combined => limits.primary.remaining_percent(),
    };
    match remaining {
        Some(0..=15) => [230, 74, 72, 255],
        Some(16..=50) => [245, 158, 11, 255],
        Some(_) => [49, 196, 141, 255],
        None => [180, 180, 180, 255],
    }
}

pub fn render_widget(widget: &TrayWidget, limits: &RateLimits) -> Vec<u8> {
    let text = label(widget, limits);
    let scale = if text.chars().count() <= 2 { 3 } else { 1 };
    let glyph_width = 8 * scale;
    let total_width = glyph_width * text.chars().count();
    let start_x = ICON_SIZE.saturating_sub(total_width) / 2;
    let start_y = ICON_SIZE.saturating_sub(8 * scale) / 2;
    let rgba = color(widget, limits);
    let mut pixels = vec![0; ICON_SIZE * ICON_SIZE * 4];
    for (index, character) in text.chars().enumerate() {
        let Some(glyph) = BASIC_FONTS.get(character) else {
            continue;
        };
        for (row, bits) in glyph.iter().copied().enumerate() {
            for column in 0..8 {
                if bits & (1 << column) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let x = start_x + index * glyph_width + column * scale + dx;
                        let y = start_y + row * scale + dy;
                        if x < ICON_SIZE && y < ICON_SIZE {
                            let offset = (y * ICON_SIZE + x) * 4;
                            pixels[offset..offset + 4].copy_from_slice(&rgba);
                        }
                    }
                }
            }
        }
    }
    pixels
}

#[cfg(windows)]
mod platform {
    use anyhow::{Context, Result};
    use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconId};

    use super::*;

    pub struct TrayManager {
        icons: Vec<TrayIcon>,
    }

    impl TrayManager {
        pub fn new() -> Self {
            Self { icons: Vec::new() }
        }

        pub fn sync(&mut self, widgets: &[TrayWidget], limits: &RateLimits) -> Result<()> {
            self.icons.truncate(widgets.len());
            while self.icons.len() < widgets.len() {
                let index = self.icons.len();
                let icon = make_icon(&widgets[index], limits)?;
                let tray = TrayIconBuilder::new()
                    .with_icon(icon)
                    .with_tooltip(tooltip(limits))
                    .with_menu_on_left_click(false)
                    .build()
                    .context("create tray icon")?;
                self.icons.push(tray);
            }
            let tooltip = tooltip(limits);
            for (tray, widget) in self.icons.iter().zip(widgets) {
                tray.set_icon(Some(make_icon(widget, limits)?))?;
                tray.set_tooltip(Some(&tooltip))?;
            }
            Ok(())
        }

        pub fn contains(&self, id: &TrayIconId) -> bool {
            self.icons.iter().any(|icon| icon.id() == id)
        }

        pub fn len(&self) -> usize {
            self.icons.len()
        }

        pub fn is_empty(&self) -> bool {
            self.icons.is_empty()
        }
    }

    impl Default for TrayManager {
        fn default() -> Self {
            Self::new()
        }
    }

    fn make_icon(widget: &TrayWidget, limits: &RateLimits) -> Result<Icon> {
        Icon::from_rgba(
            render_widget(widget, limits),
            ICON_SIZE as u32,
            ICON_SIZE as u32,
        )
        .context("create RGBA tray icon")
    }
}

#[cfg(windows)]
pub use platform::TrayManager;

#[cfg(not(windows))]
pub struct TrayManager;

#[cfg(not(windows))]
impl TrayManager {
    pub fn new() -> Self {
        Self
    }

    pub fn sync(&mut self, _widgets: &[TrayWidget], _limits: &RateLimits) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn len(&self) -> usize {
        0
    }

    pub fn is_empty(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn limits() -> RateLimits {
        RateLimits {
            primary: LimitWindow {
                used_percent: Some(27),
                resets_at: Some(Utc.timestamp_opt(1_700_003_600, 0).unwrap()),
                duration_minutes: Some(300),
            },
            secondary: LimitWindow {
                used_percent: Some(61),
                resets_at: None,
                duration_minutes: Some(10_080),
            },
            sampled_at: Utc::now(),
            ..RateLimits::default()
        }
    }

    #[test]
    fn renders_rgba_icon_with_visible_pixels() {
        let widget = TrayWidget {
            metric: TrayMetric::PrimaryRemaining,
        };
        let pixels = render_widget(&widget, &limits());
        assert_eq!(pixels.len(), ICON_SIZE * ICON_SIZE * 4);
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
    }

    #[test]
    fn tooltip_contains_both_windows() {
        let value = tooltip(&limits());
        assert!(value.contains("5h  |  73%"));
        assert!(value.contains("7d  |  39%"));
    }
}
