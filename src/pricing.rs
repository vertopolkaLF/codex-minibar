//! LiteLLM-backed API-equivalent token pricing.
//!
//! The catalog is cached locally so usage scans keep working offline. A model
//! without a complete rate entry remains unpriced instead of borrowing a rate
//! from an unrelated model.

#[cfg(not(test))]
use std::sync::Mutex;
#[cfg(not(test))]
use std::time::Duration as StdDuration;
use std::{
    collections::HashMap,
    sync::{OnceLock, RwLock},
};

#[cfg(not(test))]
use anyhow::bail;
use anyhow::{Context, Result};
#[cfg(not(test))]
use chrono::Duration;
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::{settings::ProviderKind, store};

#[cfg(not(test))]
const LITELLM_RATES_URL: &str =
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";
const RATES_SOURCE: &str = "litellm";
#[cfg(not(test))]
const RATES_TTL: Duration = Duration::hours(24);

static CATALOG: OnceLock<RwLock<Option<PricingCatalog>>> = OnceLock::new();
#[cfg(not(test))]
static REFRESH_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Debug)]
struct PricingCatalog {
    #[allow(dead_code)]
    fetched_at: DateTime<Utc>,
    rates: HashMap<String, ModelRate>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ModelRate {
    input_per_token: f64,
    output_per_token: f64,
    cache_read_per_token: f64,
    cache_creation_per_token: f64,
}

fn catalog_slot() -> &'static RwLock<Option<PricingCatalog>> {
    CATALOG.get_or_init(|| RwLock::new(None))
}

/// Hydrates the last successful rate table before the UI is created.
pub fn initialize() -> Result<()> {
    let Some((fetched_at_raw, payload)) =
        store::with_store(|store| store.load_pricing_catalog(RATES_SOURCE))?
    else {
        return Ok(());
    };
    let fetched_at = DateTime::parse_from_rfc3339(&fetched_at_raw)
        .context("parse cached LiteLLM pricing timestamp")?
        .with_timezone(&Utc);
    let document: Value =
        serde_json::from_str(&payload).context("parse cached LiteLLM pricing document")?;
    let rates = parse_rate_table(&document);
    if !rates.is_empty() {
        *catalog_slot()
            .write()
            .expect("pricing catalog lock poisoned") = Some(PricingCatalog { fetched_at, rates });
    }
    Ok(())
}

/// Refreshes the shared table at most once per day. Network access is kept out
/// of tests; a missing table then produces explicit unpriced usage.
#[allow(dead_code)]
pub(crate) fn refresh_if_stale() -> Result<bool> {
    #[cfg(test)]
    {
        return Ok(false);
    }

    #[cfg(not(test))]
    {
        let _refresh_guard = REFRESH_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("pricing refresh lock poisoned");
        let stale = catalog_slot()
            .read()
            .expect("pricing catalog lock poisoned")
            .as_ref()
            .is_none_or(|catalog| Utc::now() - catalog.fetched_at >= RATES_TTL);
        if !stale {
            return Ok(false);
        }

        let tls = ureq::native_tls::TlsConnector::new()
            .context("create TLS connector for LiteLLM pricing")?;
        let agent = ureq::AgentBuilder::new()
            .timeout(StdDuration::from_secs(10))
            .tls_connector(std::sync::Arc::new(tls))
            .build();
        let payload = agent
            .get(LITELLM_RATES_URL)
            .set("Accept", "application/json")
            .call()
            .context("request LiteLLM pricing table")?
            .into_string()
            .context("read LiteLLM pricing table")?;
        let document: Value =
            serde_json::from_str(&payload).context("parse LiteLLM pricing table")?;
        let rates = parse_rate_table(&document);
        if rates.is_empty() {
            bail!("LiteLLM pricing table contains no complete model rates");
        }
        let fetched_at = Utc::now();
        store::with_store(|store| store.save_pricing_catalog(RATES_SOURCE, fetched_at, &payload))?;
        *catalog_slot()
            .write()
            .expect("pricing catalog lock poisoned") = Some(PricingCatalog { fetched_at, rates });
        Ok(true)
    }
}

