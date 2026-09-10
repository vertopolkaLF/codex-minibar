//! Account-wide OpenRouter analytics with atomic, persistent daily and hourly caches.
use super::OpenRouterClient;
use crate::{
    store,
    usage::{DailyTokenUsage, TokenUsage, UsageStatistics, statistics_from_daily},
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};

#[derive(Default, Serialize, Deserialize)]
struct Cache {
    revision: u64,
    accounts: Vec<String>,
    days: BTreeMap<NaiveDate, CachedDay>,
    #[serde(default)]
    hourly: Option<HourlyCache>,
}
#[derive(Serialize, Deserialize)]
struct CachedDay {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    fetched_at: DateTime<Utc>,
    models: BTreeMap<String, TokenUsage>,
}
#[derive(Serialize, Deserialize)]
struct HourlyCache {
    fetched_at: DateTime<Utc>,
    granularity: String,
    rows: Vec<HourlyRow>,
}
#[derive(Serialize, Deserialize)]
struct HourlyRow {
    at: DateTime<Utc>,
    model: String,
    usage: TokenUsage,
}

// Subtract elapsed time rather than reconstructing an ambiguous local wall clock.
fn hour_start<T: TimeZone>(at: DateTime<T>) -> DateTime<T> {
    let elapsed = Duration::minutes(i64::from(at.minute()))
        + Duration::seconds(i64::from(at.second()))
        + Duration::nanoseconds(i64::from(at.nanosecond()));
    at - elapsed
}
// Anchor to the chart's elapsed-hour grid. Rounding each wall-clock label
// separately misaligns samples across half-hour DST changes (e.g. Lord Howe).
fn window_bucket<T: TimeZone>(
    at: DateTime<Utc>,
    start: DateTime<T>,
    end: DateTime<T>,
) -> Option<DateTime<T>> {
    let elapsed = at.signed_duration_since(&start);
    if elapsed < Duration::zero() || at >= end + Duration::hours(1) {
        return None;
    }
    Some(start + Duration::hours(elapsed.num_hours()))
}

