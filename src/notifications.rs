//! In-app notifications for activations, rate-limit events and updates.
//!
//! Any thread can post a [`Notification`]. It travels to the GPUI thread,
//! which shows it as an animated card in the toast host
//! (`popup_window::ui::toast`) and plays the sound of its kind.

use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Duration, Timelike, Utc};

use crate::limits::RateLimits;
use crate::{instances::ProviderId, settings::NotificationSettings};

/// App User Model ID of the process (taskbar identity of its windows).
pub const AUMID: &str = "dev.CodexMinibar";

const NEW_WINDOW_MINIMUM_ADVANCE: Duration = Duration::minutes(5);

static SOUND_ENABLED: AtomicBool = AtomicBool::new(true);

/// What a notification is about; picks its icon, color, sound and lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NotificationKind {
    Info,
    Success,
    /// A rate-limit window came back.
    Reset,
    Warning,
    Error,
    Update,
}

impl NotificationKind {
    pub const ALL: [Self; 6] = [
        Self::Info,
        Self::Success,
        Self::Reset,
        Self::Warning,
        Self::Error,
        Self::Update,
    ];

    /// How long the card stays before it leaves on its own. Hovering pauses it.
    pub fn lifetime(self) -> std::time::Duration {
        std::time::Duration::from_secs(match self {
            Self::Info | Self::Success | Self::Reset => 6,
            Self::Warning => 8,
            Self::Error => 10,
            Self::Update => 14,
        })
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn sound(self) -> &'static [u8] {
        // Synthesized by tools/notification_sounds.py.
        match self {
            Self::Info => include_bytes!("../assets/sounds/info.wav"),
            Self::Success => include_bytes!("../assets/sounds/success.wav"),
            Self::Reset => include_bytes!("../assets/sounds/reset.wav"),
            Self::Warning => include_bytes!("../assets/sounds/warning.wav"),
            Self::Error => include_bytes!("../assets/sounds/error.wav"),
            Self::Update => include_bytes!("../assets/sounds/update.wav"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotificationAction {
    InstallUpdate,
    OpenUrl(String),
}

impl NotificationAction {
    /// Whether the card should stay after this button: release notes are
    /// read beside it, so it waits to be closed by hand.
    pub fn pins_card(&self) -> bool {
        matches!(self, Self::OpenUrl(_))
    }

    /// Runs the action off the GPUI thread: installing exits the process.
    pub fn run(&self) {
        let action = self.clone();
        std::thread::spawn(move || match action {
            NotificationAction::InstallUpdate => {
                if let Err(error) = crate::updater::apply_pending_update() {
                    eprintln!("failed to apply update: {error:#}");
                    show_error(crate::i18n::tr("update-failed"), &format!("{error:#}"));
                }
            }
            NotificationAction::OpenUrl(url) => {
                if let Err(error) = crate::updater::open_url(&url) {
                    eprintln!("failed to open {url}: {error:#}");
                }
            }
        });
    }
}

/// The quota window a limit notification is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitFocus {
    /// The 5-hour session.
    Primary,
    /// The weekly (or monthly, on free plans) window.
    Secondary,
}

/// Provider quotas drawn under a limit notification, so the card answers
/// "how much do I have now" without opening the popup.
#[derive(Clone, Debug)]
pub struct LimitAlert {
    pub provider: ProviderId,
    pub limits: RateLimits,
    pub focus: LimitFocus,
}

#[derive(Clone, Debug)]
pub struct Notification {
    pub kind: NotificationKind,
    pub title: String,
    pub body: String,
    /// `(label, action)`; the first one is the primary button.
    pub actions: Vec<(String, NotificationAction)>,
    pub limits: Option<Box<LimitAlert>>,
    /// Plays the kind's sound even when Windows reports a busy state.
    pub force_sound: bool,
}

impl Notification {
    pub fn new(kind: NotificationKind, title: &str, body: &str) -> Self {
        Self {
            kind,
            title: title.to_owned(),
            body: body.to_owned(),
            actions: Vec::new(),
            limits: None,
            force_sound: false,
        }
    }

    /// A notification about one quota window of `provider`. The body names
    /// the account; the card draws the quotas themselves.
    pub fn limit(
        kind: NotificationKind,
        title: &str,
        name: &str,
        provider: ProviderId,
        limits: &RateLimits,
        focus: LimitFocus,
    ) -> Self {
        let mut notification = Self::new(kind, title, &limit_identity(name, limits));
        notification.limits = Some(Box::new(LimitAlert {
            provider,
            limits: limits.clone(),
            focus,
        }));
        notification
    }

    /// How long the card stays before it leaves on its own. Cards with
    /// quotas get a little longer to be read.
    pub fn lifetime(&self) -> std::time::Duration {
        let extra = if self.limits.is_some() { 3 } else { 0 };
        self.kind.lifetime() + std::time::Duration::from_secs(extra)
    }
}

/// `Codex · Pro · work@example.com`: instance, plan and account.
fn limit_identity(name: &str, limits: &RateLimits) -> String {
    let mut parts = vec![name.to_owned()];
    if let Some(plan) = limits
        .plan_type
        .as_deref()
        .filter(|plan| !plan.trim().is_empty())
    {
        parts.push(crate::popup_window::capitalize_plan_name(plan));
    }
    if let Some(account) = limits
        .account_name
        .as_deref()
        .map(str::trim)
        .filter(|account| !account.is_empty() && *account != name)
    {
        parts.push(account.to_owned());
    }
    parts.join(" \u{00b7} ")
}

/// Mirrors `notifications.sound` so the toast host never needs settings.
pub fn set_sound_enabled(enabled: bool) {
    SOUND_ENABLED.store(enabled, Ordering::Relaxed);
}

/// Registers the process AUMID so the taskbar and shell say "Codex Minibar".
pub fn initialize() {
    #[cfg(windows)]
    if let Err(error) = windows_impl::initialize() {
        eprintln!("failed to register the app identity: {error:#}");
    }
}

/// Posts a notification to the toast host. Safe from any thread.
pub fn notify(notification: Notification) {
    crate::logger::info(format!(
        "Notification shown: {} — {}",
        notification.title, notification.body
    ));
    crate::popup_window::send_command(crate::popup_window::PopupCommand::Toast(Box::new(
        notification,
    )));
}

/// An informational notification.
pub fn show(title: &str, body: &str) {
    show_kind(NotificationKind::Info, title, body);
}

pub fn show_kind(kind: NotificationKind, title: &str, body: &str) {
    notify(Notification::new(kind, title, body));
}

/// A failed operation the user should know about.
pub fn show_error(title: &str, body: &str) {
    show_kind(NotificationKind::Error, title, body);
}

/// A discovered app update with install and release-notes buttons.
pub fn show_update_available(version: &str, release_url: &str) {
    notify(update_available(version, release_url));
}

fn update_available(version: &str, release_url: &str) -> Notification {
    let mut notification = Notification::new(
        NotificationKind::Update,
        crate::i18n::tr("update-available-67fd3a"),
        &format!(
            "Codex Minibar {version}. {}",
            crate::i18n::tr("a-new-release-is-ready-to-install")
        ),
    );
    notification.actions = vec![
        (
            crate::i18n::tr("update-now").to_owned(),
            NotificationAction::InstallUpdate,
        ),
        (
            crate::i18n::tr("what-s-new").to_owned(),
            NotificationAction::OpenUrl(release_url.to_owned()),
        ),
    ];
    notification
}

/// Notification after a provider successfully starts a 5-hour limit, with
/// its quotas when they are known.
pub fn show_activation_succeeded(provider: ProviderId, limits: Option<&RateLimits>) {
    show_activation(
        NotificationKind::Success,
        crate::i18n::tr("msg-5-hour-limit-started"),
        provider,
        limits,
    );
}

/// Notification after automatic activation follows a newly reset 5-hour window.
pub fn show_activation_succeeded_after_reset(provider: ProviderId, limits: &RateLimits) {
    show_activation(
        NotificationKind::Reset,
        crate::i18n::tr("msg-5-hour-limit-reset-and-activated"),
        provider,
        Some(limits),
    );
}

fn show_activation(
    kind: NotificationKind,
    title: &str,
    provider: ProviderId,
    limits: Option<&RateLimits>,
) {
    let name = provider.qualified_name();
    notify(match limits {
        Some(limits) => {
            Notification::limit(kind, title, &name, provider, limits, LimitFocus::Primary)
        }
        None => Notification::new(kind, title, &name),
    });
}

/// Plays the sound of `notification` unless sounds are off or Windows says
/// the user is presenting, gaming full screen or otherwise busy.
pub(crate) fn play_sound(notification: &Notification) {
    if !SOUND_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    #[cfg(windows)]
    {
        if !notification.force_sound && !windows_impl::accepts_notifications() {
            return;
        }
        windows_impl::play(notification.kind.sound());
    }
    #[cfg(not(windows))]
    let _ = notification;
}

/// TEMP: sample notifications for the Settings demo section.
pub fn demo(kind: NotificationKind) {
    use crate::{limits::LimitWindow, settings::ProviderKind};

    let window = |used: u8, minutes: i64, length: u32| LimitWindow {
        used_percent: Some(used),
        resets_at: Some(chrono::Utc::now() + Duration::minutes(minutes)),
        duration_minutes: Some(length),
    };
    let sample = |kind: ProviderKind, plan: &str, account: &str, five, week| {
        (
            ProviderId::primary(kind),
            RateLimits {
                primary: five,
                secondary: week,
                plan_type: Some(plan.to_owned()),
                account_name: Some(account.to_owned()),
                ..Default::default()
            },
        )
    };
    let mut notification = match kind {
        NotificationKind::Info => Notification::new(
            kind,
            "New Codex reset info",
            "A forced reset is announced for Codex on Oct 12, 14:00 (in 2 days).",
        ),
        NotificationKind::Success => {
            let (provider, limits) = sample(
                ProviderKind::Codex,
                "pro",
                "work@example.com",
                window(0, 299, 300),
                window(37, 4 * 24 * 60, 10_080),
            );
            Notification::limit(
                kind,
                crate::i18n::tr("msg-5-hour-limit-started"),
                "Codex",
                provider,
                &limits,
                LimitFocus::Primary,
            )
        }
        NotificationKind::Reset => {
            let (provider, limits) = sample(
                ProviderKind::Claude,
                "max",
                "me@example.com",
                window(64, 132, 300),
                window(0, 7 * 24 * 60 - 1, 10_080),
            );
            Notification::limit(
                kind,
                crate::i18n::tr("weekly-limit-reset"),
                "Claude",
                provider,
                &limits,
                LimitFocus::Secondary,
            )
        }
        NotificationKind::Warning => {
            let (provider, limits) = sample(
                ProviderKind::Codex,
                "plus",
                "work@example.com",
                window(88, 97, 300),
                window(52, 3 * 24 * 60, 10_080),
            );
            Notification::limit(
                kind,
                &crate::i18n::format(
                    "label-limit-is-low",
                    &[("label", "Codex 5-hour".to_owned())],
                ),
                "Codex",
                provider,
                &limits,
                LimitFocus::Primary,
            )
        }
        NotificationKind::Error => Notification::new(
            kind,
            crate::i18n::tr("update-failed"),
            "The installer could not replace codex-minibar.exe: access is denied (os error 5).",
        ),
        NotificationKind::Update => update_available(
            "9.9.9",
            "https://github.com/vertopolkaLF/codex-minibar/releases",
        ),
    };
    notification.force_sound = true;
    notify(notification);
}

/// TEMP: every demo kind at once, to watch the stack.
pub fn demo_all() {
    for kind in NotificationKind::ALL {
        demo(kind);
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct LimitNotificationResult {
    pub primary_reset: bool,
}

/// Tracks previous API limit snapshots so actual reset / low-usage toasts fire
/// once. Public reset-feed announcements use a separate info-only path.
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
    /// Shown in toasts instead of the provider name, to tell apart several
    /// accounts of one provider.
    name: Option<String>,
}

impl LimitNotificationTracker {
    pub fn named(&mut self, name: String) -> &mut Self {
        self.name = Some(name);
        self
    }

    pub fn observe(
        &mut self,
        limits: &RateLimits,
        settings: &NotificationSettings,
        provider: ProviderId,
    ) -> LimitNotificationResult {
        self.observe_at(limits, settings, provider, Utc::now(), false)
    }

    pub fn observe_with_primary_reset_deferred(
        &mut self,
        limits: &RateLimits,
        settings: &NotificationSettings,
        provider: ProviderId,
    ) -> LimitNotificationResult {
        self.observe_at(limits, settings, provider, Utc::now(), true)
    }

    fn observe_at(
        &mut self,
        limits: &RateLimits,
        settings: &NotificationSettings,
        provider: ProviderId,
        now: DateTime<Utc>,
        defer_primary_reset: bool,
    ) -> LimitNotificationResult {
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
            return LimitNotificationResult::default();
        }

        let primary_reset =
            reset_has_occurred(self.primary_resets_at, limits.primary.resets_at, now);
        let secondary_reset =
            reset_has_occurred(self.secondary_resets_at, limits.secondary.resets_at, now);
        // Exhausted weekly already blocks auto-activation. A 5h reset toast is
        // equally useless until that weekly quota comes back.
        let notify_five_hour_reset = can_notify_five_hour_reset(limits);
        let name = self
            .name
            .clone()
            .unwrap_or_else(|| provider.qualified_name());

        if primary_reset {
            self.startup_low_usage_primary = None;
            if settings.limits_changed && !defer_primary_reset && notify_five_hour_reset {
                notify(Notification::limit(
                    NotificationKind::Reset,
                    crate::i18n::tr("msg-5-hour-limit-reset"),
                    &name,
                    provider,
                    limits,
                    LimitFocus::Primary,
                ));
            }
        }
        // Free plans have no weekly limit. Their single monthly quota may shift
        // while the Codex API refreshes, which must not create a reset toast.
        if secondary_reset && can_notify_weekly(limits) {
            self.startup_low_usage_secondary = None;
            if settings.limits_changed {
                notify(Notification::limit(
                    NotificationKind::Reset,
                    crate::i18n::tr("weekly-limit-reset"),
                    &name,
                    provider,
                    limits,
                    LimitFocus::Secondary,
                ));
            }
        }

        if settings.low_usage_enabled {
            let threshold = settings.low_usage_threshold_percent;
            maybe_notify_low_usage(
                &crate::i18n::format("name-5-hour", &[("name", name.to_string())]),
                (&name, provider, limits, LimitFocus::Primary),
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
            let label = secondary_limit_label(limits, &name);
            maybe_notify_low_usage(
                &label,
                (&name, provider, limits, LimitFocus::Secondary),
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
        LimitNotificationResult {
            primary_reset: primary_reset && notify_five_hour_reset,
        }
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

fn can_notify_five_hour_reset(limits: &RateLimits) -> bool {
    !limits.weekly_exhausted()
}

fn secondary_limit_label(limits: &RateLimits, name: &str) -> String {
    if let Some(name) = limits
        .secondary_limit_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        return name.to_owned();
    }
    crate::i18n::format("name-weekly", &[("name", name.to_string())])
}

fn maybe_notify_low_usage(
    label: &str,
    (name, provider, limits, focus): (&str, ProviderId, &RateLimits, LimitFocus),
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
    notify(Notification::limit(
        NotificationKind::Warning,
        &crate::i18n::format("label-limit-is-low", &[("label", label.to_string())]),
        name,
        provider,
        limits,
        focus,
    ));
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
    fn exhausted_weekly_suppresses_five_hour_reset_notifications() {
        let limits = RateLimits {
            secondary: LimitWindow {
                used_percent: Some(100),
                ..Default::default()
            },
            ..Default::default()
        };

        assert!(!can_notify_five_hour_reset(&limits));
        assert!(can_notify_five_hour_reset(&RateLimits::default()));
        assert!(can_notify_five_hour_reset(&RateLimits {
            secondary: LimitWindow {
                used_percent: Some(99),
                ..Default::default()
            },
            ..Default::default()
        }));
    }

    #[test]
    fn named_secondary_limit_is_used_in_low_usage_label() {
        let limits = RateLimits {
            secondary_limit_name: Some("Cursor Models".into()),
            ..Default::default()
        };

        assert_eq!(secondary_limit_label(&limits, "Cursor"), "Cursor Models");
    }

    #[test]
    fn unnamed_secondary_limit_keeps_weekly_fallback() {
        assert_eq!(
            secondary_limit_label(&RateLimits::default(), "Codex"),
            "Codex weekly"
        );
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
            ProviderId::primary(crate::settings::ProviderKind::Codex),
            reset - chrono::Duration::hours(5),
            false,
        );
        tracker.observe_at(
            &reset_metadata,
            &settings,
            ProviderId::primary(crate::settings::ProviderKind::Codex),
            reset - chrono::Duration::hours(1),
            false,
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

    #[test]
    fn deferred_primary_reset_reports_the_reset_for_combined_activation_toast() {
        let previous_reset = Utc.with_ymd_and_hms(2026, 7, 14, 12, 0, 0).unwrap();
        let next_reset = Utc.with_ymd_and_hms(2026, 7, 14, 17, 0, 0).unwrap();
        let settings = NotificationSettings {
            limits_changed: true,
            ..Default::default()
        };
        let initial = RateLimits {
            primary: LimitWindow {
                used_percent: Some(80),
                resets_at: Some(previous_reset),
                ..Default::default()
            },
            ..Default::default()
        };
        let updated = RateLimits {
            primary: LimitWindow {
                used_percent: Some(0),
                resets_at: Some(next_reset),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut tracker = LimitNotificationTracker::default();

        tracker.observe_at(
            &initial,
            &settings,
            ProviderId::primary(crate::settings::ProviderKind::Codex),
            previous_reset,
            false,
        );
        let result = tracker.observe_at(
            &updated,
            &settings,
            ProviderId::primary(crate::settings::ProviderKind::Codex),
            previous_reset,
            true,
        );

        assert_eq!(
            result,
            LimitNotificationResult {
                primary_reset: true
            }
        );
    }

    #[test]
    fn exhausted_weekly_does_not_report_a_five_hour_reset() {
        let previous_reset = Utc.with_ymd_and_hms(2026, 7, 14, 12, 0, 0).unwrap();
        let next_reset = Utc.with_ymd_and_hms(2026, 7, 14, 17, 0, 0).unwrap();
        let weekly = LimitWindow {
            used_percent: Some(100),
            resets_at: Some(Utc.with_ymd_and_hms(2026, 7, 21, 12, 0, 0).unwrap()),
            duration_minutes: Some(10_080),
        };
        let settings = NotificationSettings {
            limits_changed: true,
            ..Default::default()
        };
        let initial = RateLimits {
            primary: LimitWindow {
                used_percent: Some(80),
                resets_at: Some(previous_reset),
                ..Default::default()
            },
            secondary: weekly.clone(),
            ..Default::default()
        };
        let updated = RateLimits {
            primary: LimitWindow {
                used_percent: Some(0),
                resets_at: Some(next_reset),
                ..Default::default()
            },
            secondary: weekly,
            ..Default::default()
        };
        let mut tracker = LimitNotificationTracker::default();

        tracker.observe_at(
            &initial,
            &settings,
            ProviderId::primary(crate::settings::ProviderKind::Codex),
            previous_reset,
            false,
        );
        let result = tracker.observe_at(
            &updated,
            &settings,
            ProviderId::primary(crate::settings::ProviderKind::Codex),
            previous_reset,
            false,
        );

        assert_eq!(result, LimitNotificationResult::default());
    }
}

#[cfg(windows)]
mod windows_impl {
    use std::path::PathBuf;

    use anyhow::{Context, Result};
    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT},
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
            RegCreateKeyExW, RegSetValueExW,
        },
        UI::Shell::{
            QUNS_ACCEPTS_NOTIFICATIONS, QUNS_QUIET_TIME, SHQueryUserNotificationState,
            SetCurrentProcessExplicitAppUserModelID,
        },
    };

    use super::AUMID;

    pub(super) fn initialize() -> Result<()> {
        register_aumid().context("register AUMID")?;
        let aumid: Vec<u16> = AUMID.encode_utf16().chain(std::iter::once(0)).collect();
        let status = unsafe { SetCurrentProcessExplicitAppUserModelID(aumid.as_ptr()) };
        anyhow::ensure!(
            status == 0,
            "SetCurrentProcessExplicitAppUserModelID: 0x{status:08X}"
        );
        Ok(())
    }

    /// False while presenting or running a full-screen game or app: the card
    /// still shows, silently.
    pub(super) fn accepts_notifications() -> bool {
        let mut state = 0;
        let status = unsafe { SHQueryUserNotificationState(&mut state) };
        status != 0 || state == QUNS_ACCEPTS_NOTIFICATIONS || state == QUNS_QUIET_TIME
    }

    /// Plays a WAV image asynchronously; a newer sound replaces the current one.
    pub(super) fn play(wav: &'static [u8]) {
        unsafe {
            PlaySoundW(
                wav.as_ptr().cast(),
                std::ptr::null_mut(),
                SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
            );
        }
    }

    fn register_aumid() -> Result<()> {
        let key = format!(r"Software\Classes\AppUserModelId\{AUMID}");
        set_reg_sz(&key, "DisplayName", "Codex Minibar")?;
        if let Some(icon) = icon_path() {
            // Shell IconUri wants a normal Windows path with backslashes.
            set_reg_sz(&key, "IconUri", &icon.to_string_lossy().replace('/', "\\"))?;
        }
        Ok(())
    }

    fn icon_path() -> Option<PathBuf> {
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
