//! SuperGrok subscription quota from the official Grok CLI login.

use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use directories::BaseDirs;
use serde::Deserialize;
use serde_json::Value;

use crate::{
    limits::{LimitWindow, RateLimits},
    provider_cli,
    usage::UsageStatistics,
    worker::{Activator, LimitProvider, UsageProvider},
};

const BILLING_URL: &str = "https://cli-chat-proxy.grok.com/v1/billing?format=credits";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const OIDC_SCOPE_PREFIX: &str = "https://auth.x.ai::";
const LEGACY_SCOPE: &str = "https://accounts.x.ai/sign-in";
const CLI_NAMES: &[&str] = &["grok.exe", "grok.cmd", "grok.ps1", "grok.bat"];

pub struct GrokClient {
    agent: ureq::Agent,
    cli_folder: Option<PathBuf>,
}

pub struct GrokActivator;

impl GrokClient {
    pub fn new(cli_folder: Option<PathBuf>) -> Self {
        Self {
            agent: ureq::AgentBuilder::new().timeout(REQUEST_TIMEOUT).build(),
            cli_folder,
        }
    }

    fn fresh_credentials(&self) -> Result<GrokCredentials> {
        recover_stale_credentials(load_credentials()?, || {
            provider_cli::refresh_session(
                "Grok",
                &cli_candidates(self.cli_folder.as_deref()),
                &["models"],
                REQUEST_TIMEOUT,
            )
            .map_err(|_| anyhow::anyhow!("Grok session expired; run `grok login` to refresh it"))?;
            load_credentials()
        })
    }
}

impl Default for GrokClient {
    fn default() -> Self {
        Self::new(None)
    }
}

impl LimitProvider for GrokClient {
    fn read_limits(&mut self) -> Result<RateLimits> {
        let credentials = self.fresh_credentials()?;
        if credentials.is_team {
            bail!("Grok team subscription usage is not available")
        }

        let response = match self
            .agent
            .get(BILLING_URL)
            .set(
                "Authorization",
                &format!("Bearer {}", credentials.access_token),
            )
            .set("x-xai-token-auth", "xai-grok-cli")
            .set("Accept", "application/json")
            .set("User-Agent", "Codex Minibar")
            .call()
        {
            Ok(response) => response,
            Err(ureq::Error::Status(401 | 403, _)) => {
                bail!("Grok session is not authorized; run `grok login`")
            }
            Err(ureq::Error::Status(429, _)) => {
                return Err(crate::worker::rate_limit_error(
                    "Grok subscription quota request was rate limited (HTTP 429).",
                ));
            }
            Err(ureq::Error::Status(status, _)) => {
                bail!("Grok subscription quota request failed with HTTP {status}")
            }
            Err(error) => return Err(error).context("request Grok subscription quota"),
        };
        let body = response
            .into_string()
            .context("read Grok subscription quota response")?;
        let parsed = parse_billing_response(&body)?;

        Ok(RateLimits {
            primary: LimitWindow {
                used_percent: parsed.used_percent,
                resets_at: parsed.resets_at,
                duration_minutes: parsed.duration_minutes,
            },
            sampled_at: Utc::now(),
            account_name: credentials.email,
            plan_type: parsed.plan,
            limit_name: Some(parsed.cycle_label),
            ..RateLimits::default()
        })
    }
}

impl UsageProvider for GrokClient {
    fn load_cached_usage_statistics(&mut self, _history_days: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }

    fn refresh_usage_statistics(&mut self, _history_days: u16) -> Result<UsageStatistics> {
        // Subscription credits are quota, not API billing or local token spend.
        Ok(UsageStatistics::default())
    }
}

impl Activator for GrokActivator {
    fn activate(&mut self) -> Result<()> {
        bail!("Grok does not support session-window activation")
    }
}

pub fn is_installed(explicit: Option<&Path>) -> bool {
    signed_in() || cli_available(explicit).is_some()
}

/// Resolves a Grok CLI launcher. An explicit folder is searched first; a file
/// path remains supported so a copied binary still counts.
pub fn cli_available(explicit: Option<&Path>) -> Option<PathBuf> {
    cli_candidates(explicit).into_iter().next()
}

fn auth_path() -> Option<PathBuf> {
    if let Some(root) = env::var_os("GROK_HOME") {
        return Some(PathBuf::from(root).join("auth.json"));
    }
    BaseDirs::new().map(|dirs| dirs.home_dir().join(".grok/auth.json"))
}

fn signed_in() -> bool {
    auth_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .is_some_and(|raw| parse_credentials(&raw).is_ok())
}

