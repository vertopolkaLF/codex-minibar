use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
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
    claude_desktop,
    limits::{AdditionalLimit, LimitWindow, RateLimits},
    usage,
    worker::{Activator, LimitProvider, UsageProvider},
};

const OAUTH_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const OAUTH_PROFILE_URL: &str = "https://api.anthropic.com/api/oauth/profile";
const OAUTH_BETA: &str = "oauth-2025-04-20";
const FALLBACK_CLAUDE_CODE_VERSION: &str = "2.1.0";
const PROFILE_REFRESH_INTERVAL: Duration = Duration::from_secs(30 * 60);
pub const ACTIVATION_MODEL: &str = "haiku";
pub const ACTIVATION_PROMPT: &str = "reply with letter a";

/// Detect a local Claude Code installation without starting it. A signed-in
/// credentials directory is also a useful signal for portable/npm installs
/// whose launcher is no longer present on PATH in the current process, and the
/// desktop app counts too: it ships its own Claude Code and never writes a
/// credentials file.
pub fn is_installed() -> bool {
    credentials_path().is_some_and(|path| path.is_file())
        || path_contains_executable("claude")
        || claude_desktop::is_installed()
}

fn path_contains_executable(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|directory| {
        #[cfg(windows)]
        {
            [".exe", ".cmd", ".ps1", ".bat"]
                .into_iter()
                .any(|extension| directory.join(format!("{name}{extension}")).is_file())
        }
        #[cfg(not(windows))]
        {
            directory.join(name).is_file()
        }
    })
}

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
    activation_command_for(activation_program())
}

/// Prefer the launcher on PATH, then the `claude.exe` the desktop app unpacks
/// for its embedded Claude Code. Falling back to the bare name keeps the error
/// message about a missing CLI rather than a missing desktop install.
fn activation_program() -> PathBuf {
    if path_contains_executable("claude") {
        return PathBuf::from("claude");
    }
    claude_desktop::bundled_cli().unwrap_or_else(|| PathBuf::from("claude"))
}

fn activation_command_for(program: PathBuf) -> Command {
    let mut command = Command::new(program);
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
        let sign_in_hint = credentials.source.sign_in_hint();

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
                bail!("Claude OAuth request was unauthorized. {sign_in_hint}")
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
        // Both credential files record the plan next to the token, so the
        // common case needs no request at all to label it.
        let local_plan_type = plan_type_from_account_fields(
            credentials.subscription_type.clone(),
            credentials.rate_limit_tier.clone(),
        );
        let reset_schedule = reset_schedule(&limits);
        if self.account_cache.needs_refresh(&reset_schedule) {
            // Account metadata stays separate from quota reads. Cache it for
            // 30 minutes and refresh immediately when any reset changes. One
            // profile request covers both the name and the plan fallback.
            let profile = fetch_profile(&agent, &credentials.access_token).ok();
            self.account_cache.record(
                profile.as_ref().and_then(|profile| profile.account_name.clone()),
                profile.and_then(|profile| profile.plan_type),
                reset_schedule,
            );
        }
        // Newer usage responses omit organization_name, but older responses
        // still expose it. The cached profile is authoritative whenever it
        // provides an identity value.
        if let Some(account_name) = self.account_cache.account_name.clone() {
            limits.account_name = Some(account_name);
        }
        limits.plan_type = local_plan_type.or_else(|| self.account_cache.plan_type.clone());
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
    #[serde(rename = "subscriptionType")]
    subscription_type: Option<String>,
    #[serde(rename = "rateLimitTier")]
    rate_limit_tier: Option<String>,
}

struct Credentials {
    access_token: String,
    expires_at: Option<DateTime<Utc>>,
    /// Both the CLI credentials file and the desktop token cache record the
    /// plan next to the token, so neither path needs a request to label it.
    subscription_type: Option<String>,
    rate_limit_tier: Option<String>,
    source: CredentialSource,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CredentialSource {
    Cli,
    Desktop,
}

impl CredentialSource {
    /// How to get a fresh session back for this source.
    fn sign_in_hint(self) -> &'static str {
        match self {
            Self::Cli => "Run `claude` to sign in again.",
            Self::Desktop => "Open the Claude desktop app to sign in again.",
        }
    }
}

