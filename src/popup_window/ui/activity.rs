//! Per-provider usage activity card: metrics, daily stacked bars, legend.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use chrono::NaiveDate;
use gpui::{
    AnyElement, AnyView, AppContext, ClickEvent, Context, Entity, Hsla, InteractiveElement,
    IntoElement, ParentElement, Render, ScrollWheelEvent, SharedString, StatefulInteractiveElement,
    Styled, Task, Window, div, px,
};

use super::{
    components::{self, caption, card, nowrap},
    fx,
    root::{PopupRoot, eid},
    theme::{HslaExt, Palette},
    tooltip::{ActivityTip, TipContent},
};
use crate::popup_window::*;
use crate::usage::{DailyTokenUsage, TokenUsage, UsageStatistics};

const HEIGHT: f32 = 56.0;
const MAX_BARS: usize = 60;
const CHART_WIDTH: f32 = crate::popup::POPUP_WIDTH as f32 - 2.0 - 32.0 - 2.0 - 24.0;
pub(super) const MODEL_PAGE_SIZE: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Selection {
    pub(crate) mask: u8,
    pub(crate) cost: bool,
}

impl Selection {
    pub(crate) fn selected(self, series: Series) -> bool {
        !self.cost && self.mask & (1 << series as u8) != 0
    }

