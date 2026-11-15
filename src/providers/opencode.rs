//! OpenCode Zen and OpenCode Go provider support.
//!
//! Zen authentication/model discovery comes from the OpenCode API. Go quota
//! windows come from its account-wide usage endpoint. Both providers use the
//! local OpenCode SQLite history for cost-oriented activity cards.

use std::{
    collections::BTreeMap,
    env,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Local, Utc};
use directories::BaseDirs;
use rusqlite::{Connection, OpenFlags, params};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    platform::secrets,
    providers::drivers::{Credential, Driver, InstallSource, InstallStatus, SettingsCopy, Sources},
    providers::instances::{ProviderId, ProviderInstance},
    providers::registry::ProviderKind,
    providers::worker::{LimitProvider, UsageProvider, WorkerHandle, WorkerOptions},
    quota::limits::{AdditionalLimit, LimitWindow, OpenCodeGoKeySnapshot, RateLimits},
    settings::OpenCodeKeys,
    store,
    usage::{Rollup, RollupBuilder, TokenUsage, UsageStatistics, statistics_from_daily},
};

const ZEN_MODELS_URL: &str = "https://opencode.ai/zen/v1/models";
const GO_USAGE_URL: &str = "https://opencode.ai/zen/go/v1/usage";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const ZEN_SECRET_NAME: &str = "opencode-zen";
const GO_SECRET_NAME: &str = "opencode-go";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Catalog {
    Zen,
    Go,
}

impl Catalog {
    const fn provider_id(self) -> &'static str {
        match self {
            Self::Zen => "opencode",
            Self::Go => "opencode-go",
        }
    }

    const fn secret_name(self) -> &'static str {
        match self {
            Self::Zen => ZEN_SECRET_NAME,
            Self::Go => GO_SECRET_NAME,
        }
    }
}

fn is_installed(provider: ProviderKind) -> bool {
    let Some(catalog) = catalog(provider) else {
        return false;
    };
    resolve_api_key(catalog, ProviderId::primary(provider))
        .ok()
        .flatten()
        .is_some()
        || database_has_provider(catalog).unwrap_or(false)
}

/// The primary instance keeps the original slot; others are named by id.
fn secret_name(provider: ProviderId) -> Option<String> {
    let catalog = catalog(provider.kind())?;
    Some(if provider.is_primary() {
        catalog.secret_name().to_owned()
    } else {
        format!("{}.{}", catalog.secret_name(), provider.id())
    })
}

/// Protected-storage name of one tracked key. The pre-multi-key credential
/// keeps the instance's original slot, so existing installations need no
/// migration; additional keys are suffixed with their stable identity.
pub fn key_secret_name(provider: ProviderId, key_id: &str) -> Option<String> {
    let base = secret_name(provider)?;
    Some(if key_id == OpenCodeKeys::LEGACY_KEY_ID {
        base
    } else {
        format!("{base}.{key_id}")
    })
}

/// The keys one instance reads, in settings order. Instances written before
/// multi-key support fall back to the single legacy credential.
pub fn tracked_key_ids(instance: &ProviderInstance) -> Vec<String> {
    let ids = instance.opencode_keys();
    if ids.is_empty() {
        vec![OpenCodeKeys::LEGACY_KEY_ID.to_owned()]
    } else {
        ids.to_vec()
    }
}

/// Whether one tracked key has a saved secret. The legacy key can also come
/// from the environment or OpenCode's auth file; see [`is_installed`].
pub fn key_is_configured(provider: ProviderId, key_id: &str) -> bool {
    key_secret_name(provider, key_id).is_some_and(|name| secrets::has_value(&name))
}

/// Collapsed fingerprint of a saved key, never the full secret.
pub fn key_hint(provider: ProviderId, key_id: &str) -> Result<Option<String>> {
    let Some(value) = key_secret_name(provider, key_id)
        .map(|name| secrets::load(&name))
        .transpose()?
        .flatten()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };
    Ok(Some(secrets::masked_hint(&value)))
}

/// Overlay locally chosen key names onto a live quota snapshot. A rename, or
/// clearing a name, is a settings-only change and must not wait for the next
/// worker poll or an app restart.
pub fn apply_key_names(limits: &mut RateLimits, instance: &ProviderInstance) -> bool {
    let keys = instance.opencode.as_ref();
    let mut changed = false;
    for key in &mut limits.opencode_go_keys {
        let local_name = keys
            .and_then(|keys| keys.name_of(&key.id))
            .map(str::to_owned);
        if key.local_name != local_name {
            key.local_name = local_name;
            changed = true;
        }
    }
    changed
}

/// The catalog metrics one tracked key reports, in display order. These are
/// the metric ids a key-scoped widget indicator stores.
pub fn key_metric_ids() -> Vec<String> {
    use crate::providers::registry::{MetricSource, OPENCODE_GO_MONTHLY_LIMIT_ID, descriptor};
    descriptor(ProviderKind::OpenCodeGo)
        .metrics
        .iter()
        .filter(|metric| match metric.source {
            MetricSource::Primary | MetricSource::Secondary => true,
            MetricSource::Additional(id) => id == OPENCODE_GO_MONTHLY_LIMIT_ID,
        })
        .map(|metric| metric.id.to_owned())
        .collect()
}

/// The window one tracked key reports for its driver's catalog metric.
pub fn key_window<'a>(key: &'a OpenCodeGoKeySnapshot, metric_id: &str) -> Option<&'a LimitWindow> {
    use crate::providers::registry::{MetricSource, OPENCODE_GO_MONTHLY_LIMIT_ID, metric};
    match metric(ProviderKind::OpenCodeGo, metric_id)?.source {
        MetricSource::Primary => Some(&key.rolling),
        MetricSource::Secondary => Some(&key.weekly),
        MetricSource::Additional(id) if id == OPENCODE_GO_MONTHLY_LIMIT_ID => key.monthly.as_ref(),
        _ => None,
    }
}