pub(crate) fn request_cost_microusd(
    provider: ProviderKind,
    model: Option<&str>,
    cache_creation_tokens: u64,
    uncached_input_tokens: u64,
    cache_read_tokens: u64,
    output_tokens: u64,
) -> Option<u64> {
    let catalog = catalog_slot()
        .read()
        .expect("pricing catalog lock poisoned");
    let catalog = catalog.as_ref()?;
    cost_for_catalog(
        &catalog,
        provider,
        model,
        cache_creation_tokens,
        uncached_input_tokens,
        cache_read_tokens,
        output_tokens,
    )
}

pub(crate) fn cache_savings_microusd(
    provider: ProviderKind,
    model: Option<&str>,
    cache_read_tokens: u64,
) -> u64 {
    let Some(model) = model.map(str::trim).filter(|model| !model.is_empty()) else {
        return 0;
    };
    let catalog = catalog_slot()
        .read()
        .expect("pricing catalog lock poisoned");
    let Some(rate) = catalog
        .as_ref()
        .and_then(|catalog| lookup_rate(catalog, provider, model))
    else {
        return 0;
    };
    (cache_read_tokens as f64 * (rate.input_per_token - rate.cache_read_per_token) * 1_000_000.0)
        .round()
        .clamp(0.0, u64::MAX as f64) as u64
}

fn cost_for_catalog(
    catalog: &PricingCatalog,
    provider: ProviderKind,
    model: Option<&str>,
    cache_creation_tokens: u64,
    uncached_input_tokens: u64,
    cache_read_tokens: u64,
    output_tokens: u64,
) -> Option<u64> {
    let model = model?.trim();
    if model.is_empty() {
        return None;
    }
    let rate = lookup_rate(catalog, provider, model)?;
    let cost_usd = uncached_input_tokens as f64 * rate.input_per_token
        + cache_read_tokens as f64 * rate.cache_read_per_token
        + cache_creation_tokens as f64 * rate.cache_creation_per_token
        + output_tokens as f64 * rate.output_per_token;
    Some((cost_usd * 1_000_000.0).round().clamp(0.0, u64::MAX as f64) as u64)
}

fn parse_rate_table(document: &Value) -> HashMap<String, ModelRate> {
    let mut table = HashMap::new();
    let Some(entries) = document.as_object() else {
        return table;
    };
    for (name, value) in entries {
        let Some(entry) = value.as_object() else {
            continue;
        };
        let Some(input) = finite_number(entry.get("input_cost_per_token")) else {
            continue;
        };
        let Some(output) = finite_number(entry.get("output_cost_per_token")) else {
            continue;
        };
        let key = name.trim().to_ascii_lowercase();
        if key.is_empty() {
            continue;
        }
        table.insert(
            key,
            ModelRate {
                input_per_token: input,
                output_per_token: output,
                cache_read_per_token: finite_number(entry.get("cache_read_input_token_cost"))
                    .unwrap_or(input),
                cache_creation_per_token: finite_number(
                    entry.get("cache_creation_input_token_cost"),
                )
                .unwrap_or(input),
            },
        );
    }

    let mut aliases: HashMap<String, Option<ModelRate>> = HashMap::new();
    for (key, rate) in &table {
        let alias = bare_model_name(key);
        if alias == *key || table.contains_key(alias) {
            continue;
        }
        match aliases.get(alias) {
            None => {
                aliases.insert(alias.to_owned(), Some(*rate));
            }
            Some(Some(existing)) if existing == rate => {}
            Some(_) => {
                aliases.insert(alias.to_owned(), None);
            }
        }
    }
    for (alias, rate) in aliases {
        if let Some(rate) = rate {
            table.insert(alias, rate);
        }
    }
    table
}

fn finite_number(value: Option<&Value>) -> Option<f64> {
    value
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value >= 0.0)
}

