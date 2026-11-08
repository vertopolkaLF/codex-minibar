//! Detects whether an announced Codex reset actually reached an account.
//!
//! The public feed only says when a reset should land. Every Codex quota
//! sample is reduced to a small [`Observation`] and compared with the previous
//! one. The per-instance [`Track`] is persisted next to the feed cache, so the
//! comparison also spans app restarts and offline periods:
//!
//! * an *unscheduled reset* is a quota window that restarted or emptied while
//!   its previous deadline was still in the future;
//! * a *banked use* is a drop in available reset credits that expiry and new
//!   grants do not explain, or a redemption made from this app.
//!
//! A forced reset counts as received when an unscheduled reset that no banked
//! use explains happened inside its landing window, or when the account is
//! back at 100/100 after the deadline without a banked use since the window
//! opened. A banked announcement counts as received once a credit granted
//! inside its landing window shows up.
//!
//! Evaluation is a pure function of the announcements and the recorded
//! history, so an announcement that arrives late (for example through a stale
//! CDN copy of the feed) still matches a reset that was observed earlier.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::limits::{LimitWindow, RateLimits};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResetKind {
    /// Everyone's limits are reset.
    #[default]
    Forced,
    /// Everyone receives a banked reset credit to spend later.
    Banked,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResetTiming {
    /// Lands at `reset_at`.
    #[default]
    Exact,
    /// Lands at some point before `reset_at`, which is only a deadline.
    Before,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnnouncedReset {
    /// Stable bot-generated identity. It must not change when the description
    /// or the timestamp is corrected.
    pub id: String,
    #[serde(default)]
    pub kind: ResetKind,
    #[serde(default)]
    pub timing: ResetTiming,
    /// Optional HTTPS page where the announcement can be verified by the user.
    #[serde(default)]
    pub source_url: Option<String>,
    /// Optional human-readable context, for example "weekly quota".
    #[serde(default)]
    pub label: Option<String>,
    /// ISO-8601 timestamp in UTC. A deadline for [`ResetTiming::Before`].
    pub reset_at: DateTime<Utc>,
    /// Optional start of the landing window of a [`ResetTiming::Before`]
    /// announcement.
    #[serde(default)]
    pub not_before: Option<DateTime<Utc>>,
    /// Instance id -> when this client detected the reset on that account.
    #[serde(default)]
    pub received: BTreeMap<String, DateTime<Utc>>,
}

/// An exact announcement may land slightly ahead of the posted time.
const EXACT_EARLY_SLACK: Duration = Duration::minutes(15);
/// Landing window of a "before" announcement without an explicit start.
const BEFORE_DEFAULT_WINDOW: Duration = Duration::hours(12);
/// How long after its deadline another announcement may still be landing.
/// Only used to decide whether an observation is ambiguous.
const LATE_GRACE: Duration = Duration::hours(6);
/// A previous window must still have this long to run for its restart to be
/// unscheduled. Guards against clock skew around natural rollovers.
const STILL_RUNNING_SLACK: Duration = Duration::minutes(2);
/// Providers round reset deadlines; smaller moves are not a new window.
const WINDOW_MOVE: Duration = Duration::minutes(30);
/// A drop this large inside one window cannot be rounding noise.
const LARGE_DROP: u8 = 10;
/// Banked credit grant timestamps may precede the announcement slightly.
const GRANT_SLACK: Duration = Duration::hours(1);
/// Recorded history covers a full weekly window plus margin.
const HISTORY: Duration = Duration::days(8);
const MAX_SPANS: usize = 32;
/// Upper bound on a redemption round trip from the popup.
const IN_APP_REDEMPTION_SLACK: Duration = Duration::minutes(5);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowSample {
    pub used: u8,
    #[serde(default)]
    pub resets_at: Option<DateTime<Utc>>,
    /// Codex reports a not-yet-started session as a synthetic window.
    #[serde(default)]
    pub unactivated: bool,
}

impl WindowSample {
    fn from_window(window: &LimitWindow, unactivated: bool) -> Option<Self> {
        Some(Self {
            used: window.used_percent?.min(100),
            resets_at: window.resets_at,
            unactivated,
        })
    }

    fn is_full(&self) -> bool {
        self.used == 0 || self.unactivated
    }

    fn is_running(&self) -> bool {
        self.used > 0 && !self.unactivated
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditSample {
    pub available: bool,
    #[serde(default)]
    pub granted_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankedSample {
    pub available: u32,
    #[serde(default)]
    pub credits: Vec<CreditSample>,
}

/// The parts of one quota sample that reset detection needs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub sampled_at: DateTime<Utc>,
    /// Distinguishes a different login reported by the same instance.
    #[serde(default)]
    pub account: Option<String>,
    #[serde(default)]
    pub session: Option<WindowSample>,
    #[serde(default)]
    pub weekly: Option<WindowSample>,
    /// `None` when the credit request failed, which is not the same as zero.
    #[serde(default)]
    pub banked: Option<BankedSample>,
}

impl Observation {
    pub fn from_limits(limits: &RateLimits) -> Option<Self> {
        if limits.sampled_at.timestamp() <= 0 {
            return None;
        }
        let session =
            WindowSample::from_window(&limits.primary, limits.primary_window_is_unactivated);
        let weekly = WindowSample::from_window(&limits.secondary, false);
        if session.is_none() && weekly.is_none() {
            return None;
        }
        let banked = limits.reset_credits.as_ref().map(|summary| BankedSample {
            available: summary.available_count,
            credits: summary
                .credits
                .iter()
                .map(|credit| CreditSample {
                    available: credit.status.eq_ignore_ascii_case("available"),
                    granted_at: credit.granted_at,
                    expires_at: credit.expires_at,
                })
                .collect(),
        });
        Some(Self {
            sampled_at: limits.sampled_at,
            account: limits.account_name.clone(),
            session,
            weekly,
            banked,
        })
    }

    /// Every reported window is back at 100% remaining.
    pub fn is_full(&self) -> bool {
        let windows = [self.session.as_ref(), self.weekly.as_ref()];
        windows.iter().any(Option::is_some) && windows.iter().flatten().all(|w| w.is_full())
    }
}

/// Something happened after `after` and no later than `by`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub after: DateTime<Utc>,
    pub by: DateTime<Utc>,
}

impl Span {
    fn overlaps(&self, other: &Span) -> bool {
        self.after <= other.by && other.after <= self.by
    }
}

/// Persisted detection history of one provider instance.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    #[serde(default)]
    pub last: Option<Observation>,
    #[serde(default)]
    pub unscheduled_resets: Vec<Span>,
    #[serde(default)]
    pub banked_uses: Vec<Span>,
    #[serde(default)]
    pub banked_grants: Vec<Span>,
}

impl Track {
    /// Records a newer sample. Older or duplicate samples, such as cached
    /// snapshots replayed at startup, are ignored.
    pub fn observe(&mut self, observation: Observation) -> bool {
        if let Some(last) = &self.last {
            if observation.sampled_at <= last.sampled_at {
                return false;
            }
            // A name missing from one sample (for example an unreadable ID
            // token) is not evidence of another login.
            let other_login = last
                .account
                .as_ref()
                .zip(observation.account.as_ref())
                .is_some_and(|(last, current)| last != current);
            if !other_login {
                let span = Span {
                    after: last.sampled_at,
                    by: observation.sampled_at,
                };
                if unscheduled_reset(last, &observation) {
                    self.unscheduled_resets.push(span);
                }
                let (used, granted) = banked_delta(last, &observation);
                if used {
                    self.banked_uses.push(span);
                }
                if granted {
                    self.banked_grants.push(span);
                }
            } else {
                // Another login: its quota history says nothing about this one.
                *self = Self::default();
            }
        }
        let now = observation.sampled_at;
        self.last = Some(observation);
        self.prune(now);
        true
    }

    /// A reset credit spent from this app. The quota sample that follows
    /// shows the same drop, but recording it directly also covers samples
    /// whose credit request failed.
    pub fn record_banked_use(&mut self, at: DateTime<Utc>) {
        // The redemption request and the quota read that follows it finish
        // before `at`, so the span reaches back over both.
        self.banked_uses.push(Span {
            after: at - IN_APP_REDEMPTION_SLACK,
            by: at,
        });
        self.prune(at);
    }

    pub fn last_sampled_at(&self) -> Option<DateTime<Utc>> {
        self.last.as_ref().map(|last| last.sampled_at)
    }

    fn prune(&mut self, now: DateTime<Utc>) {
        for spans in [
            &mut self.unscheduled_resets,
            &mut self.banked_uses,
            &mut self.banked_grants,
        ] {
            spans.retain(|span| now - span.by <= HISTORY);
            if spans.len() > MAX_SPANS {
                let excess = spans.len() - MAX_SPANS;
                spans.drain(..excess);
            }
        }
    }
}

/// Start of the period in which `reset` may land.
pub fn earliest_landing(reset: &AnnouncedReset) -> DateTime<Utc> {
    let default = match reset.timing {
        ResetTiming::Exact => reset.reset_at - EXACT_EARLY_SLACK,
        ResetTiming::Before => reset.reset_at - BEFORE_DEFAULT_WINDOW,
    };
    reset
        .not_before
        .filter(|at| *at <= reset.reset_at)
        .unwrap_or(default)
}

/// When `track` proves that `reset` reached the account, if it does.
pub fn received_at(
    reset: &AnnouncedReset,
    track: &Track,
    announcements: &[AnnouncedReset],
) -> Option<DateTime<Utc>> {
    let last = track.last.as_ref()?;
    let earliest = earliest_landing(reset);
    let attributable = |span: &Span| {
        span.by >= earliest
            && (span.after >= earliest
                || !earlier_announcement_explains(span, reset, announcements))
    };
    match reset.kind {
        ResetKind::Forced => {
            let explained_by_banked =
                |span: &Span| track.banked_uses.iter().any(|banked| banked.overlaps(span));
            if let Some(span) = track
                .unscheduled_resets
                .iter()
                .find(|span| attributable(span) && !explained_by_banked(span))
            {
                return Some(span.by);
            }
            // Nothing visibly changed (for example an idle account), but the
            // deadline passed and the account has full limits that no banked
            // reset produced.
            let banked_since_window = track.banked_uses.iter().any(|span| span.by >= earliest);
            (last.sampled_at >= reset.reset_at && last.is_full() && !banked_since_window)
                .then_some(last.sampled_at)
        }
        ResetKind::Banked => {
            let granted = last
                .banked
                .iter()
                .flat_map(|banked| &banked.credits)
                .find_map(|credit| {
                    credit.granted_at.filter(|at| {
                        *at >= earliest - GRANT_SLACK && *at <= last.sampled_at + GRANT_SLACK
                    })
                });
            granted.or_else(|| {
                track
                    .banked_grants
                    .iter()
                    .find(|span| attributable(span))
                    .map(|span| span.by)
            })
        }
    }
}

/// Stamps every newly proven reception into the announcements. Receipts are
/// sticky: later usage must not make a received reset pending again.
pub fn apply_receipts(
    announcements: &mut [AnnouncedReset],
    tracks: &BTreeMap<String, Track>,
) -> bool {
    let mut found = Vec::new();
    for (index, reset) in announcements.iter().enumerate() {
        for (instance, track) in tracks {
            if reset.received.contains_key(instance) {
                continue;
            }
            if let Some(at) = received_at(reset, track, announcements) {
                found.push((index, instance.clone(), at));
            }
        }
    }
    let changed = !found.is_empty();
    for (index, instance, at) in found {
        announcements[index].received.insert(instance, at);
    }
    changed
}

/// The span starts before `reset` could land, and an earlier announcement of
/// the same kind was landing during that gap, so the change may be that one.
fn earlier_announcement_explains(
    span: &Span,
    reset: &AnnouncedReset,
    announcements: &[AnnouncedReset],
) -> bool {
    let earliest = earliest_landing(reset);
    announcements.iter().any(|other| {
        other.id != reset.id
            && other.kind == reset.kind
            && other.reset_at < reset.reset_at
            && Span {
                after: earliest_landing(other),
                by: other.reset_at + LATE_GRACE,
            }
            .overlaps(&Span {
                after: span.after,
                by: earliest,
            })
    })
}

fn unscheduled_reset(previous: &Observation, current: &Observation) -> bool {
    // A deadline must still be ahead of the new sample: a natural rollover
    // is expected and proves nothing.
    let still_running_until = current.sampled_at + STILL_RUNNING_SLACK;
    let restarted = |before: Option<&WindowSample>, after: Option<&WindowSample>| {
        let (Some(before), Some(after)) = (before, after) else {
            return false;
        };
        // An empty window has nothing to reset, and its deadline is synthetic.
        if !before.is_running() {
            return false;
        }
        let Some(deadline) = before.resets_at.filter(|at| *at > still_running_until) else {
            return false;
        };
        let moved = after
            .resets_at
            .is_some_and(|at| (at - deadline).abs() > WINDOW_MOVE);
        let dropped = before.used.saturating_sub(after.used) >= LARGE_DROP;
        moved || after.is_full() || dropped
    };
    restarted(previous.session.as_ref(), current.session.as_ref())
        || restarted(previous.weekly.as_ref(), current.weekly.as_ref())
}

/// `(used, granted)` reset credits between two samples.
fn banked_delta(previous: &Observation, current: &Observation) -> (bool, bool) {
    let (Some(before), Some(after)) = (&previous.banked, &current.banked) else {
        return (false, false);
    };
    let granted = after
        .credits
        .iter()
        .filter(|credit| credit.granted_at.is_some_and(|at| at > previous.sampled_at))
        .count() as i64;
    let expired = before
        .credits
        .iter()
        .filter(|credit| {
            credit.available && credit.expires_at.is_some_and(|at| at <= current.sampled_at)
        })
        .count() as i64;
    let used = i64::from(before.available) + granted - expired - i64::from(after.available);
    let grew = granted > 0 || after.available > before.available;
    (used > 0, grew)
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn at(day: u32, hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, day, hour, 0, 0).unwrap()
    }

    fn announced(
        id: &str,
        kind: ResetKind,
        timing: ResetTiming,
        reset_at: DateTime<Utc>,
    ) -> AnnouncedReset {
        AnnouncedReset {
            id: id.into(),
            kind,
            timing,
            source_url: None,
            label: None,
            reset_at,
            not_before: None,
            received: BTreeMap::new(),
        }
    }

    fn forced(reset_at: DateTime<Utc>) -> AnnouncedReset {
        announced("forced", ResetKind::Forced, ResetTiming::Exact, reset_at)
    }

    /// A running session that ends at 23:00 on the day of the sample.
    fn sample(
        sampled_at: DateTime<Utc>,
        session: u8,
        weekly: u8,
        weekly_end: DateTime<Utc>,
        banked: Option<u32>,
    ) -> Observation {
        Observation {
            sampled_at,
            account: Some("ada".into()),
            session: Some(WindowSample {
                used: session,
                resets_at: Some(
                    sampled_at
                        .date_naive()
                        .and_hms_opt(23, 0, 0)
                        .unwrap()
                        .and_utc(),
                ),
                unactivated: false,
            }),
            weekly: Some(WindowSample {
                used: weekly,
                resets_at: Some(weekly_end),
                unactivated: false,
            }),
            banked: banked.map(|available| BankedSample {
                available,
                credits: Vec::new(),
            }),
        }
    }

    fn track(samples: impl IntoIterator<Item = Observation>) -> Track {
        let mut track = Track::default();
        for sample in samples {
            track.observe(sample);
        }
        track
    }

    fn alone(reset: &AnnouncedReset) -> &[AnnouncedReset] {
        std::slice::from_ref(reset)
    }

    #[test]
    fn exact_reset_is_received_when_limits_return_to_full() {
        let reset = forced(at(23, 17));
        let track = track([
            sample(at(23, 16), 40, 60, at(27, 12), Some(0)),
            sample(at(23, 17) + Duration::minutes(3), 0, 0, at(30, 17), Some(0)),
        ]);

        assert_eq!(
            received_at(&reset, &track, alone(&reset)),
            Some(at(23, 17) + Duration::minutes(3))
        );
    }

    #[test]
    fn reset_is_received_even_when_quota_was_used_again_before_the_next_sample() {
        // The app was closed: the weekly window restarted, but usage since
        // then hides the drop.
        let reset = forced(at(23, 17));
        let track = track([
            sample(at(23, 9), 10, 30, at(27, 12), None),
            sample(at(24, 9), 20, 45, at(30, 18), None),
        ]);

        assert_eq!(received_at(&reset, &track, alone(&reset)), Some(at(24, 9)));
    }

    #[test]
    fn natural_rollovers_and_ordinary_usage_are_not_resets() {
        let reset = forced(at(23, 17));
        let rollover = track([
            sample(at(23, 9), 10, 90, at(23, 10), None),
            sample(at(23, 18), 5, 1, at(30, 10), None),
        ]);
        let usage = track([
            sample(at(23, 16), 10, 30, at(27, 12), None),
            sample(at(23, 18), 25, 34, at(27, 12), None),
        ]);

        for track in [rollover, usage] {
            assert_eq!(received_at(&reset, &track, alone(&reset)), None);
        }
    }

    #[test]
    fn a_banked_reset_spent_in_the_same_interval_does_not_count() {
        let reset = forced(at(23, 17));
        let counted = track([
            sample(at(23, 16), 40, 60, at(27, 12), Some(2)),
            sample(at(23, 18), 0, 0, at(30, 18), Some(1)),
        ]);
        let mut in_app = track([sample(at(23, 16), 40, 60, at(27, 12), None)]);
        in_app.record_banked_use(at(23, 17));
        in_app.observe(sample(at(23, 18), 0, 0, at(30, 18), None));

        for track in [counted, in_app] {
            assert_eq!(received_at(&reset, &track, alone(&reset)), None);
        }
    }

    #[test]
    fn idle_full_account_counts_only_after_the_deadline() {
        let reset = announced("forced", ResetKind::Forced, ResetTiming::Before, at(23, 17));
        let before = track([sample(at(23, 12), 0, 0, at(27, 12), None)]);
        let after = track([
            sample(at(23, 12), 0, 0, at(27, 12), None),
            sample(at(23, 17) + Duration::minutes(1), 0, 0, at(27, 12), None),
        ]);

        assert_eq!(received_at(&reset, &before, alone(&reset)), None);
        assert_eq!(
            received_at(&reset, &after, alone(&reset)),
            Some(at(23, 17) + Duration::minutes(1))
        );
    }

    #[test]
    fn before_reset_may_land_hours_ahead_of_its_deadline() {
        let reset = announced("forced", ResetKind::Forced, ResetTiming::Before, at(23, 17));
        let exact = forced(at(23, 17));
        let track = track([
            sample(at(23, 8), 40, 60, at(27, 12), None),
            sample(at(23, 9), 0, 0, at(30, 9), None),
        ]);

        assert_eq!(received_at(&reset, &track, alone(&reset)), Some(at(23, 9)));
        assert_eq!(received_at(&exact, &track, alone(&exact)), None);
    }

    #[test]
    fn a_long_gap_spanning_an_earlier_announcement_is_ambiguous() {
        let day17 = announced("day-17", ResetKind::Forced, ResetTiming::Exact, at(22, 17));
        let day18 = announced("day-18", ResetKind::Forced, ResetTiming::Exact, at(23, 17));
        let all = [day17.clone(), day18.clone()];
        let track = track([
            sample(at(21, 9), 30, 60, at(27, 12), None),
            sample(at(23, 18), 20, 30, at(29, 18), None),
        ]);

        assert_eq!(received_at(&day17, &track, &all), Some(at(23, 18)));
        assert_eq!(received_at(&day18, &track, &all), None);
        // Without the earlier announcement the restart can only be day 18.
        assert_eq!(received_at(&day18, &track, alone(&day18)), Some(at(23, 18)));
    }

    #[test]
    fn stale_and_replayed_samples_are_ignored_and_other_logins_restart_history() {
        let mut track = track([sample(at(23, 16), 40, 60, at(27, 12), None)]);
        assert!(!track.observe(sample(at(23, 15), 0, 0, at(30, 15), None)));
        // On the same login this drop would be a reset.
        let mut other = sample(at(23, 18), 5, 5, at(30, 18), None);
        other.account = Some("grace".into());
        assert!(track.observe(other));
        let reset = forced(at(23, 17));
        assert_eq!(received_at(&reset, &track, alone(&reset)), None);
    }

    #[test]
    fn banked_announcement_is_received_with_a_matching_grant() {
        let reset = announced("banked", ResetKind::Banked, ResetTiming::Before, at(23, 17));
        let mut granted = sample(at(23, 18), 40, 60, at(27, 12), Some(1));
        granted.banked.as_mut().unwrap().credits.push(CreditSample {
            available: true,
            granted_at: Some(at(23, 10)),
            expires_at: Some(at(30, 10)),
        });
        let mut old_credit = granted.clone();
        old_credit.banked.as_mut().unwrap().credits[0].granted_at = Some(at(10, 10));

        assert_eq!(
            received_at(&reset, &track([granted]), alone(&reset)),
            Some(at(23, 10))
        );
        assert_eq!(
            received_at(&reset, &track([old_credit]), alone(&reset)),
            None
        );
    }

    #[test]
    fn expired_credits_are_not_mistaken_for_a_banked_use() {
        let mut before = sample(at(23, 16), 40, 60, at(27, 12), Some(1));
        before.banked.as_mut().unwrap().credits.push(CreditSample {
            available: true,
            granted_at: Some(at(1, 0)),
            expires_at: Some(at(23, 17)),
        });
        let after = sample(at(23, 18), 0, 0, at(30, 18), Some(0));
        let track = track([before, after]);

        let reset = forced(at(23, 17));
        assert_eq!(received_at(&reset, &track, alone(&reset)), Some(at(23, 18)));
    }

    #[test]
    fn receipts_are_sticky() {
        let mut resets = vec![forced(at(23, 17))];
        let mut tracks = BTreeMap::from([(
            "codex".to_owned(),
            track([
                sample(at(23, 16), 40, 60, at(27, 12), None),
                sample(at(23, 18), 0, 0, at(30, 18), None),
            ]),
        )]);
        assert!(apply_receipts(&mut resets, &tracks));
        tracks.clear();
        assert!(!apply_receipts(&mut resets, &tracks));
        assert_eq!(resets[0].received.get("codex"), Some(&at(23, 18)));
    }
}
