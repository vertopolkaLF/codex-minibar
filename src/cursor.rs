//! Cursor dashboard usage provider.
//!
//! Cursor persists its OAuth session in the application's VS Code state DB.
//! We open that database read-only, refresh an expired access token only in
//! memory, and query the same dashboard endpoints used by Cursor itself.
//! The UI deliberately exposes its Auto and API lanes, not blended Total Usage.

use std::{collections::BTreeMap, env, path::PathBuf, time::Duration};

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Duration as ChronoDuration, Local, NaiveDate, Utc};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};

use crate::{
    limits::{AdditionalLimit, LimitWindow, RateLimits},
    settings::ProviderKind,
    store,
    usage::{DailyTokenUsage, TokenUsage, UsageStatistics, statistics_from_daily},
    worker::{Activator, LimitProvider, UsageProvider},
};

const API_BASE: &str = "https://api2.cursor.sh";
const CURSOR_BASE: &str = "https://cursor.com";
const CURSOR_CLIENT_ID: &str = "KbZUR41cY7W6zRSdpSUJ7I7mLYBKOCmB";
const ACCESS_TOKEN_KEY: &str = "cursorAuth/accessToken";
const REFRESH_TOKEN_KEY: &str = "cursorAuth/refreshToken";
const USAGE_EXPORT_PATH: &str = "/api/dashboard/export-usage-events-csv";
const USAGE_CACHE_VERSION: u8 = 2;
const USAGE_CACHE_TTL: ChronoDuration = ChronoDuration::minutes(10);

/// Detect the Cursor desktop application from its local installation or its
/// VS Code state database. No database is opened and no network call is made.
pub fn is_installed() -> bool {
    if cursor_state_db().is_file() {
        return true;
    }

    #[cfg(windows)]
    {
        let local_app_data = env::var_os("LOCALAPPDATA").map(PathBuf::from);
        let program_files = env::var_os("ProgramFiles").map(PathBuf::from);
        return local_app_data
            .into_iter()
            .map(|path| path.join("Programs/Cursor/Cursor.exe"))
            .chain(
                program_files
                    .into_iter()
                    .map(|path| path.join("Cursor/Cursor.exe")),
            )
            .any(|path| path.is_file());
    }

    #[cfg(not(windows))]
    false
}

pub struct CursorClient {
    agent: ureq::Agent,
}

pub struct CursorActivator;