pub(crate) fn cached_hourly_rows(
    raw: &str,
    start: DateTime<Local>,
    end: DateTime<Local>,
) -> Result<Vec<(String, DateTime<Local>, TokenUsage)>> {
    let cache: Cache = serde_json::from_str(raw)?;
    let mut rows = BTreeMap::<(String, DateTime<Local>), TokenUsage>::new();
    if let Some(hourly) = cache.hourly {
        for row in hourly.rows {
            if let Some(at) = window_bucket(row.at, start, end) {
                rows.entry((row.model, at)).or_default().add(&row.usage);
            }
        }
    }
    Ok(rows
        .into_iter()
        .map(|((model, at), usage)| (model, at, usage))
        .collect())
}
fn cache(client: &OpenRouterClient) -> Result<Cache> {
    let raw = store::with_store(|s| s.load_openrouter_analytics())?;
    let mut cached: Cache = raw
        .map(|s| serde_json::from_str(&s))
        .transpose()?
        .unwrap_or_default();
    let mut accounts: Vec<_> = client.accounts.iter().map(|a| a.id.clone()).collect();
    accounts.sort();
    if cached.revision != client.credentials_revision || cached.accounts != accounts {
        cached = Cache {
            revision: client.credentials_revision,
            accounts,
            ..Default::default()
        };
    }
    Ok(cached)
}
fn daily(cache: &Cache) -> Vec<DailyTokenUsage> {
    cache
        .days
        .iter()
        .map(|(&date, day)| {
            let mut usage = TokenUsage::default();
            for model in day.models.values() {
                usage.add(model);
            }
            DailyTokenUsage { date, usage }
        })
        .collect()
}
pub(super) fn load(client: &OpenRouterClient, history_days: u16) -> Result<UsageStatistics> {
    Ok(statistics_from_daily(&daily(&cache(client)?), history_days))
}
fn boundaries(date: NaiveDate) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    let midnight = |date: NaiveDate| {
        Local
            .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
            .earliest()
            .context("OpenRouter: local midnight does not exist in this timezone")
            .map(|d| d.with_timezone(&Utc))
    };
    Ok((
        midnight(date)?,
        midnight(date.succ_opt().context("date overflow")?)?,
    ))
}
fn fresh(day: &CachedDay, start: DateTime<Utc>, end: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    let ttl = if now - end < Duration::days(1) {
        Duration::minutes(5)
    } else {
        Duration::days(1)
    };
    day.start == start && day.end == end && now >= day.fetched_at && now - day.fetched_at < ttl
}
fn query(
    client: &OpenRouterClient,
    key: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    granularity: Option<&str>,
) -> Result<String> {
    let mut body = json!({
        "metrics": ["request_count", "total_usage", "tokens_prompt", "tokens_completion", "cached_tokens"],
        "dimensions": ["model"],
        "limit": 10000,
        "time_range": {
            "start": start.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            "end": end.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        }
    });
    if let Some(g) = granularity {
        body["granularity"] = json!(g);
    }
    client
        .agent
        .post("https://openrouter.ai/api/v1/analytics/query")
        .set("Authorization", &format!("Bearer {key}"))
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
        .map_err(|e| match e {
            ureq::Error::Status(401 | 403, _) => anyhow::anyhow!(
                "OpenRouter usage stats: save a valid management key in Providers / OpenRouter"
            ),
            other => anyhow::anyhow!("OpenRouter analytics request failed: {other}"),
        })?
        .into_string()
        .context("read OpenRouter analytics response")
}
pub(super) fn refresh(client: &OpenRouterClient, history_days: u16) -> Result<UsageStatistics> {
    let mut cache = cache(client)?;
    let mut seen = HashSet::new();
    let mut keys = Vec::new();
    for account in &client.accounts {
        let key = account
            .management_key
            .as_deref()
            .context("OpenRouter usage stats require a management key for each account")?;
        if seen.insert(key) {
            keys.push(key);
        }
    }
    ensure!(
        !keys.is_empty(),
        "OpenRouter usage stats require a management key"
    );
    let now = Utc::now();
    let today = now.with_timezone(&Local).date_naive();
    let first = today - Duration::days(i64::from(history_days.clamp(1, 365) - 1));
    let mut changed = false;
    for date in first.iter_days().take_while(|d| *d <= today) {
        let (start, end) = boundaries(date)?;
        if cache
            .days
            .get(&date)
            .is_some_and(|day| fresh(day, start, end, now))
        {
            continue;
        }
        let mut models: BTreeMap<String, TokenUsage> = BTreeMap::new();
        for key in &keys {
            for (model, usage) in parse(&query(client, key, start, end.min(now), None)?)? {
                models.entry(model).or_default().add(&usage);
            }
        }
        cache.days.insert(
            date,
            CachedDay {
                start,
                end,
                fetched_at: now,
                models,
            },
        );
        changed = true;
    }
    let current_hour = hour_start(now.with_timezone(&Local));
    let start = (current_hour - Duration::hours(47)).with_timezone(&Utc);
    // Minute data is necessary when UTC hour boundaries don't match local hours.
    let granularity = if (0..48).all(|i| {
        (current_hour - Duration::hours(i))
            .offset()
            .local_minus_utc()
            % 3600
            == 0
    }) {
        "hour"
    } else {
        "minute"
    };
    if !cache.hourly.as_ref().is_some_and(|h| {
        h.granularity == granularity
            && now >= h.fetched_at
            && now - h.fetched_at < Duration::minutes(5)
    }) {
        let mut rows = Vec::new();
        for key in &keys {
            rows.extend(parse_hourly(
                &query(client, key, start, now, Some(granularity))?,
                granularity,
                start,
                now,
            )?);
        }
        // Replace the whole snapshot, including rows corrected to zero. No accumulation across refreshes.
        cache.hourly = Some(HourlyCache {
            fetched_at: now,
            granularity: granularity.into(),
            rows,
        });
        changed = true;
    }
    cache
        .days
        .retain(|date, _| *date >= today - Duration::days(364) && *date <= today);
    let days = daily(&cache);
    if changed {
        let models: Vec<_> = cache
            .days
            .iter()
            .flat_map(|(&date, day)| {
                day.models
                    .iter()
                    .map(move |(model, usage)| (model.clone(), date, usage.clone()))
            })
            .collect();
        let encoded = serde_json::to_string(&cache)?;
        store::with_store(|s| s.save_openrouter_analytics(&encoded, &days, &models, now))?;
    }
    Ok(statistics_from_daily(&days, history_days))
}
fn count(row: &Value, field: &str) -> Result<u64> {
    let v = row
        .get(field)
        .with_context(|| format!("OpenRouter analytics missing {field}"))?;
    if v.is_null() {
        return Ok(0);
    }
    let n = if let Some(s) = v.as_str() {
        s.parse::<u64>().ok()
    } else {
        v.as_u64()
    };
    n.filter(|n| *n <= i64::MAX as u64)
        .with_context(|| format!("OpenRouter analytics invalid {field}"))
}
fn rows(body: &str) -> Result<Vec<Value>> {
    let value: Value = serde_json::from_str(body).context("parse OpenRouter analytics")?;
    ensure!(
        value
            .pointer("/data/metadata/truncated")
            .and_then(Value::as_bool)
            == Some(false),
        "OpenRouter analytics returned incomplete results; history was not replaced"
    );
    value
        .pointer("/data/data")
        .and_then(Value::as_array)
        .cloned()
        .context("OpenRouter analytics missing rows")
}
fn row_usage(row: &Value) -> Result<(String, TokenUsage)> {
    let model = row
        .get("model")
        .and_then(Value::as_str)
        .context("OpenRouter analytics missing model")?;
    let requests = count(row, "request_count")?;
    let cost = row
        .get("total_usage")
        .context("OpenRouter analytics missing total_usage")?;
    let cost = if let Some(s) = cost.as_str() {
        s.parse::<f64>().ok()
    } else {
        cost.as_f64()
    }
    .context("OpenRouter analytics invalid total_usage")?;
    ensure!(
        cost.is_finite() && cost >= 0.0 && cost * 1_000_000.0 < i64::MAX as f64,
        "OpenRouter analytics invalid cost"
    );
    Ok((
        model.into(),
        TokenUsage {
            input_tokens: count(row, "tokens_prompt")?,
            output_tokens: count(row, "tokens_completion")?,
            cached_input_tokens: count(row, "cached_tokens")?,
            requests,
            priced_requests: requests,
            estimated_cost_microusd: (cost * 1_000_000.0).round() as u64,
            ..Default::default()
        },
    ))
}
fn parse(body: &str) -> Result<BTreeMap<String, TokenUsage>> {
    let mut models: BTreeMap<String, TokenUsage> = BTreeMap::new();
    for row in rows(body)? {
        let (model, usage) = row_usage(&row)?;
        models.entry(model).or_default().add(&usage);
    }
    Ok(models)
}
fn parse_hourly(
    body: &str,
    granularity: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<HourlyRow>> {
    let mut output = Vec::new();
    let mut seen = HashSet::new();
    for row in rows(body)? {
        let timestamp = row
            .get(format!("date__{granularity}"))
            .or_else(|| row.get(format!("created_at__{granularity}")))
            .and_then(Value::as_str)
            .context("OpenRouter analytics missing bucket timestamp")?;
        // Analytics' SQL source returns UTC "YYYY-MM-DD HH:MM:SS";
        // its event source returns RFC3339 with an explicit offset.
        let at = if timestamp.contains('T') {
            DateTime::parse_from_rfc3339(timestamp)
                .context("OpenRouter analytics invalid bucket timestamp")?
                .with_timezone(&Utc)
        } else {
            chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S")
                .context("OpenRouter analytics invalid UTC bucket timestamp")?
                .and_utc()
        };
        ensure!(
            at >= start && at < end,
            "OpenRouter analytics bucket outside requested interval"
        );
        let (model, usage) = row_usage(&row)?;
        ensure!(
            seen.insert((at, model.clone())),
            "OpenRouter analytics duplicate model/time bucket"
        );
        output.push(HourlyRow { at, model, usage });
    }
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(at: &str, field: &str) -> Value {
        let mut row = json!({"model":"test/model","request_count":"2","total_usage":"0.123456","tokens_prompt":"100","tokens_completion":20,"cached_tokens":40});
        row[field] = json!(at);
        row
    }
    fn envelope(rows: Vec<Value>) -> String {
        json!({"data":{"metadata":{"truncated":false},"data":rows}}).to_string()
    }
    #[test]
    fn parses_cost_strings_and_both_timestamp_fields() {
        let start = DateTime::parse_from_rfc3339("2026-09-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        for field in ["date__hour", "created_at__hour"] {
            let body = envelope(vec![fixture("2026-09-10T00:00:00Z", field)]);
            let rows = parse_hourly(&body, "hour", start, start + Duration::hours(1)).unwrap();
            assert_eq!(rows[0].usage.total_tokens(), 120);
            assert_eq!(rows[0].usage.estimated_cost_microusd, 123456);
            assert_eq!(rows[0].usage.priced_requests, 2);
        }
    }
    #[test]
    fn parses_sql_utc_timestamp_without_using_local_timezone() {
        let start = DateTime::parse_from_rfc3339("2026-09-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let rows = parse_hourly(
            &envelope(vec![fixture("2026-09-10 00:00:00", "date__hour")]),
            "hour",
            start,
            start + Duration::hours(1),
        )
        .unwrap();
        assert_eq!(rows[0].at, start);
    }
    #[test]
    fn rejects_incomplete_duplicate_and_out_of_window_data() {
        let start = DateTime::parse_from_rfc3339("2026-09-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let row = fixture("2026-09-10T00:00:00Z", "date__hour");
        assert!(
            parse_hourly(
                &envelope(vec![row.clone(), row.clone()]),
                "hour",
                start,
                start + Duration::hours(1)
            )
            .is_err()
        );
        assert!(
            parse_hourly(
                &envelope(vec![row]),
                "hour",
                start - Duration::hours(1),
                start
            )
            .is_err()
        );
        for body in [
            r#"{"data":{"metadata":{"truncated":true},"data":[]}}"#,
            r#"{"data":{"data":[]}}"#,
            r#"{"error":"denied"}"#,
        ] {
            assert!(parse(body).is_err());
        }
        assert!(count(&json!({"n":-1}), "n").is_err());
        assert!(count(&json!({"n":"1.2"}), "n").is_err());
    }
    #[test]
    fn repeated_hours_and_fractional_offsets_remain_distinct() {
        let first = DateTime::parse_from_rfc3339("2026-10-25T02:37:00+02:00").unwrap();
        let second = DateTime::parse_from_rfc3339("2026-10-25T02:37:00+01:00").unwrap();
        assert_eq!(
            hour_start(second).with_timezone(&Utc) - hour_start(first).with_timezone(&Utc),
            Duration::hours(1)
        );
        assert_eq!(hour_start(first).minute(), 0);
        let fractional = DateTime::parse_from_rfc3339("2026-09-10T12:37:00+05:45").unwrap();
        assert_eq!(
            hour_start(fractional).with_timezone(&Utc).to_rfc3339(),
            "2026-09-10T06:15:00+00:00"
        );
        // A spring-forward skips a local label but not an elapsed hour.
        let before = DateTime::parse_from_rfc3339("2026-03-29T01:37:00+01:00").unwrap();
        let after = DateTime::parse_from_rfc3339("2026-03-29T03:37:00+02:00").unwrap();
        assert_eq!(
            hour_start(after).with_timezone(&Utc) - hour_start(before).with_timezone(&Utc),
            Duration::hours(1)
        );
    }
    #[test]
    fn half_hour_dst_change_uses_the_same_elapsed_grid_as_the_chart() {
        let end = DateTime::parse_from_rfc3339("2026-04-05T03:00:00+10:30").unwrap();
        let start = end - Duration::hours(23);
        let before = DateTime::parse_from_rfc3339("2026-04-05T01:40:00+11:00")
            .unwrap()
            .with_timezone(&Utc);
        let after = DateTime::parse_from_rfc3339("2026-04-05T01:40:00+10:30")
            .unwrap()
            .with_timezone(&Utc);
        let first = window_bucket(before, start, end).unwrap();
        let second = window_bucket(after, start, end).unwrap();
        assert_eq!(first, end - Duration::hours(2));
        assert_eq!(second, first);
        assert_eq!((first - start).num_seconds() % 3600, 0);
        assert_eq!((second - start).num_seconds() % 3600, 0);
        assert!(
            window_bucket(start.with_timezone(&Utc) - Duration::seconds(1), start, end).is_none()
        );
        assert!(
            window_bucket((end + Duration::hours(1)).with_timezone(&Utc), start, end).is_none()
        );
    }

    #[test]
    fn rolling_window_includes_exactly_24_hours_and_aggregates_models() {
        let end = hour_start(Local::now());
        let start = end - Duration::hours(23);
        let cache = Cache {
            hourly: Some(HourlyCache {
                fetched_at: Utc::now(),
                granularity: "hour".into(),
                rows: (0..26)
                    .flat_map(|i| {
                        let at = (start + Duration::hours(i - 1)).with_timezone(&Utc);
                        ["a", "b"].map(|model| HourlyRow {
                            at,
                            model: model.into(),
                            usage: TokenUsage {
                                requests: 1,
                                ..Default::default()
                            },
                        })
                    })
                    .collect(),
            }),
            ..Default::default()
        };
        let raw = serde_json::to_string(&cache).unwrap();
        let rows = cached_hourly_rows(&raw, start, end).unwrap();
        assert_eq!(rows.len(), 48);
        assert_eq!(rows.iter().map(|(_, _, u)| u.requests).sum::<u64>(), 48);
        assert_eq!(cached_hourly_rows(&raw, start, end).unwrap(), rows);
        let legacy = r#"{"revision":0,"accounts":[],"days":{}}"#;
        assert!(cached_hourly_rows(legacy, start, end).unwrap().is_empty());
    }
    #[test]
    fn cache_expires_and_detects_timezone_changes() {
        let now = Utc::now();
        let day = CachedDay {
            start: now - Duration::hours(12),
            end: now + Duration::hours(12),
            fetched_at: now,
            models: BTreeMap::new(),
        };
        assert!(fresh(&day, day.start, day.end, now + Duration::minutes(4)));
        assert!(!fresh(&day, day.start, day.end, now + Duration::minutes(5)));
        assert!(!fresh(&day, day.start + Duration::hours(1), day.end, now));
    }
}
