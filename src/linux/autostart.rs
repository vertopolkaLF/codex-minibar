//! "Start at login" through an XDG autostart entry.
//!
//! `$XDG_CONFIG_HOME/autostart/codex-minibar.desktop` is honored by GNOME,
//! KDE, Xfce, Cinnamon and other desktops implementing the XDG Autostart
//! specification, and like the Windows `Run` value it belongs to the user.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const FILE_NAME: &str = "codex-minibar.desktop";

fn entry_path() -> Result<PathBuf> {
    let base = directories::BaseDirs::new().context("resolve the XDG config directory")?;
    Ok(base.config_dir().join("autostart").join(FILE_NAME))
}

/// The program to start: the AppImage when running from one (its mount
/// point changes on every launch), otherwise this executable.
fn executable() -> Result<PathBuf> {
    if let Some(appimage) = std::env::var_os("APPIMAGE").filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(appimage));
    }
    std::env::current_exe().context("resolve current executable for startup")
}

/// Quotes an argument for a desktop entry `Exec` key.
fn quote_exec(path: &Path) -> String {
    let mut quoted = String::from('"');
    for character in path.display().to_string().chars() {
        if matches!(character, '"' | '`' | '$' | '\\') {
            quoted.push('\\');
        }
        quoted.push(character);
    }
    quoted.push('"');
    // `%` is a field code prefix and must be doubled.
    quoted.replace('%', "%%")
}

fn entry(executable: &Path) -> String {
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
        quote_exec(executable)
    )
}

/// Whether an enabled autostart entry exists.
pub fn is_registered() -> Result<bool> {
    let path = entry_path()?;
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return Ok(false);
    };
    let disabled = contents.lines().any(|line| {
        let line = line.trim();
        line.eq_ignore_ascii_case("Hidden=true")
            || line.eq_ignore_ascii_case("X-GNOME-Autostart-enabled=false")
    });
    Ok(!disabled)
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
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::write(&path, entry(&executable()?))
        .with_context(|| format!("write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_paths_are_quoted() {
        assert_eq!(
            quote_exec(Path::new("/opt/My $Apps/100%/minibar")),
            r#""/opt/My \$Apps/100%%/minibar""#
        );
    }

    #[test]
    fn entry_starts_the_given_program() {
        let text = entry(Path::new("/usr/bin/codex-minibar"));
        assert!(text.starts_with("[Desktop Entry]\n"));
        assert!(text.contains("Exec=\"/usr/bin/codex-minibar\"\n"));
    }
}
