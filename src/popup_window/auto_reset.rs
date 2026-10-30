//! Automatic resets: spends a banked reset credit shortly before it expires
//! for instances that opted in, so an unused credit is never lost. Checks run
//! on the bridge thread; redemption uses the same account-scoped path as the
//! popup's Use button and runs off-thread.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};

use super::state::{AppState, refresh_all_workers};
use crate::banked_reset::{AutoResetChoice, Outcome};
use crate::instances::{Capabilities, ProviderId, ProviderInstance};
use crate::limits::{ProviderLimits, RateLimits};
use crate::settings::{AutoResetLead, Settings};
use crate::worker::WorkerEvent;

/// The bridge ticks every 16–250 ms; quota snapshots change far less often.
const CHECK_INTERVAL: Duration = Duration::from_secs(5);

/// A credit that could not be spent yet (nothing to reset, a failed request)
/// is retried at this cadence until it expires.
const RETRY_INTERVAL: Duration = Duration::from_secs(10 * 60);

/// How long before an automatic reset the user is warned about it.
const WARNING_LEAD: chrono::Duration = chrono::Duration::minutes(10);

/// The nearest-expiring credit and when it will be spent, provided spending
/// it can clear something. An untouched quota has nothing to reset.
fn schedule(limits: &RateLimits, lead: AutoResetLead) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    if limits.available_reset_count() == 0 {
        return None;
    }
    let expires_at = limits.next_reset_credit_expiration()?;
    let used = [&limits.primary, &limits.secondary]
        .into_iter()
        .any(|window| window.used_percent.is_some_and(|percent| percent > 0));
    used.then(|| {
        (
            expires_at,
            expires_at - chrono::Duration::minutes(lead.minutes()),
        )
    })
}

/// Whether the credit is inside the lead window and should be spent now.
pub(super) fn due(limits: &RateLimits, lead: AutoResetLead, now: DateTime<Utc>) -> bool {
    schedule(limits, lead).is_some_and(|(expires_at, at)| at <= now && now < expires_at)
}

/// The planned spend time when it is close enough to warn about, and the
/// credit expiry identifying that one warning.
fn upcoming(
    limits: &RateLimits,
    lead: AutoResetLead,
    now: DateTime<Utc>,
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    schedule(limits, lead).filter(|(_, at)| now < *at && *at - now <= WARNING_LEAD)
}

/// `MINIBAR_AUTO_RESET_DRY_RUN=1` runs the whole schedule against live
/// quotas but never redeems: it logs and toasts what would have happened.
fn dry_run_requested() -> bool {
    std::env::var_os("MINIBAR_AUTO_RESET_DRY_RUN").is_some_and(|value| value != "0")
}

pub(super) struct AutoReset {
    /// Snapshots sampled before this are cached from an earlier run. They may
    /// list a credit that is already spent, so they never trigger anything.
    started_at: DateTime<Utc>,
    next_check: Instant,
    last_attempt: HashMap<ProviderId, Instant>,
    running: Arc<Mutex<HashSet<ProviderId>>>,
    dry_run: bool,
    /// Dry run only: the last logged planned time per provider.
    planned: HashMap<ProviderId, Option<DateTime<Utc>>>,
    /// Credit expiry each provider was already warned about.
    warned: HashMap<ProviderId, DateTime<Utc>>,
    /// Credit expiry the user chose not to spend automatically.
    skipped: HashMap<ProviderId, DateTime<Utc>>,
}

impl AutoReset {
    pub(super) fn new() -> Self {
        let dry_run = dry_run_requested();
        if dry_run {
            crate::logger::info("Automatic resets: dry run, no credit will be redeemed");
        }
        Self {
            started_at: crate::banked_reset::process_started_at(),
            next_check: Instant::now(),
            last_attempt: HashMap::new(),
            running: Arc::default(),
            dry_run,
            planned: HashMap::new(),
            warned: HashMap::new(),
            skipped: HashMap::new(),
        }
    }

