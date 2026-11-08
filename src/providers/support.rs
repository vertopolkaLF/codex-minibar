//! Plumbing shared by provider drivers: hidden CLI launches, CLI discovery,
//! and the browser sign-in handoff.

use std::{
    collections::HashSet,
    env,
    ffi::OsStr,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Result, bail, ensure};

#[cfg(windows)]
pub(crate) mod login_child;

/// Self-updaters in provider CLIs relaunch a detached copy of themselves.
/// `CREATE_NO_WINDOW` does not reach that copy, so Windows gives it a visible
/// console that flashes and steals focus. Only Minibar's own launches opt out;
/// the CLIs still update normally from the user's terminal.
const AUTO_UPDATE_OPT_OUTS: &[(&str, &str)] = &[
    // Antigravity accepts only "true".
    ("AGY_CLI_DISABLE_AUTO_UPDATE", "true"),
    ("DISABLE_AUTOUPDATER", "1"),
    ("KIRO_NO_AUTO_UPDATE", "1"),
];

/// A provider CLI launch without a console window. `.ps1` launchers run
/// through PowerShell; `.cmd`/`.bat` shims are spawned directly, so std
/// quotes their arguments and refuses any it cannot pass to cmd.exe safely.
pub(crate) fn cli_command(
    program: &Path,
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
) -> Command {
    let powershell = cfg!(windows)
        && program
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("ps1"));
    let mut command = if powershell {
        let mut command = Command::new("powershell.exe");
        command
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(program);
        command
    } else {
        Command::new(program)
    };
    command.args(args);
    for (key, value) in AUTO_UPDATE_OPT_OUTS {
        command.env(key, value);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    command
}

/// Waits up to `timeout` for `child`. A child that is still running then, or
/// that cannot be checked, is killed; `Ok(None)` reports the timeout.
pub(crate) fn wait_with_timeout(
    child: &mut Child,
    timeout: Duration,
) -> io::Result<Option<ExitStatus>> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(Some(status)),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                terminate(child);
                return Ok(None);
            }
            Err(error) => {
                terminate(child);
                return Err(error);
            }
        }
    }
}

pub(crate) fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Runs the first launchable CLI so it renews the session it owns.
pub(crate) fn refresh_session(
    provider: &str,
    candidates: &[PathBuf],
    args: &[&str],
    timeout: Duration,
) -> Result<()> {
    for program in candidates {
        if !program.is_absolute() {
            continue;
        }
        let mut child = match cli_command(program, args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(_) => bail!("{provider} session refresh could not start"),
        };
        return match wait_with_timeout(&mut child, timeout) {
            Ok(Some(status)) if status.success() => Ok(()),
            Ok(Some(_)) => bail!("{provider} session refresh did not complete"),
            Ok(None) => bail!("{provider} session refresh timed out"),
            Err(_) => bail!("{provider} session refresh could not be checked"),
        };
    }
    bail!("{provider} CLI is not installed")
}

/// Where a provider's CLI launcher can live.
pub(crate) struct CliSearch {
    /// Launcher file names in order of preference.
    pub(crate) names: &'static [&'static str],
    /// Files where the provider's own installer puts the launcher; searched
    /// right after an explicit location.
    pub(crate) known: Vec<PathBuf>,
    /// Also search the per-user bin folders of native installers, npm, pnpm
    /// and nvm after PATH.
    pub(crate) user_bins: bool,
}

