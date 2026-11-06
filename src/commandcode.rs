//! Command Code rolling and monthly credit limits from the official
//! CLI login.
//!
//! `cmd login` stores a `user_...` API key in `~/.commandcode/auth.json`. The
//! CLI's own `/alpha/billing/*` routes accept it as a bearer token: `credits`
//! reports the 5-hour and weekly USD windows plus the remaining monthly grant,
//! and `subscriptions` names the plan that sizes that grant.
//!
//! Usage statistics come only from the CLI's local session logs in
//! `~/.commandcode/projects`, where each model response records its tokens
//! and the credit cost the CLI charged for it.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime},
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use directories::BaseDirs;
use serde_json::Value;

use crate::{
    instances::ProviderId,
    limits::{AdditionalLimit, Credits, LimitWindow, RateLimits, UsageAmount},
    provider_cli, store,
    usage::{DailyTokenUsage, TokenUsage, UsageStatistics, statistics_from_daily},
    worker::{Activator, LimitProvider, UsageProvider},
};

const API_BASE: &str = "https://api.commandcode.ai";
const CREDITS_PATH: &str = "/alpha/billing/credits";
const SUBSCRIPTIONS_PATH: &str = "/alpha/billing/subscriptions";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const FIVE_HOUR_MINUTES: u32 = 5 * 60;
const WEEKLY_MINUTES: u32 = 7 * 24 * 60;
const MONTHLY_MINUTES: u32 = 30 * 24 * 60;
/// `cmd` itself is deliberately absent: on Windows it would resolve to
/// `cmd.exe`. The npm package installs these aliases next to it.
#[cfg(windows)]
const CLI_NAMES: &[&str] = &[
    "cmdc.cmd",
    "cmdc.ps1",
    "cmdc.exe",
    "command-code.cmd",
    "command-code.ps1",
    "commandcode.cmd",
    "commandcode.ps1",
];
#[cfg(not(windows))]
const CLI_NAMES: &[&str] = &["command-code", "cmd", "cmdc", "commandcode"];
pub(crate) const MONTHLY_LIMIT_ID: &str = "monthly";
const LOCAL_HISTORY_DAYS: i64 = 365;
const HOURLY_HISTORY_HOURS: i64 = 48;

/// Monthly USD allowance per subscription plan, from
/// https://commandcode.ai/pricing. The credits response reports only the
/// remaining balance unless it includes `monthlyCreditsGranted`.
const PLANS: &[(&str, &str, f64)] = &[
    ("individual-go", "Go", 10.0),
    ("individual-go-v1", "Go", 10.0),
    ("individual-pro", "Pro", 30.0),
    ("individual-pro-v1", "Pro", 80.0),
    ("individual-goat", "GOAT", 70.0),
    ("individual-max", "Max", 150.0),
    ("individual-ultra", "Ultra", 300.0),
];

pub struct CommandCodeClient {
    agent: ureq::Agent,
    provider: ProviderId,
    /// Parsed session logs keyed by path, reused while a file is unchanged.
    sessions: HashMap<PathBuf, SessionLog>,
}

pub struct CommandCodeActivator;

impl CommandCodeClient {
    pub fn new(provider: ProviderId) -> Self {
        Self {
            agent: ureq::AgentBuilder::new().timeout(REQUEST_TIMEOUT).build(),
            provider,
            sessions: HashMap::new(),
        }
    }

    fn get(&self, path: &str, api_key: &str) -> Result<String> {
        let response = match self
            .agent
            .get(&format!("{API_BASE}{path}"))
            .set("Authorization", &format!("Bearer {api_key}"))
            .set("Accept", "application/json")
            .set("User-Agent", "Codex Minibar")
            .call()
        {
            Ok(response) => response,
            Err(ureq::Error::Status(401 | 403, _)) => {
                bail!("Command Code API key was rejected; run `command-code login`")
            }
            Err(ureq::Error::Status(429, _)) => {
                return Err(crate::worker::rate_limit_error(
                    "Command Code billing request was rate limited (HTTP 429).",
                ));
            }
            Err(ureq::Error::Status(status, _)) => {
                bail!("Command Code billing request failed with HTTP {status}")
            }
            Err(error) => return Err(error).context("request Command Code billing"),
        };
        response
            .into_string()
            .context("read Command Code billing response")
    }
}

