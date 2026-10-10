//! Signs a Codex instance in by running Codex's own login with the instance's
//! `CODEX_HOME`. Codex keeps and refreshes the login there; Minibar only reads
//! it.
pub(crate) use crate::claude::profile_oauth::LoginControl;
use anyhow::{Context, Result, bail, ensure};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// Environment variables that would make Codex ignore the folder's login.
pub(crate) const AUTH_OVERRIDES: &[&str] = &[
    "OPENAI_API_KEY",
    "OPENAI_BASE_URL",
    "OPENAI_ORGANIZATION",
    "OPENAI_ORG_ID",
    "OPENAI_PROJECT_ID",
    "CODEX_API_KEY",
    "CODEX_AUTH_TOKEN",
    "CODEX_INTERNAL_ORIGINATOR_OVERRIDE",
];

fn login_command(executable: &Path, directory: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .args(["-c", "cli_auth_credentials_store=\"file\"", "login"])
        .env("CODEX_HOME", directory)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for key in AUTH_OVERRIDES {
        command.env_remove(key);
    }
    command
}

fn native_login_executable(explicit: Option<&Path>) -> Option<PathBuf> {
    crate::discovery::discover(explicit)
        .into_iter()
        .find_map(|candidate| native_for_candidate(&candidate.path))
}

fn native_for_candidate(candidate: &Path) -> Option<PathBuf> {
    if candidate
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    {
        return Some(candidate.to_owned());
    }
    // npm's shim delegates to either an optional platform package or its own
    // vendor directory. Both current vendor/<target>/bin and legacy /codex
    // layouts are supported. No shell launcher is needed for browser login.
    let root = candidate.parent()?;
    let targets = if cfg!(target_arch = "aarch64") {
        ["aarch64-pc-windows-msvc", "x86_64-pc-windows-msvc"]
    } else {
        ["x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"]
    };
    for target in targets {
        let platform_package = if target.starts_with("aarch64") {
            "codex-win32-arm64"
        } else {
            "codex-win32-x64"
        };
        let main = root.join("node_modules/@openai/codex");
        let roots = [
            main.join("node_modules/@openai").join(platform_package),
            root.join("node_modules/@openai").join(platform_package),
            main,
        ];
        for package in roots {
            for directory in ["bin", "codex"] {
                let native = package
                    .join("vendor")
                    .join(target)
                    .join(directory)
                    .join("codex.exe");
                if native.is_file() {
                    return Some(native);
                }
            }
        }
    }
    None
}

/// A ChatGPT login with a refresh token; API-key-only logins report no
/// subscription limits.
fn read_login(directory: &Path) -> Result<()> {
    let bytes = std::fs::read(directory.join("auth.json")).context(
        "Codex did not save a login. Install native Codex CLI or Codex desktop and try again.",
    )?;
    let file: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("Codex saved an unreadable login."))?;
    let tokens = &file["tokens"];
    let present = |key: &str| {
        tokens[key]
            .as_str()
            .is_some_and(|value| !value.trim().is_empty())
    };
    ensure!(
        present("access_token") && present("refresh_token"),
        "Codex did not save a refreshable ChatGPT login."
    );
    Ok(())
}

fn login_root() -> Result<std::path::PathBuf> {
    Ok(crate::settings::Settings::default_path()?
        .parent()
        .context("settings directory is unavailable")?
        .join("codex-logins"))
}

/// Removes temporary sign-in folders left by earlier versions. Never follows
/// reparse points.
pub(crate) fn cleanup_abandoned_logins() -> Result<()> {
    cleanup_root(&login_root()?)
}

fn cleanup_root(root: &Path) -> Result<()> {
    use crate::claude::profile_oauth::is_link;
    let metadata = match std::fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).context("read Codex sign-in directory"),
    };
    ensure!(
        metadata.is_dir() && !is_link(&metadata),
        "Codex sign-in directory must not be a link."
    );
    let root = root.canonicalize()?;
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        if !entry.file_name().to_string_lossy().starts_with("login-") {
            continue;
        }
        let metadata = std::fs::symlink_metadata(entry.path())?;
        if !metadata.is_dir() || is_link(&metadata) {
            continue;
        }
        let path = entry.path().canonicalize()?;
        ensure!(
            path.parent() == Some(root.as_path()),
            "Codex sign-in directory escaped its root."
        );
        std::fs::remove_dir_all(path)?;
    }
    let _ = std::fs::remove_dir(&root);
    Ok(())
}

