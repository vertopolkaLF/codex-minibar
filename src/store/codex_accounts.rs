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

#[derive(Debug, PartialEq, Eq)]
enum Observation {
    Account(Identity),
    LoggedOut,
    Uncertain,
}

static LAST_ID: Mutex<Option<String>> = Mutex::new(None);

fn remember(id: Option<String>) {
    if let Ok(mut last) = LAST_ID.lock() {
        *last = id;
    }
}

fn last_id() -> Option<String> {
    LAST_ID.lock().ok().and_then(|last| last.clone())
}

fn observe_auth(path: &Path) -> Observation {
    let before = match fs::metadata(path).and_then(|meta| meta.modified()) {
        Ok(modified) => modified,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Observation::LoggedOut;
        }
        Err(_) => return Observation::Uncertain,
    };
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Observation::LoggedOut;
        }
        Err(_) => return Observation::Uncertain,
    };
    let after = match fs::metadata(path).and_then(|meta| meta.modified()) {
        Ok(modified) => modified,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Observation::LoggedOut;
        }
        Err(_) => return Observation::Uncertain,
    };
    if before != after {
        return Observation::Uncertain;
    }
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return Observation::Uncertain;
    };
    let Some(id) = value
        .pointer("/tokens/account_id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|id| !id.is_empty())
    else {
        return Observation::LoggedOut;
    };
    let Some(stamp) = after.duration_since(std::time::UNIX_EPOCH).ok() else {
        return Observation::Uncertain;
    };
    Observation::Account(Identity {
        id: id.to_owned(),
        stamp: stamp.as_nanos(),
        session: login_session(&value),
    })
}

/// Raw observation for attribution. `None` is both logged-out and a torn
/// `auth.json` read; callers must not treat that as a new account.
pub(crate) fn identity() -> Option<Identity> {
    match observe_auth(&codex_home().join("auth.json")) {
        Observation::Account(identity) => Some(identity),
        Observation::LoggedOut | Observation::Uncertain => None,
    }
}

pub(crate) fn current_id() -> String {
    id_from_observation(observe_auth(&codex_home().join("auth.json")))
}

/// Stable identity for the usage worker. `None` means the file was unreadably
/// mid-update; keep the previous account instead of flashing logged-out.
pub(crate) fn poll_identity() -> Option<String> {
    poll_from_observation(observe_auth(&codex_home().join("auth.json")))
}

fn id_from_observation(observed: Observation) -> String {
    match observed {
        Observation::Account(identity) => {
            remember(Some(identity.id.clone()));
            identity.id
        }
        Observation::LoggedOut => {
            remember(None);
            UNKNOWN.into()
        }
        Observation::Uncertain => last_id().unwrap_or_else(|| UNKNOWN.into()),
    }
}

