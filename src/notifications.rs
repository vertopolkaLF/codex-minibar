//! Windows toast notifications for rate-limit events.

use chrono::{DateTime, Utc};

use crate::limits::RateLimits;
use crate::settings::NotificationSettings;

/// App User Model ID used for Action Center toasts.
pub const AUMID: &str = "dev.CodexMinibar";

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
    if let Err(error) = windows_impl::show(title, body) {
        eprintln!("failed to show Windows notification: {error:#}");
    }
    #[cfg(not(windows))]
    {
        let _ = (title, body);
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
    let Some(remaining) = remaining else {
        return;
    };
    let Some(resets_at) = resets_at else {
        return;
    };
    if remaining > threshold || *already_notified_for == Some(resets_at) {
        return;
    }
    show(
        &format!("{label} usage low"),
        &format!("Only {remaining}% remaining (alert at {threshold}%)."),
    );
    *already_notified_for = Some(resets_at);
}

#[cfg(windows)]
mod windows_impl {
    use std::path::PathBuf;

    use anyhow::{Context, Result};
    use windows::{
        Data::Xml::Dom::XmlDocument,
        UI::Notifications::{ToastNotification, ToastNotificationManager},
        core::HSTRING,
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
        let aumid: Vec<u16> = AUMID.encode_utf16().chain(std::iter::once(0)).collect();
        let status = unsafe { SetCurrentProcessExplicitAppUserModelID(aumid.as_ptr()) };
        anyhow::ensure!(status == 0, "SetCurrentProcessExplicitAppUserModelID: 0x{status:08X}");
        Ok(())
    }

    pub(super) fn show(title: &str, body: &str) -> Result<()> {
        let logo = notification_icon_path()
            .map(|path| {
                format!(
                    r#"<image placement="appLogoOverride" hint-crop="circle" src="{}"/>"#,
                    escape_xml(&path_to_file_uri(&path))
                )
            })
            .unwrap_or_default();
        let xml = format!(
            r#"<toast><visual><binding template="ToastGeneric"><text>{title}</text><text>{body}</text>{logo}</binding></visual></toast>"#,
            title = escape_xml(title),
            body = escape_xml(body),
            logo = logo,
        );
        let document = XmlDocument::new()?;
        document.LoadXml(&HSTRING::from(xml))?;
        let toast = ToastNotification::CreateToastNotification(&document)?;
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(AUMID))?;
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
