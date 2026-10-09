//! Usage tab: local API usage overview across providers.

use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    f32::consts::PI,
    rc::Rc,
    sync::Arc,
    time::Instant,
};

use chrono::{DateTime, Duration, Local, NaiveDate};
use gpui::{
    AnyElement, AnyView, AppContext, Bounds, ClickEvent, Context, FontWeight, Hsla,
    InteractiveElement, IntoElement, MouseMoveEvent, ParentElement, PathBuilder, Pixels, Render,
    SharedString, StatefulInteractiveElement, Styled, Window, canvas, div, point, px, relative,
};

use super::{
    components::{self, caption, card, nowrap},
    fx,
    home::{format_spend_full, format_thousands},
    root::{PopupRoot, SnapshotSlot, eid},
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
    hourly: bool,
    end_date: NaiveDate,
    metric: OverviewMetric,
}

impl UsageChartData {
    /// Daily charts of the same metric ending on the same day differ only in
    /// how far back they reach, so the switch pans the time window instead
    /// of morphing values.
    fn pans_from(&self, previous: &Self) -> bool {
        !self.hourly
            && !previous.hourly
            && self.end_date == previous.end_date
            && self.metric == previous.metric
            && self.series.len() != previous.series.len()
            && self.series.len() > 1
            && previous.series.len() > 1
    }
}

/// How the plot moves from the previously shown chart into the current one.
#[derive(Clone, Copy)]
enum ChartMotion<'a> {
    Settled,
    /// Values blend from `from` (sampled at the same relative position).
    Morph {
        from: &'a [DailySeriesPoint],
        progress: f32,
    },
    /// The painted series is right-aligned to the window end and the window
    /// spans `span` days, so days slide in or out at the left edge.
    Pan {
        span: f32,
    },
}

/// The page snapshot with the Usage tab's provider toggles applied, kept
/// until the source snapshot, the toggles or the metric change.
pub(super) struct UsageFilterCache {
    source: Arc<OverviewSnapshot>,
    excluded: BTreeSet<ProviderId>,
    metric: OverviewMetric,
    value: Arc<OverviewSnapshot>,
}

