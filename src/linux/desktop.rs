//! Session bus access and desktop preferences read through the XDG portal.
//!
//! `org.freedesktop.portal.Settings` exposes the color scheme on every
//! portal backend, the standard reduced-motion preference and, on older
//! GNOME backends, `org.gnome.desktop.interface`'s `enable-animations`. Values are
//! initialized synchronously, then refreshed off-thread because the popup
//! asks for them on every frame.

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
const SETTINGS_TIMEOUT: Duration = Duration::from_millis(500);

// A separate connection bounds optional settings calls without imposing the
// same deadline on notifications and other desktop services.
fn settings_bus() -> Option<&'static Connection> {
    static BUS: OnceLock<Option<Connection>> = OnceLock::new();
    BUS.get_or_init(|| {
        zbus::blocking::connection::Builder::session()
            .ok()?
            .method_timeout(SETTINGS_TIMEOUT)
            .build()
            .ok()
    })
    .as_ref()
}

/// The process-wide session bus connection, or `None` without a session bus.
pub fn session_bus() -> Option<&'static Connection> {
    static BUS: OnceLock<Option<Connection>> = OnceLock::new();
    BUS.get_or_init(|| {
        match zbus::blocking::connection::Builder::session().and_then(|builder| {
            // Optional desktop services must not stall notification delivery or
            // tray integration indefinitely when a daemon stops responding.
            builder.method_timeout(Duration::from_secs(5)).build()
        }) {
            Ok(connection) => Some(connection),
            Err(error) => {
                crate::logger::info(format!("D-Bus session bus unavailable: {error}"));
                None
            }
        }
    })
    .as_ref()
}

/// Reads one portal setting. Portals before version 2 only offer `Read`,
/// which wraps the value in an extra variant.
fn read_setting(namespace: &str, key: &str) -> Option<OwnedValue> {
    let bus = settings_bus()?;
    read_setting_with(|method| {
        bus.call_method(
            Some(PORTAL_NAME),
            PORTAL_PATH,
            Some(SETTINGS_INTERFACE),
            method,
            &(namespace, key),
        )?
        .body()
        .deserialize::<OwnedValue>()
    })
}

fn read_setting_with(mut call: impl FnMut(&str) -> zbus::Result<OwnedValue>) -> Option<OwnedValue> {
    let value = match call("ReadOne") {
        Ok(value) => value,
        // Only an older endpoint needs Read. Retrying a timeout, unavailable
        // service or missing setting just doubles the optional call's delay.
        Err(zbus::Error::MethodError(name, _, _))
            if name.as_str() == "org.freedesktop.DBus.Error.UnknownMethod" =>
        {
            call("Read").ok()?
        }
        Err(_) => return None,
    };
    // `Read` nests the value in a variant; unwrap one level when present.
    match &*value {
        zbus::zvariant::Value::Value(inner) => OwnedValue::try_from(inner.as_ref().clone()).ok(),
        _ => Some(value),
    }
}

struct Cached<T> {
    value: Option<T>,
    read_at: Option<Instant>,
    refreshing: bool,
}

fn cached<T: Copy + PartialEq + Send + 'static>(
    slot: &'static Mutex<Cached<T>>,
    read: impl FnOnce() -> Option<T> + Send + 'static,
) -> Option<T> {
    let Ok(mut cache) = slot.lock() else {
        return None;
    };
    // The first answer must be available before the first window is painted.
    if cache.read_at.is_none() {
        cache.value = read();
        cache.read_at = Some(Instant::now());
        return cache.value;
    }
    if !cache.refreshing && cache.read_at.is_some_and(|at| at.elapsed() >= CACHE_TTL) {
        cache.refreshing = true;
        let spawned = std::thread::Builder::new()
            .name("desktop-settings".into())
            .spawn(move || {
                let value = read();
                let changed = if let Ok(mut cache) = slot.lock() {
                    let changed = cache.value != value;
                    cache.value = value;
                    cache.read_at = Some(Instant::now());
                    cache.refreshing = false;
                    changed
                } else {
                    false
                };
                if changed {
                    crate::popup_window::send_command(
                        crate::popup_window::PopupCommand::AppearanceChanged,
                    );
                    super::wake::wake();
                }
            });
        if spawned.is_err() {
            cache.refreshing = false;
            cache.read_at = Some(Instant::now());
        }
    }
    cache.value
}

