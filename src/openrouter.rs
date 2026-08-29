//! OpenRouter API-key quota provider.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Datelike, Duration as ChronoDuration, NaiveDate, TimeZone, Utc};
use serde::Deserialize;

use crate::{
    limits::{LimitWindow, RateLimits, SpendingSummary},
    secrets,
    usage::UsageStatistics,
    worker::{Activator, LimitProvider, UsageProvider},
};

const API_URL: &str = "https://openrouter.ai/api/v1/key";
const CREDITS_API_URL: &str = "https://openrouter.ai/api/v1/credits";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const SECRET_NAME: &str = "openrouter-api-key";

pub struct OpenRouterClient {
    agent: ureq::Agent,
}

pub struct OpenRouterActivator;

impl Default for OpenRouterClient {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenRouterClient {
    pub fn new() -> Self {
        Self {
            agent: ureq::AgentBuilder::new().timeout(REQUEST_TIMEOUT).build(),
        }
    }

    fn read_key(&self) -> Result<RateLimits> {
        let api_key =
            secrets::load(SECRET_NAME)?.context("OpenRouter API key is not configured")?;
        let sampled_at = Utc::now();
        let response = self
            .agent
            .get(API_URL)
            .set("Authorization", &format!("Bearer {api_key}"))
            .set("Accept", "application/json")
            .call()
            .context("request OpenRouter API-key usage")?;
        let body = response
            .into_string()
            .context("read OpenRouter API-key response")?;
        let mut limits = parse_key_response(&body, sampled_at)?;
        // Account credits are available only to management keys. Keep the
        // regular key-level usage useful when /credits returns 403, and also
        // tolerate a transient failure of this optional second request.
        if let Ok(Some(balance)) = self.read_account_balance(&api_key)
            && let Some(spending) = limits.spending.as_mut()
        {
            spending.balance_microusd = Some(balance);
        }
        Ok(limits)
    }

    fn read_account_balance(&self, api_key: &str) -> Result<Option<u64>> {
        let response = match self
            .agent
            .get(CREDITS_API_URL)
            .set("Authorization", &format!("Bearer {api_key}"))
            .set("Accept", "application/json")
            .call()
        {
            Ok(response) => response,
            // OpenRouter intentionally rejects ordinary API keys here. That
            // is expected and must not hide the data already returned by /key.
            Err(ureq::Error::Status(403, _)) => return Ok(None),
            Err(error) => return Err(error).context("request OpenRouter account credits"),
        };
        let body = response
            .into_string()
            .context("read OpenRouter account credits response")?;
        parse_credits_response(&body).map(Some)
    }
}

pub fn is_installed() -> bool {
    key_is_configured()
}

pub fn key_is_configured() -> bool {
    secrets::load(SECRET_NAME)
        .ok()
        .flatten()
        .is_some_and(|key| !key.trim().is_empty())
}

pub fn save_api_key(value: Option<&str>) -> Result<()> {
    secrets::save(SECRET_NAME, value)
}

impl LimitProvider for OpenRouterClient {
    fn read_limits(&mut self) -> Result<RateLimits> {
        self.read_key()
    }
}

impl UsageProvider for OpenRouterClient {
    fn load_cached_usage_statistics(&mut self, _history_days: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }

    fn refresh_usage_statistics(&mut self, _history_days: u16) -> Result<UsageStatistics> {
        // `/key` reports aggregate dollar usage, not token history. Do not
        // manufacture a daily chart or pretend that one aggregate is today's
        // spend; the provider summary renders the authoritative value.
        Ok(UsageStatistics::default())
    }
}

impl Activator for OpenRouterActivator {
    fn activate(&mut self) -> Result<()> {
        bail!("OpenRouter does not support session-window activation")
    }
}

#[derive(Debug, Deserialize)]
struct KeyEnvelope {
    data: KeyData,
}

#[derive(Debug, Deserialize)]
struct KeyData {
    label: Option<String>,
    usage: Option<f64>,
    limit: Option<f64>,
    limit_remaining: Option<f64>,
    limit_reset: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreditsEnvelope {
    data: CreditsData,
}

#[derive(Debug, Deserialize)]
struct CreditsData {
    total_credits: f64,
    total_usage: f64,
}

fn parse_key_response(raw: &str, sampled_at: DateTime<Utc>) -> Result<RateLimits> {
    let envelope: KeyEnvelope =
        serde_json::from_str(raw).context("parse OpenRouter API-key response")?;
    let usage = money_value(envelope.data.usage, "usage")?.unwrap_or(0);
    let limit = money_value(envelope.data.limit, "limit")?;
    let remaining = money_value(envelope.data.limit_remaining, "limit_remaining")?;
    let reset_kind = envelope
        .data
        .limit_reset
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase);
    let bounds = reset_kind
        .as_deref()
        .and_then(|kind| period_bounds(sampled_at, kind));
    let used_percent = limit.and_then(|limit| {
        (limit > 0).then(|| {
            let used = usage.min(limit);
            ((used as f64 / limit as f64) * 100.0)
                .round()
                .clamp(0.0, 100.0) as u8
        })
    });
    let derived_remaining = remaining.or_else(|| limit.map(|limit| limit.saturating_sub(usage)));
    let account_name = envelope
        .data
        .label
        .as_deref()
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .map(str::to_owned);

