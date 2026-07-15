use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::{self, File},
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Local, NaiveDate, Utc};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const CACHE_VERSION: u8 = 1;
const CACHE_RETENTION_DAYS: i64 = 365;

/// Locally recorded Codex token usage. This is deliberately derived only from
/// session logs: it never reads credentials or contacts OpenAI directly.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub requests: u64,
    /// Locally measured or model-priced request cost. Codex's legacy cache
    /// leaves this empty and continues to use its existing estimate below.
    #[serde(default)]
    pub estimated_cost_microusd: u64,
    #[serde(default)]
    pub priced_requests: u64,
}

impl TokenUsage {
    /// `cached_input_tokens` is a subset of `input_tokens` in Codex session
    /// records, so it must not be counted twice in the displayed total.
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens.saturating_add(self.output_tokens)
    }

    /// Approximate API value in USD using GPT-5.4 list pricing as of
    /// 2026-07-14: $2.50/M input, $0.25/M cached input and $15.00/M output.
    /// This deliberately describes an API-equivalent value, not a Codex plan
    /// charge: subscription billing and included usage follow different rules.
    pub fn estimated_api_value_usd(&self) -> f64 {
        if self.priced_requests == self.requests && self.requests > 0 {
            return self.estimated_cost_microusd as f64 / 1_000_000.0;
        }
        const NANODOLLARS_PER_DOLLAR: u64 = 1_000_000_000;
        const INPUT_NANODOLLARS_PER_TOKEN: u64 = 2_500;
        const CACHED_INPUT_NANODOLLARS_PER_TOKEN: u64 = 250;
        const OUTPUT_NANODOLLARS_PER_TOKEN: u64 = 15_000;

        let uncached_input = self
            .input_tokens
            .saturating_sub(self.cached_input_tokens);
        let nanodollars = uncached_input
            .saturating_mul(INPUT_NANODOLLARS_PER_TOKEN)
            .saturating_add(
                self.cached_input_tokens
                    .saturating_mul(CACHED_INPUT_NANODOLLARS_PER_TOKEN),
            )
            .saturating_add(
                self.output_tokens
                    .saturating_mul(OUTPUT_NANODOLLARS_PER_TOKEN),
            );
        nanodollars as f64 / NANODOLLARS_PER_DOLLAR as f64
    }

    fn add(&mut self, other: &Self) {
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
struct UsageCache {
    version: u8,
    files: BTreeMap<String, CachedSessionFile>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct CachedSessionFile {
    /// Number of complete JSONL bytes already incorporated into `daily`.
    offset: u64,
    daily: Vec<DailyTokenUsage>,
}

impl CachedSessionFile {
    fn add(&mut self, date: NaiveDate, usage: TokenUsage) {
        if let Some(entry) = self.daily.iter_mut().find(|entry| entry.date == date) {
            entry.usage.add(&usage);
        } else {
            self.daily.push(DailyTokenUsage { date, usage });
        }
    }

    fn prune_before(&mut self, oldest: NaiveDate) {
        self.daily.retain(|entry| entry.date >= oldest);
        self.daily.sort_by_key(|entry| entry.date);
    }
}

/// Returns an immediately available snapshot from the persisted local cache.
/// It never opens or scans Codex session logs.
pub fn load_cached_usage_statistics(history_days: u16) -> Result<UsageStatistics> {
    let cache = load_cache()?;
    Ok(statistics_from_cache(&cache, history_days))
}

/// Incorporates only JSONL bytes appended since the previous scan, persists the
/// cache, and returns the refreshed aggregate. Truncated/replaced files are
/// safely rebuilt from their beginning.
pub fn refresh_usage_statistics(history_days: u16) -> Result<UsageStatistics> {
    let sessions_root = codex_home().join("sessions");
    let mut cache = load_cache()?;
    let mut files = Vec::new();
    collect_session_files(&sessions_root, &mut files)?;
    let known_paths: BTreeSet<String> = files
        .iter()
        .filter_map(|path| path.strip_prefix(&sessions_root).ok())
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    cache.files.retain(|path, _| known_paths.contains(path));

    let oldest = Local::now().date_naive() - Duration::days(CACHE_RETENTION_DAYS - 1);
    for path in files {
        let relative = path
            .strip_prefix(&sessions_root)
            .expect("session file was discovered below its root")
            .to_string_lossy()
            .into_owned();
        let cached = cache.files.entry(relative).or_default();
        scan_file_delta(&path, cached)?;
        cached.prune_before(oldest);
    }
    cache.version = CACHE_VERSION;
    save_cache(&cache)?;
    Ok(statistics_from_cache(&cache, history_days))
}

fn statistics_from_cache(cache: &UsageCache, history_days: u16) -> UsageStatistics {
    let history_days = history_days.clamp(1, 365);
    let today = Local::now().date_naive();
    let first_day = today - Duration::days(i64::from(history_days.saturating_sub(1)));
    let mut daily = BTreeMap::<NaiveDate, TokenUsage>::new();
    for file in cache.files.values() {
        for entry in &file.daily {
            if entry.date >= first_day && entry.date <= today {
                daily.entry(entry.date).or_default().add(&entry.usage);
            }
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
        cached.offset = 0;
        cached.daily.clear();
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
        if let Some((timestamp, usage)) = token_usage_from_line(line) {
            cached.add(timestamp.with_timezone(&Local).date_naive(), usage);
        }
    }
    cached.offset = offset;
    Ok(())
}

fn collect_session_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let Ok(entries) = fs::read_dir(root) else {
        return Ok(());
    };
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            collect_session_files(&path, files)?;
        } else if path.extension().is_some_and(|extension| extension == "jsonl") {
            files.push(path);
        }
    }
    Ok(())
}