/// Replaces one tracked key and captures its prior ciphertext in one
/// transaction, so a failed settings commit can restore it.
pub(crate) fn apply_key(
    provider: ProviderId,
    key_id: &str,
    value: Option<&str>,
) -> Result<secrets::EncodedRollback> {
    let name = key_secret_name(provider, key_id).context("provider is not an OpenCode catalog")?;
    secrets::apply_with_rollback(&[(name, value.map(str::to_owned))])
}

/// Deletes one tracked key from protected storage.
pub fn forget_key(provider: ProviderId, key_id: &str) -> Result<()> {
    match key_secret_name(provider, key_id) {
        Some(name) => secrets::save(&name, None),
        None => Ok(()),
    }
}

pub fn manual_key(provider: ProviderId) -> Result<Option<String>> {
    secret_name(provider)
        .map(|name| secrets::load(&name))
        .unwrap_or_else(|| Ok(None))
}

pub fn save_manual_key(provider: ProviderId, value: Option<&str>) -> Result<()> {
    let name = secret_name(provider).context("provider is not an OpenCode catalog")?;
    secrets::save(&name, value)
}

pub(crate) fn restore_manual_key(rollback: secrets::EncodedRollback) -> Result<()> {
    rollback.restore()
}

/// Whether the instance has at least one saved key, including the legacy
/// single-key slot.
pub fn has_saved_key(instance: &ProviderInstance) -> bool {
    let provider = instance.provider_id();
    tracked_key_ids(instance)
        .iter()
        .any(|key_id| key_is_configured(provider, key_id))
}

/// One OpenCode Go key the instance queries, in settings order.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TrackedKey {
    id: String,
    name: Option<String>,
}

#[derive(Clone)]
pub struct OpenCodeClient {
    catalog: Catalog,
    provider: ProviderId,
    /// OpenCode Go keys from the instance. Empty means the pre-multi-key
    /// credential resolved from the environment or OpenCode's auth file.
    keys: Vec<TrackedKey>,
    agent: ureq::Agent,
    /// Set when any key's usage request was rate limited, even if another key
    /// still produced the instance's aggregate sample.
    rate_limited: Arc<AtomicBool>,
}

impl OpenCodeClient {
    pub fn new(provider: ProviderId) -> Result<Self> {
        let catalog = catalog(provider.kind()).context("provider is not an OpenCode catalog")?;
        Ok(Self {
            catalog,
            provider,
            keys: Vec::new(),
            agent: ureq::AgentBuilder::new().timeout(REQUEST_TIMEOUT).build(),
            rate_limited: Arc::new(AtomicBool::new(false)),
        })
    }

    /// A client for one configured instance, querying every tracked key.
    pub fn for_instance(instance: &ProviderInstance) -> Result<Self> {
        let mut client = Self::new(instance.provider_id())?;
        if client.catalog == Catalog::Go {
            let keys = instance.opencode.as_ref();
            client.keys = instance
                .opencode_keys()
                .iter()
                .map(|id| TrackedKey {
                    id: id.clone(),
                    name: keys.and_then(|keys| keys.name_of(id)).map(str::to_owned),
                })
                .collect();
        }
        Ok(client)
    }

    /// Keys to query, including the legacy slot when settings never listed one.
    fn go_keys(&self) -> Vec<TrackedKey> {
        if self.keys.is_empty() {
            vec![TrackedKey {
                id: OpenCodeKeys::LEGACY_KEY_ID.to_owned(),
                name: None,
            }]
        } else {
            self.keys.clone()
        }
    }

    /// Resolves one key's secret. The legacy key keeps the environment and
    /// auth-file fallbacks; additional keys only use their own saved value.
    fn resolve_key(&self, key: &TrackedKey) -> Result<Option<String>> {
        if key.id == OpenCodeKeys::LEGACY_KEY_ID {
            return resolve_api_key(self.catalog, self.provider);
        }
        let Some(name) = key_secret_name(self.provider, &key.id) else {
            return Ok(None);
        };
        Ok(secrets::load(&name)?.filter(|value| !value.trim().is_empty()))
    }

    fn read_limits(&self) -> Result<RateLimits> {
        match self.catalog {
            Catalog::Zen => self.read_zen_limits(),
            Catalog::Go => self.read_go_limits(),
        }
    }

    fn read_zen_limits(&self) -> Result<RateLimits> {
        let key = resolve_api_key(self.catalog, self.provider)?.context(
            "OpenCode Zen API key not found; set OPENCODE_API_KEY, ZEN_API_KEY, use auth.json, or save a manual key",
        )?;
        let response = self
            .agent
            .get(ZEN_MODELS_URL)
            .set("Authorization", &format!("Bearer {key}"))
            .set("Accept", "application/json")
            .call()
            .context("request OpenCode Zen models")?;
        let status = response.status();
        let body = response
            .into_string()
            .context("read OpenCode Zen models response")?;
        if status != 200 {
            return Err(api_error("OpenCode Zen", status, &body));
        }
        let model_count = parse_model_count(&body)?;
        Ok(RateLimits {
            sampled_at: Utc::now(),
            plan_type: Some(format!("Zen · {model_count} models")),
            limit_name: Some("OpenCode Zen".into()),
            ..RateLimits::default()
        })
    }

    fn read_go_limits(&self) -> Result<RateLimits> {
        let snapshots = self
            .go_keys()
            .iter()
            .map(|key| self.read_go_key(key))
            .collect::<Vec<_>>();
        if snapshots.iter().all(|snapshot| snapshot.error.is_some()) {
            let first = snapshots
                .into_iter()
                .next()
                .expect("go_keys always yields one key");
            return Err(anyhow!(
                first
                    .error
                    .unwrap_or_else(|| "OpenCode Go usage failed".into())
            ));
        }
        Ok(aggregate_go_keys(snapshots))
    }

