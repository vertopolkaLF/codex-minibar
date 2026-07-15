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
const OAUTH_PROFILE_URL: &str = "https://api.anthropic.com/api/oauth/profile";
const OAUTH_ACCOUNT_SETTINGS_URL: &str = "https://api.anthropic.com/api/oauth/account/settings";
const OAUTH_BETA: &str = "oauth-2025-04-20";
const FALLBACK_CLAUDE_CODE_VERSION: &str = "2.1.0";
const PROFILE_REFRESH_INTERVAL: Duration = Duration::from_secs(30 * 60);
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
    account_cache: ClaudeAccountCache,
}

struct ClaudeAccountCache {
    account_name: Option<String>,
    plan_type: Option<String>,
    checked_at: Option<Instant>,
    reset_schedule: Vec<(String, Option<DateTime<Utc>>)>,
}

impl Default for ClaudeAccountCache {
    fn default() -> Self {
        Self {
            account_name: None,
            plan_type: None,
            checked_at: None,
            reset_schedule: Vec::new(),
        }
    }
}

impl ClaudeAccountCache {
    fn needs_refresh(&self, reset_schedule: &[(String, Option<DateTime<Utc>>)]) -> bool {
        self.checked_at
            .is_none_or(|checked_at| checked_at.elapsed() >= PROFILE_REFRESH_INTERVAL)
            || self.reset_schedule != reset_schedule
    }

    fn record(
        &mut self,
        account_name: Option<String>,
        plan_type: Option<String>,
        reset_schedule: Vec<(String, Option<DateTime<Utc>>)>,
    ) {
        self.account_name = account_name;
        self.plan_type = plan_type;
        self.checked_at = Some(Instant::now());
        self.reset_schedule = reset_schedule;
    }
}

