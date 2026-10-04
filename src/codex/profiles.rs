use super::*;
use crate::{
    limits::CodexProfileSnapshot,
    secrets,
    settings::{CodexProfile, Settings},
};
const PROFILE_SECRET_PREFIX: &str = "codex.profile.";

pub fn profiles_for_settings(settings: &Settings) -> Vec<CodexProfile> {
    profiles_with_default(&settings.codex_profiles)
}

pub(crate) fn profiles_with_default(saved: &[CodexProfile]) -> Vec<CodexProfile> {
    let mut profiles = saved.to_vec();
    if !profiles.iter().any(CodexProfile::is_default) {
        profiles.insert(
            0,
            CodexProfile {
                id: CodexProfile::DEFAULT_ID.into(),
                name: "Default".into(),
                enabled: true,
            },
        );
    }
    profiles
}

pub(crate) fn set_home_profile_visibility(
    excluded: &mut Vec<String>,
    profile_id: &str,
    visible: bool,
) {
    excluded.retain(|id| id != profile_id);
    if !visible {
        excluded.push(profile_id.to_owned());
    }
}

pub fn save_profile_credential(profile_id: &str, value: Option<&str>) -> Result<()> {
    secrets::save(&format!("{PROFILE_SECRET_PREFIX}{profile_id}"), value)
}

pub(crate) fn load_profile_credential(profile_id: &str) -> Result<Option<String>> {
    secrets::load(&format!("{PROFILE_SECRET_PREFIX}{profile_id}"))
}

/// Normalize persisted account membership before workers or UI read the cache.
/// Keep legacy ambient-only snapshots untouched, including their service name.
pub fn prepare_startup_limits(limits: &RateLimits, settings: &Settings) -> RateLimits {
    if settings.codex_profiles.is_empty() && limits.codex_profiles.is_empty() {
        return limits.clone();
    }
    let usage = limits.usage.clone();
    let mut retained = prepare_profile_refresh(limits, settings, settings);
    retained.usage = usage;
    retained
}

/// Prepare the visible samples before replacing a Codex reader.
pub fn prepare_profile_refresh(
    limits: &RateLimits,
    previous: &Settings,
    next: &Settings,
) -> RateLimits {
    let previous_profiles = profiles_for_settings(previous);
    let previous_samples = profile_samples(limits, &previous_profiles);
    let profiles = profiles_for_settings(next);
    let snapshots = profiles
        .iter()
        .filter(|profile| profile.enabled)
        .map(|profile| {
            let unchanged = previous_profiles
                .iter()
                .any(|old| old.id == profile.id && old.enabled)
                && previous
                    .codex_profile_credential_revisions
                    .get(&profile.id)
                    .copied()
                    .unwrap_or(0)
                    == next
                        .codex_profile_credential_revisions
                        .get(&profile.id)
                        .copied()
                        .unwrap_or(0)
                && (!profile.is_default() || previous.codex_path == next.codex_path);
            let mut snapshot = unchanged
                .then(|| {
                    previous_samples
                        .iter()
                        .find(|sample| sample.id == profile.id)
                        .cloned()
                })
                .flatten()
                .unwrap_or_else(|| CodexProfileSnapshot {
                    id: profile.id.clone(),
                    ..Default::default()
                });
            snapshot.name = profile.name.clone();
            snapshot.limits.account_name = Some(profile.name.clone());
            snapshot
        })
        .collect::<Vec<_>>();
    let mut retained = snapshots
        .iter()
        .find(|sample| sample.limits.sampled_at.timestamp() > 0)
        .or_else(|| snapshots.first())
        .map(|sample| sample.limits.clone())
        .unwrap_or_default();
    retained.codex_profiles = snapshots;
    retained
}

/// The legacy single-account response is always Default. Never attribute its
/// quota to whichever new profile happens to appear first in settings.
pub(super) fn profile_samples(
    limits: &RateLimits,
    profiles: &[CodexProfile],
) -> Vec<CodexProfileSnapshot> {
    if !limits.codex_profiles.is_empty() {
        return limits
            .codex_profiles
            .iter()
            .filter(|sample| {
                profiles
                    .iter()
                    .any(|profile| profile.enabled && profile.id == sample.id)
            })
            .cloned()
            .collect();
    }
    profiles
        .iter()
        .find(|profile| profile.enabled && profile.is_default())
        .map(|profile| {
            let mut sample = limits.clone();
            sample.codex_profiles.clear();
            vec![CodexProfileSnapshot {
                id: profile.id.clone(),
                name: profile.name.clone(),
                limits: sample,
                error: None,
            }]
        })
        .unwrap_or_default()
}

