use std::{
    env,
    path::{Path, PathBuf},
};

use crate::provider_support::{CliSearch, find_cli};

mod desktop_cache;
pub use desktop_cache::cleanup;
pub(crate) use desktop_cache::prepare;
pub(crate) use desktop_cache::protect_configured_paths;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexCandidate {
    pub path: PathBuf,
    /// The CLI bridge inside the Codex desktop app rather than a standalone CLI.
    pub desktop_app: bool,
}

/// Every Codex launcher on this PC in launch order: an explicit location,
/// the desktop app's bridge, PATH, then per-user bin folders.
pub fn discover(explicit: Option<&Path>) -> Vec<CodexCandidate> {
    let desktop = desktop_app_locations();
    let search = CliSearch {
        names: if cfg!(windows) {
            &["codex.exe", "codex.cmd", "codex.ps1", "codex.bat"]
        } else {
            &["codex"]
        },
        known: desktop.clone(),
        user_bins: true,
    };
    find_cli(explicit, &search)
        .into_iter()
        .map(|path| CodexCandidate {
            desktop_app: desktop.contains(&path),
            path,
        })
        .collect()
}

#[cfg(windows)]
fn desktop_app_locations() -> Vec<PathBuf> {
    let mut paths = desktop_app_locations_from_registry();

    let Some(program_files) = env::var_os("ProgramFiles") else {
        return paths;
    };
    paths.extend(desktop_paths_from_packages(
        &PathBuf::from(program_files).join("WindowsApps"),
    ));
    sort_desktop_paths(&mut paths);
    paths.dedup();
    paths
}

#[cfg(any(windows, test))]
fn desktop_paths_from_packages(packages: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(packages) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .and_then(desktop_cache::package_version)
                .is_some()
        })
        .filter_map(|entry| {
            let source = entry.path().join("app/resources/codex.exe");
            source.is_file().then_some(source)
        })
        .collect();
    sort_desktop_paths(&mut paths);
    paths.dedup();
    paths
}

#[cfg(windows)]
fn desktop_app_locations_from_registry() -> Vec<PathBuf> {
    use windows_sys::Win32::System::Registry::HKEY_CURRENT_USER;

    const KEY: &str = r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages";
    let Some(key) = crate::win32::RegKey::open(HKEY_CURRENT_USER, KEY) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = key
        .subkey_names()
        .into_iter()
        .filter(|name| {
            name.to_str()
                .and_then(desktop_cache::package_version)
                .is_some()
        })
        .filter_map(|name| key.subkey(name)?.string("PackageRootFolder"))
        .map(|root| PathBuf::from(root).join("app/resources/codex.exe"))
        .filter(|source| source.is_file())
        .collect();
    sort_desktop_paths(&mut paths);
    paths
}

#[cfg(any(windows, test))]
fn sort_desktop_paths(paths: &mut [PathBuf]) {
    paths.sort_by_cached_key(|path| {
        std::cmp::Reverse(desktop_cache::package_for_source(path).map(|(_, version)| version))
    });
}

#[cfg(not(windows))]
fn desktop_app_locations() -> Vec<PathBuf> {
    Vec::new()
}
