//! Direct2D renderer shared by the taskbar host and the Settings previews.
//!
//! Draws into a 32-bit premultiplied DIB, which the host hands to
//! `UpdateLayeredWindow`. Everything is laid out in DIPs; the render target's
//! DPI maps them to device pixels. Geometry and colors follow the Windows 11
//! taskbar: 4 DIP inset, 4 DIP corners, a 6 % white hover fill with a lighter
//! top edge, Segoe UI Variable text and WinUI progress styling.
//!
//! Widgets lie along the taskbar: side by side on a horizontal taskbar,
//! stacked on a vertical one. Each widget lays out from its own origin; the
//! main axis runs along the taskbar and the cross axis spans its thickness.

#![cfg(windows)]

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use windows::{
    Win32::{
        Foundation::RECT,
        Graphics::{
            Direct2D::{Common::*, *},
            DirectWrite::*,
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
            Gdi::HDC,
        },
    },
    core::{HSTRING, PCWSTR, w},
};
use windows_numerics::Vector2;

use super::model::{Appearance, Icon, Rgb, Row, WidgetView};
use crate::settings::TaskbarWidgetStyle;

/// Inset of the hover plate from the taskbar's long edges.
const INSET: f32 = 4.0;
const RADIUS: f32 = 4.0;
const PAD_START: f32 = 8.0;
const PAD_END: f32 = 10.0;
const ICON: f32 = 20.0;
const ICON_GAP: f32 = 8.0;
const LABEL_GAP: f32 = 4.0;
const RING: f32 = 28.0;
const RING_GAP: f32 = 6.0;
const UNIT_GAP: f32 = 12.0;
const NESTED: f32 = 34.0;
const DIAL: f32 = 30.0;
const DIAL_GAP: f32 = 4.0;
const COLUMN: f32 = 6.0;
const COLUMN_GAP: f32 = 4.0;
const BAR_SINGLE: f32 = 72.0;
const BAR_MULTI: f32 = 44.0;
const BAR_THICKNESS: f32 = 3.0;
const PACE_WIDTH: f32 = 1.5;
/// How far the pace mark reaches past a bar or ring stroke.
const PACE_OVERHANG: f32 = 2.5;
/// Padding at both ends of a widget on a vertical taskbar.
const PAD_VERTICAL: f32 = 6.0;
const MAX_LABEL_CHARS: usize = 18;

/// Device geometry of the strip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Metrics {
    /// Device pixels per DIP.
    pub(crate) scale: f32,
    /// Taskbar thickness in DIPs: its height, or its width when vertical.
    pub(crate) thickness: f32,
    /// Widgets stack top to bottom on a left or right taskbar.
    pub(crate) vertical: bool,
}

impl Metrics {
    fn inner(self) -> f32 {
        (self.thickness - INSET * 2.0).max(16.0)
    }

    /// Font size of the two-line layouts; 12 DIP like the clock, smaller on
    /// the compact taskbar.
    fn line_font(self) -> f32 {
        (self.inner() / 2.0 / 1.36).clamp(8.5, 12.0)
    }

    /// Font size when `rows` lines share the taskbar height.
    fn rows_font(self, rows: usize) -> f32 {
        if rows > 2 {
            (self.inner() / rows as f32 / 1.3).clamp(8.0, 10.0)
        } else {
            (self.line_font() - 1.0).max(8.0)
        }
    }

    fn snap(self, value: f32) -> f32 {
        (value * self.scale).round() / self.scale
    }
}

/// One widget placed on the strip, with its animation state.
pub(crate) struct Item<'a> {
    pub(crate) view: &'a WidgetView,
    /// Start along the taskbar, in DIPs.
    pub(crate) offset: f32,
    /// Extent along the taskbar, in DIPs.
    pub(crate) length: f32,
    pub(crate) hover: f32,
    pub(crate) press: f32,
    pub(crate) opacity: f32,
    /// Animated fill per row; `None` rows draw only the track.
    pub(crate) progress: &'a [Option<f32>],
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Weight {
    Regular,
    Semibold,
}

#[derive(Clone, Copy, PartialEq)]
enum Tone {
    Primary,
    Secondary,
    Tint(Rgb),
}

enum Prim {
    Text {
        layout: IDWriteTextLayout,
        x: f32,
        y: f32,
        tone: Tone,
    },
    Icon {
        icon: Icon,
        x: f32,
        y: f32,
        size: f32,
    },
    /// A horizontal WinUI progress bar centered on `y`.
    Bar {
        x: f32,
        y: f32,
        width: f32,
        row: usize,
        color: Rgb,
        pace: Option<f32>,
    },
    /// A progress ring filling clockwise from twelve o'clock.
    Ring {
        cx: f32,
        cy: f32,
        radius: f32,
        stroke: f32,
        row: usize,
        color: Rgb,
        pace: Option<f32>,
    },
    /// A level column filling from the bottom.
    Column {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        row: usize,
        color: Rgb,
        pace: Option<f32>,
    },
    /// A hairline divider.
    Rule { x: f32, y: f32, width: f32 },
}

struct Content {
    /// Extent along the taskbar.
    length: f32,
    prims: Vec<Prim>,
}

struct Line {
    layout: IDWriteTextLayout,
    width: f32,
    height: f32,
}

type Runs = Vec<(Line, Tone)>;

/// A bar row's label, value and reset time.
type Cells = (Option<Line>, Option<Line>, Option<Line>);

fn runs_width(runs: &Runs) -> f32 {
    runs.iter().map(|(line, _)| line.width).sum::<f32>()
        + LABEL_GAP * runs.len().saturating_sub(1) as f32
}

fn runs_height(runs: &Runs) -> f32 {
    runs.iter().map(|(line, _)| line.height).fold(0.0, f32::max)
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct IconKey {
    name: &'static str,
    pixels: u32,
    tint: Rgb,
}

pub(crate) struct Renderer {
    factory: ID2D1Factory,
    dwrite: IDWriteFactory,
    target: ID2D1DCRenderTarget,
    brush: ID2D1SolidColorBrush,
    round: ID2D1StrokeStyle,
    family: HSTRING,
    formats: HashMap<(u32, Weight), IDWriteTextFormat>,
    icons: HashMap<IconKey, Option<ID2D1Bitmap>>,
    dpi: f32,
}

fn color(rgb: Rgb, alpha: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: f32::from(rgb[0]) / 255.0,
        g: f32::from(rgb[1]) / 255.0,
        b: f32::from(rgb[2]) / 255.0,
        a: alpha.clamp(0.0, 1.0),
    }
}

fn rect(left: f32, top: f32, right: f32, bottom: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left,
        top,
        right,
        bottom,
    }
}

fn truncate(label: &str) -> String {
    if label.chars().count() <= MAX_LABEL_CHARS {
        return label.to_owned();
    }
    let mut short = label
        .chars()
        .take(MAX_LABEL_CHARS - 1)
        .collect::<String>()
        .trim_end()
        .to_owned();
    short.push('\u{2026}');
    short
}

/// Point on a circle, `turn` of the way clockwise from twelve o'clock.
fn on_circle(center: Vector2, radius: f32, turn: f32) -> Vector2 {
    let angle = turn * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
    Vector2 {
        x: center.x + radius * angle.cos(),
        y: center.y + radius * angle.sin(),
    }
}

impl Renderer {
    pub(crate) fn new() -> windows::core::Result<Self> {
        unsafe {
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let (target, brush) = Self::create_target(&factory, 96.0)?;
            let round = factory.CreateStrokeStyle(
                &D2D1_STROKE_STYLE_PROPERTIES {
                    startCap: D2D1_CAP_STYLE_ROUND,
                    endCap: D2D1_CAP_STYLE_ROUND,
                    dashCap: D2D1_CAP_STYLE_ROUND,
                    lineJoin: D2D1_LINE_JOIN_ROUND,
                    miterLimit: 10.0,
                    dashStyle: D2D1_DASH_STYLE_SOLID,
                    dashOffset: 0.0,
                },
                None,
            )?;
            let family = Self::resolve_family(&dwrite);
            Ok(Self {
                factory,
                dwrite,
                target,
                brush,
                round,
                family,
                formats: HashMap::new(),
                icons: HashMap::new(),
                dpi: 96.0,
            })
        }
    }

