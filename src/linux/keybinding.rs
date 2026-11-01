//! The popup shortcut as a desktop-owned custom keyboard shortcut.
//!
//! The app never grabs keys itself. Settings writes a custom shortcut that
//! runs `codex-minibar --toggle` into the desktop's own keyboard settings, so
//! the desktop owns, lists and conflict-checks it like any shortcut the user
//! added by hand. Only GNOME-based desktops (GNOME, Ubuntu, Pop!_OS, Budgie)
//! store shortcuts in a place we can write; elsewhere Settings shows the
//! command to add manually.

use std::process::Command;

use anyhow::{Context, Result, bail};

const MEDIA_KEYS: &str = "org.gnome.settings-daemon.plugins.media-keys";
const LIST_KEY: &str = "custom-keybindings";
const ENTRY_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
const ENTRY_PATH: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/codex-minibar/";
const ENTRY_NAME: &str = "Codex Minibar";

/// A key plus modifiers, stored the way GTK and GNOME spell accelerators
/// (`<Control><Alt>m`), so the desktop can read it back unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Accelerator {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub logo: bool,
    /// X11 keysym name, such as `m`, `F13` or `Page_Up`.
    key: String,
}

/// GPUI key name, X11 keysym name and the label shown to the user.
const NAMED_KEYS: &[(&str, &str, &str)] = &[
    ("space", "space", "Space"),
    ("enter", "Return", "Enter"),
    ("tab", "Tab", "Tab"),
    ("backspace", "BackSpace", "Backspace"),
    ("delete", "Delete", "Delete"),
    ("insert", "Insert", "Insert"),
    ("home", "Home", "Home"),
    ("end", "End", "End"),
    ("pageup", "Page_Up", "Page Up"),
    ("pagedown", "Page_Down", "Page Down"),
    ("left", "Left", "Left"),
    ("right", "Right", "Right"),
    ("up", "Up", "Up"),
    ("down", "Down", "Down"),
    ("-", "minus", "-"),
    ("=", "equal", "="),
    (",", "comma", ","),
    (".", "period", "."),
    ("/", "slash", "/"),
    (";", "semicolon", ";"),
    ("'", "apostrophe", "'"),
    ("[", "bracketleft", "["),
    ("]", "bracketright", "]"),
    ("\\", "backslash", "\\"),
    ("`", "grave", "`"),
];

impl Accelerator {
    /// Builds an accelerator from a GPUI key press. `None` for keys a desktop
    /// shortcut cannot use (bare modifiers, media keys, dead keys).
    pub fn from_press(key: &str, ctrl: bool, alt: bool, shift: bool, logo: bool) -> Option<Self> {
        let keysym = if let Some((_, keysym, _)) = NAMED_KEYS.iter().find(|(name, ..)| *name == key)
        {
            (*keysym).to_owned()
        } else if key.len() == 1 && key.chars().all(|c| c.is_ascii_alphanumeric()) {
            key.to_ascii_lowercase()
        } else if let Some(number) = key
            .strip_prefix('f')
            .and_then(|n| n.parse::<u8>().ok())
            .filter(|n| (1..=24).contains(n))
        {
            format!("F{number}")
        } else {
            return None;
        };
        Some(Self {
            ctrl,
            alt,
            shift,
            logo,
            key: keysym,
        })
    }

    /// Ctrl, Alt or Super is required so the shortcut cannot swallow ordinary
    /// typing; F13–F24 are free on standard keyboards and may stand alone.
    pub fn is_valid(&self) -> bool {
        self.ctrl
            || self.alt
            || self.logo
            || self
                .key
                .strip_prefix('F')
                .and_then(|n| n.parse::<u8>().ok())
                .is_some_and(|n| (13..=24).contains(&n))
    }

