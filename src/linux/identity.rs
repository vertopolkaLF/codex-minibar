use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::{Context, Result};

pub const APP_ID: &str = "io.github.vertopolkalf.CodexMinibar";
const DESKTOP_FILE: &str = "io.github.vertopolkalf.CodexMinibar.desktop";
const DESKTOP_ENTRY: &str =
    include_str!("../../packaging/linux/io.github.vertopolkalf.CodexMinibar.desktop");

pub async fn register_portal() -> Result<()> {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    if REGISTERED.get().is_some() {
        return Ok(());
    }
    ensure_desktop_entry()?;
    ashpd::register_host_app(APP_ID.parse()?)
        .await
        .context("register desktop portal app identity")?;
    let _ = REGISTERED.set(());
    Ok(())
}

fn ensure_desktop_entry() -> Result<()> {
    let base = directories::BaseDirs::new().context("resolve the XDG data directory")?;
    let search = std::env::var_os("XDG_DATA_DIRS")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let data_dirs: Vec<PathBuf> = std::env::split_paths(&search).collect();
    ensure_entry_in(
        base.data_dir(),
        &data_dirs,
        &super::autostart::executable()?,
    )
}

fn ensure_entry_in(data_home: &Path, data_dirs: &[PathBuf], executable: &Path) -> Result<()> {
    if std::iter::once(data_home)
        .chain(data_dirs.iter().map(PathBuf::as_path))
        .any(|directory| directory.join("applications").join(DESKTOP_FILE).is_file())
    {
        return Ok(());
    }
    let directory = data_home.join("applications");
    std::fs::create_dir_all(&directory).context("create desktop entry directory")?;
    let path = directory.join(DESKTOP_FILE);
    let quoted = super::autostart::quote_exec(executable);
    let entry = DESKTOP_ENTRY.replace("Exec=codex-minibar", &format!("Exec={quoted}"));
    let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),
        Err(error) => return Err(error).context("create desktop entry for portal identity"),
    };
    if let Err(error) = file.write_all(entry.as_bytes()) {
        let _ = std::fs::remove_file(&path);
        return Err(error).context("write desktop entry for portal identity");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_id_matches_packaged_desktop_entry() {
        assert!(APP_ID.parse::<ashpd::AppID>().is_ok());
        assert_eq!(DESKTOP_FILE, format!("{APP_ID}.desktop"));
        assert!(DESKTOP_ENTRY.contains(&format!("StartupWMClass={APP_ID}\n")));
        assert!(include_str!("../../build-linux.sh").contains(DESKTOP_FILE));
    }

    #[test]
    fn direct_binary_launch_gets_a_desktop_entry_with_quoted_commands() {
        let directory = tempfile::tempdir().unwrap();
        ensure_entry_in(
            directory.path(),
            &[],
            Path::new("/opt/My $Apps/100%/minibar"),
        )
        .unwrap();
        let entry =
            std::fs::read_to_string(directory.path().join("applications").join(DESKTOP_FILE))
                .unwrap();
        assert!(entry.contains("Name=Codex Minibar\n"));
        assert!(entry.contains("Exec=\"/opt/My \\\\$Apps/100%%/minibar\"\n"));
        assert!(entry.contains("Exec=\"/opt/My \\\\$Apps/100%%/minibar\" --toggle\n"));
        ensure_entry_in(directory.path(), &[], Path::new("/different/binary")).unwrap();
        assert_eq!(
            std::fs::read_to_string(directory.path().join("applications").join(DESKTOP_FILE))
                .unwrap(),
            entry
        );
    }

    #[test]
    fn packaged_entry_is_reused_without_creating_a_user_override() {
        let directory = tempfile::tempdir().unwrap();
        let home = directory.path().join("home");
        let system = directory.path().join("system");
        std::fs::create_dir_all(system.join("applications")).unwrap();
        std::fs::write(
            system.join("applications").join(DESKTOP_FILE),
            DESKTOP_ENTRY,
        )
        .unwrap();
        ensure_entry_in(&home, &[system], Path::new("/usr/bin/codex-minibar")).unwrap();
        assert!(!home.exists());
    }
}
