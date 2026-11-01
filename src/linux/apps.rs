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
        .find(|path| path.is_file())
        .or_else(|| binaries.iter().find_map(|name| on_path(name)))
        .or_else(|| appimage(appimage_prefix))
}

/// The first executable named `name` on `PATH`.
pub fn on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

fn appimage(prefix: &str) -> Option<PathBuf> {
    let home = directories::BaseDirs::new()?.home_dir().to_path_buf();
    let prefix = prefix.to_ascii_lowercase();
    [
        home.join("Applications"),
        home.join(".local/bin"),
        home.join("AppImages"),
        home.join("Downloads"),
    ]
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
    // The newest download wins when several versions are kept around.
    .max_by_key(|path| {
        std::fs::metadata(path)
            .and_then(|meta| meta.modified())
            .ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_locations_win() {
        let dir = tempfile::tempdir().expect("tempdir");
        let app = dir.path().join("app");
        std::fs::write(&app, b"").expect("write");
        let found = find_app(&[], &[app.to_str().expect("utf-8")], "nothing-matches");
        assert_eq!(found, Some(app));
    }
}
