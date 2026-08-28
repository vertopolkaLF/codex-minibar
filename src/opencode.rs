//! OpenCode Zen and OpenCode Go provider support.
//!
//! Zen authentication/model discovery comes from the OpenCode API. Go quota
//! windows come from its account-wide usage endpoint. Both providers use the
//! local OpenCode SQLite history for cost-oriented activity cards.

use std::{collections::BTreeMap, env, path::PathBuf, time::Duration};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Local, NaiveDate, Utc};
use directories::BaseDirs;
use rusqlite::{Connection, OpenFlags, params};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    limits::{AdditionalLimit, LimitWindow, RateLimits},
    secrets,
    settings::ProviderKind,
    store,
    usage::{DailyTokenUsage, TokenUsage, UsageStatistics, statistics_from_daily},
    worker::{Activator, LimitProvider, UsageProvider},
};

const ZEN_MODELS_URL: &str = "https://opencode.ai/zen/v1/models";
const GO_USAGE_URL: &str = "https://opencode.ai/zen/go/v1/usage";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const LOCAL_HISTORY_DAYS: i64 = 365;
const ZEN_SECRET_NAME: &str = "opencode-zen";
const GO_SECRET_NAME: &str = "opencode-go";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Catalog {
    Zen,
    Go,
}

impl Catalog {
    const fn provider(self) -> ProviderKind {
        match self {
            Self::Zen => ProviderKind::OpenCodeZen,
            Self::Go => ProviderKind::OpenCodeGo,
        }
    }

    const fn provider_id(self) -> &'static str {
        match self {
            Self::Zen => "opencode",
            Self::Go => "opencode-go",
        }
    }

    const fn secret_name(self) -> &'static str {
        match self {
            Self::Zen => ZEN_SECRET_NAME,
            Self::Go => GO_SECRET_NAME,
        }
    }
}

pub fn is_installed(provider: ProviderKind) -> bool {
    let Some(catalog) = catalog(provider) else {
        return false;
    };
    resolve_api_key(catalog).ok().flatten().is_some()
        || database_has_provider(catalog).unwrap_or(false)
}

pub fn manual_key(provider: ProviderKind) -> Result<Option<String>> {
    catalog(provider)
        .map(|catalog| secrets::load(catalog.secret_name()))
        .unwrap_or_else(|| Ok(None))
}

pub fn save_manual_key(provider: ProviderKind, value: Option<&str>) -> Result<()> {
    let catalog = catalog(provider).context("provider is not an OpenCode catalog")?;
    secrets::save(catalog.secret_name(), value)
}

pub fn key_is_configured(provider: ProviderKind) -> bool {
    manual_key(provider)
        .ok()
        .flatten()
        .is_some_and(|value| !value.trim().is_empty())
}

pub struct OpenCodeClient {
    catalog: Catalog,
    agent: ureq::Agent,
}

impl OpenCodeClient {
    pub fn new(provider: ProviderKind) -> Result<Self> {
        let catalog = catalog(provider).context("provider is not an OpenCode catalog")?;
        Ok(Self {
            catalog,
            agent: ureq::AgentBuilder::new().timeout(REQUEST_TIMEOUT).build(),
        })
    }

    fn read_limits(&self) -> Result<RateLimits> {
        match self.catalog {
            Catalog::Zen => self.read_zen_limits(),
            Catalog::Go => self.read_go_limits(),
        }
    }

    fn read_zen_limits(&self) -> Result<RateLimits> {
        let key = resolve_api_key(self.catalog)?.context(
            "OpenCode Zen API key not found; set OPENCODE_API_KEY, ZEN_API_KEY, use auth.json, or save a manual key",
        )?;
        let response = self
            .agent
            .get(ZEN_MODELS_URL)
            .set("Authorization", &format!("Bearer {key}"))
            .set("Accept", "application/json")
            .call()
            .context("request OpenCode Zen models")?;
        let status = response.status();
        let body = response
            .into_string()
            .context("read OpenCode Zen models response")?;
        if status != 200 {
            return Err(api_error("OpenCode Zen", status, &body));
        }
        let model_count = parse_model_count(&body)?;
        Ok(RateLimits {
            sampled_at: Utc::now(),
            plan_type: Some(format!("Zen · {model_count} models")),
            limit_name: Some("OpenCode Zen".into()),
            ..RateLimits::default()
        })
    }

