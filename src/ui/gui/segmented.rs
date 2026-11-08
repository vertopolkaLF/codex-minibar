//! Pill segmented control shared by the popup and the Settings window.

use gpui::{
    AnyElement, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, SharedString,
    Styled, div, px, relative,
};

use super::{color::HslaExt, fx, kit::eid};

pub(crate) const SEGMENT_HEIGHT: f32 = 34.0;

/// Colors of a [`segmented_track`]: the popup derives them from its palette,
/// Settings from its theme, so both surfaces draw the same control.
#[derive(Clone, Copy)]
pub(crate) struct SegmentStyle {
    pub(crate) track: Hsla,
    pub(crate) thumb: Hsla,
    pub(crate) text: Hsla,
    pub(crate) text_on_thumb: Hsla,
    pub(crate) hover: Hsla,
    pub(crate) divider: Hsla,
    pub(crate) height: f32,
}

pub(crate) struct Segment {
    pub(crate) label: SharedString,
    pub(crate) label_width: f32,
    /// Leading mark (an instance badge), with its width for fixed layouts.
    pub(crate) badge: Option<AnyElement>,
    pub(crate) badge_width: f32,
    /// Dimmed and unclickable; still wired so hover tips can explain why.
    pub(crate) disabled: bool,
}

impl Segment {
    pub(crate) fn text(label: impl Into<SharedString>, label_width: f32) -> Self {
        Self {
            label: label.into(),
            label_width,
            badge: None,
            badge_width: 0.0,
            disabled: false,
        }
    }
}

