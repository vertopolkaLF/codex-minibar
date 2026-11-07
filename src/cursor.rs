//! Cursor dashboard usage provider.
//!
//! Cursor persists its OAuth session in the application's VS Code state DB.
//! We open that database read-only, refresh an expired access token only in
//! memory, and query the same dashboard endpoints used by Cursor itself.
//! The UI exposes Cursor Models, Other Models, the opt-in All Models aggregate,
//! and Grok Bot as separate lanes.

use std::{
    env,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration as ChronoDuration, Local, NaiveDate, NaiveDateTime, Utc};
use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};

use crate::{
    drivers::{
        Driver, FolderField, InstallSource, InstallStatus, SettingsCopy, Sources, display_path,
    },
    instances::{ProviderId, ProviderInstance},
    limits::{AdditionalLimit, LimitWindow, RateLimits},
    pricing::{self, RequestTokens},
    provider_registry::ProviderKind,
    store,
    usage::{Rollup, RollupBuilder, TokenUsage, UsageStatistics},
    worker::{LimitProvider, UsageProvider, WorkerHandle, WorkerOptions},
};

const API_BASE: &str = "https://api2.cursor.sh";
const CURSOR_BASE: &str = "https://cursor.com";
const CURSOR_CLIENT_ID: &str = "KbZUR41cY7W6zRSdpSUJ7I7mLYBKOCmB";
const ACCESS_TOKEN_KEY: &str = "cursorAuth/accessToken";
const REFRESH_TOKEN_KEY: &str = "cursorAuth/refreshToken";
const USAGE_EXPORT_PATH: &str = "/api/dashboard/export-usage-events-csv";
pub(crate) const USAGE_CACHE_VERSION: u8 = 8;
const USAGE_CACHE_TTL: ChronoDuration = ChronoDuration::minutes(10);

pub(crate) static DRIVER: Driver = Driver {
    start: start_worker,
    detect,
    sources: Sources::APP,
    copy: settings_copy,
    forget_secrets: None,
    consume_reset: None,
};

fn start_worker(instance: &ProviderInstance, options: WorkerOptions) -> Result<WorkerHandle> {
    let executable = installation_path(instance.binary_path.as_deref())
        .unwrap_or_else(|| PathBuf::from("Cursor.exe"));
    crate::logger::info(format!("Cursor executable: {}", executable.display()));
    let client = CursorClient::new();
    Ok(crate::worker::start_worker(
        client.clone(),
        client,
        None,
        options,
    ))
}

/// Looks for Cursor's installation and its VS Code state database. The
/// database is never opened and no network call is made.
fn detect(instance: &ProviderInstance) -> InstallStatus {
    let app = installation_path(instance.binary_path.as_deref());
    InstallStatus {
        used: app.as_ref().map(|_| InstallSource::App),
        app: app.as_deref().map(display_path),
        signed_in: cursor_state_db().is_file(),
        ..InstallStatus::none(instance)
    }
}

fn settings_copy() -> SettingsCopy {
    SettingsCopy {
        app: crate::i18n::tr("cursor-app"),
        folders: vec![FolderField::binary(
            crate::i18n::tr("cursor-app-folder"),
            crate::i18n::tr("folder-with-cursor-exe-leave-empty-to-find-it-automatically-usage"),
            r"C:\Users\you\AppData\Local\Programs\Cursor",
        )],
        binary_source: Some(InstallSource::App),
        ..SettingsCopy::new(crate::i18n::tr(
            "reads-the-signed-in-cursor-app-for-this-billing-cycle",
        ))
    }
}

