//! Kimi Code plan quota with a Kimi Code API key.
//!
//! `GET /coding/v1/usages` reports the 5-hour and weekly windows as used
//! ratios (`usages.limit_5h`, `usages.limit_7d`), older counters
//! (`limits[0]`, `usage`) and the shared monthly pool. Keys from kimi.com and
//! kimi.ai use different hosts; a key the first rejects is tried on the
//! second. Reference: CodexBar's `KimiUsageFetcher` and `KimiUsageSnapshot`.

use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::{
    instances::ProviderId,
    limits::{AdditionalLimit, LimitWindow, RateLimits},
    provider_http::{self, Request},
    usage::UsageStatistics,
    worker::{Activator, LimitProvider, UsageProvider},
};

const HOSTS: [&str; 2] = ["https://api.kimi.com", "https://api.kimi.ai"];
const USAGE_PATH: &str = "/coding/v1/usages";
pub(crate) const MONTHLY_LIMIT_ID: &str = "monthly";
const SESSION_MINUTES: u32 = 5 * 60;
const WEEKLY_MINUTES: u32 = 7 * 24 * 60;
const MONTHLY_MINUTES: u32 = 30 * 24 * 60;
const UNAUTHORIZED: &str = "Kimi rejected the API key; replace it in Settings → Providers";

pub struct KimiClient {
    agent: ureq::Agent,
    provider: ProviderId,
    host: usize,
}

pub struct KimiActivator;

impl KimiClient {
    pub fn new(provider: ProviderId) -> Self {
        Self {
            agent: provider_http::agent(),
            provider,
            host: 0,
        }
    }

    fn fetch(&self, host: usize, key: &str) -> Result<Value> {
        Request {
            provider: "Kimi",
            unauthorized: UNAUTHORIZED,
        }
        .get_json(
            &self.agent,
            &format!("{}{USAGE_PATH}", HOSTS[host]),
            &[("Authorization", &format!("Bearer {key}"))],
        )
    }
}

impl LimitProvider for KimiClient {
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
        parse_usage(&body, Utc::now())
    }
}

impl UsageProvider for KimiClient {
    fn load_cached_usage_statistics(&mut self, _history_days: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }

    fn refresh_usage_statistics(&mut self, _history_days: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }
}

impl Activator for KimiActivator {
    fn activate(&mut self) -> Result<()> {
        bail!("Kimi does not support session-window activation")
    }
}

/// `{"used_ratio": 0.42, "reset_time": "…"}`.
fn ratio_window(pool: Option<&Value>, minutes: u32) -> Option<LimitWindow> {
    let pool = pool?;
    let ratio = provider_http::number(pool.get("used_ratio")).filter(|ratio| *ratio >= 0.0)?;
    Some(LimitWindow {
        used_percent: Some((ratio * 100.0).clamp(0.0, 100.0).round() as u8),
        resets_at: provider_http::timestamp(pool.get("reset_time")),
        duration_minutes: Some(minutes),
    })
}

/// `{"limit": "100", "used": "20", "remaining": "80", "resetTime": "…"}`.
fn count_window(detail: Option<&Value>, minutes: u32) -> Option<LimitWindow> {
    let detail = detail?;
    let limit = provider_http::number(detail.get("limit")).filter(|limit| *limit > 0.0)?;
    let used = provider_http::number(detail.get("used"))
        .filter(|used| *used >= 0.0)
        .or_else(|| {
            provider_http::number(detail.get("remaining"))
                .filter(|remaining| (0.0..=limit).contains(remaining))
                .map(|remaining| limit - remaining)
        })?;
    let reset = ["resetTime", "reset_time", "resetAt", "reset_at"]
        .iter()
        .find_map(|key| provider_http::timestamp(detail.get(*key)));
    Some(LimitWindow {
        used_percent: provider_http::percent(used, limit),
        resets_at: reset,
        duration_minutes: Some(minutes),
    })
}

/// The ratio pool, unless the counters for the same window report more use.
fn more_used(ratio: Option<LimitWindow>, count: Option<LimitWindow>) -> LimitWindow {
    match (ratio, count) {
        (Some(ratio), Some(count)) if count.used_percent > ratio.used_percent => count,
        (Some(ratio), _) => ratio,
        (None, Some(count)) => count,
        (None, None) => LimitWindow::default(),
    }
}

fn rate_limit_minutes(limit: &Value) -> Option<u32> {
    let window = limit.get("window")?;
    let duration = u32::try_from(window.get("duration")?.as_i64()?).ok()?;
    let per_unit = match window.get("timeUnit")?.as_str()? {
        "TIME_UNIT_MINUTE" => 1,
        "TIME_UNIT_HOUR" => 60,
        "TIME_UNIT_DAY" => 24 * 60,
        _ => return None,
    };
    duration
        .checked_mul(per_unit)
        .filter(|minutes| *minutes > 0)
}

