//! Public GitHub feed for announced Codex resets.
//!
//! The feed is intentionally a tiny, append/replace-friendly JSON document.
//! A bot can update it with one commit and clients can keep using the last
//! valid cache when GitHub or the raw-content CDN is temporarily unavailable.
//!
//! The client cache also remembers announcements after the bot removes them,
//! together with the per-instance reset detection history (see
//! [`crate::reset_detection`]), so a reset that never arrived stays visible
//! and a reset that did arrive is recognized across restarts.

use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::mpsc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ureq::Agent;

use crate::{
    reset_detection::{Observation, Track},
    settings::Settings,
    worker::WorkerEvent,
};

pub const FEED_URL: &str =
    "https://raw.githubusercontent.com/vertopolkaLF/codex-minibar/main/data/codex-resets.json";
const FEED_SCHEMA_VERSION: u8 = 2;
const LEGACY_FEED_SCHEMA_VERSION: u8 = 1;
const USER_AGENT: &str = "codex-minibar-reset-feed";
const MAX_FEED_ENTRIES: usize = 32;
const MAX_NOTIFIED_IDS: usize = 256;
/// Past announcements stay cached this long so a reset that never arrived can
/// still be shown and late detections can still match it.
const PAST_RETENTION: chrono::Duration = chrono::Duration::days(7);
/// Detection history of instances that stopped reporting is dropped after this.
const TRACK_RETENTION: chrono::Duration = chrono::Duration::days(8);
/// Unchanged detection history is written at most this often; it is also
/// flushed on shutdown.
const TRACK_SAVE_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResetKind {
    /// Everyone's limits are reset.
    #[default]
    Forced,
    /// Everyone receives a banked reset credit to spend later.
    Banked,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResetTiming {
    /// Lands at `reset_at`.
    #[default]
    Exact,
    /// Lands at some point before `reset_at`, which is only a deadline.
    Before,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnnouncedReset {
    /// Stable bot-generated identity. It must not change when the description
    /// or the timestamp is corrected.
    pub id: String,
    #[serde(default)]
    pub kind: ResetKind,
    #[serde(default)]
    pub timing: ResetTiming,
    /// Optional HTTPS page where the announcement can be verified by the user.
    #[serde(default)]
    pub source_url: Option<String>,
    /// Optional human-readable context, for example "weekly quota".
    #[serde(default)]
    pub label: Option<String>,
    /// ISO-8601 timestamp in UTC. A deadline for [`ResetTiming::Before`].
    pub reset_at: DateTime<Utc>,
    /// Optional start of the landing window of a [`ResetTiming::Before`]
    /// announcement.
    #[serde(default)]
    pub not_before: Option<DateTime<Utc>>,
    /// Instance id -> when this client detected the reset on that account.
    #[serde(default)]
    pub received: BTreeMap<String, DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResetFeedSnapshot {
    pub resets: Vec<AnnouncedReset>,
    pub notified_ids: Vec<String>,
}

#[derive(Debug)]
pub enum ResetFeedCommand {
    SetEnabled(bool),
    SetRefreshInterval(Duration),
    MarkNotified(String),
    /// A fresh Codex quota sample of one instance.
    Observe(String, Box<Observation>),
    /// A reset credit was spent from this app.
    RecordBankedUse(String, DateTime<Utc>),
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
    resets: Vec<AnnouncedReset>,
    #[serde(default)]
    notified_ids: Vec<String>,
    /// Instance id -> reset detection history.
    #[serde(default)]
    tracks: BTreeMap<String, Track>,
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
    timing: ResetTiming,
    #[serde(default)]
    source_url: Option<String>,
    #[serde(default)]
    label: Option<String>,
    reset_at: DateTime<Utc>,
    #[serde(default)]
    not_before: Option<DateTime<Utc>>,
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

pub fn parse_feed(body: &str, now: DateTime<Utc>) -> Result<Vec<AnnouncedReset>> {
    parse_feed_with_stats(body, now).map(|(resets, _)| resets)
}

fn parse_feed_with_stats(body: &str, now: DateTime<Utc>) -> Result<(Vec<AnnouncedReset>, usize)> {
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
        // Recent past entries are kept: the reset may not have arrived yet.
        if id.is_empty() || !is_retained(entry.reset_at, now) || !ids.insert(id.clone()) {
            continue;
        }
        let label = entry
            .label
            .map(|label| label.trim().chars().take(120).collect::<String>())
            .filter(|label| !label.is_empty());
        resets.push(AnnouncedReset {
            id,
            kind: entry.kind,
            timing: entry.timing,
            source_url,
            label,
            reset_at: entry.reset_at,
            not_before: entry.not_before.filter(|at| *at <= entry.reset_at),
            received: BTreeMap::new(),
        });
    }
    sort_resets(&mut resets);
    Ok((resets, missing_source_urls))
}

fn is_retained(reset_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    now - reset_at <= PAST_RETENTION
}

/// Combines a fresh feed with the client cache. The feed is authoritative for
/// current and future announcements (a removed future entry was retracted),
/// while past entries the bot already cleared stay cached. Receipts always
/// carry over.
fn merge_feed(
    cached: &[AnnouncedReset],
    fetched: Vec<AnnouncedReset>,
    now: DateTime<Utc>,
) -> Vec<AnnouncedReset> {
    let mut merged = fetched;
    for reset in &mut merged {
        if let Some(previous) = cached.iter().find(|cached| cached.id == reset.id) {
            for (instance, at) in &previous.received {
                reset.received.entry(instance.clone()).or_insert(*at);
            }
        }
    }
    for previous in cached {
        if previous.reset_at <= now
            && is_retained(previous.reset_at, now)
            && !merged.iter().any(|reset| reset.id == previous.id)
        {
            merged.push(previous.clone());
        }
    }
    sort_resets(&mut merged);
    if merged.len() > MAX_FEED_ENTRIES {
        let excess = merged.len() - MAX_FEED_ENTRIES;
        merged.drain(..excess);
    }
    merged
}

struct Worker {
    enabled: bool,
    cache_path: PathBuf,
    cache: ResetFeedCache,
    events: mpsc::Sender<WorkerEvent>,
    tracks_dirty: bool,
    last_save: Instant,
}

impl Worker {
    fn save(&mut self) {
        match save_cache(&self.cache_path, &self.cache) {
            Ok(()) => {
                self.tracks_dirty = false;
                self.last_save = Instant::now();
            }
            Err(error) => eprintln!("failed to cache Codex reset feed: {error:#}"),
        }
    }

    fn publish(&self) {
        if self.enabled {
            send_snapshot(&self.events, &self.cache);
        } else {
            // Keep the cache on disk for a fast re-enable, but make the
            // live surface empty immediately while this option is off.
            let mut hidden = self.cache.clone();
            hidden.resets.clear();
            send_snapshot(&self.events, &hidden);
        }
    }

    /// Re-evaluates receipts after the announcements or the history changed.
    fn detect(&mut self) -> bool {
        let changed =
            crate::reset_detection::apply_receipts(&mut self.cache.resets, &self.cache.tracks);
        if changed {
            let received = self
                .cache
                .resets
                .iter()
                .map(|reset| format!("{}:{:?}", reset.id, reset.received))
                .collect::<Vec<_>>();
            crate::logger::info(format!(
                "Codex reset detection updated receipts: [{}]",
                received.join(", ")
            ));
        }
        changed
    }

    fn history_changed(&mut self) {
        if self.detect() {
            self.save();
            self.publish();
        } else {
            self.tracks_dirty = true;
            if self.last_save.elapsed() >= TRACK_SAVE_INTERVAL {
                self.save();
            }
        }
    }
}

fn run(
    enabled: bool,
    mut refresh_interval: Duration,
    cache_path: PathBuf,
    commands: mpsc::Receiver<ResetFeedCommand>,
    events: mpsc::Sender<WorkerEvent>,
) {
    let cache = load_cache(&cache_path, Utc::now());
    crate::logger::info(format!(
        "Codex reset feed starting: enabled={enabled}, interval={}s, cache={}, cached_resets={}, notified_ids={}, tracks={}",
        refresh_interval.as_secs(),
        cache_path.display(),
        cache.resets.len(),
        cache.notified_ids.len(),
        cache.tracks.len(),
    ));
    let mut worker = Worker {
        enabled,
        cache_path,
        cache,
        events,
        tracks_dirty: false,
        last_save: Instant::now(),
    };
    if worker.enabled {
        // Publish the client-side snapshot before touching the network. This
        // is what makes received resets survive an app restart or an offline
        // launch instead of blinking out until GitHub answers.
        worker.publish();
    }
    let agent = http_agent();
    // The first remote refresh is deliberately due immediately on launch.
    let mut next_refresh = Instant::now();

    loop {
        if worker.enabled && next_refresh <= Instant::now() {
            match fetch(&agent) {
                Ok(resets) => {
                    worker.cache.resets = merge_feed(&worker.cache.resets, resets, Utc::now());
                    worker.detect();
                    worker.save();
                    crate::logger::info(format!(
                        "Codex reset feed cache updated: cached_resets={}, notified_ids={}",
                        worker.cache.resets.len(),
                        worker.cache.notified_ids.len(),
                    ));
                    worker.publish();
                }
                Err(error) => {
                    crate::logger::info(format!("Codex reset feed refresh failed: {error:#}"));
                    let _ = worker
                        .events
                        .send(WorkerEvent::ForcedResetsRefreshFailed(error.to_string()));
                }
            }
            next_refresh = Instant::now() + refresh_interval;
            continue;
        }

        let wait = if worker.enabled {
            next_refresh.saturating_duration_since(Instant::now())
        } else {
            Duration::from_secs(24 * 60 * 60)
        };
        match commands.recv_timeout(wait) {
            Ok(ResetFeedCommand::SetEnabled(value)) => {
                if worker.enabled == value {
                    continue;
                }
                worker.enabled = value;
                worker.publish();
                if value {
                    next_refresh = Instant::now();
                }
            }
            Ok(ResetFeedCommand::SetRefreshInterval(value)) => {
                refresh_interval = value.max(Duration::from_secs(60));
                if worker.enabled {
                    next_refresh = Instant::now() + refresh_interval;
                }
            }
            Ok(ResetFeedCommand::MarkNotified(id)) => {
                let notified = &mut worker.cache.notified_ids;
                if !id.trim().is_empty() && !notified.iter().any(|known| known == &id) {
                    notified.push(id);
                    if notified.len() > MAX_NOTIFIED_IDS {
                        let excess = notified.len() - MAX_NOTIFIED_IDS;
                        notified.drain(..excess);
                    }
                    worker.save();
                }
            }
            Ok(ResetFeedCommand::Observe(instance, observation)) => {
                if worker
                    .cache
                    .tracks
                    .entry(instance)
                    .or_default()
                    .observe(*observation)
                {
                    worker.history_changed();
                }
            }
            Ok(ResetFeedCommand::RecordBankedUse(instance, at)) => {
                worker
                    .cache
                    .tracks
                    .entry(instance)
                    .or_default()
                    .record_banked_use(at);
                worker.history_changed();
            }
            Ok(ResetFeedCommand::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                if worker.tracks_dirty {
                    worker.save();
                }
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

fn fetch(agent: &Agent) -> Result<Vec<AnnouncedReset>> {
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
                "{}:{:?}/{:?}@{}<{}>",
                reset.id,
                reset.kind,
                reset.timing,
                reset.reset_at,
                reset.source_url.as_deref().unwrap_or("<none>")
            )
        })
        .collect::<Vec<_>>();
    crate::logger::info(format!(
        "Codex reset feed result: accepted_resets={}, legacy_source_fallbacks={}, entries=[{}]",
        resets.len(),
        legacy_source_fallbacks,
        details.join(", ")
    ));
    Ok(resets)
}

fn http_agent() -> Agent {
    crate::http::public_agent_builder()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(15))
        .timeout_write(Duration::from_secs(15))
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
        is_retained(reset.reset_at, now)
            && !reset.id.trim().is_empty()
            && ids.insert(reset.id.clone())
    });
    sort_resets(&mut resets);
    let mut tracks = cache.tracks;
    tracks.retain(|_, track| {
        track
            .last_sampled_at()
            .is_some_and(|at| now - at <= TRACK_RETENTION)
    });
    ResetFeedCache {
        schema_version: FEED_SCHEMA_VERSION,
        resets,
        notified_ids: cache.notified_ids,
        tracks,
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

fn sort_resets(resets: &mut [AnnouncedReset]) {
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
    use chrono::TimeZone;

    use super::*;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 12, 12, 0, 0).unwrap()
    }

    fn reset(id: &str, reset_at: DateTime<Utc>) -> AnnouncedReset {
        AnnouncedReset {
            id: id.into(),
            kind: ResetKind::Forced,
            timing: ResetTiming::Exact,
            source_url: None,
            label: None,
            reset_at,
            not_before: None,
            received: BTreeMap::new(),
        }
    }

    #[test]
    fn parses_forced_and_banked_resets_with_timing() {
        let resets = parse_feed(
            r#"{
                "schema_version": 1,
                "resets": [
                    {"id":"banked-1","type":"banked","source_url":"https://x.com/tibo/status/1","reset_at":"2026-09-13T12:00:00Z"},
                    {"id":"forced-2","type":"forced","timing":"before","not_before":"2026-09-14T00:00:00Z","source_url":"https://x.com/tibo/status/2","label":"Weekly quota","reset_at":"2026-09-14T12:00:00Z"},
                    {"id":"forced-1","type":"forced","reset_at":"2026-09-13T11:00:00Z"}
                ]
            }"#,
            now(),
        )
        .unwrap();

        assert_eq!(
            resets.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["forced-1", "banked-1", "forced-2"]
        );
        assert_eq!(resets[0].source_url, None);
        assert_eq!(resets[0].timing, ResetTiming::Exact);
        assert_eq!(resets[1].kind, ResetKind::Banked);
        assert_eq!(resets[2].timing, ResetTiming::Before);
        assert_eq!(
            resets[2].not_before,
            Some(Utc.with_ymd_and_hms(2026, 9, 14, 0, 0, 0).unwrap())
        );
        assert_eq!(resets[2].label.as_deref(), Some("Weekly quota"));
    }

    #[test]
    fn keeps_recent_past_entries_and_drops_duplicates_empty_and_old_ones() {
        let resets = parse_feed(
            r#"{
                "schema_version": 2,
                "resets": [
                    {"id":"old","type":"forced","reset_at":"2026-09-01T12:00:00Z"},
                    {"id":"recent","type":"forced","reset_at":"2026-09-11T12:00:00Z"},
                    {"id":"same","type":"forced","reset_at":"2026-09-13T12:00:00Z"},
                    {"id":"same","type":"forced","reset_at":"2026-09-14T12:00:00Z"},
                    {"id":" ","type":"forced","reset_at":"2026-09-15T12:00:00Z"}
                ]
            }"#,
            now(),
        )
        .unwrap();

        assert_eq!(
            resets.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["recent", "same"]
        );
        assert_eq!(resets[1].reset_at, now() + chrono::Duration::days(1));
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
    fn merge_keeps_cleared_past_entries_drops_retracted_future_ones_and_carries_receipts() {
        let mut received = reset("received", now() - chrono::Duration::hours(2));
        received
            .received
            .insert("codex".into(), now() - chrono::Duration::hours(1));
        let cached = vec![
            received.clone(),
            reset("missed", now() - chrono::Duration::hours(1)),
            reset("retracted", now() + chrono::Duration::hours(1)),
        ];
        let mut refetched = received.clone();
        refetched.received.clear();
        let fetched = vec![refetched, reset("new", now() + chrono::Duration::hours(3))];

        let merged = merge_feed(&cached, fetched, now());

        assert_eq!(
            merged.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["received", "missed", "new"]
        );
        assert_eq!(merged[0].received, received.received);
    }

    #[test]
    fn client_cache_remembers_resets_notifications_and_detection_history() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("codex-resets-cache.json");
        let mut announced = reset("forced-1", now() - chrono::Duration::days(1));
        announced.source_url = Some("https://x.com/tibo/status/1".into());
        announced
            .received
            .insert("codex".into(), now() - chrono::Duration::hours(20));
        let mut track = Track::default();
        track.observe(Observation {
            sampled_at: now() - chrono::Duration::minutes(5),
            account: None,
            session: None,
            weekly: Some(crate::reset_detection::WindowSample {
                used: 12,
                resets_at: Some(now() + chrono::Duration::days(3)),
                unactivated: false,
            }),
            banked: None,
        });
        let cache = ResetFeedCache {
            schema_version: FEED_SCHEMA_VERSION,
            resets: vec![
                announced.clone(),
                reset("expired", now() - chrono::Duration::days(8)),
            ],
            notified_ids: vec!["forced-0".into()],
            tracks: BTreeMap::from([
                ("codex".to_owned(), track.clone()),
                ("stale".to_owned(), Track::default()),
            ]),
        };

        save_cache(&path, &cache).unwrap();
        let loaded = load_cache(&path, now());

        assert_eq!(loaded.resets, vec![announced]);
        assert_eq!(loaded.notified_ids, vec!["forced-0"]);
        assert_eq!(loaded.tracks, BTreeMap::from([("codex".to_owned(), track)]));
    }
}
