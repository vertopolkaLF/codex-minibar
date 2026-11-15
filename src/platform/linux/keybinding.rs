//! The popup shortcut as a desktop-owned custom keyboard shortcut.
//!
//! The app never grabs keys itself. Settings writes a custom shortcut that
//! runs `codex-minibar --toggle` into the desktop's own keyboard settings, so
//! the desktop owns, lists and conflict-checks it like any shortcut the user
//! added by hand. Only GNOME-based desktops (GNOME, Ubuntu, Pop!_OS, Budgie)
//! store shortcuts in a place we can write; elsewhere Settings shows the
//! command to add manually.

use std::{process::Command, time::Duration};

use super::command::output as settings_output;

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

/// GPUI reports Shift plus a symbol key as the produced symbol, with Shift
/// cleared. Shortcuts name the key itself: `~` is Shift and the grave key.
const SHIFTED_KEYS: &[(&str, &str)] = &[
    ("~", "`"),
    ("!", "1"),
    ("@", "2"),
    ("#", "3"),
    ("$", "4"),
    ("%", "5"),
    ("^", "6"),
    ("&", "7"),
    ("*", "8"),
    ("(", "9"),
    (")", "0"),
    ("_", "-"),
    ("+", "="),
    ("{", "["),
    ("}", "]"),
    ("|", "\\"),
    (":", ";"),
    ("\"", "'"),
    ("<", ","),
    (">", "."),
    ("?", "/"),
];

