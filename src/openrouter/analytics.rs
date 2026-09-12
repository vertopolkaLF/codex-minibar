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
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration as StdDuration, Instant},
};

#[derive(Clone, Default, Serialize, Deserialize)]
struct Cache {
    revision: u64,
    accounts: Vec<String>,
    days: BTreeMap<NaiveDate, CachedDay>,
    #[serde(default)]
    hourly: Option<HourlyCache>,
    #[serde(default)]
    account_data: BTreeMap<String, AccountCache>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
struct AccountCache {
    identity: String,
    days: BTreeMap<NaiveDate, CachedDay>,
    hourly: Option<HourlyCache>,
    error: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
struct CachedDay {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    fetched_at: DateTime<Utc>,
    models: BTreeMap<String, TokenUsage>,
}
#[derive(Clone, Serialize, Deserialize)]
struct HourlyCache {
    fetched_at: DateTime<Utc>,
    granularity: String,
    rows: Vec<HourlyRow>,
}
#[derive(Clone, Serialize, Deserialize)]
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
fn credential_identity(key: Option<&str>) -> String {
    key.map(|key| format!("{:x}", Sha256::digest(key.as_bytes())))
        .unwrap_or_default()
}

fn sync_accounts(cache: &mut Cache, revision: u64, accounts: &[(String, String)]) {
    // Only a single-account legacy aggregate has an unambiguous owner. Never
    // assign a combined legacy history to the last account in the settings UI.
    if cache.account_data.is_empty()
        && cache.revision == revision
        && accounts.len() == 1
        && cache.accounts == vec![accounts[0].0.clone()]
        && !accounts[0].1.is_empty()
    {
        cache.account_data.insert(
            accounts[0].0.clone(),
            AccountCache {
                identity: accounts[0].1.clone(),
                days: cache.days.clone(),
                hourly: cache.hourly.clone(),
                error: None,
            },
        );
    }
    cache
        .account_data
        .retain(|id, _| accounts.iter().any(|(current, _)| current == id));
    let mut reusable = BTreeMap::<String, AccountCache>::new();
    for account in cache
        .account_data
        .values()
        .filter(|a| !a.identity.is_empty())
    {
        let entry = reusable
            .entry(account.identity.clone())
            .or_insert_with(|| account.clone());
        if account.days.len() > entry.days.len() {
            *entry = account.clone();
        }
    }
    for (id, identity) in accounts {
        let entry = cache.account_data.entry(id.clone()).or_default();
        if let Some(cached) = reusable.get(identity) {
            *entry = cached.clone();
        } else if entry.identity != *identity {
            *entry = AccountCache {
                identity: identity.clone(),
                ..Default::default()
            };
        }
        if identity.is_empty() {
            entry.error = Some("Add a management key to load usage statistics.".into());
        }
    }
    cache.revision = revision;
    cache.accounts = accounts.iter().map(|(id, _)| id.clone()).collect();
    cache.accounts.sort();
    combine_accounts(cache);
}

fn combine_accounts(cache: &mut Cache) {
    cache.days.clear();
    cache.hourly = None;
    let mut seen = HashSet::new();
    for account in cache.account_data.values() {
        // The same management key can be entered twice. Display it under both
        // account labels but include its usage only once in provider totals.
        if account.identity.is_empty() || !seen.insert(&account.identity) {
            continue;
        }
        for (&date, day) in &account.days {
            let combined = cache.days.entry(date).or_insert_with(|| CachedDay {
                start: day.start,
                end: day.end,
                fetched_at: day.fetched_at,
                models: BTreeMap::new(),
            });
            combined.fetched_at = combined.fetched_at.min(day.fetched_at);
            for (model, usage) in &day.models {
                combined.models.entry(model.clone()).or_default().add(usage);
            }
        }
        if let Some(hourly) = &account.hourly {
            let combined = cache.hourly.get_or_insert_with(|| HourlyCache {
                fetched_at: hourly.fetched_at,
                granularity: hourly.granularity.clone(),
                rows: Vec::new(),
            });
            combined.fetched_at = combined.fetched_at.min(hourly.fetched_at);
            combined.rows.extend(hourly.rows.clone());
        }
    }
}

fn cache(client: &OpenRouterClient) -> Result<Cache> {
    let raw = store::with_store(|s| s.load_openrouter_analytics())?;
    let mut cached: Cache = raw
        .map(|s| serde_json::from_str(&s))
        .transpose()?
        .unwrap_or_default();
    let accounts: Vec<_> = client
        .accounts
        .iter()
        .map(|a| {
            (
                a.id.clone(),
                credential_identity(a.management_key.as_deref()),
            )
        })
        .collect();
    sync_accounts(&mut cached, client.credentials_revision, &accounts);
    Ok(cached)
}

fn daily_rows(days: &BTreeMap<NaiveDate, CachedDay>) -> Vec<DailyTokenUsage> {
    days.iter()
        .map(|(&date, day)| {
            let mut usage = TokenUsage::default();
            for model in day.models.values() {
                usage.add(model);
            }
            DailyTokenUsage { date, usage }
        })
        .collect()
}
fn daily(cache: &Cache) -> Vec<DailyTokenUsage> {
    daily_rows(&cache.days)
}
fn statistics(cache: &Cache, history_days: u16) -> UsageStatistics {
    let mut combined = statistics_from_daily(&daily(cache), history_days);
    for (id, account) in &cache.account_data {
        let mut usage = statistics_from_daily(&daily_rows(&account.days), history_days);
        usage.error = account.error.clone();
        combined.accounts.insert(id.clone(), usage);
    }
    combined
}
fn persist(cache: &Cache, at: DateTime<Utc>) -> Result<()> {
    let models: Vec<_> = cache
        .days
        .iter()
        .flat_map(|(&date, day)| {
            day.models
                .iter()
                .map(move |(model, usage)| (model.clone(), date, usage.clone()))
        })
        .collect();
    let encoded = serde_json::to_string(cache)?;
    store::with_store(|s| s.save_openrouter_analytics(&encoded, &daily(cache), &models, at))
}

pub(super) fn load(client: &OpenRouterClient, history_days: u16) -> Result<UsageStatistics> {
    let cached = cache(client)?;
    // Keep overview totals consistent with account removal/key replacement too.
    persist(&cached, Utc::now())?;
    Ok(statistics(&cached, history_days))
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
fn wait_for_request(cancelled: &AtomicBool, duration: StdDuration) -> Result<()> {
    let deadline = Instant::now() + duration;
    loop {
        ensure!(
            !cancelled.load(Ordering::Acquire),
            "OpenRouter analytics refresh cancelled"
        );
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(());
        }
        std::thread::sleep(remaining.min(StdDuration::from_millis(100)));
    }
}

fn reserve_request(next: &mut Option<Instant>, now: Instant) -> StdDuration {
    let slot = next.unwrap_or(now).max(now);
    *next = Some(slot + StdDuration::from_millis(1100));
    slot.saturating_duration_since(now)
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
    // Shared by all clients, including workers being replaced after a settings edit.
    // Stay below the analytics API's 64 requests/minute limit.
    static NEXT_REQUEST: Mutex<Option<Instant>> = Mutex::new(None);
    for attempt in 0..3 {
        let wait = {
            let mut next = NEXT_REQUEST
                .lock()
                .map_err(|_| anyhow::anyhow!("OpenRouter request limiter unavailable"))?;
            reserve_request(&mut next, Instant::now())
        };
        ensure!(
            wait <= StdDuration::from_secs(61),
            "OpenRouter analytics rate limited; waiting for the server cooldown before retrying"
        );
        wait_for_request(&client.cancelled, wait)?;
        match client
            .agent
            .post("https://openrouter.ai/api/v1/analytics/query")
            .set("Authorization", &format!("Bearer {key}"))
            .set("Content-Type", "application/json")
            .send_string(&body.to_string())
        {
            Ok(response) => {
                return response
                    .into_string()
                    .context("read OpenRouter analytics response");
            }
            Err(ureq::Error::Status(429, response)) => {
                let delay = response
                    .header("Retry-After")
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(60);
                let mut next = NEXT_REQUEST
                    .lock()
                    .map_err(|_| anyhow::anyhow!("OpenRouter request limiter unavailable"))?;
                let retry = Instant::now()
                    .checked_add(StdDuration::from_secs(delay.max(1)))
                    .context("OpenRouter returned an invalid retry delay")?;
                *next = Some(next.unwrap_or(retry).max(retry));
                ensure!(
                    attempt < 2 && delay <= 60,
                    "OpenRouter analytics rate limited; retrying after the server cooldown on the next refresh"
                );
            }
            Err(ureq::Error::Status(401 | 403, _)) => anyhow::bail!(
                "OpenRouter usage stats: save a valid management key in Providers / OpenRouter"
            ),
            Err(error) => {
                return Err(anyhow::anyhow!(
                    "OpenRouter analytics request failed: {error}"
                ));
            }
        }
    }
    unreachable!("the last request returns its response or error")
}
// The provider card's display period is independent of the Usage tab's range.
// Fetch enough history for every supported overview range, including on upgrades.
fn first_fetch_day(today: NaiveDate, history_days: u16) -> NaiveDate {
    let days = history_days
        .clamp(1, 365)
        .max(crate::usage_overview::OVERVIEW_MAX_DAYS);
    today - Duration::days(i64::from(days - 1))
}

fn refresh_account(
    account: &mut AccountCache,
    history_days: u16,
    now: DateTime<Utc>,
    mut fetch: impl FnMut(DateTime<Utc>, DateTime<Utc>, Option<&str>) -> Result<String>,
) -> Result<()> {
    let today = now.with_timezone(&Local).date_naive();
    let first = first_fetch_day(today, history_days);
    // Newest days first: any saved progress contains the useful current usage.
    let dates: Vec<_> = first
        .iter_days()
        .take_while(|date| *date <= today)
        .collect();
    for date in dates.into_iter().rev() {
        let (start, end) = boundaries(date)?;
        if account
            .days
            .get(&date)
            .is_some_and(|day| fresh(day, start, end, now))
        {
            continue;
        }
        let models = if start < now {
            parse(&fetch(start, end.min(now), None)?)?
        } else {
            BTreeMap::new()
        };
        account.days.insert(
            date,
            CachedDay {
                start,
                end,
                fetched_at: now,
                models,
            },
        );
    }
    let current_hour = hour_start(now.with_timezone(&Local));
    let start = (current_hour - Duration::hours(47)).with_timezone(&Utc);
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
    if !account.hourly.as_ref().is_some_and(|h| {
        h.granularity == granularity
            && now >= h.fetched_at
            && now - h.fetched_at < Duration::minutes(5)
    }) {
        let rows = parse_hourly(
            &fetch(start, now, Some(granularity))?,
            granularity,
            start,
            now,
        )?;
        account.hourly = Some(HourlyCache {
            fetched_at: now,
            granularity: granularity.into(),
            rows,
        });
    }
    account
        .days
        .retain(|date, _| *date >= today - Duration::days(364) && *date <= today);
    Ok(())
}

pub(super) fn refresh(client: &OpenRouterClient, history_days: u16) -> Result<UsageStatistics> {
    let mut cache = cache(client)?;
    let mut completed = BTreeMap::<String, AccountCache>::new();
    for credentials in &client.accounts {
        wait_for_request(&client.cancelled, StdDuration::ZERO)?;
        let account = cache
            .account_data
            .get_mut(&credentials.id)
            .context("OpenRouter account cache missing")?;
        if let Some(cached) = completed.get(&account.identity) {
            *account = cached.clone();
        } else if let Some(key) = credentials.management_key.as_deref() {
            let result = refresh_account(
                account,
                history_days,
                Utc::now(),
                |start, end, granularity| query(client, key, start, end, granularity),
            );
            if client.cancelled.load(Ordering::Acquire) {
                combine_accounts(&mut cache);
                persist(&cache, Utc::now())?;
                anyhow::bail!("OpenRouter analytics refresh cancelled");
            }
            account.error = result.err().map(|error| error.to_string());
            completed.insert(account.identity.clone(), account.clone());
        }
        // Persist completed days even after a failed request. Retrying resumes
        // from that point instead of repeating the entire 90-day recovery.
        wait_for_request(&client.cancelled, StdDuration::ZERO)?;
        combine_accounts(&mut cache);
        persist(&cache, Utc::now())?;
    }
    Ok(statistics(&cache, history_days))
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
    fn fetches_full_overview_history_without_changing_the_card_period() {
        let today = Local::now().date_naive();
        let first = first_fetch_day(today, 30);
        assert_eq!(
            (today - first).num_days() + 1,
            i64::from(crate::usage_overview::OVERVIEW_MAX_DAYS)
        );
        assert_eq!(first_fetch_day(today, 1), first);
        assert_eq!((today - first_fetch_day(today, 365)).num_days() + 1, 365);
        let mut cache = Cache::default();
        for date in first.iter_days().take_while(|date| *date <= today) {
            cache.days.insert(
                date,
                CachedDay {
                    start: date.and_hms_opt(0, 0, 0).unwrap().and_utc(),
                    end: date
                        .succ_opt()
                        .unwrap()
                        .and_hms_opt(0, 0, 0)
                        .unwrap()
                        .and_utc(),
                    fetched_at: Utc::now(),
                    models: BTreeMap::from([(
                        "test/model".into(),
                        TokenUsage {
                            requests: 1,
                            ..Default::default()
                        },
                    )]),
                },
            );
        }
        let stored_days = daily(&cache);
        let card = statistics_from_daily(&stored_days, 30);
        assert_eq!(card.history_days, 30);
        assert_eq!(card.history.requests, 30);
        assert_eq!(statistics_from_daily(&stored_days, 90).history.requests, 90);
    }

    #[test]
    fn multiple_accounts_cannot_burst_past_the_analytics_rate_limit() {
        let now = Instant::now();
        let mut next = None;
        let slots: Vec<_> = (0..182).map(|_| reserve_request(&mut next, now)).collect();
        assert_eq!(slots[0], StdDuration::ZERO);
        for window in slots.windows(64) {
            assert!(window[63] - window[0] > StdDuration::from_secs(60));
        }
        // Once the recovery is over, a later refresh can start immediately.
        assert_eq!(
            reserve_request(&mut next, now + StdDuration::from_secs(300)),
            StdDuration::ZERO
        );
    }

    #[test]
    fn retry_after_delays_all_accounts_and_worker_replacements() {
        let now = Instant::now();
        let mut next = Some(now + StdDuration::from_secs(30));
        assert_eq!(reserve_request(&mut next, now), StdDuration::from_secs(30));
        assert_eq!(
            reserve_request(&mut next, now),
            StdDuration::from_millis(31100)
        );
    }

    #[test]
    fn cancellation_interrupts_rate_limit_waits() {
        let cancelled = std::sync::Arc::new(AtomicBool::new(false));
        let worker_flag = cancelled.clone();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            done_tx
                .send(wait_for_request(&worker_flag, StdDuration::from_secs(60)).is_err())
                .unwrap();
        });
        started_rx.recv_timeout(StdDuration::from_secs(2)).unwrap();
        cancelled.store(true, Ordering::Release);
        assert!(done_rx.recv_timeout(StdDuration::from_secs(2)).unwrap());
        worker.join().unwrap();
    }

