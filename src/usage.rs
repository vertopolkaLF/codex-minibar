use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::{self, File},
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Local, NaiveDate, Timelike, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::settings::ProviderKind;
use crate::store;

// Version 5 matches T3/ccusage Codex transcript rules: first session_meta
// wins, fork/subagent copied history is dropped, and unchanged token_count
// re-emits are ignored. Older daily totals must be rebuilt from the logs.
pub(crate) const CODEX_CACHE_VERSION: u8 = 5;
// Version 2 only accepts Claude `assistant` usage lines, matching T3.
pub(crate) const CLAUDE_CACHE_VERSION: u8 = 2;
const CACHE_RETENTION_DAYS: i64 = 365;

/// Locally recorded Codex token usage. This is deliberately derived only from
/// session logs: it never reads credentials or contacts OpenAI directly.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub requests: u64,
    /// Locally measured or model-priced request cost.
    #[serde(default)]
    pub estimated_cost_microusd: u64,
    #[serde(default)]
    pub priced_requests: u64,
    /// Estimated savings from cached prompt tokens versus uncached input rates.
    #[serde(default)]
    pub cache_savings_microusd: u64,
}

impl TokenUsage {
    /// `cached_input_tokens` is a subset of `input_tokens` in Codex session
    /// records, so it must not be counted twice in the displayed total.
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens.saturating_add(self.output_tokens)
    }

    /// API-rate value is an estimate, not the user's subscription bill.
    pub fn estimated_api_value_usd(&self) -> Option<f64> {
        (self.requests > 0 && self.priced_requests > 0)
            .then(|| self.estimated_cost_microusd as f64 / 1_000_000.0)
    }

    pub(crate) fn add(&mut self, other: &Self) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.cached_input_tokens = self
            .cached_input_tokens
            .saturating_add(other.cached_input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.requests = self.requests.saturating_add(other.requests);
        self.estimated_cost_microusd = self
            .estimated_cost_microusd
            .saturating_add(other.estimated_cost_microusd);
        self.priced_requests = self.priced_requests.saturating_add(other.priced_requests);
        self.cache_savings_microusd = self
            .cache_savings_microusd
            .saturating_add(other.cache_savings_microusd);
    }

    /// Aggregates externally sourced usage that follows the same token shape
    /// as local logs (Cursor's dashboard CSV, for example).
    #[allow(dead_code)]
    pub(crate) fn add_public(&mut self, other: &Self) {
        self.add(other);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageStatistics {
    pub today: TokenUsage,
    pub history: TokenUsage,
    pub history_days: u16,
    /// One aggregate per local calendar day, ordered from oldest to newest.
    pub daily: Vec<DailyTokenUsage>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyTokenUsage {
    pub date: NaiveDate,
    pub usage: TokenUsage,
}

impl UsageStatistics {
    pub fn has_data(&self) -> bool {
        self.history.requests > 0
    }

    pub fn tokens_on(&self, date: NaiveDate) -> u64 {
        self.daily
            .iter()
            .find(|entry| entry.date == date)
            .map(|entry| entry.usage.total_tokens())
            .unwrap_or(0)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct UsageCache {
    pub(crate) version: u8,
    /// Legacy totals are safe to show immediately, but need one full re-scan
    /// before new rows can use their logged model and service tier.
    #[serde(default)]
    pub(crate) pricing_rebuild_needed: bool,
    pub(crate) files: BTreeMap<String, CachedSessionFile>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct CachedSessionFile {
    /// Number of complete JSONL bytes already incorporated into `daily`.
    pub(crate) offset: u64,
    pub(crate) daily: Vec<DailyTokenUsage>,
    /// The most recent rollout context lets an incremental scan price the next
    /// token_count without reopening the whole session file.
    #[serde(default)]
    pub(crate) current_model: Option<String>,
    #[serde(default)]
    pub(crate) fast_service_tier: bool,
    /// Per-model daily aggregates for the Usage overview breakdown.
    #[serde(default)]
    pub(crate) model_daily: BTreeMap<String, Vec<DailyTokenUsage>>,
    /// JSON of the last counted `last_token_usage`. Codex re-emits an unchanged
    /// token_count on some stream boundaries; identical consecutive payloads
    /// must not be summed (T3 / ccusage).
    #[serde(default)]
    pub(crate) last_usage_signature: Option<String>,
    #[serde(default)]
    pub(crate) saw_session_meta: bool,
    #[serde(default)]
    pub(crate) suppressing_fork_copies: bool,
    #[serde(default)]
    pub(crate) fork_copy_anchor_ms: i64,
    #[serde(default)]
    pub(crate) session_id: String,
}

impl CachedSessionFile {
    fn reset_scan_state(&mut self) {
        self.offset = 0;
        self.daily.clear();
        self.model_daily.clear();
        self.last_usage_signature = None;
        self.saw_session_meta = false;
        self.suppressing_fork_copies = false;
        self.fork_copy_anchor_ms = 0;
        self.session_id.clear();
    }

    fn add(&mut self, timestamp: DateTime<Utc>, usage: TokenUsage, model: Option<&str>) {
        let date = timestamp.with_timezone(&Local).date_naive();
        if let Some(entry) = self.daily.iter_mut().find(|entry| entry.date == date) {
            entry.usage.add(&usage);
        } else {
            self.daily.push(DailyTokenUsage {
                date,
                usage: usage.clone(),
            });
        }
        if let Some(model) = model.map(str::trim).filter(|name| !name.is_empty()) {
            let model_key = model.to_ascii_lowercase();
            let entries = self.model_daily.entry(model_key).or_default();
            if let Some(entry) = entries.iter_mut().find(|entry| entry.date == date) {
                entry.usage.add(&usage);
            } else {
                entries.push(DailyTokenUsage { date, usage });
            }
        }
    }

    fn prune_before(&mut self, oldest: NaiveDate) {
        self.daily.retain(|entry| entry.date >= oldest);
        self.daily.sort_by_key(|entry| entry.date);
        for entries in self.model_daily.values_mut() {
            entries.retain(|entry| entry.date >= oldest);
            entries.sort_by_key(|entry| entry.date);
        }
    }
}

/// Returns an immediately available snapshot from the persisted local cache.
/// It never opens or scans Codex session logs.
pub fn load_cached_usage_statistics(history_days: u16) -> Result<UsageStatistics> {
    store::with_store(|store| store.load_usage_daily(ProviderKind::Codex, history_days))
}

/// Incorporates only JSONL bytes appended since the previous scan, persists the
/// cache, and returns the refreshed aggregate. Truncated/replaced files are
/// safely rebuilt from their beginning.
pub fn refresh_usage_statistics(history_days: u16) -> Result<UsageStatistics> {
    let codex_root = codex_home();
    let mut cache = store::with_store(|store| store.load_codex_cache())?;
    if cache.pricing_rebuild_needed {
        // The old aggregate has already been published. Start a clean cache
        // now so re-reading the log cannot double-count it.
        cache.files.clear();
        cache.pricing_rebuild_needed = false;
    }
    if cache.version != CODEX_CACHE_VERSION {
        cache.files.clear();
        cache.pricing_rebuild_needed = false;
        cache.version = CODEX_CACHE_VERSION;
    }
    let files = collect_codex_session_files(&codex_root)?;
    let known_paths: BTreeSet<String> = files.iter().map(|(_, key)| key.clone()).collect();
    cache.files.retain(|path, _| known_paths.contains(path));

    let oldest = Local::now().date_naive() - Duration::days(CACHE_RETENTION_DAYS - 1);
    for (path, key) in files {
        let cached = cache.files.entry(key).or_default();
        // Older caches kept daily totals but dropped per-model rows on load.
        // Rescanning from zero rebuilds the breakdown without double-counting.
        if cached.model_daily.is_empty() && !cached.daily.is_empty() {
            cached.reset_scan_state();
        }
        scan_file_delta(&path, cached)?;
        cached.prune_before(oldest);
    }
    cache.version = CODEX_CACHE_VERSION;
    store::with_store(|store| store.save_codex_cache(&cache))?;
    if let Ok(hourly) = collect_codex_hourly_since(Utc::now() - Duration::hours(48)) {
        let _ = store::with_store(|store| store.replace_usage_hourly(ProviderKind::Codex, &hourly));
    }
    Ok(statistics_from_cache(&cache, history_days))
}

/// Walks recently touched session logs and buckets token events by local hour.
/// Daily aggregates stay incremental; this pass exists so Past 24h can use
/// the timestamps that were previously thrown away after daily rollup.
pub(crate) fn collect_codex_hourly_since(
    since: DateTime<Utc>,
) -> Result<Vec<(DateTime<Local>, TokenUsage)>> {
    let files = collect_codex_session_files(&codex_home())?;
    let modified_floor = SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(48 * 60 * 60))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let mut hourly = BTreeMap::<DateTime<Local>, TokenUsage>::new();
    for (path, _) in files {
        let modified = fs::metadata(&path).and_then(|meta| meta.modified()).ok();
        if modified.is_some_and(|time| time < modified_floor) {
            continue;
        }
        let file = File::open(&path).with_context(|| format!("open {}", path.display()))?;
        let reader = BufReader::new(file);
        let mut context = CachedSessionFile::default();
        for line in reader.lines() {
            let Ok(line) = line else {
                continue;
            };
            let Some((timestamp, usage, _)) = ingest_codex_line(&line, &mut context) else {
                continue;
            };
            if timestamp < since {
                continue;
            }
            hourly
                .entry(truncate_local_hour(timestamp.with_timezone(&Local)))
                .or_default()
                .add(&usage);
        }
    }
    Ok(hourly.into_iter().collect())
}

pub(crate) fn truncate_local_hour(timestamp: DateTime<Local>) -> DateTime<Local> {
    timestamp
        .with_minute(0)
        .and_then(|value| value.with_second(0))
        .and_then(|value| value.with_nanosecond(0))
        .unwrap_or(timestamp)
}

fn statistics_from_cache(cache: &UsageCache, history_days: u16) -> UsageStatistics {
    let days: Vec<DailyTokenUsage> = cache
        .files
        .values()
        .flat_map(|file| file.daily.iter().cloned())
        .collect();
    statistics_from_daily(&days, history_days)
}

/// Merges same-day rows and builds today/history totals for the requested window.
pub(crate) fn statistics_from_daily(
    days: &[DailyTokenUsage],
    history_days: u16,
) -> UsageStatistics {
    let history_days = history_days.clamp(1, 365);
    let today = Local::now().date_naive();
    let first_day = today - Duration::days(i64::from(history_days.saturating_sub(1)));
    let mut daily = BTreeMap::<NaiveDate, TokenUsage>::new();
    for entry in days {
        if entry.date >= first_day && entry.date <= today {
            daily.entry(entry.date).or_default().add(&entry.usage);
        }
    }
    let mut stats = UsageStatistics {
        history_days,
        daily: daily
            .into_iter()
            .map(|(date, usage)| DailyTokenUsage { date, usage })
            .collect(),
        ..Default::default()
    };
    for entry in &stats.daily {
        stats.history.add(&entry.usage);
        if entry.date == today {
            stats.today.add(&entry.usage);
        }
    }
    stats
}

fn scan_file_delta(path: &Path, cached: &mut CachedSessionFile) -> Result<()> {
    let file_size = fs::metadata(path)
        .with_context(|| format!("read metadata for {}", path.display()))?
        .len();
    if file_size < cached.offset {
        // Codex rewrote/truncated a session log. Its old aggregate is invalid.
        cached.reset_scan_state();
    }
    if file_size == cached.offset {
        return Ok(());
    }

    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut reader = BufReader::new(file);
    reader
        .seek(SeekFrom::Start(cached.offset))
        .with_context(|| format!("seek {}", path.display()))?;
    let mut offset = cached.offset;
    loop {
        let mut bytes = Vec::new();
        let read = reader
            .read_until(b'\n', &mut bytes)
            .with_context(|| format!("read {}", path.display()))?;
        if read == 0 {
            break;
        }
        // Do not advance over an unfinished line. On the next refresh it will
        // be read again once Codex has appended its newline and completed JSON.
        if bytes.last() != Some(&b'\n') {
            break;
        }
        offset = offset.saturating_add(read as u64);
        let Ok(line) = std::str::from_utf8(&bytes) else {
            continue;
        };
        if let Some((timestamp, usage, model)) = ingest_codex_line(line, cached) {
            cached.add(timestamp, usage, model.as_deref());
        }
    }
    cached.offset = offset;
    Ok(())
}

/// Returns active rollouts plus archived ones. An active path wins when an
/// archive contains the same relative rollout, matching Codex's move/copy
/// behaviour and avoiding duplicate history after archival.
fn collect_codex_session_files(root: &Path) -> Result<Vec<(PathBuf, String)>> {
    let mut active = Vec::new();
    let sessions = root.join("sessions");
    collect_session_files(&sessions, &mut active)?;
    let mut files = Vec::new();
    let mut seen_relative = BTreeSet::new();
    for path in active {
        let key = path
            .strip_prefix(&sessions)
            .expect("active rollout was discovered below sessions")
            .to_string_lossy()
            .into_owned();
        seen_relative.insert(key.clone());
        files.push((path, format!("sessions/{key}")));
    }

    let archived = root.join("archived_sessions");
    let mut archived_files = Vec::new();
    collect_session_files(&archived, &mut archived_files)?;
    for path in archived_files {
        let key = path
            .strip_prefix(&archived)
            .expect("archived rollout was discovered below archived_sessions")
            .to_string_lossy()
            .into_owned();
        if seen_relative.insert(key.clone()) {
            files.push((path, format!("archived_sessions/{key}")));
        }
    }
    Ok(files)
}

fn collect_session_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let Ok(entries) = fs::read_dir(root) else {
        return Ok(());
    };
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            collect_session_files(&path, files)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "jsonl")
        {
            files.push(path);
        }
    }
    Ok(())
}

fn codex_home() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".codex")))
        .unwrap_or_else(|| PathBuf::from(".codex"))
}

