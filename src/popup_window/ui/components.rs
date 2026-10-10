//! Stateless building blocks shared by every popup page.
//!
//! Typography mirrors the WinUI type ramp the popup used before (Caption
//! 12/16, Body 14/20, Body Strong 14/20 semibold), so card proportions stay
//! the same after the GPUI rewrite.

use gpui::{
    AnyElement, Div, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, div, px,
    relative, svg,
};

use super::{assets::icon_path, fx::RollFrame, theme::Palette};

pub(crate) const PROGRESS_TRACK_HEIGHT: f32 = 6.0;

/// Measure with the same font, size and weight used to render the label.
pub(crate) fn measure_text(
    text_system: &gpui::WindowTextSystem,
    family: SharedString,
    size: f32,
    weight: FontWeight,
    value: &str,
) -> f32 {
    let mut font = gpui::font(family);
    font.weight = weight;
    value
        .lines()
        .map(|line| {
            let run = gpui::TextRun {
                len: line.len(),
                font: font.clone(),
                color: gpui::black(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            f32::from(
                text_system
                    .shape_line(line.to_owned().into(), px(size), &[run], None)
                    .width,
            )
        })
        .fold(0.0_f32, f32::max)
        .ceil()
}
const INTERVAL_TICK_WIDTH: f32 = 2.0;
const INTERVAL_TICK_HEIGHT: f32 = PROGRESS_TRACK_HEIGHT - 2.0;
const CARD_EDGE_MARKER_HEIGHT: f32 = INTERVAL_TICK_HEIGHT + 2.0;

pub(crate) fn text(value: impl Into<SharedString>, size: f32, line: f32, color: Hsla) -> Div {
    div()
        .text_size(px(size))
        .line_height(px(line))
        .text_color(color)
        .child(value.into())
}

/// Odometer-style label. A digit that changed spins through every digit in
/// between (upward when the value rose, downward when it fell), and glyphs
/// fade as they leave the line, like a soft mask on its edges. Other
/// characters (separators, a new leading digit) slide in or out.
///
/// Every glyph glides from its place in the shaped old text to its place in
/// the shaped new text, and the label's width follows, so the first frame
/// matches the old label and the last matches the new one exactly (kerning
/// included) before it settles back into plain text. See [`roll_pairs`] for
/// how the two texts line up. At rest it is a plain single-line text
/// element, so callers may still wrap it in [`nowrap`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn rolling_text(
    frame: &RollFrame,
    text_system: &gpui::WindowTextSystem,
    family: SharedString,
    size: f32,
    line: f32,
    weight: FontWeight,
    color: Hsla,
) -> Div {
    let glyphs = |value: String| {
        text(value, size, line, color)
            .font_weight(weight)
            .whitespace_nowrap()
    };
    let Some(from) = frame.from.as_ref() else {
        return glyphs(frame.to.to_string());
    };
    let old = ShapedChars::new(text_system, family.clone(), size, weight, from);
    let new = ShapedChars::new(text_system, family, size, weight, &frame.to);
    let pairs = roll_pairs(from, &frame.to);
    // A glyph missing on one side enters or leaves at the place of the next
    // glyph that side does have, with no width.
    let edges = |side: &ShapedChars, pick: fn(&RollPair) -> Option<usize>| {
        let mut next = side.width;
        let mut edges = vec![(0.0, 0.0); pairs.len()];
        for (index, pair) in pairs.iter().enumerate().rev() {
            edges[index] = match pick(pair) {
                Some(at) => {
                    next = side.x[at];
                    (side.x[at], side.advance(at))
                }
                None => (next, 0.0),
            };
        }
        edges
    };
    let (old_edges, new_edges) = (edges(&old, |pair| pair.0), edges(&new, |pair| pair.1));
    let p = frame.progress;
    let mix = |a: f32, b: f32| a + (b - a) * p.min(1.0);
    // Glyph `offset` steps away from its resting place. Steps are a bit
    // taller than the line and glyphs are gone by half a step, so only one
    // digit reads at a time instead of two half-faded neighbors.
    let glyph = |ch: char, offset: f32| {
        glyphs(ch.to_string())
            .absolute()
            .left_0()
            .top(px(offset * line * 1.2))
            .opacity((1.0 - offset.abs() * 1.8).clamp(0.0, 1.0))
    };
    let travel = if frame.rising { 1.0 } else { -1.0 };
    let mut label = div()
        .relative()
        .flex_none()
        .h(px(line))
        .w(px(mix(old.width, new.width)));
    for (index, (before, after)) in pairs.iter().enumerate() {
        let (before, after) = (
            before.map(|at| old.chars[at]),
            after.map(|at| new.chars[at]),
        );
        let ((old_x, old_w), (new_x, new_w)) = (old_edges[index], new_edges[index]);
        let mut slot = div()
            .absolute()
            .top_0()
            .left(px(mix(old_x, new_x)))
            .w(px(mix(old_w, new_w)))
            .h(px(line));
        if before == after {
            if let Some(ch) = after {
                label = label.child(slot.child(glyphs(ch.to_string()).absolute().left_0()));
            }
            continue;
        }
        slot = slot.overflow_hidden();
        match (
            before.and_then(|ch| ch.to_digit(10)),
            after.and_then(|ch| ch.to_digit(10)),
        ) {
            (Some(start), Some(end)) => {
                // Spin the shorter way in the value's direction: 3 → 7
                // rising passes 4, 5, 6; 3 → 7 falling passes 2, 1, 0, 9, 8.
                let steps = if frame.rising {
                    (end + 10 - start) % 10
                } else {
                    (start + 10 - end) % 10
                } as f32;
                let position = start as f32 + travel * steps * p;
                let base = position.floor();
                for digit in [base, base + 1.0] {
                    let ch =
                        char::from_digit((digit as i32).rem_euclid(10) as u32, 10).unwrap_or('0');
                    slot = slot.child(glyph(ch, digit - position));
                }
            }
            _ => {
                if let Some(ch) = before {
                    slot = slot.child(glyph(ch, -travel * p));
                }
                if let Some(ch) = after {
                    slot = slot.child(glyph(ch, travel * (1.0 - p)));
                }
            }
        }
        label = label.child(slot);
    }
    label
}

/// Character indices of a glyph in the old and the new text.
type RollPair = (Option<usize>, Option<usize>);

/// A label shaped as one line: each character's x position and the width.
struct ShapedChars {
    chars: Vec<char>,
    x: Vec<f32>,
    width: f32,
}

impl ShapedChars {
    fn new(
        text_system: &gpui::WindowTextSystem,
        family: SharedString,
        size: f32,
        weight: FontWeight,
        value: &str,
    ) -> Self {
        let mut font = gpui::font(family);
        font.weight = weight;
        let run = gpui::TextRun {
            len: value.len(),
            font,
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let shaped = text_system.shape_line(value.to_owned().into(), px(size), &[run], None);
        Self {
            chars: value.chars().collect(),
            x: value
                .char_indices()
                .map(|(at, _)| f32::from(shaped.x_for_index(at)))
                .collect(),
            width: f32::from(shaped.width),
        }
    }

    fn advance(&self, at: usize) -> f32 {
        self.x.get(at + 1).copied().unwrap_or(self.width) - self.x[at]
    }
}

/// Pairs each character of `old` with the one it rolls into in `new`, as
/// character indices.
///
/// Texts that share their wording (`$996.00` / `$1,604.17`, `62.0% of cost
/// · 51.6M` / `16.5% of cost · 714.4M`) pair number by number, each aligned
/// on the right so decimals and thousands separators keep their columns.
/// Otherwise (`4.5B` / `714.4M`) the shared non-digit prefix and suffix stay
/// put and the rest aligns on the right.
pub(crate) fn roll_pairs(old: &str, new: &str) -> Vec<RollPair> {
    let numeric = |ch: char| ch.is_ascii_digit() || ch == '.' || ch == ',';
    let (a, b): (Vec<char>, Vec<char>) = (old.chars().collect(), new.chars().collect());
    let runs = |chars: &[char]| {
        let mut runs: Vec<(bool, std::ops::Range<usize>)> = Vec::new();
        for (index, ch) in chars.iter().enumerate() {
            match runs.last_mut() {
                Some((kind, range)) if *kind == numeric(*ch) => range.end = index + 1,
                _ => runs.push((numeric(*ch), index..index + 1)),
            }
        }
        runs
    };
    let (ra, rb) = (runs(&a), runs(&b));
    let same_wording = ra.len() == rb.len()
        && ra
            .iter()
            .zip(&rb)
            .all(|((x, xs), (y, ys))| x == y && (*x || a[xs.clone()] == b[ys.clone()]));
    let mut pairs = Vec::new();
    if same_wording {
        for ((_, xs), (_, ys)) in ra.into_iter().zip(rb) {
            right_aligned(xs, ys, &mut pairs);
        }
        return pairs;
    }
    let fixed = |x: &char, y: &char| x == y && !x.is_ascii_digit();
    let prefix = a.iter().zip(&b).take_while(|(x, y)| fixed(x, y)).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| fixed(x, y))
        .count();
    pairs.extend((0..prefix).map(|index| (Some(index), Some(index))));
    right_aligned(
        prefix..a.len() - suffix,
        prefix..b.len() - suffix,
        &mut pairs,
    );
    pairs.extend((0..suffix).map(|index| {
        (
            Some(a.len() - suffix + index),
            Some(b.len() - suffix + index),
        )
    }));
    pairs
}

fn right_aligned(
    old: std::ops::Range<usize>,
    new: std::ops::Range<usize>,
    pairs: &mut Vec<RollPair>,
) {
    let len = old.len().max(new.len());
    let at = |range: &std::ops::Range<usize>, index: usize| {
        (index + range.len())
            .checked_sub(len)
            .map(|index| range.start + index)
    };
    pairs.extend((0..len).map(|index| (at(&old, index), at(&new, index))));
}

pub(crate) fn caption(value: impl Into<SharedString>, color: Hsla) -> Div {
    text(value, 12.0, 16.0, color)
}

pub(crate) fn caption_strong(value: impl Into<SharedString>, color: Hsla) -> Div {
    caption(value, color).font_weight(FontWeight::SEMIBOLD)
}

pub(crate) fn body(value: impl Into<SharedString>, color: Hsla) -> Div {
    text(value, 14.0, 20.0, color)
}

pub(crate) fn body_strong(value: impl Into<SharedString>, color: Hsla) -> Div {
    body(value, color).font_weight(FontWeight::SEMIBOLD)
}

/// A single-line label that never wraps; long values end with an ellipsis.
pub(crate) fn nowrap(element: Div) -> Div {
    element
        .whitespace_nowrap()
        .overflow_hidden()
        .text_ellipsis()
}

pub(crate) fn icon(name: &str, size: f32, color: Hsla) -> gpui::Svg {
    svg()
        .path(icon_path(name))
        .size(px(size))
        .flex_none()
        .text_color(color)
}

/// Instance badge plate: up to three letters on the badge color, or on a
/// neutral plate that follows the theme for `Auto`.
pub(crate) fn badge_plate(badge: &crate::instances::Badge, height: f32, palette: &Palette) -> Div {
    let (background, foreground) = match badge.color.rgb().zip(badge.color.text_rgb()) {
        Some((rgb, text)) => (super::theme::rgb8(rgb), super::theme::rgb8(text)),
        None => (
            palette.text_secondary,
            palette.solid_background.opacity(1.0),
        ),
    };
    let size = (height * 0.68).max(7.0);
    div()
        .flex_none()
        .h(px(height))
        .min_w(px(height))
        .px(px((height * 0.22).max(2.0)))
        .rounded(px(height * 0.3))
        .bg(background)
        .border_1()
        .border_color(palette.solid_background.opacity(0.9))
        .flex()
        .items_center()
        .justify_center()
        .child(
            text(
                SharedString::from(badge.text.clone()),
                size,
                height,
                foreground,
            )
            .font_weight(FontWeight::BOLD)
            .whitespace_nowrap(),
        )
}

/// A driver icon with the instance badge in its lower-right corner. The
/// badge is drawn only while the driver has several enabled instances.
pub(crate) fn provider_mark(
    icon_name: &str,
    size: f32,
    color: Hsla,
    badge: Option<&crate::instances::Badge>,
    palette: &Palette,
) -> Div {
    let mut mark = div()
        .relative()
        .flex_none()
        .size(px(size))
        .child(icon(icon_name, size, color));
    if let Some(badge) = badge {
        let height = (size * 0.62).clamp(9.0, 14.0);
        mark = mark.child(
            div()
                .absolute()
                .right(px(-height * 0.45))
                .bottom(px(-height * 0.3))
                .child(badge_plate(badge, height, palette)),
        );
    }
    mark
}

/// Standard Fluent card surface. Without borders the outline is left out
/// entirely, so fills painted inside the card reach its edge.
pub(crate) fn card(palette: &Palette) -> Div {
    let card = div()
        .rounded(px(palette.card_radius))
        .bg(palette.card_background);
    if palette.borders {
        card.border_1().border_color(palette.card_stroke)
    } else {
        card
    }
}

/// Full-size hover tint drawn above a card's background and below content.
pub(crate) fn hover_layer(palette: &Palette, amount: f32, radius: f32) -> Option<AnyElement> {
    (amount > 0.001).then(|| {
        div()
            .absolute()
            .inset_0()
            .rounded(px(radius))
            .bg(palette.subtle_fill.opacity(amount))
            .into_any_element()
    })
}

/// Two-column row: a flexible leading cell and a trailing cell.
pub(crate) fn split_row(leading: impl IntoElement, trailing: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .gap(px(SPLIT_GAP))
        .child(div().flex_1().min_w_0().child(leading))
        .child(div().flex_none().child(trailing))
}

/// Gap between the two groups of a [`split_row`].
pub(crate) const SPLIT_GAP: f32 = 8.0;

/// Leading and trailing groups share one row when both fit; otherwise the
/// trailing group moves under the leading one, right-aligned, so translated
/// copy is never clipped.
pub(crate) fn adaptive_split(
    fits: bool,
    leading: impl IntoElement,
    trailing: impl IntoElement,
) -> Div {
    if fits {
        split_row(leading, trailing)
    } else {
        div()
            .flex()
            .flex_col()
            .w_full()
            .gap(px(4.0))
            .child(leading)
            .child(div().flex().flex_row().justify_end().child(trailing))
    }
}

/// Single-line text while it fits; wrapping text once it needs its own row.
pub(crate) fn fit_text(fits: bool, element: Div) -> Div {
    if fits { nowrap(element) } else { element }
}

/// Measures popup copy with the active font so cards can pick a layout
/// before painting instead of clipping long translations.
pub(crate) struct TextMetrics<'a> {
    text_system: &'a gpui::WindowTextSystem,
    family: SharedString,
}

