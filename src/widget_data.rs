//! Shared, provider-neutral data source for every quota widget.
//!
//! Provider workers keep the raw API shape in [`crate::limits::ProviderLimits`].
//! Widgets must not each reinterpret that shape independently: this module is
//! the single place that resolves configured metrics, applies display policy,
//! and exposes sanitized snapshots to external widget clients.

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::{
    limits::{LimitWindow, ProviderLimits, RateLimits},
    provider_registry,
    settings::ProviderKind,
};

#[derive(Clone, Debug, PartialEq)]
pub struct WidgetMetric {
    /// Globally unique provider/account/metric identity.
    pub source_id: String,
    /// The configured metric id. Keeping this stable lets a widget recover
    /// without rewriting its settings when a provider temporarily falls back.
    pub id: String,
    /// The live metric id selected by the resolver, useful for diagnostics.
    pub resolved_id: String,
    pub label: String,
    pub window: LimitWindow,
}

/// Sanitized metric data sent to widget clients such as Stream Deck.
#[derive(Clone, Debug, Serialize)]
pub struct MetricSnapshot {
    pub source_id: String,
    pub id: String,
    pub label: String,
    pub window: WindowSnapshot,
}

#[derive(Clone, Debug, Serialize)]
pub struct WindowSnapshot {
    pub used_percent: Option<u8>,
    pub remaining_percent: Option<u8>,
    pub resets_at: Option<DateTime<Utc>>,
    pub duration_minutes: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AdditionalSnapshot {
    pub id: String,
    pub metric_id: String,
    pub label: String,
    pub window: WindowSnapshot,
}

/// Complete sanitized snapshot for one provider. The legacy raw windows are
/// retained for protocol compatibility; new widgets should consume `metrics`.
#[derive(Clone, Debug, Serialize)]
pub struct ProviderSnapshot {
    pub id: String,
    pub source_id: String,
    pub profile_id: Option<String>,
    /// Independent accounts, including cached quotas; no credentials are exposed.
    pub accounts: Vec<ProviderSnapshot>,
    pub name: String,
    pub icon: String,
    pub brand_rgb: [u8; 3],
    pub account_name: Option<String>,
    pub plan_type: Option<String>,
    pub sampled_at: DateTime<Utc>,
    pub primary: WindowSnapshot,
    pub secondary: WindowSnapshot,
    pub additional: Vec<AdditionalSnapshot>,
    /// Canonically resolved widget metrics. All widget clients should use
    /// these instead of reimplementing provider-specific fallback rules.
    pub metrics: Vec<MetricSnapshot>,
}

/// Resolve a configured widget metric from the shared source.
pub fn resolve_metric(
    provider: ProviderKind,
    limits: &RateLimits,
    configured_id: &str,
) -> Option<WidgetMetric> {
    resolve_account_metric(provider, limits, None, configured_id)
}

pub fn account_source_id(provider: ProviderKind, profile_id: Option<&str>) -> String {
    let profile_id = matches!(provider, ProviderKind::Claude | ProviderKind::Codex)
        .then_some(profile_id.unwrap_or("default"));
    crate::settings::HomeWidgetId::new(
        crate::settings::PopupWidgetKind::from_provider(provider),
        profile_id,
    )
    .id()
}

pub fn account_limits<'a>(
    provider: ProviderKind,
    limits: &'a RateLimits,
    profile_id: Option<&str>,
) -> Option<&'a RateLimits> {
    let profiles = limits.account_profiles(provider);
    if profiles.is_empty() {
        return (profile_id.is_none() || profile_id == Some("default")).then_some(limits);
    }
    let wanted = profile_id.unwrap_or("default");
    profiles.iter().find(|p| p.id == wanted).map(|p| &p.limits)
}

pub fn resolve_account_metric(
    provider: ProviderKind,
    limits: &RateLimits,
    profile_id: Option<&str>,
    configured_id: &str,
) -> Option<WidgetMetric> {
    let selected = account_limits(provider, limits, profile_id)?;
    let mut metric = resolve_metric_from_limits(provider, selected, configured_id)?;
    metric.source_id = format!(
        "{}:metric:{configured_id}",
        account_source_id(provider, profile_id)
    );
    Some(metric)
}

