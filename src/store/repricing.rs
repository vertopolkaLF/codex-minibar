//! Heals usage rows that were scanned while their model had no catalog rate.
//!
//! Scanners freeze `estimated_cost_microusd` at scan time. When the LiteLLM
//! catalog later learns a model, rows with `requests > 0 AND priced_requests = 0`
//! are repriced here from their stored token columns. Already priced rows are
//! never touched, and models that still have no rate stay unpriced, so the
//! pass is idempotent.
//!
//! * Codex: `codex_events` (account-attributed events, the source of truth for
//!   the primary instance) plus the per-file cache tables `usage_file_model_daily`
//!   / `usage_file_daily` and the rollups `usage_daily` / `usage_model_daily`.
//!   Stored `input_tokens` already includes cached tokens; cache-write tokens are
//!   not stored and are priced at the input rate (the catalog default for OpenAI
//!   models). `codex_legacy_*` tables are frozen historical totals and are left
//!   alone, as are `usage_hourly` (not populated for Codex) and mixed
//!   priced/unpriced aggregate rows (the unpriced share is unknown).
//! * Claude: `usage_events` stores `input + cache_creation` merged and clips the
//!   cache-read count, so the exact cost cannot be rebuilt from columns. Matching
//!   events are re-read from the still-existing JSONL logs; events whose log was
//!   deleted stay unpriced rather than getting a wrong cost. Rollups
//!   (`usage_daily`, `usage_model_daily`) are then recomputed with the scanner's
//!   own deduplication.

use std::{
    collections::{BTreeSet, HashMap},
    fs,
};

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;
use serde_json::Value;

use super::ProviderStore;
use crate::{instances::ProviderId, pricing, settings::ProviderKind, usage};

/// `(provider, model, cache creation, uncached input, cache read, output)` to
/// `(cost, cache savings)`; `None` while the model has no rate.
pub(crate) type Pricer<'a> =
    dyn Fn(ProviderKind, &str, u64, u64, u64, u64) -> Option<(u64, u64)> + 'a;

// Only the non-test worker loop reprices.
#[cfg_attr(test, allow(dead_code))]
fn catalog_pricer(
    kind: ProviderKind,
    model: &str,
    creation: u64,
    uncached: u64,
    cached: u64,
    output: u64,
) -> Option<(u64, u64)> {
    let cost =
        pricing::request_cost_microusd(kind, Some(model), creation, uncached, cached, output)?;
    Some((
        cost,
        pricing::cache_savings_microusd(kind, Some(model), cached),
    ))
}

/// Reprices every unpriced row whose model the current catalog can price, once
/// per catalog generation. `seen` is the caller's last healed generation; each
/// usage worker keeps its own so it heals before its own scan loads the cache.
/// Returns whether anything changed.
// Only the non-test worker loop reprices.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn reprice_for_current_catalog(seen: &mut u64) -> Result<bool> {
    let generation = pricing::catalog_generation();
    if generation == 0 || generation == *seen {
        return Ok(false);
    }
    let changed = super::with_store(|store| store.reprice_unpriced_usage(&catalog_pricer))?;
    *seen = generation;
    Ok(changed)
}

fn is_kind(provider: &str, kind: ProviderKind) -> bool {
    provider == kind.id()
        || provider
            .strip_prefix(kind.id())
            .is_some_and(|rest| rest.starts_with('-'))
}

type ClaudeKey = (DateTime<Utc>, Option<String>, Option<String>, u64);

impl ProviderStore {
    /// One transaction per provider family.
    pub(crate) fn reprice_unpriced_usage(&self, pricer: &Pricer<'_>) -> Result<bool> {
        let codex = self.reprice_codex(pricer)?;
        let claude = self.reprice_claude(pricer)?;
        Ok(codex || claude)
    }