    fn read_go_limits(&self) -> Result<RateLimits> {
        let key = resolve_api_key(self.catalog)?.context(
            "OpenCode Go API key not found; set OPENCODE_GO_API_KEY, OPENCODE_API_KEY, use auth.json, or save a manual key",
        )?;
        let response = self
            .agent
            .get(GO_USAGE_URL)
            .set("Authorization", &format!("Bearer {key}"))
            .set("Accept", "application/json")
            .call()
            .context("request OpenCode Go usage")?;
        let status = response.status();
        let body = response
            .into_string()
            .context("read OpenCode Go usage response")?;
        if status != 200 {
            return Err(api_error("OpenCode Go", status, &body));
        }
        parse_go_usage(&body, Utc::now())
    }

    fn read_local_usage(&self, history_days: u16) -> Result<UsageStatistics> {
        let Some(path) = database_path() else {
            bail!("OpenCode data directory is unavailable")
        };
        self.read_local_usage_from_path(&path, history_days)
    }

    fn read_local_usage_from_path(
        &self,
        path: &PathBuf,
        history_days: u16,
    ) -> Result<UsageStatistics> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("open OpenCode database read-only at {}", path.display()))?;
        let oldest_ms =
            (Utc::now() - chrono::Duration::days(LOCAL_HISTORY_DAYS)).timestamp_millis();
        let mut statement = connection.prepare(
            "SELECT
                 CAST(COALESCE(json_extract(data, '$.time.created'), time_created) AS INTEGER),
                 CAST(json_extract(data, '$.cost') AS REAL),
                 COALESCE(CAST(json_extract(data, '$.tokens.input') AS INTEGER), 0),
                 COALESCE(CAST(json_extract(data, '$.tokens.cache.read') AS INTEGER), 0),
                 COALESCE(CAST(json_extract(data, '$.tokens.output') AS INTEGER), 0),
                 COALESCE(CAST(json_extract(data, '$.tokens.reasoning') AS INTEGER), 0)
             FROM message
             WHERE json_valid(data)
               AND json_extract(data, '$.providerID') = ?1
               AND json_extract(data, '$.role') = 'assistant'
               AND json_type(data, '$.cost') IN ('integer', 'real')
               AND CAST(COALESCE(json_extract(data, '$.time.created'), time_created) AS INTEGER) >= ?2
             ORDER BY 1 ASC",
        )?;
        let rows = statement.query_map(params![self.catalog.provider_id(), oldest_ms], |row| {
            let timestamp_ms = row.get::<_, i64>(0)?;
            let cost = row.get::<_, f64>(1)?;
            let input = non_negative_i64(row.get::<_, i64>(2)?);
            let cache_read = non_negative_i64(row.get::<_, i64>(3)?);
            let output = non_negative_i64(row.get::<_, i64>(4)?);
            let reasoning = non_negative_i64(row.get::<_, i64>(5)?);
            Ok((timestamp_ms, cost, input, cache_read, output, reasoning))
        })?;

        let mut daily = BTreeMap::<NaiveDate, TokenUsage>::new();
        for row in rows {
            let (timestamp_ms, cost, input, cache_read, output, reasoning) = row?;
            if !cost.is_finite() || cost < 0.0 {
                continue;
            }
            let Some(timestamp) = DateTime::<Utc>::from_timestamp_millis(timestamp_ms) else {
                continue;
            };
            let usage = TokenUsage {
                input_tokens: input,
                cached_input_tokens: cache_read,
                // OpenCode stores visible output and reasoning separately;
                // expose their completion total once in the existing output
                // field while using the stored cost as the spend authority.
                output_tokens: output.saturating_add(reasoning),
                requests: 1,
                estimated_cost_microusd: usd_to_microusd(cost),
                priced_requests: 1,
            };
            daily
                .entry(timestamp.with_timezone(&Local).date_naive())
                .or_default()
                .add_public(&usage);
        }
        let days = daily
            .into_iter()
            .map(|(date, usage)| DailyTokenUsage { date, usage })
            .collect::<Vec<_>>();
        Ok(statistics_from_daily(&days, history_days))
    }
}

impl LimitProvider for OpenCodeClient {
    fn read_limits(&mut self) -> Result<RateLimits> {
        OpenCodeClient::read_limits(self)
    }
}

impl UsageProvider for OpenCodeClient {
    fn load_cached_usage_statistics(&mut self, history_days: u16) -> Result<UsageStatistics> {
        store::with_store(|store| store.load_usage_daily(self.catalog.provider(), history_days))
            .or_else(|_| Ok(UsageStatistics::default()))
    }

