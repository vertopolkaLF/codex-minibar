//! Usage tab: local API usage overview across providers.

use std::{cell::Cell, collections::BTreeMap, f32::consts::PI, rc::Rc, sync::Arc, time::Instant};

use chrono::{DateTime, Duration, Local, NaiveDate};
use gpui::{
    AnyElement, AnyView, AppContext, Bounds, Context, FontWeight, Hsla, InteractiveElement,
    IntoElement, MouseMoveEvent, ParentElement, PathBuilder, Pixels, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, canvas, div, point, px, relative,
};

use super::{
    components::{self, caption, card, nowrap},
    fx,
    home::{format_spend_dollars, format_spend_full, share_bar},
    root::{PopupRoot, SnapshotSlot},
    theme::Palette,
    tooltip::{ChartTip, TipContent},
};
use crate::popup_window::*;
use crate::{
    usage::TokenUsage,
    usage_overview::{
        BreakdownMode, BreakdownRow, DailySeriesPoint, OverviewMetric, OverviewRange,
        OverviewSnapshot, ProviderOverview,
    },
};

const CHART_PLOT_HEIGHT: f32 = 132.0;
const CHART_Y_AXIS_WIDTH: f32 = 40.0;
const CHART_Y_GAP: f32 = 6.0;
const CHART_PAD_X: f32 = 4.0;
const CHART_PAD_TOP: f32 = 6.0;
const CHART_PAD_BOTTOM: f32 = 3.0;

pub(super) struct UsageChartData {
    series: Arc<Vec<DailySeriesPoint>>,
    providers: Arc<Vec<ProviderId>>,
    max_value: u64,
}

pub(super) struct UsageChartCache {
    source: Arc<OverviewSnapshot>,
    metric: OverviewMetric,
    data: Arc<UsageChartData>,
}

impl UsageChartCache {
    fn new(source: Arc<OverviewSnapshot>, metric: OverviewMetric) -> Self {
        let series = if source.hourly {
            source.daily_series.clone()
        } else {
            fill_daily_series(&source.daily_series, source.start_date, source.end_date)
        };
        let providers: Vec<_> = source
            .providers
            .iter()
            .map(|entry| entry.provider)
            .collect();
        let raw_max = series
            .iter()
            .flat_map(|point| {
                providers
                    .iter()
                    .filter_map(|provider| point.by_provider.get(provider).copied())
            })
            .max()
            .unwrap_or(0);
        Self {
            source,
            metric,
            data: Arc::new(UsageChartData {
                series: Arc::new(series),
                providers: Arc::new(providers),
                max_value: chart_scale_max(raw_max),
            }),
        }
    }

    fn matches(&self, source: &Arc<OverviewSnapshot>, metric: OverviewMetric) -> bool {
        Arc::ptr_eq(&self.source, source) && self.metric == metric
    }
}

/// A separate cached GPUI view: root hover/tooltip/footer updates must not
/// rebuild or tessellate the unchanged area chart.
pub(super) struct UsagePlot {
    data: Arc<UsageChartData>,
    scale: f32,
    palette: Palette,
}

impl Render for UsagePlot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let data = Arc::clone(&self.data);
        let scale = self.scale;
        let palette = self.palette.clone();
        let lines: Vec<_> = data
            .providers
            .iter()
            .map(|provider| (*provider, palette.series_color(*provider)))
            .collect();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                paint_area_chart(
                    bounds,
                    &data.series,
                    &lines,
                    scale.max(1.0),
                    palette.chart_grid,
                    palette.accent,
                    None,
                    window,
                );
            },
        )
        .size_full()
    }
}

fn usage_card(palette: &Palette) -> gpui::Div {
    card(palette).p(px(12.0)).flex().flex_col()
}