impl CursorClient {
    pub fn new() -> Self {
        Self {
            agent: ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(15))
                .build(),
        }
    }

    fn read_limits(&self) -> Result<RateLimits> {
        let auth = CursorAuth::load()?;
        let token = self.access_token(&auth)?;
        // Cursor's Connect endpoint is occasionally unavailable for otherwise
        // valid desktop sessions. The dashboard's REST summary carries the
        // same Auto/API counters for current plans, so either source is
        // sufficient; only fail when both are unusable.
        let usage = self
            .connect_post("/aiserver.v1.DashboardService/GetCurrentPeriodUsage", &token);
        let summary = self.usage_summary(&token);
        match (usage, summary) {
            (Ok(usage), Ok(summary)) => map_usage(Some(&usage), Some(&summary)),
            (Ok(usage), Err(_)) => map_usage(Some(&usage), None),
            (Err(_), Ok(summary)) => map_usage(None, Some(&summary)),
            (Err(usage_error), Err(summary_error)) => Err(usage_error).context(format!(
                "Cursor usage summary fallback also failed: {summary_error:#}"
            )),
        }
    }

    fn access_token(&self, auth: &CursorAuth) -> Result<String> {
        if !token_needs_refresh(&auth.access_token) {
            return Ok(auth.access_token.clone());
        }
        let response: Value = self
            .agent
            .post(&format!("{API_BASE}/oauth/token"))
            .set("Content-Type", "application/json")
            .send_string(&json!({
                "grant_type": "refresh_token",
                "client_id": CURSOR_CLIENT_ID,
                "refresh_token": auth.refresh_token,
            }).to_string())
            .context("refresh Cursor access token")?
            .into_string()
            .context("read Cursor token refresh response")
            .and_then(|body| serde_json::from_str(&body).context("parse Cursor token refresh response"))?;
        response
            .get("access_token")
            .and_then(Value::as_str)
            .filter(|token| !token.trim().is_empty())
            .map(str::to_owned)
            .context("Cursor token refresh returned no access token")
    }

    fn connect_post(&self, path: &str, token: &str) -> Result<Value> {
        self.agent
            .post(&format!("{API_BASE}{path}"))
            .set("Authorization", &format!("Bearer {token}"))
            .set("Content-Type", "application/json")
            .set("Connect-Protocol-Version", "1")
            .send_string("{}")
            .with_context(|| format!("request Cursor usage at {path}"))?
            .into_string()
            .context("read Cursor usage response")
            .and_then(|body| serde_json::from_str(&body).context("parse Cursor usage response"))
    }

    fn usage_summary(&self, token: &str) -> Result<Value> {
        let user_id = cursor_user_id(token).context("Cursor token has no user identity")?;
        self.agent
            .get(&format!("{CURSOR_BASE}/api/usage-summary"))
            .set("Cookie", &format!("WorkosCursorSessionToken={user_id}%3A%3A{token}"))
            .call()
            .context("request Cursor usage summary")?
            .into_string()
            .context("read Cursor usage summary")
            .and_then(|body| serde_json::from_str(&body).context("parse Cursor usage summary"))
    }

    fn usage_statistics(&self, history_days: u16) -> Result<UsageStatistics> {
        let cached = store::with_store(|store| {
            if store.cursor_usage_version()? != USAGE_CACHE_VERSION {
                return Ok(None);
            }
            let Some(fetched_at) = store.usage_fetched_at(ProviderKind::Cursor)? else {
                return Ok(None);
            };
            if Utc::now() - fetched_at >= USAGE_CACHE_TTL {
                return Ok(None);
            }
            store
                .load_usage_daily(ProviderKind::Cursor, history_days)
                .map(Some)
        })?;
        if let Some(statistics) = cached {
            return Ok(statistics);
        }

        match self.download_usage_statistics(history_days) {
            Ok(statistics) => {
                store::with_store(|store| {
                    store.replace_usage_daily(ProviderKind::Cursor, &statistics.daily)?;
                    store.set_usage_fetched_at(ProviderKind::Cursor, Utc::now())
                })?;
                Ok(statistics)
            }
            // An export can be delayed or intermittently rejected by Cursor.
            // Keep showing the last verified activity rather than making a
            // healthy usage card disappear on a transient network failure.
            Err(error) => store::with_store(|store| {
                store.load_usage_daily(ProviderKind::Cursor, history_days)
            })
            .context("refresh Cursor usage export")
            .or(Err(error)),
        }
    }

    fn download_usage_statistics(&self, history_days: u16) -> Result<UsageStatistics> {
        let auth = CursorAuth::load()?;
        let token = self.access_token(&auth)?;
        let user_id = cursor_user_id(&token).context("Cursor token has no user identity")?;
        let now = Utc::now();
        let start = now - ChronoDuration::days(29);
        let csv = self
            .agent
            .get(&format!("{CURSOR_BASE}{USAGE_EXPORT_PATH}"))
            .query("startDate", &start.timestamp_millis().to_string())
            .query("endDate", &now.timestamp_millis().to_string())
            .query("strategy", "tokens")
            .set("Accept", "text/csv")
            .set("Cookie", &format!("WorkosCursorSessionToken={user_id}%3A%3A{token}"))
            .call()
            .context("request Cursor usage export")?
            .into_string()
            .context("read Cursor usage export")?;
        usage_statistics_from_csv(&csv, history_days)
    }
}

impl LimitProvider for CursorClient {
    fn read_limits(&mut self) -> Result<RateLimits> {
        CursorClient::read_limits(self)
    }
}

impl UsageProvider for CursorClient {
    fn load_cached_usage_statistics(&mut self, history_days: u16) -> Result<UsageStatistics> {
        store::with_store(|store| store.load_usage_daily(ProviderKind::Cursor, history_days))
            .or_else(|_| Ok(UsageStatistics::default()))
    }