fn resolve_metric_from_limits(
    provider: ProviderKind,
    limits: &RateLimits,
    configured_id: &str,
) -> Option<WidgetMetric> {
    let (resolved_id, label, raw_window) =
        provider_registry::resolve_metric(provider, limits, configured_id)?;
    let metric_ids = [configured_id, resolved_id.as_str()];
    let five_hour_reserve = codex_five_hour_reserve_override(provider, limits, &metric_ids);
    let (label, window) = if let Some(reserve_window) = five_hour_reserve {
        let mut window = reserve_window.clone();
        window.resets_at = limits.primary.resets_at;
        window.duration_minutes = limits.primary.duration_minutes;
        (crate::limits::LUNA_RESERVE_TITLE.to_owned(), window)
    } else {
        (
            label,
            canonical_widget_window(provider, limits, &metric_ids, raw_window),
        )
    };
    Some(WidgetMetric {
        source_id: format!(
            "{}:metric:{configured_id}",
            account_source_id(provider, None)
        ),
        id: configured_id.to_owned(),
        resolved_id,
        label,
        window,
    })
}

fn codex_five_hour_reserve_override<'a>(
    provider: ProviderKind,
    limits: &'a RateLimits,
    metric_ids: &[&str],
) -> Option<&'a LimitWindow> {
    if provider != ProviderKind::Codex
        || limits.weekly_exhausted()
        || limits.primary.remaining_percent() != Some(0)
        || !metric_ids.iter().any(|id| {
            matches!(
                provider_registry::metric(provider, id).map(|metric| metric.source),
                Some(provider_registry::MetricSource::Primary)
            )
        })
    {
        return None;
    }

    limits.luna_reserve().map(|reserve| &reserve.window)
}

/// Build the shared sanitized snapshot consumed by external widget clients.
pub fn snapshot(provider: ProviderKind, limits: &ProviderLimits) -> ProviderSnapshot {
    let limits = limits.get(provider);
    let empty = RateLimits::default();
    let mut result = snapshot_for_account(
        provider,
        account_limits(provider, limits, None).unwrap_or(&empty),
        None,
    );
    result.accounts = limits
        .account_profiles(provider)
        .iter()
        .map(|profile| {
            let mut account = snapshot_for_account(provider, &profile.limits, Some(&profile.id));
            account.account_name = Some(profile.name.clone());
            account
        })
        .collect();
    result
}

fn snapshot_for_account(
    provider: ProviderKind,
    provider_limits: &RateLimits,
    profile_id: Option<&str>,
) -> ProviderSnapshot {
    let descriptor = provider_registry::descriptor(provider);
    let mut metrics = descriptor
        .metrics
        .iter()
        .filter_map(|metric| metric_snapshot(provider, provider_limits, profile_id, metric.id))
        .collect::<Vec<_>>();
    for additional in provider_limits.additional_limits.iter() {
        let metric_id = provider_registry::additional_limit_brick_id(provider, &additional.id);
        if metrics.iter().all(|metric| metric.id != metric_id)
            && let Some(metric) = metric_snapshot(provider, provider_limits, profile_id, &metric_id)
        {
            metrics.push(metric);
        }
    }

    let additional = provider_limits
        .additional_limits
        .iter()
        .map(|additional| {
            let metric_id = provider_registry::additional_limit_brick_id(provider, &additional.id);
            let window = resolve_metric_from_limits(provider, provider_limits, &metric_id)
                .map(|metric| metric.window)
                .unwrap_or_else(|| additional.window.clone());
            AdditionalSnapshot {
                id: additional.id.clone(),
                metric_id,
                label: additional.title.clone(),
                window: window_snapshot(&window),
            }
        })
        .collect();

    ProviderSnapshot {
        id: descriptor.id.into(),
        source_id: account_source_id(provider, profile_id),
        profile_id: profile_id.map(str::to_owned),
        accounts: Vec::new(),
        name: descriptor.display_name.into(),
        icon: provider_registry::icon(provider).into(),
        brand_rgb: [
            descriptor.brand_rgb.0,
            descriptor.brand_rgb.1,
            descriptor.brand_rgb.2,
        ],
        account_name: provider_limits.account_name.clone(),
        plan_type: provider_limits.plan_type.clone(),
        sampled_at: provider_limits.sampled_at,
        primary: window_snapshot(&provider_limits.primary),
        secondary: window_snapshot(&provider_limits.secondary),
        additional,
        metrics,
    }
}

fn metric_snapshot(
    provider: ProviderKind,
    limits: &RateLimits,
    profile_id: Option<&str>,
    metric_id: &str,
) -> Option<MetricSnapshot> {
    let metric = resolve_metric_from_limits(provider, limits, metric_id)?;
    Some(MetricSnapshot {
        source_id: format!(
            "{}:metric:{metric_id}",
            account_source_id(provider, profile_id)
        ),
        id: metric.id,
        label: metric.label,
        window: window_snapshot(&metric.window),
    })
}