    /// Reads one key's usage. A failure becomes a key-local row so the other
    /// keys still report their quotas.
    fn read_go_key(&self, key: &TrackedKey) -> OpenCodeGoKeySnapshot {
        let sampled_at = Utc::now();
        let mut snapshot = OpenCodeGoKeySnapshot {
            id: key.id.clone(),
            local_name: key.name.clone(),
            masked_key: key_secret_name(self.provider, &key.id)
                .map(|name| secrets::load(&name))
                .transpose()
                .ok()
                .flatten()
                .flatten()
                .filter(|value| !value.trim().is_empty())
                .map(|value| secrets::masked_hint(&value)),
            sampled_at,
            ..OpenCodeGoKeySnapshot::default()
        };
        let value = match self.resolve_key(key) {
            Ok(Some(value)) => value,
            Ok(None) => {
                snapshot.error = Some(go_missing_key_message(key));
                return snapshot;
            }
            Err(error) => {
                self.note_rate_limit(&error);
                snapshot.error = Some(go_error_message(&error));
                return snapshot;
            }
        };
        match self
            .request_go_usage(&value)
            .and_then(|body| parse_go_windows(&body, sampled_at))
        {
            Ok((rolling, weekly, monthly)) => {
                snapshot.rolling = rolling;
                snapshot.weekly = weekly;
                snapshot.monthly = monthly;
            }
            // Key-local errors reach the log, the store and the provider tab.
            // The row is one line wide, so only the reason is kept.
            Err(error) => {
                self.note_rate_limit(&error);
                snapshot.error = Some(go_error_message(&error));
            }
        }
        snapshot
    }

    /// A 429 from one key must reach the worker even when another key still
    /// reports, so the provider's pause is honoured.
    fn note_rate_limit(&self, error: &anyhow::Error) {
        if http_status_of(error) == Some(429) {
            self.rate_limited.store(true, Ordering::Relaxed);
        }
    }

    fn request_go_usage(&self, key: &str) -> Result<String> {
        let response = self
            .agent
            .get(GO_USAGE_URL)
            .set("Authorization", &format!("Bearer {key}"))
            .set("Accept", "application/json")
            .call()
            .context("request OpenCode Go usage")?;
        let status = response.status();
        let body = response
            .into_string()
            .context("read OpenCode Go usage response")?;
        if status != 200 {
            return Err(anyhow::Error::new(HttpStatusError(status)));
        }
        Ok(body)
    }

    fn read_local_usage(&self) -> Result<Rollup> {
        let Some(path) = database_path() else {
            bail!("OpenCode data directory is unavailable")
        };
        self.read_local_usage_from_path(&path)
    }

    fn read_local_usage_from_path(&self, path: &PathBuf) -> Result<Rollup> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("open OpenCode database read-only at {}", path.display()))?;
        let oldest_ms = (Utc::now()
            - chrono::Duration::days(i64::from(crate::platform::local_time::RETENTION_DAYS)))
        .timestamp_millis();
        let mut statement = connection.prepare(
            "SELECT
                 CAST(COALESCE(json_extract(data, '$.time.created'), time_created) AS INTEGER),
                 CAST(json_extract(data, '$.cost') AS REAL),
                 COALESCE(CAST(json_extract(data, '$.tokens.input') AS INTEGER), 0),
                 COALESCE(CAST(json_extract(data, '$.tokens.cache.read') AS INTEGER), 0),
                 COALESCE(CAST(json_extract(data, '$.tokens.output') AS INTEGER), 0),
                 COALESCE(CAST(json_extract(data, '$.tokens.reasoning') AS INTEGER), 0)
             FROM message
             WHERE json_valid(data)
               AND json_extract(data, '$.providerID') = ?1
               AND json_extract(data, '$.role') = 'assistant'
               AND json_type(data, '$.cost') IN ('integer', 'real')
               AND CAST(COALESCE(json_extract(data, '$.time.created'), time_created) AS INTEGER) >= ?2
             ORDER BY 1 ASC",
        )?;
        let rows = statement.query_map(params![self.catalog.provider_id(), oldest_ms], |row| {
            let timestamp_ms = row.get::<_, i64>(0)?;
            let cost = row.get::<_, f64>(1)?;
            let input = non_negative_i64(row.get::<_, i64>(2)?);
            let cache_read = non_negative_i64(row.get::<_, i64>(3)?);
            let output = non_negative_i64(row.get::<_, i64>(4)?);
            let reasoning = non_negative_i64(row.get::<_, i64>(5)?);
            Ok((timestamp_ms, cost, input, cache_read, output, reasoning))
        })?;

        let mut rollup = RollupBuilder::new(Local::now());
        for row in rows {
            let (timestamp_ms, cost, input, cache_read, output, reasoning) = row?;
            if !cost.is_finite() || cost < 0.0 {
                continue;
            }
            let Some(timestamp) = DateTime::<Utc>::from_timestamp_millis(timestamp_ms) else {
                continue;
            };
            let usage = TokenUsage {
                input_tokens: input,
                cached_input_tokens: cache_read,
                // OpenCode stores visible output and reasoning separately;
                // expose their completion total once in the existing output
                // field while using the stored cost as the spend authority.
                output_tokens: output.saturating_add(reasoning),
                requests: 1,
                estimated_cost_microusd: crate::format::money::from_usd(cost),
                priced_requests: 1,
                ..Default::default()
            };
            rollup.add(timestamp.with_timezone(&Local), &usage, None, None);
        }
        Ok(rollup.finish())
    }
}

impl LimitProvider for OpenCodeClient {
    fn read_limits(&mut self) -> Result<RateLimits> {
        OpenCodeClient::read_limits(self)
    }

    fn take_rate_limit_response(&mut self) -> bool {
        self.rate_limited.swap(false, Ordering::Relaxed)
    }
}