    pub(crate) fn with_series(mut self, series: Series, checked: bool) -> Self {
        if self.selected(series) == checked {
            return self;
        }
        let bit = 1 << series as u8;
        self.mask = if self.cost {
            bit
        } else if checked {
            self.mask | bit
        } else {
            self.mask & !bit
        };
        self.cost = false;
        self
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Bucket {
    pub(crate) first: NaiveDate,
    pub(crate) last: NaiveDate,
    pub(crate) usage: TokenUsage,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Availability {
    pub(crate) series: u8,
    pub(crate) cost: bool,
}

impl Availability {
    pub(crate) fn from_buckets(data: &[Bucket]) -> Self {
        let mut available = Self::default();
        for bucket in data {
            for series in Series::ALL {
                if series.value(&bucket.usage) > 0 {
                    available.series |= 1 << series as u8;
                }
            }
            // A priced, free request is a known zero cost, not missing data.
            available.cost |= bucket.usage.priced_requests > 0;
        }
        available
    }

    pub(crate) fn selection(self, requested: Selection) -> Selection {
        Selection {
            mask: requested.mask & self.series,
            cost: self.cost && (requested.cost || self.series == 0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Series {
    Input = 0,
    Cache = 1,
    Output = 2,
}

impl Series {
    pub(crate) const ALL: [Self; 3] = [Self::Input, Self::Cache, Self::Output];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Input => "Input",
            Self::Cache => "Cache",
            Self::Output => "Output",
        }
    }

    pub(crate) fn value(self, usage: &TokenUsage) -> u64 {
        // Cached input is a subset of input, not an additional token stream.
        match self {
            Self::Input => usage.input_tokens.saturating_sub(usage.cached_input_tokens),
            Self::Cache => usage.cached_input_tokens.min(usage.input_tokens),
            Self::Output => usage.output_tokens,
        }
    }

    pub(crate) fn menu_label(self) -> &'static str {
        match self {
            Self::Input => "Input (uncached)",
            Self::Cache => "Cached input",
            Self::Output => "Output",
        }
    }

    pub(crate) fn color(self, palette: &Palette) -> Hsla {
        match (self, palette.dark) {
            (Self::Cache, _) => palette.accent,
            (Self::Input, true) => super::theme::rgb8((155, 166, 244)),
            (Self::Input, false) => super::theme::rgb8((96, 89, 186)),
            (Self::Output, true) => super::theme::rgb8((93, 211, 171)),
            (Self::Output, false) => super::theme::rgb8((0, 126, 96)),
        }
    }
}

pub(crate) fn buckets(statistics: &UsageStatistics, today: NaiveDate) -> Vec<Bucket> {
    let days = usize::from(statistics.history_days.clamp(1, 365));
    let first = today - ChronoDuration::days((days - 1) as i64);
    let mut daily = BTreeMap::<NaiveDate, TokenUsage>::new();
    for entry in &statistics.daily {
        if entry.date >= first && entry.date <= today {
            daily.entry(entry.date).or_default().add(&entry.usage);
        }
    }
    let per_bar = days.div_ceil(MAX_BARS);
    (0..days)
        .step_by(per_bar)
        .map(|offset| {
            let last_offset = (offset + per_bar).min(days) - 1;
            let start = first + ChronoDuration::days(offset as i64);
            let end = first + ChronoDuration::days(last_offset as i64);
            let mut usage = TokenUsage::default();
            for (_, day) in daily.range(start..=end) {
                usage.add(day);
            }
            Bucket {
                first: start,
                last: end,
                usage,
            }
        })
        .collect()
}

pub(crate) fn visible_tokens(usage: &TokenUsage, mask: u8) -> u64 {
    Series::ALL
        .into_iter()
        .filter(|series| mask & (1 << *series as u8) != 0)
        .fold(0_u64, |total, series| {
            total.saturating_add(series.value(usage))
        })
}

pub(crate) fn cost_label(usage: &TokenUsage) -> String {
    if usage.priced_requests > 0 {
        let value = format_usd(usage.estimated_cost_microusd as f64 / 1_000_000.0);
        if usage.priced_requests < usage.requests {
            format!("{value} (partially priced)")
        } else {
            value
        }
    } else if usage.requests == 0 {
        "$0.00".into()
    } else {
        "Unavailable".into()
    }
}

fn bucket_title(bucket: &Bucket) -> String {
    if bucket.first == bucket.last {
        bucket.first.format("%a, %b %-d, %Y").to_string()
    } else {
        format!(
            "{} – {}",
            bucket.first.format("%b %-d, %Y"),
            bucket.last.format("%b %-d, %Y")
        )
    }
}

// ----- per-model breakdown -----------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ModelData {
    days: BTreeMap<NaiveDate, BTreeMap<String, TokenUsage>>,
}

pub(crate) fn group_models(
    rows: Vec<(String, NaiveDate, TokenUsage)>,
    bounds: &[Bucket],
    provider: ProviderKind,
) -> ModelData {
    let mut result = ModelData::default();
    for (model, date, usage) in rows {
        if let Some(bucket) = bounds.iter().find(|b| b.first <= date && date <= b.last) {
            let model = if provider == ProviderKind::Cursor {
                crate::cursor::normalize_cursor_model_name(&model)
            } else {
                model
            };
            result
                .days
                .entry(bucket.first)
                .or_default()
                .entry(model)
                .or_default()
                .add(&usage);
        }
    }
    result
}

fn model_value(usage: &TokenUsage, cost: bool) -> u64 {
    if cost {
        usage.estimated_cost_microusd
    } else {
        usage.total_tokens()
    }
}

/// A model keeps its color across dates, refreshes, metric switches and new models.
pub(crate) fn model_color(model: &str, dark: bool) -> Hsla {
    let mut hash = model.bytes().fold(0xcbf29ce484222325_u64, |hash, b| {
        (hash ^ u64::from(b)).wrapping_mul(0x100000001b3)
    });
    hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d049bb133111eb);
    hash ^= hash >> 31;
    let hue = (hash >> 32) as f64 / u32::MAX as f64 * 6.0;
    let lightness = if dark { 0.69 } else { 0.40 };
    let chroma = (1.0_f64 - (2.0_f64 * lightness - 1.0).abs()) * 0.70;
    let x = chroma * (1.0 - (hue % 2.0 - 1.0).abs());
    let (r, g, b) = match hue as u8 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = lightness - chroma / 2.0;
    super::theme::rgb8((
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    ))
}

pub(crate) fn next_page(current: usize, pages: usize, wheel: f32) -> usize {
    let last = pages.saturating_sub(1);
    if wheel < 0.0 {
        current.saturating_add(1).min(last)
    } else {
        current.min(last).saturating_sub(1)
    }
}

impl ModelData {
    pub(crate) fn availability(&self) -> Availability {
        Availability::from_buckets(
            &self
                .days
                .iter()
                .map(|(date, models)| {
                    let mut usage = TokenUsage::default();
                    for model in models.values() {
                        usage.add(model);
                    }
                    Bucket {
                        first: *date,
                        last: *date,
                        usage,
                    }
                })
                .collect::<Vec<_>>(),
        )
    }

    pub(crate) fn has_data(&self) -> bool {
        self.days
            .values()
            .flat_map(|models| models.values())
            .any(|usage| {
                usage.requests > 0 || usage.total_tokens() > 0 || usage.priced_requests > 0
            })
    }

    pub(crate) fn value(&self, date: NaiveDate, cost: bool) -> u64 {
        self.days
            .get(&date)
            .into_iter()
            .flat_map(|models| models.values())
            .fold(0_u64, |total, usage| {
                total.saturating_add(model_value(usage, cost))
            })
    }

    pub(crate) fn pages(&self, date: NaiveDate) -> usize {
        self.days
            .get(&date)
            .map_or(1, |models| models.len().div_ceil(MODEL_PAGE_SIZE).max(1))
    }

    fn sorted(&self, date: NaiveDate, cost: bool) -> Vec<(&str, &TokenUsage)> {
        let mut rows: Vec<_> = self
            .days
            .get(&date)
            .into_iter()
            .flat_map(|models| models.iter())
            .map(|(model, usage)| (model.as_str(), usage))
            .collect();
        rows.sort_by(|a, b| {
            model_value(b.1, cost)
                .cmp(&model_value(a.1, cost))
                .then_with(|| a.0.cmp(b.0))
        });
        rows
    }

    pub(crate) fn page_rows(
        &self,
        date: NaiveDate,
        cost: bool,
        dark: bool,
        page: usize,
    ) -> (Vec<(String, String, Hsla)>, usize) {
        let all = self.sorted(date, cost);
        let page = page.min(self.pages(date).saturating_sub(1));
        let total = all.len();
        let rows = all
            .iter()
            .skip(page * MODEL_PAGE_SIZE)
            .take(MODEL_PAGE_SIZE)
            .map(|(name, usage)| {
                let amount = if cost {
                    cost_label(usage)
                } else {
                    format_token_count(usage.total_tokens())
                };
                ((*name).to_string(), amount, model_color(name, dark))
            })
            .collect();
        (rows, total)
    }

    /// Stacked segments of one day, largest last so it sits on top.
    fn segments(&self, date: NaiveDate, cost: bool, dark: bool) -> Vec<(Hsla, f32)> {
        self.days
            .get(&date)
            .into_iter()
            .flat_map(|models| models.iter())
            .map(|(name, usage)| (model_color(name, dark), model_value(usage, cost) as f32))
            .collect()
    }
}

// ----- view state ---------------------------------------------------------------

#[derive(Clone, PartialEq)]
enum ModelLoad {
    Loading,
    Ready(Arc<ModelData>),
    Failed(String),
}

pub(crate) struct ChartState {
    requested: Selection,
    by_model: bool,
    model_page: usize,
    hovered: Option<NaiveDate>,
    model_key: Option<String>,
    models: Option<ModelLoad>,
    _task: Option<Task<()>>,
    buckets: Option<BucketCache>,
    bars: HashMap<NaiveDate, Entity<ActivityBarView>>,
}

#[derive(Clone, PartialEq)]
struct ActivityBarStyle {
    width: f32,
    height: f32,
    dim: f32,
    background: Option<Hsla>,
    segments: Vec<(Hsla, f32)>,
}

struct ActivityBarView {
    style: ActivityBarStyle,
}

impl Render for ActivityBarView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut bar = div()
            .w(px(self.style.width))
            .h(px(self.style.height))
            .rounded(px(1.5))
            .overflow_hidden()
            .flex()
            .flex_col()
            .opacity(1.0 - 0.25 * self.style.dim);
        if let Some(background) = self.style.background {
            bar = bar.bg(background);
        }
        for (color, weight) in &self.style.segments {
            bar = bar.child(grow(div().w_full().bg(*color), *weight));
        }
        bar
    }
}