impl PopupRoot {
    pub(super) fn render_usage_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let metric = self.overview_metric;
        let range = self.overview_range;
        let enabled = self.enabled_spend();
        let key = format!(
            "overview|{}|{:?}|{}|{}|{:?}|{:?}|{}",
            self.ui.usage_revision,
            enabled,
            crate::store::codex_accounts::cached_current_id(),
            crate::usage::truncate_local_hour(Local::now()),
            metric,
            range,
            crate::i18n::is_russian()
        );
        let snapshot = self.snapshot(
            SnapshotSlot::Overview,
            key,
            enabled,
            move |limits, enabled| {
                crate::usage_overview::build_overview_snapshot(limits, enabled, metric, range)
            },
            cx,
        );
        let recalculating = self
            .ui
            .active_requests
            .iter()
            .any(|(_, kind)| *kind == crate::worker::RequestKind::Usage)
            || self
                .snapshots
                .get(&SnapshotSlot::Overview)
                .is_some_and(|cache| cache.pending.is_some());
        if snapshot.providers.is_empty() {
            return div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(self.usage_title(None, recalculating))
                .child(caption(
                    if recalculating {
                        crate::i18n::tr("loading-usage")
                    } else {
                        crate::i18n::tr(
                            "enable-a-provider-in-settings-and-include-it-in-usage-stats-to-se",
                        )
                    },
                    palette.text_tertiary,
                ))
                .into_any_element();
        }
        let range_label = range_label(&snapshot);
        if self
            .usage_chart_cache
            .as_ref()
            .is_none_or(|cache| !cache.matches(&snapshot, metric))
        {
            self.usage_chart_cache = Some(UsageChartCache::new(Arc::clone(&snapshot), metric));
        }
        let chart_data = Arc::clone(&self.usage_chart_cache.as_ref().unwrap().data);
        let header = self.usage_header(&range_label, recalculating, window, cx);
        let hero = self.usage_hero(&snapshot, window, cx);
        let chart = self.usage_chart_card(&snapshot, chart_data, cx);
        let totals = usage_totals_card(&snapshot.totals, &palette);
        let breakdown = self.usage_breakdown_card(&snapshot, window, cx);
        let two_columns = self.two_columns();
        let page = div().flex().flex_col().gap(px(10.0)).w_full().child(header);
        if two_columns {
            page.child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(10.0))
                            .child(hero)
                            .child(chart),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(10.0))
                            .child(totals)
                            .child(breakdown),
                    ),
            )
            .into_any_element()
        } else {
            page.child(hero)
                .child(chart)
                .child(totals)
                .child(breakdown)
                .into_any_element()
        }
    }

    fn usage_title(&mut self, range_label: Option<&str>, recalculating: bool) -> gpui::Div {
        let palette = self.palette.clone();
        let shift = self
            .fx
            .toggle(fx::key("usage-title-shift"), recalculating, fx::FAST);
        let mut heading = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .h(px(24.0))
            .child(
                components::body_strong(crate::i18n::tr("usage-0bb186"), palette.text_primary)
                    .flex_none(),
            );
        if shift > 0.001 {
            let started = *self.usage_spinner_started.get_or_insert_with(Instant::now);
            let angle = started.elapsed().as_secs_f32() * 2.0 * PI;
            let color = palette.accent.opacity(shift);
            heading = heading.child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        paint_spinner(bounds, angle, color, window);
                    },
                )
                .flex_none()
                .size(px(16.0)),
            );
            if recalculating {
                self.fx.mark_animating();
            }
        } else {
            self.usage_spinner_started = None;
        }
        let mut title = div().flex().flex_col().min_w_0().child(heading);
        if let Some(label) = range_label {
            title = title.child(
                nowrap(components::body(label.to_owned(), palette.text_tertiary))
                    .flex_none()
                    .max_w(relative(1.0)),
            );
        }
        title
    }

    fn usage_header(
        &mut self,
        range_label: &str,
        recalculating: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let ranges = [
            OverviewRange::Past24h,
            OverviewRange::SevenDays,
            OverviewRange::ThirtyDays,
            OverviewRange::NinetyDays,
        ];
        let range_control = self.segmented_control(
            fx::key("usage-range"),
            ranges
                .iter()
                .map(|range| SharedString::from(range.short_label()))
                .collect(),
            ranges
                .iter()
                .position(|range| *range == self.overview_range)
                .unwrap_or(2),
            false,
            move |this, index, _| {
                this.overview_range = ranges[index];
                this.chart_hover = None;
                this.tip = None;
            },
            window,
            cx,
        );
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_x(px(8.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(self.usage_title(Some(range_label), recalculating)),
            )
            .child(div().flex_none().ml_auto().child(range_control))
            .into_any_element()
    }

    fn usage_hero(
        &mut self,
        snapshot: &OverviewSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let metric = self.overview_metric;
        let headline = match metric {
            OverviewMetric::Cost => format_total_cost(&snapshot.totals),
            OverviewMetric::Tokens => format_token_count(snapshot.totals.total_tokens()),
        };
        let mut meta =
            crate::i18n::format("sessions", &[("v0", snapshot.total_sessions.to_string())]);
        if metric == OverviewMetric::Cost {
            meta = format!("{meta} · {}", crate::i18n::tr("api-estimate"));
        }
        // Cost/Tokens only changes how the numbers read, so it sits beside
        // the headline it switches, quieter than the range control.
        let metric_control = self.segmented_control_quiet(
            fx::key("usage-metric"),
            vec![
                crate::i18n::tr("cost").into(),
                crate::i18n::tr("tokens").into(),
            ],
            usize::from(metric == OverviewMetric::Tokens),
            |this, index, _| {
                this.overview_metric = if index == 0 {
                    OverviewMetric::Cost
                } else {
                    OverviewMetric::Tokens
                };
                this.chart_hover = None;
                this.tip = None;
            },
            window,
            cx,
        );
        let mut entries: Vec<(ProviderId, u64)> = snapshot
            .providers
            .iter()
            .map(|entry| {
                (
                    entry.provider,
                    match metric {
                        OverviewMetric::Cost => entry.usage.estimated_cost_microusd,
                        OverviewMetric::Tokens => entry.usage.total_tokens(),
                    },
                )
            })
            .collect();
        entries.sort_by(|(_, a), (_, b)| b.cmp(a));
        let colored = self.ui.use_colored_provider_icons;
        let mut grid = div().flex().flex_row().flex_wrap().gap_y(px(16.0));
        for entry in &snapshot.providers {
            grid = grid.child(
                div()
                    .w(relative(0.5))
                    .pr(px(8.0))
                    .child(provider_tile(entry, metric, &palette, colored)),
            );
        }
        usage_card(&palette)
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(components::split_row(
                                components::text(headline, 28.0, 36.0, palette.text_primary)
                                    .font_weight(FontWeight::SEMIBOLD),
                                metric_control,
                            ))
                            .child(caption(meta, palette.text_tertiary)),
                    )
                    .child(share_bar(&entries, |provider| {
                        palette.spend_color(provider)
                    })),
            )
            .child(grid)
            .into_any_element()
    }

    fn usage_chart_card(
        &mut self,
        snapshot: &OverviewSnapshot,
        data: Arc<UsageChartData>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let metric = self.overview_metric;
        let series = Arc::clone(&data.series);
        let hourly = snapshot.hourly;
        let title = match (hourly, metric) {
            (true, OverviewMetric::Cost) => crate::i18n::tr("hourly-cost"),
            (true, OverviewMetric::Tokens) => crate::i18n::tr("hourly-processed-tokens"),
            (false, OverviewMetric::Cost) => crate::i18n::tr("cost"),
            (false, OverviewMetric::Tokens) => crate::i18n::tr("tokens"),
        };
        let card = usage_card(&palette)
            .gap(px(6.0))
            .child(components::body_strong(title, palette.text_primary));
        if series.is_empty() {
            return card
                .child(
                    div()
                        .h(px(CHART_PLOT_HEIGHT))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(caption(
                            crate::i18n::tr("no-activity-in-this-range"),
                            palette.text_tertiary,
                        )),
                )
                .into_any_element();
        }
        let providers = Arc::clone(&data.providers);
        let max_value = data.max_value;
        // Values glide between ranges/metrics instead of snapping.
        let scale = self
            .fx
            .value(fx::key("usage-chart-scale"), max_value as f32, fx::NORMAL);
        let count = series.len();
        let hover = self.chart_hover.filter(|index| *index < count);
        let plot_bounds: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::new(Cell::new(None));
        let plot_bounds_paint = Rc::clone(&plot_bounds);
        let rule_color = palette.text_primary;
        if let Some(plot) = self.usage_plot.as_ref() {
            plot.update(cx, |plot, cx| {
                if !Arc::ptr_eq(&plot.data, &data)
                    || plot.scale != scale
                    || plot.palette.dark != palette.dark
                    || plot.palette.accent != palette.accent
                {
                    plot.data = Arc::clone(&data);
                    plot.scale = scale;
                    plot.palette = palette.clone();
                    cx.notify();
                }
            });
        } else {
            self.usage_plot = Some(cx.new(|_| UsagePlot {
                data: Arc::clone(&data),
                scale,
                palette: palette.clone(),
            }));
        }
        let mut plot_style = div().size_full();
        let plot = AnyView::from(self.usage_plot.as_ref().unwrap().clone());
        let plot = if self.cache_graph_paint() {
            plot.cached(plot_style.style().clone())
        } else {
            plot
        };
        // Hover is a tiny independent overlay, not part of the cached plot.
        let overlay = canvas(
            move |bounds, _, _| {
                plot_bounds_paint.set(Some(bounds));
            },
            move |bounds, _, window, _| {
                if let Some(index) = hover {
                    let x = chart_x_at(index, count, f32::from(bounds.size.width));
                    window.paint_quad(gpui::fill(
                        Bounds::new(
                            bounds.origin + point(px(x - 0.5), px(0.0)),
                            gpui::size(px(1.0), bounds.size.height),
                        ),
                        rule_color,
                    ));
                }
            },
        )
        .absolute()
        .inset_0();
        let hover_series = Arc::clone(&series);
        let tip_providers = providers.clone();
        let hit_bounds = Rc::clone(&plot_bounds);
        let owner = fx::key("usage-chart-tip");
        let plot_area = div()
            .id("usage-chart-plot")
            .relative()
            .flex_1()
            .h(px(CHART_PLOT_HEIGHT))
            .child(plot)
            .child(overlay)
            .on_mouse_move(
                cx.listener(move |this, event: &MouseMoveEvent, window, cx| {
                    let Some(bounds) = hit_bounds.get() else {
                        return;
                    };
                    if !bounds.contains(&event.position) {
                        return;
                    }
                    let x = f32::from(event.position.x) - f32::from(bounds.origin.x);
                    let index =
                        chart_index_at_x(x, hover_series.len(), f32::from(bounds.size.width));
                    let content = this.chart_tip(&hover_series[index], &tip_providers, hourly);
                    if this.chart_hover != Some(index) {
                        this.chart_hover = Some(index);
                    }
                    if this.tip.as_ref().is_some_and(|tip| tip.owner == owner) {
                        if let Some(tip) = this.tip.as_mut() {
                            tip.content = TipContent::Chart(content);
                        }
                        cx.notify();
                    } else {
                        this.show_tip(owner, TipContent::Chart(content), window, cx);
                    }
                }),
            )
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if !*hovered {
                    this.chart_hover = None;
                    if this.tip.as_ref().is_some_and(|tip| tip.owner == owner) {
                        this.tip = None;
                    }
                    cx.notify();
                }
            }));
        let ticks = [max_value, max_value * 2 / 3, max_value / 3, 0];
        let plot_h = CHART_PLOT_HEIGHT - CHART_PAD_TOP - CHART_PAD_BOTTOM;
        let mut y_axis = div()
            .relative()
            .w(px(CHART_Y_AXIS_WIDTH))
            .h(px(CHART_PLOT_HEIGHT))
            .flex_none();
        for (index, value) in ticks.into_iter().enumerate() {
            let y = CHART_PAD_TOP + plot_h * index as f32 / 3.0;
            let top = (y - 8.0).clamp(0.0, CHART_PLOT_HEIGHT - 16.0);
            y_axis = y_axis.child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(top))
                    .child(nowrap(caption(
                        format_axis_value(value, metric),
                        palette.text_tertiary,
                    ))),
            );
        }
        let label = |point: &DailySeriesPoint| {
            if hourly {
                crate::usage_overview::format_hour_label(point.at)
            } else {
                crate::i18n::month_day(point.date)
            }
        };
        let x_axis = div()
            .flex()
            .flex_row()
            .w_full()
            .child(div().flex_1().child(caption(
                series.first().map(label).unwrap_or_default(),
                palette.text_tertiary,
            )))
            .child(div().flex_1().flex().justify_center().child(caption(
                series.get(series.len() / 2).map(label).unwrap_or_default(),
                palette.text_tertiary,
            )))
            .child(div().flex_1().flex().justify_end().child(caption(
                series.last().map(label).unwrap_or_default(),
                palette.text_tertiary,
            )));
        card.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(CHART_Y_GAP))
                        .child(y_axis)
                        .child(plot_area),
                )
                .child(div().pl(px(CHART_Y_AXIS_WIDTH + CHART_Y_GAP)).child(x_axis)),
        )
        .into_any_element()
    }

    fn chart_tip(
        &self,
        point: &DailySeriesPoint,
        providers: &[ProviderId],
        hourly: bool,
    ) -> ChartTip {
        let metric = self.overview_metric;
        let title = if hourly {
            format!(
                "{} · {}",
                crate::i18n::month_day(point.date),
                crate::usage_overview::format_hour_label(point.at)
            )
        } else {
            crate::i18n::month_day(point.date)
        };
        let mut total_cents = 0_u64;
        let mut total_tokens = 0_u64;
        let mut rows = Vec::new();
        for provider in providers {
            let value = point.by_provider.get(provider).copied().unwrap_or(0);
            let (amount, hidden) = match metric {
                OverviewMetric::Cost => {
                    total_cents = total_cents.saturating_add(spend_display_cents(value));
                    (format_spend_tenths(value), spend_display_tenths(value) == 0)
                }
                OverviewMetric::Tokens => {
                    total_tokens = total_tokens.saturating_add(value);
                    (format_token_count(value), value == 0)
                }
            };
            if hidden {
                continue;
            }
            rows.push((
                crate::provider_registry::icon(provider.kind()),
                provider.qualified_name(),
                amount,
                self.palette.provider_icon(provider.kind(), true),
            ));
        }
        let total = match metric {
            OverviewMetric::Cost => format_spend_tenths_from_cents(total_cents),
            OverviewMetric::Tokens => format_token_count(total_tokens),
        };
        ChartTip { title, rows, total }
    }

    fn usage_breakdown_card(
        &mut self,
        snapshot: &OverviewSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let breakdown = self.overview_breakdown;
        let control = self.segmented_control_quiet(
            fx::key("usage-breakdown"),
            vec![
                crate::i18n::tr("model").into(),
                crate::i18n::tr("day").into(),
            ],
            usize::from(breakdown == BreakdownMode::Day),
            |this, index, _| {
                this.overview_breakdown = if index == 0 {
                    BreakdownMode::Model
                } else {
                    BreakdownMode::Day
                };
            },
            window,
            cx,
        );
        let colored = self.ui.use_colored_provider_icons;
        let table = match breakdown {
            BreakdownMode::Model => model_breakdown_table(&snapshot.model_rows, &palette, colored),
            BreakdownMode::Day => {
                day_breakdown_table(snapshot, self.overview_metric, &palette, colored)
            }
        };
        usage_card(&palette)
            .gap(px(14.0))
            .child(components::split_row(
                components::body_strong(crate::i18n::tr("breakdown"), palette.text_primary),
                control,
            ))
            .child(table)
            .into_any_element()
    }
}