    fn account_fixture(identity: &str, requests: u64) -> AccountCache {
        let date = Local::now().date_naive();
        let (start, end) = boundaries(date).unwrap();
        AccountCache {
            identity: identity.into(),
            days: BTreeMap::from([(
                date,
                CachedDay {
                    start,
                    end,
                    fetched_at: Utc::now(),
                    models: BTreeMap::from([(
                        "test/model".into(),
                        TokenUsage {
                            requests,
                            input_tokens: requests * 10,
                            ..Default::default()
                        },
                    )]),
                },
            )]),
            ..Default::default()
        }
    }

    #[test]
    fn account_changes_keep_other_histories_and_aggregate_without_duplicates() {
        let mut cache = Cache {
            account_data: BTreeMap::from([("first".into(), account_fixture("key-a", 2))]),
            ..Default::default()
        };
        sync_accounts(
            &mut cache,
            2,
            &[
                ("first".into(), "key-a".into()),
                ("second".into(), "key-b".into()),
            ],
        );
        assert_eq!(statistics(&cache, 30).accounts["first"].history.requests, 2);
        assert!(cache.account_data["second"].days.is_empty());
        cache
            .account_data
            .insert("second".into(), account_fixture("key-b", 7));
        sync_accounts(
            &mut cache,
            3,
            &[
                ("second".into(), "key-b".into()),
                ("first".into(), "key-a".into()),
            ],
        );
        let stats = statistics(&cache, 30);
        assert_eq!(stats.history.requests, 9);
        assert_eq!(stats.accounts["second"].history.requests, 7);
        sync_accounts(
            &mut cache,
            4,
            &[
                ("first".into(), "key-a".into()),
                ("duplicate".into(), "key-a".into()),
            ],
        );
        assert_eq!(statistics(&cache, 30).history.requests, 2);
        assert_eq!(
            statistics(&cache, 30).accounts["duplicate"]
                .history
                .requests,
            2
        );
        sync_accounts(
            &mut cache,
            5,
            &[
                ("first".into(), "key-a".into()),
                ("duplicate".into(), "replacement".into()),
            ],
        );
        assert_eq!(statistics(&cache, 30).accounts["first"].history.requests, 2);
        assert!(cache.account_data["duplicate"].days.is_empty());
    }