fn plan_name(level: &str) -> Option<&'static str> {
    Some(match level {
        "LEVEL_FREE" => "Adagio",
        "LEVEL_TRIAL" => "Andante",
        "LEVEL_BASIC" => "Moderato",
        "LEVEL_INTERMEDIATE" => "Allegretto",
        "LEVEL_ADVANCED" => "Allegro",
        _ => return None,
    })
}

fn parse_usage(body: &Value, now: DateTime<Utc>) -> Result<RateLimits> {
    let pools = body.get("usages");
    let rate_limit = body
        .get("limits")
        .and_then(Value::as_array)
        .and_then(|limits| limits.first());
    let session_minutes = rate_limit
        .and_then(rate_limit_minutes)
        .unwrap_or(SESSION_MINUTES);
    let session = more_used(
        ratio_window(
            pools.and_then(|pools| pools.get("limit_5h")),
            SESSION_MINUTES,
        ),
        count_window(
            rate_limit.and_then(|limit| limit.get("detail")),
            session_minutes,
        ),
    );
    let weekly = more_used(
        ratio_window(
            pools.and_then(|pools| pools.get("limit_7d")),
            WEEKLY_MINUTES,
        ),
        count_window(body.get("usage"), WEEKLY_MINUTES),
    );
    let monthly = ratio_window(
        pools.and_then(|pools| pools.get("limit_month_total")),
        MONTHLY_MINUTES,
    );
    if session.is_empty() && weekly.is_empty() && monthly.is_none() {
        bail!("Kimi reported no Kimi Code quota for this key")
    }
    let plan = body
        .pointer("/user/membership/level")
        .and_then(Value::as_str)
        .and_then(|level| plan_name(level.trim()))
        .map(str::to_owned);
    Ok(RateLimits {
        primary: session,
        secondary: weekly,
        sampled_at: now,
        plan_type: plan,
        limit_name: Some("Kimi Code".into()),
        additional_limits: monthly
            .map(|window| AdditionalLimit {
                id: MONTHLY_LIMIT_ID.into(),
                title: "Total usage".into(),
                window,
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
    fn prefers_ratio_pools_and_reads_the_monthly_pool() {
        let body = json!({
            "usages": {
                "limit_5h": {"used_ratio": 0.25, "reset_time": "2026-11-02T13:00:00Z"},
                "limit_7d": {"used_ratio": 0.5, "reset_time": "2026-11-06T00:00:00Z"},
                "limit_month_total": {"used_ratio": 0.1, "reset_time": "2026-12-01T00:00:00Z"}
            },
            "usage": {"limit": "100", "used": "20", "resetTime": "2026-11-06T00:00:00Z"},
            "limits": [{"window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                        "detail": {"limit": "50", "remaining": "40"}}],
            "user": {"membership": {"level": "LEVEL_INTERMEDIATE"}}
        });
        let limits = parse_usage(&body, now()).unwrap();
        assert_eq!(limits.primary.used_percent, Some(25));
        assert_eq!(limits.primary.duration_minutes, Some(300));
        assert_eq!(limits.secondary.used_percent, Some(50));
        assert_eq!(
            limits.secondary.resets_at,
            Some(Utc.with_ymd_and_hms(2026, 11, 6, 0, 0, 0).unwrap())
        );
        assert_eq!(limits.plan_type.as_deref(), Some("Allegretto"));
        assert_eq!(limits.additional_limits[0].window.used_percent, Some(10));
    }

    #[test]
    fn falls_back_to_counters_and_takes_the_more_used_reading() {
        let body = json!({
            "usages": {"limit_7d": {"used_ratio": 0.1}},
            "usage": {"limit": 200, "used": 60},
            "limits": [{"window": {"duration": 5, "timeUnit": "TIME_UNIT_HOUR"},
                        "detail": {"limit": "100", "remaining": "70", "reset_time": "2026-11-02T12:00:00Z"}}]
        });
        let limits = parse_usage(&body, now()).unwrap();
        assert_eq!(limits.primary.used_percent, Some(30));
        assert_eq!(limits.primary.duration_minutes, Some(300));
        // 60 of 200 counted beats the 10% ratio.
        assert_eq!(limits.secondary.used_percent, Some(30));
        assert!(limits.additional_limits.is_empty());
        assert_eq!(limits.plan_type, None);
    }

    #[test]
    fn an_empty_response_is_an_error() {
        assert!(parse_usage(&json!({}), now()).is_err());
        assert!(parse_usage(&json!({"usage": {"limit": "0"}}), now()).is_err());
    }
}
