use std::{
    collections::HashSet,
    env,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexCandidate {
    pub path: PathBuf,
    pub source: CandidateSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateSource {
    Explicit,
    DesktopApp,
    Path,
    CommonLocation,
}

pub fn discover(explicit: Option<&Path>) -> Vec<CodexCandidate> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    if let Some(path) = explicit {
        if path.is_dir() {
            for name in executable_names() {
                push_candidate(
                    &mut candidates,
                    &mut seen,
                    &path.join(name),
                    CandidateSource::Explicit,
                );
            }
        } else {
            // Keep accepting the old file-shaped setting during migration.
            push_candidate(&mut candidates, &mut seen, path, CandidateSource::Explicit);
        }
    }

    for path in desktop_app_locations() {
        push_candidate(
            &mut candidates,
            &mut seen,
            &path,
            CandidateSource::DesktopApp,
        );
    }

    if let Some(path) = env::var_os("PATH") {
        for directory in env::split_paths(&path) {
            for name in executable_names() {
                push_candidate(
                    &mut candidates,
                    &mut seen,
                    &directory.join(name),
                    CandidateSource::Path,
                );
            }
        }
    }

    for path in common_locations() {
        push_candidate(
            &mut candidates,
            &mut seen,
            &path,
            CandidateSource::CommonLocation,
        );
    }
    candidates
}

#[cfg(windows)]
fn desktop_app_locations() -> Vec<PathBuf> {
    let paths = desktop_app_locations_from_registry();
    if !paths.is_empty() {
        return paths;
    }

    let Some(program_files) = env::var_os("ProgramFiles") else {
        return paths;
    };
    let packages = PathBuf::from(program_files).join("WindowsApps");
    let Ok(entries) = std::fs::read_dir(packages) else {
        return paths;
    };

    // Package versions are embedded in the directory name. Newer versions sort
    // after older ones, so prefer them when an update leaves both installed.
    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("OpenAI.Codex_")
        })
        .filter_map(|entry| {
            let package_name = entry.file_name();
            let source = entry.path().join("app/resources/codex.exe");
            cache_desktop_cli(&source, &package_name.to_string_lossy())
        })
        .collect();
    paths.sort_by(|left, right| right.cmp(left));
    paths.dedup();
    paths
}