    fn refresh_usage_statistics(&mut self, history_days: u16) -> Result<UsageStatistics> {
        match self.read_local_usage(history_days) {
            Ok(statistics) => {
                store::with_store(|store| {
                    store.replace_usage_daily(self.catalog.provider(), &statistics.daily)
                })?;
                Ok(statistics)
            }
            Err(error) => store::with_store(|store| {
                store.load_usage_daily(self.catalog.provider(), history_days)
            })
            .context("refresh OpenCode local usage")
            .or(Err(error)),
        }
    }

    fn refresh_without_limits(&self) -> bool {
        true
    }
}

impl Activator for OpenCodeClient {
    fn activate(&mut self) -> Result<()> {
        bail!("OpenCode does not support automatic session activation")
    }
}

fn catalog(provider: ProviderKind) -> Option<Catalog> {
    match provider {
        ProviderKind::OpenCodeZen => Some(Catalog::Zen),
        ProviderKind::OpenCodeGo => Some(Catalog::Go),
        _ => None,
    }
}

fn resolve_api_key(catalog: Catalog) -> Result<Option<String>> {
    let manual = secrets::load(catalog.secret_name())?;
    let provider_env_names: &[&str] = match catalog {
        Catalog::Zen => &["OPENCODE_ZEN_API_KEY", "ZEN_API_KEY"],
        Catalog::Go => &["OPENCODE_GO_API_KEY"],
    };
    let provider_env = provider_env_names.iter().find_map(|name| env_key(name));
    let common_env = env_key("OPENCODE_API_KEY");
    let auth_key = auth_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|raw| serde_json::from_str::<BTreeMap<String, AuthEntry>>(&raw).ok())
        .and_then(|entries| {
            entries
                .get(catalog.provider_id())
                .and_then(|entry| entry.key.clone())
        });
    Ok(select_api_key(manual, provider_env, common_env, auth_key))
}

fn env_key(name: &str) -> Option<String> {
    env::var_os(name)
        .map(|value| value.to_string_lossy().trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn select_api_key(
    manual: Option<String>,
    provider_env: Option<String>,
    common_env: Option<String>,
    auth_key: Option<String>,
) -> Option<String> {
    [manual, provider_env, common_env, auth_key]
        .into_iter()
        .flatten()
        .map(|value| value.trim().to_owned())
        .find(|value| !value.is_empty())
}

#[derive(Deserialize)]
struct AuthEntry {
    key: Option<String>,
}

fn auth_path() -> Option<PathBuf> {
    data_root().map(|root| root.join("auth.json"))
}

fn database_path() -> Option<PathBuf> {
    data_root().map(|root| root.join("opencode.db"))
}

fn data_root() -> Option<PathBuf> {
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| BaseDirs::new().map(|dirs| dirs.home_dir().join(".local").join("share")))
        .map(|path| path.join("opencode"))
}

fn database_has_provider(catalog: Catalog) -> Result<bool> {
    let Some(path) = database_path() else {
        return Ok(false);
    };
    if !path.is_file() {
        return Ok(false);
    }
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    Ok(connection.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM message
             WHERE json_valid(data)
               AND json_extract(data, '$.providerID') = ?1
               AND json_extract(data, '$.role') = 'assistant'
         )",
        [catalog.provider_id()],
        |row| row.get(0),
    )?)
}

fn parse_model_count(body: &str) -> Result<usize> {
    let value: Value = serde_json::from_str(body).context("parse OpenCode Zen models response")?;
    let models = value
        .as_array()
        .or_else(|| value.get("data").and_then(Value::as_array))
        .or_else(|| value.get("models").and_then(Value::as_array))
        .context("OpenCode Zen models response contains no model array")?;
    Ok(models.len())
}

fn parse_go_usage(body: &str, sampled_at: DateTime<Utc>) -> Result<RateLimits> {
    let value: Value = serde_json::from_str(body).context("parse OpenCode Go usage response")?;
    let usage = usage_object(&value);
    let rolling = parse_window(
        usage,
        &["rolling", "rollingUsage", "rolling_usage"],
        sampled_at,
    )
    .context("OpenCode Go response has no rolling usage window")?;
    let weekly = parse_window(
        usage,
        &["weekly", "weeklyUsage", "weekly_usage"],
        sampled_at,
    );
    let monthly = parse_window(
        usage,
        &["monthly", "monthlyUsage", "monthly_usage"],
        sampled_at,
    );
    let additional_limits = monthly
        .map(|window| AdditionalLimit {
            id: "monthly".into(),
            title: "Monthly".into(),
            window,
        })
        .into_iter()
        .collect();
    Ok(RateLimits {
        primary: rolling,
        secondary: weekly.unwrap_or_default(),
        sampled_at,
        plan_type: Some("Go".into()),
        limit_name: Some("OpenCode Go".into()),
        additional_limits,
        ..RateLimits::default()
    })
}