/// Pill segmented control with a sliding accent thumb, hover fills and
/// dividers that fade around the selection. `wire` attaches each cell's
/// hover/click listeners for the hosting view.
#[allow(clippy::too_many_arguments)]
pub(crate) fn segmented_track(
    fx: &mut fx::Fx,
    key: u64,
    segments: Vec<Segment>,
    selected: usize,
    stretch: bool,
    style: SegmentStyle,
    hovered: &dyn Fn(usize) -> bool,
    wire: &mut dyn FnMut(usize, gpui::Stateful<gpui::Div>) -> gpui::Stateful<gpui::Div>,
) -> AnyElement {
    let selected = selected.min(segments.len().saturating_sub(1));
    // The selected cell takes no clicks; `segmented_track_optional` wires all.
    let mut wire_unselected = |index, cell| {
        if index == selected {
            cell
        } else {
            wire(index, cell)
        }
    };
    segmented_track_optional(
        fx,
        key,
        segments,
        Some(selected),
        stretch,
        style,
        hovered,
        &mut wire_unselected,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn segmented_track_optional(
    fx: &mut fx::Fx,
    key: u64,
    segments: Vec<Segment>,
    selected: Option<usize>,
    stretch: bool,
    style: SegmentStyle,
    hovered: &dyn Fn(usize) -> bool,
    wire: &mut dyn FnMut(usize, gpui::Stateful<gpui::Div>) -> gpui::Stateful<gpui::Div>,
) -> AnyElement {
    let count = segments.len().max(1);
    let selected = selected.map(|index| index.min(count - 1));
    let widths = segments
        .iter()
        .enumerate()
        .map(|(index, segment)| {
            let width = segment.label_width
                + 20.0
                + segment.badge_width
                + if segment.badge.is_some() { 5.0 } else { 0.0 };
            fx.value(fx::key(("segment-width", key, index)), width, fx::FAST)
        })
        .collect::<Vec<_>>();
    let (total_width, target_left, target_width) =
        segment_geometry(&widths, selected.unwrap_or(0), stretch);
    // Fixed segments have independent text-sized widths; stretch stays equal.
    let thumb = fx.value(fx::key(("segment-thumb", key)), target_left, fx::FAST);
    let thumb_width = fx.value(
        fx::key(("segment-thumb-width", key)),
        target_width,
        fx::FAST,
    );
    let thumb_opacity = fx.toggle(
        fx::key(("segment-thumb-visible", key)),
        selected.is_some(),
        fx::FAST,
    );
    let mut track = div()
        .id(eid(format!("segmented-{key}")))
        .relative()
        .flex()
        .flex_row()
        .h(px(style.height))
        .rounded(px(8.0))
        .overflow_hidden()
        .bg(style.track)
        .flex_none();
    track = if stretch {
        track.w_full()
    } else {
        track.w(px(total_width))
    };
    track = track.child(
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(relative(thumb))
            .w(relative(thumb_width))
            .rounded(px(8.0))
            .opacity(thumb_opacity)
            .bg(style.thumb),
    );
    for (index, segment) in segments.into_iter().enumerate() {
        let on = fx.toggle(
            fx::key(("segment-on", key, index)),
            Some(index) == selected,
            fx::FAST,
        );
        let hover = fx.toggle(
            fx::key(("segment-hover-fx", key, index)),
            hovered(index) && Some(index) != selected && !segment.disabled,
            fx::FASTER,
        );
        let hide_divider = index == 0
            || selected == Some(index)
            || selected.is_some_and(|selected| selected + 1 == index);
        let divider = fx.toggle(
            fx::key(("segment-rule", key, index)),
            !hide_divider,
            fx::FAST,
        );
        let mut color = style.text.mix(style.text_on_thumb, on);
        if segment.disabled {
            color = color.opacity(0.5);
        }
        let mut cell = div()
            .id(eid(format!("segment-{key}-{index}")))
            .relative()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .px(px(10.0));
        cell = if segment.disabled || Some(index) == selected {
            wire(index, cell)
        } else {
            wire(index, cell.cursor_pointer())
        };
        // flex_none() leaves flex_basis unchanged in GPUI. Applying
        // flex_1() first would retain a zero basis and collapse fixed
        // cells to their padding while the thumb keeps its full width.
        cell = if stretch {
            cell.flex_1().min_w_0()
        } else {
            cell.flex_none().w(px(widths[index]))
        };
        if hover > 0.001 {
            cell = cell.child(
                div()
                    .absolute()
                    .inset(px(2.0))
                    .rounded(px(6.0))
                    .bg(style.hover.opacity(hover)),
            );
        }
        if divider > 0.001 {
            cell = cell.child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(6.0))
                    .bottom(px(6.0))
                    .w(px(1.0))
                    .bg(style.divider.alpha(divider)),
            );
        }
        let mut content = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(5.0))
            .min_w_0();
        if let Some(badge) = segment.badge {
            content = content.child(badge);
        }
        content = content.child(
            div()
                .text_size(px(12.0))
                .line_height(px(16.0))
                .text_color(color)
                .font_weight(FontWeight::SEMIBOLD)
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .child(segment.label),
        );
        cell = cell.child(content);
        track = track.child(cell);
    }
    track.into_any_element()
}

fn segment_geometry(widths: &[f32], selected: usize, stretch: bool) -> (f32, f32, f32) {
    let total = widths.iter().sum::<f32>();
    if widths.is_empty() {
        return (0.0, 0.0, 1.0);
    }
    let selected = selected.min(widths.len() - 1);
    if stretch {
        let count = widths.len() as f32;
        (total, selected as f32 / count, 1.0 / count)
    } else {
        let denominator = total.max(1.0);
        (
            total,
            widths[..selected].iter().sum::<f32>() / denominator,
            widths[selected] / denominator,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::segment_geometry;

    #[test]
    fn thumb_matches_individual_text_sized_segments() {
        assert_eq!(segment_geometry(&[60.0, 90.0], 0, false), (150.0, 0.0, 0.4));
        assert_eq!(segment_geometry(&[60.0, 90.0], 1, false), (150.0, 0.4, 0.6));
        assert_eq!(segment_geometry(&[60.0, 90.0], 1, true), (150.0, 0.5, 0.5));
    }
}
