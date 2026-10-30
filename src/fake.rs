//! `MINIBAR_FAKE_PROVIDERS=1` turns a build into a local sandbox for testing
//! automatic resets: settings live in a separate temp folder, Codex and
//! Claude are canned accounts, and no request reaches either provider.
//! Codex's banked reset expires 62 minutes after launch and Claude's two
//! minutes later (`MINIBAR_FAKE_RESET_EXPIRES_IN` overrides Codex's minutes).
//! Redeeming a fake credit only flips local state.

use std::{
    path::PathBuf,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Result, bail};
use chrono::{DateTime, Duration, Utc};

use crate::banked_reset::Outcome;
use crate::limits::{LimitWindow, RateLimitResetCredit, RateLimitResetCreditsSummary, RateLimits};
use crate::settings::{AutoResetLead, ProviderKind, Settings};
use crate::usage::UsageStatistics;

static CODEX_SPENT: AtomicBool = AtomicBool::new(false);
static CLAUDE_SPENT: AtomicBool = AtomicBool::new(false);

pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var_os("MINIBAR_FAKE_PROVIDERS").is_some_and(|value| value != "0")
    })
}

/// Whether `driver` is served by the sandbox instead of its real client.
pub fn serves(driver: ProviderKind) -> bool {
    enabled() && matches!(driver, ProviderKind::Codex | ProviderKind::Claude)
}

pub fn settings_path() -> PathBuf {
    std::env::temp_dir()
        .join("codex-minibar-fake")
        .join("settings.toml")
}

/// Minutes after launch at which Codex's credit expires; 62 by default.
fn expires_in_minutes() -> i64 {
    std::env::var("MINIBAR_FAKE_RESET_EXPIRES_IN")
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(62)
}

fn launched_at() -> DateTime<Utc> {
    static AT: OnceLock<DateTime<Utc>> = OnceLock::new();
    *AT.get_or_init(Utc::now)
}

fn spent(driver: ProviderKind) -> &'static AtomicBool {
    if driver == ProviderKind::Claude {
        &CLAUDE_SPENT
    } else {
        &CODEX_SPENT
    }
}

/// Fresh sandbox settings on every launch: Codex and Claude, automatic resets
/// on, applied one hour before expiry.
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
        let served = serves(instance.driver);
        instance.enabled = served;
        instance.auto_reset = served;
        instance.usage_stats = false;
    }
    let path = settings_path();
    let _ = std::fs::remove_dir_all(path.parent().unwrap_or(&path));
    settings.save(&path)
}

pub fn limits(driver: ProviderKind) -> RateLimits {
    let now = Utc::now();
    let spent = spent(driver).load(Ordering::Acquire);
    let claude = driver == ProviderKind::Claude;
    let window = |used: u8, resets_in: Duration, minutes: u32| LimitWindow {
        used_percent: Some(if spent { 0 } else { used }),
        resets_at: Some(now + resets_in),
        duration_minutes: Some(minutes),
    };
    let expires_in = expires_in_minutes() + if claude { 2 } else { 0 };
    RateLimits {
        primary: window(if claude { 78 } else { 63 }, Duration::minutes(137), 300),
        secondary: window(if claude { 55 } else { 41 }, Duration::days(3), 7 * 24 * 60),
        sampled_at: now,
        plan_type: (!claude).then(|| "plus".into()),
        account_name: claude.then(|| "Sandbox".into()),
        reset_credits: Some(RateLimitResetCreditsSummary {
            next_credit_id: (claude && !spent).then(|| "sandbox-grant".into()),
            available_count: u32::from(!spent),
            credits: (!spent)
                .then(|| RateLimitResetCredit {
                    reset_type: Some(
                        if claude {
                            "five_hour,seven_day"
                        } else {
                            "full"
                        }
                        .into(),
                    ),
                    status: "available".into(),
                    granted_at: Some(launched_at() - Duration::days(6)),
                    expires_at: Some(launched_at() + Duration::minutes(expires_in)),
                    title: Some("Banked reset".into()),
                    description: claude.then(|| "sandbox-grant".into()),
                })
                .into_iter()
                .collect(),
        }),
        ..RateLimits::default()
    }
}

pub fn consume(driver: ProviderKind) -> Result<(Outcome, Result<RateLimits>)> {
    if !serves(driver) {
        bail!("Provider does not support reset credits");
    }
    let outcome = if spent(driver).swap(true, Ordering::AcqRel) {
        Outcome::NoCredit
    } else {
        Outcome::Reset
    };
    crate::logger::info(format!(
        "Fake {} reset redeemed: {outcome:?}",
        driver.display_name()
    ));
    Ok((outcome, Ok(limits(driver))))
}

pub struct FakeProvider(pub ProviderKind);

impl crate::worker::LimitProvider for FakeProvider {
    fn read_limits(&mut self) -> Result<RateLimits> {
        Ok(limits(self.0))
    }
}

impl crate::worker::UsageProvider for FakeProvider {
    fn load_cached_usage_statistics(&mut self, _: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }
    fn refresh_usage_statistics(&mut self, _: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }
}

impl crate::worker::Activator for FakeProvider {
    fn activate(&mut self) -> Result<()> {
        Ok(())
    }
}
