//! Account attribution is an observation, not billing evidence: rollouts do
//! not identify the account. Never infer ownership from a plan or display name.
use super::*;
use crate::usage::codex_home;

pub(crate) const UNKNOWN: &str = "unattributed";
const STATE: &str = "codex.accounts.v1";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Identity {
    pub id: String,
    stamp: u128,
    #[serde(default)]
    session: Option<String>,
}

impl PartialEq for Identity {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && match (&self.session, &other.session) {
                (Some(a), Some(b)) => a == b,
                (None, None) => self.stamp == other.stamp,
                _ => false,
            }
    }
}
impl Eq for Identity {}

fn login_session(value: &serde_json::Value) -> Option<String> {
    use base64::Engine;
    let payload = value
        .pointer("/tokens/id_token")?
        .as_str()?
        .split('.')
        .nth(1)?;
    let claims: serde_json::Value = serde_json::from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .ok()?,
    )
    .ok()?;
    if let Some(id) = claims
        .get("sid")
        .and_then(|v| v.as_str())
        .filter(|v| !v.is_empty())
    {
        return Some(format!("sid:{id}"));
    }
    claims
        .get("auth_time")
        .and_then(|v| v.as_i64())
        .map(|at| format!("auth:{at}"))
}

pub(crate) fn identity() -> Option<Identity> {
    let path = codex_home().join("auth.json");
    let before = fs::metadata(&path).ok()?.modified().ok()?;
    let value: serde_json::Value = serde_json::from_slice(&fs::read(&path).ok()?).ok()?;
    let after = fs::metadata(&path).ok()?.modified().ok()?;
    if before != after {
        return None;
    }
    let id = value.pointer("/tokens/account_id")?.as_str()?.trim();
    if id.is_empty() {
        return None;
    }
    Some(Identity {
        id: id.to_owned(),
        stamp: after.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos(),
        session: login_session(&value),
    })
}

