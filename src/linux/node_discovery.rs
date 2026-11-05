//! Read-only NVM discovery for desktops that do not inherit shell setup.

use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

pub(crate) fn bin_dirs() -> Vec<PathBuf> {
    let base = directories::BaseDirs::new();
    let explicit = std::env::var_os("NVM_DIR")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute());
    let roots = match explicit {
        Some(root) => vec![root],
        None => base
            .map(|base| {
                let legacy = base.home_dir().join(".nvm");
                let config = base.config_dir().join("nvm");
                if std::env::var_os("XDG_CONFIG_HOME")
                    .is_some_and(|value| Path::new(&value).is_absolute())
                {
                    vec![config, legacy]
                } else {
                    vec![legacy, config]
                }
            })
            .unwrap_or_default(),
    };
    let active = std::env::var_os("NVM_BIN").map(PathBuf::from);
    bin_dirs_in(&roots, active.as_deref())
}

fn bin_dirs_in(roots: &[PathBuf], active: Option<&Path>) -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(active) =
        active.filter(|path| path.is_absolute() && super::apps::is_executable(&path.join("node")))
    {
        directories.push(active.to_owned());
    }
    for root in roots {
        let Ok(entries) = fs::read_dir(root.join("versions/node")) else {
            continue;
        };
        let preferred = preferred_version(root);
        let mut versions = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let version = version_prefix(entry.file_name().to_str()?)?;
                let version: [u32; 3] = version.try_into().ok()?;
                let directory = entry.path().join("bin");
                super::apps::is_executable(&directory.join("node")).then_some((version, directory))
            })
            .collect::<Vec<_>>();
        versions.sort_by_key(|(version, _)| {
            (
                !preferred
                    .as_ref()
                    .is_some_and(|prefix| version.starts_with(prefix)),
                std::cmp::Reverse(*version),
            )
        });
        for (_, directory) in versions {
            if !directories.contains(&directory) {
                directories.push(directory);
            }
        }
    }
    directories
}

fn version_prefix(value: &str) -> Option<Vec<u32>> {
    let value = value.strip_prefix('v').unwrap_or(value);
    let parts = value
        .split('.')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (1..=3).contains(&parts.len()).then_some(parts)
}

/// Alias files are data, not shell scripts. Resolve a bounded chain, including
/// the literal `alias/lts/*` file maintained by NVM, without glob expansion.
fn preferred_version(root: &Path) -> Option<Vec<u32>> {
    let mut alias = String::from("default");
    let mut seen = HashSet::new();
    for _ in 0..16 {
        if let Some(version) = version_prefix(&alias) {
            return Some(version);
        }
        if matches!(alias.as_str(), "node" | "stable") {
            return Some(Vec::new());
        }
        let path = Path::new(&alias);
        if !path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
            || !seen.insert(alias.clone())
        {
            return None;
        }
        let path = root.join("alias").join(path);
        if !path.is_file() {
            return None;
        }
        let mut value = String::new();
        fs::File::open(path)
            .ok()?
            .take(1025)
            .read_to_string(&mut value)
            .ok()?;
        if value.len() > 1024 {
            return None;
        }
        alias = value.lines().next()?.trim().to_owned();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn node(root: &Path, version: &str, executable: bool) -> PathBuf {
        let directory = root.join("versions/node").join(version).join("bin");
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("node");
        fs::write(&path, "fixture, never launched").unwrap();
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(if executable { 0o755 } else { 0o600 }),
        )
        .unwrap();
        directory
    }

    fn alias(root: &Path, name: &str, value: &str) {
        let path = root.join("alias").join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, value).unwrap();
    }

    #[test]
    fn nvm_candidates_prefer_active_then_default_then_numeric_versions() {
        let root = tempfile::tempdir().unwrap();
        let old = node(root.path(), "v9.9.9", true);
        let newest = node(root.path(), "v26.1.0", true);
        let preferred = node(root.path(), "v22.10.0", true);
        let older_preferred = node(root.path(), "v22.9.0", true);
        node(root.path(), "v30.0.0", false);
        node(root.path(), "not-a-version", true);
        alias(root.path(), "default", "lts/*\n");
        alias(root.path(), "lts/*", "lts/example\n");
        alias(root.path(), "lts/example", "22\n");
        assert_eq!(
            bin_dirs_in(&[root.path().to_owned()], Some(&old)),
            vec![old, preferred, older_preferred, newest]
        );
    }

    #[test]
    fn invalid_or_cyclic_aliases_do_not_hide_installed_clis() {
        let root = tempfile::tempdir().unwrap();
        let older = node(root.path(), "v9.9.9", true);
        let newer = node(root.path(), "v22.1.0", true);
        alias(root.path(), "default", "loop");
        alias(root.path(), "loop", "default");
        assert_eq!(
            bin_dirs_in(&[root.path().to_owned()], None),
            vec![newer.clone(), older.clone()]
        );
        let outside = tempfile::NamedTempFile::new().unwrap();
        fs::write(outside.path(), "9").unwrap();
        alias(root.path(), "default", outside.path().to_str().unwrap());
        assert_eq!(
            bin_dirs_in(&[root.path().to_owned()], None),
            vec![newer, older]
        );
    }

    #[test]
    fn providers_discover_nvm_clis_without_replacing_path_or_explicit_choices() {
        const PROBE_BIN: &str = "MINIBAR_NVM_DISCOVERY_PROBE_BIN";
        if let Some(expected) = std::env::var_os(PROBE_BIN) {
            let expected = PathBuf::from(expected);
            assert_eq!(
                crate::codex::first_available(None).unwrap(),
                expected.join("codex")
            );
            assert_eq!(
                crate::claude::cli_available(None),
                Some(expected.join("claude"))
            );
            assert_eq!(
                crate::provider_cli::executable_candidates(&[], &["agy"])[0],
                expected.join("agy").canonicalize().unwrap()
            );
            let explicit = expected.join("explicit");
            assert_eq!(
                crate::codex::first_available(Some(&explicit)).unwrap(),
                explicit.join("codex")
            );
            assert_eq!(
                crate::claude::cli_available(Some(&explicit)),
                Some(explicit.join("claude"))
            );
            assert_eq!(
                crate::provider_cli::candidates_from(Some(&explicit), [], &["agy"])[0],
                explicit.join("agy").canonicalize().unwrap()
            );
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let nvm = root.path().join("nvm");
        let nvm_bin = node(&nvm, "v22.10.0", true);
        let path_bin = root.path().join("path-bin");
        for directory in [
            &nvm_bin,
            &path_bin,
            &nvm_bin.join("explicit"),
            &path_bin.join("explicit"),
        ] {
            fs::create_dir_all(directory).unwrap();
            for name in ["codex", "claude", "agy"] {
                let path = directory.join(name);
                fs::write(&path, "fixture, never launched").unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
        for (path, expected) in [
            (root.path().join("empty"), &nvm_bin),
            (path_bin.clone(), &path_bin),
        ] {
            // A separate test process gives us a desktop-like environment
            // without mutating the multithreaded test harness's environment.
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "linux::node_discovery::tests::providers_discover_nvm_clis_without_replacing_path_or_explicit_choices"])
                .env(PROBE_BIN, expected)
                .env("HOME", root.path().join("home"))
                .env("XDG_CONFIG_HOME", root.path().join("config"))
                .env("NVM_DIR", &nvm)
                .env_remove("NVM_BIN")
                .env("PATH", path)
                .output().unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