struct BucketCache {
    history_days: u16,
    daily: Vec<DailyTokenUsage>,
    today: NaiveDate,
    data: Arc<Vec<Bucket>>,
}

impl ChartState {
    fn new(cost_based: bool) -> Self {
        Self {
            requested: Selection {
                mask: 7,
                cost: cost_based,
            },
            by_model: false,
            model_page: 0,
            hovered: None,
            model_key: None,
            models: None,
            _task: None,
            buckets: None,
            bars: HashMap::new(),
        }
    }

    fn buckets(&mut self, statistics: &UsageStatistics, today: NaiveDate) -> Arc<Vec<Bucket>> {
        if self.buckets.as_ref().is_none_or(|cache| {
            cache.today != today
                || cache.history_days != statistics.history_days
                || cache.daily != statistics.daily
        }) {
            self.buckets = Some(BucketCache {
                history_days: statistics.history_days,
                daily: statistics.daily.clone(),
                today,
                data: Arc::new(buckets(statistics, today)),
            });
        }
        Arc::clone(&self.buckets.as_ref().unwrap().data)
    }

    pub(super) fn release_data(&mut self) {
        self._task = None;
        self.models = None;
        self.model_key = None;
        self.buckets = None;
        self.bars = HashMap::new();
        self.hovered = None;
    }
}