/// Resolves Cursor.exe without opening its profile database. The configured
/// value is a folder; accepting a file keeps pre-release settings compatible.
pub fn installation_path(explicit: Option<&Path>) -> Option<PathBuf> {
    explicit
        .and_then(|path| {
            if path.is_file() {
                Some(path.to_path_buf())
            } else {
                let executable = path.join("Cursor.exe");
                executable.is_file().then_some(executable)
            }
        })
        .or_else(|| {
            #[cfg(windows)]
            {
                let local_app_data = env::var_os("LOCALAPPDATA").map(PathBuf::from);
                let program_files = env::var_os("ProgramFiles").map(PathBuf::from);
                local_app_data
                    .into_iter()
                    .map(|path| path.join("Programs/Cursor/Cursor.exe"))
                    .chain(
                        program_files
                            .into_iter()
                            .map(|path| path.join("Cursor/Cursor.exe")),
                    )
                    .find(|path| path.is_file())
            }
            #[cfg(not(windows))]
            {
                None
            }
        })
}

#[derive(Clone)]
pub struct CursorClient {
    agent: ureq::Agent,
}

impl Default for CursorClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Cursor reads this PC's single login, so it only ever has one instance.
const CURSOR: ProviderId = ProviderId::primary(ProviderKind::Cursor);

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
        // Run the requests concurrently — they share a token and are independent.
        // Grok Bot is a separate weekly allowance; a failed Sand lookup must
        // never discard otherwise valid Auto/API usage.
        let (usage, summary, sand) = std::thread::scope(|scope| {
            let usage = scope.spawn(|| {
                self.connect_post(
                    "/aiserver.v1.DashboardService/GetCurrentPeriodUsage",
                    &token,
                )
            });
            let summary = scope.spawn(|| self.usage_summary(&token));
            let sand = scope.spawn(|| {
                self.connect_post("/aiserver.v1.DashboardService/GetSandUsageStatus", &token)
            });
            (
                usage
                    .join()
                    .unwrap_or_else(|_| Err(anyhow!("Cursor usage worker panicked"))),
                summary
                    .join()
                    .unwrap_or_else(|_| Err(anyhow!("Cursor summary worker panicked"))),
                sand.join()
                    .unwrap_or_else(|_| Err(anyhow!("Cursor Grok Bot worker panicked"))),
            )
        });
        if let Err(error) = &usage
            && crate::net::is_rate_limited_error(error)
        {
            return Err(crate::net::rate_limit_error(format!(
                "Cursor quota request was rate limited: {error:#}"
            )));
        }
        if let Err(error) = &summary
            && crate::net::is_rate_limited_error(error)
        {
            return Err(crate::net::rate_limit_error(format!(
                "Cursor quota summary was rate limited: {error:#}"
            )));
        }
        if let Err(error) = &sand
            && crate::net::is_rate_limited_error(error)
        {
            return Err(crate::net::rate_limit_error(format!(
                "Cursor Grok Bot quota was rate limited: {error:#}"
            )));
        }
        let sand = sand.as_ref().ok();
        match (usage, summary) {
            (Ok(usage), Ok(summary)) => map_usage(Some(&usage), Some(&summary), sand),
            (Ok(usage), Err(_)) => map_usage(Some(&usage), None, sand),
            (Err(_), Ok(summary)) => map_usage(None, Some(&summary), sand),
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
            .send_string(
                &json!({
                    "grant_type": "refresh_token",
                    "client_id": CURSOR_CLIENT_ID,
                    "refresh_token": auth.refresh_token,
                })
                .to_string(),
            )
            .context("refresh Cursor access token")?
            .into_string()
            .context("read Cursor token refresh response")
            .and_then(|body| {
                serde_json::from_str(&body).context("parse Cursor token refresh response")
            })?;
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
            .set(
                "Cookie",
                &format!("WorkosCursorSessionToken={user_id}%3A%3A{token}"),
            )
            .call()
            .context("request Cursor usage summary")?
            .into_string()
            .context("read Cursor usage summary")
            .and_then(|body| serde_json::from_str(&body).context("parse Cursor usage summary"))
    }

    fn usage_statistics(&self, history_days: u16) -> Result<UsageStatistics> {
        let cached = store::with_store(|store| {
            if store.usage_version(CURSOR)? != USAGE_CACHE_VERSION {
                return Ok(None);
            }
            let Some(fetched_at) = store.usage_fetched_at(CURSOR)? else {
                return Ok(None);
            };
            if Utc::now() - fetched_at >= USAGE_CACHE_TTL {
                return Ok(None);
            }
            let start = crate::local_time::hourly_window_start(Local::now());
            let end = crate::local_time::truncate_hour(Local::now());
            if store.load_usage_hourly(CURSOR, start, end)?.is_empty() {
                return Ok(None);
            }
            store.load_usage_daily(CURSOR, history_days).map(Some)
        })?;
        if let Some(statistics) = cached {
            return Ok(statistics);
        }

        match self.download_usage() {
            Ok((rollup, fetched_since)) => {
                store::with_store(|store| {
                    // Only the fetched window is authoritative; older stored
                    // days are accumulated history and must survive.
                    store.save_rollup(CURSOR, &rollup, Some(fetched_since))?;
                    store.set_usage_fetched_at(CURSOR, Utc::now(), USAGE_CACHE_VERSION)?;
                    store.load_usage_daily(CURSOR, history_days)
                })
            }
            // An export can be delayed or intermittently rejected by Cursor.
            // Keep showing the last verified activity rather than making a
            // healthy usage card disappear on a transient network failure.
            Err(error) => {
                let cached =
                    store::with_store(|store| store.load_usage_daily(CURSOR, history_days))
                        .context("read cached Cursor usage after export failure")?;
                if cached.has_data() {
                    if crate::net::is_rate_limited_error(&error) {
                        Ok(UsageStatistics {
                            error: Some("Cursor usage export was rate limited (HTTP 429).".into()),
                            ..cached
                        })
                    } else {
                        Ok(cached)
                    }
                } else {
                    Err(error.context("refresh Cursor usage export with no cached data"))
                }
            }
        }
    }

    /// Returns the export's rollup and the first local date it covered.
    fn download_usage(&self) -> Result<(Rollup, NaiveDate)> {
        let auth = CursorAuth::load()?;
        let token = self.access_token(&auth)?;
        let user_id = cursor_user_id(&token).context("Cursor token has no user identity")?;
        let now = Utc::now();
        // Start at a local midnight so the oldest fetched day is complete and
        // can safely overwrite the stored total for that date.
        let first_day = Local::now().date_naive() - ChronoDuration::days(29);
        let start = first_day
            .and_hms_opt(0, 0, 0)
            .and_then(|midnight| midnight.and_local_timezone(Local).earliest())
            .map_or(now - ChronoDuration::days(29), |at| at.with_timezone(&Utc));
        let csv = self
            .agent
            .get(&format!("{CURSOR_BASE}{USAGE_EXPORT_PATH}"))
            .query("startDate", &start.timestamp_millis().to_string())
            .query("endDate", &now.timestamp_millis().to_string())
            .query("strategy", "tokens")
            .set("Accept", "text/csv")
            .set(
                "Cookie",
                &format!("WorkosCursorSessionToken={user_id}%3A%3A{token}"),
            )
            .call()
            .context("request Cursor usage export")?
            .into_string()
            .context("read Cursor usage export")?;
        Ok((
            rollup_from_csv(&csv)?,
            start.with_timezone(&Local).date_naive(),
        ))
    }
}