/// Runs `codex login` with `folder` as `CODEX_HOME`.
pub(crate) fn login(explicit: Option<&Path>, folder: &Path, control: &LoginControl) -> Result<()> {
    ensure!(!control.cancelled(), "Codex sign-in cancelled.");
    let executable = native_login_executable(explicit)
        .context("Install native Windows Codex CLI or Codex desktop to sign in from Minibar.")?;
    let prepared = crate::discovery::prepare(&executable)?;
    let executable = &prepared.path;
    ensure!(
        executable
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe")),
        "Sign-in requires native Windows Codex CLI or Codex desktop. Install Codex desktop or the native CLI and try again."
    );
    std::fs::create_dir_all(folder).with_context(|| format!("create {}", folder.display()))?;
    ensure!(
        !crate::claude::profile_oauth::is_link(&std::fs::symlink_metadata(folder)?),
        "The config folder must not be a link."
    );
    let command = login_command(executable, folder);
    #[cfg(windows)]
    let mut child = crate::claude::profile_oauth::login_child::LoginChild::spawn(&command)?;
    #[cfg(not(windows))]
    let mut child = {
        let mut command = command;
        command.spawn()?
    };
    let started = Instant::now();
    loop {
        if control.cancelled() || started.elapsed() >= Duration::from_secs(300) {
            let _ = child.kill();
            let _ = child.wait();
            ensure!(!control.cancelled(), "Codex sign-in cancelled.");
            bail!(
                "Codex sign-in timed out. Finish the browser login within five minutes. Try again after closing an unfinished sign-in."
            );
        }
        let status = match child.try_wait() {
            Ok(status) => status,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                bail!("Could not wait for Codex sign-in.");
            }
        };
        if let Some(status) = status {
            ensure!(
                status.success(),
                "Codex sign-in did not finish (exit {}). Try signing in again.",
                status.code().unwrap_or(-1)
            );
            control.begin_save()?;
            return read_login(folder);
        }
        thread::sleep(Duration::from_millis(150));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_targets_the_instance_folder_and_removes_auth_overrides() {
        let command = login_command(Path::new("codex.exe"), Path::new("instance"));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["-c", "cli_auth_credentials_store=\"file\"", "login"]
        );
        assert_eq!(command.get_current_dir(), Some(Path::new("instance")));
        assert!(
            command.get_envs().any(|(key, value)| key == "CODEX_HOME"
                && value == Some(std::ffi::OsStr::new("instance")))
        );
        for key in AUTH_OVERRIDES {
            assert!(
                command
                    .get_envs()
                    .any(|(name, value)| name == *key && value.is_none())
            );
        }
    }

    #[test]
    fn resolves_native_npm_binaries_in_current_legacy_and_nested_layouts() {
        let root = tempfile::tempdir().unwrap();
        let candidate = root.path().join("codex.cmd");
        for relative in [
            "node_modules/@openai/codex/vendor/x86_64-pc-windows-msvc/codex/codex.exe",
            "node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe",
            "node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe",
        ] {
            let path = root.path().join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"fixture").unwrap();
            assert_eq!(native_for_candidate(&candidate), Some(path.clone()));
            std::fs::remove_file(path).unwrap();
        }
        assert_eq!(native_for_candidate(&candidate), None);
    }

    #[test]
    fn reads_full_login_and_rejects_api_key_only_auth() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("auth.json");
        std::fs::write(&path, r#"{"tokens":{"access_token":"fixture-access","refresh_token":"fixture-refresh","id_token":"fixture-id","account_id":"workspace-fixture"}}"#).unwrap();
        assert!(read_login(directory.path()).is_ok());
        std::fs::write(&path, r#"{"OPENAI_API_KEY":"fixture-api-key"}"#).unwrap();
        assert!(read_login(directory.path()).is_err());
    }
}
