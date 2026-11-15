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
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    time::{Duration as StdDuration, SystemTime},
};

use anyhow::{Context, Result};
use chrono::{DateTime, Local, Utc};
use serde::Deserialize;

use crate::{
    platform::local_time::RETENTION_DAYS,
    providers::drivers::{Driver, InstallStatus, SettingsCopy, Sources},
    providers::instances::{ProviderId, ProviderInstance},
    providers::worker::{LimitProvider, UsageProvider, WorkerHandle, WorkerOptions},
    quota::limits::RateLimits,
    store,
    usage::{Rollup, RollupBuilder, TokenUsage, UsageStatistics, statistics_from_daily},
};

/// WorkBuddy writes one transcript per session; a busy machine can accumulate
/// thousands of them, so files older than the retention window are skipped by
/// mtime instead of being parsed.
pub struct WorkBuddyClient {
    provider: ProviderId,
}

impl WorkBuddyClient {
    pub fn new(provider: ProviderId) -> Self {
        Self { provider }
    }

    fn read_local_usage(&self) -> Result<Rollup> {
        let root = transcripts_root().context("WorkBuddy home directory is unavailable")?;
        scan_root(&root, Local::now())
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
        let rollup = match self.read_local_usage() {
            Ok(rollup) => rollup,
            Err(error) => {
                // A partial scan is not authoritative. Preserve all stored
                // daily/hourly rows and keep showing the last complete scan.
                crate::platform::logger::info(format!("WorkBuddy usage scan failed: {error:#}"));
                return store::with_store(|store| {
                    store.load_usage_daily(self.provider, history_days)
                });
            }
        };
        store::with_store(|store| store.save_rollup(self.provider, &rollup, None))?;
        Ok(statistics_from_daily(&rollup.daily, history_days))
    }

    /// Local history stays useful without a quota endpoint.
    fn refresh_without_limits(&self) -> bool {
        true
    }
}

pub(crate) static DRIVER: Driver = Driver {
    start: start_worker,
    detect,
    sources: Sources {
        app: false,
        crew: false,
        cli: false,
    },
    copy: settings_copy,
    forget_secrets: None,
    consume_reset: None,
};

fn start_worker(instance: &ProviderInstance, options: WorkerOptions) -> Result<WorkerHandle> {
    Ok(crate::providers::worker::start_worker(
        WorkBuddyClient::new(instance.provider_id()),
        WorkBuddyClient::new(instance.provider_id()),
        None,
        options,
    ))
}

fn detect(instance: &ProviderInstance) -> InstallStatus {
    InstallStatus {
        signed_in: is_installed(),
        ..InstallStatus::none(instance)
    }
}

fn settings_copy() -> SettingsCopy {
    SettingsCopy {
        binary_source: None,
        ..SettingsCopy::new(crate::i18n::tr(
            "reads-workbuddy-token-history-from-your-local-workbuddy-transcripts",
        ))
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
    #[serde(default, rename = "rawUsage")]
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
    directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".workbuddy").join("projects"))
}

/// Aggregates every transcript under `root` into per-local-day token totals.
fn scan_root(root: &Path, now: DateTime<Local>) -> Result<Rollup> {
    let mut rollup = RollupBuilder::new(now);
    let oldest = now.date_naive() - chrono::Duration::days(i64::from(RETENTION_DAYS) - 1);
    let cutoff = SystemTime::from(now)
        .checked_sub(StdDuration::from_secs(
            u64::from(RETENTION_DAYS) * 24 * 60 * 60,
        ))
        .unwrap_or(SystemTime::UNIX_EPOCH);

    for path in transcripts(root)? {
        if !modified_after(&path, cutoff)? {
            continue;
        }
        let file = File::open(&path)
            .with_context(|| format!("open WorkBuddy transcript {}", path.display()))?;
        for line in BufReader::new(file).lines() {
            let line =
                line.with_context(|| format!("read WorkBuddy transcript {}", path.display()))?;
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
            let at = stamp.with_timezone(&Local);
            if at.date_naive() < oldest || at.date_naive() > now.date_naive() {
                continue;
            }
            rollup.add(
                at,
                &TokenUsage {
                    input_tokens: usage.prompt_tokens,
                    cached_input_tokens: usage.prompt_cache_hit_tokens,
                    output_tokens: usage.completion_tokens,
                    requests: 1,
                    ..TokenUsage::default()
                },
                None,
                None,
            );
        }
    }

    Ok(rollup.finish())
}

