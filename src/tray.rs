use chrono::{DateTime, Local, Utc};
use std::{fs, path::PathBuf, sync::OnceLock};

use font8x8::{BASIC_FONTS, UnicodeFonts};
use fontdue::{
    Font, FontSettings,
    layout::{CoordinateSystem, HorizontalAlign, Layout, LayoutSettings, TextStyle, VerticalAlign},
};

use crate::{
    limits::{LimitWindow, RateLimits},
    settings::{LimitValue, TrayPresentation, TraySource, TrayWidget},
};

const ICON_SIZE: usize = 32;

pub fn tooltip(limits: &RateLimits) -> String {
    let five_hour = if limits.five_hour_disabled() {
        "Disabled".to_string()
    } else {
        format_remaining(&limits.primary)
    };
    let five_hour_reset = if limits.five_hour_disabled() {
        "—".to_string()
    } else {
        format_reset(limits.primary.resets_at)
    };
    format!(
        "5h  |  {}  |  {}\n7d  |  {}  |  {}",
        five_hour,
        five_hour_reset,
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

fn percent(window: &LimitWindow, value: LimitValue) -> Option<u8> {
    match value {
        LimitValue::Remaining => window.remaining_percent(),
        LimitValue::Used => window.used_percent.map(|value| value.min(100)),
    }
}

fn stacked_reset_label(reset: Option<DateTime<Utc>>, countdown: bool) -> String {
    if countdown {
        return reset.map_or_else(
            || "?".into(),
            |value| {
                let minutes = (value - Utc::now()).num_minutes().max(0);
                if minutes < 60 {
                    minutes.to_string()
                } else {
                    format!("{}\n{:02}", minutes / 60, minutes % 60)
                }
            },
        );
    }
    reset
        .map(|value| value.with_timezone(&Local).format("%H\n%M").to_string())
        .unwrap_or_else(|| "?".into())
}

fn icon_color(value: Option<u8>, limit_value: LimitValue) -> [u8; 3] {
    // Remaining: low is bad. Used: high is bad — color against remaining-equivalent.
    let severity = match limit_value {
        LimitValue::Used => value.map(|used| 100u8.saturating_sub(used.min(100))),
        LimitValue::Remaining => value,
    };
    match severity {
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
    if !has_real_data(limits) {
        return app_icon_pixels().to_vec();
    }
    let five_hour_disabled = limits.five_hour_disabled();
    let primary_window = limits.effective_primary();
    let primary = percent(primary_window, widget.limit_value);
    let secondary = percent(&limits.secondary, widget.limit_value);
    // When the 5h window is gone, primary-oriented widgets show weekly instead.
    let limit_value = widget.limit_value;
    if five_hour_disabled
        && matches!(widget.source, TraySource::Combined | TraySource::Primary)
        && matches!(
            widget.presentation,
            TrayPresentation::StackedBars
                | TrayPresentation::NestedRings
                | TrayPresentation::StackedNumbers
        )
    {
        return match widget.presentation {
            TrayPresentation::StackedBars => render_bars(&[secondary], limit_value),
            TrayPresentation::NestedRings => render_rings(&[secondary], limit_value),
            _ => {
                let text = secondary.map_or_else(|| "?".into(), |v| v.to_string());
                render_text_icon(&text, icon_color(secondary, limit_value))
            }
        };
    }
    if matches!(widget.presentation, TrayPresentation::StackedBars) {
        return render_bars(&[primary, secondary], limit_value);
    }
    if matches!(widget.presentation, TrayPresentation::NestedRings) {
        return render_rings(&[primary, secondary], limit_value);
    }
    if matches!(widget.presentation, TrayPresentation::Bar) {
        let value = match widget.source {
            TraySource::Secondary => secondary,
            _ => primary,
        };
        return render_bars(&[value], limit_value);
    }
    if matches!(widget.presentation, TrayPresentation::Ring) {
        let value = match widget.source {
            TraySource::Secondary => secondary,
            _ => primary,
        };
        return render_rings(&[value], limit_value);
    }
    let (text, rgb) = match widget.source {
        TraySource::Combined => (
            format!(
                "{}\n{}",
                primary.map_or_else(|| "?".into(), |v| v.to_string()),
                secondary.map_or_else(|| "?".into(), |v| v.to_string())
            ),
            icon_color(primary, limit_value),
        ),
        TraySource::Primary => (
            primary.map_or_else(|| "?".into(), |v| v.to_string()),
            icon_color(primary, limit_value),
        ),
        TraySource::Secondary => (
            secondary.map_or_else(|| "?".into(), |v| v.to_string()),
            icon_color(secondary, limit_value),
        ),
        TraySource::PrimaryReset => (
            stacked_reset_label(
                primary_window.resets_at,
                matches!(widget.presentation, TrayPresentation::ResetCountdown),
            ),
            [255, 255, 255],
        ),
    };
    render_text_icon(&text, rgb)
}

fn render_text_icon(text: &str, rgb: [u8; 3]) -> Vec<u8> {
    let Some(font) = system_font() else {
        return render_fallback(text, rgb);
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

fn render_bars(values: &[Option<u8>], limit_value: LimitValue) -> Vec<u8> {
    let mut pixels = vec![0; ICON_SIZE * ICON_SIZE * 4];
    let count = values.len();
    for (index, value) in values.iter().enumerate() {
        let height = if count == 1 { 8 } else { 6 };
        let y = if count == 1 { 12 } else { 7 + index * 13 };
        let filled = value.unwrap_or(0) as usize * 24 / 100;
        let color = icon_color(*value, limit_value);
        for yy in y..y + height {
            for x in 4..28 {
                let offset = (yy * ICON_SIZE + x) * 4;
                let rgb = if x - 4 < filled { color } else { [70, 70, 70] };
                pixels[offset..offset + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
    }
    pixels
}

fn render_rings(values: &[Option<u8>], limit_value: LimitValue) -> Vec<u8> {
    let mut pixels = vec![0; ICON_SIZE * ICON_SIZE * 4];
    let radii: &[f32] = if values.len() == 1 {
        &[11.0]
    } else {
        &[12.0, 7.5]
    };
    for (value, radius) in values.iter().zip(radii) {
        let filled = value.unwrap_or(0) as f32 / 100.0;
        let color = icon_color(*value, limit_value);
        for y in 0..ICON_SIZE {
            for x in 0..ICON_SIZE {
                let dx = x as f32 + 0.5 - 16.0;
                let dy = y as f32 + 0.5 - 16.0;
                let distance = (dx * dx + dy * dy).sqrt();
                if (distance - radius).abs() > 2.0 {
                    continue;
                }
                let angle =
                    (dy.atan2(dx) + std::f32::consts::FRAC_PI_2).rem_euclid(std::f32::consts::TAU);
                let rgb = if angle <= filled * std::f32::consts::TAU {
                    color
                } else {
                    [70, 70, 70]
                };
                let offset = (y * ICON_SIZE + x) * 4;
                pixels[offset..offset + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
    }
    pixels
}

fn has_real_data(limits: &RateLimits) -> bool {
    limits.primary.used_percent.is_some()
        || limits.primary.resets_at.is_some()
        || limits.secondary.used_percent.is_some()
        || limits.secondary.resets_at.is_some()
}

fn app_icon_pixels() -> &'static [u8] {
    static PIXELS: OnceLock<Vec<u8>> = OnceLock::new();
    PIXELS.get_or_init(|| {
        let decoder = png::Decoder::new(
            &include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/icons/app-icon-32.png"
            ))[..],
        );
        let mut reader = decoder.read_info().expect("decode embedded tray icon");
        let mut buffer = vec![0; reader.output_buffer_size()];
        let info = reader
            .next_frame(&mut buffer)
            .expect("read embedded tray icon");
        assert_eq!(
            (info.width, info.height),
            (ICON_SIZE as u32, ICON_SIZE as u32)
        );
        buffer.truncate(info.buffer_size());
        buffer
    })
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
        update_available: bool,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum TrayMenuAction {
        Update,
        Settings,
        Exit,
    }

    impl TrayManager {
        pub fn new() -> Self {
            Self {
                icons: Vec::new(),
                update_available: false,
            }
        }

        pub fn sync(
            &mut self,
            widgets: &[TrayWidget],
            limits: &RateLimits,
            update_available: bool,
        ) -> Result<()> {
            let menu_changed = self.update_available != update_available;
            self.update_available = update_available;
            // No configured widgets is a deliberate state: retain one ordinary app icon.
            let icon_count = widgets.len().max(1);
            self.icons.truncate(icon_count);
            while self.icons.len() < icon_count {
                let index = self.icons.len();
                let icon = make_icon(widget_for_icon(index, widgets), limits)?;
                let tray = TrayIconBuilder::new()
                    .with_icon(icon)
                    .with_menu(Box::new(make_menu(update_available)?))
                    .with_tooltip(tooltip(limits))
                    .with_menu_on_left_click(false)
                    .build()
                    .context("create tray icon")?;
                self.icons.push(tray);
            }
            let tooltip = tooltip(limits);
            for (index, tray) in self.icons.iter().enumerate() {
                tray.set_icon(Some(make_icon(widget_for_icon(index, widgets), limits)?))?;
                tray.set_tooltip(Some(&tooltip))?;
                if menu_changed {
                    tray.set_menu(Some(Box::new(make_menu(update_available)?)));
                }
            }
            Ok(())
        }

        /// Recreate after an order change: Windows assigns the newest icon next
        /// to the clock, so creation is intentionally reversed below.
        pub fn rebuild(
            &mut self,
            widgets: &[TrayWidget],
            limits: &RateLimits,
            update_available: bool,
        ) -> Result<()> {
            self.icons.clear();
            self.sync(widgets, limits, update_available)
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
                    "update" => actions.push(TrayMenuAction::Update),
                    "settings" => actions.push(TrayMenuAction::Settings),
                    "exit" => actions.push(TrayMenuAction::Exit),
                    _ => {}
                }
            }
            actions
        }
    }

    fn widget_for_icon(index: usize, widgets: &[TrayWidget]) -> Option<&TrayWidget> {
        if widgets.is_empty() {
            None
        } else {
            widgets.get(widgets.len() - 1 - index)
        }
    }

    impl Default for TrayManager {
        fn default() -> Self {
            Self::new()
        }
    }

    fn make_icon(widget: Option<&TrayWidget>, limits: &RateLimits) -> Result<Icon> {
        Icon::from_rgba(
            widget.map_or_else(
                || app_icon_pixels().to_vec(),
                |widget| render_widget(widget, limits),
            ),
            ICON_SIZE as u32,
            ICON_SIZE as u32,
        )
        .context("create RGBA tray icon")
    }

    fn make_menu(update_available: bool) -> Result<Menu> {
        let header = IconMenuItem::new(
            format!("Codex Minibar - {}", env!("CARGO_PKG_VERSION")),
            false,
            None,
            None,
        );
        let settings = MenuItem::with_id("settings", "Settings", true, None);
        let exit = MenuItem::with_id("exit", "Exit", true, None);
        if update_available {
            let update = MenuItem::with_id("update", "Update Available", true, None);
            return Menu::with_items(&[
                &header,
                &PredefinedMenuItem::separator(),
                &update,
                &settings,
                &exit,
            ])
            .context("create tray menu");
        }
        Menu::with_items(&[&header, &PredefinedMenuItem::separator(), &settings, &exit])
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

    pub fn sync(
        &mut self,
        _widgets: &[TrayWidget],
        _limits: &RateLimits,
        _update_available: bool,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn rebuild(
        &mut self,
        _widgets: &[TrayWidget],
        _limits: &RateLimits,
        _update_available: bool,
    ) -> anyhow::Result<()> {
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
    Update,
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
            source: TraySource::Primary,
            presentation: TrayPresentation::Number,
            limit_value: LimitValue::Remaining,
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
    fn tooltip_marks_disabled_five_hour_window() {
        let limits = RateLimits {
            primary: LimitWindow::default(),
            secondary: LimitWindow {
                used_percent: Some(40),
                resets_at: Some(Utc.timestamp_opt(1_700_475_600, 0).unwrap()),
                duration_minutes: Some(10_080),
            },
            sampled_at: Utc::now(),
            ..RateLimits::default()
        };
        let value = tooltip(&limits);
        assert!(value.contains("5h  |  Disabled  |  —"));
        assert!(value.contains("7d  |  60%"));
    }

    #[test]
    fn primary_tray_falls_back_to_weekly_when_five_hour_disabled() {
        let limits = RateLimits {
            primary: LimitWindow::default(),
            secondary: LimitWindow {
                used_percent: Some(40),
                resets_at: None,
                duration_minutes: Some(10_080),
            },
            sampled_at: Utc::now(),
            ..RateLimits::default()
        };
        assert_eq!(limits.effective_primary().remaining_percent(), Some(60));
        let widget = TrayWidget {
            source: TraySource::Primary,
            presentation: TrayPresentation::Number,
            limit_value: LimitValue::Remaining,
        };
        let pixels = render_widget(&widget, &limits);
        assert_eq!(pixels.len(), ICON_SIZE * ICON_SIZE * 4);
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
        assert_ne!(pixels, app_icon_pixels());
    }

    #[test]
    fn reset_icons_are_white_and_percentage_icons_are_colored() {
        assert_eq!(icon_color(None, LimitValue::Remaining), [180, 180, 180]);
        assert_eq!(icon_color(Some(80), LimitValue::Remaining), [49, 196, 141]);
        // Used inverts severity: low used is healthy, high used is critical.
        assert_eq!(icon_color(Some(1), LimitValue::Used), [49, 196, 141]);
        assert_eq!(icon_color(Some(99), LimitValue::Used), [230, 74, 72]);
    }

    #[test]
    fn uses_app_icon_until_rate_limit_data_arrives() {
        let widget = TrayWidget {
            source: TraySource::Primary,
            presentation: TrayPresentation::Number,
            limit_value: LimitValue::Remaining,
        };
        let pixels = render_widget(&widget, &RateLimits::default());
        assert_eq!(pixels.len(), ICON_SIZE * ICON_SIZE * 4);
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
        assert_eq!(pixels, app_icon_pixels());
    }
}
