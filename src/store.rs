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
use chrono::{DateTime, Local, NaiveDate, Utc};
use directories::ProjectDirs;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::limits::{ProviderLimits, RateLimits};
use crate::settings::ProviderKind;
use crate::usage::{
    CachedClaudeSessionFile, CachedClaudeUsageEntry, CachedSessionFile, ClaudeUsageCache,
    DailyTokenUsage, TokenUsage, UsageCache, UsageStatistics, statistics_from_daily,
};

const SCHEMA_VERSION: i64 = 1;
const CODEX_CACHE_VERSION: u8 = 3;
const CLAUDE_CACHE_VERSION: u8 = 1;
const CURSOR_USAGE_VERSION: u8 = 2;
const CACHE_RETENTION_DAYS: i64 = 365;

static SHARED: OnceLock<Arc<Mutex<ProviderStore>>> = OnceLock::new();

/// Process-wide store. Opened once, shared by startup hydration and workers.
pub fn shared() -> Result<Arc<Mutex<ProviderStore>>> {
    if let Some(store) = SHARED.get() {
        return Ok(Arc::clone(store));
    }
    let store = Arc::new(Mutex::new(ProviderStore::open()?));
    let _ = SHARED.set(Arc::clone(&store));
    Ok(SHARED
        .get()
        .map(Arc::clone)
        .unwrap_or(store))
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
             PRAGMA foreign_keys=ON;",
        )
        .context("configure sqlite")?;
        let store = Self { conn };
        store.migrate()?;
        store.migrate_legacy_json_caches();
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
            ",
        )?;
        self.set_meta("schema_version", &SCHEMA_VERSION.to_string())?;
        Ok(())
    }

    pub fn hydrate_provider_limits(&self, history_days: u16) -> Result<ProviderLimits> {
        let mut limits = ProviderLimits::default();
        for provider in [ProviderKind::Codex, ProviderKind::Claude, ProviderKind::Cursor] {
            let mut snapshot = self
                .load_limits(provider)?
                .unwrap_or_default();
            snapshot.usage = self.load_usage_daily(provider, history_days)?;
            *limits.get_mut(provider) = snapshot;
        }
        Ok(limits)
    }

    pub fn load_limits(&self, provider: ProviderKind) -> Result<Option<RateLimits>> {
        let mut statement = self.conn.prepare(
            "SELECT payload_json FROM limits WHERE provider = ?1",
        )?;
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

    pub fn save_limits(&self, provider: ProviderKind, limits: &RateLimits) -> Result<()> {
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
        provider: ProviderKind,
        history_days: u16,
    ) -> Result<UsageStatistics> {
        let mut statement = self.conn.prepare(
            "SELECT date, input_tokens, cached_input_tokens, output_tokens,
                    requests, estimated_cost_microusd, priced_requests
             FROM usage_daily
             WHERE provider = ?1
             ORDER BY date ASC",
        )?;
        let rows = statement.query_map(params![provider.id()], |row| {
            Ok(DailyTokenUsage {
                date: parse_date(&row.get::<_, String>(0)?),
                usage: token_usage_from_row(row, 1)?,
            })
        })?;
        let mut daily = Vec::new();
        for row in rows {
            daily.push(row?);
        }
        Ok(statistics_from_daily(&daily, history_days))
    }

    pub fn replace_usage_daily(
        &self,
        provider: ProviderKind,
        days: &[DailyTokenUsage],
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let mut retained = BTreeSet::new();
        {
            let mut insert = tx.prepare(
                "INSERT INTO usage_daily(
                    provider, date, input_tokens, cached_input_tokens, output_tokens,
                    requests, estimated_cost_microusd, priced_requests
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(provider, date) DO UPDATE SET
                    input_tokens=excluded.input_tokens,
                    cached_input_tokens=excluded.cached_input_tokens,
                    output_tokens=excluded.output_tokens,
                    requests=excluded.requests,
                    estimated_cost_microusd=excluded.estimated_cost_microusd,
                    priced_requests=excluded.priced_requests
                 WHERE input_tokens IS NOT excluded.input_tokens
                    OR cached_input_tokens IS NOT excluded.cached_input_tokens
                    OR output_tokens IS NOT excluded.output_tokens
                    OR requests IS NOT excluded.requests
                    OR estimated_cost_microusd IS NOT excluded.estimated_cost_microusd
                    OR priced_requests IS NOT excluded.priced_requests",
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
                ])?;
            }
        }
        let existing = {
            let mut statement = tx.prepare("SELECT date FROM usage_daily WHERE provider = ?1")?;
            let rows = statement.query_map(params![provider.id()], |row| row.get::<_, String>(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        for date in existing {
            if !retained.contains(&date) {
                tx.execute(
                    "DELETE FROM usage_daily WHERE provider = ?1 AND date = ?2",
                    params![provider.id(), date],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn usage_fetched_at(&self, provider: ProviderKind) -> Result<Option<DateTime<Utc>>> {
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
        provider: ProviderKind,
        fetched_at: DateTime<Utc>,
    ) -> Result<()> {
        self.upsert_provider_meta(
            provider,
            Some(fetched_at),
            CURSOR_USAGE_VERSION as i64,
            None,
        )
    }

    pub fn cursor_usage_version(&self) -> Result<u8> {
        let mut statement = self
            .conn
            .prepare("SELECT schema_version FROM provider_meta WHERE provider = ?1")?;
        let version: Option<i64> = statement
            .query_row(params![ProviderKind::Cursor.id()], |row| row.get(0))
            .optional()?;
        Ok(version.unwrap_or(0) as u8)
    }

    pub(crate) fn load_codex_cache(&self) -> Result<UsageCache> {
        let flags = self.provider_flags(ProviderKind::Codex)?;
        let pricing_rebuild_needed = flags
            .get("pricing_rebuild_needed")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let version = flags
            .get("cache_version")
            .and_then(|value| value.as_u64())
            .unwrap_or(u64::from(CODEX_CACHE_VERSION)) as u8;

        let mut files = BTreeMap::new();
        {
            let mut statement = self.conn.prepare(
                "SELECT path, offset, meta_json FROM scan_files WHERE provider = ?1",
            )?;
            let rows = statement.query_map(params![ProviderKind::Codex.id()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)? as u64,
                    row.get::<_, String>(2)?,
                ))
            })?;
            for row in rows {
                let (path, offset, meta_json) = row?;
                let meta: CodexFileMeta =
                    serde_json::from_str(&meta_json).unwrap_or_default();
                files.insert(
                    path,
                    CachedSessionFile {
                        offset,
                        daily: Vec::new(),
                        current_model: meta.current_model,
                        fast_service_tier: meta.fast_service_tier,
                    },
                );
            }
        }
        {
            let mut statement = self.conn.prepare(
                "SELECT path, date, input_tokens, cached_input_tokens, output_tokens,
                        requests, estimated_cost_microusd, priced_requests
                 FROM usage_file_daily WHERE provider = ?1",
            )?;
            let rows = statement.query_map(params![ProviderKind::Codex.id()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    parse_date(&row.get::<_, String>(1)?),
                    token_usage_from_row(row, 2)?,
                ))
            })?;
            for row in rows {
                let (path, date, usage) = row?;
                let file = files.entry(path).or_default();
                file.daily.push(DailyTokenUsage { date, usage });
            }
        }
        for file in files.values_mut() {
            file.daily.sort_by_key(|entry| entry.date);
        }
        Ok(UsageCache {
            version,
            pricing_rebuild_needed,
            files,
        })
    }

    pub(crate) fn save_codex_cache(&self, cache: &UsageCache) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let mut retained_files = BTreeSet::new();
        let mut retained_days = BTreeSet::new();
        {
            let mut scan = tx.prepare(
                "INSERT INTO scan_files(provider, path, offset, meta_json)
                 VALUES(?1, ?2, ?3, ?4)
                 ON CONFLICT(provider, path) DO UPDATE SET
                    offset=excluded.offset,
                    meta_json=excluded.meta_json
                 WHERE offset IS NOT excluded.offset OR meta_json IS NOT excluded.meta_json",
            )?;
            let mut daily = tx.prepare(
                "INSERT INTO usage_file_daily(
                    provider, path, date, input_tokens, cached_input_tokens, output_tokens,
                    requests, estimated_cost_microusd, priced_requests
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(provider, path, date) DO UPDATE SET
                    input_tokens=excluded.input_tokens,
                    cached_input_tokens=excluded.cached_input_tokens,
                    output_tokens=excluded.output_tokens,
                    requests=excluded.requests,
                    estimated_cost_microusd=excluded.estimated_cost_microusd,
                    priced_requests=excluded.priced_requests
                 WHERE input_tokens IS NOT excluded.input_tokens
                    OR cached_input_tokens IS NOT excluded.cached_input_tokens
                    OR output_tokens IS NOT excluded.output_tokens
                    OR requests IS NOT excluded.requests
                    OR estimated_cost_microusd IS NOT excluded.estimated_cost_microusd
                    OR priced_requests IS NOT excluded.priced_requests",
            )?;
            for (path, file) in &cache.files {
                retained_files.insert(path.clone());
                let meta = serde_json::to_string(&CodexFileMeta {
                    current_model: file.current_model.clone(),
                    fast_service_tier: file.fast_service_tier,
                })?;
                scan.execute(params![
                    ProviderKind::Codex.id(),
                    path,
                    file.offset as i64,
                    meta
                ])?;
                for entry in &file.daily {
                    retained_days.insert((path.clone(), entry.date.to_string()));
                    daily.execute(params![
                        ProviderKind::Codex.id(),
                        path,
                        entry.date.to_string(),
                        entry.usage.input_tokens as i64,
                        entry.usage.cached_input_tokens as i64,
                        entry.usage.output_tokens as i64,
                        entry.usage.requests as i64,
                        entry.usage.estimated_cost_microusd as i64,
                        entry.usage.priced_requests as i64,
                    ])?;
                }
            }
        }
        delete_stale_file_rows(&tx, &retained_files, &retained_days)?;
        tx.commit()?;

        let flags = json!({
            "cache_version": cache.version,
            "pricing_rebuild_needed": cache.pricing_rebuild_needed,
        });
        self.upsert_provider_meta(
            ProviderKind::Codex,
            None,
            i64::from(cache.version),
            Some(flags.to_string()),
        )?;
        self.replace_usage_daily(
            ProviderKind::Codex,
            &aggregate_codex_daily(cache, CACHE_RETENTION_DAYS as u16),
        )?;
        Ok(())
    }

    pub(crate) fn load_claude_cache(&self) -> Result<ClaudeUsageCache> {
        let flags = self.provider_flags(ProviderKind::Claude)?;
        let version = flags
            .get("cache_version")
            .and_then(|value| value.as_u64())
            .unwrap_or(u64::from(CLAUDE_CACHE_VERSION)) as u8;
        if version != CLAUDE_CACHE_VERSION {
            return Ok(ClaudeUsageCache::default());
        }

        let mut files = BTreeMap::new();
        {
            let mut statement = self.conn.prepare(
                "SELECT path, offset FROM scan_files WHERE provider = ?1",
            )?;
            let rows = statement.query_map(params![ProviderKind::Claude.id()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
            })?;
            for row in rows {
                let (path, offset) = row?;
                files.insert(
                    path,
                    CachedClaudeSessionFile {
                        offset,
                        entries: Vec::new(),
                    },
                );
            }
        }
        {
            let mut statement = self.conn.prepare(
                "SELECT path, ts, message_id, request_id, is_sidechain, has_speed,
                        input_tokens, cached_input_tokens, output_tokens,
                        requests, estimated_cost_microusd, priced_requests
                 FROM usage_events
                 WHERE provider = ?1
                 ORDER BY path ASC, event_ord ASC",
            )?;
            let rows = statement.query_map(params![ProviderKind::Claude.id()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    CachedClaudeUsageEntry {
                        timestamp: parse_datetime(&row.get::<_, String>(1)?),
                        message_id: row.get(2)?,
                        request_id: row.get(3)?,
                        is_sidechain: row.get::<_, i64>(4)? != 0,
                        has_speed: row.get::<_, i64>(5)? != 0,
                        usage: token_usage_from_row(row, 6)?,
                    },
                ))
            })?;
            for row in rows {
                let (path, entry) = row?;
                files.entry(path).or_default().entries.push(entry);
            }
        }
        Ok(ClaudeUsageCache { version, files })
    }

    pub(crate) fn save_claude_cache(&self, cache: &ClaudeUsageCache) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let mut retained_files = BTreeSet::new();
        let mut retained_events = BTreeSet::new();
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
                    output_tokens, requests, estimated_cost_microusd, priced_requests
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
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
                    priced_requests=excluded.priced_requests
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
                    OR priced_requests IS NOT excluded.priced_requests",
            )?;
            for (path, file) in &cache.files {
                retained_files.insert(path.clone());
                scan.execute(params![
                    ProviderKind::Claude.id(),
                    path,
                    file.offset as i64
                ])?;
                for (event_ord, entry) in file.entries.iter().enumerate() {
                    retained_events.insert((path.clone(), event_ord as i64));
                    events.execute(params![
                        ProviderKind::Claude.id(),
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
                    ])?;
                }
            }
        }
        delete_stale_event_rows(&tx, &retained_files, &retained_events)?;
        tx.commit()?;

        let flags = json!({ "cache_version": cache.version });
        self.upsert_provider_meta(
            ProviderKind::Claude,
            None,
            i64::from(cache.version),
            Some(flags.to_string()),
        )?;
        Ok(())
    }

    fn provider_flags(&self, provider: ProviderKind) -> Result<serde_json::Value> {
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
        provider: ProviderKind,
        usage_fetched_at: Option<DateTime<Utc>>,
        schema_version: i64,
        flags_json: Option<String>,
    ) -> Result<()> {
        let existing_flags = self.provider_flags(provider)?;
        let flags = flags_json.unwrap_or_else(|| existing_flags.to_string());
        let fetched = usage_fetched_at
            .map(|at| at.to_rfc3339())
            .or_else(|| {
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

    fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value
             WHERE meta.value IS NOT excluded.value",
            params![key, value],
        )?;
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
        if self.provider_has_scan_cache(ProviderKind::Codex).unwrap_or(false)
            || self.import_codex_json(&codex).unwrap_or(false)
        {
            let _ = fs::remove_file(codex);
        }

        let claude = config.join("claude-usage-cache.json");
        if self.provider_has_scan_cache(ProviderKind::Claude).unwrap_or(false)
            || self.import_claude_json(&claude).unwrap_or(false)
        {
            let _ = fs::remove_file(claude);
        }

        let cursor = config.join("cursor-usage-cache.json");
        if self.provider_has_daily_cache(ProviderKind::Cursor).unwrap_or(false)
            || self.import_cursor_json(&cursor).unwrap_or(false)
        {
            let _ = fs::remove_file(cursor);
        }
    }

    fn provider_has_scan_cache(&self, provider: ProviderKind) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM scan_files WHERE provider = ?1)",
            params![provider.id()],
            |row| row.get(0),
        )?)
    }

    fn provider_has_daily_cache(&self, provider: ProviderKind) -> Result<bool> {
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
        self.save_codex_cache(&cache)?;
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
        self.save_claude_cache(&cache)?;
        let stats = crate::usage::statistics_from_claude_cache(
            &cache,
            CACHE_RETENTION_DAYS as u16,
        );
        self.replace_usage_daily(ProviderKind::Claude, &stats.daily)?;
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
        self.replace_usage_daily(ProviderKind::Cursor, &cache.daily)?;
        self.set_usage_fetched_at(ProviderKind::Cursor, cache.fetched_at)?;
        Ok(true)
    }
}