fn lookup_rate(
    catalog: &PricingCatalog,
    provider: ProviderKind,
    raw_model: &str,
) -> Option<ModelRate> {
    let mut key = raw_model.to_ascii_lowercase();
    if provider == ProviderKind::Cursor {
        // Cursor's Auto and Composer lanes are billed against the same
        // underlying model bucket as Grok 4.5. LiteLLM intentionally has no
        // public Cursor-specific entries for these UI labels.
        if matches!(key.as_str(), "auto" | "composer-2.5") {
            key = "grok-4.5".into();
        }
    }
    if is_unpriceable_model(&key) {
        return None;
    }
    if let Some(rate) = catalog.rates.get(&key).copied() {
        return Some(rate);
    }
    let candidates: Vec<String> = match provider {
        ProviderKind::Codex => vec![format!("openai/{key}")],
        ProviderKind::Claude => vec![format!("anthropic/{key}")],
        ProviderKind::Cursor => vec![
            format!("cursor/{key}"),
            format!("openai/{key}"),
            format!("anthropic/{key}"),
            format!("google/{key}"),
            format!("xai/{key}"),
            format!("x-ai/{key}"),
        ],
        _ => Vec::new(),
    };
    let mut resolved = None;
    for candidate in candidates {
        let Some(rate) = catalog.rates.get(&candidate).copied() else {
            continue;
        };
        if resolved.is_some_and(|existing| existing != rate) {
            return None;
        }
        resolved = Some(rate);
    }
    resolved
}

fn bare_model_name(key: &str) -> &str {
    key.rsplit('/').next().unwrap_or(key)
}

fn is_unpriceable_model(model: &str) -> bool {
    matches!(
        bare_model_name(model),
        "<synthetic>" | "synthetic" | "opus" | "sonnet" | "haiku" | "fable"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn catalog(document: Value) -> PricingCatalog {
        PricingCatalog {
            fetched_at: Utc::now(),
            rates: parse_rate_table(&document),
        }
    }

    #[test]
    fn prices_token_categories_and_does_not_charge_reasoning_twice() {
        let catalog = catalog(json!({
            "gpt-6-astra": {
                "input_cost_per_token": 0.00001,
                "output_cost_per_token": 0.00005,
                "cache_read_input_token_cost": 0.000001,
                "cache_creation_input_token_cost": 0.0000125
            }
        }));
        assert_eq!(
            cost_for_catalog(
                &catalog,
                ProviderKind::Codex,
                Some("gpt-6-astra"),
                100,
                200,
                300,
                400,
            ),
            Some(23_550)
        );
    }

    #[test]
    fn keeps_unknown_and_ambiguous_models_unpriced() {
        let catalog = catalog(json!({
            "openai/example": {
                "input_cost_per_token": 0.000001,
                "output_cost_per_token": 0.000002
            },
            "anthropic/example": {
                "input_cost_per_token": 0.000003,
                "output_cost_per_token": 0.000004
            }
        }));
        assert_eq!(
            cost_for_catalog(&catalog, ProviderKind::Codex, Some("missing"), 0, 1, 0, 1),
            None
        );
        assert_eq!(
            cost_for_catalog(&catalog, ProviderKind::Cursor, Some("example"), 0, 1, 0, 1),
            None
        );
    }

    #[test]
    fn cursor_reuses_the_same_anthropic_rate_for_a_bare_model_name() {
        let catalog = catalog(json!({
            "anthropic/claude-opus-5": {
                "input_cost_per_token": 0.000005,
                "output_cost_per_token": 0.000025
            }
        }));
        assert_eq!(
            cost_for_catalog(
                &catalog,
                ProviderKind::Cursor,
                Some("claude-opus-5"),
                0,
                1_000,
                0,
                100,
            ),
            Some(7_500)
        );
    }

    #[test]
    fn cursor_maps_auto_and_composer_to_grok_rate() {
        let catalog = catalog(json!({
            "xai/grok-4.5": {
                "input_cost_per_token": 0.000002,
                "output_cost_per_token": 0.000006
            }
        }));
        for model in ["auto", "composer-2.5"] {
            assert_eq!(
                cost_for_catalog(
                    &catalog,
                    ProviderKind::Cursor,
                    Some(model),
                    0,
                    1_000,
                    0,
                    100
                ),
                Some(2_600)
            );
        }
    }
}
