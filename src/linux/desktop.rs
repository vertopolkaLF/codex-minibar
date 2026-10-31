//! Session bus access and desktop preferences read through the XDG portal.
//!
//! `org.freedesktop.portal.Settings` exposes the color scheme on every
//! portal backend and, on GNOME (and desktops reusing its schemas), the
//! `org.gnome.desktop.interface` keys such as `enable-animations`. Values are
//! cached briefly because the popup asks for them on every frame.

use std::{
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use zbus::{blocking::Connection, zvariant::OwnedValue};

const PORTAL_NAME: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const SETTINGS_INTERFACE: &str = "org.freedesktop.portal.Settings";
/// How long a portal answer is reused before asking again.
const CACHE_TTL: Duration = Duration::from_secs(1);

/// The process-wide session bus connection, or `None` without a session bus.
pub fn session_bus() -> Option<&'static Connection> {
    static BUS: OnceLock<Option<Connection>> = OnceLock::new();
    BUS.get_or_init(|| match Connection::session() {
        Ok(connection) => Some(connection),
        Err(error) => {
            crate::logger::info(format!("D-Bus session bus unavailable: {error}"));
            None
        }
    })
    .as_ref()
}

/// Reads one portal setting. Portals before version 2 only offer `Read`,
/// which wraps the value in an extra variant.
fn read_setting(namespace: &str, key: &str) -> Option<OwnedValue> {
    let bus = session_bus()?;
    let call = |method: &str| {
        bus.call_method(
            Some(PORTAL_NAME),
            PORTAL_PATH,
            Some(SETTINGS_INTERFACE),
            method,
            &(namespace, key),
        )
        .ok()?
        .body()
        .deserialize::<OwnedValue>()
        .ok()
    };
    let value = call("ReadOne").or_else(|| call("Read"))?;
    // `Read` nests the value in a variant; unwrap one level when present.
    match &*value {
        zbus::zvariant::Value::Value(inner) => OwnedValue::try_from(inner.as_ref().clone()).ok(),
        _ => Some(value),
    }
}

struct Cached<T> {
    value: Option<T>,
    read_at: Option<Instant>,
}

fn cached<T: Copy>(slot: &Mutex<Cached<T>>, read: impl FnOnce() -> Option<T>) -> Option<T> {
    let Ok(mut slot) = slot.lock() else {
        return read();
    };
    if slot
        .read_at
        .is_some_and(|read_at| read_at.elapsed() < CACHE_TTL)
    {
        return slot.value;
    }
    slot.value = read();
    slot.read_at = Some(Instant::now());
    slot.value
}

/// The desktop's color-scheme preference: `Some(true)` for dark,
/// `Some(false)` for light, `None` when the desktop states no preference.
pub fn prefers_dark() -> Option<bool> {
    static CACHE: Mutex<Cached<bool>> = Mutex::new(Cached {
        value: None,
        read_at: None,
    });
    cached(&CACHE, || {
        let scheme =
            u32::try_from(read_setting("org.freedesktop.appearance", "color-scheme")?).ok()?;
        match scheme {
            1 => Some(true),
            2 => Some(false),
            _ => None,
        }
    })
}

/// The desktop's "Animations" accessibility switch, when it publishes one.
pub fn animations_enabled() -> Option<bool> {
    static CACHE: Mutex<Cached<bool>> = Mutex::new(Cached {
        value: None,
        read_at: None,
    });
    cached(&CACHE, || {
        bool::try_from(read_setting(
            "org.gnome.desktop.interface",
            "enable-animations",
        )?)
        .ok()
    })
}

/// The desktop accent color, when the portal publishes one.
pub fn accent_color() -> Option<(u8, u8, u8)> {
    static CACHE: Mutex<Cached<(u8, u8, u8)>> = Mutex::new(Cached {
        value: None,
        read_at: None,
    });
    cached(&CACHE, || {
        let value = read_setting("org.freedesktop.appearance", "accent-color")?;
        let (red, green, blue) = <(f64, f64, f64)>::try_from(value).ok()?;
        // Out-of-range channels mean "no accent set".
        let channel = |value: f64| {
            (0.0..=1.0)
                .contains(&value)
                .then(|| (value * 255.0).round() as u8)
        };
        Some((channel(red)?, channel(green)?, channel(blue)?))
    })
}

/// The desktop's interface font family (GNOME `font-name`, read through
/// the portal), else fontconfig's default sans-serif.
/// Resolved once per process; renderers ask for it every frame.
pub fn ui_font_family() -> Option<String> {
    static FAMILY: OnceLock<Option<String>> = OnceLock::new();
    FAMILY
        .get_or_init(|| gnome_font("font-name").or_else(|| fontconfig_family("sans-serif")))
        .clone()
}

/// The desktop's monospace font family, resolved like [`ui_font_family`].
pub fn monospace_font_family() -> Option<String> {
    static FAMILY: OnceLock<Option<String>> = OnceLock::new();
    FAMILY
        .get_or_init(|| {
            gnome_font("monospace-font-name").or_else(|| fontconfig_family("monospace"))
        })
        .clone()
}

/// A GNOME font setting such as `"Ubuntu Sans 11"`, without its size.
fn gnome_font(key: &str) -> Option<String> {
    let value = String::try_from(read_setting("org.gnome.desktop.interface", key)?).ok()?;
    family_from_font_name(&value)
}

fn family_from_font_name(value: &str) -> Option<String> {
    let value = value.trim();
    let family = match value.rsplit_once(' ') {
        Some((family, size)) if size.parse::<f32>().is_ok() => family.trim(),
        _ => value,
    };
    (!family.is_empty()).then(|| family.to_owned())
}

fn fontconfig_family(pattern: &str) -> Option<String> {
    let output = std::process::Command::new("fc-match")
        .args(["--format=%{family[0]}", pattern])
        .output()
        .ok()?;
    let family = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (output.status.success() && !family.is_empty()).then_some(family)
}

/// GNOME Shell (and the Ubuntu, Pantheon and Budgie shells derived from its
/// design) draw a dark top bar regardless of the color scheme.
pub fn panel_always_dark() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|desktops| {
        desktops.split(':').any(|desktop| {
            matches!(
                desktop.to_ascii_lowercase().as_str(),
                "gnome" | "ubuntu" | "pantheon" | "budgie" | "unity"
            )
        })
    })
}

/// Opens a URL or path with the user's default handler.
pub fn open(target: &std::ffi::OsStr) -> anyhow::Result<()> {
    use anyhow::Context;
    let status = std::process::Command::new("xdg-open")
        .arg(target)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("run xdg-open")?;
    anyhow::ensure!(status.success(), "xdg-open exited with {status}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_names_drop_their_size() {
        assert_eq!(
            family_from_font_name("Ubuntu Sans 11").as_deref(),
            Some("Ubuntu Sans")
        );
        assert_eq!(
            family_from_font_name("Cantarell 10.5").as_deref(),
            Some("Cantarell")
        );
        assert_eq!(family_from_font_name("Inter").as_deref(), Some("Inter"));
        assert_eq!(family_from_font_name("  "), None);
    }
}
