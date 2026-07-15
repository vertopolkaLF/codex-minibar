use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::usage::UsageStatistics;
use crate::settings::ProviderKind;

/// Windows longer than this are treated as weekly (or similar), not the 5h session.
const SHORT_WINDOW_MAX_MINUTES: u32 = 12 * 60;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LimitWindow {
    pub used_percent: Option<u8>,
    pub resets_at: Option<DateTime<Utc>>,
    pub duration_minutes: Option<u32>,
}

/// A named quota window supplied in addition to the standard session and
/// weekly limits. Claude adds model- and feature-specific windows over time,
/// so these must remain data-driven rather than being discarded by a fixed
/// two-window model.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AdditionalLimit {
    /// Stable field name from the provider response, used as the UI identity.
    pub id: String,
    /// Human-readable title, such as "Fable" or "Opus".
    pub title: String,
    pub window: LimitWindow,
}

/// Pace tip on a usage progress bar (even-burn marker position).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaceTip {
    /// Marker position on the bar in the same units as the fill (used or remaining).
    pub percent: f64,
    /// Actual used percentage minus the expected even-burn percentage.
    pub delta_percent: f64,
}

impl PaceTip {
    /// Compact CodexBar-style description for the usage-card header.
    pub fn summary(self) -> String {
        const ON_PACE_TOLERANCE: f64 = 2.0;
        if self.delta_percent.abs() <= ON_PACE_TOLERANCE {
            return "On pace".into();
        }

        let delta = self.delta_percent.abs().round() as u32;
        if self.delta_percent > 0.0 {
            format!("{delta}% in deficit")
        } else {
            format!("{delta}% in reserve")
        }
    }
}

impl LimitWindow {
    pub fn remaining_percent(&self) -> Option<u8> {
        self.used_percent
            .map(|used| 100u8.saturating_sub(used.min(100)))
    }

    pub fn is_empty(&self) -> bool {
        self.used_percent.is_none() && self.resets_at.is_none()
    }

    /// True when this window is clearly longer than a 5-hour session.
    pub fn looks_like_weekly(&self, now: DateTime<Utc>) -> bool {
        if let Some(minutes) = self.duration_minutes {
            return minutes > SHORT_WINDOW_MAX_MINUTES;
        }
        self.resets_at
            .map(|reset| (reset - now).num_minutes() > i64::from(SHORT_WINDOW_MAX_MINUTES))
            .unwrap_or(false)
    }

    /// Expected used % for an even burn across the current window.
    ///
    /// With 1h left in a 5h window this is 80%; when the bar shows remaining,
    /// the tip is mirrored to 20%.
    pub fn expected_used_percent(&self, now: DateTime<Utc>) -> Option<f64> {
        let duration_minutes = self.duration_minutes.filter(|&minutes| minutes > 0)?;
        let resets_at = self.resets_at?;
        let duration_secs = f64::from(duration_minutes) * 60.0;
        let time_until_reset = (resets_at - now).num_milliseconds() as f64 / 1000.0;
        if time_until_reset <= 0.0 || time_until_reset > duration_secs {
            return None;
        }
        let elapsed = (duration_secs - time_until_reset).clamp(0.0, duration_secs);
        Some(((elapsed / duration_secs) * 100.0).clamp(0.0, 100.0))
    }