fn range_label(snapshot: &OverviewSnapshot) -> String {
    if snapshot.hourly {
        let start = snapshot
            .daily_series
            .first()
            .map(|point| crate::usage_overview::format_hour_label(point.at))
            .unwrap_or_else(|| crate::i18n::month_day(snapshot.start_date));
        let end = snapshot
            .daily_series
            .last()
            .map(|point| crate::usage_overview::format_hour_label(point.at))
            .unwrap_or_else(|| crate::i18n::month_day(snapshot.end_date));
        crate::i18n::format(
            "start-to-end",
            &[("start", start.to_string()), ("end", end.to_string())],
        )
    } else {
        crate::i18n::format(
            "to",
            &[
                (
                    "v0",
                    (crate::i18n::month_day(snapshot.start_date)).to_string(),
                ),
                (
                    "v1",
                    (crate::i18n::month_day(snapshot.end_date)).to_string(),
                ),
            ],
        )
    }
}

fn provider_tile(
    entry: &ProviderOverview,
    metric: OverviewMetric,
    palette: &Palette,
    colored: bool,
) -> gpui::Div {
    let descriptor = crate::provider_registry::descriptor(entry.provider.kind());
    let value = match metric {
        OverviewMetric::Cost => format_usage_cost(&entry.usage),
        OverviewMetric::Tokens => format_token_count(entry.usage.total_tokens()),
    };
    let share = match metric {
        OverviewMetric::Cost => entry.share_cost,
        OverviewMetric::Tokens => entry.share_tokens,
    };
    let other = match metric {
        OverviewMetric::Cost => format_token_count(entry.usage.total_tokens()),
        OverviewMetric::Tokens => format_usage_cost(&entry.usage),
    };
    let detail = crate::i18n::format(
        "share-1-of-other",
        &[
            ("share", format!("{:.1}", share)),
            (
                "v0",
                (match metric {
                    OverviewMetric::Cost => crate::i18n::tr("cost-885dc4"),
                    OverviewMetric::Tokens => crate::i18n::tr("tokens-339143"),
                })
                .to_string(),
            ),
            ("other", other.to_string()),
        ],
    );
    div()
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
                    crate::provider_registry::icon(entry.provider.kind()),
                    16.0,
                    palette.provider_icon(entry.provider.kind(), colored),
                    entry.provider.badge().as_ref(),
                    palette,
                ))
                .child(nowrap(components::body_strong(
                    descriptor.display_name,
                    palette.text_primary,
                ))),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(1.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(4.0))
                        .child(components::caption_strong(value, palette.text_primary))
                        .child(nowrap(caption(
                            crate::i18n::format(
                                "sessions-0e5e29",
                                &[("v0", entry.sessions.to_string())],
                            ),
                            palette.text_secondary,
                        ))),
                )
                .child(nowrap(caption(detail, palette.text_tertiary))),
        )
}