pub(crate) fn current_id() -> String {
    identity()
        .map(|identity| identity.id)
        .unwrap_or_else(|| UNKNOWN.into())
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Attribution {
    pub initialized_at: DateTime<Utc>,
    pub historical_owner: String,
    #[serde(default)]
    history_pending: bool,
    pub observed: Option<Identity>,
    pub observed_at: DateTime<Utc>,
    pub ready: bool,
    #[serde(default)]
    spans: Vec<ObservationSpan>,
}

#[derive(Clone, Serialize, Deserialize)]
struct ObservationSpan {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    account: String,
}

impl Attribution {
    pub(crate) fn owner(
        &self,
        at: DateTime<Utc>,
        before: &Option<Identity>,
        after: &Option<Identity>,
        end: DateTime<Utc>,
    ) -> &str {
        if at <= self.initialized_at {
            return &self.historical_owner;
        }
        if let Some(span) = self
            .spans
            .iter()
            .find(|span| span.start <= at && at <= span.end)
        {
            return &span.account;
        }
        if at >= self.observed_at
            && at <= end
            && self.observed.is_some()
            && &self.observed == before
            && before == after
        {
            return &self.observed.as_ref().unwrap().id;
        }
        UNKNOWN
    }

    fn advance(&mut self, before: &Option<Identity>, after: &Option<Identity>, end: DateTime<Utc>) {
        if self.observed.is_some()
            && &self.observed == before
            && before == after
            && end >= self.observed_at
        {
            let account = self.observed.as_ref().unwrap().id.clone();
            if let Some(last) = self
                .spans
                .last_mut()
                .filter(|last| last.account == account && last.end == self.observed_at)
            {
                last.end = end;
            } else {
                self.spans.push(ObservationSpan {
                    start: self.observed_at,
                    end,
                    account,
                });
            }
        }
        self.observed = after.clone();
        self.observed_at = end;
        self.ready = true;
    }
}

pub(crate) struct AccountEvent {
    pub source: String,
    pub offset: u64,
    pub session: String,
    pub timestamp: DateTime<Utc>,
    pub signature: String,
    pub model: String,
    pub usage: TokenUsage,
}

const VALUES: &str = "input_tokens,cached_input_tokens,output_tokens,requests,estimated_cost_microusd,priced_requests,cache_savings_microusd";
const SUMS: &str = "SUM(input_tokens), SUM(cached_input_tokens), SUM(output_tokens), SUM(requests), SUM(estimated_cost_microusd), SUM(priced_requests), SUM(cache_savings_microusd)";

impl ProviderStore {
    pub(super) fn migrate_codex_accounts(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS codex_account_events (
            account TEXT NOT NULL, session TEXT NOT NULL, ts INTEGER NOT NULL,
            signature TEXT NOT NULL, date TEXT NOT NULL, model TEXT NOT NULL,
            input_tokens INTEGER NOT NULL, cached_input_tokens INTEGER NOT NULL,
            output_tokens INTEGER NOT NULL, requests INTEGER NOT NULL,
            estimated_cost_microusd INTEGER NOT NULL, priced_requests INTEGER NOT NULL,
            cache_savings_microusd INTEGER NOT NULL,
            PRIMARY KEY(session, ts, signature));
            CREATE INDEX IF NOT EXISTS codex_account_date ON codex_account_events(account,date);
            CREATE INDEX IF NOT EXISTS codex_account_time ON codex_account_events(account,ts);",
        )?;
        self.ensure_column(
            "codex_account_events",
            "covered",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        for (kind, columns) in [
            ("daily", "date"),
            ("model_daily", "date,model"),
            ("hourly", "hour"),
        ] {
            self.conn.execute_batch(&format!("CREATE TABLE IF NOT EXISTS codex_legacy_{kind} AS SELECT CAST('' AS TEXT) AS account,{columns},{VALUES} FROM usage_{kind} WHERE 0;"))?;
        }
        self.conn.execute_batch("CREATE TABLE IF NOT EXISTS codex_legacy_cursors(source TEXT PRIMARY KEY, offset INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS codex_legacy_sessions(account TEXT NOT NULL,session TEXT NOT NULL,date TEXT NOT NULL,PRIMARY KEY(account,session,date));")?;
        Ok(())
    }

    pub(crate) fn codex_attribution(&self) -> Result<Option<Attribution>> {
        self.conn
            .query_row("SELECT value FROM meta WHERE key=?1", [STATE], |r| {
                r.get::<_, String>(0)
            })
            .optional()?
            .map(|s| serde_json::from_str(&s).context("read Codex account attribution"))
            .transpose()
    }

    pub(crate) fn use_codex_account_data(&self) -> Result<bool> {
        Ok(self
            .codex_attribution()?
            .is_some_and(|state| state.ready || state.historical_owner != current_id()))
    }

    pub(crate) fn initialize_codex_attribution(&self) -> Result<Attribution> {
        self.initialize_codex_attribution_at(identity(), Utc::now())
    }

    fn initialize_codex_attribution_at(
        &self,
        observed: Option<Identity>,
        now: DateTime<Utc>,
    ) -> Result<Attribution> {
        // Serialize the first snapshot across connections/processes: checking
        // the marker outside the write transaction could migrate it twice.
        let tx = rusqlite::Transaction::new_unchecked(
            &self.conn,
            rusqlite::TransactionBehavior::Immediate,
        )?;
        if let Some(mut state) = self.codex_attribution()? {
            // An upgrade while logged out must not strand a single user's
            // existing history. Bind that snapshot once, at the first login.
            if state.history_pending
                && let Some(identity) = observed.as_ref()
            {
                for table in [
                    "codex_legacy_daily",
                    "codex_legacy_model_daily",
                    "codex_legacy_hourly",
                    "codex_legacy_sessions",
                ] {
                    tx.execute(
                        &format!("UPDATE {table} SET account=?1 WHERE account=?2"),
                        params![identity.id, state.historical_owner],
                    )?;
                }
                tx.execute(
                    "UPDATE codex_account_events SET account=?1 WHERE account=?2 AND ts<=?3",
                    params![
                        identity.id,
                        state.historical_owner,
                        state.initialized_at.timestamp_millis()
                    ],
                )?;
                state.historical_owner = identity.id.clone();
                state.history_pending = false;
                self.set_meta(STATE, &serde_json::to_string(&state)?)?;
            }
            // Upgrade early account caches without discarding known ownership.
            if state.ready && state.spans.is_empty() {
                let mut query=self.conn.prepare("SELECT ts,account FROM codex_account_events WHERE ts>?1 AND account<>?2 ORDER BY ts")?;
                for row in query.query_map(
                    params![state.initialized_at.timestamp_millis(), UNKNOWN],
                    |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
                )? {
                    let (at, account) = row?;
                    let at = Utc
                        .timestamp_millis_opt(at)
                        .single()
                        .context("invalid attribution timestamp")?;
                    state.spans.push(ObservationSpan {
                        start: at,
                        end: at,
                        account,
                    });
                }
                self.set_meta(STATE, &serde_json::to_string(&state)?)?;
            }
            tx.commit()?;
            return Ok(state);
        }
        // Optional one-time migration override. Never shipped with a user's ID.
        let owner: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key='codex.history_owner'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        let state = Attribution {
            initialized_at: now,
            history_pending: owner.is_none() && observed.is_none(),
            historical_owner: owner.unwrap_or_else(|| {
                observed
                    .as_ref()
                    .map(|i| i.id.clone())
                    .unwrap_or_else(|| UNKNOWN.into())
            }),
            observed,
            observed_at: now,
            ready: false,
            spans: Vec::new(),
        };
        // Freeze the existing cache before any rollout is rescanned. It remains
        // available even if old files were deleted, archived or are inaccessible.
        for (kind, columns) in [
            ("daily", "date"),
            ("model_daily", "date,model"),
            ("hourly", "hour"),
        ] {
            tx.execute(&format!("INSERT INTO codex_legacy_{kind} SELECT ?1,{columns},{VALUES} FROM usage_{kind} WHERE provider='codex'"), [&state.historical_owner])?;
        }
        {
            let mut paths = BTreeMap::new();
            let mut query =
                tx.prepare("SELECT path,offset,meta_json FROM scan_files WHERE provider='codex'")?;
            let files = query.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;
            for file in files {
                let (path, offset, meta) = file?;
                let source = Path::new(&path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                let meta: serde_json::Value =
                    serde_json::from_str(&meta).context("read legacy Codex file metadata")?;
                let session = meta
                    .get("session_id")
                    .and_then(|id| id.as_str())
                    .filter(|id| !id.is_empty())
                    .unwrap_or(&source)
                    .to_owned();
                tx.execute("INSERT INTO codex_legacy_cursors VALUES(?1,?2) ON CONFLICT(source) DO UPDATE SET offset=MAX(offset,excluded.offset)", params![source,offset])?;
                paths.insert(path, session);
            }
            let mut query =
                tx.prepare("SELECT path,date FROM usage_file_daily WHERE provider='codex'")?;
            for row in query.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })? {
                let (path, date) = row?;
                let session = paths.get(&path).cloned().unwrap_or(path);
                tx.execute(
                    "INSERT OR IGNORE INTO codex_legacy_sessions VALUES(?1,?2,?3)",
                    params![state.historical_owner, session, date],
                )?;
            }
        }
        self.set_meta(STATE, &serde_json::to_string(&state)?)?;
        tx.commit()?;
        Ok(state)
    }

    pub(crate) fn save_account_events(
        &self,
        events: &[AccountEvent],
        state: &Attribution,
        before: &Option<Identity>,
        after: &Option<Identity>,
        end: DateTime<Utc>,
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut query = tx.prepare("SELECT source,offset FROM codex_legacy_cursors")?;
            let cursors = query
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
                })?
                .collect::<rusqlite::Result<BTreeMap<_, _>>>()?;
            let mut insert = tx.prepare("INSERT INTO codex_account_events VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
                ON CONFLICT(session,ts,signature) DO UPDATE SET
                    model=excluded.model,input_tokens=excluded.input_tokens,cached_input_tokens=excluded.cached_input_tokens,
                    output_tokens=excluded.output_tokens,requests=excluded.requests,estimated_cost_microusd=excluded.estimated_cost_microusd,
                    priced_requests=excluded.priced_requests,cache_savings_microusd=excluded.cache_savings_microusd
                WHERE model IS NOT excluded.model OR input_tokens IS NOT excluded.input_tokens OR cached_input_tokens IS NOT excluded.cached_input_tokens
                    OR output_tokens IS NOT excluded.output_tokens OR requests IS NOT excluded.requests OR estimated_cost_microusd IS NOT excluded.estimated_cost_microusd
                    OR priced_requests IS NOT excluded.priced_requests OR cache_savings_microusd IS NOT excluded.cache_savings_microusd")?;
            for e in events {
                let u = &e.usage;
                insert.execute(params![
                    state.owner(e.timestamp, before, after, end),
                    e.session,
                    e.timestamp.timestamp_millis(),
                    e.signature,
                    e.timestamp.with_timezone(&Local).date_naive().to_string(),
                    e.model,
                    u.input_tokens as i64,
                    u.cached_input_tokens as i64,
                    u.output_tokens as i64,
                    u.requests as i64,
                    u.estimated_cost_microusd as i64,
                    u.priced_requests as i64,
                    u.cache_savings_microusd as i64,
                    e.timestamp <= state.initialized_at
                        && cursors
                            .get(&e.source)
                            .is_some_and(|offset| e.offset <= *offset)
                ])?;
            }
        }
        let mut updated = state.clone();
        updated.advance(before, after, end);
        tx.execute("INSERT INTO meta(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![STATE,serde_json::to_string(&updated)?])?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn account_statistics_for(
        &self,
        account: &str,
        days: u16,
    ) -> Result<UsageStatistics> {
        let today = Local::now().date_naive();
        let start = today - Duration::days(i64::from(days.clamp(1, 365) - 1));
        let mut query=self.conn.prepare(&format!("SELECT date,{SUMS} FROM (
            SELECT date,{VALUES} FROM codex_account_events WHERE account=?1 AND date>=?2 AND date<=?3 AND covered=0
            UNION ALL SELECT date,{VALUES} FROM codex_legacy_daily WHERE account=?1 AND date>=?2 AND date<=?3
            ) GROUP BY date ORDER BY date"))?;
        let rows = query
            .query_map(
                params![account, start.to_string(), today.to_string()],
                |row| {
                    Ok(DailyTokenUsage {
                        date: parse_date(&row.get::<_, String>(0)?),
                        usage: token_usage_from_row(row, 1)?,
                    })
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut stats = statistics_from_daily(&rows, days);
        stats.account_id = Some(account.into());
        Ok(stats)
    }

    pub(crate) fn account_daily_for(
        &self,
        account: &str,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<(String, NaiveDate, TokenUsage)>> {
        let mut stmt = self.conn.prepare(&format!("SELECT model,date,{SUMS} FROM (
            SELECT model,date,{VALUES} FROM codex_account_events WHERE account=?1 AND date>=?2 AND date<=?3 AND covered=0
            UNION ALL SELECT model,date,{VALUES} FROM codex_legacy_model_daily WHERE account=?1 AND date>=?2 AND date<=?3
            ) GROUP BY date,model ORDER BY date,model"))?;
        let rows = stmt.query_map(params![account, start.to_string(), end.to_string()], |r| {
            Ok((
                r.get(0)?,
                r.get::<_, String>(1)?,
                token_usage_from_row(r, 2)?,
            ))
        })?;
        rows.map(|r| {
            let (model, date, usage) = r?;
            Ok((model, NaiveDate::parse_from_str(&date, "%Y-%m-%d")?, usage))
        })
        .collect()
    }

    pub(super) fn account_hourly(
        &self,
        start: DateTime<Local>,
        end: DateTime<Local>,
    ) -> Result<BTreeMap<DateTime<Local>, TokenUsage>> {
        self.account_hourly_for(&current_id(), start, end)
    }

    fn account_hourly_for(
        &self,
        account: &str,
        start: DateTime<Local>,
        end: DateTime<Local>,
    ) -> Result<BTreeMap<DateTime<Local>, TokenUsage>> {
        let mut stmt = self.conn.prepare("SELECT ts,input_tokens,cached_input_tokens,output_tokens,requests,estimated_cost_microusd,priced_requests,cache_savings_microusd,covered FROM codex_account_events WHERE account=?1 AND ts>=?2 AND ts<?3")?;
        let rows = stmt.query_map(
            params![
                account,
                start.timestamp_millis(),
                (end + Duration::hours(1)).timestamp_millis()
            ],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    token_usage_from_row(r, 1)?,
                    r.get::<_, bool>(8)?,
                ))
            },
        )?;
        let mut covered_hours = BTreeMap::<DateTime<Local>, TokenUsage>::new();
        let mut result = BTreeMap::<DateTime<Local>, TokenUsage>::new();
        for row in rows {
            let (at, usage, covered) = row?;
            let at = Utc
                .timestamp_millis_opt(at)
                .single()
                .context("invalid usage timestamp")?;
            let hours = if covered {
                &mut covered_hours
            } else {
                &mut result
            };
            hours
                .entry(truncate_local_hour(at.with_timezone(&Local)))
                .or_default()
                .add(&usage);
        }
        let mut query = self.conn.prepare(&format!(
            "SELECT hour,{VALUES} FROM codex_legacy_hourly WHERE account=?1"
        ))?;
        for row in query.query_map([account], |r| {
            Ok((r.get::<_, String>(0)?, token_usage_from_row(r, 1)?))
        })? {
            let (hour, usage) = row?;
            let hour = DateTime::parse_from_rfc3339(&hour)?.with_timezone(&Local);
            if hour >= start && hour <= end {
                // Prefer the saved snapshot for the covered portion, including
                // sessions whose files are no longer available. New tail events
                // are kept separately and added exactly once.
                covered_hours.insert(hour, usage);
            }
        }
        for (hour, usage) in covered_hours {
            result.entry(hour).or_default().add(&usage);
        }
        Ok(result)
    }

    pub(super) fn account_sessions(&self, start: NaiveDate, end: NaiveDate) -> Result<u64> {
        self.account_sessions_for(&current_id(), start, end)
    }

    fn account_sessions_for(&self, account: &str, start: NaiveDate, end: NaiveDate) -> Result<u64> {
        Ok(self.conn.query_row("SELECT COUNT(DISTINCT session) FROM (SELECT session FROM codex_account_events WHERE account=?1 AND date>=?2 AND date<=?3 AND covered=0 UNION SELECT session FROM codex_legacy_sessions WHERE account=?1 AND date>=?2 AND date<=?3)",params![account,start.to_string(),end.to_string()], |r| r.get::<_,i64>(0))? as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(seconds, 0).unwrap()
    }
    fn identity_for(id: &str, stamp: u128) -> Option<Identity> {
        Some(Identity {
            id: id.into(),
            stamp,
            session: None,
        })
    }
    fn state() -> Attribution {
        Attribution {
            initialized_at: at(100),
            historical_owner: "old".into(),
            history_pending: false,
            observed: identity_for("new", 1),
            observed_at: at(110),
            ready: true,
            spans: Vec::new(),
        }
    }
    fn event(seconds: i64, tokens: u64) -> AccountEvent {
        AccountEvent {
            source: "same-conversation.jsonl".into(),
            offset: 200,
            session: "same-conversation".into(),
            timestamp: at(seconds),
            signature: format!("usage-{tokens}"),
            model: "model-a".into(),
            usage: TokenUsage {
                input_tokens: tokens,
                cached_input_tokens: tokens / 2,
                requests: 1,
                estimated_cost_microusd: tokens * 3,
                priced_requests: 1,
                ..Default::default()
            },
        }
    }
    fn store() -> ProviderStore {
        let store = ProviderStore {
            conn: Connection::open_in_memory().unwrap(),
        };
        store.migrate().unwrap();
        store
    }
    #[test]
    fn token_refresh_keeps_identity_but_a_new_login_does_not() {
        let a = Identity {
            id: "a".into(),
            stamp: 1,
            session: Some("login-a".into()),
        };
        let mut refreshed = a.clone();
        refreshed.stamp = 2;
        assert_eq!(a, refreshed);
        refreshed.session = Some("login-b".into());
        assert_ne!(a, refreshed);
        refreshed.session = None;
        refreshed.stamp = 1;
        assert_ne!(a, refreshed);
        use base64::Engine;
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(br#"{"sid":"login-a","auth_time":123}"#);
        assert_eq!(
            login_session(&json!({"tokens":{"id_token":format!("header.{payload}.signature")}})),
            Some("sid:login-a".into())
        );
        assert_eq!(
            login_session(&json!({"tokens":{"id_token":"malformed"}})),
            None
        );
    }

    #[test]
    fn single_account_upgrade_preserves_cache_without_source_files() {
        let db = store();
        let today = Local::now().date_naive();
        let yesterday = today - Duration::days(1);
        let old = event(90, 100).usage;
        db.replace_usage_daily(
            ProviderKind::Codex,
            &[DailyTokenUsage {
                date: yesterday,
                usage: old.clone(),
            }],
        )
        .unwrap();
        db.replace_usage_model_daily(
            ProviderKind::Codex,
            &[("model-a".into(), yesterday, old.clone())],
        )
        .unwrap();
        let hour = truncate_local_hour(Local::now() - Duration::hours(2));
        db.replace_usage_hourly(ProviderKind::Codex, &[(hour, old.clone())])
            .unwrap();
        let state = db
            .initialize_codex_attribution_at(identity_for("only-account", 1), Utc::now())
            .unwrap();
        assert_eq!(state.historical_owner, "only-account");
        // The scanner no longer finds the original file, so the old raw cache
        // is rebuilt empty. Its migration snapshot must still be visible.
        db.replace_usage_daily(ProviderKind::Codex, &[]).unwrap();
        db.replace_usage_model_daily(ProviderKind::Codex, &[])
            .unwrap();
        db.save_account_events(&[], &state, &state.observed, &state.observed, Utc::now())
            .unwrap();
        assert_eq!(
            db.account_statistics_for("only-account", 30)
                .unwrap()
                .history,
            old
        );
        assert_eq!(
            db.account_daily_for("only-account", yesterday, today)
                .unwrap()[0]
                .2,
            old
        );
        assert_eq!(
            db.account_hourly_for("only-account", hour, hour).unwrap()[&hour],
            old
        );
        assert_eq!(
            db.account_statistics_for("another-account", 30)
                .unwrap()
                .history,
            TokenUsage::default()
        );
        let again = db
            .initialize_codex_attribution_at(identity_for("another-account", 2), Utc::now())
            .unwrap();
        assert_eq!(again.historical_owner, "only-account");
        assert_eq!(
            db.account_statistics_for("only-account", 30)
                .unwrap()
                .history,
            old
        );
    }

    #[test]
    fn concurrent_initialization_copies_the_legacy_snapshot_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("concurrent.sqlite");
        let db = ProviderStore {
            conn: Connection::open(&path).unwrap(),
        };
        db.migrate().unwrap();
        let day = Local::now().date_naive();
        db.replace_usage_daily(
            ProviderKind::Codex,
            &[DailyTokenUsage {
                date: day,
                usage: event(90, 100).usage,
            }],
        )
        .unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let handles: Vec<_> = (0..2)
            .map(|i| {
                let path = path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let db = ProviderStore {
                        conn: Connection::open(path).unwrap(),
                    };
                    barrier.wait();
                    db.initialize_codex_attribution_at(
                        identity_for(&format!("account-{i}"), 1),
                        Utc::now(),
                    )
                    .unwrap()
                    .historical_owner
                })
            })
            .collect();
        let owners: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(owners[0], owners[1]);
        assert_eq!(
            db.account_statistics_for(&owners[0], 30)
                .unwrap()
                .today
                .input_tokens,
            100
        );
        assert_eq!(
            db.conn
                .query_row("SELECT COUNT(*) FROM codex_legacy_daily", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn logged_out_upgrade_binds_only_legacy_history_at_first_login() {
        let db = store();
        let initial = db.initialize_codex_attribution_at(None, at(100)).unwrap();
        assert!(initial.history_pending);
        db.save_account_events(
            &[event(90, 100), event(105, 7)],
            &initial,
            &None,
            &None,
            at(110),
        )
        .unwrap();
        let adopted = db
            .initialize_codex_attribution_at(identity_for("first-account", 1), at(120))
            .unwrap();
        assert_eq!(adopted.historical_owner, "first-account");
        assert!(!adopted.history_pending);
        assert_eq!(
            db.account_daily_for("first-account", at(0).date_naive(), at(200).date_naive())
                .unwrap()[0]
                .2
                .input_tokens,
            100
        );
        assert_eq!(
            db.account_daily_for(UNKNOWN, at(0).date_naive(), at(200).date_naive())
                .unwrap()[0]
                .2
                .input_tokens,
            7
        );
        let later = db
            .initialize_codex_attribution_at(identity_for("second-account", 2), at(130))
            .unwrap();
        assert_eq!(later.historical_owner, "first-account");
    }

    #[test]
    fn replay_of_migration_snapshot_adds_only_new_tail_once() {
        let db = store();
        let now = Utc::now();
        let day = now.with_timezone(&Local).date_naive();
        let mut old = event(90, 100);
        old.timestamp = now - Duration::seconds(20);
        let mut tail = event(95, 10);
        tail.timestamp = now - Duration::seconds(10);
        tail.offset = 300;
        db.replace_usage_daily(
            ProviderKind::Codex,
            &[DailyTokenUsage {
                date: day,
                usage: old.usage.clone(),
            }],
        )
        .unwrap();
        db.replace_usage_model_daily(
            ProviderKind::Codex,
            &[(old.model.clone(), day, old.usage.clone())],
        )
        .unwrap();
        db.conn
            .execute(
                "INSERT INTO scan_files VALUES('codex',?1,200,?2)",
                params![old.source, json!({"session_id":old.session}).to_string()],
            )
            .unwrap();
        db.conn.execute(&format!("INSERT INTO usage_file_daily SELECT provider,?1,date,{VALUES} FROM usage_daily WHERE provider='codex'"),[&old.source]).unwrap();
        let state = db
            .initialize_codex_attribution_at(identity_for("only-account", 1), now)
            .unwrap();
        let events = [old, tail];
        for _ in 0..2 {
            db.save_account_events(&events, &state, &state.observed, &state.observed, now)
                .unwrap();
            assert_eq!(
                db.account_statistics_for("only-account", 30)
                    .unwrap()
                    .today
                    .input_tokens,
                110
            );
            assert_eq!(
                db.account_daily_for("only-account", day, day).unwrap()[0]
                    .2
                    .input_tokens,
                110
            );
            assert_eq!(
                db.account_sessions_for("only-account", day, day).unwrap(),
                1
            );
        }
        let hour = truncate_local_hour(now.with_timezone(&Local));
        assert_eq!(
            db.account_hourly_for("only-account", hour - Duration::hours(1), hour)
                .unwrap()
                .values()
                .map(|u| u.input_tokens)
                .sum::<u64>(),
            110
        );
    }

    #[test]
    fn clearing_and_replaying_preserves_observed_account_ownership() {
        let db = store();
        let state = state();
        let a = state.observed.clone();
        db.save_account_events(&[event(90, 100), event(115, 10)], &state, &a, &a, at(120))
            .unwrap();
        let state = db.codex_attribution().unwrap().unwrap();
        let b = identity_for("third", 2);
        // A switch itself is ambiguous; the next stable interval belongs to B.
        db.save_account_events(&[], &state, &b, &b, at(125))
            .unwrap();
        let state = db.codex_attribution().unwrap().unwrap();
        db.save_account_events(&[event(128, 7)], &state, &b, &b, at(130))
            .unwrap();
        db.clear_usage_data().unwrap();
        let state = db.codex_attribution().unwrap().unwrap();
        assert!(!state.ready);
        db.save_account_events(
            &[event(90, 100), event(115, 10), event(128, 7)],
            &state,
            &b,
            &b,
            at(140),
        )
        .unwrap();
        for (owner, tokens) in [("old", 100), ("new", 10), ("third", 7)] {
            assert_eq!(
                db.account_daily_for(owner, at(0).date_naive(), at(200).date_naive())
                    .unwrap()[0]
                    .2
                    .input_tokens,
                tokens
            );
        }
    }

    #[test]
    fn repricing_updates_values_without_reassigning_the_account() {
        let db = store();
        let state = state();
        let a = state.observed.clone();
        db.save_account_events(&[event(115, 10)], &state, &a, &a, at(120))
            .unwrap();
        let mut repriced = event(115, 10);
        repriced.usage.estimated_cost_microusd = 900;
        db.save_account_events(&[repriced], &state, &None, &None, at(130))
            .unwrap();
        let rows = db
            .account_daily_for("new", at(0).date_naive(), at(200).date_naive())
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].2.estimated_cost_microusd, 900);
        assert!(
            db.account_daily_for(UNKNOWN, at(0).date_naive(), at(200).date_naive())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    #[ignore = "reads local rollouts; requires a disposable database copy in CODEX_ACCOUNT_TEST_DB"]
    fn replay_local_history_in_disposable_database() {
        let path =
            std::env::var("CODEX_ACCOUNT_TEST_DB").expect("disposable database copy required");
        assert!(path.ends_with("account-migration-test.sqlite"));
        let db = ProviderStore {
            conn: Connection::open(&path).unwrap(),
        };
        db.migrate().unwrap();
        let state = db.initialize_codex_attribution().unwrap();
        assert_eq!(state.historical_owner, "legacy:previous-account");
        let before = identity();
        let mut events = Vec::new();
        let mut expected = TokenUsage::default();
        let from = Local::now().date_naive() - Duration::days(364);
        let to = Local::now().date_naive() - Duration::days(1);
        for (path, _) in crate::usage::collect_codex_session_files(&codex_home()).unwrap() {
            let mut file = crate::usage::CachedSessionFile::default();
            events.extend(crate::usage::scan_file_delta(&path, &mut file).unwrap());
            for day in file.daily {
                if day.date >= from && day.date <= to {
                    expected.add(&day.usage);
                }
            }
        }
        let after = identity();
        db.save_account_events(&events, &state, &before, &after, Utc::now())
            .unwrap();
        let rows = db
            .account_daily_for(&state.historical_owner, from, to)
            .unwrap();
        let mut total = TokenUsage::default();
        for (_, _, usage) in rows {
            total.add(&usage);
        }
        // The test scanner deliberately has no live pricing catalog. The
        // migration must preserve cached prices rather than replace them with
        // that unpriced replay, while matching all token/request counters.
        assert_eq!(total.input_tokens, expected.input_tokens);
        assert_eq!(total.cached_input_tokens, expected.cached_input_tokens);
        assert_eq!(total.output_tokens, expected.output_tokens);
        assert_eq!(total.requests, expected.requests);
        let cached_price:i64=db.conn.query_row("SELECT COALESCE(SUM(estimated_cost_microusd),0) FROM usage_daily WHERE provider='codex' AND date>=?1 AND date<=?2",params![from.to_string(),to.to_string()],|r|r.get(0)).unwrap();
        assert!(total.estimated_cost_microusd >= cached_price as u64);
        assert!(
            db.account_daily_for(&current_id(), from, to)
                .unwrap()
                .is_empty()
        );
        let count = db
            .conn
            .query_row("SELECT COUNT(*) FROM codex_account_events", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        db.save_account_events(&events, &state, &None, &None, Utc::now())
            .unwrap();
        assert_eq!(
            count,
            db.conn
                .query_row("SELECT COUNT(*) FROM codex_account_events", [], |r| r
                    .get::<_, i64>(0))
                .unwrap()
        );
        assert_eq!(
            db.conn
                .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
        println!(
            "Migrated {count} events; historical tokens={}; active account has no historical events; replay unchanged",
            total.total_tokens()
        );
    }
    #[test]
    fn ownership_keeps_history_and_rejects_ambiguous_intervals() {
        let s = state();
        let new = identity_for("new", 1);
        assert_eq!(s.owner(at(90), &new, &new, at(120)), "old");
        assert_eq!(s.owner(at(115), &new, &new, at(120)), "new");
        assert_eq!(s.owner(at(105), &new, &new, at(120)), UNKNOWN);
        assert_eq!(s.owner(at(121), &new, &new, at(120)), UNKNOWN);
        assert_eq!(
            s.owner(at(115), &new, &identity_for("another", 2), at(120)),
            UNKNOWN
        );
        // Even A -> B -> A or a token refresh is treated conservatively.
        assert_eq!(
            s.owner(
                at(115),
                &identity_for("new", 2),
                &identity_for("new", 2),
                at(120)
            ),
            UNKNOWN
        );
        assert_eq!(s.owner(at(115), &None, &None, at(120)), UNKNOWN);
    }
    #[test]
    fn events_are_idempotent_and_never_reassigned_on_replay() {
        let db = store();
        let s = state();
        let new = identity_for("new", 1);
        db.save_account_events(&[event(90, 100), event(115, 10)], &s, &new, &new, at(120))
            .unwrap();
        let mut switched = db.codex_attribution().unwrap().unwrap();
        switched.observed = identity_for("third", 2);
        db.save_account_events(
            &[event(90, 100), event(115, 10), event(125, 7)],
            &switched,
            &switched.observed,
            &switched.observed,
            at(130),
        )
        .unwrap();
        for (owner, total) in [("old", 100), ("new", 10), ("third", 7)] {
            let rows = db
                .account_daily_for(owner, at(0).date_naive(), at(200).date_naive())
                .unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].2.input_tokens, total);
            assert_eq!(rows[0].2.cached_input_tokens, total / 2);
            assert_eq!(rows[0].2.estimated_cost_microusd, total * 3);
        }
        assert_eq!(
            db.conn
                .query_row("SELECT COUNT(*) FROM codex_account_events", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            3
        );
    }
    #[test]
    fn failed_commit_keeps_events_and_checkpoint_atomic() {
        let db = store();
        let s = state();
        db.set_meta(STATE, &serde_json::to_string(&s).unwrap())
            .unwrap();
        db.conn.execute_batch("CREATE TRIGGER reject_checkpoint BEFORE UPDATE ON meta BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
        assert!(
            db.save_account_events(&[event(115, 10)], &s, &s.observed, &s.observed, at(120))
                .is_err()
        );
        assert_eq!(
            db.conn
                .query_row("SELECT COUNT(*) FROM codex_account_events", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            db.codex_attribution().unwrap().unwrap().observed_at,
            at(110)
        );
    }
    #[test]
    fn migration_override_is_one_time_and_persists_across_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.sqlite");
        let db = ProviderStore {
            conn: Connection::open(&path).unwrap(),
        };
        db.migrate().unwrap();
        db.set_meta("codex.history_owner", "previous-account")
            .unwrap();
        let initial = db.initialize_codex_attribution().unwrap();
        assert_eq!(initial.historical_owner, "previous-account");
        drop(db);
        let db = ProviderStore {
            conn: Connection::open(&path).unwrap(),
        };
        db.migrate().unwrap();
        db.set_meta("codex.history_owner", "wrong-account").unwrap();
        let restored = db.initialize_codex_attribution().unwrap();
        assert_eq!(restored.historical_owner, "previous-account");
        assert_eq!(restored.initialized_at, initial.initialized_at);
    }
}
