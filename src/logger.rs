//! Small, dependency-free application log used by the in-app Log tab.

use std::{
    collections::VecDeque,
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::{Context, Result};
use chrono::{DateTime, Local};

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
const MAX_ARCHIVED_LOGS: usize = 10;

/// Archives the preceding `log.txt`, creates a clean one, and starts a new session.
pub fn initialize(settings_path: &Path) -> Result<PathBuf> {
    let path = settings_path.with_file_name("log.txt");
    rotate_log(&path)?;
    let _ = LOG_PATH.set(path.clone());
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .with_context(|| format!("open {}", path.display()))?;
    info("Codex Minibar started");
    info(format!("Log file: {}", path.display()));
    Ok(path)
}

fn rotate_log(path: &Path) -> Result<()> {
    if path.exists() {
        let archive = next_archive_path(path, Local::now())?;
        fs::rename(path, &archive)
            .with_context(|| format!("archive {} as {}", path.display(), archive.display()))?;
    }
    prune_archived_logs(path)?;
    Ok(())
}

fn next_archive_path(path: &Path, now: DateTime<Local>) -> Result<PathBuf> {
    let directory = path.parent().context("log path has no parent")?;
    let timestamp = now.format("%Y-%m-%d_%H-%M-%S");
    let mut suffix = 0usize;
    loop {
        let name = if suffix == 0 {
            format!("{timestamp}-log.txt")
        } else {
            format!("{timestamp}-{suffix}-log.txt")
        };
        let candidate = directory.join(name);
        if !candidate.exists() {
            return Ok(candidate);
        }
        suffix += 1;
    }
}

fn prune_archived_logs(path: &Path) -> Result<()> {
    let directory = path.parent().context("log path has no parent")?;
    let mut archives = fs::read_dir(directory)
        .with_context(|| format!("list {}", directory.display()))?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter(|entry| is_archive_name(&entry.file_name().to_string_lossy()))
        .collect::<Vec<_>>();
    archives.sort_by_key(|entry| entry.file_name());
    let excess = archives.len().saturating_sub(MAX_ARCHIVED_LOGS);
    for archive in archives.into_iter().take(excess) {
        fs::remove_file(archive.path())
            .with_context(|| format!("remove old log archive {}", archive.path().display()))?;
    }
    Ok(())
}

fn is_archive_name(name: &str) -> bool {
    name.ends_with("-log.txt") && name.len() >= "0000-00-00_00-00-00-log.txt".len()
}

pub fn path() -> Option<&'static Path> {
    LOG_PATH.get().map(PathBuf::as_path)
}

/// Appends one timestamped event. Logging must never disrupt provider polling.
pub fn info(message: impl AsRef<str>) {
    let Some(path) = path() else {
        return;
    };
    let line = format!(
        "[{}] {}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        message.as_ref()
    );
    if let Err(error) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| file.write_all(line.as_bytes()))
    {
        eprintln!("failed to write {}: {error}", path.display());
    }
}

/// Reads only the requested tail, keeping the UI responsive even after a long run.
pub fn tail_lines(max_lines: usize) -> Result<String> {
    let Some(path) = path() else {
        return Ok("Log is not initialized yet.".into());
    };
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
    };
    let mut lines = VecDeque::with_capacity(max_lines);
    for line in BufReader::new(file).lines() {
        let line = line.with_context(|| format!("read {}", path.display()))?;
        if lines.len() == max_lines {
            lines.pop_front();
        }
        lines.push_back(line);
    }
    Ok(lines.into_iter().collect::<Vec<_>>().join("\n"))
}

pub fn open() -> Result<()> {
    let path = path().context("log path is not initialized")?;
    crate::updater::open_url(&path.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_archives_the_previous_log_and_keeps_ten_archives() {
        let directory = tempfile::tempdir().unwrap();
        let log = directory.path().join("log.txt");
        fs::write(&log, "previous session").unwrap();
        for second in 0..10 {
            fs::write(
                directory
                    .path()
                    .join(format!("2026-07-28_23-00-{second:02}-log.txt")),
                "old",
            )
            .unwrap();
        }

        rotate_log(&log).unwrap();

        assert!(!log.exists());
        let archives = fs::read_dir(directory.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| is_archive_name(&entry.file_name().to_string_lossy()))
            .count();
        assert_eq!(archives, MAX_ARCHIVED_LOGS);
    }
}
