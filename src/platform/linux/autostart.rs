//! "Start at login" through an XDG autostart entry.
//!
//! `$XDG_CONFIG_HOME/autostart/codex-minibar.desktop` is honored by GNOME,
//! KDE, Xfce, Cinnamon and other desktops implementing the XDG Autostart
//! specification, and like the Windows `Run` value it belongs to the user.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const FILE_NAME: &str = "codex-minibar.desktop";
pub(super) const EXTRACT_AND_RUN_FLAG: &str = "--appimage-extract-and-run";

fn entry_path() -> Result<PathBuf> {
    let base = directories::BaseDirs::new().context("resolve the XDG config directory")?;
    Ok(base.config_dir().join("autostart").join(FILE_NAME))
}

/// The persistent program to start: a private extraction wrapper, the AppImage
/// file for FUSE launches, or this executable for an ordinary native install.
pub(super) fn executable() -> Result<PathBuf> {
    let program = raw_executable()?;
    if requires_extraction() {
        super::appimage_launch::for_image(&program)
    } else {
        Ok(program)
    }
}

pub(super) fn raw_executable() -> Result<PathBuf> {
    let executable = std::env::current_exe().context("resolve current executable for startup")?;
    Ok(launch_executable(
        &executable,
        std::env::var_os("APPDIR").as_deref().map(Path::new),
        std::env::var_os("APPIMAGE").as_deref().map(Path::new),
    ))
}

fn launch_executable(executable: &Path, appdir: Option<&Path>, appimage: Option<&Path>) -> PathBuf {
    // AppImage runtimes export these variables to every child process.
    // Only use them when this executable really belongs to that mount.
    if let Some(appdir) = appdir.and_then(|path| path.canonicalize().ok())
        && executable.starts_with(&appdir)
        && let Some(appimage) = appimage.filter(|path| path.is_absolute() && path.is_file())
    {
        return appimage.to_path_buf();
    }
    executable.to_path_buf()
}

/// The pinned AppImage runtime removes its extraction flag from argv. Its
/// extracted APPDIR remains identifiable, and is not a reusable mount path.
pub(super) fn requires_extraction() -> bool {
    static REQUIRED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *REQUIRED.get_or_init(|| {
        let Ok(executable) = std::env::current_exe() else {
            return false;
        };
        let appdir = std::env::var_os("APPDIR").map(PathBuf::from);
        let appimage = std::env::var_os("APPIMAGE").map(PathBuf::from);
        extraction_required_for(&executable, appdir.as_deref(), appimage.as_deref())
    })
}

fn extraction_required_for(
    executable: &Path,
    appdir: Option<&Path>,
    appimage: Option<&Path>,
) -> bool {
    use std::os::unix::ffi::OsStrExt;
    if launch_executable(executable, appdir, appimage) == executable {
        return false;
    }
    // AppImage/type2-runtime 20251108 extracts to this content-addressed
    // directory, including when TMPDIR itself happens to be a FUSE filesystem.
    appdir
        .and_then(|path| path.canonicalize().ok())
        .and_then(|path| {
            path.file_name()
                .map(|name| name.as_bytes().starts_with(b"appimage_extracted_"))
        })
        .unwrap_or(false)
}

pub(super) fn desktop_command(executable: &Path) -> String {
    desktop_command_with_extraction(executable, requires_extraction())
}

fn desktop_command_with_extraction(executable: &Path, extract: bool) -> String {
    let quoted = quote_exec(executable);
    if extract {
        format!("{quoted} {EXTRACT_AND_RUN_FLAG}")
    } else {
        quoted
    }
}

/// Quotes an argument for a desktop entry `Exec` key.
pub(super) fn quote_exec(path: &Path) -> String {
    let mut quoted = String::from('"');
    for character in path.display().to_string().chars() {
        if matches!(character, '"' | '`' | '$' | '\\') {
            quoted.push('\\');
        }
        quoted.push(character);
    }
    quoted.push('"');
    // `%` is a field code prefix and must be doubled.
    quoted
        .replace('\\', "\\\\")
        .replace('%', "%%")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn entry(executable: &Path) -> String {
    entry_for_command(&desktop_command(executable))
}

fn entry_for_command(command: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Codex Minibar\n\
         Comment=AI tools usage in the system tray\n\
         Exec={}\n\
         Icon=codex-minibar\n\
         Terminal=false\n\
         NoDisplay=true\n\
         X-GNOME-Autostart-enabled=true\n",
        command
    )
}

/// Whether an enabled autostart entry exists.
pub fn is_registered() -> Result<bool> {
    let path = entry_path()?;
    match std::fs::read_to_string(&path) {
        Ok(contents) => Ok(entry_enabled(&contents)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("read {}", path.display())),
    }
}

fn entry_enabled(contents: &str) -> bool {
    let mut in_entry = false;
    let mut application = false;
    let mut executable = false;
    let mut disabled = false;
    for line in contents.lines().map(str::trim) {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match (key.trim(), value.trim()) {
            ("Type", "Application") => application = true,
            ("Exec", value) if !value.is_empty() => executable = true,
            ("Hidden", "true") | ("X-GNOME-Autostart-enabled", "false") => disabled = true,
            _ => {}
        }
    }
    application && executable && !disabled
}

/// Creates or removes the autostart entry.
pub fn apply(enabled: bool) -> Result<()> {
    let path = entry_path()?;
    if !enabled {
        return match std::fs::remove_file(&path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                Err(error).with_context(|| format!("remove {}", path.display()))
            }
            _ => Ok(()),
        };
    }
    write_entry(&path, &executable()?)
}

