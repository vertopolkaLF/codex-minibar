//! Windows toast notifications for rate-limit events.

use chrono::{DateTime, Utc};

use crate::limits::RateLimits;
use crate::settings::NotificationSettings;

/// App User Model ID used for Action Center toasts.
pub const AUMID: &str = "dev.CodexMinibar";

/// Custom URL protocol used by the update toast action button.
pub const TOAST_PROTOCOL_UPDATE: &str = "codex-minibar:update";

const TOAST_ACTION_TRIGGER: &str = ".toast-action";
const TOAST_ACTION_UPDATE_NOW: &str = "update_now";

/// Returns true when this process was spawned by the update toast protocol link.
pub fn launched_via_toast_update() -> bool {
    std::env::args().any(|arg| {
        arg.to_ascii_lowercase()
            .contains("codex-minibar:update")
    })
}

#[cfg(windows)]
pub fn publish_toast_update_request() -> anyhow::Result<()> {
    toast_activation::publish()
}

#[cfg(not(windows))]
pub fn publish_toast_update_request() -> anyhow::Result<()> {
    Ok(())
}

/// Returns true once when the primary instance should apply a toast update request.
#[cfg(windows)]
pub fn take_toast_update_request() -> bool {
    toast_activation::take()
}

#[cfg(not(windows))]
pub fn take_toast_update_request() -> bool {
    false
}

#[cfg(windows)]
mod toast_activation {
    use std::fs;
    use std::path::{Path, PathBuf};

    use anyhow::{Context, Result};

    use super::{TOAST_ACTION_TRIGGER, TOAST_ACTION_UPDATE_NOW};

    pub fn publish() -> Result<()> {
        let path = trigger_path()?;
        fs::write(&path, TOAST_ACTION_UPDATE_NOW)
            .with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    pub fn take() -> bool {
        let Ok(path) = trigger_path() else {
            return false;
        };
        if !path.exists() {
            return false;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            return false;
        };
        let _ = fs::remove_file(&path);
        content.trim() == TOAST_ACTION_UPDATE_NOW
    }

    fn trigger_path() -> Result<PathBuf> {
        Ok(install_dir()?.join(TOAST_ACTION_TRIGGER))
    }

    fn install_dir() -> Result<PathBuf> {
        std::env::current_exe()
            .context("resolve current executable")
            .and_then(|path| {
                path.parent()
                    .map(Path::to_path_buf)
                    .context("executable has no parent directory")
            })
    }
}

/// Registers the process AUMID and notification identity so Windows can show
/// toasts under "Codex Minibar" instead of a nameless host.
pub fn initialize() {
    #[cfg(windows)]
    if let Err(error) = windows_impl::initialize() {
        eprintln!("failed to register Windows notification identity: {error:#}");
    }
}

/// Shows a Windows toast. Failures are logged; callers should not abort on them.
pub fn show(title: &str, body: &str) {
    #[cfg(windows)]
    if let Err(error) = windows_impl::show(title, body, None) {
        eprintln!("failed to show Windows notification: {error:#}");
    }
    #[cfg(not(windows))]
    {
        let _ = (title, body);
    }
}

/// Toast for a discovered app update with action buttons.
pub fn show_update_available(version: &str, release_url: &str) {
    #[cfg(windows)]
    if let Err(error) = windows_impl::show_update_available(version, release_url) {
        eprintln!("failed to show update notification: {error:#}");
    }
    #[cfg(not(windows))]
    {
        let _ = (version, release_url);
    }
}

/// Tracks previous limit snapshots so reset / low-usage toasts fire once.
#[derive(Debug, Default)]
pub struct LimitNotificationTracker {
    primed: bool,
    primary_resets_at: Option<DateTime<Utc>>,
    secondary_resets_at: Option<DateTime<Utc>>,
    /// `resets_at` of the window we already notified for low primary usage.
    low_usage_notified_primary: Option<DateTime<Utc>>,
    low_usage_notified_secondary: Option<DateTime<Utc>>,
}

impl LimitNotificationTracker {
    pub fn observe(&mut self, limits: &RateLimits, settings: &NotificationSettings) {
        if !self.primed {
            self.capture(limits);
            self.primed = true;
            return;
        }

        let primary_reset = self.primary_resets_at != limits.primary.resets_at
            && limits.primary.resets_at.is_some();
        let secondary_reset = self.secondary_resets_at != limits.secondary.resets_at
            && limits.secondary.resets_at.is_some();

        if primary_reset {
            self.low_usage_notified_primary = None;
            if settings.limits_changed {
                show(
                    "5-hour limit reset",
                    "Your Codex 5-hour usage window has reset.",
                );
            }
        }
        if secondary_reset {
            self.low_usage_notified_secondary = None;
            if settings.limits_changed {
                show(
                    "Weekly limit reset",
                    "Your Codex weekly usage window has reset.",
                );
            }
        }

        if settings.low_usage_enabled {
            let threshold = settings.low_usage_threshold_percent;
            maybe_notify_low_usage(
                "5-hour",
                limits.primary.remaining_percent(),
                limits.primary.resets_at,
                threshold,
                &mut self.low_usage_notified_primary,
            );
        }
        if settings.weekly_low_usage_enabled {
            let threshold = settings.weekly_low_usage_threshold_percent;
            maybe_notify_low_usage(
                "Weekly",
                limits.secondary.remaining_percent(),
                limits.secondary.resets_at,
                threshold,
                &mut self.low_usage_notified_secondary,
            );
        }

        self.capture(limits);
    }

