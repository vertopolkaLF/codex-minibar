//! Signs a Claude instance in by running Claude Code's own login inside the
//! instance's config folder. Claude Code keeps and refreshes the login there;
//! Minibar only reads it.
use std::{
    path::Path,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

#[cfg(windows)]
pub(crate) mod login_child;

/// 0 = waiting; 1 = cancelled; 2 = saving. Cancellation cannot race a commit.
#[derive(Clone, Default)]
pub(crate) struct LoginControl(Arc<AtomicU8>);

impl PartialEq for LoginControl {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl LoginControl {
    pub(crate) fn cancel(&self) {
        let _ = self
            .0
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst);
    }
    pub(crate) fn cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst) == 1
    }
    pub(crate) fn begin_save(&self) -> Result<()> {
        ensure!(
            self.0
                .compare_exchange(0, 2, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok(),
            "Sign-in cancelled."
        );
        Ok(())
    }
}

/// The parts of `claudeAiOauth` that make a login usable for limits.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Session {
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    scopes: Vec<String>,
}

impl Session {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.access_token.starts_with("sk-ant-oat") && !self.refresh_token.trim().is_empty(),
            "Claude Code did not save a refreshable subscription login."
        );
        ensure!(
            self.scopes.iter().any(|scope| scope == "user:profile"),
            "Claude login has no subscription usage access (user:profile)."
        );
        Ok(())
    }
}

fn login_command(executable: &Path, directory: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .args(["auth", "login", "--claudeai"])
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    super::scope_command(&mut command, Some(directory));
    command
}

fn read_login(directory: &Path) -> Result<()> {
    let bytes = std::fs::read(directory.join(".credentials.json")).context(
        "Claude Code did not save a login. Install native Windows Claude Code and try again.",
    )?;
    let file: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("Claude Code saved an unreadable login."))?;
    let session: Session = serde_json::from_value(file["claudeAiOauth"].clone()).map_err(|_| {
        anyhow::anyhow!("Claude Code did not save a refreshable subscription login.")
    })?;
    session.validate()
}

fn login_root() -> Result<std::path::PathBuf> {
    Ok(crate::settings::Settings::default_path()?
        .parent()
        .context("settings directory is unavailable")?
        .join("claude-logins"))
}

/// Removes temporary sign-in folders left by earlier versions. Never follows
/// reparse points.
pub(crate) fn cleanup_abandoned_logins() -> Result<()> {
    cleanup_root(&login_root()?)
}

pub(crate) fn is_link(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    metadata.file_type().is_symlink()
}

fn cleanup_root(root: &Path) -> Result<()> {
    let metadata = match std::fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).context("read Claude sign-in directory"),
    };
    ensure!(
        metadata.is_dir() && !is_link(&metadata),
        "Claude sign-in directory must not be a link."
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
            "Claude sign-in directory escaped its root."
        );
        std::fs::remove_dir_all(path)?;
    }
    let _ = std::fs::remove_dir(&root);
    Ok(())
}

/// Runs `claude auth login` with `folder` as `CLAUDE_CONFIG_DIR`. The browser
/// login lands in the folder, where Claude Code keeps refreshing it.
pub(crate) fn login(explicit: Option<&Path>, folder: &Path, control: &LoginControl) -> Result<()> {
    ensure!(!control.cancelled(), "Claude sign-in cancelled.");
    let executable = super::cli_available(explicit)
        .or_else(super::claude_desktop::bundled_cli)
        .context("Install native Windows Claude Code to sign in from Minibar.")?;
    ensure!(
        executable
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe")),
        "Sign-in requires native Windows Claude Code. Install its native executable, or switch Source to Manual."
    );
    std::fs::create_dir_all(folder)
        .with_context(|| format!("create {}", folder.display()))?;
    ensure!(
        !is_link(&std::fs::symlink_metadata(folder)?),
        "The config folder must not be a link."
    );
    let command = login_command(&executable, folder);
    #[cfg(windows)]
    let mut child = login_child::LoginChild::spawn(&command)?;
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
            ensure!(!control.cancelled(), "Claude sign-in cancelled.");
            bail!(
                "Claude sign-in timed out. Finish the browser login within five minutes, or switch Source to Manual and paste a token."
            );
        }
        let status = match child.try_wait() {
            Ok(status) => status,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                bail!("Could not wait for Claude Code sign-in.");
            }
        };
        if let Some(status) = status {
            ensure!(
                status.success(),
                "Claude sign-in did not finish (exit {}). Try again.",
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
    fn abandoned_login_cleanup_keeps_other_directories_and_files() {
        let root = tempfile::tempdir().unwrap();
        let login = root.path().join("login-fixture");
        std::fs::create_dir(&login).unwrap();
        std::fs::write(login.join(".credentials.json"), "fixture").unwrap();
        let keep = root.path().join("unrelated");
        std::fs::create_dir(&keep).unwrap();
        std::fs::write(root.path().join("login-file"), "keep").unwrap();
        cleanup_root(root.path()).unwrap();
        assert!(!login.exists());
        assert!(keep.exists());
        assert!(root.path().join("login-file").exists());
    }

    #[test]
    fn login_command_targets_the_instance_folder_and_removes_auth_overrides() {
        let command = login_command(Path::new("claude.exe"), Path::new("instance"));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["auth", "login", "--claudeai"]
        );
        assert_eq!(command.get_current_dir(), Some(Path::new("instance")));
        for key in super::super::AUTH_OVERRIDES {
            assert!(
                command
                    .get_envs()
                    .any(|(name, value)| name == *key && value.is_none())
            );
        }
        assert!(
            command
                .get_envs()
                .any(|(name, value)| name == "CLAUDE_CONFIG_DIR"
                    && value == Some(std::ffi::OsStr::new("instance")))
        );
    }

    #[test]
    fn login_requires_refresh_and_usage_scope() {
        let folder = tempfile::tempdir().unwrap();
        let write = |value: serde_json::Value| {
            std::fs::write(
                folder.path().join(".credentials.json"),
                serde_json::json!({ "claudeAiOauth": value }).to_string(),
            )
            .unwrap();
        };
        write(serde_json::json!({"accessToken":"sk-ant-oat-x","refreshToken":"","scopes":["user:profile"]}));
        assert!(read_login(folder.path()).is_err());
        write(serde_json::json!({"accessToken":"sk-ant-oat-x","refreshToken":"r","scopes":["user:inference"]}));
        assert!(read_login(folder.path()).is_err());
        write(serde_json::json!({"accessToken":"sk-ant-oat-x","refreshToken":"r","scopes":["user:profile"]}));
        assert!(read_login(folder.path()).is_ok());
    }

    #[test]
    fn cancellation_and_save_are_mutually_exclusive() {
        let control = LoginControl::default();
        control.cancel();
        assert!(control.begin_save().is_err());
        let control = LoginControl::default();
        control.begin_save().unwrap();
        control.cancel();
        assert!(!control.cancelled());
    }
}