    fn reprice_codex(&self, pricer: &Pricer<'_>) -> Result<bool> {
        let tx = self.conn.unchecked_transaction()?;
        let mut changed = false;
        let mut priceable: HashMap<String, bool> = HashMap::new();
        let mut can_price = |model: &str| -> bool {
            *priceable
                .entry(model.to_owned())
                .or_insert_with(|| pricer(ProviderKind::Codex, model, 0, 1, 0, 0).is_some())
        };

        // Account-attributed events.
        let events: Vec<(i64, i64, i64, String, u64, u64, u64)> = {
            let mut q = tx.prepare(
                "SELECT e.session, e.ts, e.sig, n.name, e.input_tokens, e.cached_input_tokens, e.output_tokens
                 FROM codex_events e JOIN codex_names n ON n.id = e.model
                 WHERE e.requests > 0 AND e.priced_requests = 0",
            )?;
            q.query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get::<_, i64>(4)? as u64,
                    r.get::<_, i64>(5)? as u64,
                    r.get::<_, i64>(6)? as u64,
                ))
            })?
            .collect::<rusqlite::Result<_>>()?
        };
        {
            let mut update = tx.prepare(
                "UPDATE codex_events SET estimated_cost_microusd=?4, priced_requests=requests, cache_savings_microusd=?5
                 WHERE session=?1 AND ts=?2 AND sig=?3 AND priced_requests=0",
            )?;
            for (session, ts, sig, model, input, cached, output) in events {
                if !can_price(&model) {
                    continue;
                }
                let uncached = input.saturating_sub(cached);
                if let Some((cost, savings)) =
                    pricer(ProviderKind::Codex, &model, 0, uncached, cached, output)
                {
                    changed |=
                        update.execute(params![session, ts, sig, cost as i64, savings as i64])? > 0;
                }
            }
        }

        // Per-file model/day cache rows (also the data of non-primary instances).
        let rows: Vec<(String, String, String, String, u64, u64, u64)> = {
            let mut q = tx.prepare(
                "SELECT provider, path, date, model, input_tokens, cached_input_tokens, output_tokens
                 FROM usage_file_model_daily WHERE requests > 0 AND priced_requests = 0",
            )?;
            q.query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get::<_, i64>(4)? as u64,
                    r.get::<_, i64>(5)? as u64,
                    r.get::<_, i64>(6)? as u64,
                ))
            })?
            .collect::<rusqlite::Result<_>>()?
        };
        let mut file_days = BTreeSet::new();
        let mut model_days = BTreeSet::new();
        {
            let mut update = tx.prepare(
                "UPDATE usage_file_model_daily SET estimated_cost_microusd=?5, priced_requests=requests, cache_savings_microusd=?6
                 WHERE provider=?1 AND path=?2 AND date=?3 AND model=?4 AND priced_requests=0",
            )?;
            for (provider, path, date, model, input, cached, output) in rows {
                if !is_kind(&provider, ProviderKind::Codex) || !can_price(&model) {
                    continue;
                }
                let uncached = input.saturating_sub(cached);
                let Some((cost, savings)) =
                    pricer(ProviderKind::Codex, &model, 0, uncached, cached, output)
                else {
                    continue;
                };
                if update.execute(params![
                    provider,
                    path,
                    date,
                    model,
                    cost as i64,
                    savings as i64
                ])? > 0
                {
                    changed = true;
                    file_days.insert((provider.clone(), path, date.clone()));
                    model_days.insert((provider, date, model));
                }
            }
        }
        // Parents are only rewritten when their children cover every request.
        for (provider, path, date) in &file_days {
            tx.execute(
                "UPDATE usage_file_daily SET
                    estimated_cost_microusd=(SELECT SUM(estimated_cost_microusd) FROM usage_file_model_daily WHERE provider=?1 AND path=?2 AND date=?3),
                    priced_requests=(SELECT SUM(priced_requests) FROM usage_file_model_daily WHERE provider=?1 AND path=?2 AND date=?3),
                    cache_savings_microusd=(SELECT SUM(cache_savings_microusd) FROM usage_file_model_daily WHERE provider=?1 AND path=?2 AND date=?3)
                 WHERE provider=?1 AND path=?2 AND date=?3
                   AND requests=(SELECT SUM(requests) FROM usage_file_model_daily WHERE provider=?1 AND path=?2 AND date=?3)",
                params![provider, path, date],
            )?;
        }
        let provider_days: BTreeSet<_> = model_days
            .iter()
            .map(|(p, d, _)| (p.clone(), d.clone()))
            .collect();
        for (provider, date) in &provider_days {
            tx.execute(
                "UPDATE usage_daily SET
                    estimated_cost_microusd=(SELECT SUM(estimated_cost_microusd) FROM usage_file_daily WHERE provider=?1 AND date=?2),
                    priced_requests=(SELECT SUM(priced_requests) FROM usage_file_daily WHERE provider=?1 AND date=?2),
                    cache_savings_microusd=(SELECT SUM(cache_savings_microusd) FROM usage_file_daily WHERE provider=?1 AND date=?2)
                 WHERE provider=?1 AND date=?2
                   AND requests=(SELECT SUM(requests) FROM usage_file_daily WHERE provider=?1 AND date=?2)",
                params![provider, date],
            )?;
        }
        for (provider, date, model) in &model_days {
            tx.execute(
                "UPDATE usage_model_daily SET
                    estimated_cost_microusd=(SELECT SUM(estimated_cost_microusd) FROM usage_file_model_daily WHERE provider=?1 AND date=?2 AND model=?3),
                    priced_requests=(SELECT SUM(priced_requests) FROM usage_file_model_daily WHERE provider=?1 AND date=?2 AND model=?3),
                    cache_savings_microusd=(SELECT SUM(cache_savings_microusd) FROM usage_file_model_daily WHERE provider=?1 AND date=?2 AND model=?3)
                 WHERE provider=?1 AND date=?2 AND model=?3
                   AND requests=(SELECT SUM(requests) FROM usage_file_model_daily WHERE provider=?1 AND date=?2 AND model=?3)",
                params![provider, date, model],
            )?;
        }
        tx.commit()?;
        Ok(changed)
    }

    fn reprice_claude(&self, pricer: &Pricer<'_>) -> Result<bool> {
        struct Row {
            provider: String,
            path: String,
            ord: i64,
            key: ClaudeKey,
            input: u64,
            model: String,
        }
        let mut by_path: HashMap<String, Vec<Row>> = HashMap::new();
        let mut priceable: HashMap<String, bool> = HashMap::new();
        {
            let mut q = self.conn.prepare(
                "SELECT provider, path, event_ord, ts, message_id, request_id, input_tokens, output_tokens, model
                 FROM usage_events WHERE requests > 0 AND priced_requests = 0 AND model IS NOT NULL",
            )?;
            let rows = q.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, i64>(6)?,
                    r.get::<_, i64>(7)?,
                    r.get::<_, String>(8)?,
                ))
            })?;
            for row in rows {
                let (provider, path, ord, ts, message_id, request_id, input, output, model) = row?;
                let Ok(ts) = DateTime::parse_from_rfc3339(&ts) else {
                    continue;
                };
                if !is_kind(&provider, ProviderKind::Claude) {
                    continue;
                }
                // Only models the catalog can price are worth re-reading logs for.
                let ok = *priceable
                    .entry(model.clone())
                    .or_insert_with(|| pricer(ProviderKind::Claude, &model, 0, 1, 0, 0).is_some());
                if ok {
                    by_path.entry(path.clone()).or_default().push(Row {
                        provider,
                        path,
                        ord,
                        key: (
                            ts.with_timezone(&Utc),
                            message_id,
                            request_id,
                            output as u64,
                        ),
                        input: input as u64,
                        model,
                    });
                }
            }
        }
        if by_path.is_empty() {
            return Ok(false);
        }

        let tx = self.conn.unchecked_transaction()?;
        let mut touched = BTreeSet::new();
        {
            let mut update = tx.prepare(
                "UPDATE usage_events SET estimated_cost_microusd=?4, priced_requests=requests, cache_savings_microusd=?5
                 WHERE provider=?1 AND path=?2 AND event_ord=?3 AND priced_requests=0",
            )?;
            for (path, rows) in by_path {
                let Ok(raw) = fs::read(&path) else { continue };
                let mut lines: HashMap<ClaudeKey, Vec<(u64, u64, u64)>> = HashMap::new();
                for line in raw.split(|&b| b == b'\n') {
                    if !line.windows(7).any(|w| w == b"\"usage\"") {
                        continue;
                    }
                    if let Some((key, tokens)) = claude_line(line) {
                        lines.entry(key).or_default().push(tokens);
                    }
                }
                for row in rows {
                    // The stored input is `input + cache creation + cache read`
                    // (older rows omit the cache read).
                    let Some(&(input, creation, cache_read)) = lines.get(&row.key).and_then(|c| {
                        c.iter()
                            .find(|t| t.0 + t.1 + t.2 == row.input || t.0 + t.1 == row.input)
                    }) else {
                        continue;
                    };
                    let Some((cost, savings)) = pricer(
                        ProviderKind::Claude,
                        &row.model,
                        creation,
                        input,
                        cache_read,
                        row.key.3,
                    ) else {
                        continue;
                    };
                    if update.execute(params![
                        row.provider,
                        row.path,
                        row.ord,
                        cost as i64,
                        savings as i64
                    ])? > 0
                    {
                        touched.insert(row.provider);
                    }
                }
            }
        }
        for provider in &touched {
            let id = ProviderId::lookup(provider)
                .unwrap_or_else(|| ProviderId::new(ProviderKind::Claude, provider));
            let cache = self.load_claude_cache(id)?;
            let stats = usage::statistics_from_claude_cache(&cache, 365);
            let model_daily = usage::aggregate_claude_model_daily(&cache)
                .into_iter()
                .map(|(date, model, usage)| (model, date, usage))
                .collect::<Vec<_>>();
            self.replace_usage_daily_in_transaction(id, &stats.daily)?;
            self.replace_usage_model_daily_in_transaction(id, &model_daily)?;
        }
        tx.commit()?;
        Ok(!touched.is_empty())
    }
}