#[cfg(windows)]
fn desktop_app_locations_from_registry() -> Vec<PathBuf> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_READ, RRF_RT_REG_SZ, RegCloseKey, RegEnumKeyExW,
            RegGetValueW, RegOpenKeyExW,
        },
    };

    const KEY: &str = "Software\\Classes\\Local Settings\\Software\\Microsoft\\Windows\\CurrentVersion\\AppModel\\Repository\\Packages";
    let wide = |value: &str| {
        value
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>()
    };
    let mut key: HKEY = std::ptr::null_mut();
    if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, wide(KEY).as_ptr(), 0, KEY_READ, &mut key) }
        != ERROR_SUCCESS
    {
        return Vec::new();
    }

    let mut paths = Vec::new();
    let mut index = 0;
    loop {
        let mut name = [0_u16; 512];
        let mut name_len = name.len() as u32;
        let status = unsafe {
            RegEnumKeyExW(
                key,
                index,
                name.as_mut_ptr(),
                &mut name_len,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if status != ERROR_SUCCESS {
            break;
        }
        index += 1;
        let package_name = OsString::from_wide(&name[..name_len as usize]);
        if !package_name.to_string_lossy().starts_with("OpenAI.Codex_") {
            continue;
        }

        let mut package_key: HKEY = std::ptr::null_mut();
        let package_name_wide = name[..name_len as usize]
            .iter()
            .copied()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        if unsafe {
            RegOpenKeyExW(
                key,
                package_name_wide.as_ptr(),
                0,
                KEY_READ,
                &mut package_key,
            )
        } != ERROR_SUCCESS
        {
            continue;
        }
        let mut root = [0_u16; 1024];
        let mut bytes = std::mem::size_of_val(&root) as u32;
        let status = unsafe {
            RegGetValueW(
                package_key,
                std::ptr::null(),
                wide("PackageRootFolder").as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                root.as_mut_ptr().cast(),
                &mut bytes,
            )
        };
        unsafe { RegCloseKey(package_key) };
        if status == ERROR_SUCCESS {
            let len = root
                .iter()
                .position(|&unit| unit == 0)
                .unwrap_or(root.len());
            let source =
                PathBuf::from(OsString::from_wide(&root[..len])).join("app/resources/codex.exe");
            if let Some(cached) = cache_desktop_cli(&source, &package_name.to_string_lossy()) {
                paths.push(cached);
            }
        }
    }
    unsafe { RegCloseKey(key) };
    paths
}

#[cfg(windows)]
fn cache_desktop_cli(source: &Path, package_name: &str) -> Option<PathBuf> {
    let local = PathBuf::from(env::var_os("LOCALAPPDATA")?);
    let directory = local
        .join("Codex Minibar")
        .join("desktop-cli")
        .join(package_name);
    let destination = directory.join("codex.exe");
    let source_len = std::fs::metadata(source).ok()?.len();
    if std::fs::metadata(&destination).is_ok_and(|metadata| metadata.len() == source_len) {
        return Some(destination);
    }

    std::fs::create_dir_all(&directory).ok()?;
    let temporary = directory.join("codex.exe.tmp");
    let _ = std::fs::remove_file(&temporary);
    std::fs::copy(source, &temporary).ok()?;
    if std::fs::rename(&temporary, &destination).is_err() {
        let _ = std::fs::remove_file(&destination);
        std::fs::rename(&temporary, &destination).ok()?;
    }
    Some(destination)
}

#[cfg(not(windows))]
fn desktop_app_locations() -> Vec<PathBuf> {
    Vec::new()
}

fn push_candidate(
    candidates: &mut Vec<CodexCandidate>,
    seen: &mut HashSet<String>,
    path: &Path,
    source: CandidateSource,
) {
    if !path.is_file() {
        return;
    }
    let normalized = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_lowercase();
    if seen.insert(normalized) {
        candidates.push(CodexCandidate {
            path: path.to_path_buf(),
            source,
        });
    }
}

#[cfg(windows)]
fn executable_names() -> &'static [&'static str] {
    &["codex.exe", "codex.cmd", "codex.ps1", "codex.bat"]
}

#[cfg(not(windows))]
fn executable_names() -> &'static [&'static str] {
    &["codex"]
}

/// Folders where Node package managers place global launchers on Windows.
/// nvm-windows keeps the active version behind `NVM_SYMLINK` and every
/// installed version under `NVM_HOME`, none of which are guaranteed to be on
/// the PATH a tray app inherited at login.
#[cfg(windows)]
pub fn node_global_bin_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
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
    dirs
}

#[cfg(windows)]
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

fn common_locations() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    #[cfg(windows)]
    {
        for dir in node_global_bin_dirs() {
            paths.extend([dir.join("codex.cmd"), dir.join("codex.ps1")]);
        }
        if let Some(local) = env::var_os("LOCALAPPDATA") {
            let local = PathBuf::from(local);
            paths.extend([
                local.join("pnpm/codex.cmd"),
                local.join("pnpm/codex.ps1"),
                local.join("npm/codex.cmd"),
                local.join("npm/codex.ps1"),
            ]);
        }
        if let Some(roaming) = env::var_os("APPDATA") {
            let roaming = PathBuf::from(roaming);
            paths.extend([roaming.join("npm/codex.cmd"), roaming.join("npm/codex.ps1")]);
        }
    }
    #[cfg(not(windows))]
    if let Some(home) = env::var_os("HOME") {
        let home = PathBuf::from(home);
        paths.extend([
            home.join(".local/bin/codex"),
            home.join(".npm-global/bin/codex"),
        ]);
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
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
    fn explicit_missing_candidate_is_ignored() {
        assert!(
            discover(Some(Path::new("definitely-not-a-real-codex-binary")))
                .iter()
                .all(|candidate| candidate.source != CandidateSource::Explicit)
        );
    }
}