impl UsageProvider for OpenCodeClient {
    fn load_cached_usage_statistics(&mut self, history_days: u16) -> Result<UsageStatistics> {
        store::with_store(|store| store.load_usage_daily(self.provider, history_days)).or_else(
            |error| {
                crate::platform::logger::info(format!(
                    "failed to read cached OpenCode usage: {error:#}"
                ));
                Ok(UsageStatistics::default())
            },
        )
    }

    fn refresh_usage_statistics(&mut self, history_days: u16) -> Result<UsageStatistics> {
        match self.read_local_usage() {
            Ok(rollup) => {
                store::with_store(|store| store.save_rollup(self.provider, &rollup, None))?;
                Ok(statistics_from_daily(&rollup.daily, history_days))
            }
            Err(error) => {
                store::with_store(|store| store.load_usage_daily(self.provider, history_days))
                    .context("refresh OpenCode local usage")
                    .or(Err(error))
            }
        }
    }

    fn refresh_without_limits(&self) -> bool {
        true
    }
}

pub(crate) static ZEN_DRIVER: Driver = Driver {
    start: start_worker,
    detect,
    sources: Sources::APP,
    copy: zen_settings_copy,
    forget_secrets: Some(forget_secrets),
    consume_reset: None,
};

pub(crate) static GO_DRIVER: Driver = Driver {
    copy: go_settings_copy,
    ..ZEN_DRIVER
};

fn start_worker(instance: &ProviderInstance, options: WorkerOptions) -> Result<WorkerHandle> {
    let client = OpenCodeClient::for_instance(instance)?;
    Ok(crate::providers::worker::start_worker(
        client.clone(),
        client,
        None,
        options,
    ))
}

fn detect(instance: &ProviderInstance) -> InstallStatus {
    let provider = instance.provider_id();
    // Go tracks several keys per instance, so any saved key counts. Its legacy
    // slot also falls back to the environment and OpenCode's auth file.
    let (detected, detail) = if provider.kind() == ProviderKind::OpenCodeGo {
        // Environment variables and OpenCode's auth file describe the first
        // instance's login only; other instances depend on their saved keys.
        let installed = instance.is_primary() && is_installed(instance.driver);
        (
            installed || has_saved_key(instance),
            if installed {
                crate::i18n::tr("opencode-auth-json-or-local-database")
            } else {
                crate::i18n::tr("saved-api-key")
            },
        )
    } else if provider.is_primary() {
        // Secondary instances read only their own API key.
        (
            is_installed(instance.driver),
            crate::i18n::tr("opencode-auth-json-or-local-database"),
        )
    } else {
        (has_saved_key(instance), crate::i18n::tr("saved-api-key"))
    };
    InstallStatus {
        app: detected.then(|| detail.into()),
        used: detected.then_some(InstallSource::App),
        ..InstallStatus::none(instance)
    }
}

fn settings_copy(description: &'static str) -> SettingsCopy {
    SettingsCopy {
        found: crate::i18n::tr("found-in-opencode-auth-or-local-history"),
        not_found: crate::i18n::tr("not-found-turn-it-on-if-it-s-set-up-elsewhere"),
        credential: Credential::ApiKey,
        ..SettingsCopy::new(description)
    }
}

fn zen_settings_copy() -> SettingsCopy {
    settings_copy(crate::i18n::tr("reads-zen-auth-and-local-opencode-history"))
}

fn go_settings_copy() -> SettingsCopy {
    settings_copy(crate::i18n::tr(
        "reads-go-quota-windows-and-local-opencode-history",
    ))
}

fn forget_secrets(instance: &ProviderInstance) -> Result<()> {
    let provider = instance.provider_id();
    let mut changes: Vec<(String, Option<String>)> = Vec::new();
    for key_id in tracked_key_ids(instance) {
        if let Some(name) = key_secret_name(provider, &key_id) {
            changes.push((name, None));
        }
    }
    // Drop the instance's legacy slot even when settings list their own ids.
    if let Some(name) = secret_name(provider)
        && !changes.iter().any(|(existing, _)| existing == &name)
    {
        changes.push((name, None));
    }
    secrets::save_many(&changes)
}

fn catalog(provider: ProviderKind) -> Option<Catalog> {
    match provider {
        ProviderKind::OpenCodeZen => Some(Catalog::Zen),
        ProviderKind::OpenCodeGo => Some(Catalog::Go),
        _ => None,
    }
}

/// Additional instances only use their own saved key; environment variables
/// and OpenCode's auth file describe this PC's primary login.
fn resolve_api_key(catalog: Catalog, provider: ProviderId) -> Result<Option<String>> {
    let manual = manual_key(provider)?;
    if !provider.is_primary() {
        return Ok(manual.filter(|key| !key.trim().is_empty()));
    }
    let provider_env_names: &[&str] = match catalog {
        Catalog::Zen => &["OPENCODE_ZEN_API_KEY", "ZEN_API_KEY"],
        Catalog::Go => &["OPENCODE_GO_API_KEY"],
    };
    let provider_env = provider_env_names.iter().find_map(|name| env_key(name));
    let common_env = env_key("OPENCODE_API_KEY");
    let auth_key = auth_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|raw| serde_json::from_str::<BTreeMap<String, AuthEntry>>(&raw).ok())
        .and_then(|entries| {
            entries
                .get(catalog.provider_id())
                .and_then(|entry| entry.key.clone())
        });
    Ok(select_api_key(manual, provider_env, common_env, auth_key))
}