    /// Progress-bar pace tip for session and weekly bars.
    pub fn pace_tip(&self, show_used: bool, now: DateTime<Utc>) -> Option<PaceTip> {
        let expected_used = self.expected_used_percent(now)?;
        // Hide until ~3% of the window has elapsed (too noisy at the start).
        if expected_used < 3.0 {
            return None;
        }
        // Need a real usage sample; otherwise the bar itself is empty/unavailable.
        let actual_used = f64::from(self.used_percent?);
        let percent = if show_used {
            expected_used
        } else {
            100.0 - expected_used
        };
        Some(PaceTip {
            percent: percent.clamp(0.0, 100.0),
            delta_percent: actual_used - expected_used,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RateLimits {
    pub primary: LimitWindow,
    pub secondary: LimitWindow,
    pub sampled_at: DateTime<Utc>,
    pub plan_type: Option<String>,
    pub limit_name: Option<String>,
    pub credits: Credits,
    pub reset_credits: Option<RateLimitResetCreditsSummary>,
    /// Provider-specific quota windows beyond primary and secondary.
    pub additional_limits: Vec<AdditionalLimit>,
    /// Token statistics computed from local Codex session logs.
    pub usage: UsageStatistics,
}

/// Independent snapshots for every enabled provider. Provider data is never
/// merged: a Claude weekly limit must not overwrite Codex's five-hour window.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProviderLimits {
    pub codex: RateLimits,
    pub claude: RateLimits,
}

impl ProviderLimits {
    pub fn get(&self, provider: ProviderKind) -> &RateLimits {
        match provider {
            ProviderKind::Codex => &self.codex,
            ProviderKind::Claude => &self.claude,
        }
    }

    pub fn get_mut(&mut self, provider: ProviderKind) -> &mut RateLimits {
        match provider {
            ProviderKind::Codex => &mut self.codex,
            ProviderKind::Claude => &mut self.claude,
        }
    }
}

impl RateLimits {
    /// OpenAI sometimes drops the 5h window and leaves weekly data in `primary`.
    /// Remap that so the UI/tray keep treating primary as the short session.
    pub fn normalized(mut self, now: DateTime<Utc>) -> Self {
        if self.secondary.is_empty()
            && !self.primary.is_empty()
            && self.primary.looks_like_weekly(now)
        {
            self.secondary = std::mem::take(&mut self.primary);
        }
        self
    }

    pub fn five_hour_disabled(&self) -> bool {
        self.primary.is_empty()
    }

    /// Free plans expose a single monthly window instead of a 5-hour session
    /// plus a weekly window.
    pub fn is_free_plan(&self) -> bool {
        self.plan_type
            .as_deref()
            .is_some_and(|plan| plan.trim().eq_ignore_ascii_case("free"))
    }

    /// Window used by tray widgets that target the 5h/primary source.
    pub fn effective_primary(&self) -> &LimitWindow {
        if self.five_hour_disabled() {
            &self.secondary
        } else {
            &self.primary
        }
    }

    pub fn available_reset_count(&self) -> u32 {
        self.reset_credits
            .as_ref()
            .map(|summary| summary.available_count)
            .unwrap_or(0)
    }

    pub fn next_reset_credit_expiration(&self) -> Option<DateTime<Utc>> {
        self.reset_credits
            .as_ref()?
            .credits
            .iter()
            .filter(|credit| credit.status.eq_ignore_ascii_case("available"))
            .filter_map(|credit| credit.expires_at)
            .min()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Credits {
    pub has_credits: bool,
    pub unlimited: bool,
    pub balance: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RateLimitResetCreditsSummary {
    pub available_count: u32,
    pub credits: Vec<RateLimitResetCredit>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RateLimitResetCredit {
    pub reset_type: Option<String>,
    pub status: String,
    pub granted_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub title: Option<String>,
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn window_with_hour_left(used: u8) -> (LimitWindow, DateTime<Utc>) {
        let now = Utc.with_ymd_and_hms(2026, 7, 12, 15, 0, 0).unwrap();
        let window = LimitWindow {
            used_percent: Some(used),
            resets_at: Some(now + chrono::Duration::hours(1)),
            duration_minutes: Some(300),
        };
        (window, now)
    }

    #[test]
    fn one_hour_left_of_five_is_twenty_percent_remaining_pace() {
        let (window, now) = window_with_hour_left(40);
        assert!((window.expected_used_percent(now).unwrap() - 80.0).abs() < 0.01);
        let tip = window.pace_tip(false, now).unwrap();
        assert!((tip.percent - 20.0).abs() < 0.01);
    }

    #[test]
    fn pace_tip_uses_expected_used_when_showing_used() {
        let (window, now) = window_with_hour_left(40);
        let tip = window.pace_tip(true, now).unwrap();
        assert!((tip.percent - 80.0).abs() < 0.01);
    }

    #[test]
    fn pace_tip_still_shown_when_on_track() {
        let (window, now) = window_with_hour_left(80);
        let tip = window.pace_tip(false, now).unwrap();
        assert!((tip.percent - 20.0).abs() < 0.01);
        assert_eq!(tip.summary(), "On pace");
    }

    #[test]
    fn pace_tip_summarizes_reserve_and_deficit_like_codexbar() {
        let (reserve, now) = window_with_hour_left(70);
        assert_eq!(
            reserve.pace_tip(false, now).unwrap().summary(),
            "10% in reserve"
        );

        let (deficit, now) = window_with_hour_left(92);
        assert_eq!(
            deficit.pace_tip(false, now).unwrap().summary(),
            "12% in deficit"
        );
    }

    #[test]
    fn weekly_bar_also_gets_pace_tip() {
        let now = Utc.with_ymd_and_hms(2026, 7, 12, 15, 0, 0).unwrap();
        let weekly = LimitWindow {
            used_percent: Some(30),
            resets_at: Some(now + chrono::Duration::days(3)),
            duration_minutes: Some(10_080),
        };
        let tip = weekly.pace_tip(false, now).unwrap();
        assert!((tip.percent - (100.0 - weekly.expected_used_percent(now).unwrap())).abs() < 0.01);
    }

    #[test]
    fn identifies_free_plan_case_insensitively() {
        let limits = RateLimits {
            plan_type: Some(" FREE ".into()),
            ..Default::default()
        };

        assert!(limits.is_free_plan());
        assert!(!RateLimits::default().is_free_plan());
    }
}