    /// `<Control><Alt>m`.
    pub fn to_gtk(&self) -> String {
        let mut text = String::new();
        for (on, name) in [
            (self.logo, "<Super>"),
            (self.ctrl, "<Control>"),
            (self.alt, "<Alt>"),
            (self.shift, "<Shift>"),
        ] {
            if on {
                text.push_str(name);
            }
        }
        text.push_str(&self.key);
        text
    }

    pub fn parse(text: &str) -> Option<Self> {
        let mut rest = text.trim();
        let mut accelerator = Self {
            ctrl: false,
            alt: false,
            shift: false,
            logo: false,
            key: String::new(),
        };
        while let Some(after) = rest.strip_prefix('<') {
            let (name, tail) = after.split_once('>')?;
            match name.to_ascii_lowercase().as_str() {
                "control" | "ctrl" | "primary" => accelerator.ctrl = true,
                "alt" | "mod1" => accelerator.alt = true,
                "shift" => accelerator.shift = true,
                "super" | "mod4" | "meta" | "hyper" => accelerator.logo = true,
                _ => return None,
            }
            rest = tail;
        }
        if rest.is_empty() || rest.contains(['<', '>']) {
            return None;
        }
        accelerator.key = rest.to_owned();
        Some(accelerator)
    }

    /// `Ctrl+Alt+M`.
    pub fn label(&self) -> String {
        let mut parts = Vec::with_capacity(5);
        if self.ctrl {
            parts.push("Ctrl".to_owned());
        }
        if self.alt {
            parts.push("Alt".to_owned());
        }
        if self.shift {
            parts.push("Shift".to_owned());
        }
        if self.logo {
            parts.push("Super".to_owned());
        }
        parts.push(
            NAMED_KEYS
                .iter()
                .find(|(_, keysym, _)| *keysym == self.key)
                .map(|(_, _, label)| (*label).to_owned())
                .unwrap_or_else(|| self.key.to_ascii_uppercase()),
        );
        parts.join("+")
    }
}

/// Whether this session stores keyboard shortcuts where we can write them.
pub fn supported() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|desktop| {
        desktop
            .split(':')
            .any(|part| part.eq_ignore_ascii_case("GNOME"))
    }) && gsettings(&["get", MEDIA_KEYS, LIST_KEY]).is_ok()
}

/// The shortcut we installed, if it is still there.
pub fn current() -> Option<Accelerator> {
    if !paths().ok()?.iter().any(|path| path == ENTRY_PATH) {
        return None;
    }
    let value = gsettings(&["get", &entry_schema(), "binding"]).ok()?;
    Accelerator::parse(&unquote(&value)?)
}

/// Installs, replaces (`Some`) or removes (`None`) the popup shortcut.
pub fn save(accelerator: Option<&Accelerator>) -> Result<()> {
    let mut list = paths()?;
    match accelerator {
        Some(accelerator) => {
            let entry = entry_schema();
            let command = super::instance::toggle_command();
            gsettings(&["set", &entry, "name", &variant_string(ENTRY_NAME)])?;
            gsettings(&["set", &entry, "command", &variant_string(&command)])?;
            gsettings(&[
                "set",
                &entry,
                "binding",
                &variant_string(&accelerator.to_gtk()),
            ])?;
            if !list.iter().any(|path| path == ENTRY_PATH) {
                list.push(ENTRY_PATH.to_owned());
                write_paths(&list)?;
            }
        }
        None => {
            if list.iter().any(|path| path == ENTRY_PATH) {
                list.retain(|path| path != ENTRY_PATH);
                write_paths(&list)?;
            }
            let _ = gsettings(&["reset-recursively", &entry_schema()]);
        }
    }
    Ok(())
}

fn entry_schema() -> String {
    format!("{ENTRY_SCHEMA}:{ENTRY_PATH}")
}

fn paths() -> Result<Vec<String>> {
    Ok(quoted_strings(&gsettings(&["get", MEDIA_KEYS, LIST_KEY])?))
}