impl LimitProvider for CursorClient {
    fn read_limits(&mut self) -> Result<RateLimits> {
        CursorClient::read_limits(self)
    }
}

impl UsageProvider for CursorClient {
    fn load_cached_usage_statistics(&mut self, history_days: u16) -> Result<UsageStatistics> {
        store::with_store(|store| store.load_usage_daily(CURSOR, history_days)).or_else(|error| {
            crate::logger::info(format!("failed to read cached Cursor usage: {error:#}"));
            Ok(UsageStatistics::default())
        })
    }

    fn refresh_usage_statistics(&mut self, history_days: u16) -> Result<UsageStatistics> {
        self.usage_statistics(history_days)
    }

    /// Cursor's usage export is independent of the quota endpoints. A
    /// temporary limits failure must not suppress the local usage scan.
    fn refresh_without_limits(&self) -> bool {
        true
    }
}

fn rollup_from_csv(csv_text: &str) -> Result<Rollup> {
    const DATE: &str = "Date";
    const MODEL: &str = "Model";
    const CACHE_WRITE: &str = "Input (w/ Cache Write)";
    const INPUT: &str = "Input (w/o Cache Write)";
    const CACHE_READ: &str = "Cache Read";
    const OUTPUT: &str = "Output Tokens";

    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(csv_text.as_bytes());
    let headers = reader
        .headers()
        .context("read Cursor usage export headers")?
        .clone();
    let column = |name: &str| {
        headers
            .iter()
            .position(|header| header == name)
            .with_context(|| format!("Cursor usage export is missing {name}"))
    };
    let date_column = column(DATE)?;
    let model_column = column(MODEL)?;
    let cache_write_column = column(CACHE_WRITE)?;
    let input_column = column(INPUT)?;
    let cache_read_column = column(CACHE_READ)?;
    let output_column = column(OUTPUT)?;

    let mut rollup = RollupBuilder::new(Local::now());
    let mut rows = 0_usize;
    for row in reader.records() {
        let Ok(row) = row else { continue };
        let Some((date, timestamp)) = row.get(date_column).and_then(cursor_export_timestamp) else {
            continue;
        };
        let model = row.get(model_column).unwrap_or_default();
        let Some(cache_write) = row.get(cache_write_column).and_then(cursor_export_tokens) else {
            continue;
        };
        let Some(input) = row.get(input_column).and_then(cursor_export_tokens) else {
            continue;
        };
        let Some(cache_read) = row.get(cache_read_column).and_then(cursor_export_tokens) else {
            continue;
        };
        let Some(output) = row.get(output_column).and_then(cursor_export_tokens) else {
            continue;
        };
        let model = normalize_cursor_model_name(model);
        // Export rows are aggregates rather than individual requests; each
        // counts as one request so the common usage card still reports activity.
        let usage = cursor_row_usage(&model, cache_write, input, cache_read, output);
        match timestamp {
            Some(at) => rollup.add(at, &usage, Some(&model), None),
            None => rollup.add_on(date, &usage, Some(&model), None),
        }
        rows += 1;
    }
    if rows == 0 {
        bail!("Cursor usage export contained no valid usage rows");
    }
    Ok(rollup.finish())
}

