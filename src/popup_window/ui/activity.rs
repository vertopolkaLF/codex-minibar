//! Per-provider usage activity card: metrics, daily stacked bars, legend.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
};

use chrono::NaiveDate;
use gpui::{
    AnyElement, AnyView, AppContext, ClickEvent, Context, Entity, FontWeight, Hsla,
    InteractiveElement, IntoElement, ParentElement, Render, ScrollWheelEvent, SharedString,
    StatefulInteractiveElement, Styled, Task, Window, div, px,
};

use super::{
    components::{self, caption, card, nowrap},
    controls::QuietSegment,
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
/// Input and output; cache reads dwarf both, so they start hidden.
const DEFAULT_SERIES: u8 = 0b101;
const LEGEND_GAP: f32 = 2.0;

/// Click handler of one legend toggle.
type LegendClick = Box<dyn Fn(&mut PopupRoot, &mut Context<PopupRoot>)>;

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
        let mut mask = requested.mask & self.series;
        // A cache-only history would otherwise open on an empty chart.
        if mask == 0 && requested.mask == DEFAULT_SERIES {
            mask = self.series;
        }
        Selection {
            mask,
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
            Self::Input => crate::i18n::tr("input"),
            Self::Cache => crate::i18n::tr("cache"),
            Self::Output => crate::i18n::tr("output"),
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
            Self::Input => crate::i18n::tr("input-uncached"),
            Self::Cache => crate::i18n::tr("cached-input"),
            Self::Output => crate::i18n::tr("output"),
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
            crate::i18n::format("value-partially-priced", &[("value", value.to_string())])
        } else {
            value
        }
    } else if usage.requests == 0 {
        "$0.00".into()
    } else {
        crate::i18n::tr("unavailable").into()
    }
}