    fn refresh_usage_statistics(&mut self, history_days: u16) -> Result<UsageStatistics> {
        self.usage_statistics(history_days)
    }
}

fn usage_statistics_from_csv(csv_text: &str, history_days: u16) -> Result<UsageStatistics> {
    const DATE: &str = "Date";
    const MODEL: &str = "Model";
    const CACHE_WRITE: &str = "Input (w/ Cache Write)";
    const INPUT: &str = "Input (w/o Cache Write)";
    const CACHE_READ: &str = "Cache Read";
    const OUTPUT: &str = "Output Tokens";

    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(csv_text.as_bytes());
    let headers = reader.headers().context("read Cursor usage export headers")?.clone();
    let column = |name: &str| headers.iter().position(|header| header == name)
        .with_context(|| format!("Cursor usage export is missing {name}"));
    let date_column = column(DATE)?;
    let model_column = column(MODEL)?;
    let cache_write_column = column(CACHE_WRITE)?;
    let input_column = column(INPUT)?;
    let cache_read_column = column(CACHE_READ)?;
    let output_column = column(OUTPUT)?;

    let mut daily = BTreeMap::<NaiveDate, TokenUsage>::new();
    for row in reader.records() {
        let Ok(row) = row else { continue };
        let Some(date) = row.get(date_column).and_then(cursor_export_date) else { continue };
        let model = row.get(model_column).unwrap_or_default();
        let Some(cache_write) = row.get(cache_write_column).and_then(cursor_export_tokens) else { continue };
        let Some(input) = row.get(input_column).and_then(cursor_export_tokens) else { continue };
        let Some(cache_read) = row.get(cache_read_column).and_then(cursor_export_tokens) else { continue };
        let Some(output) = row.get(output_column).and_then(cursor_export_tokens) else { continue };
        let usage = daily.entry(date).or_default();
        usage.input_tokens = usage
            .input_tokens
            .saturating_add(input)
            .saturating_add(cache_write)
            .saturating_add(cache_read);
        usage.cached_input_tokens = usage.cached_input_tokens.saturating_add(cache_read);
        usage.output_tokens = usage.output_tokens.saturating_add(output);
        // Export rows are aggregates rather than individual requests; retain a
        // row count so the common usage card can still report activity.
        usage.requests = usage.requests.saturating_add(1);
        usage.estimated_cost_microusd = usage
            .estimated_cost_microusd
            .saturating_add(cursor_estimated_cost_microusd(model, cache_write, input, cache_read, output));
        usage.priced_requests = usage.priced_requests.saturating_add(1);
    }

    let daily = daily
        .into_iter()
        .map(|(date, usage)| DailyTokenUsage { date, usage })
        .collect::<Vec<_>>();
    Ok(statistics_from_daily(&daily, history_days))
}
fn cursor_estimated_cost_microusd(
    model: &str,
    cache_write: u64,
    input: u64,
    cache_read: u64,
    output: u64,
) -> u64 {
    let model = model.trim().to_ascii_lowercase();
    let (input_rate, cache_read_rate, output_rate) = if model.contains("claude-opus") {
        (15.0, 1.5, 75.0)
    } else if model.contains("claude-sonnet") {
        (3.0, 0.3, 15.0)
    } else if model.contains("claude-haiku") {
        (1.0, 0.1, 5.0)
    } else if model.contains("gemini-2.5-pro") {
        (1.25, 0.3125, 10.0)
    } else if model.contains("gemini") {
        (0.3, 0.03, 2.5)
    } else if model.contains("gpt-5.4") {
        (2.5, 0.25, 15.0)
    } else if model.contains("gpt-5.3") || model.contains("gpt-5.2") {
        (1.75, 0.175, 14.0)
    } else if model.contains("gpt-5") {
        (1.25, 0.125, 10.0)
    } else if model.contains("composer") || model == "auto" {
        (1.25, 0.25, 6.0)
    } else {
        // Cursor occasionally exports a new alias before its public price is
        // published. Use the current Auto baseline so usage never vanishes.
        (1.25, 0.25, 6.0)
    };
    let cost = (cache_write as f64 * input_rate * 1.25
        + input as f64 * input_rate
        + cache_read as f64 * cache_read_rate
        + output as f64 * output_rate)
        / 1_000_000.0;
    (cost * 1_000_000.0).round().clamp(0.0, u64::MAX as f64) as u64
}

