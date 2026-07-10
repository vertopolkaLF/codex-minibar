use chrono::{DateTime, Local, Utc};
use std::{fs, path::PathBuf, sync::OnceLock};

use font8x8::{BASIC_FONTS, UnicodeFonts};
use fontdue::{
    Font, FontSettings,
    layout::{CoordinateSystem, HorizontalAlign, Layout, LayoutSettings, TextStyle, VerticalAlign},
};

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
        TrayMetric::PrimaryReset => stacked_reset_label(limits.primary.resets_at),
        TrayMetric::SecondaryRemaining => limits
            .secondary
            .remaining_percent()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "?".into()),
        TrayMetric::SecondaryReset => stacked_reset_label(limits.secondary.resets_at),
        TrayMetric::Combined => format!(
            "{}\n{}",
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

fn stacked_reset_label(reset: Option<DateTime<Utc>>) -> String {
    reset
        .map(|value| value.with_timezone(&Local).format("%H\n%M").to_string())
        .unwrap_or_else(|| "?".into())
}

fn icon_color(widget: &TrayWidget, limits: &RateLimits) -> [u8; 3] {
    let remaining = match widget.metric {
        TrayMetric::PrimaryRemaining => limits.primary.remaining_percent(),
        TrayMetric::SecondaryRemaining => limits.secondary.remaining_percent(),
        TrayMetric::Combined => limits.primary.remaining_percent(),
        TrayMetric::PrimaryReset | TrayMetric::SecondaryReset => return [255, 255, 255],
    };
    match remaining {
        Some(0..=15) => [230, 74, 72],
        Some(16..=50) => [245, 158, 11],
        Some(_) => [49, 196, 141],
        None => [180, 180, 180],
    }
}

fn system_font() -> Option<&'static Font> {
    static FONT: OnceLock<Option<Font>> = OnceLock::new();
    FONT.get_or_init(|| {
        font_candidates().into_iter().find_map(|path| {
            fs::read(path)
                .ok()
                .and_then(|bytes| Font::from_bytes(bytes, FontSettings::default()).ok())
        })
    })
    .as_ref()
}

pub fn render_widget(widget: &TrayWidget, limits: &RateLimits) -> Vec<u8> {
    let text = label(widget, limits);
    let rgb = icon_color(widget, limits);
    let Some(font) = system_font() else {
        return render_fallback(&text, rgb);
    };
    let lines: Vec<_> = text.lines().collect();
    let two_lines = lines.len() > 1;
    let font_size = if two_lines {
        15.0
    } else if text.chars().count() <= 2 {
        24.0
    } else {
        20.0
    };
    let fonts = [font.clone()];
    let mut pixels = vec![0; ICON_SIZE * ICON_SIZE * 4];
    let line_height = if two_lines { 15.0 } else { ICON_SIZE as f32 };
    for (line_index, line) in lines.iter().enumerate() {
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            y: line_index as f32 * line_height,
            max_width: Some(ICON_SIZE as f32),
            max_height: Some(line_height),
            horizontal_align: HorizontalAlign::Center,
            vertical_align: VerticalAlign::Middle,
            ..LayoutSettings::default()
        });
        layout.append(&fonts, &TextStyle::new(line, font_size, 0));
        for glyph in layout.glyphs() {
            let (metrics, coverage) = fonts[glyph.font_index].rasterize_config(glyph.key);
            for row in 0..metrics.height {
                for column in 0..metrics.width {
                    let x = glyph.x.round() as isize + column as isize;
                    let y = glyph.y.round() as isize + row as isize;
                    if x < 0 || y < 0 || x >= ICON_SIZE as isize || y >= ICON_SIZE as isize {
                        continue;
                    }
                    let alpha = coverage[row * metrics.width + column];
                    let offset = (y as usize * ICON_SIZE + x as usize) * 4;
                    pixels[offset..offset + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], alpha]);
                }
            }
        }
    }
    pixels
}

fn font_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(windows) = std::env::var_os("WINDIR") {
        let fonts = PathBuf::from(windows).join("Fonts");
        candidates.extend([
            fonts.join("seguisb.ttf"),
            fonts.join("segoeuib.ttf"),
            fonts.join("arialbd.ttf"),
        ]);
    }
    candidates.extend([
        PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"),
        PathBuf::from("/System/Library/Fonts/Supplemental/Arial Bold.ttf"),
    ]);
    candidates
}

fn render_fallback(text: &str, rgb: [u8; 3]) -> Vec<u8> {
    let text = text.replace('\n', "");
    let scale = if text.chars().count() <= 2 { 3 } else { 1 };
    let glyph_width = 8 * scale;
    let total_width = glyph_width * text.chars().count();
    let start_x = ICON_SIZE.saturating_sub(total_width) / 2;
    let start_y = ICON_SIZE.saturating_sub(8 * scale) / 2;
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
                            pixels[offset..offset + 4]
                                .copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
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
    use tray_icon::{
        Icon, TrayIcon, TrayIconBuilder, TrayIconId,
        menu::{IconMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    };

    use super::*;

    pub struct TrayManager {
        icons: Vec<TrayIcon>,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum TrayMenuAction {
        Settings,
        Exit,
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
                    .with_menu(Box::new(make_menu()?))
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

        pub fn drain_menu_actions(&self) -> Vec<TrayMenuAction> {
            let mut actions = Vec::new();
            while let Ok(event) = MenuEvent::receiver().try_recv() {
                match event.id.as_ref() {
                    "settings" => actions.push(TrayMenuAction::Settings),
                    "exit" => actions.push(TrayMenuAction::Exit),
                    _ => {}
                }
            }
            actions
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

    fn make_menu() -> Result<Menu> {
        let header = IconMenuItem::new(
            format!("Codex Minibar - v{}", env!("CARGO_PKG_VERSION")),
            false,
            None,
            None,
        );
        let settings = MenuItem::with_id("settings", "Settings", true, None);
        let exit = MenuItem::with_id("exit", "Exit", true, None);
        Menu::with_items(&[
            &header,
            &PredefinedMenuItem::separator(),
            &settings,
            &exit,
        ])
        .context("create tray menu")
    }
}

#[cfg(windows)]
pub use platform::TrayManager;
#[cfg(windows)]
pub use platform::TrayMenuAction;

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

    pub fn drain_menu_actions(&self) -> Vec<TrayMenuAction> {
        Vec::new()
    }
}

#[cfg(not(windows))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayMenuAction {
    Settings,
    Exit,
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

    #[test]
    fn reset_icons_are_white_and_percentage_icons_are_colored() {
        let reset = TrayWidget {
            metric: TrayMetric::PrimaryReset,
        };
        let percentage = TrayWidget {
            metric: TrayMetric::PrimaryRemaining,
        };
        assert_eq!(icon_color(&reset, &limits()), [255, 255, 255]);
        assert_eq!(icon_color(&percentage, &limits()), [49, 196, 141]);
    }
}
