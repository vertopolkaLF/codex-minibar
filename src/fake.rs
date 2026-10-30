//! `MINIBAR_FAKE_CODEX=1` turns a build into a local sandbox for testing
//! automatic resets: settings live in a separate temp folder, Codex is a
//! canned account with one banked reset expiring 62 minutes after launch
//! (`MINIBAR_FAKE_CODEX_EXPIRES_IN` overrides the minutes), and
//! no request reaches OpenAI. Redeeming the fake credit only flips local state.

use std::{
    path::PathBuf,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};

use crate::banked_reset::Outcome;
use crate::limits::{LimitWindow, RateLimitResetCredit, RateLimitResetCreditsSummary, RateLimits};
use crate::settings::{AutoResetLead, ProviderKind, Settings};
use crate::usage::UsageStatistics;

static SPENT: AtomicBool = AtomicBool::new(false);

pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED
        .get_or_init(|| std::env::var_os("MINIBAR_FAKE_CODEX").is_some_and(|value| value != "0"))
}

pub fn settings_path() -> PathBuf {
    std::env::temp_dir()
        .join("codex-minibar-fake")
        .join("settings.toml")
}

/// `MINIBAR_FAKE_CODEX_EXPIRES_IN` minutes after launch; 62 by default.
fn expires_in_minutes() -> i64 {
    std::env::var("MINIBAR_FAKE_CODEX_EXPIRES_IN")
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(62)
}

fn launched_at() -> DateTime<Utc> {
    static AT: OnceLock<DateTime<Utc>> = OnceLock::new();
    *AT.get_or_init(Utc::now)
}

/// Fresh sandbox settings on every launch: only Codex, automatic resets on,
/// applied one hour before expiry.
pub fn seed_settings() -> Result<()> {
    launched_at();
    let mut settings = Settings {
        onboarding_completed: true,
        start_at_login: false,
        check_for_updates: false,
        auto_reset_lead: AutoResetLead::Hour1,
        ..Settings::default()
    };
    for instance in &mut settings.instances {
        let codex = instance.driver == ProviderKind::Codex;
        instance.enabled = codex;
        instance.auto_reset = codex;
        instance.usage_stats = false;
    }
    let path = settings_path();
    let _ = std::fs::remove_dir_all(path.parent().unwrap_or(&path));
    settings.save(&path)
}

pub fn limits() -> RateLimits {
    let now = Utc::now();
    let spent = SPENT.load(Ordering::Acquire);
    let window = |used: u8, resets_in: Duration, minutes: u32| LimitWindow {
        used_percent: Some(if spent { 0 } else { used }),
        resets_at: Some(now + resets_in),
        duration_minutes: Some(minutes),
    };
    RateLimits {
        primary: window(63, Duration::minutes(137), 300),
        secondary: window(41, Duration::days(3), 7 * 24 * 60),
        sampled_at: now,
        plan_type: Some("plus".into()),
        reset_credits: Some(RateLimitResetCreditsSummary {
            next_credit_id: None,
            available_count: u32::from(!spent),
            credits: (!spent)
                .then(|| RateLimitResetCredit {
                    reset_type: Some("full".into()),
                    status: "available".into(),
                    granted_at: Some(launched_at() - Duration::days(6)),
                    expires_at: Some(launched_at() + Duration::minutes(expires_in_minutes())),
                    title: Some("Banked reset".into()),
                    description: None,
                })
                .into_iter()
                .collect(),
        }),
        ..RateLimits::default()
    }
}

pub fn consume() -> (Outcome, Result<RateLimits>) {
    let outcome = if SPENT.swap(true, Ordering::AcqRel) {
        Outcome::NoCredit
    } else {
        Outcome::Reset
    };
    crate::logger::info(format!("Fake Codex reset redeemed: {outcome:?}"));
    (outcome, Ok(limits()))
}

pub struct FakeCodex;

impl crate::worker::LimitProvider for FakeCodex {
    fn read_limits(&mut self) -> Result<RateLimits> {
        Ok(limits())
    }
}

impl crate::worker::UsageProvider for FakeCodex {
    fn load_cached_usage_statistics(&mut self, _: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }
    fn refresh_usage_statistics(&mut self, _: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }
}

impl crate::worker::Activator for FakeCodex {
    fn activate(&mut self) -> Result<()> {
        Ok(())
    }
}
