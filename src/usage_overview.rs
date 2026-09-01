//! Cross-provider usage aggregation for the popup Usage tab.

use std::collections::BTreeMap;

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Utc, Weekday};

use crate::{
    limits::ProviderLimits,
    provider_registry,
    settings::{ProviderKind, TotalSpendPeriod},
    store::{self},
    usage::TokenUsage,
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

    pub const fn label(self) -> &'static str {
        match self {
            Self::Past24h => "Past 24h",
            Self::SevenDays => "7 days",
            Self::ThirtyDays => "30 days",
            Self::NinetyDays => "90 days",
        }
    }

    pub const fn from_total_spend_period(period: TotalSpendPeriod) -> Self {
        match period {
            TotalSpendPeriod::Past24h => Self::Past24h,
            TotalSpendPeriod::SevenDays => Self::SevenDays,
            TotalSpendPeriod::ThirtyDays => Self::ThirtyDays,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BreakdownMode {
    #[default]
    Model,
    Day,
}

#[derive(Clone, Debug, Default)]
pub struct ProviderOverview {
    pub provider: ProviderKind,
    pub sessions: u64,
    pub usage: TokenUsage,
    pub share_cost: f64,
    pub share_tokens: f64,
}

#[derive(Clone, Debug)]
pub struct DailySeriesPoint {
    pub at: DateTime<Local>,
    pub date: NaiveDate,
    pub by_provider: BTreeMap<ProviderKind, u64>,
    pub total: u64,
}

#[derive(Clone, Debug, Default)]
pub struct BreakdownRow {
    pub label: String,
    pub weekday: Option<String>,
    pub provider: Option<ProviderKind>,
    pub cost_microusd: u64,
    pub tokens: u64,
    pub share: f64,
    pub by_provider: BTreeMap<ProviderKind, TokenUsage>,
}

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
    pub day_rows: Vec<BreakdownRow>,
}

pub fn build_overview_snapshot(
    limits: &ProviderLimits,
    enabled: &[ProviderKind],
    metric: OverviewMetric,
    range: OverviewRange,
) -> OverviewSnapshot {
    let now = Local::now();
    let hourly = range == OverviewRange::Past24h;
    let end_hour = crate::usage::truncate_local_hour(now);
    let start_hour = end_hour - Duration::hours(23);
    let end_date = now.date_naive();
    let start_date = if hourly {
        start_hour.date_naive()
    } else {
        end_date - Duration::days(i64::from(range.days().saturating_sub(1)))
    };
    let load_days = if hourly {
        2
    } else {
        range.days().min(OVERVIEW_MAX_DAYS)
    };

    let mut snapshot = OverviewSnapshot {
        start_date,
        end_date,
        hourly,
        ..Default::default()
    };

    let spend_providers: Vec<ProviderKind> = enabled
        .iter()
        .copied()
        .filter(|provider| {
            provider_registry::PROVIDERS
                .iter()
                .any(|descriptor| descriptor.kind == *provider && descriptor.include_in_total_spend)
        })
        .collect();

    if spend_providers.is_empty() {
        return snapshot;
    }

    let store_data = store::with_store(|store| {
        let mut provider_daily = BTreeMap::new();
        let mut provider_hourly = BTreeMap::new();
        let mut provider_sessions = BTreeMap::new();
        let mut model_rows = BTreeMap::<(ProviderKind, String), TokenUsage>::new();
        for provider in &spend_providers {
            let statistics = store
                .load_usage_daily(*provider, load_days)
                .unwrap_or_default();
            provider_daily.insert(*provider, statistics.daily);
            if hourly {
                provider_hourly.insert(
                    *provider,
                    store
                        .load_usage_hourly(*provider, start_hour, end_hour)
                        .unwrap_or_default(),
                );
            }
            provider_sessions.insert(
                *provider,
                store
                    .count_session_paths(*provider, start_date, end_date)
                    .unwrap_or(0),
            );
            for (model, usage) in store
                .load_model_breakdown(*provider, start_date, end_date)
                .unwrap_or_default()
            {
                let model = if *provider == ProviderKind::Cursor {
                    crate::cursor::normalize_cursor_model_name(&model)
                } else {
                    model
                };
                model_rows
                    .entry((*provider, model))
                    .or_default()
                    .add(&usage);
            }
        }
        Ok((provider_daily, provider_hourly, provider_sessions, model_rows))
    })
    .unwrap_or_default();

    let (mut provider_daily, mut provider_hourly, provider_sessions, mut model_rows) = store_data;
    // The live limits snapshot is the only safe fallback while the SQLite
    // usage tables are still being hydrated. Keep it in the same per-provider
    // input map so totals, charts, and breakdowns all receive identical data.
    for provider in &spend_providers {
        let has_store_data = provider_daily
            .get(provider)
            .is_some_and(|days| days.iter().any(|entry| usage_has_data(&entry.usage)));
        if !has_store_data {
            let fallback = limits.get(*provider).usage.daily.clone();
            if !fallback.is_empty() {
                provider_daily.insert(*provider, fallback);
            }
        }
    }
    let codex_has_usage = provider_daily
        .get(&ProviderKind::Codex)
        .is_some_and(|days| {
            days.iter().any(|entry| {
                entry.date >= start_date && entry.date <= end_date && usage_has_data(&entry.usage)
            })
        });
    let codex_missing_models = !model_rows
        .keys()
        .any(|(provider, _)| *provider == ProviderKind::Codex);
    if spend_providers.contains(&ProviderKind::Codex) && codex_has_usage && codex_missing_models {
        // Incremental Codex saves used to wipe usage_model_daily. Rebuild
        // from session logs instead of asking the user to delete the store.
        if crate::usage::refresh_usage_statistics(load_days).is_ok() {
            if let Ok(rows) = store::with_store(|store| {
                store.load_model_breakdown(ProviderKind::Codex, start_date, end_date)
            }) {
                for (model, usage) in rows {
                    model_rows
                        .entry((ProviderKind::Codex, model))
                        .or_default()
                        .add(&usage);
                }
            }
        }
    }
    if hourly
        && spend_providers.contains(&ProviderKind::Codex)
        && provider_hourly
            .get(&ProviderKind::Codex)
            .is_none_or(|hours| hours.is_empty())
    {
        if let Ok(rows) =
            crate::usage::collect_codex_hourly_since(start_hour.with_timezone(&Utc) - Duration::hours(1))
        {
            let mapped = rows.iter().cloned().collect::<BTreeMap<_, _>>();
            let _ = store::with_store(|store| {
                store.replace_usage_hourly(ProviderKind::Codex, &rows)
            });
            provider_hourly.insert(ProviderKind::Codex, mapped);
        }
    }

    let mut daily_by_date: BTreeMap<NaiveDate, BTreeMap<ProviderKind, TokenUsage>> =
        BTreeMap::new();
    for (provider, days) in &provider_daily {
        for entry in days {
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

    let mut providers = Vec::new();
    for provider in &spend_providers {
        let mut usage = TokenUsage::default();
        if hourly {
            if let Some(hours) = provider_hourly.get(provider) {
                for hour_usage in hours.values() {
                    usage.add(hour_usage);
                }
            }
            // Cursor (and anyone else without timestamps) still has daily rows.
            if usage.requests == 0 {
                if let Some(days) = provider_daily.get(provider) {
                    for entry in days {
                        if entry.date >= start_date && entry.date <= end_date {
                            usage.add(&entry.usage);
                        }
                    }
                }
            }
        } else if let Some(days) = provider_daily.get(provider) {
            for entry in days {
                if entry.date >= start_date && entry.date <= end_date {
                    usage.add(&entry.usage);
                }
            }
        }
        let tracked = provider_sessions.get(provider).copied().unwrap_or(0);
        // Codex/Claude have real session files or event paths. Cursor (and
        // anyone else with only a daily rollup) never writes those tables —
        // its CSV rows already live in `requests`. A stored 0 is not "unknown".
        let sessions = if tracked > 0 {
            tracked
        } else {
            usage.requests
        };
        snapshot.totals.add(&usage);
        snapshot.total_sessions = snapshot.total_sessions.saturating_add(sessions);
        providers.push(ProviderOverview {
            provider: *provider,
            sessions,
            usage,
            share_cost: 0.0,
            share_tokens: 0.0,
        });
    }

    let total_cost = snapshot.totals.estimated_cost_microusd.max(1);
    let total_tokens = snapshot.totals.total_tokens().max(1);
    for entry in &mut providers {
        entry.share_cost = entry.usage.estimated_cost_microusd as f64 / total_cost as f64 * 100.0;
        entry.share_tokens = entry.usage.total_tokens() as f64 / total_tokens as f64 * 100.0;
    }
    providers.sort_by(|left, right| {
        right
            .usage
            .estimated_cost_microusd
            .cmp(&left.usage.estimated_cost_microusd)
    });
    snapshot.providers = providers;

    let total_metric = match metric {
        OverviewMetric::Cost => snapshot.totals.estimated_cost_microusd.max(1),
        OverviewMetric::Tokens => snapshot.totals.total_tokens().max(1),
    };

    if hourly {
        snapshot.day_rows = (0..24)
            .rev()
            .map(|offset| {
                let at = start_hour + Duration::hours(offset);
                let mut cost = 0_u64;
                let mut tokens = 0_u64;
                let mut by_provider = BTreeMap::new();
                for (provider, hours) in &provider_hourly {
                    if let Some(usage) = hours.get(&at) {
                        cost = cost.saturating_add(usage.estimated_cost_microusd);
                        tokens = tokens.saturating_add(usage.total_tokens());
                        by_provider.insert(*provider, usage.clone());
                    }
                }
                let metric_value = match metric {
                    OverviewMetric::Cost => cost,
                    OverviewMetric::Tokens => tokens,
                };
                BreakdownRow {
                    label: format_hour_label(at),
                    weekday: None,
                    provider: None,
                    cost_microusd: cost,
                    tokens,
                    share: metric_value as f64 / total_metric as f64 * 100.0,
                    by_provider,
                }
            })
            .filter(|row| row.tokens > 0 || row.cost_microusd > 0)
            .collect();

        snapshot.daily_series = (0..24)
            .map(|offset| {
                let at = start_hour + Duration::hours(offset);
                let mut values = BTreeMap::new();
                let mut total = 0_u64;
                for (provider, hours) in &provider_hourly {
                    let usage = hours.get(&at);
                    let value = match (metric, usage) {
                        (OverviewMetric::Cost, Some(usage)) => usage.estimated_cost_microusd,
                        (OverviewMetric::Tokens, Some(usage)) => usage.total_tokens(),
                        _ => 0,
                    };
                    if value > 0 {
                        values.insert(*provider, value);
                        total = total.saturating_add(value);
                    }
                }
                DailySeriesPoint {
                    at,
                    date: at.date_naive(),
                    by_provider: values,
                    total,
                }
            })
            .collect();
    } else {
        snapshot.day_rows = daily_by_date
            .iter()
            .rev()
            .map(|(date, providers)| {
                let cost = providers.values().fold(0_u64, |total, usage| {
                    total.saturating_add(usage.estimated_cost_microusd)
                });
                let tokens = providers.values().fold(0_u64, |total, usage| {
                    total.saturating_add(usage.total_tokens())
                });
                let metric_value = match metric {
                    OverviewMetric::Cost => cost,
                    OverviewMetric::Tokens => tokens,
                };
                BreakdownRow {
                    label: date.format("%b %-d").to_string(),
                    weekday: Some(weekday_short(*date).to_owned()),
                    provider: None,
                    cost_microusd: cost,
                    tokens,
                    share: metric_value as f64 / total_metric as f64 * 100.0,
                    by_provider: providers.clone(),
                }
            })
            .collect();

        snapshot.daily_series = daily_by_date
            .into_iter()
            .map(|(date, by_provider)| {
                let mut values = BTreeMap::new();
                let mut total = 0_u64;
                for (provider, usage) in by_provider {
                    let value = match metric {
                        OverviewMetric::Cost => usage.estimated_cost_microusd,
                        OverviewMetric::Tokens => usage.total_tokens(),
                    };
                    values.insert(provider, value);
                    total = total.saturating_add(value);
                }
                DailySeriesPoint {
                    at: start_of_local_day(date),
                    date,
                    by_provider: values,
                    total,
                }
            })
            .collect();
    }

    snapshot.model_rows = model_rows
        .into_iter()
        .map(|((provider, model), usage)| BreakdownRow {
            label: model,
            weekday: None,
            provider: Some(provider),
            cost_microusd: usage.estimated_cost_microusd,
            tokens: usage.total_tokens(),
            share: match metric {
                OverviewMetric::Cost => usage.estimated_cost_microusd as f64 / total_metric as f64 * 100.0,
                OverviewMetric::Tokens => usage.total_tokens() as f64 / total_metric as f64 * 100.0,
            },
            by_provider: BTreeMap::new(),
        })
        .collect();
    snapshot.model_rows.sort_by(|left, right| match metric {
        OverviewMetric::Cost => right.cost_microusd.cmp(&left.cost_microusd),
        OverviewMetric::Tokens => right.tokens.cmp(&left.tokens),
    });

    snapshot
}

fn weekday_short(date: NaiveDate) -> &'static str {
    match date.weekday() {
        Weekday::Mon => "Mon",
        Weekday::Tue => "Tue",
        Weekday::Wed => "Wed",
        Weekday::Thu => "Thu",
        Weekday::Fri => "Fri",
        Weekday::Sat => "Sat",
        Weekday::Sun => "Sun",
    }
}

pub fn format_hour_label(at: DateTime<Local>) -> String {
    crate::settings::TimeFormat::current().format_hour_label(at)
}

/// Compact API-value formatting shared by the Home Total Spend card and the
/// Usage page. Keep cents visible for sub-thousand-dollar values so both
/// surfaces report the same number instead of rounding one of them to a
/// suspiciously tidy integer.
pub fn format_spend(microusd: u64) -> String {
    let dollars = microusd as f64 / 1_000_000.0;
    if dollars >= 1_000_000.0 {
        format!("${:.1}M", dollars / 1_000_000.0)
    } else if dollars >= 1_000.0 {
        format!("${:.1}K", dollars / 1_000.0)
    } else {
        format!("${dollars:.2}")
    }
}

fn start_of_local_day(date: NaiveDate) -> DateTime<Local> {
    date.and_hms_opt(0, 0, 0)
        .and_then(|naive| Local.from_local_datetime(&naive).single())
        .unwrap_or_else(Local::now)
}

fn usage_has_data(usage: &TokenUsage) -> bool {
    usage.requests > 0 || usage.total_tokens() > 0 || usage.estimated_cost_microusd > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_spend_keeps_the_same_precision_as_total_spend() {
        assert_eq!(format_spend(1_250_000), "$1.25");
        assert_eq!(format_spend(1_250_000_000), "$1.2K");
        assert_eq!(format_spend(1_250_000_000_000), "$1.2M");
    }
}
