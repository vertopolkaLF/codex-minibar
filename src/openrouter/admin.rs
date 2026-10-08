//! OpenRouter key administration through an account's management key.
//!
//! Every call is blocking and must run off the UI thread. The plaintext of a
//! newly created key only exists inside [`CreatedKey`], which wipes it on drop;
//! nothing here logs or persists it.

use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use zeroize::Zeroizing;

use super::{KEYS_API_URL, management_secret_name, money_value, parse_optional_timestamp};

const ADMIN_TIMEOUT: Duration = Duration::from_secs(20);
/// Pages walked before giving up on an unexpectedly endless directory.
const MAX_PAGES: usize = 50;

/// How often a key's spending limit starts over.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum LimitReset {
    #[default]
    Never,
    Daily,
    Weekly,
    Monthly,
}

impl LimitReset {
    pub(crate) const ALL: [Self; 4] = [Self::Never, Self::Daily, Self::Weekly, Self::Monthly];

    fn from_api(value: Option<&str>) -> Self {
        match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            Some("daily") => Self::Daily,
            Some("weekly") => Self::Weekly,
            Some("monthly") => Self::Monthly,
            _ => Self::Never,
        }
    }

    fn api_value(self) -> Value {
        match self {
            Self::Never => Value::Null,
            Self::Daily => json!("daily"),
            Self::Weekly => json!("weekly"),
            Self::Monthly => json!("monthly"),
        }
    }

    pub(crate) const fn label_id(self) -> &'static str {
        match self {
            Self::Never => "openrouter-keys-reset-never",
            Self::Daily => "openrouter-keys-reset-daily",
            Self::Weekly => "openrouter-keys-reset-weekly",
            Self::Monthly => "openrouter-keys-reset-monthly",
        }
    }
}

/// One key from the account's key directory. Amounts are micro-dollars.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ManagedKey {
    pub(crate) hash: String,
    pub(crate) name: String,
    /// OpenRouter's masked fingerprint, e.g. `sk-or-v1-abc...xyz`.
    pub(crate) label: Option<String>,
    pub(crate) disabled: bool,
    pub(crate) limit_microusd: Option<u64>,
    pub(crate) limit_reset: LimitReset,
    pub(crate) include_byok_in_limit: bool,
    pub(crate) usage_microusd: u64,
    pub(crate) usage_daily_microusd: u64,
    pub(crate) usage_weekly_microusd: u64,
    pub(crate) usage_monthly_microusd: u64,
    pub(crate) created_at: Option<DateTime<Utc>>,
    pub(crate) expires_at: Option<DateTime<Utc>>,
}

impl ManagedKey {
    /// Spend counted against the limit in the current reset period.
    pub(crate) fn period_usage_microusd(&self) -> u64 {
        match self.limit_reset {
            LimitReset::Never => self.usage_microusd,
            LimitReset::Daily => self.usage_daily_microusd,
            LimitReset::Weekly => self.usage_weekly_microusd,
            LimitReset::Monthly => self.usage_monthly_microusd,
        }
    }

    pub(crate) fn is_expired(&self, now: DateTime<Utc>) -> bool {
        self.expires_at.is_some_and(|at| at <= now)
    }

    /// Whether this directory entry is the key behind a locally masked secret.
    pub(crate) fn matches_mask(&self, mask: &str) -> bool {
        self.label.as_deref().is_some_and(|label| {
            label.trim() == mask.trim() || super::masked_labels_compatible(label, mask)
        })
    }
}

/// Settings for a key that does not exist yet.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NewKey {
    pub(crate) name: String,
    pub(crate) limit_microusd: Option<u64>,
    pub(crate) limit_reset: LimitReset,
    pub(crate) include_byok_in_limit: bool,
    pub(crate) expires_at: Option<DateTime<Utc>>,
}

/// Fields to change on an existing key; `None` leaves a field untouched.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct KeyPatch {
    /// `Some(None)` removes the limit.
    pub(crate) limit_microusd: Option<Option<u64>>,
    pub(crate) limit_reset: Option<LimitReset>,
    pub(crate) include_byok_in_limit: Option<bool>,
    pub(crate) disabled: Option<bool>,
}

impl KeyPatch {
    pub(crate) fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// A freshly created key. OpenRouter returns its plaintext exactly once.
pub(crate) struct CreatedKey {
    pub(crate) secret: Zeroizing<String>,
    pub(crate) key: ManagedKey,
}

impl std::fmt::Debug for CreatedKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CreatedKey")
            .field("secret", &"<redacted>")
            .field("key", &self.key)
            .finish()
    }
}