fn cursor_export_date(value: &str) -> Option<NaiveDate> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|date| date.with_timezone(&Local).date_naive())
        .or_else(|| NaiveDate::parse_from_str(value.get(..10)?, "%Y-%m-%d").ok())
}

fn cursor_export_tokens(value: &str) -> Option<u64> {
    let normalized = value.trim().replace(',', "");
    if normalized.is_empty() { Some(0) } else { normalized.parse().ok() }
}

impl Activator for CursorActivator {
    fn activate(&mut self) -> Result<()> {
        // Cursor billing windows are calendar-based; unlike Codex/Claude,
        // there is no harmless request that starts a session window.
        Ok(())
    }
}

struct CursorAuth {
    access_token: String,
    refresh_token: String,
}

impl CursorAuth {
    fn load() -> Result<Self> {
        let db_path = cursor_state_db();
        let db = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| format!("open Cursor session database at {}", db_path.display()))?;
        let value = |key: &str| -> Result<String> {
            db.query_row("SELECT value FROM ItemTable WHERE key = ?1 LIMIT 1", [key], |row| row.get(0))
                .with_context(|| format!("read {key} from Cursor session database"))
        };
        Ok(Self {
            access_token: value(ACCESS_TOKEN_KEY)?,
            refresh_token: value(REFRESH_TOKEN_KEY)?,
        })
    }
}

fn cursor_state_db() -> PathBuf {
    env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|dirs| dirs.config_dir().to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Cursor/User/globalStorage/state.vscdb")
}

fn token_needs_refresh(token: &str) -> bool {
    let Some(exp) = jwt_payload(token)
        .and_then(|payload| payload.get("exp").and_then(Value::as_i64))
    else {
        return true;
    };
    exp <= Utc::now().timestamp() + 5 * 60
}

fn cursor_user_id(token: &str) -> Option<String> {
    let payload = jwt_payload(token)?;
    let subject = payload.get("sub")?.as_str()?;
    let id = subject.rsplit('|').next()?.trim();
    (!id.is_empty()).then(|| id.to_owned())
}

fn jwt_payload(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn map_usage(usage: Option<&Value>, summary: Option<&Value>) -> Result<RateLimits> {
    let now = Utc::now();
    let plan = usage
        .and_then(|usage| usage.get("planUsage"))
        .and_then(Value::as_object);
    let summary_plan = summary
        .and_then(|value| value.pointer("/individualUsage/plan"))
        .and_then(Value::as_object);
    let enabled = plan.is_some();
    if !enabled && summary_plan.is_none() {
        bail!("Cursor returned no active subscription usage");
    }

    let (resets_at, duration_minutes) = billing_cycle(summary, usage);
    let make_window = |percent: f64| LimitWindow {
        used_percent: Some(percent.round().clamp(0.0, 100.0) as u8),
        resets_at,
        duration_minutes,
    };
    let auto_percent = number(summary_plan.and_then(|plan| plan.get("autoPercentUsed")))
        .or_else(|| number(plan.and_then(|plan| plan.get("autoPercentUsed"))));
    let api_percent = number(summary_plan.and_then(|plan| plan.get("apiPercentUsed")))
        .or_else(|| number(plan.and_then(|plan| plan.get("apiPercentUsed"))));
    let mut additional_limits = Vec::new();
    if let Some(percent) = api_percent {
        additional_limits.push(AdditionalLimit {
            id: "cursor-api".into(),
            title: "API".into(),
            window: make_window(percent),
        });
    }

    Ok(RateLimits {
        // Total Usage blends separate allowances and is intentionally hidden.
        primary: LimitWindow::default(),
        secondary: auto_percent.map(make_window).unwrap_or_default(),
        sampled_at: now,
        account_name: None,
        plan_type: usage
            .and_then(|usage| usage.pointer("/planUsage/planName"))
            .or_else(|| summary.and_then(|value| value.get("membershipType")))
            .and_then(Value::as_str)
            .map(str::to_owned),
        limit_name: Some("Cursor".into()),
        additional_limits,
        ..RateLimits::default()
    })
}

fn billing_cycle(summary: Option<&Value>, usage: Option<&Value>) -> (Option<DateTime<Utc>>, Option<u32>) {
    let start = summary
        .and_then(|value| value.get("billingCycleStart"))
        .and_then(Value::as_str)
        .and_then(parse_time)
        .or_else(|| usage.and_then(|usage| usage.get("startOfMonth")).and_then(Value::as_str).and_then(parse_time));
    let end = summary
        .and_then(|value| value.get("billingCycleEnd"))
        .and_then(Value::as_str)
        .and_then(parse_time);
    let duration = match (start, end) {
        (Some(start), Some(end)) if end > start => u32::try_from((end - start).num_minutes()).ok(),
        _ => Some(31 * 24 * 60),
    };
    let reset = end.or_else(|| start.map(|start| start + chrono::Duration::minutes(i64::from(duration.unwrap_or(44_640)))));
    (reset, duration)
}

fn parse_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value).ok().map(|time| time.with_timezone(&Utc))
}