impl<'a> TextMetrics<'a> {
    pub(crate) fn new(window: &'a gpui::Window, family: SharedString) -> Self {
        Self {
            text_system: window.text_system(),
            family,
        }
    }

    fn measure(&self, value: &str, size: f32, weight: FontWeight) -> f32 {
        measure_text(self.text_system, self.family.clone(), size, weight, value)
    }

    /// [`caption`] and [`card_metadata`].
    pub(crate) fn caption(&self, value: &str) -> f32 {
        self.measure(value, 12.0, FontWeight::NORMAL)
    }

    /// [`body`].
    pub(crate) fn body(&self, value: &str) -> f32 {
        self.measure(value, 14.0, FontWeight::NORMAL)
    }

    /// [`body_strong`].
    pub(crate) fn strong(&self, value: &str) -> f32 {
        self.measure(value, 14.0, FontWeight::SEMIBOLD)
    }

    /// [`status_row`].
    pub(crate) fn status_row(&self, label: &str, value: &str) -> f32 {
        self.body(label) + STATUS_GAP + self.body(value)
    }

    /// [`icon_status`].
    pub(crate) fn icon_status(&self, value: &str) -> f32 {
        ICON_STATUS_GLYPH + ICON_STATUS_GAP + self.body(value)
    }

    /// [`pace_status`].
    pub(crate) fn pace_status(&self, pace: crate::limits::PaceTip) -> f32 {
        self.caption(&pace_status_label(pace)) + PACE_GAP + PACE_GLYPH
    }