fn grok_home_cli() -> Option<PathBuf> {
    if let Some(root) = env::var_os("GROK_HOME") {
        Some(PathBuf::from(root).join("bin/grok.exe"))
    } else {
        BaseDirs::new().map(|dirs| dirs.home_dir().join(".grok/bin/grok.exe"))
    }
}

fn cli_candidates(explicit: Option<&Path>) -> Vec<PathBuf> {
    provider_cli::candidates_from(explicit, grok_home_cli(), CLI_NAMES)
}

struct GrokCredentials {
    access_token: String,
    expires_at: Option<DateTime<Utc>>,
    email: Option<String>,
    is_team: bool,
}

fn recover_stale_credentials(
    initial: GrokCredentials,
    refresh: impl FnOnce() -> Result<GrokCredentials>,
) -> Result<GrokCredentials> {
    if !session_is_stale(initial.expires_at) {
        return Ok(initial);
    }
    let refreshed = refresh()?;
    if session_is_stale(refreshed.expires_at) {
        bail!("Grok session expired; run `grok login` to refresh it")
    }
    Ok(refreshed)
}

fn session_is_stale(expires_at: Option<DateTime<Utc>>) -> bool {
    expires_at.is_some_and(|expires| expires <= Utc::now() + chrono::Duration::minutes(1))
}

fn load_credentials() -> Result<GrokCredentials> {
    let path = auth_path().context("Grok home directory is unavailable")?;
    let raw = fs::read_to_string(&path).with_context(|| {
        format!(
            "Grok auth not found at {}; run `grok login`",
            path.display()
        )
    })?;
    parse_credentials(&raw)
}

fn parse_credentials(raw: &str) -> Result<GrokCredentials> {
    let root: serde_json::Map<String, Value> =
        serde_json::from_str(raw).context("parse Grok auth.json")?;
    let entry = root
        .iter()
        .find(|(scope, value)| scope.starts_with(OIDC_SCOPE_PREFIX) && usable_entry(value))
        .or_else(|| {
            root.iter().find(|(scope, value)| {
                (scope.as_str() == LEGACY_SCOPE || scope.contains("/sign-in"))
                    && usable_entry(value)
            })
        })
        .map(|(_, value)| value)
        .context("Grok auth.json contains no usable session; run `grok login`")?;
    let access_token = entry["key"]
        .as_str()
        .context("Grok auth.json session has no access token")?
        .to_owned();
    let expires_at = entry
        .get("expires_at")
        .and_then(Value::as_str)
        .and_then(parse_timestamp);
    Ok(GrokCredentials {
        access_token,
        expires_at,
        email: entry
            .get("email")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned),
        is_team: entry
            .get("principal_type")
            .and_then(Value::as_str)
            .is_some_and(|value| value.eq_ignore_ascii_case("team")),
    })
}

fn usable_entry(value: &Value) -> bool {
    value
        .get("key")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
}

#[derive(Deserialize)]
struct BillingEnvelope {
    config: Option<BillingConfig>,
    #[serde(rename = "subscriptionTier")]
    subscription_tier: Option<String>,
}

#[derive(Deserialize)]
struct BillingConfig {
    #[serde(rename = "creditUsagePercent")]
    credit_usage_percent: Option<f64>,
    #[serde(rename = "currentPeriod")]
    current_period: Option<BillingPeriod>,
    #[serde(rename = "billingPeriodEnd")]
    billing_period_end: Option<String>,
    #[serde(rename = "onDemandCap")]
    on_demand_cap: Option<BillingAmount>,
    #[serde(rename = "onDemandUsed")]
    on_demand_used: Option<BillingAmount>,
    #[serde(rename = "subscriptionTier")]
    subscription_tier: Option<String>,
}

#[derive(Deserialize)]
struct BillingPeriod {
    start: Option<String>,
    end: Option<String>,
}

#[derive(Deserialize)]
struct BillingAmount {
    val: Option<f64>,
}

struct ParsedBilling {
    used_percent: Option<u8>,
    resets_at: Option<DateTime<Utc>>,
    duration_minutes: Option<u32>,
    cycle_label: String,
    plan: Option<String>,
}