fn env_key(name: &str) -> Option<String> {
    env::var_os(name)
        .map(|value| value.to_string_lossy().trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn select_api_key(
    manual: Option<String>,
    provider_env: Option<String>,
    common_env: Option<String>,
    auth_key: Option<String>,
) -> Option<String> {
    [manual, provider_env, common_env, auth_key]
        .into_iter()
        .flatten()
        .map(|value| value.trim().to_owned())
        .find(|value| !value.is_empty())
}

#[derive(Deserialize)]
struct AuthEntry {
    key: Option<String>,
}

fn auth_path() -> Option<PathBuf> {
    data_root().map(|root| root.join("auth.json"))
}

fn database_path() -> Option<PathBuf> {
    data_root().map(|root| root.join("opencode.db"))
}

fn data_root() -> Option<PathBuf> {
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| BaseDirs::new().map(|dirs| dirs.home_dir().join(".local").join("share")))
        .map(|path| path.join("opencode"))
}

fn database_has_provider(catalog: Catalog) -> Result<bool> {
    let Some(path) = database_path() else {
        return Ok(false);
    };
    if !path.is_file() {
        return Ok(false);
    }
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    Ok(connection.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM message
             WHERE json_valid(data)
               AND json_extract(data, '$.providerID') = ?1
               AND json_extract(data, '$.role') = 'assistant'
         )",
        [catalog.provider_id()],
        |row| row.get(0),
    )?)
}

fn parse_model_count(body: &str) -> Result<usize> {
    let value: Value = serde_json::from_str(body).context("parse OpenCode Zen models response")?;
    let models = value
        .as_array()
        .or_else(|| value.get("data").and_then(Value::as_array))
        .or_else(|| value.get("models").and_then(Value::as_array))
        .context("OpenCode Zen models response contains no model array")?;
    Ok(models.len())
}

/// Single-key shape of the Go endpoint. The instance path reads one snapshot
/// per tracked key and aggregates them in [`aggregate_go_keys`].
#[cfg(test)]
fn parse_go_usage(body: &str, sampled_at: DateTime<Utc>) -> Result<RateLimits> {
    let (rolling, weekly, monthly) = parse_go_windows(body, sampled_at)?;
    let additional_limits = monthly
        .map(|window| AdditionalLimit {
            id: crate::providers::registry::OPENCODE_GO_MONTHLY_LIMIT_ID.into(),
            title: "Monthly".into(),
            window,
        })
        .into_iter()
        .collect();
    Ok(RateLimits {
        primary: rolling,
        secondary: weekly,
        sampled_at,
        plan_type: Some("Go".into()),
        limit_name: Some("OpenCode Go".into()),
        additional_limits,
        ..RateLimits::default()
    })
}

/// The three quota windows one usage response reports.
fn parse_go_windows(
    body: &str,
    sampled_at: DateTime<Utc>,
) -> Result<(LimitWindow, LimitWindow, Option<LimitWindow>)> {
    let value: Value = serde_json::from_str(body).context("parse OpenCode Go usage response")?;
    let usage = usage_object(&value);
    let rolling = parse_window(
        usage,
        &["rolling", "rollingUsage", "rolling_usage"],
        sampled_at,
    )
    .context("OpenCode Go response has no rolling usage window")?;
    let weekly = parse_window(
        usage,
        &["weekly", "weeklyUsage", "weekly_usage"],
        sampled_at,
    )
    .unwrap_or_default();
    let monthly = parse_window(
        usage,
        &["monthly", "monthlyUsage", "monthly_usage"],
        sampled_at,
    );
    Ok((rolling, weekly, monthly))
}

/// Combines per-key samples into the instance's lanes: each window shows the
/// most-consumed key so tray, Home and notifications keep their meaning,
/// while every key keeps its own row for the provider tab.
fn aggregate_go_keys(snapshots: Vec<OpenCodeGoKeySnapshot>) -> RateLimits {
    let sampled_at = snapshots
        .iter()
        .map(|snapshot| snapshot.sampled_at)
        .max()
        .unwrap_or_else(Utc::now);
    let rolling = worst_window(
        snapshots
            .iter()
            .filter(|snapshot| snapshot.error.is_none())
            .map(|snapshot| &snapshot.rolling),
    );
    let weekly = worst_window(
        snapshots
            .iter()
            .filter(|snapshot| snapshot.error.is_none())
            .map(|snapshot| &snapshot.weekly),
    );
    let monthly = {
        let window = worst_window(
            snapshots
                .iter()
                .filter(|snapshot| snapshot.error.is_none())
                .filter_map(|snapshot| snapshot.monthly.as_ref()),
        );
        (!window.is_empty()).then_some(window)
    };
    let additional_limits = monthly
        .clone()
        .map(|window| AdditionalLimit {
            id: crate::providers::registry::OPENCODE_GO_MONTHLY_LIMIT_ID.into(),
            title: "Monthly".into(),
            window,
        })
        .into_iter()
        .collect();
    let key_count = snapshots.len();
    RateLimits {
        primary: rolling,
        secondary: weekly,
        sampled_at,
        plan_type: Some(if key_count > 1 {
            format!("Go · {key_count} keys")
        } else {
            "Go".into()
        }),
        limit_name: Some("OpenCode Go".into()),
        additional_limits,
        opencode_go_keys: snapshots,
        ..RateLimits::default()
    }
}

/// The most-consumed window across keys. Ties keep the earlier key, and a
/// reset-only window still counts when no key reports a percentage.
fn worst_window<'a>(windows: impl Iterator<Item = &'a LimitWindow>) -> LimitWindow {
    let mut worst: Option<&LimitWindow> = None;
    for window in windows {
        if window.is_empty() {
            continue;
        }
        let better = worst.is_none_or(|current| {
            window.used_percent.unwrap_or(0) > current.used_percent.unwrap_or(0)
        });
        if better {
            worst = Some(window);
        }
    }
    worst.cloned().unwrap_or_default()
}

/// Why one key produced no sample, phrased for the provider tab and the
/// worker's error line.
fn go_missing_key_message(key: &TrackedKey) -> String {
    if key.id == OpenCodeKeys::LEGACY_KEY_ID {
        "No OpenCode Go API key found".into()
    } else {
        format!(
            "No saved value for {}",
            key.name.as_deref().unwrap_or(&key.id)
        )
    }
}