fn poll_from_observation(observed: Observation) -> Option<String> {
    match observed {
        Observation::Account(identity) => {
            remember(Some(identity.id.clone()));
            Some(identity.id)
        }
        Observation::LoggedOut => {
            remember(None);
            Some(UNKNOWN.into())
        }
        Observation::Uncertain => None,
    }
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
        if let Some(observed) = &self.observed
            && at >= self.observed_at
            && at <= end
            && &self.observed == before
            && before == after
        {
            return &observed.id;
        }
        UNKNOWN
    }

    fn advance(&mut self, before: &Option<Identity>, after: &Option<Identity>, end: DateTime<Utc>) {
        if let Some(observed) = &self.observed
            && &self.observed == before
            && before == after
            && end >= self.observed_at
        {
            let account = observed.id.clone();
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

#[derive(Clone, Debug)]
pub(crate) struct QuotaSampleEvent {
    pub source: String,
    pub offset: u64,
    pub timestamp: DateTime<Utc>,
    pub limit_id: String,
    pub window_minutes: u32,
    pub resets_at: Option<DateTime<Utc>>,
    pub used_percent: u8,
}

const RESET_TOLERANCE_SECONDS: i64 = 5 * 60;

#[derive(Clone, Debug)]
struct AttributedQuotaSample {
    account: String,
    source: String,
    offset: u64,
    timestamp: DateTime<Utc>,
    limit_id: String,
    window_minutes: u32,
    resets_at: DateTime<Utc>,
    used_percent: u8,
    baseline: bool,
}

#[derive(Default)]
struct QuotaCycleSamples {
    account: String,
    limit_id: String,
    window_minutes: u32,
    reset_at: i64,
    samples: Vec<AttributedQuotaSample>,
}

fn group_quota_samples(samples: Vec<AttributedQuotaSample>) -> Vec<QuotaCycleSamples> {
    let mut cycles: Vec<QuotaCycleSamples> = Vec::new();
    for sample in samples {
        let reset_at = sample.resets_at.timestamp();
        if let Some(cycle) = cycles.iter_mut().find(|cycle| {
            cycle.account == sample.account
                && cycle.limit_id == sample.limit_id
                && cycle.window_minutes == sample.window_minutes
                && (cycle.reset_at - reset_at).abs() <= RESET_TOLERANCE_SECONDS
        }) {
            cycle.samples.push(sample);
        } else {
            cycles.push(QuotaCycleSamples {
                account: sample.account.clone(),
                limit_id: sample.limit_id.clone(),
                window_minutes: sample.window_minutes,
                reset_at,
                samples: vec![sample],
            });
        }
    }
    cycles
}

fn compact_quota_progression(
    mut samples: Vec<AttributedQuotaSample>,
) -> Vec<AttributedQuotaSample> {
    samples.sort_by(|left, right| {
        left.timestamp
            .cmp(&right.timestamp)
            .then_with(|| right.used_percent.cmp(&left.used_percent))
            .then_with(|| left.source.cmp(&right.source))
            .then_with(|| left.offset.cmp(&right.offset))
    });
    let mut maximum = 0_u8;
    samples
        .into_iter()
        .filter(|sample| {
            if sample.used_percent <= maximum {
                false
            } else {
                maximum = sample.used_percent;
                true
            }
        })
        .collect()
}

fn start_of_local_date(date: NaiveDate) -> DateTime<Local> {
    let naive = date.and_hms_opt(0, 0, 0).expect("valid date midnight");
    Local
        .from_local_datetime(&naive)
        .earliest()
        .unwrap_or_else(Local::now)
}

// The collector treats identical relative paths in sessions and archived_sessions
// as the same rollout, preferring the active copy. Keep this identity on moves.
fn archive_alias(source: &str) -> Option<String> {
    source
        .strip_prefix("sessions/")
        .map(|tail| format!("archived_sessions/{tail}"))
        .or_else(|| {
            source
                .strip_prefix("archived_sessions/")
                .map(|tail| format!("sessions/{tail}"))
        })
}

const VALUES: &str = "input_tokens,cached_input_tokens,output_tokens,requests,estimated_cost_microusd,priced_requests,cache_savings_microusd";
const SUMS: &str = "SUM(input_tokens), SUM(cached_input_tokens), SUM(output_tokens), SUM(requests), SUM(estimated_cost_microusd), SUM(priced_requests), SUM(cache_savings_microusd)";

impl ProviderStore {
    pub(super) fn migrate_codex_accounts(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS codex_account_events (
            account TEXT NOT NULL, source TEXT NOT NULL DEFAULT '',
            session TEXT NOT NULL, ts INTEGER NOT NULL,
            signature TEXT NOT NULL, date TEXT NOT NULL, model TEXT NOT NULL,
            input_tokens INTEGER NOT NULL, cached_input_tokens INTEGER NOT NULL,
            output_tokens INTEGER NOT NULL, requests INTEGER NOT NULL,
            estimated_cost_microusd INTEGER NOT NULL, priced_requests INTEGER NOT NULL,
            cache_savings_microusd INTEGER NOT NULL,
            PRIMARY KEY(session, ts, signature));",
        )?;
        self.ensure_column("codex_account_events", "source", "TEXT NOT NULL DEFAULT ''")?;
        self.ensure_column(
            "codex_account_events",
            "covered",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        self.conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS codex_account_date ON codex_account_events(account,date);
            CREATE INDEX IF NOT EXISTS codex_account_time ON codex_account_events(account,ts);
            CREATE INDEX IF NOT EXISTS codex_account_source ON codex_account_events(source);",
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
        self.conn.execute_batch("CREATE TABLE IF NOT EXISTS codex_event_sources (
            source TEXT NOT NULL, session TEXT NOT NULL, ts INTEGER NOT NULL,
            signature TEXT NOT NULL, PRIMARY KEY(source,session,ts,signature));
            CREATE INDEX IF NOT EXISTS codex_event_sources_event ON codex_event_sources(session,ts,signature);
            CREATE TABLE IF NOT EXISTS codex_quota_samples (
                account TEXT NOT NULL, source TEXT NOT NULL, event_offset INTEGER NOT NULL,
                ts INTEGER NOT NULL, date TEXT NOT NULL, limit_id TEXT NOT NULL,
                window_minutes INTEGER NOT NULL, reset_at INTEGER,
                used_percent INTEGER NOT NULL,
                baseline INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY(source,event_offset,limit_id,window_minutes)
            );
            CREATE INDEX IF NOT EXISTS codex_quota_account_time
                ON codex_quota_samples(account,limit_id,window_minutes,ts);
            CREATE INDEX IF NOT EXISTS codex_quota_source ON codex_quota_samples(source);")?;
        self.ensure_column(
            "codex_quota_samples",
            "baseline",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        // Existing preview databases did not persist sources. Recover links from
        // the still-present scanner metadata before the version-8 full replay.
        let tx = rusqlite::Transaction::new_unchecked(
            &self.conn,
            rusqlite::TransactionBehavior::Immediate,
        )?;
        let migrated: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM meta WHERE key='codex.event_sources.v1')",
            [],
            |r| r.get(0),
        )?;
        if !migrated {
            tx.execute(
                "INSERT OR IGNORE INTO codex_event_sources
                 SELECT source,session,ts,signature FROM codex_account_events WHERE source<>''",
                [],
            )?;
            let files = {
                let mut query =
                    tx.prepare("SELECT path,meta_json FROM scan_files WHERE provider='codex'")?;
                query
                    .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
                    .collect::<rusqlite::Result<Vec<_>>>()?
            };
            let mut basename_counts = BTreeMap::new();
            for (path, _) in &files {
                *basename_counts
                    .entry(Path::new(path).file_name().unwrap_or_default().to_owned())
                    .or_insert(0) += 1;
            }
            for (path, raw) in files {
                let meta: serde_json::Value = serde_json::from_str(&raw)?;
                let basename = Path::new(&path).file_name().unwrap_or_default();
                let old_name = basename.to_string_lossy();
                let session = meta
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .filter(|v| !v.is_empty())
                    .unwrap_or(&old_name);
                tx.execute(
                    "INSERT OR IGNORE INTO codex_event_sources
                    SELECT ?1,session,ts,signature FROM codex_account_events WHERE session=?2 AND source=''",
                    params![path, session],
                )?;
                // Older preview cursors cannot be split if their basenames
                // collide. Never guess their offsets or change frozen totals.
                if basename_counts.get(basename) == Some(&1) {
                    tx.execute("INSERT OR IGNORE INTO codex_legacy_cursors SELECT ?1,offset FROM codex_legacy_cursors WHERE source=?2",params![path,old_name.as_ref()])?;
                }
            }
            tx.execute(
                "INSERT INTO meta(key,value) VALUES('codex.event_sources.v1','1')",
                [],
            )?;
        }
        tx.commit()?;
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
                let source = path.clone();
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

    #[cfg(test)]
    fn save_account_events(
        &self,
        events: &[AccountEvent],
        state: &Attribution,
        before: &Option<Identity>,
        after: &Option<Identity>,
        end: DateTime<Utc>,
    ) -> Result<()> {
        self.save_account_scan(events, &[], state, before, after, end)
    }

    pub(crate) fn save_account_scan(
        &self,
        events: &[AccountEvent],
        rebuilt_sources: &[String],
        state: &Attribution,
        before: &Option<Identity>,
        after: &Option<Identity>,
        end: DateTime<Utc>,
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let mut previous = std::collections::BTreeSet::new();
        for source in rebuilt_sources {
            if let Some(alias) = archive_alias(source) {
                tx.execute("INSERT OR IGNORE INTO codex_event_sources SELECT ?1,session,ts,signature FROM codex_event_sources WHERE source=?2",params![source,alias])?;
                tx.execute("DELETE FROM codex_event_sources WHERE source=?1", [alias])?;
            }
            let mut query =
                tx.prepare("SELECT session,ts,signature FROM codex_event_sources WHERE source=?1")?;
            for row in query.query_map([source], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })? {
                previous.insert(row?);
            }
            tx.execute("DELETE FROM codex_event_sources WHERE source=?1", [source])?;
        }

        {
            let mut query = tx.prepare("SELECT source,offset FROM codex_legacy_cursors")?;
            let cursors = query
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
                })?
                .collect::<rusqlite::Result<BTreeMap<_, _>>>()?;
            let mut insert = tx.prepare("INSERT INTO codex_account_events(
                    account,session,ts,signature,date,model,input_tokens,cached_input_tokens,
                    output_tokens,requests,estimated_cost_microusd,priced_requests,cache_savings_microusd,covered)
                VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
                ON CONFLICT(session,ts,signature) DO UPDATE SET
                    model=excluded.model,input_tokens=excluded.input_tokens,cached_input_tokens=excluded.cached_input_tokens,
                    output_tokens=excluded.output_tokens,requests=excluded.requests,estimated_cost_microusd=excluded.estimated_cost_microusd,
                    priced_requests=excluded.priced_requests,cache_savings_microusd=excluded.cache_savings_microusd
                WHERE model IS NOT excluded.model OR input_tokens IS NOT excluded.input_tokens OR cached_input_tokens IS NOT excluded.cached_input_tokens
                    OR output_tokens IS NOT excluded.output_tokens OR requests IS NOT excluded.requests OR estimated_cost_microusd IS NOT excluded.estimated_cost_microusd
                    OR priced_requests IS NOT excluded.priced_requests OR cache_savings_microusd IS NOT excluded.cache_savings_microusd")?;
            let mut link =
                tx.prepare("INSERT OR IGNORE INTO codex_event_sources VALUES(?1,?2,?3,?4)")?;
            // Older previews keyed only by timestamp + usage. Promote their
            // existing row and every source link before inserting a record key,
            // retaining ownership/coverage instead of counting the old row twice.
            let mut legacy = tx.prepare("SELECT EXISTS(SELECT 1 FROM codex_account_events WHERE session=?1 AND ts=?2 AND signature=?3)")?;
            let mut promote = tx.prepare(&format!("INSERT OR IGNORE INTO codex_account_events(account,source,session,ts,signature,date,model,{VALUES},covered)
                SELECT account,source,session,ts,?4,date,model,{VALUES},covered FROM codex_account_events
                WHERE session=?1 AND ts=?2 AND signature=?3"))?;
            let mut promote_links = tx.prepare("INSERT OR IGNORE INTO codex_event_sources
                SELECT source,session,ts,?4 FROM codex_event_sources WHERE session=?1 AND ts=?2 AND signature=?3")?;
            let mut remove_legacy_links = tx.prepare(
                "DELETE FROM codex_event_sources WHERE session=?1 AND ts=?2 AND signature=?3",
            )?;
            let mut remove_legacy = tx.prepare(
                "DELETE FROM codex_account_events WHERE session=?1 AND ts=?2 AND signature=?3",
            )?;
            for e in events {
                // Byte offsets distinguish repeated A/B/A records even when
                // timestamps collide. Moving a rollout to the archive preserves
                // its record offsets and therefore its persisted event identity.
                let record = format!("record:{}:{}", e.offset, e.signature);
                let old_key = params![e.session, e.timestamp.timestamp_millis(), e.signature];
                if legacy.query_row(old_key, |r| r.get::<_, bool>(0))? {
                    let keys = params![
                        e.session,
                        e.timestamp.timestamp_millis(),
                        e.signature,
                        record
                    ];
                    promote.execute(keys)?;
                    promote_links.execute(keys)?;
                    remove_legacy_links.execute(old_key)?;
                    remove_legacy.execute(old_key)?;
                }
                link.execute(params![
                    e.source,
                    e.session,
                    e.timestamp.timestamp_millis(),
                    record
                ])?;
                let u = &e.usage;
                insert.execute(params![
                    state.owner(e.timestamp, before, after, end),
                    e.session,
                    e.timestamp.timestamp_millis(),
                    record,
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
                            .or_else(
                                || archive_alias(&e.source).and_then(|alias| cursors.get(&alias))
                            )
                            .is_some_and(|offset| e.offset <= *offset)
                ])?;
            }
        }
        // Delete only obsolete events after replay, preserving both ownership
        // of surviving events and copies still present in another rollout.
        for (session, at, signature) in previous {
            tx.execute("DELETE FROM codex_account_events WHERE session=?1 AND ts=?2 AND signature=?3
                AND NOT EXISTS(SELECT 1 FROM codex_event_sources WHERE session=?1 AND ts=?2 AND signature=?3)",
                params![session,at,signature])?;
        }
        let mut updated = state.clone();
        updated.advance(before, after, end);
        tx.execute("INSERT INTO meta(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![STATE,serde_json::to_string(&updated)?])?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn save_codex_quota_scan(
        &self,
        samples: &[QuotaSampleEvent],
        rebuilt_sources: &[String],
        state: &Attribution,
        before: &Option<Identity>,
        after: &Option<Identity>,
        end: DateTime<Utc>,
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for source in rebuilt_sources {
            tx.execute("DELETE FROM codex_quota_samples WHERE source=?1", [source])?;
            if let Some(alias) = archive_alias(source) {
                tx.execute("DELETE FROM codex_quota_samples WHERE source=?1", [alias])?;
            }
        }
        let attributed = samples
            .iter()
            .filter_map(|sample| {
                let resets_at = sample.resets_at?;
                // Resumed/forked rollouts can replay an old rate-limit payload
                // under a new log timestamp. Once its reset is already in the
                // past it is historical transcript content, not a new snapshot.
                if resets_at + Duration::seconds(RESET_TOLERANCE_SECONDS) < sample.timestamp {
                    return None;
                }
                Some(AttributedQuotaSample {
                    account: state.owner(sample.timestamp, before, after, end).to_owned(),
                    source: sample.source.clone(),
                    offset: sample.offset,
                    timestamp: sample.timestamp,
                    limit_id: sample.limit_id.clone(),
                    window_minutes: sample.window_minutes,
                    resets_at,
                    used_percent: sample.used_percent,
                    baseline: false,
                })
            })
            .collect();
        let mut cycles = group_quota_samples(attributed);
        for cycle in &mut cycles {
            let existing = {
                let mut query = tx.prepare(
                    "SELECT account,source,event_offset,ts,limit_id,window_minutes,reset_at,used_percent,baseline
                     FROM codex_quota_samples
                     WHERE account=?1 AND limit_id=?2 AND window_minutes=?3
                       AND reset_at BETWEEN ?4 AND ?5",
                )?;
                query
                    .query_map(
                        params![
                            cycle.account,
                            cycle.limit_id,
                            i64::from(cycle.window_minutes),
                            cycle.reset_at - RESET_TOLERANCE_SECONDS,
                            cycle.reset_at + RESET_TOLERANCE_SECONDS,
                        ],
                        |row| {
                            let timestamp = Utc
                                .timestamp_millis_opt(row.get::<_, i64>(3)?)
                                .single()
                                .ok_or_else(|| {
                                    rusqlite::Error::IntegralValueOutOfRange(3, i64::MAX)
                                })?;
                            let resets_at = Utc
                                .timestamp_opt(row.get::<_, i64>(6)?, 0)
                                .single()
                                .ok_or_else(|| {
                                    rusqlite::Error::IntegralValueOutOfRange(6, i64::MAX)
                                })?;
                            Ok(AttributedQuotaSample {
                                account: row.get(0)?,
                                source: row.get(1)?,
                                offset: row.get(2)?,
                                timestamp,
                                limit_id: row.get(4)?,
                                window_minutes: row.get(5)?,
                                resets_at,
                                used_percent: row.get(7)?,
                                baseline: row.get(8)?,
                            })
                        },
                    )?
                    .collect::<rusqlite::Result<Vec<_>>>()?
            };
            cycle.samples.extend(existing);
            tx.execute(
                "DELETE FROM codex_quota_samples
                 WHERE account=?1 AND limit_id=?2 AND window_minutes=?3
                   AND reset_at BETWEEN ?4 AND ?5",
                params![
                    cycle.account,
                    cycle.limit_id,
                    i64::from(cycle.window_minutes),
                    cycle.reset_at - RESET_TOLERANCE_SECONDS,
                    cycle.reset_at + RESET_TOLERANCE_SECONDS,
                ],
            )?;
        }

        let compacted = cycles
            .into_iter()
            .flat_map(|cycle| compact_quota_progression(cycle.samples))
            .collect::<Vec<_>>();
        let mut insert = tx.prepare(
            "INSERT INTO codex_quota_samples(
                account,source,event_offset,ts,date,limit_id,window_minutes,reset_at,used_percent,baseline)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(source,event_offset,limit_id,window_minutes) DO UPDATE SET
                account=excluded.account,ts=excluded.ts,date=excluded.date,
                reset_at=excluded.reset_at,used_percent=excluded.used_percent,
                baseline=excluded.baseline",
        )?;
        for sample in compacted {
            insert.execute(params![
                sample.account,
                sample.source,
                sample.offset as i64,
                sample.timestamp.timestamp_millis(),
                sample
                    .timestamp
                    .with_timezone(&Local)
                    .date_naive()
                    .to_string(),
                sample.limit_id,
                i64::from(sample.window_minutes),
                sample.resets_at.timestamp(),
                i64::from(sample.used_percent),
                sample.baseline,
            ])?;
        }
        drop(insert);
        tx.commit()?;
        Ok(())
    }

    /// Records the live account quota returned by Codex itself. The first
    /// observation in a period is a baseline: it positions the remaining-quota
    /// line, while only later increases are assigned to a calendar day.
    pub(crate) fn save_codex_limit_snapshot(
        &self,
        account: &str,
        limits: &RateLimits,
    ) -> Result<()> {
        if account == UNKNOWN {
            return Ok(());
        }
        let tx = self.conn.unchecked_transaction()?;
        for window in [&limits.primary, &limits.secondary] {
            let (Some(used_percent), Some(resets_at), Some(window_minutes)) = (
                window.used_percent,
                window.resets_at,
                window.duration_minutes,
            ) else {
                continue;
            };
            if used_percent == 0
                || resets_at + Duration::seconds(RESET_TOLERANCE_SECONDS) < limits.sampled_at
            {
                continue;
            }
            let (count, maximum): (u64, u8) = tx.query_row(
                "SELECT COUNT(*),COALESCE(MAX(used_percent),0)
                 FROM codex_quota_samples
                 WHERE account=?1 AND limit_id='codex' AND window_minutes=?2
                   AND reset_at BETWEEN ?3 AND ?4",
                params![
                    account,
                    i64::from(window_minutes),
                    resets_at.timestamp() - RESET_TOLERANCE_SECONDS,
                    resets_at.timestamp() + RESET_TOLERANCE_SECONDS,
                ],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            if used_percent <= maximum {
                continue;
            }
            tx.execute(
                "INSERT INTO codex_quota_samples(
                    account,source,event_offset,ts,date,limit_id,window_minutes,
                    reset_at,used_percent,baseline)
                 VALUES(?1,?2,?3,?4,?5,'codex',?6,?7,?8,?9)
                 ON CONFLICT(source,event_offset,limit_id,window_minutes) DO UPDATE SET
                    account=excluded.account,ts=excluded.ts,date=excluded.date,
                    reset_at=excluded.reset_at,used_percent=excluded.used_percent,
                    baseline=excluded.baseline",
                params![
                    account,
                    format!("live-limits/{account}"),
                    limits.sampled_at.timestamp_millis(),
                    limits.sampled_at.timestamp_millis(),
                    limits
                        .sampled_at
                        .with_timezone(&Local)
                        .date_naive()
                        .to_string(),
                    i64::from(window_minutes),
                    resets_at.timestamp(),
                    i64::from(used_percent),
                    count == 0,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Reconstructs percentage points consumed per local day. Codex emits the
    /// same global quota snapshot in every active task, sometimes out of order,
    /// so each reset period uses one monotonic progression rather than summing
    /// per-session copies.
    pub(crate) fn load_codex_quota_history(
        &self,
        account: &str,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<(
        Option<u32>,
        BTreeMap<NaiveDate, u64>,
        Vec<crate::usage::CodexQuotaCycle>,
    )> {
        if start > end {
            return Ok((None, BTreeMap::new(), Vec::new()));
        }
        let expanded_start = start - Duration::days(8);
        let mut query = self.conn.prepare(
            "SELECT date,ts,window_minutes,reset_at,used_percent,baseline
             FROM codex_quota_samples
             WHERE account=?1 AND limit_id='codex' AND date>=?2 AND date<=?3
             ORDER BY ts",
        )?;
        let rows = query
            .query_map(
                params![account, expanded_start.to_string(), end.to_string()],
                |row| {
                    Ok((
                        parse_date(&row.get::<_, String>(0)?),
                        row.get::<_, i64>(1)?,
                        row.get::<_, u32>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, u8>(4)?,
                        row.get::<_, bool>(5)?,
                    ))
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let window_minutes = if rows.iter().any(|row| row.2 == 10_080) {
            Some(10_080)
        } else if rows.iter().any(|row| row.2 == 300) {
            Some(300)
        } else {
            None
        };
        let Some(window_minutes) = window_minutes else {
            return Ok((None, BTreeMap::new(), Vec::new()));
        };

        #[derive(Default)]
        struct Cycle {
            reset_at: i64,
            samples: Vec<(NaiveDate, i64, u8, bool)>,
        }
        let mut cycles: Vec<Cycle> = Vec::new();
        for (date, timestamp, minutes, reset_at, used, baseline) in rows {
            if minutes != window_minutes {
                continue;
            }
            let Some(reset_at) = reset_at else {
                continue;
            };
            if reset_at + RESET_TOLERANCE_SECONDS < timestamp.div_euclid(1_000) {
                continue;
            }
            if let Some(cycle) = cycles
                .iter_mut()
                .find(|cycle| (cycle.reset_at - reset_at).abs() <= RESET_TOLERANCE_SECONDS)
            {
                cycle.samples.push((date, timestamp, used, baseline));
            } else {
                cycles.push(Cycle {
                    reset_at,
                    samples: vec![(date, timestamp, used, baseline)],
                });
            }
        }

        let range_start = start_of_local_date(start).with_timezone(&Utc);
        let range_end = start_of_local_date(end + Duration::days(1)).with_timezone(&Utc);
        let now = Utc::now();
        let visible_end = range_end.min(now);
        let mut daily = BTreeMap::<NaiveDate, u64>::new();
        let mut quota_cycles = Vec::new();
        cycles.sort_by_key(|cycle| cycle.reset_at);
        let cycle_windows = cycles
            .iter()
            .map(|cycle| {
                Utc.timestamp_opt(cycle.reset_at, 0)
                    .single()
                    .map(|resets_at| {
                        (
                            resets_at - Duration::minutes(i64::from(window_minutes)),
                            resets_at,
                        )
                    })
            })
            .collect::<Vec<_>>();

        for (index, cycle) in cycles.iter_mut().enumerate() {
            cycle.samples.sort_by_key(|sample| sample.1);
            let Some((starts_at, scheduled_reset)) = cycle_windows[index] else {
                continue;
            };
            // A changed reset schedule means Codex started a new quota cycle
            // before the previous projected reset. End the older cycle at that
            // transition so historical schedules never render or accumulate in
            // parallel.
            let effective_reset = cycle_windows
                .iter()
                .skip(index + 1)
                .flatten()
                .map(|(next_start, _)| *next_start)
                .find(|next_start| *next_start > starts_at)
                .map_or(scheduled_reset, |next_start| {
                    scheduled_reset.min(next_start)
                });
            let was_superseded = effective_reset < scheduled_reset;

            let mut previous = 0_u8;
            for (date, timestamp, used, baseline) in &cycle.samples {
                let Some(at) = Utc.timestamp_millis_opt(*timestamp).single() else {
                    continue;
                };
                if at < starts_at
                    || at > effective_reset
                    || (was_superseded && at == effective_reset)
                {
                    continue;
                }
                let increase = if *baseline {
                    0
                } else {
                    used.saturating_sub(previous)
                };
                previous = previous.max(*used);
                if increase > 0 && *date >= start && *date <= end {
                    daily
                        .entry(*date)
                        .and_modify(|value| {
                            *value = value.saturating_add(
                                u64::from(increase) * crate::usage::ANALYTICS_PERCENT_SCALE,
                            )
                        })
                        .or_insert(u64::from(increase) * crate::usage::ANALYTICS_PERCENT_SCALE);
                }
            }

            let segment_start = starts_at.max(range_start);
            let segment_end = effective_reset.min(visible_end);
            if segment_start > segment_end {
                continue;
            }
            let mut running = 0_u8;
            let reset_is_visible = starts_at >= range_start;
            if !reset_is_visible {
                for (_, timestamp, used, _) in &cycle.samples {
                    let Some(at) = Utc.timestamp_millis_opt(*timestamp).single() else {
                        continue;
                    };
                    if at <= segment_start {
                        running = running.max(*used);
                    }
                }
            }
            let mut points = vec![crate::usage::CodexQuotaPoint {
                at: segment_start,
                remaining_percent_micros: u64::from(100_u8.saturating_sub(running))
                    * crate::usage::ANALYTICS_PERCENT_SCALE,
            }];
            for (_, timestamp, used, _) in &cycle.samples {
                let Some(at) = Utc.timestamp_millis_opt(*timestamp).single() else {
                    continue;
                };
                if ((!reset_is_visible && at <= segment_start) || at < segment_start)
                    || at > segment_end
                    || (was_superseded && at == segment_end)
                    || *used <= running
                {
                    continue;
                }
                running = *used;
                points.push(crate::usage::CodexQuotaPoint {
                    at,
                    remaining_percent_micros: u64::from(100_u8.saturating_sub(running))
                        * crate::usage::ANALYTICS_PERCENT_SCALE,
                });
            }
            if points.last().is_some_and(|point| point.at < segment_end) {
                points.push(crate::usage::CodexQuotaPoint {
                    at: segment_end,
                    remaining_percent_micros: u64::from(100_u8.saturating_sub(running))
                        * crate::usage::ANALYTICS_PERCENT_SCALE,
                });
            }
            quota_cycles.push(crate::usage::CodexQuotaCycle {
                starts_at,
                resets_at: effective_reset,
                points,
            });
        }
        Ok((Some(window_minutes), daily, quota_cycles))
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

    fn insert_quota(
        db: &ProviderStore,
        account: &str,
        source: &str,
        offset: i64,
        timestamp: DateTime<Utc>,
        window_minutes: u32,
        reset_at: DateTime<Utc>,
        used_percent: u8,
    ) {
        db.conn
            .execute(
                "INSERT INTO codex_quota_samples(
                    account,source,event_offset,ts,date,limit_id,window_minutes,reset_at,used_percent)
                 VALUES(?1,?2,?3,?4,?5,'codex',?6,?7,?8)",
                params![
                    account,
                    source,
                    offset,
                    timestamp.timestamp_millis(),
                    timestamp.with_timezone(&Local).date_naive().to_string(),
                    i64::from(window_minutes),
                    reset_at.timestamp(),
                    i64::from(used_percent),
                ],
            )
            .unwrap();
    }

    #[test]
    fn daily_quota_merges_stale_copies_resets_and_accounts() {
        let db = store();
        let first_reset = Utc::now() - Duration::hours(12);
        let first = first_reset - Duration::days(1);
        let second = first_reset - Duration::hours(1);
        let next_reset = first_reset + Duration::days(7);

        insert_quota(&db, "account-a", "one", 1, first, 10_080, first_reset, 10);
        insert_quota(
            &db,
            "account-a",
            "two",
            1,
            first + Duration::hours(1),
            10_080,
            first_reset + Duration::seconds(45),
            11,
        );
        // A lagging task re-emits an older global snapshot. It must not reduce
        // the cycle maximum or create another increment.
        insert_quota(
            &db,
            "account-a",
            "three",
            1,
            first + Duration::hours(2),
            10_080,
            first_reset,
            10,
        );
        insert_quota(&db, "account-a", "one", 2, second, 10_080, first_reset, 15);
        // A reset on the same local day starts a new cycle, so its first three
        // percentage points are also consumption for that day.
        insert_quota(
            &db,
            "account-a",
            "one",
            3,
            second + Duration::hours(2),
            10_080,
            next_reset,
            3,
        );
        // Prefer the weekly window when both are available.
        insert_quota(
            &db,
            "account-a",
            "one",
            4,
            second + Duration::hours(3),
            300,
            second + Duration::hours(5),
            99,
        );
        insert_quota(
            &db,
            "account-b",
            "other",
            1,
            second,
            10_080,
            first_reset,
            90,
        );

        let start = first.with_timezone(&Local).date_naive();
        let end = second.with_timezone(&Local).date_naive();
        let (window, daily, cycles) = db
            .load_codex_quota_history("account-a", start, end)
            .unwrap();
        assert_eq!(window, Some(10_080));
        assert_eq!(daily.get(&start), Some(&11_000_000));
        assert_eq!(daily.get(&end), Some(&7_000_000));
        assert_eq!(daily.len(), 2);
        assert_eq!(cycles.len(), 2);
        assert_eq!(
            cycles[0].points.first().unwrap().remaining_percent_micros,
            100_000_000
        );
        assert_eq!(
            cycles[0]
                .points
                .iter()
                .map(|point| point.remaining_percent_micros)
                .min(),
            Some(85_000_000)
        );
        assert_eq!(
            cycles[1]
                .points
                .iter()
                .map(|point| point.remaining_percent_micros)
                .min(),
            Some(97_000_000)
        );

        let (_, other, other_cycles) = db
            .load_codex_quota_history("account-b", start, end)
            .unwrap();
        assert_eq!(other.get(&end), Some(&90_000_000));
        assert_eq!(other_cycles.len(), 1);
        assert_eq!(
            other_cycles[0]
                .points
                .last()
                .unwrap()
                .remaining_percent_micros,
            10_000_000
        );
    }

    #[test]
    fn quota_history_truncates_a_cycle_when_a_new_schedule_starts() {
        let db = store();
        let first_start = Utc::now() - Duration::days(20);
        let first_reset = first_start + Duration::days(7);
        let forced_start = first_start + Duration::days(3);
        let forced_reset = forced_start + Duration::days(7);
        let first_sample = first_start + Duration::days(1);
        let stale_sample = first_start + Duration::days(4);
        let forced_sample = forced_start + Duration::hours(1);
        let forced_later = forced_start + Duration::days(2);

        insert_quota(
            &db,
            "account-a",
            "first",
            1,
            first_sample,
            10_080,
            first_reset,
            10,
        );
        insert_quota(
            &db,
            "account-a",
            "first",
            2,
            stale_sample,
            10_080,
            first_reset,
            40,
        );
        insert_quota(
            &db,
            "account-a",
            "forced",
            1,
            forced_sample,
            10_080,
            forced_reset,
            5,
        );
        insert_quota(
            &db,
            "account-a",
            "forced",
            2,
            forced_later,
            10_080,
            forced_reset,
            20,
        );

        let start = first_start.with_timezone(&Local).date_naive();
        let end = (first_start + Duration::days(6))
            .with_timezone(&Local)
            .date_naive();
        let (_, daily, cycles) = db
            .load_codex_quota_history("account-a", start, end)
            .unwrap();

        assert_eq!(cycles.len(), 2);
        assert_eq!(cycles[0].resets_at.timestamp(), forced_start.timestamp());
        assert_eq!(cycles[0].resets_at, cycles[1].starts_at);
        assert!(
            cycles
                .windows(2)
                .all(|pair| pair[0].resets_at <= pair[1].starts_at)
        );
        assert_eq!(
            cycles[0].points.last().unwrap().remaining_percent_micros,
            90_000_000
        );
        assert!(
            cycles[0]
                .points
                .iter()
                .all(|point| point.at <= forced_start)
        );
        assert_eq!(
            daily.get(&first_sample.with_timezone(&Local).date_naive()),
            Some(&10_000_000)
        );
        assert_eq!(
            daily.get(&forced_sample.with_timezone(&Local).date_naive()),
            Some(&5_000_000)
        );
        assert_eq!(
            daily.get(&forced_later.with_timezone(&Local).date_naive()),
            Some(&15_000_000)
        );
        assert!(!daily.contains_key(&stale_sample.with_timezone(&Local).date_naive()));
    }

    #[test]
    fn quota_scan_persists_only_global_progression_changes() {
        let db = store();
        let at = Utc.with_ymd_and_hms(2026, 9, 14, 8, 0, 0).unwrap();
        let reset = at + Duration::days(6);
        let state = Attribution {
            initialized_at: at + Duration::days(1),
            historical_owner: "account-a".into(),
            history_pending: false,
            observed: None,
            observed_at: at + Duration::days(1),
            ready: true,
            spans: Vec::new(),
        };
        let sample = |source: &str, offset: u64, hours: i64, used_percent: u8| QuotaSampleEvent {
            source: source.into(),
            offset,
            timestamp: at + Duration::hours(hours),
            limit_id: "codex".into(),
            window_minutes: 10_080,
            resets_at: Some(reset),
            used_percent,
        };
        db.save_codex_quota_scan(
            &[
                sample("one", 1, 0, 10),
                sample("two", 1, 1, 10),
                sample("three", 1, 2, 9),
                sample("two", 2, 3, 11),
                QuotaSampleEvent {
                    source: "replayed".into(),
                    offset: 1,
                    timestamp: reset + Duration::hours(1),
                    limit_id: "codex".into(),
                    window_minutes: 10_080,
                    resets_at: Some(reset),
                    used_percent: 99,
                },
            ],
            &[
                "one".into(),
                "two".into(),
                "three".into(),
                "replayed".into(),
            ],
            &state,
            &None,
            &None,
            at + Duration::days(1),
        )
        .unwrap();
        assert_eq!(
            db.conn
                .query_row("SELECT COUNT(*) FROM codex_quota_samples", [], |row| row
                    .get::<_, u64>(0))
                .unwrap(),
            2
        );

        db.save_codex_quota_scan(
            &[sample("four", 1, 4, 10), sample("four", 2, 5, 12)],
            &[],
            &state,
            &None,
            &None,
            at + Duration::days(1),
        )
        .unwrap();
        assert_eq!(
            db.conn
                .query_row("SELECT COUNT(*) FROM codex_quota_samples", [], |row| row
                    .get::<_, u64>(0))
                .unwrap(),
            3
        );
    }

    #[test]
    fn live_limit_snapshot_sets_a_baseline_then_counts_only_new_usage() {
        let db = store();
        let sampled_at = Utc::now() - Duration::hours(2);
        let reset = sampled_at + Duration::days(3);
        let limits = |sampled_at, used_percent| RateLimits {
            sampled_at,
            secondary: crate::limits::LimitWindow {
                used_percent: Some(used_percent),
                resets_at: Some(reset),
                duration_minutes: Some(10_080),
            },
            ..Default::default()
        };
        db.save_codex_limit_snapshot("account-a", &limits(sampled_at, 71))
            .unwrap();
        let day = sampled_at.with_timezone(&Local).date_naive();
        let (_, daily, cycles) = db.load_codex_quota_history("account-a", day, day).unwrap();
        assert!(daily.is_empty());
        assert_eq!(
            cycles[0].points.last().unwrap().remaining_percent_micros,
            29_000_000
        );

        db.save_codex_limit_snapshot("account-a", &limits(sampled_at + Duration::hours(1), 75))
            .unwrap();
        let (_, daily, cycles) = db.load_codex_quota_history("account-a", day, day).unwrap();
        assert_eq!(daily.get(&day), Some(&4_000_000));
        assert_eq!(
            cycles[0].points.last().unwrap().remaining_percent_micros,
            25_000_000
        );
        let (_, other, _) = db.load_codex_quota_history("account-b", day, day).unwrap();
        assert!(other.is_empty());
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
    fn missing_or_empty_auth_is_logout_but_invalid_json_is_uncertain() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        assert_eq!(observe_auth(&path), Observation::LoggedOut);
        fs::write(&path, json!({"tokens":{"account_id":"acct-a"}}).to_string()).unwrap();
        match observe_auth(&path) {
            Observation::Account(identity) => assert_eq!(identity.id, "acct-a"),
            other => panic!("expected account, got {other:?}"),
        }
        fs::write(&path, json!({"tokens":{"account_id":"  "}}).to_string()).unwrap();
        assert_eq!(observe_auth(&path), Observation::LoggedOut);
        fs::write(&path, "{").unwrap();
        assert_eq!(observe_auth(&path), Observation::Uncertain);
    }

    #[test]
    fn sticky_id_survives_uncertain_reads_and_clears_on_logout() {
        let account = Observation::Account(Identity {
            id: "acct-a".into(),
            stamp: 1,
            session: None,
        });
        remember(None);
        assert_eq!(id_from_observation(account), "acct-a");
        assert_eq!(id_from_observation(Observation::Uncertain), "acct-a");
        assert_eq!(poll_from_observation(Observation::Uncertain), None);
        assert_eq!(
            poll_from_observation(Observation::LoggedOut).as_deref(),
            Some(UNKNOWN)
        );
        assert_eq!(id_from_observation(Observation::LoggedOut), UNKNOWN);
        remember(None);
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
        for (path, source) in crate::usage::collect_codex_session_files(&codex_home()).unwrap() {
            let mut file = crate::usage::CachedSessionFile::default();
            events.extend(
                crate::usage::scan_file_delta(&path, &source, &mut file)
                    .unwrap()
                    .events,
            );
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

    #[test]
    fn truncated_rollout_replaces_events_and_empty_rewrite_clears_them() {
        let db = store();
        let s = state();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rollout.jsonl");
        let source = "sessions/2026/01/rollout.jsonl";
        let record = |tokens| {
            format!(
                "{}\n{}\n",
                json!({"type":"turn_context","payload":{"model":"gpt-5.4"}}),
                json!({"timestamp":"1970-01-01T00:01:55Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":tokens}}}})
            )
        };
        let mut file = crate::usage::CachedSessionFile::default();
        let save = |delta: crate::usage::FileDelta| {
            let rebuilt = if delta.rebuilt {
                vec![source.to_owned()]
            } else {
                vec![]
            };
            db.save_account_scan(
                &delta.events,
                &rebuilt,
                &s,
                &s.observed,
                &s.observed,
                at(120),
            )
            .unwrap();
        };
        fs::write(&path, record(1000)).unwrap();
        save(crate::usage::scan_file_delta(&path, source, &mut file).unwrap());
        fs::write(&path, record(2)).unwrap();
        save(crate::usage::scan_file_delta(&path, source, &mut file).unwrap());
        assert_eq!(
            db.account_daily_for("new", at(0).date_naive(), at(200).date_naive())
                .unwrap()[0]
                .2
                .input_tokens,
            2
        );
        // Replaying the same rebuild is idempotent and leaves the other account alone.
        file = crate::usage::CachedSessionFile::default();
        save(crate::usage::scan_file_delta(&path, source, &mut file).unwrap());
        assert_eq!(
            db.conn
                .query_row("SELECT COUNT(*) FROM codex_account_events", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        fs::write(&path, "").unwrap();
        save(crate::usage::scan_file_delta(&path, source, &mut file).unwrap());
        assert!(
            db.account_daily_for("new", at(0).date_naive(), at(200).date_naive())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn resetting_one_copy_keeps_other_sources_and_surviving_ownership() {
        let db = store();
        let s = state();
        let a = event(115, 10);
        let mut b = event(115, 10);
        b.source = "archived_sessions/copy.jsonl".into();
        let mut unrelated = event(115, 20);
        unrelated.source = "sessions/unrelated.jsonl".into();
        unrelated.session = "other-session".into();
        db.save_account_events(&[a, b, unrelated], &s, &s.observed, &s.observed, at(120))
            .unwrap();
        db.save_account_scan(
            &[],
            &["same-conversation.jsonl".into()],
            &s,
            &None,
            &None,
            at(125),
        )
        .unwrap();
        assert_eq!(
            db.account_daily_for("new", at(0).date_naive(), at(200).date_naive())
                .unwrap()[0]
                .2
                .input_tokens,
            30
        );
        let mut replay = event(115, 10);
        replay.source = "archived_sessions/copy.jsonl".into();
        db.save_account_scan(
            &[replay],
            &["archived_sessions/copy.jsonl".into()],
            &s,
            &None,
            &None,
            at(130),
        )
        .unwrap();
        assert!(
            db.account_daily_for(UNKNOWN, at(0).date_naive(), at(200).date_naive())
                .unwrap()
                .is_empty()
        );
        db.save_account_scan(
            &[],
            &["archived_sessions/copy.jsonl".into()],
            &s,
            &None,
            &None,
            at(135),
        )
        .unwrap();
        assert_eq!(
            db.account_daily_for("new", at(0).date_naive(), at(200).date_naive())
                .unwrap()[0]
                .2
                .input_tokens,
            20
        );
    }

    #[test]
    fn failed_replacement_rolls_back_deleted_sources_and_events() {
        let db = store();
        let s = state();
        db.save_account_events(&[event(115, 10)], &s, &s.observed, &s.observed, at(120))
            .unwrap();
        db.conn.execute_batch("CREATE TRIGGER reject_checkpoint BEFORE UPDATE ON meta BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
        assert!(
            db.save_account_scan(
                &[event(115, 2)],
                &["same-conversation.jsonl".into()],
                &s,
                &None,
                &None,
                at(125)
            )
            .is_err()
        );
        assert_eq!(
            db.account_daily_for("new", at(0).date_naive(), at(200).date_naive())
                .unwrap()[0]
                .2
                .input_tokens,
            10
        );
        assert_eq!(
            db.conn
                .query_row("SELECT COUNT(*) FROM codex_event_sources", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn migration_keeps_distinct_full_path_cursors_for_identical_basenames() {
        let db = store();
        let now = Utc::now();
        let day = now.with_timezone(&Local).date_naive();
        let mut old_a = event(90, 100);
        old_a.source = "sessions/first/same.jsonl".into();
        old_a.session = "first".into();
        old_a.offset = 100;
        old_a.timestamp = now - Duration::seconds(20);
        let mut old_b = event(90, 50);
        old_b.source = "sessions/second/same.jsonl".into();
        old_b.session = "second".into();
        old_b.offset = 500;
        old_b.timestamp = now - Duration::seconds(20);
        let mut frozen = old_a.usage.clone();
        frozen.add(&old_b.usage);
        db.replace_usage_daily(
            ProviderKind::Codex,
            &[DailyTokenUsage {
                date: day,
                usage: frozen.clone(),
            }],
        )
        .unwrap();
        db.replace_usage_model_daily(ProviderKind::Codex, &[("model-a".into(), day, frozen)])
            .unwrap();
        for e in [&old_a, &old_b] {
            db.conn
                .execute(
                    "INSERT INTO scan_files VALUES('codex',?1,?2,?3)",
                    params![
                        e.source,
                        e.offset,
                        json!({"session_id":e.session}).to_string()
                    ],
                )
                .unwrap();
        }
        let state = db
            .initialize_codex_attribution_at(identity_for("only", 1), now)
            .unwrap();
        let mut tail = event(95, 7);
        tail.source = old_a.source.clone();
        tail.session = old_a.session.clone();
        tail.offset = 200;
        tail.timestamp = now - Duration::seconds(10);
        db.save_account_events(
            &[old_a, old_b, tail],
            &state,
            &state.observed,
            &state.observed,
            now,
        )
        .unwrap();
        assert_eq!(
            db.account_statistics_for("only", 30)
                .unwrap()
                .today
                .input_tokens,
            157
        );
        assert_eq!(
            db.account_daily_for("only", day, day).unwrap()[0]
                .2
                .input_tokens,
            157
        );
        assert_eq!(
            db.conn
                .query_row("SELECT COUNT(*) FROM codex_legacy_cursors", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            2
        );
    }

    #[test]
    fn upgrading_preview_recovers_source_links_without_reassigning_history() {
        let db = store();
        let s = state();
        db.save_account_events(&[event(115, 10)], &s, &s.observed, &s.observed, at(120))
            .unwrap();
        db.conn.execute("INSERT INTO scan_files VALUES('codex','sessions/path/same-conversation.jsonl',200,?1)",[json!({"session_id":"same-conversation"}).to_string()]).unwrap();
        db.conn
            .execute(
                "INSERT INTO codex_legacy_cursors VALUES('same-conversation.jsonl',100)",
                [],
            )
            .unwrap();
        db.conn.execute_batch("DROP TABLE codex_event_sources; DELETE FROM meta WHERE key='codex.event_sources.v1';").unwrap();
        db.migrate_codex_accounts().unwrap();
        db.migrate_codex_accounts().unwrap();
        assert_eq!(db.conn.query_row("SELECT offset FROM codex_legacy_cursors WHERE source='sessions/path/same-conversation.jsonl'",[],|r|r.get::<_,i64>(0)).unwrap(),100);
        assert_eq!(
            db.codex_attribution().unwrap().unwrap().historical_owner,
            "old"
        );
        db.save_account_scan(
            &[],
            &["sessions/path/same-conversation.jsonl".into()],
            &s,
            &None,
            &None,
            at(130),
        )
        .unwrap();
        assert!(
            db.account_daily_for("new", at(0).date_naive(), at(200).date_naive())
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn archival_preserves_migration_coverage_and_later_rewrites_remove_old_links() {
        let db = store();
        let s = state();
        let mut old = event(90, 100);
        old.source = "sessions/day/same.jsonl".into();
        old.offset = 100;
        db.conn
            .execute(
                "INSERT INTO codex_legacy_cursors VALUES(?1,100)",
                [&old.source],
            )
            .unwrap();
        old.source = "archived_sessions/day/same.jsonl".into();
        db.save_account_events(&[old], &s, &s.observed, &s.observed, at(120))
            .unwrap();
        assert!(
            db.conn
                .query_row("SELECT covered FROM codex_account_events", [], |r| r
                    .get::<_, bool>(0))
                .unwrap()
        );
        let mut live = event(115, 10);
        live.source = "sessions/day/same.jsonl".into();
        db.save_account_events(&[live], &s, &s.observed, &s.observed, at(120))
            .unwrap();
        let mut archived = event(115, 10);
        archived.source = "archived_sessions/day/same.jsonl".into();
        db.save_account_scan(
            &[archived],
            &["archived_sessions/day/same.jsonl".into()],
            &s,
            &s.observed,
            &s.observed,
            at(125),
        )
        .unwrap();
        db.save_account_scan(
            &[],
            &["archived_sessions/day/same.jsonl".into()],
            &s,
            &s.observed,
            &s.observed,
            at(130),
        )
        .unwrap();
        assert_eq!(
            db.conn
                .query_row("SELECT COUNT(*) FROM codex_account_events", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn migration_keeps_same_basename_cursors_separate() {
        let db = store();
        for (path, offset) in [
            ("sessions/2026/09/one/shared.jsonl", 100_i64),
            ("sessions/2026/09/two/shared.jsonl", 300_i64),
        ] {
            db.conn
                .execute(
                    "INSERT INTO scan_files(provider,path,offset,meta_json)
                     VALUES('codex',?1,?2,'{}')",
                    params![path, offset],
                )
                .unwrap();
        }

        db.initialize_codex_attribution_at(identity_for("only-account", 1), at(100))
            .unwrap();
        let mut query = db
            .conn
            .prepare("SELECT source,offset FROM codex_legacy_cursors ORDER BY source")
            .unwrap();
        let cursors = query
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(
            cursors,
            vec![
                ("sessions/2026/09/one/shared.jsonl".into(), 100),
                ("sessions/2026/09/two/shared.jsonl".into(), 300),
            ]
        );
    }

    #[test]
    fn source_reset_replaces_obsolete_account_events() {
        let db = store();
        let initial = state();
        let account = initial.observed.clone();
        let mut first = event(115, 10);
        first.source = "sessions/2026/09/session.jsonl".into();
        let mut second = event(116, 20);
        second.source = first.source.clone();
        let source = first.source.clone();
        db.save_account_events(&[first, second], &initial, &account, &account, at(120))
            .unwrap();

        let state = db.codex_attribution().unwrap().unwrap();
        let mut replacement = event(125, 7);
        replacement.source = source.clone();
        db.save_account_scan(
            &[replacement],
            &[source],
            &state,
            &account,
            &account,
            at(130),
        )
        .unwrap();

        let rows = db
            .account_daily_for("new", at(0).date_naive(), at(200).date_naive())
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].2.input_tokens, 7);
        assert_eq!(
            db.conn
                .query_row("SELECT COUNT(*) FROM codex_account_events", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            1
        );
    }
    #[test]
    fn upgrading_single_source_preview_recovers_explicit_links() {
        let db = store();
        let s = state();
        db.save_account_events(&[event(115, 10)], &s, &s.observed, &s.observed, at(120))
            .unwrap();
        // The maintainer's preview persisted a source directly on each event.
        // Its scanner metadata may already have been cleared for a rebuild.
        db.conn
            .execute_batch(
                "UPDATE codex_account_events SET source='sessions/old/rollout.jsonl';
            DROP TABLE codex_event_sources;
            DELETE FROM meta WHERE key='codex.event_sources.v1';",
            )
            .unwrap();
        db.migrate_codex_accounts().unwrap();
        db.migrate_codex_accounts().unwrap();
        assert_eq!(db.conn.query_row("SELECT COUNT(*) FROM codex_event_sources WHERE source='sessions/old/rollout.jsonl'", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
        assert_eq!(
            db.account_daily_for("new", at(0).date_naive(), at(200).date_naive())
                .unwrap()[0]
                .2
                .input_tokens,
            10
        );
        db.save_account_scan(
            &[],
            &["sessions/old/rollout.jsonl".into()],
            &s,
            &None,
            &None,
            at(130),
        )
        .unwrap();
        assert!(
            db.account_daily_for("new", at(0).date_naive(), at(200).date_naive())
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn legacy_shared_session_rows_survive_rebuilding_only_one_source() {
        let db = store();
        let s = state();
        db.save_account_events(
            &[event(115, 10), event(116, 20)],
            &s,
            &s.observed,
            &s.observed,
            at(120),
        )
        .unwrap();
        for path in ["sessions/a.jsonl", "sessions/b.jsonl"] {
            db.conn
                .execute(
                    "INSERT INTO scan_files VALUES('codex',?1,200,?2)",
                    params![path, json!({"session_id":"same-conversation"}).to_string()],
                )
                .unwrap();
        }
        db.conn.execute_batch("DROP TABLE codex_event_sources; DELETE FROM meta WHERE key='codex.event_sources.v1';").unwrap();
        db.migrate_codex_accounts().unwrap();
        // Before both old sources have been observed again, do not guess which
        // file contributed an event or erase another file's conversation rows.
        db.save_account_scan(&[], &["sessions/a.jsonl".into()], &s, &None, &None, at(130))
            .unwrap();
        assert_eq!(
            db.account_daily_for("new", at(0).date_naive(), at(200).date_naive())
                .unwrap()[0]
                .2
                .input_tokens,
            30
        );
        let mut remaining = event(116, 20);
        remaining.source = "sessions/b.jsonl".into();
        db.save_account_scan(
            &[remaining],
            &["sessions/b.jsonl".into()],
            &s,
            &None,
            &None,
            at(140),
        )
        .unwrap();
        assert_eq!(
            db.account_daily_for("new", at(0).date_naive(), at(200).date_naive())
                .unwrap()[0]
                .2
                .input_tokens,
            20
        );
    }
    #[test]
    fn equal_timestamp_nonconsecutive_usage_records_survive_incremental_scan_and_replay() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rollout.jsonl");
        let source = "sessions/day/rollout.jsonl";
        let line = |tokens| {
            json!({
                "timestamp": at(115).to_rfc3339(), "type":"event_msg",
                "payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":tokens}}}
            })
            .to_string()
        };
        let prefix = format!(
            "{}\n{}\n",
            json!({"type":"session_meta","payload":{"id":"conversation"}}),
            json!({"type":"turn_context","payload":{"model":"gpt-5.4"}})
        );
        fs::write(&path, format!("{prefix}{}\n", line(10))).unwrap();
        let db = store();
        let s = state();
        let mut cached = crate::usage::CachedSessionFile::default();
        let first = crate::usage::scan_file_delta(&path, source, &mut cached).unwrap();
        db.save_account_scan(
            &first.events,
            &[source.into()],
            &s,
            &s.observed,
            &s.observed,
            at(120),
        )
        .unwrap();
        fs::write(
            &path,
            format!("{prefix}{}\n{}\n{}\n", line(10), line(20), line(10)),
        )
        .unwrap();
        let tail = crate::usage::scan_file_delta(&path, source, &mut cached).unwrap();
        assert!(!tail.rebuilt);
        db.save_account_scan(&tail.events, &[], &s, &s.observed, &s.observed, at(120))
            .unwrap();
        assert_eq!(cached.daily[0].usage.input_tokens, 40);
        assert_eq!(
            db.account_daily_for("new", at(0).date_naive(), at(200).date_naive())
                .unwrap()[0]
                .2
                .input_tokens,
            40
        );
        let archived = "archived_sessions/day/rollout.jsonl";
        let replay =
            crate::usage::scan_file_delta(&path, archived, &mut Default::default()).unwrap();
        db.save_account_scan(
            &replay.events,
            &[archived.into()],
            &s,
            &None,
            &None,
            at(130),
        )
        .unwrap();
        let rows = db
            .account_daily_for("new", at(0).date_naive(), at(200).date_naive())
            .unwrap();
        assert_eq!(rows[0].2.input_tokens, 40);
        assert_eq!(rows[0].2.requests, 3);
    }
    #[test]
    fn record_identity_upgrade_keeps_legacy_owner_coverage_and_other_sources() {
        let db = store();
        let s = state();
        let mut first = event(115, 10);
        first.source = "sessions/a.jsonl".into();
        let mut second = event(115, 10);
        second.source = "sessions/b.jsonl".into();
        db.save_account_events(&[first, second], &s, &s.observed, &s.observed, at(120))
            .unwrap();
        // Simulate a preview's old key, retained by a second unavailable file.
        db.conn
            .execute_batch(
                "UPDATE codex_account_events SET signature='usage-10', covered=1;
            UPDATE codex_event_sources SET signature='usage-10';",
            )
            .unwrap();
        let mut replay = event(115, 10);
        replay.source = "sessions/a.jsonl".into();
        db.save_account_scan(
            &[replay],
            &["sessions/a.jsonl".into()],
            &s,
            &None,
            &None,
            at(130),
        )
        .unwrap();
        let (owner, covered, signature): (String, bool, String) = db
            .conn
            .query_row(
                "SELECT account,covered,signature FROM codex_account_events",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(owner, "new");
        assert!(covered);
        assert!(signature.starts_with("record:"));
        assert_eq!(
            db.conn
                .query_row("SELECT COUNT(*) FROM codex_account_events", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        db.save_account_scan(&[], &["sessions/a.jsonl".into()], &s, &None, &None, at(140))
            .unwrap();
        assert_eq!(
            db.conn
                .query_row("SELECT COUNT(*) FROM codex_account_events", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        db.save_account_scan(&[], &["sessions/b.jsonl".into()], &s, &None, &None, at(150))
            .unwrap();
        assert_eq!(
            db.conn
                .query_row("SELECT COUNT(*) FROM codex_account_events", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}