    fn create_target(
        factory: &ID2D1Factory,
        dpi: f32,
    ) -> windows::core::Result<(ID2D1DCRenderTarget, ID2D1SolidColorBrush)> {
        unsafe {
            let target = factory.CreateDCRenderTarget(&D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: dpi,
                dpiY: dpi,
                usage: D2D1_RENDER_TARGET_USAGE_GDI_COMPATIBLE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            })?;
            target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            let brush = target.CreateSolidColorBrush(&color([255, 255, 255], 1.0), None)?;
            Ok((target, brush))
        }
    }

    /// Segoe UI Variable Text on Windows 11, Segoe UI before it.
    fn resolve_family(dwrite: &IDWriteFactory) -> HSTRING {
        unsafe {
            let mut collection = None;
            if dwrite
                .GetSystemFontCollection(&mut collection, false)
                .is_ok()
                && let Some(collection) = collection
            {
                let mut index = 0;
                let mut exists = windows::core::BOOL(0);
                if collection
                    .FindFamilyName(w!("Segoe UI Variable Text"), &mut index, &mut exists)
                    .is_ok()
                    && exists.as_bool()
                {
                    return HSTRING::from("Segoe UI Variable Text");
                }
            }
        }
        HSTRING::from("Segoe UI")
    }

    fn set_dpi(&mut self, scale: f32) -> windows::core::Result<()> {
        let dpi = 96.0 * scale;
        if (dpi - self.dpi).abs() > f32::EPSILON {
            unsafe { self.target.SetDpi(dpi, dpi) };
            self.dpi = dpi;
            // Icons are rasterized at device size.
            self.icons.clear();
        }
        Ok(())
    }

    fn recreate(&mut self) -> windows::core::Result<()> {
        let (target, brush) = Self::create_target(&self.factory, self.dpi)?;
        self.target = target;
        self.brush = brush;
        self.icons.clear();
        Ok(())
    }

    fn format(&mut self, size: f32, weight: Weight) -> windows::core::Result<IDWriteTextFormat> {
        let key = ((size * 100.0).round() as u32, weight);
        if let Some(format) = self.formats.get(&key) {
            return Ok(format.clone());
        }
        let format = unsafe {
            let format = self.dwrite.CreateTextFormat(
                PCWSTR(self.family.as_ptr()),
                None,
                match weight {
                    Weight::Regular => DWRITE_FONT_WEIGHT_NORMAL,
                    Weight::Semibold => DWRITE_FONT_WEIGHT_SEMI_BOLD,
                },
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                w!(""),
            )?;
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP).ok()?;
            format
        };
        self.formats.insert(key, format.clone());
        Ok(format)
    }

    fn line(&mut self, text: &str, size: f32, weight: Weight) -> windows::core::Result<Line> {
        let format = self.format(size, weight)?;
        let wide = text.encode_utf16().collect::<Vec<_>>();
        unsafe {
            let layout = self
                .dwrite
                .CreateTextLayout(&wide, &format, 4096.0, 256.0)?;
            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut metrics).ok()?;
            Ok(Line {
                layout,
                width: metrics.widthIncludingTrailingWhitespace,
                height: metrics.height,
            })
        }
    }

    fn icon_bitmap(&mut self, icon: &Icon, size: f32) -> Option<ID2D1Bitmap> {
        let pixels = (size * self.dpi / 96.0).round().max(1.0) as u32;
        let key = IconKey {
            name: icon.name,
            pixels,
            tint: icon.tint,
        };
        if let Some(bitmap) = self.icons.get(&key) {
            return bitmap.clone();
        }
        let bitmap = rasterize_icon(icon.name, pixels, icon.tint).and_then(|data| unsafe {
            self.target
                .CreateBitmap(
                    D2D_SIZE_U {
                        width: pixels,
                        height: pixels,
                    },
                    Some(data.as_ptr().cast()),
                    pixels * 4,
                    &D2D1_BITMAP_PROPERTIES {
                        pixelFormat: D2D1_PIXEL_FORMAT {
                            format: DXGI_FORMAT_B8G8R8A8_UNORM,
                            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                        },
                        dpiX: 96.0,
                        dpiY: 96.0,
                    },
                )
                .ok()
        });
        self.icons.insert(key, bitmap.clone());
        bitmap
    }

    /// Extent of a widget along the taskbar, in DIPs.
    pub(crate) fn measure(
        &mut self,
        view: &WidgetView,
        metrics: Metrics,
    ) -> windows::core::Result<f32> {
        self.set_dpi(metrics.scale)?;
        Ok(metrics.snap(self.content(view, metrics)?.length))
    }

    fn content(&mut self, view: &WidgetView, metrics: Metrics) -> windows::core::Result<Content> {
        if metrics.vertical {
            return self.column(view, metrics);
        }
        match view.style {
            TaskbarWidgetStyle::Glance => self.glance(view, metrics),
            TaskbarWidgetStyle::Rings => self.rings(view, metrics),
            TaskbarWidgetStyle::Bars => self.bars(view, metrics),
            TaskbarWidgetStyle::Nested => self.nested(view, metrics),
            TaskbarWidgetStyle::Dials => self.dials(view, metrics),
            TaskbarWidgetStyle::Meters => self.meters(view, metrics),
        }
    }

    fn icon_size(metrics: Metrics) -> f32 {
        ICON.min(metrics.inner() - 12.0).max(12.0)
    }

    fn pace(view: &WidgetView, row: &Row) -> Option<f32> {
        row.pace.filter(|_| view.show_pace)
    }

    /// Leading provider mark; returns the x where content continues.
    fn leading_icon(view: &WidgetView, metrics: Metrics, prims: &mut Vec<Prim>) -> f32 {
        let Some(icon) = view.icon.clone() else {
            return PAD_START + 2.0;
        };
        let size = Self::icon_size(metrics);
        prims.push(Prim::Icon {
            icon,
            x: PAD_START,
            y: (metrics.thickness - size) / 2.0,
            size,
        });
        PAD_START + size + ICON_GAP
    }

    /// Stacks lines vertically centered in the taskbar, left-aligned at `x`
    /// (or right-aligned to `x + block` when `right` is set).
    fn stack(
        lines: Vec<Runs>,
        x: f32,
        metrics: Metrics,
        right: bool,
        prims: &mut Vec<Prim>,
    ) -> f32 {
        let widths = lines.iter().map(runs_width).collect::<Vec<_>>();
        let block = widths.iter().copied().fold(0.0, f32::max);
        let heights = lines.iter().map(runs_height).collect::<Vec<_>>();
        let total = heights.iter().sum::<f32>();
        let mut y = (metrics.thickness - total) / 2.0;
        for ((runs, width), height) in lines.into_iter().zip(widths).zip(heights) {
            let cursor = if right { x + block - width } else { x };
            Self::place_runs(runs, cursor, y, height, prims);
            y += height;
        }
        block
    }

    /// Lays `runs` left to right from `x`, sharing the bottom edge of a
    /// line `height` tall at `y`.
    fn place_runs(runs: Runs, x: f32, y: f32, height: f32, prims: &mut Vec<Prim>) {
        let mut cursor = x;
        for (line, tone) in runs {
            let advance = line.width + LABEL_GAP;
            prims.push(Prim::Text {
                // Baselines of mixed sizes share the bottom edge.
                y: y + height - line.height,
                layout: line.layout,
                x: cursor,
                tone,
            });
            cursor += advance;
        }
    }

    fn value_tone(row: &Row) -> Tone {
        if row.progress.is_none() {
            Tone::Secondary
        } else if row.tinted {
            Tone::Tint(row.color)
        } else {
            Tone::Primary
        }
    }

    /// A row's pieces in reading order, as far as the widget shows them:
    /// `5h 45% 2h 13m`. Text-only designs fall back to the value rather than
    /// say nothing.
    fn row_runs(
        &mut self,
        view: &WidgetView,
        row: &Row,
        font: f32,
        value_weight: Weight,
    ) -> windows::core::Result<Runs> {
        let mut runs = Vec::with_capacity(3);
        if view.show_labels {
            runs.push((
                self.line(&truncate(&row.label), font, Weight::Regular)?,
                Tone::Secondary,
            ));
        }
        if view.show_values || (runs.is_empty() && row.reset.is_none() && !view.style.has_graphic())
        {
            runs.push((
                self.line(&row.value, font, value_weight)?,
                Self::value_tone(row),
            ));
        }
        if let Some(reset) = &row.reset {
            runs.push((self.line(reset, font, Weight::Regular)?, Tone::Secondary));
        }
        Ok(runs)
    }

    /// One row as a headline over a caption: the value over `label ·
    /// reset`, or whichever of them the widget shows. Empty when it shows
    /// none of them.
    fn headline(
        &mut self,
        view: &WidgetView,
        row: &Row,
        font: f32,
        weight: Weight,
        caption_font: f32,
    ) -> windows::core::Result<Vec<Runs>> {
        let label = view.show_labels.then(|| truncate(&row.label));
        let mut caption = Vec::new();
        let headline = if view.show_values {
            caption.extend(label);
            caption.extend(row.reset.clone());
            Some((self.line(&row.value, font, weight)?, Self::value_tone(row)))
        } else if let Some(reset) = &row.reset {
            caption.extend(label);
            Some((self.line(reset, font, weight)?, Tone::Primary))
        } else if let Some(label) = label {
            Some((self.line(&label, font, weight)?, Tone::Primary))
        } else if !view.style.has_graphic() {
            Some((self.line(&row.value, font, weight)?, Self::value_tone(row)))
        } else {
            None
        };
        let mut lines = Vec::with_capacity(2);
        lines.extend(headline.map(|run| vec![run]));
        if !caption.is_empty() {
            lines.push(vec![(
                self.line(&caption.join(" \u{00b7} "), caption_font, Weight::Regular)?,
                Tone::Secondary,
            )]);
        }
        Ok(lines)
    }

    /// One line per row, stacked beside a graphic at `x`; returns the
    /// width used, nothing when the widget writes nothing.
    fn values_beside(
        &mut self,
        view: &WidgetView,
        x: f32,
        metrics: Metrics,
        prims: &mut Vec<Prim>,
    ) -> windows::core::Result<f32> {
        if !view.has_text() {
            return Ok(0.0);
        }
        let font = metrics.rows_font(view.rows.len());
        let mut lines = Vec::with_capacity(view.rows.len());
        for row in &view.rows {
            lines.push(self.row_runs(view, row, font, Weight::Semibold)?);
        }
        Ok(Self::stack(lines, x, metrics, false, prims))
    }

    fn glance(&mut self, view: &WidgetView, metrics: Metrics) -> windows::core::Result<Content> {
        let mut prims = Vec::new();
        let x = Self::leading_icon(view, metrics, &mut prims);
        let font = metrics.line_font();
        let lines = if let [row] = view.rows.as_slice() {
            self.headline(view, row, font, Weight::Semibold, font)?
        } else {
            let mut lines = Vec::new();
            for row in &view.rows {
                lines.push(self.row_runs(view, row, font, Weight::Semibold)?);
            }
            lines
        };
        let block = Self::stack(lines, x, metrics, false, &mut prims);
        Ok(Content {
            length: x + block + PAD_END,
            prims,
        })
    }

    fn bars(&mut self, view: &WidgetView, metrics: Metrics) -> windows::core::Result<Content> {
        let mut prims = Vec::new();
        let x = Self::leading_icon(view, metrics, &mut prims);
        if !view.has_text() {
            // The bars alone, evenly spaced around the middle.
            let width = if view.rows.len() > 1 {
                BAR_MULTI
            } else {
                BAR_SINGLE
            };
            let pitch = 8.0;
            let top = (metrics.thickness - pitch * (view.rows.len() as f32 - 1.0)) / 2.0;
            for (index, row) in view.rows.iter().enumerate() {
                prims.push(Prim::Bar {
                    x,
                    y: top + pitch * index as f32,
                    width,
                    row: index,
                    color: row.color,
                    pace: Self::pace(view, row),
                });
            }
            return Ok(Content {
                length: x + width + PAD_END,
                prims,
            });
        }
        if let [row] = view.rows.as_slice() {
            // `label · reset` over the bar's start, the value over its end.
            let font = metrics.line_font();
            let mut caption = Vec::new();
            if view.show_labels {
                caption.push(truncate(&row.label));
            }
            caption.extend(row.reset.clone());
            let caption = if caption.is_empty() {
                None
            } else {
                Some(self.line(&caption.join(" \u{00b7} "), font - 1.0, Weight::Regular)?)
            };
            let value = if view.show_values {
                Some(self.line(&row.value, font, Weight::Semibold)?)
            } else {
                None
            };
            let caption_width = caption.as_ref().map_or(0.0, |line| line.width + 12.0);
            let value_width = value.as_ref().map_or(0.0, |line| line.width);
            let width = BAR_SINGLE.max(caption_width + value_width);
            let text_height = caption
                .iter()
                .chain(&value)
                .map(|line| line.height)
                .fold(0.0, f32::max);
            let gap = 5.0;
            let top = (metrics.thickness - text_height - gap - BAR_THICKNESS) / 2.0;
            if let Some(caption) = caption {
                prims.push(Prim::Text {
                    x,
                    y: top + text_height - caption.height,
                    layout: caption.layout,
                    tone: Tone::Secondary,
                });
            }
            if let Some(value) = value {
                prims.push(Prim::Text {
                    x: x + width - value.width,
                    y: top + text_height - value.height,
                    layout: value.layout,
                    tone: Self::value_tone(row),
                });
            }
            prims.push(Prim::Bar {
                x,
                y: top + text_height + gap + BAR_THICKNESS / 2.0,
                width,
                row: 0,
                color: row.color,
                pace: Self::pace(view, row),
            });
            return Ok(Content {
                length: x + width + PAD_END,
                prims,
            });
        }
        // Columns: label | bar | value | reset.
        let count = view.rows.len().max(1) as f32;
        let font = metrics.rows_font(view.rows.len());
        let mut cells = Vec::with_capacity(view.rows.len());
        for row in &view.rows {
            let label = if view.show_labels {
                Some(self.line(&truncate(&row.label), font, Weight::Regular)?)
            } else {
                None
            };
            let value = if view.show_values {
                Some(self.line(&row.value, font, Weight::Semibold)?)
            } else {
                None
            };
            let reset = match &row.reset {
                Some(reset) => Some(self.line(reset, font, Weight::Regular)?),
                None => None,
            };
            cells.push((label, value, reset));
        }
        let widest = |pick: fn(&Cells) -> Option<&Line>| {
            cells
                .iter()
                .filter_map(pick)
                .map(|line| line.width)
                .fold(0.0, f32::max)
        };
        let label_width = widest(|cell| cell.0.as_ref());
        let value_width = widest(|cell| cell.1.as_ref());
        let reset_width = widest(|cell| cell.2.as_ref());
        let pitch = cells
            .iter()
            .flat_map(|(label, value, reset)| [label, value, reset])
            .flatten()
            .map(|line| line.height)
            .fold(0.0, f32::max)
            .min((metrics.inner() - 2.0) / count);
        let top = (metrics.thickness - pitch * count) / 2.0;
        let bar_x = if label_width > 0.0 {
            x + label_width + 6.0
        } else {
            x
        };
        let value_x = bar_x + BAR_MULTI + 6.0;
        let reset_x = if value_width > 0.0 {
            value_x + value_width + LABEL_GAP + 2.0
        } else {
            value_x
        };
        let end = if value_width + reset_width > 0.0 {
            reset_x + reset_width
        } else {
            bar_x + BAR_MULTI
        };
        for (index, ((label, value, reset), row)) in cells.into_iter().zip(&view.rows).enumerate() {
            let center = top + pitch * (index as f32 + 0.5);
            if let Some(label) = label {
                prims.push(Prim::Text {
                    x,
                    y: center - label.height / 2.0,
                    layout: label.layout,
                    tone: Tone::Secondary,
                });
            }
            prims.push(Prim::Bar {
                x: bar_x,
                y: center,
                width: BAR_MULTI,
                row: index,
                color: row.color,
                pace: Self::pace(view, row),
            });
            if let Some(value) = value {
                prims.push(Prim::Text {
                    x: value_x + value_width - value.width,
                    y: center - value.height / 2.0,
                    layout: value.layout,
                    tone: Self::value_tone(row),
                });
            }
            if let Some(reset) = reset {
                prims.push(Prim::Text {
                    x: reset_x,
                    y: center - reset.height / 2.0,
                    layout: reset.layout,
                    tone: Tone::Secondary,
                });
            }
        }
        Ok(Content {
            length: end + PAD_END,
            prims,
        })
    }

    fn rings(&mut self, view: &WidgetView, metrics: Metrics) -> windows::core::Result<Content> {
        let mut prims = Vec::new();
        let diameter = RING.min(metrics.inner() - 8.0).max(16.0);
        let stroke = if diameter >= 24.0 { 3.0 } else { 2.5 };
        let font = metrics.line_font();
        let mut x = PAD_START;
        for (index, row) in view.rows.iter().enumerate() {
            if index > 0 {
                x += if view.has_text() { UNIT_GAP } else { RING_GAP };
            }
            let cx = x + diameter / 2.0;
            let cy = metrics.thickness / 2.0;
            prims.push(Prim::Ring {
                cx,
                cy,
                radius: (diameter - stroke) / 2.0,
                stroke,
                row: index,
                color: row.color,
                pace: Self::pace(view, row),
            });
            if view.icon.is_some() {
                let size = (diameter * 0.46).round();
                prims.push(Prim::Icon {
                    icon: row.icon.clone(),
                    x: cx - size / 2.0,
                    y: cy - size / 2.0,
                    size,
                });
            }
            x += diameter;
            let lines = self.headline(view, row, font, Weight::Semibold, font - 1.0)?;
            if lines.is_empty() {
                continue;
            }
            x += RING_GAP;
            x += Self::stack(lines, x, metrics, false, &mut prims);
        }
        Ok(Content {
            length: x + PAD_END,
            prims,
        })
    }

    /// Concentric rings centered at `center`, the first indicator outermost.
    /// Returns the diameter of the free disc inside the innermost ring.
    fn concentric(view: &WidgetView, center: Vector2, diameter: f32, prims: &mut Vec<Prim>) -> f32 {
        let count = view.rows.len().max(1);
        let stroke = if count > 2 { 2.5 } else { 3.0 };
        let gap = 1.5;
        let mut radius = (diameter - stroke) / 2.0;
        for (index, row) in view.rows.iter().enumerate() {
            if index > 0 {
                radius -= stroke + gap;
            }
            prims.push(Prim::Ring {
                cx: center.x,
                cy: center.y,
                radius,
                stroke,
                row: index,
                color: row.color,
                pace: Self::pace(view, row),
            });
        }
        ((radius - stroke / 2.0) * 2.0 - 3.0).max(0.0)
    }

    fn nested(&mut self, view: &WidgetView, metrics: Metrics) -> windows::core::Result<Content> {
        let mut prims = Vec::new();
        let diameter = NESTED.min(metrics.inner() - 6.0).max(18.0);
        // Two rings leave room for the mark inside; three push it out front.
        let inside = view.rows.len() <= 2;
        let mut x = if inside {
            PAD_START
        } else {
            Self::leading_icon(view, metrics, &mut prims)
        };
        let center = Vector2 {
            x: x + diameter / 2.0,
            y: metrics.thickness / 2.0,
        };
        let hole = Self::concentric(view, center, diameter, &mut prims);
        if inside && let Some(icon) = view.icon.clone() {
            let size = hole.min(16.0).floor();
            if size >= 8.0 {
                prims.push(Prim::Icon {
                    icon,
                    x: center.x - size / 2.0,
                    y: center.y - size / 2.0,
                    size,
                });
            }
        }
        x += diameter;
        if view.has_text() {
            x += RING_GAP + 2.0;
            x += self.values_beside(view, x, metrics, &mut prims)?;
        }
        Ok(Content {
            length: x + PAD_END,
            prims,
        })
    }

    /// The value inside a dial: the number alone, `%` would not fit.
    fn dial_text(&mut self, row: &Row, diameter: f32) -> windows::core::Result<Line> {
        let number = row.value.trim_end_matches('%');
        let size = if number.chars().count() > 2 {
            diameter * 0.3
        } else {
            diameter * 0.37
        };
        self.line(number, size.clamp(8.0, 12.0), Weight::Semibold)
    }

    fn dial(
        &mut self,
        view: &WidgetView,
        index: usize,
        center: Vector2,
        diameter: f32,
        prims: &mut Vec<Prim>,
    ) -> windows::core::Result<()> {
        let row = &view.rows[index];
        let stroke = 2.5;
        prims.push(Prim::Ring {
            cx: center.x,
            cy: center.y,
            radius: (diameter - stroke) / 2.0,
            stroke,
            row: index,
            color: row.color,
            pace: Self::pace(view, row),
        });
        if !view.show_values {
            return Ok(());
        }
        let text = self.dial_text(row, diameter)?;
        prims.push(Prim::Text {
            x: center.x - text.width / 2.0,
            y: center.y - text.height / 2.0,
            layout: text.layout,
            tone: Self::value_tone(row),
        });
        Ok(())
    }

    fn dials(&mut self, view: &WidgetView, metrics: Metrics) -> windows::core::Result<Content> {
        let mut prims = Vec::new();
        let diameter = DIAL.min(metrics.inner() - 6.0).max(18.0);
        let mut x = Self::leading_icon(view, metrics, &mut prims);
        if view.icon.is_some() {
            x -= ICON_GAP - DIAL_GAP;
        }
        for index in 0..view.rows.len() {
            if index > 0 {
                x += DIAL_GAP;
            }
            let center = Vector2 {
                x: x + diameter / 2.0,
                y: metrics.thickness / 2.0,
            };
            self.dial(view, index, center, diameter, &mut prims)?;
            x += diameter;
        }
        Ok(Content {
            length: x + PAD_START,
            prims,
        })
    }

    /// Level columns side by side from `x`, vertically centered on `cy`.
    fn columns(view: &WidgetView, x: f32, cy: f32, height: f32, prims: &mut Vec<Prim>) -> f32 {
        let mut cursor = x;
        for (index, row) in view.rows.iter().enumerate() {
            if index > 0 {
                cursor += COLUMN_GAP;
            }
            prims.push(Prim::Column {
                x: cursor,
                y: cy - height / 2.0,
                width: COLUMN,
                height,
                row: index,
                color: row.color,
                pace: Self::pace(view, row),
            });
            cursor += COLUMN;
        }
        cursor - x
    }

    fn meters(&mut self, view: &WidgetView, metrics: Metrics) -> windows::core::Result<Content> {
        let mut prims = Vec::new();
        let mut x = Self::leading_icon(view, metrics, &mut prims);
        let height = (metrics.inner() - 10.0).max(12.0);
        x += Self::columns(view, x, metrics.thickness / 2.0, height, &mut prims);
        if view.has_text() {
            x += ICON_GAP;
            x += self.values_beside(view, x, metrics, &mut prims)?;
        }
        Ok(Content {
            length: x + PAD_END,
            prims,
        })
    }

    /// A widget on a vertical taskbar: the mark, the graphic and the values
    /// stacked top to bottom, each centered across the taskbar.
    fn column(&mut self, view: &WidgetView, metrics: Metrics) -> windows::core::Result<Content> {
        let mut prims = Vec::new();
        let cross = metrics.thickness;
        let room = (cross - INSET * 2.0 - 8.0).max(16.0);
        let middle = cross / 2.0;
        let mut y = PAD_VERTICAL;
        let style = view.style;
        // Rings carry the mark inside each ring, two nested rings inside
        // the innermost one.
        let icon_inside = style == TaskbarWidgetStyle::Rings
            || (style == TaskbarWidgetStyle::Nested && view.rows.len() <= 2);
        let icon_first = !icon_inside;
        if icon_first && let Some(icon) = view.icon.clone() {
            let size = Self::icon_size(metrics).min(room);
            prims.push(Prim::Icon {
                icon,
                x: middle - size / 2.0,
                y,
                size,
            });
            y += size + 6.0;
        }
        let font = if view.rows.len() > 2 { 11.0 } else { 12.0 };
        // Value lines under the graphic; rings, dials and bars carry their
        // values with each indicator instead.
        let values = matches!(style, TaskbarWidgetStyle::Glance)
            || (matches!(
                style,
                TaskbarWidgetStyle::Nested | TaskbarWidgetStyle::Meters
            ) && view.show_values);
        match style {
            TaskbarWidgetStyle::Rings | TaskbarWidgetStyle::Dials => {
                let diameter = if style == TaskbarWidgetStyle::Dials {
                    DIAL
                } else {
                    RING
                }
                .min(room);
                for (index, row) in view.rows.iter().enumerate() {
                    let center = Vector2 {
                        x: middle,
                        y: y + diameter / 2.0,
                    };
                    if style == TaskbarWidgetStyle::Dials {
                        self.dial(view, index, center, diameter, &mut prims)?;
                        y += diameter + DIAL_GAP;
                        continue;
                    }
                    let stroke = 3.0;
                    prims.push(Prim::Ring {
                        cx: center.x,
                        cy: center.y,
                        radius: (diameter - stroke) / 2.0,
                        stroke,
                        row: index,
                        color: row.color,
                        pace: Self::pace(view, row),
                    });
                    if view.icon.is_some() {
                        let size = (diameter * 0.46).round();
                        prims.push(Prim::Icon {
                            icon: row.icon.clone(),
                            x: center.x - size / 2.0,
                            y: center.y - size / 2.0,
                            size,
                        });
                    }
                    y += diameter + 3.0;
                    if view.show_values {
                        y += self.centered(view, row, font, middle, room, y, &mut prims)? + 3.0;
                    }
                    y += 3.0;
                }
                if style == TaskbarWidgetStyle::Dials {
                    y += 6.0 - DIAL_GAP;
                }
            }
            TaskbarWidgetStyle::Nested => {
                let diameter = NESTED.min(room);
                let center = Vector2 {
                    x: middle,
                    y: y + diameter / 2.0,
                };
                let hole = Self::concentric(view, center, diameter, &mut prims);
                if !icon_first && let Some(icon) = view.icon.clone() {
                    let size = hole.min(16.0).floor();
                    if size >= 8.0 {
                        prims.push(Prim::Icon {
                            icon,
                            x: center.x - size / 2.0,
                            y: center.y - size / 2.0,
                            size,
                        });
                    }
                }
                y += diameter + 6.0;
            }
            TaskbarWidgetStyle::Meters => {
                let count = view.rows.len().max(1) as f32;
                let width = count * COLUMN + (count - 1.0) * COLUMN_GAP;
                let height = 28.0;
                Self::columns(
                    view,
                    middle - width / 2.0,
                    y + height / 2.0,
                    height,
                    &mut prims,
                );
                y += height + 6.0;
            }
            TaskbarWidgetStyle::Bars => {
                let width = room;
                for (index, row) in view.rows.iter().enumerate() {
                    if view.show_values {
                        y += self.centered(view, row, font, middle, room, y, &mut prims)? + 3.0;
                    }
                    prims.push(Prim::Bar {
                        x: middle - width / 2.0,
                        y: y + BAR_THICKNESS / 2.0,
                        width,
                        row: index,
                        color: row.color,
                        pace: Self::pace(view, row),
                    });
                    y += BAR_THICKNESS + 6.0;
                }
            }
            TaskbarWidgetStyle::Glance => {}
        }
        if values {
            for row in &view.rows {
                y += self.centered(view, row, font, middle, room, y, &mut prims)?;
            }
            y += PAD_VERTICAL;
        }
        Ok(Content { length: y, prims })
    }

    /// One row's `label value` centered on `middle`, dropping the label when
    /// it does not fit `room`. Returns the line height.
    #[allow(clippy::too_many_arguments)]
    fn centered(
        &mut self,
        view: &WidgetView,
        row: &Row,
        font: f32,
        middle: f32,
        room: f32,
        y: f32,
        prims: &mut Vec<Prim>,
    ) -> windows::core::Result<f32> {
        self.centered_with(view, row, font, Weight::Semibold, middle, room, y, prims)
    }

    #[allow(clippy::too_many_arguments)]
    fn centered_with(
        &mut self,
        view: &WidgetView,
        row: &Row,
        font: f32,
        weight: Weight,
        middle: f32,
        room: f32,
        y: f32,
        prims: &mut Vec<Prim>,
    ) -> windows::core::Result<f32> {
        let mut runs = self.row_runs(view, row, font, weight)?;
        // Too wide for the taskbar: drop the label, then the reset time.
        if runs.len() > 1 && view.show_labels && runs_width(&runs) > room {
            runs.remove(0);
        }
        if runs.len() > 1 && row.reset.is_some() && runs_width(&runs) > room {
            runs.pop();
        }
        let width = runs_width(&runs);
        let height = runs_height(&runs);
        Self::place_runs(runs, middle - width / 2.0, y, height, prims);
        Ok(height)
    }

    /// Paints the strip into a DIB selected into `hdc`.
    pub(crate) fn paint(
        &mut self,
        hdc: *mut core::ffi::c_void,
        size: (i32, i32),
        metrics: Metrics,
        appearance: Appearance,
        items: &[Item<'_>],
        backdrop: bool,
    ) -> windows::core::Result<()> {
        self.set_dpi(metrics.scale)?;
        let result = self.paint_once(hdc, size, metrics, appearance, items, backdrop);
        if let Err(error) = &result
            && error.code() == windows::Win32::Foundation::D2DERR_RECREATE_TARGET
        {
            self.recreate()?;
            return self.paint_once(hdc, size, metrics, appearance, items, backdrop);
        }
        result
    }

    fn paint_once(
        &mut self,
        hdc: *mut core::ffi::c_void,
        size: (i32, i32),
        metrics: Metrics,
        appearance: Appearance,
        items: &[Item<'_>],
        backdrop: bool,
    ) -> windows::core::Result<()> {
        let bounds = RECT {
            left: 0,
            top: 0,
            right: size.0,
            bottom: size.1,
        };
        // Build every text layout before drawing begins.
        let mut contents = Vec::with_capacity(items.len());
        for item in items {
            contents.push(self.content(item.view, metrics)?);
        }
        unsafe {
            self.target.BindDC(HDC(hdc), &bounds).ok()?;
            self.target.BeginDraw();
            // A barely-there backdrop keeps the whole window hit-testable;
            // fully transparent pixels of a layered window let clicks through.
            let alpha = if backdrop { 1.0 / 255.0 } else { 0.0 };
            self.target.Clear(Some(&color([0, 0, 0], alpha)));
        }
        for (item, content) in items.iter().zip(contents) {
            self.paint_item(item, content, metrics, appearance);
        }
        unsafe { self.target.EndDraw(None, None).ok() }
    }

    fn fill(&self, shape: &D2D1_ROUNDED_RECT, rgb: Rgb, alpha: f32) {
        unsafe {
            self.brush.SetColor(&color(rgb, alpha));
            self.target.FillRoundedRectangle(shape, &self.brush);
        }
    }

    fn paint_item(
        &mut self,
        item: &Item<'_>,
        content: Content,
        metrics: Metrics,
        appearance: Appearance,
    ) {
        let offset = metrics.snap(item.offset);
        let (plate, translate) = if metrics.vertical {
            (
                rect(
                    INSET,
                    offset,
                    metrics.thickness - INSET,
                    offset + item.length,
                ),
                windows_numerics::Matrix3x2::translation(0.0, offset),
            )
        } else {
            (
                rect(
                    offset,
                    INSET,
                    offset + item.length,
                    metrics.thickness - INSET,
                ),
                windows_numerics::Matrix3x2::translation(offset, 0.0),
            )
        };
        let plate = D2D1_ROUNDED_RECT {
            rect: plate,
            radiusX: RADIUS,
            radiusY: RADIUS,
        };
        let opacity = item.opacity.clamp(0.0, 1.0);
        // Layered windows let clicks fall through fully transparent pixels;
        // the whole plate stays hit-testable without becoming visible.
        self.fill(&plate, [255, 255, 255], 1.0 / 255.0);
        self.paint_plate(&plate, item.hover, item.press, opacity, appearance);
        // Pressed content dims like a taskbar button.
        let content_opacity = opacity * (1.0 - 0.2 * item.press);
        unsafe { self.target.SetTransform(&translate) };
        for prim in content.prims {
            self.paint_prim(prim, item.progress, metrics, appearance, content_opacity);
        }
        unsafe {
            self.target
                .SetTransform(&windows_numerics::Matrix3x2::identity());
        }
    }

    fn paint_plate(
        &self,
        plate: &D2D1_ROUNDED_RECT,
        hover: f32,
        press: f32,
        opacity: f32,
        appearance: Appearance,
    ) {
        let amount = hover.max(press) * opacity;
        if amount <= 0.001 {
            return;
        }
        let (fill, edge, edge_alpha) = if appearance.light {
            // Light taskbar buttons brighten toward white with a hairline.
            ([255, 255, 255], [0, 0, 0], 0.07)
        } else {
            ([255, 255, 255], [255, 255, 255], 0.045)
        };
        let fill_alpha = match (appearance.light, press > 0.5) {
            (false, false) => 0.0605,
            (false, true) => 0.04,
            (true, false) => 0.7,
            (true, true) => 0.5,
        };
        self.fill(plate, fill, fill_alpha * amount);
        // Elevation edge: a lighter top in dark mode, a darker bottom in light.
        unsafe {
            let top = plate.rect.top;
            let bottom = plate.rect.bottom;
            let (start, end) = if appearance.light {
                (bottom, bottom - 3.0)
            } else {
                (top, top + 3.0)
            };
            let stops = [
                D2D1_GRADIENT_STOP {
                    position: 0.0,
                    color: color(edge, edge_alpha * amount),
                },
                D2D1_GRADIENT_STOP {
                    position: 1.0,
                    color: color(edge, 0.0),
                },
            ];
            let Ok(collection) = self.target.CreateGradientStopCollection(
                &stops,
                D2D1_GAMMA_2_2,
                D2D1_EXTEND_MODE_CLAMP,
            ) else {
                return;
            };
            let Ok(gradient) = self.target.CreateLinearGradientBrush(
                &D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                    startPoint: Vector2 { x: 0.0, y: start },
                    endPoint: Vector2 { x: 0.0, y: end },
                },
                None,
                &collection,
            ) else {
                return;
            };
            let half = 0.5 / (self.dpi / 96.0);
            let edge_rect = D2D1_ROUNDED_RECT {
                rect: rect(
                    plate.rect.left + half,
                    plate.rect.top + half,
                    plate.rect.right - half,
                    plate.rect.bottom - half,
                ),
                radiusX: RADIUS - half,
                radiusY: RADIUS - half,
            };
            self.target
                .DrawRoundedRectangle(&edge_rect, &gradient, half * 2.0, None);
        }
    }

    fn tone_color(tone: Tone, appearance: Appearance) -> (Rgb, f32) {
        match tone {
            Tone::Primary => (appearance.text(), appearance.text_alpha()),
            Tone::Secondary => (appearance.text(), appearance.secondary_alpha()),
            Tone::Tint(rgb) => (rgb, 1.0),
        }
    }

    fn track_alpha(appearance: Appearance) -> f32 {
        // WinUI ProgressBar track: ControlStrongStrokeColorDefault.
        if appearance.light { 0.446 } else { 0.545 }
    }

    /// The pace mark: white on a dark taskbar, near-black on a light one,
    /// where white would vanish.
    fn set_pace_color(&self, appearance: Appearance, opacity: f32) {
        unsafe {
            self.brush
                .SetColor(&color(appearance.text(), appearance.text_alpha() * opacity));
        }
    }

    fn paint_prim(
        &mut self,
        prim: Prim,
        progress: &[Option<f32>],
        metrics: Metrics,
        appearance: Appearance,
        opacity: f32,
    ) {
        let fill_of = |row: usize| progress.get(row).copied().flatten();
        match prim {
            Prim::Text { layout, x, y, tone } => {
                let (rgb, alpha) = Self::tone_color(tone, appearance);
                unsafe {
                    self.brush.SetColor(&color(rgb, alpha * opacity));
                    self.target.DrawTextLayout(
                        Vector2 {
                            x: metrics.snap(x),
                            y: metrics.snap(y),
                        },
                        &layout,
                        &self.brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                    );
                }
            }
            Prim::Icon { icon, x, y, size } => {
                if let Some(bitmap) = self.icon_bitmap(&icon, size) {
                    let (x, y) = (metrics.snap(x), metrics.snap(y));
                    let device = (size * metrics.scale).round() / metrics.scale;
                    unsafe {
                        self.target.DrawBitmap(
                            &bitmap,
                            Some(&rect(x, y, x + device, y + device)),
                            opacity,
                            D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                            None,
                        );
                    }
                }
                if let Some(badge) = &icon.badge {
                    // Keep the plate inside the icon bounds so every layout,
                    // including rings and vertical taskbars, reserves enough room.
                    let height = (size * 0.46).clamp(9.0, 14.0);
                    if let Ok(line) = self.line(&badge.text, height * 0.68, Weight::Semibold) {
                        let width = (line.width + 4.0).max(height);
                        let left = x + size - width;
                        let top = y + size - height;
                        let (background, foreground) = badge
                            .color
                            .rgb()
                            .zip(badge.color.text_rgb())
                            .map(|(background, foreground)| {
                                (Rgb::from(background), Rgb::from(foreground))
                            })
                            .unwrap_or((
                                appearance.text(),
                                if appearance.light {
                                    [255, 255, 255]
                                } else {
                                    [0, 0, 0]
                                },
                            ));
                        self.fill(
                            &D2D1_ROUNDED_RECT {
                                rect: rect(left, top, left + width, top + height),
                                radiusX: height * 0.3,
                                radiusY: height * 0.3,
                            },
                            background,
                            opacity,
                        );
                        unsafe {
                            self.brush.SetColor(&color(foreground, opacity));
                            self.target.DrawTextLayout(
                                Vector2 {
                                    x: metrics.snap(left + (width - line.width) / 2.0),
                                    y: metrics.snap(top + (height - line.height) / 2.0),
                                },
                                &line.layout,
                                &self.brush,
                                D2D1_DRAW_TEXT_OPTIONS_NONE,
                            );
                        }
                    }
                }
            }
            Prim::Rule { x, y, width } => unsafe {
                let one = 1.0 / metrics.scale;
                self.brush.SetColor(&color(
                    appearance.text(),
                    if appearance.light { 0.08 } else { 0.0837 } * opacity,
                ));
                let y = metrics.snap(y);
                self.target
                    .FillRectangle(&rect(x, y, x + width, y + one), &self.brush);
            },
            Prim::Bar {
                x,
                y,
                width,
                row,
                color: rgb,
                pace,
            } => unsafe {
                let one = 1.0 / metrics.scale;
                let y = metrics.snap(y);
                self.brush.SetColor(&color(
                    appearance.text(),
                    Self::track_alpha(appearance) * opacity,
                ));
                self.target.FillRectangle(
                    &rect(x, y - one / 2.0, x + width, y + one / 2.0),
                    &self.brush,
                );
                if let Some(fill) = fill_of(row)
                    && fill > 0.004
                {
                    let half = BAR_THICKNESS / 2.0;
                    let length = (width * fill.clamp(0.0, 1.0)).max(BAR_THICKNESS);
                    self.fill(
                        &D2D1_ROUNDED_RECT {
                            rect: rect(x, y - half, x + length, y + half),
                            radiusX: half,
                            radiusY: half,
                        },
                        rgb,
                        opacity,
                    );
                }
                if let Some(pace) = pace {
                    let at = metrics.snap(x + width * pace.clamp(0.0, 1.0));
                    let reach = BAR_THICKNESS / 2.0 + PACE_OVERHANG;
                    self.set_pace_color(appearance, opacity);
                    self.fill_pace(rect(
                        at - PACE_WIDTH / 2.0,
                        y - reach,
                        at + PACE_WIDTH / 2.0,
                        y + reach,
                    ));
                }
            },
            Prim::Column {
                x,
                y,
                width,
                height,
                row,
                color: rgb,
                pace,
            } => {
                let half = width / 2.0;
                let track = D2D1_ROUNDED_RECT {
                    rect: rect(x, y, x + width, y + height),
                    radiusX: half,
                    radiusY: half,
                };
                let track_alpha = if appearance.light { 0.1 } else { 0.14 };
                self.fill(&track, appearance.text(), track_alpha * opacity);
                if let Some(fill) = fill_of(row)
                    && fill > 0.004
                {
                    let level = (height * fill.clamp(0.0, 1.0)).max(width);
                    self.fill(
                        &D2D1_ROUNDED_RECT {
                            rect: rect(x, y + height - level, x + width, y + height),
                            radiusX: half,
                            radiusY: half,
                        },
                        rgb,
                        opacity,
                    );
                }
                if let Some(pace) = pace {
                    let at = metrics.snap(y + height * (1.0 - pace.clamp(0.0, 1.0)));
                    self.set_pace_color(appearance, opacity);
                    self.fill_pace(rect(
                        x - PACE_OVERHANG + 1.0,
                        at - PACE_WIDTH / 2.0,
                        x + width + PACE_OVERHANG - 1.0,
                        at + PACE_WIDTH / 2.0,
                    ));
                }
            }
            Prim::Ring {
                cx,
                cy,
                radius,
                stroke,
                row,
                color: rgb,
                pace,
            } => unsafe {
                let one = 1.0 / metrics.scale;
                let center = Vector2 { x: cx, y: cy };
                let circle = D2D1_ELLIPSE {
                    point: center,
                    radiusX: radius,
                    radiusY: radius,
                };
                self.brush.SetColor(&color(
                    appearance.text(),
                    Self::track_alpha(appearance) * opacity,
                ));
                self.target.DrawEllipse(&circle, &self.brush, one, None);
                if let Some(fill) = fill_of(row).map(|fill| fill.clamp(0.0, 1.0))
                    && fill > 0.004
                {
                    self.brush.SetColor(&color(rgb, opacity));
                    if fill >= 0.999 {
                        self.target
                            .DrawEllipse(&circle, &self.brush, stroke, &self.round);
                    } else if let Some(arc) = self.arc(center, radius, fill) {
                        self.target
                            .DrawGeometry(&arc, &self.brush, stroke, &self.round);
                    }
                }
                if let Some(pace) = pace {
                    let reach = stroke / 2.0 + PACE_OVERHANG - 1.0;
                    let turn = pace.clamp(0.0, 1.0);
                    self.set_pace_color(appearance, opacity);
                    self.target.DrawLine(
                        on_circle(center, (radius - reach).max(0.0), turn),
                        on_circle(center, radius + reach, turn),
                        &self.brush,
                        PACE_WIDTH,
                        &self.round,
                    );
                }
            },
        }
    }

    fn fill_pace(&self, mark: D2D_RECT_F) {
        let radius = PACE_WIDTH / 2.0;
        unsafe {
            self.target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT {
                    rect: mark,
                    radiusX: radius,
                    radiusY: radius,
                },
                &self.brush,
            );
        }
    }

    /// Clockwise arc from twelve o'clock covering `fill` of the circle.
    fn arc(&self, center: Vector2, radius: f32, fill: f32) -> Option<ID2D1PathGeometry> {
        unsafe {
            let path = self.factory.CreatePathGeometry().ok()?;
            let sink = path.Open().ok()?;
            sink.BeginFigure(on_circle(center, radius, 0.0), D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: on_circle(center, radius, fill),
                size: D2D_SIZE_F {
                    width: radius,
                    height: radius,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: if fill > 0.5 {
                    D2D1_ARC_SIZE_LARGE
                } else {
                    D2D1_ARC_SIZE_SMALL
                },
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close().ok().ok()?;
            Some(path)
        }
    }
}

/// Transparent margin around the hover card that holds its shadow.
pub(crate) const CARD_MARGIN: f32 = 16.0;
const CARD_WIDTH: f32 = 272.0;
const CARD_PAD: f32 = 14.0;
const CARD_RADIUS: f32 = 8.0;

impl Renderer {
    /// Hover card content, laid out from the card's top-left corner: one
    /// section per provider with every quota and today's usage. Bars index
    /// [`card_progress`] order.
    fn card_content(
        &mut self,
        view: &WidgetView,
        now: DateTime<Utc>,
    ) -> windows::core::Result<(f32, Vec<Prim>)> {
        let mut prims = Vec::new();
        let inner = CARD_WIDTH - CARD_PAD * 2.0;
        let mut y = CARD_PAD;
        let mut bar = 0;
        for (index, section) in view.card.iter().enumerate() {
            if index > 0 {
                y += 12.0;
                prims.push(Prim::Rule {
                    x: CARD_PAD,
                    y,
                    width: inner,
                });
                y += 12.0;
            }
            // Mark, name and plan.
            let name = self.line(&section.provider.qualified_name(), 14.0, Weight::Semibold)?;
            prims.push(Prim::Icon {
                icon: section.icon.clone(),
                x: CARD_PAD,
                y: y + (name.height - 16.0) / 2.0,
                size: 16.0,
            });
            let name_x = CARD_PAD + 16.0 + 8.0;
            let (name_width, name_height) = (name.width, name.height);
            prims.push(Prim::Text {
                layout: name.layout,
                x: name_x,
                y,
                tone: Tone::Primary,
            });
            if let Some(plan) = &section.plan {
                let plan = self.line(plan, 12.0, Weight::Regular)?;
                prims.push(Prim::Text {
                    x: name_x + name_width + 6.0,
                    y: y + name_height - plan.height - 1.0,
                    layout: plan.layout,
                    tone: Tone::Secondary,
                });
            }
            y += name_height + 10.0;
            for (row_index, row) in section.rows.iter().enumerate() {
                if row_index > 0 {
                    y += 12.0;
                }
                let label = self.line(&row.full_label, 12.0, Weight::Regular)?;
                let value_text = if row.progress.is_some() {
                    let key = match row.limit_value {
                        crate::settings::LimitValue::Remaining => "taskbar-card-left",
                        crate::settings::LimitValue::Used => "taskbar-card-used",
                    };
                    crate::i18n::format(key, &[("value", row.value.clone())])
                } else {
                    row.value.clone()
                };
                let value = self.line(&value_text, 12.0, Weight::Semibold)?;
                let line_height = label.height.max(value.height);
                prims.push(Prim::Text {
                    layout: label.layout,
                    x: CARD_PAD,
                    y,
                    tone: Tone::Primary,
                });
                prims.push(Prim::Text {
                    x: CARD_PAD + inner - value.width,
                    y,
                    layout: value.layout,
                    tone: Self::value_tone(row),
                });
                y += line_height + 6.0;
                prims.push(Prim::Bar {
                    x: CARD_PAD,
                    y: y + BAR_THICKNESS / 2.0,
                    width: inner,
                    row: bar,
                    color: row.color,
                    pace: row.pace,
                });
                bar += 1;
                y += BAR_THICKNESS + 6.0;
                let detail = match row.resets_at.filter(|reset| *reset > now) {
                    Some(reset) if row.progress.is_some() => crate::i18n::format(
                        "taskbar-card-resets",
                        &[
                            ("time", super::model::countdown(reset, now)),
                            ("clock", super::model::reset_clock(reset)),
                        ],
                    ),
                    _ if row.progress.is_none() => {
                        crate::i18n::tr("taskbar-card-no-data").to_owned()
                    }
                    _ => String::new(),
                };
                if !detail.is_empty() {
                    let detail = self.line(&detail, 12.0, Weight::Regular)?;
                    let height = detail.height;
                    prims.push(Prim::Text {
                        layout: detail.layout,
                        x: CARD_PAD,
                        y,
                        tone: Tone::Secondary,
                    });
                    y += height;
                }
            }
            if let Some(today) = &section.today {
                y += 12.0;
                let label = self.line(crate::i18n::tr("today"), 12.0, Weight::Regular)?;
                let value = self.line(today, 12.0, Weight::Semibold)?;
                let height = label.height.max(value.height);
                prims.push(Prim::Text {
                    layout: label.layout,
                    x: CARD_PAD,
                    y,
                    tone: Tone::Primary,
                });
                prims.push(Prim::Text {
                    x: CARD_PAD + inner - value.width,
                    y,
                    layout: value.layout,
                    tone: Tone::Primary,
                });
                y += height;
            }
        }
        y += CARD_PAD;
        Ok((y, prims))
    }

    /// Size of the hover card window in DIPs, shadow margin included.
    pub(crate) fn card_size(
        &mut self,
        view: &WidgetView,
        scale: f32,
        now: DateTime<Utc>,
    ) -> windows::core::Result<(f32, f32)> {
        self.set_dpi(scale)?;
        let (height, _) = self.card_content(view, now)?;
        Ok((CARD_WIDTH + CARD_MARGIN * 2.0, height + CARD_MARGIN * 2.0))
    }

    /// Paints the hover card: a Fluent flyout surface with a soft shadow.
    /// `rise` lifts the card for its entrance, in DIPs.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_card(
        &mut self,
        hdc: *mut core::ffi::c_void,
        size: (i32, i32),
        scale: f32,
        appearance: Appearance,
        view: &WidgetView,
        opacity: f32,
        rise: f32,
        now: DateTime<Utc>,
    ) -> windows::core::Result<()> {
        self.set_dpi(scale)?;
        let (height, prims) = self.card_content(view, now)?;
        let progress = view
            .card
            .iter()
            .flat_map(|section| section.rows.iter().map(|row| row.progress))
            .collect::<Vec<_>>();
        let bounds = RECT {
            left: 0,
            top: 0,
            right: size.0,
            bottom: size.1,
        };
        let metrics = Metrics {
            scale,
            thickness: height + CARD_MARGIN * 2.0,
            vertical: false,
        };
        let opacity = opacity.clamp(0.0, 1.0);
        unsafe {
            self.target.BindDC(HDC(hdc), &bounds).ok()?;
            self.target.BeginDraw();
            self.target.Clear(Some(&color([0, 0, 0], 0.0)));
            let top = metrics.snap(CARD_MARGIN + rise);
            let left = CARD_MARGIN;
            // Shadow: Fluent flyout depth, 0 8 16 at 14 % (26 % dark).
            let depth = if appearance.light { 0.14 } else { 0.26 };
            let steps = 8;
            for step in 0..steps {
                let spread = step as f32 * 1.5;
                let alpha = depth * opacity * (1.0 - step as f32 / steps as f32).powi(2) / 3.0;
                self.fill(
                    &D2D1_ROUNDED_RECT {
                        rect: rect(
                            left - spread,
                            top + 6.0 - spread,
                            left + CARD_WIDTH + spread,
                            top + height + 6.0 + spread,
                        ),
                        radiusX: CARD_RADIUS + spread,
                        radiusY: CARD_RADIUS + spread,
                    },
                    [0, 0, 0],
                    alpha,
                );
            }
            let surface = D2D1_ROUNDED_RECT {
                rect: rect(left, top, left + CARD_WIDTH, top + height),
                radiusX: CARD_RADIUS,
                radiusY: CARD_RADIUS,
            };
            let (background, stroke, stroke_alpha) = if appearance.light {
                ([249, 249, 249], [0, 0, 0], 0.0578)
            } else {
                ([44, 44, 44], [0, 0, 0], 0.32)
            };
            self.fill(&surface, background, opacity);
            let half = 0.5 / scale;
            let edge = D2D1_ROUNDED_RECT {
                rect: rect(
                    left + half,
                    top + half,
                    left + CARD_WIDTH - half,
                    top + height - half,
                ),
                radiusX: CARD_RADIUS - half,
                radiusY: CARD_RADIUS - half,
            };
            self.brush.SetColor(&color(stroke, stroke_alpha * opacity));
            self.target
                .DrawRoundedRectangle(&edge, &self.brush, half * 2.0, None);
            if !appearance.light {
                // Dark flyouts carry a faint light rim inside the dark one.
                let inner = D2D1_ROUNDED_RECT {
                    rect: rect(
                        edge.rect.left + half * 2.0,
                        edge.rect.top + half * 2.0,
                        edge.rect.right - half * 2.0,
                        edge.rect.bottom - half * 2.0,
                    ),
                    radiusX: CARD_RADIUS - half * 3.0,
                    radiusY: CARD_RADIUS - half * 3.0,
                };
                self.brush.SetColor(&color([255, 255, 255], 0.06 * opacity));
                self.target
                    .DrawRoundedRectangle(&inner, &self.brush, half * 2.0, None);
            }
            self.target
                .SetTransform(&windows_numerics::Matrix3x2::translation(left, top));
        }
        for prim in prims {
            self.paint_prim(prim, &progress, metrics, appearance, opacity);
        }
        unsafe {
            self.target
                .SetTransform(&windows_numerics::Matrix3x2::identity());
            self.target.EndDraw(None, None).ok()
        }
    }
}