impl LimitProvider for CommandCodeClient {
    fn read_limits(&mut self) -> Result<RateLimits> {
        let auth = load_auth()?;
        // The plan only sizes the monthly row, so its lookup runs alongside
        // the credits request and a failure never hides the rolling windows.
        let (credits, subscription) = thread::scope(|scope| {
            let subscription = scope.spawn(|| {
                self.get(SUBSCRIPTIONS_PATH, &auth.api_key)
                    .and_then(|body| parse_subscription(&body))
            });
            let credits = self
                .get(CREDITS_PATH, &auth.api_key)
                .and_then(|body| parse_credits(&body));
            let subscription = subscription
                .join()
                .unwrap_or_else(|_| Err(anyhow::anyhow!("Command Code plan lookup panicked")));
            (credits, subscription)
        });
        let credits = credits?;
        let subscription = subscription.unwrap_or_else(|error| {
            crate::logger::info(format!("Command Code plan lookup failed: {error:#}"));
            SubscriptionLookup::Unavailable
        });
        Ok(rate_limits(
            credits,
            subscription,
            auth.user_name,
            Utc::now(),
        ))
    }
}

impl UsageProvider for CommandCodeClient {
    fn load_cached_usage_statistics(&mut self, history_days: u16) -> Result<UsageStatistics> {
        store::with_store(|store| store.load_usage_daily(self.provider, history_days))
    }

    fn refresh_usage_statistics(&mut self, history_days: u16) -> Result<UsageStatistics> {
        // A retiring worker's scan can still be running when its replacement
        // starts; serialize them so their store writes cannot interleave.
        static SCAN: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _scan = SCAN
            .lock()
            .map_err(|_| anyhow::anyhow!("Command Code scan lock poisoned"))?;
        let Some(root) = projects_root() else {
            bail!("Command Code home directory is unavailable")
        };
        refresh_sessions(&mut self.sessions, &session_files(&root));
        let rollup = rollup(&self.sessions, Utc::now());
        store::with_store(|store| {
            store.replace_usage_daily(self.provider, &rollup.daily)?;
            store.replace_usage_hourly(self.provider, &rollup.hourly)?;
            store.replace_usage_model_daily(self.provider, &rollup.models)?;
            store.replace_usage_project_daily(self.provider, &rollup.projects)
        })?;
        Ok(statistics_from_daily(&rollup.daily, history_days))
    }
}

impl Activator for CommandCodeActivator {
    fn activate(&mut self) -> Result<()> {
        bail!("Command Code does not support session-window activation")
    }
}

pub fn is_installed(explicit: Option<&Path>) -> bool {
    signed_in() || cli_available(explicit).is_some()
}

/// Resolves a Command Code CLI launcher. An explicit folder is searched first;
/// a file path remains supported so a copied launcher still counts.
pub fn cli_available(explicit: Option<&Path>) -> Option<PathBuf> {
    provider_cli::candidates_from(explicit, npm_launchers(), CLI_NAMES)
        .into_iter()
        .next()
}

#[cfg(windows)]
fn npm_launchers() -> Vec<PathBuf> {
    let Some(appdata) = std::env::var_os("APPDATA") else {
        return Vec::new();
    };
    let npm = PathBuf::from(appdata).join("npm");
    CLI_NAMES.iter().map(|name| npm.join(name)).collect()
}

#[cfg(not(windows))]
fn npm_launchers() -> Vec<PathBuf> {
    Vec::new() // Unix npm launchers are discovered on PATH.
}

fn auth_path() -> Option<PathBuf> {
    BaseDirs::new().map(|dirs| dirs.home_dir().join(".commandcode/auth.json"))
}

fn signed_in() -> bool {
    auth_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .is_some_and(|raw| parse_auth(&raw).is_ok())
}

struct CommandCodeAuth {
    api_key: String,
    user_name: Option<String>,
}

fn load_auth() -> Result<CommandCodeAuth> {
    let path = auth_path().context("Command Code home directory is unavailable")?;
    let raw = fs::read_to_string(&path).with_context(|| {
        format!(
            "Command Code auth not found at {}; run `command-code login`",
            path.display()
        )
    })?;
    parse_auth(&raw)
}

fn parse_auth(raw: &str) -> Result<CommandCodeAuth> {
    let root: Value = serde_json::from_str(raw).context("parse Command Code auth.json")?;
    let api_key = root
        .get("apiKey")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .context("Command Code auth.json has no API key; run `command-code login`")?
        .to_owned();
    Ok(CommandCodeAuth {
        api_key,
        user_name: root
            .get("userName")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned),
    })
}

