use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::{Context, Result};

pub const APP_ID: &str = "io.github.vertopolkalf.CodexMinibar";
const DESKTOP_FILE: &str = "io.github.vertopolkalf.CodexMinibar.desktop";
const MANAGED: &str = "X-CodexMinibar-Managed=true";
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
    let path = data_home.join("applications").join(DESKTOP_FILE);
    let entry = desktop_entry(DESKTOP_ENTRY, executable).replacen(
        "[Desktop Entry]\n",
        &format!("[Desktop Entry]\n{MANAGED}\n"),
        1,
    );
    // Only entries explicitly created and owned by this application may be
    // refreshed after moving a portable binary. Preserve custom entries,
    // symlinks and package-manager launchers.
    if std::fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_file())
        && let Ok(previous) = std::fs::read_to_string(&path)
        && previous.lines().any(|line| line == MANAGED)
    {
        if previous != entry {
            let mut staged = tempfile::NamedTempFile::new_in(path.parent().unwrap())
                .context("stage managed desktop entry")?;
            staged
                .write_all(entry.as_bytes())
                .context("write managed desktop entry")?;
            staged
                .as_file()
                .sync_all()
                .context("flush managed desktop entry")?;
            staged
                .persist(&path)
                .context("replace managed desktop entry")?;
        }
        return Ok(());
    }
    if std::iter::once(data_home)
        .chain(data_dirs.iter().map(PathBuf::as_path))
        .any(|directory| directory.join("applications").join(DESKTOP_FILE).is_file())
    {
        return Ok(());
    }
    let directory = data_home.join("applications");
    std::fs::create_dir_all(&directory).context("create desktop entry directory")?;
    let path = directory.join(DESKTOP_FILE);
    let mut file = tempfile::NamedTempFile::new_in(&directory)
        .context("stage desktop entry for portal identity")?;
    file.write_all(entry.as_bytes())
        .context("write desktop entry for portal identity")?;
    file.as_file()
        .sync_all()
        .context("flush desktop entry for portal identity")?;
    // Publish complete contents without replacing an entry created since
    // the search above. Never expose a partial launcher to desktop services.
    match file.persist_noclobber(&path) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error.error).context("create desktop entry for portal identity"),
    }
}

pub(super) fn desktop_entry(template: &str, executable: &Path) -> String {
    let quoted = super::autostart::quote_exec(executable);
    template
        .lines()
        .map(|line| match line {
            "Exec=codex-minibar" => format!("Exec={quoted}"),
            "Exec=codex-minibar --toggle" => format!("Exec={quoted} --toggle"),
            _ => line.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
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
        let updated =
            std::fs::read_to_string(directory.path().join("applications").join(DESKTOP_FILE))
                .unwrap();
        assert!(updated.contains("Exec=\"/different/binary\"\n"));
        assert!(updated.contains("Exec=\"/different/binary\" --toggle\n"));
        assert!(updated.contains(MANAGED));
    }

    #[test]
    fn existing_unmanaged_user_desktop_entry_is_preserved() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("applications");
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join(DESKTOP_FILE);
        let custom = desktop_entry(DESKTOP_ENTRY, Path::new("/user/chosen/program"));
        std::fs::write(&path, &custom).unwrap();
        ensure_entry_in(root.path(), &[], Path::new("/new/portable/program")).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), custom);
    }

    #[test]
    fn a_user_symlink_is_preserved_even_when_its_target_has_a_managed_marker() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("applications");
        std::fs::create_dir(&directory).unwrap();
        let target = root.path().join("chosen.desktop");
        let contents = format!("[Desktop Entry]\n{MANAGED}\nExec=/chosen/program\n");
        std::fs::write(&target, &contents).unwrap();
        let link = directory.join(DESKTOP_FILE);
        symlink(&target, &link).unwrap();
        ensure_entry_in(root.path(), &[], Path::new("/new/program")).unwrap();
        assert!(
            std::fs::symlink_metadata(link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read_to_string(target).unwrap(), contents);
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

    #[test]
    fn tarball_installer_uses_absolute_launchers_with_desktop_entry_escaping() {
        let root = tempfile::tempdir().unwrap();
        let package = root.path().join("package");
        let applications = package.join("share/applications");
        std::fs::create_dir_all(&applications).unwrap();
        std::fs::create_dir(package.join("bin")).unwrap();
        std::fs::write(package.join("bin/codex-minibar"), "fixture, never launched").unwrap();
        std::fs::write(applications.join(DESKTOP_FILE), DESKTOP_ENTRY).unwrap();
        let installer = package.join("install.sh");
        std::fs::write(&installer, include_str!("../../packaging/linux/install.sh")).unwrap();
        for name in [
            "normal",
            "spaces and Юникод",
            "quote\" $`%\\",
            "line\nbreak",
        ] {
            let prefix = root.path().join(name);
            let output = std::process::Command::new("/bin/sh")
                .arg(&installer)
                .env("PREFIX", &prefix)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let installed =
                std::fs::read_to_string(prefix.join("share/applications").join(DESKTOP_FILE))
                    .unwrap();
            assert_eq!(
                installed,
                desktop_entry(DESKTOP_ENTRY, &prefix.join("bin/codex-minibar"))
            );
        }
    }
}
