//! Interactive controls shared by several pages.

use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};

use super::{components, root::PopupRoot};
use crate::ui::gui::{
    color::HslaExt,
    fx,
    segmented::{SEGMENT_HEIGHT, Segment, SegmentStyle, segmented_track_optional},
};

const COMPACT_SEGMENT_HEIGHT: f32 = 28.0;

impl PopupRoot {
    /// One row of the profile grid. Rows without the selected account have
    /// no thumb; clicking a cell still selects its original stable member ID.
    pub(super) fn profile_row_control(
        &mut self,
        key: u64,
        members: Vec<crate::providers::instances::ProviderId>,
        selected: Option<usize>,
        driver: crate::providers::registry::ProviderKind,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let labels = members
            .iter()
            .map(|member| SharedString::from(member.display_name()))
            .collect::<Vec<_>>();
        let tips = labels.iter().cloned().map(Some).collect();
        let segments = members
            .iter()
            .zip(labels)
            .map(|(member, label)| {
                let badge = member.badge();
                Segment {
                    label_width: crate::ui::gui::measure_text(
                        window.text_system(),
                        palette.font_family.clone(),
                        12.0,
                        FontWeight::SEMIBOLD,
                        &label,
                    ),
                    label,
                    badge_width: badge
                        .as_ref()
                        .map_or(0.0, |badge| badge.text.chars().count() as f32 * 7.0 + 12.0),
                    badge: badge.map(|badge| {
                        crate::ui::gui::mark::badge_plate(&badge, 14.0, palette.plate())
                            .into_any_element()
                    }),
                    disabled: false,
                }
            })
            .collect();
        let style = SegmentStyle {
            track: palette.control_fill,
            thumb: palette.accent,
            text: palette.text_primary,
            text_on_thumb: palette.text_on_accent,
            hover: palette.subtle_fill,
            divider: palette.divider,
            height: SEGMENT_HEIGHT,
        };
        self.segmented_control_styled(
            key,
            segments,
            tips,
            selected,
            true,
            style,
            move |this, index, cx| {
                if let Some(member) = members.get(index).copied() {
                    this.select_group_member(driver, member, cx);
                }
            },
            cx,
        )
    }

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