#[derive(Debug, PartialEq)]
struct RollingWindow {
    used: f64,
    cap: f64,
    resets_at: Option<DateTime<Utc>>,
}

#[derive(Debug, PartialEq)]
struct CreditsPayload {
    monthly_remaining: f64,
    monthly_granted: Option<f64>,
    purchased: f64,
    five_hour: Option<RollingWindow>,
    weekly: Option<RollingWindow>,
}

#[derive(Debug, PartialEq)]
struct Subscription {
    plan_id: String,
    period_end: Option<DateTime<Utc>>,
}

#[derive(Debug, PartialEq)]
enum SubscriptionLookup {
    /// A successful response with `data: null`: no paid subscription.
    Free,
    Paid(Subscription),
    /// The lookup failed; the monthly row may still be sized from the grant.
    Unavailable,
}

fn parse_credits(body: &str) -> Result<CreditsPayload> {
    let root: Value = serde_json::from_str(body).context("parse Command Code credits response")?;
    let credits = root
        .get("credits")
        .filter(|credits| credits.is_object())
        .context("Command Code credits response has no credits object")?;
    let monthly_remaining = number(credits.get("monthlyCredits"))
        .context("Command Code credits response has no monthly credits")?;
    let windows = root
        .get("windowLimits")
        .or_else(|| credits.get("windowLimits"));
    Ok(CreditsPayload {
        monthly_remaining,
        monthly_granted: number(credits.get("monthlyCreditsGranted")).filter(|value| *value > 0.0),
        purchased: number(credits.get("purchasedCredits")).unwrap_or(0.0),
        five_hour: windows.and_then(|windows| rolling_window(windows.get("fiveHour"))),
        weekly: windows.and_then(|windows| rolling_window(windows.get("weekly"))),
    })
}

fn rolling_window(value: Option<&Value>) -> Option<RollingWindow> {
    let value = value?;
    let cap = number(value.get("cap")).filter(|cap| *cap > 0.0)?;
    Some(RollingWindow {
        used: number(value.get("used")).unwrap_or(0.0).max(0.0),
        cap,
        resets_at: timestamp(value.get("resetAt")),
    })
}

