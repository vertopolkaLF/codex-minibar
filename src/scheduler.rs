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
        // Activate when the 5h window still has 99%+ remaining (nearly unused).
        if primary.remaining_percent().unwrap_or_default() < 99 {
            return Decision::AlreadyActivated;
        }
        if let Some(last_attempt) = self.last_attempt_at {
            if now - last_attempt < Duration::hours(1) {
                return Decision::AlreadyActivated;
            }
        }
        Decision::ActivateNow
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

    fn fresh_window() -> LimitWindow {
        LimitWindow {
            used_percent: Some(1),
            resets_at: Some(at(15, 0)),
            duration_minutes: Some(300),
        }
    }

    #[test]
    fn activates_when_remaining_is_at_least_99_and_no_recent_attempt() {
        assert_eq!(
            ActivationState::default().decide(&fresh_window(), at(10, 0)),
            Decision::ActivateNow
        );
        let window = LimitWindow {
            used_percent: Some(0),
            ..fresh_window()
        };
        assert_eq!(
            ActivationState::default().decide(&window, at(10, 0)),
            Decision::ActivateNow
        );
    }

    #[test]
    fn skips_when_remaining_is_below_99() {
        let window = LimitWindow {
            used_percent: Some(2),
            ..fresh_window()
        };
        assert_eq!(
            ActivationState::default().decide(&window, at(10, 0)),
            Decision::AlreadyActivated
        );
        assert_eq!(
            ActivationState::default().decide(&LimitWindow::default(), at(10, 0)),
            Decision::AlreadyActivated
        );
    }

    #[test]
    fn skips_when_activation_happened_within_the_past_hour() {
        let window = fresh_window();
        let mut state = ActivationState::default();
        state.record_attempt(&window, at(10, 0));
        assert_eq!(
            state.decide(&window, at(10, 59)),
            Decision::AlreadyActivated
        );
        assert_eq!(state.decide(&window, at(11, 0)), Decision::ActivateNow);
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
