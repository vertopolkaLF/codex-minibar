//! Bounded, hidden CLI handoff for providers that own their refresh credentials.

use std::{
    collections::HashSet,
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Result, bail};

pub(crate) fn candidates_from(
    explicit: Option<&Path>,
    known: impl IntoIterator<Item = PathBuf>,
    names: &[&str],
) -> Vec<PathBuf> {
    let mut preferred = Vec::new();
    if let Some(path) = explicit {
        if path.is_file() {
            preferred.push(path.to_path_buf());
        } else {
            for name in names {
                preferred.push(path.join(name));
                preferred.push(path.join("bin").join(name));
            }
        }
    }
    preferred.extend(known);
    executable_candidates(&preferred, names)
}

pub(crate) fn executable_candidates(known: &[PathBuf], names: &[&str]) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    let discovered = env::var_os("PATH")
        .into_iter()
        .flat_map(|path| env::split_paths(&path).collect::<Vec<_>>())
        .flat_map(|directory| names.iter().map(move |name| directory.join(name)));

    for candidate in known.iter().cloned().chain(discovered) {
        let Ok(candidate) = fs::canonicalize(candidate) else {
            continue;
        };
        if candidate.is_absolute() && candidate.is_file() && seen.insert(candidate.clone()) {
            candidates.push(candidate);
        }
    }
    candidates
}

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

pub(crate) fn disable_auto_update(command: &mut Command) -> &mut Command {
    for (key, value) in AUTO_UPDATE_OPT_OUTS {
        command.env(key, value);
    }
    command
}

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
        let mut command = Command::new(program);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        disable_auto_update(&mut command);

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }

        #[cfg(windows)]
        let spawned = command.spawn();
        #[cfg(not(windows))]
        let spawned = crate::linux::login_child::LoginChild::spawn(command);
        let mut child = match spawned {
            Ok(child) => child,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(_) => bail!("{provider} session refresh could not start"),
        };
        let deadline = Instant::now() + timeout;
        loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => return Ok(()),
                Ok(Some(_)) => bail!("{provider} session refresh did not complete"),
                Ok(None) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(50));
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    bail!("{provider} session refresh timed out")
                }
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    bail!("{provider} session refresh could not be checked")
                }
            }
        }
    }
    bail!("{provider} CLI is not installed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn refresh_timeout_terminates_the_cli_helper_process() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let cli = root.path().join("refresh-cli");
        let pid_file = root.path().join("helper.pid");
        fs::write(&cli, "#!/bin/sh\nsleep 3 &\necho $! > \"$1\"\nwait\n").unwrap();
        fs::set_permissions(&cli, fs::Permissions::from_mode(0o755)).unwrap();
        let error = refresh_session(
            "Fixture",
            &[cli],
            &[pid_file.to_str().unwrap()],
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert!(error.to_string().contains("timed out"));
        let pid: u32 = fs::read_to_string(pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            // A killed orphan can briefly remain a zombie until init reaps it.
            let running = fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| {
                !stat
                    .rsplit_once(')')
                    .unwrap()
                    .1
                    .trim_start()
                    .starts_with('Z')
            });
            if !running {
                break;
            }
            assert!(Instant::now() < deadline, "refresh helper survived timeout");
            thread::sleep(Duration::from_millis(10));
        }
    }

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
        let mut command = Command::new("provider.exe");
        disable_auto_update(&mut command);
        let envs: Vec<_> = command.get_envs().collect();
        for (key, value) in AUTO_UPDATE_OPT_OUTS {
            assert!(envs.contains(&(key.as_ref(), Some(value.as_ref()))));
        }
    }

    #[test]
    fn known_candidates_are_resolved_to_existing_absolute_files() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("provider.exe");
        fs::write(&executable, b"fixture").unwrap();

        let candidates = executable_candidates(&[executable], &[]);

        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].is_absolute());
        assert!(candidates[0].is_file());
    }

    #[test]
    fn explicit_folder_is_searched_before_known_locations() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("agy.exe");
        fs::write(&executable, b"fixture").unwrap();

        let candidates = candidates_from(Some(directory.path()), [], &["agy.exe"]);

        assert_eq!(
            candidates.first(),
            Some(&fs::canonicalize(executable).unwrap())
        );
    }
}
