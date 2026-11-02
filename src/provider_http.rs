//! JSON requests for quota endpoints, with the status handling every
//! provider driver shares: an expired credential, a 429 and other failures
//! each become one readable error.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::Value;

pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

pub fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(REQUEST_TIMEOUT).build()
}

/// What a 401 or 403 means for this provider, e.g. "the API key was rejected".
pub struct Request<'a> {
    pub provider: &'a str,
    pub unauthorized: &'a str,
}

impl Request<'_> {
    /// GETs `url` with `headers` and parses the JSON body.
    pub fn get_json(
        &self,
        agent: &ureq::Agent,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<Value> {
        let mut request = agent
            .get(url)
            .set("Accept", "application/json")
            .set("User-Agent", "Codex Minibar");
        for (name, value) in headers {
            request = request.set(name, value);
        }
        let response = match request.call() {
            Ok(response) => response,
            Err(ureq::Error::Status(401 | 403, _)) => bail!("{}", self.unauthorized),
            Err(ureq::Error::Status(429, _)) => {
                return Err(crate::worker::rate_limit_error(format!(
                    "{} quota request was rate limited (HTTP 429).",
                    self.provider
                )));
            }
            Err(ureq::Error::Status(status, _)) => {
                bail!("{} quota request failed with HTTP {status}", self.provider)
            }
            Err(error) => {
                return Err(error).with_context(|| format!("request {} quota", self.provider));
            }
        };
        let body = response
            .into_string()
            .with_context(|| format!("read {} quota response", self.provider))?;
        serde_json::from_str(&body)
            .with_context(|| format!("{} returned an unexpected quota response", self.provider))
    }
}

/// A number that providers send as a JSON number or a numeric string.
pub fn number(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
    .filter(|value: &f64| value.is_finite())
}

/// A whole percentage from a used share, clamped to 0–100.
pub fn percent(used: f64, limit: f64) -> Option<u8> {
    (limit > 0.0).then(|| (used / limit * 100.0).clamp(0.0, 100.0).round() as u8)
}

/// An RFC 3339 timestamp or epoch seconds/milliseconds.
pub fn timestamp(value: Option<&Value>) -> Option<chrono::DateTime<chrono::Utc>> {
    match value? {
        Value::String(text) => {
            let text = text.trim();
            chrono::DateTime::parse_from_rfc3339(text)
                .ok()
                .map(|parsed| parsed.with_timezone(&chrono::Utc))
                .or_else(|| text.parse::<i64>().ok().and_then(epoch))
        }
        Value::Number(number) => number.as_i64().and_then(epoch),
        _ => None,
    }
}

fn epoch(value: i64) -> Option<chrono::DateTime<chrono::Utc>> {
    if value <= 0 {
        return None;
    }
    if value > 100_000_000_000 {
        chrono::DateTime::from_timestamp_millis(value)
    } else {
        chrono::DateTime::from_timestamp(value, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numbers_and_timestamps_accept_both_encodings() {
        assert_eq!(number(Some(&json!(12.5))), Some(12.5));
        assert_eq!(number(Some(&json!(" 7 "))), Some(7.0));
        assert_eq!(number(Some(&json!("x"))), None);
        assert_eq!(number(None), None);
        assert_eq!(percent(30.0, 120.0), Some(25));
        assert_eq!(percent(500.0, 100.0), Some(100));
        assert_eq!(percent(1.0, 0.0), None);

        let expected = chrono::DateTime::parse_from_rfc3339("2026-11-02T10:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert_eq!(
            timestamp(Some(&json!("2026-11-02T10:00:00Z"))),
            Some(expected)
        );
        assert_eq!(
            timestamp(Some(&json!(expected.timestamp()))),
            Some(expected)
        );
        assert_eq!(
            timestamp(Some(&json!(expected.timestamp_millis()))),
            Some(expected)
        );
        assert_eq!(timestamp(Some(&json!(0))), None);
    }
}
