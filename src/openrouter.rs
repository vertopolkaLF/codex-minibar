//! OpenRouter API-key quota provider.

use std::{collections::HashMap, time::Duration};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Datelike, Duration as ChronoDuration, NaiveDate, TimeZone, Utc};
use serde::Deserialize;

use crate::{
    limits::{
        LimitWindow, OpenRouterAccountSnapshot, OpenRouterApiKeySnapshot, RateLimits,
        SpendingSummary,
    },
    secrets,
    settings::{OpenRouterAccount, Settings},
    usage::UsageStatistics,
    worker::{Activator, LimitProvider, UsageProvider},
};

const API_URL: &str = "https://openrouter.ai/api/v1/key";
const KEYS_API_URL: &str = "https://openrouter.ai/api/v1/keys";
const CREDITS_API_URL: &str = "https://openrouter.ai/api/v1/credits";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const SECRET_NAME: &str = "openrouter-api-key";
const SECRET_PREFIX: &str = "openrouter-account-";
const LEGACY_ACCOUNT_ID: &str = "legacy";
const LEGACY_API_KEY_ID: &str = "legacy";

pub struct OpenRouterClient {
    agent: ureq::Agent,
    accounts: Vec<AccountCredentials>,
    /// Stable key metadata (name/limit/reset). Usage is never stored here.
    key_cache: HashMap<String, CachedOpenRouterKey>,
}

pub struct OpenRouterActivator;

struct AccountCredentials {
    id: String,
    name: String,
    api_keys: Vec<ApiKeyCredential>,
    management_key: Option<String>,
}

struct ApiKeyCredential {
    id: String,
    value: String,
}

#[derive(Clone, Debug, Default)]
struct CachedOpenRouterKey {
    label: Option<String>,
    masked_key: Option<String>,
    limit_microusd: Option<u64>,
    reset_kind: Option<String>,
    expires_at: Option<DateTime<Utc>>,
    disabled: bool,
}