/// Key and `(input, cache creation, cache read)` of an assistant line without
/// a reported `costUSD`, mirroring the scanner's field handling.
fn claude_line(line: &[u8]) -> Option<(ClaudeKey, (u64, u64, u64))> {
    let event: Value = serde_json::from_slice(line).ok()?;
    if event.get("type")?.as_str()? != "assistant"
        || event.get("costUSD").and_then(Value::as_f64).is_some()
    {
        return None;
    }
    let ts = DateTime::parse_from_rfc3339(event.get("timestamp")?.as_str()?)
        .ok()?
        .with_timezone(&Utc);
    let message = event.get("message")?;
    let usage = message.get("usage")?;
    let input = usage.get("input_tokens")?.as_u64()?;
    let output = usage.get("output_tokens")?.as_u64()?;
    let get = |value: Option<&Value>| value.and_then(Value::as_u64);
    let cache_read = get(usage.get("cache_read_input_tokens")).unwrap_or(0);
    let total_creation = get(usage.get("cache_creation_input_tokens")).unwrap_or(0);
    let details = usage.get("cache_creation");
    let w5 =
        get(details.and_then(|v| v.get("ephemeral_5m_input_tokens"))).unwrap_or(total_creation);
    let w1 = get(details.and_then(|v| v.get("ephemeral_1h_input_tokens"))).unwrap_or(0);
    let id = message.get("id").and_then(Value::as_str).map(str::to_owned);
    let request = event
        .get("requestId")
        .and_then(Value::as_str)
        .map(str::to_owned);
    Some((
        (ts, id, request, output),
        (input, w5.saturating_add(w1), cache_read),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use serde_json::json;
    use std::fs;

    fn store() -> ProviderStore {
        let store = ProviderStore {
            conn: Connection::open_in_memory().unwrap(),
        };
        store.migrate().unwrap();
        store
    }

    fn pricer() -> impl Fn(ProviderKind, &str, u64, u64, u64, u64) -> Option<(u64, u64)> {
        pricing::test_pricer(&json!({
            "gpt-6-luna": {"input_cost_per_token": 0.000001, "output_cost_per_token": 0.000004, "cache_read_input_token_cost": 0.0000001},
            "claude-x": {"input_cost_per_token": 0.000003, "output_cost_per_token": 0.000015, "cache_read_input_token_cost": 0.0000003, "cache_creation_input_token_cost": 0.00000375}
        }))
    }

    #[test]
    fn codex_unpriced_rows_are_priced_and_rollups_follow() {
        let s = store();
        s.conn
            .execute_batch(
                "INSERT INTO codex_names(id,name) VALUES(1,'gpt-6-luna'),(2,'mystery');
             INSERT INTO codex_events(session,ts,sig,account,date,model,input_tokens,cached_input_tokens,output_tokens,requests,estimated_cost_microusd,priced_requests,cache_savings_microusd)
             VALUES(1,1,1,1,'2026-09-22',1,1000,400,100,1,0,0,0),
                   (1,2,2,1,'2026-09-22',2,1000,0,100,1,0,0,0),
                   (1,3,3,1,'2026-09-22',1,1000,0,100,1,777,1,0);
             INSERT INTO usage_file_model_daily VALUES('codex','f','2026-09-22','gpt-6-luna',1000,400,100,1,0,0,0);
             INSERT INTO usage_file_daily(provider,path,date,input_tokens,cached_input_tokens,output_tokens,requests,estimated_cost_microusd,priced_requests,cache_savings_microusd)
               VALUES('codex','f','2026-09-22',1000,400,100,1,0,0,0);
             INSERT INTO usage_daily(provider,date,input_tokens,cached_input_tokens,output_tokens,requests,estimated_cost_microusd,priced_requests,cache_savings_microusd)
               VALUES('codex','2026-09-22',1000,400,100,1,0,0,0);
             INSERT INTO usage_model_daily VALUES('codex','2026-09-22','gpt-6-luna',1000,400,100,1,0,0,0);",
            )
            .unwrap();
        let p = pricer();
        assert!(s.reprice_unpriced_usage(&p).unwrap());
        let ev = |ts: i64| -> (i64, i64) {
            s.conn
                .query_row(
                    "SELECT estimated_cost_microusd, priced_requests FROM codex_events WHERE ts=?1",
                    [ts],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap()
        };
        // 600*1e-6 + 400*1e-7 + 100*4e-6 = 0.00104 USD
        assert_eq!(ev(1), (1040, 1));
        assert_eq!(ev(2), (0, 0));
        assert_eq!(ev(3), (777, 1));
        for table in ["usage_file_daily", "usage_daily", "usage_model_daily"] {
            let v: (i64, i64) = s
                .conn
                .query_row(
                    &format!("SELECT estimated_cost_microusd, priced_requests FROM {table}"),
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(v, (1040, 1), "{table}");
        }
        assert!(!s.reprice_unpriced_usage(&p).unwrap());
    }

    #[test]
    fn claude_unpriced_event_is_repriced_from_its_log_and_rollups_update() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("a.jsonl");
        let line = r#"{"type":"assistant","timestamp":"2026-09-22T10:00:00+00:00","requestId":"r1","message":{"id":"m1","model":"claude-x","usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":1000,"cache_creation_input_tokens":200}}}"#;
        fs::write(&log, format!("{line}\n")).unwrap();
        let path = log.to_string_lossy().into_owned();
        let s = store();
        let insert = "INSERT INTO usage_events(provider,path,event_ord,ts,message_id,request_id,is_sidechain,has_speed,input_tokens,cached_input_tokens,output_tokens,requests,estimated_cost_microusd,priced_requests,cache_savings_microusd,model)";
        s.conn
            .execute(
                &format!("{insert} VALUES('claude',?1,0,'2026-09-22T10:00:00+00:00','m1','r1',0,0,300,100,50,1,0,0,0,'claude-x')"),
                [&path],
            )
            .unwrap();
        s.conn
            .execute(
                &format!("{insert} VALUES('claude',?1,1,'2026-09-22T11:00:00+00:00','m2','r2',0,0,10,0,5,1,0,0,0,'<synthetic>')"),
                [&path],
            )
            .unwrap();
        let p = pricer();
        assert!(s.reprice_unpriced_usage(&p).unwrap());
        // 100*3e-6 + 1000*3e-7 + 200*3.75e-6 + 50*1.5e-5 = 0.0021 USD
        let (cost, priced): (i64, i64) = s
            .conn
            .query_row(
                "SELECT estimated_cost_microusd, priced_requests FROM usage_events WHERE event_ord=0",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((cost, priced), (2100, 1));
        let synthetic: i64 = s
            .conn
            .query_row(
                "SELECT priced_requests FROM usage_events WHERE event_ord=1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(synthetic, 0);
        let daily: i64 = s
            .conn
            .query_row(
                "SELECT SUM(estimated_cost_microusd) FROM usage_daily WHERE provider='claude'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let model: i64 = s
            .conn
            .query_row(
                "SELECT SUM(estimated_cost_microusd) FROM usage_model_daily WHERE provider='claude' AND model='claude-x'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!((daily, model), (2100, 2100));
    }
}
