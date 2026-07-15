use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    process::{Child, Command, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use directories::BaseDirs;
use serde::Deserialize;
use serde_json::Value;

use crate::{
    limits::{AdditionalLimit, LimitWindow, RateLimits},
    usage,
    worker::{Activator, LimitProvider, UsageProvider},
};

const OAUTH_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const OAUTH_BETA: &str = "oauth-2025-04-20";
const FALLBACK_CLAUDE_CODE_VERSION: &str = "2.1.0";
pub const ACTIVATION_MODEL: &str = "haiku";
pub const ACTIVATION_PROMPT: &str = "reply with letter a";

/// Starts Claude Code's five-hour window with the smallest supported prompt.
pub struct ClaudeActivator {
    timeout: Duration,
}

impl ClaudeActivator {
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(120),
        }
    }

    pub fn activate_minimal(&self) -> Result<()> {
        let mut child = activation_command()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("launch Claude activation through `claude`")?;
        let deadline = Instant::now() + self.timeout;
        loop {
            if let Some(status) = child.try_wait().context("wait for Claude activation")? {
                anyhow::ensure!(status.success(), "Claude activation exited with {status}");
                return Ok(());
            }
            if Instant::now() >= deadline {
                terminate(&mut child);
                bail!("Claude activation timed out after {:?}", self.timeout);
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}

impl Default for ClaudeActivator {
    fn default() -> Self {
        Self::new()
    }
}

impl Activator for ClaudeActivator {
    fn activate(&mut self) -> Result<()> {
        self.activate_minimal()
    }
}

fn activation_command() -> Command {
    let mut command = Command::new("claude");
    command.args([
        "-p",
        ACTIVATION_PROMPT,
        "--model",
        ACTIVATION_MODEL,
        "--effort=low",
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    command
}

fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Reads Claude Code's local OAuth session and queries the same usage endpoint
/// used by CodexBar. Credentials stay in Claude's own `.credentials.json`.
pub struct ClaudeClient {
    timeout: Duration,
}

impl ClaudeClient {
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(15),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn read_rate_limits(&self) -> Result<RateLimits> {
        let credentials = load_credentials()?;
        if credentials.expires_at.is_some_and(|expires_at| expires_at <= Utc::now()) {
            bail!("Claude login has expired. Run `claude` to sign in again.");
        }

        // `ureq` is built without its default Rustls backend. Configure the
        // native TLS adapter explicitly so Claude's HTTPS endpoint uses the
        // Windows certificate store (Schannel), as the updater already does.
        let tls = ureq::native_tls::TlsConnector::new().context("create Windows TLS connector")?;
        let agent = ureq::AgentBuilder::new()
            .timeout(self.timeout)
            .tls_connector(Arc::new(tls))
            .build();
        let response = agent
            .get(OAUTH_USAGE_URL)
            .set("Authorization", &format!("Bearer {}", credentials.access_token))
            .set("Accept", "application/json")
            .set("Content-Type", "application/json")
            .set("anthropic-beta", OAUTH_BETA)
            .set(
                "User-Agent",
                &format!("claude-code/{FALLBACK_CLAUDE_CODE_VERSION}"),
            )
            .call();
        let body = match response {
            Ok(response) => response.into_string().context("read Claude OAuth response")?,
            Err(ureq::Error::Status(401, _)) => {
                bail!("Claude OAuth request was unauthorized. Run `claude` to sign in again.")
            }
            Err(ureq::Error::Status(429, _)) => {
                bail!("Claude usage endpoint is rate limited. Try again in a few minutes.")
            }
            Err(ureq::Error::Status(status, _)) => {
                bail!("Claude OAuth usage request failed with HTTP {status}")
            }
            Err(error) => return Err(error).context("request Claude OAuth usage"),
        };
        parse_usage_response(&body, Utc::now())
    }
}

impl Default for ClaudeClient {
    fn default() -> Self {
        Self::new()
    }
}

impl LimitProvider for ClaudeClient {
    fn read_limits(&mut self) -> Result<RateLimits> {
        self.read_rate_limits()
    }
}

impl UsageProvider for ClaudeClient {
    fn load_cached_usage_statistics(&mut self, history_days: u16) -> Result<usage::UsageStatistics> {
        usage::load_cached_claude_usage_statistics(history_days)
    }

    fn refresh_usage_statistics(&mut self, history_days: u16) -> Result<usage::UsageStatistics> {
        usage::refresh_claude_usage_statistics(history_days)
    }
}

#[derive(Deserialize)]
struct CredentialFile {
    #[serde(rename = "claudeAiOauth")]
    oauth: Option<OAuthCredentials>,
}

#[derive(Deserialize)]
struct OAuthCredentials {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "expiresAt")]
    expires_at_millis: Option<i64>,
}

struct Credentials {
    access_token: String,
    expires_at: Option<DateTime<Utc>>,
}

fn load_credentials() -> Result<Credentials> {
    let directories = BaseDirs::new()
        .context("could not resolve the home directory for Claude credentials")?;
    let home = directories.home_dir();
    let path = home.join(".claude").join(".credentials.json");
    let contents = fs::read(&path).with_context(|| {
        format!(
            "read {} (install Claude Code and sign in first)",
            path.display()
        )
    })?;
    let file: CredentialFile = serde_json::from_slice(&contents)
        .with_context(|| format!("parse {}", path.display()))?;
    let oauth = file
        .oauth
        .context("Claude credentials do not contain a Claude OAuth session; run `claude` to sign in")?;
    let access_token = oauth.access_token.trim().to_owned();
    anyhow::ensure!(!access_token.is_empty(), "Claude OAuth access token is empty");
    let expires_at = oauth
        .expires_at_millis
        .and_then(|milliseconds| DateTime::from_timestamp_millis(milliseconds));
    Ok(Credentials {
        access_token,
        expires_at,
    })
}

#[derive(Deserialize)]
struct OAuthUsageResponse {
    five_hour: Option<OAuthUsageWindow>,
    seven_day: Option<OAuthUsageWindow>,
    #[serde(default)]
    limits: Vec<OAuthLimitEntry>,
    /// Claude regularly adds model- and feature-specific quota windows (for
    /// example `seven_day_fable`). Keep every window-shaped field instead of
    /// silently throwing newer limits away.
    #[serde(flatten)]
    additional_windows: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct OAuthLimitEntry {
    kind: Option<String>,
    group: Option<String>,
    percent: Option<f64>,
    resets_at: Option<String>,
    scope: Option<OAuthLimitScope>,
}

#[derive(Deserialize)]
struct OAuthLimitScope {
    model: Option<OAuthLimitScopeModel>,
}

#[derive(Deserialize)]
struct OAuthLimitScopeModel {
    id: Option<String>,
    display_name: Option<String>,
}

#[derive(Clone, Deserialize)]
struct OAuthUsageWindow {
    utilization: Option<f64>,
    resets_at: Option<String>,
    #[serde(default, alias = "windowDurationMins", alias = "window_duration_mins")]
    duration_minutes: Option<u32>,
}

pub fn parse_usage_response(response: &str, sampled_at: DateTime<Utc>) -> Result<RateLimits> {
    let response: OAuthUsageResponse = serde_json::from_str(response).context("parse Claude OAuth usage")?;
    let primary = parse_window(response.five_hour, Some(5 * 60));
    let secondary = parse_window(response.seven_day, Some(7 * 24 * 60));
    let mut additional_limits = response
        .additional_windows
        .into_iter()
        .filter_map(|(id, value)| {
            let window = serde_json::from_value::<OAuthUsageWindow>(value).ok()?;
            let window = parse_window(Some(window), inferred_duration_minutes(&id));
            (!window.is_empty()).then(|| AdditionalLimit {
                title: additional_limit_title(&id),
                id,
                window,
            })
        })
        .collect::<Vec<_>>();
    additional_limits.extend(scoped_weekly_limits(response.limits));
    additional_limits.sort_by(|left, right| left.id.cmp(&right.id));
    additional_limits.dedup_by(|left, right| left.id == right.id);
    anyhow::ensure!(
        !primary.is_empty() || !secondary.is_empty() || !additional_limits.is_empty(),
        "Claude OAuth response does not contain usage windows"
    );
    Ok(RateLimits {
        primary,
        secondary,
        additional_limits,
        sampled_at,
        plan_type: Some("Claude".into()),
        ..RateLimits::default()
    }
    .normalized(sampled_at))
}

/// The OAuth endpoint's current shape puts promotional/model-only weekly
/// quotas in `limits[]`. A Fable limit, for example, is a `weekly_scoped`
/// entry with its visible name at `scope.model.display_name`.
fn scoped_weekly_limits(limits: Vec<OAuthLimitEntry>) -> Vec<AdditionalLimit> {
    let mut seen_ids = BTreeSet::new();
    limits
        .into_iter()
        .filter_map(|limit| {
            if limit.kind.as_deref() != Some("weekly_scoped")
                || limit.group.as_deref() != Some("weekly")
            {
                return None;
            }
            let model = limit.scope?.model?;
            let title = non_empty(model.display_name)?;
            if title.eq_ignore_ascii_case("all models") {
                return None;
            }
            let identity = non_empty(model.id).unwrap_or_else(|| title.clone());
            let identity_slug = limit_slug(&identity);
            if identity_slug == "all-models" || identity_slug.ends_with("-all-models") {
                return None;
            }
            let id = format!("claude-weekly-scoped-{identity_slug}");
            if identity_slug.is_empty() || !seen_ids.insert(id.clone()) {
                return None;
            }
            let used_percent = limit
                .percent
                .filter(|value| value.is_finite())
                .map(|value| value.round().clamp(0.0, 100.0) as u8);
            let resets_at = limit
                .resets_at
                .as_deref()
                .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
                .map(|value| value.with_timezone(&Utc));
            let window = LimitWindow {
                used_percent,
                resets_at,
                duration_minutes: Some(7 * 24 * 60),
            };
            (!window.is_empty()).then(|| AdditionalLimit {
                id,
                title: format!("{title} only"),
                window,
            })
        })
        .collect()
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

fn limit_slug(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for character in value.chars() {
        if character.is_alphanumeric() {
            slug.extend(character.to_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }
    slug.trim_matches('-').to_owned()
}

fn parse_window(window: Option<OAuthUsageWindow>, duration_minutes: Option<u32>) -> LimitWindow {
    let Some(window) = window else {
        return LimitWindow::default();
    };
    LimitWindow {
        used_percent: window
            .utilization
            .filter(|value| value.is_finite())
            .map(|value| value.round().clamp(0.0, 100.0) as u8),
        resets_at: window
            .resets_at
            .as_deref()
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc)),
        duration_minutes: window.duration_minutes.or(duration_minutes),
    }
}

fn inferred_duration_minutes(id: &str) -> Option<u32> {
    match id {
        name if name.starts_with("five_hour") => Some(5 * 60),
        name if name.starts_with("seven_day") => Some(7 * 24 * 60),
        name if name.starts_with("monthly") => Some(30 * 24 * 60),
        _ => None,
    }
}

fn additional_limit_title(id: &str) -> String {
    let name = id
        .strip_prefix("seven_day_")
        .or_else(|| id.strip_prefix("five_hour_"))
        .or_else(|| id.strip_prefix("monthly_"))
        .unwrap_or(id);
    name.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            let Some(first) = characters.next() else {
                return String::new();
            };
            format!("{}{}", first.to_uppercase(), characters.as_str().to_lowercase())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn parses_oauth_session_and_weekly_windows() {
        let sampled_at = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();
        let limits = parse_usage_response(
            r#"{"five_hour":{"utilization":12.5,"resets_at":"2026-07-15T15:00:00.000Z"},"seven_day":{"utilization":30,"resets_at":"2026-07-21T00:00:00.000Z"}}"#,
            sampled_at,
        )
        .unwrap();
        assert_eq!(limits.primary.used_percent, Some(13));
        assert_eq!(limits.primary.duration_minutes, Some(300));
        assert_eq!(limits.secondary.used_percent, Some(30));
        assert_eq!(limits.secondary.duration_minutes, Some(10_080));
    }

    #[test]
    fn accepts_a_weekly_only_oauth_response() {
        let limits = parse_usage_response(r#"{"seven_day":{"utilization":42}}"#, Utc::now()).unwrap();
        assert!(limits.primary.is_empty());
        assert_eq!(limits.secondary.used_percent, Some(42));
    }

    #[test]
    fn preserves_every_additional_claude_limit_including_fable() {
        let limits = parse_usage_response(
            r#"{"five_hour":{"utilization":12},"seven_day":{"utilization":30},"seven_day_opus":{"utilization":7},"limits":[{"kind":"weekly_scoped","group":"weekly","percent":42,"resets_at":"2026-07-21T00:00:00.000Z","scope":{"model":{"id":"claude/fable.5:promo","display_name":"Fable"}}},{"kind":"weekly_scoped","group":"weekly","percent":30,"scope":{"model":{"display_name":"All models"}}}],"organization_name":"example"}"#,
            Utc::now(),
        )
        .unwrap();

        assert_eq!(limits.additional_limits.len(), 2);
        assert_eq!(limits.additional_limits[0].id, "claude-weekly-scoped-claude-fable-5-promo");
        assert_eq!(limits.additional_limits[0].title, "Fable only");
        assert_eq!(limits.additional_limits[0].window.used_percent, Some(42));
        assert_eq!(limits.additional_limits[0].window.duration_minutes, Some(10_080));
        assert_eq!(limits.additional_limits[1].title, "Opus");
    }

    #[test]
    fn activation_uses_the_minimal_haiku_command() {
        let command = activation_command();
        assert_eq!(command.get_program().to_string_lossy(), "claude");
        assert_eq!(
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            [
                "-p",
                "reply with letter a",
                "--model",
                "haiku",
                "--effort=low",
            ]
        );
    }
}