fn usage_totals_card(totals: &TokenUsage, palette: &Palette) -> AnyElement {
    let metric = |label: &'static str, value: String| {
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(caption(label, palette.text_tertiary))
            .child(components::body_strong(value, palette.text_primary))
    };
    let pair = |a, b| div().flex().flex_row().gap(px(8.0)).child(a).child(b);
    let uncached = totals
        .input_tokens
        .saturating_sub(totals.cached_input_tokens);
    usage_card(palette)
        .gap(px(8.0))
        .child(components::body_strong(
            crate::i18n::tr("totals"),
            palette.text_primary,
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(pair(
                    metric(
                        crate::i18n::tr("processed-tokens"),
                        format_token_count(totals.total_tokens()),
                    ),
                    metric(
                        crate::i18n::tr("cached-input"),
                        format_token_count(totals.cached_input_tokens),
                    ),
                ))
                .child(pair(
                    metric(
                        crate::i18n::tr("uncached-input"),
                        format_token_count(uncached),
                    ),
                    metric(
                        crate::i18n::tr("output"),
                        format_token_count(totals.output_tokens),
                    ),
                ))
                .child(metric(
                    crate::i18n::tr("cache-savings"),
                    format_spend(totals.cache_savings_microusd),
                )),
        )
        .into_any_element()
}