fn number(value: Option<&Value>) -> Option<f64> {
    value.and_then(Value::as_f64).or_else(|| value.and_then(Value::as_i64).map(|value| value as f64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_cursor_auto_and_api_percentages_and_cycle() {
        let usage = json!({"planUsage":{"limit":10000,"totalSpend":2500,"autoPercentUsed":12.4,"apiPercentUsed":3.6}});
        let summary = json!({"billingCycleStart":"2026-07-01T00:00:00Z","billingCycleEnd":"2026-08-01T00:00:00Z","individualUsage":{"plan":{"totalPercentUsed":25.0}}});
        let limits = map_usage(Some(&usage), Some(&summary)).unwrap();
        assert!(limits.primary.is_empty());
        assert_eq!(limits.secondary.used_percent, Some(12));
        assert_eq!(limits.additional_limits[0].title, "API");
        assert_eq!(limits.secondary.duration_minutes, Some(31 * 24 * 60));
    }

    #[test]
    fn maps_rest_summary_when_connect_usage_is_unavailable() {
        let summary = json!({
            "billingCycleStart":"2026-07-01T00:00:00Z",
            "billingCycleEnd":"2026-08-01T00:00:00Z",
            "membershipType":"pro",
            "individualUsage":{"plan":{"totalPercentUsed":25.0,"autoPercentUsed":10.0,"apiPercentUsed":5.0}}
        });
        let limits = map_usage(None, Some(&summary)).unwrap();
        assert!(limits.primary.is_empty());
        assert_eq!(limits.secondary.used_percent, Some(10));
        assert_eq!(limits.additional_limits[0].window.used_percent, Some(5));
    }

    #[test]
    fn turns_cursor_export_rows_into_usage_card_statistics() {
        let date = Local::now().date_naive().format("%Y-%m-%d");
        let csv = format!(
            "Date,Model,Input (w/ Cache Write),Input (w/o Cache Write),Cache Read,Output Tokens\n{date} 12:00:00,gpt-5,10,20,30,40\n"
        );
        let statistics = usage_statistics_from_csv(&csv, 30).unwrap();
        assert_eq!(statistics.today.input_tokens, 60);
        assert_eq!(statistics.today.cached_input_tokens, 30);
        assert_eq!(statistics.today.output_tokens, 40);
        assert_eq!(statistics.today.total_tokens(), 100);
    }

    #[test]
    fn cached_cursor_usage_is_available_without_downloading_again() {
        let today = Local::now().date_naive();
        let statistics = statistics_from_daily(
            &[DailyTokenUsage {
                date: today,
                usage: TokenUsage {
                    input_tokens: 12,
                    output_tokens: 8,
                    requests: 1,
                    ..Default::default()
                },
            }],
            30,
        );
        assert_eq!(statistics.today.total_tokens(), 20);
        assert_eq!(statistics.history.requests, 1);
    }
}
