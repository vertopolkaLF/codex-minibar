use std::{
    collections::{BTreeMap, BTreeSet},
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
        },
    ))
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
        };
        assert!((usage.estimated_api_value_usd() - 15.25).abs() < f64::EPSILON);

        let usage = TokenUsage {
            input_tokens: 1_000_000,
            cached_input_tokens: 0,
            output_tokens: 1_000_000,
            requests: 1,
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
    fn token_usage_saturates() {
        let usage = TokenUsage {
            input_tokens: u64::MAX,
            cached_input_tokens: 1,
            output_tokens: 1,
            requests: 0,
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