fn cell(width: f32, content: impl IntoElement) -> gpui::Div {
    div()
        .w(px(width))
        .flex_none()
        .flex()
        .justify_end()
        .child(content)
}

fn model_breakdown_table(rows: &[BreakdownRow], palette: &Palette, colored: bool) -> gpui::Div {
    let header = div()
        .flex()
        .flex_row()
        .items_center()
        .child(
            div()
                .flex_1()
                .child(caption(crate::i18n::tr("model"), palette.text_tertiary)),
        )
        .child(cell(
            56.0,
            caption(crate::i18n::tr("cost"), palette.text_tertiary),
        ))
        .child(cell(
            44.0,
            caption(crate::i18n::tr("share"), palette.text_tertiary),
        ))
        .child(cell(
            56.0,
            caption(crate::i18n::tr("tokens"), palette.text_tertiary),
        ));
    let mut list = div().flex().flex_col().gap(px(6.0));
    for row in rows {
        let mut title = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .min_w_0();
        if let Some(provider) = row.provider {
            title = title.child(components::provider_mark(
                crate::provider_registry::icon(provider.kind()),
                14.0,
                palette.provider_icon(provider.kind(), colored),
                provider.badge().as_ref(),
                palette,
            ));
        }
        title = title.child(
            nowrap(caption(row.label.clone(), palette.text_primary))
                .ml(px(if row.provider.is_some() { 2.0 } else { 0.0 })),
        );
        if let Some(weekday) = &row.weekday {
            title = title.child(caption(weekday.clone(), palette.text_tertiary));
        }
        list = list.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .child(div().flex_1().min_w_0().child(title))
                .child(cell(
                    56.0,
                    caption(format_spend(row.cost_microusd), palette.text_primary),
                ))
                .child(cell(
                    44.0,
                    caption(format!("{:.1}%", row.share), palette.text_primary),
                ))
                .child(cell(
                    56.0,
                    caption(format_token_count(row.tokens), palette.text_primary),
                )),
        );
    }
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(header)
        .child(list)
}