    fn capture(&mut self, limits: &RateLimits) {
        self.primary_resets_at = limits.primary.resets_at;
        self.secondary_resets_at = limits.secondary.resets_at;
    }
}

fn maybe_notify_low_usage(
    label: &str,
    remaining: Option<u8>,
    resets_at: Option<DateTime<Utc>>,
    threshold: u8,
    already_notified_for: &mut Option<DateTime<Utc>>,
) {
    if !take_low_usage_notification(
        remaining,
        resets_at,
        threshold,
        already_notified_for,
    ) {
        return;
    }
    let remaining = remaining.expect("notification requires a remaining percentage");
    show(
        &format!("{label} usage low"),
        &format!("Only {remaining}% remaining (alert at {threshold}%)."),
    );
}

/// Claims the one low-usage notification allowed for a rate-limit window.
fn take_low_usage_notification(
    remaining: Option<u8>,
    resets_at: Option<DateTime<Utc>>,
    threshold: u8,
    already_notified_for: &mut Option<DateTime<Utc>>,
) -> bool {
    let Some(remaining) = remaining else {
        return false;
    };
    let Some(resets_at) = resets_at else {
        return false;
    };
    if remaining > threshold || *already_notified_for == Some(resets_at) {
        return false;
    }
    *already_notified_for = Some(resets_at);
    true
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn low_usage_notification_is_claimed_once_per_limit_window() {
        let first_reset = Utc.with_ymd_and_hms(2026, 7, 14, 12, 0, 0).unwrap();
        let next_reset = Utc.with_ymd_and_hms(2026, 7, 21, 12, 0, 0).unwrap();
        let mut notified_for = None;

        assert!(!take_low_usage_notification(
            Some(21),
            Some(first_reset),
            20,
            &mut notified_for,
        ));
        assert!(take_low_usage_notification(
            Some(20),
            Some(first_reset),
            20,
            &mut notified_for,
        ));
        assert!(!take_low_usage_notification(
            Some(19),
            Some(first_reset),
            20,
            &mut notified_for,
        ));
        assert!(!take_low_usage_notification(
            Some(75),
            Some(first_reset),
            20,
            &mut notified_for,
        ));
        assert!(!take_low_usage_notification(
            Some(20),
            Some(first_reset),
            20,
            &mut notified_for,
        ));
        assert!(take_low_usage_notification(
            Some(20),
            Some(next_reset),
            20,
            &mut notified_for,
        ));
    }
}

#[cfg(windows)]
mod windows_impl {
    use std::path::PathBuf;

