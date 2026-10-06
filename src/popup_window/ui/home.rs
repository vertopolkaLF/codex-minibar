//! Home page: reorderable provider widgets and the Usage Stats summary.

use std::{collections::HashMap, f32::consts::PI, rc::Rc};

use gpui::{
    AnyElement, AppContext, Bounds, ClickEvent, Context, DragMoveEvent, Hsla, InteractiveElement,
    IntoElement, ParentElement, PathBuilder, Pixels, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, canvas, div, point, px,
};

use super::{
    components::{self, CARD_RADIUS, caption, card, nowrap},
    fx,
    root::{PopupRoot, SnapshotSlot, eid},
    theme::{HslaExt, Palette},
};
use crate::popup_window::{model::*, *};
use crate::usage_overview::OverviewSnapshot;

const SECTION_GAP: f32 = 6.0;
const COLUMN_GAP: f32 = 12.0;
const HEADING_TOP: f32 = 8.0;

/// Payload carried while a Home widget is dragged.
#[derive(Clone)]
pub(super) struct WidgetDrag {
    widget: HomeWidgetId,
    label: SharedString,
    palette: Palette,
}

impl Render for WidgetDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .px(px(12.0))
            .py(px(6.0))
            .rounded(px(6.0))
            .bg(self.palette.solid_background.opacity(0.94))
            .border_1()
            .border_color(self.palette.accent.opacity(0.6))
            .shadow_md()
            .font_family(self.palette.font_family.clone())
            .child(components::icon(
                "fluent-drag",
                14.0,
                self.palette.chrome_icon,
            ))
            .child(components::body_strong(
                self.label.clone(),
                self.palette.text_primary,
            ))
    }
}

impl PopupRoot {
    pub(super) fn widget_drag_handle(
        &mut self,
        widget: HomeWidgetId,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hover_id = fx::key(("drag-handle", widget.id()));
        let active = self.widget_drag.as_ref() == Some(&widget);
        let grip = self.drag_grip(hover_id, active);
        let drag = WidgetDrag {
            widget: widget.clone(),
            label: home_widget_label(&self.ui, &widget).into(),
            palette: self.palette.clone(),
        };
        let root = cx.entity();
        div()
            .id(eid(format!("drag-handle-{}", widget.id())))
            .on_hover(self.hover_listener(hover_id, Some("Drag to reorder".into()), cx))
            .on_drag(drag, move |drag, _, _, cx| {
                let widget = drag.widget.clone();
                root.update(cx, |root, cx| {
                    root.widget_drag = Some(widget);
                    root.tip = None;
                    cx.notify();
                });
                cx.new(|_| drag.clone())
            })
            .child(grip)
            .into_any_element()
    }

    pub(super) fn render_home(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let ui = Rc::clone(&self.ui);
        let limits = Rc::clone(&self.limits);
        let forced_resets = Rc::clone(&self.forced_resets);
        let show_tabs = self.show_provider_tabs();
        let show_spend = show_total_spend(&ui) && show_tabs;
        let widgets = visible_home_widgets(&ui, show_spend);
        let two_columns = self.two_columns();
        let right_column = home_widget_right_column(&ui, show_spend);
        let can_reorder = widgets.len() > 1 || two_columns;
        let mut columns: [Vec<AnyElement>; 2] = [Vec::new(), Vec::new()];
        let mut placed = [0usize; 2];
        for widget in widgets {
            let column = if two_columns {
                usize::from(right_column.contains(&widget))
            } else {
                0
            };
            let is_first = placed[column] == 0;
            let section = if widget.is_total_spend() {
                Some(self.render_total_spend(is_first, can_reorder, window, cx))
            } else {
                model::home_widget_provider(&ui, &widget).map(|provider| {
                    let options = CardOptions {
                        popup_visibility: &ui.popup_visibility,
                        surface: PopupSurface::HomeTab,
                        show_provider_tabs: show_tabs,
                        include_usage_stats: ui.usage_stats_provider_enabled(provider),
                        show_account_name: ui.show_account_name,
                        drag_handle: can_reorder,
                        openrouter_actions: provider.kind() == ProviderKind::OpenRouter,
                        provider_error: ui.provider_error(provider),
                        now: Utc::now(),
                    };
                    let cards = provider_cards(
                        provider,
                        is_first,
                        true,
                        limits.get(provider),
                        &forced_resets,
                        &options,
                    );
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(SECTION_GAP))
                        .children(self.render_cards(&cards, PopupSurface::HomeTab, window, cx))
                        .into_any_element()
                })
            };
            let Some(section) = section else {
                continue;
            };
            placed[column] += 1;
            let section = if can_reorder {
                self.widget_drop_target(widget, section, two_columns.then_some(column), cx)
            } else {
                section
            };
            columns[column].push(section);
        }