/// The saved management key of one account, if any.
pub(crate) fn load_management_key(account_id: &str) -> Result<Zeroizing<String>> {
    crate::secrets::load(&management_secret_name(account_id))?
        .filter(|value| !value.trim().is_empty())
        .map(|value| Zeroizing::new(value.trim().to_owned()))
        .ok_or_else(|| anyhow!(crate::i18n::tr("openrouter-keys-no-management-key")))
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(ADMIN_TIMEOUT).build()
}

fn authorized(request: ureq::Request, management_key: &str) -> ureq::Request {
    request
        .set("Authorization", &format!("Bearer {management_key}"))
        .set("Accept", "application/json")
}

/// Every key on the account, including disabled ones, newest first.
pub(crate) fn list_keys(management_key: &str) -> Result<Vec<ManagedKey>> {
    let agent = agent();
    let mut keys: Vec<ManagedKey> = Vec::new();
    for _ in 0..MAX_PAGES {
        let url = format!("{KEYS_API_URL}?include_disabled=true&offset={}", keys.len());
        let body = send(authorized(agent.get(&url), management_key), None)?;
        let page = parse_key_list(&body)?;
        let fresh = page
            .into_iter()
            .filter(|key| !keys.iter().any(|known| known.hash == key.hash))
            .collect::<Vec<_>>();
        if fresh.is_empty() {
            break;
        }
        keys.extend(fresh);
    }
    keys.sort_by_key(|key| std::cmp::Reverse(key.created_at));
    Ok(keys)
}

pub(crate) fn create_key(management_key: &str, new: &NewKey) -> Result<CreatedKey> {
    let body = create_body(new);
    let response = send(
        authorized(agent().post(KEYS_API_URL), management_key),
        Some(body),
    )?;
    parse_created_key(response)
}

pub(crate) fn update_key(management_key: &str, hash: &str, patch: &KeyPatch) -> Result<ManagedKey> {
    let url = format!("{KEYS_API_URL}/{}", path_segment(hash)?);
    let body = send(
        authorized(agent().request("PATCH", &url), management_key),
        Some(patch_body(patch)),
    )?;
    parse_single_key(&body)
}

pub(crate) fn delete_key(management_key: &str, hash: &str) -> Result<()> {
    let url = format!("{KEYS_API_URL}/{}", path_segment(hash)?);
    send(authorized(agent().delete(&url), management_key), None)?;
    Ok(())
}

/// Key hashes are hex digests; refuse anything that could change the path.
fn path_segment(hash: &str) -> Result<&str> {
    let hash = hash.trim();
    if hash.is_empty() || !hash.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        bail!("OpenRouter returned an unexpected key identifier")
    }
    Ok(hash)
}

fn send(request: ureq::Request, body: Option<Value>) -> Result<String> {
    let result = match body {
        Some(body) => request
            .set("Content-Type", "application/json")
            .send_string(&body.to_string()),
        None => request.call(),
    };
    match result {
        Ok(response) => response
            .into_string()
            .context("read OpenRouter key management response"),
        Err(ureq::Error::Status(status, response)) => {
            let body = response.into_string().unwrap_or_default();
            Err(status_error(status, &body))
        }
        Err(error) => Err(anyhow!(error)).context(crate::i18n::tr("could-not-reach-openrouter")),
    }
}

fn status_error(status: u16, body: &str) -> anyhow::Error {
    let message = api_error_message(body);
    match status {
        401 => anyhow!(crate::i18n::tr("openrouter-keys-management-key-rejected")),
        403 => anyhow!(crate::i18n::tr("openrouter-keys-not-a-management-key")),
        404 => anyhow!(crate::i18n::tr("openrouter-keys-key-not-found")),
        429 => crate::worker::rate_limit_error(
            "OpenRouter key management request was rate limited (HTTP 429).",
        ),
        _ => match message {
            Some(message) => anyhow!("OpenRouter: {message} (HTTP {status})"),
            None => anyhow!("OpenRouter returned status {status}. Try again in a moment."),
        },
    }
}