/// A short reason for a failed key. The card is one line wide, so the raw
/// transport error (URLs, retry hints) is reduced to what the row can show.
fn go_error_message(error: &anyhow::Error) -> String {
    if let Some(status) = http_status_of(error) {
        return format!("OpenCode Go returned HTTP {status}");
    }
    if format!("{error:#}").contains("parse OpenCode Go usage response") {
        return "OpenCode Go sent an unexpected response".into();
    }
    "Could not reach OpenCode Go".into()
}

/// The numeric HTTP status carried by an error chain, if it is one.
fn http_status_of(error: &anyhow::Error) -> Option<u16> {
    error.chain().find_map(|cause| {
        if let Some(ureq::Error::Status(status, _)) = cause.downcast_ref::<ureq::Error>() {
            return Some(*status);
        }
        cause.downcast_ref::<HttpStatusError>().map(|error| error.0)
    })
}

/// A response that was not `200`. The body never becomes part of the message,
/// so a server echoing the credential cannot leak it.
#[derive(Debug)]
struct HttpStatusError(u16);

impl std::fmt::Display for HttpStatusError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "OpenCode Go returned HTTP {}", self.0)
    }
}

impl std::error::Error for HttpStatusError {}

fn usage_object(value: &Value) -> &Value {
    if value.get("usage").is_some_and(Value::is_object) {
        return value.get("usage").expect("checked above");
    }
    value.get("data").map(usage_object).unwrap_or(value)
}

fn parse_window(parent: &Value, names: &[&str], sampled_at: DateTime<Utc>) -> Option<LimitWindow> {
    let value = names.iter().find_map(|name| parent.get(*name))?;
    let object = value.as_object()?;
    let percent = ["percent", "usagePercent", "usedPercent", "percentUsed"]
        .iter()
        .find_map(|key| object.get(*key).and_then(number))?;
    let resets_at = ["resetsAt", "resetAt", "resets_at", "nextReset"]
        .iter()
        .find_map(|key| object.get(*key).and_then(parse_reset_value));
    let reset_in_seconds = [
        "resetInSec",
        "resetInSeconds",
        "resetSeconds",
        "reset_in_sec",
    ]
    .iter()
    .find_map(|key| object.get(*key).and_then(Value::as_i64));
    let resets_at = resets_at.or_else(|| {
        reset_in_seconds.and_then(|seconds| {
            let reset = chrono::Duration::try_seconds(seconds.max(0))
                .and_then(|duration| sampled_at.checked_add_signed(duration));
            if reset.is_none() {
                crate::platform::logger::info("OpenCode returned an out-of-range reset interval");
            }
            reset
        })
    });
    Some(LimitWindow {
        // The OpenCode Go API reports percentage points. In particular, 1 is
        // one percent, not a fractional 1.0 that should become 100.
        used_percent: Some(percent.round().clamp(0.0, 100.0) as u8),
        resets_at,
        // Do not invent a duration for pace calculations from a reset-only
        // response; the API does not provide the window start.
        duration_minutes: None,
    })
}

fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|value| value as f64))
}

fn parse_reset_value(value: &Value) -> Option<DateTime<Utc>> {
    if let Some(raw) = value.as_str() {
        return DateTime::parse_from_rfc3339(raw)
            .ok()
            .map(|date| date.with_timezone(&Utc));
    }
    let numeric = number(value)?;
    if numeric > 10_000_000_000.0 {
        DateTime::<Utc>::from_timestamp_millis(numeric as i64)
    } else {
        DateTime::<Utc>::from_timestamp(numeric as i64, 0)
    }
}

fn api_error(provider: &str, status: u16, body: &str) -> anyhow::Error {
    let detail = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            ["message", "error", "detail"]
                .iter()
                .find_map(|key| value.get(*key).and_then(Value::as_str).map(str::to_owned))
        })
        .unwrap_or_default();
    if detail.is_empty() {
        anyhow!("{provider} API returned HTTP {status}")
    } else {
        anyhow!("{provider} API returned HTTP {status}: {detail}")
    }
}