    /// [`Self::segmented_control`] at the quiet control's 28 DIP height, for
    /// headings where the full-size pill would dominate.
    pub(super) fn segmented_control_compact(
        &mut self,
        key: u64,
        labels: Vec<SharedString>,
        selected: usize,
        on_select: impl Fn(&mut Self, usize, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let segments = labels.into_iter().map(|label| (label, None)).collect();
        self.segmented_control_accent(
            key,
            segments,
            selected,
            false,
            COMPACT_SEGMENT_HEIGHT,
            on_select,
            window,
            cx,
        )
    }

    /// [`Self::segmented_control`] whose segments may lead with an instance
    /// badge.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn segmented_control_badged(
        &mut self,
        key: u64,
        segments: Vec<(SharedString, Option<crate::providers::instances::Badge>)>,
        selected: usize,
        stretch: bool,
        on_select: impl Fn(&mut Self, usize, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.segmented_control_accent(
            key,
            segments,
            selected,
            stretch,
            SEGMENT_HEIGHT,
            on_select,
            window,
            cx,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn segmented_control_accent(
        &mut self,
        key: u64,
        segments: Vec<(SharedString, Option<crate::providers::instances::Badge>)>,
        selected: usize,
        stretch: bool,
        height: f32,
        on_select: impl Fn(&mut Self, usize, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let segments = segments
            .into_iter()
            .map(|(label, badge)| Segment {
                label_width: crate::ui::gui::measure_text(
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
                    crate::ui::gui::mark::badge_plate(&badge, 14.0, palette.plate())
                        .into_any_element()
                }),
                disabled: false,
            })
            .collect();
        let style = SegmentStyle {
            track: palette.control_fill,
            thumb: palette.accent,
            text: palette.text_primary,
            text_on_thumb: palette.text_on_accent,
            hover: palette.subtle_fill,
            divider: palette.divider,
            height,
        };
        self.segmented_control_styled(
            key,
            segments,
            Vec::new(),
            Some(selected),
            stretch,
            style,
            on_select,
            cx,
        )
    }

    /// Compact neutral variant for secondary choices (display mode rather
    /// than data scope), so it never competes with an accent control.
    pub(super) fn segmented_control_quiet(
        &mut self,
        key: u64,
        labels: Vec<SharedString>,
        selected: usize,
        on_select: impl Fn(&mut Self, usize, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let segments = labels
            .into_iter()
            .map(|label| QuietSegment {
                label,
                tip: None,
                disabled: false,
            })
            .collect();
        self.segmented_control_quiet_ext(key, segments, selected, on_select, window, cx)
    }

    /// [`Self::segmented_control_quiet`] stretched to fill its container,
    /// with the segments sharing the width evenly.
    pub(super) fn segmented_control_quiet_wide(
        &mut self,
        key: u64,
        labels: Vec<SharedString>,
        selected: usize,
        on_select: impl Fn(&mut Self, usize, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let segments = labels
            .into_iter()
            .map(|label| QuietSegment {
                label,
                tip: None,
                disabled: false,
            })
            .collect();
        self.quiet_segments(key, segments, selected, true, on_select, window, cx)
    }

    /// [`Self::segmented_control_quiet`] whose segments carry hover tips and
    /// may be disabled (still hoverable so their tip explains why).
    pub(super) fn segmented_control_quiet_ext(
        &mut self,
        key: u64,
        segments: Vec<QuietSegment>,
        selected: usize,
        on_select: impl Fn(&mut Self, usize, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.quiet_segments(key, segments, selected, false, on_select, window, cx)
    }

    #[allow(clippy::too_many_arguments)]
    fn quiet_segments(
        &mut self,
        key: u64,
        segments: Vec<QuietSegment>,
        selected: usize,
        stretch: bool,
        on_select: impl Fn(&mut Self, usize, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let tips = segments.iter().map(|segment| segment.tip.clone()).collect();
        let segments = segments
            .into_iter()
            .map(
                |QuietSegment {
                     label, disabled, ..
                 }| {
                    let width = crate::ui::gui::measure_text(
                        window.text_system(),
                        palette.font_family.clone(),
                        12.0,
                        FontWeight::SEMIBOLD,
                        &label,
                    );
                    Segment {
                        disabled,
                        ..Segment::text(label, width)
                    }
                },
            )
            .collect();
        let style = SegmentStyle {
            track: palette.control_fill,
            thumb: palette.text_primary.opacity(0.14),
            text: palette.text_tertiary,
            text_on_thumb: palette.text_primary,
            hover: palette.subtle_fill,
            divider: palette.divider.opacity(0.0),
            height: COMPACT_SEGMENT_HEIGHT,
        };
        self.segmented_control_styled(
            key,
            segments,
            tips,
            Some(selected),
            stretch,
            style,
            on_select,
            cx,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn segmented_control_styled(
        &mut self,
        key: u64,
        segments: Vec<Segment>,
        tips: Vec<Option<SharedString>>,
        selected: Option<usize>,
        stretch: bool,
        style: SegmentStyle,
        on_select: impl Fn(&mut Self, usize, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = selected.map(|index| index.min(segments.len().saturating_sub(1)));
        let on_select = Rc::new(on_select);
        let disabled = segments
            .iter()
            .map(|segment| segment.disabled)
            .collect::<Vec<_>>();
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
            let tip = tips.get(index).cloned().flatten();
            let cell = cell.on_hover(self.hover_listener(hover_ids[index.min(63)], tip, cx));
            if disabled.get(index).copied().unwrap_or(false) || selected == Some(index) {
                return cell;
            }
            let on_select = Rc::clone(&on_select);
            cell.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                on_select(this, index, cx);
                cx.notify();
            }))
        };
        let control = segmented_track_optional(
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
            .rounded(px(palette.control_radius));
        if let Some(layer) = components::hover_layer(&palette, fill, 4.0) {
            grip = grip.child(layer);
        }
        grip.child(crate::ui::gui::mark::icon(
            "fluent-drag",
            14.0,
            palette.chrome_icon.mix(palette.chrome_icon_hover, fill),
        ))
    }
}

/// One option of [`PopupRoot::segmented_control_quiet_ext`].
pub(super) struct QuietSegment {
    pub(super) label: SharedString,
    pub(super) tip: Option<SharedString>,
    pub(super) disabled: bool,
}