/// Combines per-profile reads into one provider snapshot. A failed profile
/// keeps its previous numbers next to the error, and the first profile fills
/// the provider-level fields the tray and notifications read.
///
/// When no profile could be read the provider itself has failed, so the worker
/// reports it and, for a rate limit, pauses polling as it does for one profile.
pub(super) fn merge_profiles(
    results: Vec<(CodexProfile, Result<RateLimits>)>,
    previous: &[CodexProfileSnapshot],
    _now: DateTime<Utc>,
) -> Result<RateLimits> {
    if results.iter().all(|(_, result)| result.is_err()) {
        let mut errors = results
            .into_iter()
            .filter_map(|(_, result)| result.err())
            .collect::<Vec<_>>();
        errors.sort_by_key(|error| !crate::worker::is_rate_limited_error(error));
        if let Some(error) = errors.into_iter().next() {
            return Err(error);
        }
        return Ok(RateLimits::default());
    }
    let mut snapshots = results
        .into_iter()
        .map(|(profile, result)| {
            let (limits, error) = match result {
                Ok(limits) => (limits, None),
                Err(error) => {
                    crate::logger::info(format!(
                        "Codex profile {} failed: {error:#}",
                        profile.name
                    ));
                    let limits = previous
                        .iter()
                        .find(|snapshot| snapshot.id == profile.id)
                        .map(|snapshot| snapshot.limits.clone())
                        .unwrap_or_default();
                    (limits, Some(format!("{error:#}")))
                }
            };
            CodexProfileSnapshot {
                id: profile.id,
                name: profile.name,
                limits,
                error,
            }
        })
        .collect::<Vec<_>>();
    let mut limits = snapshots
        .first()
        .filter(|snapshot| {
            snapshot.error.is_none()
                || previous.iter().any(|cached| {
                    cached.id == snapshot.id && cached.limits.sampled_at.timestamp() > 0
                })
        })
        .or_else(|| snapshots.iter().find(|snapshot| snapshot.error.is_none()))
        .map(|snapshot| snapshot.limits.clone())
        .unwrap_or_default();
    // Profile cards are told apart by the name the user gave them.
    for snapshot in &mut snapshots {
        snapshot.limits.account_name = Some(snapshot.name.clone());
    }
    limits.codex_profiles = snapshots;
    Ok(limits)
}