/// The desktop's color-scheme preference: `Some(true)` for dark,
/// `Some(false)` for light, `None` when the desktop states no preference.
pub fn prefers_dark() -> Option<bool> {
    static CACHE: Mutex<Cached<bool>> = Mutex::new(Cached {
        value: None,
        read_at: None,
        refreshing: false,
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
        refreshing: false,
    });
    cached(&CACHE, || read_animations_enabled(read_setting))
}

fn read_animations_enabled(mut read: impl FnMut(&str, &str) -> Option<OwnedValue>) -> Option<bool> {
    let standard = read("org.freedesktop.appearance", "reduced-motion")
        .and_then(|value| u32::try_from(value).ok())
        // The portal specifies that unknown values mean no preference.
        .map(|value| value != 1);
    if standard == Some(false) {
        return Some(false);
    }
    // A legacy explicit opt-out still applies when the standard setting
    // reports no preference, or when an older portal does not offer it.
    read("org.gnome.desktop.interface", "enable-animations")
        .and_then(|value| bool::try_from(value).ok())
        .or(standard)
}

/// The desktop accent color, when the portal publishes one.
pub fn accent_color() -> Option<(u8, u8, u8)> {
    static CACHE: Mutex<Cached<(u8, u8, u8)>> = Mutex::new(Cached {
        value: None,
        read_at: None,
        refreshing: false,
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
        .get_or_init(|| {
            gnome_font("font-name")
                .or_else(|| fontconfig_family(&[font_kit::family_name::FamilyName::SansSerif]))
        })
        .clone()
}

/// The desktop's monospace font family, resolved like [`ui_font_family`].
pub fn monospace_font_family() -> Option<String> {
    static FAMILY: OnceLock<Option<String>> = OnceLock::new();
    FAMILY
        .get_or_init(|| {
            gnome_font("monospace-font-name")
                .or_else(|| fontconfig_family(&[font_kit::family_name::FamilyName::Monospace]))
        })
        .clone()
}

/// Resolve a GNOME Pango description to a family GPUI can actually load.
fn gnome_font(key: &str) -> Option<String> {
    use font_kit::family_name::FamilyName;
    let value = String::try_from(read_setting("org.gnome.desktop.interface", key)?).ok()?;
    let family = family_from_font_name(&value)?;
    let families = family
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| match name.to_ascii_lowercase().as_str() {
            "sans" | "sans-serif" | "system-ui" => FamilyName::SansSerif,
            "serif" => FamilyName::Serif,
            "monospace" => FamilyName::Monospace,
            _ => FamilyName::Title(name.to_owned()),
        })
        .collect::<Vec<_>>();
    fontconfig_family(&families)
}

fn family_from_font_name(value: &str) -> Option<String> {
    // Pango: [families] [style options] [size[px]] [@variations] [#features].
    // https://docs.gtk.org/Pango/type_func.FontDescription.from_string.html
    let mut family = value.trim();
    for marker in ['#', '@'] {
        let (rest, option) = family
            .rsplit_once(char::is_whitespace)
            .unwrap_or(("", family));
        if option.starts_with(marker) {
            family = rest.trim_end();
        }
    }
    let (rest, size) = family
        .rsplit_once(char::is_whitespace)
        .unwrap_or(("", family));
    if size
        .strip_suffix("px")
        .unwrap_or(size)
        .parse::<f32>()
        .is_ok()
    {
        family = rest.trim_end();
    }
    loop {
        let (rest, option) = family
            .rsplit_once(char::is_whitespace)
            .unwrap_or(("", family));
        // A trailing comma disambiguates families ending in a style word,
        // e.g. "DejaVu Sans Ultra-Light, 11".
        if !is_font_style(option) {
            break;
        }
        family = rest.trim_end();
    }
    let family = family.trim_end_matches(',').trim_end();
    (!family.is_empty()).then(|| family.to_owned())
}

fn is_font_style(value: &str) -> bool {
    matches!(
        value.replace('-', "").to_ascii_lowercase().as_str(),
        "normal"
            | "roman"
            | "oblique"
            | "italic"
            | "smallcaps"
            | "allsmallcaps"
            | "petitecaps"
            | "allpetitecaps"
            | "unicase"
            | "titlecaps"
            | "thin"
            | "ultralight"
            | "extralight"
            | "light"
            | "semilight"
            | "demilight"
            | "book"
            | "regular"
            | "medium"
            | "semibold"
            | "demibold"
            | "bold"
            | "ultrabold"
            | "extrabold"
            | "heavy"
            | "black"
            | "ultrablack"
            | "extrablack"
            | "ultracondensed"
            | "extracondensed"
            | "condensed"
            | "semicondensed"
            | "semiexpanded"
            | "expanded"
            | "extraexpanded"
            | "ultraexpanded"
            | "notrotated"
            | "south"
            | "upsidedown"
            | "north"
            | "rotatedleft"
            | "east"
            | "rotatedright"
            | "west"
            | "withcolor"
            | "withoutcolor"
    )
}

fn fontconfig_family(families: &[font_kit::family_name::FamilyName]) -> Option<String> {
    // Share GPUI's native fontconfig stack. Minimal desktops can have the
    // library and installed fonts without the separate `fc-match` utility.
    let font = font_kit::source::SystemSource::new()
        .select_best_match(families, &font_kit::properties::Properties::new())
        .ok()?
        .load()
        .ok()?;
    let family = font.family_name();
    (!family.is_empty()).then_some(family)
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
    open_with_activation_token(target, None)
}

pub(crate) fn open_with_activation_token(
    target: &std::ffi::OsStr,
    token: Option<&str>,
) -> anyhow::Result<()> {
    use anyhow::Context;
    let status = open_command(target, token)
        .status()
        .context("run xdg-open")?;
    anyhow::ensure!(status.success(), "xdg-open exited with {status}");
    Ok(())
}

fn open_command(target: &std::ffi::OsStr, token: Option<&str>) -> std::process::Command {
    let mut command = std::process::Command::new("xdg-open");
    command
        .arg(target)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .env_remove("XDG_ACTIVATION_TOKEN");
    if let Some(token) = token.filter(|token| super::instance::is_token(token)) {
        command.env("XDG_ACTIVATION_TOKEN", token);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_open_forwards_only_the_current_valid_focus_grant() {
        use std::ffi::OsStr;
        let target = OsStr::new("https://example.com/release?x=1&y=2");
        let grant = "{opaque}/focus+grant=Юникод\nvalue";
        let command = open_command(target, Some(grant));
        assert_eq!(command.get_args().collect::<Vec<_>>(), [target]);
        assert_eq!(
            command
                .get_envs()
                .find(|(key, _)| *key == "XDG_ACTIVATION_TOKEN")
                .unwrap()
                .1,
            Some(OsStr::new(grant))
        );
        let oversized = "x".repeat(4097);
        for token in [
            None,
            Some(""),
            Some("invalid\0grant"),
            Some(oversized.as_str()),
        ] {
            let command = open_command(target, token);
            // Explicit removal also prevents inheriting an unrelated grant.
            assert_eq!(
                command
                    .get_envs()
                    .find(|(key, _)| *key == "XDG_ACTIVATION_TOKEN")
                    .unwrap()
                    .1,
                None
            );
        }
    }

    fn method_error(name: &str) -> zbus::Error {
        zbus::Error::MethodError(
            name.to_owned().try_into().unwrap(),
            None,
            zbus::Message::method_call(PORTAL_PATH, "ReadOne")
                .unwrap()
                .build(&())
                .unwrap(),
        )
    }

    #[test]
    fn old_portals_use_legacy_read_and_unwrap_its_extra_variant() {
        let mut calls = Vec::new();
        let value = read_setting_with(|method| {
            calls.push(method.to_owned());
            match method {
                "ReadOne" => Err(method_error("org.freedesktop.DBus.Error.UnknownMethod")),
                "Read" => Ok(OwnedValue::try_from(zbus::zvariant::Value::Value(Box::new(
                    zbus::zvariant::Value::from(1u32),
                )))
                .unwrap()),
                _ => panic!("unexpected method"),
            }
        })
        .unwrap();
        assert_eq!(u32::try_from(value).unwrap(), 1);
        assert_eq!(calls, ["ReadOne", "Read"]);
    }

    #[test]
    fn modern_portal_failures_do_not_repeat_the_read_through_the_legacy_api() {
        for error in [
            method_error("org.freedesktop.portal.Error.NotFound"),
            method_error("org.freedesktop.DBus.Error.NoReply"),
            method_error("org.freedesktop.DBus.Error.ServiceUnknown"),
            zbus::Error::InputOutput(std::sync::Arc::new(std::io::Error::from(
                std::io::ErrorKind::TimedOut,
            ))),
            zbus::Error::InvalidReply,
        ] {
            let mut calls = Vec::new();
            assert!(
                read_setting_with(|method| {
                    calls.push(method.to_owned());
                    Err(error.clone())
                })
                .is_none()
            );
            assert_eq!(calls, ["ReadOne"]);
        }
        let mut calls = Vec::new();
        let value = read_setting_with(|method| {
            calls.push(method.to_owned());
            Ok(OwnedValue::from(2u32))
        })
        .unwrap();
        assert_eq!(u32::try_from(value).unwrap(), 2);
        assert_eq!(calls, ["ReadOne"]);
    }

    #[test]
    fn standard_reduced_motion_disables_animations_without_gnome_settings() {
        assert_eq!(
            read_animations_enabled(|namespace, key| {
                assert_eq!(
                    (namespace, key),
                    ("org.freedesktop.appearance", "reduced-motion")
                );
                Some(OwnedValue::from(1u32))
            }),
            Some(false)
        );
    }

    #[test]
    fn motion_preferences_keep_legacy_opt_out_and_handle_unknown_portal_values() {
        for (standard, legacy, expected) in [
            (Some(0u32), None, Some(true)),
            (Some(99), None, Some(true)),
            (Some(0), Some(false), Some(false)),
            (None, Some(false), Some(false)),
            (None, Some(true), Some(true)),
            (None, None, None),
        ] {
            assert_eq!(
                read_animations_enabled(|namespace, key| match (namespace, key) {
                    ("org.freedesktop.appearance", "reduced-motion") =>
                        standard.map(OwnedValue::from),
                    ("org.gnome.desktop.interface", "enable-animations") =>
                        legacy.map(OwnedValue::from),
                    _ => panic!("unexpected portal setting"),
                }),
                expected
            );
        }
    }

    #[test]
    fn initial_setting_is_ready_before_returning() {
        let cache: &'static Mutex<Cached<i32>> = Box::leak(Box::new(Mutex::new(Cached {
            value: None,
            read_at: None,
            refreshing: false,
        })));
        assert_eq!(cached(cache, || Some(7)), Some(7));
        assert_eq!(
            cached(cache, || panic!("fresh cache must not read")),
            Some(7)
        );
    }

    #[test]
    fn expired_settings_refresh_without_blocking_or_duplicate_reads() {
        use std::sync::mpsc;
        let cache: &'static Mutex<Cached<i32>> = Box::leak(Box::new(Mutex::new(Cached {
            value: Some(7),
            read_at: Some(Instant::now() - CACHE_TTL),
            refreshing: false,
        })));
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        // The reader cannot finish until this call has returned and the test
        // releases it. No timing threshold is needed to prove nonblocking.
        assert_eq!(
            cached(cache, move || {
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Some(9)
            }),
            Some(7)
        );
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(
            cached(cache, || panic!("refresh already in flight")),
            Some(7)
        );
        // Wait for the worker's cache update by taking the mutex after it has
        // published, rather than relying on arbitrary sleeps.
        release_tx.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while cache.lock().unwrap().refreshing {
            assert!(Instant::now() < deadline, "refresh did not complete");
            std::thread::yield_now();
        }
        assert_eq!(
            cached(cache, || panic!("fresh cache must not read")),
            Some(9)
        );
    }

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

    #[test]
    fn pango_styles_and_options_are_not_part_of_the_font_family() {
        for (description, expected) in [
            ("Noto Sans Semi-Bold Italic 11", Some("Noto Sans")),
            (
                "Cantarell Italic Light 15 @wght=200 #tnum=1",
                Some("Cantarell"),
            ),
            ("Fira Code\tMedium 13px", Some("Fira Code")),
            (
                "DejaVu Sans Ultra-Light, 11",
                Some("DejaVu Sans Ultra-Light"),
            ),
            ("Times New Roman, Bold 11", Some("Times New Roman")),
            (
                "Noto Sans, DejaVu Sans Bold 12",
                Some("Noto Sans, DejaVu Sans"),
            ),
            ("Bold 11", None),
            ("11", None),
            ("11px", None),
            ("#tnum=1", None),
        ] {
            assert_eq!(
                family_from_font_name(description).as_deref(),
                expected,
                "{description}"
            );
        }
    }
}
