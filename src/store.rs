//! SQLite-backed provider store for limits and usage caches.
//!
//! Hot UI path still reads `AppState` in memory. Workers and startup hydrate
//! from this WAL database — the only on-disk persistence for provider data.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Duration, Local, NaiveDate, SecondsFormat, TimeZone, Timelike, Utc};
use directories::ProjectDirs;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::limits::{ProviderLimits, RateLimits};
use crate::usage::{
    CachedClaudeSessionFile, CachedClaudeUsageEntry, CachedSessionFile, ClaudeUsageCache,
    DailyTokenUsage, TokenUsage, UsageCache, UsageStatistics, statistics_from_daily,
};
use crate::{instances::ProviderId, settings::ProviderKind};

pub(crate) mod codex_accounts;
pub(crate) mod repricing;

const SCHEMA_VERSION: i64 = 1;
const CODEX_CACHE_VERSION: u8 = crate::usage::CODEX_CACHE_VERSION;
const CLAUDE_CACHE_VERSION: u8 = 4;
const CURSOR_USAGE_VERSION: u8 = 8;
const CACHE_RETENTION_DAYS: i64 = 365;

static SHARED: OnceLock<Arc<Mutex<ProviderStore>>> = OnceLock::new();

/// Process-wide store. Opened once, shared by startup hydration and workers.
pub fn shared() -> Result<Arc<Mutex<ProviderStore>>> {
    if let Some(store) = SHARED.get() {
        return Ok(Arc::clone(store));
    }
    static OPEN: Mutex<()> = Mutex::new(());
    let _open = OPEN
        .lock()
        .map_err(|_| anyhow!("provider store initialization lock poisoned"))?;
    if let Some(store) = SHARED.get() {
        return Ok(Arc::clone(store));
    }
    let store = Arc::new(Mutex::new(ProviderStore::open()?));
    let _ = SHARED.set(Arc::clone(&store));
    Ok(SHARED.get().map(Arc::clone).unwrap_or(store))
}

pub struct ProviderStore {
    conn: Connection,
}

