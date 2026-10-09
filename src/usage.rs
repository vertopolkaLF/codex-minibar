use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::{self, File},
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Local, NaiveDate, Timelike, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{instances::ProviderId, pricing, settings::ProviderKind, store};

// Version 9 upgrades attributed event identity to include the record offset.
// Version 8 rebuilt event source links after the account-source migration.
// Version 7 matched T3/ccusage Codex transcript rules and the current pricing
// table: first session_meta
// wins, fork/subagent copied history is dropped, and unchanged token_count
// re-emits are ignored. Older daily totals must be rebuilt from the logs.
pub(crate) const CODEX_CACHE_VERSION: u8 = 9;
// Version 4 only accepts Claude `assistant` usage lines, matching T3 and the
// current model pricing table.
pub(crate) const CLAUDE_CACHE_VERSION: u8 = 4;
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
    /// Independently cached account histories for providers with multiple accounts.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub accounts: BTreeMap<String, UsageStatistics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Account scope for memoization and stale-response rejection.
    #[serde(default)]
    pub account_id: Option<String>,
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
pub fn load_cached_usage_statistics(
    provider: ProviderId,
    history_days: u16,
) -> Result<UsageStatistics> {
    store::with_store(|store| store.load_usage_daily(provider, history_days))
}

/// Incorporates only JSONL bytes appended since the previous scan, persists the
/// cache, and returns the refreshed aggregate. Truncated/replaced files are
/// safely rebuilt from their beginning.
pub fn refresh_usage_statistics(
    provider: ProviderId,
    home: Option<&Path>,
    history_days: u16,
) -> Result<UsageStatistics> {
    // Overview repair and the background worker may both request a scan.
    static SCAN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _scan = SCAN
        .lock()
        .map_err(|_| anyhow::anyhow!("Codex scan lock poisoned"))?;
    if !provider.is_primary() {
        return refresh_instance_usage_statistics(provider, home, history_days);
    }
    let before = store::codex_accounts::identity();
    let attribution = store::with_store(|store| store.initialize_codex_attribution())?;
    let codex_root = home.map_or_else(codex_home, Path::to_path_buf);
    let mut cache = store::with_store(|store| store.load_codex_cache(provider))?;
    if !attribution.ready {
        cache.files.clear();
    }
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
    let mut account_events = Vec::new();
    let mut rebuilt_sources = Vec::new();
    for (path, key) in files {
        let cached = cache.files.entry(key.clone()).or_default();
        // Older caches kept daily totals but dropped per-model rows on load.
        // Rescanning from zero rebuilds the breakdown without double-counting.
        if cached.model_daily.is_empty() && !cached.daily.is_empty() {
            cached.reset_scan_state();
        }
        let delta = scan_file_delta(&path, &key, cached)?;
        if delta.rebuilt {
            rebuilt_sources.push(key);
        }
        account_events.extend(
            delta
                .events
                .into_iter()
                .filter(|event| event.timestamp.with_timezone(&Local).date_naive() >= oldest),
        );
        cached.prune_before(oldest);
    }
    cache.version = CODEX_CACHE_VERSION;
    let end = Utc::now();
    let after = store::codex_accounts::identity();
    // Commit attribution before offsets: a failed subsequent cache save can
    // replay these events safely, but can never skip an uncommitted event.
    store::with_store(|store| {
        store.save_account_scan(
            &account_events,
            &rebuilt_sources,
            &attribution,
            &before,
            &after,
            end,
        )
    })?;
    store::with_store(|store| store.save_codex_cache(provider, &cache))?;
    load_cached_usage_statistics(provider, history_days)
}