/// Every existing launcher in search order: the explicit file or folder,
/// the provider's known locations, PATH, then per-user bin folders.
pub(crate) fn find_cli(explicit: Option<&Path>, search: &CliSearch) -> Vec<PathBuf> {
    let path_dirs = env::var_os("PATH")
        .map(|path| env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default();
    let user_dirs = if search.user_bins {
        user_bin_dirs()
    } else {
        Vec::new()
    };
    search_launchers(explicit, search, &path_dirs, &user_dirs)
}

/// Relative locations are never returned, so a PATH entry like `.` cannot
/// pick up a binary from the working directory.
fn search_launchers(
    explicit: Option<&Path>,
    search: &CliSearch,
    path_dirs: &[PathBuf],
    user_dirs: &[PathBuf],
) -> Vec<PathBuf> {
    let in_folders = |folders: &[PathBuf]| {
        folders
            .iter()
            .flat_map(|folder| search.names.iter().map(move |name| folder.join(name)))
            .collect::<Vec<_>>()
    };
    let mut ordered = Vec::new();
    if let Some(path) = explicit {
        if path.is_file() {
            // Older settings stored the launcher itself instead of its folder.
            ordered.push(path.to_path_buf());
        } else {
            ordered.extend(in_folders(&[path.to_path_buf(), path.join("bin")]));
        }
    }
    ordered.extend(search.known.iter().cloned());
    ordered.extend(in_folders(path_dirs));
    ordered.extend(in_folders(user_dirs));

    let mut seen = HashSet::new();
    ordered
        .into_iter()
        .filter(|path| path.is_absolute() && path.is_file())
        .filter(|path| {
            seen.insert(
                path.canonicalize()
                    .unwrap_or_else(|_| path.clone())
                    .to_string_lossy()
                    .to_lowercase(),
            )
        })
        .collect()
}

/// Per-user launcher folders that the PATH a tray app inherited at login may
/// lack: native installers use `~/.local/bin`; nvm-windows keeps the active
/// Node version behind `NVM_SYMLINK` and every installed one under `NVM_HOME`.
fn user_bin_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(base) = directories::BaseDirs::new() {
        dirs.push(base.home_dir().join(".local/bin"));
        #[cfg(not(windows))]
        dirs.push(base.home_dir().join(".npm-global/bin"));
    }
    #[cfg(windows)]
    {
        if let Some(roaming) = env::var_os("APPDATA") {
            dirs.push(PathBuf::from(roaming).join("npm"));
        }
        if let Some(local) = env::var_os("LOCALAPPDATA") {
            let local = PathBuf::from(local);
            dirs.extend([local.join("npm"), local.join("pnpm")]);
        }
        if let Some(pnpm) = env::var_os("PNPM_HOME") {
            dirs.push(PathBuf::from(pnpm));
        }
        if let Some(symlink) = env::var_os("NVM_SYMLINK") {
            dirs.push(PathBuf::from(symlink));
        }
        if let Some(program_files) = env::var_os("ProgramFiles") {
            dirs.push(PathBuf::from(program_files).join("nodejs"));
        }
        let nvm_home = env::var_os("NVM_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("APPDATA").map(|roaming| PathBuf::from(roaming).join("nvm")));
        if let Some(nvm_home) = nvm_home {
            dirs.extend(nvm_version_dirs(&nvm_home));
        }
    }
    dirs
}

#[cfg(any(windows, test))]
fn nvm_version_dirs(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut versions: Vec<_> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let name = entry.file_name();
            let mut parts = name.to_str()?.strip_prefix('v')?.split('.');
            let version: [u32; 3] = [
                parts.next()?.parse().ok()?,
                parts.next()?.parse().ok()?,
                parts.next()?.parse().ok()?,
            ];
            parts.next().is_none().then_some((version, path))
        })
        .collect();
    // The active symlink/PATH was checked first. For inactive installations,
    // numeric ordering keeps v22 above v9 and v22.10 above v22.9.
    versions.sort_by_key(|(version, _)| std::cmp::Reverse(*version));
    versions.into_iter().map(|(_, path)| path).collect()
}