    /// Two groups separated by [`SPLIT_GAP`].
    pub(crate) fn fits_split(available: f32, leading: f32, trailing: f32) -> bool {
        leading + SPLIT_GAP + trailing <= available
    }
}

pub(crate) fn card_metadata(value: impl Into<SharedString>, palette: &Palette) -> Div {
    nowrap(caption(value, palette.text_tertiary))
}

const PACE_GLYPH: f32 = 12.0;
const PACE_GAP: f32 = 2.0;

/// "21% ↑" — deviation from an even burn with a colored direction arrow;
/// "On pace ✓" when usage tracks an even burn.
pub(crate) fn pace_status(pace: crate::limits::PaceTip, palette: &Palette) -> Div {
    use crate::limits::PaceTrend;
    let (glyph, color) = match pace.trend() {
        PaceTrend::OnPace => ("fluent-checkmark", palette.text_tertiary),
        PaceTrend::Deficit => ("fluent-arrow-up", palette.critical),
        PaceTrend::Reserve => ("fluent-arrow-down", palette.positive),
    };
    div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(PACE_GAP))
        .whitespace_nowrap()
        .child(caption(pace_status_label(pace), palette.text_tertiary))
        .child(icon(glyph, PACE_GLYPH, color))
}

fn pace_status_label(pace: crate::limits::PaceTip) -> String {
    match pace.trend() {
        crate::limits::PaceTrend::OnPace => crate::i18n::tr("on-pace").into(),
        _ => pace.delta_label(),
    }
}