fn load_cache() -> Result<UsageCache> {
    let path = cache_path()?;
    let Ok(contents) = fs::read(path) else {
        return Ok(UsageCache::default());
    };
    let cache: UsageCache = serde_json::from_slice(&contents).unwrap_or_default();
    if cache.version == CACHE_VERSION {
        Ok(cache)
    } else {
        Ok(UsageCache::default())
    }
}

fn save_cache(cache: &UsageCache) -> Result<()> {
    let path = cache_path()?;
    let parent = path.parent().context("usage cache path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("create usage cache in {}", parent.display()))?;
    serde_json::to_writer_pretty(temporary.as_file_mut(), cache)?;
    temporary.as_file().sync_all().context("flush usage cache")?;
    temporary
        .persist(path)
        .context("commit usage cache")?;
    Ok(())
}

fn cache_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("dev", "Codex Minibar", "Codex Minibar")
        .context("could not resolve the application config directory")?;
    Ok(dirs.config_dir().join("usage-cache.json"))
}

fn codex_home() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".codex")))
        .unwrap_or_else(|| PathBuf::from(".codex"))
}

fn token_usage_from_line(line: &str) -> Option<(DateTime<Utc>, TokenUsage)> {
    let event: Value = serde_json::from_str(line).ok()?;
    if event.get("type")?.as_str()? != "event_msg"
        || event.pointer("/payload/type")?.as_str()? != "token_count"
    {
        return None;
    }
    let timestamp = DateTime::parse_from_rfc3339(event.get("timestamp")?.as_str()?)
        .ok()?
        .with_timezone(&Utc);
    let usage = event.pointer("/payload/info/last_token_usage")?;
    let token = |name: &str| usage.get(name).and_then(Value::as_u64).unwrap_or(0);
    Some((
        timestamp,
        TokenUsage {
            input_tokens: token("input_tokens"),
            cached_input_tokens: token("cached_input_tokens").max(token("cache_read_input_tokens")),
            output_tokens: token("output_tokens"),
            requests: 1,
            ..Default::default()
        },
    ))
}