impl Credentials {
    fn is_expired(&self) -> bool {
        self.expires_at
            .is_some_and(|expires_at| expires_at <= Utc::now())
    }
}

fn credentials_path() -> Option<PathBuf> {
    BaseDirs::new()
        .map(|directories| directories.home_dir().join(".claude").join(".credentials.json"))
}

/// Collects every local Claude session and prefers a live one. Both sources
/// yield byte-identical usage data, so ordering only decides whose expiry we
/// follow: the desktop app refreshes its token whenever it is open, while the
/// CLI refreshes only when `claude` runs, so the desktop session goes stale
/// less often. A stale session on either side never masks a live one.
fn load_credentials() -> Result<Credentials> {
    let attempts = [load_desktop_credentials(), load_cli_credentials()];
    let mut errors = Vec::new();
    let mut expired = None;
    for attempt in attempts {
        match attempt {
            Ok(credentials) if !credentials.is_expired() => return Ok(credentials),
            Ok(credentials) => expired = expired.or(Some(credentials)),
            Err(error) => errors.push(format!("{error:#}")),
        }
    }
    if let Some(credentials) = expired {
        bail!(
            "Claude login has expired. {}",
            credentials.source.sign_in_hint()
        );
    }
    bail!(
        "no Claude login found. Install Claude Code or the Claude desktop app and sign in first ({})",
        errors.join("; ")
    )
}

fn load_cli_credentials() -> Result<Credentials> {
    let path = credentials_path()
        .context("could not resolve the home directory for Claude credentials")?;
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
        subscription_type: oauth.subscription_type,
        rate_limit_tier: oauth.rate_limit_tier,
        source: CredentialSource::Cli,
    })
}