/// Overlay renamed profiles onto a live snapshot. A rename is a settings-only
/// change and must not wait for the next read.
pub fn apply_profile_names(limits: &mut RateLimits, settings: &Settings) -> bool {
    // A rename can arrive before the legacy single-Default reader publishes
    // another sample. Promote its existing quota immediately, without a fetch.
    if limits.codex_profiles.is_empty() {
        let enabled = profiles_for_settings(settings)
            .into_iter()
            .filter(|profile| profile.enabled)
            .collect::<Vec<_>>();
        if let [profile] = enabled.as_slice()
            && (!profile.is_default() || profile.name != "Default")
        {
            let mut snapshot = limits.clone();
            snapshot.account_name = Some(profile.name.clone());
            limits.codex_profiles.push(CodexProfileSnapshot {
                id: profile.id.clone(),
                name: profile.name.clone(),
                limits: snapshot,
                error: None,
            });
            return true;
        }
    }
    let mut changed = false;
    for snapshot in &mut limits.codex_profiles {
        if let Some(profile) = settings
            .codex_profiles
            .iter()
            .find(|profile| profile.id == snapshot.id && profile.name != snapshot.name)
        {
            snapshot.name = profile.name.clone();
            snapshot.limits.account_name = Some(profile.name.clone());
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_first_profile_keeps_a_later_defaults_cached_provider_quota() {
        let previous = Settings::default();
        let mut next = previous.clone();
        next.codex_profiles = vec![
            CodexProfile::new("Work"),
            profiles_for_settings(&previous)[0].clone(),
        ];
        let limits = RateLimits {
            primary: LimitWindow {
                used_percent: Some(42),
                ..Default::default()
            },
            sampled_at: Utc::now(),
            ..Default::default()
        };
        let retained = prepare_profile_refresh(&limits, &previous, &next);
        assert_eq!(retained.primary.used_percent, Some(42));
        assert_eq!(retained.codex_profiles[0].name, "Work");
        assert_eq!(retained.codex_profiles[0].limits.sampled_at.timestamp(), 0);
        assert_eq!(
            retained.codex_profiles[1].limits.primary.used_percent,
            Some(42)
        );
    }

    #[test]
    fn startup_without_saved_settings_discards_deleted_accounts_and_keeps_default() {
        let settings = Settings::default();
        let default = profiles_for_settings(&settings)[0].clone();
        let saved = CodexProfile::new("Deleted");
        let sample = |used| RateLimits {
            primary: LimitWindow {
                used_percent: Some(used),
                ..Default::default()
            },
            sampled_at: Utc::now(),
            ..Default::default()
        };
        let mut cached = sample(90);
        cached.account_name = Some("Deleted".into());
        cached.codex_profiles = vec![
            CodexProfileSnapshot {
                id: saved.id,
                name: saved.name,
                limits: sample(90),
                error: None,
            },
            CodexProfileSnapshot {
                id: default.id,
                name: default.name,
                limits: sample(15),
                error: None,
            },
        ];
        let retained = prepare_startup_limits(&cached, &settings);
        assert_eq!(retained.codex_profiles.len(), 1);
        assert_eq!(retained.codex_profiles[0].id, "default");
        assert_eq!(retained.primary.used_percent, Some(15));
        assert_ne!(retained.account_name.as_deref(), Some("Deleted"));
    }

    #[test]
    fn startup_keeps_legacy_ambient_name_and_clears_saved_only_cache() {
        let settings = Settings::default();
        let legacy = RateLimits {
            account_name: Some("Local account".into()),
            sampled_at: Utc::now(),
            ..Default::default()
        };
        assert_eq!(prepare_startup_limits(&legacy, &settings), legacy);
        let mut cached = legacy;
        cached.codex_profiles = vec![CodexProfileSnapshot {
            id: "deleted".into(),
            name: "Deleted".into(),
            limits: cached.clone(),
            error: None,
        }];
        let retained = prepare_startup_limits(&cached, &settings);
        assert_eq!(retained.primary.used_percent, None);
        assert_eq!(retained.codex_profiles[0].limits.sampled_at.timestamp(), 0);
    }

    #[test]
    fn account_snapshots_roundtrip_without_oauth_credentials() {
        let profile = CodexProfile::new("Work");
        let merged =
            merge_profiles(vec![(profile, Ok(RateLimits::default()))], &[], Utc::now()).unwrap();
        let encoded = serde_json::to_string(&merged).unwrap();
        assert!(!encoded.contains("access_token"));
        assert!(!encoded.contains("refresh_token"));
        let restored: RateLimits = serde_json::from_str(&encoded).unwrap();
        assert_eq!(merged, restored);
    }

    #[test]
    fn renaming_the_only_default_profile_promotes_its_live_sample_immediately() {
        let mut limits = RateLimits {
            primary: LimitWindow {
                used_percent: Some(42),
                ..Default::default()
            },
            account_name: Some("Service name".into()),
            ..Default::default()
        };
        assert!(!apply_profile_names(&mut limits, &Settings::default()));
        let mut settings = Settings::default();
        settings.codex_profiles = profiles_for_settings(&settings);
        settings.codex_profiles[0].name = "Personal".into();
        assert!(apply_profile_names(&mut limits, &settings));
        let profile = &limits.codex_profiles[0];
        assert_eq!(profile.name, "Personal");
        assert_eq!(profile.limits.account_name.as_deref(), Some("Personal"));
        assert_eq!(profile.limits.primary.used_percent, Some(42));
        assert!(profile.limits.codex_profiles.is_empty());
        assert!(!apply_profile_names(&mut limits, &settings));
        settings.codex_profiles[0].name = "Work".into();
        assert!(apply_profile_names(&mut limits, &settings));
        assert_eq!(
            limits.codex_profiles[0].limits.account_name.as_deref(),
            Some("Work")
        );
    }

    #[test]
    fn one_manual_profile_keeps_its_identity_when_default_is_disabled() {
        let profile = CodexProfile {
            id: "work".into(),
            name: "Work".into(),
            enabled: true,
        };
        let sample = RateLimits {
            primary: LimitWindow {
                used_percent: Some(20),
                ..Default::default()
            },
            account_name: Some("Organization".into()),
            ..Default::default()
        };
        let mut limits =
            merge_profiles(vec![(profile.clone(), Ok(sample))], &[], Utc::now()).unwrap();
        assert_eq!(limits.codex_profiles.len(), 1);
        assert_eq!(limits.codex_profiles[0].id, "work");
        assert_eq!(
            limits.codex_profiles[0].limits.account_name.as_deref(),
            Some("Work")
        );
        let mut settings = Settings::default();
        settings.codex_profiles = profiles_for_settings(&settings);
        settings.codex_profiles[0].enabled = false;
        settings.codex_profiles.push(CodexProfile {
            name: "Renamed".into(),
            ..profile
        });
        assert!(apply_profile_names(&mut limits, &settings));
        assert_eq!(limits.codex_profiles[0].name, "Renamed");
        assert_eq!(
            limits.codex_profiles[0].limits.primary.used_percent,
            Some(20)
        );
    }

    #[test]
    fn adding_profile_and_restarting_reader_keeps_default_sample_on_429() {
        let previous = Settings::default();
        let mut next = previous.clone();
        next.codex_profiles = profiles_for_settings(&previous);
        let added = CodexProfile::new("Account 1");
        next.codex_profiles.push(added.clone());
        let sample = RateLimits {
            primary: LimitWindow {
                used_percent: Some(0),
                ..Default::default()
            },
            secondary: LimitWindow {
                used_percent: Some(3),
                ..Default::default()
            },
            sampled_at: Utc::now() - chrono::Duration::minutes(1),
            ..Default::default()
        };
        let visible = prepare_profile_refresh(&sample, &previous, &next);
        let mut restarted = CodexClient::new("fixture-codex")
            .with_profiles(profiles_for_settings(&next))
            .with_cached_limits(&visible);
        let merged = restarted
            .merge_profile_results(vec![
                (
                    profiles_for_settings(&next)[0].clone(),
                    Err(crate::worker::rate_limit_error("fixture 429")),
                ),
                (added, Ok(sample.clone())),
            ])
            .unwrap();
        let default = merged
            .codex_profiles
            .iter()
            .find(|profile| profile.id == CodexProfile::DEFAULT_ID)
            .unwrap();
        assert_eq!(default.limits.primary.used_percent, Some(0));
        assert_eq!(default.limits.secondary.used_percent, Some(3));
        assert_eq!(default.limits.sampled_at, sample.sampled_at);
        assert!(default.error.is_some());
        assert!(restarted.take_rate_limit_response());
        assert!(!restarted.take_rate_limit_response());
    }

    #[test]
    fn profile_refresh_discards_only_disabled_removed_and_replaced_credentials() {
        let mut previous = Settings::default();
        let profile = |id: &str| CodexProfile {
            id: id.into(),
            name: id.into(),
            enabled: true,
        };
        previous.codex_profiles = vec![
            profile("default"),
            profile("changed"),
            profile("disabled"),
            profile("removed"),
            profile("untouched"),
        ];
        let sample = RateLimits {
            primary: LimitWindow {
                used_percent: Some(25),
                ..Default::default()
            },
            sampled_at: Utc::now(),
            ..Default::default()
        };
        let limits = merge_profiles(
            previous
                .codex_profiles
                .iter()
                .cloned()
                .map(|profile| (profile, Ok(sample.clone())))
                .collect(),
            &[],
            Utc::now(),
        )
        .unwrap();
        let mut next = previous.clone();
        next.codex_profiles
            .retain(|profile| profile.id != "removed");
        next.codex_profiles
            .iter_mut()
            .find(|profile| profile.id == "disabled")
            .unwrap()
            .enabled = false;
        next.codex_profiles
            .iter_mut()
            .find(|profile| profile.id == "untouched")
            .unwrap()
            .name = "Renamed".into();
        next.codex_profile_credential_revisions
            .insert("changed".into(), 1);
        next.codex_credentials_revision += 1;
        let retained = prepare_profile_refresh(&limits, &previous, &next);
        assert_eq!(retained.codex_profiles.len(), 3);
        for id in ["default", "untouched"] {
            let cached = retained
                .codex_profiles
                .iter()
                .find(|profile| profile.id == id)
                .unwrap();
            assert_eq!(cached.limits.primary.used_percent, Some(25));
            assert_eq!(cached.limits.sampled_at, sample.sampled_at);
        }
        let changed = retained
            .codex_profiles
            .iter()
            .find(|profile| profile.id == "changed")
            .unwrap();
        assert_eq!(changed.limits.primary.used_percent, None);
        assert_eq!(changed.limits.sampled_at.timestamp(), 0);
        assert_eq!(
            retained
                .codex_profiles
                .iter()
                .find(|profile| profile.id == "untouched")
                .unwrap()
                .name,
            "Renamed"
        );
    }

    #[test]
    fn legacy_default_sample_never_moves_to_a_new_first_profile() {
        let previous = Settings::default();
        let mut next = previous.clone();
        next.codex_profiles = vec![
            CodexProfile::new("First"),
            profiles_for_settings(&previous)[0].clone(),
        ];
        let sample = RateLimits {
            primary: LimitWindow {
                used_percent: Some(42),
                ..Default::default()
            },
            ..Default::default()
        };
        let retained = prepare_profile_refresh(&sample, &previous, &next);
        assert_eq!(retained.codex_profiles[0].limits.primary.used_percent, None);
        assert_eq!(retained.codex_profiles[1].id, "default");
        assert_eq!(
            retained.codex_profiles[1].limits.primary.used_percent,
            Some(42)
        );
    }

    #[test]
    fn failed_first_profile_without_cache_uses_a_successful_profile_for_provider_quota() {
        let default = profiles_for_settings(&Settings::default())[0].clone();
        let work = CodexProfile::new("Work");
        let sample = RateLimits {
            primary: LimitWindow {
                used_percent: Some(25),
                ..Default::default()
            },
            sampled_at: Utc::now(),
            account_name: Some("Work identity".into()),
            ..Default::default()
        };
        let merged = merge_profiles(
            vec![
                (default, Err(anyhow::anyhow!("fixture failure"))),
                (work, Ok(sample)),
            ],
            &[],
            Utc::now(),
        )
        .unwrap();
        assert_eq!(merged.primary.used_percent, Some(25));
        assert_eq!(merged.account_name.as_deref(), Some("Work identity"));
        assert!(merged.codex_profiles[0].error.is_some());
        assert_eq!(merged.codex_profiles[0].limits.primary.used_percent, None);
    }

    #[test]
    fn failed_first_profile_with_cache_keeps_its_provider_quota() {
        let default = profiles_for_settings(&Settings::default())[0].clone();
        let sample = |percent| RateLimits {
            primary: LimitWindow {
                used_percent: Some(percent),
                ..Default::default()
            },
            sampled_at: Utc::now(),
            ..Default::default()
        };
        let previous = vec![CodexProfileSnapshot {
            id: default.id.clone(),
            name: default.name.clone(),
            limits: sample(10),
            error: None,
        }];
        let merged = merge_profiles(
            vec![
                (default, Err(anyhow::anyhow!("fixture failure"))),
                (CodexProfile::new("Work"), Ok(sample(40))),
            ],
            &previous,
            Utc::now(),
        )
        .unwrap();
        assert_eq!(merged.primary.used_percent, Some(10));
        assert_eq!(
            merged.codex_profiles[1].limits.primary.used_percent,
            Some(40)
        );
    }

    #[test]
    fn a_failing_profile_keeps_its_previous_limits_and_reports_the_error() {
        let profile = |id: &str, name: &str| CodexProfile {
            id: id.into(),
            name: name.into(),
            enabled: true,
        };
        let limits = |percent| RateLimits {
            primary: LimitWindow {
                used_percent: Some(percent),
                ..LimitWindow::default()
            },
            account_name: Some("Real Name".into()),
            ..RateLimits::default()
        };
        let now = Utc::now();
        let first = merge_profiles(
            vec![
                (profile("default", "Default"), Ok(limits(10))),
                (profile("work", "Work"), Ok(limits(40))),
            ],
            &[],
            now,
        )
        .unwrap();
        // The provider-level fields describe the first profile, unrenamed.
        assert_eq!(first.primary.used_percent, Some(10));
        assert_eq!(first.account_name.as_deref(), Some("Real Name"));

        let second = merge_profiles(
            vec![
                (profile("default", "Default"), Ok(limits(20))),
                (
                    profile("work", "Work"),
                    Err(anyhow::anyhow!("session expired")),
                ),
            ],
            &first.codex_profiles,
            now,
        )
        .unwrap();
        let [default, work] = second.codex_profiles.as_slice() else {
            panic!("expected both profiles");
        };
        assert_eq!(default.limits.primary.used_percent, Some(20));
        assert_eq!(default.error, None);
        assert_eq!(work.limits.primary.used_percent, Some(40));
        assert_eq!(work.limits.account_name.as_deref(), Some("Work"));
        assert_eq!(work.error.as_deref(), Some("session expired"));

        // With nothing readable the provider fails, and a rate limit wins so
        // the worker pauses polling.
        let error = merge_profiles(
            vec![
                (
                    profile("default", "Default"),
                    Err(anyhow::anyhow!("session expired")),
                ),
                (
                    profile("work", "Work"),
                    Err(crate::worker::rate_limit_error("slow down")),
                ),
            ],
            &second.codex_profiles,
            now,
        )
        .unwrap_err();
        assert!(crate::worker::is_rate_limited_error(&error));
    }
}
