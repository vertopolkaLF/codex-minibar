//! The desktop CLI is copied out of WindowsApps only when it will be used.
//! Keep two versions, plus any copies temporarily pinned by callers/Windows.
use std::{
    collections::HashMap,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use anyhow::{Context, Result, ensure};

type Version = [u32; 4];
static CACHE: OnceLock<Mutex<HashMap<PathBuf, usize>>> = OnceLock::new();
static CONFIGURED: OnceLock<Mutex<Vec<PathBuf>>> = OnceLock::new();

pub(crate) fn protect_configured_paths<'a>(paths: impl IntoIterator<Item = &'a Path>) {
    let next = paths
        .into_iter()
        .filter_map(|path| {
            let path = if path.is_dir() {
                path.join("codex.exe")
            } else {
                path.to_owned()
            };
            path.canonicalize().ok()
        })
        .collect();
    *CONFIGURED
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = next;
}

/// Background startup maintenance, without copying or launching a CLI.
pub fn cleanup() -> Result<()> {
    if !cfg!(windows) {
        return Ok(());
    }
    let local = std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA is unavailable")?;
    cleanup_in(&PathBuf::from(local).join("Codex Minibar/desktop-cli"))
}

fn cleanup_in(root: &Path) -> Result<()> {
    no_links(root)?;
    if !root.exists() {
        return Ok(());
    }
    let pins = state().lock().unwrap_or_else(|error| error.into_inner());
    let root = root.canonicalize()?;
    let versions = cached_versions(&root)?;
    // No source/copy is needed: keep the two newest cached versions. Callers
    // and explicitly configured older copies are protected independently.
    let current = versions
        .first()
        .map(|(_, dir)| dir.join("codex.exe"))
        .unwrap_or_else(|| root.join("not-a-package/codex.exe"));
    prune(&root, &current, &pins)
}

fn state() -> &'static Mutex<HashMap<PathBuf, usize>> {
    CACHE.get_or_init(Mutex::default)
}

pub(crate) struct PreparedCli {
    pub path: PathBuf,
    // Denies deletion/replacement on Windows, including across Minibar processes.
    _file: Option<File>,
    pinned: bool,
}

impl PreparedCli {
    pub(crate) fn is_cached(&self) -> bool {
        self.pinned
    }
}

impl Drop for PreparedCli {
    fn drop(&mut self) {
        if self.pinned {
            let mut pins = state().lock().unwrap_or_else(|error| error.into_inner());
            if let Some(count) = pins.get_mut(&self.path) {
                *count -= 1;
                if *count == 0 {
                    pins.remove(&self.path);
                }
            }
        }
    }
}

pub(crate) fn package_version(name: &str) -> Option<Version> {
    let mut parts = name.strip_prefix("OpenAI.Codex_")?.split('_');
    let version: Vec<u32> = parts
        .next()?
        .split('.')
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()
        .ok()?;
    let architecture = parts.next()?;
    if !matches!(architecture, "x64" | "arm64" | "x86")
        || !parts.next()?.is_empty()
        || parts.next()? != "2p2nqsd0c76g0"
        || parts.next().is_some()
    {
        return None;
    }
    version.try_into().ok()
}

pub(super) fn package_for_source(source: &Path) -> Option<(String, Version)> {
    if source.file_name()? != "codex.exe"
        || source.parent()?.file_name()? != "resources"
        || source.parent()?.parent()?.file_name()? != "app"
    {
        return None;
    }
    let name = source.ancestors().nth(3)?.file_name()?.to_str()?.to_owned();
    let version = package_version(&name)?;
    Some((name, version))
}

pub(crate) fn prepare(source: &Path) -> Result<PreparedCli> {
    if cfg!(windows) {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let root = PathBuf::from(local).join("Codex Minibar/desktop-cli");
            if package_for_source(source).is_some() {
                return prepare_in(source, &root);
            }
            // A user may explicitly configure a legacy cached executable.
            // Pin that path too, so another thread cannot prune it before spawn.
            if source.file_name().is_some_and(|name| name == "codex.exe")
                && source
                    .parent()
                    .and_then(Path::file_name)
                    .and_then(|name| name.to_str())
                    .and_then(package_version)
                    .is_some()
                && source
                    .parent()
                    .and_then(Path::parent)
                    .is_some_and(|parent| {
                        parent
                            .canonicalize()
                            .ok()
                            .zip(root.canonicalize().ok())
                            .is_some_and(|(parent, root)| parent == root)
                    })
            {
                no_links(source)?;
                let mut pins = state().lock().unwrap_or_else(|error| error.into_inner());
                return pin(source.canonicalize()?, &mut pins);
            }
        } else if package_for_source(source).is_some() {
            anyhow::bail!("LOCALAPPDATA is unavailable");
        }
    }
    Ok(PreparedCli {
        path: source.to_owned(),
        _file: None,
        pinned: false,
    })
}