    #[test]
    fn legacy_single_account_is_preserved_but_combined_history_is_never_misattributed() {
        let mut single = Cache {
            revision: 1,
            accounts: vec!["a".into()],
            days: account_fixture("key", 12).days,
            ..Default::default()
        };
        sync_accounts(&mut single, 1, &[("a".into(), "key".into())]);
        assert_eq!(statistics(&single, 30).accounts["a"].history.requests, 12);
        let mut combined = Cache {
            revision: 1,
            accounts: vec!["a".into(), "b".into()],
            days: account_fixture("key", 12).days,
            ..Default::default()
        };
        sync_accounts(
            &mut combined,
            1,
            &[("a".into(), "key-a".into()), ("b".into(), "key-b".into())],
        );
        assert!(
            statistics(&combined, 30)
                .accounts
                .values()
                .all(|s| s.history.requests == 0)
        );
    }

    #[test]
    fn errors_do_not_hide_a_healthy_account_and_cache_round_trips() {
        let mut failed = account_fixture("key-b", 7);
        failed.error = Some("Server rate limited this account".into());
        let mut cache = Cache {
            account_data: BTreeMap::from([
                ("a".into(), account_fixture("key-a", 2)),
                ("b".into(), failed),
            ]),
            ..Default::default()
        };
        combine_accounts(&mut cache);
        let stats = statistics(&cache, 30);
        assert!(stats.accounts["a"].error.is_none());
        assert!(stats.accounts["b"].error.is_some());
        assert_eq!(stats.accounts["a"].history.requests, 2);
        assert_eq!(stats.accounts["b"].history.requests, 7);
        let raw = serde_json::to_string(&cache).unwrap();
        let round_trip: Cache = serde_json::from_str(&raw).unwrap();
        assert_eq!(statistics(&round_trip, 30), stats);
    }

