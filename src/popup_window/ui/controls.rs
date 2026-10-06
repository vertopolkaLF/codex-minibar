//! Interactive controls shared by several pages.

use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px, relative,
};

use super::{
    components, fx,
    root::{PopupRoot, eid},
    theme::HslaExt,
};

const SEGMENT_HEIGHT: f32 = 34.0;

/// Width of a fixed (non-stretched) segment, matching the WinUI control.
fn segment_width(label: &str) -> f32 {
    (label.chars().count() as f32 * 8.0 + 22.0).max(48.0)
}

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
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let count = labels.len().max(1);
        let selected = selected.min(count - 1);
        let on_select = Rc::new(on_select);
        let fixed_width = labels
            .iter()
            .map(|label| segment_width(label))
            .fold(0.0_f32, f32::max);
        // The thumb glides between cells as a fraction of the track.
        let thumb = self.fx.value(
            fx::key(("segment-thumb", key)),
            selected as f32 / count as f32,
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
            .bg(palette.control_fill)
            .flex_none();
        track = if stretch {
            track.w_full()
        } else {
            track.w(px(fixed_width * count as f32))
        };
        track = track.child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(relative(thumb))
                .w(relative(1.0 / count as f32))
                .rounded(px(8.0))
                .bg(palette.accent),
        );
        for (index, label) in labels.into_iter().enumerate() {
            let on = self.fx.toggle(
                fx::key(("segment-on", key, index)),
                index == selected,
                fx::FAST,
            );
            let hover_id = fx::key(("segment-hover", key, index));
            let hovered = self.hovered(hover_id) && index != selected;
            let hover =
                self.fx
                    .toggle(fx::key(("segment-hover-fx", hover_id)), hovered, fx::FASTER);
            let hide_divider = index == 0 || selected == index || selected + 1 == index;
            let divider = self.fx.toggle(
                fx::key(("segment-rule", key, index)),
                !hide_divider,
                fx::FAST,
            );
            let color = palette.text_primary.mix(palette.text_on_accent, on);
            let on_select = Rc::clone(&on_select);
            let mut cell = div()
                .id(eid(format!("segment-{key}-{index}")))
                .relative()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .px(px(10.0))
                .on_hover(self.hover_listener(hover_id, None, cx))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    on_select(this, index, cx);
                    cx.notify();
                }));
            // flex_none() leaves flex_basis unchanged in GPUI. Applying
            // flex_1() first would retain a zero basis and collapse fixed
            // cells to their padding while the thumb keeps its full width.
            cell = if stretch {
                cell.flex_1().min_w_0()
            } else {
                cell.flex_none().w(px(fixed_width))
            };
            if hover > 0.001 {
                cell = cell.child(
                    div()
                        .absolute()
                        .inset(px(2.0))
                        .rounded(px(6.0))
                        .bg(palette.subtle_fill.opacity(hover)),
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
                        .bg(palette.divider.alpha(divider)),
                );
            }
            cell = cell.child(
                components::nowrap(components::caption(label, color))
                    .font_weight(FontWeight::SEMIBOLD),
            );
            track = track.child(cell);
        }
        track.into_any_element()
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