        if !two_columns {
            let [single, _] = columns;
            return vec![self.measured_column(None, single).into_any_element()];
        }
        let [left, right] = columns;
        let left = self.home_column(0, left, cx);
        let right = self.home_column(1, right, cx);
        vec![
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap(px(COLUMN_GAP))
                .w_full()
                .child(div().flex_1().min_w_0().child(left))
                .child(div().flex_1().min_w_0().child(right))
                .into_any_element(),
        ]
    }

    /// A column's sections, recording the column origin for reorder motion.
    fn measured_column(&mut self, column: Option<usize>, sections: Vec<AnyElement>) -> gpui::Div {
        let bounds = Rc::clone(&self.widget_bounds);
        let slot = column.unwrap_or(0);
        div()
            .relative()
            .flex()
            .flex_col()
            .gap(px(SECTION_GAP))
            .w_full()
            .children(sections)
            .child(
                canvas(
                    move |area, _, _| {
                        bounds.borrow_mut().columns[slot] = Some(area);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
    }

    /// A Home column stays a drop target when empty and below its last block.
    fn home_column(
        &mut self,
        column: usize,
        mut sections: Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let dragging = self.widget_drag.is_some();
        if dragging || sections.is_empty() {
            let palette = self.palette.clone();
            let over = self.widget_drop.as_ref()
                == self
                    .widget_drag
                    .as_ref()
                    .map(|widget| (widget.clone(), Some(column)))
                    .as_ref();
            let reveal = self
                .fx
                .toggle(fx::key(("drop-zone", column)), dragging, fx::FAST);
            let highlight = self
                .fx
                .toggle(fx::key(("drop-zone-over", column)), over, fx::FASTER);
            sections.push(
                div()
                    .id(eid(format!("column-drop-{column}")))
                    .h(px(if sections.is_empty() { 80.0 } else { 32.0 }))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(palette.accent.alpha(reveal * (0.55 + 0.45 * highlight)))
                    .bg(palette.accent.alpha(0.08 * highlight))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(caption(
                        if dragging { "Drop here" } else { "" },
                        palette.text_tertiary.alpha(reveal),
                    ))
                    .on_drag_move(cx.listener(
                        move |this, event: &DragMoveEvent<WidgetDrag>, _, cx| {
                            if event.bounds.contains(&event.event.position) {
                                let next = Some((event.drag(cx).widget.clone(), Some(column)));
                                if this.widget_drop != next {
                                    this.widget_drop = next;
                                    cx.notify();
                                }
                            }
                        },
                    ))
                    .on_drop(cx.listener(move |this, drag: &WidgetDrag, _, cx| {
                        this.commit_widget_drop(
                            drag.widget.clone(),
                            drag.widget.clone(),
                            Some(column),
                            cx,
                        );
                    }))
                    .into_any_element(),
            );
        }
        self.measured_column(Some(column), sections)
    }

    fn widget_drop_target(
        &mut self,
        widget: HomeWidgetId,
        section: AnyElement,
        column: Option<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let active = self.widget_drag.as_ref() == Some(&widget);
        let over = self.widget_drag.is_some()
            && !active
            && self.widget_drop.as_ref().map(|(target, _)| target) == Some(&widget);
        let outline = self
            .fx
            .toggle(fx::key(("widget-outline", widget.id())), over, fx::FASTER);
        let dim = self
            .fx
            .toggle(fx::key(("widget-dim", widget.id())), active, fx::FAST);
        let dx = self
            .fx
            .value(fx::key(("widget-dx", widget.id())), 0.0, fx::NORMAL);
        let dy = self
            .fx
            .value(fx::key(("widget-dy", widget.id())), 0.0, fx::NORMAL);
        let bounds = Rc::clone(&self.widget_bounds);
        let measured_widget = widget.clone();
        let hover_widget = widget.clone();
        let mut element = div()
            .id(eid(format!("widget-{}", widget.id())))
            .relative()
            .left(px(dx))
            .top(px(dy))
            .opacity(1.0 - 0.45 * dim)
            .child(section)
            .child(
                canvas(
                    move |area, _, _| {
                        bounds
                            .borrow_mut()
                            .widgets
                            .insert(measured_widget.clone(), area);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .on_drag_move(
                cx.listener(move |this, event: &DragMoveEvent<WidgetDrag>, _, cx| {
                    if event.bounds.contains(&event.event.position) {
                        let next = Some((hover_widget.clone(), column));
                        if this.widget_drop != next {
                            this.widget_drop = next;
                            cx.notify();
                        }
                    }
                }),
            )
            .on_drop(cx.listener(move |this, drag: &WidgetDrag, _, cx| {
                this.commit_widget_drop(drag.widget.clone(), widget.clone(), column, cx);
            }));
        if outline > 0.001 {
            element = element.child(
                div()
                    .absolute()
                    .inset(px(-3.0))
                    .rounded(px(CARD_RADIUS))
                    .border_1()
                    .border_color(palette.accent.alpha(outline)),
            );
        }
        element.into_any_element()
    }

    fn commit_widget_drop(
        &mut self,
        active: HomeWidgetId,
        over: HomeWidgetId,
        column: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        self.widget_drag = None;
        self.widget_drop = None;
        let Some((order, right)) = home_widget_drop_layout(
            &self.ui,
            &active,
            &over,
            column,
            show_total_spend(&self.ui) && self.show_provider_tabs(),
        ) else {
            cx.notify();
            return;
        };
        self.animate_widget_reflow(&order, &right);
        let local_order = order.clone();
        let local_right = right.clone();
        self.persist(
            cx,
            |ui| {
                ui.popup_home_order = local_order;
                ui.popup_home_right_column = Some(local_right);
            },
            move |settings| {
                settings.popup_home_order = order;
                settings.popup_home_right_column = Some(right);
            },
        );
    }

    /// FLIP: predict where each block lands in the new order (from the last
    /// measured heights) and glide it there from its old position, so the
    /// first frame of the new layout never jumps.
    fn animate_widget_reflow(&mut self, order: &[HomeWidgetId], right: &[HomeWidgetId]) {
        if !self.fx.enabled() {
            return;
        }
        let measured = self.widget_bounds.borrow();
        let two_columns = self.two_columns();
        let mut columns: [Vec<HomeWidgetId>; 2] = [Vec::new(), Vec::new()];
        for widget in order {
            if measured.widgets.contains_key(widget) {
                let column = if two_columns {
                    usize::from(right.contains(widget))
                } else {
                    0
                };
                columns[column].push(widget.clone());
            }
        }
        let mut targets: HashMap<HomeWidgetId, (f32, f32)> = HashMap::new();
        for (column, widgets) in columns.iter().enumerate() {
            let Some(origin) = measured.columns[column].or(measured.columns[0]) else {
                continue;
            };
            let x = f32::from(origin.origin.x);
            let mut y = f32::from(origin.origin.y);
            for (index, widget) in widgets.iter().enumerate() {
                let old = measured.widgets[widget];
                let was_first = old_is_first(&measured.widgets, old);
                let mut height = f32::from(old.size.height);
                // Headings drop their top margin when they lead a column.
                match (was_first, index == 0) {
                    (true, false) => height += HEADING_TOP,
                    (false, true) => height -= HEADING_TOP,
                    _ => {}
                }
                targets.insert(widget.clone(), (x, y));
                y += height + SECTION_GAP;
            }
        }
        let deltas = targets
            .into_iter()
            .map(|(widget, (x, y))| {
                let old = measured.widgets[&widget];
                (
                    widget,
                    f32::from(old.origin.x) - x,
                    f32::from(old.origin.y) - y,
                )
            })
            .collect::<Vec<_>>();
        drop(measured);
        for (widget, dx, dy) in deltas {
            self.fx.snap(fx::key(("widget-dx", widget.id())), dx);
            self.fx.snap(fx::key(("widget-dy", widget.id())), dy);
        }
        self.fx.mark_animating();
    }

    fn render_total_spend(
        &mut self,
        is_first: bool,
        can_reorder: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let period = self.ui.total_spend_period;
        let enabled = self.enabled_spend();
        let key = format!(
            "spend|{}|{:?}|{}|{}|{}",
            self.ui.usage_revision,
            enabled,
            crate::store::codex_accounts::cached_current_id(),
            crate::usage::truncate_local_hour(Local::now()),
            period.key()
        );
        let snapshot = self.snapshot(
            SnapshotSlot::Spend,
            key,
            enabled,
            move |limits, enabled| {
                crate::usage_overview::total_spend_snapshot(limits, enabled, period)
            },
            cx,
        );
        let entries = crate::usage_overview::spend_entries(&snapshot);
        let total: u64 = entries
            .iter()
            .fold(0, |sum, (_, spend)| sum.saturating_add(*spend));

        let title_id = fx::key("usage-stats-title");
        let title_hover = self.fx.toggle(
            fx::key(("usage-stats-title-fx", title_id)),
            self.hovered(title_id),
            fx::TEXT_FADE,
        );
        let title = div()
            .id("usage-stats-title")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .on_hover(self.hover_listener(title_id, None, cx))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.navigate(PopupView::Usage, cx);
            }))
            .child(components::icon(
                "fluent-chart",
                16.0,
                palette.text_secondary.mix(palette.accent, title_hover),
            ))
            .child(components::body_strong(
                "Usage Stats",
                palette.text_secondary.mix(palette.accent, title_hover),
            ));
        let periods = [
            TotalSpendPeriod::Today,
            TotalSpendPeriod::Yesterday,
            TotalSpendPeriod::ThirtyDays,
        ];
        let tabs = periods
            .iter()
            .map(|item| (item.label(), *item == period))
            .collect::<Vec<_>>();
        let selector = self.text_tabs(
            fx::key("spend-period"),
            &tabs,
            move |this, index, cx| {
                let period = periods[index];
                if this.ui.total_spend_period != period {
                    this.persist(
                        cx,
                        |ui| ui.total_spend_period = period,
                        move |settings| settings.total_spend_period = period,
                    );
                }
            },
            cx,
        );
        let mut trailing = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .child(selector);
        if can_reorder {
            trailing = trailing.child(self.widget_drag_handle(HomeWidgetId::total_spend(), cx));
        }
        let heading = components::split_row(title, trailing)
            .px(px(4.0))
            .mt(px(if is_first { 0.0 } else { HEADING_TOP }))
            .mb(px(2.0));

        let initial_loading = self
            .snapshots
            .get(&SnapshotSlot::Spend)
            .is_some_and(|cache| cache.key.is_none() && cache.pending.is_some());
        let content = if initial_loading {
            // Pending data is not a zero-dollar result. Keep aggregation off
            // the UI thread without flashing a false total during opening.
            div()
                .h(px(112.0))
                .flex()
                .items_center()
                .justify_center()
                .child(caption("Loading usage…", palette.text_tertiary))
                .into_any_element()
        } else {
            match self.ui.total_spend_presentation {
                TotalSpendPresentation::Donut => self.spend_donut_content(&entries, total),
                TotalSpendPresentation::ProgressBar => self.spend_hero_content(&entries, total),
            }
        };
        let card_id = fx::key("usage-stats-card");
        let card_hover = self.fx.toggle(
            fx::key(("usage-stats-card-fx", card_id)),
            self.hovered(card_id),
            fx::FASTER,
        );
        let mut body = card(&palette)
            .id("usage-stats-card")
            .relative()
            .on_hover(self.hover_listener(card_id, None, cx))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.navigate(PopupView::Usage, cx);
            }));
        if let Some(layer) = components::hover_layer(&palette, card_hover, CARD_RADIUS - 1.0) {
            body = body.child(layer);
        }
        let _ = window;
        div()
            .flex()
            .flex_col()
            .gap(px(SECTION_GAP))
            .child(heading)
            .child(body.child(div().relative().p(px(12.0)).child(content)))
            .into_any_element()
    }

    fn spend_hero_content(&self, entries: &[(ProviderId, u64)], total: u64) -> AnyElement {
        let palette = &self.palette;
        let colored = self.ui.use_colored_provider_icons;
        let mut tiles = div().grid().grid_cols(3).gap_x(px(8.0)).gap_y(px(16.0));
        for (provider, spend) in entries {
            tiles = tiles.child(
                div()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(components::provider_mark(
                                crate::provider_registry::icon(provider.kind()),
                                16.0,
                                palette.provider_icon(provider.kind(), colored),
                                provider.badge().as_ref(),
                                palette,
                            ))
                            .child(nowrap(components::body_strong(
                                provider.qualified_name(),
                                palette.text_primary,
                            ))),
                    )
                    .child(components::caption_strong(
                        format_spend_full(*spend),
                        palette.text_primary,
                    )),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        components::text(
                            format_spend_full(total),
                            28.0,
                            36.0,
                            palette.text_primary,
                        )
                        .font_weight(gpui::FontWeight::SEMIBOLD),
                    )
                    .child(share_bar(entries, |provider| palette.spend_color(provider))),
            )
            .child(tiles)
            .into_any_element()
    }

    fn spend_donut_content(&self, entries: &[(ProviderId, u64)], total: u64) -> AnyElement {
        let palette = self.palette.clone();
        let colored = self.ui.use_colored_provider_icons;
        let mut legend = div().flex().flex_col().gap(px(12.0)).flex_1().min_w_0();
        for (provider, spend) in entries {
            legend = legend.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(components::provider_mark(
                        crate::provider_registry::icon(provider.kind()),
                        16.0,
                        palette.provider_icon(provider.kind(), colored),
                        provider.badge().as_ref(),
                        &palette,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(nowrap(components::body_strong(
                                provider.qualified_name(),
                                palette.text_primary,
                            ))),
                    )
                    .child(components::body_strong(
                        format_spend_compact(*spend),
                        palette.text_primary,
                    )),
            );
        }
        let segments = donut_segments(entries, total)
            .into_iter()
            .map(|(provider, start, end)| {
                (
                    provider.map_or(super::theme::rgb8((0x78, 0x78, 0x78)), |provider| {
                        palette.spend_color(provider)
                    }),
                    start,
                    end,
                )
            })
            .collect::<Vec<_>>();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(16.0))
            .child(
                div()
                    .relative()
                    .size(px(124.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |bounds, _, window, _| {
                                paint_donut(bounds, &segments, window);
                            },
                        )
                        .absolute()
                        .inset_0(),
                    )
                    .child(
                        components::text(
                            format_spend_compact(total),
                            18.0,
                            24.0,
                            palette.text_primary,
                        )
                        .font_weight(gpui::FontWeight::SEMIBOLD),
                    ),
            )
            .child(legend)
            .into_any_element()
    }
}