fn parse_subscription(body: &str) -> Result<SubscriptionLookup> {
    let root: Value =
        serde_json::from_str(body).context("parse Command Code subscription response")?;
    // Only an explicit successful null identifies the free tier; failure
    // envelopes are transient.
    if root.get("success").and_then(Value::as_bool) != Some(true) {
        bail!("Command Code subscription response was unsuccessful")
    }
    let data = match root.get("data") {
        Some(Value::Null) => return Ok(SubscriptionLookup::Free),
        Some(data) if data.is_object() => data,
        _ => bail!("Command Code subscription response has no data"),
    };
    let plan_id = data
        .get("planId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|plan| !plan.is_empty())
        .context("Command Code subscription has no plan")?
        .to_owned();
    Ok(SubscriptionLookup::Paid(Subscription {
        plan_id,
        period_end: timestamp(data.get("currentPeriodEnd")),
    }))
}

fn plan(plan_id: &str) -> Option<(&'static str, f64)> {
    PLANS
        .iter()
        .find(|(id, _, _)| id.eq_ignore_ascii_case(plan_id))
        .map(|(_, name, credits)| (*name, *credits))
}

fn rate_limits(
    credits: CreditsPayload,
    subscription: SubscriptionLookup,
    account_name: Option<String>,
    sampled_at: DateTime<Utc>,
) -> RateLimits {
    let (plan_type, plan_credits, period_end) = match &subscription {
        SubscriptionLookup::Paid(subscription) => match plan(&subscription.plan_id) {
            Some((name, credits)) => (
                Some(name.to_owned()),
                Some(credits),
                subscription.period_end,
            ),
            // Unknown plans keep their raw id and only an API-reported grant.
            None => (
                Some(subscription.plan_id.clone()),
                None,
                subscription.period_end,
            ),
        },
        SubscriptionLookup::Free | SubscriptionLookup::Unavailable => (None, None, None),
    };
    let monthly_total = credits.monthly_granted.or(plan_credits);
    let additional_limits = monthly_total
        .map(|total| {
            let used = (total - credits.monthly_remaining).clamp(0.0, total);
            AdditionalLimit {
                id: MONTHLY_LIMIT_ID.into(),
                title: "Monthly".into(),
                window: LimitWindow {
                    used_percent: Some(percent(used, total)),
                    resets_at: period_end,
                    duration_minutes: Some(MONTHLY_MINUTES),
                },
            }
        })
        .into_iter()
        .collect();
    let window = |window: Option<&RollingWindow>, minutes| {
        window.map_or_else(LimitWindow::default, |window| LimitWindow {
            used_percent: Some(percent(window.used, window.cap)),
            resets_at: window.resets_at,
            duration_minutes: Some(minutes),
        })
    };
    let amount = |window: Option<&RollingWindow>| {
        window.map(|window| UsageAmount {
            used: window.used,
            limit: window.cap,
        })
    };
    let purchased = (credits.purchased > 0.0).then(|| format!("${:.2}", credits.purchased));
    RateLimits {
        primary: window(credits.five_hour.as_ref(), FIVE_HOUR_MINUTES),
        secondary: window(credits.weekly.as_ref(), WEEKLY_MINUTES),
        primary_usage_amount: amount(credits.five_hour.as_ref()),
        secondary_usage_amount: amount(credits.weekly.as_ref()),
        sampled_at,
        account_name,
        plan_type,
        limit_name: Some("Command Code".into()),
        credits: Credits {
            has_credits: purchased.is_some(),
            unlimited: false,
            balance: purchased,
        },
        additional_limits,
        ..RateLimits::default()
    }
}

fn percent(used: f64, limit: f64) -> u8 {
    (used / limit * 100.0).clamp(0.0, 100.0).round() as u8
}

fn number(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
    .filter(|value: &f64| value.is_finite())
}

/// Epoch seconds, epoch milliseconds, or RFC 3339. The API reports `0` for a
/// window that has not started yet, which has no reset time.
fn timestamp(value: Option<&Value>) -> Option<DateTime<Utc>> {
    if let Some(Value::String(text)) = value
        && let Ok(parsed) = DateTime::parse_from_rfc3339(text.trim())
    {
        return Some(parsed.with_timezone(&Utc));
    }
    let raw = number(value).filter(|value| *value > 0.0)?;
    let millis = if raw > 10_000_000_000.0 {
        raw
    } else {
        raw * 1000.0
    };
    Utc.timestamp_millis_opt(millis as i64).single()
}

// ----- local usage -----------------------------------------------------------

fn projects_root() -> Option<PathBuf> {
    BaseDirs::new().map(|dirs| dirs.home_dir().join(".commandcode/projects"))
}

/// Session transcripts, one folder per project. Checkpoint journals share the
/// `.jsonl` extension but record file snapshots, not model responses.
fn session_files(root: &Path) -> Vec<PathBuf> {
    let Ok(projects) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut files = projects
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter_map(|project| fs::read_dir(project).ok())
        .flat_map(|entries| entries.flatten().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.ends_with(".jsonl") && !name.ends_with(".checkpoints.jsonl")
                })
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

#[derive(Debug, Default, PartialEq)]
struct SessionLog {
    len: u64,
    modified: Option<SystemTime>,
    /// Working directory from the transcript's `session` header, if any.
    cwd: Option<String>,
    records: Vec<UsageRecord>,
}

#[derive(Clone, Debug, PartialEq)]
struct UsageRecord {
    id: Option<String>,
    timestamp: DateTime<Utc>,
    model: String,
    usage: TokenUsage,
}

/// Re-reads only logs whose size or modification time changed, and forgets
/// logs the CLI has deleted.
fn refresh_sessions(sessions: &mut HashMap<PathBuf, SessionLog>, files: &[PathBuf]) {
    let known = files.iter().collect::<HashSet<_>>();
    sessions.retain(|path, _| known.contains(path));
    for path in files {
        let Ok(metadata) = fs::metadata(path) else {
            sessions.remove(path);
            continue;
        };
        let modified = metadata.modified().ok();
        if sessions
            .get(path)
            .is_some_and(|log| log.len == metadata.len() && log.modified == modified)
        {
            continue;
        }
        match read_session(path) {
            Ok((cwd, records)) => {
                sessions.insert(
                    path.clone(),
                    SessionLog {
                        len: metadata.len(),
                        modified,
                        cwd,
                        records,
                    },
                );
            }
            Err(error) => {
                crate::logger::info(format!(
                    "Command Code session log {} was skipped: {error:#}",
                    path.display()
                ));
            }
        }
    }
}

fn read_session(path: &Path) -> Result<(Option<String>, Vec<UsageRecord>)> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut cwd = None;
    let mut records = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line.with_context(|| format!("read {}", path.display()))?;
        // Tool outputs dominate transcripts; skip them before parsing JSON.
        let header = cwd.is_none() && line.contains("\"cwd\"");
        if !header && !line.contains("\"usage\"") {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if header
            && value.get("type").and_then(Value::as_str) == Some("session")
            && let Some(dir) = value.get("cwd").and_then(Value::as_str)
            && !dir.trim().is_empty()
        {
            cwd = Some(dir.to_owned());
        }
        if let Some(record) = usage_record(&value) {
            records.push(record);
        }
    }
    Ok((cwd, records))
}

