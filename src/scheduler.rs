use std::{fs, path::Path};

use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};

use crate::limits::LimitWindow;
use crate::settings::ScheduledActivation;

/// Auto activation must leave the upcoming planned window untouched.
pub const AUTO_ACTIVATION_SCHEDULE_GUARD: Duration = Duration::hours(6);

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
    /// The exact weekly occurrence already fired for each schedule rule.
    #[serde(default)]
    pub fired_scheduled_occurrences: std::collections::HashMap<String, DateTime<Utc>>,
}

/// Returns the next local-time occurrence on one selected weekday after `now`.
fn next_scheduled_activation_on(
    rule: &ScheduledActivation,
    weekday: u8,
    now: DateTime<Utc>,
) -> DateTime<Utc> {
    let local_now = now.with_timezone(&Local);
    let today = local_now.date_naive();
    let days = (u32::from(weekday) + 7 - today.weekday().num_days_from_monday()) % 7;
    let date = today + Duration::days(i64::from(days));
    let hour = u32::from(rule.time_minutes / 60);
    let minute = u32::from(rule.time_minutes % 60);
    let candidate = Local
        .with_ymd_and_hms(date.year(), date.month(), date.day(), hour, minute, 0)
        .single()
        .or_else(|| {
            Local
                .with_ymd_and_hms(date.year(), date.month(), date.day(), hour, minute, 0)
                .earliest()
        })
        .unwrap_or(local_now)
        .with_timezone(&Utc);
    if candidate > now {
        candidate
    } else {
        candidate + Duration::days(7)
    }
}

/// Returns the earliest next occurrence among all selected weekdays.
pub fn next_scheduled_activation(rule: &ScheduledActivation, now: DateTime<Utc>) -> DateTime<Utc> {
    rule.weekdays
        .iter()
        .copied()
        .map(|weekday| next_scheduled_activation_on(rule, weekday, now))
        .min()
        .unwrap_or_else(|| next_scheduled_activation_on(rule, rule.weekday, now))
}

/// Finds a rule due since the last poll. A six-hour grace period fires a
/// planned activation after wake/resume without replaying stale calendar work.
pub fn due_scheduled_activation<'a>(
    rules: &'a [ScheduledActivation],
    state: &ActivationState,
    now: DateTime<Utc>,
) -> Option<(&'a ScheduledActivation, DateTime<Utc>)> {
    rules
        .iter()
        .filter(|rule| rule.enabled)
        .flat_map(|rule| {
            rule.weekdays
                .iter()
                .copied()
                .map(move |weekday| (rule, weekday))
        })
        .filter_map(|(rule, weekday)| {
            let occurrence =
                next_scheduled_activation_on(rule, weekday, now) - Duration::days(7);
            (now - occurrence <= AUTO_ACTIVATION_SCHEDULE_GUARD
                && state.fired_scheduled_occurrences.get(&rule.id) != Some(&occurrence))
                .then_some((rule, occurrence))
        })
        .min_by_key(|(_, occurrence)| *occurrence)
}

pub fn scheduled_activation_within(
    rules: &[ScheduledActivation],
    now: DateTime<Utc>,
    within: Duration,
) -> bool {
    rules
        .iter()
        .filter(|rule| rule.enabled)
        .any(|rule| next_scheduled_activation(rule, now) - now <= within)
}

impl ActivationState {
    pub fn record_scheduled_activation(&mut self, rule_id: &str, occurrence: DateTime<Utc>) {
        self.fired_scheduled_occurrences
            .insert(rule_id.into(), occurrence);
    }
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
        // An empty primary window means the provider does not currently expose
        // a session limit. It is not evidence of an available-but-idle window.
        if primary.is_empty() {
            return Decision::Skip;
        }
        let Some(resets_at) = primary.resets_at else {
            return if self.attempted_without_active_window {
                Decision::Skip
            } else {
                Decision::ActivateNow
            };
        };
        // A reset deadline can appear one poll after a successful activation.
        // Treat that as confirmation of the attempt, not as another new
        // window. `observe` will store the deadline and clear the guard.
        if self.attempted_without_active_window {
            return Decision::Skip;
        }
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
    fn reset_appearing_after_activation_attempt_is_confirmation() {
        let mut state = ActivationState::default();
        state.observe(&window_at(at(15, 0)));
        state.record_attempt(at(15, 1));

        assert_eq!(state.decide(&window_at(at(20, 0))), Decision::Skip);
        state.observe(&window_at(at(20, 0)));
        assert_eq!(state.last_seen_resets_at, Some(at(20, 0)));
        assert!(!state.attempted_without_active_window);
    }

    #[test]
    fn never_activates_when_the_session_window_is_absent() {
        let mut state = ActivationState {
            last_seen_resets_at: Some(at(15, 0)),
            ..ActivationState::default()
        };
        assert_eq!(state.decide(&LimitWindow::default()), Decision::Skip);
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

    fn schedule_at_local(when: DateTime<Local>) -> ScheduledActivation {
        ScheduledActivation {
            id: "test-rule".into(),
            provider_id: "codex".into(),
            weekday: when.weekday().num_days_from_monday() as u8,
            weekdays: vec![when.weekday().num_days_from_monday() as u8],
            time_minutes: (when.hour() * 60 + when.minute()) as u16,
            enabled: true,
        }
    }

    #[test]
    fn scheduled_occurrence_is_due_once_after_its_local_time() {
        let now = Utc::now();
        let rule = schedule_at_local((now - Duration::minutes(1)).with_timezone(&Local));
        let mut state = ActivationState::default();
        let rules = [rule.clone()];
        let (due_rule, occurrence) = due_scheduled_activation(&rules, &state, now).unwrap();
        assert_eq!(due_rule.id, rule.id);
        state.record_scheduled_activation(&rule.id, occurrence);
        assert!(due_scheduled_activation(&[rule], &state, now).is_none());
    }

    #[test]
    fn upcoming_schedule_blocks_automatic_activation_for_six_hours() {
        let now = Utc::now();
        let rule = schedule_at_local((now + Duration::hours(2)).with_timezone(&Local));
        assert!(scheduled_activation_within(
            &[rule],
            now,
            AUTO_ACTIVATION_SCHEDULE_GUARD
        ));
    }

    #[test]
    fn a_multi_day_rule_recognizes_the_latest_selected_day_as_due() {
        let now = Utc::now();
        let local_now = now.with_timezone(&Local);
        let today = local_now.weekday().num_days_from_monday() as u8;
        let tomorrow = (today + 1) % 7;
        let rule = ScheduledActivation {
            id: "weekdays".into(),
            provider_id: "claude".into(),
            weekday: today,
            weekdays: vec![today, tomorrow],
            time_minutes: (local_now.hour() * 60 + local_now.minute()) as u16,
            enabled: true,
        };

        let (_, occurrence) = due_scheduled_activation(&[rule], &ActivationState::default(), now)
            .expect("today's occurrence should be within the catch-up window");
        assert_eq!(occurrence.with_timezone(&Local).weekday().num_days_from_monday() as u8, today);
    }
}