/// Additional Codex instances read their own `CODEX_HOME`, which only ever
/// holds that instance's account, so no cross-account attribution is needed.
fn refresh_instance_usage_statistics(
    provider: ProviderId,
    home: Option<&Path>,
    history_days: u16,
) -> Result<UsageStatistics> {
    let Some(home) = home else {
        return load_cached_usage_statistics(provider, history_days);
    };
    let mut cache = store::with_store(|store| store.load_codex_cache(provider))?;
    if cache.pricing_rebuild_needed || cache.version != CODEX_CACHE_VERSION {
        cache.files.clear();
        cache.pricing_rebuild_needed = false;
        cache.version = CODEX_CACHE_VERSION;
    }
    let files = collect_codex_session_files(home)?;
    let known_paths: BTreeSet<String> = files.iter().map(|(_, key)| key.clone()).collect();
    cache.files.retain(|path, _| known_paths.contains(path));
    let oldest = Local::now().date_naive() - Duration::days(CACHE_RETENTION_DAYS - 1);
    for (path, key) in files {
        let cached = cache.files.entry(key.clone()).or_default();
        if cached.model_daily.is_empty() && !cached.daily.is_empty() {
            cached.reset_scan_state();
        }
        scan_file_delta(&path, &key, cached)?;
        cached.prune_before(oldest);
    }
    store::with_store(|store| store.save_codex_cache(provider, &cache))?;
    load_cached_usage_statistics(provider, history_days)
}

pub(crate) fn truncate_local_hour(timestamp: DateTime<Local>) -> DateTime<Local> {
    // Preserve the UTC offset during repeated local hours at the DST transition.
    timestamp
        - Duration::minutes(i64::from(timestamp.minute()))
        - Duration::seconds(i64::from(timestamp.second()))
        - Duration::nanoseconds(i64::from(timestamp.nanosecond()))
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

pub(crate) struct FileDelta {
    pub events: Vec<store::codex_accounts::AccountEvent>,
    pub rebuilt: bool,
}

pub(crate) fn scan_file_delta(
    path: &Path,
    source: &str,
    cached: &mut CachedSessionFile,
) -> Result<FileDelta> {
    let mut events = Vec::new();
    let file_size = fs::metadata(path)
        .with_context(|| format!("read metadata for {}", path.display()))?
        .len();
    let rebuilt = cached.offset == 0 || file_size < cached.offset;
    if file_size < cached.offset {
        // Codex rewrote/truncated a session log. Its old aggregate is invalid.
        cached.reset_scan_state();
    }
    if file_size == cached.offset {
        return Ok(FileDelta { events, rebuilt });
    }

    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut reader = BufReader::new(file);
    reader
        .seek(SeekFrom::Start(cached.offset))
        .with_context(|| format!("seek {}", path.display()))?;
    let mut offset = cached.offset;
    let mut bytes = Vec::new();
    loop {
        bytes.clear();
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
            events.push(store::codex_accounts::AccountEvent {
                source: source.to_owned(),
                offset,
                session: if cached.session_id.is_empty() {
                    source.to_owned()
                } else {
                    cached.session_id.clone()
                },
                timestamp,
                signature: cached.last_usage_signature.clone().unwrap_or_default(),
                model: model.clone().unwrap_or_default(),
                usage: usage.clone(),
            });
            cached.add(timestamp, usage, model.as_deref());
        }
    }
    cached.offset = offset;
    Ok(FileDelta { events, rebuilt })
}