/// Premultiplied BGRA alpha mask of a monochrome icon, tinted.
fn rasterize_icon(name: &str, pixels: u32, tint: Rgb) -> Option<Vec<u8>> {
    use resvg::{tiny_skia, usvg};
    let source = crate::popup_window::ui::assets::svg_source(name)?;
    let svg = crate::popup_window::ui::assets::normalize_svg(source);
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).ok()?;
    let size = tree.size();
    let scale = pixels as f32 / size.width().max(size.height());
    let mut pixmap = tiny_skia::Pixmap::new(pixels, pixels)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Some(
        pixmap
            .pixels()
            .iter()
            .flat_map(|pixel| {
                let alpha = u16::from(pixel.alpha());
                let channel = |value: u8| ((u16::from(value) * alpha + 127) / 255) as u8;
                [
                    channel(tint[2]),
                    channel(tint[1]),
                    channel(tint[0]),
                    pixel.alpha(),
                ]
            })
            .collect(),
    )
}

/// Straight-alpha RGBA rendering of widgets, for previews in Settings.
pub(crate) struct Snapshot {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

/// Renders `views` side by side the way the taskbar host lays them out.
pub(crate) fn snapshot(
    renderer: &mut Renderer,
    views: &[WidgetView],
    metrics: Metrics,
    appearance: Appearance,
    hovered: Option<usize>,
) -> Option<Snapshot> {
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
        DeleteDC, DeleteObject, SelectObject,
    };
    let mut widths = Vec::with_capacity(views.len());
    for view in views {
        widths.push(renderer.measure(view, metrics).ok()?);
    }
    let gap = super::WIDGET_GAP;
    let total = widths.iter().sum::<f32>() + gap * widths.len().saturating_sub(1) as f32;
    let width = (total * metrics.scale).ceil().max(1.0) as i32;
    let height = (metrics.thickness * metrics.scale).round().max(1.0) as i32;
    let progress = views
        .iter()
        .map(|view| view.rows.iter().map(|row| row.progress).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let mut x = 0.0;
    let items = views
        .iter()
        .zip(&widths)
        .zip(&progress)
        .enumerate()
        .map(|(index, ((view, width), progress))| {
            let item = Item {
                view,
                offset: x,
                length: *width,
                hover: if hovered == Some(index) { 1.0 } else { 0.0 },
                press: 0.0,
                opacity: 1.0,
                progress,
            };
            x += width + gap;
            item
        })
        .collect::<Vec<_>>();
    unsafe {
        let dc = CreateCompatibleDC(std::ptr::null_mut());
        if dc.is_null() {
            return None;
        }
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width;
        info.bmiHeader.biHeight = -height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut bits = std::ptr::null_mut();
        let dib = CreateDIBSection(
            dc,
            &info,
            DIB_RGB_COLORS,
            &mut bits,
            std::ptr::null_mut(),
            0,
        );
        if dib.is_null() || bits.is_null() {
            DeleteDC(dc);
            return None;
        }
        let previous = SelectObject(dc, dib);
        let painted = renderer
            .paint(dc, (width, height), metrics, appearance, &items, false)
            .is_ok();
        let pixels = std::slice::from_raw_parts(bits as *const u8, (width * height * 4) as usize);
        let rgba = painted.then(|| {
            pixels
                .chunks_exact(4)
                .flat_map(|pixel| {
                    let alpha = pixel[3];
                    let straight = |value: u8| {
                        if alpha == 0 {
                            0
                        } else {
                            ((u16::from(value) * 255 + u16::from(alpha) / 2) / u16::from(alpha))
                                .min(255) as u8
                        }
                    };
                    [
                        straight(pixel[2]),
                        straight(pixel[1]),
                        straight(pixel[0]),
                        alpha,
                    ]
                })
                .collect::<Vec<_>>()
        });
        SelectObject(dc, previous);
        DeleteObject(dib);
        DeleteDC(dc);
        rgba.map(|rgba| Snapshot {
            width: width as u32,
            height: height as u32,
            rgba,
        })
    }
}