impl Accelerator {
    /// Builds an accelerator from a GPUI key press. `None` for keys a desktop
    /// shortcut cannot use (bare modifiers, media keys, dead keys).
    pub fn from_press(key: &str, ctrl: bool, alt: bool, shift: bool, logo: bool) -> Option<Self> {
        let (key, shift) = match SHIFTED_KEYS.iter().find(|(symbol, _)| *symbol == key) {
            Some((_, base)) => (*base, true),
            None => (key, shift),
        };
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

/// Installs, replaces (`Some`) or removes (`None`) the popup shortcut, and
/// confirms the desktop now reports exactly that.
pub fn save(accelerator: Option<&Accelerator>) -> Result<()> {
    write(accelerator)?;
    let stored = current();
    if stored.as_ref() != accelerator {
        bail!("the desktop reports {stored:?} after writing {accelerator:?}");
    }
    Ok(())
}

/// Upgrade only our original command when the AppImage now needs extraction.
/// Preserve desktop-owned bindings, labels and commands the user customized.
pub fn refresh_extracted_appimage_command() -> Result<()> {
    if !super::autostart::requires_extraction()
        || !supported()
        || !paths()?.iter().any(|path| path == ENTRY_PATH)
    {
        return Ok(());
    }
    let entry = entry_schema();
    let current = unquote(&gsettings(&["get", &entry, "command"])?)
        .context("read installed popup shortcut command")?;
    let program = super::autostart::raw_executable()?;
    let launcher = super::autostart::executable()?;
    if let Some(command) = extracted_command_update(&current, &program, &launcher) {
        gsettings(&["set", &entry, "command", &variant_string(&command)])?;
    }
    Ok(())
}

fn extracted_command_update(
    current: &str,
    program: &std::path::Path,
    launcher: &std::path::Path,
) -> Option<String> {
    let program = program.display().to_string();
    (current == super::instance::toggle_command_for(&program, false)
        || current == super::instance::toggle_command_for(&program, true))
    .then(|| super::instance::toggle_command_for(&launcher.display().to_string(), true))
}

fn write(accelerator: Option<&Accelerator>) -> Result<()> {
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
    quoted_strings(&gsettings(&["get", MEDIA_KEYS, LIST_KEY])?)
        .context("parse desktop shortcut list; leaving existing shortcuts unchanged")
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
    let mut command = Command::new("gsettings");
    command.args(args);
    let (status, stdout, stderr) = settings_output(command, Duration::from_secs(2))?;
    if !status.success() {
        bail!(
            "gsettings {} failed: {}",
            args.first().copied().unwrap_or_default(),
            stderr.trim()
        );
    }
    Ok(stdout.trim().to_owned())
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

/// Parses the string-array output of `gsettings get`. Never recover a
/// partial list: writing it back would silently remove unrelated shortcuts.
fn quoted_strings(text: &str) -> Option<Vec<String>> {
    let text = text.trim();
    let text = text.strip_prefix("@as").unwrap_or(text).trim();
    let text = text.strip_prefix('[')?.strip_suffix(']')?;
    let mut chars = text.chars().peekable();
    let mut strings = Vec::new();
    loop {
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        if chars.peek().is_none() {
            return Some(strings);
        }
        strings.push(parse_string(&mut chars)?);
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        match chars.next() {
            None => return Some(strings),
            Some(',') => {}
            _ => return None,
        }
    }
}

fn parse_string(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<String> {
    let quote = chars.next()?;
    if !matches!(quote, '\'' | '"') {
        return None;
    }
    let mut value = String::new();
    while let Some(c) = chars.next() {
        if c == quote {
            return (!value.contains('\0')).then_some(value);
        }
        if c != '\\' {
            value.push(c);
            continue;
        }
        let escaped = match chars.next()? {
            '\n' => continue,
            'a' => '\x07',
            'b' => '\x08',
            'f' => '\x0c',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'v' => '\x0b',
            c @ ('u' | 'U') => {
                let mut code = 0u32;
                for _ in 0..if c == 'u' { 4 } else { 8 } {
                    code = code
                        .checked_mul(16)?
                        .checked_add(chars.next()?.to_digit(16)?)?;
                }
                char::from_u32(code)?
            }
            c => c,
        };
        value.push(escaped);
    }
    None
}

fn unquote(text: &str) -> Option<String> {
    let mut chars = text.trim().chars().peekable();
    let value = parse_string(&mut chars)?;
    chars.all(char::is_whitespace).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn settings_command_timeout_is_bounded() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 30 & wait"]);
        let started = Instant::now();
        let error = settings_output(command, Duration::from_millis(100)).unwrap_err();
        assert!(error.to_string().contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn settings_command_captures_output_larger_than_a_pipe_buffer() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "head -c 131072 /dev/zero; printf diagnostic >&2"]);
        let (status, stdout, stderr) = settings_output(command, Duration::from_secs(3)).unwrap();
        assert!(status.success());
        assert_eq!(stdout.len(), 131072);
        assert_eq!(stderr, "diagnostic");
    }

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
    fn shifted_symbols_name_the_key_with_shift() {
        // GPUI clears Shift and reports the symbol it produced.
        let accelerator = press("~", true, false, false, false).unwrap();
        assert_eq!(accelerator.to_gtk(), "<Control><Shift>grave");
        assert_eq!(accelerator.label(), "Ctrl+Shift+`");
        let accelerator = press("!", true, false, false, false).unwrap();
        assert_eq!(accelerator.to_gtk(), "<Control><Shift>1");
        let accelerator = press("?", false, true, false, false).unwrap();
        assert_eq!(accelerator.to_gtk(), "<Alt><Shift>slash");
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
    fn desktop_lists_preserve_both_quote_styles_and_unicode_escapes() {
        assert_eq!(
            quoted_strings(r#"@as ["/it's/", '/other/', '/caf\u00e9/']"#).unwrap(),
            vec!["/it's/", "/other/", "/café/"]
        );
        let original = "a \"quoted\" path \\with\ncontrols";
        assert_eq!(
            unquote(&variant_string(original)).as_deref(),
            Some(original)
        );
        assert_eq!(unquote(r"'\U0001f600'").as_deref(), Some("😀"));
    }

    #[test]
    fn malformed_desktop_lists_never_produce_a_partial_replacement() {
        for malformed in [
            "['/keep/',",
            "['/keep/', 42]",
            "['/keep/' '/lost/']",
            "['/keep/', '/unterminated]",
            r"['/keep/', '\uZZZZ']",
            "not a list",
        ] {
            assert!(quoted_strings(malformed).is_none(), "{malformed}");
        }
        assert!(unquote("'binding' extra").is_none());
        assert!(unquote(r"'\u0000'").is_none());
    }

    #[test]
    fn gvariant_text_round_trips_quotes_and_paths() {
        assert_eq!(variant_string(r#"a "b" \c"#), r#""a \"b\" \\c""#);
        assert_eq!(
            quoted_strings("['/one/', '/it\\'s/']").unwrap(),
            vec!["/one/".to_owned(), "/it's/".to_owned()]
        );
        assert!(quoted_strings("@as []").unwrap().is_empty());
        assert_eq!(unquote("'<Control>m'").as_deref(), Some("<Control>m"));
        assert_eq!(unquote("''").as_deref(), Some(""));
    }
    #[test]
    fn extraction_command_refresh_preserves_custom_shortcuts() {
        let program = std::path::Path::new("/opt/My AppImage");
        let launcher = std::path::Path::new("/home/me/isolated-launch.sh");
        for current in [
            "'/opt/My AppImage' --toggle",
            "'/opt/My AppImage' --appimage-extract-and-run --toggle",
        ] {
            assert_eq!(
                extracted_command_update(current, program, launcher),
                Some("'/home/me/isolated-launch.sh' --appimage-extract-and-run --toggle".into())
            );
        }
        for current in [
            "custom-wrapper --toggle",
            "'/opt/My AppImage' --show",
            "'/home/me/isolated-launch.sh' --appimage-extract-and-run --toggle",
        ] {
            assert_eq!(extracted_command_update(current, program, launcher), None);
        }
    }
}