#[derive(Deserialize)]
struct LegacyCursorUsageCache {
    version: u8,
    fetched_at: DateTime<Utc>,
    daily: Vec<DailyTokenUsage>,
}

fn delete_stale_file_rows(
    tx: &rusqlite::Transaction<'_>,
    retained_files: &BTreeSet<String>,
    retained_days: &BTreeSet<(String, String)>,
) -> Result<()> {
    let provider = ProviderKind::Codex.id();
    let existing_files = {
        let mut statement = tx.prepare("SELECT path FROM scan_files WHERE provider = ?1")?;
        let rows = statement.query_map(params![provider], |row| row.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for path in existing_files {
        if !retained_files.contains(&path) {
            tx.execute(
                "DELETE FROM scan_files WHERE provider = ?1 AND path = ?2",
                params![provider, path],
            )?;
        }
    }

    let existing_days = {
        let mut statement = tx.prepare(
            "SELECT path, date FROM usage_file_daily WHERE provider = ?1",
        )?;
        let rows = statement.query_map(params![provider], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (path, date) in existing_days {
        if !retained_days.contains(&(path.clone(), date.clone())) {
            tx.execute(
                "DELETE FROM usage_file_daily WHERE provider = ?1 AND path = ?2 AND date = ?3",
                params![provider, path, date],
            )?;
        }
    }
    Ok(())
}

fn delete_stale_event_rows(
    tx: &rusqlite::Transaction<'_>,
    retained_files: &BTreeSet<String>,
    retained_events: &BTreeSet<(String, i64)>,
) -> Result<()> {
    let provider = ProviderKind::Claude.id();
    let existing_files = {
        let mut statement = tx.prepare("SELECT path FROM scan_files WHERE provider = ?1")?;
        let rows = statement.query_map(params![provider], |row| row.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for path in existing_files {
        if !retained_files.contains(&path) {
            tx.execute(
                "DELETE FROM scan_files WHERE provider = ?1 AND path = ?2",
                params![provider, path],
            )?;
        }
    }

    let existing_events = {
        let mut statement = tx.prepare(
            "SELECT path, event_ord FROM usage_events WHERE provider = ?1",
        )?;
        let rows = statement.query_map(params![provider], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (path, event_ord) in existing_events {
        if !retained_events.contains(&(path.clone(), event_ord)) {
            tx.execute(
                "DELETE FROM usage_events WHERE provider = ?1 AND path = ?2 AND event_ord = ?3",
                params![provider, path, event_ord],
            )?;
        }
    }
    Ok(())
}

#[derive(Default, Serialize, Deserialize)]
struct CodexFileMeta {
    current_model: Option<String>,
    #[serde(default)]
    fast_service_tier: bool,
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
    })
}

fn parse_date(raw: &str) -> NaiveDate {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").unwrap_or_else(|_| Local::now().date_naive())
}

fn parse_datetime(raw: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
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

    fn test_store(path: &Path) -> ProviderStore {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        let store = ProviderStore { conn };
        store.migrate().unwrap();
        store
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
                },
            )]),
        }
    }

    #[test]
    fn round_trips_usage_daily() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.sqlite");
        let store = test_store(&path);
        let days = vec![DailyTokenUsage {
            date: NaiveDate::from_ymd_opt(2026, 7, 14).unwrap(),
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
            .replace_usage_daily(ProviderKind::Cursor, &days)
            .unwrap();
        let stats = store.load_usage_daily(ProviderKind::Cursor, 30).unwrap();
        assert_eq!(stats.daily.len(), 1);
        assert_eq!(stats.history.requests, 1);
    }

    #[test]
    fn unchanged_codex_cache_does_not_rewrite_rows() {
        let dir = tempdir().unwrap();
        let store = test_store(&dir.path().join("test.sqlite"));
        let cache = sample_codex_cache();

        store.save_codex_cache(&cache).unwrap();
        let changes_after_first_save = store.conn.total_changes();
        store.save_codex_cache(&cache).unwrap();

        assert_eq!(store.conn.total_changes(), changes_after_first_save);
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
        let migrated = store.load_codex_cache().unwrap();
        let file = migrated.files.get("sessions/sample.jsonl").unwrap();
        assert_eq!(file.offset, 42);
        assert_eq!(file.daily[0].usage.total_tokens(), 15);
        assert_eq!(
            store
                .load_usage_daily(ProviderKind::Codex, 30)
                .unwrap()
                .history
                .requests,
            1
        );
    }
}
