//! DeepSeek API balance with an API key.
//!
//! DeepSeek bills prepaid API credit and has no quota window, so the balance
//! is shown as credits. Reference: CodexBar's `DeepSeekUsageFetcher`.

use anyhow::{Result, bail};
use chrono::Utc;
use serde_json::Value;

use crate::{
    instances::ProviderId,
    limits::{Credits, RateLimits},
    provider_http::{self, Request},
    usage::UsageStatistics,
    worker::{Activator, LimitProvider, UsageProvider},
};

const BALANCE_URL: &str = "https://api.deepseek.com/user/balance";

pub struct DeepSeekClient {
    agent: ureq::Agent,
    provider: ProviderId,
}

pub struct DeepSeekActivator;

impl DeepSeekClient {
    pub fn new(provider: ProviderId) -> Self {
        Self {
            agent: provider_http::agent(),
            provider,
        }
    }
}

impl LimitProvider for DeepSeekClient {
    fn read_limits(&mut self) -> Result<RateLimits> {
        let key = crate::provider_keys::require(self.provider)?;
        let body = Request {
            provider: "DeepSeek",
            unauthorized: "DeepSeek rejected the API key; replace it in Settings → Providers",
        }
        .get_json(
            &self.agent,
            BALANCE_URL,
            &[("Authorization", &format!("Bearer {key}"))],
        )?;
        parse_balance(&body)
    }
}

impl UsageProvider for DeepSeekClient {
    fn load_cached_usage_statistics(&mut self, _history_days: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }

    fn refresh_usage_statistics(&mut self, _history_days: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }
}

impl Activator for DeepSeekActivator {
    fn activate(&mut self) -> Result<()> {
        bail!("DeepSeek does not support session-window activation")
    }
}

/// `{"is_available":true,"balance_infos":[{"currency":"CNY","total_balance":"110.00",…}]}`
fn parse_balance(body: &Value) -> Result<RateLimits> {
    let Some(infos) = body.get("balance_infos").and_then(Value::as_array) else {
        bail!("DeepSeek returned an unexpected balance response")
    };
    let balances = infos
        .iter()
        .filter_map(|info| {
            let total = provider_http::number(info.get("total_balance"))?;
            let currency = info.get("currency").and_then(Value::as_str).unwrap_or("");
            Some(format_amount(total, currency))
        })
        .collect::<Vec<_>>();
    let available = body
        .get("is_available")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let balance = if balances.is_empty() {
        format_amount(0.0, "USD")
    } else {
        balances.join(" + ")
    };
    Ok(RateLimits {
        sampled_at: Utc::now(),
        limit_name: Some("DeepSeek".into()),
        plan_type: (!available).then(|| "Out of balance".into()),
        credits: Credits {
            has_credits: true,
            unlimited: false,
            balance: Some(balance),
        },
        ..RateLimits::default()
    })
}

fn format_amount(amount: f64, currency: &str) -> String {
    match currency {
        "CNY" => format!("¥{amount:.2}"),
        "USD" | "" => format!("${amount:.2}"),
        other => format!("{amount:.2} {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn balances_become_credits() {
        let limits = parse_balance(&json!({
            "is_available": true,
            "balance_infos": [
                {"currency": "CNY", "total_balance": "110.00", "granted_balance": "10.00", "topped_up_balance": "100.00"},
                {"currency": "USD", "total_balance": 2.5, "granted_balance": "0", "topped_up_balance": "2.5"}
            ]
        }))
        .unwrap();
        assert!(limits.credits.has_credits);
        assert_eq!(limits.credits.balance.as_deref(), Some("¥110.00 + $2.50"));
        assert_eq!(limits.plan_type, None);
        assert!(limits.primary.is_empty() && limits.secondary.is_empty());
    }

    #[test]
    fn an_empty_account_reports_zero_and_unavailable() {
        let limits = parse_balance(&json!({"is_available": false, "balance_infos": []})).unwrap();
        assert_eq!(limits.credits.balance.as_deref(), Some("$0.00"));
        assert_eq!(limits.plan_type.as_deref(), Some("Out of balance"));
        assert!(parse_balance(&json!({"error": "x"})).is_err());
    }
}
