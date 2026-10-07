//! Interactive controls shared by several pages (and, for the segmented
//! control, by the Settings window).

use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, div, px, relative,
};

use super::{
    components, fx,
    root::{PopupRoot, eid},
    theme::HslaExt,
};

const SEGMENT_HEIGHT: f32 = 34.0;

impl PopupRoot {
    /// Pill segmented control with a sliding accent thumb.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn segmented_control(
        &mut self,
        key: u64,
        labels: Vec<SharedString>,
        selected: usize,
        stretch: bool,
        on_select: impl Fn(&mut Self, usize, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let segments = labels.into_iter().map(|label| (label, None)).collect();
        self.segmented_control_badged(key, segments, selected, stretch, on_select, window, cx)
    }

    /// [`Self::segmented_control`] whose segments may lead with an instance
    /// badge.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn segmented_control_badged(
        &mut self,
        key: u64,
        segments: Vec<(SharedString, Option<crate::instances::Badge>)>,
        selected: usize,
        stretch: bool,
        on_select: impl Fn(&mut Self, usize, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let segments = segments
            .into_iter()
            .map(|(label, badge)| Segment {
                label_width: components::measure_text(
                    window.text_system(),
                    self.palette.font_family.clone(),
                    12.0,
                    FontWeight::SEMIBOLD,
                    &label,
                ),
                label,
                badge_width: badge
                    .as_ref()
                    .map_or(0.0, |badge| badge.text.chars().count() as f32 * 7.0 + 12.0),
                badge: badge.map(|badge| {
                    components::badge_plate(&badge, 14.0, &palette).into_any_element()
                }),
            })
            .collect();
        let style = SegmentStyle {
            track: palette.control_fill,
            thumb: palette.accent,
            text: palette.text_primary,
            text_on_thumb: palette.text_on_accent,
            hover: palette.subtle_fill,
            divider: palette.divider,
        };
        let on_select = Rc::new(on_select);
        let hover_ids = (0..64)
            .map(|index| fx::key(("segment-hover", key, index)))
            .collect::<Vec<_>>();
        let hovered = hover_ids
            .iter()
            .map(|id| self.hovered(*id))
            .collect::<Vec<_>>();
        // The listeners borrow `self`; the tweens are swapped out meanwhile.
        let mut tweens = std::mem::take(&mut self.fx);
        let mut wire = |index: usize, cell: gpui::Stateful<gpui::Div>| {
            let on_select = Rc::clone(&on_select);
            cell.on_hover(self.hover_listener(hover_ids[index.min(63)], None, cx))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    on_select(this, index, cx);
                    cx.notify();
                }))
        };
        let control = segmented_track(
            &mut tweens,
            key,
            segments,
            selected,
            stretch,
            style,
            &|index| hovered.get(index).copied().unwrap_or(false),
            &mut wire,
        );
        self.fx = tweens;
        control
    }

    /// Text tabs (Today / Yesterday / 30 days) with crossfaded colors.
    pub(super) fn text_tabs(
        &mut self,
        key: u64,
        labels: &[(&'static str, bool)],
        on_select: impl Fn(&mut Self, usize, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let on_select = Rc::new(on_select);
        let mut row = div().flex().flex_row().items_center().gap(px(12.0));
        for (index, (label, selected)) in labels.iter().copied().enumerate() {
            let hover_id = fx::key(("text-tab", key, index));
            let hovered = self.hovered(hover_id);
            let hover = self.fx.toggle(
                fx::key(("text-tab-hover", hover_id)),
                hovered,
                fx::TEXT_FADE,
            );
            let on = self.fx.toggle(
                fx::key(("text-tab-on", key, index)),
                selected,
                fx::TEXT_FADE,
            );
            let color = palette
                .text_tertiary
                .mix(palette.text_secondary, hover)
                .mix(palette.accent, on);
            let on_select = Rc::clone(&on_select);
            row = row.child(
                div()
                    .id(eid(format!("text-tab-{key}-{index}")))
                    .on_hover(self.hover_listener(hover_id, None, cx))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        on_select(this, index, cx);
                        cx.notify();
                    }))
                    .child(components::nowrap(components::body_strong(label, color))),
            );
        }
        row.into_any_element()
    }

    /// Small 28 DIP reorder grip shown in Home headings.
    pub(super) fn drag_grip(&mut self, id: u64, active: bool) -> gpui::Div {
        let palette = self.palette.clone();
        let hovered = self.hovered(id);
        let fill = self
            .fx
            .toggle(fx::key(("grip", id)), hovered || active, fx::FASTER);
        let mut grip = div()
            .relative()
            .size(px(28.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.0));
        if let Some(layer) = components::hover_layer(&palette, fill, 4.0) {
            grip = grip.child(layer);
        }
        grip.child(components::icon(
            "fluent-drag",
            14.0,
            palette.chrome_icon.mix(palette.chrome_icon_hover, fill),
        ))
    }
}

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
}

pub(crate) struct Segment {
    pub(crate) label: SharedString,
    pub(crate) label_width: f32,
    /// Leading mark (an instance badge), with its width for fixed layouts.
    pub(crate) badge: Option<AnyElement>,
    pub(crate) badge_width: f32,
}

impl Segment {
    pub(crate) fn text(label: impl Into<SharedString>, label_width: f32) -> Self {
        Self {
            label: label.into(),
            label_width,
            badge: None,
            badge_width: 0.0,
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
    let count = segments.len().max(1);
    let selected = selected.min(count - 1);
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
    let (total_width, target_left, target_width) = segment_geometry(&widths, selected, stretch);
    // Fixed segments have independent text-sized widths; stretch stays equal.
    let thumb = fx.value(fx::key(("segment-thumb", key)), target_left, fx::FAST);
    let thumb_width = fx.value(
        fx::key(("segment-thumb-width", key)),
        target_width,
        fx::FAST,
    );
    let mut track = div()
        .id(eid(format!("segmented-{key}")))
        .relative()
        .flex()
        .flex_row()
        .h(px(SEGMENT_HEIGHT))
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
            .bg(style.thumb),
    );
    for (index, segment) in segments.into_iter().enumerate() {
        let on = fx.toggle(
            fx::key(("segment-on", key, index)),
            index == selected,
            fx::FAST,
        );
        let hover = fx.toggle(
            fx::key(("segment-hover-fx", key, index)),
            hovered(index) && index != selected,
            fx::FASTER,
        );
        let hide_divider = index == 0 || selected == index || selected + 1 == index;
        let divider = fx.toggle(
            fx::key(("segment-rule", key, index)),
            !hide_divider,
            fx::FAST,
        );
        let color = style.text.mix(style.text_on_thumb, on);
        let mut cell = div()
            .id(eid(format!("segment-{key}-{index}")))
            .relative()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .px(px(10.0));
        if index != selected {
            cell = wire(index, cell.cursor_pointer());
        }
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
            components::nowrap(components::caption(segment.label, color))
                .font_weight(FontWeight::SEMIBOLD),
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