impl OpenRouterClient {
    pub fn new(settings: &Settings) -> Result<Self> {
        Ok(Self {
            agent: ureq::AgentBuilder::new().timeout(REQUEST_TIMEOUT).build(),
            accounts: load_credentials(settings)?,
            key_cache: load_key_cache_from_store(),
        })
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

    /// Maps masked `label` values from `/key` to directory metadata.
    /// Requires a management key; ordinary inference keys get a quiet empty map.
    fn read_key_directory(&self, api_key: &str) -> Result<HashMap<String, DirectoryKeyInfo>> {
        let response = match self
            .agent
            .get(&format!("{KEYS_API_URL}?include_disabled=true"))
            .set("Authorization", &format!("Bearer {api_key}"))
            .set("Accept", "application/json")
            .call()
        {
            Ok(response) => response,
            Err(ureq::Error::Status(403, _)) => return Ok(HashMap::new()),
            Err(error) => return Err(error).context("request OpenRouter API key directory"),
        };
        let body = response
            .into_string()
            .context("read OpenRouter API key directory response")?;
        parse_keys_directory(&body)
    }
}

fn read_key_with_agent(
    agent: &ureq::Agent,
    api_key: &str,
    sampled_at: DateTime<Utc>,
) -> Result<ParsedOpenRouterKey> {
    let response = agent
        .get(API_URL)
        .set("Authorization", &format!("Bearer {api_key}"))
        .set("Accept", "application/json")
        .call()
        .context("request OpenRouter API-key usage")?;
    let body = response
        .into_string()
        .context("read OpenRouter API-key response")?;
    parse_key_response(&body, sampled_at)
}

pub fn is_installed() -> bool {
    key_is_configured() || secrets::contains_prefix(SECRET_PREFIX)
}

pub fn key_is_configured() -> bool {
    secrets::load(SECRET_NAME)
        .ok()
        .flatten()
        .is_some_and(|key| !key.trim().is_empty())
}

pub fn save_api_key(value: Option<&str>) -> Result<()> {
    save_account_api_key(LEGACY_ACCOUNT_ID, LEGACY_API_KEY_ID, value)
}

pub fn accounts_for_settings(settings: &Settings) -> Vec<OpenRouterAccount> {
    let mut accounts = settings.openrouter_accounts.clone();
    if key_is_configured()
        && !accounts
            .iter()
            .any(|account| account.id == LEGACY_ACCOUNT_ID)
    {
        accounts.insert(0, OpenRouterAccount::legacy());
    }
    accounts
}

pub fn is_installed_for_accounts(accounts: &[OpenRouterAccount]) -> bool {
    is_installed()
        || accounts.iter().any(|account| {
            account
                .api_key_ids
                .iter()
                .any(|key_id| api_key_is_configured(&account.id, key_id))
                || management_key_is_configured(&account.id)
        })
}

pub fn api_key_is_configured(account_id: &str, key_id: &str) -> bool {
    load_secret(&api_secret_name(account_id, key_id))
}

pub fn management_key_is_configured(account_id: &str) -> bool {
    load_secret(&management_secret_name(account_id))
}

pub fn save_account_api_key(account_id: &str, key_id: &str, value: Option<&str>) -> Result<()> {
    secrets::save(&api_secret_name(account_id, key_id), value)
}

pub fn save_management_key(account_id: &str, value: Option<&str>) -> Result<()> {
    secrets::save(&management_secret_name(account_id), value)
}

impl LimitProvider for OpenRouterClient {
    fn read_limits(&mut self) -> Result<RateLimits> {
        let sampled_at = Utc::now();
        let mut accounts = Vec::new();
        for account in &self.accounts {
            // Key display names must come only from this account's management
            // key directory. Never fall back to an ordinary API key here — that
            // can resolve names from a different OpenRouter org and make a key
            // look like it belongs under the wrong account heading.
            let key_directory = account
                .management_key
                .as_deref()
                .and_then(|key| self.read_key_directory(key).ok())
                .unwrap_or_default();
            // A management key is preferred for account-level credits. Keep the
            // first API key as a compatibility fallback for users whose existing
            // key is itself a management key.
            let credits_key = account
                .management_key
                .as_deref()
                .or_else(|| account.api_keys.first().map(|key| key.value.as_str()));

            let mut api_keys = Vec::new();
            // Fetch every key concurrently. Sequential 15s timeouts made adding
            // a third key blank the OpenRouter tab for a minute-plus.
            let live_results: Vec<Result<ParsedOpenRouterKey>> = std::thread::scope(|scope| {
                account
                    .api_keys
                    .iter()
                    .map(|api_key| {
                        let agent = self.agent.clone();
                        let value = api_key.value.clone();
                        scope.spawn(move || read_key_with_agent(&agent, &value, sampled_at))
                    })
                    .map(|handle| {
                        handle
                            .join()
                            .unwrap_or_else(|_| bail!("OpenRouter API-key worker panicked"))
                    })
                    .collect()
            });

            for (api_key, live) in account.api_keys.iter().zip(live_results) {
                let cache_id = key_cache_id(&account.id, &api_key.id);
                let cached = self.key_cache.get(&cache_id).cloned().unwrap_or_default();
                let masked_key = collapse_api_key(&api_key.value)
                    .or_else(|| cached.masked_key.clone());
                // OpenRouter's own label mask (sk-or-v1-abc...xyz) often uses a
                // different head/tail length than our local collapse — match the
                // full secret against directory labels instead of exact strings.
                let directory =
                    find_directory_entry(&api_key.value, masked_key.as_deref(), &key_directory);
                let live = match live {
                    Ok(parsed) => Some(parsed),
                    Err(error) => {
                        crate::logger::info(format!(
                            "OpenRouter account {} API key {} failed: {error:#}",
                            account.name, api_key.id
                        ));
                        None
                    }
                };

                let (label, spending, has_live_usage, expires_at, disabled) = match live {
                    Some(parsed) => {
                        // Prefer the directory name for this key's own mask when
                        // /key only returned a masked label. Never borrow another
                        // key's cached title — cache is already account+key keyed.
                        let label = resolve_key_display_name(
                            parsed.account_name.as_deref(),
                            &key_directory,
                        )
                        .or_else(|| directory.as_ref().and_then(|info| info.name.clone()))
                        .or_else(|| {
                            resolve_key_display_name(masked_key.as_deref(), &key_directory)
                        })
                        .or_else(|| cached.label.clone());
                        let expires_at = parsed
                            .expires_at
                            .or_else(|| directory.as_ref().and_then(|info| info.expires_at))
                            .or(cached.expires_at);
                        let disabled = directory
                            .as_ref()
                            .map(|info| info.disabled)
                            .unwrap_or(cached.disabled);
                        let spending = merge_key_spending(
                            Some(&parsed.spending),
                            &cached,
                            sampled_at,
                            true,
                        );
                        (label, spending, true, expires_at, disabled)
                    }
                    None => {
                        let label = cached
                            .label
                            .clone()
                            .or_else(|| directory.as_ref().and_then(|info| info.name.clone()))
                            .or_else(|| {
                                resolve_key_display_name(masked_key.as_deref(), &key_directory)
                            });
                        let expires_at = directory
                            .as_ref()
                            .and_then(|info| info.expires_at)
                            .or(cached.expires_at);
                        let disabled = directory
                            .as_ref()
                            .map(|info| info.disabled)
                            .unwrap_or(cached.disabled);
                        let spending = merge_key_spending(None, &cached, sampled_at, false);
                        (label, spending, false, expires_at, disabled)
                    }
                };

                self.key_cache.insert(
                    cache_id,
                    CachedOpenRouterKey {
                        label: label.clone(),
                        masked_key: masked_key.clone(),
                        limit_microusd: spending.limit_microusd,
                        reset_kind: spending.reset_kind.clone(),
                        expires_at,
                        disabled,
                    },
                );

                api_keys.push(OpenRouterApiKeySnapshot {
                    id: api_key.id.clone(),
                    label,
                    masked_key,
                    spending,
                    has_live_usage,
                    expires_at,
                    disabled,
                });
            }

            let balance =
                credits_key.and_then(|key| self.read_account_balance(key).ok().flatten());
            if api_keys.is_empty() && balance.is_none() {
                continue;
            }
            accounts.push(OpenRouterAccountSnapshot {
                id: account.id.clone(),
                name: account.name.clone(),
                api_keys,
                balance_microusd: balance,
            });
        }

        if accounts.is_empty() {
            bail!("OpenRouter has no usable API or management key")
        }
        Ok(rate_limits_from_accounts(accounts, sampled_at))
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

#[derive(Debug)]
struct ParsedOpenRouterKey {
    account_name: Option<String>,
    spending: SpendingSummary,
    expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct KeyEnvelope {
    data: KeyData,
}

#[derive(Debug, Deserialize)]
struct KeyData {
    /// Masked key fingerprint from `/key`, e.g. `sk-or-v1-abc...123`.
    label: Option<String>,
    /// Human-readable key name when the endpoint provides it.
    #[serde(default)]
    name: Option<String>,
    usage: Option<f64>,
    limit: Option<f64>,
    limit_remaining: Option<f64>,
    limit_reset: Option<String>,
    /// Absolute expiry timestamp, or null when the key never expires.
    #[serde(default)]
    expires_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KeysDirectoryEnvelope {
    data: Vec<KeyDirectoryEntry>,
}

#[derive(Debug, Deserialize)]
struct KeyDirectoryEntry {
    label: Option<String>,
    name: Option<String>,
    #[serde(default)]
    expires_at: Option<String>,
    #[serde(default)]
    disabled: bool,
}

#[derive(Clone, Debug, Default)]
struct DirectoryKeyInfo {
    name: Option<String>,
    expires_at: Option<DateTime<Utc>>,
    disabled: bool,
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

fn load_credentials(settings: &Settings) -> Result<Vec<AccountCredentials>> {
    accounts_for_settings(settings)
        .into_iter()
        .map(|account| {
            let mut api_keys = Vec::new();
            for key_id in &account.api_key_ids {
                let secret_name = api_secret_name(&account.id, key_id);
                if let Some(value) = load_secret_value(&secret_name)? {
                    api_keys.push(ApiKeyCredential {
                        id: key_id.clone(),
                        value,
                    });
                }
            }
            let management_key = load_secret_value(&management_secret_name(&account.id))?;
            if api_keys.is_empty() && management_key.is_none() {
                return Ok(None);
            }
            Ok(Some(AccountCredentials {
                id: account.id,
                name: account.name,
                api_keys,
                management_key,
            }))
        })
        .filter_map(|result| result.transpose())
        .collect()
}

fn load_secret(name: &str) -> bool {
    secrets::load(name)
        .ok()
        .flatten()
        .is_some_and(|value| !value.trim().is_empty())
}

fn load_secret_value(name: &str) -> Result<Option<String>> {
    secrets::load(name)
}

fn api_secret_name(account_id: &str, key_id: &str) -> String {
    if account_id == LEGACY_ACCOUNT_ID && key_id == LEGACY_API_KEY_ID {
        SECRET_NAME.into()
    } else {
        format!("{SECRET_PREFIX}{account_id}-api-{key_id}")
    }
}

fn management_secret_name(account_id: &str) -> String {
    format!("{SECRET_PREFIX}{account_id}-management")
}

fn rate_limits_from_accounts(
    accounts: Vec<OpenRouterAccountSnapshot>,
    sampled_at: DateTime<Utc>,
) -> RateLimits {
    let spending = aggregate_spending(&accounts);
    let primary = spending
        .as_ref()
        .map_or_else(LimitWindow::default, |value| {
            let used_percent = value.limit_microusd.and_then(|limit| {
                (limit > 0).then(|| {
                    ((value.used_microusd.min(limit) as f64 / limit as f64) * 100.0)
                        .round()
                        .clamp(0.0, 100.0) as u8
                })
            });
            LimitWindow {
                used_percent,
                resets_at: value.resets_at,
                duration_minutes: value.reset_kind.as_deref().and_then(|kind| {
                    period_bounds(sampled_at, kind).map(|(_, _, minutes)| minutes)
                }),
            }
        });
    RateLimits {
        primary,
        sampled_at,
        account_name: (accounts.len() == 1).then(|| accounts[0].name.clone()),
        spending,
        openrouter_accounts: accounts,
        ..RateLimits::default()
    }
}

fn aggregate_spending(accounts: &[OpenRouterAccountSnapshot]) -> Option<SpendingSummary> {
    let spendings = accounts
        .iter()
        .flat_map(|account| account.api_keys.iter())
        .filter(|key| key.has_live_usage)
        .map(|key| &key.spending)
        .collect::<Vec<_>>();
    let used_microusd = spendings.iter().fold(0_u64, |total, spending| {
        total.saturating_add(spending.used_microusd)
    });
    let limit_microusd = (!spendings.is_empty()
        && spendings
            .iter()
            .all(|spending| spending.limit_microusd.is_some()))
    .then(|| {
        spendings.iter().fold(0_u64, |total, spending| {
            total.saturating_add(spending.limit_microusd.unwrap_or_default())
        })
    });
    let remaining_microusd = (!spendings.is_empty()
        && spendings
            .iter()
            .all(|spending| spending.remaining_microusd.is_some()))
    .then(|| {
        spendings.iter().fold(0_u64, |total, spending| {
            total.saturating_add(spending.remaining_microusd.unwrap_or_default())
        })
    });
    let balance_microusd = accounts
        .iter()
        .filter_map(|account| account.balance_microusd)
        .reduce(|total, balance| total.saturating_add(balance));
    let resets_at = common_value(spendings.iter().map(|spending| spending.resets_at));
    let reset_kind = common_value(spendings.iter().map(|spending| spending.reset_kind.clone()));
    (spendings.len() > 0 || balance_microusd.is_some()).then_some(SpendingSummary {
        used_microusd,
        limit_microusd,
        remaining_microusd,
        resets_at,
        reset_kind,
        balance_microusd,
    })
}

fn common_value<T>(mut values: impl Iterator<Item = Option<T>>) -> Option<T>
where
    T: Clone + PartialEq,
{
    let first = values.next()?;
    values
        .all(|value| value == first)
        .then_some(first)
        .flatten()
}

fn parse_key_response(raw: &str, sampled_at: DateTime<Utc>) -> Result<ParsedOpenRouterKey> {
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
    let derived_remaining = remaining.or_else(|| limit.map(|limit| limit.saturating_sub(usage)));
    let account_name = key_identity_from_response(&envelope.data);
    let expires_at = parse_optional_timestamp(envelope.data.expires_at.as_deref());

    Ok(ParsedOpenRouterKey {
        account_name,
        spending: SpendingSummary {
            used_microusd: usage,
            limit_microusd: limit,
            remaining_microusd: derived_remaining,
            resets_at: bounds.map(|(_, reset, _)| reset),
            reset_kind,
            balance_microusd: None,
        },
        expires_at,
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

fn load_key_cache_from_store() -> HashMap<String, CachedOpenRouterKey> {
    let Ok(Some(previous)) =
        crate::store::with_store(|store| store.load_limits(crate::settings::ProviderKind::OpenRouter))
    else {
        return HashMap::new();
    };
    let mut cache = HashMap::new();
    for account in previous.openrouter_accounts {
        for key in account.api_keys {
            cache.insert(
                key_cache_id(&account.id, &key.id),
                CachedOpenRouterKey {
                    label: key.label,
                    masked_key: key.masked_key,
                    limit_microusd: key.spending.limit_microusd,
                    reset_kind: key.spending.reset_kind,
                    expires_at: key.expires_at,
                    disabled: key.disabled,
                },
            );
        }
    }
    cache
}

fn key_cache_id(account_id: &str, key_id: &str) -> String {
    format!("{account_id}\0{key_id}")
}

/// Merge live `/key` spending with cached metadata. Usage is taken from the
/// live response only — never from cache.
fn merge_key_spending(
    live: Option<&SpendingSummary>,
    cached: &CachedOpenRouterKey,
    sampled_at: DateTime<Utc>,
    has_live_usage: bool,
) -> SpendingSummary {
    let used_microusd = if has_live_usage {
        live.map(|spending| spending.used_microusd).unwrap_or(0)
    } else {
        0
    };
    let limit_microusd = live
        .and_then(|spending| spending.limit_microusd)
        .or(cached.limit_microusd);
    let reset_kind = live
        .and_then(|spending| spending.reset_kind.clone())
        .or_else(|| cached.reset_kind.clone());
    let bounds = reset_kind
        .as_deref()
        .and_then(|kind| period_bounds(sampled_at, kind));
    let remaining_microusd = if has_live_usage {
        live.and_then(|spending| spending.remaining_microusd)
            .or_else(|| limit_microusd.map(|limit| limit.saturating_sub(used_microusd)))
    } else {
        None
    };
    SpendingSummary {
        used_microusd,
        limit_microusd,
        remaining_microusd,
        resets_at: bounds
            .map(|(_, reset, _)| reset)
            .or_else(|| live.and_then(|spending| spending.resets_at)),
        reset_kind,
        balance_microusd: None,
    }
}

fn parse_keys_directory(raw: &str) -> Result<HashMap<String, DirectoryKeyInfo>> {
    let envelope: KeysDirectoryEnvelope =
        serde_json::from_str(raw).context("parse OpenRouter API key directory")?;
    let mut keys = HashMap::new();
    for entry in envelope.data {
        let Some(label) = cleaned_key_text(entry.label.as_deref()) else {
            continue;
        };
        keys.insert(
            label,
            DirectoryKeyInfo {
                name: cleaned_key_text(entry.name.as_deref()),
                expires_at: parse_optional_timestamp(entry.expires_at.as_deref()),
                disabled: entry.disabled,
            },
        );
    }
    Ok(keys)
}

fn parse_optional_timestamp(raw: Option<&str>) -> Option<DateTime<Utc>> {
    let raw = raw.map(str::trim).filter(|value| !value.is_empty())?;
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

fn find_directory_entry(
    secret: &str,
    masked_key: Option<&str>,
    key_directory: &HashMap<String, DirectoryKeyInfo>,
) -> Option<DirectoryKeyInfo> {
    if let Some(mask) = masked_key.map(str::trim).filter(|value| !value.is_empty())
        && let Some(info) = key_directory.get(mask)
    {
        return Some(info.clone());
    }
    key_directory
        .iter()
        .find(|(label, _)| masked_label_matches_secret(label, secret))
        .map(|(_, info)| info.clone())
}

/// OpenRouter masks keys as `sk-or-v1-<head>...<tail>` with variable lengths.
/// Match against the full secret by head/tail so local collapse differences
/// cannot hide directory metadata such as `expires_at` / `disabled`.
fn masked_label_matches_secret(label: &str, secret: &str) -> bool {
    let label = label.trim();
    let secret = secret.trim();
    if secret.is_empty() || !is_masked_openrouter_key_label(label) {
        return false;
    }
    let Some((head, tail)) = label.split_once("...") else {
        return false;
    };
    if head.is_empty() || tail.is_empty() {
        return false;
    }
    secret.starts_with(head) && secret.ends_with(tail)
}

fn key_identity_from_response(data: &KeyData) -> Option<String> {
    cleaned_key_text(data.name.as_deref()).or_else(|| cleaned_key_text(data.label.as_deref()))
}

fn resolve_key_display_name(
    identity: Option<&str>,
    key_directory: &HashMap<String, DirectoryKeyInfo>,
) -> Option<String> {
    let identity = cleaned_key_text(identity)?;
    if let Some(name) = key_directory
        .get(&identity)
        .and_then(|info| info.name.clone())
    {
        return Some(name);
    }
    if let Some(name) = key_directory.iter().find_map(|(label, info)| {
        (label == &identity || masked_labels_compatible(label, &identity))
            .then(|| info.name.clone())
            .flatten()
    }) {
        return Some(name);
    }
    if is_masked_openrouter_key_label(&identity) {
        return None;
    }
    Some(identity)
}

/// Two masked labels refer to the same key when one head/tail is a prefix/suffix
/// of the other (OpenRouter may show more or fewer visible characters than we do).
fn masked_labels_compatible(left: &str, right: &str) -> bool {
    let Some((left_head, left_tail)) = left.split_once("...") else {
        return false;
    };
    let Some((right_head, right_tail)) = right.split_once("...") else {
        return false;
    };
    if left_head.is_empty()
        || left_tail.is_empty()
        || right_head.is_empty()
        || right_tail.is_empty()
    {
        return false;
    }
    (left_head.starts_with(right_head) || right_head.starts_with(left_head))
        && (left_tail.ends_with(right_tail) || right_tail.ends_with(left_tail))
}

fn cleaned_key_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn is_masked_openrouter_key_label(label: &str) -> bool {
    let trimmed = label.trim();
    trimmed.starts_with("sk-or-") && trimmed.contains("...")
}

/// Collapses a secret into a short fingerprint. Never returns the full key.
fn collapse_api_key(value: &str) -> Option<String> {
    let key = value.trim();
    if key.is_empty() {
        return None;
    }
    const HEAD: usize = 10;
    const TAIL: usize = 3;
    if key.len() <= HEAD + TAIL + 3 {
        // Too short to safely show ends — obscure everything after a tiny prefix.
        let prefix: String = key.chars().take(4).collect();
        return Some(format!("{prefix}..."));
    }
    let head: String = key.chars().take(HEAD).collect();
    let tail: String = key
        .chars()
        .rev()
        .take(TAIL)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    Some(format!("{head}...{tail}"))
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
        let sampled_at = Utc.with_ymd_and_hms(2026, 8, 19, 12, 30, 0).unwrap();
        let parsed = parse_key_response(raw, sampled_at).unwrap();
        let used_percent = parsed.spending.limit_microusd.and_then(|limit| {
            (limit > 0).then(|| {
                ((parsed.spending.used_microusd.min(limit) as f64 / limit as f64) * 100.0)
                    .round()
                    .clamp(0.0, 100.0) as u8
            })
        });
        RateLimits {
            primary: LimitWindow {
                used_percent,
                resets_at: parsed.spending.resets_at,
                duration_minutes: parsed.spending.reset_kind.as_deref().and_then(|kind| {
                    period_bounds(sampled_at, kind).map(|(_, _, minutes)| minutes)
                }),
            },
            sampled_at,
            account_name: parsed.account_name,
            spending: Some(parsed.spending),
            ..RateLimits::default()
        }
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
    fn collapses_api_keys_without_exposing_the_secret() {
        let collapsed =
            collapse_api_key("sk-or-v1-a35b9c8d7e6f5a4b3c2d1e0f9a8b7c6d5e4f3a2b1").unwrap();
        assert!(collapsed.contains("..."));
        assert!(!collapsed.contains("a35b9c8d7e6f5a4b3c2d1e0f9a8b7c6d5e4f3a2b1"));
        assert!(collapsed.starts_with("sk-or-v1-a"));
        assert!(collapse_api_key("   ").is_none());
    }

    #[test]
    fn merge_keeps_cached_metadata_but_never_cached_usage() {
        let sampled_at = Utc.with_ymd_and_hms(2026, 8, 19, 12, 30, 0).unwrap();
        let cached = CachedOpenRouterKey {
            label: Some("Test Key".into()),
            limit_microusd: Some(1_000_000),
            reset_kind: Some("daily".into()),
            ..Default::default()
        };
        let live = SpendingSummary {
            used_microusd: 250_000,
            limit_microusd: Some(1_000_000),
            remaining_microusd: Some(750_000),
            resets_at: None,
            reset_kind: Some("daily".into()),
            balance_microusd: None,
        };
        let merged = merge_key_spending(Some(&live), &cached, sampled_at, true);
        assert_eq!(merged.used_microusd, 250_000);
        assert_eq!(merged.limit_microusd, Some(1_000_000));
        assert!(merged.resets_at.is_some());

        let placeholder = merge_key_spending(None, &cached, sampled_at, false);
        assert_eq!(placeholder.used_microusd, 0);
        assert_eq!(placeholder.limit_microusd, Some(1_000_000));
        assert!(placeholder.resets_at.is_some());
    }

    #[test]
    fn prefers_key_name_over_masked_label() {
        let limits = sample(
            r#"{"data":{"name":"Leon Flame","label":"sk-or-v1-a35...26a","usage":0,"limit":1}}"#,
        );
        assert_eq!(limits.account_name.as_deref(), Some("Leon Flame"));
    }

    #[test]
    fn resolves_display_names_from_management_directory() {
        let names = parse_keys_directory(
            r#"{"data":[{"label":"sk-or-v1-a35...26a","name":"Leon Flame"},{"label":"sk-or-v1-bbb...ccc","name":""}]}"#,
        )
        .unwrap();
        assert_eq!(
            resolve_key_display_name(Some("sk-or-v1-a35...26a"), &names).as_deref(),
            Some("Leon Flame")
        );
        assert_eq!(
            resolve_key_display_name(Some("sk-or-v1-unknown...zzz"), &names),
            None
        );
        assert_eq!(
            resolve_key_display_name(Some("Build key"), &HashMap::new()).as_deref(),
            Some("Build key")
        );
    }

    #[test]
    fn parses_expires_at_from_key_and_directory() {
        let sampled_at = Utc.with_ymd_and_hms(2026, 8, 19, 12, 30, 0).unwrap();
        let parsed = parse_key_response(
            r#"{"data":{"usage":1,"limit":10,"expires_at":"2026-08-19T10:00:00Z"}}"#,
            sampled_at,
        )
        .unwrap();
        assert_eq!(
            parsed.expires_at,
            Some(Utc.with_ymd_and_hms(2026, 8, 19, 10, 0, 0).unwrap())
        );

        let directory = parse_keys_directory(
            r#"{"data":[{"label":"sk-or-v1-a35...26a","name":"TEST","expires_at":"2026-08-18T15:30:00Z","disabled":true}]}"#,
        )
        .unwrap();
        assert_eq!(
            directory["sk-or-v1-a35...26a"].expires_at,
            Some(Utc.with_ymd_and_hms(2026, 8, 18, 15, 30, 0).unwrap())
        );
        assert!(directory["sk-or-v1-a35...26a"].disabled);
    }

    #[test]
    fn matches_directory_labels_against_full_secrets_despite_mask_length() {
        let secret = "sk-or-v1-c0123456789abcdef0123456789abcdef0123456789abcdef0123456789ef0";
        let directory = parse_keys_directory(
            r#"{"data":[{"label":"sk-or-v1-c012...ef0","name":"TEST","expires_at":"2026-01-01T00:00:00Z","disabled":true}]}"#,
        )
        .unwrap();
        // Local collapse uses a shorter head than OpenRouter's label.
        let local_mask = collapse_api_key(secret);
        assert_eq!(local_mask.as_deref(), Some("sk-or-v1-c...ef0"));
        assert!(directory.get("sk-or-v1-c...ef0").is_none());

        let matched = find_directory_entry(secret, local_mask.as_deref(), &directory).unwrap();
        assert_eq!(matched.name.as_deref(), Some("TEST"));
        assert!(matched.disabled);
        assert_eq!(
            matched.expires_at,
            Some(Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap())
        );
        assert!(masked_label_matches_secret("sk-or-v1-c012...ef0", secret));
        assert!(!masked_label_matches_secret("sk-or-v1-d012...ef0", secret));
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

    #[test]
    fn keeps_multiple_accounts_and_aggregates_their_key_limits() {
        let sampled_at = Utc.with_ymd_and_hms(2026, 8, 19, 12, 30, 0).unwrap();
        let first = sample(
            r#"{"data":{"label":"first-key","usage":10,"limit":100,"limit_remaining":90,"limit_reset":"monthly"}}"#,
        );
        let second = sample(
            r#"{"data":{"label":"second-key","usage":20,"limit":100,"limit_remaining":80,"limit_reset":"monthly"}}"#,
        );
        let limits = rate_limits_from_accounts(
            vec![
                OpenRouterAccountSnapshot {
                    id: "account-one".into(),
                    name: "First account".into(),
                    api_keys: vec![OpenRouterApiKeySnapshot {
                        id: "key-one".into(),
                        label: first.account_name,
                        masked_key: Some("sk-or-v1-aaa...111".into()),
                        spending: first.spending.unwrap(),
                        has_live_usage: true,
                        expires_at: None,
                        disabled: false,
                    }],
                    balance_microusd: Some(50_000_000),
                },
                OpenRouterAccountSnapshot {
                    id: "account-two".into(),
                    name: "Second account".into(),
                    api_keys: vec![OpenRouterApiKeySnapshot {
                        id: "key-two".into(),
                        label: second.account_name,
                        masked_key: Some("sk-or-v1-bbb...222".into()),
                        spending: second.spending.unwrap(),
                        has_live_usage: true,
                        expires_at: None,
                        disabled: false,
                    }],
                    balance_microusd: Some(75_000_000),
                },
            ],
            sampled_at,
        );

        assert_eq!(limits.openrouter_accounts.len(), 2);
        assert_eq!(limits.primary.used_percent, Some(15));
        let spending = limits.spending.unwrap();
        assert_eq!(spending.used_microusd, 30_000_000);
        assert_eq!(spending.limit_microusd, Some(200_000_000));
        assert_eq!(spending.remaining_microusd, Some(170_000_000));
        assert_eq!(spending.balance_microusd, Some(125_000_000));
        assert!(limits.account_name.is_none());
    }

    #[test]
    fn never_moves_api_keys_between_account_snapshots() {
        let sampled_at = Utc.with_ymd_and_hms(2026, 8, 19, 12, 30, 0).unwrap();
        let leon_key = sample(
            r#"{"data":{"name":"TEST2","label":"sk-or-v1-f12...662","usage":9,"limit":10,"limit_remaining":1,"limit_reset":"daily"}}"#,
        );
        let pixel_key = sample(
            r#"{"data":{"name":"KEY 1","label":"sk-or-v1-a12...1f8","usage":0.04,"limit":null}}"#,
        );
        let limits = rate_limits_from_accounts(
            vec![
                OpenRouterAccountSnapshot {
                    id: "leon-flame".into(),
                    name: "Leon Flame".into(),
                    api_keys: vec![OpenRouterApiKeySnapshot {
                        id: "test2".into(),
                        label: leon_key.account_name,
                        masked_key: Some("sk-or-v1-f12...662".into()),
                        spending: leon_key.spending.unwrap(),
                        has_live_usage: true,
                        expires_at: None,
                        disabled: false,
                    }],
                    balance_microusd: None,
                },
                OpenRouterAccountSnapshot {
                    id: "pixelscan".into(),
                    name: "Pixelscan".into(),
                    api_keys: vec![OpenRouterApiKeySnapshot {
                        id: "key-1".into(),
                        label: pixel_key.account_name,
                        masked_key: Some("sk-or-v1-a12...1f8".into()),
                        spending: pixel_key.spending.unwrap(),
                        has_live_usage: true,
                        expires_at: None,
                        disabled: false,
                    }],
                    balance_microusd: Some(38_960_000),
                },
            ],
            sampled_at,
        );

        assert_eq!(limits.openrouter_accounts[0].id, "leon-flame");
        assert_eq!(
            limits.openrouter_accounts[0].api_keys[0].label.as_deref(),
            Some("TEST2")
        );
        assert_eq!(limits.openrouter_accounts[1].id, "pixelscan");
        assert_eq!(
            limits.openrouter_accounts[1].api_keys[0].label.as_deref(),
            Some("KEY 1")
        );
        assert!(
            limits.openrouter_accounts[1]
                .api_keys
                .iter()
                .all(|key| key.id != "test2"),
            "TEST2 must stay under Leon Flame and never appear under Pixelscan"
        );
    }
}
