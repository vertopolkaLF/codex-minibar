//! Popup-owned tooltips.
//!
//! GPUI's built-in tooltips clamp to the window, but this window is a large
//! transparent host whose visible capsule is region-clipped. Tooltips are
//! therefore drawn inside the capsule and clamped to it, so they are never
//! cut off by the region or the monitor edge.

use std::time::{Duration, Instant};

use gpui::{
    AnyElement, Context, FontWeight, Hsla, IntoElement, ParentElement, Pixels, Point, SharedString,
    Styled, Window, div, px,
};

use super::{
    components::{self, caption, nowrap},
    fx,
    root::PopupRoot,
};

pub(super) const TOOLTIP_DELAY: Duration = Duration::from_millis(500);
const EDGE_INSET: f32 = 6.0;
const CURSOR_GAP: f32 = 12.0;
const TEXT_MAX_WIDTH: f32 = 300.0;

#[derive(Clone, Debug)]
pub(super) enum ActivityTip {
    Usage {
        title: String,
        total: String,
        cost: String,
        requests: String,
        series: Vec<(&'static str, String, Hsla)>,
    },
    Models {
        title: String,
        metric: String,
        rows: Vec<(String, String, Hsla)>,
        footer: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub(super) struct ChartTip {
    pub(super) title: String,
    /// `(icon, label, amount, icon color)` for each visible provider.
    pub(super) rows: Vec<(&'static str, String, String, Hsla)>,
    pub(super) total: String,
}

#[derive(Clone, Debug)]
pub(super) enum TipContent {
    Text(SharedString),
    Activity(ActivityTip),
    Chart(ChartTip),
}

pub(super) struct TipRequest {
    pub(super) owner: u64,
    pub(super) content: TipContent,
    /// Window coordinates of the pointer when the tip was requested.
    pub(super) anchor: Point<Pixels>,
    pub(super) since: Instant,
    pub(super) delayed: bool,
}

fn row(label: impl IntoElement, amount: String, color: Hsla) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(24.0))
        .w_full()
        .child(div().flex_1().min_w_0().child(label))
        .child(components::caption_strong(amount, color))
}

fn dot(color: Hsla) -> gpui::Div {
    div().size(px(6.0)).rounded(px(3.0)).flex_none().bg(color)
}

impl PopupRoot {
    /// Estimated tooltip size (DIP), used to clamp the bubble in the capsule.
    fn tip_size(&self, content: &TipContent, window: &Window) -> (f32, f32) {
        let measure = |text: &str, size: f32| -> f32 {
            let run = gpui::TextRun {
                len: text.len(),
                font: gpui::font(self.font_family.clone()),
                color: self.palette.text_primary,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            f32::from(
                window
                    .text_system()
                    .shape_line(SharedString::from(text.to_owned()), px(size), &[run], None)
                    .width,
            )
        };
        match content {
            TipContent::Text(text) => {
                let widest = text
                    .lines()
                    .map(|line| measure(line, 12.0))
                    .fold(0.0_f32, f32::max);
                let width = widest.min(TEXT_MAX_WIDTH);
                let wrapped: usize = text
                    .lines()
                    .map(|line| (measure(line, 12.0) / TEXT_MAX_WIDTH).ceil().max(1.0) as usize)
                    .sum();
                (width + 18.0, wrapped.max(1) as f32 * 16.0 + 12.0)
            }
            TipContent::Activity(ActivityTip::Usage { title, series, .. }) => (
                (measure(title, 14.0) + 32.0).max(230.0),
                16.0 + 20.0 + 10.0 + 44.0 + 7.0 + series.len() as f32 * 22.0 + 16.0,
            ),
            TipContent::Activity(ActivityTip::Models {
                title,
                rows,
                footer,
                ..
            }) => {
                let widest = rows
                    .iter()
                    .map(|(name, amount, _)| measure(name, 12.0) + measure(amount, 12.0))
                    .fold(0.0_f32, f32::max);
                (
                    (widest + 6.0 + 8.0 + 24.0 + 28.0)
                        .max(measure(title, 14.0) + 28.0)
                        .clamp(200.0, 330.0),
                    16.0 + 20.0
                        + 10.0
                        + 22.0
                        + rows.len().max(1) as f32 * 22.0
                        + if footer.is_some() { 16.0 } else { 0.0 },
                )
            }
            TipContent::Chart(chart) => {
                let widest = chart
                    .rows
                    .iter()
                    .map(|(_, label, amount, _)| measure(label, 12.0) + measure(amount, 12.0))
                    .fold(measure(&chart.total, 12.0) + 40.0, f32::max);
                (
                    (14.0 + 8.0 + widest + 24.0 + 28.0).max(measure(&chart.title, 14.0) + 28.0),
                    16.0 + 20.0 + 10.0 + (chart.rows.len() + 2) as f32 * 22.0,
                )
            }
        }
    }

    pub(super) fn render_tip(
        &mut self,
        capsule_width: f32,
        capsule_height: f32,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let tip = self.tip.as_ref()?;
        let ready = !tip.delayed || tip.since.elapsed() >= TOOLTIP_DELAY;
        let reveal = self
            .fx
            .toggle(fx::key(("tip", tip.owner, tip.since)), ready, fx::FAST);
        if !ready || reveal <= 0.001 {
            return None;
        }
        let content = tip.content.clone();
        let anchor = tip.anchor;
        let follows_cursor = !tip.delayed;
        let (width, height) = self.tip_size(&content, window);
        let cursor = if follows_cursor {
            window.mouse_position()
        } else {
            anchor
        };
        let local_x = f32::from(cursor.x) - f32::from(self.capsule_origin.x);
        let local_y = f32::from(cursor.y) - f32::from(self.capsule_origin.y);
        let (left, top) = if follows_cursor {
            // Charts: beside the cursor, flipping when the side is full.
            let left = if local_x + CURSOR_GAP + width <= capsule_width - EDGE_INSET {
                local_x + CURSOR_GAP
            } else {
                local_x - CURSOR_GAP - width
            };
            let top = if local_y + CURSOR_GAP + height <= capsule_height - EDGE_INSET {
                local_y + CURSOR_GAP
            } else {
                local_y - CURSOR_GAP - height
            };
            (left, top)
        } else {
            // Text: centered above the pointer, below when there is no room.
            let left = local_x - width / 2.0;
            let top = if local_y - CURSOR_GAP - height >= EDGE_INSET {
                local_y - CURSOR_GAP - height
            } else {
                local_y + CURSOR_GAP + 8.0
            };
            (left, top)
        };
        let left = left.clamp(
            EDGE_INSET,
            (capsule_width - EDGE_INSET - width).max(EDGE_INSET),
        );
        let top = top.clamp(
            EDGE_INSET,
            (capsule_height - EDGE_INSET - height).max(EDGE_INSET),
        );
        let palette = self.palette.clone();
        let bubble = div()
            .absolute()
            .left(self.capsule_origin.x + px(left))
            .top(self.capsule_origin.y + px(top + (1.0 - reveal) * 4.0))
            .rounded(px(if follows_cursor { 6.0 } else { 4.0 }))
            // Reveal by motion only: the surface must occlude underlying UI
            // from its first visible frame rather than fading its background.
            .bg(palette.tooltip_background.alpha(1.0))
            .border_1()
            .border_color(palette.card_stroke.opacity(4.0))
            .shadow_md();
        let element = match content {
            TipContent::Text(text) => bubble
                .max_w(px(TEXT_MAX_WIDTH + 18.0))
                .px(px(8.0))
                .py(px(5.0))
                .children(
                    text.lines()
                        .map(|line| {
                            caption(SharedString::from(line.to_owned()), palette.text_primary)
                        })
                        .collect::<Vec<_>>(),
                ),
            TipContent::Activity(ActivityTip::Usage {
                title,
                total,
                cost,
                requests,
                series,
            }) => {
                let mut rows = div().flex().flex_col().gap(px(6.0));
                for (label, amount, color) in series {
                    rows = rows.child(row(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(dot(color))
                            .child(caption(label, palette.text_secondary)),
                        amount,
                        palette.accent,
                    ));
                }
                bubble
                    .w(px(width))
                    .px(px(14.0))
                    .py(px(8.0))
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(components::body_strong(title, palette.text_primary))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_end()
                                    .gap(px(12.0))
                                    .child(
                                        div()
                                            .flex_1()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.0))
                                            .child(caption(
                                                crate::i18n::tr("tokens"),
                                                palette.text_secondary,
                                            ))
                                            .child(
                                                components::text(
                                                    total,
                                                    20.0,
                                                    26.0,
                                                    palette.text_primary,
                                                )
                                                .font_weight(FontWeight::SEMIBOLD),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .items_end()
                                            .gap(px(2.0))
                                            .child(caption(
                                                crate::i18n::tr("cost"),
                                                palette.text_secondary,
                                            ))
                                            .child(
                                                components::text(cost, 16.0, 22.0, palette.accent)
                                                    .font_weight(FontWeight::SEMIBOLD),
                                            ),
                                    ),
                            )
                            .child(components::rule(&palette))
                            .child(rows)
                            .child(caption(requests, palette.text_secondary)),
                    )
            }
            TipContent::Activity(ActivityTip::Models {
                title,
                metric,
                rows,
                footer,
            }) => {
                let mut list = div().flex().flex_col().gap(px(6.0));
                if rows.is_empty() {
                    list = list.child(caption(
                        crate::i18n::tr("no-model-data"),
                        palette.text_secondary,
                    ));
                }
                for (name, amount, color) in rows {
                    list = list.child(row(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(dot(color))
                            .child(nowrap(caption(name, palette.text_secondary))),
                        amount,
                        palette.accent,
                    ));
                }
                let mut body = div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(caption(metric, palette.text_secondary))
                    .child(list);
                if let Some(footer) = footer {
                    body = body.child(caption(footer, palette.text_secondary));
                }
                bubble
                    .w(px(width))
                    .px(px(14.0))
                    .py(px(8.0))
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(nowrap(components::body_strong(title, palette.text_primary)))
                    .child(body)
            }
            TipContent::Chart(chart) => {
                let mut list = div().flex().flex_col().gap(px(6.0));
                for (icon, label, amount, color) in chart.rows {
                    list = list.child(row(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(components::icon(icon, 14.0, color))
                            .child(caption(label, palette.text_secondary)),
                        amount,
                        palette.accent,
                    ));
                }
                list = list.child(components::rule(&palette)).child(row(
                    caption(crate::i18n::tr("total"), palette.text_secondary),
                    chart.total,
                    palette.accent,
                ));
                bubble
                    .w(px(width))
                    .px(px(14.0))
                    .py(px(8.0))
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(components::body_strong(chart.title, palette.text_primary))
                    .child(list)
            }
        };
        Some(element.into_any_element())
    }
}