fn non_negative_i64(value: i64) -> u64 {
    value.max(0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn go_usage_ignores_out_of_range_reset_intervals() {
        for (sampled_at, seconds) in [
            (Utc::now(), i64::MAX),
            (Utc::now(), i64::MAX / 1000),
            (DateTime::<Utc>::MAX_UTC, 1),
        ] {
            let raw = serde_json::json!({"rolling": {"percent": 25, "resetInSec": seconds}});
            let limits = parse_go_usage(&raw.to_string(), sampled_at).unwrap();
            assert_eq!(limits.primary.used_percent, Some(25));
            assert_eq!(limits.primary.resets_at, None);
        }
    }

    #[test]
    fn go_usage_maps_all_windows_without_rescaling_one_percent() {
        let sampled_at = Utc.with_ymd_and_hms(2026, 8, 29, 10, 0, 0).unwrap();
        let limits = parse_go_usage(
            r#"{
                "usage": {
                    "rolling": {"percent": 1, "resetsAt": "2026-08-29T15:00:00Z"},
                    "weekly": {"percent": 15, "resetInSec": 3600},
                    "monthly": {"usagePercent": 4, "resetInSec": 7200}
                }
            }"#,
            sampled_at,
        )
        .unwrap();
        assert_eq!(limits.primary.used_percent, Some(1));
        assert_eq!(limits.secondary.used_percent, Some(15));
        assert_eq!(limits.additional_limits[0].window.used_percent, Some(4));
        assert_eq!(limits.plan_type.as_deref(), Some("Go"));
        assert!(limits.primary.duration_minutes.is_none());
    }

    #[test]
    fn go_usage_accepts_nested_fields_and_rejects_missing_rolling_window() {
        let limits = parse_go_usage(
            r#"{"data":{"usage":{"rollingUsage":{"usagePercent":0.5,"resetInSec":10}}}}"#,
            Utc::now(),
        )
        .unwrap();
        assert_eq!(limits.primary.used_percent, Some(1));
        assert!(parse_go_usage(r#"{"usage":{"weekly":{"percent":2}}}"#, Utc::now()).is_err());
    }

    #[test]
    fn multi_key_aggregation_takes_the_most_consumed_key_for_every_lane() {
        let sampled_at = Utc.with_ymd_and_hms(2026, 8, 29, 10, 0, 0).unwrap();
        let key = |id: &str, rolling: u8, weekly: u8, monthly: u8| OpenCodeGoKeySnapshot {
            id: id.into(),
            rolling: LimitWindow {
                used_percent: Some(rolling),
                resets_at: Some(sampled_at + chrono::Duration::minutes(30)),
                duration_minutes: None,
            },
            weekly: LimitWindow {
                used_percent: Some(weekly),
                ..LimitWindow::default()
            },
            monthly: Some(LimitWindow {
                used_percent: Some(monthly),
                ..LimitWindow::default()
            }),
            sampled_at,
            ..OpenCodeGoKeySnapshot::default()
        };
        let limits = aggregate_go_keys(vec![key("a", 12, 40, 5), key("b", 70, 20, 9)]);
        assert_eq!(limits.primary.used_percent, Some(70));
        assert_eq!(limits.secondary.used_percent, Some(40));
        assert_eq!(limits.additional_limits[0].window.used_percent, Some(9));
        assert_eq!(limits.plan_type.as_deref(), Some("Go · 2 keys"));
        assert_eq!(limits.opencode_go_keys.len(), 2);
        assert_eq!(limits.opencode_go_keys[0].id, "a");
    }

    #[test]
    fn a_failed_key_keeps_its_row_without_lowering_the_instance_lanes() {
        let sampled_at = Utc.with_ymd_and_hms(2026, 8, 29, 10, 0, 0).unwrap();
        let good = OpenCodeGoKeySnapshot {
            id: "good".into(),
            rolling: LimitWindow {
                used_percent: Some(30),
                ..LimitWindow::default()
            },
            weekly: LimitWindow {
                used_percent: Some(10),
                ..LimitWindow::default()
            },
            sampled_at,
            ..OpenCodeGoKeySnapshot::default()
        };
        let bad = OpenCodeGoKeySnapshot {
            id: "bad".into(),
            error: Some("OpenCode Go API returned HTTP 429".into()),
            sampled_at,
            ..OpenCodeGoKeySnapshot::default()
        };
        let limits = aggregate_go_keys(vec![good, bad]);
        assert_eq!(limits.primary.used_percent, Some(30));
        assert_eq!(
            limits.opencode_go_keys[1].error.as_deref(),
            Some("OpenCode Go API returned HTTP 429")
        );
    }

    #[test]
    fn live_key_names_overlay_onto_a_snapshot() {
        let mut instance = ProviderInstance::primary(ProviderKind::OpenCodeGo);
        instance.opencode = Some(crate::settings::OpenCodeKeys {
            key_ids: vec!["a".into(), "b".into()],
            key_names: std::collections::BTreeMap::from([("a".to_owned(), "Team A".to_owned())]),
        });
        let mut limits = RateLimits {
            opencode_go_keys: vec![
                OpenCodeGoKeySnapshot {
                    id: "a".into(),
                    local_name: Some("Old".into()),
                    ..OpenCodeGoKeySnapshot::default()
                },
                OpenCodeGoKeySnapshot {
                    id: "b".into(),
                    local_name: Some("Cleared".into()),
                    ..OpenCodeGoKeySnapshot::default()
                },
            ],
            ..RateLimits::default()
        };
        assert!(apply_key_names(&mut limits, &instance));
        assert_eq!(
            limits.opencode_go_keys[0].local_name.as_deref(),
            Some("Team A")
        );
        assert_eq!(limits.opencode_go_keys[1].local_name, None);
        // Nothing left to change on the second pass.
        assert!(!apply_key_names(&mut limits, &instance));
    }

    #[test]
    fn key_errors_are_short_and_never_carry_the_credential() {
        let http = anyhow::Error::new(HttpStatusError(401));
        assert_eq!(go_error_message(&http), "OpenCode Go returned HTTP 401");
        let offline = anyhow!(
            "request OpenCode Go usage: https://opencode.ai/zen/go/v1/usage: connection refused"
        );
        assert_eq!(go_error_message(&offline), "Could not reach OpenCode Go");
        let parse = anyhow!("parse OpenCode Go usage response: invalid json");
        assert_eq!(
            go_error_message(&parse),
            "OpenCode Go sent an unexpected response"
        );
        // The status comes from the typed error, not from words in the text.
        let misleading = anyhow!("request failed near HTTP 200 docs");
        assert_eq!(go_error_message(&misleading), "Could not reach OpenCode Go");
    }

    #[test]
    fn a_rate_limited_key_reaches_the_worker_even_when_another_reports() {
        let mut client =
            OpenCodeClient::new(ProviderId::primary(ProviderKind::OpenCodeGo)).unwrap();
        client.note_rate_limit(&anyhow::Error::new(HttpStatusError(429)));
        assert!(LimitProvider::take_rate_limit_response(&mut client));
        assert!(!LimitProvider::take_rate_limit_response(&mut client));

        client.note_rate_limit(&anyhow::Error::new(HttpStatusError(401)));
        assert!(!LimitProvider::take_rate_limit_response(&mut client));
    }

    #[test]
    fn key_secret_names_keep_the_legacy_slot_and_scope_extra_keys() {
        let primary = ProviderId::primary(ProviderKind::OpenCodeGo);
        assert_eq!(
            key_secret_name(primary, OpenCodeKeys::LEGACY_KEY_ID).unwrap(),
            "opencode-go"
        );
        assert_eq!(key_secret_name(primary, "k1").unwrap(), "opencode-go.k1");
        let secondary = ProviderId::new(ProviderKind::OpenCodeGo, "opencode-go-1a2b");
        assert_eq!(
            key_secret_name(secondary, OpenCodeKeys::LEGACY_KEY_ID).unwrap(),
            "opencode-go.opencode-go-1a2b"
        );
        assert_eq!(
            key_secret_name(secondary, "k1").unwrap(),
            "opencode-go.opencode-go-1a2b.k1"
        );
    }

    #[test]
    fn instances_from_before_multi_key_tracking_query_the_legacy_slot() {
        let mut instance = ProviderInstance::primary(ProviderKind::OpenCodeGo);
        instance.opencode = None;
        assert_eq!(tracked_key_ids(&instance), ["primary"]);
        instance.opencode = Some(OpenCodeKeys {
            key_ids: vec!["work".into(), "private".into()],
            ..OpenCodeKeys::default()
        });
        assert_eq!(tracked_key_ids(&instance), ["work", "private"]);
    }

    #[test]
    fn zen_models_response_accepts_data_envelope() {
        assert_eq!(
            parse_model_count(r#"{"data":[{"id":"a"},{"id":"b"}]}"#).unwrap(),
            2
        );
        assert_eq!(parse_model_count(r#"[{"id":"a"}]"#).unwrap(), 1);
    }

    #[test]
    fn manual_key_wins_over_provider_env_common_env_and_auth() {
        assert_eq!(
            select_api_key(
                Some(" manual ".into()),
                Some("provider".into()),
                Some("common".into()),
                Some("auth".into()),
            )
            .as_deref(),
            Some("manual")
        );
        assert_eq!(
            select_api_key(
                None,
                Some("provider".into()),
                Some("common".into()),
                Some("auth".into()),
            )
            .as_deref(),
            Some("provider")
        );
        assert_eq!(
            select_api_key(None, None, None, Some("auth".into())).as_deref(),
            Some("auth")
        );
    }

    #[test]
    fn reads_only_matching_assistant_rows_from_open_code_sqlite() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("opencode.db");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE message(
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL,
                    time_created INTEGER NOT NULL,
                    time_updated INTEGER NOT NULL,
                    data TEXT NOT NULL
                );",
            )
            .unwrap();
        let now = Utc::now();
        let today_ms = now.timestamp_millis();
        let yesterday_ms = (now - chrono::Duration::days(1)).timestamp_millis();
        let insert = |id: &str, timestamp: i64, data: &str| {
            connection
                .execute(
                    "INSERT INTO message(id, session_id, time_created, time_updated, data)
                     VALUES(?1, 'session', ?2, ?2, ?3)",
                    params![id, timestamp, data],
                )
                .unwrap();
        };
        insert(
            "zen",
            today_ms,
            &serde_json::json!({
                "role":"assistant",
                "providerID":"opencode",
                "cost":1.25,
                "modelID":"gpt-5.6-sol",
                "time":{"created":today_ms},
                "tokens":{"input":10,"output":4,"reasoning":3,"cache":{"read":2}}
            })
            .to_string(),
        );
        insert(
            "go-free",
            yesterday_ms,
            &serde_json::json!({
                "role":"assistant",
                "providerID":"opencode-go",
                "cost":0,
                "time":{"created":yesterday_ms},
                "tokens":{"input":5,"output":2,"reasoning":1,"cache":{"read":0}}
            })
            .to_string(),
        );
        insert(
            "user",
            today_ms,
            &serde_json::json!({"role":"user","providerID":"opencode","cost":99}).to_string(),
        );
        insert(
            "other-provider",
            today_ms,
            &serde_json::json!({"role":"assistant","providerID":"openrouter","cost":99})
                .to_string(),
        );
        insert("malformed", today_ms, "not-json");
        drop(connection);

        let zen = OpenCodeClient::new(crate::providers::instances::ProviderId::from(
            ProviderKind::OpenCodeZen,
        ))
        .unwrap();
        let zen_stats =
            statistics_from_daily(&zen.read_local_usage_from_path(&path).unwrap().daily, 30);
        assert_eq!(zen_stats.history.requests, 1);
        assert_eq!(zen_stats.today.estimated_cost_microusd, 1_250_000);
        assert_eq!(zen_stats.today.input_tokens, 10);
        assert_eq!(zen_stats.today.output_tokens, 7);
        assert_eq!(zen_stats.today.cached_input_tokens, 2);

        let go = OpenCodeClient::new(crate::providers::instances::ProviderId::from(
            ProviderKind::OpenCodeGo,
        ))
        .unwrap();
        let go_stats =
            statistics_from_daily(&go.read_local_usage_from_path(&path).unwrap().daily, 30);
        assert_eq!(go_stats.history.requests, 1);
        assert_eq!(go_stats.history.estimated_cost_microusd, 0);
    }

    #[test]
    fn go_usage_normalizes_offsets_clamps_percent_and_ignores_null_optional_windows() {
        let now = Utc.with_ymd_and_hms(2026, 11, 1, 5, 0, 0).unwrap();
        let limits = parse_go_usage(
            r#"{"data":{"usage":{
            "rolling":{"percent":101.5,"resetsAt":"2026-11-01T01:30:00-04:00","resetInSec":1},
            "weekly":null, "monthly":{"percent":null}, "future":{"percent":99}
        }}}"#,
            now,
        )
        .unwrap();
        assert_eq!(limits.primary.used_percent, Some(100));
        assert_eq!(
            limits.primary.resets_at,
            Some(now + chrono::Duration::minutes(30))
        );
        assert_eq!(limits.secondary, LimitWindow::default());
        assert!(limits.additional_limits.is_empty());
        assert!(parse_go_usage(r#"{"usage":{"rolling":{"percent":null}}}"#, now).is_err());
        assert!(parse_model_count(r#"{"data":null}"#).is_err());
    }
}
