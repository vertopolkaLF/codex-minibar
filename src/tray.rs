use chrono::{DateTime, Local, Utc};
use std::{fs, path::PathBuf, sync::OnceLock};

use font8x8::{BASIC_FONTS, UnicodeFonts};
use fontdue::{
    Font, FontSettings,
    layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle, VerticalAlign},
};

use crate::{
    instances::ProviderId,
    limits::{LimitWindow, ProviderLimits, RateLimits},
    provider_registry,
    settings::{
        LimitValue, TimeFormat, TrayColorMode, TrayPresentation, TrayWidget, TrayWidgetKind,
    },
};

const ICON_SIZE: usize = 32;

pub fn tooltip(limits: &RateLimits) -> String {
    let five_hour = if limits.five_hour_disabled() {
        crate::i18n::tr("disabled").to_string()
    } else {
        format_remaining(&limits.primary)
    };
    let five_hour_reset = if limits.five_hour_disabled() {
        "—".to_string()
    } else {
        format_reset(limits.primary.resets_at)
    };
    let mut rows = vec![crate::i18n::format(
        "msg-5h-7d",
        &[
            ("v0", five_hour.to_string()),
            ("v1", five_hour_reset.to_string()),
            ("v2", (format_remaining(&limits.secondary)).to_string()),
            ("v3", (format_reset(limits.secondary.resets_at)).to_string()),
        ],
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
    let mut rows = Vec::<(String, String, Vec<String>)>::new();
    for indicator in &widget.indicators {
        let Some(provider) = indicator.provider() else {
            continue;
        };
        let metric = crate::widget_data::resolve_metric(
            provider,
            limits.get(provider),
            &indicator.metric_id,
        );
        let value = metric
            .as_ref()
            .and_then(|metric| percent(&metric.window, indicator.limit_value))
            .map(|value| format!("{value}%"))
            .unwrap_or_else(|| "?".into());
        let label = metric.map(|metric| metric.label).unwrap_or_else(|| {
            provider_registry::metric_label(
                provider.kind(),
                limits.get(provider),
                &indicator.metric_id,
            )
        });
        let item = format!("{label} {value}");
        let source_id = provider.id();
        let title = provider.qualified_name();
        if let Some((_, _, items)) = rows
            .iter_mut()
            .find(|(row_source, _, _)| *row_source == source_id)
        {
            items.push(item);
        } else {
            rows.push((source_id.to_owned(), title, vec![item]));
        }
    }
    if rows.is_empty() {
        return "Codex Minibar".into();
    }
    truncate_tooltip(
        &rows
            .into_iter()
            .map(|(_, title, items)| format!("{title}: {}", items.join(", ")))
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
            let local = value.with_timezone(&Local);
            format!(
                "{} {}",
                TimeFormat::current().format_hm(local),
                local.format("%d.%m")
            )
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
            || "?\n?".into(),
            |value| {
                let minutes = (value - Utc::now()).num_minutes().max(0);
                // Always hours on top, minutes on bottom — same stacked layout
                // whether the remaining time is under an hour or not.
                format!("{}\n{:02}", minutes / 60, minutes % 60)
            },
        );
    }
    reset
        .map(|value| TimeFormat::current().format_stacked_hm(value.with_timezone(&Local)))
        .unwrap_or_else(|| "?\n?".into())
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
pub(crate) fn system_uses_light_theme() -> bool {
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

/// Whether the panel hosting the tray is light. GNOME-family top bars are
/// dark in both color schemes; other desktops follow the portal color scheme
/// and default to light without a preference.
#[cfg(not(windows))]
pub(crate) fn system_uses_light_theme() -> bool {
    !crate::linux::desktop::panel_always_dark()
        && crate::linux::desktop::prefers_dark() != Some(true)
}

fn system_font() -> Option<&'static Font> {
    static FONT: OnceLock<Option<Font>> = OnceLock::new();
    FONT.get_or_init(|| {
        let font = font_candidates().into_iter().find_map(|path| {
            fs::read(path)
                .ok()
                .and_then(|bytes| Font::from_bytes(bytes, FontSettings::default()).ok())
        });
        #[cfg(not(windows))]
        let font = font.or_else(fontconfig_bold_sans);
        font
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
    indicator: &crate::settings::TrayIndicator,
    provider: ProviderId,
    remaining: Option<u8>,
    accent: [u8; 3],
) -> [u8; 3] {
    match indicator.color_mode {
        TrayColorMode::Status => match remaining {
            Some(0..=15) => [230, 74, 72],
            Some(16..=50) => [245, 158, 11],
            Some(_) => [49, 196, 141],
            None => tray_text_color(system_uses_light_theme()),
        },
        TrayColorMode::Fixed => [
            indicator.fixed_color.red,
            indicator.fixed_color.green,
            indicator.fixed_color.blue,
        ],
        TrayColorMode::Provider => {
            let (red, green, blue) = provider_registry::descriptor(provider.kind()).brand_rgb;
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
            let metric = crate::widget_data::resolve_metric(
                provider,
                limits.get(provider),
                &indicator.metric_id,
            );
            let window = metric.map(|metric| metric.window).unwrap_or_default();
            let remaining = window.remaining_percent();
            Some(ResolvedIndicator {
                displayed_percent: percent(&window, indicator.limit_value),
                reset: window.resets_at,
                color: indicator_color(indicator, provider, remaining, accent),
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
    if matches!(
        widget.presentation.canonical_percentage(),
        TrayPresentation::StackedBars | TrayPresentation::NestedRings
    ) && indicators
        .iter()
        .any(|indicator| indicator.displayed_percent.is_none())
    {
        return render_percentages(&indicators);
    }
    match widget.presentation.canonical_percentage() {
        TrayPresentation::StackedBars => render_bars(&indicators),
        TrayPresentation::NestedRings => render_rings(&indicators),
        TrayPresentation::ResetTime | TrayPresentation::ResetCountdown => {
            let countdown = widget.presentation == TrayPresentation::ResetCountdown;
            // One provider: two stacked glyph lines (hours top, minutes bottom).
            // Multiple providers: one HH:MM line per indicator.
            if indicators.len() == 1 {
                let indicator = &indicators[0];
                let lines = stacked_reset_label(indicator.reset, countdown)
                    .lines()
                    .map(|line| (line.to_owned(), indicator.color))
                    .collect::<Vec<_>>();
                render_text_lines(&lines, false)
            } else {
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
                    false,
                )
            }
        }
        _ => render_percentages(&indicators),
    }
}

fn render_percentages(indicators: &[ResolvedIndicator]) -> Vec<u8> {
    render_text_lines(
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
        true,
    )
}

/// Draws one line per indicator. Lines share one column centered in the icon,
/// on whole pixels, so stacked values line up; `right_align` aligns them by
/// their last digit like a table, otherwise each line centers in the column.
fn render_text_lines(lines: &[(String, [u8; 3])], right_align: bool) -> Vec<u8> {
    let Some(font) = system_font() else {
        return render_fallback_lines(lines, right_align);
    };
    let line_refs = lines
        .iter()
        .map(|(line, _)| line.as_str())
        .collect::<Vec<_>>();
    let font_size = text_font_size(&line_refs);
    let fonts = [font.clone()];
    let mut pixels = vec![0; ICON_SIZE * ICON_SIZE * 4];
    let line_height = ICON_SIZE as f32 / lines.len().max(1) as f32;
    // Advance widths, not ink bounds: those differ per glyph ("3" vs "8") and
    // would shift every line by its own amount.
    let widths = lines
        .iter()
        .map(|(line, _)| {
            line.chars()
                .map(|character| font.metrics(character, font_size).advance_width)
                .sum::<f32>()
        })
        .collect::<Vec<_>>();
    let column = widths.iter().copied().fold(0.0_f32, f32::max);
    let left = ((ICON_SIZE as f32 - column) / 2.0).round();
    for (line_index, ((line, rgb), width)) in lines.iter().zip(&widths).enumerate() {
        let slack = column - width;
        let x = left
            + if right_align {
                slack
            } else {
                (slack / 2.0).round()
            };
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x,
            y: line_index as f32 * line_height,
            max_height: Some(line_height),
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
    // Reset/countdown use two stacked lines (hours / minutes) — keep those
    // glyphs as large as the slot can still resolve cleanly.
    match (lines.len(), widest_line) {
        (0 | 1, 0..=2) => 24.0,
        (0 | 1, _) => 17.0,
        (2, 0..=2) => 18.0,
        (2, _) => 14.0,
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
        let filled = f32::from(value.displayed_percent.unwrap_or(0)) / 100.0;
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
    // Bold sans faces where Debian/Ubuntu, Fedora and Arch install them.
    candidates.extend(
        [
            "/usr/share/fonts/truetype/ubuntu/Ubuntu-B.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
            "/usr/share/fonts/google-noto/NotoSans-Bold.ttf",
            "/usr/share/fonts/noto/NotoSans-Bold.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
            "/usr/share/fonts/liberation-sans/LiberationSans-Bold.ttf",
            "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
        ]
        .map(PathBuf::from),
    );
    candidates
}

/// Whatever bold sans face fontconfig resolves, for other layouts.
#[cfg(not(windows))]
fn fontconfig_bold_sans() -> Option<Font> {
    // Share GPUI's fontconfig library rather than requiring the separate
    // fc-match utility. Only consult it when the ordinary paths failed.
    let handle = font_kit::source::SystemSource::new()
        .select_best_match(
            &[font_kit::family_name::FamilyName::SansSerif],
            font_kit::properties::Properties::new().weight(font_kit::properties::Weight::BOLD),
        )
        .ok()?;
    font_from_handle(handle)
}

#[cfg(not(windows))]
fn font_from_handle(handle: font_kit::handle::Handle) -> Option<Font> {
    let (bytes, collection_index) = match handle {
        font_kit::handle::Handle::Path { path, font_index } => (fs::read(path).ok()?, font_index),
        font_kit::handle::Handle::Memory { bytes, font_index } => ((*bytes).clone(), font_index),
        font_kit::handle::Handle::Native { .. } => return None,
    };
    Font::from_bytes(
        bytes,
        FontSettings {
            collection_index,
            ..FontSettings::default()
        },
    )
    .ok()
}

fn render_fallback_lines(lines: &[(String, [u8; 3])], right_align: bool) -> Vec<u8> {
    let mut pixels = vec![0; ICON_SIZE * ICON_SIZE * 4];
    let line_height = ICON_SIZE / lines.len().max(1);
    let column = lines
        .iter()
        .map(|(text, _)| text.chars().count())
        .max()
        .unwrap_or(0);
    for (line_index, (text, rgb)) in lines.iter().enumerate() {
        let scale = if lines.len() == 1 && text.chars().count() <= 2 {
            3
        } else {
            1
        };
        let glyph_width = 8 * scale;
        let count = text.chars().count();
        let start_x = if right_align && scale == 1 {
            ICON_SIZE.saturating_sub(glyph_width * column) / 2 + glyph_width * (column - count)
        } else {
            ICON_SIZE.saturating_sub(glyph_width * count) / 2
        };
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
        /// Last RGBA payloads pushed to Explorer. Avoid recreating identical
        /// HICONs on every provider poll — that churn was a GDI/handle tax.
        last_pixels: Vec<Vec<u8>>,
        last_tooltips: Vec<String>,
        update_available: bool,
        language: crate::i18n::Language,
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
                last_pixels: Vec::new(),
                last_tooltips: Vec::new(),
                update_available: false,
                language: crate::i18n::current_language(),
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
            crate::taskbar::set_update_available(update_available);
            let language = crate::i18n::current_language();
            let menu_changed =
                self.update_available != update_available || self.language != language;
            self.language = language;
            self.update_available = update_available;
            // No configured widgets is a deliberate state: retain one ordinary app icon.
            let icon_count = widgets.len().max(1);
            self.icons.truncate(icon_count);
            self.last_pixels.truncate(icon_count);
            self.last_tooltips.truncate(icon_count);
            while self.icons.len() < icon_count {
                let index = self.icons.len();
                let widget = widget_for_icon(index, widgets);
                let pixels = icon_pixels(widget, limits);
                let tooltip = widget
                    .map(|widget| widget_tooltip(widget, limits))
                    .unwrap_or_else(|| "Codex Minibar".into());
                let tray = TrayIconBuilder::new()
                    .with_icon(icon_from_pixels(&pixels)?)
                    .with_menu(Box::new(make_menu(update_available)?))
                    .with_tooltip(tooltip.clone())
                    .with_menu_on_left_click(false)
                    .build()
                    .context("create tray icon")?;
                self.icons.push(tray);
                self.last_pixels.push(pixels);
                self.last_tooltips.push(tooltip);
            }
            for (index, tray) in self.icons.iter().enumerate() {
                let widget = widget_for_icon(index, widgets);
                let pixels = icon_pixels(widget, limits);
                if self.last_pixels.get(index) != Some(&pixels) {
                    tray.set_icon(Some(icon_from_pixels(&pixels)?))?;
                    if index < self.last_pixels.len() {
                        self.last_pixels[index] = pixels;
                    } else {
                        self.last_pixels.push(pixels);
                    }
                }
                let tooltip = widget
                    .map(|widget| widget_tooltip(widget, limits))
                    .unwrap_or_else(|| "Codex Minibar".into());
                if self.last_tooltips.get(index) != Some(&tooltip) {
                    tray.set_tooltip(Some(&tooltip))?;
                    if index < self.last_tooltips.len() {
                        self.last_tooltips[index] = tooltip;
                    } else {
                        self.last_tooltips.push(tooltip);
                    }
                }
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
            self.last_pixels.clear();
            self.last_tooltips.clear();
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

    fn icon_pixels(widget: Option<&TrayWidget>, limits: &ProviderLimits) -> Vec<u8> {
        widget.map_or_else(
            || app_icon_pixels().to_vec(),
            |widget| render_widget(widget, limits),
        )
    }

    fn icon_from_pixels(pixels: &[u8]) -> Result<Icon> {
        Icon::from_rgba(pixels.to_vec(), ICON_SIZE as u32, ICON_SIZE as u32)
            .context("create RGBA tray icon")
    }

    fn make_menu(update_available: bool) -> Result<Menu> {
        let header = IconMenuItem::new(
            format!("Codex Minibar - {}", env!("CARGO_PKG_VERSION")),
            false,
            None,
            None,
        );
        let settings = MenuItem::with_id("settings", crate::i18n::tr("settings"), true, None);
        let exit = MenuItem::with_id("exit", crate::i18n::tr("exit"), true, None);
        if update_available {
            let update = MenuItem::with_id(
                "update",
                crate::i18n::tr("update-available-67fd3a"),
                true,
                None,
            );
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
mod platform {
    //! StatusNotifierItem tray icons (KDE, GNOME with the AppIndicator
    //! extension, Xfce, Cinnamon, Budgie, wlroots bars). Each widget is its own
    //! item, like each widget is its own notification-area icon on Windows.

    use std::{
        sync::Mutex,
        time::{Duration, Instant},
    };

    use anyhow::Result;
    use ksni::blocking::{Handle, TrayMethods};

    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum TrayMenuAction {
        ShowPopup,
        Update,
        Settings,
        Exit,
    }

    /// Input from the D-Bus tray threads, drained by the bridge thread.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum TrayEvent {
        /// Primary click at screen coordinates (`0, 0` when the host does
        /// not report a position).
        Activate(i32, i32),
        Menu(TrayMenuAction),
    }

    static EVENTS: Mutex<Vec<TrayEvent>> = Mutex::new(Vec::new());

    fn push(event: TrayEvent) {
        if let Ok(mut events) = EVENTS.lock() {
            events.push(event);
        }
        crate::linux::wake::wake();
    }

    struct Item {
        slot: usize,
        icon: ksni::Icon,
        tooltip: String,
        update_available: bool,
    }

    impl ksni::Tray for Item {
        fn id(&self) -> String {
            // Hosts remember item placement by id; keep it stable per slot.
            if self.slot == 0 {
                "codex-minibar".into()
            } else {
                format!("codex-minibar-{}", self.slot)
            }
        }

        fn title(&self) -> String {
            "Codex Minibar".into()
        }

        fn category(&self) -> ksni::Category {
            ksni::Category::ApplicationStatus
        }

        fn icon_pixmap(&self) -> Vec<ksni::Icon> {
            vec![self.icon.clone()]
        }

        fn tool_tip(&self) -> ksni::ToolTip {
            ksni::ToolTip {
                title: "Codex Minibar".into(),
                description: self.tooltip.clone(),
                ..Default::default()
            }
        }

        fn activate(&mut self, x: i32, y: i32) {
            push(TrayEvent::Activate(x, y));
        }

        fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
            use ksni::menu::StandardItem;
            let item = |label: String, action: TrayMenuAction| -> ksni::MenuItem<Self> {
                StandardItem {
                    label,
                    activate: Box::new(move |_: &mut Self| push(TrayEvent::Menu(action))),
                    ..Default::default()
                }
                .into()
            };
            let mut items = vec![
                StandardItem {
                    label: format!("Codex Minibar - {}", env!("CARGO_PKG_VERSION")),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
                ksni::MenuItem::Separator,
                item(
                    crate::i18n::tr("show").to_string(),
                    TrayMenuAction::ShowPopup,
                ),
            ];
            if self.update_available {
                items.push(item(
                    crate::i18n::tr("update-available-67fd3a").to_string(),
                    TrayMenuAction::Update,
                ));
            }
            items.push(item(
                crate::i18n::tr("settings").to_string(),
                TrayMenuAction::Settings,
            ));
            items.push(item(
                crate::i18n::tr("exit").to_string(),
                TrayMenuAction::Exit,
            ));
            items
        }
    }

    pub struct TrayManager {
        items: Vec<Handle<Item>>,
        last_pixels: Vec<Vec<u8>>,
        last_tooltips: Vec<String>,
        update_available: bool,
        language: crate::i18n::Language,
        uses_light_theme: bool,
        next_theme_check: Instant,
        host_checked: bool,
    }

    impl TrayManager {
        pub fn new() -> Self {
            Self {
                items: Vec::new(),
                last_pixels: Vec::new(),
                last_tooltips: Vec::new(),
                update_available: false,
                language: crate::i18n::current_language(),
                uses_light_theme: system_uses_light_theme(),
                next_theme_check: Instant::now(),
                host_checked: false,
            }
        }

        pub fn sync(
            &mut self,
            widgets: &[TrayWidget],
            limits: &ProviderLimits,
            update_available: bool,
        ) -> Result<()> {
            self.uses_light_theme = system_uses_light_theme();
            self.next_theme_check = Instant::now() + Duration::from_secs(1);
            crate::taskbar::set_update_available(update_available);
            let language = crate::i18n::current_language();
            let menu_changed =
                self.update_available != update_available || self.language != language;
            self.language = language;
            self.update_available = update_available;
            // No configured widgets is a deliberate state: retain one app icon.
            let count = widgets.len().max(1);
            for handle in self.items.drain(count.min(self.items.len())..) {
                handle.shutdown();
            }
            self.last_pixels.truncate(count);
            self.last_tooltips.truncate(count);
            while self.items.len() < count {
                let slot = self.items.len();
                let widget = widgets.get(slot);
                let pixels = icon_pixels(widget, limits);
                let tooltip = tooltip_for(widget, limits);
                let item = Item {
                    slot,
                    icon: icon_from_rgba(&pixels),
                    tooltip: tooltip.clone(),
                    update_available,
                };
                // Register even before a tray host exists (the shell may start
                // after us); the item appears as soon as one does.
                let handle = item
                    .assume_sni_available(true)
                    .spawn()
                    .map_err(|error| anyhow::anyhow!("create tray icon: {error}"))?;
                self.items.push(handle);
                self.last_pixels.push(pixels);
                self.last_tooltips.push(tooltip);
            }
            for (slot, handle) in self.items.iter().enumerate() {
                let widget = widgets.get(slot);
                let pixels = icon_pixels(widget, limits);
                let tooltip = tooltip_for(widget, limits);
                let icon_changed = self.last_pixels[slot] != pixels;
                let tooltip_changed = self.last_tooltips[slot] != tooltip;
                if !(icon_changed || tooltip_changed || menu_changed) {
                    continue;
                }
                let icon = icon_changed.then(|| icon_from_rgba(&pixels));
                let text = tooltip.clone();
                handle.update(move |item| {
                    if let Some(icon) = icon {
                        item.icon = icon;
                    }
                    item.tooltip = text;
                    item.update_available = update_available;
                });
                self.last_pixels[slot] = pixels;
                self.last_tooltips[slot] = tooltip;
            }
            if !self.host_checked {
                self.host_checked = true;
                warn_when_no_tray_host();
            }
            Ok(())
        }

        /// Repaints when the panel switches between light and dark.
        pub fn refresh_system_theme(
            &mut self,
            widgets: &[TrayWidget],
            limits: &ProviderLimits,
        ) -> Result<()> {
            if Instant::now() < self.next_theme_check {
                return Ok(());
            }
            self.next_theme_check = Instant::now() + Duration::from_secs(1);
            if system_uses_light_theme() == self.uses_light_theme {
                return Ok(());
            }
            self.sync(widgets, limits, self.update_available)
        }

        /// Recreates every item so hosts lay them out in the new order.
        pub fn rebuild(
            &mut self,
            widgets: &[TrayWidget],
            limits: &ProviderLimits,
            update_available: bool,
        ) -> Result<()> {
            for handle in self.items.drain(..) {
                handle.shutdown();
            }
            self.last_pixels.clear();
            self.last_tooltips.clear();
            self.sync(widgets, limits, update_available)
        }

        pub fn len(&self) -> usize {
            self.items.len()
        }

        pub fn is_empty(&self) -> bool {
            self.items.is_empty()
        }

        pub fn drain_events(&self) -> Vec<TrayEvent> {
            EVENTS
                .lock()
                .map(|mut events| std::mem::take(&mut *events))
                .unwrap_or_default()
        }
    }

    impl Default for TrayManager {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Drop for TrayManager {
        fn drop(&mut self) {
            for handle in self.items.drain(..) {
                handle.shutdown().wait();
            }
        }
    }

    fn icon_pixels(widget: Option<&TrayWidget>, limits: &ProviderLimits) -> Vec<u8> {
        widget.map_or_else(
            || app_icon_pixels().to_vec(),
            |widget| render_widget(widget, limits),
        )
    }

    fn tooltip_for(widget: Option<&TrayWidget>, limits: &ProviderLimits) -> String {
        widget
            .map(|widget| widget_tooltip(widget, limits))
            .unwrap_or_else(|| "Codex Minibar".into())
    }

    /// RGBA rows to the ARGB32 (network byte order) pixmap SNI expects.
    fn icon_from_rgba(pixels: &[u8]) -> ksni::Icon {
        let data = pixels
            .chunks_exact(4)
            .flat_map(|pixel| [pixel[3], pixel[0], pixel[1], pixel[2]])
            .collect();
        ksni::Icon {
            width: ICON_SIZE as i32,
            height: ICON_SIZE as i32,
            data,
        }
    }

    /// Without a StatusNotifierWatcher (stock GNOME without the AppIndicator
    /// extension) the icons never show. Tell the user how to reach the app.
    fn warn_when_no_tray_host() {
        let Some(bus) = crate::linux::desktop::session_bus() else {
            return;
        };
        let has_watcher = bus
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "NameHasOwner",
                &("org.kde.StatusNotifierWatcher",),
            )
            .ok()
            .and_then(|reply| reply.body().deserialize::<bool>().ok())
            .unwrap_or(true);
        if !has_watcher {
            crate::logger::info("No StatusNotifierItem host is running; tray icons are hidden");
            crate::notifications::show(
                crate::i18n::tr("linux-no-tray-title"),
                crate::i18n::tr("linux-no-tray-body"),
            );
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn tray_click_and_menu_actions_reach_the_bridge() {
            use ksni::Tray;

            let mut item = Item {
                slot: 0,
                icon: icon_from_rgba(&vec![0; ICON_SIZE * ICON_SIZE * 4]),
                tooltip: String::new(),
                update_available: false,
            };
            item.activate(120, 30);
            let menu = item.menu();
            for position in [2, 3] {
                let ksni::MenuItem::Standard(action) = &menu[position] else {
                    panic!("expected a tray menu action");
                };
                assert!(action.enabled);
                (action.activate)(&mut item);
            }
            assert_eq!(
                TrayManager::new().drain_events(),
                vec![
                    TrayEvent::Activate(120, 30),
                    TrayEvent::Menu(TrayMenuAction::ShowPopup),
                    TrayEvent::Menu(TrayMenuAction::Settings),
                ]
            );
            item.update_available = true;
            assert_eq!(item.menu().len(), menu.len() + 1);
        }

        #[test]
        fn pixmaps_are_argb_in_network_order() {
            let mut rgba = vec![0u8; ICON_SIZE * ICON_SIZE * 4];
            rgba[..4].copy_from_slice(&[10, 20, 30, 40]);
            let icon = icon_from_rgba(&rgba);
            assert_eq!(&icon.data[..4], &[40, 10, 20, 30]);
            assert_eq!(icon.data.len(), rgba.len());
        }
    }
}

#[cfg(not(windows))]
pub use platform::{TrayEvent, TrayManager, TrayMenuAction};

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::settings::ProviderKind;

    #[cfg(not(windows))]
    #[test]
    fn fontconfig_tray_font_works_without_desktop_utilities_on_path() {
        const CHILD: &str = "MINIBAR_TRAY_FONT_PROBE";
        if std::env::var_os(CHILD).is_some() {
            let font = fontconfig_bold_sans().expect("native fontconfig must find the fixture");
            let (metrics, pixels) = font.rasterize('8', 18.0);
            assert!(metrics.width > 0 && metrics.height > 0);
            assert!(pixels.iter().any(|pixel| *pixel > 0));
            return;
        }
        let root = tempfile::tempdir().unwrap();
        // No standard system directories, no filename extension, no helper
        // executable: only a user-installed font registered with fontconfig.
        fs::write(
            root.path().join("user-font"),
            include_bytes!("../assets/fonts/geist/Geist-Bold.ttf"),
        )
        .unwrap();
        let config = root.path().join("fonts.conf");
        fs::write(&config, format!("<?xml version=\"1.0\"?><!DOCTYPE fontconfig SYSTEM \"urn:fontconfig:fonts.dtd\"><fontconfig><dir>{}</dir><cachedir>{}/cache</cachedir></fontconfig>", root.path().display(), root.path().display())).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tray::tests::fontconfig_tray_font_works_without_desktop_utilities_on_path",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env("FONTCONFIG_FILE", config)
            .env("FONTCONFIG_PATH", root.path())
            .env("PATH", root.path().join("no-utilities"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn tray_font_handle_preserves_the_selected_collection_face() {
        // Build a TTC containing regular then bold. SFNT table offsets in a
        // collection are absolute, so rebase each embedded font's table list.
        let faces: [&[u8]; 2] = [
            include_bytes!("../assets/fonts/geist/Geist-Regular.ttf"),
            include_bytes!("../assets/fonts/geist/Geist-Bold.ttf"),
        ];
        let mut collection = b"ttcf\0\x01\0\0\0\0\0\x02".to_vec();
        collection.resize(20, 0);
        for (index, face) in faces.iter().enumerate() {
            let start = collection.len();
            collection[12 + index * 4..16 + index * 4]
                .copy_from_slice(&(start as u32).to_be_bytes());
            collection.extend_from_slice(face);
            let tables = u16::from_be_bytes([face[4], face[5]]) as usize;
            for table in 0..tables {
                let offset = 12 + table * 16 + 8;
                let original = u32::from_be_bytes(face[offset..offset + 4].try_into().unwrap());
                collection[start + offset..start + offset + 4]
                    .copy_from_slice(&(original + start as u32).to_be_bytes());
            }
            while !collection.len().is_multiple_of(4) {
                collection.push(0);
            }
        }
        let font = font_from_handle(font_kit::handle::Handle::from_memory(
            std::sync::Arc::new(collection),
            1,
        ))
        .unwrap();
        let expected = Font::from_bytes(faces[1], FontSettings::default()).unwrap();
        let regular = Font::from_bytes(faces[0], FontSettings::default()).unwrap();
        let rendered = font.rasterize('8', 18.0);
        assert_eq!(rendered, expected.rasterize('8', 18.0));
        assert_ne!(rendered, regular.rasterize('8', 18.0));
    }

    #[test]
    fn tray_instances_resolve_independently_and_missing_instances_keep_their_slot() {
        use crate::instances::ProviderId;
        let primary = ProviderId::primary(ProviderKind::Claude);
        let work = ProviderId::new(ProviderKind::Claude, "claude-tray-work");
        let mut limits = ProviderLimits::default();
        limits.get_mut(primary).primary.used_percent = Some(7);
        limits.get_mut(work).primary.used_percent = Some(80);
        let mut widget = TrayWidget::custom_for_provider(primary);
        let default = crate::settings::TrayIndicator::new(primary, "claude.session");
        let mut second = default.clone();
        second.provider_id = work.id().into();
        widget.indicators = vec![default, second];
        let resolved = resolve_indicators(&widget, &limits, [0, 120, 212]);
        assert_eq!(resolved[0].displayed_percent, Some(93));
        assert_eq!(resolved[1].displayed_percent, Some(20));
        assert_eq!(widget_tooltip(&widget, &limits).lines().count(), 2);
        let settings = crate::settings::Settings {
            tray_widgets: vec![widget.clone()],
            ..Default::default()
        };
        let decoded: crate::settings::Settings =
            toml::from_str(&toml::to_string(&settings).unwrap()).unwrap();
        assert_eq!(
            decoded.tray_widgets[0].indicators[1].provider_id,
            "claude-tray-work"
        );
        widget.indicators[0].provider_id = "claude-removed".into();
        let missing = resolve_indicators(&widget, &limits, [0, 120, 212]);
        assert_eq!(missing.len(), 2);
        assert_eq!(missing[0].displayed_percent, None);
        assert_eq!(missing[1].displayed_percent, Some(20));
    }

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
        ProviderLimits::from_entries([(ProviderKind::Codex.into(), limits())])
    }

    #[test]
    fn renders_rgba_icon_with_visible_pixels() {
        let mut widget = TrayWidget::custom_for_provider(crate::instances::ProviderId::from(
            ProviderKind::Codex,
        ));
        widget.presentation = TrayPresentation::Number;
        let pixels = render_widget(&widget, &provider_limits());
        assert_eq!(pixels.len(), ICON_SIZE * ICON_SIZE * 4);
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
    }

    #[test]
    fn stacked_three_digit_values_use_a_compact_font() {
        assert_eq!(text_font_size(&["100", "100"]), 14.0);
        assert_eq!(text_font_size(&["99", "99"]), 18.0);
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
        let mut widget = TrayWidget::custom_for_provider(crate::instances::ProviderId::from(
            ProviderKind::Codex,
        ));
        widget.presentation = TrayPresentation::Number;
        let limits = ProviderLimits::from_entries([(ProviderKind::Codex.into(), limits)]);
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
    fn missing_quota_keeps_the_widget_instead_of_using_the_app_icon() {
        let mut widget = TrayWidget::custom_for_provider(crate::instances::ProviderId::from(
            ProviderKind::Codex,
        ));
        for presentation in [
            TrayPresentation::Number,
            TrayPresentation::StackedBars,
            TrayPresentation::NestedRings,
            TrayPresentation::ResetTime,
            TrayPresentation::ResetCountdown,
        ] {
            widget.presentation = presentation;
            let pixels = render_widget(&widget, &ProviderLimits::default());
            assert_eq!(pixels.len(), ICON_SIZE * ICON_SIZE * 4);
            assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
            assert_ne!(pixels, app_icon_pixels());
        }
        let tooltip = widget_tooltip(&widget, &ProviderLimits::default());
        assert!(tooltip.contains("Codex"));
        assert!(tooltip.contains('?'));
        assert_eq!(
            render_widget(&TrayWidget::app_icon(), &ProviderLimits::default()),
            app_icon_pixels()
        );
    }

    #[test]
    fn partially_missing_quota_preserves_available_values() {
        let limits = ProviderLimits::from_entries([(
            ProviderKind::Codex.into(),
            RateLimits {
                primary: LimitWindow {
                    used_percent: Some(38),
                    ..Default::default()
                },
                ..Default::default()
            },
        )]);
        let mut widget = TrayWidget::custom_for_provider(ProviderKind::Codex.into());
        widget.indicators.push(crate::settings::TrayIndicator::new(
            ProviderKind::Claude.into(),
            "claude.session",
        ));
        let indicators = resolve_indicators(&widget, &limits, [100, 100, 100]);
        assert!(
            indicators
                .iter()
                .any(|value| value.displayed_percent.is_some())
        );
        assert!(
            indicators
                .iter()
                .any(|value| value.displayed_percent.is_none())
        );
        let expected = render_percentages(&indicators);
        for presentation in [TrayPresentation::StackedBars, TrayPresentation::NestedRings] {
            widget.presentation = presentation;
            assert_eq!(render_widget(&widget, &limits), expected);
        }
        let tooltip = widget_tooltip(&widget, &limits);
        assert!(tooltip.contains('?'));
        assert!(tooltip.contains("62%"));
    }

    #[test]
    fn renders_three_indicators_with_independent_status_colors() {
        let limits = ProviderLimits::from_entries([
            (
                ProviderKind::Codex.into(),
                RateLimits {
                    primary: LimitWindow {
                        used_percent: Some(38),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ),
            (
                ProviderKind::Claude.into(),
                RateLimits {
                    primary: LimitWindow {
                        used_percent: Some(55),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ),
            (
                ProviderKind::Cursor.into(),
                RateLimits {
                    secondary: LimitWindow {
                        used_percent: Some(88),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ),
        ]);
        let mut widget = TrayWidget::custom_for_provider(crate::instances::ProviderId::from(
            ProviderKind::Codex,
        ));
        widget.indicators = vec![
            crate::settings::TrayIndicator::new(
                crate::instances::ProviderId::from(ProviderKind::Codex),
                "codex.session",
            ),
            crate::settings::TrayIndicator::new(
                crate::instances::ProviderId::from(ProviderKind::Claude),
                "claude.session",
            ),
            crate::settings::TrayIndicator::new(
                crate::instances::ProviderId::from(ProviderKind::Cursor),
                "cursor.auto",
            ),
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