fn write_paths(paths: &[String]) -> Result<()> {
    let list = paths
        .iter()
        .map(|path| variant_string(path))
        .collect::<Vec<_>>()
        .join(", ");
    let value = if paths.is_empty() {
        "@as []".to_owned()
    } else {
        format!("[{list}]")
    };
    gsettings(&["set", MEDIA_KEYS, LIST_KEY, &value]).map(drop)
}

fn gsettings(args: &[&str]) -> Result<String> {
    let output = Command::new("gsettings")
        .args(args)
        .output()
        .context("run gsettings")?;
    if !output.status.success() {
        bail!(
            "gsettings {} failed: {}",
            args.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// A GVariant text string literal.
fn variant_string(value: &str) -> String {
    let mut text = String::with_capacity(value.len() + 2);
    text.push('"');
    for c in value.chars() {
        if matches!(c, '"' | '\\') {
            text.push('\\');
        }
        text.push(c);
    }
    text.push('"');
    text
}

/// The single-quoted strings of GVariant text such as `['/a/', '/b/']`.
fn quoted_strings(text: &str) -> Vec<String> {
    let mut strings = Vec::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\'' {
            continue;
        }
        let mut value = String::new();
        while let Some(c) = chars.next() {
            match c {
                '\\' => value.extend(chars.next()),
                '\'' => break,
                c => value.push(c),
            }
        }
        strings.push(value);
    }
    strings
}

fn unquote(text: &str) -> Option<String> {
    quoted_strings(text).into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: &str, ctrl: bool, alt: bool, shift: bool, logo: bool) -> Option<Accelerator> {
        Accelerator::from_press(key, ctrl, alt, shift, logo)
    }

    #[test]
    fn presses_become_gtk_accelerators_and_round_trip() {
        let accelerator = press("m", true, true, false, false).unwrap();
        assert_eq!(accelerator.to_gtk(), "<Control><Alt>m");
        assert_eq!(accelerator.label(), "Ctrl+Alt+M");
        assert_eq!(Accelerator::parse("<Control><Alt>m"), Some(accelerator));

        let accelerator = press("pagedown", false, false, true, true).unwrap();
        assert_eq!(accelerator.to_gtk(), "<Super><Shift>Page_Down");
        assert_eq!(accelerator.label(), "Shift+Super+Page Down");
        assert_eq!(Accelerator::parse(&accelerator.to_gtk()), Some(accelerator));
    }

    #[test]
    fn unsupported_keys_are_rejected_and_modifiers_are_required() {
        assert!(press("shift", false, false, true, false).is_none());
        assert!(press("f25", true, false, false, false).is_none());
        assert!(!press("a", false, false, true, false).unwrap().is_valid());
        assert!(!press("f5", false, false, false, false).unwrap().is_valid());
        assert!(press("f13", false, false, false, false).unwrap().is_valid());
        assert!(
            press("space", false, false, false, true)
                .unwrap()
                .is_valid()
        );
    }

    #[test]
    fn parses_desktop_spellings_and_rejects_garbage() {
        let accelerator = Accelerator::parse("<Primary><Mod1>F5").unwrap();
        assert!(accelerator.ctrl && accelerator.alt);
        assert_eq!(accelerator.label(), "Ctrl+Alt+F5");
        assert!(Accelerator::parse("").is_none());
        assert!(Accelerator::parse("<Control>").is_none());
        assert!(Accelerator::parse("<Bogus>a").is_none());
    }

    #[test]
    fn gvariant_text_round_trips_quotes_and_paths() {
        assert_eq!(variant_string(r#"a "b" \c"#), r#""a \"b\" \\c""#);
        assert_eq!(
            quoted_strings("['/one/', '/it\\'s/']"),
            vec!["/one/".to_owned(), "/it's/".to_owned()]
        );
        assert!(quoted_strings("@as []").is_empty());
        assert_eq!(unquote("'<Control>m'").as_deref(), Some("<Control>m"));
        assert_eq!(unquote("''").as_deref(), Some(""));
    }
}