fn load_desktop_credentials() -> Result<Credentials> {
    let session = claude_desktop::load_session()?;
    Ok(Credentials {
        access_token: session.access_token,
        expires_at: session.expires_at,
        subscription_type: session.subscription_type,
        rate_limit_tier: session.rate_limit_tier,
        source: CredentialSource::Desktop,
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
    has_claude_max: Option<bool>,
    has_claude_pro: Option<bool>,
}

#[derive(Deserialize)]
struct OAuthProfileOrganization {
    name: Option<String>,
    organization_type: Option<String>,
    seat_tier: Option<String>,
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

/// Identity and plan both come from the profile endpoint. The former
/// `account/settings` endpoint no longer reports `subscriptionType` or
/// `rateLimitTier` at all, so it is not consulted.
struct AccountProfile {
    account_name: Option<String>,
    plan_type: Option<String>,
}

fn fetch_profile(agent: &ureq::Agent, access_token: &str) -> Result<AccountProfile> {
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
    parse_profile(&body)
}

fn parse_profile(response: &str) -> Result<AccountProfile> {
    let profile: OAuthProfileResponse =
        serde_json::from_str(response).context("parse Claude OAuth profile")?;
    let (person_name, email, has_max, has_pro) =
        profile.account.map_or((None, None, false, false), |account| {
            (
                non_empty(account.full_name).or_else(|| non_empty(account.display_name)),
                non_empty(account.email),
                account.has_claude_max.unwrap_or_default(),
                account.has_claude_pro.unwrap_or_default(),
            )
        });
    let (organization_name, plan_type) = profile.organization.map_or((None, None), |organization| {
        (
            non_empty(organization.name),
            // A seat or organization type names the plan directly; the
            // rate-limit tier is the weakest signal and usually opaque
            // (`default_raven`), so it is consulted last.
            plan_type_from_tier(organization.seat_tier)
                .or_else(|| plan_type_from_tier(organization.organization_type))
                .or_else(|| plan_type_from_tier(organization.rate_limit_tier)),
        )
    });
    Ok(AccountProfile {
        account_name: person_name.or(organization_name).or(email),
        plan_type: plan_type.or_else(|| {
            // Personal entitlement flags only matter when no organization
            // named a plan, so a team seat is not relabelled by them.
            has_max
                .then(|| "max".to_owned())
                .or_else(|| has_pro.then(|| "pro".to_owned()))
        }),
    })
}

/// Shared by both credential files, which report the same two fields.
fn plan_type_from_account_fields(
    subscription_type: Option<String>,
    rate_limit_tier: Option<String>,
) -> Option<String> {
    non_empty(subscription_type).or_else(|| plan_type_from_tier(rate_limit_tier))
}

/// Infers a visible subscription tier from an internal tier identifier such as
/// `team_standard` or `claude_team`. Only unambiguous identifiers count;
/// generic values like `default_raven` stay intentionally absent from the UI.
fn plan_type_from_tier(tier: Option<String>) -> Option<String> {
    non_empty(tier).and_then(|tier| {
        let tier = tier.to_ascii_lowercase();
        ["enterprise", "team", "max", "pro"]
            .into_iter()
            .find(|plan| tier.split(|character: char| !character.is_alphanumeric()).any(|part| part == *plan))
            .map(str::to_owned)
    })
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
        let name = parse_profile(
            r#"{"account":{"full_name":"Ada Lovelace","email":"ada@example.com"},"organization":{"name":"Example Studio"}}"#,
        )
        .unwrap();
        assert_eq!(name.account_name.as_deref(), Some("Ada Lovelace"));

        let organization_name = parse_profile(
            r#"{"account":{"full_name":" ","email":"ada@example.com"},"organization":{"name":"Example Studio"}}"#,
        )
        .unwrap();
        assert_eq!(organization_name.account_name.as_deref(), Some("Example Studio"));

        let email = parse_profile(r#"{"account":{"email":"ada@example.com"},"organization":{}}"#)
            .unwrap();
        assert_eq!(email.account_name.as_deref(), Some("ada@example.com"));
    }

    #[test]
    fn profile_plan_prefers_the_seat_tier_over_personal_entitlements() {
        // Shape observed live: a team seat whose rate-limit tier is opaque.
        let team = parse_profile(
            r#"{"account":{"full_name":"Ada","has_claude_max":false,"has_claude_pro":false},"organization":{"name":"Example","organization_type":"claude_team","seat_tier":"team_standard","rate_limit_tier":"default_raven"}}"#,
        )
        .unwrap();
        assert_eq!(team.plan_type.as_deref(), Some("team"));

        // A team seat is not relabelled by a personal Max entitlement.
        let both = parse_profile(
            r#"{"account":{"has_claude_max":true},"organization":{"seat_tier":"team_standard"}}"#,
        )
        .unwrap();
        assert_eq!(both.plan_type.as_deref(), Some("team"));

        let personal = parse_profile(
            r#"{"account":{"has_claude_max":true,"has_claude_pro":false},"organization":{"rate_limit_tier":"default_raven"}}"#,
        )
        .unwrap();
        assert_eq!(personal.plan_type.as_deref(), Some("max"));

        let pro = parse_profile(r#"{"account":{"has_claude_pro":true},"organization":{}}"#).unwrap();
        assert_eq!(pro.plan_type.as_deref(), Some("pro"));

        let unknown = parse_profile(r#"{"account":{},"organization":{"rate_limit_tier":"default"}}"#)
            .unwrap();
        assert_eq!(unknown.plan_type, None);
    }

    #[test]
    fn credential_files_prefer_the_explicit_subscription_type() {
        assert_eq!(
            plan_type_from_account_fields(
                Some("pro".into()),
                Some("default_claude_max_20x".into())
            )
            .as_deref(),
            Some("pro")
        );
        assert_eq!(
            plan_type_from_account_fields(None, Some("default_claude_max_20x".into())).as_deref(),
            Some("max")
        );
        assert_eq!(plan_type_from_account_fields(None, Some("default".into())), None);
        // The live value on a team account carries no usable tier keyword.
        assert_eq!(plan_type_from_account_fields(None, Some("default_raven".into())), None);
    }

    #[test]
    fn cli_credentials_carry_the_plan_alongside_the_token() {
        let file: CredentialFile = serde_json::from_str(
            r#"{"claudeAiOauth":{"accessToken":"token","expiresAt":1785183619245,"subscriptionType":"team","rateLimitTier":"default_raven"}}"#,
        )
        .unwrap();
        let oauth = file.oauth.unwrap();
        assert_eq!(oauth.subscription_type.as_deref(), Some("team"));
        assert_eq!(oauth.rate_limit_tier.as_deref(), Some("default_raven"));
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
        let command = activation_command_for(PathBuf::from("claude"));
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
