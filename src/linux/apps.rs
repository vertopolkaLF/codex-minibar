//! Locating desktop applications installed the Linux ways: distribution
//! packages (`.deb`/`.rpm` under `/usr/share/<app>` with a launcher on
//! `PATH`), tarballs under `/opt`, and AppImages kept in the usual folders.

use std::path::{Path, PathBuf};

/// Finds an installed app. `binaries` are launcher names looked up on `PATH`,
/// `locations` absolute install paths, and `appimage_prefix` the start of
/// the AppImage file name (matched case-insensitively).
pub fn find_app(binaries: &[&str], locations: &[&str], appimage_prefix: &str) -> Option<PathBuf> {
    locations
        .iter()
        .map(PathBuf::from)
        .find(|path| is_executable(path))
        .or_else(|| binaries.iter().find_map(|name| on_path(name)))
        .or_else(|| appimage(appimage_prefix))
}

/// The first executable named `name` on `PATH`.
pub fn on_path(name: &str) -> Option<PathBuf> {
    on_path_os(std::ffi::OsStr::new(name))
}

/// Follow exec's path rules without converting Unix filenames to UTF-8.
pub(crate) fn on_path_os(name: &std::ffi::OsStr) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    if name.as_bytes().contains(&b'/') {
        let path = PathBuf::from(name);
        return is_executable(&path).then_some(path);
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| is_executable(candidate))
}

pub(crate) fn is_executable(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    if !std::fs::metadata(path).is_ok_and(|meta| meta.is_file()) {
        return false;
    }
    let Ok(path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // Check the credentials used by exec, including owner/group selection,
    // ACLs and noexec mounts. Any set mode bit alone is insufficient.
    // SAFETY: path is a valid NUL-terminated string, and no pointer is retained.
    unsafe { libc::faccessat(libc::AT_FDCWD, path.as_ptr(), libc::X_OK, libc::AT_EACCESS) == 0 }
}

fn appimage(prefix: &str) -> Option<PathBuf> {
    let home = directories::BaseDirs::new()?.home_dir().to_path_buf();
    let downloads = directories::UserDirs::new()
        .and_then(|dirs| dirs.download_dir().map(Path::to_path_buf))
        .unwrap_or_else(|| home.join("Downloads"));
    appimage_in(
        [
            home.join("Applications"),
            home.join(".local/bin"),
            home.join("AppImages"),
            downloads,
        ],
        prefix,
    )
}

fn appimage_in(directories: impl IntoIterator<Item = PathBuf>, prefix: &str) -> Option<PathBuf> {
    let prefix = prefix.to_ascii_lowercase();
    directories
        .into_iter()
        .filter_map(|directory| std::fs::read_dir(directory).ok())
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name().is_some_and(|name| {
                let name = name.to_string_lossy().to_ascii_lowercase();
                name.starts_with(&prefix) && name.ends_with(".appimage")
            })
        })
        .filter(|path| is_executable(path))
        // The newest runnable download wins when several versions are kept around.
        .max_by_key(|path| {
            std::fs::metadata(path)
                .and_then(|meta| meta.modified())
                .ok()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn executable(path: &Path) {
        std::fs::write(path, b"test executable").unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn executable_detection_checks_the_current_users_permission_class() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("group-only-executable");
        std::fs::write(&file, b"fixture, never launched").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o010)).unwrap();
        // For the owner, group execute does not substitute for owner execute.
        // Root has special execution semantics when any execute bit is set.
        // SAFETY: geteuid has no preconditions.
        assert_eq!(is_executable(&file), unsafe { libc::geteuid() } == 0);
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o100)).unwrap();
        assert!(is_executable(&file));
        assert!(!is_executable(root.path()));
    }

    #[test]
    fn non_executable_locations_do_not_hide_working_installations() {
        let dir = tempfile::tempdir().unwrap();
        let incomplete = dir.path().join("incomplete");
        let app = dir.path().join("app");
        std::fs::write(&incomplete, b"").unwrap();
        executable(&app);
        assert_eq!(
            find_app(
                &[],
                &[incomplete.to_str().unwrap(), app.to_str().unwrap()],
                "no-match"
            ),
            Some(app)
        );
    }

    #[test]
    fn appimages_must_be_executable_files_in_the_configured_directory() {
        let root = tempfile::tempdir().unwrap();
        let downloads = root.path().join("Téléchargements");
        std::fs::create_dir(&downloads).unwrap();
        let app = downloads.join("Cursor-old.AppImage");
        executable(&app);
        std::fs::create_dir(downloads.join("Cursor-directory.AppImage")).unwrap();
        let incomplete = downloads.join("Cursor-new.AppImage");
        std::fs::write(&incomplete, b"not executable").unwrap();
        assert_eq!(
            appimage_in([downloads.clone()], "cursor"),
            Some(app.clone())
        );
        std::fs::remove_file(app).unwrap();
        assert_eq!(appimage_in([downloads], "cursor"), None);
    }

    #[test]
    fn absolute_locations_win() {
        let dir = tempfile::tempdir().expect("tempdir");
        let app = dir.path().join("app");
        executable(&app);
        let found = find_app(&[], &[app.to_str().expect("utf-8")], "nothing-matches");
        assert_eq!(found, Some(app));
    }
}