fn day_breakdown_table(
    snapshot: &OverviewSnapshot,
    metric: OverviewMetric,
    palette: &Palette,
    colored: bool,
) -> gpui::Div {
    let providers: Vec<ProviderId> = snapshot
        .providers
        .iter()
        .map(|entry| entry.provider)
        .collect();
    let provider_col = match providers.len() {
        0 | 1 => 64.0,
        2 => 58.0,
        3 => 48.0,
        _ => 40.0,
    };
    let mut header =
        div()
            .flex()
            .flex_row()
            .items_center()
            .py(px(2.0))
            .child(div().flex_1().child(caption(
                if snapshot.hourly {
                    crate::i18n::tr("hour")
                } else {
                    crate::i18n::tr("day")
                },
                palette.text_tertiary,
            )));
    for provider in &providers {
        header = header.child(cell(
            provider_col,
            components::provider_mark(
                crate::provider_registry::icon(provider.kind()),
                14.0,
                palette.provider_icon(provider.kind(), colored),
                provider.badge().as_ref(),
                palette,
            ),
        ));
    }
    header = header.child(cell(
        56.0,
        caption(
            match metric {
                OverviewMetric::Cost => crate::i18n::tr("cost"),
                OverviewMetric::Tokens => crate::i18n::tr("tokens"),
            },
            palette.text_tertiary,
        ),
    ));
    let mut table = div().flex().flex_col().child(header);
    for row in &snapshot.day_rows {
        table = table.child(components::rule(palette));
        let mut date = div()
            .flex()
            .flex_row()
            .gap(px(4.0))
            .child(caption(row.label.clone(), palette.text_primary));
        if let Some(weekday) = &row.weekday {
            let weekend = weekday == crate::i18n::tr("sat") || weekday == crate::i18n::tr("sun");
            date = date.child(caption(
                weekday.clone(),
                if weekend {
                    palette.accent
                } else {
                    palette.text_tertiary
                },
            ));
        }
        let mut line = div()
            .flex()
            .flex_row()
            .items_center()
            .py(px(2.0))
            .child(div().flex_1().min_w_0().child(date));
        for provider in &providers {
            let value = row.by_provider.get(provider);
            let text = match metric {
                OverviewMetric::Cost => value
                    .map(format_usage_day_cost)
                    .unwrap_or_else(|| "$0".into()),
                OverviewMetric::Tokens => {
                    format_token_count(value.map(TokenUsage::total_tokens).unwrap_or(0))
                }
            };
            line = line.child(cell(provider_col, caption(text, palette.text_primary)));
        }
        let total = match metric {
            OverviewMetric::Cost => format_breakdown_cost(row),
            OverviewMetric::Tokens => format_token_count(row.tokens),
        };
        line = line.child(cell(56.0, caption(total, palette.text_primary)));
        table = table.child(line);
    }
    table
}

// ----- chart geometry --------------------------------------------------------------

pub(crate) fn chart_x_at(index: usize, count: usize, width: f32) -> f32 {
    let count = count.max(1);
    let plot_w = (width - CHART_PAD_X * 2.0).max(1.0);
    if count == 1 {
        return CHART_PAD_X + plot_w / 2.0;
    }
    CHART_PAD_X + plot_w * index.min(count - 1) as f32 / (count - 1) as f32
}

pub(crate) fn chart_index_at_x(x: f32, count: usize, width: f32) -> usize {
    let count = count.max(1);
    if count == 1 {
        return 0;
    }
    let plot_w = (width - CHART_PAD_X * 2.0).max(1.0);
    let t = (x - CHART_PAD_X) / plot_w * (count - 1) as f32;
    t.round().clamp(0.0, (count - 1) as f32) as usize
}

pub(crate) fn monotone_tangents(xs: &[f32], ys: &[f32]) -> Vec<f32> {
    let n = ys.len();
    let mut tangents = vec![0.0; n];
    if n < 2 {
        return tangents;
    }
    let mut slopes = vec![0.0; n - 1];
    for index in 0..n - 1 {
        let dx = (xs[index + 1] - xs[index]).max(f32::EPSILON);
        slopes[index] = (ys[index + 1] - ys[index]) / dx;
    }
    tangents[0] = slopes[0];
    tangents[n - 1] = slopes[n - 2];
    for index in 1..n - 1 {
        tangents[index] = if slopes[index - 1] * slopes[index] <= 0.0 {
            0.0
        } else {
            (slopes[index - 1] + slopes[index]) / 2.0
        };
    }
    for index in 0..n - 1 {
        if slopes[index].abs() < f32::EPSILON {
            tangents[index] = 0.0;
            tangents[index + 1] = 0.0;
            continue;
        }
        let alpha = tangents[index] / slopes[index];
        let beta = tangents[index + 1] / slopes[index];
        let sum = alpha * alpha + beta * beta;
        if sum > 9.0 {
            let scale = 3.0 / sum.sqrt();
            tangents[index] = scale * alpha * slopes[index];
            tangents[index + 1] = scale * beta * slopes[index];
        }
    }
    tangents
}

