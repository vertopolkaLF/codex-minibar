//! Public GitHub feed for announced Codex forced resets.
//!
//! The feed is intentionally a tiny, append/replace-friendly JSON document.
//! A bot can update it with one commit and clients can keep using the last
//! valid cache when GitHub or the raw-content CDN is temporarily unavailable.

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ureq::Agent;

use crate::{settings::Settings, worker::WorkerEvent};

pub const FEED_URL: &str =
    "https://raw.githubusercontent.com/vertopolkaLF/codex-minibar/main/data/codex-resets.json";
const FEED_SCHEMA_VERSION: u8 = 2;
const LEGACY_FEED_SCHEMA_VERSION: u8 = 1;
const USER_AGENT: &str = "codex-minibar-reset-feed";
const MAX_FEED_ENTRIES: usize = 32;
const MAX_NOTIFIED_IDS: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForcedReset {
    /// Stable bot-generated identity. It must not change when the description
    /// or the timestamp is corrected.
    pub id: String,
    /// Optional HTTPS page where the announcement can be verified by the user.
    #[serde(default)]
    pub source_url: Option<String>,
    /// Optional human-readable context, for example "weekly quota".
    #[serde(default)]
    pub label: Option<String>,
    /// ISO-8601 timestamp in UTC.
    pub reset_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResetFeedSnapshot {
    pub resets: Vec<ForcedReset>,
    pub notified_ids: Vec<String>,
}

#[derive(Debug)]
pub enum ResetFeedCommand {
    SetEnabled(bool),
    SetRefreshInterval(Duration),
    MarkNotified(String),
    Shutdown,
}

pub struct ResetFeedWorker {
    pub commands: mpsc::Sender<ResetFeedCommand>,
    join: Option<JoinHandle<()>>,
}

impl ResetFeedWorker {
    pub fn shutdown(mut self) {
        let _ = self.commands.send(ResetFeedCommand::Shutdown);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct ResetFeedCache {
    #[serde(default = "default_schema_version")]
    schema_version: u8,
    #[serde(default)]
    resets: Vec<ForcedReset>,
    #[serde(default)]
    notified_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ResetFeedDocument {
    schema_version: u8,
    #[serde(default)]
    resets: Vec<ResetFeedEntry>,
}

#[derive(Debug, Deserialize)]
struct ResetFeedEntry {
    id: String,
    #[serde(rename = "type")]
    kind: ResetKind,
    #[serde(default)]
    source_url: Option<String>,
    #[serde(default)]
    label: Option<String>,
    reset_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ResetKind {
    Forced,
    Banked,
}

fn default_schema_version() -> u8 {
    FEED_SCHEMA_VERSION
}

pub fn cache_path(settings_path: &Path) -> PathBuf {
    settings_path.with_file_name("codex-resets-cache.json")
}

pub fn start_worker(
    settings: &Settings,
    cache_path: PathBuf,
    events: mpsc::Sender<WorkerEvent>,
) -> ResetFeedWorker {
    let (commands, command_receiver) = mpsc::channel();
    let enabled = settings.notifications.forced_reset_feed_enabled;
    let interval = Duration::from_secs(settings.reset_announcement_refresh_interval.seconds());
    let join = thread::spawn(move || run(enabled, interval, cache_path, command_receiver, events));
    ResetFeedWorker {
        commands,
        join: Some(join),
    }
}

pub fn parse_feed(body: &str, now: DateTime<Utc>) -> Result<Vec<ForcedReset>> {
    parse_feed_with_stats(body, now).map(|(resets, _)| resets)
}

fn parse_feed_with_stats(body: &str, now: DateTime<Utc>) -> Result<(Vec<ForcedReset>, usize)> {
    let document: ResetFeedDocument =
        serde_json::from_str(body).context("parse Codex reset feed JSON")?;
    if !matches!(
        document.schema_version,
        LEGACY_FEED_SCHEMA_VERSION | FEED_SCHEMA_VERSION
    ) {
        bail!(
            "unsupported Codex reset feed schema version {}",
            document.schema_version
        );
    }

    let mut resets = Vec::new();
    let mut ids = HashSet::new();
    let mut missing_source_urls = 0;
    for entry in document.resets {
        // Banked resets are provider credits, not Tibo's forced reset
        // announcements. Ignore them at the ingestion boundary so no later
        // UI path can accidentally render them as countdowns.
        if entry.kind != ResetKind::Forced {
            continue;
        }
        if resets.len() >= MAX_FEED_ENTRIES {
            break;
        }
        let id = entry.id.trim().to_owned();
        let source_url = entry
            .source_url
            .as_deref()
            .map(str::trim)
            .filter(|url| is_valid_source_url(url))
            .map(str::to_owned);
        if source_url.is_none() {
            missing_source_urls += 1;
        }
        if id.is_empty() || entry.reset_at <= now || !ids.insert(id.clone()) {
            continue;
        }
        let label = entry
            .label
            .map(|label| label.trim().chars().take(120).collect::<String>())
            .filter(|label| !label.is_empty());
        resets.push(ForcedReset {
            id,
            source_url,
            label,
            reset_at: entry.reset_at,
        });
    }
    sort_resets(&mut resets);
    Ok((resets, missing_source_urls))
}

fn run(
    mut enabled: bool,
    mut refresh_interval: Duration,
    cache_path: PathBuf,
    commands: mpsc::Receiver<ResetFeedCommand>,
    events: mpsc::Sender<WorkerEvent>,
) {
    let mut cache = load_cache(&cache_path, Utc::now());
    crate::logger::info(format!(
        "Codex reset feed starting: enabled={enabled}, interval={}s, cache={}, cached_resets={}, notified_ids={}",
        refresh_interval.as_secs(),
        cache_path.display(),
        cache.resets.len(),
        cache.notified_ids.len(),
    ));
    if enabled {
        // Publish the client-side snapshot before touching the network. This
        // is what makes received resets survive an app restart or an offline
        // launch instead of blinking out until GitHub answers.
        send_snapshot(&events, &cache);
    }
    let agent = http_agent();
    // The first remote refresh is deliberately due immediately on launch.
    let mut next_refresh = Instant::now();

    loop {
        if enabled && next_refresh <= Instant::now() {
            match fetch(&agent) {
                Ok(resets) => {
                    cache.resets = resets;
                    if let Err(error) = save_cache(&cache_path, &cache) {
                        eprintln!("failed to cache Codex reset feed: {error:#}");
                    } else {
                        crate::logger::info(format!(
                            "Codex reset feed cache updated: cached_resets={}, notified_ids={}",
                            cache.resets.len(),
                            cache.notified_ids.len(),
                        ));
                    }
                    send_snapshot(&events, &cache);
                }
                Err(error) => {
                    crate::logger::info(format!("Codex reset feed refresh failed: {error:#}"));
                    let _ = events.send(WorkerEvent::ForcedResetsRefreshFailed(error.to_string()));
                }
            }
            next_refresh = Instant::now() + refresh_interval;
            continue;
        }

        let wait = if enabled {
            next_refresh.saturating_duration_since(Instant::now())
        } else {
            Duration::from_secs(24 * 60 * 60)
        };
        match commands.recv_timeout(wait) {
            Ok(ResetFeedCommand::SetEnabled(value)) => {
                if enabled == value {
                    continue;
                }
                enabled = value;
                if enabled {
                    cache = load_cache(&cache_path, Utc::now());
                    send_snapshot(&events, &cache);
                    next_refresh = Instant::now();
                } else {
                    // Keep the cache on disk for a fast re-enable, but make the
                    // live surface empty immediately while this option is off.
                    let mut hidden = cache.clone();
                    hidden.resets.clear();
                    send_snapshot(&events, &hidden);
                }
            }
            Ok(ResetFeedCommand::SetRefreshInterval(value)) => {
                refresh_interval = value.max(Duration::from_secs(60));
                if enabled {
                    next_refresh = Instant::now() + refresh_interval;
                }
            }
            Ok(ResetFeedCommand::MarkNotified(id)) => {
                if !id.trim().is_empty() && !cache.notified_ids.iter().any(|known| known == &id) {
                    cache.notified_ids.push(id);
                    if cache.notified_ids.len() > MAX_NOTIFIED_IDS {
                        let excess = cache.notified_ids.len() - MAX_NOTIFIED_IDS;
                        cache.notified_ids.drain(..excess);
                    }
                    if let Err(error) = save_cache(&cache_path, &cache) {
                        eprintln!("failed to cache Codex reset notification state: {error:#}");
                    }
                }
            }
            Ok(ResetFeedCommand::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

fn fetch(agent: &Agent) -> Result<Vec<ForcedReset>> {
    crate::logger::info(format!("Codex reset feed request: GET {FEED_URL}"));
    let response = agent
        .get(FEED_URL)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/json")
        .set("Cache-Control", "no-cache")
        .call()
        .with_context(|| format!("GET {FEED_URL}"))?;
    let status = response.status();
    let body = response
        .into_string()
        .context("read Codex reset feed body")?;
    crate::logger::info(format!(
        "Codex reset feed response: status={status}, bytes={}",
        body.len()
    ));
    if status / 100 != 2 {
        bail!("Codex reset feed returned {status}: {body}");
    }
    let (resets, legacy_source_fallbacks) = parse_feed_with_stats(&body, Utc::now())?;
    let details = resets
        .iter()
        .map(|reset| {
            format!(
                "{}@{}<{}>",
                reset.id,
                reset.reset_at,
                reset.source_url.as_deref().unwrap_or("<none>")
            )
        })
        .collect::<Vec<_>>();
    crate::logger::info(format!(
        "Codex reset feed result: accepted_forced_resets={}, legacy_source_fallbacks={}, entries=[{}]",
        resets.len(),
        legacy_source_fallbacks,
        details.join(", ")
    ));
    Ok(resets)
}

fn http_agent() -> Agent {
    let tls = ureq::native_tls::TlsConnector::new().expect("create native-tls connector");
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(15))
        .timeout_write(Duration::from_secs(15))
        .tls_connector(Arc::new(tls))
        .build()
}

fn load_cache(path: &Path, now: DateTime<Utc>) -> ResetFeedCache {
    let cache = fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<ResetFeedCache>(&raw).ok())
        .unwrap_or_default();
    let mut resets = cache.resets;
    let mut ids = HashSet::new();
    resets.retain(|reset| {
        reset.reset_at > now && !reset.id.trim().is_empty() && ids.insert(reset.id.clone())
    });
    sort_resets(&mut resets);
    ResetFeedCache {
        schema_version: FEED_SCHEMA_VERSION,
        resets,
        notified_ids: cache.notified_ids,
    }
}

fn save_cache(path: &Path, cache: &ResetFeedCache) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let content = serde_json::to_vec_pretty(cache).context("serialize Codex reset feed cache")?;
    fs::write(path, content).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn send_snapshot(events: &mpsc::Sender<WorkerEvent>, cache: &ResetFeedCache) {
    let _ = events.send(WorkerEvent::ForcedResetsUpdated(ResetFeedSnapshot {
        resets: cache.resets.clone(),
        notified_ids: cache.notified_ids.clone(),
    }));
}

fn sort_resets(resets: &mut [ForcedReset]) {
    resets.sort_by(|left, right| {
        left.reset_at
            .cmp(&right.reset_at)
            .then_with(|| left.id.cmp(&right.id))
    });
}

pub(crate) fn is_valid_source_url(url: &str) -> bool {
    url.trim().to_ascii_lowercase().starts_with("https://")
}

#[cfg(test)]
mod tests {
    use crate::settings::ResetAnnouncementRefreshInterval;
    use chrono::TimeZone;

    use super::*;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 12, 12, 0, 0).unwrap()
    }

    #[test]
    fn parses_forced_resets_and_discards_banked_resets() {
        let resets = parse_feed(
            r#"{
                "schema_version": 1,
                "resets": [
                    {"id":"banked-1","type":"banked","source_url":"https://x.com/tibo/status/1","reset_at":"2026-09-13T12:00:00Z"},
                    {"id":"forced-2","type":"forced","source_url":"https://x.com/tibo/status/2","label":"Weekly quota","reset_at":"2026-09-14T12:00:00Z"},
                    {"id":"forced-1","type":"forced","reset_at":"2026-09-13T12:00:00Z"}
                ]
            }"#,
            now(),
        )
        .unwrap();

        assert_eq!(resets.len(), 2);
        assert_eq!(resets[0].id, "forced-1");
        assert_eq!(resets[0].source_url, None);
        assert_eq!(resets[1].label.as_deref(), Some("Weekly quota"));
    }

    #[test]
    fn ignores_duplicate_empty_and_past_forced_entries() {
        let resets = parse_feed(
            r#"{
                "schema_version": 2,
                "resets": [
                    {"id":"same","type":"forced","source_url":"https://x.com/tibo/status/1","reset_at":"2026-09-11T12:00:00Z"},
                    {"id":"same","type":"forced","source_url":"https://x.com/tibo/status/1","reset_at":"2026-09-13T12:00:00Z"},
                    {"id":"same","type":"forced","source_url":"https://x.com/tibo/status/1","reset_at":"2026-09-14T12:00:00Z"},
                    {"id":" ","type":"forced","source_url":"https://x.com/tibo/status/1","reset_at":"2026-09-15T12:00:00Z"}
                ]
            }"#,
            now(),
        )
        .unwrap();

        assert_eq!(resets.len(), 1);
        assert_eq!(resets[0].reset_at, now() + chrono::Duration::days(1));
    }

    #[test]
    fn rejects_unknown_schema_versions() {
        let error = parse_feed(r#"{"schema_version":3,"resets":[]}"#, now()).unwrap_err();
        assert!(error.to_string().contains("unsupported"));
    }

    #[test]
    fn keeps_entries_without_a_verifiable_https_source() {
        let resets = parse_feed(
            r#"{
                "schema_version": 2,
                "resets": [
                    {"id":"http","type":"forced","source_url":"http://example.com","reset_at":"2026-09-13T12:00:00Z"},
                    {"id":"missing","type":"forced","reset_at":"2026-09-14T12:00:00Z"},
                    {"id":"valid","type":"forced","source_url":"https://example.com/status","reset_at":"2026-09-15T12:00:00Z"}
                ]
            }"#,
            now(),
        )
        .unwrap();

        assert_eq!(
            resets
                .iter()
                .map(|reset| reset.id.as_str())
                .collect::<Vec<_>>(),
            ["http", "missing", "valid"]
        );
        assert_eq!(resets[0].source_url, None);
        assert_eq!(resets[1].source_url, None);
        assert_eq!(
            resets[2].source_url.as_deref(),
            Some("https://example.com/status")
        );
    }

    #[test]
    fn accepts_a_legacy_entry_without_a_source_url() {
        let resets = parse_feed(
            r#"{
                "schema_version": 1,
                "resets": [
                    {"id":"legacy","type":"forced","reset_at":"2026-09-13T12:00:00Z"}
                ]
            }"#,
            now(),
        )
        .unwrap();

        assert_eq!(resets.len(), 1);
        assert_eq!(resets[0].source_url, None);
    }

    #[test]
    fn refresh_interval_has_a_one_hour_default() {
        assert_eq!(
            ResetAnnouncementRefreshInterval::default().seconds(),
            60 * 60
        );
    }

    #[test]
    fn client_cache_remembers_received_resets_and_sent_notification_ids() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("codex-resets-cache.json");
        let reset = ForcedReset {
            id: "forced-1".into(),
            source_url: Some("https://x.com/tibo/status/1".into()),
            label: Some("Weekly quota".into()),
            reset_at: now() + chrono::Duration::days(1),
        };
        let cache = ResetFeedCache {
            schema_version: FEED_SCHEMA_VERSION,
            resets: vec![reset.clone()],
            notified_ids: vec!["forced-0".into()],
        };

        save_cache(&path, &cache).unwrap();
        let loaded = load_cache(&path, now());

        assert_eq!(loaded.resets, vec![reset]);
        assert_eq!(loaded.notified_ids, vec!["forced-0"]);
    }
}
