use chrono::{DateTime, Local, Utc};
use std::{fs, path::PathBuf, sync::OnceLock};

use font8x8::{BASIC_FONTS, UnicodeFonts};
use fontdue::{
    Font, FontSettings,
    layout::{CoordinateSystem, HorizontalAlign, Layout, LayoutSettings, TextStyle, VerticalAlign},
};

use crate::{
    limits::{LimitWindow, ProviderLimits, RateLimits},
    provider_registry,
    settings::{
        LimitValue, ProviderKind, TrayColorMode, TrayPresentation, TrayWidget, TrayWidgetKind,
    },
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
    let mut rows = vec![format!(
        "5h  |  {}  |  {}\n7d  |  {}  |  {}",
        five_hour,
        five_hour_reset,
        format_remaining(&limits.secondary),
        format_reset(limits.secondary.resets_at),
    )];
    rows.extend(limits.additional_limits.iter().map(|limit| {
        format!(
            "{}  |  {}  |  {}",
            limit.title,
            format_remaining(&limit.window),
            format_reset(limit.window.resets_at),
        )
    }));
    rows.join("\n")
}

fn truncate_tooltip(value: &str) -> String {
    const MAX_UNITS: usize = 127;
    if value.encode_utf16().count() <= MAX_UNITS {
        return value.into();
    }
    let mut output = String::new();
    let mut units = 0usize;
    for character in value.chars() {
        let width = character.len_utf16();
        if units + width + 1 > MAX_UNITS {
            break;
        }
        output.push(character);
        units += width;
    }
    output.push('…');
    output
}

fn widget_tooltip(widget: &TrayWidget, limits: &ProviderLimits) -> String {
    if widget.kind == TrayWidgetKind::AppIcon {
        return "Codex Minibar".into();
    }
    let mut rows = Vec::<(ProviderKind, Vec<String>)>::new();
    for indicator in &widget.indicators {
        let Some(provider) = indicator.provider() else {
            continue;
        };
        let provider_limits = limits.get(provider);
        let Some((_, label, window)) =
            provider_registry::resolve_metric(provider, provider_limits, &indicator.metric_id)
        else {
            continue;
        };
        let value = percent(window, indicator.limit_value)
            .map(|value| format!("{value}%"))
            .unwrap_or_else(|| "?".into());
        let item = format!("{label} {value}");
        if let Some((_, items)) = rows
            .iter_mut()
            .find(|(row_provider, _)| *row_provider == provider)
        {
            items.push(item);
        } else {
            rows.push((provider, vec![item]));
        }
    }
    if rows.is_empty() {
        return "Codex Minibar".into();
    }
    truncate_tooltip(
        &rows
            .into_iter()
            .map(|(provider, items)| format!("{}: {}", provider.display_name(), items.join(", ")))
            .collect::<Vec<_>>()
            .join("\n"),
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

#[cfg(test)]
fn icon_color(value: Option<u8>, limit_value: LimitValue) -> [u8; 3] {
    icon_color_for_theme(value, limit_value, system_uses_light_theme())
}

#[cfg(test)]
fn icon_color_for_theme(
    value: Option<u8>,
    limit_value: LimitValue,
    uses_light_theme: bool,
) -> [u8; 3] {
    // Remaining: low is bad. Used: high is bad — color against remaining-equivalent.
    let severity = match limit_value {
        LimitValue::Used => value.map(|used| 100u8.saturating_sub(used.min(100))),
        LimitValue::Remaining => value,
    };
    match severity {
        Some(0..=15) => [230, 74, 72],
        Some(16..=50) => [245, 158, 11],
        Some(_) => [49, 196, 141],
        // Unavailable values are text, not an inactive control: keep them
        // readable against the system tray in either Windows theme.
        None => tray_text_color(uses_light_theme),
    }
}

fn tray_text_color(uses_light_theme: bool) -> [u8; 3] {
    if uses_light_theme {
        [0, 0, 0]
    } else {
        [255, 255, 255]
    }
}

/// `AppsUseLightTheme` is the source Windows itself uses for application
/// surfaces. Treat an unreadable value as light so tray text never disappears
/// against the ordinary light notification area.
#[cfg(windows)]
fn system_uses_light_theme() -> bool {
    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW},
    };

    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let value_name: Vec<u16> = "AppsUseLightTheme"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut value = 1u32;
    let mut value_size = size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value_name.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            &mut value as *mut u32 as *mut _,
            &mut value_size,
        )
    };
    status == ERROR_SUCCESS && value != 0
}

