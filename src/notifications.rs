//! Windows toast notifications for rate-limit events.

use chrono::{DateTime, Duration, Timelike, Utc};

use crate::limits::RateLimits;
use crate::settings::{NotificationSettings, ProviderKind};

/// App User Model ID used for Action Center toasts.
pub const AUMID: &str = "dev.CodexMinibar";

/// Custom URL protocol used by the update toast action button.
pub const TOAST_PROTOCOL_UPDATE: &str = "codex-minibar:update";

const TOAST_ACTION_TRIGGER: &str = ".toast-action";
const TOAST_ACTION_UPDATE_NOW: &str = "update_now";
const NEW_WINDOW_MINIMUM_ADVANCE: Duration = Duration::minutes(5);

/// Returns true when this process was spawned by the update toast protocol link.
pub fn launched_via_toast_update() -> bool {
    std::env::args().any(|arg| arg.to_ascii_lowercase().contains("codex-minibar:update"))
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
    crate::logger::info(format!("Notification shown: {title} — {body}"));
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

/// Toast after a provider successfully starts a 5-hour limit.
pub fn show_activation_succeeded(provider: ProviderKind) {
    show("5-hour limit started", provider.display_name());
}

/// Tracks previous limit snapshots so reset / low-usage toasts fire once.
#[derive(Debug, Default)]
pub struct LimitNotificationTracker {
    primed: bool,
    primary_resets_at: Option<DateTime<Utc>>,
    secondary_resets_at: Option<DateTime<Utc>>,
    /// Whether the initial snapshot was already inside the low-usage zone.
    /// This remains useful when the provider supplies `resets_at` later.
    startup_low_usage_primary: Option<bool>,
    startup_low_usage_secondary: Option<bool>,
    /// `resets_at` of the window we already notified for low primary usage.
    low_usage_notified_primary: Option<DateTime<Utc>>,
    low_usage_notified_secondary: Option<DateTime<Utc>>,
}

impl LimitNotificationTracker {
    pub fn observe(
        &mut self,
        limits: &RateLimits,
        settings: &NotificationSettings,
        provider: ProviderKind,
    ) {
        self.observe_at(limits, settings, provider, Utc::now());
    }

    fn observe_at(
        &mut self,
        limits: &RateLimits,
        settings: &NotificationSettings,
        provider: ProviderKind,
        now: DateTime<Utc>,
    ) {
        if !self.primed {
            self.capture(limits);
            self.startup_low_usage_primary = low_usage_state(
                limits.primary.remaining_percent(),
                settings.low_usage_threshold_percent,
            );
            self.startup_low_usage_secondary = low_usage_state(
                limits.secondary.remaining_percent(),
                settings.weekly_low_usage_threshold_percent,
            );
            self.primed = true;
            return;
        }

        let primary_reset =
            reset_has_occurred(self.primary_resets_at, limits.primary.resets_at, now);
        let secondary_reset =
            reset_has_occurred(self.secondary_resets_at, limits.secondary.resets_at, now);

        if primary_reset {
            self.startup_low_usage_primary = None;
            if settings.limits_changed {
                show("5-hour limit reset", provider.display_name());
            }
        }
        // Free plans have no weekly limit. Their single monthly quota may shift
        // while the Codex API refreshes, which must not create a reset toast.
        if secondary_reset && can_notify_weekly(limits) {
            self.startup_low_usage_secondary = None;
            if settings.limits_changed {
                show("Weekly limit reset", provider.display_name());
            }
        }

        if settings.low_usage_enabled {
            let threshold = settings.low_usage_threshold_percent;
            maybe_notify_low_usage(
                &format!("{} 5-hour", provider.display_name()),
                limits.primary.remaining_percent(),
                limits.primary.resets_at,
                threshold,
                &mut self.low_usage_notified_primary,
                suppress_startup_low_usage(
                    self.startup_low_usage_primary,
                    limits.primary.remaining_percent(),
                    threshold,
                ),
                now,
            );
        }
        if settings.weekly_low_usage_enabled && can_notify_weekly(limits) {
            let threshold = settings.weekly_low_usage_threshold_percent;
            maybe_notify_low_usage(
                &format!("{} weekly", provider.display_name()),
                limits.secondary.remaining_percent(),
                limits.secondary.resets_at,
                threshold,
                &mut self.low_usage_notified_secondary,
                suppress_startup_low_usage(
                    self.startup_low_usage_secondary,
                    limits.secondary.remaining_percent(),
                    threshold,
                ),
                now,
            );
        }

        self.capture(limits);
    }

    fn capture(&mut self, limits: &RateLimits) {
        self.primary_resets_at = limits.primary.resets_at;
        self.secondary_resets_at = limits.secondary.resets_at;
    }
}

fn low_usage_state(remaining: Option<u8>, threshold: u8) -> Option<bool> {
    remaining.map(|remaining| remaining <= threshold)
}

fn suppress_startup_low_usage(
    startup_low_usage: Option<bool>,
    remaining: Option<u8>,
    threshold: u8,
) -> bool {
    startup_low_usage == Some(true) && low_usage_state(remaining, threshold) == Some(true)
}

/// A reset toast is valid only after the previously advertised deadline has
/// elapsed, the replacement deadline is still ahead of us, and the provider
/// moves it forward by at least five minutes. `None -> Some` is just delayed
/// metadata becoming available, sub-minute corrections are not resets, and
/// replacing one stale deadline with another is not a new active window.
fn reset_has_occurred(
    previous: Option<DateTime<Utc>>,
    current: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> bool {
    let (Some(previous), Some(current)) = (previous, current) else {
        return false;
    };
    let previous = reset_minute(previous);
    let current = reset_minute(current);
    let now = reset_minute(now);
    previous <= now && current > now && current - previous >= NEW_WINDOW_MINIMUM_ADVANCE
}

fn reset_minute(reset: DateTime<Utc>) -> DateTime<Utc> {
    reset
        .with_second(0)
        .and_then(|value| value.with_nanosecond(0))
        .unwrap_or(reset)
}

fn can_notify_weekly(limits: &RateLimits) -> bool {
    !limits.is_free_plan()
}

fn maybe_notify_low_usage(
    label: &str,
    remaining: Option<u8>,
    resets_at: Option<DateTime<Utc>>,
    threshold: u8,
    already_notified_for: &mut Option<DateTime<Utc>>,
    suppress_startup_low_usage: bool,
    now: DateTime<Utc>,
) {
    if suppress_startup_low_usage {
        return;
    }
    if !take_low_usage_notification(remaining, resets_at, threshold, already_notified_for, now) {
        return;
    }
    let remaining = remaining.expect("notification requires a remaining percentage");
    show(
        &format!("{label} limit is low"),
        &format!("{remaining}% remaining"),
    );
}

/// Claims the one low-usage notification allowed for a rate-limit window.
fn take_low_usage_notification(
    remaining: Option<u8>,
    resets_at: Option<DateTime<Utc>>,
    threshold: u8,
    already_notified_for: &mut Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> bool {
    let Some(remaining) = remaining else {
        return false;
    };
    let Some(resets_at) = resets_at else {
        return false;
    };
    if remaining > threshold {
        return false;
    }

    // `resets_at` is not a reliable window identifier by itself: it can move or
    // disappear temporarily while Codex refreshes its rate-limit snapshot. Once
    // a low-usage toast has fired, keep it latched until that window's original
    // deadline has actually passed. This prevents timestamp corrections from
    // producing duplicate notifications during the same reset period.
    if let Some(notified_reset) = *already_notified_for
        && (now < notified_reset || resets_at <= notified_reset)
    {
        return false;
    }

    *already_notified_for = Some(resets_at);
    true
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use crate::limits::LimitWindow;

    use super::*;

    #[test]
    fn free_plan_suppresses_weekly_notifications() {
        let limits = RateLimits {
            plan_type: Some("free".into()),
            ..Default::default()
        };

        assert!(!can_notify_weekly(&limits));
        assert!(can_notify_weekly(&RateLimits::default()));
    }

    #[test]
    fn reset_requires_the_previous_deadline_to_have_elapsed() {
        let previous = Utc.with_ymd_and_hms(2026, 7, 14, 12, 0, 0).unwrap();
        let next = Utc.with_ymd_and_hms(2026, 7, 14, 17, 0, 0).unwrap();

        assert!(!reset_has_occurred(
            Some(previous),
            Some(next),
            previous - chrono::Duration::seconds(1)
        ));
        assert!(reset_has_occurred(Some(previous), Some(next), previous));
        assert!(!reset_has_occurred(
            Some(previous),
            Some(previous),
            previous
        ));
        assert!(!reset_has_occurred(None, Some(next), previous));
    }

    #[test]
    fn reset_rejects_a_new_deadline_that_is_already_stale() {
        let previous = Utc.with_ymd_and_hms(2026, 7, 14, 12, 0, 0).unwrap();
        let stale_replacement = Utc.with_ymd_and_hms(2026, 7, 14, 17, 0, 0).unwrap();
        let now = Utc.with_ymd_and_hms(2026, 7, 14, 18, 0, 0).unwrap();

        assert!(!reset_has_occurred(
            Some(previous),
            Some(stale_replacement),
            now,
        ));
    }

    #[test]
    fn reset_ignores_sub_minute_deadline_corrections_after_expiry() {
        let previous = Utc
            .with_ymd_and_hms(2026, 7, 14, 12, 0, 2)
            .unwrap()
            .with_nanosecond(100_000_000)
            .unwrap();
        let corrected = previous.with_nanosecond(900_000_000).unwrap();

        assert!(!reset_has_occurred(
            Some(previous),
            Some(corrected),
            previous + chrono::Duration::minutes(1),
        ));
    }

    #[test]
    fn reset_ignores_a_one_minute_rounding_boundary() {
        let previous = Utc.with_ymd_and_hms(2026, 7, 14, 12, 59, 0).unwrap();
        let rounded = Utc.with_ymd_and_hms(2026, 7, 14, 13, 0, 0).unwrap();

        assert!(!reset_has_occurred(
            Some(previous),
            Some(rounded),
            previous + chrono::Duration::minutes(1),
        ));
    }

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
            first_reset - chrono::Duration::hours(1),
        ));
        assert!(take_low_usage_notification(
            Some(20),
            Some(first_reset),
            20,
            &mut notified_for,
            first_reset - chrono::Duration::hours(1),
        ));
        assert!(!take_low_usage_notification(
            Some(19),
            Some(first_reset),
            20,
            &mut notified_for,
            first_reset - chrono::Duration::minutes(30),
        ));
        assert!(!take_low_usage_notification(
            Some(75),
            Some(first_reset),
            20,
            &mut notified_for,
            first_reset - chrono::Duration::minutes(30),
        ));
        assert!(!take_low_usage_notification(
            Some(20),
            Some(first_reset),
            20,
            &mut notified_for,
            first_reset + chrono::Duration::minutes(1),
        ));
        // A corrected reset timestamp is still the same active period until the
        // reset we notified for has elapsed.
        assert!(!take_low_usage_notification(
            Some(19),
            Some(next_reset),
            20,
            &mut notified_for,
            first_reset - chrono::Duration::minutes(1),
        ));
        assert!(take_low_usage_notification(
            Some(20),
            Some(next_reset),
            20,
            &mut notified_for,
            first_reset + chrono::Duration::minutes(1),
        ));
    }

    #[test]
    fn startup_low_usage_stays_suppressed_when_reset_metadata_arrives_later() {
        let reset = Utc.with_ymd_and_hms(2026, 7, 14, 12, 0, 0).unwrap();
        let settings = NotificationSettings {
            low_usage_enabled: true,
            low_usage_threshold_percent: 20,
            ..Default::default()
        };
        let initial = RateLimits {
            primary: LimitWindow {
                used_percent: Some(80),
                resets_at: None,
                ..Default::default()
            },
            ..Default::default()
        };
        let reset_metadata = RateLimits {
            primary: LimitWindow {
                used_percent: Some(80),
                resets_at: Some(reset),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut tracker = LimitNotificationTracker::default();

        tracker.observe_at(
            &initial,
            &settings,
            ProviderKind::Codex,
            reset - chrono::Duration::hours(5),
        );
        tracker.observe_at(
            &reset_metadata,
            &settings,
            ProviderKind::Codex,
            reset - chrono::Duration::hours(1),
        );

        assert!(tracker.low_usage_notified_primary.is_none());
    }

    #[test]
    fn startup_low_usage_suppression_only_applies_while_still_low() {
        assert!(suppress_startup_low_usage(Some(true), Some(20), 20));
        assert!(!suppress_startup_low_usage(Some(true), Some(21), 20));
        assert!(!suppress_startup_low_usage(Some(false), Some(20), 20));
        assert!(!suppress_startup_low_usage(None, Some(20), 20));
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
            HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
            RegCreateKeyExW, RegSetValueExW,
        },
        UI::Shell::SetCurrentProcessExplicitAppUserModelID,
    };

    use super::AUMID;

    pub(super) fn initialize() -> Result<()> {
        register_aumid().context("register notification AUMID")?;
        register_update_protocol().context("register update protocol")?;
        let aumid: Vec<u16> = AUMID.encode_utf16().chain(std::iter::once(0)).collect();
        let status = unsafe { SetCurrentProcessExplicitAppUserModelID(aumid.as_ptr()) };
        anyhow::ensure!(
            status == 0,
            "SetCurrentProcessExplicitAppUserModelID: 0x{status:08X}"
        );
        Ok(())
    }

    pub(super) fn show(
        title: &str,
        body: &str,
        actions: Option<&[(&str, &str, &str)]>,
    ) -> Result<()> {
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
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(
            &windows::core::HSTRING::from(super::AUMID),
        )?;
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
        let exe =
            std::env::current_exe().context("resolve executable for protocol registration")?;
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
        anyhow::ensure!(
            status == ERROR_SUCCESS,
            "RegCreateKeyExW({subkey}): {status}"
        );
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