/// "Resets in 4h 13m" — tertiary label, primary value.
const STATUS_GAP: f32 = 6.0;

/// "⟳ 4h 13m" — tertiary glyph in place of a text label, primary value.
const ICON_STATUS_GLYPH: f32 = 14.0;
const ICON_STATUS_GAP: f32 = 4.0;
const CAPTION_ICON_STATUS_GLYPH: f32 = 12.0;

/// Glyph for a quota's countdown to its next refill.
pub(crate) const RESET_ICON: &str = "fluent-arrow-clockwise-dashes";

pub(crate) fn icon_status(
    icon_name: &str,
    value: impl Into<SharedString>,
    palette: &Palette,
) -> Div {
    div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(ICON_STATUS_GAP))
        .whitespace_nowrap()
        .child(icon(icon_name, ICON_STATUS_GLYPH, palette.text_tertiary))
        .child(body(value, palette.text_primary))
}

/// [`icon_status`] at caption size, for single-row compact cards.
pub(crate) fn caption_icon_status(
    icon_name: &str,
    value: impl Into<SharedString>,
    palette: &Palette,
) -> Div {
    div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(ICON_STATUS_GAP))
        .whitespace_nowrap()
        .child(icon(
            icon_name,
            CAPTION_ICON_STATUS_GLYPH,
            palette.text_tertiary,
        ))
        .child(caption(value, palette.text_primary))
}

