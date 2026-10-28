//! WorkBuddy (Tencent) — usage-only provider.
//!
//! WorkBuddy keeps one JSONL transcript per session under
//! `%USERPROFILE%\.workbuddy\projects\<slug>\<session-id>.jsonl`. Every model
//! call appends an event whose `providerData.rawUsage` carries the accounting
//! for that call, which is everything this driver needs: WorkBuddy bills per
//! request in credits and exposes no 5-hour or weekly quota endpoint, so there
//! is nothing to poll.
//!
//! Field semantics mirror Codex rollouts: `prompt_tokens` is the whole prompt of
//! the call and `prompt_cache_hit_tokens` is a *subset* of it, so the cached
//! part must never be added twice. `credit` is WorkBuddy's own billing unit
//! (积分), not USD, so it is deliberately not folded into `estimated_cost_microusd`
//! — these models have no pricing entry and a $0.00 estimate would be a lie.

use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    time::{Duration as StdDuration, SystemTime},
};

use anyhow::{Result, bail};
use chrono::{DateTime, Local, NaiveDate, Utc};
use serde::Deserialize;

use crate::{
    instances::ProviderId,
    limits::RateLimits,
    store,
    usage::{DailyTokenUsage, TokenUsage, UsageStatistics, statistics_from_daily},
    worker::{Activator, LimitProvider, UsageProvider},
};

/// WorkBuddy writes one transcript per session; a busy machine can accumulate
/// thousands of them, so files older than the requested window are skipped by
/// mtime instead of being parsed.
pub struct WorkBuddyClient {
    provider: ProviderId,
}

impl WorkBuddyClient {
    pub fn new(provider: ProviderId) -> Self {
        Self { provider }
    }

    fn read_local_usage(&self, history_days: u16) -> Result<UsageStatistics> {
        match transcripts_root() {
            Some(root) => scan_root(&root, history_days),
            None => Ok(UsageStatistics::default()),
        }
    }
}

impl LimitProvider for WorkBuddyClient {
    fn read_limits(&mut self) -> Result<RateLimits> {
        // No quota windows exist. Returning an empty sample keeps the provider
        // out of the "failed" state; the UI already renders providers without a
        // session window (Codex Free reports the same shape).
        Ok(RateLimits {
            account_name: Some("Local usage".to_string()),
            sampled_at: Utc::now(),
            ..RateLimits::default()
        })
    }
}

impl UsageProvider for WorkBuddyClient {
    fn load_cached_usage_statistics(&mut self, history_days: u16) -> Result<UsageStatistics> {
        store::with_store(|store| store.load_usage_daily(self.provider, history_days))
            .or_else(|_| Ok(UsageStatistics::default()))
    }

    fn refresh_usage_statistics(&mut self, history_days: u16) -> Result<UsageStatistics> {
        let statistics = self.read_local_usage(history_days)?;
        store::with_store(|store| store.replace_usage_daily(self.provider, &statistics.daily))?;
        Ok(statistics)
    }

    /// Local history stays useful without a quota endpoint.
    fn refresh_without_limits(&self) -> bool {
        true
    }
}

impl Activator for WorkBuddyClient {
    fn activate(&mut self) -> Result<()> {
        bail!("WorkBuddy does not expose a session activation endpoint")
    }
}

// ---------------------------------------------------------------- local scan

#[derive(Debug, Deserialize)]
struct SessionEvent {
    /// Milliseconds since the Unix epoch, as written by WorkBuddy.
    timestamp: i64,
    #[serde(default, rename = "providerData")]
    provider_data: Option<ProviderData>,
}

#[derive(Debug, Deserialize)]
struct ProviderData {
    #[serde(default)]
    raw_usage: Option<RawUsage>,
}

#[derive(Debug, Default, Deserialize)]
struct RawUsage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
    #[serde(default)]
    prompt_cache_hit_tokens: u64,
}

/// Whether this PC has any WorkBuddy history to read.
pub fn is_installed() -> bool {
    transcripts_root().is_some_and(|root| root.is_dir())
}

fn transcripts_root() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .map(|home| home.join(".workbuddy").join("projects"))
}

/// Aggregates every transcript under `root` into per-local-day token totals.
fn scan_root(root: &Path, history_days: u16) -> Result<UsageStatistics> {
    let mut daily = BTreeMap::<NaiveDate, TokenUsage>::new();
    let cutoff = SystemTime::now()
        .checked_sub(StdDuration::from_secs(
            u64::from(history_days.max(1)) * 24 * 60 * 60,
        ))
        .unwrap_or(SystemTime::UNIX_EPOCH);

    for path in transcripts(root) {
        if !modified_after(&path, cutoff) {
            continue;
        }
        let Ok(file) = File::open(&path) else {
            continue;
        };
        for line in BufReader::new(file).lines() {
            let Ok(line) = line else { break };
            // Most lines are prompts and tool output with no accounting at all.
            if !line.contains("\"rawUsage\"") {
                continue;
            }
            let Ok(event) = serde_json::from_str::<SessionEvent>(&line) else {
                continue;
            };
            let Some(usage) = event.provider_data.and_then(|data| data.raw_usage) else {
                continue;
            };
            let Some(stamp) = DateTime::<Utc>::from_timestamp_millis(event.timestamp) else {
                continue;
            };
            daily
                .entry(stamp.with_timezone(&Local).date_naive())
                .or_default()
                .add_public(&TokenUsage {
                    input_tokens: usage.prompt_tokens,
                    cached_input_tokens: usage.prompt_cache_hit_tokens,
                    output_tokens: usage.completion_tokens,
                    requests: 1,
                    ..TokenUsage::default()
                });
        }
    }

    let days = daily
        .into_iter()
        .map(|(date, usage)| DailyTokenUsage { date, usage })
        .collect::<Vec<_>>();
    Ok(statistics_from_daily(&days, history_days))
}

fn modified_after(path: &Path, cutoff: SystemTime) -> bool {
    path.metadata()
        .and_then(|meta| meta.modified())
        .map(|modified| modified >= cutoff)
        .unwrap_or(true)
}

fn transcripts(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    if !root.is_dir() {
        return found;
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("jsonl"))
            {
                found.push(path);
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_totals_match_expected() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/workbuddy");
        let stats = scan_root(&root, 30).expect("scan fixture");
        let day = stats
            .daily
            .iter()
            .find(|entry| entry.date == NaiveDate::from_ymd_opt(2026, 10, 27).unwrap())
            .expect("fixture day");
        assert_eq!(day.usage.input_tokens, 9_800);
        assert_eq!(day.usage.cached_input_tokens, 8_600);
        assert_eq!(day.usage.output_tokens, 900);
        assert_eq!(day.usage.requests, 3);
        assert_eq!(day.usage.estimated_cost_microusd, 0);
    }

    #[test]
    fn missing_root_is_empty_not_an_error() {
        let stats = scan_root(Path::new("no-such-directory"), 30).expect("scan");
        assert!(!stats.has_data());
    }
}
