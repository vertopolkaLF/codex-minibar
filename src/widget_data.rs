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
    let (resolved_id, label, raw_window) =
        provider_registry::resolve_metric(provider, limits, configured_id)?;
    let window = canonical_widget_window(
        provider,
        limits,
        &[configured_id, resolved_id.as_str()],
        raw_window,
    );
    Some(WidgetMetric {
        id: configured_id.to_owned(),
        resolved_id,
        label,
        window,
    })
}

/// Build the shared sanitized snapshot consumed by external widget clients.
pub fn snapshot(provider: ProviderKind, limits: &ProviderLimits) -> ProviderSnapshot {
    let descriptor = provider_registry::descriptor(provider);
    let provider_limits = limits.get(provider);
    let mut metrics = descriptor
        .metrics
        .iter()
        .filter_map(|metric| metric_snapshot(provider, provider_limits, metric.id))
        .collect::<Vec<_>>();
    for additional in provider_limits.additional_limits.iter() {
        let metric_id = provider_registry::additional_limit_brick_id(provider, &additional.id);
        if metrics.iter().all(|metric| metric.id != metric_id)
            && let Some(metric) = metric_snapshot(provider, provider_limits, &metric_id)
        {
            metrics.push(metric);
        }
    }

    let additional = provider_limits
        .additional_limits
        .iter()
        .map(|additional| {
            let metric_id = provider_registry::additional_limit_brick_id(provider, &additional.id);
            let window = resolve_metric(provider, provider_limits, &metric_id)
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
    metric_id: &str,
) -> Option<MetricSnapshot> {
    let metric = resolve_metric(provider, limits, metric_id)?;
    Some(MetricSnapshot {
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
