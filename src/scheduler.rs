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

/// Codex reset probes must be close enough to describe one continuous API
/// observation series rather than unrelated polls hours apart.
const UNACTIVATED_PROBE_MAX_GAP: Duration = Duration::minutes(5);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct UnactivatedResetObservation {
    sampled_at: DateTime<Utc>,
    resets_at: DateTime<Utc>,
}

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
    /// Up to two preceding Codex reset samples used to distinguish a fixed,
    /// active deadline from the moving `request time + 5h` placeholder.
    #[serde(default)]
    unactivated_reset_observations: Vec<UnactivatedResetObservation>,
    /// A reset observed unchanged in two neighboring Codex responses. While
    /// this stays fixed, the 5-hour window is known to be active.
    #[serde(default)]
    stable_unactivated_candidate_reset: Option<DateTime<Utc>>,
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
            let occurrence = next_scheduled_activation_on(rule, weekday, now) - Duration::days(7);
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
    /// first normal sample only establishes a baseline. A Codex response that
    /// resembles its synthetic 5h placeholder is confirmed separately across
    /// two or three neighboring samples.
    pub fn decide(&self, primary: &LimitWindow) -> Decision {
        self.decide_with_unactivated(primary, false, Utc::now())
    }

    /// Variant of [`Self::decide`] for a provider that identified a possible
    /// Codex synthetic 5h response. Two identical reset timestamps confirm an
    /// active window; activation requires three timestamps that each move
    /// forward inside one five-minute observation series.
    pub fn decide_with_unactivated(
        &self,
        primary: &LimitWindow,
        is_unactivated_candidate: bool,
        sampled_at: DateTime<Utc>,
    ) -> Decision {
        // An empty primary window means the provider does not currently expose
        // a session limit. It is not evidence of an available-but-idle window.
        if primary.is_empty() {
            return Decision::Skip;
        }
        if is_unactivated_candidate {
            if self.attempted_without_active_window {
                return Decision::Skip;
            }
            let Some(resets_at) = primary.resets_at else {
                return Decision::Skip;
            };
            if self.stable_unactivated_candidate_reset == Some(resets_at) {
                return Decision::Skip;
            }
            // A freshly reset real session also looks like `sampled_at + 5h`
            // during its first minutes. Preserve the normal reset-transition
            // behavior: a full-window jump from a previously established
            // deadline activates immediately. A synthetic probe already in
            // progress is excluded because its moving timestamps are not an
            // established active-window baseline.
            if self.unactivated_reset_observations.is_empty()
                && self.last_seen_resets_at.is_some_and(|previous| {
                    is_new_window(normalize_reset(previous), normalize_reset(resets_at))
                })
            {
                return Decision::ActivateNow;
            }
            let observations = &self.unactivated_reset_observations;
            if observations.len() == 2 {
                let current = UnactivatedResetObservation {
                    sampled_at,
                    resets_at,
                };
                if unactivated_reset_series_is_moving(&observations[0], &observations[1], &current)
                {
                    return Decision::ActivateNow;
                }
            }
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
        self.observe_with_unactivated(primary, false, Utc::now());
    }

    /// Variant of [`Self::observe`] that tracks neighboring Codex reset samples
    /// until the deadline is either stable twice or moving three times.
    pub fn observe_with_unactivated(
        &mut self,
        primary: &LimitWindow,
        is_unactivated_candidate: bool,
        sampled_at: DateTime<Utc>,
    ) {
        if !is_unactivated_candidate {
            self.unactivated_reset_observations.clear();
            self.stable_unactivated_candidate_reset = None;
        }
        if let Some(resets_at) = primary.resets_at {
            self.last_seen_resets_at = Some(normalize_reset(resets_at));
            if is_unactivated_candidate {
                if self.stable_unactivated_candidate_reset == Some(resets_at) {
                    self.unactivated_reset_observations.clear();
                    self.attempted_without_active_window = false;
                    return;
                }

                if self
                    .unactivated_reset_observations
                    .last()
                    .is_some_and(|previous| previous.resets_at == resets_at)
                {
                    self.stable_unactivated_candidate_reset = Some(resets_at);
                    self.unactivated_reset_observations.clear();
                    self.attempted_without_active_window = false;
                    return;
                }

                self.stable_unactivated_candidate_reset = None;
                let current = UnactivatedResetObservation {
                    sampled_at,
                    resets_at,
                };
                if self
                    .unactivated_reset_observations
                    .last()
                    .is_some_and(|previous| !unactivated_reset_moved(previous, &current))
                {
                    self.unactivated_reset_observations.clear();
                }
                self.unactivated_reset_observations.push(current);
                if self.unactivated_reset_observations.len() > 2 {
                    self.unactivated_reset_observations.remove(0);
                }
            } else {
                self.attempted_without_active_window = false;
            }
        }
    }

    /// The worker uses a short follow-up poll while a Codex reset series still
    /// needs a second or third neighboring sample.
    pub fn awaits_unactivated_confirmation(&self) -> bool {
        !self.attempted_without_active_window
            && self.stable_unactivated_candidate_reset.is_none()
            && !self.unactivated_reset_observations.is_empty()
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

fn unactivated_reset_moved(
    previous: &UnactivatedResetObservation,
    current: &UnactivatedResetObservation,
) -> bool {
    let sample_gap = current.sampled_at - previous.sampled_at;
    let reset_shift = current.resets_at - previous.resets_at;
    sample_gap > Duration::zero()
        && sample_gap <= UNACTIVATED_PROBE_MAX_GAP
        && reset_shift > Duration::zero()
        && reset_shift <= UNACTIVATED_PROBE_MAX_GAP
}

fn unactivated_reset_series_is_moving(
    first: &UnactivatedResetObservation,
    second: &UnactivatedResetObservation,
    third: &UnactivatedResetObservation,
) -> bool {
    unactivated_reset_moved(first, second)
        && unactivated_reset_moved(second, third)
        && third.sampled_at - first.sampled_at <= UNACTIVATED_PROBE_MAX_GAP
        && third.resets_at - first.resets_at <= UNACTIVATED_PROBE_MAX_GAP
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
    fn first_codex_candidate_only_starts_confirmation() {
        let sampled_at = at(15, 0);
        let primary = LimitWindow {
            used_percent: Some(0),
            resets_at: Some(sampled_at + Duration::hours(5)),
            duration_minutes: Some(300),
        };
        let mut state = ActivationState::default();

        assert_eq!(
            state.decide_with_unactivated(&primary, true, sampled_at),
            Decision::Skip
        );
        state.observe_with_unactivated(&primary, true, sampled_at);
        assert!(state.awaits_unactivated_confirmation());
    }

    #[test]
    fn two_equal_codex_resets_confirm_active_window() {
        let first_sample = at(15, 0);
        let second_sample = first_sample + Duration::seconds(30);
        let fixed_reset = first_sample + Duration::hours(5);
        let primary = window_at(fixed_reset);
        let mut state = ActivationState::default();

        state.observe_with_unactivated(&primary, true, first_sample);
        assert_eq!(
            state.decide_with_unactivated(&primary, true, second_sample),
            Decision::Skip
        );
        state.observe_with_unactivated(&primary, true, second_sample);

        assert!(!state.awaits_unactivated_confirmation());
        assert_eq!(state.stable_unactivated_candidate_reset, Some(fixed_reset));
    }

    #[test]
    fn three_moving_codex_resets_trigger_activation() {
        let first_sample = at(15, 0);
        let second_sample = first_sample + Duration::seconds(30);
        let third_sample = second_sample + Duration::seconds(30);
        let mut state = ActivationState::default();

        state.observe_with_unactivated(
            &window_at(first_sample + Duration::hours(5)),
            true,
            first_sample,
        );
        state.observe_with_unactivated(
            &window_at(second_sample + Duration::hours(5)),
            true,
            second_sample,
        );

        assert_eq!(
            state.decide_with_unactivated(
                &window_at(third_sample + Duration::hours(5)),
                true,
                third_sample,
            ),
            Decision::ActivateNow
        );
    }

    #[test]
    fn fresh_reset_codex_candidate_still_triggers_activation() {
        let previous_reset = at(15, 0);
        let sampled_at = previous_reset + Duration::minutes(1);
        let fresh_reset = sampled_at + Duration::hours(5);
        let mut state = ActivationState::default();
        state.observe(&window_at(previous_reset));

        assert_eq!(
            state.decide_with_unactivated(&window_at(fresh_reset), true, sampled_at),
            Decision::ActivateNow
        );
    }

    #[test]
    fn codex_reset_series_longer_than_five_minutes_does_not_activate() {
        let first_sample = at(15, 0);
        let second_sample = first_sample + Duration::minutes(3);
        let third_sample = second_sample + Duration::minutes(3);
        let mut state = ActivationState::default();

        state.observe_with_unactivated(
            &window_at(first_sample + Duration::hours(5)),
            true,
            first_sample,
        );
        state.observe_with_unactivated(
            &window_at(second_sample + Duration::hours(5)),
            true,
            second_sample,
        );

        assert_eq!(
            state.decide_with_unactivated(
                &window_at(third_sample + Duration::hours(5)),
                true,
                third_sample,
            ),
            Decision::Skip
        );
    }

    #[test]
    fn weekly_placeholder_does_not_trigger_activation() {
        let sampled_at = at(15, 0);
        let primary = LimitWindow {
            used_percent: Some(0),
            resets_at: Some(sampled_at + Duration::days(7)),
            duration_minutes: Some(10_080),
        };
        let state = ActivationState::default();

        assert_eq!(
            state.decide_with_unactivated(&primary, false, sampled_at),
            Decision::Skip
        );
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

    #[test]
    fn codex_confirmation_series_survives_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("activation.toml");
        let sampled_at = at(15, 0);
        let primary = window_at(sampled_at + Duration::hours(5));
        let mut state = ActivationState::default();
        state.observe_with_unactivated(&primary, true, sampled_at);

        state.save(&path).unwrap();
        let restored = ActivationState::load_or_default(&path).unwrap();

        assert_eq!(restored, state);
        assert!(restored.awaits_unactivated_confirmation());
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
        assert_eq!(
            occurrence
                .with_timezone(&Local)
                .weekday()
                .num_days_from_monday() as u8,
            today
        );
    }
}
