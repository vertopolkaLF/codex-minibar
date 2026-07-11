use std::{fs, path::Path};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::limits::LimitWindow;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationState {
    /// Last observed primary `resets_at`. A change means the 5h window moved
    /// (real reset or the sliding "not yet activated" clock) and we should activate.
    #[serde(default, alias = "last_activated_reset")]
    pub last_seen_resets_at: Option<DateTime<Utc>>,
    /// Last command attempt, successful or not. Surfaced in the UI.
    #[serde(default)]
    pub last_attempt_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    ActivateNow,
    Skip,
}

impl ActivationState {
    /// Activate only when the primary reset timestamp changed since the last
    /// observation. The first sample only establishes a baseline.
    pub fn decide(&self, primary: &LimitWindow) -> Decision {
        let Some(resets_at) = primary.resets_at else {
            return Decision::Skip;
        };
        match self.last_seen_resets_at {
            Some(previous) if previous != resets_at => Decision::ActivateNow,
            _ => Decision::Skip,
        }
    }

    /// Remember the latest primary reset time so the next poll can detect drift.
    pub fn observe(&mut self, primary: &LimitWindow) {
        if let Some(resets_at) = primary.resets_at {
            self.last_seen_resets_at = Some(resets_at);
        }
    }

    pub fn record_attempt(&mut self, now: DateTime<Utc>) {
        self.last_attempt_at = Some(now);
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
    fn activates_when_resets_at_changes() {
        let mut state = ActivationState::default();
        state.observe(&window_at(at(15, 0)));
        assert_eq!(state.decide(&window_at(at(15, 1))), Decision::ActivateNow);
    }

    #[test]
    fn skips_when_resets_at_is_unchanged() {
        let mut state = ActivationState::default();
        state.observe(&window_at(at(15, 0)));
        assert_eq!(state.decide(&window_at(at(15, 0))), Decision::Skip);
    }

    #[test]
    fn skips_when_resets_at_is_missing() {
        let mut state = ActivationState {
            last_seen_resets_at: Some(at(15, 0)),
            ..ActivationState::default()
        };
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