    /// Providers to warn about an automatic reset, with minutes until it
    /// happens. Each credit is announced once.
    fn take_warnings(
        &mut self,
        settings: &Settings,
        limits: &ProviderLimits,
        utc_now: DateTime<Utc>,
    ) -> Vec<(ProviderId, i64)> {
        let mut warnings = Vec::new();
        for instance in settings.instances.iter().filter(|instance| {
            instance.enabled && instance.auto_reset && Capabilities::of(instance).auto_reset
        }) {
            let provider = instance.provider_id();
            let limits = limits.get(provider);
            if limits.sampled_at < self.started_at {
                continue;
            }
            let Some((expires_at, at)) = upcoming(limits, settings.auto_reset_lead, utc_now) else {
                continue;
            };
            if self.warned.insert(provider, expires_at) != Some(expires_at) {
                let seconds = (at - utc_now).num_seconds().max(1);
                warnings.push((provider, (seconds + 59) / 60));
            }
        }
        warnings
    }

    /// Instances whose credit should be spent now. Marks them attempted and
    /// running; the attempt must clear `running` when it ends.
    fn take_due(
        &mut self,
        settings: &Settings,
        limits: &ProviderLimits,
        now: Instant,
        utc_now: DateTime<Utc>,
    ) -> Vec<ProviderInstance> {
        let candidates = settings
            .instances
            .iter()
            .filter(|instance| {
                instance.enabled && instance.auto_reset && Capabilities::of(instance).auto_reset
            })
            .collect::<Vec<_>>();
        self.last_attempt
            .retain(|provider, _| candidates.iter().any(|i| i.provider_id() == *provider));
        let mut due_now = Vec::new();
        for instance in candidates {
            let provider = instance.provider_id();
            let limits = limits.get(provider);
            if self.dry_run {
                self.log_plan(provider, limits, settings.auto_reset_lead);
            }
            if limits.sampled_at < self.started_at
                || !due(limits, settings.auto_reset_lead, utc_now)
                || self.skipped.get(&provider).copied() == limits.next_reset_credit_expiration()
                || self
                    .last_attempt
                    .get(&provider)
                    .is_some_and(|at| now.duration_since(*at) < RETRY_INTERVAL)
                || !self.running.lock().unwrap().insert(provider)
            {
                continue;
            }
            self.last_attempt.insert(provider, now);
            due_now.push(instance.clone());
        }
        due_now
    }

    /// Logs the planned redemption time whenever it changes.
    fn log_plan(&mut self, provider: ProviderId, limits: &RateLimits, lead: AutoResetLead) {
        let at = (limits.available_reset_count() > 0)
            .then(|| limits.next_reset_credit_expiration())
            .flatten()
            .map(|expires_at| expires_at - chrono::Duration::minutes(lead.minutes()));
        if self.planned.insert(provider, at) == Some(at) {
            return;
        }
        let name = provider.qualified_name();
        crate::logger::info(match at {
            Some(at) => format!(
                "{name} automatic reset planned at {} (dry run)",
                at.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M")
            ),
            None => format!("{name} automatic reset: no expiring credit (dry run)"),
        });
    }

    /// Applies the answers given on the warning notification.
    fn apply_choices(&mut self, settings: &Settings, state: &Arc<AppState>) {
        for choice in crate::banked_reset::take_choices() {
            match choice {
                AutoResetChoice::Skip(provider) => {
                    let limits = state.current_limits();
                    if let Some(expires_at) = limits.get(provider).next_reset_credit_expiration() {
                        crate::logger::info(format!(
                            "{} automatic reset skipped for the credit expiring at {expires_at}",
                            provider.qualified_name()
                        ));
                        self.skipped.insert(provider, expires_at);
                    }
                }
                AutoResetChoice::ApplyNow(provider) => {
                    let Some(instance) = settings.instance(provider).filter(|instance| {
                        instance.enabled && Capabilities::of(instance).auto_reset
                    }) else {
                        continue;
                    };
                    if !self.running.lock().unwrap().insert(provider) {
                        continue;
                    }
                    self.last_attempt.insert(provider, Instant::now());
                    self.spawn_attempt(instance.clone(), state, true);
                }
            }
        }
    }