impl ProviderStore {
    pub fn open() -> Result<Self> {
        let path = store_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create provider store dir {}", parent.display()))?;
        }
        let conn = Connection::open(&path)
            .with_context(|| format!("open provider store at {}", path.display()))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;
             PRAGMA journal_size_limit=8388608;",
        )
        .context("configure sqlite")?;
        let store = Self { conn };
        // Shrink a WAL left large by earlier sessions before anything else.
        store.checkpoint_wal()?;
        store.migrate()?;
        if let Err(error) = store.prune_hourly() {
            eprintln!("failed to prune hourly usage: {error:#}");
        }
        store.migrate_legacy_json_caches();
        store.initialize_codex_attribution()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS limits (
                provider TEXT PRIMARY KEY NOT NULL,
                fetched_at TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                stale INTEGER NOT NULL DEFAULT 1
            );
            CREATE TABLE IF NOT EXISTS usage_daily (
                provider TEXT NOT NULL,
                date TEXT NOT NULL,
                input_tokens INTEGER NOT NULL,
                cached_input_tokens INTEGER NOT NULL,
                output_tokens INTEGER NOT NULL,
                requests INTEGER NOT NULL,
                estimated_cost_microusd INTEGER NOT NULL,
                priced_requests INTEGER NOT NULL,
                PRIMARY KEY (provider, date)
            );
            CREATE TABLE IF NOT EXISTS scan_files (
                provider TEXT NOT NULL,
                path TEXT NOT NULL,
                offset INTEGER NOT NULL,
                meta_json TEXT NOT NULL DEFAULT '{}',
                PRIMARY KEY (provider, path)
            );
            CREATE TABLE IF NOT EXISTS usage_file_daily (
                provider TEXT NOT NULL,
                path TEXT NOT NULL,
                date TEXT NOT NULL,
                input_tokens INTEGER NOT NULL,
                cached_input_tokens INTEGER NOT NULL,
                output_tokens INTEGER NOT NULL,
                requests INTEGER NOT NULL,
                estimated_cost_microusd INTEGER NOT NULL,
                priced_requests INTEGER NOT NULL,
                PRIMARY KEY (provider, path, date)
            );
            CREATE TABLE IF NOT EXISTS usage_events (
                provider TEXT NOT NULL,
                path TEXT NOT NULL,
                event_ord INTEGER NOT NULL,
                ts TEXT NOT NULL,
                message_id TEXT,
                request_id TEXT,
                is_sidechain INTEGER NOT NULL,
                has_speed INTEGER NOT NULL,
                input_tokens INTEGER NOT NULL,
                cached_input_tokens INTEGER NOT NULL,
                output_tokens INTEGER NOT NULL,
                requests INTEGER NOT NULL,
                estimated_cost_microusd INTEGER NOT NULL,
                priced_requests INTEGER NOT NULL,
                PRIMARY KEY (provider, path, event_ord)
            );
            CREATE TABLE IF NOT EXISTS provider_meta (
                provider TEXT PRIMARY KEY NOT NULL,
                usage_fetched_at TEXT,
                schema_version INTEGER NOT NULL DEFAULT 1,
                flags_json TEXT NOT NULL DEFAULT '{}'
            );
            CREATE TABLE IF NOT EXISTS pricing_catalog (
                source TEXT PRIMARY KEY NOT NULL,
                fetched_at TEXT NOT NULL,
                payload_json TEXT NOT NULL
            );
            ",
        )?;
        self.migrate_schema_updates()?;
        self.migrate_codex_accounts()?;
        self.set_meta("schema_version", &SCHEMA_VERSION.to_string())?;
        Ok(())
    }

    fn migrate_schema_updates(&self) -> Result<()> {
        self.ensure_column(
            "usage_daily",
            "cache_savings_microusd",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        self.ensure_column(
            "usage_file_daily",
            "cache_savings_microusd",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        self.ensure_column("usage_events", "model", "TEXT")?;
        self.ensure_column(
            "usage_events",
            "cache_savings_microusd",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS usage_model_daily (
                provider TEXT NOT NULL,
                date TEXT NOT NULL,
                model TEXT NOT NULL,
                input_tokens INTEGER NOT NULL,
                cached_input_tokens INTEGER NOT NULL,
                output_tokens INTEGER NOT NULL,
                requests INTEGER NOT NULL,
                estimated_cost_microusd INTEGER NOT NULL,
                priced_requests INTEGER NOT NULL,
                cache_savings_microusd INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (provider, date, model)
            );
            CREATE TABLE IF NOT EXISTS usage_hourly (
                provider TEXT NOT NULL,
                hour TEXT NOT NULL,
                input_tokens INTEGER NOT NULL,
                cached_input_tokens INTEGER NOT NULL,
                output_tokens INTEGER NOT NULL,
                requests INTEGER NOT NULL,
                estimated_cost_microusd INTEGER NOT NULL,
                priced_requests INTEGER NOT NULL,
                cache_savings_microusd INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (provider, hour)
            );
            CREATE TABLE IF NOT EXISTS usage_file_model_daily (
                provider TEXT NOT NULL,
                path TEXT NOT NULL,
                date TEXT NOT NULL,
                model TEXT NOT NULL,
                input_tokens INTEGER NOT NULL,
                cached_input_tokens INTEGER NOT NULL,
                output_tokens INTEGER NOT NULL,
                requests INTEGER NOT NULL,
                estimated_cost_microusd INTEGER NOT NULL,
                priced_requests INTEGER NOT NULL,
                cache_savings_microusd INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (provider, path, date, model)
            );
            ",
        )?;
        self.migrate_hourly_to_utc()?;
        self.conn.execute(
            "DELETE FROM meta WHERE key IN ('openrouter.analytics.v1', 'openrouter.analytics.v2')",
            [],
        )?;
        Ok(())
    }

    /// One-time rewrite of `usage_hourly.hour` from local-offset RFC3339 into
    /// canonical UTC (`...Z`) so lexical range compares survive DST/timezone
    /// changes. Rows that cannot be parsed are dropped.
    fn migrate_hourly_to_utc(&self) -> Result<()> {
        const KEY: &str = "usage_hourly.utc.v1";
        let done: Option<String> = self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", params![KEY], |r| {
                r.get(0)
            })
            .optional()?;
        if done.is_some() {
            return Ok(());
        }
        let tx = self.conn.unchecked_transaction()?;
        let rows = {
            let mut statement = tx.prepare("SELECT provider, hour FROM usage_hourly")?;
            let rows = statement.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        for (provider, hour) in rows {
            match DateTime::parse_from_rfc3339(&hour) {
                Ok(parsed) => {
                    let canonical = hour_key(parsed);
                    if canonical != hour {
                        tx.execute(
                            "UPDATE OR REPLACE usage_hourly SET hour = ?1
                             WHERE provider = ?2 AND hour = ?3",
                            params![canonical, provider, hour],
                        )?;
                    }
                }
                Err(_) => {
                    tx.execute(
                        "DELETE FROM usage_hourly WHERE provider = ?1 AND hour = ?2",
                        params![provider, hour],
                    )?;
                }
            }
        }
        tx.execute(
            "INSERT INTO meta(key, value) VALUES(?1, '1')
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![KEY],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Deletes hourly rows older than the retention window for all providers.
    fn prune_hourly(&self) -> Result<()> {
        self.conn.execute(
            "DELETE FROM usage_hourly WHERE hour < ?1",
            params![hourly_cutoff()],
        )?;
        Ok(())
    }

    /// Passive WAL checkpoint; call after large scans. Failures are logged,
    /// never fatal.
    pub(crate) fn checkpoint_wal(&self) -> Result<()> {
        if let Err(error) = self
            .conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
        {
            eprintln!("provider store wal checkpoint failed: {error}");
        }
        Ok(())
    }

    /// Removes rows belonging to provider instances that are no longer
    /// configured. `known` must list every configured instance, enabled or
    /// not. An empty list skips pruning.
    pub fn prune_unknown_providers(&self, known: &[ProviderId]) -> Result<()> {
        if known.is_empty() {
            return Ok(());
        }
        let tx = self.conn.unchecked_transaction()?;
        tx.execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS known_providers(id TEXT PRIMARY KEY NOT NULL);
             DELETE FROM known_providers;",
        )?;
        {
            let mut insert =
                tx.prepare("INSERT OR IGNORE INTO temp.known_providers(id) VALUES(?1)")?;
            for provider in known {
                insert.execute(params![provider.id()])?;
            }
        }
        for table in [
            "limits",
            "usage_daily",
            "usage_hourly",
            "usage_model_daily",
            "usage_file_daily",
            "usage_file_model_daily",
            "usage_events",
            "scan_files",
            "provider_meta",
        ] {
            tx.execute(
                &format!(
                    "DELETE FROM {table} WHERE provider NOT IN (SELECT id FROM temp.known_providers)"
                ),
                [],
            )?;
        }
        const PREFIX: &str = "openrouter.analytics.v3.";
        tx.execute(
            "DELETE FROM meta
             WHERE substr(key, 1, ?1) = ?2
               AND substr(key, ?1 + 1) NOT IN (SELECT id FROM temp.known_providers)",
            params![PREFIX.len() as i64, PREFIX],
        )?;
        tx.execute("DELETE FROM temp.known_providers", [])?;
        tx.commit()?;
        Ok(())
    }

    fn ensure_column(&self, table: &str, column: &str, definition: &str) -> Result<()> {
        let mut statement = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
        for name in rows {
            if name? == column {
                return Ok(());
            }
        }
        self.conn
            .execute(
                &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
                [],
            )
            .with_context(|| format!("add column {column} to {table}"))?;
        Ok(())
    }

    pub fn hydrate_provider_limits(
        &self,
        providers: &[ProviderId],
        history_days: u16,
    ) -> Result<ProviderLimits> {
        let mut limits = ProviderLimits::default();
        let mut legacy_accounts = Vec::new();
        for provider in providers {
            let mut snapshot = self.load_limits(*provider)?.unwrap_or_default();
            legacy_accounts.extend(std::mem::take(&mut snapshot.legacy_accounts));
            snapshot.usage = self.load_usage_daily(*provider, history_days)?;
            *limits.get_mut(*provider) = snapshot;
        }
        // Samples cached per account before accounts became instances seed
        // each migrated instance until its own first read.
        for account in legacy_accounts {
            let Some(provider) = providers
                .iter()
                .find(|provider| !provider.is_primary() && provider.id() == account.id)
            else {
                continue;
            };
            let snapshot = limits.get_mut(*provider);
            if snapshot.sampled_at.timestamp() <= 0 {
                let usage = std::mem::take(&mut snapshot.usage);
                *snapshot = account.limits;
                snapshot.usage = usage;
            }
        }
        Ok(limits)
    }

    pub(crate) fn load_pricing_catalog(&self, source: &str) -> Result<Option<(String, String)>> {
        self.conn
            .query_row(
                "SELECT fetched_at, payload_json FROM pricing_catalog WHERE source = ?1",
                params![source],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .context("load cached pricing catalog")
    }

    #[allow(dead_code)]
    pub(crate) fn save_pricing_catalog(
        &self,
        source: &str,
        fetched_at: DateTime<Utc>,
        payload: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO pricing_catalog(source, fetched_at, payload_json)
             VALUES(?1, ?2, ?3)
             ON CONFLICT(source) DO UPDATE SET
                fetched_at=excluded.fetched_at,
                payload_json=excluded.payload_json",
            params![source, fetched_at.to_rfc3339(), payload],
        )?;
        Ok(())
    }

    pub fn load_limits(&self, provider: ProviderId) -> Result<Option<RateLimits>> {
        let mut statement = self
            .conn
            .prepare("SELECT payload_json FROM limits WHERE provider = ?1")?;
        let payload: Option<String> = statement
            .query_row(params![provider.id()], |row| row.get(0))
            .optional()?;
        let Some(payload) = payload else {
            return Ok(None);
        };
        let mut limits: RateLimits =
            serde_json::from_str(&payload).context("parse persisted rate limits")?;
        // Usage lives in usage_daily; avoid stale nested copies.
        limits.usage = UsageStatistics::default();
        Ok(Some(limits))
    }

    pub fn save_limits(&self, provider: ProviderId, limits: &RateLimits) -> Result<()> {
        let mut persisted = limits.clone();
        persisted.usage = UsageStatistics::default();
        let payload = serde_json::to_string(&persisted).context("serialize rate limits")?;
        let fetched_at = limits.sampled_at.to_rfc3339();
        self.conn.execute(
            "INSERT INTO limits(provider, fetched_at, payload_json, stale)
             VALUES(?1, ?2, ?3, 0)
             ON CONFLICT(provider) DO UPDATE SET
                fetched_at=excluded.fetched_at,
                payload_json=excluded.payload_json,
                stale=0",
            params![provider.id(), fetched_at, payload],
        )?;
        Ok(())
    }

    pub fn load_usage_daily(
        &self,
        provider: ProviderId,
        history_days: u16,
    ) -> Result<UsageStatistics> {
        if provider.kind() == ProviderKind::OpenRouter
            && !self.has_openrouter_analytics(provider)?
        {
            return Ok(UsageStatistics::default());
        }
        if provider == ProviderId::primary(ProviderKind::Codex) {
            if let Some(account) = self.codex_account_for_reads()? {
                return self.account_statistics_for(&account, history_days);
            }
        }
        let mut statement = self.conn.prepare(
            "SELECT date, input_tokens, cached_input_tokens, output_tokens,
                    requests, estimated_cost_microusd, priced_requests, cache_savings_microusd
             FROM usage_daily
             WHERE provider = ?1
             ORDER BY date ASC",
        )?;
        let rows = statement.query_map(params![provider.id()], |row| {
            let date_str = row.get::<_, String>(0)?;
            let usage = token_usage_from_row(row, 1)?;
            Ok((date_str, usage))
        })?;
        let mut daily = Vec::new();
        for row in rows {
            let (date_str, usage) = row?;
            if let Some(date) = parse_date_option(&date_str) {
                daily.push(DailyTokenUsage { date, usage });
            } else {
                eprintln!("Skipping usage_daily row with malformed date: {}", date_str);
            }
        }
        Ok(statistics_from_daily(&daily, history_days))
    }

    /// Deletes all locally derived usage data while preserving provider
    /// credentials, quota snapshots, and the cached pricing catalog.
    pub fn clear_usage_data(&self) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for table in [
            "usage_daily",
            "usage_hourly",
            "usage_model_daily",
            "usage_file_daily",
            "usage_file_model_daily",
            "usage_events",
            "scan_files",
            "codex_events",
            "codex_event_links",
            "codex_names",
            "codex_legacy_daily",
            "codex_legacy_model_daily",
            "codex_legacy_hourly",
            "codex_legacy_sessions",
            "codex_legacy_cursors",
        ] {
            tx.execute(&format!("DELETE FROM {table}"), [])?;
        }
        tx.execute(
            "DELETE FROM meta WHERE key LIKE 'openrouter.analytics.%'",
            [],
        )?;
        // Rebuild existing attribution after a clear; never reassign history.
        if let Some(mut state) = self.codex_attribution()? {
            state.ready = false;
            tx.execute(
                "UPDATE meta SET value=?1 WHERE key='codex.accounts.v1'",
                [serde_json::to_string(&state)?],
            )?;
        }
        tx.execute("UPDATE provider_meta SET usage_fetched_at = NULL", [])?;
        tx.commit()?;
        Ok(())
    }

    /// Hourly usage in `[start, end]`, local hours. Claude prefers `usage_events`
    /// timestamps; Codex/OpenCode read the hourly table filled on refresh.
    pub fn load_usage_hourly(
        &self,
        provider: ProviderId,
        start: DateTime<Local>,
        end: DateTime<Local>,
    ) -> Result<BTreeMap<DateTime<Local>, TokenUsage>> {
        if provider == ProviderId::primary(ProviderKind::Codex) {
            if let Some(account) = self.codex_account_for_reads()? {
                return self.account_hourly_for(&account, start, end);
            }
        }
        if provider.kind() == ProviderKind::OpenRouter {
            let mut hours = BTreeMap::<DateTime<Local>, TokenUsage>::new();
            for (_, at, usage) in self.load_openrouter_hourly_rows(provider, start, end)? {
                hours.entry(at).or_default().add(&usage);
            }
            return Ok(hours);
        }
        let from_events = self.load_event_hourly(provider, start, end)?;
        if !from_events.is_empty() {
            return Ok(from_events);
        }
        let mut statement = self.conn.prepare(
            "SELECT hour, input_tokens, cached_input_tokens, output_tokens,
                    requests, estimated_cost_microusd, priced_requests, cache_savings_microusd
             FROM usage_hourly
             WHERE provider = ?1 AND hour >= ?2 AND hour <= ?3
             ORDER BY hour ASC",
        )?;
        let start_key = hour_key(start);
        let end_key = hour_key(end);
        let rows = statement.query_map(params![provider.id(), start_key, end_key], |row| {
            let hour_str = row.get::<_, String>(0)?;
            let usage = token_usage_from_row(row, 1)?;
            Ok((hour_str, usage))
        })?;
        let mut hourly = BTreeMap::<DateTime<Local>, TokenUsage>::new();
        for row in rows {
            let (hour_str, usage) = row?;
            if let Some(dt) = parse_datetime_option(&hour_str) {
                let hour = dt.with_timezone(&Local);
                hourly.entry(truncate_local_hour(hour)).or_default().add(&usage);
            } else {
                eprintln!("Skipping usage_hourly row with malformed hour: {}", hour_str);
            }
        }
        Ok(hourly)
    }

    fn load_event_hourly(
        &self,
        provider: ProviderId,
        start: DateTime<Local>,
        end: DateTime<Local>,
    ) -> Result<BTreeMap<DateTime<Local>, TokenUsage>> {
        let mut statement = self.conn.prepare(
            "SELECT ts, input_tokens, cached_input_tokens, output_tokens,
                    requests, estimated_cost_microusd, priced_requests, cache_savings_microusd
             FROM usage_events
             WHERE provider = ?1 AND ts >= ?2 AND ts <= ?3",
        )?;
        let rows = statement.query_map(
            params![
                provider.id(),
                start.with_timezone(&Utc).to_rfc3339(),
                end.with_timezone(&Utc).to_rfc3339()
            ],
            |row| {
                let ts_str = row.get::<_, String>(0)?;
                let usage = token_usage_from_row(row, 1)?;
                Ok((ts_str, usage))
            },
        )?;
        let mut hourly = BTreeMap::<DateTime<Local>, TokenUsage>::new();
        for row in rows {
            let (ts_str, usage) = row?;
            if let Some(timestamp) = parse_datetime_option(&ts_str) {
                let local = timestamp.with_timezone(&Local);
                if local < start || local > end {
                    continue;
                }
                hourly
                    .entry(truncate_local_hour(local))
                    .or_default()
                    .add(&usage);
            } else {
                eprintln!("Skipping usage_events row with malformed timestamp: {}", ts_str);
            }
        }
        Ok(hourly)
    }

    pub fn replace_usage_hourly(
        &self,
        provider: ProviderId,
        hours: &[(DateTime<Local>, TokenUsage)],
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let cutoff = hourly_cutoff();
        // Retention applies to every provider, not just the one being written.
        tx.execute("DELETE FROM usage_hourly WHERE hour < ?1", params![cutoff])?;
        {
            let mut insert = tx.prepare(
                "INSERT INTO usage_hourly(
                    provider, hour, input_tokens, cached_input_tokens, output_tokens,
                    requests, estimated_cost_microusd, priced_requests, cache_savings_microusd
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(provider, hour) DO UPDATE SET
                    input_tokens=excluded.input_tokens,
                    cached_input_tokens=excluded.cached_input_tokens,
                    output_tokens=excluded.output_tokens,
                    requests=excluded.requests,
                    estimated_cost_microusd=excluded.estimated_cost_microusd,
                    priced_requests=excluded.priced_requests,
                    cache_savings_microusd=excluded.cache_savings_microusd",
            )?;
            for (hour, usage) in hours {
                let hour_key = hour_key(*hour);
                if hour_key < cutoff {
                    continue;
                }
                insert.execute(params![
                    provider.id(),
                    hour_key,
                    usage.input_tokens as i64,
                    usage.cached_input_tokens as i64,
                    usage.output_tokens as i64,
                    usage.requests as i64,
                    usage.estimated_cost_microusd as i64,
                    usage.priced_requests as i64,
                    usage.cache_savings_microusd as i64,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn replace_usage_daily(
        &self,
        provider: ProviderId,
        days: &[DailyTokenUsage],
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        self.replace_usage_daily_in_transaction(provider, days)?;
        tx.commit()?;
        Ok(())
    }

    fn replace_usage_daily_in_transaction(
        &self,
        provider: ProviderId,
        days: &[DailyTokenUsage],
    ) -> Result<()> {
        let mut retained = BTreeSet::new();
        {
            let mut insert = self.conn.prepare(
                "INSERT INTO usage_daily(
                    provider, date, input_tokens, cached_input_tokens, output_tokens,
                    requests, estimated_cost_microusd, priced_requests, cache_savings_microusd
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(provider, date) DO UPDATE SET
                    input_tokens=excluded.input_tokens,
                    cached_input_tokens=excluded.cached_input_tokens,
                    output_tokens=excluded.output_tokens,
                    requests=excluded.requests,
                    estimated_cost_microusd=excluded.estimated_cost_microusd,
                    priced_requests=excluded.priced_requests,
                    cache_savings_microusd=excluded.cache_savings_microusd
                 WHERE input_tokens IS NOT excluded.input_tokens
                    OR cached_input_tokens IS NOT excluded.cached_input_tokens
                    OR output_tokens IS NOT excluded.output_tokens
                    OR requests IS NOT excluded.requests
                    OR estimated_cost_microusd IS NOT excluded.estimated_cost_microusd
                    OR priced_requests IS NOT excluded.priced_requests
                    OR cache_savings_microusd IS NOT excluded.cache_savings_microusd",
            )?;
            for entry in days {
                retained.insert(entry.date.to_string());
                insert.execute(params![
                    provider.id(),
                    entry.date.to_string(),
                    entry.usage.input_tokens as i64,
                    entry.usage.cached_input_tokens as i64,
                    entry.usage.output_tokens as i64,
                    entry.usage.requests as i64,
                    entry.usage.estimated_cost_microusd as i64,
                    entry.usage.priced_requests as i64,
                    entry.usage.cache_savings_microusd as i64,
                ])?;
            }
        }
        self.conn.execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS retained_daily(date TEXT PRIMARY KEY NOT NULL);
             DELETE FROM retained_daily;",
        )?;
        {
            let mut keep = self
                .conn
                .prepare("INSERT OR IGNORE INTO temp.retained_daily(date) VALUES(?1)")?;
            for date in &retained {
                keep.execute(params![date])?;
            }
        }
        self.conn.execute(
            "DELETE FROM usage_daily
             WHERE provider = ?1 AND date NOT IN (SELECT date FROM temp.retained_daily)",
            params![provider.id()],
        )?;
        self.conn.execute("DELETE FROM temp.retained_daily", [])?;
        Ok(())
    }

    pub fn usage_fetched_at(&self, provider: ProviderId) -> Result<Option<DateTime<Utc>>> {
        let mut statement = self
            .conn
            .prepare("SELECT usage_fetched_at FROM provider_meta WHERE provider = ?1")?;
        let value: Option<Option<String>> = statement
            .query_row(params![provider.id()], |row| row.get(0))
            .optional()?;
        Ok(value
            .flatten()
            .and_then(|raw| DateTime::parse_from_rfc3339(&raw).ok())
            .map(|dt| dt.with_timezone(&Utc)))
    }

    pub fn set_usage_fetched_at(
        &self,
        provider: ProviderId,
        fetched_at: DateTime<Utc>,
    ) -> Result<()> {
        self.upsert_provider_meta(
            provider,
            Some(fetched_at),
            CURSOR_USAGE_VERSION as i64,
            None,
        )
    }

    pub fn cursor_usage_version(&self, provider: ProviderId) -> Result<u8> {
        let mut statement = self
            .conn
            .prepare("SELECT schema_version FROM provider_meta WHERE provider = ?1")?;
        let version: Option<i64> = statement
            .query_row(params![provider.id()], |row| row.get(0))
            .optional()?;
        Ok(version.unwrap_or(0) as u8)
    }

    pub(crate) fn load_codex_cache(&self, provider: ProviderId) -> Result<UsageCache> {
        let flags = self.provider_flags(provider)?;
        let mut pricing_rebuild_needed = flags
            .get("pricing_rebuild_needed")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let version = flags
            .get("cache_version")
            .and_then(|value| value.as_u64())
            .unwrap_or(u64::from(CODEX_CACHE_VERSION)) as u8;
        if version != CODEX_CACHE_VERSION {
            pricing_rebuild_needed = true;
        }

        let mut files = BTreeMap::new();
        {
            let mut statement = self
                .conn
                .prepare("SELECT path, offset, meta_json FROM scan_files WHERE provider = ?1")?;
            let rows = statement.query_map(params![provider.id()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)? as u64,
                    row.get::<_, String>(2)?,
                ))
            })?;
            for row in rows {
                let (path, offset, meta_json) = row?;
                let meta: CodexFileMeta = serde_json::from_str(&meta_json).unwrap_or_default();
                files.insert(
                    path,
                    CachedSessionFile {
                        offset,
                        daily: Vec::new(),
                        current_model: meta.current_model,
                        fast_service_tier: meta.fast_service_tier,
                        model_daily: BTreeMap::new(),
                        last_usage_signature: meta.last_usage_signature,
                        saw_session_meta: meta.saw_session_meta,
                        suppressing_fork_copies: meta.suppressing_fork_copies,
                        fork_copy_anchor_ms: meta.fork_copy_anchor_ms,
                        session_id: meta.session_id,
                        persisted: true,
                    },
                );
            }
        }
        {
            let mut statement = self.conn.prepare(
                "SELECT path, date, input_tokens, cached_input_tokens, output_tokens,
                        requests, estimated_cost_microusd, priced_requests, cache_savings_microusd
                 FROM usage_file_daily WHERE provider = ?1",
            )?;
            let rows = statement.query_map(params![provider.id()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    token_usage_from_row(row, 2)?,
                ))
            })?;
            for row in rows {
                let (path, date_str, usage) = row?;
                if let Some(date) = parse_date_option(&date_str) {
                    let file = files.entry(path).or_default();
                    file.daily.push(DailyTokenUsage { date, usage });
                } else {
                    eprintln!("Skipping usage_file_daily row with malformed date: {}", date_str);
                }
            }
        }
        {
            let mut statement = self.conn.prepare(
                "SELECT path, date, model, input_tokens, cached_input_tokens, output_tokens,
                        requests, estimated_cost_microusd, priced_requests, cache_savings_microusd
                 FROM usage_file_model_daily WHERE provider = ?1",
            )?;
            let rows = statement.query_map(params![provider.id()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    token_usage_from_row(row, 3)?,
                ))
            })?;
            for row in rows {
                let (path, date_str, model, usage) = row?;
                if let Some(date) = parse_date_option(&date_str) {
                    let file = files.entry(path).or_default();
                    file.model_daily
                        .entry(model)
                        .or_default()
                        .push(DailyTokenUsage { date, usage });
                } else {
                    eprintln!("Skipping usage_file_model_daily row with malformed date: {}", date_str);
                }
            }
        }
        for file in files.values_mut() {
            file.daily.sort_by_key(|entry| entry.date);
            for days in file.model_daily.values_mut() {
                days.sort_by_key(|entry| entry.date);
            }
        }
        Ok(UsageCache {
            version,
            pricing_rebuild_needed,
            files,
        })
    }

    pub(crate) fn save_codex_cache(&self, provider: ProviderId, cache: &UsageCache) -> Result<()> {
        // Files whose rows already equal the cache are never rewritten; only
        // dirty files are replaced and files that left the cache are deleted.
        let stale = self.stale_codex_paths(provider, cache)?;
        let dirty = cache.files.values().filter(|file| !file.persisted).count();
        if dirty > 0 || !stale.is_empty() {
            let tx = self.conn.unchecked_transaction()?;
            {
                let mut scan = tx.prepare(
                    "INSERT INTO scan_files(provider, path, offset, meta_json)
                     VALUES(?1, ?2, ?3, ?4)
                     ON CONFLICT(provider, path) DO UPDATE SET
                        offset=excluded.offset,
                        meta_json=excluded.meta_json",
                )?;
                let mut clear_daily = tx
                    .prepare("DELETE FROM usage_file_daily WHERE provider = ?1 AND path = ?2")?;
                let mut clear_models = tx.prepare(
                    "DELETE FROM usage_file_model_daily WHERE provider = ?1 AND path = ?2",
                )?;
                let mut daily = tx.prepare(
                    "INSERT INTO usage_file_daily(
                        provider, path, date, input_tokens, cached_input_tokens, output_tokens,
                        requests, estimated_cost_microusd, priced_requests, cache_savings_microusd
                     ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                )?;
                let mut models = tx.prepare(
                    "INSERT INTO usage_file_model_daily(
                        provider, path, date, model, input_tokens, cached_input_tokens,
                        output_tokens, requests, estimated_cost_microusd, priced_requests,
                        cache_savings_microusd
                     ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                )?;
                for (path, file) in cache.files.iter().filter(|(_, file)| !file.persisted) {
                    let meta = serde_json::to_string(&CodexFileMeta {
                        current_model: file.current_model.clone(),
                        fast_service_tier: file.fast_service_tier,
                        last_usage_signature: file.last_usage_signature.clone(),
                        saw_session_meta: file.saw_session_meta,
                        suppressing_fork_copies: file.suppressing_fork_copies,
                        fork_copy_anchor_ms: file.fork_copy_anchor_ms,
                        session_id: file.session_id.clone(),
                    })?;
                    scan.execute(params![provider.id(), path, file.offset as i64, meta])?;
                    clear_daily.execute(params![provider.id(), path])?;
                    clear_models.execute(params![provider.id(), path])?;
                    for entry in &file.daily {
                        daily.execute(params![
                            provider.id(),
                            path,
                            entry.date.to_string(),
                            entry.usage.input_tokens as i64,
                            entry.usage.cached_input_tokens as i64,
                            entry.usage.output_tokens as i64,
                            entry.usage.requests as i64,
                            entry.usage.estimated_cost_microusd as i64,
                            entry.usage.priced_requests as i64,
                            entry.usage.cache_savings_microusd as i64,
                        ])?;
                    }
                    for (model, days) in &file.model_daily {
                        for entry in days {
                            models.execute(params![
                                provider.id(),
                                path,
                                entry.date.to_string(),
                                model,
                                entry.usage.input_tokens as i64,
                                entry.usage.cached_input_tokens as i64,
                                entry.usage.output_tokens as i64,
                                entry.usage.requests as i64,
                                entry.usage.estimated_cost_microusd as i64,
                                entry.usage.priced_requests as i64,
                                entry.usage.cache_savings_microusd as i64,
                            ])?;
                        }
                    }
                }
            }
            delete_stale_file_rows(&tx, provider, &stale)?;
            tx.commit()?;
            if dirty + stale.len() >= 100 {
                self.checkpoint_wal()?;
            }
        }

        let flags = json!({
            "cache_version": cache.version,
            "pricing_rebuild_needed": cache.pricing_rebuild_needed,
        });
        self.upsert_provider_meta(
            provider,
            None,
            i64::from(cache.version),
            Some(flags.to_string()),
        )?;
        self.replace_usage_daily(
            provider,
            &aggregate_codex_daily(cache, CACHE_RETENTION_DAYS as u16),
        )?;
        self.replace_usage_model_daily(provider, &aggregate_codex_model_daily(cache))?;
        Ok(())
    }

    /// Paths with stored per-file Codex rows that are no longer in `cache`.
    fn stale_codex_paths(&self, provider: ProviderId, cache: &UsageCache) -> Result<Vec<String>> {
        let mut statement = self.conn.prepare(
            "SELECT path FROM scan_files WHERE provider = ?1
             UNION SELECT path FROM usage_file_daily WHERE provider = ?1
             UNION SELECT path FROM usage_file_model_daily WHERE provider = ?1",
        )?;
        let rows = statement.query_map(params![provider.id()], |row| row.get::<_, String>(0))?;
        let mut stale = Vec::new();
        for path in rows {
            let path = path?;
            if !cache.files.contains_key(&path) {
                stale.push(path);
            }
        }
        Ok(stale)
    }

    /// Scanned byte offset per Claude log, without loading any events. `None`
    /// when the stored cache predates the current version and needs a rebuild.
    pub(crate) fn load_claude_scan_offsets(
        &self,
        provider: ProviderId,
    ) -> Result<Option<BTreeMap<String, u64>>> {
        let flags = self.provider_flags(provider)?;
        let version = flags
            .get("cache_version")
            .and_then(|value| value.as_u64())
            .unwrap_or(u64::from(CLAUDE_CACHE_VERSION)) as u8;
        if version != CLAUDE_CACHE_VERSION {
            return Ok(None);
        }
        let mut statement = self
            .conn
            .prepare("SELECT path, offset FROM scan_files WHERE provider = ?1")?;
        let rows = statement.query_map(params![provider.id()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
        })?;
        Ok(Some(rows.collect::<rusqlite::Result<_>>()?))
    }

    pub(crate) fn load_claude_cache(&self, provider: ProviderId) -> Result<ClaudeUsageCache> {
        let flags = self.provider_flags(provider)?;
        let version = flags
            .get("cache_version")
            .and_then(|value| value.as_u64())
            .unwrap_or(u64::from(CLAUDE_CACHE_VERSION)) as u8;
        if version != CLAUDE_CACHE_VERSION {
            return Ok(ClaudeUsageCache::default());
        }

        let mut files = BTreeMap::new();
        let mut scanned = BTreeSet::new();
        {
            let mut statement = self
                .conn
                .prepare("SELECT path, offset FROM scan_files WHERE provider = ?1")?;
            let rows = statement.query_map(params![provider.id()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
            })?;
            for row in rows {
                let (path, offset) = row?;
                scanned.insert(path.clone());
                files.insert(
                    path,
                    CachedClaudeSessionFile {
                        offset,
                        entries: Vec::new(),
                        persisted: None,
                    },
                );
            }
        }
        {
            let mut statement = self.conn.prepare(
                "SELECT path, ts, message_id, request_id, is_sidechain, has_speed,
                        input_tokens, cached_input_tokens, output_tokens,
                        requests, estimated_cost_microusd, priced_requests,
                        cache_savings_microusd, model
                 FROM usage_events
                 WHERE provider = ?1
                 ORDER BY path ASC, event_ord ASC",
            )?;
            let rows = statement.query_map(params![provider.id()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    token_usage_from_row(row, 6)?,
                    row.get(13)?,
                ))
            })?;
            for row in rows {
                let (path, ts_str, message_id, request_id, is_sidechain_i64, has_speed_i64, usage, model) = row?;
                if let Some(timestamp) = parse_datetime_option(&ts_str) {
                    let entry = CachedClaudeUsageEntry {
                        timestamp,
                        message_id,
                        request_id,
                        is_sidechain: is_sidechain_i64 != 0,
                        has_speed: has_speed_i64 != 0,
                        usage,
                        model,
                    };
                    files.entry(path).or_default().entries.push(entry);
                } else {
                    eprintln!("Skipping usage_events row with malformed timestamp: {}", ts_str);
                }
            }
        }
        // Only files with a stored scan row are trusted to match the DB;
        // anything else (orphan events) is rewritten in full on the next save.
        for (path, file) in &mut files {
            if scanned.contains(path) {
                file.persisted = Some((file.offset, file.entries.len()));
            }
        }
        Ok(ClaudeUsageCache { version, files })
    }

    pub(crate) fn save_claude_cache(
        &self,
        provider: ProviderId,
        cache: &ClaudeUsageCache,
    ) -> Result<()> {
        // A file is clean when its stored offset and event count match; then
        // only events appended past the stored count are written. Anything
        // else (new, rebuilt, pruned) is rewritten and its tail truncated.
        let pending = |file: &CachedClaudeSessionFile| {
            file.persisted != Some((file.offset, file.entries.len()))
        };
        let stale = self.stale_claude_paths(provider, cache)?;
        if !stale.is_empty() || cache.files.values().any(pending) {
            let tx = self.conn.unchecked_transaction()?;
            {
                let mut scan = tx.prepare(
                    "INSERT INTO scan_files(provider, path, offset, meta_json)
                     VALUES(?1, ?2, ?3, '{}')
                     ON CONFLICT(provider, path) DO UPDATE SET offset=excluded.offset
                     WHERE offset IS NOT excluded.offset",
                )?;
                let mut events = tx.prepare(
                    "INSERT INTO usage_events(
                        provider, path, event_ord, ts, message_id, request_id,
                        is_sidechain, has_speed, input_tokens, cached_input_tokens,
                        output_tokens, requests, estimated_cost_microusd, priced_requests,
                        cache_savings_microusd, model
                     ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
                     ON CONFLICT(provider, path, event_ord) DO UPDATE SET
                        ts=excluded.ts,
                        message_id=excluded.message_id,
                        request_id=excluded.request_id,
                        is_sidechain=excluded.is_sidechain,
                        has_speed=excluded.has_speed,
                        input_tokens=excluded.input_tokens,
                        cached_input_tokens=excluded.cached_input_tokens,
                        output_tokens=excluded.output_tokens,
                        requests=excluded.requests,
                        estimated_cost_microusd=excluded.estimated_cost_microusd,
                        priced_requests=excluded.priced_requests,
                        cache_savings_microusd=excluded.cache_savings_microusd,
                        model=excluded.model
                     WHERE ts IS NOT excluded.ts
                        OR message_id IS NOT excluded.message_id
                        OR request_id IS NOT excluded.request_id
                        OR is_sidechain IS NOT excluded.is_sidechain
                        OR has_speed IS NOT excluded.has_speed
                        OR input_tokens IS NOT excluded.input_tokens
                        OR cached_input_tokens IS NOT excluded.cached_input_tokens
                        OR output_tokens IS NOT excluded.output_tokens
                        OR requests IS NOT excluded.requests
                        OR estimated_cost_microusd IS NOT excluded.estimated_cost_microusd
                        OR priced_requests IS NOT excluded.priced_requests
                        OR cache_savings_microusd IS NOT excluded.cache_savings_microusd
                        OR model IS NOT excluded.model",
                )?;
                for (path, file) in cache.files.iter().filter(|(_, file)| pending(file)) {
                    scan.execute(params![provider.id(), path, file.offset as i64])?;
                    let first_new = match file.persisted {
                        Some((_, stored)) if stored <= file.entries.len() => stored,
                        _ => 0,
                    };
                    for (event_ord, entry) in file.entries.iter().enumerate().skip(first_new) {
                        events.execute(params![
                            provider.id(),
                            path,
                            event_ord as i64,
                            entry.timestamp.to_rfc3339(),
                            entry.message_id,
                            entry.request_id,
                            entry.is_sidechain as i64,
                            entry.has_speed as i64,
                            entry.usage.input_tokens as i64,
                            entry.usage.cached_input_tokens as i64,
                            entry.usage.output_tokens as i64,
                            entry.usage.requests as i64,
                            entry.usage.estimated_cost_microusd as i64,
                            entry.usage.priced_requests as i64,
                            entry.usage.cache_savings_microusd as i64,
                            entry.model,
                        ])?;
                    }
                    if first_new == 0 {
                        // Rebuilt or pruned: drop ordinals past the new end.
                        tx.execute(
                            "DELETE FROM usage_events
                             WHERE provider = ?1 AND path = ?2 AND event_ord >= ?3",
                            params![provider.id(), path, file.entries.len() as i64],
                        )?;
                    }
                }
            }
            delete_stale_event_rows(&tx, provider, &stale)?;
            tx.commit()?;
            if stale.len() + cache.files.values().filter(|file| pending(file)).count() >= 100 {
                self.checkpoint_wal()?;
            }
        }

        let flags = json!({ "cache_version": cache.version });
        self.upsert_provider_meta(
            provider,
            None,
            i64::from(cache.version),
            Some(flags.to_string()),
        )?;
        Ok(())
    }

    /// Paths with stored scan or event rows that are no longer in `cache`.
    fn stale_claude_paths(
        &self,
        provider: ProviderId,
        cache: &ClaudeUsageCache,
    ) -> Result<Vec<String>> {
        let mut statement = self.conn.prepare(
            "SELECT path FROM scan_files WHERE provider = ?1
             UNION SELECT DISTINCT path FROM usage_events WHERE provider = ?1",
        )?;
        let rows = statement.query_map(params![provider.id()], |row| row.get::<_, String>(0))?;
        let mut stale = Vec::new();
        for path in rows {
            let path = path?;
            if !cache.files.contains_key(&path) {
                stale.push(path);
            }
        }
        Ok(stale)
    }

    fn provider_flags(&self, provider: ProviderId) -> Result<serde_json::Value> {
        let mut statement = self
            .conn
            .prepare("SELECT flags_json FROM provider_meta WHERE provider = ?1")?;
        let flags: Option<String> = statement
            .query_row(params![provider.id()], |row| row.get(0))
            .optional()?;
        Ok(flags
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_else(|| json!({})))
    }

    fn upsert_provider_meta(
        &self,
        provider: ProviderId,
        usage_fetched_at: Option<DateTime<Utc>>,
        schema_version: i64,
        flags_json: Option<String>,
    ) -> Result<()> {
        let existing_flags = self.provider_flags(provider)?;
        let flags = flags_json.unwrap_or_else(|| existing_flags.to_string());
        let fetched = usage_fetched_at.map(|at| at.to_rfc3339()).or_else(|| {
            self.usage_fetched_at(provider)
                .ok()
                .flatten()
                .map(|at| at.to_rfc3339())
        });
        self.conn.execute(
            "INSERT INTO provider_meta(provider, usage_fetched_at, schema_version, flags_json)
             VALUES(?1, ?2, ?3, ?4)
             ON CONFLICT(provider) DO UPDATE SET
                usage_fetched_at=COALESCE(excluded.usage_fetched_at, provider_meta.usage_fetched_at),
                schema_version=excluded.schema_version,
                flags_json=excluded.flags_json
             WHERE provider_meta.usage_fetched_at IS NOT COALESCE(excluded.usage_fetched_at, provider_meta.usage_fetched_at)
                OR provider_meta.schema_version IS NOT excluded.schema_version
                OR provider_meta.flags_json IS NOT excluded.flags_json",
            params![provider.id(), fetched, schema_version, flags],
        )?;
        Ok(())
    }

    pub(crate) fn load_openrouter_hourly_rows(
        &self,
        provider: ProviderId,
        start: DateTime<Local>,
        end: DateTime<Local>,
    ) -> Result<Vec<(String, DateTime<Local>, TokenUsage)>> {
        match self.load_openrouter_analytics(provider)? {
            Some(raw) => crate::openrouter::analytics::cached_hourly_rows(&raw, start, end),
            None => Ok(Vec::new()),
        }
    }

    pub(crate) fn load_openrouter_account_models(
        &self,
        provider: ProviderId,
        account: &str,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<(String, NaiveDate, TokenUsage)>> {
        match self.load_openrouter_analytics(provider)? {
            Some(raw) => {
                crate::openrouter::analytics::cached_account_models(&raw, account, start, end)
            }
            None => Ok(Vec::new()),
        }
    }

    pub(crate) fn load_openrouter_analytics(&self, provider: ProviderId) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                params![openrouter_analytics_key(provider)],
                |row| row.get(0),
            )
            .optional()?)
    }

    fn has_openrouter_analytics(&self, provider: ProviderId) -> Result<bool> {
        Ok(self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM meta WHERE key = ?1)",
                params![openrouter_analytics_key(provider)],
                |row| row.get(0),
            )?)
    }

    pub(crate) fn save_openrouter_analytics(
        &self,
        provider: ProviderId,
        cache: &str,
        daily: &[DailyTokenUsage],
        models: &[(String, NaiveDate, TokenUsage)],
        at: DateTime<Utc>,
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        self.replace_usage_daily_in_transaction(provider, daily)?;
        self.replace_usage_model_daily_in_transaction(provider, models)?;
        self.set_meta(&openrouter_analytics_key(provider), cache)?;
        self.set_usage_fetched_at(provider, at)?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn load_openrouter_key_directory(&self, account: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                params![format!("openrouter-key-directory:{account}")],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub(crate) fn save_openrouter_key_directory(&self, account: &str, value: &str) -> Result<()> {
        self.set_meta(&format!("openrouter-key-directory:{account}"), value)
    }

    fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value
             WHERE meta.value IS NOT excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn count_session_paths(
        &self,
        provider: ProviderId,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<u64> {
        if provider == ProviderId::primary(ProviderKind::Codex) {
            if let Some(account) = self.codex_account_for_reads()? {
                return self.account_sessions_for(&account, start, end);
            }
        }
        let from_files: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT path) FROM usage_file_daily
             WHERE provider = ?1 AND date >= ?2 AND date <= ?3",
            params![provider.id(), start.to_string(), end.to_string()],
            |row| row.get(0),
        )?;
        if from_files > 0 {
            return Ok(from_files as u64);
        }

        // Claude (and any other event-scanned provider) never writes
        // usage_file_daily — only usage_events + rolled-up usage_daily.
        // Counting files there always returned 0 while spend was real.
        let start_ts = start_of_local_day(start).with_timezone(&Utc).to_rfc3339();
        let end_ts = start_of_local_day(end + Duration::days(1))
            .with_timezone(&Utc)
            .to_rfc3339();
        let from_events: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT path) FROM usage_events
             WHERE provider = ?1 AND ts >= ?2 AND ts < ?3",
            params![provider.id(), start_ts, end_ts],
            |row| row.get(0),
        )?;
        Ok(from_events.max(0) as u64)
    }

    pub fn load_model_breakdown(
        &self,
        provider: ProviderId,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<(String, TokenUsage)>> {
        if provider.kind() == ProviderKind::OpenRouter
            && !self.has_openrouter_analytics(provider)?
        {
            return Ok(Vec::new());
        }
        if provider == ProviderId::primary(ProviderKind::Codex) {
            if let Some(account) = self.codex_account_for_reads()? {
                let mut merged = BTreeMap::<String, TokenUsage>::new();
                for (model, _, usage) in
                    self.account_daily_for(&account, start, end)?
                {
                    merged.entry(model).or_default().add(&usage);
                }
                return Ok(merged.into_iter().collect());
            }
        }
        let mut statement = self.conn.prepare(
            "SELECT model, input_tokens, cached_input_tokens, output_tokens,
                    requests, estimated_cost_microusd, priced_requests, cache_savings_microusd
             FROM usage_model_daily
             WHERE provider = ?1 AND date >= ?2 AND date <= ?3",
        )?;
        let rows = statement.query_map(
            params![provider.id(), start.to_string(), end.to_string()],
            |row| {
                let model: String = row.get(0)?;
                Ok((model, token_usage_from_row(row, 1)?))
            },
        )?;
        let mut merged = BTreeMap::<String, TokenUsage>::new();
        for row in rows {
            let (model, usage) = row?;
            merged.entry(model).or_default().add(&usage);
        }
        Ok(merged.into_iter().collect())
    }

    /// Date-preserving model rows for activity cards, using the same cached data
    /// as the Usage screen. Call off the UI thread; no provider requests.
    pub(crate) fn load_model_daily(
        &self,
        provider: ProviderId,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<(String, NaiveDate, TokenUsage)>> {
        if provider.kind() == ProviderKind::OpenRouter
            && !self.has_openrouter_analytics(provider)?
        {
            return Ok(Vec::new());
        }
        if provider == ProviderId::primary(ProviderKind::Codex) {
            if let Some(account) = self.codex_account_for_reads()? {
                return self.account_daily_for(&account, start, end);
            }
        }
        let mut statement = self.conn.prepare(
            "SELECT model, date, input_tokens, cached_input_tokens, output_tokens,
                    requests, estimated_cost_microusd, priced_requests, cache_savings_microusd
             FROM usage_model_daily
             WHERE provider = ?1 AND date >= ?2 AND date <= ?3
             ORDER BY date, model",
        )?;
        let rows = statement.query_map(
            params![provider.id(), start.to_string(), end.to_string()],
            |row| {
                let model: String = row.get(0)?;
                let date: String = row.get(1)?;
                Ok((model, date, token_usage_from_row(row, 2)?))
            },
        )?;
        rows.map(|row| {
            let (model, date, usage) = row?;
            Ok((model, NaiveDate::parse_from_str(&date, "%Y-%m-%d")?, usage))
        })
        .collect()
    }

    pub(crate) fn replace_usage_model_daily(
        &self,
        provider: ProviderId,
        rows: &[(String, NaiveDate, TokenUsage)],
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        self.replace_usage_model_daily_in_transaction(provider, rows)?;
        tx.commit()?;
        Ok(())
    }

    fn replace_usage_model_daily_in_transaction(
        &self,
        provider: ProviderId,
        rows: &[(String, NaiveDate, TokenUsage)],
    ) -> Result<()> {
        let mut retained = BTreeSet::new();
        {
            let mut insert = self.conn.prepare(
                "INSERT INTO usage_model_daily(
                    provider, date, model, input_tokens, cached_input_tokens, output_tokens,
                    requests, estimated_cost_microusd, priced_requests, cache_savings_microusd
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(provider, date, model) DO UPDATE SET
                    input_tokens=excluded.input_tokens,
                    cached_input_tokens=excluded.cached_input_tokens,
                    output_tokens=excluded.output_tokens,
                    requests=excluded.requests,
                    estimated_cost_microusd=excluded.estimated_cost_microusd,
                    priced_requests=excluded.priced_requests,
                    cache_savings_microusd=excluded.cache_savings_microusd
                 WHERE input_tokens IS NOT excluded.input_tokens
                    OR cached_input_tokens IS NOT excluded.cached_input_tokens
                    OR output_tokens IS NOT excluded.output_tokens
                    OR requests IS NOT excluded.requests
                    OR estimated_cost_microusd IS NOT excluded.estimated_cost_microusd
                    OR priced_requests IS NOT excluded.priced_requests
                    OR cache_savings_microusd IS NOT excluded.cache_savings_microusd",
            )?;
            for (model, date, usage) in rows {
                retained.insert((date.to_string(), model.clone()));
                insert.execute(params![
                    provider.id(),
                    date.to_string(),
                    model,
                    usage.input_tokens as i64,
                    usage.cached_input_tokens as i64,
                    usage.output_tokens as i64,
                    usage.requests as i64,
                    usage.estimated_cost_microusd as i64,
                    usage.priced_requests as i64,
                    usage.cache_savings_microusd as i64,
                ])?;
            }
        }
        self.conn.execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS retained_model_daily(
                date TEXT NOT NULL, model TEXT NOT NULL, PRIMARY KEY (date, model)
             );
             DELETE FROM retained_model_daily;",
        )?;
        {
            let mut keep = self.conn.prepare(
                "INSERT OR IGNORE INTO temp.retained_model_daily(date, model) VALUES(?1, ?2)",
            )?;
            for (date, model) in &retained {
                keep.execute(params![date, model])?;
            }
        }
        self.conn.execute(
            "DELETE FROM usage_model_daily
             WHERE provider = ?1 AND NOT EXISTS (
                SELECT 1 FROM temp.retained_model_daily r
                WHERE r.date = usage_model_daily.date AND r.model = usage_model_daily.model
             )",
            params![provider.id()],
        )?;
        self.conn
            .execute("DELETE FROM temp.retained_model_daily", [])?;
        Ok(())
    }

    /// Imports the previous JSON caches once so upgrading does not force a
    /// multi-gigabyte rescan of local session logs. Malformed caches remain on
    /// disk and the normal scanner can recover without losing their evidence.
    fn migrate_legacy_json_caches(&self) {
        let Ok(config) = config_dir() else {
            return;
        };

        let codex = config.join("usage-cache.json");
        if self
            .provider_has_scan_cache(ProviderId::primary(ProviderKind::Codex))
            .unwrap_or(false)
            || self.import_codex_json(&codex).unwrap_or(false)
        {
            let _ = fs::remove_file(codex);
        }

        let claude = config.join("claude-usage-cache.json");
        if self
            .provider_has_scan_cache(ProviderId::primary(ProviderKind::Claude))
            .unwrap_or(false)
            || self.import_claude_json(&claude).unwrap_or(false)
        {
            let _ = fs::remove_file(claude);
        }

        let cursor = config.join("cursor-usage-cache.json");
        if self
            .provider_has_daily_cache(ProviderId::primary(ProviderKind::Cursor))
            .unwrap_or(false)
            || self.import_cursor_json(&cursor).unwrap_or(false)
        {
            let _ = fs::remove_file(cursor);
        }
    }

    fn provider_has_scan_cache(&self, provider: ProviderId) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM scan_files WHERE provider = ?1)",
            params![provider.id()],
            |row| row.get(0),
        )?)
    }

    fn provider_has_daily_cache(&self, provider: ProviderId) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM usage_daily WHERE provider = ?1)",
            params![provider.id()],
            |row| row.get(0),
        )?)
    }

    fn import_codex_json(&self, path: &Path) -> Result<bool> {
        let Ok(bytes) = fs::read(path) else {
            return Ok(false);
        };
        let Ok(mut cache) = serde_json::from_slice::<UsageCache>(&bytes) else {
            return Ok(false);
        };
        if cache.version > CODEX_CACHE_VERSION {
            return Ok(false);
        }
        if cache.version != CODEX_CACHE_VERSION {
            cache.pricing_rebuild_needed = true;
        }
        self.save_codex_cache(ProviderId::primary(ProviderKind::Codex), &cache)?;
        Ok(true)
    }

    fn import_claude_json(&self, path: &Path) -> Result<bool> {
        let Ok(bytes) = fs::read(path) else {
            return Ok(false);
        };
        let Ok(cache) = serde_json::from_slice::<ClaudeUsageCache>(&bytes) else {
            return Ok(false);
        };
        if cache.version != CLAUDE_CACHE_VERSION {
            return Ok(false);
        }
        self.save_claude_cache(ProviderId::primary(ProviderKind::Claude), &cache)?;
        let stats = crate::usage::statistics_from_claude_cache(&cache, CACHE_RETENTION_DAYS as u16);
        self.replace_usage_daily(ProviderId::primary(ProviderKind::Claude), &stats.daily)?;
        self.replace_usage_model_daily(
            ProviderId::primary(ProviderKind::Claude),
            &aggregate_claude_model_daily(&cache),
        )?;
        Ok(true)
    }

    fn import_cursor_json(&self, path: &Path) -> Result<bool> {
        let Ok(bytes) = fs::read(path) else {
            return Ok(false);
        };
        let Ok(cache) = serde_json::from_slice::<LegacyCursorUsageCache>(&bytes) else {
            return Ok(false);
        };
        if cache.version != CURSOR_USAGE_VERSION {
            return Ok(false);
        }
        self.replace_usage_daily(ProviderId::primary(ProviderKind::Cursor), &cache.daily)?;
        self.set_usage_fetched_at(ProviderId::primary(ProviderKind::Cursor), cache.fetched_at)?;
        Ok(true)
    }
}