fn usage_object(value: &Value) -> &Value {
    if value.get("usage").is_some_and(Value::is_object) {
        return value.get("usage").expect("checked above");
    }
    value.get("data").map(usage_object).unwrap_or(value)
}

fn parse_window(parent: &Value, names: &[&str], sampled_at: DateTime<Utc>) -> Option<LimitWindow> {
    let value = names.iter().find_map(|name| parent.get(*name))?;
    let object = value.as_object()?;
    let percent = ["percent", "usagePercent", "usedPercent", "percentUsed"]
        .iter()
        .find_map(|key| object.get(*key).and_then(number))?;
    let resets_at = ["resetsAt", "resetAt", "resets_at", "nextReset"]
        .iter()
        .find_map(|key| object.get(*key).and_then(parse_reset_value));
    let reset_in_seconds = [
        "resetInSec",
        "resetInSeconds",
        "resetSeconds",
        "reset_in_sec",
    ]
    .iter()
    .find_map(|key| object.get(*key).and_then(Value::as_i64));
    let resets_at = resets_at.or_else(|| {
        reset_in_seconds.map(|seconds| sampled_at + chrono::Duration::seconds(seconds.max(0)))
    });
    Some(LimitWindow {
        // The OpenCode Go API reports percentage points. In particular, 1 is
        // one percent, not a fractional 1.0 that should become 100.
        used_percent: Some(percent.round().clamp(0.0, 100.0) as u8),
        resets_at,
        // Do not invent a duration for pace calculations from a reset-only
        // response; the API does not provide the window start.
        duration_minutes: None,
    })
}

fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|value| value as f64))
}

fn parse_reset_value(value: &Value) -> Option<DateTime<Utc>> {
    if let Some(raw) = value.as_str() {
        return DateTime::parse_from_rfc3339(raw)
            .ok()
            .map(|date| date.with_timezone(&Utc));
    }
    let numeric = number(value)?;
    if numeric > 10_000_000_000.0 {
        DateTime::<Utc>::from_timestamp_millis(numeric as i64)
    } else {
        DateTime::<Utc>::from_timestamp(numeric as i64, 0)
    }
}

fn api_error(provider: &str, status: u16, body: &str) -> anyhow::Error {
    let detail = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            ["message", "error", "detail"]
                .iter()
                .find_map(|key| value.get(*key).and_then(Value::as_str).map(str::to_owned))
        })
        .unwrap_or_default();
    if detail.is_empty() {
        anyhow!("{provider} API returned HTTP {status}")
    } else {
        anyhow!("{provider} API returned HTTP {status}: {detail}")
    }
}

fn non_negative_i64(value: i64) -> u64 {
    value.max(0) as u64
}

fn usd_to_microusd(value: f64) -> u64 {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    (value * 1_000_000.0).round().clamp(0.0, u64::MAX as f64) as u64
}