pub(crate) fn status_row(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    palette: &Palette,
) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(STATUS_GAP))
        .whitespace_nowrap()
        .child(body(label, palette.text_tertiary))
        .child(body(value, palette.text_primary))
}

pub(crate) fn rule(palette: &Palette) -> Div {
    div().h(px(1.0)).w_full().flex_none().bg(palette.divider)
}

/// Equal-width interior ticks, each pinned to the right end of its bucket.
fn interval_ticks(count: u32, palette: &Palette, compact: bool) -> Option<Div> {
    if count == 0 {
        return None;
    }
    let mut row = div().absolute().inset_0().flex().flex_row();
    for index in 0..=count {
        let mut cell = div().flex_1().h_full().flex().flex_row().justify_end();
        if index < count {
            let tick = if compact {
                div()
                    .w(px(INTERVAL_TICK_WIDTH))
                    .h(px(CARD_EDGE_MARKER_HEIGHT))
                    .rounded_t(px(INTERVAL_TICK_WIDTH / 2.0))
            } else {
                div()
                    .w(px(INTERVAL_TICK_WIDTH))
                    .h(px(INTERVAL_TICK_HEIGHT))
                    .rounded(px(INTERVAL_TICK_WIDTH / 2.0))
            };
            cell = if compact {
                cell.items_end()
            } else {
                cell.items_center()
            };
            cell = cell.child(tick.bg(palette.interval_tick));
        }
        row = row.child(cell);
    }
    Some(row)
}

/// Thin pill progress track with a rounded fill, interval ticks and an
/// optional pace marker. `value` and `pace` are percentages.
pub(crate) fn progress_track(
    value: f32,
    pace: Option<f32>,
    ticks: u32,
    fill: Hsla,
    palette: &Palette,
) -> Div {
    let value = value.clamp(0.0, 100.0);
    let mut track = div()
        .relative()
        .w_full()
        .h(px(PROGRESS_TRACK_HEIGHT))
        .flex_none()
        .child(
            div()
                .absolute()
                .inset_0()
                .rounded(px(PROGRESS_TRACK_HEIGHT / 2.0))
                .bg(fill.opacity(0.2)),
        );
    if value > 0.0 {
        track = track.child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(relative(value / 100.0))
                .min_w(px(PROGRESS_TRACK_HEIGHT))
                .rounded(px(PROGRESS_TRACK_HEIGHT / 2.0))
                .bg(fill),
        );
    }
    if let Some(ticks) = interval_ticks(ticks, palette, false) {
        track = track.child(ticks);
    }
    if let Some(pace) = pace {
        track = track.child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(relative(pace.clamp(0.0, 100.0) / 100.0))
                .w(px(2.0))
                .bg(palette.pace_marker),
        );
    }
    track
}