fn monotone_path(builder: &mut PathBuilder, xs: &[f32], ys: &[f32], ox: f32, oy: f32, start: bool) {
    let p = |x: f32, y: f32| point(px(ox + x), px(oy + y));
    if start {
        builder.move_to(p(xs[0], ys[0]));
    } else {
        builder.line_to(p(xs[0], ys[0]));
    }
    let tangents = monotone_tangents(xs, ys);
    for index in 0..xs.len().saturating_sub(1) {
        let dx = xs[index + 1] - xs[index];
        builder.cubic_bezier_to(
            p(xs[index + 1], ys[index + 1]),
            p(xs[index] + dx / 3.0, ys[index] + tangents[index] * dx / 3.0),
            p(
                xs[index + 1] - dx / 3.0,
                ys[index + 1] - tangents[index + 1] * dx / 3.0,
            ),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_area_chart(
    bounds: Bounds<Pixels>,
    series: &[DailySeriesPoint],
    lines: &[(ProviderId, Hsla)],
    max_value: f32,
    grid: Hsla,
    baseline_color: Hsla,
    hover: Option<(usize, Hsla)>,
    window: &mut Window,
) {
    let ox = f32::from(bounds.origin.x);
    let oy = f32::from(bounds.origin.y);
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    let plot_h = (height - CHART_PAD_TOP - CHART_PAD_BOTTOM).max(1.0);
    let baseline = height - CHART_PAD_BOTTOM;
    let hline = |y: f32, thickness: f32, color: Hsla, window: &mut Window| {
        window.paint_quad(gpui::fill(
            Bounds::new(
                point(px(ox), px(oy + y - thickness / 2.0)),
                gpui::size(px(width), px(thickness)),
            ),
            color,
        ));
    };
    for tick in 0..4 {
        hline(
            CHART_PAD_TOP + plot_h * tick as f32 / 3.0,
            1.0,
            grid,
            window,
        );
    }
    let count = series.len();
    let xs: Vec<f32> = if count == 1 {
        vec![CHART_PAD_X, width - CHART_PAD_X]
    } else {
        (0..count)
            .map(|index| chart_x_at(index, count, width))
            .collect()
    };
    let mut strokes = Vec::new();
    for (provider, color) in lines {
        let mut ys: Vec<f32> = series
            .iter()
            .map(|point| {
                let value = point.by_provider.get(provider).copied().unwrap_or(0) as f32;
                baseline - (value / max_value).min(1.2) * plot_h
            })
            .collect();
        if ys.len() == 1 {
            ys.push(ys[0]);
        }
        if ys.iter().all(|y| (*y - baseline).abs() < 0.01) {
            continue;
        }
        let mut fill = PathBuilder::fill();
        monotone_path(&mut fill, &xs, &ys, ox, oy, true);
        fill.line_to(point(px(ox + xs[xs.len() - 1]), px(oy + baseline)));
        fill.line_to(point(px(ox + xs[0]), px(oy + baseline)));
        fill.close();
        if let Ok(path) = fill.build() {
            window.paint_path(path, color.opacity(0.2));
        }
        strokes.push((ys, *color));
    }
    for (ys, color) in strokes {
        let mut stroke = PathBuilder::stroke(px(2.0));
        monotone_path(&mut stroke, &xs, &ys, ox, oy, true);
        if let Ok(path) = stroke.build() {
            window.paint_path(path, color);
        }
    }
    hline(baseline, 1.25, baseline_color, window);
    if let Some((index, color)) = hover {
        let x = chart_x_at(index, count, width);
        window.paint_quad(gpui::fill(
            Bounds::new(
                point(px(ox + x - 0.5), px(oy)),
                gpui::size(px(1.0), px(height)),
            ),
            color,
        ));
    }
}

fn paint_spinner(bounds: Bounds<Pixels>, angle: f32, color: Hsla, window: &mut Window) {
    let cx = f32::from(bounds.origin.x) + f32::from(bounds.size.width) / 2.0;
    let cy = f32::from(bounds.origin.y) + f32::from(bounds.size.height) / 2.0;
    let radius = f32::from(bounds.size.width) / 2.0 - 1.5;
    let at = |a: f32| point(px(cx + radius * a.cos()), px(cy + radius * a.sin()));
    let mut builder = PathBuilder::stroke(px(2.0));
    builder.move_to(at(angle));
    builder.arc_to(
        point(px(radius), px(radius)),
        px(0.0),
        true,
        true,
        at(angle + PI * 1.5),
    );
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

fn fill_daily_series(
    series: &[DailySeriesPoint],
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Vec<DailySeriesPoint> {
    if series.is_empty() || start_date > end_date {
        return Vec::new();
    }
    let mut by_date = BTreeMap::new();
    for point in series {
        by_date.insert(point.date, point.clone());
    }
    let mut filled = Vec::new();
    let mut day = start_date;
    while day <= end_date {
        filled.push(by_date.remove(&day).unwrap_or(DailySeriesPoint {
            at: start_of_local_day(day),
            date: day,
            by_provider: BTreeMap::new(),
            total: 0,
        }));
        day += Duration::days(1);
    }
    filled
}

fn start_of_local_day(date: NaiveDate) -> DateTime<Local> {
    use chrono::TimeZone;
    date.and_hms_opt(0, 0, 0)
        .and_then(|naive| Local.from_local_datetime(&naive).single())
        .unwrap_or_else(Local::now)
}

fn chart_scale_max(raw_max: u64) -> u64 {
    if raw_max == 0 {
        return 1;
    }
    (raw_max as f64 * 1.10).ceil().max(1.0) as u64
}

fn format_axis_value(value: u64, metric: OverviewMetric) -> String {
    match metric {
        OverviewMetric::Cost => format_spend(value),
        OverviewMetric::Tokens => format_token_count(value),
    }
}

fn format_spend(microusd: u64) -> String {
    format_spend_dollars((microusd as f64 / 1_000_000.0).round() as u64)
}

fn format_total_cost(usage: &TokenUsage) -> String {
    if usage.requests > 0 && usage.priced_requests == 0 {
        "—".into()
    } else {
        format_spend_full(usage.estimated_cost_microusd)
    }
}

fn format_usage_cost(usage: &TokenUsage) -> String {
    if usage.requests > 0 && usage.priced_requests == 0 {
        "—".into()
    } else {
        format_spend(usage.estimated_cost_microusd)
    }
}

fn format_usage_day_cost(usage: &TokenUsage) -> String {
    if usage.requests > 0 && usage.priced_requests == 0 {
        "—".into()
    } else {
        format_day_cost(usage.estimated_cost_microusd)
    }
}

fn format_breakdown_cost(row: &BreakdownRow) -> String {
    if row.requests > 0 && row.priced_requests == 0 {
        "—".into()
    } else {
        format_spend(row.cost_microusd)
    }
}

fn format_day_cost(microusd: u64) -> String {
    let cents = spend_display_cents(microusd);
    if cents == 0 {
        return "$0".into();
    }
    if cents >= 100_000 {
        return format_spend(microusd);
    }
    format!("${}.{:02}", cents / 100, cents % 100)
}

fn spend_display_cents(microusd: u64) -> u64 {
    (microusd as f64 / 10_000.0)
        .round()
        .clamp(0.0, u64::MAX as f64) as u64
}

fn spend_display_tenths(microusd: u64) -> u64 {
    (microusd as f64 / 100_000.0)
        .round()
        .clamp(0.0, u64::MAX as f64) as u64
}

fn format_spend_tenths(microusd: u64) -> String {
    format_spend_tenths_value(spend_display_tenths(microusd), microusd)
}

fn format_spend_tenths_from_cents(cents: u64) -> String {
    format_spend_tenths_value(
        ((cents as f64) / 10.0).round().clamp(0.0, u64::MAX as f64) as u64,
        cents.saturating_mul(10_000),
    )
}

fn format_spend_tenths_value(tenths: u64, microusd: u64) -> String {
    if tenths >= 10_000 {
        return format_spend(microusd);
    }
    format!("${:.1}", tenths as f64 / 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart_snapshot() -> Arc<OverviewSnapshot> {
        let date = NaiveDate::from_ymd_opt(2026, 10, 6).unwrap();
        Arc::new(OverviewSnapshot {
            start_date: date - Duration::days(2),
            end_date: date,
            providers: vec![ProviderOverview {
                provider: crate::instances::ProviderId::from(ProviderKind::Codex),
                ..Default::default()
            }],
            daily_series: vec![DailySeriesPoint {
                at: start_of_local_day(date),
                date,
                by_provider: BTreeMap::from([(ProviderKind::Codex.into(), 100)]),
                total: 100,
            }],
            ..Default::default()
        })
    }

    #[test]
    fn chart_cache_reuses_data_until_snapshot_or_metric_changes() {
        let snapshot = chart_snapshot();
        let cache = UsageChartCache::new(Arc::clone(&snapshot), OverviewMetric::Cost);
        let prepared = Arc::clone(&cache.data);
        for _ in 0..120 {
            assert!(cache.matches(&snapshot, OverviewMetric::Cost));
            assert!(Arc::ptr_eq(&cache.data, &prepared));
        }
        assert!(!cache.matches(&snapshot, OverviewMetric::Tokens));
        let mut updated = (*snapshot).clone();
        updated.daily_series[0]
            .by_provider
            .insert(crate::instances::ProviderId::from(ProviderKind::Codex), 200);
        let updated = Arc::new(updated);
        assert!(!cache.matches(&updated, OverviewMetric::Cost));
        let next = UsageChartCache::new(updated, OverviewMetric::Cost);
        assert!(next.data.max_value > cache.data.max_value);
    }

    #[test]
    fn chart_cache_fills_missing_days_once_and_does_not_cycle_ownership() {
        let snapshot = chart_snapshot();
        let cache = UsageChartCache::new(Arc::clone(&snapshot), OverviewMetric::Cost);
        assert_eq!(cache.data.series.len(), 3);
        assert_eq!(cache.data.series[0].total, 0);
        assert_eq!(cache.data.series[2].total, 100);
        let data = Arc::downgrade(&cache.data);
        let source = Arc::downgrade(&snapshot);
        drop(cache);
        drop(snapshot);
        assert!(data.upgrade().is_none());
        assert!(source.upgrade().is_none());
    }

    #[test]
    fn hourly_chart_cache_preserves_hours_instead_of_filling_calendar_days() {
        let mut snapshot = (*chart_snapshot()).clone();
        snapshot.hourly = true;
        let cache = UsageChartCache::new(Arc::new(snapshot), OverviewMetric::Tokens);
        assert_eq!(cache.data.series.len(), 1);
    }

    #[test]
    fn hover_index_lands_on_painted_vertices() {
        for count in [1, 2, 7, 24, 90] {
            for index in 0..count {
                let x = chart_x_at(index, count, 274.0);
                assert_eq!(chart_index_at_x(x, count, 274.0), index);
            }
        }
    }

    #[test]
    fn monotone_tangents_never_overshoot_a_plateau() {
        let xs = [0.0, 1.0, 2.0, 3.0];
        let ys = [0.0, 10.0, 10.0, 0.0];
        let tangents = monotone_tangents(&xs, &ys);
        assert_eq!(tangents[1], 0.0);
        assert_eq!(tangents[2], 0.0);
    }

    #[test]
    fn day_costs_keep_cents_until_a_thousand_dollars() {
        assert_eq!(format_day_cost(0), "$0");
        assert_eq!(format_day_cost(1_234_567), "$1.23");
        assert_eq!(format_day_cost(1_500_000_000), "$1.5K");
    }
}