#[cfg(test)]
fn catalog_for_test(provider: ProviderKind) -> Catalog {
    catalog(provider).expect("OpenCode provider")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn go_usage_maps_all_windows_without_rescaling_one_percent() {
        let sampled_at = Utc.with_ymd_and_hms(2026, 8, 29, 10, 0, 0).unwrap();
        let limits = parse_go_usage(
            r#"{
                "usage": {
                    "rolling": {"percent": 1, "resetsAt": "2026-08-29T15:00:00Z"},
                    "weekly": {"percent": 15, "resetInSec": 3600},
                    "monthly": {"usagePercent": 4, "resetInSec": 7200}
                }
            }"#,
            sampled_at,
        )
        .unwrap();
        assert_eq!(limits.primary.used_percent, Some(1));
        assert_eq!(limits.secondary.used_percent, Some(15));
        assert_eq!(limits.additional_limits[0].window.used_percent, Some(4));
        assert_eq!(limits.plan_type.as_deref(), Some("Go"));
        assert!(limits.primary.duration_minutes.is_none());
    }

    #[test]
    fn go_usage_accepts_nested_fields_and_rejects_missing_rolling_window() {
        let limits = parse_go_usage(
            r#"{"data":{"usage":{"rollingUsage":{"usagePercent":0.5,"resetInSec":10}}}}"#,
            Utc::now(),
        )
        .unwrap();
        assert_eq!(limits.primary.used_percent, Some(1));
        assert!(parse_go_usage(r#"{"usage":{"weekly":{"percent":2}}}"#, Utc::now()).is_err());
    }

    #[test]
    fn zen_models_response_accepts_data_envelope() {
        assert_eq!(
            parse_model_count(r#"{"data":[{"id":"a"},{"id":"b"}]}"#).unwrap(),
            2
        );
        assert_eq!(parse_model_count(r#"[{"id":"a"}]"#).unwrap(), 1);
    }

    #[test]
    fn provider_catalogs_remain_distinct() {
        assert_eq!(
            catalog_for_test(ProviderKind::OpenCodeZen).provider_id(),
            "opencode"
        );
        assert_eq!(
            catalog_for_test(ProviderKind::OpenCodeGo).provider_id(),
            "opencode-go"
        );
    }

    #[test]
    fn maps_usd_cost_to_micro_usd_without_negative_values() {
        assert_eq!(usd_to_microusd(1.25), 1_250_000);
        assert_eq!(usd_to_microusd(-1.0), 0);
    }

    #[test]
    fn manual_key_wins_over_provider_env_common_env_and_auth() {
        assert_eq!(
            select_api_key(
                Some(" manual ".into()),
                Some("provider".into()),
                Some("common".into()),
                Some("auth".into()),
            )
            .as_deref(),
            Some("manual")
        );
        assert_eq!(
            select_api_key(
                None,
                Some("provider".into()),
                Some("common".into()),
                Some("auth".into()),
            )
            .as_deref(),
            Some("provider")
        );
        assert_eq!(
            select_api_key(None, None, None, Some("auth".into())).as_deref(),
            Some("auth")
        );
    }

    #[test]
    fn reads_only_matching_assistant_rows_from_open_code_sqlite() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("opencode.db");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE message(
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL,
                    time_created INTEGER NOT NULL,
                    time_updated INTEGER NOT NULL,
                    data TEXT NOT NULL
                );",
            )
            .unwrap();
        let now = Utc::now();
        let today_ms = now.timestamp_millis();
        let yesterday_ms = (now - chrono::Duration::days(1)).timestamp_millis();
        let insert = |id: &str, timestamp: i64, data: &str| {
            connection
                .execute(
                    "INSERT INTO message(id, session_id, time_created, time_updated, data)
                     VALUES(?1, 'session', ?2, ?2, ?3)",
                    params![id, timestamp, data],
                )
                .unwrap();
        };
        insert(
            "zen",
            today_ms,
            &serde_json::json!({
                "role":"assistant",
                "providerID":"opencode",
                "cost":1.25,
                "modelID":"gpt-5.6-sol",
                "time":{"created":today_ms},
                "tokens":{"input":10,"output":4,"reasoning":3,"cache":{"read":2}}
            })
            .to_string(),
        );
        insert(
            "go-free",
            yesterday_ms,
            &serde_json::json!({
                "role":"assistant",
                "providerID":"opencode-go",
                "cost":0,
                "time":{"created":yesterday_ms},
                "tokens":{"input":5,"output":2,"reasoning":1,"cache":{"read":0}}
            })
            .to_string(),
        );
        insert(
            "user",
            today_ms,
            &serde_json::json!({"role":"user","providerID":"opencode","cost":99}).to_string(),
        );
        insert(
            "other-provider",
            today_ms,
            &serde_json::json!({"role":"assistant","providerID":"openrouter","cost":99})
                .to_string(),
        );
        insert("malformed", today_ms, "not-json");
        drop(connection);

        let zen = OpenCodeClient::new(ProviderKind::OpenCodeZen).unwrap();
        let zen_stats = zen.read_local_usage_from_path(&path, 30).unwrap();
        assert_eq!(zen_stats.history.requests, 1);
        assert_eq!(zen_stats.today.estimated_cost_microusd, 1_250_000);
        assert_eq!(zen_stats.today.input_tokens, 10);
        assert_eq!(zen_stats.today.output_tokens, 7);
        assert_eq!(zen_stats.today.cached_input_tokens, 2);

        let go = OpenCodeClient::new(ProviderKind::OpenCodeGo).unwrap();
        let go_stats = go.read_local_usage_from_path(&path, 30).unwrap();
        assert_eq!(go_stats.history.requests, 1);
        assert_eq!(go_stats.history.estimated_cost_microusd, 0);
        assert!(go_stats.history.requests > 0);
    }
}
