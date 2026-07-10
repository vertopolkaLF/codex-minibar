use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::limits::LimitWindow;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationState {
    /// Reset timestamp identifying the last window for which activation was attempted.
    pub last_activated_reset: Option<DateTime<Utc>>,
    /// Prevents missing-reset responses from causing an immediate request loop.
    pub missing_reset_activated_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    ActivateNow,
    WaitUntil(DateTime<Utc>),
    AlreadyActivated,
}

impl ActivationState {
    pub fn decide(&self, primary: &LimitWindow, now: DateTime<Utc>) -> Decision {
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
        if let Some(reset) = primary.resets_at {
            self.last_activated_reset = Some(reset);
            self.missing_reset_activated_at = None;
        } else {
            self.missing_reset_activated_at = Some(now);
        }
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
        };
        let mut state = ActivationState::default();
        state.record_attempt(&window, at(10, 1));
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
}
