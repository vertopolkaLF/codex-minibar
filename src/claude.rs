use std::{
    fs,
    sync::Arc,
    time::Duration,
};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use directories::BaseDirs;
use serde::Deserialize;

use crate::{
    limits::{LimitWindow, RateLimits},
    usage,
    worker::{LimitProvider, UsageProvider},
};

const OAUTH_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const OAUTH_BETA: &str = "oauth-2025-04-20";
const FALLBACK_CLAUDE_CODE_VERSION: &str = "2.1.0";

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
        usage::load_cached_usage_statistics(history_days)
    }

    fn refresh_usage_statistics(&mut self, history_days: u16) -> Result<usage::UsageStatistics> {
        usage::refresh_usage_statistics(history_days)
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
}

#[derive(Deserialize)]
struct OAuthUsageWindow {
    utilization: Option<f64>,
    resets_at: Option<String>,
}

pub fn parse_usage_response(response: &str, sampled_at: DateTime<Utc>) -> Result<RateLimits> {
    let response: OAuthUsageResponse = serde_json::from_str(response).context("parse Claude OAuth usage")?;
    let primary = parse_window(response.five_hour, 5 * 60);
    let secondary = parse_window(response.seven_day, 7 * 24 * 60);
    anyhow::ensure!(
        !primary.is_empty() || !secondary.is_empty(),
        "Claude OAuth response does not contain usage windows"
    );
    Ok(RateLimits {
        primary,
        secondary,
        sampled_at,
        plan_type: Some("Claude".into()),
        ..RateLimits::default()
    }
    .normalized(sampled_at))
}

fn parse_window(window: Option<OAuthUsageWindow>, duration_minutes: u32) -> LimitWindow {
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
        duration_minutes: Some(duration_minutes),
    }
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
}