#[derive(Deserialize)]
struct LegacyCursorUsageCache {
    version: u8,
    fetched_at: DateTime<Utc>,
    daily: Vec<DailyTokenUsage>,
}

/// The primary instance keeps the original key so existing analytics survive.
const HOURLY_RETENTION_DAYS: i64 = 8;

/// Canonical UTC key for `usage_hourly.hour`; lexical order equals time order.
fn hour_key<Tz: TimeZone>(at: DateTime<Tz>) -> String {
    at.with_timezone(&Utc)
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn hourly_cutoff() -> String {
    hour_key(Utc::now() - Duration::days(HOURLY_RETENTION_DAYS))
}

fn openrouter_analytics_key(provider: ProviderId) -> String {
    if provider.is_primary() {
        "openrouter.analytics.v3".into()
    } else {
        format!("openrouter.analytics.v3.{}", provider.id())
    }
}

fn delete_stale_file_rows(
    tx: &rusqlite::Transaction<'_>,
    provider: ProviderId,
    stale: &[String],
) -> Result<()> {
    let provider = provider.id();
    for table in ["scan_files", "usage_file_daily", "usage_file_model_daily"] {
        let mut delete = tx.prepare(&format!(
            "DELETE FROM {table} WHERE provider = ?1 AND path = ?2"
        ))?;
        for path in stale {
            delete.execute(params![provider, path])?;
        }
    }
    Ok(())
}

/// Files that left the cache lose their scan row and all stored events. Events
/// of retained files are handled per file by `save_claude_cache`.
fn delete_stale_event_rows(
    tx: &rusqlite::Transaction<'_>,
    provider: ProviderId,
    stale: &[String],
) -> Result<()> {
    let provider = provider.id();
    for table in ["scan_files", "usage_events"] {
        let mut delete = tx.prepare(&format!(
            "DELETE FROM {table} WHERE provider = ?1 AND path = ?2"
        ))?;
        for path in stale {
            delete.execute(params![provider, path])?;
        }
    }
    Ok(())
}

#[derive(Default, Serialize, Deserialize)]
struct CodexFileMeta {
    current_model: Option<String>,
    #[serde(default)]
    fast_service_tier: bool,
    #[serde(default)]
    last_usage_signature: Option<String>,
    #[serde(default)]
    saw_session_meta: bool,
    #[serde(default)]
    suppressing_fork_copies: bool,
    #[serde(default)]
    fork_copy_anchor_ms: i64,
    #[serde(default)]
    session_id: String,
}

fn aggregate_codex_daily(cache: &UsageCache, history_days: u16) -> Vec<DailyTokenUsage> {
    statistics_from_daily(
        &cache
            .files
            .values()
            .flat_map(|file| file.daily.iter().cloned())
            .collect::<Vec<_>>(),
        history_days,
    )
    .daily
}

fn token_usage_from_row(row: &rusqlite::Row<'_>, start: usize) -> rusqlite::Result<TokenUsage> {
    Ok(TokenUsage {
        input_tokens: row.get::<_, i64>(start)? as u64,
        cached_input_tokens: row.get::<_, i64>(start + 1)? as u64,
        output_tokens: row.get::<_, i64>(start + 2)? as u64,
        requests: row.get::<_, i64>(start + 3)? as u64,
        estimated_cost_microusd: row.get::<_, i64>(start + 4)? as u64,
        priced_requests: row.get::<_, i64>(start + 5)? as u64,
        cache_savings_microusd: row.get::<_, i64>(start + 6).unwrap_or(0) as u64,
    })
}

fn aggregate_codex_model_daily(cache: &UsageCache) -> Vec<(String, NaiveDate, TokenUsage)> {
    let mut merged = BTreeMap::<(String, NaiveDate), TokenUsage>::new();
    for file in cache.files.values() {
        for (model, days) in &file.model_daily {
            for entry in days {
                merged
                    .entry((model.clone(), entry.date))
                    .or_default()
                    .add(&entry.usage);
            }
        }
    }
    merged
        .into_iter()
        .map(|((model, date), usage)| (model, date, usage))
        .collect()
}

fn aggregate_claude_model_daily(cache: &ClaudeUsageCache) -> Vec<(String, NaiveDate, TokenUsage)> {
    crate::usage::aggregate_claude_model_daily(cache)
        .into_iter()
        .map(|(date, model, usage)| (model, date, usage))
        .collect()
}

fn start_of_local_day(date: NaiveDate) -> DateTime<Local> {
    date.and_hms_opt(0, 0, 0)
        .and_then(|naive| Local.from_local_datetime(&naive).single())
        .unwrap_or_else(Local::now)
}

fn parse_date_option(raw: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok()
}

fn parse_date(raw: &str) -> NaiveDate {
    parse_date_option(raw).unwrap_or_else(|| Local::now().date_naive())
}

fn parse_datetime_option(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
}

fn truncate_local_hour(timestamp: DateTime<Local>) -> DateTime<Local> {
    timestamp
        .with_minute(0)
        .and_then(|value| value.with_second(0))
        .and_then(|value| value.with_nanosecond(0))
        .unwrap_or(timestamp)
}

fn config_dir() -> Result<PathBuf> {
    ProjectDirs::from("dev", "Codex Minibar", "Codex Minibar")
        .map(|dirs| dirs.config_dir().to_path_buf())
        .context("could not resolve the application config directory")
}

fn store_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("provider-store.sqlite"))
}