fn parse_billing_response(body: &str) -> Result<ParsedBilling> {
    let envelope: BillingEnvelope =
        serde_json::from_str(body).context("parse Grok subscription quota response")?;
    let config = envelope
        .config
        .context("Grok subscription quota response has no config")?;
    let used = config.credit_usage_percent.or_else(|| {
        let cap = config.on_demand_cap?.val?;
        let used = config.on_demand_used?.val?;
        (cap > 0.0).then_some(used / cap * 100.0)
    });
    let used_percent = used.filter(|value| value.is_finite()).map(percent);
    let start = config
        .current_period
        .as_ref()
        .and_then(|period| period.start.as_deref())
        .and_then(parse_timestamp);
    let resets_at = config
        .current_period
        .as_ref()
        .and_then(|period| period.end.as_deref())
        .and_then(parse_timestamp)
        .or_else(|| {
            config
                .billing_period_end
                .as_deref()
                .and_then(parse_timestamp)
        });
    if used_percent.is_none() && resets_at.is_none() {
        bail!("Grok subscription quota response contains no usable limit data")
    }
    let duration_minutes = start.zip(resets_at).and_then(|(start, end)| {
        u32::try_from((end - start).num_minutes())
            .ok()
            .filter(|value| *value > 0)
    });
    let cycle_label = match duration_minutes {
        Some(minutes) if minutes <= 8 * 24 * 60 => "Weekly credits",
        Some(minutes) if minutes >= 25 * 24 * 60 => "Monthly credits",
        _ => "Credits",
    }
    .to_owned();
    Ok(ParsedBilling {
        used_percent,
        resets_at,
        duration_minutes,
        cycle_label,
        plan: config.subscription_tier.or(envelope.subscription_tier),
    })
}

fn percent(value: f64) -> u8 {
    value.clamp(0.0, 100.0).round() as u8
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_supergrok_oauth_entry_without_exposing_the_token() {
        let credentials = parse_credentials(
            r#"{
              "https://accounts.x.ai/sign-in":{"key":"legacy"},
              "https://auth.x.ai::client":{"key":"secret","auth_mode":"oidc","email":"user@example.com","expires_at":"2099-01-01T00:00:00Z"}
            }"#,
        )
        .unwrap();
        assert_eq!(credentials.access_token, "secret");
        assert_eq!(credentials.email.as_deref(), Some("user@example.com"));
    }

    #[test]
    fn expired_session_uses_the_owner_cli_result_once() {
        let initial = test_credentials("2000-01-01T00:00:00Z");
        let mut calls = 0;
        let refreshed = recover_stale_credentials(initial, || {
            calls += 1;
            Ok(test_credentials("2099-01-01T00:00:00Z"))
        })
        .unwrap();
        assert_eq!(calls, 1);
        assert!(!session_is_stale(refreshed.expires_at));
    }

    #[test]
    fn fresh_session_does_not_start_the_owner_cli() {
        let initial = test_credentials("2099-01-01T00:00:00Z");
        let mut calls = 0;
        recover_stale_credentials(initial, || {
            calls += 1;
            Ok(test_credentials("2099-01-01T00:00:00Z"))
        })
        .unwrap();
        assert_eq!(calls, 0);
    }

    #[test]
    fn still_expired_owner_result_keeps_the_manual_login_message() {
        let error = recover_stale_credentials(test_credentials("2000-01-01T00:00:00Z"), || {
            Ok(test_credentials("2000-01-01T00:00:00Z"))
        })
        .err()
        .expect("stale refreshed credentials should fail");
        assert_eq!(
            error.to_string(),
            "Grok session expired; run `grok login` to refresh it"
        );
    }

    fn test_credentials(expiry: &str) -> GrokCredentials {
        GrokCredentials {
            access_token: "secret".into(),
            expires_at: parse_timestamp(expiry),
            email: None,
            is_team: false,
        }
    }

    #[test]
    fn parses_subscription_percent_reset_and_cycle() {
        let parsed = parse_billing_response(
            r#"{"config":{"creditUsagePercent":12.6,"currentPeriod":{"start":"2026-09-01T00:00:00Z","end":"2026-10-01T00:00:00Z"},"subscriptionTier":"SUPERGROK"}}"#,
        )
        .unwrap();
        assert_eq!(parsed.used_percent, Some(13));
        assert_eq!(parsed.duration_minutes, Some(43_200));
        assert_eq!(parsed.cycle_label, "Monthly credits");
    }

    #[test]
    fn preserves_unknown_usage_when_only_reset_is_reported() {
        let parsed =
            parse_billing_response(r#"{"config":{"billingPeriodEnd":"2026-10-01T00:00:00Z"}}"#)
                .unwrap();
        assert_eq!(parsed.used_percent, None);
        assert!(parsed.resets_at.is_some());
    }

    #[test]
    fn falls_back_when_current_period_reset_is_invalid() {
        let parsed = parse_billing_response(
            r#"{"config":{"currentPeriod":{"end":"invalid"},"billingPeriodEnd":"2026-10-01T00:00:00Z"}}"#,
        )
        .unwrap();
        assert_eq!(parsed.resets_at, parse_timestamp("2026-10-01T00:00:00Z"));
    }

    #[test]
    fn prefers_an_explicit_cli_folder() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("grok.exe");
        fs::write(&executable, b"fixture").unwrap();

        assert_eq!(
            cli_available(Some(directory.path())),
            Some(fs::canonicalize(executable).unwrap())
        );
    }
}