fn no_links(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => ensure!(
                !crate::claude::profile_oauth::is_link(&metadata),
                "CLI cache path must not contain links: {}",
                ancestor.display()
            ),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn usable(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        metadata.is_file()
            && metadata.len() > 0
            && !crate::claude::profile_oauth::is_link(&metadata)
    })
}

fn cached_versions(root: &Path) -> Result<Vec<(Version, PathBuf)>> {
    Ok(cache_directories(root)?
        .into_iter()
        .filter(|(_, directory)| usable(&directory.join("codex.exe")))
        .collect())
}

fn cache_directories(root: &Path) -> Result<Vec<(Version, PathBuf)>> {
    let mut versions = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let Some(version) = package_version(&entry.file_name().to_string_lossy()) else {
            continue;
        };
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.is_dir() && !crate::claude::profile_oauth::is_link(&metadata) {
            versions.push((version, entry.path()));
        }
    }
    versions.sort_by_key(|(version, _)| std::cmp::Reverse(*version));
    Ok(versions)
}

fn publish(source: &Path, directory: &Path) -> Result<PathBuf> {
    no_links(directory)?;
    fs::create_dir_all(directory)?;
    no_links(directory)?;
    let destination = directory.join("codex.exe");
    let metadata = fs::metadata(source)?;
    ensure!(
        metadata.is_file() && metadata.len() > 0,
        "Desktop CLI is empty or not a file"
    );
    // Detect same-sized updates too; legacy copies without a stamp get replaced once.
    let stamp = format!("{} {:?}", metadata.len(), metadata.modified()?);
    let stamp_path = directory.join("source.txt");
    no_links(&stamp_path)?;
    no_links(&destination)?;
    if usable(&destination)
        && fs::metadata(&destination)?.len() == metadata.len()
        && fs::read_to_string(&stamp_path).ok().as_deref() == Some(&stamp)
    {
        return Ok(destination);
    }
    let mut temporary = tempfile::Builder::new()
        .prefix(".codex-cache-")
        .tempfile_in(directory)?;
    let mut input = File::open(source)?;
    let copied = io::copy(&mut input, &mut temporary)?;
    ensure!(
        copied == metadata.len() && fs::metadata(source)?.modified()? == metadata.modified()?,
        "Desktop CLI changed during copying"
    );
    temporary.as_file().sync_all()?;
    // Never delete a working destination before publishing its replacement.
    temporary
        .persist(&destination)
        .map_err(|error| error.error)?;
    let mut stamp_file = tempfile::Builder::new()
        .prefix(".codex-cache-")
        .tempfile_in(directory)?;
    use std::io::Write;
    stamp_file.write_all(stamp.as_bytes())?;
    stamp_file
        .persist(stamp_path)
        .map_err(|error| error.error)?;
    Ok(destination)
}

fn prepare_in(source: &Path, root: &Path) -> Result<PreparedCli> {
    let (package, _) = package_for_source(source).context("Not a desktop CLI package")?;
    let mut pins = state().lock().unwrap_or_else(|error| error.into_inner());
    no_links(root)?;
    fs::create_dir_all(root)?;
    no_links(root)?;
    let root = root.canonicalize()?;
    let path = match publish(source, &root.join(&package)) {
        Ok(path) => path,
        Err(error) => {
            crate::logger::info(format!(
                "Codex CLI cache preparation failed: {error:#}; retaining existing cache"
            ));
            let desired = root.join(&package).join("codex.exe");
            let fallback = if usable(&desired) && no_links(&desired).is_ok() {
                Some(desired)
            } else {
                cached_versions(&root)?
                    .iter()
                    .find(|(_, dir)| {
                        dir.file_name()
                            .and_then(|name| name.to_str())
                            .and_then(|name| name.split('_').nth(2))
                            == package.split('_').nth(2)
                    })
                    .map(|(_, dir)| dir.join("codex.exe"))
            };
            let Some(path) = fallback else {
                return Err(error);
            };
            // Failure never triggers pruning of the last working copy.
            return pin(path, &mut pins);
        }
    };
    let prepared = pin(path, &mut pins)?;
    if let Err(error) = prune(&root, &prepared.path, &pins) {
        crate::logger::info(format!("Codex CLI cache cleanup deferred: {error:#}"));
    }
    Ok(prepared)
}

