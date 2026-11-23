//! Local usage from Antigravity's conversation databases. The metadata fields
//! and discovery paths follow T3 Code's Antigravity usage reader.

mod metadata;
#[cfg(test)]
mod tests;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    env, fs, io,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime},
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Days, Local, Utc};
use rusqlite::{Connection, OpenFlags};

use self::metadata::{Call, Database, decode_database, rate_model};
use crate::{
    platform::local_time::RETENTION_DAYS,
    providers::{instances::ProviderId, registry::ProviderKind, worker::UsageProvider},
    store,
    usage::{
        DailyTokenUsage, Rollup, RollupBuilder, TokenUsage, UsageStatistics,
        pricing::request_usage, statistics_from_daily,
    },
};

pub(super) struct Reader {
    provider: ProviderId,
    profile: Option<PathBuf>,
    databases: BTreeMap<PathBuf, CachedDatabase>,
    cancelled: Arc<AtomicBool>,
}

struct CachedDatabase {
    fingerprint: Fingerprint,
    data: Database,
}

struct Scan {
    rollup: Rollup,
    files: Vec<(String, DailyTokenUsage)>,
}

#[derive(PartialEq, Eq)]
struct Fingerprint {
    db: FileStamp,
    wal: Option<FileStamp>,
}

#[derive(PartialEq, Eq)]
struct FileStamp {
    len: u64,
    modified: SystemTime,
    #[cfg(unix)]
    changed: (i64, i64, u64, u64),
}

impl FileStamp {
    fn read(path: &Path) -> io::Result<Self> {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;

        let stat = fs::metadata(path)?;
        Ok(Self {
            len: stat.len(),
            modified: stat.modified()?,
            #[cfg(unix)]
            changed: (stat.ctime(), stat.ctime_nsec(), stat.dev(), stat.ino()),
        })
    }
}

impl Fingerprint {
    fn read(path: &Path) -> io::Result<Self> {
        let mut wal = path.as_os_str().to_os_string();
        wal.push("-wal");
        Ok(Self {
            db: FileStamp::read(path)?,
            wal: optional(FileStamp::read(Path::new(&wal)))?,
        })
    }
}

impl Reader {
    pub(super) fn new(provider: ProviderId, profile: Option<PathBuf>) -> Self {
        Self {
            provider,
            profile,
            databases: BTreeMap::new(),
            cancelled: Arc::default(),
        }
    }

    fn scan(
        &mut self,
        roots: Vec<PathBuf>,
        excluded: &[PathBuf],
        now: DateTime<Local>,
    ) -> Result<Scan> {
        let paths = conversation_databases(roots, excluded)?;
        self.databases.retain(|path, _| paths.contains(path));
        let mut incomplete = false;
        for path in paths {
            self.check_cancelled()?;
            if let Err(error) = self.update_database(&path) {
                incomplete |= !self.databases.contains_key(&path);
                crate::platform::logger::info(format!(
                    "Antigravity history: could not read {}: {error:#}",
                    path.display()
                ));
            }
        }

        let oldest = now.date_naive() - Days::new(u64::from(RETENTION_DAYS) - 1);
        let mut models = HashMap::new();
        let mut calls = Vec::new();
        let mut sources = Vec::new();
        for (index, (path, cached)) in self.databases.iter().enumerate() {
            sources.push(path.to_string_lossy());
            for (id, model) in &cached.data.models {
                metadata::remember_model(&mut models, *id, model.as_deref());
            }
            calls.extend(cached.data.calls.iter().cloned().map(|call| (index, call)));
        }
        let mut rollup = RollupBuilder::new(now);
        let mut per_file = BTreeMap::<_, TokenUsage>::new();
        for (source, call) in Call::deduplicate(calls, &models) {
            let at = call.at.with_timezone(&Local);
            let date = at.date_naive();
            if date < oldest {
                continue;
            }
            let usage = request_usage(
                ProviderKind::Antigravity,
                Some(rate_model(&call.model)),
                call.tokens(),
                None,
            );
            rollup.add(at, &usage, Some(&call.model), None);
            per_file.entry((source, date)).or_default().add(&usage);
        }
        let rollup = rollup.finish();
        if incomplete && rollup.daily.is_empty() {
            bail!("Antigravity conversation history could not be read");
        }
        let files = per_file
            .into_iter()
            .map(|((source, date), usage)| {
                (sources[source].to_string(), DailyTokenUsage { date, usage })
            })
            .collect();
        Ok(Scan { rollup, files })
    }

    fn check_cancelled(&self) -> Result<()> {
        if self.cancelled.load(Ordering::Acquire) {
            bail!("Antigravity history scan cancelled");
        }
        Ok(())
    }

