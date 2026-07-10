use std::{fs, path::Path};

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::limits::LimitWindow;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationState {
    /// Reset timestamp identifying the last window for which activation was attempted.
    pub last_activated_reset: Option<DateTime<Utc>>,
    /// Prevents missing-reset responses from causing an immediate request loop.
    pub missing_reset_activated_at: Option<DateTime<Utc>>,
    /// Last command attempt, successful or not. Used for bounded retry backoff.
    pub last_attempt_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    ActivateNow,
    WaitUntil(DateTime<Utc>),
    AlreadyActivated,
}

impl ActivationState {
    pub fn decide(&self, primary: &LimitWindow, now: DateTime<Utc>) -> Decision {
        if let Some(last_attempt) = self.last_attempt_at {
            if now - last_attempt < Duration::minutes(5) {
                return Decision::AlreadyActivated;
            }
            // Some Codex versions may report a full window with a reset timestamp that
            // moves with every request. Do not treat each moving timestamp as a new window.
            if primary.used_percent.unwrap_or_default() >= 99
                && now - last_attempt < Duration::hours(4)
            {
                return Decision::AlreadyActivated;
            }
        }
        match primary.resets_at {
            Some(reset) if self.last_activated_reset == Some(reset) => Decision::AlreadyActivated,
            Some(reset) => {
                let run_at = reset + Duration::minutes(1);
                if now >= run_at {
                    Decision::ActivateNow
                } else {
                    Decision::WaitUntil(run_at)
                }
            }
            None => match self.missing_reset_activated_at {
                // Missing data is retried only after a bounded cool-down, never every poll.
                Some(at) if now - at < Duration::minutes(15) => Decision::AlreadyActivated,
                _ => Decision::ActivateNow,
            },
        }
    }

    pub fn record_attempt(&mut self, primary: &LimitWindow, now: DateTime<Utc>) {
        self.last_attempt_at = Some(now);
        if primary.resets_at.is_none() {
            self.missing_reset_activated_at = Some(now);
        }
    }

    pub fn record_success(&mut self, primary: &LimitWindow, now: DateTime<Utc>) {
        if let Some(reset) = primary.resets_at {
            self.last_activated_reset = Some(reset);
            self.missing_reset_activated_at = None;
        } else {
            self.missing_reset_activated_at = Some(now);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(hour: u32, minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 10, hour, minute, 0).unwrap()
    }

    #[test]
    fn waits_until_one_minute_after_reset() {
        let window = LimitWindow {
            used_percent: Some(100),
            resets_at: Some(at(10, 0)),
            duration_minutes: Some(300),
        };
        assert_eq!(
            ActivationState::default().decide(&window, at(9, 0)),
            Decision::WaitUntil(at(10, 1))
        );
        assert_eq!(
            ActivationState::default().decide(&window, at(10, 1)),
            Decision::ActivateNow
        );
    }

    #[test]
    fn never_repeats_same_observed_window() {
        let window = LimitWindow {
            used_percent: Some(99),
            resets_at: Some(at(10, 0)),
            duration_minutes: Some(300),
        };
        let mut state = ActivationState::default();
        state.record_attempt(&window, at(10, 1));
        state.record_success(&window, at(10, 1));
        assert_eq!(state.decide(&window, at(11, 0)), Decision::AlreadyActivated);
    }

    #[test]
    fn missing_reset_is_immediate_but_cooled_down() {
        let window = LimitWindow::default();
        let mut state = ActivationState::default();
        assert_eq!(state.decide(&window, at(10, 0)), Decision::ActivateNow);
        state.record_attempt(&window, at(10, 0));
        assert_eq!(state.decide(&window, at(10, 1)), Decision::AlreadyActivated);
        assert_eq!(state.decide(&window, at(10, 15)), Decision::ActivateNow);
    }

    #[test]
    fn moving_reset_at_full_usage_does_not_repeat() {
        let first = LimitWindow {
            used_percent: Some(100),
            resets_at: Some(at(10, 0)),
            duration_minutes: Some(300),
        };
        let moving = LimitWindow {
            used_percent: Some(99),
            resets_at: Some(at(10, 2)),
            duration_minutes: Some(300),
        };
        let mut state = ActivationState::default();
        state.record_attempt(&first, at(10, 1));
        assert_eq!(
            state.decide(&moving, at(10, 10)),
            Decision::AlreadyActivated
        );
    }

    #[test]
    fn state_survives_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("activation.toml");
        let mut state = ActivationState::default();
        state.record_attempt(&LimitWindow::default(), at(10, 0));
        state.save(&path).unwrap();
        assert_eq!(ActivationState::load_or_default(&path).unwrap(), state);
    }
}
