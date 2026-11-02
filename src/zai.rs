//! Z.ai GLM Coding Plan quota with an API key.
//!
//! `GET /api/monitor/usage/quota/limit` lists the plan's token or credit
//! windows (5 hours, a week) and the monthly MCP allowance. Keys from
//! z.ai and from BigModel (open.bigmodel.cn) use the same API on different
//! hosts; a key the first host rejects is tried on the second, and the host
//! that accepted it is remembered. Reference: CodexBar's `zai.js` plugin.

use anyhow::{Result, bail};
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;

use crate::{
    instances::ProviderId,
    limits::{AdditionalLimit, LimitWindow, RateLimits},
    provider_http::{self, Request},
    usage::UsageStatistics,
    worker::{Activator, LimitProvider, UsageProvider},
};

const HOSTS: [&str; 2] = ["https://api.z.ai", "https://open.bigmodel.cn"];
const QUOTA_PATH: &str = "/api/monitor/usage/quota/limit";
pub(crate) const MCP_LIMIT_ID: &str = "mcp";
const UNAUTHORIZED: &str = "Z.ai rejected the API key; replace it in Settings → Providers";

pub struct ZaiClient {
    agent: ureq::Agent,
    provider: ProviderId,
    /// Index into [`HOSTS`] of the host that last accepted the key.
    host: usize,
}

pub struct ZaiActivator;

impl ZaiClient {
    pub fn new(provider: ProviderId) -> Self {
        Self {
            agent: provider_http::agent(),
            provider,
            host: 0,
        }
    }

    fn fetch(&self, host: usize, key: &str) -> Result<Value> {
        Request {
            provider: "Z.ai",
            unauthorized: UNAUTHORIZED,
        }
        .get_json(
            &self.agent,
            &format!("{}{QUOTA_PATH}", HOSTS[host]),
            &[("Authorization", &format!("Bearer {key}"))],
        )
    }
}

impl LimitProvider for ZaiClient {
    fn read_limits(&mut self) -> Result<RateLimits> {
        let key = crate::provider_keys::require(self.provider)?;
        let body = match self.fetch(self.host, &key) {
            Err(error) if error.to_string() == UNAUTHORIZED => {
                let other = (self.host + 1) % HOSTS.len();
                let body = self.fetch(other, &key)?;
                self.host = other;
                body
            }
            result => result?,
        };
        parse_quota(&body, Utc::now())
    }
}

impl UsageProvider for ZaiClient {
    fn load_cached_usage_statistics(&mut self, _history_days: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }

    fn refresh_usage_statistics(&mut self, _history_days: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }
}

impl Activator for ZaiActivator {
    fn activate(&mut self) -> Result<()> {
        bail!("Z.ai does not support session-window activation")
    }
}

struct Limit {
    kind: String,
    percent: u8,
    minutes: Option<u32>,
    resets_at: Option<DateTime<Utc>>,
}

fn parse_limit(raw: &Value, now: DateTime<Utc>) -> Option<Limit> {
    let kind = raw.get("type")?.as_str()?;
    if !matches!(kind, "TOKENS_LIMIT" | "TIME_LIMIT" | "CREDIT_LIMIT") {
        return None;
    }
    let unit = raw.get("unit")?.as_i64()?;
    let number = raw.get("number")?.as_i64()?;
    let mut percent = provider_http::number(raw.get("percentage"))?;
    let usage = provider_http::number(raw.get("usage"));
    let current = provider_http::number(raw.get("currentValue"));
    let remaining = provider_http::number(raw.get("remaining"));
    // Counts are more precise than the rounded percentage when present.
    if let Some(usage) = usage.filter(|usage| *usage > 0.0) {
        let used = match (remaining, current) {
            (Some(remaining), Some(current)) => Some((usage - remaining).max(current)),
            (Some(remaining), None) => Some(usage - remaining),
            (None, Some(current)) => Some(current),
            (None, None) => None,
        };
        if let Some(used) = used {
            percent = used.clamp(0.0, usage) / usage * 100.0;
        }
    }
    let minutes = if kind == "TIME_LIMIT" && unit == 5 && number == 1 {
        // The MCP allowance is monthly; its unit fields are a marker.
        Some(30 * 24 * 60)
    } else {
        let per_unit = match unit {
            1 => Some(24 * 60),
            3 => Some(60),
            5 => Some(1),
            6 => Some(7 * 24 * 60),
            _ => None,
        };
        per_unit
            .filter(|_| number > 0)
            .and_then(|per_unit| u32::try_from(number * per_unit).ok())
    };
    let mut resets_at = provider_http::timestamp(raw.get("nextResetTime"));
    // A five-hour reset cannot be further away than five hours; a later one
    // is a time-zone slip on the server, not a deadline to show.
    if kind != "TIME_LIMIT"
        && minutes == Some(300)
        && resets_at.is_some_and(|at| at > now + Duration::minutes(301))
    {
        resets_at = None;
    }
    Some(Limit {
        kind: kind.to_owned(),
        percent: percent.clamp(0.0, 100.0).round() as u8,
        minutes,
        resets_at,
    })
}