/// Convenience helper used by workers that only need a short critical section.
pub fn with_store<R>(f: impl FnOnce(&mut ProviderStore) -> Result<R>) -> Result<R> {
    let store = shared()?;
    let mut guard = store
        .lock()
        .map_err(|_| anyhow!("provider store lock poisoned"))?;
    f(&mut guard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn id(kind: ProviderKind) -> ProviderId {
        ProviderId::primary(kind)
    }

    fn test_store(path: &Path) -> ProviderStore {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        let store = ProviderStore { conn };
        store.migrate().unwrap();
        store
    }

    fn hour_usage(requests: u64) -> TokenUsage {
        TokenUsage {
            requests,
            ..Default::default()
        }
    }

    fn hourly_rows(store: &ProviderStore) -> Vec<(String, String)> {
        let mut statement = store
            .conn
            .prepare("SELECT provider, hour FROM usage_hourly ORDER BY provider, hour")
            .unwrap();
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    }

    #[test]
    fn hourly_retention_skips_old_rows_and_prunes_all_providers() {
        let dir = tempdir().unwrap();
        let store = test_store(&dir.path().join("hourly.sqlite"));
        let now = Local::now();
        let old = now - Duration::days(20);
        let recent = now - Duration::days(1);
        // Stale row for another provider written directly.
        store
            .conn
            .execute(
                "INSERT INTO usage_hourly(provider, hour, input_tokens, cached_input_tokens,
                    output_tokens, requests, estimated_cost_microusd, priced_requests)
                 VALUES('codex', ?1, 0, 0, 0, 1, 0, 0)",
                params![hour_key(old)],
            )
            .unwrap();
        store
            .replace_usage_hourly(
                id(ProviderKind::Cursor),
                &[(old, hour_usage(1)), (recent, hour_usage(2))],
            )
            .unwrap();
        let rows = hourly_rows(&store);
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].0, "cursor");
        assert!(rows[0].1.ends_with('Z'));
    }

    #[test]
    fn hourly_offset_rows_migrate_to_utc_and_range_is_offset_independent() {
        let dir = tempdir().unwrap();
        let store = test_store(&dir.path().join("migrate.sqlite"));
        let at = Utc::now() - Duration::days(1);
        let at = at
            .with_timezone(&Local)
            .with_minute(0)
            .and_then(|t| t.with_second(0))
            .and_then(|t| t.with_nanosecond(0))
            .unwrap();
        // Legacy format: arbitrary non-UTC offset text.
        let legacy = at
            .with_timezone(&chrono::FixedOffset::east_opt(3 * 3600).unwrap())
            .to_rfc3339();
        assert!(legacy.ends_with("+03:00"));
        store
            .conn
            .execute(
                "INSERT INTO usage_hourly(provider, hour, input_tokens, cached_input_tokens,
                    output_tokens, requests, estimated_cost_microusd, priced_requests)
                 VALUES('codex', ?1, 0, 0, 0, 5, 0, 0)",
                params![legacy],
            )
            .unwrap();
        store
            .conn
            .execute("DELETE FROM meta WHERE key='usage_hourly.utc.v1'", [])
            .unwrap();
        store.migrate_hourly_to_utc().unwrap();
        let rows = hourly_rows(&store);
        assert_eq!(rows, vec![("codex".into(), hour_key(at))]);

        // The query window is expressed in a different offset than the stored
        // text ever was; the instant comparison must still include the row.
        let start = at - Duration::minutes(30);
        let end = at + Duration::minutes(30);
        let loaded = store
            .load_usage_hourly(id(ProviderKind::Codex), start, end)
            .unwrap();
        assert_eq!(loaded.values().map(|u| u.requests).sum::<u64>(), 5);
        let outside = store
            .load_usage_hourly(
                id(ProviderKind::Codex),
                at + Duration::minutes(1),
                at + Duration::hours(2),
            )
            .unwrap();
        assert!(outside.is_empty());
    }

    #[test]
    fn prune_unknown_providers_keeps_known_and_clears_legacy_meta() {
        let dir = tempdir().unwrap();
        let store = test_store(&dir.path().join("prune.sqlite"));
        let keep = id(ProviderKind::Claude);
        let gone = id(ProviderKind::Cursor);
        let day = NaiveDate::from_ymd_opt(2026, 9, 10).unwrap();
        for provider in [keep, gone] {
            store
                .replace_usage_model_daily(provider, &[("m".into(), day, hour_usage(1))])
                .unwrap();
            store
                .replace_usage_hourly(provider, &[(Local::now(), hour_usage(1))])
                .unwrap();
            store
                .set_meta(&openrouter_analytics_key(provider), "{}")
                .unwrap();
        }
        store.set_meta("openrouter.analytics.v1", "x").unwrap();
        store.set_meta("openrouter.analytics.v2", "x").unwrap();
        store
            .set_meta("openrouter.analytics.v3.stale", "x")
            .unwrap();
        // Legacy keys are removed by migration.
        store.migrate().unwrap();
        let meta_keys = |store: &ProviderStore| -> Vec<String> {
            let mut s = store
                .conn
                .prepare("SELECT key FROM meta WHERE key LIKE 'openrouter.analytics.%'")
                .unwrap();
            s.query_map([], |r| r.get(0))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap()
        };
        assert!(
            !meta_keys(&store)
                .iter()
                .any(|k| k.ends_with(".v1") || k.ends_with(".v2"))
        );

        store.prune_unknown_providers(&[]).unwrap();
        assert_eq!(hourly_rows(&store).len(), 2, "empty list must not prune");

        store.prune_unknown_providers(&[keep]).unwrap();
        let rows = hourly_rows(&store);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, keep.id());
        let models: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM usage_model_daily", [], |r| r.get(0))
            .unwrap();
        assert_eq!(models, 1);
        let keys = meta_keys(&store);
        assert!(keys.iter().all(|k| !k.ends_with(".stale")), "{keys:?}");
        assert!(
            keys.contains(&openrouter_analytics_key(keep))
                || keep.kind() != ProviderKind::OpenRouter
        );
    }

    #[test]
    fn model_daily_keeps_dates_and_filters_provider_and_period() {
        let dir = tempdir().unwrap();
        let store = test_store(&dir.path().join("models.sqlite"));
        let day = NaiveDate::from_ymd_opt(2026, 9, 10).unwrap();
        let usage = TokenUsage {
            input_tokens: 10,
            requests: 1,
            ..Default::default()
        };
        store
            .replace_usage_model_daily(
                id(ProviderKind::Codex),
                &[
                    ("a".into(), day, usage.clone()),
                    ("a".into(), day + Duration::days(1), usage.clone()),
                    ("b".into(), day + Duration::days(1), usage.clone()),
                    ("old".into(), day - Duration::days(1), usage.clone()),
                ],
            )
            .unwrap();
        store
            .replace_usage_model_daily(
                id(ProviderKind::Claude),
                &[("other".into(), day, usage.clone())],
            )
            .unwrap();
        let rows = store
            .load_model_daily(
                crate::instances::ProviderId::from(ProviderKind::Codex),
                day,
                day + Duration::days(1),
            )
            .unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], ("a".into(), day, usage.clone()));
        assert_eq!(rows[1], ("a".into(), day + Duration::days(1), usage));
        assert_eq!(rows[2].0, "b");
    }

    fn sample_codex_cache() -> UsageCache {
        UsageCache {
            version: CODEX_CACHE_VERSION,
            pricing_rebuild_needed: false,
            files: BTreeMap::from([(
                "sessions/sample.jsonl".into(),
                CachedSessionFile {
                    offset: 42,
                    daily: vec![DailyTokenUsage {
                        date: Local::now().date_naive(),
                        usage: TokenUsage {
                            input_tokens: 10,
                            output_tokens: 5,
                            requests: 1,
                            priced_requests: 1,
                            estimated_cost_microusd: 100,
                            ..Default::default()
                        },
                    }],
                    current_model: Some("gpt-5".into()),
                    fast_service_tier: false,
                    model_daily: BTreeMap::from([(
                        "gpt-5".into(),
                        vec![DailyTokenUsage {
                            date: Local::now().date_naive(),
                            usage: TokenUsage {
                                input_tokens: 10,
                                output_tokens: 5,
                                requests: 1,
                                priced_requests: 1,
                                estimated_cost_microusd: 100,
                                ..Default::default()
                            },
                        }],
                    )]),
                    ..Default::default()
                },
            )]),
        }
    }

    #[test]
    fn openrouter_analytics_is_atomic_and_cleared() {
        let dir = tempdir().unwrap();
        let store = test_store(&dir.path().join("test.sqlite"));
        let day = DailyTokenUsage {
            date: Local::now().date_naive(),
            usage: TokenUsage {
                requests: 2,
                ..Default::default()
            },
        };
        store
            .save_openrouter_analytics(
                id(ProviderKind::OpenRouter),
                "first",
                std::slice::from_ref(&day),
                &[("model".into(), day.date, day.usage.clone())],
                Utc::now(),
            )
            .unwrap();
        store.conn.execute_batch("CREATE TRIGGER reject_analytics BEFORE INSERT ON usage_model_daily BEGIN SELECT RAISE(ABORT, 'test failure'); END;").unwrap();
        let mut changed = day.clone();
        changed.usage.requests = 10;
        assert!(
            store
                .save_openrouter_analytics(
                    id(ProviderKind::OpenRouter),
                    "second",
                    &[changed.clone()],
                    &[("model".into(), changed.date, changed.usage)],
                    Utc::now()
                )
                .is_err()
        );
        assert_eq!(
            store
                .load_usage_daily(
                    crate::instances::ProviderId::from(ProviderKind::OpenRouter),
                    30
                )
                .unwrap()
                .history
                .requests,
            2
        );
        assert_eq!(
            store
                .load_openrouter_analytics(id(ProviderKind::OpenRouter))
                .unwrap()
                .as_deref(),
            Some("first")
        );
        store.clear_usage_data().unwrap();
        assert!(
            store
                .load_openrouter_analytics(id(ProviderKind::OpenRouter))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn legacy_openrouter_total_usage_cache_is_hidden_until_credit_costs_are_refetched() {
        let dir = tempdir().unwrap();
        let store = test_store(&dir.path().join("test.sqlite"));
        let day = DailyTokenUsage {
            date: Local::now().date_naive(),
            usage: TokenUsage {
                requests: 1,
                priced_requests: 1,
                estimated_cost_microusd: 26_070_000,
                ..Default::default()
            },
        };
        store
            .save_openrouter_analytics(
                id(ProviderKind::OpenRouter),
                "old",
                std::slice::from_ref(&day),
                &[("model".into(), day.date, day.usage.clone())],
                Utc::now(),
            )
            .unwrap();
        store
            .conn
            .execute(
                "UPDATE meta SET key='openrouter.analytics.v1' WHERE key='openrouter.analytics.v3'",
                [],
            )
            .unwrap();
        assert!(
            store
                .load_openrouter_analytics(id(ProviderKind::OpenRouter))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            store
                .load_usage_daily(
                    crate::instances::ProviderId::from(ProviderKind::OpenRouter),
                    30
                )
                .unwrap()
                .history
                .estimated_cost_microusd,
            0
        );
        assert!(
            store
                .load_model_breakdown(
                    crate::instances::ProviderId::from(ProviderKind::OpenRouter),
                    day.date,
                    day.date
                )
                .unwrap()
                .is_empty()
        );
        assert!(
            store
                .load_model_daily(
                    crate::instances::ProviderId::from(ProviderKind::OpenRouter),
                    day.date,
                    day.date
                )
                .unwrap()
                .is_empty()
        );
        let fresh = DailyTokenUsage {
            usage: TokenUsage {
                estimated_cost_microusd: 15_440_000,
                ..day.usage.clone()
            },
            ..day.clone()
        };
        store
            .save_openrouter_analytics(
                id(ProviderKind::OpenRouter),
                "fresh",
                std::slice::from_ref(&fresh),
                &[("model".into(), day.date, fresh.usage.clone())],
                Utc::now(),
            )
            .unwrap();
        assert_eq!(
            store
                .load_usage_daily(
                    crate::instances::ProviderId::from(ProviderKind::OpenRouter),
                    30
                )
                .unwrap()
                .history
                .estimated_cost_microusd,
            15_440_000
        );
    }

    #[test]
    fn round_trips_usage_daily() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.sqlite");
        let store = test_store(&path);
        let days = vec![DailyTokenUsage {
            date: Local::now().date_naive(),
            usage: TokenUsage {
                input_tokens: 10,
                output_tokens: 5,
                requests: 1,
                priced_requests: 1,
                estimated_cost_microusd: 100,
                ..Default::default()
            },
        }];
        store
            .replace_usage_daily(
                crate::instances::ProviderId::from(ProviderKind::Cursor),
                &days,
            )
            .unwrap();
        let stats = store
            .load_usage_daily(crate::instances::ProviderId::from(ProviderKind::Cursor), 30)
            .unwrap();
        assert_eq!(stats.daily.len(), 1);
        assert_eq!(stats.history.requests, 1);
    }

    #[test]
    fn clear_usage_data_removes_derived_rows_and_fetch_markers() {
        let dir = tempdir().unwrap();
        let store = test_store(&dir.path().join("test.sqlite"));
        let today = Local::now().date_naive();
        store
            .replace_usage_daily(
                id(ProviderKind::Cursor),
                &[DailyTokenUsage {
                    date: today,
                    usage: TokenUsage {
                        input_tokens: 10,
                        requests: 1,
                        ..Default::default()
                    },
                }],
            )
            .unwrap();
        store
            .set_usage_fetched_at(
                crate::instances::ProviderId::from(ProviderKind::Cursor),
                Utc::now(),
            )
            .unwrap();

        store.clear_usage_data().unwrap();

        assert!(
            store
                .load_usage_daily(crate::instances::ProviderId::from(ProviderKind::Cursor), 30)
                .unwrap()
                .daily
                .is_empty()
        );
        assert!(
            store
                .usage_fetched_at(crate::instances::ProviderId::from(ProviderKind::Cursor))
                .unwrap()
                .is_none()
        );
    }

    /// Counts writes to the per-file Codex tables, ignoring temp-table
    /// bookkeeping that `total_changes()` would also include.
    fn track_file_writes(store: &ProviderStore) {
        let mut sql = String::from(
            "CREATE TEMP TABLE IF NOT EXISTS file_writes(n INTEGER NOT NULL);",
        );
        for table in ["scan_files", "usage_file_daily", "usage_file_model_daily"] {
            for op in ["INSERT", "UPDATE", "DELETE"] {
                sql.push_str(&format!(
                    "CREATE TEMP TRIGGER IF NOT EXISTS count_{table}_{op} AFTER {op} ON main.{table}
                     BEGIN INSERT INTO file_writes VALUES(1); END;"
                ));
            }
        }
        store.conn.execute_batch(&sql).unwrap();
    }

    fn file_writes(store: &ProviderStore) -> i64 {
        store
            .conn
            .query_row("SELECT COUNT(*) FROM temp.file_writes", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn unchanged_codex_cache_does_not_rewrite_rows() {
        let dir = tempdir().unwrap();
        let store = test_store(&dir.path().join("test.sqlite"));
        store
            .save_codex_cache(id(ProviderKind::Codex), &sample_codex_cache())
            .unwrap();
        let cache = store.load_codex_cache(id(ProviderKind::Codex)).unwrap();
        track_file_writes(&store);
        store
            .save_codex_cache(id(ProviderKind::Codex), &cache)
            .unwrap();

        assert_eq!(file_writes(&store), 0);
    }

    #[test]
    fn codex_save_rewrites_only_dirty_and_removes_stale_files() {
        let dir = tempdir().unwrap();
        let store = test_store(&dir.path().join("test.sqlite"));
        let codex = id(ProviderKind::Codex);
        let mut cache = sample_codex_cache();
        let other = cache.files["sessions/sample.jsonl"].clone();
        cache.files.insert("sessions/other.jsonl".into(), other);
        store.save_codex_cache(codex, &cache).unwrap();

        let mut cache = store.load_codex_cache(codex).unwrap();
        assert_eq!(cache.files.len(), 2);
        track_file_writes(&store);
        {
            let file = cache.files.get_mut("sessions/other.jsonl").unwrap();
            file.offset = 99;
            file.persisted = false;
        }
        store.save_codex_cache(codex, &cache).unwrap();
        // scan row + 1 daily delete/insert + 1 model delete/insert for one file.
        assert!(file_writes(&store) <= 5);
        let loaded = store.load_codex_cache(codex).unwrap();
        assert_eq!(loaded.files["sessions/other.jsonl"].offset, 99);
        assert_eq!(loaded.files["sessions/sample.jsonl"].offset, 42);
        assert_eq!(loaded.files["sessions/other.jsonl"].daily.len(), 1);

        let mut cache = loaded;
        cache.files.remove("sessions/other.jsonl");
        store.save_codex_cache(codex, &cache).unwrap();
        let loaded = store.load_codex_cache(codex).unwrap();
        assert_eq!(loaded.files.len(), 1);
        assert!(loaded.files.contains_key("sessions/sample.jsonl"));
    }

    #[test]
    fn claude_save_writes_only_new_tail_events() {
        let dir = tempdir().unwrap();
        let store = test_store(&dir.path().join("test.sqlite"));
        let claude = id(ProviderKind::Claude);
        let entry = |id: &str| CachedClaudeUsageEntry {
            timestamp: Utc::now(),
            message_id: Some(id.into()),
            request_id: None,
            is_sidechain: false,
            has_speed: false,
            usage: TokenUsage {
                input_tokens: 1,
                requests: 1,
                ..Default::default()
            },
            model: Some("claude-sonnet-4-20250514".into()),
        };
        let cache = ClaudeUsageCache {
            version: CLAUDE_CACHE_VERSION,
            files: BTreeMap::from([(
                "/p/a.jsonl".into(),
                CachedClaudeSessionFile {
                    offset: 10,
                    entries: vec![entry("m1"), entry("m2")],
                    persisted: None,
                },
            )]),
        };
        store.save_claude_cache(claude, &cache).unwrap();

        let mut cache = store.load_claude_cache(claude).unwrap();
        let before = store.conn.total_changes();
        store.save_claude_cache(claude, &cache).unwrap();
        assert_eq!(store.conn.total_changes(), before);

        let file = cache.files.get_mut("/p/a.jsonl").unwrap();
        file.entries.push(entry("m3"));
        file.offset = 20;
        let before = store.conn.total_changes();
        store.save_claude_cache(claude, &cache).unwrap();
        assert_eq!(store.conn.total_changes() - before, 2); // scan row + 1 event
        let mut cache = store.load_claude_cache(claude).unwrap();
        assert_eq!(cache.files["/p/a.jsonl"].entries.len(), 3);

        // Rebuild with fewer events truncates the stored tail.
        let file = cache.files.get_mut("/p/a.jsonl").unwrap();
        file.entries.truncate(1);
        file.persisted = None;
        store.save_claude_cache(claude, &cache).unwrap();
        let mut cache = store.load_claude_cache(claude).unwrap();
        assert_eq!(cache.files["/p/a.jsonl"].entries.len(), 1);

        cache.files.clear();
        store.save_claude_cache(claude, &cache).unwrap();
        assert!(store.load_claude_cache(claude).unwrap().files.is_empty());
    }

    #[test]
    fn codex_json_migration_preserves_offsets_and_daily_usage() {
        let dir = tempdir().unwrap();
        let store = test_store(&dir.path().join("test.sqlite"));
        let legacy_path = dir.path().join("usage-cache.json");
        fs::write(
            &legacy_path,
            serde_json::to_vec_pretty(&sample_codex_cache()).unwrap(),
        )
        .unwrap();

        assert!(store.import_codex_json(&legacy_path).unwrap());
        let migrated = store.load_codex_cache(id(ProviderKind::Codex)).unwrap();
        let file = migrated.files.get("sessions/sample.jsonl").unwrap();
        assert_eq!(file.offset, 42);
        assert_eq!(file.daily[0].usage.total_tokens(), 15);
        assert_eq!(
            file.model_daily
                .get("gpt-5")
                .map(|days| days[0].usage.total_tokens()),
            Some(15)
        );
        assert_eq!(
            store
                .load_usage_daily(crate::instances::ProviderId::from(ProviderKind::Codex), 30)
                .unwrap()
                .history
                .requests,
            1
        );
    }

    #[test]
    fn counts_claude_sessions_from_events_not_file_daily() {
        let dir = tempdir().unwrap();
        let store = test_store(&dir.path().join("test.sqlite"));
        let cache = ClaudeUsageCache {
            version: CLAUDE_CACHE_VERSION,
            files: BTreeMap::from([(
                "/home/.claude/projects/a/session.jsonl".into(),
                CachedClaudeSessionFile {
                    offset: 10,
                    entries: vec![CachedClaudeUsageEntry {
                        timestamp: Utc::now(),
                        message_id: Some("m1".into()),
                        request_id: Some("r1".into()),
                        is_sidechain: false,
                        has_speed: true,
                        usage: TokenUsage {
                            input_tokens: 10,
                            output_tokens: 2,
                            requests: 1,
                            priced_requests: 1,
                            estimated_cost_microusd: 100,
                            ..Default::default()
                        },
                        model: Some("claude-sonnet-4-20250514".into()),
                    }],
                    persisted: None,
                },
            )]),
        };
        store
            .save_claude_cache(id(ProviderKind::Claude), &cache)
            .unwrap();
        let today = Local::now().date_naive();
        assert_eq!(
            store
                .count_session_paths(
                    crate::instances::ProviderId::from(ProviderKind::Claude),
                    today,
                    today
                )
                .unwrap(),
            1
        );
    }

    #[test]
    fn round_trips_codex_model_daily_into_breakdown() {
        let dir = tempdir().unwrap();
        let store = test_store(&dir.path().join("test.sqlite"));
        store
            .save_codex_cache(id(ProviderKind::Codex), &sample_codex_cache())
            .unwrap();
        let loaded = store.load_codex_cache(id(ProviderKind::Codex)).unwrap();
        let file = loaded.files.get("sessions/sample.jsonl").unwrap();
        assert_eq!(
            file.model_daily
                .get("gpt-5")
                .map(|days| days[0].usage.total_tokens()),
            Some(15)
        );
        let today = Local::now().date_naive();
        let models = store
            .load_model_breakdown(
                crate::instances::ProviderId::from(ProviderKind::Codex),
                today,
                today,
            )
            .unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].0, "gpt-5");
        assert_eq!(models[0].1.total_tokens(), 15);
    }
}