fn api_error_message(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    value
        .pointer("/error/message")
        .or_else(|| value.get("message"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .map(|message| message.chars().take(200).collect())
}

fn dollars(microusd: u64) -> Value {
    json!(microusd as f64 / 1_000_000.0)
}

fn create_body(new: &NewKey) -> Value {
    let mut body = Map::new();
    body.insert("name".into(), json!(new.name.trim()));
    if let Some(limit) = new.limit_microusd {
        body.insert("limit".into(), dollars(limit));
        body.insert("limit_reset".into(), new.limit_reset.api_value());
    }
    body.insert(
        "include_byok_in_limit".into(),
        json!(new.include_byok_in_limit),
    );
    if let Some(expires_at) = new.expires_at {
        body.insert(
            "expires_at".into(),
            json!(expires_at.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
        );
    }
    Value::Object(body)
}

fn patch_body(patch: &KeyPatch) -> Value {
    let mut body = Map::new();
    if let Some(limit) = patch.limit_microusd {
        body.insert("limit".into(), limit.map_or(Value::Null, dollars));
    }
    if let Some(reset) = patch.limit_reset {
        body.insert("limit_reset".into(), reset.api_value());
    }
    if let Some(byok) = patch.include_byok_in_limit {
        body.insert("include_byok_in_limit".into(), json!(byok));
    }
    if let Some(disabled) = patch.disabled {
        body.insert("disabled".into(), json!(disabled));
    }
    Value::Object(body)
}

#[derive(Debug, Deserialize)]
struct KeyRecord {
    hash: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    disabled: bool,
    #[serde(default)]
    limit: Option<f64>,
    #[serde(default)]
    limit_reset: Option<String>,
    #[serde(default)]
    include_byok_in_limit: bool,
    #[serde(default)]
    usage: Option<f64>,
    #[serde(default)]
    usage_daily: Option<f64>,
    #[serde(default)]
    usage_weekly: Option<f64>,
    #[serde(default)]
    usage_monthly: Option<f64>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    expires_at: Option<String>,
}

impl KeyRecord {
    fn into_key(self) -> Result<ManagedKey> {
        let amount = |value: Option<f64>, field: &str| -> Result<u64> {
            Ok(money_value(value, field)?.unwrap_or(0))
        };
        Ok(ManagedKey {
            name: self
                .name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .or(self.label.as_deref())
                .unwrap_or_default()
                .to_owned(),
            label: self
                .label
                .map(|label| label.trim().to_owned())
                .filter(|label| !label.is_empty()),
            disabled: self.disabled,
            limit_microusd: money_value(self.limit, "limit")?,
            limit_reset: LimitReset::from_api(self.limit_reset.as_deref()),
            include_byok_in_limit: self.include_byok_in_limit,
            usage_microusd: amount(self.usage, "usage")?,
            usage_daily_microusd: amount(self.usage_daily, "usage_daily")?,
            usage_weekly_microusd: amount(self.usage_weekly, "usage_weekly")?,
            usage_monthly_microusd: amount(self.usage_monthly, "usage_monthly")?,
            created_at: parse_optional_timestamp(self.created_at.as_deref()),
            expires_at: parse_optional_timestamp(self.expires_at.as_deref()),
            hash: self.hash,
        })
    }
}

#[derive(Debug, Deserialize)]
struct KeyListEnvelope {
    data: Vec<KeyRecord>,
}

#[derive(Debug, Deserialize)]
struct SingleKeyEnvelope {
    data: KeyRecord,
}

#[derive(Deserialize)]
struct CreatedEnvelope {
    key: String,
    data: KeyRecord,
}

fn parse_key_list(raw: &str) -> Result<Vec<ManagedKey>> {
    let envelope: KeyListEnvelope =
        serde_json::from_str(raw).context("parse OpenRouter key list")?;
    envelope.data.into_iter().map(KeyRecord::into_key).collect()
}

fn parse_single_key(raw: &str) -> Result<ManagedKey> {
    let envelope: SingleKeyEnvelope =
        serde_json::from_str(raw).context("parse OpenRouter key response")?;
    envelope.data.into_key()
}

fn parse_created_key(raw: String) -> Result<CreatedKey> {
    // The response body holds the only copy of the plaintext; wipe it too.
    let raw = Zeroizing::new(raw);
    let envelope: CreatedEnvelope = serde_json::from_str(&raw)
        .map_err(|_| anyhow!("OpenRouter returned an unreadable key creation response"))?;
    let secret = Zeroizing::new(envelope.key);
    if secret.trim().is_empty() {
        bail!("OpenRouter did not return the new key")
    }
    Ok(CreatedKey {
        secret,
        key: envelope.data.into_key()?,
    })
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    const LIST: &str = r#"{"data":[
        {"hash":"aaa111","name":"cursor-laptop","label":"sk-or-v1-0e6...1c7","disabled":false,
         "limit":20,"limit_remaining":1.6,"limit_reset":"weekly","include_byok_in_limit":true,
         "usage":61.3,"usage_daily":4.1,"usage_weekly":18.4,"usage_monthly":61.3,
         "created_at":"2026-09-01T10:00:00Z","updated_at":null,"expires_at":null},
        {"hash":"bbb222","name":"","label":"sk-or-v1-9ff...2a0","disabled":true,
         "limit":null,"limit_reset":null,"usage":34.95,
         "created_at":"2026-08-01T10:00:00Z","expires_at":"2026-10-31T00:00:00Z"}
    ]}"#;

    #[test]
    fn parses_the_key_directory_with_period_usage() {
        let keys = parse_key_list(LIST).unwrap();
        assert_eq!(keys.len(), 2);
        let first = &keys[0];
        assert_eq!(first.name, "cursor-laptop");
        assert_eq!(first.limit_microusd, Some(20_000_000));
        assert_eq!(first.limit_reset, LimitReset::Weekly);
        assert_eq!(first.period_usage_microusd(), 18_400_000);
        assert!(first.include_byok_in_limit);
        let second = &keys[1];
        // An unnamed key falls back to its masked label.
        assert_eq!(second.name, "sk-or-v1-9ff...2a0");
        assert!(second.disabled);
        assert_eq!(second.limit_reset, LimitReset::Never);
        assert_eq!(second.period_usage_microusd(), 34_950_000);
        assert_eq!(
            second.expires_at,
            Some(Utc.with_ymd_and_hms(2026, 10, 31, 0, 0, 0).unwrap())
        );
        assert!(second.is_expired(Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap()));
    }

    #[test]
    fn matches_local_masks_with_different_visible_lengths() {
        let keys = parse_key_list(LIST).unwrap();
        assert!(keys[0].matches_mask("sk-or-v1-0e6...1c7"));
        assert!(keys[0].matches_mask("sk-or-v1-0e6ab...1c7"));
        assert!(!keys[0].matches_mask("sk-or-v1-123...1c7"));
    }

    #[test]
    fn create_body_only_sends_reset_with_a_limit() {
        let body = create_body(&NewKey {
            name: "  ci  ".into(),
            limit_microusd: Some(25_000_000),
            limit_reset: LimitReset::Monthly,
            include_byok_in_limit: false,
            expires_at: Some(Utc.with_ymd_and_hms(2026, 11, 7, 12, 30, 15).unwrap()),
        });
        assert_eq!(body["name"], "ci");
        assert_eq!(body["limit"], 25.0);
        assert_eq!(body["limit_reset"], "monthly");
        assert_eq!(body["expires_at"], "2026-11-07T12:30:15Z");

        let unlimited = create_body(&NewKey {
            name: "x".into(),
            limit_microusd: None,
            limit_reset: LimitReset::Daily,
            include_byok_in_limit: true,
            expires_at: None,
        });
        assert!(unlimited.get("limit").is_none());
        assert!(unlimited.get("limit_reset").is_none());
        assert!(unlimited.get("expires_at").is_none());
    }

    #[test]
    fn patch_body_sends_only_changed_fields_and_null_clears_a_limit() {
        let body = patch_body(&KeyPatch {
            limit_microusd: Some(None),
            limit_reset: Some(LimitReset::Never),
            ..KeyPatch::default()
        });
        assert_eq!(body, json!({"limit": null, "limit_reset": null}));
        assert_eq!(
            patch_body(&KeyPatch {
                disabled: Some(true),
                ..KeyPatch::default()
            }),
            json!({"disabled": true})
        );
        assert!(KeyPatch::default().is_empty());
    }

    #[test]
    fn created_key_keeps_the_secret_out_of_debug_output() {
        let created = parse_created_key(
            r#"{"key":"sk-or-v1-secretsecret","data":{"hash":"ccc333","name":"new","label":"sk-or-v1-sec...ret"}}"#
                .to_owned(),
        )
        .unwrap();
        assert_eq!(created.secret.as_str(), "sk-or-v1-secretsecret");
        assert_eq!(created.key.hash, "ccc333");
        assert!(!format!("{created:?}").contains("secretsecret"));
        assert!(parse_created_key(r#"{"key":" ","data":{"hash":"x"}}"#.to_owned()).is_err());
    }

    #[test]
    fn rejects_hashes_that_would_change_the_request_path() {
        assert!(path_segment("abc123").is_ok());
        assert!(path_segment("../credits").is_err());
        assert!(path_segment("a/b").is_err());
        assert!(path_segment("").is_err());
    }

    #[test]
    fn surfaces_api_error_messages() {
        assert_eq!(
            api_error_message(r#"{"error":{"message":"Name is required"}}"#).as_deref(),
            Some("Name is required")
        );
        assert_eq!(api_error_message("not json"), None);
    }
}