    pub(super) fn poll(&mut self, settings: &Settings, state: &Arc<AppState>) {
        self.apply_choices(settings, state);
        let now = Instant::now();
        if now < self.next_check {
            return;
        }
        self.next_check = now + CHECK_INTERVAL;
        if !settings
            .instances
            .iter()
            .any(|instance| instance.auto_reset)
        {
            self.last_attempt.clear();
            return;
        }
        let limits = state.current_limits();
        let utc_now = Utc::now();
        let warnings = self.take_warnings(settings, &limits, utc_now);
        if settings.notifications.auto_reset_upcoming {
            for (provider, minutes) in warnings {
                use crate::notifications::{
                    LimitFocus, Notification, NotificationAction, NotificationKind,
                };
                crate::logger::info(format!(
                    "{} automatic reset in {minutes} min",
                    provider.qualified_name()
                ));
                // The quotas show what the reset is about to clear.
                crate::notifications::notify(
                    Notification::limit(
                        NotificationKind::Warning,
                        &crate::i18n::format(
                            "banked-reset-applies-in-minutes",
                            &[("minutes", minutes.to_string())],
                        ),
                        &provider.qualified_name(),
                        provider,
                        limits.get(provider),
                        LimitFocus::Primary,
                    )
                    .with_action(
                        crate::i18n::tr("apply-reset-now"),
                        NotificationAction::AutoReset(AutoResetChoice::ApplyNow(provider)),
                    )
                    .with_action(
                        crate::i18n::tr("skip-automatic-reset"),
                        NotificationAction::AutoReset(AutoResetChoice::Skip(provider)),
                    ),
                );
            }
        }
        for instance in self.take_due(settings, &limits, now, utc_now) {
            self.spawn_attempt(instance, state, false);
        }
    }