/// Cheap substring gate before `JSON.parse`, matching T3's `mightCarryUsage`.
fn might_carry_codex_line(line: &str) -> bool {
    line.contains("\"token_count\"")
        || line.contains("\"turn_context\"")
        || line.contains("\"session_meta\"")
        || line.contains("\"thread_settings_applied\"")
}

/// Copied parent history in a forked/subagent rollout is written in one burst
/// (0-40ms gaps). The child's first real turn lands seconds later. One second
/// is the T3 / ccusage split.
const FORK_COPY_MAX_GAP_MS: i64 = 1000;

fn is_forked_session_meta(payload: &Value) -> bool {
    if payload.get("forked_from_id").and_then(Value::as_str).is_some() {
        return true;
    }
    payload
        .pointer("/source/subagent/thread_spawn/parent_thread_id")
        .and_then(Value::as_str)
        .is_some()
}

fn parse_line_timestamp_ms(event: &Value) -> Option<i64> {
    DateTime::parse_from_rfc3339(event.get("timestamp")?.as_str()?)
        .ok()
        .map(|timestamp| timestamp.timestamp_millis())
}

/// Feeds one Codex rollout line into scan state. Returns a usage event when
/// the line is a real `token_count` that T3/ccusage would keep.
fn ingest_codex_line(
    line: &str,
    cached: &mut CachedSessionFile,
) -> Option<(DateTime<Utc>, TokenUsage, Option<String>)> {
    if !might_carry_codex_line(line) {
        return None;
    }
    let event: Value = serde_json::from_str(line).ok()?;
    let payload = event.get("payload")?;
    let event_type = event.get("type").and_then(Value::as_str);

    if event_type == Some("session_meta") {
        if cached.saw_session_meta {
            return None;
        }
        cached.saw_session_meta = true;
        let id = payload
            .get("id")
            .or_else(|| payload.get("session_id"))
            .and_then(Value::as_str);
        if let Some(id) = id {
            cached.session_id = id.to_owned();
        }
        if let Some(timestamp_ms) = parse_line_timestamp_ms(&event)
            && is_forked_session_meta(payload)
        {
            cached.suppressing_fork_copies = true;
            cached.fork_copy_anchor_ms = timestamp_ms;
        }
        return None;
    }

    if event_type == Some("turn_context") {
        if let Some(model) = payload.get("model").and_then(Value::as_str).map(str::trim)
            && !model.is_empty()
        {
            cached.current_model = Some(model.to_owned());
        }
        return None;
    }

    if event_type == Some("event_msg")
        && payload.get("type").and_then(Value::as_str) == Some("thread_settings_applied")
    {
        let tier = payload
            .pointer("/thread_settings/service_tier")
            .or_else(|| payload.get("service_tier"))
            .and_then(Value::as_str)
            .map(str::trim);
        if let Some(tier) = tier {
            cached.fast_service_tier = matches!(tier, "fast" | "priority");
        }
        return None;
    }

    if payload.get("type").and_then(Value::as_str) != Some("token_count") {
        return None;
    }

    let timestamp = DateTime::parse_from_rfc3339(event.get("timestamp")?.as_str()?)
        .ok()?
        .with_timezone(&Utc);
    let usage = event.pointer("/payload/info/last_token_usage")?;
    let model = usage
        .get("model")
        .or_else(|| usage.get("model_name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(|| cached.current_model.clone());
    // T3 skips token_count before a model is known so a later re-emit after
    // turn_context is not discarded as a duplicate of an unpriced event.
    if model.as_deref().is_none_or(str::is_empty) {
        return None;
    }

    let signature = usage.to_string();
    if cached.last_usage_signature.as_deref() == Some(signature.as_str()) {
        return None;
    }
    cached.last_usage_signature = Some(signature);

    let timestamp_ms = timestamp.timestamp_millis();
    if cached.suppressing_fork_copies {
        if timestamp_ms - cached.fork_copy_anchor_ms < FORK_COPY_MAX_GAP_MS {
            cached.fork_copy_anchor_ms = timestamp_ms;
            return None;
        }
        cached.suppressing_fork_copies = false;
    }

    let token = |name: &str| usage.get(name).and_then(Value::as_u64).unwrap_or(0);
    let input_tokens = token("input_tokens");
    let cached_input_tokens = token("cached_input_tokens").max(token("cache_read_input_tokens"));
    let output_tokens = token("output_tokens");
    if input_tokens == 0 && cached_input_tokens == 0 && output_tokens == 0 {
        return None;
    }
    let estimated_cost_microusd = codex_estimated_cost_microusd(
        model.as_deref(),
        input_tokens,
        cached_input_tokens,
        output_tokens,
        cached.fast_service_tier,
    );
    let cache_savings_microusd =
        codex_cache_savings_microusd(model.as_deref(), cached_input_tokens, cached.fast_service_tier);
    Some((
        timestamp,
        TokenUsage {
            input_tokens,
            cached_input_tokens,
            output_tokens,
            requests: 1,
            estimated_cost_microusd: estimated_cost_microusd.unwrap_or_default(),
            priced_requests: u64::from(estimated_cost_microusd.is_some()),
            cache_savings_microusd,
        },
        model,
    ))
}

#[derive(Clone, Copy)]
struct CodexModelRates {
    input_per_million: f64,
    cached_input_per_million: f64,
    output_per_million: f64,
    fast_multiplier: f64,
}

/// Price a single Codex event using its model and the tier recorded in that
/// rollout. The values are OpenAI API list prices per million tokens, not a
/// charge for a ChatGPT subscription. Older rollout formats sometimes omit a
/// model, so those events use the current Codex base rate as an explicit
/// fallback rather than erasing the whole spend tile.
fn codex_estimated_cost_microusd(
    model: Option<&str>,
    input: u64,
    cached_input: u64,
    output: u64,
    fast_service_tier: bool,
) -> Option<u64> {
    let model = model.unwrap_or("gpt-5.3-codex").trim().to_ascii_lowercase();
    let base = codex_price_model_name(&model);
    let mut rates = match base {
        "gpt-5" | "gpt-5-codex" | "gpt-5.1" | "gpt-5.1-codex" => CodexModelRates {
            input_per_million: 1.25,
            cached_input_per_million: 0.125,
            output_per_million: 10.0,
            fast_multiplier: 2.0,
        },
        "gpt-5.2" | "gpt-5.2-codex" | "gpt-5.3" | "gpt-5.3-codex" => CodexModelRates {
            input_per_million: 1.75,
            cached_input_per_million: 0.175,
            output_per_million: 14.0,
            fast_multiplier: 2.0,
        },
        "gpt-5.4" => CodexModelRates {
            input_per_million: 2.5,
            cached_input_per_million: 0.25,
            output_per_million: 15.0,
            fast_multiplier: 2.0,
        },
        "gpt-5.4-pro" => CodexModelRates {
            input_per_million: 30.0,
            cached_input_per_million: 30.0,
            output_per_million: 180.0,
            fast_multiplier: 2.0,
        },
        "gpt-5.5" => CodexModelRates {
            input_per_million: 5.0,
            cached_input_per_million: 0.5,
            output_per_million: 30.0,
            fast_multiplier: 2.5,
        },
        "gpt-5.5-pro" => CodexModelRates {
            input_per_million: 30.0,
            cached_input_per_million: 30.0,
            output_per_million: 180.0,
            fast_multiplier: 2.5,
        },
        // Codex can publish a model slug before public pricing catalogs have
        // caught up. Keep the local spend estimate useful and conservative.
        _ => CodexModelRates {
            input_per_million: 1.75,
            cached_input_per_million: 0.175,
            output_per_million: 14.0,
            fast_multiplier: 2.0,
        },
    };

    // OpenAI's published long-context tiers apply to the whole request.
    if input > 272_000 {
        match base {
            "gpt-5.4" => {
                rates.input_per_million = 5.0;
                rates.cached_input_per_million = 0.5;
                rates.output_per_million = 22.5;
            }
            "gpt-5.4-pro" | "gpt-5.5-pro" => {
                rates.input_per_million = 60.0;
                rates.cached_input_per_million = 60.0;
                rates.output_per_million = 270.0;
            }
            "gpt-5.5" => {
                rates.input_per_million = 10.0;
                rates.cached_input_per_million = 1.0;
                rates.output_per_million = 45.0;
            }
            _ => {}
        }
    }
    let multiplier = if fast_service_tier {
        rates.fast_multiplier
    } else {
        1.0
    };
    let uncached_input = input.saturating_sub(cached_input);
    let cost = (uncached_input as f64 * rates.input_per_million
        + cached_input.min(input) as f64 * rates.cached_input_per_million
        + output as f64 * rates.output_per_million)
        * multiplier
        / 1_000_000.0;
    Some((cost * 1_000_000.0).round().clamp(0.0, u64::MAX as f64) as u64)
}

fn codex_cache_savings_microusd(
    model: Option<&str>,
    cached_input: u64,
    fast_service_tier: bool,
) -> u64 {
    if cached_input == 0 {
        return 0;
    }
    let model = model.unwrap_or("gpt-5.3-codex").trim().to_ascii_lowercase();
    let base = codex_price_model_name(&model);
    let rates = match base {
        "gpt-5" | "gpt-5-codex" | "gpt-5.1" | "gpt-5.1-codex" => (1.25, 0.125, 2.0),
        "gpt-5.2" | "gpt-5.2-codex" | "gpt-5.3" | "gpt-5.3-codex" => (1.75, 0.175, 2.0),
        "gpt-5.4" => (2.5, 0.25, 2.0),
        "gpt-5.4-pro" => (30.0, 30.0, 2.0),
        "gpt-5.5" => (5.0, 0.5, 2.5),
        "gpt-5.5-pro" => (30.0, 30.0, 2.5),
        _ => (1.75, 0.175, 2.0),
    };
    let multiplier = if fast_service_tier { rates.2 } else { 1.0 };
    let savings = cached_input as f64 * (rates.0 - rates.1) * multiplier / 1_000_000.0;
    (savings * 1_000_000.0).round().clamp(0.0, u64::MAX as f64) as u64
}

/// Dates in rollout names are model revisions, not new rates. Retain the
/// meaningful `-codex` and `-pro` suffixes while stripping only a final date.
fn codex_price_model_name(model: &str) -> &str {
    model
        .strip_suffix("-2025-08-07")
        .or_else(|| model.strip_suffix("-2025-11-13"))
        .or_else(|| model.strip_suffix("-2025-12-11"))
        .or_else(|| model.strip_suffix("-2026-03-05"))
        .or_else(|| model.strip_suffix("-2026-04-23"))
        .unwrap_or(model)
}

/// Cached representation of one Claude Code response. Keeping individual
/// messages (rather than just daily totals) lets us suppress the same
/// sidechain/replayed message when it appears in more than one session log.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CachedClaudeUsageEntry {
    pub(crate) timestamp: DateTime<Utc>,
    pub(crate) message_id: Option<String>,
    pub(crate) request_id: Option<String>,
    pub(crate) is_sidechain: bool,
    pub(crate) has_speed: bool,
    pub(crate) usage: TokenUsage,
    #[serde(default)]
    pub(crate) model: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ClaudeUsageCache {
    pub(crate) version: u8,
    pub(crate) files: BTreeMap<String, CachedClaudeSessionFile>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct CachedClaudeSessionFile {
    /// Number of complete JSONL bytes incorporated into `entries`.
    pub(crate) offset: u64,
    pub(crate) entries: Vec<CachedClaudeUsageEntry>,
}

/// Returns Claude Code usage from the on-disk cache without opening a log.
pub fn load_cached_claude_usage_statistics(history_days: u16) -> Result<UsageStatistics> {
    store::with_store(|store| store.load_usage_daily(ProviderKind::Claude, history_days))
}

/// Scans Claude Code's `projects/**/*.jsonl` logs incrementally. The cache is
/// separate from Codex's and stores a byte offset per file, so reopening the
/// popup never causes a full re-read of an ever-growing Claude history.
pub fn refresh_claude_usage_statistics(history_days: u16) -> Result<UsageStatistics> {
    let mut cache = store::with_store(|store| store.load_claude_cache())?;
    let files = collect_claude_session_files();
    let known_paths: BTreeSet<String> = files
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    cache.files.retain(|path, _| known_paths.contains(path));

    let oldest = Local::now().date_naive() - Duration::days(CACHE_RETENTION_DAYS - 1);
    for path in files {
        let key = path.to_string_lossy().into_owned();
        let cached = cache.files.entry(key).or_default();
        scan_claude_file_delta(&path, cached)?;
        cached
            .entries
            .retain(|entry| entry.timestamp.with_timezone(&Local).date_naive() >= oldest);
    }
    cache.version = CLAUDE_CACHE_VERSION;
    let stats = statistics_from_claude_cache(&cache, history_days);
    store::with_store(|store| {
        store.save_claude_cache(&cache)?;
        store.replace_usage_daily(ProviderKind::Claude, &stats.daily)?;
        store.replace_usage_model_daily(
            ProviderKind::Claude,
            &aggregate_claude_model_daily(&cache)
                .into_iter()
                .map(|(date, model, usage)| (model, date, usage))
                .collect::<Vec<_>>(),
        )
    })?;
    Ok(stats)
}

pub(crate) fn statistics_from_claude_cache(
    cache: &ClaudeUsageCache,
    history_days: u16,
) -> UsageStatistics {
    let days: Vec<DailyTokenUsage> = deduplicate_claude_entries(cache)
        .into_iter()
        .map(|entry| DailyTokenUsage {
            date: entry.timestamp.with_timezone(&Local).date_naive(),
            usage: entry.usage,
        })
        .collect();
    statistics_from_daily(&days, history_days)
}

pub(crate) fn aggregate_claude_model_daily(
    cache: &ClaudeUsageCache,
) -> Vec<(NaiveDate, String, TokenUsage)> {
    let mut merged = BTreeMap::<(String, NaiveDate), TokenUsage>::new();
    for entry in deduplicate_claude_entries(cache) {
        let model = entry
            .model
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let date = entry.timestamp.with_timezone(&Local).date_naive();
        merged.entry((model, date)).or_default().add(&entry.usage);
    }
    merged
        .into_iter()
        .map(|((model, date), usage)| (date, model, usage))
        .collect()
}

/// Mirrors Claude Code/OpenUsage's duplicate preference: the original message
/// beats a sidechain replay; otherwise retain the richer/larger record.
pub(crate) fn deduplicate_claude_entries(cache: &ClaudeUsageCache) -> Vec<CachedClaudeUsageEntry> {
    let mut entries: Vec<CachedClaudeUsageEntry> = Vec::new();
    let mut exact = HashMap::<(String, Option<String>), usize>::new();
    let mut by_message = HashMap::<String, Vec<usize>>::new();

    for entry in cache.files.values().flat_map(|file| &file.entries) {
        let Some(message_id) = &entry.message_id else {
            entries.push(entry.clone());
            continue;
        };
        let key = (message_id.clone(), entry.request_id.clone());
        let collision = exact.get(&key).copied().or_else(|| {
            by_message.get(message_id).and_then(|indices| {
                indices
                    .iter()
                    .copied()
                    .find(|&index| entry.is_sidechain || entries[index].is_sidechain)
            })
        });
        if let Some(index) = collision {
            // Exact message+request repeats: T3 keeps the first. Sidechain
            // collisions across request ids still prefer the original message.
            if exact.contains_key(&key) {
                continue;
            }
            if claude_entry_should_replace(entry, &entries[index]) {
                let previous = &entries[index];
                if let Some(previous_id) = &previous.message_id {
                    exact.remove(&(previous_id.clone(), previous.request_id.clone()));
                }
                entries[index] = entry.clone();
                exact.insert(key, index);
            }
            continue;
        }

        let index = entries.len();
        entries.push(entry.clone());
        exact.insert(key, index);
        by_message
            .entry(message_id.clone())
            .or_default()
            .push(index);
    }
    entries
}

fn claude_entry_should_replace(
    candidate: &CachedClaudeUsageEntry,
    existing: &CachedClaudeUsageEntry,
) -> bool {
    if candidate.is_sidechain != existing.is_sidechain {
        return existing.is_sidechain;
    }
    let candidate_total = candidate.usage.total_tokens();
    let existing_total = existing.usage.total_tokens();
    candidate_total > existing_total
        || (candidate_total == existing_total && candidate.has_speed && !existing.has_speed)
}

fn scan_claude_file_delta(path: &Path, cached: &mut CachedClaudeSessionFile) -> Result<()> {
    let file_size = fs::metadata(path)
        .with_context(|| format!("read metadata for {}", path.display()))?
        .len();
    if file_size < cached.offset {
        cached.offset = 0;
        cached.entries.clear();
    }
    if file_size == cached.offset {
        return Ok(());
    }

    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut reader = BufReader::new(file);
    reader
        .seek(SeekFrom::Start(cached.offset))
        .with_context(|| format!("seek {}", path.display()))?;
    let mut offset = cached.offset;
    loop {
        let mut bytes = Vec::new();
        let read = reader
            .read_until(b'\n', &mut bytes)
            .with_context(|| format!("read {}", path.display()))?;
        if read == 0 || bytes.last() != Some(&b'\n') {
            break;
        }
        offset = offset.saturating_add(read as u64);
        if !std::str::from_utf8(&bytes).is_ok_and(|line| line.contains("\"usage\"")) {
            continue;
        }
        if let Some(entry) = claude_usage_from_line(&bytes) {
            cached.entries.push(entry);
        }
    }
    cached.offset = offset;
    Ok(())
}

fn claude_usage_from_line(line: &[u8]) -> Option<CachedClaudeUsageEntry> {
    let event: Value = serde_json::from_slice(line).ok()?;
    if event.get("type").and_then(Value::as_str) != Some("assistant") {
        return None;
    }
    let timestamp = DateTime::parse_from_rfc3339(event.get("timestamp")?.as_str()?)
        .ok()?
        .with_timezone(&Utc);
    let message = event.get("message")?;
    let usage_json = message.get("usage")?;
    let input_tokens = usage_json.get("input_tokens")?.as_u64()?;
    let output_tokens = usage_json.get("output_tokens")?.as_u64()?;
    let cache_read = usage_json
        .get("cache_read_input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_creation = usage_json
        .get("cache_creation_input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_creation_details = usage_json.get("cache_creation");
    let cache_write_5m = cache_creation_details
        .and_then(|value| value.get("ephemeral_5m_input_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(cache_creation);
    let cache_write_1h = cache_creation_details
        .and_then(|value| value.get("ephemeral_1h_input_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let model = message
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())?;
    let cost = event
        .get("costUSD")
        .and_then(Value::as_f64)
        .filter(|cost| cost.is_finite() && *cost >= 0.0)
        .or_else(|| {
            claude_estimated_cost_usd(
                Some(model),
                input_tokens,
                cache_read,
                output_tokens,
                cache_write_5m,
                cache_write_1h,
            )
        })?;
    let cache_savings_microusd = claude_cache_savings_microusd(Some(model), cache_read);
    let usage = TokenUsage {
        input_tokens,
        cached_input_tokens: cache_read.min(input_tokens),
        output_tokens,
        requests: 1,
        estimated_cost_microusd: (cost * 1_000_000.0).round().clamp(0.0, u64::MAX as f64) as u64,
        priced_requests: 1,
        cache_savings_microusd,
    };
    Some(CachedClaudeUsageEntry {
        timestamp,
        message_id: message.get("id").and_then(Value::as_str).map(str::to_owned),
        request_id: event
            .get("requestId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        is_sidechain: event
            .get("isSidechain")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        has_speed: usage_json.get("speed").is_some(),
        usage,
        model: Some(model.to_owned()),
    })
}

/// Claude Code normally writes `costUSD`; the local estimate only fills gaps
/// in older logs. Rates are the public standard-tier list prices per million
/// tokens, including Claude's 5m/1h cache-write multipliers.
fn claude_estimated_cost_usd(
    model: Option<&str>,
    input: u64,
    cache_read: u64,
    output: u64,
    cache_write_5m: u64,
    cache_write_1h: u64,
) -> Option<f64> {
    let model = model?.to_ascii_lowercase();
    let (input_rate, output_rate) = if model.contains("opus") {
        (15.0, 75.0)
    } else if model.contains("sonnet") {
        (3.0, 15.0)
    } else if model.contains("haiku") {
        (1.0, 5.0)
    } else {
        return None;
    };
    let uncached_input = input.saturating_sub(cache_read);
    Some(
        (uncached_input as f64 * input_rate
            + cache_read as f64 * input_rate * 0.1
            + output as f64 * output_rate
            + cache_write_5m as f64 * input_rate * 1.25
            + cache_write_1h as f64 * input_rate * 2.0)
            / 1_000_000.0,
    )
}

fn claude_cache_savings_microusd(model: Option<&str>, cache_read: u64) -> u64 {
    if cache_read == 0 {
        return 0;
    }
    let Some(model) = model else {
        return 0;
    };
    let model = model.to_ascii_lowercase();
    let input_rate = if model.contains("opus") {
        15.0
    } else if model.contains("sonnet") {
        3.0
    } else if model.contains("haiku") {
        1.0
    } else {
        return 0;
    };
    let savings = cache_read as f64 * input_rate * 0.9 / 1_000_000.0;
    (savings * 1_000_000.0).round().clamp(0.0, u64::MAX as f64) as u64
}

fn collect_claude_session_files() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(config_dirs) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        // Claude Code accepts comma-separated roots; Windows also commonly
        // receives a normal PATH-style list from launchers, so tolerate both.
        let raw = config_dirs.to_string_lossy();
        let configured_paths: Vec<PathBuf> = if raw.contains(',') {
            raw.split(',')
                .map(|part| PathBuf::from(part.trim()))
                .collect()
        } else {
            std::env::split_paths(&config_dirs).collect()
        };
        for path in configured_paths
            .into_iter()
            .filter(|path| !path.as_os_str().is_empty())
        {
            roots.push(if path.file_name().is_some_and(|name| name == "projects") {
                path.parent().map(Path::to_path_buf).unwrap_or(path)
            } else {
                path
            });
        }
    } else if let Some(base) = directories::BaseDirs::new() {
        roots.push(base.home_dir().join(".config").join("claude"));
        roots.push(base.home_dir().join(".claude"));
    }

    let mut seen = BTreeSet::new();
    let mut files = Vec::new();
    for root in roots {
        let projects = root.join("projects");
        if seen.insert(projects.clone()) {
            let _ = collect_session_files(&projects, &mut files);
        }
    }
    files.sort();
    files.dedup();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_per_request_token_usage() {
        let line = r#"{"timestamp":"2026-07-14T10:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":12,"cached_input_tokens":7,"output_tokens":4}}}}"#;
        let mut cached = CachedSessionFile {
            current_model: Some("gpt-5.4".into()),
            ..Default::default()
        };
        let (_, usage, _) = ingest_codex_line(line, &mut cached).unwrap();
        assert_eq!(usage.total_tokens(), 16);
        assert_eq!(usage.requests, 1);
        assert_eq!(usage.priced_requests, 1);
    }

    #[test]
    fn prices_codex_by_logged_model_and_service_tier() {
        // Stay under the 272k long-context threshold so this exercises the
        // base rates; the long-context tier has its own test.
        let standard =
            codex_estimated_cost_microusd(Some("gpt-5.4"), 200_000, 100_000, 100_000, false)
                .unwrap();
        assert_eq!(standard, 1_775_000);
        let fast = codex_estimated_cost_microusd(Some("gpt-5.4"), 200_000, 100_000, 100_000, true)
            .unwrap();
        assert_eq!(fast, 3_550_000);
        assert!(codex_estimated_cost_microusd(Some("unknown-model"), 1, 0, 1, false).is_some());
    }

    #[test]
    fn applies_codex_long_context_rate_to_the_whole_request() {
        let cost =
            codex_estimated_cost_microusd(Some("gpt-5.4"), 300_000, 0, 100_000, false).unwrap();
        assert_eq!(cost, 3_750_000);
    }

    #[test]
    fn ignores_non_usage_events() {
        assert!(
            ingest_codex_line(
                r#"{"type":"event_msg","payload":{"type":"task_started"}}"#,
                &mut CachedSessionFile::default(),
            )
            .is_none()
        );
    }

    #[test]
    fn reads_claude_usage_and_uses_its_recorded_cost() {
        let line = r#"{"type":"assistant","timestamp":"2026-07-14T10:00:00Z","requestId":"request-1","message":{"id":"message-1","model":"claude-sonnet-4-20250514","usage":{"input_tokens":100,"cache_read_input_tokens":40,"output_tokens":25,"speed":"standard"}},"costUSD":0.0125}"#;
        let entry = claude_usage_from_line(line.as_bytes()).unwrap();
        assert_eq!(entry.usage.total_tokens(), 125);
        assert_eq!(entry.usage.cached_input_tokens, 40);
        assert_eq!(entry.usage.estimated_api_value_usd(), Some(0.0125));
        assert!(entry.has_speed);
    }

    #[test]
    fn ignores_claude_events_that_are_not_assistant_or_have_no_model() {
        assert!(claude_usage_from_line(
            br#"{"type":"result","timestamp":"2026-07-14T10:00:00Z","message":{"id":"message-1","model":"claude-sonnet-4-20250514","usage":{"input_tokens":100,"output_tokens":25}},"costUSD":0.01}"#
        )
        .is_none());
        assert!(claude_usage_from_line(
            br#"{"type":"assistant","timestamp":"2026-07-14T10:00:00Z","message":{"id":"message-1","usage":{"input_tokens":100,"output_tokens":25}},"costUSD":0.01}"#
        )
        .is_none());
    }

    #[test]
    fn claude_exact_message_request_keeps_the_first() {
        let first = CachedClaudeUsageEntry {
            timestamp: Utc::now(),
            message_id: Some("message-1".into()),
            request_id: Some("request-1".into()),
            is_sidechain: false,
            has_speed: false,
            usage: TokenUsage {
                input_tokens: 10,
                output_tokens: 2,
                requests: 1,
                estimated_cost_microusd: 100,
                priced_requests: 1,
                ..Default::default()
            },
            model: Some("claude-sonnet-4-20250514".into()),
        };
        let repeat = CachedClaudeUsageEntry {
            usage: TokenUsage {
                input_tokens: 999,
                output_tokens: 999,
                requests: 1,
                estimated_cost_microusd: 9_999,
                priced_requests: 1,
                ..Default::default()
            },
            ..first.clone()
        };
        let cache = ClaudeUsageCache {
            version: CLAUDE_CACHE_VERSION,
            files: BTreeMap::from([(
                "a.jsonl".into(),
                CachedClaudeSessionFile {
                    offset: 0,
                    entries: vec![first.clone(), repeat],
                },
            )]),
        };
        let kept = deduplicate_claude_entries(&cache);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].usage.input_tokens, 10);
    }

    #[test]
    fn claude_sidechain_replay_does_not_double_count() {
        let original = CachedClaudeUsageEntry {
            timestamp: Utc::now(),
            message_id: Some("message-1".into()),
            request_id: Some("request-parent".into()),
            is_sidechain: false,
            has_speed: true,
            usage: TokenUsage {
                input_tokens: 100,
                output_tokens: 20,
                requests: 1,
                estimated_cost_microusd: 1_000,
                priced_requests: 1,
                ..Default::default()
            },
            model: Some("claude-sonnet-4-20250514".into()),
        };
        let replay = CachedClaudeUsageEntry {
            request_id: Some("request-sidechain".into()),
            is_sidechain: true,
            ..original.clone()
        };
        let cache = ClaudeUsageCache {
            version: CLAUDE_CACHE_VERSION,
            files: BTreeMap::from([
                (
                    "a.jsonl".into(),
                    CachedClaudeSessionFile {
                        offset: 0,
                        entries: vec![original],
                    },
                ),
                (
                    "b.jsonl".into(),
                    CachedClaudeSessionFile {
                        offset: 0,
                        entries: vec![replay],
                    },
                ),
            ]),
        };
        assert_eq!(deduplicate_claude_entries(&cache).len(), 1);
    }

    #[test]
    fn token_usage_saturates() {
        let usage = TokenUsage {
            input_tokens: u64::MAX,
            cached_input_tokens: 1,
            output_tokens: 1,
            requests: 0,
            ..Default::default()
        };
        assert_eq!(usage.total_tokens(), u64::MAX);
    }

    #[test]
    fn incremental_scan_counts_only_new_complete_lines() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.jsonl");
        let context = r#"{"type":"turn_context","payload":{"model":"gpt-5.4"}}"#;
        let first = r#"{"timestamp":"2026-07-14T10:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":10}}}}"#;
        let second = r#"{"timestamp":"2026-07-14T11:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"output_tokens":5}}}}"#;
        fs::write(&path, format!("{context}\n{first}\n{second}")).unwrap();

        let mut cached = CachedSessionFile::default();
        scan_file_delta(&path, &mut cached).unwrap();
        assert_eq!(cached.daily[0].usage.total_tokens(), 10);

        fs::write(&path, format!("{context}\n{first}\n{second}\n")).unwrap();
        scan_file_delta(&path, &mut cached).unwrap();
        assert_eq!(cached.daily[0].usage.total_tokens(), 15);
        assert_eq!(cached.daily[0].usage.requests, 2);
    }

    fn token_count(input: u64, output: u64, timestamp: &str) -> String {
        format!(
            r#"{{"timestamp":"{timestamp}","type":"event_msg","payload":{{"type":"token_count","info":{{"last_token_usage":{{"input_tokens":{input},"output_tokens":{output}}}}}}}}}"#
        )
    }

    fn session_meta(id: &str, timestamp: &str, forked_from: Option<&str>) -> String {
        let fork = match forked_from {
            Some(parent) => format!(r#","forked_from_id":"{parent}""#),
            None => String::new(),
        };
        format!(
            r#"{{"type":"session_meta","timestamp":"{timestamp}","payload":{{"type":"session_meta","id":"{id}"{fork}}}}}"#
        )
    }

    #[test]
    fn skips_token_count_before_model_is_known() {
        let mut cached = CachedSessionFile::default();
        assert!(ingest_codex_line(&token_count(10, 1, "2026-08-01T05:00:00.000Z"), &mut cached).is_none());
    }

    #[test]
    fn skips_unchanged_consecutive_token_counts() {
        let mut cached = CachedSessionFile {
            current_model: Some("gpt-5.4".into()),
            ..Default::default()
        };
        assert!(ingest_codex_line(&token_count(10, 1, "2026-08-01T05:00:00.000Z"), &mut cached).is_some());
        assert!(ingest_codex_line(&token_count(10, 1, "2026-08-01T05:00:00.100Z"), &mut cached).is_none());
    }

    #[test]
    fn forked_rollout_drops_copied_burst_and_keeps_first_real_event() {
        let mut cached = CachedSessionFile::default();
        ingest_codex_line(
            &session_meta("child", "2026-08-01T05:00:00.000Z", Some("parent")),
            &mut cached,
        );
        ingest_codex_line(
            r#"{"type":"turn_context","payload":{"model":"gpt-5.4"}}"#,
            &mut cached,
        );
        assert!(ingest_codex_line(
            &token_count(100, 10, "2026-08-01T05:00:00.001Z"),
            &mut cached,
        )
        .is_none());
        let real = ingest_codex_line(
            &token_count(300, 30, "2026-08-01T05:00:06.000Z"),
            &mut cached,
        );
        assert_eq!(real.unwrap().1.output_tokens, 30);
        assert_eq!(cached.session_id, "child");
    }

    #[test]
    fn later_session_meta_does_not_steal_child_id() {
        let mut cached = CachedSessionFile::default();
        ingest_codex_line(
            &session_meta("child", "2026-08-01T05:00:00.000Z", None),
            &mut cached,
        );
        ingest_codex_line(
            &session_meta("parent", "2026-08-01T05:00:00.000Z", None),
            &mut cached,
        );
        assert_eq!(cached.session_id, "child");
    }
}