    fn update_database(&mut self, path: &Path) -> Result<()> {
        let fingerprint = Fingerprint::read(path)?;
        if self
            .databases
            .get(path)
            .is_some_and(|cached| cached.fingerprint == fingerprint)
        {
            return Ok(());
        }
        let mut connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        connection.busy_timeout(Duration::from_millis(100))?;
        let snapshot = connection.transaction()?;
        let fallback = DateTime::<Utc>::from(fingerprint.db.modified);
        let data = decode_database(&snapshot, fallback)?;
        snapshot.commit()?;
        self.databases
            .insert(path.to_owned(), CachedDatabase { fingerprint, data });
        Ok(())
    }
}

impl UsageProvider for Reader {
    fn set_cancellation(&mut self, cancelled: Arc<AtomicBool>) {
        self.cancelled = cancelled;
    }

    fn load_cached_usage_statistics(&mut self, history_days: u16) -> Result<UsageStatistics> {
        store::with_store(|store| store.load_usage_daily(self.provider, history_days))
    }

    fn refresh_usage_statistics(&mut self, history_days: u16) -> Result<UsageStatistics> {
        let (roots, excluded) = if let Some(profile) = &self.profile {
            (vec![profile.join("antigravity-acp")], Vec::new())
        } else {
            let home = directories::BaseDirs::new();
            let roots = data_roots(
                home.as_ref().map(directories::BaseDirs::home_dir),
                env::var("ANTIGRAVITY_DATA_DIR").ok().as_deref(),
                env::var("T3CODE_HOME").ok().as_deref(),
            )?;
            // Explicit profiles publish their own usage. Keep discovering
            // other T3 profiles for existing setups with one Minibar instance.
            let excluded = super::configured_profiles(&crate::settings::store::read().instances)
                .into_iter()
                .filter_map(|profile| profile.join("antigravity-acp").canonicalize().ok())
                .collect();
            (roots, excluded)
        };
        let scan = self.scan(roots, &excluded, Local::now())?;
        store::with_store(|store| {
            self.check_cancelled()?;
            store.replace_usage_rollup(self.provider, &scan.rollup, &scan.files)
        })?;
        Ok(statistics_from_daily(&scan.rollup.daily, history_days))
    }

    fn refresh_without_limits(&self) -> bool {
        true
    }
}

fn data_roots(
    home: Option<&Path>,
    configured: Option<&str>,
    t3_home: Option<&str>,
) -> Result<Vec<PathBuf>> {
    let expand_home = |value: &str| {
        for prefix in ["~/", "~\\"] {
            if let Some((home, rest)) = home.zip(value.strip_prefix(prefix)) {
                return home.join(rest);
            }
        }
        if value == "~"
            && let Some(home) = home
        {
            return home.to_owned();
        }
        PathBuf::from(value)
    };
    let mut roots: Vec<_> = configured
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(expand_home)
        .collect();
    if roots.is_empty()
        && let Some(home) = home
    {
        roots.extend(
            [
                ".gemini/antigravity",
                ".gemini/antigravity-cli",
                ".gemini/antigravity-ide",
                ".gemini/antigravity-backup",
                ".config/antigravity",
            ]
            .map(|name| home.join(name)),
        );
    }
    let t3_home = t3_home
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(expand_home)
        .or_else(|| home.map(|home| home.join(".t3")));
    if let Some(t3_home) = t3_home {
        for state in ["userdata", "dev"] {
            let profiles = t3_home.join(state).join("providers/antigravity");
            for (path, kind) in children(&profiles)? {
                if kind.is_dir() {
                    roots.push(path.join("antigravity-acp"));
                }
            }
        }
    }
    Ok(roots)
}

fn optional<T>(result: io::Result<T>) -> io::Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn children(path: &Path) -> Result<Vec<(PathBuf, fs::FileType)>> {
    let read = || -> io::Result<_> {
        let Some(entries) = optional(fs::read_dir(path))? else {
            return Ok(Vec::new());
        };
        entries
            .map(|entry| {
                let entry = entry?;
                Ok((entry.path(), entry.file_type()?))
            })
            .collect()
    };
    read().with_context(|| format!("list Antigravity history directory {}", path.display()))
}

fn conversation_databases(roots: Vec<PathBuf>, excluded: &[PathBuf]) -> Result<BTreeSet<PathBuf>> {
    let mut pending = Vec::new();
    for root in roots {
        let nested = root.join("conversations");
        let nested_meta = optional(fs::metadata(&nested)).with_context(|| {
            format!("inspect Antigravity history directory {}", nested.display())
        })?;
        pending.push(if nested_meta.is_some_and(|meta| meta.is_dir()) {
            nested
        } else {
            root
        });
    }
    let mut visited = BTreeSet::new();
    let mut databases = BTreeSet::new();
    while let Some(dir) = pending.pop() {
        let Some(dir) = optional(dir.canonicalize())
            .with_context(|| format!("resolve Antigravity history directory {}", dir.display()))?
        else {
            continue;
        };
        if excluded.iter().any(|profile| dir.starts_with(profile)) || !visited.insert(dir.clone()) {
            continue;
        }
        for (path, kind) in children(&dir)? {
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file() && path.extension().is_some_and(|extension| extension == "db")
            {
                databases.insert(path);
            }
        }
    }
    Ok(databases)
}