fn pin(path: PathBuf, pins: &mut HashMap<PathBuf, usize>) -> Result<PreparedCli> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1); // FILE_SHARE_READ; no write/delete while preparing/spawning.
    }
    let file = options.open(&path)?;
    *pins.entry(path.clone()).or_default() += 1;
    Ok(PreparedCli {
        path,
        _file: Some(file),
        pinned: true,
    })
}

fn prune(root: &Path, current: &Path, pins: &HashMap<PathBuf, usize>) -> Result<()> {
    no_links(root)?;
    let current_dir = current.parent().context("CLI has no parent")?;
    let versions = cached_versions(root)?;
    let previous = versions
        .iter()
        .find(|(_, dir)| dir != current_dir)
        .map(|(_, dir)| dir.as_path());
    let mut freed = 0;
    let mut removed = 0;
    for (_, directory) in cache_directories(root)? {
        if directory == current_dir
            || Some(directory.as_path()) == previous
            || pins.contains_key(&directory.join("codex.exe"))
            || CONFIGURED
                .get_or_init(Mutex::default)
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .contains(&directory.join("codex.exe"))
        {
            continue;
        }
        // Never recursively delete: reject unknown children and links, including junctions.
        no_links(&directory)?;
        ensure!(
            directory.canonicalize()?.parent() == Some(root),
            "Cache directory escaped its root"
        );
        let entries: Vec<_> = fs::read_dir(&directory)?.collect::<io::Result<_>>()?;
        if entries.iter().any(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            !matches!(name.as_ref(), "codex.exe" | "codex.exe.tmp" | "source.txt")
                && !name.starts_with(".codex-cache-")
                || !entry.file_type().is_ok_and(|kind| kind.is_file())
                || fs::symlink_metadata(entry.path())
                    .is_ok_and(|metadata| crate::claude::profile_oauth::is_link(&metadata))
        }) {
            crate::logger::info(format!(
                "Codex CLI cache cleanup skipped {}: contains unknown files or links",
                directory.display()
            ));
            continue;
        }
        let executable = directory.join("codex.exe");
        let bytes = fs::metadata(&executable)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        // Windows refuses this while another caller/process has the executable open.
        if let Err(error) = fs::remove_file(&executable)
            && error.kind() != io::ErrorKind::NotFound
        {
            crate::logger::info(format!(
                "Codex CLI cache cleanup deferred for {}: {error}",
                directory.display()
            ));
            continue;
        }
        freed += bytes;
        removed += 1;
        for entry in entries {
            if entry.path() != executable {
                let _ = fs::remove_file(entry.path());
            }
        }
        let _ = fs::remove_dir(&directory);
    }
    if removed > 0 {
        crate::logger::info(format!(
            "Codex CLI cache: removed {removed} obsolete versions, freed {freed} bytes"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(version: &str) -> String {
        format!("OpenAI.Codex_{version}_x64__2p2nqsd0c76g0")
    }
    fn source(root: &Path, version: &str, bytes: &[u8]) -> PathBuf {
        let path = root
            .join("packages")
            .join(name(version))
            .join("app/resources/codex.exe");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }
    fn cached(root: &Path, version: &str) -> PathBuf {
        let directory = root.join(name(version));
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("codex.exe"), b"old-cli").unwrap();
        directory
    }

    #[test]
    fn numeric_versions_and_package_validation() {
        let root = tempfile::tempdir().unwrap();
        let mut paths = vec![
            source(root.path(), "26.9.0.0", b"cli"),
            source(root.path(), "26.10.0.0", b"cli"),
        ];
        super::super::sort_desktop_paths(&mut paths);
        assert!(paths[0].to_string_lossy().contains("26.10.0.0"));
        for invalid in [
            "OpenAI.Codex_26.1_x64__2p2nqsd0c76g0",
            "OpenAI.Codex_26.1.0.0_x64__other",
            "OpenAI.Codex_26.1.0.0_x64__2p2nqsd0c76g0/../bad",
        ] {
            assert!(package_version(invalid).is_none());
        }
    }

    #[test]
    fn discovery_only_returns_sources_without_creating_a_cache() {
        let root = tempfile::tempdir().unwrap();
        let old = source(root.path(), "26.9.0.0", b"old");
        let new = source(root.path(), "26.10.0.0", b"new");
        assert_eq!(
            super::super::desktop_paths_from_packages(&root.path().join("packages")),
            vec![new, old]
        );
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn startup_cleanup_needs_no_installed_cli_and_preserves_configured_copy() {
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join("cache");
        let explicit = cached(&cache, "26.1.0.0");
        // Add this test's unique path without replacing other parallel tests' configuration.
        CONFIGURED
            .get_or_init(Mutex::default)
            .lock()
            .unwrap()
            .push(explicit.join("codex.exe").canonicalize().unwrap());
        for i in 2..=35 {
            cached(&cache, &format!("26.{i}.0.0"));
        }
        cleanup_in(&cache).unwrap();
        assert_eq!(cached_versions(&cache).unwrap().len(), 3);
        assert!(explicit.join("codex.exe").exists());
        assert!(cache.join(name("26.34.0.0")).exists());
        assert!(cache.join(name("26.35.0.0")).exists());
        CONFIGURED
            .get()
            .unwrap()
            .lock()
            .unwrap()
            .retain(|path| path != &explicit.join("codex.exe").canonicalize().unwrap());
        cleanup_in(&cache).unwrap();
        assert_eq!(cached_versions(&cache).unwrap().len(), 2);
        assert!(!explicit.exists());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1); // no packages/copies created
        cleanup_in(&root.path().join("missing-cache")).unwrap();
        assert!(!root.path().join("missing-cache").exists());
    }

    #[test]
    fn same_sized_source_update_replaces_cached_copy() {
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join("cache");
        let path = source(root.path(), "26.1.0.0", b"old");
        drop(prepare_in(&path, &cache).unwrap());
        let mut file = fs::OpenOptions::new().write(true).open(&path).unwrap();
        use std::io::Write;
        file.write_all(b"new").unwrap();
        file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5))
            .unwrap();
        drop(file);
        let prepared = prepare_in(&path, &cache).unwrap();
        assert_eq!(fs::read(&prepared.path).unwrap(), b"new");
    }

    #[cfg(windows)]
    #[test]
    fn running_process_protects_copy_after_spawn_lease_is_released() {
        use std::process::{Command, Stdio};
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join("cache");
        let path = source(root.path(), "26.1.0.0", b"placeholder");
        fs::copy(
            PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/cmd.exe"),
            &path,
        )
        .unwrap();
        let prepared = prepare_in(&path, &cache).unwrap();
        let cached = prepared.path.clone();
        // A harmless shell waiting on its piped stdin, never the Minibar app.
        let mut child = Command::new(&cached)
            .args(["/D", "/C", "set /p cache_test="])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        drop(prepared);
        assert!(child.try_wait().unwrap().is_none());
        for i in 2..=3 {
            drop(prepare_in(&source(root.path(), &format!("26.{i}.0.0"), b"cli"), &cache).unwrap());
        }
        assert!(cached.exists());
        child.kill().unwrap();
        child.wait().unwrap();
        let newest = root
            .path()
            .join("packages")
            .join(name("26.3.0.0"))
            .join("app/resources/codex.exe");
        drop(prepare_in(&newest, &cache).unwrap());
        assert!(!cached.exists());
    }

    #[test]
    fn prepares_one_version_and_prunes_legacy_cache_to_two() {
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join("cache");
        for i in 1..=35 {
            cached(&cache, &format!("26.{i}.0.0"));
        }
        let source = source(root.path(), "26.36.0.0", b"new-cli");
        let prepared = prepare_in(&source, &cache).unwrap();
        assert_eq!(fs::read(&prepared.path).unwrap(), b"new-cli");
        assert_eq!(cached_versions(&cache).unwrap().len(), 2);
        assert!(cache.join(name("26.35.0.0")).exists());
        let modified = fs::metadata(&prepared.path).unwrap().modified().unwrap();
        let repeated = prepare_in(&source, &cache).unwrap();
        assert_eq!(
            fs::metadata(&repeated.path).unwrap().modified().unwrap(),
            modified
        );
    }

    #[test]
    fn failed_copy_preserves_previous_versions_and_falls_back() {
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join("cache");
        for i in 1..=3 {
            cached(&cache, &format!("26.{i}.0.0"));
        }
        let missing = root
            .path()
            .join("packages")
            .join(name("26.4.0.0"))
            .join("app/resources/codex.exe");
        let prepared = prepare_in(&missing, &cache).unwrap();
        assert_eq!(
            prepared.path.parent().unwrap().file_name().unwrap(),
            name("26.3.0.0").as_str()
        );
        assert_eq!(cached_versions(&cache).unwrap().len(), 3);
    }

    #[test]
    fn active_lease_defers_pruning_until_released() {
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join("cache");
        let old = source(root.path(), "26.1.0.0", b"old");
        let lease = prepare_in(&old, &cache).unwrap();
        for i in 2..=3 {
            drop(prepare_in(&source(root.path(), &format!("26.{i}.0.0"), b"new"), &cache).unwrap());
        }
        assert!(lease.path.exists());
        let old_path = lease.path.clone();
        drop(lease);
        drop(prepare_in(&source(root.path(), "26.3.0.0", b"new"), &cache).unwrap());
        assert!(!old_path.exists());
    }

    #[test]
    fn unknown_files_and_incomplete_copies_are_handled_safely() {
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join("cache");
        let foreign = cached(&cache, "26.1.0.0");
        fs::write(foreign.join("keep.txt"), b"user data").unwrap();
        let incomplete = cache.join(name("26.0.0.0"));
        fs::create_dir_all(&incomplete).unwrap();
        fs::write(incomplete.join("codex.exe.tmp"), b"partial").unwrap();
        cached(&cache, "26.2.0.0");
        drop(prepare_in(&source(root.path(), "26.3.0.0", b"cli"), &cache).unwrap());
        assert_eq!(fs::read(foreign.join("keep.txt")).unwrap(), b"user data");
        assert!(foreign.join("codex.exe").exists());
        assert!(!incomplete.exists());
    }

    #[test]
    fn parallel_preparation_publishes_one_complete_copy() {
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join("cache");
        let source = source(root.path(), "26.1.0.0", &[42; 8192]);
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| scope.spawn(|| prepare_in(&source, &cache).unwrap()))
                .collect();
            for handle in handles {
                let prepared = handle.join().unwrap();
                assert_eq!(fs::read(&prepared.path).unwrap(), vec![42; 8192]);
            }
        });
        assert_eq!(
            fs::read_dir(cache.join(name("26.1.0.0"))).unwrap().count(),
            2
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_open_file_defers_cleanup_and_failed_replacement_preserves_bytes() {
        use std::os::windows::fs::OpenOptionsExt;
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join("cache");
        let directory = cached(&cache, "26.1.0.0");
        let file = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(directory.join("codex.exe"))
            .unwrap();
        let replacement = source(root.path(), "26.1.0.0", b"replacement");
        let lease = prepare_in(&replacement, &cache).unwrap();
        assert_eq!(fs::read(&lease.path).unwrap(), b"old-cli");
        drop(lease);
        cached(&cache, "26.2.0.0");
        let current = source(root.path(), "26.3.0.0", b"new");
        drop(prepare_in(&current, &cache).unwrap());
        assert!(directory.join("codex.exe").exists());
        drop(file);
        drop(prepare_in(&current, &cache).unwrap());
        assert!(!directory.exists());
    }

    #[cfg(windows)]
    #[test]
    fn junctions_are_never_followed() {
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join("cache");
        let outside = root.path().join("outside");
        fs::create_dir_all(&cache).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("codex.exe"), b"untouched").unwrap();
        let junction = cache.join(name("26.1.0.0"));
        let status = std::process::Command::new("cmd.exe")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&outside)
            .status()
            .unwrap();
        assert!(status.success());
        cached(&cache, "26.2.0.0");
        drop(prepare_in(&source(root.path(), "26.3.0.0", b"cli"), &cache).unwrap());
        assert_eq!(fs::read(outside.join("codex.exe")).unwrap(), b"untouched");
        assert!(prepare_in(&source(root.path(), "26.1.0.0", b"cli"), &cache).is_ok()); // safely falls back, without writing through the link
        assert_eq!(fs::read(outside.join("codex.exe")).unwrap(), b"untouched");
        assert!(cleanup_in(&junction).is_err());
        fs::remove_dir(&junction).unwrap();
    }
}