fn modified_after(path: &Path, cutoff: SystemTime) -> Result<bool> {
    let modified = path
        .metadata()
        .and_then(|meta| meta.modified())
        .with_context(|| format!("read WorkBuddy transcript metadata {}", path.display()))?;
    Ok(modified >= cutoff)
}

fn transcripts(root: &Path) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    match root.try_exists() {
        Ok(false) => return Ok(found),
        Ok(true) => (),
        Err(error) => return Err(error).context("inspect WorkBuddy transcript root"),
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)
            .with_context(|| format!("read WorkBuddy transcript directory {}", dir.display()))?;
        for entry in entries {
            let entry = entry.with_context(|| format!("read entry in {}", dir.display()))?;
            let path = entry.path();
            let kind = entry
                .file_type()
                .with_context(|| format!("read WorkBuddy entry type {}", path.display()))?;
            // Inspect the entry itself: following links can duplicate history
            // or recurse indefinitely through a link to an ancestor.
            if kind.is_dir() {
                stack.push(path);
            } else if kind.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("jsonl"))
            {
                found.push(path);
            }
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixture_now() -> DateTime<Local> {
        Utc.with_ymd_and_hms(2026, 10, 28, 12, 0, 0)
            .unwrap()
            .with_timezone(&Local)
    }

    #[test]
    fn fixture_totals_match_expected() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("session.jsonl");
        std::fs::write(
            &path,
            include_str!("../../fixtures/workbuddy/sample-session.jsonl"),
        )
        .unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(SystemTime::from(fixture_now())))
            .unwrap();
        let rollup = scan_root(root.path(), fixture_now()).expect("scan fixture");
        // The third accounting event can be on the next local day (UTC+09).
        let mut expected_days = std::collections::BTreeMap::new();
        for stamp in [1_793_110_006_000, 1_793_110_007_000, 1_793_113_600_000] {
            let date = DateTime::<Utc>::from_timestamp_millis(stamp)
                .unwrap()
                .with_timezone(&Local)
                .date_naive();
            *expected_days.entry(date).or_insert(0_u64) += 1;
        }
        assert_eq!(
            rollup
                .daily
                .iter()
                .map(|day| (day.date, day.usage.requests))
                .collect::<Vec<_>>(),
            expected_days.into_iter().collect::<Vec<_>>()
        );
        let mut total = TokenUsage::default();
        for day in &rollup.daily {
            total.add(&day.usage);
        }
        assert_eq!(total.input_tokens, 9_800);
        assert_eq!(total.cached_input_tokens, 8_600);
        assert_eq!(total.output_tokens, 900);
        assert_eq!(total.requests, 3);
        assert_eq!(total.estimated_cost_microusd, 0);
        assert_eq!(total.priced_requests, 0);
        assert_eq!(
            rollup
                .hourly
                .iter()
                .map(|(_, usage)| usage.requests)
                .sum::<u64>(),
            3
        );

        // A recent file mtime must not bring expired transcript events back.
        let later = fixture_now() + chrono::Duration::days(i64::from(RETENTION_DAYS));
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(SystemTime::from(later)))
            .unwrap();
        assert!(scan_root(root.path(), later).unwrap().daily.is_empty());
    }

    #[test]
    fn missing_root_is_empty_not_an_error() {
        let root = tempfile::tempdir().unwrap();
        let rollup = scan_root(&root.path().join("missing"), fixture_now()).expect("scan");
        assert!(rollup.daily.is_empty());
    }

    #[test]
    fn unreadable_transcript_does_not_return_a_partial_rollup() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("good.jsonl"),
            include_str!("../../fixtures/workbuddy/sample-session.jsonl"),
        )
        .unwrap();
        std::fs::write(root.path().join("bad.jsonl"), [0xff, b'\n']).unwrap();
        assert!(scan_root(root.path(), fixture_now()).is_err());
        // Discovery and metadata failures must not look like empty history.
        assert!(scan_root(&root.path().join("good.jsonl"), fixture_now()).is_err());
        assert!(
            modified_after(&root.path().join("missing.jsonl"), SystemTime::UNIX_EPOCH).is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn discovery_skips_symlinked_files_and_directory_cycles() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("project");
        std::fs::create_dir(&project).unwrap();
        let path = project.join("session.jsonl");
        std::fs::write(&path, "").unwrap();
        symlink(&project, root.path().join("alias")).unwrap();
        symlink(root.path(), project.join("cycle")).unwrap();
        symlink(&path, project.join("alias.jsonl")).unwrap();
        assert_eq!(transcripts(root.path()).unwrap(), vec![path]);
    }
}
