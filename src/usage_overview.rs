//! Cross-provider usage aggregation for the popup Usage tab.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate};

use crate::{
    instances::ProviderId,
    limits::ProviderLimits,
    provider_registry,
    provider_registry::ProviderKind,
    settings::TotalSpendPeriod,
    store::{self},
    usage::{DailyTokenUsage, TokenUsage},
};

pub const OVERVIEW_MAX_DAYS: u16 = 90;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverviewMetric {
    #[default]
    Cost,
    Tokens,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverviewRange {
    Past24h,
    SevenDays,
    #[default]
    ThirtyDays,
    NinetyDays,
}

impl OverviewRange {
    pub const fn days(self) -> u16 {
        match self {
            Self::Past24h => 1,
            Self::SevenDays => 7,
            Self::ThirtyDays => 30,
            Self::NinetyDays => 90,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Past24h => crate::i18n::tr("past-24h"),
            Self::SevenDays => crate::i18n::tr("msg-7-days"),
            Self::ThirtyDays => crate::i18n::tr("msg-30-days"),
            Self::NinetyDays => crate::i18n::tr("msg-90-days"),
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::Past24h => crate::i18n::tr("range-24h"),
            Self::SevenDays => crate::i18n::tr("range-7d"),
            Self::ThirtyDays => crate::i18n::tr("range-30d"),
            Self::NinetyDays => crate::i18n::tr("range-90d"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BreakdownMode {
    #[default]
    Model,
    Project,
    Day,
}

#[derive(Clone, Debug, Default)]
pub struct ProviderOverview {
    pub provider: ProviderId,
    pub sessions: u64,
    pub usage: TokenUsage,
    pub share_cost: f64,
    pub share_tokens: f64,
    /// Toggled off on the Usage tab: the tile keeps its own numbers, but the
    /// provider is left out of every total, share, chart and breakdown.
    pub excluded: bool,
}

#[derive(Clone, Debug)]
pub struct DailySeriesPoint {
    pub at: DateTime<Local>,
    pub date: NaiveDate,
    pub by_provider: BTreeMap<ProviderId, u64>,
    pub total: u64,
}

#[derive(Clone, Debug, Default)]
pub struct BreakdownRow {
    pub label: String,
    pub weekday: Option<String>,
    pub provider: Option<ProviderId>,
    pub cost_microusd: u64,
    pub tokens: u64,
    pub requests: u64,
    pub priced_requests: u64,
    pub share: f64,
    pub by_provider: BTreeMap<ProviderId, TokenUsage>,
    /// Project row collecting usage whose working directory is unknown.
    pub unknown: bool,
    /// Project folder the row opens when clicked; only set for folders that
    /// exist on this PC.
    pub path: Option<String>,
}

/// Projects that cost less than this in the selected range are left out of
/// the project breakdown: ten cents, in micro-USD.
const MIN_PROJECT_COST_MICROUSD: u64 = 100_000;

#[derive(Clone, Debug, Default)]
pub struct OverviewSnapshot {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub hourly: bool,
    pub total_sessions: u64,
    pub totals: TokenUsage,
    pub providers: Vec<ProviderOverview>,
    pub daily_series: Vec<DailySeriesPoint>,
    pub model_rows: Vec<BreakdownRow>,
    /// One row per repository; `by_provider` holds each provider's share.
    pub project_rows: Vec<BreakdownRow>,
    pub day_rows: Vec<BreakdownRow>,
}

/// Calendar window for the Home Usage Stats card. Thirty days matches the
/// Usage tab's 30-day Cost snapshot exactly; today/yesterday are slices of
/// that same store aggregation.
pub fn dates_for_total_spend(period: TotalSpendPeriod) -> (NaiveDate, NaiveDate) {
    let today = Local::now().date_naive();
    match period {
        TotalSpendPeriod::Today => (today, today),
        TotalSpendPeriod::Yesterday => {
            let yesterday = today - Duration::days(1);
            (yesterday, yesterday)
        }
        TotalSpendPeriod::ThirtyDays => {
            let start = today
                - Duration::days(i64::from(
                    OverviewRange::ThirtyDays.days().saturating_sub(1),
                ));
            (start, today)
        }
    }
}

pub fn spend_entries(snapshot: &OverviewSnapshot) -> Vec<(ProviderId, u64)> {
    let mut entries: Vec<_> = snapshot
        .providers
        .iter()
        .map(|entry| (entry.provider, entry.usage.estimated_cost_microusd))
        .collect();
    entries.sort_by(|(_, left), (_, right)| right.cmp(left));
    entries
}

pub fn total_spend_snapshot(
    limits: &ProviderLimits,
    enabled: &[ProviderId],
    period: TotalSpendPeriod,
) -> OverviewSnapshot {
    match period {
        TotalSpendPeriod::ThirtyDays => build_overview_snapshot(
            limits,
            enabled,
            OverviewMetric::Cost,
            OverviewRange::ThirtyDays,
        ),
        TotalSpendPeriod::Today | TotalSpendPeriod::Yesterday => {
            let (start_date, end_date) = dates_for_total_spend(period);
            build_overview_snapshot_for_dates(
                limits,
                enabled,
                OverviewMetric::Cost,
                start_date,
                end_date,
            )
        }
    }
}

pub fn build_overview_snapshot(
    limits: &ProviderLimits,
    enabled: &[ProviderId],
    metric: OverviewMetric,
    range: OverviewRange,
) -> OverviewSnapshot {
    let now = Local::now();
    let hourly = range == OverviewRange::Past24h;
    let (start_hour, _) = hour_window(now);
    let end_date = now.date_naive();
    let start_date = if hourly {
        start_hour.date_naive()
    } else {
        end_date - Duration::days(i64::from(range.days().saturating_sub(1)))
    };
    assemble_overview_snapshot(limits, enabled, metric, start_date, end_date, hourly)
}

fn build_overview_snapshot_for_dates(
    limits: &ProviderLimits,
    enabled: &[ProviderId],
    metric: OverviewMetric,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> OverviewSnapshot {
    assemble_overview_snapshot(limits, enabled, metric, start_date, end_date, false)
}

/// The 24 local hours ending with the current one, as `(first, last)`.
fn hour_window(now: DateTime<Local>) -> (DateTime<Local>, DateTime<Local>) {
    let end_hour = crate::local_time::truncate_hour(now);
    (end_hour - Duration::hours(23), end_hour)
}

/// One provider's stored usage for the snapshot window.
#[derive(Default)]
struct ProviderData {
    daily: Vec<DailyTokenUsage>,
    hourly: BTreeMap<DateTime<Local>, TokenUsage>,
    sessions: u64,
    models: Vec<(String, TokenUsage)>,
    projects: Vec<(String, TokenUsage)>,
}

fn load_provider_data(
    store: &store::ProviderStore,
    provider: ProviderId,
    load_days: u16,
    (start_date, end_date): (NaiveDate, NaiveDate),
    hours: Option<(DateTime<Local>, DateTime<Local>)>,
) -> anyhow::Result<ProviderData> {
    let mut data = ProviderData {
        daily: store.load_usage_daily(provider, load_days)?.daily,
        sessions: store.count_session_paths(provider, start_date, end_date)?,
        projects: store.load_project_breakdown(provider, start_date, end_date)?,
        ..Default::default()
    };
    match hours {
        // OpenRouter keeps hours per model in its analytics cache, aligned to
        // the chart's own hour grid.
        Some((start, end)) if provider.kind() == ProviderKind::OpenRouter => {
            for (model, at, usage) in
                crate::openrouter::analytics::hourly_rows(store, provider, start, end)?
            {
                data.hourly.entry(at).or_default().add(&usage);
                data.models.push((model, usage));
            }
        }
        Some((start, end)) => {
            data.hourly = store.load_usage_hourly(provider, start, end)?;
            data.models = store.load_model_breakdown(provider, start_date, end_date)?;
        }
        None => data.models = store.load_model_breakdown(provider, start_date, end_date)?,
    }
    Ok(data)
}

fn assemble_overview_snapshot(
    limits: &ProviderLimits,
    enabled: &[ProviderId],
    metric: OverviewMetric,
    start_date: NaiveDate,
    end_date: NaiveDate,
    hourly: bool,
) -> OverviewSnapshot {
    let now = Local::now();
    let (start_hour, end_hour) = hour_window(now);
    // statistics_from_daily is always anchored to today, so load enough days
    // to include `start_date` even when the window ends on yesterday.
    let load_days = if hourly {
        2
    } else {
        let span = (now.date_naive() - start_date).num_days() + 1;
        u16::try_from(span.max(1))
            .unwrap_or(OVERVIEW_MAX_DAYS)
            .min(OVERVIEW_MAX_DAYS)
    };

    let mut snapshot = OverviewSnapshot {
        start_date,
        end_date,
        hourly,
        ..Default::default()
    };

    let spend_providers: Vec<ProviderId> = enabled
        .iter()
        .copied()
        .filter(|provider| provider_registry::descriptor(provider.kind()).include_in_total_spend)
        .collect();

    if spend_providers.is_empty() {
        return snapshot;
    }

    // A provider whose rows cannot be read shows as empty; the others stay.
    let data: BTreeMap<ProviderId, ProviderData> = store::with_store(|store| {
        Ok(spend_providers
            .iter()
            .map(|&provider| {
                let data = load_provider_data(
                    store,
                    provider,
                    load_days,
                    (start_date, end_date),
                    hourly.then_some((start_hour, end_hour)),
                )
                .unwrap_or_else(|error| {
                    crate::logger::info(format!(
                        "failed to read {} usage for the overview: {error:#}",
                        provider.id()
                    ));
                    ProviderData::default()
                });
                (provider, data)
            })
            .collect())
    })
    .unwrap_or_else(|error| {
        crate::logger::info(format!("failed to open usage for the overview: {error:#}"));
        BTreeMap::new()
    });

    let mut daily_by_date: BTreeMap<NaiveDate, BTreeMap<ProviderId, TokenUsage>> = BTreeMap::new();
    for (provider, provider_data) in &data {
        for entry in &provider_data.daily {
            if entry.date < start_date || entry.date > end_date {
                continue;
            }
            daily_by_date
                .entry(entry.date)
                .or_default()
                .entry(*provider)
                .or_default()
                .add(&entry.usage);
        }
    }

    for provider in &spend_providers {
        let provider_data = data.get(provider);
        let usage = match provider_data {
            // No daily fallback: calendar days would pull in activity older
            // than 24 hours. Providers without hourly rows show zero here.
            Some(provider_data) if hourly => sum_usage(provider_data.hourly.values()),
            Some(provider_data) => slice_provider_usage(&provider_data.daily, start_date, end_date),
            None => TokenUsage::default(),
        };
        let tracked = provider_data.map_or(0, |provider_data| provider_data.sessions);
        // Codex/Claude have real session files or event paths. Cursor (and
        // anyone else with only a daily rollup) never writes those tables —
        // its CSV rows already live in `requests`. A stored 0 is not "unknown".
        let sessions = if tracked > 0 { tracked } else { usage.requests };
        snapshot.providers.push(ProviderOverview {
            provider: *provider,
            sessions,
            usage,
            ..Default::default()
        });
    }
    // Keep in-memory limits as a fallback when the store has not hydrated yet.
    if snapshot
        .providers
        .iter()
        .all(|entry| entry.usage.requests == 0)
    {
        for entry in &mut snapshot.providers {
            entry.usage = slice_provider_usage(
                limits.get(entry.provider).usage.daily.as_slice(),
                start_date,
                end_date,
            );
            entry.sessions = entry.usage.requests;
        }
    }
    update_provider_totals(&mut snapshot, &BTreeSet::new());
    snapshot.providers.sort_by(|left, right| {
        right
            .usage
            .estimated_cost_microusd
            .cmp(&left.usage.estimated_cost_microusd)
    });

    let total_metric = metric_value(metric, &snapshot.totals).max(1);

    if hourly {
        snapshot.day_rows = (0..24)
            .rev()
            .map(|offset| {
                let at = start_hour + Duration::hours(offset);
                let by_provider: BTreeMap<ProviderId, TokenUsage> = data
                    .iter()
                    .filter_map(|(provider, provider_data)| {
                        Some((*provider, provider_data.hourly.get(&at)?.clone()))
                    })
                    .collect();
                let mut row = BreakdownRow {
                    label: format_hour_label(at),
                    by_provider,
                    ..Default::default()
                };
                sum_row_providers(&mut row);
                row.share = share(row_metric(metric, &row), total_metric);
                row
            })
            .filter(|row| row.tokens > 0 || row.cost_microusd > 0)
            .collect();

        snapshot.daily_series = (0..24)
            .map(|offset| {
                let at = start_hour + Duration::hours(offset);
                let by_provider: BTreeMap<ProviderId, u64> = data
                    .iter()
                    .filter_map(|(provider, provider_data)| {
                        let value = metric_value(metric, provider_data.hourly.get(&at)?);
                        (value > 0).then_some((*provider, value))
                    })
                    .collect();
                DailySeriesPoint {
                    at,
                    date: at.date_naive(),
                    total: sum_values(by_provider.values()),
                    by_provider,
                }
            })
            .collect();
    } else {
        snapshot.day_rows = daily_by_date
            .iter()
            .rev()
            .map(|(date, providers)| {
                let mut row = BreakdownRow {
                    label: crate::i18n::month_day(*date),
                    weekday: Some(weekday_short(*date).to_owned()),
                    by_provider: providers.clone(),
                    ..Default::default()
                };
                sum_row_providers(&mut row);
                row.share = share(row_metric(metric, &row), total_metric);
                row
            })
            .collect();

        snapshot.daily_series = daily_by_date
            .into_iter()
            .map(|(date, by_provider)| {
                let by_provider: BTreeMap<ProviderId, u64> = by_provider
                    .iter()
                    .map(|(provider, usage)| (*provider, metric_value(metric, usage)))
                    .collect();
                DailySeriesPoint {
                    at: crate::local_time::start_of_local_day(date),
                    date,
                    total: sum_values(by_provider.values()),
                    by_provider,
                }
            })
            .collect();
    }

    let mut model_rows = BTreeMap::<(ProviderId, String), TokenUsage>::new();
    let mut project_cwds = Vec::<(ProviderId, String, TokenUsage)>::new();
    for (provider, provider_data) in data {
        for (model, usage) in provider_data.models {
            let model = if provider.kind() == ProviderKind::Cursor {
                crate::cursor::normalize_cursor_model_name(&model)
            } else {
                model
            };
            model_rows.entry((provider, model)).or_default().add(&usage);
        }
        project_cwds.extend(
            provider_data
                .projects
                .into_iter()
                .map(|(cwd, usage)| (provider, cwd, usage)),
        );
    }
    snapshot.model_rows = model_rows
        .into_iter()
        .map(|((provider, model), usage)| BreakdownRow {
            label: model,
            provider: Some(provider),
            cost_microusd: usage.estimated_cost_microusd,
            tokens: usage.total_tokens(),
            requests: usage.requests,
            priced_requests: usage.priced_requests,
            share: share(metric_value(metric, &usage), total_metric),
            ..Default::default()
        })
        .collect();
    snapshot
        .model_rows
        .sort_by_key(|row| std::cmp::Reverse(row_metric(metric, row)));
    snapshot.project_rows = project_rows(project_cwds, metric, total_metric);
    snapshot
}

fn metric_value(metric: OverviewMetric, usage: &TokenUsage) -> u64 {
    match metric {
        OverviewMetric::Cost => usage.estimated_cost_microusd,
        OverviewMetric::Tokens => usage.total_tokens(),
    }
}

fn row_metric(metric: OverviewMetric, row: &BreakdownRow) -> u64 {
    match metric {
        OverviewMetric::Cost => row.cost_microusd,
        OverviewMetric::Tokens => row.tokens,
    }
}

/// Percentage of `total`, which callers keep at least 1.
fn share(value: u64, total: u64) -> f64 {
    value as f64 / total as f64 * 100.0
}

fn sum_usage<'a>(usages: impl IntoIterator<Item = &'a TokenUsage>) -> TokenUsage {
    usages
        .into_iter()
        .fold(TokenUsage::default(), |mut total, usage| {
            total.add(usage);
            total
        })
}

fn sum_values<'a>(values: impl IntoIterator<Item = &'a u64>) -> u64 {
    values
        .into_iter()
        .fold(0_u64, |sum, value| sum.saturating_add(*value))
}

/// Recomputes totals, sessions and both shares from the providers that are
/// not `excluded`; excluded ones get no share.
fn update_provider_totals(snapshot: &mut OverviewSnapshot, excluded: &BTreeSet<ProviderId>) {
    snapshot.totals = TokenUsage::default();
    snapshot.total_sessions = 0;
    for entry in &mut snapshot.providers {
        entry.excluded = excluded.contains(&entry.provider);
        if !entry.excluded {
            snapshot.totals.add(&entry.usage);
            snapshot.total_sessions = snapshot.total_sessions.saturating_add(entry.sessions);
        }
    }
    let total_cost = snapshot.totals.estimated_cost_microusd.max(1);
    let total_tokens = snapshot.totals.total_tokens().max(1);
    for entry in &mut snapshot.providers {
        (entry.share_cost, entry.share_tokens) = if entry.excluded {
            (0.0, 0.0)
        } else {
            (
                share(entry.usage.estimated_cost_microusd, total_cost),
                share(entry.usage.total_tokens(), total_tokens),
            )
        };
    }
}

/// Leaves `excluded` providers out of a built snapshot without touching the
/// store: totals, shares, the chart series and both breakdowns are recomputed
/// from the remaining providers. Excluded tiles keep their own usage.
pub fn exclude_providers(
    snapshot: &OverviewSnapshot,
    excluded: &BTreeSet<ProviderId>,
    metric: OverviewMetric,
) -> OverviewSnapshot {
    let mut snapshot = snapshot.clone();
    if excluded.is_empty() {
        return snapshot;
    }
    update_provider_totals(&mut snapshot, excluded);
    let total_metric = metric_value(metric, &snapshot.totals).max(1);

    for point in &mut snapshot.daily_series {
        point
            .by_provider
            .retain(|provider, _| !excluded.contains(provider));
        point.total = sum_values(point.by_provider.values());
    }
    for row in &mut snapshot.day_rows {
        row.by_provider
            .retain(|provider, _| !excluded.contains(provider));
        sum_row_providers(row);
        row.share = share(row_metric(metric, row), total_metric);
    }
    snapshot.day_rows.retain(|row| !row.by_provider.is_empty());
    snapshot.model_rows.retain(|row| {
        row.provider
            .is_none_or(|provider| !excluded.contains(&provider))
    });
    for row in &mut snapshot.model_rows {
        row.share = share(row_metric(metric, row), total_metric);
    }
    for row in &mut snapshot.project_rows {
        row.by_provider
            .retain(|provider, _| !excluded.contains(provider));
        sum_row_providers(row);
        row.share = share(row_metric(metric, row), total_metric);
    }
    snapshot
        .project_rows
        .retain(|row| row.cost_microusd >= MIN_PROJECT_COST_MICROUSD);
    sort_rows(&mut snapshot.project_rows, metric);
    snapshot
}

/// Folds `(provider, cwd, usage)` rows into one row per repository, largest
/// first. Usage without a known working directory is collected last.
fn project_rows(
    cwds: Vec<(ProviderId, String, TokenUsage)>,
    metric: OverviewMetric,
    total_metric: u64,
) -> Vec<BreakdownRow> {
    let mut projects =
        BTreeMap::<String, (String, String, BTreeMap<ProviderId, TokenUsage>)>::new();
    for (provider, cwd, usage) in cwds {
        let (key, name, root) = match crate::usage_projects::resolve(&cwd) {
            Some(project) => (project.key, project.name, project.root),
            None => (String::new(), String::new(), String::new()),
        };
        projects
            .entry(key)
            .or_insert_with(|| (name, root, BTreeMap::new()))
            .2
            .entry(provider)
            .or_default()
            .add(&usage);
    }
    let mut name_counts = BTreeMap::<String, usize>::new();
    for (key, (name, _, _)) in &projects {
        if !key.is_empty() {
            *name_counts.entry(name.to_lowercase()).or_default() += 1;
        }
    }
    let mut rows: Vec<BreakdownRow> = projects
        .into_iter()
        .map(|(key, (name, root, by_provider))| {
            let label = if key.is_empty() {
                crate::i18n::tr("unknown-project").to_owned()
            } else if name_counts.get(&name.to_lowercase()).copied().unwrap_or(0) > 1 {
                // Same folder name in two places: prefix the parent folder.
                disambiguated_name(&root, &name)
            } else {
                name
            };
            let mut row = BreakdownRow {
                label,
                unknown: key.is_empty(),
                path: (!key.is_empty() && std::path::Path::new(&root).is_dir())
                    .then(|| root.clone()),
                by_provider,
                ..Default::default()
            };
            sum_row_providers(&mut row);
            row.share = share(row_metric(metric, &row), total_metric.max(1));
            row
        })
        .filter(|row| row.cost_microusd >= MIN_PROJECT_COST_MICROUSD)
        .collect();
    sort_rows(&mut rows, metric);
    rows
}

fn disambiguated_name(root: &str, name: &str) -> String {
    std::path::Path::new(root)
        .parent()
        .and_then(|parent| parent.file_name())
        .map(|parent| format!("{}/{name}", parent.to_string_lossy()))
        .unwrap_or_else(|| root.to_owned())
}

/// Recomputes a row's totals from its per-provider usage.
fn sum_row_providers(row: &mut BreakdownRow) {
    let usage = sum_usage(row.by_provider.values());
    row.cost_microusd = usage.estimated_cost_microusd;
    row.tokens = usage.total_tokens();
    row.requests = usage.requests;
    row.priced_requests = usage.priced_requests;
}

/// Largest first by the active metric; the unknown-project row always sinks
/// to the bottom.
fn sort_rows(rows: &mut [BreakdownRow], metric: OverviewMetric) {
    rows.sort_by(|left, right| {
        left.unknown
            .cmp(&right.unknown)
            .then_with(|| row_metric(metric, right).cmp(&row_metric(metric, left)))
    });
}

fn weekday_short(date: NaiveDate) -> &'static str {
    crate::i18n::weekday(date.weekday())
}

pub fn format_hour_label(at: DateTime<Local>) -> String {
    crate::settings::TimeFormat::current().format_hour_label(at)
}

fn slice_provider_usage(days: &[DailyTokenUsage], start: NaiveDate, end: NaiveDate) -> TokenUsage {
    sum_usage(
        days.iter()
            .filter(|entry| entry.date >= start && entry.date <= end)
            .map(|entry| &entry.usage),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_spend_periods_match_usage_tab_ranges() {
        let today = Local::now().date_naive();
        let yesterday = today - Duration::days(1);
        assert_eq!(
            dates_for_total_spend(TotalSpendPeriod::Today),
            (today, today)
        );
        assert_eq!(
            dates_for_total_spend(TotalSpendPeriod::Yesterday),
            (yesterday, yesterday)
        );
        let thirty_days = Duration::days(i64::from(
            OverviewRange::ThirtyDays.days().saturating_sub(1),
        ));
        assert_eq!(
            dates_for_total_spend(TotalSpendPeriod::ThirtyDays),
            (today - thirty_days, today)
        );
    }

    #[test]
    fn excluded_providers_leave_every_aggregate() {
        let claude = crate::instances::ProviderId::from(ProviderKind::Claude);
        let codex = crate::instances::ProviderId::from(ProviderKind::Codex);
        let usage = |cost| TokenUsage {
            estimated_cost_microusd: cost,
            input_tokens: cost,
            requests: 1,
            ..Default::default()
        };
        let date = NaiveDate::from_ymd_opt(2026, 10, 6).unwrap();
        let mut totals = usage(100);
        totals.add(&usage(300));
        let snapshot = OverviewSnapshot {
            total_sessions: 5,
            totals,
            providers: vec![
                ProviderOverview {
                    provider: claude,
                    sessions: 2,
                    usage: usage(100),
                    ..Default::default()
                },
                ProviderOverview {
                    provider: codex,
                    sessions: 3,
                    usage: usage(300),
                    ..Default::default()
                },
            ],
            daily_series: vec![DailySeriesPoint {
                at: crate::local_time::start_of_local_day(date),
                date,
                by_provider: BTreeMap::from([(claude, 100), (codex, 300)]),
                total: 400,
            }],
            day_rows: vec![
                BreakdownRow {
                    by_provider: BTreeMap::from([(claude, usage(100)), (codex, usage(300))]),
                    ..Default::default()
                },
                BreakdownRow {
                    by_provider: BTreeMap::from([(codex, usage(50))]),
                    ..Default::default()
                },
            ],
            model_rows: vec![
                BreakdownRow {
                    provider: Some(claude),
                    cost_microusd: 100,
                    ..Default::default()
                },
                BreakdownRow {
                    provider: Some(codex),
                    cost_microusd: 300,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let filtered = exclude_providers(&snapshot, &BTreeSet::from([codex]), OverviewMetric::Cost);
        assert_eq!(filtered.totals.estimated_cost_microusd, 100);
        assert_eq!(filtered.total_sessions, 2);
        assert!(filtered.providers[1].excluded);
        assert_eq!(filtered.providers[1].usage.estimated_cost_microusd, 300);
        assert_eq!(filtered.providers[0].share_cost, 100.0);
        assert_eq!(filtered.daily_series[0].total, 100);
        assert_eq!(filtered.day_rows.len(), 1);
        assert_eq!(filtered.day_rows[0].cost_microusd, 100);
        assert_eq!(filtered.day_rows[0].share, 100.0);
        assert_eq!(filtered.model_rows.len(), 1);
        assert_eq!(filtered.model_rows[0].provider, Some(claude));
    }

    #[test]
    fn spend_entries_use_overview_provider_costs() {
        let snapshot = OverviewSnapshot {
            providers: vec![
                ProviderOverview {
                    provider: crate::instances::ProviderId::from(ProviderKind::Claude),
                    usage: TokenUsage {
                        estimated_cost_microusd: 500_000,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ProviderOverview {
                    provider: crate::instances::ProviderId::from(ProviderKind::Codex),
                    usage: TokenUsage {
                        estimated_cost_microusd: 2_000_000,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(
            spend_entries(&snapshot),
            vec![
                (ProviderKind::Codex.into(), 2_000_000),
                (ProviderKind::Claude.into(), 500_000),
            ]
        );
    }

    #[test]
    fn project_rows_hide_projects_under_ten_cents() {
        let codex = ProviderId::primary(ProviderKind::Codex);
        let claude = ProviderId::primary(ProviderKind::Claude);
        let cost = |microusd| TokenUsage {
            input_tokens: 10,
            requests: 1,
            estimated_cost_microusd: microusd,
            priced_requests: 1,
            ..Default::default()
        };
        let rows = project_rows(
            vec![
                (codex, "big-app".into(), cost(60_000)),
                (claude, "big-app".into(), cost(50_000)),
                (codex, "tiny-app".into(), cost(99_999)),
                (codex, String::new(), cost(200_000)),
            ],
            OverviewMetric::Cost,
            310_000,
        );
        let labels = rows
            .iter()
            .map(|row| (row.label.as_str(), row.cost_microusd, row.unknown))
            .collect::<Vec<_>>();
        assert_eq!(
            labels,
            vec![
                ("big-app", 110_000, false),
                (crate::i18n::tr("unknown-project"), 200_000, true),
            ]
        );
        assert_eq!(rows[0].by_provider.len(), 2);
        // A name is not a folder on this PC, so the row is not clickable.
        assert_eq!(rows[0].path, None);
    }
}