impl PopupRoot {
    fn ensure_models(
        &mut self,
        chart: &str,
        provider: ProviderKind,
        statistics: &UsageStatistics,
        today: NaiveDate,
        bounds: &[Bucket],
        cx: &mut Context<Self>,
    ) {
        let key = format!(
            "{}|{}|{}|{}|{}",
            today,
            statistics.history_days,
            statistics.daily.len(),
            statistics.history.total_tokens(),
            statistics.history.requests
        );
        let Some(state) = self.charts.get_mut(chart) else {
            return;
        };
        if state.model_key.as_deref() == Some(key.as_str()) {
            return;
        }
        state.model_key = Some(key.clone());
        if state.models.is_none() {
            state.models = Some(ModelLoad::Loading);
        }
        let account = statistics.account_id.clone();
        let bounds = bounds.to_vec();
        let chart = chart.to_owned();
        state._task = Some(cx.spawn(async move |this, cx| {
            let first = bounds.first().map_or(today, |bucket| bucket.first);
            let query_bounds = bounds.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    crate::store::with_store(|store| {
                        if provider == ProviderKind::Codex
                            && let Some(account) = account.as_deref()
                        {
                            store.account_daily_for(account, first, today)
                        } else if provider == ProviderKind::OpenRouter
                            && let Some(account) = account.as_deref()
                        {
                            store.load_openrouter_account_models(account, first, today)
                        } else {
                            store.load_model_daily(provider, first, today)
                        }
                    })
                    .map(|rows| Arc::new(group_models(rows, &query_bounds, provider)))
                    .map_err(|error| format!("Could not load model data: {error:#}"))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(state) = this.charts.get_mut(&chart)
                    && state.model_key.as_deref() == Some(key.as_str())
                {
                    state.models = Some(match result {
                        Ok(data) => ModelLoad::Ready(data),
                        Err(error) => ModelLoad::Failed(error),
                    });
                    cx.notify();
                }
            });
        }));
    }

    pub(super) fn render_activity_card(
        &mut self,
        provider: ProviderKind,
        statistics: &UsageStatistics,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        if provider == ProviderKind::Cursor && !statistics.has_data() {
            return card(&palette)
                .p(px(12.0))
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(components::body_strong(
                    "Usage activity",
                    palette.text_primary,
                ))
                .child(caption(
                    "Waiting for Cursor's usage export. Refresh to retry.",
                    palette.text_tertiary,
                ))
                .into_any_element();
        }
        let cost_based = is_cost_provider(provider);
        let chart = format!(
            "{}-{}",
            provider.id(),
            statistics.account_id.as_deref().unwrap_or("all")
        );
        let today = Local::now().date_naive();
        let data = self
            .charts
            .entry(chart.clone())
            .or_insert_with(|| ChartState::new(cost_based))
            .buckets(statistics, today);
        self.charts
            .get_mut(&chart)
            .unwrap()
            .bars
            .retain(|date, _| data.iter().any(|bucket| bucket.first == *date));
        let by_model = self.charts[&chart].by_model;
        if by_model {
            self.ensure_models(&chart, provider, statistics, today, &data, cx);
        }
        let state = &self.charts[&chart];
        let models = match &state.models {
            Some(ModelLoad::Ready(models)) => Some(Arc::clone(models)),
            _ => None,
        };
        let loading = matches!(state.models, Some(ModelLoad::Loading));
        let failed = matches!(state.models, Some(ModelLoad::Failed(_)));
        let requested = state.requested;
        let hovered = state
            .hovered
            .filter(|date| data.iter().any(|b| b.first == *date));
        let availability = if by_model {
            models
                .as_ref()
                .map(|m| m.availability())
                .unwrap_or_default()
        } else {
            Availability::from_buckets(&data)
        };
        let selection = availability.selection(requested);
        let Selection {
            mask,
            cost: cost_mode,
        } = selection;
        let value_of = |bucket: &Bucket| -> u64 {
            if by_model {
                models
                    .as_ref()
                    .map_or(0, |models| models.value(bucket.first, cost_mode))
            } else if cost_mode {
                bucket.usage.estimated_cost_microusd
            } else {
                visible_tokens(&bucket.usage, mask)
            }
        };
        let maximum = data.iter().map(&value_of).max().unwrap_or(0);
        let slot_width = CHART_WIDTH / data.len().max(1) as f32;
        let bar_width = (slot_width - 2.0).clamp(1.0, 12.0);

        let mut bars = div()
            .id(eid(format!("activity-bars-{chart}")))
            .relative()
            .flex()
            .flex_row()
            .items_end()
            .h(px(HEIGHT))
            .w_full()
            // The tooltip tracks the pointer across the bars.
            .on_mouse_move(cx.listener(|this, _: &gpui::MouseMoveEvent, _, cx| {
                if this.tip.is_some() {
                    cx.notify();
                }
            }));
        let cache_paint = self.cache_graph_paint();
        for bucket in data.iter() {
            let value = value_of(bucket);
            let date = bucket.first;
            let target = if maximum == 0 {
                2.0
            } else {
                (HEIGHT * value as f32 / maximum as f32).max(2.0)
            };
            let height = self.fx.value(
                fx::key(("activity-bar", chart.as_str(), date)),
                target,
                fx::FAST,
            );
            let dim = self.fx.toggle(
                fx::key(("activity-dim", chart.as_str(), date)),
                hovered.is_some_and(|h| h != date),
                fx::FAST,
            );
            let mut bar_style = ActivityBarStyle {
                width: bar_width,
                height,
                dim,
                background: None,
                segments: Vec::new(),
            };
            if by_model && value > 0 {
                if let Some(models) = models.as_ref() {
                    for (color, weight) in models.segments(date, cost_mode, palette.dark) {
                        bar_style.segments.push((color, weight));
                    }
                }
            } else if cost_mode || value == 0 {
                bar_style.background =
                    Some(palette.accent.opacity(if value == 0 { 0.2 } else { 1.0 }));
            } else {
                for series in [Series::Output, Series::Cache, Series::Input] {
                    let share = if selection.selected(series) {
                        series.value(&bucket.usage) as f32 / value as f32
                    } else {
                        0.0
                    };
                    let share = self.fx.value(
                        fx::key(("activity-share", chart.as_str(), date, series as u8)),
                        share,
                        fx::FAST,
                    );
                    bar_style.segments.push((series.color(&palette), share));
                }
            }
            let existing = self.charts[&chart].bars.get(&date).cloned();
            let bar = if let Some(bar) = existing {
                bar.update(cx, |bar, cx| {
                    if bar.style != bar_style {
                        bar.style = bar_style.clone();
                        cx.notify();
                    }
                });
                bar
            } else {
                let bar = cx.new(|_| ActivityBarView { style: bar_style });
                self.charts
                    .get_mut(&chart)
                    .unwrap()
                    .bars
                    .insert(date, bar.clone());
                bar
            };
            let mut cached_style = div().w(px(bar_width)).h(px(height));
            let bar = AnyView::from(bar);
            let bar = if cache_paint {
                bar.cached(cached_style.style().clone())
            } else {
                bar
            };
            let chart_for_hover = chart.clone();
            let chart_for_wheel = chart.clone();
            let bucket_for_tip = bucket.clone();
            let tip_owner = fx::key(("activity-tip", chart.as_str()));
            let models_for_tip = models.clone();
            let pages = models.as_ref().map_or(1, |models| models.pages(date));
            let mut slot = div()
                .id(eid(format!("activity-{chart}-{date}")))
                .flex_1()
                .h_full()
                .flex()
                .items_end()
                .justify_center()
                .child(bar)
                .on_hover(cx.listener(move |this, hovered: &bool, window, cx| {
                    let Some(state) = this.charts.get_mut(&chart_for_hover) else {
                        return;
                    };
                    if *hovered {
                        state.hovered = Some(date);
                        state.model_page = 0;
                        let content = this.activity_tip(
                            &bucket_for_tip,
                            models_for_tip.as_deref(),
                            by_model,
                            cost_mode,
                            0,
                        );
                        this.show_tip(tip_owner, TipContent::Activity(content), window, cx);
                    } else if state.hovered == Some(date) {
                        state.hovered = None;
                        if this.tip.as_ref().is_some_and(|tip| tip.owner == tip_owner) {
                            this.tip = None;
                        }
                    }
                    cx.notify();
                }));
            if by_model && pages > 1 {
                let bucket_for_page = bucket.clone();
                let models_for_page = models.clone();
                slot = slot.on_scroll_wheel(cx.listener(
                    move |this, event: &ScrollWheelEvent, _, cx| {
                        let delta = f32::from(event.delta.pixel_delta(px(20.0)).y);
                        if delta == 0.0 {
                            return;
                        }
                        cx.stop_propagation();
                        let Some(state) = this.charts.get_mut(&chart_for_wheel) else {
                            return;
                        };
                        state.model_page = next_page(state.model_page, pages, delta);
                        let page = state.model_page;
                        let content = this.activity_tip(
                            &bucket_for_page,
                            models_for_page.as_deref(),
                            true,
                            cost_mode,
                            page,
                        );
                        if let Some(tip) = this.tip.as_mut() {
                            tip.content = TipContent::Activity(content);
                        }
                        cx.notify();
                    },
                ));
            }
            bars = bars.child(slot);
        }
        let empty_label = if by_model && failed {
            Some("Model data unavailable")
        } else if by_model && (loading || models.is_none()) {
            Some("Loading models…")
        } else if by_model && availability.series == 0 && !availability.cost {
            Some("No model data")
        } else if availability.series == 0 && !availability.cost {
            Some("No usage data")
        } else if !by_model && !cost_mode && mask == 0 {
            Some("No series selected")
        } else {
            None
        };
        if let Some(label) = empty_label {
            bars = bars.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(components::text(label, 11.0, 14.0, palette.text_tertiary)),
            );
        }

        // Legend: one toggle per token series.
        let mut legend = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.0))
            .h(px(24.0));
        if !by_model {
            for series in Series::ALL {
                let available = availability.series & (1 << series as u8) != 0;
                let selected = selection.selected(series);
                let opacity = self.fx.value(
                    fx::key(("legend", chart.as_str(), series as u8)),
                    if !available {
                        0.3
                    } else if selected {
                        1.0
                    } else {
                        0.45
                    },
                    fx::FAST,
                );
                let hover_id = fx::key(("legend-hover", chart.as_str(), series as u8));
                let tip = if available {
                    format!(
                        "{}: {} tokens",
                        series.menu_label(),
                        format_token_count(series.value(&statistics.history))
                    )
                } else {
                    format!(
                        "No {} tokens in this period",
                        series.menu_label().to_lowercase()
                    )
                };
                let chart_key = chart.clone();
                let mut item = div()
                    .id(eid(format!("legend-{chart}-{}", series.label())))
                    .px(px(3.0))
                    .h(px(24.0))
                    .flex()
                    .items_center()
                    .rounded(px(4.0))
                    .opacity(opacity)
                    .on_hover(self.hover_listener(hover_id, Some(tip.into()), cx))
                    .child(components::text(
                        format!("● {}", series.label()),
                        11.0,
                        14.0,
                        series.color(&palette),
                    ));
                if available {
                    item = item.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(state) = this.charts.get_mut(&chart_key) {
                            let current = Selection {
                                cost: availability.selection(state.requested).cost,
                                ..state.requested
                            };
                            state.requested =
                                current.with_series(series, !current.selected(series));
                            cx.notify();
                        }
                    }));
                }
                legend = legend.child(item);
            }
        }

        let model_available = models.as_ref().is_some_and(|m| m.has_data()) || !by_model;
        let model_hint: SharedString = if by_model && loading {
            "Loading model breakdown".into()
        } else if by_model && failed {
            "Model data unavailable".into()
        } else {
            "Group tokens or cost by model".into()
        };
        let model_on = self
            .fx
            .toggle(fx::key(("model-on", chart.as_str())), by_model, fx::FAST);
        let chart_key = chart.clone();
        let model_selector = div()
            .id(eid(format!("model-selector-{chart}")))
            .w(px(48.0))
            .h(px(26.0))
            .p(px(2.0))
            .rounded(px(6.0))
            .bg(palette.subtle_fill)
            .on_hover(self.hover_listener(
                fx::key(("model-selector", chart.as_str())),
                Some(model_hint),
                cx,
            ))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if let Some(state) = this.charts.get_mut(&chart_key) {
                    state.by_model = !state.by_model;
                    cx.notify();
                }
            }))
            .child(
                div()
                    .size_full()
                    .rounded(px(4.0))
                    .bg(palette.control_fill.alpha(model_on))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(components::text(
                        "Model",
                        10.0,
                        12.0,
                        palette
                            .text_secondary
                            .mix(palette.text_primary, model_on)
                            .alpha(if model_available { 1.0 } else { 0.5 }),
                    )),
            );
        let mut metric_selector = div()
            .flex()
            .flex_row()
            .h(px(26.0))
            .p(px(2.0))
            .rounded(px(6.0))
            .bg(super::theme::rgba8(
                0,
                0,
                0,
                if palette.dark { 35 } else { 14 },
            ));
        for (index, (label, cost, width)) in [("Tokens", false, 46.0), ("Cost", true, 38.0)]
            .into_iter()
            .enumerate()
        {
            let available = if cost {
                availability.cost
            } else {
                availability.series != 0
            };
            let selected = available && cost_mode == cost;
            let on = self.fx.toggle(
                fx::key(("metric-on", chart.as_str(), index)),
                selected,
                fx::FAST,
            );
            let (idle, active) = if palette.dark {
                (
                    super::theme::rgb8((155, 155, 155)),
                    super::theme::rgb8((245, 245, 245)),
                )
            } else {
                (
                    super::theme::rgb8((100, 100, 100)),
                    super::theme::rgb8((24, 24, 24)),
                )
            };
            let tip = if !available && cost {
                "No cost data for this period"
            } else if !available {
                "No token data for this period"
            } else if cost {
                "Daily cost in USD"
            } else {
                "Daily token volume"
            };
            let chart_key = chart.clone();
            let mut cell = div()
                .id(eid(format!("metric-{chart}-{label}")))
                .w(px(width))
                .h_full()
                .rounded(px(4.0))
                .bg(palette.control_fill.alpha(on))
                .flex()
                .items_center()
                .justify_center()
                .on_hover(self.hover_listener(
                    fx::key(("metric-hover", chart.as_str(), index)),
                    Some(tip.into()),
                    cx,
                ))
                .child(components::text(
                    label,
                    10.0,
                    12.0,
                    idle.mix(active, on)
                        .alpha(if available { 1.0 } else { 0.5 }),
                ));
            if available {
                cell = cell.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(state) = this.charts.get_mut(&chart_key) {
                        state.requested.cost = cost;
                        cx.notify();
                    }
                }));
            }
            metric_selector = metric_selector.child(cell);
        }
        let footer = div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .child(div().flex_1().min_w_0().child(legend))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .child(model_selector)
                    .child(metric_selector),
            );

        let _ = window;
        card(&palette)
            .p(px(12.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(usage_card_metrics(provider, statistics, &palette))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(bars)
                    .child(footer),
            )
            .into_any_element()
    }

    pub(super) fn activity_tip(
        &self,
        bucket: &Bucket,
        models: Option<&ModelData>,
        by_model: bool,
        cost: bool,
        page: usize,
    ) -> ActivityTip {
        let title = bucket_title(bucket);
        let dark = self.palette.dark;
        if by_model && let Some(models) = models {
            let (rows, total) = models.page_rows(bucket.first, cost, dark, page);
            let page = page.min(models.pages(bucket.first).saturating_sub(1));
            let footer = if rows.is_empty() {
                Some("No model data".into())
            } else if total > MODEL_PAGE_SIZE {
                Some(format!(
                    "{}–{} of {total} models · Scroll for more",
                    page * MODEL_PAGE_SIZE + 1,
                    ((page + 1) * MODEL_PAGE_SIZE).min(total)
                ))
            } else {
                None
            };
            return ActivityTip::Models {
                title,
                metric: if cost {
                    "Cost (USD) by model".into()
                } else {
                    "Tokens by model".into()
                },
                rows,
                footer,
            };
        }
        let usage = &bucket.usage;
        let cost = if usage.priced_requests > 0 {
            format_usd(usage.estimated_cost_microusd as f64 / 1_000_000.0)
        } else if usage.requests == 0 {
            "$0.00".into()
        } else {
            "Unavailable".into()
        };
        let requests = if usage.priced_requests > 0 && usage.priced_requests < usage.requests {
            format!(
                "{} requests · {} priced",
                usage.requests, usage.priced_requests
            )
        } else {
            format!("{} requests", usage.requests)
        };
        ActivityTip::Usage {
            title,
            total: format_token_count(usage.total_tokens()),
            cost,
            requests,
            series: Series::ALL
                .into_iter()
                .map(|series| {
                    (
                        series.menu_label(),
                        format_token_count(series.value(usage)),
                        series.color(&self.palette),
                    )
                })
                .collect(),
        }
    }
}