fn write_entry(path: &Path, executable: &Path) -> Result<()> {
    use std::io::Write;
    let parent = path.parent().context("autostart entry has no parent")?;
    std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let mut staged = tempfile::NamedTempFile::new_in(parent).context("stage autostart entry")?;
    staged
        .write_all(entry(executable).as_bytes())
        .context("write autostart entry")?;
    staged
        .as_file()
        .sync_all()
        .context("flush autostart entry")?;
    staged
        .persist(path)
        .with_context(|| format!("replace {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autostart_replacement_preserves_open_readers_and_does_not_follow_symlinks() {
        use std::{io::Read, os::unix::fs::symlink};
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("autostart/app.desktop");
        write_entry(&path, Path::new("/old/program")).unwrap();
        let mut reader = std::fs::File::open(&path).unwrap();
        write_entry(&path, Path::new("/new/program")).unwrap();
        let mut old = String::new();
        reader.read_to_string(&mut old).unwrap();
        assert_eq!(old, entry(Path::new("/old/program")));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            entry(Path::new("/new/program"))
        );
        std::fs::remove_file(&path).unwrap();
        let outside = root.path().join("unrelated.desktop");
        std::fs::write(&outside, "unchanged").unwrap();
        symlink(&outside, &path).unwrap();
        write_entry(&path, Path::new("/new/program")).unwrap();
        assert_eq!(std::fs::read_to_string(outside).unwrap(), "unchanged");
        assert!(entry_enabled(&std::fs::read_to_string(&path).unwrap()));
    }

    #[test]
    fn autostart_status_honors_desktop_entry_groups_and_whitespace() {
        let enabled = entry(Path::new("/usr/bin/codex-minibar"));
        assert!(entry_enabled(&enabled));
        assert!(!entry_enabled(&format!("{enabled}Hidden = true\n")));
        assert!(!entry_enabled(&enabled.replace(
            "X-GNOME-Autostart-enabled=true",
            "X-GNOME-Autostart-enabled = false",
        )));
        assert!(entry_enabled(&format!(
            "{enabled}[Desktop Action Other]\nHidden=true\n"
        )));
        assert!(entry_enabled(
            "# generated by desktop settings\n[Desktop Entry]\nType = Application\nExec = /usr/bin/codex-minibar\n"
        ));
    }

    #[test]
    fn empty_or_incomplete_autostart_files_are_not_registered() {
        for invalid in [
            "",
            "# comment",
            "[Desktop Entry]",
            "[Desktop Entry]\nType=Application\n",
            "[Other]\nType=Application\nExec=app\n",
        ] {
            assert!(!entry_enabled(invalid), "{invalid}");
        }
    }

    #[test]
    fn inherited_appimage_environment_cannot_register_another_app() {
        let dir = tempfile::tempdir().unwrap();
        let mount = dir.path().join("mount");
        std::fs::create_dir(&mount).unwrap();
        let image = dir.path().join("Other.AppImage");
        std::fs::write(&image, b"").unwrap();
        let binary = Path::new("/usr/bin/codex-minibar");
        assert_eq!(
            launch_executable(binary, Some(&mount), Some(&image)),
            binary
        );
        let bundled = mount.join("usr/bin/codex-minibar");
        assert_eq!(
            launch_executable(&bundled, Some(&mount), Some(&image)),
            image
        );
        assert_eq!(launch_executable(binary, None, Some(&image)), binary);
    }

    #[test]
    fn exec_paths_are_quoted() {
        assert_eq!(
            quote_exec(Path::new("/opt/My $Apps/100%/minibar")),
            r#""/opt/My \\$Apps/100%%/minibar""#
        );
    }

    #[test]
    fn exec_paths_escape_both_desktop_entry_and_command_layers() {
        assert_eq!(
            quote_exec(Path::new("/opt/a\\b\"c`d/minibar")),
            r#""/opt/a\\\\b\\"c\\`d/minibar""#
        );
        assert_eq!(
            quote_exec(Path::new("/opt/a\nb/minibar")),
            r#""/opt/a\nb/minibar""#
        );
    }

    #[test]
    fn entry_starts_the_given_program() {
        let text = entry(Path::new("/usr/bin/codex-minibar"));
        assert!(text.starts_with("[Desktop Entry]\n"));
        assert!(text.contains("Exec=\"/usr/bin/codex-minibar\"\n"));
    }

    #[test]
    fn extraction_mode_requires_this_programs_actual_appimage_context() {
        let root = tempfile::tempdir().unwrap();
        let image = root.path().join("Minibar.AppImage");
        std::fs::write(&image, "fixture, never launched").unwrap();
        for (name, expected) in [
            ("appimage_extracted_fixture", true),
            (".mount_Minibar", false),
        ] {
            let directory = root.path().join(name);
            let program = directory.join("usr/bin/codex-minibar");
            std::fs::create_dir_all(program.parent().unwrap()).unwrap();
            std::fs::write(&program, "fixture, never launched").unwrap();
            assert_eq!(
                extraction_required_for(&program, Some(&directory), Some(&image)),
                expected
            );
            assert!(!extraction_required_for(
                Path::new("/usr/bin/codex-minibar"),
                Some(&directory),
                Some(&image)
            ));
            assert!(!extraction_required_for(&program, Some(&directory), None));
        }
    }

    #[test]
    fn extracted_appimages_keep_the_runtime_flag_in_autostart() {
        let command = desktop_command_with_extraction(Path::new("/opt/My AppImage"), true);
        assert_eq!(command, "\"/opt/My AppImage\" --appimage-extract-and-run");
        let entry = entry_for_command(&command);
        assert!(entry.contains("Exec=\"/opt/My AppImage\" --appimage-extract-and-run\n"));
        assert!(!entry.contains("--show"));
    }
}