fn bucket_title(bucket: &Bucket) -> String {
    if bucket.first == bucket.last {
        format!(
            "{}, {}",
            crate::i18n::weekday(chrono::Datelike::weekday(&bucket.first)),
            crate::i18n::date_with_year(bucket.first)
        )
    } else {
        format!(
            "{} – {}",
            crate::i18n::date_with_year(bucket.first),
            crate::i18n::date_with_year(bucket.last)
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

/// Legend label for a model id: no vendor prefix or snapshot date, words
/// capitalized and split versions rejoined (`claude-sonnet-4-5-20250929`
/// reads "Sonnet 4.5", `openai/gpt-5-codex` reads "GPT-5 Codex"). The full
/// id stays in the entry's tooltip.
pub(crate) fn short_model_name(model: &str) -> String {
    let model = model.rsplit('/').next().unwrap_or(model);
    let mut parts: Vec<&str> = model
        .split(['-', '_', ' '])
        .filter(|part| !part.is_empty())
        .collect();
    if parts
        .last()
        .is_some_and(|part| part.len() == 8 && part.bytes().all(|b| b.is_ascii_digit()))
    {
        parts.pop();
    }
    // Keep "Claude" when nothing but a version follows it.
    if parts.len() > 1
        && parts[0].eq_ignore_ascii_case("claude")
        && !parts[1].starts_with(|c: char| c.is_ascii_digit())
    {
        parts.remove(0);
    }
    let numeric = |part: &str| part.bytes().all(|b| b.is_ascii_digit() || b == b'.');
    let mut words: Vec<String> = Vec::new();
    let mut previous_numeric = false;
    for part in parts {
        let is_numeric = numeric(part);
        let last = words.last_mut();
        match last {
            Some(last) if is_numeric && previous_numeric => {
                last.push('.');
                last.push_str(part);
            }
            Some(last) if is_numeric && (last == "GPT" || last == "GLM") => {
                last.push('-');
                last.push_str(part);
            }
            _ if part.eq_ignore_ascii_case("gpt") || part.eq_ignore_ascii_case("glm") => {
                words.push(part.to_ascii_uppercase());
            }
            _ => {
                let mut chars = part.chars();
                words.push(match chars.next() {
                    Some(first) => first.to_uppercase().chain(chars).collect(),
                    None => String::new(),
                });
            }
        }
        previous_numeric = is_numeric;
    }
    if words.is_empty() {
        model.to_owned()
    } else {
        words.join(" ")
    }
}

/// How many legend entries fit inline beside a `+N` chip for the rest.
pub(crate) fn inline_legend_count(
    widths: &[f32],
    chip_width: impl Fn(usize) -> f32,
    available: f32,
) -> usize {
    let span = |count: usize| {
        widths[..count].iter().sum::<f32>() + LEGEND_GAP * count.saturating_sub(1) as f32
    };
    if span(widths.len()) <= available {
        return widths.len();
    }
    (0..widths.len())
        .rev()
        .find(|&count| {
            let gap = if count == 0 { 0.0 } else { LEGEND_GAP };
            span(count) + gap + chip_width(widths.len() - count) <= available
        })
        .unwrap_or(0)
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

    fn visible<'a>(
        &'a self,
        date: NaiveDate,
        hidden: &'a BTreeSet<String>,
    ) -> impl Iterator<Item = (&'a String, &'a TokenUsage)> + 'a {
        self.days
            .get(&date)
            .into_iter()
            .flat_map(|models| models.iter())
            .filter(move |(model, _)| !hidden.contains(*model))
    }

    pub(crate) fn value(&self, date: NaiveDate, cost: bool, hidden: &BTreeSet<String>) -> u64 {
        self.visible(date, hidden).fold(0_u64, |total, (_, usage)| {
            total.saturating_add(model_value(usage, cost))
        })
    }

    pub(crate) fn pages(&self, date: NaiveDate, hidden: &BTreeSet<String>) -> usize {
        self.visible(date, hidden)
            .count()
            .div_ceil(MODEL_PAGE_SIZE)
            .max(1)
    }

    /// Every model of the period with its total, largest first: the legend
    /// order, so the inline entries are the ones that shape the chart.
    pub(crate) fn ranking(&self, cost: bool) -> Vec<(String, TokenUsage)> {
        let mut totals = BTreeMap::<&str, TokenUsage>::new();
        for (model, usage) in self.days.values().flat_map(|models| models.iter()) {
            totals.entry(model.as_str()).or_default().add(usage);
        }
        let mut rows: Vec<_> = totals
            .into_iter()
            .filter(|(_, usage)| usage.total_tokens() > 0 || usage.estimated_cost_microusd > 0)
            .map(|(model, usage)| (model.to_owned(), usage))
            .collect();
        rows.sort_by(|a, b| {
            model_value(&b.1, cost)
                .cmp(&model_value(&a.1, cost))
                .then_with(|| a.0.cmp(&b.0))
        });
        rows
    }

    fn sorted<'a>(
        &'a self,
        date: NaiveDate,
        cost: bool,
        hidden: &'a BTreeSet<String>,
    ) -> Vec<(&'a str, &'a TokenUsage)> {
        let mut rows: Vec<_> = self
            .visible(date, hidden)
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
        hidden: &BTreeSet<String>,
    ) -> (Vec<(String, String, Hsla)>, usize) {
        let all = self.sorted(date, cost, hidden);
        let page = page.min(self.pages(date, hidden).saturating_sub(1));
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
    fn segments(
        &self,
        date: NaiveDate,
        cost: bool,
        dark: bool,
        hidden: &BTreeSet<String>,
    ) -> Vec<(Hsla, f32)> {
        self.visible(date, hidden)
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
    hidden_models: BTreeSet<String>,
    models_menu: bool,
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
                mask: DEFAULT_SERIES,
                cost: cost_based,
            },
            by_model: false,
            hidden_models: BTreeSet::new(),
            models_menu: false,
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
        self.models_menu = false;
    }
}