    use anyhow::{Context, Result};
    use windows::{
        Data::Xml::Dom::XmlDocument,
        UI::Notifications::{ToastNotification, ToastNotificationManager},
    };
    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
            RegCloseKey, RegCreateKeyExW, RegSetValueExW,
        },
        UI::Shell::SetCurrentProcessExplicitAppUserModelID,
    };

    use super::AUMID;

    pub(super) fn initialize() -> Result<()> {
        register_aumid().context("register notification AUMID")?;
        register_update_protocol().context("register update protocol")?;
        let aumid: Vec<u16> = AUMID.encode_utf16().chain(std::iter::once(0)).collect();
        let status = unsafe { SetCurrentProcessExplicitAppUserModelID(aumid.as_ptr()) };
        anyhow::ensure!(status == 0, "SetCurrentProcessExplicitAppUserModelID: 0x{status:08X}");
        Ok(())
    }

    pub(super) fn show(title: &str, body: &str, actions: Option<&[(&str, &str, &str)]>) -> Result<()> {
        let logo = notification_icon_path()
            .map(|path| {
                format!(
                    r#"<image placement="appLogoOverride" hint-crop="circle" src="{}"/>"#,
                    escape_xml(&path_to_file_uri(&path))
                )
            })
            .unwrap_or_default();
        let action_xml = actions
            .map(|items| {
                let mut out = String::from("<actions>");
                for (label, activation_type, arguments) in items {
                    out.push_str(&format!(
                        r#"<action content="{}" activationType="{}" arguments="{}"/>"#,
                        escape_xml(label),
                        escape_xml(activation_type),
                        escape_xml(arguments),
                    ));
                }
                out.push_str("</actions>");
                out
            })
            .unwrap_or_default();
        let xml = format!(
            r#"<toast><visual><binding template="ToastGeneric"><text>{title}</text><text>{body}</text>{logo}</binding></visual>{actions}</toast>"#,
            title = escape_xml(title),
            body = escape_xml(body),
            logo = logo,
            actions = action_xml,
        );
        show_toast_xml(&xml)
    }

    pub(super) fn show_update_available(version: &str, release_url: &str) -> Result<()> {
        let body = format!("Codex Minibar {version} is ready to install.");
        let actions = [
            ("Update Now", "protocol", super::TOAST_PROTOCOL_UPDATE),
            ("What's New", "protocol", release_url),
        ];
        show("Update available", &body, Some(&actions))
    }

    fn show_toast_xml(xml: &str) -> Result<()> {
        let document = XmlDocument::new()?;
        document.LoadXml(&windows::core::HSTRING::from(xml))?;
        let toast = ToastNotification::CreateToastNotification(&document)?;
        let notifier =
            ToastNotificationManager::CreateToastNotifierWithId(&windows::core::HSTRING::from(
                super::AUMID,
            ))?;
        notifier.Show(&toast)?;
        Ok(())
    }

    fn escape_xml(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    fn register_aumid() -> Result<()> {
        let key = format!(r"Software\Classes\AppUserModelId\{AUMID}");
        set_reg_sz(&key, "DisplayName", "Codex Minibar")?;
        if let Some(icon) = notification_icon_path() {
            // Shell IconUri wants a normal Windows path with backslashes.
            set_reg_sz(&key, "IconUri", &path_to_windows_path(&icon))?;
        }
        Ok(())
    }

    fn register_update_protocol() -> Result<()> {
        let exe = std::env::current_exe().context("resolve executable for protocol registration")?;
        let command = format!("\"{}\" \"%1\"", exe.display());
        let root = r"Software\Classes\codex-minibar";
        set_reg_sz(root, "", "URL:codex-minibar Protocol")?;
        set_reg_sz(root, "URL Protocol", "")?;
        set_reg_sz(&format!(r"{root}\shell\open\command"), "", &command)?;
        Ok(())
    }

    fn notification_icon_path() -> Option<PathBuf> {
        let candidates = [
            std::env::current_exe().ok().and_then(|path| {
                path.parent()
                    .map(|parent| parent.join("assets").join("icons").join("app-icon-64.png"))
            }),
            Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("assets")
                    .join("icons")
                    .join("app-icon-64.png"),
            ),
        ];
        candidates
            .into_iter()
            .flatten()
            .find(|path| path.exists())
            .and_then(|path| path.canonicalize().ok().or(Some(path)))
            .map(strip_extended_path_prefix)
    }

    /// `\\?\C:\...` → `C:\...` so toast/shell APIs accept the path.
    fn strip_extended_path_prefix(path: PathBuf) -> PathBuf {
        let raw = path.to_string_lossy();
        if let Some(stripped) = raw.strip_prefix(r"\\?\") {
            PathBuf::from(stripped)
        } else {
            path
        }
    }

    fn path_to_windows_path(path: &std::path::Path) -> String {
        path.to_string_lossy().replace('/', "\\")
    }

    fn path_to_file_uri(path: &std::path::Path) -> String {
        let windows = path_to_windows_path(path);
        format!("file:///{}", windows.replace('\\', "/"))
    }

    fn set_reg_sz(subkey: &str, name: &str, value: &str) -> Result<()> {
        let subkey_w: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
        let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let data: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
        let mut key: HKEY = std::ptr::null_mut();
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                subkey_w.as_ptr(),
                0,
                std::ptr::null_mut(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        };
        anyhow::ensure!(status == ERROR_SUCCESS, "RegCreateKeyExW({subkey}): {status}");
        let status = unsafe {
            RegSetValueExW(
                key,
                name_w.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * size_of::<u16>()) as u32,
            )
        };
        unsafe { RegCloseKey(key) };
        anyhow::ensure!(status == ERROR_SUCCESS, "RegSetValueExW({name}): {status}");
        Ok(())
    }
}