/// Collapse Cursor catalog flavors onto one family id.
///
/// Wire slugs keep a `cursor-` prefix plus throwaway reasoning / effort / `-fast`
/// suffixes (`cursor-grok-4.6-high-fast`, `4.6-medium`). Those are the same
/// model for usage rollup, so they become `grok-4.6`.
pub(crate) fn normalize_cursor_model_name(model: &str) -> String {
    let mut name = model
        .trim()
        .to_ascii_lowercase()
        .trim_end_matches('-')
        .to_owned();
    if name.is_empty() {
        return "unknown".into();
    }
    if let Some(rest) = name.strip_prefix("cursor-") {
        name = rest.to_string();
    }
    while let Some(stripped) = name
        .strip_suffix("-fast")
        .or_else(|| name.strip_suffix("-thinking"))
        .or_else(|| name.strip_suffix("-reasoning"))
        .or_else(|| name.strip_suffix("-throwaway"))
        .or_else(|| name.strip_suffix("-xhigh"))
        .or_else(|| name.strip_suffix("-high"))
        .or_else(|| name.strip_suffix("-medium"))
        .or_else(|| name.strip_suffix("-low"))
        .or_else(|| name.strip_suffix("-none"))
    {
        if stripped.is_empty() {
            break;
        }
        name = stripped.to_string();
    }
    name = name.trim_end_matches('-').to_string();
    if name.is_empty() {
        "unknown".into()
    } else {
        name
    }
}

fn cursor_row_usage(
    model: &str,
    cache_write: u64,
    input: u64,
    cache_read: u64,
    output: u64,
) -> TokenUsage {
    let tokens = RequestTokens {
        input: input.saturating_add(cache_write).saturating_add(cache_read),
        cache_read,
        cache_creation: cache_write,
        output,
    };
    pricing::request_usage(ProviderKind::Cursor, Some(model), tokens, None)
}