/// Symlinks and Windows reparse points (junctions included).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_refresh_candidates_are_never_launched() {
        let error = refresh_session(
            "Test provider",
            &[PathBuf::from("untrusted-provider.exe")],
            &["models"],
            Duration::ZERO,
        )
        .expect_err("a relative candidate must be rejected");
        assert_eq!(error.to_string(), "Test provider CLI is not installed");
    }

    #[test]
    fn launches_opt_out_of_detached_self_updates() {
        let command = cli_command(Path::new("provider.exe"), ["models"]);
        let envs: Vec<_> = command.get_envs().collect();
        for (key, value) in AUTO_UPDATE_OPT_OUTS {
            assert!(envs.contains(&(key.as_ref(), Some(value.as_ref()))));
        }
    }

    #[cfg(windows)]
    #[test]
    fn powershell_launchers_run_through_powershell() {
        let command = cli_command(Path::new(r"C:\Tools\provider.ps1"), ["--x&calc"]);
        assert_eq!(command.get_program(), "powershell.exe");
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args[args.len() - 2], r"C:\Tools\provider.ps1");
        assert_eq!(args[args.len() - 1], "--x&calc");
    }

    #[test]
    fn known_candidates_are_existing_absolute_files() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("provider.exe");
        std::fs::write(&executable, b"fixture").unwrap();
        let search = CliSearch {
            names: &[],
            known: vec![executable, directory.path().join("missing.exe")],
            user_bins: false,
        };

        let candidates = find_cli(None, &search);

        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].is_absolute());
        assert!(candidates[0].is_file());
    }

    #[test]
    fn explicit_folder_is_searched_before_known_locations() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("agy.exe");
        std::fs::write(&executable, b"fixture").unwrap();
        let known = tempfile::tempdir().unwrap();
        let other = known.path().join("agy.exe");
        std::fs::write(&other, b"fixture").unwrap();
        let search = CliSearch {
            names: &["agy.exe"],
            known: vec![other],
            user_bins: false,
        };

        let candidates = find_cli(Some(directory.path()), &search);

        assert_eq!(candidates.first(), Some(&executable));
    }

    #[test]
    fn finds_native_cli_when_path_is_missing_and_honors_overrides() {
        let root = tempfile::tempdir().unwrap();
        let native = root.path().join(".local/bin");
        let on_path = root.path().join("path");
        let explicit = root.path().join("custom");
        let name = "claude.exe";
        for directory in [&native, &on_path, &explicit] {
            std::fs::create_dir_all(directory).unwrap();
            std::fs::write(directory.join(name), b"fixture, never executed").unwrap();
        }
        let search = CliSearch {
            names: &["claude.exe"],
            known: Vec::new(),
            user_bins: true,
        };
        let user = std::slice::from_ref(&native);
        let first = |explicit: Option<&Path>, path: &[PathBuf]| {
            search_launchers(explicit, &search, path, user)
                .into_iter()
                .next()
        };
        assert_eq!(first(None, &[]), Some(native.join(name)));
        assert_eq!(
            first(None, std::slice::from_ref(&on_path)),
            Some(on_path.join(name))
        );
        assert_eq!(
            first(Some(&explicit), std::slice::from_ref(&on_path)),
            Some(explicit.join(name))
        );
        std::fs::remove_file(native.join(name)).unwrap();
        assert_eq!(first(None, &[]), None);
    }

    #[test]
    fn explicit_missing_candidate_is_ignored() {
        let search = CliSearch {
            names: &["codex.exe"],
            known: Vec::new(),
            user_bins: false,
        };
        assert!(
            !find_cli(
                Some(Path::new("definitely-not-a-real-codex-binary")),
                &search
            )
            .iter()
            .any(|path| path.ends_with("definitely-not-a-real-codex-binary"))
        );
    }

    #[test]
    fn nvm_discovery_ignores_non_versions_and_prefers_newest_numeric_version() {
        let root = tempfile::tempdir().unwrap();
        for name in [
            "v9.9.9",
            "v22.9.0",
            "v22.10.0",
            "v22.9.2",
            "versions",
            "v",
            "v23.0.0-extra",
        ] {
            std::fs::create_dir(root.path().join(name)).unwrap();
        }
        std::fs::write(root.path().join("v99.0.0"), b"not a directory").unwrap();
        assert_eq!(
            nvm_version_dirs(root.path()),
            ["v22.10.0", "v22.9.2", "v22.9.0", "v9.9.9"].map(|name| root.path().join(name))
        );
        assert!(nvm_version_dirs(&root.path().join("missing")).is_empty());
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
