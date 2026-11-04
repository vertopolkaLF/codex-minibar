//! Public status pages of providers.
//!
//! Ongoing incidents explain failures that are not the user's fault ("Codex
//! is down", not "Minibar is broken"). Only components that matter for the
//! provider's coding tools count: a ChatGPT voice outage must not mark Codex
//! as degraded. Incidents are never cached across launches, so a resolved
//! incident cannot linger offline; only sent-notification ids are persisted.

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ureq::Agent;

use crate::{
    settings::{ProviderKind, Settings},
    worker::WorkerEvent,
};

const USER_AGENT: &str = "codex-minibar-provider-status";
const REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// A failed refresh keeps the last answer only this long, so an outage of the
/// status page itself neither hides a real incident nor keeps a stale one.
const STALE_AFTER: Duration = Duration::from_secs(30 * 60);
const MAX_INCIDENTS: usize = 8;
const MAX_NOTIFIED_IDS: usize = 256;

/// Severity of an incident for the provider's relevant components.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum IncidentImpact {
    Maintenance,
    Degraded,
    PartialOutage,
    MajorOutage,
}

impl IncidentImpact {
    pub fn label(self) -> &'static str {
        match self {
            Self::Maintenance => crate::i18n::tr("status-impact-maintenance"),
            Self::Degraded => crate::i18n::tr("status-impact-degraded"),
            Self::PartialOutage => crate::i18n::tr("status-impact-partial-outage"),
            Self::MajorOutage => crate::i18n::tr("status-impact-major-outage"),
        }
    }

    fn from_component_status(status: &str) -> Option<Self> {
        match status {
            "degraded_performance" => Some(Self::Degraded),
            "partial_outage" => Some(Self::PartialOutage),
            "major_outage" | "full_outage" => Some(Self::MajorOutage),
            "under_maintenance" => Some(Self::Maintenance),
            _ => None,
        }
    }

    fn from_statuspage_impact(impact: &str) -> Self {
        match impact {
            "major" => Self::PartialOutage,
            "critical" => Self::MajorOutage,
            "maintenance" => Self::Maintenance,
            _ => Self::Degraded,
        }
    }
}

/// Progress reported by the status page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IncidentStage {
    Investigating,
    Identified,
    Monitoring,
    Maintenance,
}