/// Did this block start its column (no measured block above it)?
fn old_is_first(widgets: &HashMap<HomeWidgetId, Bounds<Pixels>>, bounds: Bounds<Pixels>) -> bool {
    !widgets.values().any(|other| {
        (f32::from(other.origin.x) - f32::from(bounds.origin.x)).abs() < 1.0
            && other.origin.y < bounds.origin.y
    })
}

/// Rounded segments sized by weight, separated by 4 DIP gaps.
pub(super) fn share_bar(
    entries: &[(ProviderId, u64)],
    color: impl Fn(ProviderId) -> Hsla,
) -> gpui::Div {
    let total: u64 = entries
        .iter()
        .fold(0, |sum, (_, value)| sum.saturating_add(*value));
    let mut bar = div().flex().flex_row().gap(px(4.0)).h(px(10.0)).w_full();
    for (provider, value) in entries {
        let weight = if total == 0 { 1 } else { (*value).max(1) };
        bar = bar.child(
            div()
                .h_full()
                .flex_grow()
                .flex_basis(px(0.0))
                .min_w(px(4.0))
                .rounded(px(4.0))
                .bg(color(*provider))
                .flex_grow_weight(weight as f32),
        );
    }
    bar
}

trait FlexGrowWeight {
    fn flex_grow_weight(self, weight: f32) -> Self;
}