fn grow(element: gpui::Div, weight: f32) -> gpui::Div {
    let mut element = element.flex_basis(px(0.0)).min_h(px(0.0));
    element.style().flex_grow = Some(weight.max(0.0));
    element.style().flex_shrink = Some(1.0);
    element
}

pub(crate) fn is_cost_provider(provider: ProviderKind) -> bool {
    matches!(
        provider,
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo
    )
}

fn metric_cell(label: String, value: String, detail: String, palette: &Palette) -> gpui::Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(1.0))
        .child(caption(label, palette.text_tertiary))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(5.0))
                .child(components::body_strong(value, palette.text_primary))
                .child(nowrap(caption(detail, palette.text_tertiary))),
        )
}

fn usage_card_metrics(
    provider: ProviderKind,
    statistics: &UsageStatistics,
    palette: &Palette,
) -> gpui::Div {
    let period = statistics.history_days;
    let row = div().flex().flex_row().w_full().gap(px(8.0));
    if is_cost_provider(provider) {
        let spend = |usage: &TokenUsage| format_usd(usage.estimated_cost_microusd as f64 / 1e6);
        return row
            .child(metric_cell(
                "Today".into(),
                spend(&statistics.today),
                format!("{} requests", statistics.today.requests),
                palette,
            ))
            .child(metric_cell(
                format!("Last {period} days"),
                spend(&statistics.history),
                format!("{} requests", statistics.history.requests),
                palette,
            ));
    }
    let exact = provider == ProviderKind::OpenRouter;
    let value = |usage: &TokenUsage| {
        format!(
            "{} {}",
            if exact { "=" } else { "≈" },
            usage
                .estimated_api_value_usd()
                .map(format_usd)
                .unwrap_or_else(|| "No data".into())
        )
    };
    row.child(metric_cell(
        "Today".into(),
        format_token_count(statistics.today.total_tokens()),
        value(&statistics.today),
        palette,
    ))
    .child(metric_cell(
        format!("Last {period} days"),
        format_token_count(statistics.history.total_tokens()),
        value(&statistics.history),
        palette,
    ))
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    #[test]
    fn buckets_refresh_for_new_data_period_and_calendar_day() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 6).unwrap();
        let mut statistics = UsageStatistics {
            history_days: 7,
            daily: vec![DailyTokenUsage {
                date: today,
                usage: TokenUsage {
                    input_tokens: 10,
                    requests: 1,
                    ..Default::default()
                },
            }],
            ..Default::default()
        };
        let mut state = ChartState::new(false);
        let first = state.buckets(&statistics, today);
        assert!(Arc::ptr_eq(&first, &state.buckets(&statistics, today)));
        statistics.daily[0].usage.input_tokens = 20;
        let changed = state.buckets(&statistics, today);
        assert!(!Arc::ptr_eq(&first, &changed));
        assert_eq!(changed.last().unwrap().usage.input_tokens, 20);
        statistics.history_days = 30;
        let wider = state.buckets(&statistics, today);
        assert_eq!(wider.len(), 30);
        assert!(!Arc::ptr_eq(&wider, &changed));
        assert!(!Arc::ptr_eq(
            &wider,
            &state.buckets(&statistics, today + chrono::Duration::days(1))
        ));
    }

    #[test]
    fn hiding_releases_model_and_bucket_data_but_preserves_chart_choices() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 6).unwrap();
        let mut state = ChartState::new(true);
        state.by_model = true;
        state.model_page = 2;
        let bucket_data = Arc::downgrade(&state.buckets(&UsageStatistics::default(), today));
        let models = Arc::new(ModelData::default());
        let model_data = Arc::downgrade(&models);
        state.models = Some(ModelLoad::Ready(models));
        state.model_key = Some("loaded".into());
        state.release_data();
        assert!(bucket_data.upgrade().is_none());
        assert!(model_data.upgrade().is_none());
        assert!(state.model_key.is_none());
        assert!(state.by_model);
        assert!(state.requested.cost);
        assert_eq!(state.model_page, 2);
    }
}