/// One model response. `inputTokens` already includes cache reads, matching
/// the convention that `cached_input_tokens` is a subset of `input_tokens`.
fn usage_record(value: &Value) -> Option<UsageRecord> {
    let usage = value.get("usage").filter(|usage| usage.is_object())?;
    let timestamp = value
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(|text| DateTime::parse_from_rfc3339(text).ok())?
        .with_timezone(&Utc);
    let tokens = |key| {
        number(usage.get(key))
            .filter(|value| *value > 0.0)
            .map_or(0, |value| value.round() as u64)
    };
    let input = tokens("inputTokens");
    let output = tokens("outputTokens");
    let cost = number(usage.get("costUsd")).filter(|cost| *cost >= 0.0);
    if input == 0 && output == 0 && cost.is_none_or(|cost| cost == 0.0) {
        return None;
    }
    Some(UsageRecord {
        id: value
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .map(str::to_owned),
        timestamp,
        model: value
            .get("model")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|model| !model.is_empty())
            .unwrap_or("unknown")
            .to_owned(),
        usage: TokenUsage {
            input_tokens: input,
            cached_input_tokens: tokens("cacheReadTokens").min(input),
            output_tokens: output,
            requests: 1,
            estimated_cost_microusd: cost.map_or(0, |cost| (cost * 1_000_000.0).round() as u64),
            priced_requests: u64::from(cost.is_some()),
            ..TokenUsage::default()
        },
    })
}

#[derive(Debug, Default)]
struct UsageRollup {
    daily: Vec<DailyTokenUsage>,
    hourly: Vec<(DateTime<Local>, TokenUsage)>,
    models: Vec<(String, NaiveDate, TokenUsage)>,
    projects: Vec<(String, NaiveDate, TokenUsage)>,
}