fn cursor_export_timestamp(value: &str) -> Option<(NaiveDate, Option<DateTime<Local>>)> {
    let value = value.trim();
    if let Ok(parsed) = DateTime::parse_from_rfc3339(value) {
        let local = parsed.with_timezone(&Local);
        return Some((local.date_naive(), Some(local)));
    }
    for format in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S%.f",
    ] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(value, format) {
            let local = naive.and_local_timezone(Local).single()?;
            return Some((local.date_naive(), Some(local)));
        }
    }
    let date = NaiveDate::parse_from_str(value.get(..10)?, "%Y-%m-%d").ok()?;
    Some((date, None))
}

fn cursor_export_tokens(value: &str) -> Option<u64> {
    let normalized = value.trim().replace(',', "");
    if normalized.is_empty() {
        Some(0)
    } else {
        normalized.parse().ok()
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
            db.query_row(
                "SELECT value FROM ItemTable WHERE key = ?1 LIMIT 1",
                [key],
                |row| row.get(0),
            )
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
    let Some(exp) =
        jwt_payload(token).and_then(|payload| payload.get("exp").and_then(Value::as_i64))
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

fn map_usage(
    usage: Option<&Value>,
    summary: Option<&Value>,
    sand: Option<&Value>,
) -> Result<RateLimits> {
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
    let all_models_percent = number(summary_plan.and_then(|plan| plan.get("totalPercentUsed")))
        .or_else(|| number(plan.and_then(|plan| plan.get("totalPercentUsed"))));
    let secondary_limit_name = auto_percent.is_some().then(|| "Cursor Models".to_owned());
    let mut additional_limits = Vec::new();
    if let Some(percent) = api_percent {
        additional_limits.push(AdditionalLimit {
            id: crate::provider_registry::CURSOR_API_LIMIT_ID.into(),
            title: "Other Models".into(),
            window: make_window(percent),
        });
    }
    if let Some(percent) = all_models_percent {
        additional_limits.push(AdditionalLimit {
            id: crate::provider_registry::CURSOR_ALL_MODELS_LIMIT_ID.into(),
            title: "All Models".into(),
            window: make_window(percent),
        });
    }
    if let Some(limit) = grok_bot_limit(sand) {
        additional_limits.push(limit);
    }

    Ok(RateLimits {
        // Keep the aggregate as an opt-in additional lane instead of blending
        // it into the independent Cursor Models and Other Models allowances.
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
        secondary_limit_name,
        additional_limits,
        ..RateLimits::default()
    })
}

fn billing_cycle(
    summary: Option<&Value>,
    usage: Option<&Value>,
) -> (Option<DateTime<Utc>>, Option<u32>) {
    let start = summary
        .and_then(|value| value.get("billingCycleStart"))
        .and_then(Value::as_str)
        .and_then(parse_time)
        .or_else(|| {
            usage
                .and_then(|usage| usage.get("startOfMonth"))
                .and_then(Value::as_str)
                .and_then(parse_time)
        });
    let end = summary
        .and_then(|value| value.get("billingCycleEnd"))
        .and_then(Value::as_str)
        .and_then(parse_time);
    let duration = match (start, end) {
        (Some(start), Some(end)) if end > start => u32::try_from((end - start).num_minutes()).ok(),
        _ => Some(31 * 24 * 60),
    };
    let reset = end.or_else(|| {
        start.map(|start| start + chrono::Duration::minutes(i64::from(duration.unwrap_or(44_640))))
    });
    (reset, duration)
}

fn parse_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

fn grok_bot_limit(sand: Option<&Value>) -> Option<AdditionalLimit> {
    let sand = sand?;
    if sand
        .get("usesPooledEnterpriseAllowance")
        .and_then(Value::as_bool)
        == Some(true)
        || sand.get("includedLimitZero").and_then(Value::as_bool) == Some(true)
        || sand.get("hasNonZeroIncludedLimit").and_then(Value::as_bool) != Some(true)
    {
        return None;
    }
    let percent = number(sand.get("usagePercent")).filter(|percent| *percent >= 0.0)?;
    let start = sand
        .get("currentPeriodStart")
        .and_then(Value::as_str)
        .and_then(parse_time);
    let resets_at = sand
        .get("nextResetTimestampUtc")
        .and_then(Value::as_str)
        .and_then(parse_time);
    let duration_minutes = match (start, resets_at) {
        (Some(start), Some(end)) if end > start => u32::try_from((end - start).num_minutes()).ok(),
        _ => Some(7 * 24 * 60),
    };
    Some(AdditionalLimit {
        id: crate::provider_registry::CURSOR_GROK_BOT_LIMIT_ID.into(),
        title: "Grok Bot".into(),
        window: LimitWindow {
            used_percent: Some(percent.round().clamp(0.0, 100.0) as u8),
            resets_at,
            duration_minutes,
        },
    })
}

fn number(value: Option<&Value>) -> Option<f64> {
    value
        .and_then(Value::as_f64)
        .or_else(|| value.and_then(Value::as_i64).map(|value| value as f64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    #[test]
    fn maps_cursor_auto_and_api_percentages_and_cycle() {
        let usage = json!({"planUsage":{"limit":10000,"totalSpend":2500,"autoPercentUsed":12.4,"apiPercentUsed":3.6}});
        let summary = json!({"billingCycleStart":"2026-07-01T00:00:00Z","billingCycleEnd":"2026-08-01T00:00:00Z","individualUsage":{"plan":{"totalPercentUsed":25.0}}});
        let limits = map_usage(Some(&usage), Some(&summary), None).unwrap();
        assert!(limits.primary.is_empty());
        assert_eq!(limits.secondary.used_percent, Some(12));
        assert_eq!(
            limits.secondary_limit_name.as_deref(),
            Some("Cursor Models")
        );
        assert_eq!(limits.additional_limits[0].title, "Other Models");
        assert_eq!(limits.additional_limits[1].title, "All Models");
        assert_eq!(limits.additional_limits[1].window.used_percent, Some(25));
        assert_eq!(limits.secondary.duration_minutes, Some(31 * 24 * 60));
    }

    #[test]
    fn maps_grok_bot_weekly_window_after_api() {
        let usage = json!({"planUsage":{"autoPercentUsed":12.4,"apiPercentUsed":3.6}});
        let sand = json!({
            "currentPeriodStart": "2026-09-01T19:10:22.052Z",
            "nextResetTimestampUtc": "2026-09-08T19:10:22.052Z",
            "usagePercent": 1.120428,
            "hasAvailableUsage": true,
            "hasNonZeroIncludedLimit": true
        });
        let limits = map_usage(Some(&usage), None, Some(&sand)).unwrap();
        assert_eq!(limits.additional_limits.len(), 2);
        assert_eq!(limits.additional_limits[0].id, "cursor-api");
        assert_eq!(limits.additional_limits[1].id, "cursor-grok-bot");
        assert_eq!(limits.additional_limits[1].title, "Grok Bot");
        assert_eq!(limits.additional_limits[1].window.used_percent, Some(1));
        assert_eq!(
            limits.additional_limits[1].window.duration_minutes,
            Some(7 * 24 * 60)
        );
        assert_eq!(
            limits.additional_limits[1].window.resets_at,
            parse_time("2026-09-08T19:10:22.052Z")
        );
    }

    #[test]
    fn hides_grok_bot_without_an_included_allowance() {
        let usage = json!({"planUsage":{"autoPercentUsed":12.4,"apiPercentUsed":3.6}});
        let sand = json!({
            "usagePercent": 100,
            "hasAvailableUsage": false,
            "hasNonZeroIncludedLimit": false
        });
        let limits = map_usage(Some(&usage), None, Some(&sand)).unwrap();
        assert_eq!(limits.additional_limits.len(), 1);
        assert_eq!(limits.additional_limits[0].id, "cursor-api");
    }

    #[test]
    fn hides_pooled_enterprise_grok_bot_meter() {
        let usage = json!({"planUsage":{"autoPercentUsed":12.4}});
        let sand = json!({
            "usagePercent": 42,
            "hasNonZeroIncludedLimit": true,
            "usesPooledEnterpriseAllowance": true
        });
        let limits = map_usage(Some(&usage), None, Some(&sand)).unwrap();
        assert!(limits.additional_limits.is_empty());
    }

    #[test]
    fn missing_grok_bot_status_leaves_auto_and_api_intact() {
        let usage = json!({"planUsage":{"autoPercentUsed":12.4,"apiPercentUsed":3.6}});
        let limits = map_usage(Some(&usage), None, None).unwrap();
        assert_eq!(limits.secondary.used_percent, Some(12));
        assert_eq!(limits.additional_limits.len(), 1);
        assert_eq!(limits.additional_limits[0].id, "cursor-api");
    }

    #[test]
    fn maps_rest_summary_when_connect_usage_is_unavailable() {
        let summary = json!({
            "billingCycleStart":"2026-07-01T00:00:00Z",
            "billingCycleEnd":"2026-08-01T00:00:00Z",
            "membershipType":"pro",
            "individualUsage":{"plan":{"totalPercentUsed":25.0,"autoPercentUsed":10.0,"apiPercentUsed":5.0}}
        });
        let limits = map_usage(None, Some(&summary), None).unwrap();
        assert!(limits.primary.is_empty());
        assert_eq!(limits.secondary.used_percent, Some(10));
        assert_eq!(limits.additional_limits[0].window.used_percent, Some(5));
        assert_eq!(limits.additional_limits[1].title, "All Models");
        assert_eq!(limits.additional_limits[1].window.used_percent, Some(25));
    }

    #[test]
    fn turns_cursor_export_rows_into_usage_card_statistics() {
        let date = Local::now().date_naive().format("%Y-%m-%d");
        let csv = format!(
            "Date,Model,Input (w/ Cache Write),Input (w/o Cache Write),Cache Read,Output Tokens\n{date} 12:00:00,gpt-5,10,20,30,40\n"
        );
        let rollup = rollup_from_csv(&csv).unwrap();
        let statistics = crate::usage::statistics_from_daily(&rollup.daily, 30);
        assert_eq!(statistics.today.input_tokens, 60);
        assert_eq!(statistics.today.cached_input_tokens, 30);
        assert_eq!(statistics.today.output_tokens, 40);
        assert_eq!(statistics.today.total_tokens(), 100);
        assert_eq!(
            cursor_export_timestamp(&format!("{date} 12:00:00"))
                .and_then(|(_, timestamp)| timestamp)
                .map(|timestamp| timestamp.hour()),
            Some(12)
        );
    }

    #[test]
    fn leaves_cursor_usage_unpriced_without_a_rate_catalog() {
        let usage = cursor_row_usage("grok-4.6", 0, 100_000, 50_000, 10_000);
        assert_eq!(usage.priced_requests, 0);
        assert_eq!(usage.estimated_cost_microusd, 0);
    }

    #[test]
    fn cursor_usage_can_refresh_without_quota_data() {
        assert!(CursorClient::new().refresh_without_limits());
    }

    #[test]
    fn cached_cursor_usage_is_available_without_downloading_again() {
        let today = Local::now().date_naive();
        let statistics = crate::usage::statistics_from_daily(
            &[crate::usage::DailyTokenUsage {
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

    #[test]
    fn normalizes_cursor_effort_and_fast_flavors_onto_one_family() {
        assert_eq!(
            normalize_cursor_model_name("cursor-grok-4.6-high-fast"),
            "grok-4.6"
        );
        assert_eq!(
            normalize_cursor_model_name("cursor-grok-4.6-medium"),
            "grok-4.6"
        );
        assert_eq!(normalize_cursor_model_name("4.6-high"), "4.6");
        assert_eq!(normalize_cursor_model_name("4.6-medium-fast"), "4.6");
        assert_eq!(
            normalize_cursor_model_name("claude-opus-4-8-thinking-"),
            "claude-opus-4-8"
        );
        assert_eq!(normalize_cursor_model_name("auto"), "auto");
        assert_eq!(normalize_cursor_model_name("  "), "unknown");
    }
}