fn canonical_widget_window(
    provider: ProviderKind,
    limits: &RateLimits,
    metric_ids: &[&str],
    raw_window: &LimitWindow,
) -> LimitWindow {
    if provider != ProviderKind::Codex
        || !metric_ids
            .iter()
            .any(|id| is_luna_reserve_metric(provider, limits, id))
    {
        return raw_window.clone();
    }

    let mut window = raw_window.clone();
    // Luna Reserve is usable quota inside the same weekly allowance. Its own
    // endpoint window can expose a short/irrelevant deadline, so widgets show
    // the weekly deadline that actually matters to the user.
    if let Some(reset) = limits.secondary.resets_at {
        window.resets_at = Some(reset);
    }
    if let Some(duration_minutes) = limits.secondary.duration_minutes {
        window.duration_minutes = Some(duration_minutes);
    }
    window
}

fn is_luna_reserve_metric(provider: ProviderKind, limits: &RateLimits, metric_id: &str) -> bool {
    if provider != ProviderKind::Codex {
        return false;
    }
    let reserve_metric_id =
        provider_registry::additional_limit_brick_id(provider, crate::limits::GPT_RESERVE_LIMIT_ID);
    if metric_id.eq_ignore_ascii_case(&reserve_metric_id)
        || metric_id.eq_ignore_ascii_case(&format!(
            "{}.additional.{}",
            provider_registry::descriptor(provider).id,
            crate::limits::GPT_RESERVE_LIMIT_ID
        ))
    {
        return true;
    }
    limits.codex_luna_reserve_override().is_some()
        && matches!(
            provider_registry::metric(provider, metric_id).map(|metric| metric.source),
            Some(
                provider_registry::MetricSource::Primary
                    | provider_registry::MetricSource::Secondary
            )
        )
}

