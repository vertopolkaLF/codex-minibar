//! Every file and folder the app keeps on disk.
//!
//! `MINIBAR_FAKE_PROVIDERS=1` redirects all of them into a temp sandbox so a
//! test build never reads or overwrites the real install.

use std::{path::PathBuf, sync::OnceLock};

use anyhow::{Context, Result};
use directories::ProjectDirs;

pub fn fake_sandbox() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var_os("MINIBAR_FAKE_PROVIDERS").is_some_and(|value| value != "0")
    })
}

fn sandbox_dir() -> PathBuf {
    std::env::temp_dir().join("codex-minibar-fake")
}

fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("dev", "Codex Minibar", "Codex Minibar")
        .context("could not resolve the application directories")
}

/// Roaming config folder holding settings, secrets, caches and the log.
pub fn config_dir() -> Result<PathBuf> {
    if fake_sandbox() {
        return Ok(sandbox_dir());
    }
    Ok(project_dirs()?.config_dir().to_path_buf())
}

fn data_local_dir() -> Result<PathBuf> {
    if fake_sandbox() {
        return Ok(sandbox_dir());
    }
    Ok(project_dirs()?.data_local_dir().to_path_buf())
}

pub fn settings() -> Result<PathBuf> {
    Ok(config_dir()?.join("settings.toml"))
}

pub fn secrets() -> Result<PathBuf> {
    Ok(config_dir()?.join("provider-secrets.json"))
}

pub fn log() -> Result<PathBuf> {
    Ok(config_dir()?.join("log.txt"))
}

pub fn store_db() -> Result<PathBuf> {
    Ok(config_dir()?.join("provider-store.sqlite"))
}

/// Base activation state; other instances derive siblings from it.
pub fn activation_state() -> Result<PathBuf> {
    Ok(config_dir()?.join("activation.toml"))
}

pub fn reset_feed_cache() -> Result<PathBuf> {
    Ok(config_dir()?.join("codex-resets-cache.json"))
}

pub fn provider_status_cache() -> Result<PathBuf> {
    Ok(config_dir()?.join("provider-status-cache.json"))
}

pub fn streamdeck_endpoint() -> Result<PathBuf> {
    Ok(config_dir()?.join("streamdeck-bridge.json"))
}

pub fn whats_new_state() -> Result<PathBuf> {
    Ok(config_dir()?.join("whats-new.json"))
}

pub fn command_palette_icons() -> Result<PathBuf> {
    Ok(config_dir()?.join("command-palette").join("icons"))
}

pub fn vscode_themes() -> Result<PathBuf> {
    Ok(config_dir()?.join("vscode-themes"))
}

pub fn claude_logins() -> Result<PathBuf> {
    Ok(config_dir()?.join("claude-logins"))
}

pub fn codex_logins() -> Result<PathBuf> {
    Ok(config_dir()?.join("codex-logins"))
}

/// App-managed `CLAUDE_CONFIG_DIR` / `CODEX_HOME` folders, one per instance.
pub fn managed_instances() -> Result<PathBuf> {
    Ok(data_local_dir()?.join("instances"))
}
