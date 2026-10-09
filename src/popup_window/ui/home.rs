//! Home page: reorderable provider widgets and the Usage Stats summary.

use std::{collections::HashMap, f32::consts::PI, rc::Rc};

use gpui::{
    AnyElement, AppContext, Bounds, ClickEvent, Context, DragMoveEvent, Hsla, InteractiveElement,
    IntoElement, ParentElement, PathBuilder, Pixels, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, canvas, div, point, px, relative,
};

use super::{
    components::{self, caption, card, nowrap},
    fx,
    root::{PopupRoot, SnapshotSlot, eid},
    theme::{HslaExt, Palette},
};
use crate::popup_window::{model::*, *};
use crate::usage_overview::OverviewSnapshot;

pub(super) const COLUMN_GAP: f32 = 20.0;
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
    /// Heading trailing content plus the reorder grip, which slides in only
    /// while Home is in edit mode.
    pub(super) fn with_widget_grip(
        &mut self,
        trailing: gpui::Div,
        widget: HomeWidgetId,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let reveal = self.fx.toggle(
            fx::key(("widget-grip", widget.id())),
            self.home_editing,
            fx::FAST,
        );
        // The wrapper takes whatever width the caller gives the trailing
        // group (half the heading row beside an account name); the group
        // fills it so its content stays against the end, ahead of the grip.
        let row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_end()
            .child(trailing.flex_1().min_w_0());
        if reveal < 0.001 {
            return row;
        }
        row.child(
            div()
                .flex_none()
                .flex()
                .justify_end()
                .w(px(32.0 * reveal))
                .overflow_hidden()
                .opacity(reveal)
                .child(self.widget_drag_handle(widget, cx)),
        )
    }

    fn widget_drag_handle(&mut self, widget: HomeWidgetId, cx: &mut Context<Self>) -> AnyElement {
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
            .on_hover(self.hover_listener(
                hover_id,
                Some(crate::i18n::tr("drag-to-reorder").into()),
                cx,
            ))
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
        let gap = self.card_gap();
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
                    let layout = ui.home_card_layout(provider);
                    let has_limits = cards.iter().any(|card| matches!(card, Card::Limit { .. }));
                    let mut body = self.render_home_cards(&cards, layout, window, cx);
                    let heading = body.drain(..1.min(body.len())).collect::<Vec<_>>();
                    // A changed layout fades in instead of popping.
                    let fade = self.fx.value(
                        fx::key(("card-layout-fade", provider.id())),
                        1.0,
                        fx::NORMAL,
                    );
                    let mut content = div().flex().flex_col();
                    if let Some(picker) =
                        self.card_layout_picker(provider, layout, has_limits, window, cx)
                    {
                        content = content.child(picker);
                    }
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(gap))
                        .children(heading)
                        .child(
                            content.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(gap))
                                    .opacity(fade)
                                    .children(body),
                            ),
                        )
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

    /// Edit-mode row choosing how a provider widget lays out its quota
    /// windows. It folds open under the heading while Home is being edited.
    fn card_layout_picker(
        &mut self,
        provider: ProviderId,
        layout: HomeCardLayout,
        available: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let gap = self.card_gap();
        let picker_height = 28.0 + gap;
        let reveal = self.fx.toggle(
            fx::key(("card-layout-picker", provider.id())),
            self.home_editing && available,
            fx::FAST,
        );
        if reveal < 0.001 {
            return None;
        }
        let layouts = HomeCardLayout::ALL;
        let labels = [
            "home-card-layout-cards",
            "home-card-layout-lines",
            "home-card-layout-rings",
        ]
        .into_iter()
        .map(|key| SharedString::from(crate::i18n::tr(key)))
        .collect();
        let selector = self.segmented_control_compact(
            fx::key(("card-layout", provider.id())),
            labels,
            layouts.iter().position(|item| *item == layout).unwrap_or(0),
            move |this, index, cx| {
                let next = layouts[index];
                if this.ui.home_card_layout(provider) == next {
                    return;
                }
                this.fx
                    .snap(fx::key(("card-layout-fade", provider.id())), 0.0);
                let id = provider.id().to_owned();
                let local_id = id.clone();
                this.persist(
                    cx,
                    move |ui| set_card_layout(&mut ui.popup_home_card_layouts, local_id, next),
                    move |settings| {
                        set_card_layout(&mut settings.popup_home_card_layouts, id, next);
                    },
                );
            },
            window,
            cx,
        );
        let palette = &self.palette;
        Some(
            div()
                .flex_none()
                .overflow_hidden()
                .h(px(picker_height * reveal))
                .opacity(reveal)
                .child(
                    components::split_row(
                        nowrap(caption(
                            crate::i18n::tr("home-card-layout"),
                            palette.text_tertiary,
                        )),
                        selector,
                    )
                    .px(px(4.0)),
                )
                .into_any_element(),
        )
    }

    /// A column's sections, recording the column origin for reorder motion.
    fn measured_column(&mut self, column: Option<usize>, sections: Vec<AnyElement>) -> gpui::Div {
        let gap = self.card_gap();
        let bounds = Rc::clone(&self.widget_bounds);
        let slot = column.unwrap_or(0);
        div()
            .relative()
            .flex()
            .flex_col()
            .gap(px(gap))
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
                        if dragging {
                            crate::i18n::tr("drop-here")
                        } else {
                            ""
                        },
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
                    .rounded(px(palette.card_radius))
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
        let gap = self.card_gap();
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
                y += height + gap;
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
        let gap = self.card_gap();
        let palette = self.palette.clone();
        let period = self.ui.total_spend_period;
        let enabled = self.enabled_spend();
        let key = format!(
            "spend|{}|{:?}|{}|{}|{}|{}",
            self.ui.usage_revision,
            enabled,
            crate::store::codex_accounts::cached_current_id(),
            crate::usage::truncate_local_hour(Local::now()),
            period.key(),
            crate::i18n::current_language().index()
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
            .child(nowrap(components::body_strong(
                crate::i18n::tr("home-usage-title"),
                palette.text_secondary.mix(palette.accent, title_hover),
            )));
        let periods = [
            TotalSpendPeriod::Today,
            TotalSpendPeriod::Yesterday,
            TotalSpendPeriod::ThirtyDays,
        ];
        let selector = self.segmented_control_compact(
            fx::key("spend-period"),
            periods
                .iter()
                .map(|item| SharedString::from(item.label()))
                .collect(),
            periods.iter().position(|item| *item == period).unwrap_or(2),
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
            window,
            cx,
        );
        let mut trailing = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .child(selector);
        if can_reorder {
            trailing = self.with_widget_grip(trailing, HomeWidgetId::total_spend(), cx);
        }
        // Same row as provider headings so the drag grips line up.
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
                .child(caption(
                    crate::i18n::tr("loading-usage"),
                    palette.text_tertiary,
                ))
                .into_any_element()
        } else {
            match self.ui.total_spend_presentation {
                TotalSpendPresentation::Donut => self.spend_donut_content(&entries, total, window),
                TotalSpendPresentation::ProgressBar => {
                    self.spend_hero_content(&entries, total, window)
                }
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
        if let Some(layer) =
            components::hover_layer(&palette, card_hover, palette.card_radius - 1.0)
        {
            body = body.child(layer);
        }
        let _ = window;
        div()
            .flex()
            .flex_col()
            .gap(px(gap))
            .child(heading)
            .child(body.child(div().relative().p(px(12.0)).child(content)))
            .into_any_element()
    }

    fn spend_hero_content(
        &mut self,
        entries: &[(ProviderId, u64)],
        total: u64,
        window: &Window,
    ) -> AnyElement {
        const SCOPE: &str = "spend-cards";
        const COLUMNS: usize = 3;
        const TILE_HEIGHT: f32 = 40.0;
        const ROW_PITCH: f32 = TILE_HEIGHT + 16.0;
        let palette = self.palette.clone();
        let colored = self.ui.use_colored_provider_icons;
        let slots = self.spend_slots(SCOPE, entries, COLUMNS);
        let rows = entries.len().div_ceil(COLUMNS) as f32;
        let height = self.fx.value(
            fx::key((SCOPE, "height")),
            (rows * ROW_PITCH - 16.0).max(0.0),
            fx::SETTLE,
        );
        // Tiles sit on animated grid cells so a new ranking slides each
        // provider to its new place.
        let mut tiles = div().relative().w_full().h(px(height));
        for ((provider, spend), (column, row)) in entries.iter().zip(slots) {
            let amount = self.spend_amount(SCOPE, Some(*provider), *spend, 12.0, 16.0, window);
            tiles = tiles.child(
                div()
                    .absolute()
                    .left(relative(column / COLUMNS as f32))
                    .top(px(row * ROW_PITCH))
                    .w(relative(1.0 / COLUMNS as f32))
                    .h(px(TILE_HEIGHT))
                    .pr(px(8.0))
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
                                &palette,
                            ))
                            .child(nowrap(components::body_strong(
                                provider.qualified_name(),
                                palette.text_primary,
                            ))),
                    )
                    .child(amount),
            );
        }
        let total_label = self.spend_amount(SCOPE, None, total, 28.0, 36.0, window);
        let bar = self.share_bar(SCOPE, entries, total);
        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(total_label)
                    .child(bar),
            )
            .child(tiles)
            .into_any_element()
    }

    fn spend_donut_content(
        &mut self,
        entries: &[(ProviderId, u64)],
        total: u64,
        window: &Window,
    ) -> AnyElement {
        const SCOPE: &str = "spend-donut";
        const ROW_HEIGHT: f32 = 20.0;
        const ROW_PITCH: f32 = ROW_HEIGHT + 12.0;
        let palette = self.palette.clone();
        // The total sits in the ring's 68 DIP hole; long amounts shrink
        // (text width scales linearly with size) instead of moving out.
        let label_width = components::measure_text(
            window.text_system(),
            palette.font_family.clone(),
            18.0,
            gpui::FontWeight::SEMIBOLD,
            &format_spend_full(total),
        );
        let label_size = if label_width > DONUT_LABEL_WIDTH {
            ((18.0 * DONUT_LABEL_WIDTH / label_width) * 2.0).floor() / 2.0
        } else {
            18.0
        }
        .max(9.0);
        let label_size = self
            .fx
            .value(fx::key((SCOPE, "label-size")), label_size, fx::SETTLE);
        let total_label = self.spend_amount(
            SCOPE,
            None,
            total,
            label_size,
            (label_size * 4.0 / 3.0).ceil(),
            window,
        );
        let colored = self.ui.use_colored_provider_icons;
        let slots = self.spend_slots(SCOPE, entries, 1);
        let height = self.fx.value(
            fx::key((SCOPE, "height")),
            (entries.len() as f32 * ROW_PITCH - 12.0).max(0.0),
            fx::SETTLE,
        );
        let mut legend = div().relative().flex_1().min_w_0().h(px(height));
        for ((provider, spend), (_, row)) in entries.iter().zip(slots) {
            let amount = self.spend_amount(SCOPE, Some(*provider), *spend, 14.0, 20.0, window);
            legend = legend.child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(row * ROW_PITCH))
                    .h(px(ROW_HEIGHT))
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
                    .child(amount),
            );
        }
        // The neutral ring fades in as the last share collapses.
        let empty = self
            .fx
            .toggle(fx::key((SCOPE, "empty")), total == 0, fx::SETTLE);
        let mut segments = Vec::new();
        if empty > 0.0 {
            segments.push((
                super::theme::rgb8((0x78, 0x78, 0x78)).opacity(empty),
                -90.0,
                270.0,
            ));
        }
        for (provider, start, end) in self.spend_spans(SCOPE, entries, total, false) {
            let (start, end) = donut_arc(start, end);
            segments.push((palette.spend_color(provider), start, end));
        }
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
                    .child(total_label),
            )
            .child(legend)
            .into_any_element()
    }

    /// Each provider's animated `(column, row)` cell, so a new ranking
    /// glides rows into place instead of jumping.
    fn spend_slots(
        &mut self,
        scope: &'static str,
        entries: &[(ProviderId, u64)],
        columns: usize,
    ) -> Vec<(f32, f32)> {
        entries
            .iter()
            .enumerate()
            .map(|(index, (provider, _))| {
                (
                    self.fx.value(
                        fx::key((scope, "column", *provider)),
                        (index % columns) as f32,
                        fx::SETTLE,
                    ),
                    self.fx.value(
                        fx::key((scope, "row", *provider)),
                        (index / columns) as f32,
                        fx::SETTLE,
                    ),
                )
            })
            .collect()
    }

    /// Animated [`spend_spans`]: shares grow and shrink in place. Segments
    /// keep the enabled-provider order rather than the ranking, so a new
    /// ranking never slides one slice across another.
    fn spend_spans(
        &mut self,
        scope: &'static str,
        entries: &[(ProviderId, u64)],
        total: u64,
        even_when_empty: bool,
    ) -> Vec<(ProviderId, f32, f32)> {
        let order = self.enabled_spend();
        let mut entries = entries.to_vec();
        entries.sort_by_key(|(provider, _)| order.iter().position(|id| id == provider));
        spend_spans(&entries, total, even_when_empty)
            .into_iter()
            .map(|(provider, start, end)| {
                let start = self
                    .fx
                    .value(fx::key((scope, "start", provider)), start, fx::SETTLE);
                let end = self
                    .fx
                    .value(fx::key((scope, "end", provider)), end, fx::SETTLE);
                (provider, start, end.max(start))
            })
            .collect()
    }

    /// A dollar amount whose changed digits roll to the new value.
    fn spend_amount(
        &mut self,
        scope: &'static str,
        provider: Option<ProviderId>,
        spend: u64,
        size: f32,
        line: f32,
        window: &Window,
    ) -> gpui::Div {
        let color = self.palette.text_primary;
        self.rolling_label(
            fx::key((scope, "amount", provider)),
            format_spend_full(spend),
            spend,
            (size, line, gpui::FontWeight::SEMIBOLD, color),
            window,
        )
        .flex_none()
    }

    /// A label whose changed digits roll from its previous text; `value`
    /// orders the two texts so the digits spin up or down.
    pub(super) fn rolling_label(
        &mut self,
        id: u64,
        text: impl Into<SharedString>,
        value: u64,
        (size, line, weight, color): (f32, f32, gpui::FontWeight, Hsla),
        window: &Window,
    ) -> gpui::Div {
        let frame = self.fx.roll(id, text, value, fx::ROLL);
        components::rolling_text(
            &frame,
            window.text_system(),
            self.palette.font_family.clone(),
            size,
            line,
            weight,
            color,
        )
    }

    /// Rounded segments sized by share, separated by 4 DIP gaps. Segments
    /// sit on animated spans so a period change resizes and reorders them.
    pub(super) fn share_bar(
        &mut self,
        scope: &'static str,
        entries: &[(ProviderId, u64)],
        total: u64,
    ) -> gpui::Div {
        let mut segments = div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(-2.0))
            .right(px(-2.0));
        for (provider, start, end) in self.spend_spans(scope, entries, total, true) {
            segments = segments.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(relative(start))
                    .w(relative(end - start))
                    .px(px(2.0))
                    .child(
                        div()
                            .size_full()
                            .rounded(px(4.0))
                            .bg(self.palette.spend_color(provider)),
                    ),
            );
        }
        div().relative().w_full().h(px(10.0)).child(segments)
    }
}