fn window(limit: &Limit) -> LimitWindow {
    LimitWindow {
        used_percent: Some(limit.percent),
        resets_at: limit.resets_at,
        duration_minutes: limit.minutes,
    }
}

fn parse_quota(body: &Value, now: DateTime<Utc>) -> Result<RateLimits> {
    if body.get("success").and_then(Value::as_bool) != Some(true) {
        let message = body
            .get("msg")
            .and_then(Value::as_str)
            .unwrap_or("unexpected response");
        bail!("Z.ai quota request failed: {message}")
    }
    let data = body.get("data").unwrap_or(&Value::Null);
    let Some(raw_limits) = data.get("limits").and_then(Value::as_array) else {
        bail!("Z.ai returned an unexpected quota response")
    };
    let limits = raw_limits
        .iter()
        .filter_map(|raw| parse_limit(raw, now))
        .collect::<Vec<_>>();
    let mut quotas = limits
        .iter()
        .filter(|limit| limit.kind != "TIME_LIMIT")
        .collect::<Vec<_>>();
    quotas.sort_by_key(|limit| limit.minutes.unwrap_or(u32::MAX));
    let mcp = limits.iter().rfind(|limit| limit.kind == "TIME_LIMIT");
    if quotas.is_empty() && mcp.is_none() {
        bail!("Z.ai reported no Coding Plan quota for this key")
    }
    let primary = quotas.first().copied();
    let secondary = (quotas.len() >= 2).then(|| quotas[quotas.len() - 1]);
    let plan = ["planName", "plan", "plan_type", "packageName", "level"]
        .iter()
        .find_map(|key| data.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|plan| !plan.is_empty())
        .map(str::to_owned);
    Ok(RateLimits {
        primary: primary.map(window).unwrap_or_default(),
        secondary: secondary.map(window).unwrap_or_default(),
        sampled_at: now,
        plan_type: plan,
        limit_name: Some("Z.ai".into()),
        additional_limits: mcp
            .map(|limit| AdditionalLimit {
                id: MCP_LIMIT_ID.into(),
                title: "MCP".into(),
                window: window(limit),
            })
            .into_iter()
            .collect(),
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
    fn maps_session_weekly_and_mcp_windows() {
        let reset = (now() + Duration::hours(3)).timestamp_millis();
        let weekly_reset = (now() + Duration::days(4)).timestamp_millis();
        let body = json!({
            "code": 200, "success": true, "msg": "ok",
            "data": {
                "planName": "Pro",
                "limits": [
                    {"type": "TOKENS_LIMIT", "unit": 6, "number": 1, "percentage": 40,
                     "usage": 1000, "currentValue": 410, "remaining": 590, "nextResetTime": weekly_reset},
                    {"type": "TIME_LIMIT", "unit": 5, "number": 1, "percentage": 12, "usageDetails": []},
                    {"type": "TOKENS_LIMIT", "unit": 3, "number": 5, "percentage": 25, "nextResetTime": reset},
                    {"type": "SOMETHING_NEW", "unit": 1, "number": 1, "percentage": 99}
                ]
            }
        });
        let limits = parse_quota(&body, now()).unwrap();
        assert_eq!(limits.primary.used_percent, Some(25));
        assert_eq!(limits.primary.duration_minutes, Some(300));
        assert_eq!(limits.primary.resets_at.unwrap().timestamp_millis(), reset);
        // Counts win over the rounded percentage: 410 of 1000.
        assert_eq!(limits.secondary.used_percent, Some(41));
        assert_eq!(limits.secondary.duration_minutes, Some(10_080));
        assert_eq!(limits.plan_type.as_deref(), Some("Pro"));
        let mcp = &limits.additional_limits[0];
        assert_eq!(mcp.id, MCP_LIMIT_ID);
        assert_eq!(mcp.window.used_percent, Some(12));
        assert_eq!(mcp.window.duration_minutes, Some(43_200));
    }

    #[test]
    fn drops_an_implausible_five_hour_reset() {
        let late = (now() + Duration::hours(10)).timestamp_millis();
        let body = json!({"success": true, "data": {"limits": [
            {"type": "TOKENS_LIMIT", "unit": 3, "number": 5, "percentage": 5, "nextResetTime": late}
        ]}});
        let limits = parse_quota(&body, now()).unwrap();
        assert_eq!(limits.primary.resets_at, None);
        assert!(limits.secondary.is_empty());
    }

    #[test]
    fn reports_api_errors_and_missing_plans() {
        let error = parse_quota(
            &json!({"success": false, "code": 500, "msg": "no plan"}),
            now(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("no plan"));
        assert!(parse_quota(&json!({"success": true, "data": {"limits": []}}), now()).is_err());
        assert!(parse_quota(&json!({"success": true, "data": {}}), now()).is_err());
    }
}