pub(super) struct UsageChartCache {
    source: Arc<OverviewSnapshot>,
    metric: OverviewMetric,
    data: Arc<UsageChartData>,
    /// Chart shown before the last period/metric/data change; the plot morphs
    /// from it into `data`.
    previous: Option<Arc<UsageChartData>>,
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
            .filter(|entry| !entry.excluded)
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
        let (hourly, end_date) = (source.hourly, source.end_date);
        Self {
            source,
            metric,
            data: Arc::new(UsageChartData {
                series: Arc::new(series),
                providers: Arc::new(providers),
                max_value: chart_scale_max(raw_max),
                hourly,
                end_date,
                metric,
            }),
            previous: None,
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
    previous: Option<Arc<UsageChartData>>,
    /// Morph progress from `previous` to `data` (1.0 = settled).
    progress: f32,
    scale: f32,
    palette: Palette,
}

impl Render for UsagePlot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let data = Arc::clone(&self.data);
        let previous = self
            .previous
            .clone()
            .filter(|_| self.progress < 1.0)
            .map(|previous| (previous, self.progress));
        let scale = self.scale;
        let palette = self.palette.clone();
        // Providers that left the chart keep painting while they sink away.
        let mut providers = data.providers.to_vec();
        if let Some((previous, _)) = &previous {
            for provider in previous.providers.iter() {
                if !providers.contains(provider) {
                    providers.push(*provider);
                }
            }
        }
        let lines: Vec<_> = providers
            .into_iter()
            .map(|provider| (provider, palette.series_color(provider)))
            .collect();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let (series, motion) = match &previous {
                    None => (data.series.as_slice(), ChartMotion::Settled),
                    Some((previous, progress)) if data.pans_from(previous) => {
                        // Paint the longer series so the whole window stays
                        // covered; overlapping days hold the same values.
                        let series = if previous.series.len() > data.series.len() {
                            previous.series.as_slice()
                        } else {
                            data.series.as_slice()
                        };
                        // Interpolate in log space so zooming feels uniform.
                        let from = (previous.series.len() - 1) as f32;
                        let to = (data.series.len() - 1) as f32;
                        let span = (from.ln() + (to.ln() - from.ln()) * progress).exp();
                        (series, ChartMotion::Pan { span })
                    }
                    Some((previous, progress)) => (
                        data.series.as_slice(),
                        ChartMotion::Morph {
                            from: previous.series.as_slice(),
                            progress: *progress,
                        },
                    ),
                };
                paint_area_chart(
                    bounds,
                    series,
                    motion,
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
        let enabled_count = enabled.len();
        let key = format!(
            "overview|{}|{:?}|{}|{}|{:?}|{:?}|{}",
            self.ui.usage_revision,
            enabled,
            crate::store::codex_accounts::cached_current_id(),
            crate::usage::truncate_local_hour(Local::now()),
            metric,
            range,
            crate::i18n::current_language().index()
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
        if snapshot.providers.is_empty() && recalculating && enabled_count > 0 {
            return self.usage_skeleton(enabled_count, window, cx);
        }
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
        let snapshot = self.usage_filtered_snapshot(snapshot, metric);
        let range_label = range_label(&snapshot);
        if self
            .usage_chart_cache
            .as_ref()
            .is_none_or(|cache| !cache.matches(&snapshot, metric))
        {
            let mut cache = UsageChartCache::new(Arc::clone(&snapshot), metric);
            cache.previous = self
                .usage_chart_cache
                .take()
                .map(|previous| previous.data)
                .filter(|previous| !previous.series.is_empty());
            // Restart the morph; with animations off it settles immediately.
            self.fx.snap(fx::key("usage-chart-morph"), 0.0);
            self.usage_chart_cache = Some(cache);
        }
        let cache = self.usage_chart_cache.as_ref().unwrap();
        let chart_data = Arc::clone(&cache.data);
        let chart_previous = cache.previous.clone();
        let header = self.usage_header(Some(&range_label), recalculating, window, cx);
        let hero = self.usage_hero(&snapshot, window, cx);
        let chart = self.usage_chart_card(&snapshot, chart_data, chart_previous, window, cx);
        let totals = self.usage_totals_values(&snapshot.totals, window);
        let totals = usage_totals_card(Some(totals), &palette, palette.subtle_fill);
        let breakdown = self.usage_breakdown_card(&snapshot, window, cx);
        self.usage_layout(header, hero, chart, totals, breakdown)
    }

    /// Applies the provider toggles synchronously so a click is instant; the
    /// store-backed snapshot itself stays shared and unfiltered.
    fn usage_filtered_snapshot(
        &mut self,
        source: Arc<OverviewSnapshot>,
        metric: OverviewMetric,
    ) -> Arc<OverviewSnapshot> {
        if self.usage_excluded.is_empty() {
            self.usage_filtered = None;
            return source;
        }
        if let Some(cache) = self.usage_filtered.as_ref()
            && Arc::ptr_eq(&cache.source, &source)
            && cache.excluded == self.usage_excluded
            && cache.metric == metric
        {
            return Arc::clone(&cache.value);
        }
        let value = Arc::new(crate::usage_overview::exclude_providers(
            &source,
            &self.usage_excluded,
            metric,
        ));
        self.usage_filtered = Some(UsageFilterCache {
            source,
            excluded: self.usage_excluded.clone(),
            metric,
            value: Arc::clone(&value),
        });
        value
    }

    fn usage_layout(
        &self,
        header: AnyElement,
        hero: AnyElement,
        chart: AnyElement,
        totals: AnyElement,
        breakdown: AnyElement,
    ) -> AnyElement {
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

    /// Pulsing placeholder color for loading bones. Static when animations
    /// are off.
    fn usage_bone(&mut self) -> Hsla {
        let base = self.palette.text_primary.opacity(0.08);
        if !super::animations_enabled(&self.ui) {
            return base;
        }
        // Shares the header spinner's clock: both run only while loading.
        let started = *self.usage_spinner_started.get_or_insert_with(Instant::now);
        let phase = started.elapsed().as_secs_f32() * 2.0 * PI / 1.6;
        self.fx.mark_animating();
        base.opacity(0.65 + 0.35 * phase.cos())
    }

    /// First load: the real header, controls and labels with bones in place
    /// of the data, laid out like the loaded page so nothing jumps.
    fn usage_skeleton(
        &mut self,
        providers: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let header = self.usage_header(None, true, window, cx);
        let bone = self.usage_bone();
        let metric_control = self.usage_metric_control(window, cx);
        let mut grid = div().flex().flex_row().flex_wrap().gap_y(px(16.0));
        for _ in 0..providers {
            grid = grid.child(
                div()
                    .w(relative(0.5))
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
                            .h(px(20.0))
                            .child(bone_block(bone, px(16.0), 16.0, 4.0))
                            .child(bone_block(bone, px(72.0), 14.0, 4.0)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(1.0))
                            .child(bone_line(bone, px(96.0), 16.0, 12.0))
                            .child(bone_line(bone, px(124.0), 16.0, 12.0)),
                    ),
            );
        }
        let hero = usage_card(&palette)
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
                                bone_line(bone, px(128.0), 36.0, 28.0),
                                metric_control,
                            ))
                            .child(bone_line(bone, px(148.0), 16.0, 12.0)),
                    )
                    .child(bone_block(bone, relative(1.0), 10.0, 4.0)),
            )
            .child(grid)
            .into_any_element();

        let hourly = self.overview_range == OverviewRange::Past24h;
        let mut y_axis = div()
            .w(px(CHART_Y_AXIS_WIDTH))
            .h(px(CHART_PLOT_HEIGHT))
            .flex_none()
            .flex()
            .flex_col()
            .justify_between();
        for _ in 0..4 {
            y_axis = y_axis.child(bone_line(bone, px(28.0), 16.0, 10.0));
        }
        let x_axis = div()
            .flex()
            .flex_row()
            .justify_between()
            .w_full()
            .children((0..3).map(|_| bone_line(bone, px(40.0), 16.0, 10.0)));
        let chart = usage_card(&palette)
            .gap(px(6.0))
            .child(components::body_strong(
                chart_title(hourly, self.overview_metric),
                palette.text_primary,
            ))
            .child(
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
                            .child(
                                div()
                                    .flex_1()
                                    .h(px(CHART_PLOT_HEIGHT))
                                    .rounded(px(6.0))
                                    .bg(bone),
                            ),
                    )
                    .child(div().pl(px(CHART_Y_AXIS_WIDTH + CHART_Y_GAP)).child(x_axis)),
            )
            .into_any_element();

        let totals = usage_totals_card(None, &palette, bone);

        let breakdown_control = self.usage_breakdown_control(window, cx);
        let mut rows = div().flex().flex_col().gap(px(6.0));
        for width in [0.62, 0.48, 0.55, 0.4, 0.5] {
            rows = rows.child(components::split_row(
                bone_line(bone, relative(width), 20.0, 14.0),
                bone_line(bone, px(96.0), 20.0, 12.0),
            ));
        }
        let breakdown = usage_card(&palette)
            .gap(px(14.0))
            .child(components::split_row(
                components::body_strong(crate::i18n::tr("breakdown"), palette.text_primary),
                breakdown_control,
            ))
            .child(rows)
            .into_any_element();

        self.usage_layout(header, hero, chart, totals, breakdown)
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
        range_label: Option<&str>,
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
        let mut title = self.usage_title(range_label, recalculating);
        if range_label.is_none() {
            let bone = self.usage_bone();
            title = title.child(bone_line(bone, px(108.0), 20.0, 12.0));
        }
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_x(px(8.0))
            .child(div().flex_1().min_w_0().child(title))
            .child(div().flex_none().ml_auto().child(range_control))
            .into_any_element()
    }

    /// Cost/Tokens only changes how the numbers read, so it sits beside
    /// the headline it switches, quieter than the range control.
    fn usage_metric_control(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let metric = self.overview_metric;
        self.segmented_control_quiet(
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
        )
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
        let metric_control = self.usage_metric_control(window, cx);
        let headline_value = match metric {
            OverviewMetric::Cost => snapshot.totals.estimated_cost_microusd,
            OverviewMetric::Tokens => snapshot.totals.total_tokens(),
        };
        let headline = self.rolling_label(
            fx::key(("usage-headline", metric as u8)),
            headline,
            headline_value,
            (28.0, 36.0, FontWeight::SEMIBOLD, palette.text_primary),
            window,
        );
        let meta = self.rolling_label(
            fx::key(("usage-meta", metric as u8)),
            meta,
            snapshot.total_sessions,
            (12.0, 16.0, FontWeight::NORMAL, palette.text_tertiary),
            window,
        );
        // Excluded providers shrink out of the bar instead of vanishing.
        let entries: Vec<(ProviderId, u64)> = snapshot
            .providers
            .iter()
            .map(|entry| {
                let value = match metric {
                    OverviewMetric::Cost => entry.usage.estimated_cost_microusd,
                    OverviewMetric::Tokens => entry.usage.total_tokens(),
                };
                (entry.provider, if entry.excluded { 0 } else { value })
            })
            .collect();
        let shown_total = entries
            .iter()
            .fold(0_u64, |sum, (_, value)| sum.saturating_add(*value));
        let bar = self.share_bar("usage-share", &entries, shown_total);
        let colored = self.ui.use_colored_provider_icons;
        // Tiles carry 6px of hover padding; the grid bleeds it back out so
        // the text stays aligned with the headline. Tiles sit on animated
        // cells so a new ranking slides each provider to its new place.
        const TILE_HEIGHT: f32 = 69.0;
        const ROW_PITCH: f32 = TILE_HEIGHT + 4.0;
        let rows = snapshot.providers.len().div_ceil(2) as f32;
        let height = self.fx.value(
            fx::key("usage-tiles-height"),
            (rows * ROW_PITCH - 4.0).max(0.0),
            fx::SETTLE,
        );
        let mut grid = div().mx(px(-6.0)).relative().h(px(height));
        for (index, entry) in snapshot.providers.iter().enumerate() {
            let column = self.fx.value(
                fx::key(("usage-tile-column", entry.provider)),
                (index % 2) as f32,
                fx::SETTLE,
            );
            let row = self.fx.value(
                fx::key(("usage-tile-row", entry.provider)),
                (index / 2) as f32,
                fx::SETTLE,
            );
            let tile = self.usage_provider_tile(entry, metric, colored, window, cx);
            grid = grid.child(
                div()
                    .absolute()
                    .left(relative(column * 0.5))
                    .top(px(row * ROW_PITCH))
                    .w(relative(0.5))
                    .pr(px(2.0))
                    .child(tile),
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
                            .child(components::split_row(headline, metric_control))
                            .child(meta),
                    )
                    .child(bar),
            )
            .child(grid)
            .into_any_element()
    }

    /// A provider tile doubles as its stats toggle: clicking it leaves the
    /// provider out of every number on the page (or brings it back).
    fn usage_provider_tile(
        &mut self,
        entry: &ProviderOverview,
        metric: OverviewMetric,
        colored: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let provider = entry.provider;
        let labels = self.provider_tile_labels(entry, metric, window);
        let hover_id = fx::key(("usage-tile-hover", provider));
        let shown = self.fx.toggle(
            fx::key(("usage-tile-shown", provider)),
            !entry.excluded,
            fx::FAST,
        );
        let hover = self.fx.toggle(
            fx::key(("usage-tile-hover-fx", provider)),
            self.hovered(hover_id),
            fx::FASTER,
        );
        let tip = SharedString::from(if entry.excluded {
            crate::i18n::tr("include-in-usage-stats")
        } else {
            crate::i18n::tr("exclude-from-usage-stats")
        });
        let mut tile = div()
            .id(eid(format!("usage-tile-{provider:?}")))
            .relative()
            .p(px(6.0))
            .rounded(px(palette.control_radius))
            .cursor_pointer()
            .on_hover(self.hover_listener(hover_id, Some(tip), cx))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if !this.usage_excluded.remove(&provider) {
                    this.usage_excluded.insert(provider);
                }
                this.chart_hover = None;
                this.tip = None;
                cx.notify();
            }));
        if let Some(layer) = components::hover_layer(&palette, hover, palette.control_radius) {
            tile = tile.child(layer);
        }
        tile.child(provider_tile(entry, labels, &palette, colored).opacity(0.4 + 0.6 * shown))
            .into_any_element()
    }

    /// A tile's value, session count and share line, each rolling its
    /// digits when the range or the metric changes.
    fn provider_tile_labels(
        &mut self,
        entry: &ProviderOverview,
        metric: OverviewMetric,
        window: &Window,
    ) -> [gpui::Div; 3] {
        let palette = self.palette.clone();
        let provider = entry.provider;
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
        let detail = if entry.excluded {
            crate::i18n::format("excluded-other", &[("other", other.to_string())])
        } else {
            crate::i18n::format(
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
            )
        };
        let raw = match metric {
            OverviewMetric::Cost => entry.usage.estimated_cost_microusd,
            OverviewMetric::Tokens => entry.usage.total_tokens(),
        };
        let style = |weight, color| (12.0, 16.0, weight, color);
        [
            self.rolling_label(
                fx::key(("usage-tile-value", provider, metric as u8)),
                value,
                raw,
                style(FontWeight::SEMIBOLD, palette.text_primary),
                window,
            ),
            self.rolling_label(
                fx::key(("usage-tile-sessions", provider)),
                crate::i18n::format("sessions-0e5e29", &[("v0", entry.sessions.to_string())]),
                entry.sessions,
                style(FontWeight::NORMAL, palette.text_secondary),
                window,
            ),
            self.rolling_label(
                fx::key(("usage-tile-detail", provider, metric as u8)),
                detail,
                (share * 10.0).round().max(0.0) as u64,
                style(FontWeight::NORMAL, palette.text_tertiary),
                window,
            ),
        ]
    }

    /// The Totals card's values, each rolling its digits on a range change.
    fn usage_totals_values(&mut self, totals: &TokenUsage, window: &Window) -> [gpui::Div; 5] {
        let style = (14.0, 20.0, FontWeight::SEMIBOLD, self.palette.text_primary);
        let uncached = totals
            .input_tokens
            .saturating_sub(totals.cached_input_tokens);
        [
            ("processed", totals.total_tokens(), false),
            ("cached", totals.cached_input_tokens, false),
            ("uncached", uncached, false),
            ("output", totals.output_tokens, false),
            ("savings", totals.cache_savings_microusd, true),
        ]
        .map(|(id, value, money)| {
            let text = if money {
                format_spend(value)
            } else {
                format_token_count(value)
            };
            self.rolling_label(fx::key(("usage-totals", id)), text, value, style, window)
        })
    }

    fn usage_chart_card(
        &mut self,
        snapshot: &OverviewSnapshot,
        data: Arc<UsageChartData>,
        previous: Option<Arc<UsageChartData>>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let metric = self.overview_metric;
        let series = Arc::clone(&data.series);
        let hourly = snapshot.hourly;
        let title = chart_title(hourly, metric);
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
        // Period/metric switches morph the curves out of the previous chart.
        let progress = if previous.is_some() {
            self.fx.value(fx::key("usage-chart-morph"), 1.0, fx::NORMAL)
        } else {
            1.0
        };
        let count = series.len();
        let hover = self.chart_hover.filter(|index| *index < count);
        let plot_bounds: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::new(Cell::new(None));
        let plot_bounds_paint = Rc::clone(&plot_bounds);
        let rule_color = palette.text_primary;
        if let Some(plot) = self.usage_plot.as_ref() {
            plot.update(cx, |plot, cx| {
                let same_previous = match (&plot.previous, &previous) {
                    (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                    (None, None) => true,
                    _ => false,
                };
                if !Arc::ptr_eq(&plot.data, &data)
                    || !same_previous
                    || plot.progress != progress
                    || plot.scale != scale
                    || plot.palette.dark != palette.dark
                    || plot.palette.accent != palette.accent
                {
                    plot.data = Arc::clone(&data);
                    plot.previous = previous.clone();
                    plot.progress = progress;
                    plot.scale = scale;
                    plot.palette = palette.clone();
                    cx.notify();
                }
            });
        } else {
            self.usage_plot = Some(cx.new(|_| UsagePlot {
                data: Arc::clone(&data),
                previous: previous.clone(),
                progress,
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
        let axis_width = ticks
            .iter()
            .map(|value| {
                components::measure_text(
                    window.text_system(),
                    palette.font_family.clone(),
                    12.0,
                    FontWeight::NORMAL,
                    &format_axis_value(*value, metric),
                )
            })
            .fold(CHART_Y_AXIS_WIDTH, f32::max)
            .ceil();
        let plot_h = CHART_PLOT_HEIGHT - CHART_PAD_TOP - CHART_PAD_BOTTOM;
        let mut y_axis = div()
            .relative()
            .w(px(axis_width))
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
                .child(div().pl(px(axis_width + CHART_Y_GAP)).child(x_axis)),
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
                    (format_spend_full(value), spend_display_cents(value) == 0)
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
            OverviewMetric::Cost => format_spend_from_cents(total_cents),
            OverviewMetric::Tokens => format_token_count(total_tokens),
        };
        ChartTip { title, rows, total }
    }

    fn usage_breakdown_control(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let breakdown = self.overview_breakdown;
        self.segmented_control_quiet(
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
        )
    }

    fn usage_breakdown_card(
        &mut self,
        snapshot: &OverviewSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let breakdown = self.overview_breakdown;
        let control = self.usage_breakdown_control(window, cx);
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
    [value, sessions, detail]: [gpui::Div; 3],
    palette: &Palette,
    colored: bool,
) -> gpui::Div {
    let descriptor = crate::provider_registry::descriptor(entry.provider.kind());
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
                        .child(value.flex_none())
                        .child(nowrap(sessions)),
                )
                .child(nowrap(detail)),
        )
}

/// `None` renders the loading skeleton with the same labels and row heights.
/// Values come in display order: processed, cached input, uncached input,
/// output and cache savings.
fn usage_totals_card(values: Option<[gpui::Div; 5]>, palette: &Palette, bone: Hsla) -> AnyElement {
    let metric = |label: &'static str, value: Option<gpui::Div>| {
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(caption(label, palette.text_tertiary))
            .child(value.unwrap_or_else(|| bone_line(bone, px(64.0), 20.0, 14.0)))
    };
    let [processed, cached, uncached, output, savings] = match values {
        Some(values) => values.map(Some),
        None => [None, None, None, None, None],
    };
    let pair = |a, b| div().flex().flex_row().gap(px(8.0)).child(a).child(b);
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
                    metric(crate::i18n::tr("processed-tokens"), processed),
                    metric(crate::i18n::tr("cached-input"), cached),
                ))
                .child(pair(
                    metric(crate::i18n::tr("uncached-input"), uncached),
                    metric(crate::i18n::tr("output"), output),
                ))
                .child(metric(crate::i18n::tr("cache-savings"), savings)),
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
        .filter(|entry| !entry.excluded)
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
    motion: ChartMotion<'_>,
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
    let xs: Vec<f32> = if let ChartMotion::Pan { span } = motion {
        let plot_w = (width - CHART_PAD_X * 2.0).max(1.0);
        (0..count)
            .map(|index| {
                let days_back = (count - 1 - index) as f32;
                CHART_PAD_X + plot_w * (1.0 - days_back / span.max(1.0))
            })
            .collect()
    } else if count == 1 {
        vec![CHART_PAD_X, width - CHART_PAD_X]
    } else {
        (0..count)
            .map(|index| chart_x_at(index, count, width))
            .collect()
    };
    // Panned days outside the window must not spill over the y-axis.
    window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
        paint_curves(
            series, motion, lines, &xs, max_value, plot_h, baseline, ox, oy, window,
        );
    });
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