/// Full-card quota layer for compact cards: the unfilled card surface is the
/// track and an accent-tinted left segment is the fill.
pub(crate) fn compact_progress_layers(
    value: f32,
    pace: Option<f32>,
    ticks: u32,
    fill: Hsla,
    palette: &Palette,
) -> Vec<AnyElement> {
    let value = value.clamp(0.0, 100.0);
    let radius = palette.card_radius - 1.0;
    let mut layers = Vec::with_capacity(3);
    if value > 0.0 {
        // Overflow masks are rectangular in GPUI. Paint the full card's
        // rounded contour through a percentage mask so even a narrow fill
        // follows the card corners instead of shrinking its own radius.
        let segment = gpui::canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let mut filled_bounds = bounds;
                filled_bounds.size.width *= value / 100.0;
                window.with_content_mask(
                    Some(gpui::ContentMask {
                        bounds: filled_bounds,
                    }),
                    |window| {
                        let mut quad = gpui::fill(bounds, fill.opacity(0.2));
                        quad.corner_radii = gpui::Corners::all(px(radius));
                        window.paint_quad(quad);
                    },
                );
            },
        )
        .absolute()
        .inset_0();
        layers.push(segment.into_any_element());
    }
    if let Some(ticks) = interval_ticks(ticks, palette, true) {
        layers.push(ticks.into_any_element());
    }
    if let Some(pace) = pace {
        layers.push(
            div()
                .absolute()
                .top_0()
                .left(relative(pace.clamp(0.0, 100.0) / 100.0))
                .w(px(2.0))
                .h(px(CARD_EDGE_MARKER_HEIGHT))
                .rounded_b(px(1.0))
                .bg(palette.pace_marker)
                .into_any_element(),
        );
    }
    layers
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Severity {
    Error,
    Caution,
    Informational,
}

/// WinUI InfoBar look: tinted surface, severity glyph, bold title and body.
pub(crate) fn info_bar(
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    severity: Severity,
    palette: &Palette,
) -> Div {
    info_bar_with_action(title, message, severity, palette, None)
}

/// [`info_bar`] with an action button under the message, as WinUI places it
/// when the text wraps.
pub(crate) fn info_bar_with_action(
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    severity: Severity,
    palette: &Palette,
    action: Option<AnyElement>,
) -> Div {
    let (background, glyph_color, glyph) = match severity {
        Severity::Error => (
            palette.critical_background,
            palette.critical,
            "fluent-error-circle",
        ),
        Severity::Caution => (
            palette.caution_background,
            palette.caution,
            "fluent-warning",
        ),
        Severity::Informational => (
            palette.attention_background,
            palette.accent,
            "fluent-error-circle",
        ),
    };
    let message: SharedString = message.into();
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap(px(12.0))
        .px(px(15.0))
        .py(px(13.0))
        .rounded(px(palette.control_radius))
        .bg(background)
        .border_1()
        .border_color(palette.card_stroke)
        .child(div().pt(px(2.0)).child(icon(glyph, 16.0, glyph_color)))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(2.0))
                .child(body_strong(title, palette.text_primary))
                .children(
                    message
                        .lines()
                        .map(|line| body(SharedString::from(line.to_owned()), palette.text_primary))
                        .collect::<Vec<_>>(),
                )
                .children(action.map(|action| div().pt(px(8.0)).flex().child(action))),
        )
}

/// The compact error glyph used in headings and footer tabs.
pub(crate) fn error_badge(size: f32, palette: &Palette) -> gpui::Svg {
    icon("fluent-error-circle", size, palette.critical)
}

#[cfg(test)]
mod tests {
    use super::roll_pairs;

    fn changed(old: &str, new: &str) -> String {
        let (a, b): (Vec<char>, Vec<char>) = (old.chars().collect(), new.chars().collect());
        roll_pairs(old, new)
            .into_iter()
            .map(|(x, y)| {
                if x.map(|x| a[x]) == y.map(|y| b[y]) {
                    '='
                } else {
                    '^'
                }
            })
            .collect()
    }

    #[test]
    fn rolled_numbers_keep_their_wording_and_decimal_columns() {
        assert_eq!(changed("$996.00", "$1,604.17"), "=^^^^^=^^");
        assert_eq!(changed("7495 sessions", "336 sessions"), "^^^^=========");
        assert_eq!(
            changed("62.0% of cost · 51.6M", "16.5% of cost · 714.4M"),
            "^^=^============^^^=^="
        );
        // Different suffixes: the whole value aligns on the right.
        assert_eq!(changed("4.5B", "714.4M"), "^^==^^");
    }
}
