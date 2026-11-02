//! GitHub Copilot monthly quotas.
//!
//! `GET https://api.github.com/copilot_internal/user` reports the premium
//! request and chat allowances of the signed-in GitHub account. The token is
//! the one a Copilot editor extension (VS Code, JetBrains, Neovim, Copilot
//! CLI) saved in `github-copilot/apps.json`, or a GitHub token pasted in
//! Settings. Reference: CodexBar's `CopilotUsageFetcher`.

use std::path::PathBuf;

use anyhow::{Result, bail};
use chrono::{DateTime, NaiveDate, Utc};
use serde_json::Value;

use crate::{
    instances::ProviderId,
    limits::{LimitWindow, RateLimits},
    provider_http::{self, Request},
    usage::UsageStatistics,
    worker::{Activator, LimitProvider, UsageProvider},
};

const USAGE_URL: &str = "https://api.github.com/copilot_internal/user";

pub struct CopilotClient {
    agent: ureq::Agent,
    provider: ProviderId,
}

pub struct CopilotActivator;

impl CopilotClient {
    pub fn new(provider: ProviderId) -> Self {
        Self {
            agent: provider_http::agent(),
            provider,
        }
    }
}

/// A GitHub token and the account it belongs to, when known.
struct Token {
    value: String,
    user: Option<String>,
}

fn token(provider: ProviderId) -> Result<Token> {
    if let Some(value) = crate::provider_keys::resolve(provider)? {
        return Ok(Token { value, user: None });
    }
    // Extra instances track the account of their own pasted token.
    if provider.is_primary()
        && let Some(token) = editor_token()
    {
        return Ok(token);
    }
    bail!(
        "Copilot is not signed in; sign in to Copilot in your editor or add a GitHub token in Settings → Providers"
    )
}

impl LimitProvider for CopilotClient {
    fn read_limits(&mut self) -> Result<RateLimits> {
        let token = token(self.provider)?;
        let body = Request {
            provider: "Copilot",
            unauthorized: "GitHub rejected the Copilot token; sign in to Copilot again or replace the token",
        }
        .get_json(
            &self.agent,
            USAGE_URL,
            &[
                ("Authorization", &format!("token {}", token.value)),
                ("Editor-Version", "vscode/1.96.2"),
                ("Editor-Plugin-Version", "copilot-chat/0.26.7"),
                ("X-Github-Api-Version", "2025-04-01"),
            ],
        )?;
        let mut limits = parse_usage(&body, Utc::now())?;
        limits.account_name = token.user;
        Ok(limits)
    }
}

impl UsageProvider for CopilotClient {
    fn load_cached_usage_statistics(&mut self, _history_days: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }

    fn refresh_usage_statistics(&mut self, _history_days: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }
}

impl Activator for CopilotActivator {
    fn activate(&mut self) -> Result<()> {
        bail!("Copilot does not support session-window activation")
    }
}

/// Whether a Copilot editor sign-in is on this computer.
pub fn is_installed() -> bool {
    editor_token().is_some()
}

/// Where Copilot editor extensions keep their GitHub sign-in.
fn config_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        return std::env::var_os("LOCALAPPDATA")
            .map(|dir| PathBuf::from(dir).join("github-copilot"));
    }
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".config")))
        .map(|dir| dir.join("github-copilot"))
}

fn editor_token() -> Option<Token> {
    let dir = config_dir()?;
    ["apps.json", "hosts.json"].iter().find_map(|name| {
        let raw = std::fs::read_to_string(dir.join(name)).ok()?;
        parse_editor_token(&raw)
    })
}

/// `{"github.com:Iv1.…": {"user": "octocat", "oauth_token": "gho_…"}}` in
/// `apps.json`, or `{"github.com": {…}}` in the older `hosts.json`.
fn parse_editor_token(raw: &str) -> Option<Token> {
    let root: Value = serde_json::from_str(raw).ok()?;
    root.as_object()?
        .iter()
        .filter(|(host, _)| *host == "github.com" || host.starts_with("github.com:"))
        .find_map(|(_, entry)| {
            let value = entry.get("oauth_token")?.as_str()?.trim();
            (!value.is_empty()).then(|| Token {
                value: value.to_owned(),
                user: entry.get("user").and_then(Value::as_str).map(str::to_owned),
            })
        })
}

/// One quota snapshot as a used-percentage window, or `None` when it is
/// unlimited or a placeholder without a real allowance.
fn quota_window(snapshot: Option<&Value>, resets_at: Option<DateTime<Utc>>) -> Option<LimitWindow> {
    let snapshot = snapshot?;
    if snapshot.get("unlimited").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let entitlement = provider_http::number(snapshot.get("entitlement"));
    let remaining = provider_http::number(snapshot.get("remaining"));
    // Business seats with token billing report zero of zero: no quota.
    if entitlement == Some(0.0) && remaining == Some(0.0) {
        return None;
    }
    let percent_remaining =
        provider_http::number(snapshot.get("percent_remaining")).or_else(|| {
            let entitlement = entitlement.filter(|entitlement| *entitlement > 0.0)?;
            Some(remaining? / entitlement * 100.0)
        })?;
    Some(LimitWindow {
        used_percent: Some((100.0 - percent_remaining).clamp(0.0, 100.0).round() as u8),
        resets_at,
        duration_minutes: None,
    })
}