/// Ordinary cards are the default, so they leave no entry behind.
fn set_card_layout(
    layouts: &mut std::collections::BTreeMap<String, HomeCardLayout>,
    id: String,
    layout: HomeCardLayout,
) {
    if layout == HomeCardLayout::Cards {
        layouts.remove(&id);
    } else {
        layouts.insert(id, layout);
    }
}

/// Did this block start its column (no measured block above it)?
fn old_is_first(widgets: &HashMap<HomeWidgetId, Bounds<Pixels>>, bounds: Bounds<Pixels>) -> bool {
    !widgets.values().any(|other| {
        (f32::from(other.origin.x) - f32::from(bounds.origin.x)).abs() < 1.0
            && other.origin.y < bounds.origin.y
    })
}

/// Each provider's share as `(start, end)` fractions of the whole, in
/// ranking order. With no spend, `even_when_empty` splits it evenly;
/// otherwise every span collapses to zero width.
fn spend_spans(
    entries: &[(ProviderId, u64)],
    total: u64,
    even_when_empty: bool,
) -> Vec<(ProviderId, f32, f32)> {
    let weight = |value: u64| {
        if total == 0 && even_when_empty {
            1.0
        } else {
            value as f64
        }
    };
    let sum: f64 = entries.iter().map(|(_, value)| weight(*value)).sum();
    let mut start = 0.0_f64;
    entries
        .iter()
        .map(|(provider, value)| {
            let end = if sum > 0.0 {
                start + weight(*value) / sum
            } else {
                start
            };
            let span = (*provider, start as f32, end as f32);
            start = end;
            span
        })
        .collect()
}