impl IncidentStage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Investigating => crate::i18n::tr("status-stage-investigating"),
            Self::Identified => crate::i18n::tr("status-stage-identified"),
            Self::Monitoring => crate::i18n::tr("status-stage-monitoring"),
            Self::Maintenance => crate::i18n::tr("status-stage-in-progress"),
        }
    }

    /// `None` for finished or not yet started entries.
    fn parse(status: &str) -> Option<Self> {
        match status {
            "investigating" => Some(Self::Investigating),
            "identified" => Some(Self::Identified),
            "monitoring" => Some(Self::Monitoring),
            "in_progress" | "verifying" | "maintenance_in_progress" => Some(Self::Maintenance),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusIncident {
    /// Unique across status pages (`<page>:<incident id>`).
    pub id: String,
    pub title: String,
    pub impact: IncidentImpact,
    pub stage: IncidentStage,
    pub started_at: Option<DateTime<Utc>>,
    /// Names of the relevant affected components.
    pub components: Vec<String>,
    pub url: String,
}

impl StatusIncident {
    pub fn is_maintenance(&self) -> bool {
        self.stage == IncidentStage::Maintenance
    }
}

/// Ongoing incidents of one driver; empty when everything is operational.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderStatus {
    pub kind: ProviderKind,
    pub page_url: &'static str,
    pub incidents: Vec<StatusIncident>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderStatusSnapshot {
    pub statuses: Vec<ProviderStatus>,
    pub notified_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
enum Api {
    /// Atlassian Statuspage `api/v2/summary.json`.
    Statuspage(&'static str),
    /// incident.io's public page proxy. Its Statuspage-compatible endpoint
    /// omits component groups, which is where OpenAI keeps Codex.
    IncidentIo(&'static str),
}

#[derive(Clone, Copy, Debug)]
enum Relevance {
    /// Component (or its group) names containing one of these, ignoring case.
    Matching(&'static [&'static str]),
    /// Every component except these exact names.
    Except(&'static [&'static str]),
}

impl Relevance {
    fn matches(self, component: &str, group: Option<&str>) -> bool {
        match self {
            Self::Matching(needles) => [Some(component), group].into_iter().flatten().any(|name| {
                let name = name.to_lowercase();
                needles.iter().any(|needle| name.contains(needle))
            }),
            Self::Except(names) => !names
                .iter()
                .any(|name| name.eq_ignore_ascii_case(component.trim())),
        }
    }

    /// Statuspage incidents may list no components; only a page that is
    /// entirely relevant can attribute those to the provider.
    fn covers_unattributed(self) -> bool {
        matches!(self, Self::Except(_))
    }
}

#[derive(Clone, Copy, Debug)]
struct Source {
    kind: ProviderKind,
    page_url: &'static str,
    api: Api,
    relevance: Relevance,
}

const SOURCES: &[Source] = &[
    Source {
        kind: ProviderKind::Codex,
        page_url: "https://status.openai.com",
        api: Api::IncidentIo("https://status.openai.com/proxy/status.openai.com"),
        relevance: Relevance::Matching(&["codex"]),
    },
    Source {
        kind: ProviderKind::Claude,
        page_url: "https://status.claude.com",
        api: Api::Statuspage("https://status.claude.com/api/v2/summary.json"),
        relevance: Relevance::Matching(&["claude code", "claude api", "claude.ai"]),
    },
    Source {
        kind: ProviderKind::Cursor,
        page_url: "https://status.cursor.com",
        api: Api::Statuspage("https://status.cursor.com/api/v2/summary.json"),
        relevance: Relevance::Except(&["Grok Bot"]),
    },
];

/// Drivers with a public status page.
pub fn has_status_page(kind: ProviderKind) -> bool {
    SOURCES.iter().any(|source| source.kind == kind)
}

/// Drivers whose status pages are polled for these settings.
pub fn watched_kinds(settings: &Settings) -> Vec<ProviderKind> {
    if !settings.notifications.provider_status_enabled {
        return Vec::new();
    }
    let enabled = settings
        .instances
        .iter()
        .filter(|instance| instance.enabled)
        .map(|instance| instance.driver)
        .collect::<HashSet<_>>();
    SOURCES
        .iter()
        .map(|source| source.kind)
        .filter(|kind| enabled.contains(kind))
        .collect()
}

#[derive(Debug)]
pub enum ProviderStatusCommand {
    /// Drivers to watch; empty pauses polling and clears every status.
    Watch(Vec<ProviderKind>),
    MarkNotified(String),
    Shutdown,
}

pub struct ProviderStatusWorker {
    pub commands: mpsc::Sender<ProviderStatusCommand>,
}

impl ProviderStatusWorker {
    /// Does not wait: a slow status page must never delay quitting or an
    /// update restart, and the thread holds nothing that needs flushing.
    pub fn shutdown(self) {
        let _ = self.commands.send(ProviderStatusCommand::Shutdown);
    }
}

pub fn cache_path(settings_path: &Path) -> PathBuf {
    settings_path.with_file_name("provider-status-cache.json")
}

pub fn start_worker(
    settings: &Settings,
    cache_path: PathBuf,
    events: mpsc::Sender<WorkerEvent>,
) -> ProviderStatusWorker {
    let (commands, command_receiver) = mpsc::channel();
    let watched = watched_kinds(settings);
    thread::spawn(move || run(watched, cache_path, command_receiver, events));
    ProviderStatusWorker { commands }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct NotifiedCache {
    #[serde(default)]
    notified_ids: Vec<String>,
}

struct Known {
    status: ProviderStatus,
    fetched_at: Instant,
}

fn run(
    mut watched: Vec<ProviderKind>,
    cache_path: PathBuf,
    commands: mpsc::Receiver<ProviderStatusCommand>,
    events: mpsc::Sender<WorkerEvent>,
) {
    let mut cache = load_cache(&cache_path);
    let mut known = HashMap::<ProviderKind, Known>::new();
    let agent = http_agent();
    let mut next_refresh = Instant::now();
    crate::logger::info(format!(
        "Provider status starting: watched={watched:?}, notified_ids={}",
        cache.notified_ids.len()
    ));

    loop {
        if !watched.is_empty() && next_refresh <= Instant::now() {
            for source in SOURCES
                .iter()
                .filter(|source| watched.contains(&source.kind))
            {
                match fetch(&agent, source) {
                    Ok(status) => {
                        known.insert(
                            source.kind,
                            Known {
                                status,
                                fetched_at: Instant::now(),
                            },
                        );
                    }
                    Err(error) => {
                        crate::logger::info(format!(
                            "{} status refresh failed: {error:#}",
                            source.kind.display_name()
                        ));
                        if known
                            .get(&source.kind)
                            .is_some_and(|known| known.fetched_at.elapsed() > STALE_AFTER)
                        {
                            known.remove(&source.kind);
                        }
                    }
                }
            }
            send_snapshot(&events, &watched, &known, &cache);
            next_refresh = Instant::now() + REFRESH_INTERVAL;
            continue;
        }

        let wait = if watched.is_empty() {
            Duration::from_secs(24 * 60 * 60)
        } else {
            next_refresh.saturating_duration_since(Instant::now())
        };
        match commands.recv_timeout(wait) {
            Ok(ProviderStatusCommand::Watch(kinds)) => {
                if kinds == watched {
                    continue;
                }
                let added = kinds.iter().any(|kind| !watched.contains(kind));
                watched = kinds;
                known.retain(|kind, _| watched.contains(kind));
                // Removed drivers disappear at once; added ones are fetched
                // right away instead of waiting for the next tick.
                send_snapshot(&events, &watched, &known, &cache);
                if added {
                    next_refresh = Instant::now();
                }
            }
            Ok(ProviderStatusCommand::MarkNotified(id)) => {
                if !id.trim().is_empty() && !cache.notified_ids.contains(&id) {
                    cache.notified_ids.push(id);
                    if cache.notified_ids.len() > MAX_NOTIFIED_IDS {
                        let excess = cache.notified_ids.len() - MAX_NOTIFIED_IDS;
                        cache.notified_ids.drain(..excess);
                    }
                    if let Err(error) = save_cache(&cache_path, &cache) {
                        crate::logger::info(format!(
                            "failed to save provider status notification state: {error:#}"
                        ));
                    }
                }
            }
            Ok(ProviderStatusCommand::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

fn send_snapshot(
    events: &mpsc::Sender<WorkerEvent>,
    watched: &[ProviderKind],
    known: &HashMap<ProviderKind, Known>,
    cache: &NotifiedCache,
) {
    let statuses = SOURCES
        .iter()
        .filter(|source| watched.contains(&source.kind))
        .filter_map(|source| known.get(&source.kind))
        .map(|known| known.status.clone())
        .collect();
    let _ = events.send(WorkerEvent::ProviderStatusUpdated(ProviderStatusSnapshot {
        statuses,
        notified_ids: cache.notified_ids.clone(),
    }));
}

fn fetch(agent: &Agent, source: &Source) -> Result<ProviderStatus> {
    let url = match source.api {
        Api::Statuspage(url) | Api::IncidentIo(url) => url,
    };
    let response = agent
        .get(url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/json")
        .call()
        .with_context(|| format!("GET {url}"))?;
    let status = response.status();
    let body = response.into_string().context("read status page body")?;
    if status / 100 != 2 {
        bail!("status page returned {status}");
    }
    let incidents = match source.api {
        Api::Statuspage(_) => parse_statuspage(&body, source)?,
        Api::IncidentIo(_) => parse_incident_io(&body, source)?,
    };
    crate::logger::info(format!(
        "{} status: incidents=[{}]",
        source.kind.display_name(),
        incidents
            .iter()
            .map(|incident| format!("{}:{:?}", incident.id, incident.impact))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    Ok(ProviderStatus {
        kind: source.kind,
        page_url: source.page_url,
        incidents,
    })
}

#[derive(Debug, Deserialize)]
struct StatuspageSummary {
    #[serde(default)]
    components: Vec<StatuspageComponent>,
    #[serde(default)]
    incidents: Vec<StatuspageIncident>,
    #[serde(default)]
    scheduled_maintenances: Vec<StatuspageIncident>,
}

#[derive(Debug, Deserialize)]
struct StatuspageComponent {
    id: String,
    name: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    group: bool,
    #[serde(default)]
    group_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StatuspageIncident {
    id: String,
    name: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    impact: String,
    #[serde(default)]
    started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    scheduled_for: Option<DateTime<Utc>>,
    #[serde(default)]
    components: Vec<StatuspageComponent>,
}

fn parse_statuspage(body: &str, source: &Source) -> Result<Vec<StatusIncident>> {
    let summary: StatuspageSummary =
        serde_json::from_str(body).context("parse Statuspage summary")?;
    let groups = summary
        .components
        .iter()
        .filter(|component| component.group)
        .map(|component| (component.id.as_str(), component.name.as_str()))
        .collect::<HashMap<_, _>>();
    let mut incidents = Vec::new();
    for incident in summary
        .incidents
        .iter()
        .chain(&summary.scheduled_maintenances)
    {
        let Some(stage) = IncidentStage::parse(&incident.status) else {
            continue;
        };
        let relevant = incident
            .components
            .iter()
            .filter(|component| !component.group)
            .filter(|component| {
                let group = component
                    .group_id
                    .as_deref()
                    .and_then(|id| groups.get(id).copied());
                source.relevance.matches(&component.name, group)
            })
            .collect::<Vec<_>>();
        if relevant.is_empty()
            && !(incident.components.is_empty() && source.relevance.covers_unattributed())
        {
            continue;
        }
        let impact = if stage == IncidentStage::Maintenance {
            IncidentImpact::Maintenance
        } else {
            relevant
                .iter()
                .filter_map(|component| IncidentImpact::from_component_status(&component.status))
                .filter(|impact| *impact != IncidentImpact::Maintenance)
                .max()
                .unwrap_or_else(|| IncidentImpact::from_statuspage_impact(&incident.impact))
        };
        incidents.push(StatusIncident {
            id: format!("{}:{}", source.kind.display_name(), incident.id),
            title: clean_title(&incident.name),
            impact,
            stage,
            started_at: incident
                .started_at
                .or(incident.scheduled_for)
                .or(incident.created_at),
            components: relevant
                .iter()
                .map(|component| component.name.clone())
                .collect(),
            url: format!("{}/incidents/{}", source.page_url, incident.id),
        });
    }
    Ok(finish(incidents))
}

#[derive(Debug, Deserialize)]
struct IncidentIoEnvelope {
    summary: IncidentIoSummary,
}

#[derive(Debug, Deserialize)]
struct IncidentIoSummary {
    #[serde(default)]
    components: Vec<IncidentIoComponent>,
    #[serde(default)]
    structure: Option<IncidentIoStructure>,
    #[serde(default)]
    ongoing_incidents: Vec<IncidentIoIncident>,
}

#[derive(Debug, Deserialize)]
struct IncidentIoComponent {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct IncidentIoStructure {
    #[serde(default)]
    items: Vec<IncidentIoItem>,
}

#[derive(Debug, Deserialize)]
struct IncidentIoItem {
    #[serde(default)]
    group: Option<IncidentIoGroup>,
}

#[derive(Debug, Deserialize)]
struct IncidentIoGroup {
    name: String,
    #[serde(default)]
    components: Vec<IncidentIoGroupComponent>,
}

#[derive(Debug, Deserialize)]
struct IncidentIoGroupComponent {
    component_id: String,
}

#[derive(Debug, Deserialize)]
struct IncidentIoIncident {
    id: String,
    name: String,
    #[serde(default)]
    status: String,
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    published_at: Option<DateTime<Utc>>,
    #[serde(default)]
    affected_components: Vec<IncidentIoAffected>,
}

#[derive(Debug, Deserialize)]
struct IncidentIoAffected {
    component_id: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    current_status: Option<String>,
}

fn parse_incident_io(body: &str, source: &Source) -> Result<Vec<StatusIncident>> {
    let envelope: IncidentIoEnvelope =
        serde_json::from_str(body).context("parse incident.io status summary")?;
    let summary = envelope.summary;
    let names = summary
        .components
        .iter()
        .map(|component| (component.id.as_str(), component.name.as_str()))
        .collect::<HashMap<_, _>>();
    let groups = summary
        .structure
        .iter()
        .flat_map(|structure| &structure.items)
        .filter_map(|item| item.group.as_ref())
        .flat_map(|group| {
            group
                .components
                .iter()
                .map(move |component| (component.component_id.as_str(), group.name.as_str()))
        })
        .collect::<HashMap<_, _>>();
    let mut incidents = Vec::new();
    for incident in &summary.ongoing_incidents {
        let maintenance = incident.kind == "maintenance";
        let stage = if maintenance {
            IncidentStage::Maintenance
        } else {
            match IncidentStage::parse(&incident.status) {
                Some(stage) => stage,
                None => continue,
            }
        };
        let mut components = Vec::new();
        let mut worst = None;
        for affected in &incident.affected_components {
            let Some(name) = names.get(affected.component_id.as_str()) else {
                continue;
            };
            let group = groups.get(affected.component_id.as_str()).copied();
            if !source.relevance.matches(name, group) {
                continue;
            }
            components.push((*name).to_owned());
            let status = affected
                .current_status
                .as_deref()
                .unwrap_or(&affected.status);
            worst = worst.max(IncidentImpact::from_component_status(status));
        }
        if components.is_empty() {
            continue;
        }
        let impact = if maintenance {
            IncidentImpact::Maintenance
        } else {
            worst
                .filter(|impact| *impact != IncidentImpact::Maintenance)
                .unwrap_or(IncidentImpact::Degraded)
        };
        incidents.push(StatusIncident {
            id: format!("{}:{}", source.kind.display_name(), incident.id),
            title: clean_title(&incident.name),
            impact,
            stage,
            started_at: incident.published_at,
            components,
            url: format!("{}/incidents/{}", source.page_url, incident.id),
        });
    }
    Ok(finish(incidents))
}

fn clean_title(title: &str) -> String {
    title.trim().chars().take(160).collect()
}

/// Most severe first, then most recent.
fn finish(mut incidents: Vec<StatusIncident>) -> Vec<StatusIncident> {
    let mut ids = HashSet::new();
    incidents.retain(|incident| ids.insert(incident.id.clone()));
    incidents.sort_by(|left, right| {
        right
            .impact
            .cmp(&left.impact)
            .then_with(|| right.started_at.cmp(&left.started_at))
    });
    incidents.truncate(MAX_INCIDENTS);
    incidents
}

fn http_agent() -> Agent {
    crate::http::public_agent_builder()
        .timeout_connect(Duration::from_secs(10))
        // OpenAI's page answers slowly under load.
        .timeout_read(Duration::from_secs(30))
        .timeout_write(Duration::from_secs(15))
        .build()
}

fn load_cache(path: &Path) -> NotifiedCache {
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_cache(path: &Path, cache: &NotifiedCache) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let content = serde_json::to_vec_pretty(cache).context("serialize provider status cache")?;
    fs::write(path, content).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(kind: ProviderKind) -> &'static Source {
        SOURCES.iter().find(|source| source.kind == kind).unwrap()
    }

    #[test]
    fn statuspage_keeps_only_relevant_components() {
        let body = r#"{
            "components": [],
            "incidents": [
                {"id":"console","name":"Elevated errors on platform.claude.com","status":"monitoring","impact":"major",
                 "started_at":"2026-10-12T10:00:00Z",
                 "components":[{"id":"a","name":"Claude Console (platform.claude.com)","status":"degraded_performance"}]},
                {"id":"code","name":"Claude Code login failures","status":"investigating","impact":"major",
                 "started_at":"2026-10-12T11:00:00Z",
                 "components":[{"id":"b","name":"Claude Code","status":"partial_outage"},
                               {"id":"c","name":"Claude Cowork","status":"major_outage"}]},
                {"id":"old","name":"Resolved","status":"resolved","impact":"minor",
                 "components":[{"id":"b","name":"Claude Code","status":"operational"}]}
            ],
            "scheduled_maintenances": []
        }"#;
        let incidents = parse_statuspage(body, source(ProviderKind::Claude)).unwrap();
        assert_eq!(incidents.len(), 1);
        let incident = &incidents[0];
        assert_eq!(incident.id, "Claude:code");
        assert_eq!(incident.impact, IncidentImpact::PartialOutage);
        assert_eq!(incident.stage, IncidentStage::Investigating);
        assert_eq!(incident.components, ["Claude Code"]);
        assert_eq!(incident.url, "https://status.claude.com/incidents/code");
    }

    #[test]
    fn statuspage_attributes_componentless_incidents_only_to_whole_pages() {
        let body = r#"{
            "incidents": [
                {"id":"x","name":"Slow requests","status":"identified","impact":"minor","components":[]}
            ]
        }"#;
        assert_eq!(
            parse_statuspage(body, source(ProviderKind::Cursor))
                .unwrap()
                .len(),
            1
        );
        assert!(
            parse_statuspage(body, source(ProviderKind::Claude))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn statuspage_reports_in_progress_maintenance_but_not_scheduled() {
        let body = r#"{
            "scheduled_maintenances": [
                {"id":"m1","name":"Database upgrade","status":"in_progress","impact":"maintenance",
                 "scheduled_for":"2026-10-12T09:00:00Z",
                 "components":[{"id":"i","name":"IDE","status":"under_maintenance"}]},
                {"id":"m2","name":"Later","status":"scheduled","impact":"maintenance",
                 "components":[{"id":"i","name":"IDE","status":"operational"}]}
            ]
        }"#;
        let incidents = parse_statuspage(body, source(ProviderKind::Cursor)).unwrap();
        assert_eq!(incidents.len(), 1);
        assert!(incidents[0].is_maintenance());
        assert_eq!(incidents[0].impact, IncidentImpact::Maintenance);
    }

    #[test]
    fn incident_io_matches_components_by_their_group() {
        let body = r#"{"summary":{
            "components":[
                {"id":"cli","name":"CLI"},
                {"id":"voice","name":"Voice mode"},
                {"id":"desktop","name":"Codex in ChatGPT Desktop"}
            ],
            "structure":{"items":[
                {"group":{"name":"Codex","components":[{"component_id":"cli"}]}},
                {"group":{"name":"ChatGPT","components":[{"component_id":"voice"},{"component_id":"desktop"}]}}
            ]},
            "ongoing_incidents":[
                {"id":"i1","name":"Voice is down","status":"investigating","type":"incident",
                 "affected_components":[{"component_id":"voice","status":"full_outage"}]},
                {"id":"i2","name":"Codex CLI errors","status":"identified","type":"incident",
                 "published_at":"2026-10-12T12:00:00Z",
                 "affected_components":[{"component_id":"cli","status":"degraded_performance","current_status":"partial_outage"},
                                        {"component_id":"desktop","status":"degraded_performance"}]}
            ]
        }}"#;
        let incidents = parse_incident_io(body, source(ProviderKind::Codex)).unwrap();
        assert_eq!(incidents.len(), 1);
        assert_eq!(incidents[0].id, "Codex:i2");
        assert_eq!(incidents[0].impact, IncidentImpact::PartialOutage);
        assert_eq!(incidents[0].components, ["CLI", "Codex in ChatGPT Desktop"]);
        assert_eq!(incidents[0].url, "https://status.openai.com/incidents/i2");
    }

    #[test]
    fn incidents_sort_by_severity_then_recency() {
        let incident = |id: &str, impact, hour| StatusIncident {
            id: id.into(),
            title: id.into(),
            impact,
            stage: IncidentStage::Investigating,
            started_at: Some(
                DateTime::parse_from_rfc3339(&format!("2026-10-12T{hour:02}:00:00Z"))
                    .unwrap()
                    .with_timezone(&Utc),
            ),
            components: Vec::new(),
            url: String::new(),
        };
        let sorted = finish(vec![
            incident("a", IncidentImpact::Degraded, 9),
            incident("b", IncidentImpact::MajorOutage, 8),
            incident("c", IncidentImpact::Degraded, 10),
        ]);
        assert_eq!(
            sorted.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            ["b", "c", "a"]
        );
    }
}