/// Cached representation of one Claude Code response. Keeping individual
/// messages (rather than just daily totals) lets us suppress the same
/// sidechain/replayed message when it appears in more than one session log.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct CachedClaudeUsageEntry {
    timestamp: DateTime<Utc>,
    message_id: Option<String>,
    request_id: Option<String>,
    is_sidechain: bool,
    has_speed: bool,
    usage: TokenUsage,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct ClaudeUsageCache {
    version: u8,
    files: BTreeMap<String, CachedClaudeSessionFile>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct CachedClaudeSessionFile {
    /// Number of complete JSONL bytes incorporated into `entries`.
    offset: u64,
    entries: Vec<CachedClaudeUsageEntry>,
}

/// Returns Claude Code usage from the on-disk cache without opening a log.
pub fn load_cached_claude_usage_statistics(history_days: u16) -> Result<UsageStatistics> {
    let cache = load_claude_cache()?;
    Ok(claude_statistics_from_cache(&cache, history_days))
}

/// Scans Claude Code's `projects/**/*.jsonl` logs incrementally. The cache is
/// separate from Codex's and stores a byte offset per file, so reopening the
/// popup never causes a full re-read of an ever-growing Claude history.
pub fn refresh_claude_usage_statistics(history_days: u16) -> Result<UsageStatistics> {
    let mut cache = load_claude_cache()?;
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
    cache.version = CACHE_VERSION;
    save_claude_cache(&cache)?;
    Ok(claude_statistics_from_cache(&cache, history_days))
}

fn claude_statistics_from_cache(cache: &ClaudeUsageCache, history_days: u16) -> UsageStatistics {
    let history_days = history_days.clamp(1, 365);
    let today = Local::now().date_naive();
    let first_day = today - Duration::days(i64::from(history_days.saturating_sub(1)));
    let mut daily = BTreeMap::<NaiveDate, TokenUsage>::new();

    for entry in deduplicate_claude_entries(cache) {
        let date = entry.timestamp.with_timezone(&Local).date_naive();
        if date >= first_day && date <= today {
            daily.entry(date).or_default().add(&entry.usage);
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

/// Mirrors Claude Code/OpenUsage's duplicate preference: the original message
/// beats a sidechain replay; otherwise retain the richer/larger record.
fn deduplicate_claude_entries(cache: &ClaudeUsageCache) -> Vec<CachedClaudeUsageEntry> {
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
        by_message.entry(message_id.clone()).or_default().push(index);
    }
    entries
}

fn claude_entry_should_replace(candidate: &CachedClaudeUsageEntry, existing: &CachedClaudeUsageEntry) -> bool {
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
        if let Some(entry) = claude_usage_from_line(&bytes) {
            cached.entries.push(entry);
        }
    }
    cached.offset = offset;
    Ok(())
}

fn claude_usage_from_line(line: &[u8]) -> Option<CachedClaudeUsageEntry> {
    let event: Value = serde_json::from_slice(line).ok()?;
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
    let model = message.get("model").and_then(Value::as_str);
    let cost = event
        .get("costUSD")
        .and_then(Value::as_f64)
        .filter(|cost| cost.is_finite() && *cost >= 0.0)
        .or_else(|| {
            claude_estimated_cost_usd(
                model,
                input_tokens,
                cache_read,
                output_tokens,
                cache_write_5m,
                cache_write_1h,
            )
        })?;
    let usage = TokenUsage {
        input_tokens,
        cached_input_tokens: cache_read.min(input_tokens),
        output_tokens,
        requests: 1,
        estimated_cost_microusd: (cost * 1_000_000.0)
            .round()
            .clamp(0.0, u64::MAX as f64) as u64,
        priced_requests: 1,
    };
    Some(CachedClaudeUsageEntry {
        timestamp,
        message_id: message.get("id").and_then(Value::as_str).map(str::to_owned),
        request_id: event.get("requestId").and_then(Value::as_str).map(str::to_owned),
        is_sidechain: event.get("isSidechain").and_then(Value::as_bool).unwrap_or(false),
        has_speed: usage_json.get("speed").is_some(),
        usage,
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

fn collect_claude_session_files() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(config_dirs) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        // Claude Code accepts comma-separated roots; Windows also commonly
        // receives a normal PATH-style list from launchers, so tolerate both.
        let raw = config_dirs.to_string_lossy();
        let configured_paths: Vec<PathBuf> = if raw.contains(',') {
            raw.split(',').map(|part| PathBuf::from(part.trim())).collect()
        } else {
            std::env::split_paths(&config_dirs).collect()
        };
        for path in configured_paths.into_iter().filter(|path| !path.as_os_str().is_empty()) {
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

fn load_claude_cache() -> Result<ClaudeUsageCache> {
    let path = claude_cache_path()?;
    let Ok(contents) = fs::read(path) else {
        return Ok(ClaudeUsageCache::default());
    };
    let cache: ClaudeUsageCache = serde_json::from_slice(&contents).unwrap_or_default();
    if cache.version == CACHE_VERSION {
        Ok(cache)
    } else {
        Ok(ClaudeUsageCache::default())
    }
}

fn save_claude_cache(cache: &ClaudeUsageCache) -> Result<()> {
    let path = claude_cache_path()?;
    let parent = path.parent().context("Claude usage cache path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("create Claude usage cache in {}", parent.display()))?;
    serde_json::to_writer_pretty(temporary.as_file_mut(), cache)?;
    temporary.as_file().sync_all().context("flush Claude usage cache")?;
    temporary.persist(path).context("commit Claude usage cache")?;
    Ok(())
}

fn claude_cache_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("dev", "Codex Minibar", "Codex Minibar")
        .context("could not resolve the application config directory")?;
    Ok(dirs.config_dir().join("claude-usage-cache.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_per_request_token_usage() {
        let line = r#"{"timestamp":"2026-07-14T10:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":12,"cached_input_tokens":7,"output_tokens":4}}}}"#;
        let (_, usage) = token_usage_from_line(line).unwrap();
        assert_eq!(usage.total_tokens(), 16);
        assert_eq!(usage.requests, 1);
    }

    #[test]
    fn estimates_gpt_5_4_api_value_without_double_counting_cache() {
        let usage = TokenUsage {
            input_tokens: 1_000_000,
            cached_input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            requests: 1,
            ..Default::default()
        };
        assert!((usage.estimated_api_value_usd() - 15.25).abs() < f64::EPSILON);

        let usage = TokenUsage {
            input_tokens: 1_000_000,
            cached_input_tokens: 0,
            output_tokens: 1_000_000,
            requests: 1,
            ..Default::default()
        };
        assert!((usage.estimated_api_value_usd() - 17.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ignores_non_usage_events() {
        assert!(
            token_usage_from_line(r#"{"type":"event_msg","payload":{"type":"task_started"}}"#)
                .is_none()
        );
    }

    #[test]
    fn reads_claude_usage_and_uses_its_recorded_cost() {
        let line = r#"{"timestamp":"2026-07-14T10:00:00Z","requestId":"request-1","message":{"id":"message-1","model":"claude-sonnet-4-20250514","usage":{"input_tokens":100,"cache_read_input_tokens":40,"output_tokens":25,"speed":"standard"}},"costUSD":0.0125}"#;
        let entry = claude_usage_from_line(line.as_bytes()).unwrap();
        assert_eq!(entry.usage.total_tokens(), 125);
        assert_eq!(entry.usage.cached_input_tokens, 40);
        assert_eq!(entry.usage.estimated_api_value_usd(), 0.0125);
        assert!(entry.has_speed);
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
        };
        let replay = CachedClaudeUsageEntry {
            request_id: Some("request-sidechain".into()),
            is_sidechain: true,
            ..original.clone()
        };
        let cache = ClaudeUsageCache {
            version: CACHE_VERSION,
            files: BTreeMap::from([
                ("a.jsonl".into(), CachedClaudeSessionFile { offset: 0, entries: vec![original] }),
                ("b.jsonl".into(), CachedClaudeSessionFile { offset: 0, entries: vec![replay] }),
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
        let first = r#"{"timestamp":"2026-07-14T10:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":10}}}}"#;
        let second = r#"{"timestamp":"2026-07-14T11:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"output_tokens":5}}}}"#;
        fs::write(&path, format!("{first}\n{second}")).unwrap();

        let mut cached = CachedSessionFile::default();
        scan_file_delta(&path, &mut cached).unwrap();
        assert_eq!(cached.daily[0].usage.total_tokens(), 10);

        fs::write(&path, format!("{first}\n{second}\n")).unwrap();
        scan_file_delta(&path, &mut cached).unwrap();
        assert_eq!(cached.daily[0].usage.total_tokens(), 15);
        assert_eq!(cached.daily[0].usage.requests, 2);
    }
}
