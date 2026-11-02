//! Zed edit-prediction allowance and plan from the Zed editor's sign-in.
//!
//! Zed stores its GitHub sign-in with GPUI's credential API: Windows
//! Credential Manager target `zed:url=https://zed.dev`, or a Secret Service
//! item labeled `zed-github-account` with `url = https://zed.dev` on Linux
//! (see `vendor/gpui`'s `read_credentials`). The user id and access token
//! authorize `GET https://cloud.zed.dev/client/users/me`, which reports the
//! plan, the edit predictions used this billing period and the period.
//! Reference: CodexBar's `ZedStatusProbe` and `zed.js` plugin.

use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::{
    limits::{LimitWindow, RateLimits},
    provider_http::{self, Request},
    usage::UsageStatistics,
    worker::{Activator, LimitProvider, UsageProvider},
};

const ZED_URL: &str = "https://zed.dev";
const USER_URL: &str = "https://cloud.zed.dev/client/users/me";

pub struct ZedClient {
    agent: ureq::Agent,
}

pub struct ZedActivator;

impl ZedClient {
    pub fn new() -> Self {
        Self {
            agent: provider_http::agent(),
        }
    }
}

impl Default for ZedClient {
    fn default() -> Self {
        Self::new()
    }
}

impl LimitProvider for ZedClient {
    fn read_limits(&mut self) -> Result<RateLimits> {
        let Some((user_id, token)) = credentials() else {
            bail!("Zed is not signed in; sign in from the Zed editor")
        };
        let body = Request {
            provider: "Zed",
            unauthorized: "Zed rejected its saved sign-in; sign in to Zed again",
        }
        .get_json(
            &self.agent,
            USER_URL,
            &[("Authorization", &format!("{user_id} {token}"))],
        )?;
        parse_user(&body, Utc::now())
    }
}

impl UsageProvider for ZedClient {
    fn load_cached_usage_statistics(&mut self, _history_days: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }

    fn refresh_usage_statistics(&mut self, _history_days: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }
}

impl Activator for ZedActivator {
    fn activate(&mut self) -> Result<()> {
        bail!("Zed does not support session-window activation")
    }
}

/// Whether the Zed editor's sign-in is on this computer.
pub fn is_installed() -> bool {
    credentials().is_some()
}

#[cfg(windows)]
fn credentials() -> Option<(String, String)> {
    use std::ffi::c_void;
    use windows_sys::Win32::Security::Credentials::{
        CRED_TYPE_GENERIC, CREDENTIALW, CredFree, CredReadW,
    };

    let target = format!("zed:url={ZED_URL}")
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
    // SAFETY: `target` is NUL-terminated and `credential` receives a buffer
    // that is released with `CredFree` below.
    let found = unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) } != 0;
    if !found || credential.is_null() {
        return None;
    }
    // SAFETY: CredReadW succeeded, so the pointer is a valid CREDENTIALW.
    let value = unsafe { &*credential };
    let user = if value.UserName.is_null() {
        String::new()
    } else {
        // SAFETY: UserName is a NUL-terminated wide string owned by the buffer.
        let length = (0..)
            .take_while(|&index| unsafe { *value.UserName.add(index) } != 0)
            .count();
        String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(value.UserName, length) })
    };
    let size = usize::try_from(value.CredentialBlobSize).unwrap_or_default();
    let token = if size == 0 || value.CredentialBlob.is_null() {
        String::new()
    } else {
        // SAFETY: the blob is `CredentialBlobSize` bytes owned by the buffer.
        String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(value.CredentialBlob, size) })
            .into_owned()
    };
    // SAFETY: releases the buffer CredReadW allocated.
    unsafe { CredFree(credential.cast::<c_void>()) };
    usable(user, token)
}

#[cfg(not(windows))]
fn credentials() -> Option<(String, String)> {
    futures::executor::block_on(async {
        let keyring = oo7::Keyring::new().await.ok()?;
        keyring.unlock().await.ok()?;
        let items = keyring.search_items(&vec![("url", ZED_URL)]).await.ok()?;
        for item in items {
            if item.label().await.ok().as_deref() != Some("zed-github-account") {
                continue;
            }
            let Some(user) = item.attributes().await.ok()?.get("username").cloned() else {
                continue;
            };
            if item.unlock().await.is_err() {
                continue;
            }
            let Ok(secret) = item.secret().await else {
                continue;
            };
            if let Some(found) = usable(user, String::from_utf8_lossy(&secret).into_owned()) {
                return Some(found);
            }
        }
        None
    })
}