/// Donut angles for a `(start, end)` share, in degrees from 12 o'clock
/// clockwise, keeping half of a 2° gap on both ends. A slice thinner than
/// the gap comes back with a negative sweep, which the painter skips.
fn donut_arc(start: f32, end: f32) -> (f32, f32) {
    const GAP_DEGREES: f32 = 2.0;
    let (start, end) = (-90.0 + start * 360.0, -90.0 + end * 360.0);
    if end - start >= 359.0 {
        (-90.0, 270.0)
    } else {
        (start + GAP_DEGREES / 2.0, end - GAP_DEGREES / 2.0)
    }
}

/// Angular spans of the spend donut at rest.
/// `None` marks the neutral ring drawn when there is no spend at all.
#[cfg(test)]
pub(crate) fn donut_segments(
    entries: &[(ProviderId, u64)],
    total: u64,
) -> Vec<(Option<ProviderId>, f32, f32)> {
    if total == 0 {
        return vec![(None, -90.0, 270.0)];
    }
    spend_spans(entries, total, false)
        .into_iter()
        .zip(entries)
        .filter(|(_, (_, spend))| *spend > 0)
        .map(|((provider, start, end), _)| {
            let (start, end) = donut_arc(start, end);
            (Some(provider), start, end)
        })
        .collect()
}

/// Widest total label that clears the donut's inner edge with some air.
const DONUT_LABEL_WIDTH: f32 = 60.0;

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
    // Integer rounding also preserves cents at the u64 upper bound.
    let cents = microusd / 10_000 + u64::from(microusd % 10_000 >= 5_000);
    format!("${}.{:02}", format_thousands(cents / 100), cents % 100)
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

#[cfg(test)]
mod money_tests {
    use super::format_spend_full;

    #[test]
    fn microdollars_keep_cents_through_rounding_and_u64_boundaries() {
        for (value, expected) in [
            (0, "$0.00"),
            (4_999, "$0.00"),
            (5_000, "$0.01"),
            (995_000, "$1.00"),
            (999_995_000, "$1,000.00"),
            (1_284_000_000, "$1,284.00"),
            (1_299_000_000, "$1,299.00"),
            (u64::MAX, "$18,446,744,073,709.55"),
        ] {
            assert_eq!(format_spend_full(value), expected);
        }
    }
}