    /// Redeems off-thread. `manual` attempts come from "Apply now" and report
    /// every outcome; scheduled ones stay quiet unless the reset applied.
    fn spawn_attempt(&self, instance: ProviderInstance, state: &Arc<AppState>, manual: bool) {
        use crate::notifications::{NotificationKind, show_kind};
        let provider = instance.provider_id();
        let running = Arc::clone(&self.running);
        let state = Arc::clone(state);
        let dry_run = self.dry_run;
        let applied_title = crate::i18n::tr(if manual {
            "banked-reset-applied"
        } else {
            "banked-reset-applied-automatically"
        });
        thread::spawn(move || {
            let name = provider.qualified_name();
            if dry_run {
                crate::logger::info(format!("{name} reset would apply now (dry run)"));
                show_kind(
                    NotificationKind::Info,
                    &format!("[Dry run] {applied_title}"),
                    &name,
                );
                running.lock().unwrap().remove(&provider);
                return;
            }
            crate::logger::info(if manual {
                format!("{name} applying banked reset from the notification")
            } else {
                format!("{name} banked reset expires soon; applying it automatically")
            });
            let result = crate::banked_reset::consume(&instance);
            match &result {
                Ok((Outcome::Reset, _)) => {
                    crate::logger::info(format!("{name} banked reset applied"));
                    state.record_banked_reset_use(provider);
                    show_kind(NotificationKind::Reset, applied_title, &name);
                }
                Ok((outcome, _)) => {
                    let message = crate::i18n::tr(outcome.message_key());
                    crate::logger::info(format!("{name} banked reset not applied: {message}"));
                    if manual {
                        show_kind(NotificationKind::Warning, &name, message);
                    }
                }
                Err(error) => {
                    crate::logger::info(format!("{name} banked reset failed: {error}"));
                    if manual {
                        show_kind(NotificationKind::Error, &name, &error.to_string());
                    }
                }
            }
            // The worker revision check drops this snapshot if the
            // instance's login changed while the request ran.
            if let Ok((_, Ok(limits))) = result {
                let _ = state
                    .worker_events_tx
                    .send(WorkerEvent::ProviderLimitsUpdated(
                        provider,
                        instance.credentials_revision,
                        limits,
                    ));
            }
            // Sibling instances sharing the account see the spent credit.
            refresh_all_workers(&state.worker_commands());
            running.lock().unwrap().remove(&provider);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instances::InstanceSource;
    use crate::limits::{LimitWindow, RateLimitResetCredit, RateLimitResetCreditsSummary};
    use crate::settings::ProviderKind;

    fn limits(used: u8, expires_in: chrono::Duration, now: DateTime<Utc>) -> RateLimits {
        RateLimits {
            primary: LimitWindow {
                used_percent: Some(used),
                ..LimitWindow::default()
            },
            reset_credits: Some(RateLimitResetCreditsSummary {
                next_credit_id: None,
                available_count: 1,
                credits: vec![RateLimitResetCredit {
                    status: "available".into(),
                    expires_at: Some(now + expires_in),
                    ..RateLimitResetCredit::default()
                }],
            }),
            ..RateLimits::default()
        }
    }

    fn instance(driver: ProviderKind, enabled: bool, auto_reset: bool) -> ProviderInstance {
        ProviderInstance {
            enabled,
            auto_reset,
            ..ProviderInstance::primary(driver)
        }
    }

    fn settings_with(instance: ProviderInstance) -> Settings {
        Settings {
            instances: vec![instance],
            auto_reset_lead: AutoResetLead::Minutes30,
            ..Settings::default()
        }
    }

    fn scheduler() -> AutoReset {
        AutoReset {
            started_at: DateTime::<Utc>::MIN_UTC,
            next_check: Instant::now(),
            last_attempt: HashMap::new(),
            running: Arc::default(),
            dry_run: false,
            planned: HashMap::new(),
            warned: HashMap::new(),
            skipped: HashMap::new(),
        }
    }

    #[test]
    fn cached_snapshot_from_an_earlier_run_never_triggers() {
        let utc = Utc::now();
        let codex = instance(ProviderKind::Codex, true, true);
        let provider = codex.provider_id();
        let settings = settings_with(codex);
        let mut auto = AutoReset {
            started_at: utc,
            ..scheduler()
        };
        let mut cached = limits(40, chrono::Duration::minutes(5), utc);
        cached.sampled_at = utc - chrono::Duration::minutes(1);
        let stale = ProviderLimits::from_entries([(provider, cached.clone())]);
        let now = Instant::now();
        assert!(auto.take_due(&settings, &stale, now, utc).is_empty());
        assert!(auto.take_warnings(&settings, &stale, utc).is_empty());
        cached.sampled_at = utc;
        let live = ProviderLimits::from_entries([(provider, cached)]);
        assert_eq!(auto.take_due(&settings, &live, now, utc).len(), 1);
    }

    #[test]
    fn skipped_credit_is_not_spent_but_the_next_one_is() {
        let utc = Utc::now();
        let codex = instance(ProviderKind::Codex, true, true);
        let provider = codex.provider_id();
        let settings = settings_with(codex);
        let soon = limits(40, chrono::Duration::minutes(10), utc);
        let mut auto = scheduler();
        auto.skipped
            .insert(provider, soon.next_reset_credit_expiration().unwrap());
        let now = Instant::now();
        let skipped = ProviderLimits::from_entries([(provider, soon)]);
        assert!(auto.take_due(&settings, &skipped, now, utc).is_empty());
        let next = ProviderLimits::from_entries([(
            provider,
            limits(40, chrono::Duration::minutes(5), utc),
        )]);
        assert_eq!(auto.take_due(&settings, &next, now, utc).len(), 1);
    }

    #[test]
    fn warns_once_within_ten_minutes_of_the_planned_reset() {
        let utc = Utc::now();
        let codex = instance(ProviderKind::Codex, true, true);
        let provider = codex.provider_id();
        let settings = settings_with(codex);
        // Lead is 30 minutes, so the reset is planned 11 minutes from now.
        let at =
            |expires_in| ProviderLimits::from_entries([(provider, limits(40, expires_in, utc))]);
        let mut auto = scheduler();
        let early = at(chrono::Duration::minutes(42));
        assert!(auto.take_warnings(&settings, &early, utc).is_empty());
        let soon = at(chrono::Duration::minutes(39));
        assert_eq!(
            auto.take_warnings(&settings, &soon, utc),
            vec![(provider, 9)]
        );
        assert!(auto.take_warnings(&settings, &soon, utc).is_empty());
        // Already due: the reset itself is the notification.
        let mut fresh = scheduler();
        let due_now = at(chrono::Duration::minutes(20));
        assert!(fresh.take_warnings(&settings, &due_now, utc).is_empty());
    }

    #[test]
    fn due_only_inside_the_lead_window() {
        let now = Utc::now();
        let lead = AutoResetLead::Minutes30;
        assert!(due(
            &limits(40, chrono::Duration::minutes(20), now),
            lead,
            now
        ));
        assert!(!due(
            &limits(40, chrono::Duration::minutes(45), now),
            lead,
            now
        ));
        assert!(!due(
            &limits(40, chrono::Duration::minutes(-1), now),
            lead,
            now
        ));
        assert!(due(
            &limits(40, chrono::Duration::hours(20), now),
            AutoResetLead::Hours24,
            now
        ));
    }

    #[test]
    fn untouched_quota_or_no_credit_is_never_due() {
        let now = Utc::now();
        let lead = AutoResetLead::Hour1;
        assert!(!due(
            &limits(0, chrono::Duration::minutes(5), now),
            lead,
            now
        ));
        let mut spent = limits(40, chrono::Duration::minutes(5), now);
        spent.reset_credits.as_mut().unwrap().available_count = 0;
        assert!(!due(&spent, lead, now));
    }

    #[test]
    fn take_due_fires_once_then_waits_for_the_retry_interval() {
        let utc = Utc::now();
        let codex = instance(ProviderKind::Codex, true, true);
        let provider = codex.provider_id();
        let settings = settings_with(codex);
        let limits = ProviderLimits::from_entries([(
            provider,
            limits(40, chrono::Duration::minutes(10), utc),
        )]);
        let mut auto = scheduler();
        let start = Instant::now();

        assert_eq!(auto.take_due(&settings, &limits, start, utc).len(), 1);
        // Still running: never a second concurrent attempt.
        assert!(auto.take_due(&settings, &limits, start, utc).is_empty());
        auto.running.lock().unwrap().remove(&provider);
        // Finished, but still inside the retry interval.
        let soon = start + Duration::from_secs(60);
        assert!(auto.take_due(&settings, &limits, soon, utc).is_empty());
        let later = start + RETRY_INTERVAL;
        assert_eq!(auto.take_due(&settings, &limits, later, utc).len(), 1);
    }

    #[test]
    fn take_due_skips_instances_that_cannot_or_did_not_opt_in() {
        let utc = Utc::now();
        let soon = limits(40, chrono::Duration::minutes(10), utc);
        let now = Instant::now();
        let manual_claude = ProviderInstance {
            source: InstanceSource::Manual,
            ..instance(ProviderKind::Claude, true, true)
        };
        for instance in [
            instance(ProviderKind::Codex, true, false),
            instance(ProviderKind::Codex, false, true),
            instance(ProviderKind::Cursor, true, true),
            manual_claude,
        ] {
            let limits = ProviderLimits::from_entries([(instance.provider_id(), soon.clone())]);
            let settings = settings_with(instance);
            assert!(
                scheduler()
                    .take_due(&settings, &limits, now, utc)
                    .is_empty()
            );
        }
    }
}