    Ok(RateLimits {
        primary: LimitWindow {
            used_percent,
            resets_at: bounds.map(|(_, reset, _)| reset),
            duration_minutes: bounds.map(|(_, _, minutes)| minutes),
        },
        sampled_at,
        account_name,
        spending: Some(SpendingSummary {
            used_microusd: usage,
            limit_microusd: limit,
            remaining_microusd: derived_remaining,
            resets_at: bounds.map(|(_, reset, _)| reset),
            reset_kind,
            balance_microusd: None,
        }),
        ..RateLimits::default()
    })
}

fn parse_credits_response(raw: &str) -> Result<u64> {
    let envelope: CreditsEnvelope =
        serde_json::from_str(raw).context("parse OpenRouter account credits response")?;
    let total_credits = money_value(Some(envelope.data.total_credits), "total_credits")?
        .context("OpenRouter credits response is missing total_credits")?;
    let total_usage = money_value(Some(envelope.data.total_usage), "total_usage")?
        .context("OpenRouter credits response is missing total_usage")?;
    Ok(total_credits.saturating_sub(total_usage))
}

fn money_value(value: Option<f64>, field: &str) -> Result<Option<u64>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if !value.is_finite() || value < 0.0 {
        bail!("OpenRouter {field} is not a valid non-negative amount")
    }
    let micros = value * 1_000_000.0;
    if micros > u64::MAX as f64 {
        bail!("OpenRouter {field} is too large")
    }
    Ok(Some(micros.round() as u64))
}

/// Returns the current UTC period start, reset boundary, and exact duration.
fn period_bounds(
    now: DateTime<Utc>,
    reset_kind: &str,
) -> Option<(DateTime<Utc>, DateTime<Utc>, u32)> {
    let date = now.date_naive();
    let start_date = match reset_kind {
        "daily" => date,
        "weekly" => date - ChronoDuration::days(i64::from(date.weekday().num_days_from_monday())),
        "monthly" => NaiveDate::from_ymd_opt(date.year(), date.month(), 1)?,
        _ => return None,
    };
    let reset_date = match reset_kind {
        "daily" => start_date + ChronoDuration::days(1),
        "weekly" => start_date + ChronoDuration::days(7),
        "monthly" => {
            if date.month() == 12 {
                NaiveDate::from_ymd_opt(date.year().checked_add(1)?, 1, 1)?
            } else {
                NaiveDate::from_ymd_opt(date.year(), date.month() + 1, 1)?
            }
        }
        _ => return None,
    };
    let start = Utc.from_utc_datetime(&start_date.and_hms_opt(0, 0, 0)?);
    let reset = Utc.from_utc_datetime(&reset_date.and_hms_opt(0, 0, 0)?);
    let minutes = u32::try_from((reset - start).num_minutes()).ok()?;
    Some((start, reset, minutes))
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn sample(raw: &str) -> RateLimits {
        parse_key_response(raw, Utc.with_ymd_and_hms(2026, 8, 19, 12, 30, 0).unwrap()).unwrap()
    }

    #[test]
    fn maps_a_monthly_capped_key_to_a_spending_window() {
        let limits = sample(
            r#"{"data":{"label":"Build key","usage":25.5,"limit":100,"limit_remaining":74.5,"limit_reset":"monthly"}}"#,
        );
        assert_eq!(limits.primary.used_percent, Some(26));
        assert_eq!(limits.primary.duration_minutes, Some(31 * 24 * 60));
        assert_eq!(
            limits.primary.resets_at,
            Some(Utc.with_ymd_and_hms(2026, 9, 1, 0, 0, 0).unwrap())
        );
        let spending = limits.spending.unwrap();
        assert_eq!(spending.used_microusd, 25_500_000);
        assert_eq!(spending.limit_microusd, Some(100_000_000));
        assert_eq!(spending.remaining_microusd, Some(74_500_000));
        assert_eq!(limits.account_name.as_deref(), Some("Build key"));
    }

    #[test]
    fn supports_daily_and_weekly_reset_boundaries() {
        let daily = sample(r#"{"data":{"usage":1,"limit":10,"limit_reset":"daily"}}"#);
        assert_eq!(daily.primary.duration_minutes, Some(24 * 60));
        assert_eq!(
            daily.primary.resets_at,
            Some(Utc.with_ymd_and_hms(2026, 8, 20, 0, 0, 0).unwrap())
        );

        let weekly = sample(r#"{"data":{"usage":1,"limit":10,"limit_reset":"weekly"}}"#);
        assert_eq!(weekly.primary.duration_minutes, Some(7 * 24 * 60));
        assert_eq!(
            weekly.primary.resets_at,
            Some(Utc.with_ymd_and_hms(2026, 8, 24, 0, 0, 0).unwrap())
        );
    }

    #[test]
    fn preserves_usage_when_a_key_has_no_spending_limit() {
        let limits = sample(r#"{"data":{"usage":4.25,"limit":null,"limit_remaining":null}}"#);
        assert!(limits.primary.is_empty());
        assert_eq!(limits.spending.unwrap().used_microusd, 4_250_000);
    }

    #[test]
    fn derives_remaining_and_rejects_invalid_amounts() {
        let limits = sample(r#"{"data":{"usage":3,"limit":10}}"#);
        assert_eq!(limits.spending.unwrap().remaining_microusd, Some(7_000_000));
        assert!(parse_key_response(r#"{"data":{"usage":-1}}"#, Utc::now()).is_err());
        assert!(parse_key_response("{}", Utc::now()).is_err());
    }

    #[test]
    fn parses_account_credit_balance_from_total_credits_and_usage() {
        let balance =
            parse_credits_response(r#"{"data":{"total_credits":100.5,"total_usage":25.75}}"#)
                .unwrap();
        assert_eq!(balance, 74_750_000);
    }

    #[test]
    fn clamps_account_balance_when_usage_exceeds_credits() {
        let balance =
            parse_credits_response(r#"{"data":{"total_credits":1,"total_usage":2}}"#).unwrap();
        assert_eq!(balance, 0);
        assert!(
            parse_credits_response(r#"{"data":{"total_credits":-1,"total_usage":0}}"#).is_err()
        );
    }
}