fn rollup(sessions: &HashMap<PathBuf, SessionLog>, now: DateTime<Utc>) -> UsageRollup {
    let oldest = now - chrono::Duration::days(LOCAL_HISTORY_DAYS);
    let hourly_since = now - chrono::Duration::hours(HOURLY_HISTORY_HOURS);
    // A log without its own `session` header shares its siblings' project.
    let folder = |path: &Path| {
        path.parent()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let mut paths = sessions.keys().collect::<Vec<_>>();
    paths.sort();
    let mut folder_cwd = HashMap::<String, &str>::new();
    for path in &paths {
        if let Some(cwd) = sessions[*path].cwd.as_deref() {
            folder_cwd.entry(folder(path)).or_insert(cwd);
        }
    }
    let mut seen = HashSet::new();
    let mut daily = BTreeMap::<NaiveDate, TokenUsage>::new();
    let mut hourly = BTreeMap::<DateTime<Local>, TokenUsage>::new();
    let mut models = BTreeMap::<(String, NaiveDate), TokenUsage>::new();
    let mut projects = BTreeMap::<(String, NaiveDate), TokenUsage>::new();
    for path in paths {
        let log = &sessions[path];
        let project = log
            .cwd
            .clone()
            .or_else(|| folder_cwd.get(&folder(path)).map(|cwd| (*cwd).to_owned()))
            .unwrap_or_else(|| folder(path));
        for record in &log.records {
            if record.timestamp < oldest {
                continue;
            }
            // A resumed or forked session can copy earlier responses.
            if let Some(id) = &record.id
                && !seen.insert((id.as_str(), record.timestamp))
            {
                continue;
            }
            let local = record.timestamp.with_timezone(&Local);
            let date = local.date_naive();
            daily.entry(date).or_default().add(&record.usage);
            models
                .entry((record.model.clone(), date))
                .or_default()
                .add(&record.usage);
            projects
                .entry((project.clone(), date))
                .or_default()
                .add(&record.usage);
            if record.timestamp >= hourly_since {
                hourly
                    .entry(crate::usage::truncate_local_hour(local))
                    .or_default()
                    .add(&record.usage);
            }
        }
    }
    let keyed = |rows: BTreeMap<(String, NaiveDate), TokenUsage>| {
        rows.into_iter()
            .map(|((key, date), usage)| (key, date, usage))
            .collect()
    };
    UsageRollup {
        daily: daily
            .into_iter()
            .map(|(date, usage)| DailyTokenUsage { date, usage })
            .collect(),
        hourly: hourly.into_iter().collect(),
        models: keyed(models),
        projects: keyed(projects),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CREDITS: &str = r#"{
      "credits":{"monthlyCredits":7.5,"purchasedCredits":2,"freeCredits":0},
      "windowLimits":{
        "fiveHour":{"used":1.5,"cap":3,"resetAt":1792141200000},
        "weekly":{"used":0,"cap":6,"resetAt":0}
      }
    }"#;

    #[test]
    fn reads_the_api_key_and_user_name() {
        let auth = parse_auth(r#"{"apiKey":" user_secret ","userName":"octocat","keyName":"cli"}"#)
            .unwrap();
        assert_eq!(auth.api_key, "user_secret");
        assert_eq!(auth.user_name.as_deref(), Some("octocat"));
        assert!(parse_auth(r#"{"apiKey":""}"#).is_err());
    }

    #[test]
    fn maps_rolling_windows_and_sizes_monthly_from_the_plan() {
        let credits = parse_credits(CREDITS).unwrap();
        let subscription = parse_subscription(
            r#"{"success":true,"data":{"planId":"individual-go-v1","status":"active","currentPeriodEnd":"2026-11-16T16:09:56.000Z"}}"#,
        )
        .unwrap();
        let limits = rate_limits(credits, subscription, None, Utc::now());

        assert_eq!(limits.primary.used_percent, Some(50));
        assert_eq!(limits.primary.duration_minutes, Some(FIVE_HOUR_MINUTES));
        assert!(limits.primary.resets_at.is_some());
        assert_eq!(
            limits.primary_usage_amount,
            Some(UsageAmount {
                used: 1.5,
                limit: 3.0
            })
        );
        // An untouched window reports `resetAt: 0`, which is not a reset time.
        assert_eq!(limits.secondary.used_percent, Some(0));
        assert_eq!(limits.secondary.resets_at, None);
        assert_eq!(limits.plan_type.as_deref(), Some("Go"));

        let monthly = &limits.additional_limits[0];
        assert_eq!(monthly.id, MONTHLY_LIMIT_ID);
        assert_eq!(monthly.window.used_percent, Some(25));
        assert_eq!(
            monthly.window.resets_at,
            timestamp(Some(&Value::from("2026-11-16T16:09:56.000Z")))
        );
        assert_eq!(limits.credits.balance.as_deref(), Some("$2.00"));
    }

    #[test]
    fn reported_grant_wins_over_the_catalog_and_survives_a_failed_lookup() {
        let credits =
            parse_credits(r#"{"credits":{"monthlyCredits":"15","monthlyCreditsGranted":20}}"#)
                .unwrap();
        let limits = rate_limits(credits, SubscriptionLookup::Unavailable, None, Utc::now());
        assert_eq!(limits.additional_limits[0].window.used_percent, Some(25));
        assert_eq!(limits.additional_limits[0].window.resets_at, None);
        assert!(limits.primary.is_empty());
        assert!(!limits.credits.has_credits);
    }

    #[test]
    fn unknown_or_free_plans_have_no_monthly_percentage() {
        let free = parse_subscription(r#"{"success":true,"data":null}"#).unwrap();
        assert_eq!(free, SubscriptionLookup::Free);
        let limits = rate_limits(parse_credits(CREDITS).unwrap(), free, None, Utc::now());
        assert!(limits.additional_limits.is_empty());
        // "Free" would switch the popup to its monthly-only layout.
        assert_eq!(limits.plan_type, None);

        let unknown = parse_subscription(
            r#"{"success":true,"data":{"planId":"team-enterprise","status":"active"}}"#,
        )
        .unwrap();
        let limits = rate_limits(parse_credits(CREDITS).unwrap(), unknown, None, Utc::now());
        assert!(limits.additional_limits.is_empty());
        assert_eq!(limits.plan_type.as_deref(), Some("team-enterprise"));
    }

    #[test]
    fn failure_envelopes_are_not_the_free_tier() {
        assert!(parse_subscription(r#"{"success":false,"data":null}"#).is_err());
        assert!(parse_subscription(r#"{"data":null}"#).is_err());
        assert!(parse_credits(r#"{"credits":{}}"#).is_err());
    }

    #[test]
    fn parses_second_millisecond_and_iso_timestamps() {
        let expected = Utc.timestamp_opt(1_792_141_200, 0).single();
        assert_eq!(timestamp(Some(&Value::from(1_792_141_200_u64))), expected);
        assert_eq!(
            timestamp(Some(&Value::from(1_792_141_200_000_u64))),
            expected
        );
        assert_eq!(timestamp(Some(&Value::from(0))), None);
        assert!(timestamp(Some(&Value::from("2026-11-16T16:09:56Z"))).is_some());
    }

    #[cfg(not(windows))]
    #[test]
    fn resolves_unix_command_code_names_from_an_explicit_folder() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        for name in ["command-code", "cmd"] {
            let launcher = directory.path().join(name);
            fs::write(&launcher, b"fixture").unwrap();
            fs::set_permissions(&launcher, fs::Permissions::from_mode(0o755)).unwrap();
            assert_eq!(
                cli_available(Some(directory.path())),
                Some(launcher.canonicalize().unwrap())
            );
            fs::remove_file(launcher).unwrap();
        }
        assert!(!CLI_NAMES.contains(&"cmd.exe"));
    }

    #[cfg(windows)]
    #[test]
    fn never_resolves_the_windows_cmd_shell() {
        assert!(!CLI_NAMES.iter().any(|name| name.starts_with("cmd.")));
        let directory = tempfile::tempdir().unwrap();
        let launcher = directory.path().join("cmdc.cmd");
        fs::write(&launcher, b"fixture").unwrap();
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert_eq!(
            cli_available(Some(directory.path())),
            Some(fs::canonicalize(launcher).unwrap())
        );
    }

    fn transcript(lines: &[&str]) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("c-dev-app");
        fs::create_dir(&project).unwrap();
        fs::write(project.join("a.jsonl"), lines.join("\n")).unwrap();
        // Checkpoint journals are not transcripts and must never be counted.
        fs::write(
            project.join("a.checkpoints.jsonl"),
            r#"{"usage":{"inputTokens":999,"costUsd":9},"timestamp":"2026-10-16T16:00:00Z"}"#,
        )
        .unwrap();
        root
    }

    const SESSION: &str = r#"{"type":"session","version":3,"id":"s","timestamp":"2026-10-16T16:00:00Z","cwd":"C:\\Dev\\app"}"#;
    const RESPONSE: &str = r#"{"type":"message","id":"4e685697","timestamp":"2026-10-16T16:49:52.698Z","message":{"role":"assistant","content":[]},"model":"deepseek/deepseek-v4.1-flash-fast","usage":{"inputTokens":21923,"outputTokens":341,"cacheReadTokens":6144,"cacheWriteTokens":0,"costUsd":0.002820724}}"#;

    #[test]
    fn reads_model_responses_with_their_cost_and_project() {
        let root = transcript(&[
            SESSION,
            r#"{"role":"tool","content":[{"type":"tool-result","output":{"value":"no usage here"}}]}"#,
            RESPONSE,
        ]);
        let files = session_files(root.path());
        assert_eq!(files.len(), 1);
        let mut sessions = HashMap::new();
        refresh_sessions(&mut sessions, &files);
        let log = &sessions[&files[0]];
        assert_eq!(log.cwd.as_deref(), Some(r"C:\Dev\app"));
        assert_eq!(log.records.len(), 1);
        let record = &log.records[0];
        assert_eq!(record.model, "deepseek/deepseek-v4.1-flash-fast");
        assert_eq!(record.usage.input_tokens, 21_923);
        assert_eq!(record.usage.cached_input_tokens, 6_144);
        assert_eq!(record.usage.output_tokens, 341);
        assert_eq!(record.usage.estimated_cost_microusd, 2_821);
        assert_eq!(record.usage.priced_requests, 1);

        let now = DateTime::parse_from_rfc3339("2026-10-16T18:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let rollup = rollup(&sessions, now);
        assert_eq!(rollup.daily.len(), 1);
        assert_eq!(rollup.daily[0].usage.requests, 1);
        assert_eq!(rollup.hourly.len(), 1);
        assert_eq!(rollup.models[0].0, "deepseek/deepseek-v4.1-flash-fast");
        assert_eq!(rollup.projects[0].0, r"C:\Dev\app");
    }

    #[test]
    fn copied_responses_count_once_and_headerless_logs_borrow_the_project() {
        let root = transcript(&[SESSION, RESPONSE]);
        // A resumed session without its own header repeats the response.
        fs::write(root.path().join("c-dev-app/b.jsonl"), RESPONSE).unwrap();
        let other = root.path().join("c-dev-other");
        fs::create_dir(&other).unwrap();
        fs::write(
            other.join("c.jsonl"),
            RESPONSE.replace("4e685697", "77777777"),
        )
        .unwrap();

        let mut sessions = HashMap::new();
        refresh_sessions(&mut sessions, &session_files(root.path()));
        let now = DateTime::parse_from_rfc3339("2026-10-16T18:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let rollup = rollup(&sessions, now);
        assert_eq!(rollup.daily[0].usage.requests, 2);
        let projects = rollup
            .projects
            .iter()
            .map(|(project, _, usage)| (project.as_str(), usage.requests))
            .collect::<Vec<_>>();
        assert_eq!(projects, [(r"C:\Dev\app", 1), ("c-dev-other", 1)]);
    }

    #[test]
    fn unchanged_logs_are_not_reread_and_deleted_logs_are_forgotten() {
        let root = transcript(&[SESSION, RESPONSE]);
        let files = session_files(root.path());
        let mut sessions = HashMap::new();
        refresh_sessions(&mut sessions, &files);
        // A cached entry that disagrees with the file proves it was reused.
        sessions.get_mut(&files[0]).unwrap().records.clear();
        refresh_sessions(&mut sessions, &files);
        assert!(sessions[&files[0]].records.is_empty());

        refresh_sessions(&mut sessions, &[]);
        assert!(sessions.is_empty());
    }

    #[test]
    fn ignores_lines_without_usable_usage() {
        let value = |raw: &str| serde_json::from_str::<Value>(raw).unwrap();
        assert!(usage_record(&value(r#"{"usage":{"inputTokens":5}}"#)).is_none());
        assert!(
            usage_record(&value(
                r#"{"timestamp":"2026-10-16T16:00:00Z","usage":{"inputTokens":0,"outputTokens":0}}"#
            ))
            .is_none()
        );
        let unpriced = usage_record(&value(
            r#"{"timestamp":"2026-10-16T16:00:00Z","usage":{"inputTokens":5,"cacheReadTokens":9}}"#,
        ))
        .unwrap();
        assert_eq!(unpriced.model, "unknown");
        assert_eq!(unpriced.usage.cached_input_tokens, 5);
        assert_eq!(unpriced.usage.priced_requests, 0);
    }

    #[test]
    fn transcript_null_metadata_and_partial_cache_usage_keep_real_tokens() {
        let record = usage_record(&serde_json::json!({
            "id": null, "model": null, "type": "future-response",
            "timestamp": "2026-11-01T01:30:00-04:00",
            "usage": {"inputTokens": 10, "outputTokens": null,
                      "cacheReadTokens": 99, "costUsd": null, "futureTokens": 500}
        }))
        .unwrap();
        assert_eq!(
            record.timestamp,
            "2026-11-01T05:30:00Z".parse::<DateTime<Utc>>().unwrap()
        );
        assert_eq!(record.model, "unknown");
        assert_eq!(record.id, None);
        assert_eq!(record.usage.input_tokens, 10);
        assert_eq!(record.usage.cached_input_tokens, 10);
        assert_eq!(record.usage.output_tokens, 0);
        assert_eq!(record.usage.priced_requests, 0);
        assert!(
            usage_record(&serde_json::json!({
                "timestamp": "invalid", "usage": {"inputTokens": 10}
            }))
            .is_none()
        );
    }
}