/// Free plans report `monthly_quotas` and what is left in
/// `limited_user_quotas` instead of snapshots.
fn counted_window(
    body: &Value,
    key: &str,
    resets_at: Option<DateTime<Utc>>,
) -> Option<LimitWindow> {
    let monthly = provider_http::number(body.pointer(&format!("/monthly_quotas/{key}")))?;
    let left = provider_http::number(body.pointer(&format!("/limited_user_quotas/{key}")))?;
    let used = (monthly - left.max(0.0)).max(0.0);
    Some(LimitWindow {
        used_percent: provider_http::percent(used, monthly),
        resets_at,
        duration_minutes: None,
    })
}

fn reset_date(value: Option<&Value>) -> Option<DateTime<Utc>> {
    let text = value?.as_str()?.trim();
    provider_http::timestamp(value).or_else(|| {
        NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .ok()?
            .and_hms_opt(0, 0, 0)
            .map(|at| at.and_utc())
    })
}

fn parse_usage(body: &Value, now: DateTime<Utc>) -> Result<RateLimits> {
    if body.get("access_type_sku").and_then(Value::as_str) == Some("subscription_ended") {
        bail!("The Copilot subscription of this GitHub account has ended")
    }
    let resets_at = reset_date(body.get("quota_reset_date"));
    let snapshots = body.get("quota_snapshots");
    let premium = quota_window(
        snapshots.and_then(|snapshots| snapshots.get("premium_interactions")),
        resets_at,
    )
    .or_else(|| counted_window(body, "completions", resets_at));
    let chat = quota_window(
        snapshots.and_then(|snapshots| snapshots.get("chat")),
        resets_at,
    )
    .or_else(|| counted_window(body, "chat", resets_at));
    let plan = body
        .get("copilot_plan")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|plan| !plan.is_empty() && *plan != "unknown")
        // The popup capitalizes plans; `individual_pro` reads "Individual pro".
        .map(|plan| plan.replace('_', " "));
    let metered = premium.is_some() || chat.is_some();
    let unlimited_or_token_billed = body.get("token_based_billing").and_then(Value::as_bool)
        == Some(true)
        || ["premium_interactions", "chat"].iter().any(|key| {
            snapshots
                .and_then(|snapshots| snapshots.get(*key))
                .and_then(|snapshot| snapshot.get("unlimited"))
                .and_then(Value::as_bool)
                == Some(true)
        });
    if !metered && !unlimited_or_token_billed {
        bail!("Copilot returned no quota for this account")
    }
    Ok(RateLimits {
        primary: premium.unwrap_or_default(),
        secondary: chat.unwrap_or_default(),
        sampled_at: now,
        plan_type: plan,
        limit_name: Some("Copilot".into()),
        ..RateLimits::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 2, 10, 0, 0).unwrap()
    }

    #[test]
    fn reads_premium_and_chat_snapshots() {
        let body = json!({
            "copilot_plan": "individual_pro",
            "quota_reset_date": "2026-12-01",
            "quota_snapshots": {
                "premium_interactions": {"entitlement": 300, "remaining": 210, "percent_remaining": 70.0,
                                          "quota_id": "premium_interactions", "unlimited": false},
                "chat": {"entitlement": 0, "remaining": 0, "percent_remaining": 100.0,
                         "quota_id": "chat", "unlimited": true}
            }
        });
        let limits = parse_usage(&body, now()).unwrap();
        assert_eq!(limits.primary.used_percent, Some(30));
        assert_eq!(
            limits.primary.resets_at,
            Some(Utc.with_ymd_and_hms(2026, 12, 1, 0, 0, 0).unwrap())
        );
        assert!(limits.secondary.is_empty());
        assert_eq!(limits.plan_type.as_deref(), Some("individual pro"));
    }

    #[test]
    fn free_plans_use_monthly_counts() {
        let body = json!({
            "copilot_plan": "free",
            "quota_reset_date": "2026-11-15T00:00:00Z",
            "monthly_quotas": {"chat": 50, "completions": 2000},
            "limited_user_quotas": {"chat": 35, "completions": 1500}
        });
        let limits = parse_usage(&body, now()).unwrap();
        assert_eq!(limits.primary.used_percent, Some(25));
        assert_eq!(limits.secondary.used_percent, Some(30));
    }

    #[test]
    fn placeholders_ended_and_empty_accounts() {
        let token_billed = json!({
            "copilot_plan": "business", "token_based_billing": true,
            "quota_snapshots": {"premium_interactions": {"entitlement": 0, "remaining": 0, "percent_remaining": 100}}
        });
        let limits = parse_usage(&token_billed, now()).unwrap();
        assert!(limits.primary.is_empty() && limits.secondary.is_empty());
        assert!(parse_usage(&json!({"access_type_sku": "subscription_ended"}), now()).is_err());
        assert!(parse_usage(&json!({"copilot_plan": "pro"}), now()).is_err());
    }

    #[test]
    fn reads_editor_sign_ins() {
        let apps = r#"{"github.com:Iv1.b507a08c87ecfe98": {"user": "octocat", "oauth_token": "gho_abc", "githubAppId": "Iv1.b507a08c87ecfe98"}}"#;
        let token = parse_editor_token(apps).unwrap();
        assert_eq!(token.value, "gho_abc");
        assert_eq!(token.user.as_deref(), Some("octocat"));
        let hosts = r#"{"github.com": {"user": "octo", "oauth_token": "ghu_x"}}"#;
        assert_eq!(parse_editor_token(hosts).unwrap().value, "ghu_x");
        assert!(parse_editor_token(r#"{"ghe.example.com": {"oauth_token": "x"}}"#).is_none());
        assert!(parse_editor_token("not json").is_none());
    }
}
