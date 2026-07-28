use std::{fs, path::Path};

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Timelike, Utc};
use serde::{Deserialize, Serialize};

use crate::limits::LimitWindow;

/// A real 5-hour window advance moves the deadline by hours. Smaller changes
/// are API timestamp jitter, including a value rounding across a minute.
const NEW_WINDOW_MINIMUM_ADVANCE: Duration = Duration::minutes(5);

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationState {
    /// Last observed primary `resets_at`, normalized to a whole minute.
    ///
    /// Claude's endpoint can vary seconds and fractional seconds of the same
    /// deadline on consecutive reads. That is not a new window and must never
    /// retrigger an activation.
    #[serde(default, alias = "last_activated_reset")]
    pub last_seen_resets_at: Option<DateTime<Utc>>,
    /// Last command attempt, successful or not. Surfaced in the UI.
    #[serde(default)]
    pub last_attempt_at: Option<DateTime<Utc>>,
    /// Prevent repeated activation attempts while a provider still reports no
    /// active 5h window after one attempt.
    #[serde(default)]
    pub attempted_without_active_window: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    ActivateNow,
    Skip,
}

impl ActivationState {
    /// Activate only when the primary reset timestamp changed since the last
    /// observation. Seconds and fractional-second jitter are ignored; the
    /// first sample only establishes a baseline.
    pub fn decide(&self, primary: &LimitWindow) -> Decision {
        let Some(resets_at) = primary.resets_at else {
            return if self.attempted_without_active_window {
                Decision::Skip
            } else {
                Decision::ActivateNow
            };
        };
        let resets_at = normalize_reset(resets_at);
        match self.last_seen_resets_at {
            Some(previous) if is_new_window(normalize_reset(previous), resets_at) => {
                Decision::ActivateNow
            }
            _ => Decision::Skip,
        }
    }

    /// Remember the latest primary reset time so the next poll can detect a
    /// real new window rather than endpoint timestamp jitter.
    pub fn observe(&mut self, primary: &LimitWindow) {
        if let Some(resets_at) = primary.resets_at {
            self.last_seen_resets_at = Some(normalize_reset(resets_at));
            self.attempted_without_active_window = false;
        }
    }

    pub fn record_attempt(&mut self, now: DateTime<Utc>) {
        self.last_attempt_at = Some(now);
        self.attempted_without_active_window = true;
    }

    pub fn load_or_default(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        toml::from_str(&raw).context("parse activation state")
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .context("activation state path has no parent")?;
        fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        use std::io::Write;
        temporary.write_all(toml::to_string_pretty(self)?.as_bytes())?;
        temporary.as_file().sync_all()?;
        temporary.persist(path)?;
        Ok(())
    }
}

fn normalize_reset(reset: DateTime<Utc>) -> DateTime<Utc> {
    reset
        .with_second(0)
        .and_then(|value| value.with_nanosecond(0))
        .unwrap_or(reset)
}

fn is_new_window(previous: DateTime<Utc>, current: DateTime<Utc>) -> bool {
    current - previous >= NEW_WINDOW_MINIMUM_ADVANCE
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(hour: u32, minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 10, hour, minute, 0).unwrap()
    }

    fn window_at(reset: DateTime<Utc>) -> LimitWindow {
        LimitWindow {
            used_percent: Some(0),
            resets_at: Some(reset),
            duration_minutes: Some(300),
        }
    }

    #[test]
    fn first_observation_only_baselines() {
        let state = ActivationState::default();
        assert_eq!(state.decide(&window_at(at(15, 0))), Decision::Skip);
    }

    #[test]
    fn activates_when_reset_moves_to_a_new_window() {
        let mut state = ActivationState::default();
        state.observe(&window_at(at(15, 0)));
        assert_eq!(state.decide(&window_at(at(15, 5))), Decision::ActivateNow);
    }

    #[test]
    fn skips_sub_minute_reset_jitter() {
        let mut state = ActivationState::default();
        let initial = Utc
            .with_ymd_and_hms(2026, 7, 10, 15, 0, 42)
            .unwrap()
            .with_nanosecond(824_588_000)
            .unwrap();
        let jittered = Utc
            .with_ymd_and_hms(2026, 7, 10, 15, 0, 59)
            .unwrap()
            .with_nanosecond(965_796_000)
            .unwrap();
        state.observe(&window_at(initial));
        assert_eq!(state.decide(&window_at(jittered)), Decision::Skip);
    }

    #[test]
    fn skips_a_one_minute_rounding_boundary() {
        let mut state = ActivationState::default();
        state.observe(&window_at(at(15, 0)));
        assert_eq!(state.decide(&window_at(at(15, 1))), Decision::Skip);
    }

    #[test]
    fn skips_when_resets_at_is_unchanged() {
        let mut state = ActivationState::default();
        state.observe(&window_at(at(15, 0)));
        assert_eq!(state.decide(&window_at(at(15, 0))), Decision::Skip);
    }

    #[test]
    fn activates_once_when_the_session_is_not_activated() {
        let mut state = ActivationState {
            last_seen_resets_at: Some(at(15, 0)),
            ..ActivationState::default()
        };
        assert_eq!(state.decide(&LimitWindow::default()), Decision::ActivateNow);
        state.record_attempt(at(10, 0));
        assert_eq!(state.decide(&LimitWindow::default()), Decision::Skip);
        state.observe(&LimitWindow::default());
        assert_eq!(state.last_seen_resets_at, Some(at(15, 0)));
    }

    #[test]
    fn loads_legacy_last_activated_reset_as_baseline() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("activation.toml");
        fs::write(
            &path,
            "last_activated_reset = \"2026-07-10T15:00:00Z\"\nlast_attempt_at = \"2026-07-10T10:00:00Z\"\n",
        )
        .unwrap();
        let state = ActivationState::load_or_default(&path).unwrap();
        assert_eq!(state.last_seen_resets_at, Some(at(15, 0)));
        assert_eq!(state.last_attempt_at, Some(at(10, 0)));
    }

    #[test]
    fn state_survives_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("activation.toml");
        let mut state = ActivationState::default();
        state.observe(&window_at(at(15, 0)));
        state.record_attempt(at(10, 0));
        state.save(&path).unwrap();
        assert_eq!(ActivationState::load_or_default(&path).unwrap(), state);
    }
}