#[allow(clippy::too_many_arguments)]
fn paint_curves(
    series: &[DailySeriesPoint],
    motion: ChartMotion<'_>,
    lines: &[(ProviderId, Hsla)],
    xs: &[f32],
    max_value: f32,
    plot_h: f32,
    baseline: f32,
    ox: f32,
    oy: f32,
    window: &mut Window,
) {
    let count = series.len();
    let mut strokes = Vec::new();
    for (provider, color) in lines {
        let mut ys: Vec<f32> = series
            .iter()
            .enumerate()
            .map(|(index, point)| {
                let mut value = point.by_provider.get(provider).copied().unwrap_or(0) as f32;
                if let ChartMotion::Morph {
                    from: previous,
                    progress,
                } = motion
                {
                    let t = if count > 1 {
                        index as f32 / (count - 1) as f32
                    } else {
                        0.5
                    };
                    let from = sample_series(previous, *provider, t);
                    value = from + (value - from) * progress;
                }
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
        monotone_path(&mut fill, xs, &ys, ox, oy, true);
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
        monotone_path(&mut stroke, xs, &ys, ox, oy, true);
        if let Ok(path) = stroke.build() {
            window.paint_path(path, color);
        }
    }
}

/// Value of `provider` at normalized position `t` (0..=1) along `series`,
/// linearly interpolated between neighbouring points.
fn sample_series(series: &[DailySeriesPoint], provider: ProviderId, t: f32) -> f32 {
    let value = |index: usize| {
        series
            .get(index)
            .and_then(|point| point.by_provider.get(&provider).copied())
            .unwrap_or(0) as f32
    };
    if series.len() < 2 {
        return value(0);
    }
    let position = t.clamp(0.0, 1.0) * (series.len() - 1) as f32;
    let index = (position.floor() as usize).min(series.len() - 2);
    let fraction = position - index as f32;
    value(index) + (value(index + 1) - value(index)) * fraction
}

fn chart_title(hourly: bool, metric: OverviewMetric) -> &'static str {
    match (hourly, metric) {
        (true, OverviewMetric::Cost) => crate::i18n::tr("hourly-cost"),
        (true, OverviewMetric::Tokens) => crate::i18n::tr("hourly-processed-tokens"),
        (false, OverviewMetric::Cost) => crate::i18n::tr("cost"),
        (false, OverviewMetric::Tokens) => crate::i18n::tr("tokens"),
    }
}

/// Loading placeholder block.
fn bone_block(
    color: Hsla,
    width: impl Into<gpui::Length> + Clone,
    height: f32,
    radius: f32,
) -> gpui::Div {
    div()
        .flex_none()
        .w(width)
        .h(px(height))
        .rounded(px(radius))
        .bg(color)
}

/// A bone sized like a glyph run, vertically centered in a text line box so
/// it occupies the same height as the text it stands in for.
fn bone_line(
    color: Hsla,
    width: impl Into<gpui::Length> + Clone,
    line: f32,
    glyph: f32,
) -> gpui::Div {
    div()
        .h(px(line))
        .flex()
        .items_center()
        .child(bone_block(color, width, glyph, 4.0))
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
    format_spend_full(microusd)
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
    format_spend_full(microusd)
}

fn spend_display_cents(microusd: u64) -> u64 {
    microusd / 10_000 + u64::from(microusd % 10_000 >= 5_000)
}

fn format_spend_from_cents(cents: u64) -> String {
    format!("${}.{:02}", format_thousands(cents / 100), cents % 100)
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
    fn day_costs_keep_full_dollars_and_cents() {
        assert_eq!(format_day_cost(0), "$0.00");
        assert_eq!(format_day_cost(1_234_567), "$1.23");
        assert_eq!(format_day_cost(1_500_000_000), "$1,500.00");
    }
}