impl FlexGrowWeight for gpui::Div {
    fn flex_grow_weight(mut self, weight: f32) -> Self {
        self.style().flex_grow = Some(weight);
        self
    }
}

/// Angular spans of the spend donut, in degrees from 12 o'clock clockwise.
/// `None` marks the neutral ring drawn when there is no spend at all.
pub(crate) fn donut_segments(
    entries: &[(ProviderId, u64)],
    total: u64,
) -> Vec<(Option<ProviderId>, f32, f32)> {
    const GAP_DEGREES: f32 = 2.0;
    if total == 0 {
        return vec![(None, -90.0, 270.0)];
    }
    let mut start = -90.0_f32;
    let mut segments = Vec::new();
    for (provider, spend) in entries.iter().filter(|(_, spend)| *spend > 0) {
        let end = start + *spend as f32 / total as f32 * 360.0;
        if end - start >= 359.0 {
            segments.push((Some(*provider), -90.0, 270.0));
        } else {
            segments.push((
                Some(*provider),
                start + GAP_DEGREES / 2.0,
                end - GAP_DEGREES / 2.0,
            ));
        }
        start = end;
    }
    segments
}

fn paint_donut(bounds: Bounds<Pixels>, segments: &[(Hsla, f32, f32)], window: &mut Window) {
    const OUTER: f32 = 53.0;
    const INNER: f32 = 34.0;
    let center = point(
        f32::from(bounds.origin.x) + f32::from(bounds.size.width) / 2.0,
        f32::from(bounds.origin.y) + f32::from(bounds.size.height) / 2.0,
    );
    let at = |radius: f32, degrees: f32| {
        let radians = degrees * PI / 180.0;
        point(
            px(center.x + radius * radians.cos()),
            px(center.y + radius * radians.sin()),
        )
    };
    for (color, start, end) in segments {
        let sweep = end - start;
        if sweep <= 0.0 {
            continue;
        }
        let mut builder = PathBuilder::fill();
        if sweep >= 359.0 {
            // Full ring: two half arcs outside, two inside (even-odd hole).
            builder.move_to(at(OUTER, -90.0));
            builder.arc_to(
                point(px(OUTER), px(OUTER)),
                px(0.0),
                false,
                true,
                at(OUTER, 90.0),
            );
            builder.arc_to(
                point(px(OUTER), px(OUTER)),
                px(0.0),
                false,
                true,
                at(OUTER, 270.0),
            );
            builder.close();
            builder.move_to(at(INNER, -90.0));
            builder.arc_to(
                point(px(INNER), px(INNER)),
                px(0.0),
                false,
                false,
                at(INNER, 90.0),
            );
            builder.arc_to(
                point(px(INNER), px(INNER)),
                px(0.0),
                false,
                false,
                at(INNER, 270.0),
            );
            builder.close();
        } else {
            let large = sweep > 180.0;
            builder.move_to(at(OUTER, *start));
            builder.arc_to(
                point(px(OUTER), px(OUTER)),
                px(0.0),
                large,
                true,
                at(OUTER, *end),
            );
            builder.line_to(at(INNER, *end));
            builder.arc_to(
                point(px(INNER), px(INNER)),
                px(0.0),
                large,
                false,
                at(INNER, *start),
            );
            builder.close();
        }
        if let Ok(path) = builder.build() {
            window.paint_path(path, *color);
        }
    }
}

pub(super) fn format_spend_full(microusd: u64) -> String {
    let cents = (microusd as f64 / 10_000.0)
        .round()
        .clamp(0.0, u64::MAX as f64) as u64;
    format!("${}.{:02}", format_thousands(cents / 100), cents % 100)
}

pub(super) fn format_spend_compact(microusd: u64) -> String {
    format_spend_dollars((microusd as f64 / 1_000_000.0).round() as u64)
}

pub(super) fn format_spend_dollars(dollars: u64) -> String {
    if dollars >= 1_000_000 {
        format!("${:.1}M", dollars as f64 / 1_000_000.0)
    } else if dollars >= 1_000 {
        format!("${:.1}K", dollars as f64 / 1_000.0)
    } else {
        format!("${dollars}")
    }
}

pub(super) fn format_thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    grouped.chars().rev().collect()
}

#[allow(dead_code)]
fn _unused(_: &OverviewSnapshot) {}