impl ClaudeClient {
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(15),
            account_cache: ClaudeAccountCache::default(),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn read_rate_limits(&mut self) -> Result<RateLimits> {
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
        let mut limits = parse_usage_response(&body, Utc::now())?;
        let reset_schedule = reset_schedule(&limits);
        if self.account_cache.needs_refresh(&reset_schedule) {
            // Account metadata stays separate from quota reads. Cache it for
            // 30 minutes and refresh immediately when any reset changes.
            self.account_cache.record(
                fetch_account_name(&agent, &credentials.access_token)
                    .ok()
                    .flatten(),
                fetch_plan_type(&agent, &credentials.access_token)
                    .ok()
                    .flatten(),
                reset_schedule,
            );
        }
        // Newer usage responses omit organization_name, but older responses
        // still expose it. The cached profile is authoritative whenever it
        // provides an identity value.
        if let Some(account_name) = self.account_cache.account_name.clone() {
            limits.account_name = Some(account_name);
        }
        limits.plan_type = self.account_cache.plan_type.clone();
        Ok(limits)
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

fn reset_schedule(limits: &RateLimits) -> Vec<(String, Option<DateTime<Utc>>)> {
    let mut schedule = vec![
        ("primary".into(), limits.primary.resets_at),
        ("secondary".into(), limits.secondary.resets_at),
    ];
    schedule.extend(
        limits
            .additional_limits
            .iter()
            .map(|limit| (limit.id.clone(), limit.window.resets_at)),
    );
    schedule
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
    organization_name: Option<String>,
    #[serde(default)]
    limits: Vec<OAuthLimitEntry>,
    /// Claude regularly adds model- and feature-specific quota windows (for
    /// example `seven_day_fable`). Keep every window-shaped field instead of
    /// silently throwing newer limits away.
    #[serde(flatten)]
    additional_windows: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct OAuthProfileResponse {
    account: Option<OAuthProfileAccount>,
    organization: Option<OAuthProfileOrganization>,
}

#[derive(Deserialize)]
struct OAuthProfileAccount {
    full_name: Option<String>,
    display_name: Option<String>,
    email: Option<String>,
}

#[derive(Deserialize)]
struct OAuthProfileOrganization {
    name: Option<String>,
}

#[derive(Deserialize)]
struct OAuthAccountSettingsResponse {
    #[serde(rename = "subscriptionType")]
    subscription_type: Option<String>,
    #[serde(rename = "rateLimitTier")]
    rate_limit_tier: Option<String>,
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
        account_name: non_empty(response.organization_name),
        // The OAuth usage payload does not contain a subscription tier. Do
        // not present the provider name as if it were a plan.
        plan_type: None,
        ..RateLimits::default()
    }
    .normalized(sampled_at))
}

fn fetch_account_name(agent: &ureq::Agent, access_token: &str) -> Result<Option<String>> {
    let response = agent
        .get(OAUTH_PROFILE_URL)
        .set("Authorization", &format!("Bearer {access_token}"))
        .set("Accept", "application/json")
        .set("anthropic-beta", OAUTH_BETA)
        .set(
            "User-Agent",
            &format!("claude-code/{FALLBACK_CLAUDE_CODE_VERSION}"),
        )
        .call()
        .context("request Claude OAuth profile")?;
    let body = response
        .into_string()
        .context("read Claude OAuth profile response")?;
    parse_account_name(&body)
}

fn parse_account_name(response: &str) -> Result<Option<String>> {
    let profile: OAuthProfileResponse =
        serde_json::from_str(response).context("parse Claude OAuth profile")?;
    let (person_name, email) = profile.account.map_or((None, None), |account| {
        (
            non_empty(account.full_name).or_else(|| non_empty(account.display_name)),
            non_empty(account.email),
        )
    });
    Ok(person_name
        .or_else(|| profile.organization.and_then(|organization| non_empty(organization.name)))
        .or(email))
}

fn fetch_plan_type(agent: &ureq::Agent, access_token: &str) -> Result<Option<String>> {
    let response = agent
        .get(OAUTH_ACCOUNT_SETTINGS_URL)
        .set("Authorization", &format!("Bearer {access_token}"))
        .set("Accept", "application/json")
        .set("anthropic-beta", OAUTH_BETA)
        .set(
            "User-Agent",
            &format!("claude-code/{FALLBACK_CLAUDE_CODE_VERSION}"),
        )
        .call()
        .context("request Claude OAuth account settings")?;
    let body = response
        .into_string()
        .context("read Claude OAuth account settings")?;
    parse_plan_type(&body)
}

fn parse_plan_type(response: &str) -> Result<Option<String>> {
    let settings: OAuthAccountSettingsResponse =
        serde_json::from_str(response).context("parse Claude OAuth account settings")?;
    if let Some(subscription_type) = non_empty(settings.subscription_type) {
        return Ok(Some(subscription_type));
    }

    // Older accounts may only report an internal rate-limit tier. Infer a
    // visible subscription tier only from unambiguous identifiers; generic
    // values such as `default` remain intentionally absent from the UI.
    let plan = non_empty(settings.rate_limit_tier).and_then(|tier| {
        let tier = tier.to_ascii_lowercase();
        ["enterprise", "team", "max", "pro"]
            .into_iter()
            .find(|plan| tier.split(|character: char| !character.is_alphanumeric()).any(|part| part == *plan))
            .map(str::to_owned)
    });
    Ok(plan)
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
    fn profile_name_falls_back_to_organization_then_email() {
        let name = parse_account_name(
            r#"{"account":{"full_name":"Ada Lovelace","email":"ada@example.com"},"organization":{"name":"Example Studio"}}"#,
        )
        .unwrap();
        assert_eq!(name.as_deref(), Some("Ada Lovelace"));

        let organization_name = parse_account_name(
            r#"{"account":{"full_name":" ","email":"ada@example.com"},"organization":{"name":"Example Studio"}}"#,
        )
        .unwrap();
        assert_eq!(organization_name.as_deref(), Some("Example Studio"));

        let email = parse_account_name(r#"{"account":{"email":"ada@example.com"},"organization":{}}"#)
            .unwrap();
        assert_eq!(email.as_deref(), Some("ada@example.com"));
    }

    #[test]
    fn account_settings_prefer_the_explicit_subscription_type() {
        assert_eq!(
            parse_plan_type(r#"{"subscriptionType":"pro","rateLimitTier":"default_claude_max_20x"}"#)
                .unwrap()
                .as_deref(),
            Some("pro")
        );
        assert_eq!(
            parse_plan_type(r#"{"rateLimitTier":"default_claude_max_20x"}"#)
                .unwrap()
                .as_deref(),
            Some("max")
        );
        assert_eq!(parse_plan_type(r#"{"rateLimitTier":"default"}"#).unwrap(), None);
    }

    #[test]
    fn account_cache_refreshes_after_30_minutes_or_a_reset_change() {
        let schedule = vec![("primary".into(), None), ("secondary".into(), None)];
        let mut cache = ClaudeAccountCache::default();
        assert!(cache.needs_refresh(&schedule));

        cache.record(Some("Ada Lovelace".into()), Some("pro".into()), schedule.clone());
        assert!(!cache.needs_refresh(&schedule));
        assert_eq!(cache.plan_type.as_deref(), Some("pro"));

        let changed_schedule = vec![("primary".into(), None), ("secondary".into(), Some(Utc::now()))];
        assert!(cache.needs_refresh(&changed_schedule));

        cache.checked_at = Some(Instant::now() - PROFILE_REFRESH_INTERVAL);
        assert!(cache.needs_refresh(&schedule));
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
        assert_eq!(limits.account_name.as_deref(), Some("example"));
        assert_eq!(limits.plan_type, None);
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