    #[test]
    fn interrupted_recovery_resumes_completed_days_and_fresh_accounts_make_no_requests() {
        let now = boundaries(Local::now().date_naive()).unwrap().0 + Duration::hours(12);
        let mut account = AccountCache::default();
        let mut requests = 0;
        let result = refresh_account(&mut account, 30, now, |_, _, _| {
            requests += 1;
            if requests == 3 {
                anyhow::bail!("429");
            }
            Ok(envelope(vec![fixture("unused", "unused")]))
        });
        assert!(result.is_err());
        assert_eq!(account.days.len(), 2);
        let encoded = serde_json::to_string(&account).unwrap();
        let mut restored: AccountCache = serde_json::from_str(&encoded).unwrap();
        let mut resumed = 0;
        refresh_account(&mut restored, 30, now, |_, _, granularity| {
            resumed += 1;
            Ok(if granularity.is_some() {
                envelope(vec![])
            } else {
                envelope(vec![fixture("unused", "unused")])
            })
        })
        .unwrap();
        assert_eq!(resumed, 89); // 88 remaining daily queries + hourly snapshot.
        assert_eq!(restored.days.len(), 90);
        refresh_account(&mut restored, 30, now, |_, _, _| {
            panic!("fresh account must reuse its cache")
        })
        .unwrap();
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