#[cfg(not(windows))]
fn system_uses_light_theme() -> bool {
    true
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

#[derive(Clone, Debug)]
struct ResolvedIndicator {
    displayed_percent: Option<u8>,
    reset: Option<DateTime<Utc>>,
    color: [u8; 3],
}

fn indicator_color(
    widget: &TrayWidget,
    provider: ProviderKind,
    remaining: Option<u8>,
    accent: [u8; 3],
) -> [u8; 3] {
    match widget.color_mode {
        TrayColorMode::Status => match remaining {
            Some(0..=15) => [230, 74, 72],
            Some(16..=50) => [245, 158, 11],
            Some(_) => [49, 196, 141],
            None => tray_text_color(system_uses_light_theme()),
        },
        TrayColorMode::Fixed => [
            widget.fixed_color.red,
            widget.fixed_color.green,
            widget.fixed_color.blue,
        ],
        TrayColorMode::Provider => {
            let (red, green, blue) = provider_registry::descriptor(provider).brand_rgb;
            [red, green, blue]
        }
        TrayColorMode::Accent => accent,
        TrayColorMode::Monochrome => tray_text_color(system_uses_light_theme()),
    }
}

fn resolve_indicators(
    widget: &TrayWidget,
    limits: &ProviderLimits,
    accent: [u8; 3],
) -> Vec<ResolvedIndicator> {
    widget
        .indicators
        .iter()
        .take(3)
        .filter_map(|indicator| {
            let provider = indicator.provider()?;
            let (_, _, window) = provider_registry::resolve_metric(
                provider,
                limits.get(provider),
                &indicator.metric_id,
            )?;
            let remaining = window.remaining_percent();
            Some(ResolvedIndicator {
                displayed_percent: percent(window, indicator.limit_value),
                reset: window.resets_at,
                color: indicator_color(widget, provider, remaining, accent),
            })
        })
        .collect()
}

pub fn render_widget(widget: &TrayWidget, limits: &ProviderLimits) -> Vec<u8> {
    render_widget_with_accent(widget, limits, crate::theme::current_accent_rgb())
}

pub fn render_widget_with_accent(
    widget: &TrayWidget,
    limits: &ProviderLimits,
    accent: [u8; 3],
) -> Vec<u8> {
    if widget.kind == TrayWidgetKind::AppIcon {
        return app_icon_pixels().to_vec();
    }
    let indicators = resolve_indicators(widget, limits, accent);
    if indicators.is_empty() {
        return app_icon_pixels().to_vec();
    }
    match widget.presentation.canonical_percentage() {
        TrayPresentation::StackedBars => render_bars(&indicators),
        TrayPresentation::NestedRings => render_rings(&indicators),
        TrayPresentation::ResetTime | TrayPresentation::ResetCountdown => {
            let countdown = widget.presentation == TrayPresentation::ResetCountdown;
            render_text_lines(
                &indicators
                    .iter()
                    .map(|indicator| {
                        (
                            stacked_reset_label(indicator.reset, countdown).replace('\n', ":"),
                            indicator.color,
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        }
        _ => render_text_lines(
            &indicators
                .iter()
                .map(|indicator| {
                    (
                        indicator
                            .displayed_percent
                            .map_or_else(|| "?".into(), |value| value.to_string()),
                        indicator.color,
                    )
                })
                .collect::<Vec<_>>(),
        ),
    }
}

fn render_text_lines(lines: &[(String, [u8; 3])]) -> Vec<u8> {
    let Some(font) = system_font() else {
        return render_fallback_lines(lines);
    };
    let line_refs = lines
        .iter()
        .map(|(line, _)| line.as_str())
        .collect::<Vec<_>>();
    let font_size = text_font_size(&line_refs);
    let fonts = [font.clone()];
    let mut pixels = vec![0; ICON_SIZE * ICON_SIZE * 4];
    let line_height = ICON_SIZE as f32 / lines.len().max(1) as f32;
    for (line_index, (line, rgb)) in lines.iter().enumerate() {
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

fn text_font_size(lines: &[&str]) -> f32 {
    let widest_line = lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    // Windows downscales our 32px source to the 16px notification-area slot.
    // Leave enough horizontal padding for `100` on either stacked line so the
    // third digit stays crisp instead of being clipped at the icon edge.
    match (lines.len(), widest_line) {
        (0 | 1, 0..=2) => 24.0,
        (0 | 1, _) => 17.0,
        (2, 0..=2) => 15.0,
        (2, _) => 12.0,
        (_, 0..=2) => 10.0,
        (_, _) => 8.0,
    }
}

fn render_bars(values: &[ResolvedIndicator]) -> Vec<u8> {
    let mut pixels = vec![0; ICON_SIZE * ICON_SIZE * 4];
    let count = values.len();
    for (index, value) in values.iter().enumerate() {
        let height = match count {
            0 | 1 => 8,
            2 => 6,
            _ => 5,
        };
        let gap = if count >= 3 { 3 } else { 5 };
        let total_height = count * height + count.saturating_sub(1) * gap;
        let y = ICON_SIZE.saturating_sub(total_height) / 2 + index * (height + gap);
        let filled = value.displayed_percent.unwrap_or(0) as usize * 24 / 100;
        let color = value.color;
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

fn render_rings(values: &[ResolvedIndicator]) -> Vec<u8> {
    let mut pixels = vec![0; ICON_SIZE * ICON_SIZE * 4];
    let radii: &[f32] = match values.len() {
        0 | 1 => &[11.0],
        2 => &[12.0, 7.5],
        _ => &[13.0, 9.0, 5.0],
    };
    for (value, radius) in values.iter().zip(radii) {
        let filled = value.displayed_percent.unwrap_or(0) as f32 / 100.0;
        let color = value.color;
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

fn render_fallback_lines(lines: &[(String, [u8; 3])]) -> Vec<u8> {
    let mut pixels = vec![0; ICON_SIZE * ICON_SIZE * 4];
    let line_height = ICON_SIZE / lines.len().max(1);
    for (line_index, (text, rgb)) in lines.iter().enumerate() {
        let scale = if lines.len() == 1 && text.chars().count() <= 2 {
            3
        } else {
            1
        };
        let glyph_width = 8 * scale;
        let total_width = glyph_width * text.chars().count();
        let start_x = ICON_SIZE.saturating_sub(total_width) / 2;
        let start_y = line_index * line_height + line_height.saturating_sub(8 * scale) / 2;
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
    }
    pixels
}

#[cfg(windows)]
mod platform {
    use anyhow::{Context, Result};
    use std::time::{Duration, Instant};
    use tray_icon::{
        Icon, TrayIcon, TrayIconBuilder, TrayIconId,
        menu::{IconMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    };

    use super::*;

    pub struct TrayManager {
        icons: Vec<TrayIcon>,
        update_available: bool,
        uses_light_theme: bool,
        next_theme_check: Instant,
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
                uses_light_theme: system_uses_light_theme(),
                next_theme_check: Instant::now(),
            }
        }

        pub fn sync(
            &mut self,
            widgets: &[TrayWidget],
            limits: &ProviderLimits,
            update_available: bool,
        ) -> Result<()> {
            self.uses_light_theme = system_uses_light_theme();
            self.next_theme_check = Instant::now() + Duration::from_millis(250);
            let menu_changed = self.update_available != update_available;
            self.update_available = update_available;
            // No configured widgets is a deliberate state: retain one ordinary app icon.
            let icon_count = widgets.len().max(1);
            self.icons.truncate(icon_count);
            while self.icons.len() < icon_count {
                let index = self.icons.len();
                let widget = widget_for_icon(index, widgets);
                let icon = make_icon(widget, limits)?;
                let tray = TrayIconBuilder::new()
                    .with_icon(icon)
                    .with_menu(Box::new(make_menu(update_available)?))
                    .with_tooltip(
                        widget
                            .map(|widget| widget_tooltip(widget, limits))
                            .unwrap_or_else(|| "Codex Minibar".into()),
                    )
                    .with_menu_on_left_click(false)
                    .build()
                    .context("create tray icon")?;
                self.icons.push(tray);
            }
            for (index, tray) in self.icons.iter().enumerate() {
                let widget = widget_for_icon(index, widgets);
                tray.set_icon(Some(make_icon(widget, limits)?))?;
                let tooltip = widget
                    .map(|widget| widget_tooltip(widget, limits))
                    .unwrap_or_else(|| "Codex Minibar".into());
                tray.set_tooltip(Some(&tooltip))?;
                if menu_changed {
                    tray.set_menu(Some(Box::new(make_menu(update_available)?)));
                }
            }
            Ok(())
        }

        /// Refresh only when Windows changes between light and dark. The
        /// polling guard keeps registry reads cheap while repainting existing
        /// icons in place within a quarter second of a theme switch.
        pub fn refresh_system_theme(
            &mut self,
            widgets: &[TrayWidget],
            limits: &ProviderLimits,
        ) -> Result<()> {
            if Instant::now() < self.next_theme_check {
                return Ok(());
            }
            self.next_theme_check = Instant::now() + Duration::from_millis(250);
            if system_uses_light_theme() == self.uses_light_theme {
                return Ok(());
            }
            self.sync(widgets, limits, self.update_available)
        }

        /// Recreate after an order change: Windows assigns the newest icon next
        /// to the clock, so creation is intentionally reversed below.
        pub fn rebuild(
            &mut self,
            widgets: &[TrayWidget],
            limits: &ProviderLimits,
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

    fn make_icon(widget: Option<&TrayWidget>, limits: &ProviderLimits) -> Result<Icon> {
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
        _limits: &ProviderLimits,
        _update_available: bool,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn rebuild(
        &mut self,
        _widgets: &[TrayWidget],
        _limits: &ProviderLimits,
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

    pub fn refresh_system_theme(
        &mut self,
        _widgets: &[TrayWidget],
        _limits: &ProviderLimits,
    ) -> anyhow::Result<()> {
        Ok(())
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

    fn provider_limits() -> ProviderLimits {
        ProviderLimits::from_entries([(ProviderKind::Codex, limits())])
    }

    #[test]
    fn renders_rgba_icon_with_visible_pixels() {
        let mut widget = TrayWidget::custom_for_provider(ProviderKind::Codex);
        widget.presentation = TrayPresentation::Number;
        let pixels = render_widget(&widget, &provider_limits());
        assert_eq!(pixels.len(), ICON_SIZE * ICON_SIZE * 4);
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
    }

    #[test]
    fn stacked_three_digit_values_use_a_compact_font() {
        assert_eq!(text_font_size(&["100", "100"]), 12.0);
        assert_eq!(text_font_size(&["99", "99"]), 15.0);
        assert_eq!(text_font_size(&["99", "99", "99"]), 10.0);
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
    fn tooltip_includes_additional_claude_limits() {
        let mut limits = RateLimits::default();
        limits
            .additional_limits
            .push(crate::limits::AdditionalLimit {
                id: "seven_day_fable".into(),
                title: "Fable".into(),
                window: LimitWindow {
                    used_percent: Some(12),
                    ..Default::default()
                },
            });

        assert!(tooltip(&limits).contains("Fable  |  88%  |  ?"));
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
        let mut widget = TrayWidget::custom_for_provider(ProviderKind::Codex);
        widget.presentation = TrayPresentation::Number;
        let limits = ProviderLimits::from_entries([(ProviderKind::Codex, limits)]);
        let pixels = render_widget(&widget, &limits);
        assert_eq!(pixels.len(), ICON_SIZE * ICON_SIZE * 4);
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
        assert_ne!(pixels, app_icon_pixels());
    }

    #[test]
    fn tray_text_uses_the_system_theme_and_percentage_icons_stay_colored() {
        assert_eq!(tray_text_color(true), [0, 0, 0]);
        assert_eq!(tray_text_color(false), [255, 255, 255]);
        assert_eq!(
            icon_color_for_theme(None, LimitValue::Remaining, true),
            [0, 0, 0]
        );
        assert_eq!(icon_color(Some(80), LimitValue::Remaining), [49, 196, 141]);
        // Used inverts severity: low used is healthy, high used is critical.
        assert_eq!(icon_color(Some(1), LimitValue::Used), [49, 196, 141]);
        assert_eq!(icon_color(Some(99), LimitValue::Used), [230, 74, 72]);
    }

    #[test]
    fn uses_app_icon_until_rate_limit_data_arrives() {
        let mut widget = TrayWidget::custom_for_provider(ProviderKind::Codex);
        widget.presentation = TrayPresentation::Number;
        let pixels = render_widget(&widget, &ProviderLimits::default());
        assert_eq!(pixels.len(), ICON_SIZE * ICON_SIZE * 4);
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
        assert_eq!(pixels, app_icon_pixels());
    }

    #[test]
    fn renders_three_indicators_with_independent_status_colors() {
        let limits = ProviderLimits::from_entries([
            (
                ProviderKind::Codex,
                RateLimits {
                    primary: LimitWindow {
                        used_percent: Some(38),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ),
            (
                ProviderKind::Claude,
                RateLimits {
                    primary: LimitWindow {
                        used_percent: Some(55),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ),
            (
                ProviderKind::Cursor,
                RateLimits {
                    secondary: LimitWindow {
                        used_percent: Some(88),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ),
        ]);
        let mut widget = TrayWidget::custom_for_provider(ProviderKind::Codex);
        widget.indicators = vec![
            crate::settings::TrayIndicator::new(ProviderKind::Codex, "codex.session"),
            crate::settings::TrayIndicator::new(ProviderKind::Claude, "claude.session"),
            crate::settings::TrayIndicator::new(ProviderKind::Cursor, "cursor.auto"),
        ];
        widget.presentation = TrayPresentation::StackedBars;

        let pixels = render_widget(&widget, &limits);

        let colors = pixels
            .chunks_exact(4)
            .filter(|pixel| pixel[3] != 0)
            .map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect::<std::collections::HashSet<_>>();
        assert!(colors.contains(&[49, 196, 141]));
        assert!(colors.contains(&[245, 158, 11]));
        assert!(colors.contains(&[230, 74, 72]));
    }

    #[test]
    fn tooltip_truncation_preserves_utf16_boundary() {
        let value = "provider: metric 100% 🚀 ".repeat(20);
        let truncated = truncate_tooltip(&value);
        assert!(truncated.encode_utf16().count() <= 127);
        assert!(truncated.ends_with('…'));
    }
}