/// Returns active rollouts plus archived ones. An active path wins when an
/// archive contains the same relative rollout, matching Codex's move/copy
/// behaviour and avoiding duplicate history after archival.
pub(crate) fn collect_codex_session_files(root: &Path) -> Result<Vec<(PathBuf, String)>> {
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

pub(crate) fn codex_home() -> PathBuf {
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
    if payload
        .get("forked_from_id")
        .and_then(Value::as_str)
        .is_some()
    {
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
    let cache_creation_tokens = token("cache_write_input_tokens");
    let output_tokens = token("output_tokens");
    if input_tokens == 0 && cached_input_tokens == 0 && output_tokens == 0 {
        return None;
    }
    let uncached_input_tokens = input_tokens
        .saturating_sub(cached_input_tokens)
        .saturating_sub(cache_creation_tokens);
    let estimated_cost_microusd = pricing::request_cost_microusd(
        ProviderKind::Codex,
        model.as_deref(),
        cache_creation_tokens,
        uncached_input_tokens,
        cached_input_tokens,
        output_tokens,
    );
    let cache_savings_microusd =
        pricing::cache_savings_microusd(ProviderKind::Codex, model.as_deref(), cached_input_tokens);
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
pub fn load_cached_claude_usage_statistics(
    provider: ProviderId,
    history_days: u16,
) -> Result<UsageStatistics> {
    store::with_store(|store| store.load_usage_daily(provider, history_days))
}

/// Scans Claude Code's `projects/**/*.jsonl` logs incrementally. The cache is
/// separate from Codex's and stores a byte offset per file, so reopening the
/// popup never causes a full re-read of an ever-growing Claude history.
pub fn refresh_claude_usage_statistics(
    provider: ProviderId,
    config_folder: Option<&Path>,
    history_days: u16,
) -> Result<UsageStatistics> {
    let roots = claude_projects_roots(config_folder);
    let files = collect_claude_session_files(&roots);
    // Idle refreshes usually find every log exactly where the last scan left
    // it. Answer those from the stored rollups instead of loading, rebuilding
    // and rewriting every cached event.
    if claude_logs_unchanged(provider, &roots, &files)? {
        return load_cached_claude_usage_statistics(provider, history_days);
    }
    let mut cache = store::with_store(|store| store.load_claude_cache(provider))?;
    let known_paths: BTreeSet<String> = files
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    drop_foreign_claude_logs(&mut cache, &known_paths, &roots);
    for path in files {
        let key = path.to_string_lossy().into_owned();
        let cached = cache.files.entry(key).or_default();
        scan_claude_file_delta(&path, cached)?;
    }
    let oldest = Local::now().date_naive() - Duration::days(CACHE_RETENTION_DAYS - 1);
    prune_claude_history(&mut cache, &known_paths, oldest);
    cache.version = CLAUDE_CACHE_VERSION;
    let entries = deduplicate_claude_entries(&cache);
    let stats = claude_statistics(&entries, history_days);
    let model_daily = claude_model_daily(&entries)
        .into_iter()
        .map(|(date, model, usage)| (model, date, usage))
        .collect::<Vec<_>>();
    drop(entries);
    store::with_store(|store| {
        store.save_claude_cache(provider, &cache)?;
        store.replace_usage_daily(provider, &stats.daily)?;
        store.replace_usage_model_daily(provider, &model_daily)
    })?;
    Ok(stats)
}

/// Claude Code deletes old transcripts (30 days by default), but their usage
/// stays counted for the cache's own retention. Only logs outside this
/// instance's folders are dropped: its folder was changed, so they belong to
/// another account.
fn drop_foreign_claude_logs(
    cache: &mut ClaudeUsageCache,
    known_paths: &BTreeSet<String>,
    roots: &[PathBuf],
) {
    cache
        .files
        .retain(|path, _| known_paths.contains(path) || under_any_root(Path::new(path), roots));
}

/// Applies the cache retention to every log, and forgets deleted logs once
/// none of their usage is retained.
fn prune_claude_history(
    cache: &mut ClaudeUsageCache,
    known_paths: &BTreeSet<String>,
    oldest: NaiveDate,
) {
    for cached in cache.files.values_mut() {
        cached
            .entries
            .retain(|entry| entry.timestamp.with_timezone(&Local).date_naive() >= oldest);
    }
    cache
        .files
        .retain(|path, cached| !cached.entries.is_empty() || known_paths.contains(path));
}

/// True when the stored scan already covers every current Claude log with its
/// full length incorporated, and holds no log a full refresh would drop.
fn claude_logs_unchanged(
    provider: ProviderId,
    roots: &[PathBuf],
    files: &[PathBuf],
) -> Result<bool> {
    let Some(offsets) = store::with_store(|store| store.load_claude_scan_offsets(provider))? else {
        return Ok(false);
    };
    if !offsets
        .keys()
        .all(|path| under_any_root(Path::new(path), roots))
    {
        return Ok(false);
    }
    Ok(files.iter().all(|path| {
        offsets
            .get(path.to_string_lossy().as_ref())
            .is_some_and(|&offset| fs::metadata(path).is_ok_and(|meta| meta.len() == offset))
    }))
}

pub(crate) fn statistics_from_claude_cache(
    cache: &ClaudeUsageCache,
    history_days: u16,
) -> UsageStatistics {
    claude_statistics(&deduplicate_claude_entries(cache), history_days)
}

pub(crate) fn aggregate_claude_model_daily(
    cache: &ClaudeUsageCache,
) -> Vec<(NaiveDate, String, TokenUsage)> {
    claude_model_daily(&deduplicate_claude_entries(cache))
}

fn claude_statistics(entries: &[&CachedClaudeUsageEntry], history_days: u16) -> UsageStatistics {
    let days: Vec<DailyTokenUsage> = entries
        .iter()
        .map(|entry| DailyTokenUsage {
            date: entry.timestamp.with_timezone(&Local).date_naive(),
            usage: entry.usage.clone(),
        })
        .collect();
    statistics_from_daily(&days, history_days)
}

fn claude_model_daily(entries: &[&CachedClaudeUsageEntry]) -> Vec<(NaiveDate, String, TokenUsage)> {
    let mut merged = BTreeMap::<(&str, NaiveDate), TokenUsage>::new();
    for entry in entries {
        let model = entry.model.as_deref().unwrap_or("unknown");
        let date = entry.timestamp.with_timezone(&Local).date_naive();
        merged.entry((model, date)).or_default().add(&entry.usage);
    }
    merged
        .into_iter()
        .map(|((model, date), usage)| (date, model.to_owned(), usage))
        .collect()
}

/// Mirrors Claude Code/OpenUsage's duplicate preference: the original message
/// beats a sidechain replay; otherwise retain the richer/larger record.
pub(crate) fn deduplicate_claude_entries(cache: &ClaudeUsageCache) -> Vec<&CachedClaudeUsageEntry> {
    let mut entries: Vec<&CachedClaudeUsageEntry> = Vec::new();
    let mut exact = HashMap::<(&str, Option<&str>), usize>::new();
    let mut by_message = HashMap::<&str, Vec<usize>>::new();

    for entry in cache.files.values().flat_map(|file| &file.entries) {
        let Some(message_id) = entry.message_id.as_deref() else {
            entries.push(entry);
            continue;
        };
        let key = (message_id, entry.request_id.as_deref());
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
            if claude_entry_should_replace(entry, entries[index]) {
                let previous = entries[index];
                if let Some(previous_id) = previous.message_id.as_deref() {
                    exact.remove(&(previous_id, previous.request_id.as_deref()));
                }
                entries[index] = entry;
                exact.insert(key, index);
            }
            continue;
        }

        let index = entries.len();
        entries.push(entry);
        exact.insert(key, index);
        by_message.entry(message_id).or_default().push(index);
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
    let mut bytes = Vec::new();
    loop {
        bytes.clear();
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

/// The few fields a Claude log line contributes. Everything else (message
/// content, tool output, attachments) is skipped while parsing instead of
/// being materialized as a `Value` tree. Scalars stay `Value` so a field of an
/// unexpected type is ignored exactly as before rather than rejecting the line.
#[derive(Deserialize)]
struct ClaudeLogLine {
    #[serde(rename = "type")]
    kind: Option<Value>,
    timestamp: Option<Value>,
    message: Option<ClaudeLogMessage>,
    #[serde(rename = "requestId")]
    request_id: Option<Value>,
    #[serde(rename = "isSidechain")]
    is_sidechain: Option<Value>,
    #[serde(rename = "costUSD")]
    cost_usd: Option<Value>,
}

#[derive(Deserialize)]
struct ClaudeLogMessage {
    id: Option<Value>,
    model: Option<Value>,
    usage: Option<Value>,
}

fn claude_usage_from_line(line: &[u8]) -> Option<CachedClaudeUsageEntry> {
    let event: ClaudeLogLine = serde_json::from_slice(line).ok()?;
    if event.kind.as_ref().and_then(Value::as_str) != Some("assistant") {
        return None;
    }
    let timestamp = DateTime::parse_from_rfc3339(event.timestamp.as_ref()?.as_str()?)
        .ok()?
        .with_timezone(&Utc);
    let message = event.message.as_ref()?;
    let usage_json = message.usage.as_ref()?;
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
    let cache_creation_tokens = cache_write_5m.saturating_add(cache_write_1h);
    let model = message
        .model
        .as_ref()
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())?;
    let reported_cost = event
        .cost_usd
        .as_ref()
        .and_then(Value::as_f64)
        .filter(|cost| cost.is_finite() && *cost >= 0.0);
    let estimated_cost_microusd = reported_cost
        .map(|cost| (cost * 1_000_000.0).round().clamp(0.0, u64::MAX as f64) as u64)
        .or_else(|| {
            pricing::request_cost_microusd(
                ProviderKind::Claude,
                Some(model),
                cache_creation_tokens,
                input_tokens,
                cache_read,
                output_tokens,
            )
        });
    let cache_savings_microusd =
        pricing::cache_savings_microusd(ProviderKind::Claude, Some(model), cache_read);
    let usage = TokenUsage {
        input_tokens: input_tokens.saturating_add(cache_creation_tokens),
        cached_input_tokens: cache_read.min(input_tokens),
        output_tokens,
        requests: 1,
        estimated_cost_microusd: estimated_cost_microusd.unwrap_or_default(),
        priced_requests: u64::from(estimated_cost_microusd.is_some()),
        cache_savings_microusd,
    };
    Some(CachedClaudeUsageEntry {
        timestamp,
        message_id: message
            .id
            .as_ref()
            .and_then(Value::as_str)
            .map(str::to_owned),
        request_id: event
            .request_id
            .as_ref()
            .and_then(Value::as_str)
            .map(str::to_owned),
        is_sidechain: event
            .is_sidechain
            .as_ref()
            .and_then(Value::as_bool)
            .unwrap_or(false),
        has_speed: usage_json.get("speed").is_some(),
        usage,
        model: Some(model.to_owned()),
    })
}

/// The `projects` folders holding an instance's Claude logs. `config_folder`
/// is the instance's own `CLAUDE_CONFIG_DIR`; `None` is this PC's standard
/// login, the same folder its credentials are read from. The process
/// environment is deliberately ignored: Minibar inherits whatever shell or
/// tool launched it, and an unrelated `CLAUDE_CONFIG_DIR` there would make
/// this instance count another account's logs.
fn claude_projects_roots(config_folder: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(folder) = config_folder {
        roots.push(folder.join("projects"));
    } else if let Some(base) = directories::BaseDirs::new() {
        roots.push(
            base.home_dir()
                .join(".config")
                .join("claude")
                .join("projects"),
        );
        roots.push(base.home_dir().join(".claude").join("projects"));
    }
    roots.dedup();
    roots
}

fn under_any_root(path: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| path.starts_with(root))
}

fn collect_claude_session_files(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for projects in roots {
        let _ = collect_session_files(projects, &mut files);
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
        assert_eq!(usage.priced_requests, 0);
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

    fn claude_entry_at(timestamp: DateTime<Utc>, message_id: &str) -> CachedClaudeUsageEntry {
        CachedClaudeUsageEntry {
            timestamp,
            message_id: Some(message_id.into()),
            request_id: None,
            is_sidechain: false,
            has_speed: false,
            usage: TokenUsage {
                input_tokens: 10,
                requests: 1,
                ..Default::default()
            },
            model: Some("claude-sonnet-4-20250514".into()),
        }
    }

    fn claude_file(entries: Vec<CachedClaudeUsageEntry>) -> CachedClaudeSessionFile {
        CachedClaudeSessionFile { offset: 1, entries }
    }

    fn key(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn deleted_claude_logs_keep_their_usage_inside_the_instance_folder() {
        let root = PathBuf::from(r"C:\Users\me\.claude\projects");
        let live = root.join("a").join("live.jsonl");
        let deleted = root.join("a").join("deleted.jsonl");
        let foreign = PathBuf::from(r"C:\Users\me\.claude-other\projects\b\x.jsonl");
        let now = Utc::now();
        let mut cache = ClaudeUsageCache {
            version: CLAUDE_CACHE_VERSION,
            files: BTreeMap::from([
                (key(&live), claude_file(vec![claude_entry_at(now, "m1")])),
                (key(&deleted), claude_file(vec![claude_entry_at(now, "m2")])),
                (key(&foreign), claude_file(vec![claude_entry_at(now, "m3")])),
            ]),
        };
        let known = BTreeSet::from([key(&live)]);

        drop_foreign_claude_logs(&mut cache, &known, std::slice::from_ref(&root));

        assert!(cache.files.contains_key(&key(&live)));
        assert!(cache.files.contains_key(&key(&deleted)));
        assert!(!cache.files.contains_key(&key(&foreign)));
        assert_eq!(deduplicate_claude_entries(&cache).len(), 2);
    }

    #[test]
    fn deleted_claude_logs_are_forgotten_once_their_usage_expires() {
        let root = PathBuf::from(r"C:\Users\me\.claude\projects");
        let live = root.join("live.jsonl");
        let deleted = root.join("deleted.jsonl");
        let expired = root.join("expired.jsonl");
        let now = Utc::now();
        let old = now - Duration::days(400);
        let mut cache = ClaudeUsageCache {
            version: CLAUDE_CACHE_VERSION,
            files: BTreeMap::from([
                (key(&live), claude_file(vec![claude_entry_at(old, "m1")])),
                (
                    key(&deleted),
                    claude_file(vec![claude_entry_at(old, "m2"), claude_entry_at(now, "m3")]),
                ),
                (key(&expired), claude_file(vec![claude_entry_at(old, "m4")])),
            ]),
        };
        let known = BTreeSet::from([key(&live)]);
        let oldest = Local::now().date_naive() - Duration::days(CACHE_RETENTION_DAYS - 1);

        prune_claude_history(&mut cache, &known, oldest);

        // A live log keeps its scan offset even with nothing retained.
        assert!(cache.files[&key(&live)].entries.is_empty());
        assert_eq!(cache.files[&key(&deleted)].entries.len(), 1);
        assert!(!cache.files.contains_key(&key(&expired)));
    }

    #[test]
    fn primary_claude_logs_ignore_the_inherited_config_dir() {
        let home = directories::BaseDirs::new()
            .unwrap()
            .home_dir()
            .to_path_buf();
        assert_eq!(
            claude_projects_roots(None),
            vec![
                home.join(".config").join("claude").join("projects"),
                home.join(".claude").join("projects"),
            ]
        );
        let folder = PathBuf::from(r"D:\claude-work");
        assert_eq!(
            claude_projects_roots(Some(&folder)),
            vec![folder.join("projects")]
        );
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
        let delta = scan_file_delta(&path, "sessions/session.jsonl", &mut cached).unwrap();
        assert!(delta.rebuilt);
        assert_eq!(cached.daily[0].usage.total_tokens(), 10);

        fs::write(&path, format!("{context}\n{first}\n{second}\n")).unwrap();
        let delta = scan_file_delta(&path, "sessions/session.jsonl", &mut cached).unwrap();
        assert!(!delta.rebuilt);
        assert_eq!(delta.events[0].source, "sessions/session.jsonl");
        assert_eq!(cached.daily[0].usage.total_tokens(), 15);
        assert_eq!(cached.daily[0].usage.requests, 2);

        fs::write(&path, format!("{context}\n{first}\n")).unwrap();
        let delta = scan_file_delta(&path, "sessions/session.jsonl", &mut cached).unwrap();
        assert!(delta.rebuilt);
        assert_eq!(delta.events.len(), 1);
        assert_eq!(cached.daily[0].usage.total_tokens(), 10);
        assert_eq!(cached.daily[0].usage.requests, 1);
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
        assert!(
            ingest_codex_line(&token_count(10, 1, "2026-08-01T05:00:00.000Z"), &mut cached)
                .is_none()
        );
    }

    #[test]
    fn skips_unchanged_consecutive_token_counts() {
        let mut cached = CachedSessionFile {
            current_model: Some("gpt-5.4".into()),
            ..Default::default()
        };
        assert!(
            ingest_codex_line(&token_count(10, 1, "2026-08-01T05:00:00.000Z"), &mut cached)
                .is_some()
        );
        assert!(
            ingest_codex_line(&token_count(10, 1, "2026-08-01T05:00:00.100Z"), &mut cached)
                .is_none()
        );
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
        assert!(
            ingest_codex_line(
                &token_count(100, 10, "2026-08-01T05:00:00.001Z"),
                &mut cached,
            )
            .is_none()
        );
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