fn usable(user: String, token: String) -> Option<(String, String)> {
    let (user, token) = (user.trim().to_owned(), token.trim().to_owned());
    (!user.is_empty() && !token.is_empty()).then_some((user, token))
}

/// `zed_pro` → `Zed Pro`.
fn plan_label(plan: &str) -> String {
    plan.split(['_', ' '])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut characters = word.chars();
            characters.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + &characters.as_str().to_lowercase()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_user(body: &Value, now: DateTime<Utc>) -> Result<RateLimits> {
    let Some(plan) = body.get("plan") else {
        bail!("Zed returned an unexpected account response")
    };
    let plan_name = plan
        .get("plan_v3")
        .or_else(|| plan.get("plan"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|plan| !plan.is_empty())
        .map(plan_label);
    let period_start = provider_http::timestamp(plan.pointer("/subscription_period/started_at"));
    let period_end = provider_http::timestamp(plan.pointer("/subscription_period/ended_at"));
    let predictions = plan.pointer("/usage/edit_predictions");
    let used = provider_http::number(predictions.and_then(|value| value.get("used")));
    let limit = predictions
        .and_then(|value| value.get("limit"))
        .and_then(|limit| {
            provider_http::number(Some(limit))
                .or_else(|| provider_http::number(limit.get("limited")))
        });
    let duration = match (period_start, period_end) {
        (Some(start), Some(end)) if end > start => u32::try_from((end - start).num_minutes()).ok(),
        _ => None,
    };
    let primary = match (used, limit) {
        (Some(used), Some(limit)) if limit > 0.0 => LimitWindow {
            used_percent: provider_http::percent(used, limit),
            resets_at: period_end,
            duration_minutes: duration,
        },
        // Unlimited plans have no allowance to show.
        _ => LimitWindow::default(),
    };
    let account = body
        .pointer("/user/github_login")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|login| !login.is_empty())
        .map(str::to_owned);
    if primary.is_empty() && plan_name.is_none() {
        bail!("Zed returned no plan for this account")
    }
    Ok(RateLimits {
        primary,
        sampled_at: now,
        account_name: account,
        plan_type: plan_name,
        limit_name: Some("Zed".into()),
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
    fn reads_edit_predictions_for_the_billing_period() {
        let body = json!({
            "user": {"id": 42, "github_login": "octocat", "name": "Octo"},
            "plan": {
                "plan_v3": "zed_free",
                "usage": {"edit_predictions": {"used": 500, "limit": {"limited": 2000}}},
                "subscription_period": {"started_at": "2026-10-20T00:00:00Z", "ended_at": "2026-11-20T00:00:00Z"},
                "has_overdue_invoices": false
            }
        });
        let limits = parse_user(&body, now()).unwrap();
        assert_eq!(limits.primary.used_percent, Some(25));
        assert_eq!(
            limits.primary.resets_at,
            Some(Utc.with_ymd_and_hms(2026, 11, 20, 0, 0, 0).unwrap())
        );
        assert_eq!(limits.primary.duration_minutes, Some(31 * 24 * 60));
        assert_eq!(limits.plan_type.as_deref(), Some("Zed Free"));
        assert_eq!(limits.account_name.as_deref(), Some("octocat"));
    }

    #[test]
    fn unlimited_plans_show_the_plan_only() {
        let body = json!({
            "user": {"github_login": "octocat"},
            "plan": {"plan_v3": "zed_pro", "usage": {"edit_predictions": {"used": 9000, "limit": "unlimited"}}}
        });
        let limits = parse_user(&body, now()).unwrap();
        assert!(limits.primary.is_empty());
        assert_eq!(limits.plan_type.as_deref(), Some("Zed Pro"));
        assert!(parse_user(&json!({"user": {}}), now()).is_err());
        assert!(parse_user(&json!({"plan": {}}), now()).is_err());
    }

    #[test]
    fn credentials_need_both_parts() {
        assert_eq!(
            usable(" 42 ".into(), "tok\n".into()),
            Some(("42".into(), "tok".into()))
        );
        assert_eq!(usable(String::new(), "tok".into()), None);
        assert_eq!(usable("42".into(), " ".into()), None);
    }
}