pub(crate) fn window_snapshot(window: &LimitWindow) -> WindowSnapshot {
    WindowSnapshot {
        used_percent: window.used_percent,
        remaining_percent: window.remaining_percent(),
        resets_at: window.resets_at,
        duration_minutes: window.duration_minutes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn account_metrics_and_snapshots_use_identity_without_positional_fallback() {
        use crate::limits::AccountProfileSnapshot;
        for provider in [ProviderKind::Claude, ProviderKind::Codex] {
            let mut limits = ProviderLimits::default();
            let profile = |id: &str, used| {
                let mut account = RateLimits::default();
                account.primary.used_percent = Some(used);
                AccountProfileSnapshot {
                    id: id.into(),
                    name: "Same name".into(),
                    limits: account,
                    error: None,
                }
            };
            let mut parent = RateLimits::default();
            let profiles = vec![profile("work", 80), profile("default", 7)];
            match provider {
                ProviderKind::Claude => parent.claude_profiles = profiles,
                ProviderKind::Codex => parent.codex_profiles = profiles,
                _ => unreachable!(),
            }
            *limits.get_mut(provider) = parent;
            let metric_id = provider_registry::descriptor(provider).default_tray_metrics[0];
            let primary = resolve_metric(provider, limits.get(provider), metric_id).unwrap();
            let work =
                resolve_account_metric(provider, limits.get(provider), Some("work"), metric_id)
                    .unwrap();
            assert_eq!(primary.window.used_percent, Some(7));
            assert_eq!(work.window.used_percent, Some(80));
            assert_ne!(primary.source_id, work.source_id);
            assert!(
                resolve_account_metric(provider, limits.get(provider), Some("removed"), metric_id)
                    .is_none()
            );
            let exported = snapshot(provider, &limits);
            assert_eq!(exported.accounts.len(), 2);
            assert_eq!(exported.primary.used_percent, Some(7));
            assert_eq!(exported.accounts[0].profile_id.as_deref(), Some("work"));
            assert_eq!(
                exported.accounts[0].source_id,
                account_source_id(provider, Some("work"))
            );
            assert_ne!(
                exported.accounts[0].metrics[0].source_id,
                exported.accounts[1].metrics[0].source_id
            );
            assert_ne!(
                account_source_id(ProviderKind::Claude, Some("work")),
                account_source_id(ProviderKind::Codex, Some("work"))
            );
            // A removed Default must not turn an existing widget into Work.
            match provider {
                ProviderKind::Claude => limits
                    .get_mut(provider)
                    .claude_profiles
                    .retain(|p| p.id != "default"),
                ProviderKind::Codex => limits
                    .get_mut(provider)
                    .codex_profiles
                    .retain(|p| p.id != "default"),
                _ => unreachable!(),
            }
            assert!(resolve_metric(provider, limits.get(provider), metric_id).is_none());
            assert_eq!(snapshot(provider, &limits).primary.used_percent, None);
        }
    }

    #[test]
    fn luna_reserve_uses_weekly_reset_but_keeps_reserve_usage() {
        let weekly_reset = Utc.timestamp_opt(1_700_475_600, 0).unwrap();
        let reserve_reset = Utc.timestamp_opt(1_700_003_600, 0).unwrap();
        let limits = RateLimits {
            secondary: LimitWindow {
                used_percent: Some(100),
                resets_at: Some(weekly_reset),
                duration_minutes: Some(10_080),
            },
            additional_limits: vec![crate::limits::AdditionalLimit {
                id: crate::limits::GPT_RESERVE_LIMIT_ID.into(),
                title: crate::limits::LUNA_RESERVE_TITLE.into(),
                window: LimitWindow {
                    used_percent: Some(12),
                    resets_at: Some(reserve_reset),
                    duration_minutes: Some(300),
                },
            }],
            ..RateLimits::default()
        };

        let metric = resolve_metric(ProviderKind::Codex, &limits, "codex.lunaReserve").unwrap();
        assert_eq!(metric.window.used_percent, Some(12));
        assert_eq!(metric.window.resets_at, Some(weekly_reset));
        assert_eq!(metric.window.duration_minutes, Some(10_080));
    }

    #[test]
    fn exhausted_weekly_fallback_is_identical_for_session_and_weekly_metrics() {
        let weekly_reset = Utc.timestamp_opt(1_700_475_600, 0).unwrap();
        let limits = RateLimits {
            primary: LimitWindow {
                used_percent: Some(7),
                duration_minutes: Some(300),
                ..Default::default()
            },
            secondary: LimitWindow {
                used_percent: Some(100),
                resets_at: Some(weekly_reset),
                duration_minutes: Some(10_080),
            },
            additional_limits: vec![crate::limits::AdditionalLimit {
                id: crate::limits::GPT_RESERVE_LIMIT_ID.into(),
                title: crate::limits::LUNA_RESERVE_TITLE.into(),
                window: LimitWindow {
                    used_percent: Some(12),
                    resets_at: Some(Utc.timestamp_opt(1_700_003_600, 0).unwrap()),
                    duration_minutes: Some(300),
                },
            }],
            ..RateLimits::default()
        };

        let session = resolve_metric(ProviderKind::Codex, &limits, "codex.session").unwrap();
        let weekly = resolve_metric(ProviderKind::Codex, &limits, "codex.weekly").unwrap();
        assert_eq!(session.window, weekly.window);
        assert_eq!(session.window.resets_at, Some(weekly_reset));
    }

    #[test]
    fn snapshot_exposes_the_same_luna_policy_to_external_widgets() {
        let weekly_reset = Utc.timestamp_opt(1_700_475_600, 0).unwrap();
        let limits = ProviderLimits::from_entries([(
            ProviderKind::Codex,
            RateLimits {
                primary: LimitWindow {
                    used_percent: Some(7),
                    duration_minutes: Some(300),
                    ..Default::default()
                },
                secondary: LimitWindow {
                    used_percent: Some(100),
                    resets_at: Some(weekly_reset),
                    duration_minutes: Some(10_080),
                },
                additional_limits: vec![crate::limits::AdditionalLimit {
                    id: crate::limits::GPT_RESERVE_LIMIT_ID.into(),
                    title: crate::limits::LUNA_RESERVE_TITLE.into(),
                    window: LimitWindow {
                        used_percent: Some(12),
                        resets_at: Some(Utc.timestamp_opt(1_700_003_600, 0).unwrap()),
                        duration_minutes: Some(300),
                    },
                }],
                ..RateLimits::default()
            },
        )]);

        let snapshot = snapshot(ProviderKind::Codex, &limits);
        let reserve = snapshot
            .metrics
            .iter()
            .find(|metric| metric.id == "codex.lunaReserve")
            .unwrap();
        assert_eq!(reserve.window.used_percent, Some(12));
        assert_eq!(reserve.window.resets_at, Some(weekly_reset));
        assert_eq!(
            snapshot
                .metrics
                .iter()
                .filter(|metric| metric.id == "codex.lunaReserve")
                .count(),
            1
        );
    }
}