impl PopupRoot {
    fn ensure_models(
        &mut self,
        chart: &str,
        provider: ProviderId,
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
                        if provider == ProviderId::primary(ProviderKind::Codex)
                            && let Some(account) = account.as_deref()
                        {
                            store.account_daily_for(account, first, today)
                        } else if provider.kind() == ProviderKind::OpenRouter
                            && let Some(account) = account.as_deref()
                        {
                            store.load_openrouter_account_models(provider, account, first, today)
                        } else {
                            store.load_model_daily(provider, first, today)
                        }
                    })
                    .map(|rows| Arc::new(group_models(rows, &query_bounds, provider.kind())))
                    .map_err(|error| {
                        crate::i18n::format(
                            "could-not-load-model-data-error",
                            &[("error", format!("{:#}", error))],
                        )
                    })
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
        provider: ProviderId,
        statistics: &UsageStatistics,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        if provider.kind() == ProviderKind::Cursor && !statistics.has_data() {
            return card(&palette)
                .p(px(12.0))
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(components::body_strong(
                    crate::i18n::tr("usage-activity"),
                    palette.text_primary,
                ))
                .child(caption(
                    crate::i18n::tr("waiting-for-cursor-s-usage-export-refresh-to-retry"),
                    palette.text_tertiary,
                ))
                .into_any_element();
        }
        let cost_based = is_cost_provider(provider.kind());
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
        let hidden = Arc::new(state.hidden_models.clone());
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
                    .map_or(0, |models| models.value(bucket.first, cost_mode, &hidden))
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
                    for (color, weight) in models.segments(date, cost_mode, palette.dark, &hidden) {
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
            let pages = models
                .as_ref()
                .map_or(1, |models| models.pages(date, &hidden));
            let hidden_for_tip = Arc::clone(&hidden);
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
                            &hidden_for_tip,
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
                let hidden_for_page = Arc::clone(&hidden);
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
                            &hidden_for_page,
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
            Some(crate::i18n::tr("model-data-unavailable"))
        } else if by_model && (loading || models.is_none()) {
            Some(crate::i18n::tr("loading-models"))
        } else if by_model && availability.series == 0 && !availability.cost {
            Some(crate::i18n::tr("no-model-data"))
        } else if by_model && maximum == 0 && !hidden.is_empty() {
            Some(crate::i18n::tr("no-series-selected"))
        } else if availability.series == 0 && !availability.cost {
            Some(crate::i18n::tr("no-usage-data"))
        } else if !by_model && !cost_mode && mask == 0 {
            Some(crate::i18n::tr("no-series-selected"))
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

        let split_labels: [SharedString; 2] = [
            crate::i18n::tr("split-type").into(),
            crate::i18n::tr("model").into(),
        ];
        // Quiet segments are their label plus 20 DIP of padding.
        let split_width = split_labels
            .iter()
            .map(|label| {
                components::measure_text(
                    window.text_system(),
                    palette.font_family.clone(),
                    12.0,
                    FontWeight::SEMIBOLD,
                    label,
                ) + 20.0
            })
            .sum::<f32>();
        let legend = if by_model {
            self.model_legend(
                &chart,
                models.as_deref(),
                &hidden,
                cost_mode,
                CHART_WIDTH - split_width - 8.0,
                window,
                cx,
            )
        } else {
            self.series_legend(&chart, statistics, availability, selection, cx)
        };

        let model_hint: SharedString = if by_model && loading {
            crate::i18n::tr("loading-model-breakdown").into()
        } else if by_model && failed {
            crate::i18n::tr("model-data-unavailable").into()
        } else {
            crate::i18n::tr("group-tokens-or-cost-by-model").into()
        };
        let [type_label, model_label] = split_labels;
        let chart_key = chart.clone();
        let split_selector = self.segmented_control_quiet_ext(
            fx::key(("activity-split", chart.as_str())),
            vec![
                QuietSegment {
                    label: type_label,
                    tip: Some(crate::i18n::tr("split-by-token-type").into()),
                    disabled: false,
                },
                QuietSegment {
                    label: model_label,
                    tip: Some(model_hint),
                    disabled: false,
                },
            ],
            usize::from(by_model),
            move |this, index, _| {
                if let Some(state) = this.charts.get_mut(&chart_key) {
                    state.by_model = index == 1;
                    state.models_menu = false;
                }
            },
            window,
            cx,
        );
        let metric_segments = [false, true]
            .into_iter()
            .map(|cost| {
                let available = if cost {
                    availability.cost
                } else {
                    availability.series != 0
                };
                let tip = if !available && cost {
                    crate::i18n::tr("no-cost-data-for-this-period")
                } else if !available {
                    crate::i18n::tr("no-token-data-for-this-period")
                } else if cost {
                    crate::i18n::tr("daily-cost-in-usd")
                } else {
                    crate::i18n::tr("daily-token-volume")
                };
                QuietSegment {
                    label: if cost {
                        "$".into()
                    } else {
                        crate::i18n::tr("tokens").into()
                    },
                    tip: Some(tip.into()),
                    disabled: !available,
                }
            })
            .collect();
        let chart_key = chart.clone();
        let metric_selector = self.segmented_control_quiet_ext(
            fx::key(("activity-metric", chart.as_str())),
            metric_segments,
            usize::from(cost_mode),
            move |this, index, _| {
                if let Some(state) = this.charts.get_mut(&chart_key) {
                    state.requested.cost = index == 1;
                }
            },
            window,
            cx,
        );
        // The metric only changes what the bars measure, so it sits with the
        // totals; the footer keeps what the colors mean and how bars split.
        let header = usage_card_metrics(provider.kind(), statistics, &palette)
            .items_start()
            .child(div().flex_none().child(metric_selector));
        let footer = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .w_full()
            .h(px(28.0))
            .child(div().flex_1().min_w_0().child(legend))
            .child(split_selector);

        card(&palette)
            .p(px(12.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(header)
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

    /// One legend toggle: a filled dot while its series is drawn, a hollow
    /// ring while it is hidden, with a hover fill so it reads as clickable.
    #[allow(clippy::too_many_arguments)]
    fn legend_entry(
        &mut self,
        id: &str,
        label: String,
        color: Hsla,
        on: bool,
        available: bool,
        tip: SharedString,
        on_click: Option<LegendClick>,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let palette = self.palette.clone();
        let hover_id = fx::key(("legend-hover", id));
        let shown = self
            .fx
            .toggle(fx::key(("legend-on", id)), on && available, fx::FAST);
        let hover = self.fx.toggle(
            fx::key(("legend-hover-fx", id)),
            self.hovered(hover_id) && available,
            fx::FASTER,
        );
        let dot = div()
            .size(px(7.0))
            .rounded_full()
            .flex_none()
            .border_1()
            .border_color(palette.text_tertiary.mix(color, shown))
            .bg(color.opacity(shown));
        let mut entry = div()
            .id(eid(format!("legend-{id}")))
            .relative()
            .flex_none()
            .h(px(24.0))
            .px(px(4.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(5.0))
            .rounded(px(4.0))
            .opacity(if available { 1.0 } else { 0.4 })
            .on_hover(self.hover_listener(hover_id, Some(tip), cx));
        if let Some(layer) = components::hover_layer(&palette, hover, 4.0) {
            entry = entry.child(layer);
        }
        entry = entry.child(dot).child(nowrap(components::text(
            label,
            11.0,
            14.0,
            palette.text_tertiary.mix(palette.text_secondary, shown),
        )));
        if available && let Some(on_click) = on_click {
            entry =
                entry
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        on_click(this, cx);
                        cx.notify();
                    }));
        }
        entry
    }

    fn series_legend(
        &mut self,
        chart: &str,
        statistics: &UsageStatistics,
        availability: Availability,
        selection: Selection,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let mut legend = div().flex().flex_row().items_center().gap(px(LEGEND_GAP));
        for series in Series::ALL {
            let available = availability.series & (1 << series as u8) != 0;
            let tip = if available {
                crate::i18n::format(
                    "tokens-94e0b9",
                    &[
                        ("v0", series.menu_label().to_string()),
                        (
                            "v1",
                            format_token_count(series.value(&statistics.history)).to_string(),
                        ),
                    ],
                )
            } else {
                crate::i18n::format(
                    "no-tokens-in-this-period",
                    &[("v0", series.menu_label().to_lowercase())],
                )
            };
            let chart_key = chart.to_owned();
            let toggle: LegendClick = Box::new(move |this, _| {
                if let Some(state) = this.charts.get_mut(&chart_key) {
                    let current = Selection {
                        cost: availability.selection(state.requested).cost,
                        ..state.requested
                    };
                    state.requested = current.with_series(series, !current.selected(series));
                }
            });
            legend = legend.child(self.legend_entry(
                &format!("{chart}-{}", series as u8),
                series.label().to_owned(),
                series.color(&palette),
                selection.selected(series),
                available,
                tip.into(),
                Some(toggle),
                cx,
            ));
        }
        legend.into_any_element()
    }

    /// Model legend: as many entries as fit inline (largest first), the rest
    /// behind a `+N` chip whose menu toggles them the same way.
    #[allow(clippy::too_many_arguments)]
    fn model_legend(
        &mut self,
        chart: &str,
        models: Option<&ModelData>,
        hidden: &BTreeSet<String>,
        cost: bool,
        available: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let ranking = models
            .map(|models| models.ranking(cost))
            .unwrap_or_default();
        let labels: Vec<String> = ranking
            .iter()
            .map(|(model, _)| short_model_name(model))
            .collect();
        let measure = |text: &str, weight: FontWeight| {
            components::measure_text(
                window.text_system(),
                palette.font_family.clone(),
                11.0,
                weight,
                text,
            )
        };
        // Padding, dot and gap around each label; see `legend_entry`.
        let widths: Vec<f32> = labels
            .iter()
            .map(|label| measure(label, FontWeight::NORMAL) + 20.0)
            .collect();
        let inline = inline_legend_count(
            &widths,
            |rest| measure(&format!("+{rest}"), FontWeight::SEMIBOLD) + 12.0,
            available,
        );
        let menu_open = self
            .charts
            .get(chart)
            .is_some_and(|state| state.models_menu)
            && inline < ranking.len();
        let reveal = self
            .fx
            .toggle(fx::key(("models-menu", chart)), menu_open, fx::FAST);

        let mut entries = Vec::with_capacity(ranking.len());
        for ((model, usage), label) in ranking.iter().zip(labels) {
            let amount = if cost {
                cost_label(usage)
            } else {
                format_token_count(usage.total_tokens()).to_string()
            };
            let chart_key = chart.to_owned();
            let model_key = model.clone();
            let toggle: LegendClick = Box::new(move |this, _| {
                if let Some(state) = this.charts.get_mut(&chart_key)
                    && !state.hidden_models.remove(&model_key)
                {
                    state.hidden_models.insert(model_key.clone());
                }
            });
            entries.push(self.legend_entry(
                &format!("{chart}-model-{model}"),
                label,
                model_color(model, palette.dark),
                !hidden.contains(model),
                true,
                format!("{model} · {amount}").into(),
                Some(toggle),
                cx,
            ));
        }
        let rest = entries.split_off(inline.min(entries.len()));
        let mut legend = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(LEGEND_GAP))
            .children(entries);
        if rest.is_empty() {
            return legend.into_any_element();
        }

        let rest_hidden = ranking[inline..]
            .iter()
            .any(|(model, _)| hidden.contains(model));
        let chip_hover_id = fx::key(("models-chip", chart));
        let chip_hover = self.fx.toggle(
            fx::key(("models-chip-fx", chart)),
            self.hovered(chip_hover_id) || menu_open,
            fx::FASTER,
        );
        let chart_key = chart.to_owned();
        let mut chip = div()
            .id(eid(format!("models-chip-{chart}")))
            .relative()
            .flex_none()
            .h(px(20.0))
            .px(px(6.0))
            .flex()
            .items_center()
            .rounded(px(5.0))
            .bg(palette.control_fill)
            .cursor_pointer()
            .on_hover(self.hover_listener(chip_hover_id, None, cx))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if let Some(state) = this.charts.get_mut(&chart_key) {
                    state.models_menu = !state.models_menu;
                    cx.notify();
                }
            }));
        if let Some(layer) = components::hover_layer(&palette, chip_hover, 5.0) {
            chip = chip.child(layer);
        }
        // Dimmed while any model behind it is hidden from the chart.
        chip = chip.child(
            nowrap(caption(
                format!("+{}", rest.len()),
                if rest_hidden {
                    palette.text_tertiary
                } else {
                    palette.text_secondary
                },
            ))
            .font_weight(FontWeight::SEMIBOLD),
        );

        let mut holder = div().relative().flex_none().child(chip);
        if menu_open {
            let chart_key = chart.to_owned();
            let panel = div()
                .id(eid(format!("models-menu-{chart}")))
                .occlude()
                .mb(px(4.0 + (1.0 - reveal) * 4.0))
                .p(px(4.0))
                .flex()
                .flex_col()
                .rounded(px(8.0))
                // Reveal by motion only, like tooltips: the surface occludes
                // the chart from its first visible frame.
                .bg(palette.tooltip_background.alpha(1.0))
                .border_1()
                .border_color(palette.card_stroke.opacity(4.0))
                .shadow_md()
                .on_mouse_down_out(cx.listener(move |this, _: &gpui::MouseDownEvent, _, cx| {
                    // The chip's own click toggles the menu closed.
                    if this.hovered(chip_hover_id) {
                        return;
                    }
                    if let Some(state) = this.charts.get_mut(&chart_key) {
                        state.models_menu = false;
                        cx.notify();
                    }
                }))
                .children(rest);
            holder = holder.child(
                div().absolute().top_0().left_0().child(
                    gpui::deferred(
                        gpui::anchored()
                            .anchor(gpui::Corner::BottomLeft)
                            .snap_to_window_with_margin(px(8.0))
                            .child(panel),
                    )
                    .with_priority(2),
                ),
            );
        }
        legend = legend.child(holder);
        legend.into_any_element()
    }

    pub(super) fn activity_tip(
        &self,
        bucket: &Bucket,
        models: Option<&ModelData>,
        by_model: bool,
        cost: bool,
        page: usize,
        hidden: &BTreeSet<String>,
    ) -> ActivityTip {
        let title = bucket_title(bucket);
        let dark = self.palette.dark;
        if by_model && let Some(models) = models {
            let (rows, total) = models.page_rows(bucket.first, cost, dark, page, hidden);
            let page = page.min(models.pages(bucket.first, hidden).saturating_sub(1));
            let footer = if rows.is_empty() {
                Some(crate::i18n::tr("no-model-data").into())
            } else if total > MODEL_PAGE_SIZE {
                Some(crate::i18n::format(
                    "of-total-models-scroll-for-more",
                    &[
                        ("v0", (page * MODEL_PAGE_SIZE + 1).to_string()),
                        (
                            "v1",
                            (((page + 1) * MODEL_PAGE_SIZE).min(total)).to_string(),
                        ),
                        ("total", total.to_string()),
                    ],
                ))
            } else {
                None
            };
            return ActivityTip::Models {
                title,
                metric: if cost {
                    crate::i18n::tr("cost-usd-by-model").into()
                } else {
                    crate::i18n::tr("tokens-by-model").into()
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
            crate::i18n::tr("unavailable").into()
        };
        let requests = if usage.priced_requests > 0 && usage.priced_requests < usage.requests {
            crate::i18n::format(
                "requests-priced",
                &[
                    ("v0", usage.requests.to_string()),
                    ("v1", usage.priced_requests.to_string()),
                ],
            )
        } else {
            crate::i18n::format("requests", &[("v0", usage.requests.to_string())])
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
                crate::i18n::tr("today").into(),
                spend(&statistics.today),
                crate::i18n::format("requests", &[("v0", statistics.today.requests.to_string())]),
                palette,
            ))
            .child(metric_cell(
                crate::i18n::format("last-period-days", &[("period", period.to_string())]),
                spend(&statistics.history),
                crate::i18n::format(
                    "requests",
                    &[("v0", statistics.history.requests.to_string())],
                ),
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
                .unwrap_or_else(|| crate::i18n::tr("no-data").into())
        )
    };
    row.child(metric_cell(
        crate::i18n::tr("today").into(),
        format_token_count(statistics.today.total_tokens()),
        value(&statistics.today),
        palette,
    ))
    .child(metric_cell(
        crate::i18n::format("last-period-days", &[("period", period.to_string())]),
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

#[cfg(test)]
mod legend_tests {
    use super::*;

    #[test]
    fn short_model_names_drop_vendor_and_date() {
        assert_eq!(short_model_name("claude-sonnet-4-5-20250929"), "Sonnet 4.5");
        assert_eq!(short_model_name("claude-opus-4-1"), "Opus 4.1");
        assert_eq!(short_model_name("openai/gpt-5-codex"), "GPT-5 Codex");
        assert_eq!(short_model_name("gpt-5.1-codex-max"), "GPT-5.1 Codex Max");
        assert_eq!(short_model_name("anthropic/claude-haiku-4.5"), "Haiku 4.5");
        assert_eq!(short_model_name("gemini-2.5-pro"), "Gemini 2.5 Pro");
        assert_eq!(short_model_name("claude-4.5-sonnet"), "Claude 4.5 Sonnet");
        assert_eq!(short_model_name("o3"), "O3");
    }

    #[test]
    fn inline_legend_leaves_room_for_the_overflow_chip() {
        let chip = |_| 20.0;
        assert_eq!(inline_legend_count(&[50.0, 50.0], chip, 102.0), 2);
        // 50 + 2 + 50 + 2 + 20 = 124 fits; a third entry does not.
        assert_eq!(inline_legend_count(&[50.0, 50.0, 50.0], chip, 130.0), 2);
        assert_eq!(inline_legend_count(&[50.0, 50.0, 50.0], chip, 60.0), 0);
        assert_eq!(inline_legend_count(&[], chip, 10.0), 0);
    }

    #[test]
    fn hidden_models_leave_bars_and_tips() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 6).unwrap();
        let usage = |tokens| TokenUsage {
            input_tokens: tokens,
            requests: 1,
            ..Default::default()
        };
        let bounds = [Bucket {
            first: day,
            last: day,
            usage: TokenUsage::default(),
        }];
        let models = group_models(
            vec![("a".into(), day, usage(10)), ("b".into(), day, usage(30))],
            &bounds,
            ProviderKind::Codex,
        );
        let none = BTreeSet::new();
        let hidden = BTreeSet::from(["b".to_owned()]);
        assert_eq!(models.value(day, false, &none), 40);
        assert_eq!(models.value(day, false, &hidden), 10);
        assert_eq!(models.page_rows(day, false, true, 0, &hidden).1, 1);
        let ranking: Vec<_> = models.ranking(false).into_iter().map(|(m, _)| m).collect();
        assert_eq!(ranking, ["b", "a"]);
    }

    #[test]
    fn cache_starts_hidden_unless_it_is_the_only_series() {
        let requested = ChartState::new(false).requested;
        let all = Availability {
            series: 0b111,
            cost: true,
        };
        assert_eq!(all.selection(requested).mask, DEFAULT_SERIES);
        let cache_only = Availability {
            series: 0b010,
            cost: false,
        };
        assert_eq!(cache_only.selection(requested).mask, 0b010);
    }
}
