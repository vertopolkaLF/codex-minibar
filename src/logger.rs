//! Small, dependency-free application log used by the in-app Log tab.

use std::{
    collections::VecDeque,
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::{Context, Result};
use chrono::Local;

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Creates `log.txt` beside the user settings file and starts a new session.
pub fn initialize(settings_path: &Path) -> Result<PathBuf> {
    let path = settings_path.with_file_name("log.txt");
    let _ = LOG_PATH.set(path.clone());
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("open {}", path.display()))?;
    info("Codex Minibar started");
    info(format!("Log file: {}", path.display()));
    Ok(path)
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
