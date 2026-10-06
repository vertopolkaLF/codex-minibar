//! Stateless building blocks shared by every popup page.
//!
//! Typography mirrors the WinUI type ramp the popup used before (Caption
//! 12/16, Body 14/20, Body Strong 14/20 semibold), so card proportions stay
//! the same after the GPUI rewrite.

use gpui::{
    AnyElement, Div, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, div, px,
    relative, svg,
};

use super::{assets::icon_path, theme::Palette};

pub(crate) const CARD_RADIUS: f32 = crate::popup::CARD_CORNER_RADIUS_DIP as f32;
pub(crate) const PROGRESS_TRACK_HEIGHT: f32 = 6.0;
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

/// Standard Fluent card surface.
pub(crate) fn card(palette: &Palette) -> Div {
    div()
        .rounded(px(CARD_RADIUS))
        .bg(palette.card_background)
        .border_1()
        .border_color(palette.card_stroke)
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
        .gap(px(8.0))
        .child(div().flex_1().min_w_0().child(leading))
        .child(div().flex_none().child(trailing))
}

pub(crate) fn card_metadata(value: impl Into<SharedString>, palette: &Palette) -> Div {
    nowrap(caption(value, palette.text_tertiary))
}

/// "Resets in 4h 13m" — tertiary label, primary value.
pub(crate) fn status_row(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    palette: &Palette,
) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
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
    let mut layers = Vec::with_capacity(3);
    if value > 0.0 {
        let mut segment = div()
            .absolute()
            .left_0()
            .top_0()
            .bottom_0()
            .w(relative(value / 100.0))
            .bg(fill.opacity(0.2));
        segment = if value >= 99.9 {
            segment.rounded(px(CARD_RADIUS - 1.0))
        } else {
            segment.rounded_l(px(CARD_RADIUS - 1.0))
        };
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
    Informational,
}

/// WinUI InfoBar look: tinted surface, severity glyph, bold title and body.
pub(crate) fn info_bar(
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    severity: Severity,
    palette: &Palette,
) -> Div {
    let (background, glyph_color) = match severity {
        Severity::Error => (palette.critical_background, palette.critical),
        Severity::Informational => (palette.attention_background, palette.accent),
    };
    let message: SharedString = message.into();
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap(px(12.0))
        .px(px(15.0))
        .py(px(13.0))
        .rounded(px(4.0))
        .bg(background)
        .border_1()
        .border_color(palette.card_stroke)
        .child(
            div()
                .pt(px(2.0))
                .child(icon("fluent-error-circle", 16.0, glyph_color)),
        )
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
                ),
        )
}

/// The compact error glyph used in headings and footer tabs.
pub(crate) fn error_badge(size: f32, palette: &Palette) -> gpui::Svg {
    icon("fluent-error-circle", size, palette.critical)
}
