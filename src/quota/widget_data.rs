//! Shared, provider-neutral data source for every quota widget.
//!
//! Provider workers keep the raw API shape in [`crate::quota::limits::ProviderLimits`].
//! Widgets must not each reinterpret that shape independently: this module is
//! the single place that resolves configured metrics, applies display policy,
//! and exposes sanitized snapshots to external widget clients.

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::{
    providers::instances::ProviderId,
    providers::registry,
    providers::registry::ProviderKind,
    quota::limits::{LimitWindow, ProviderLimits, RateLimits},
    settings::{LimitValue, TrayColorMode, TrayIndicator},
};

pub type Rgb = [u8; 3];

/// Written in place of a value while no data has arrived.
pub const NO_VALUE: &str = "\u{2014}";

/// The percentage an indicator shows: what is left or what is used.
pub fn displayed_percent(window: &LimitWindow, value: LimitValue) -> Option<u8> {
    match value {
        LimitValue::Remaining => window.remaining_percent(),
        LimitValue::Used => window.used_percent.map(|used| used.min(100)),
    }
}

/// `42%`, or [`NO_VALUE`].
pub fn percent_text(percent: Option<u8>) -> String {
    percent.map_or_else(|| NO_VALUE.to_owned(), |value| format!("{value}%"))
}

/// How urgent a quota is, by what is left of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Critical,
    Warning,
    Healthy,
}

impl Status {
    pub fn of(remaining: Option<u8>) -> Option<Self> {
        remaining.map(|remaining| match remaining {
            0..=15 => Self::Critical,
            16..=50 => Self::Warning,
            _ => Self::Healthy,
        })
    }
}

/// The colors one surface draws indicators with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndicatorPalette {
    pub critical: Rgb,
    pub warning: Rgb,
    pub healthy: Rgb,
    pub text: Rgb,
    pub accent: Rgb,
}

impl IndicatorPalette {
    /// The color of `indicator` and whether it is a real tint rather than
    /// the text color. Status colors follow what is left even when the
    /// indicator shows what is used.
    pub fn color(
        &self,
        indicator: &TrayIndicator,
        remaining: Option<u8>,
        provider_tint: Rgb,
    ) -> (Rgb, bool) {
        match indicator.color_mode {
            TrayColorMode::Status => match Status::of(remaining) {
                Some(Status::Critical) => (self.critical, true),
                Some(Status::Warning) => (self.warning, true),
                Some(Status::Healthy) => (self.healthy, true),
                None => (self.text, false),
            },
            TrayColorMode::Fixed => (
                [
                    indicator.fixed_color.red,
                    indicator.fixed_color.green,
                    indicator.fixed_color.blue,
                ],
                true,
            ),
            TrayColorMode::Provider => (provider_tint, true),
            TrayColorMode::Accent => (self.accent, true),
            TrayColorMode::Monochrome => (self.text, false),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WidgetMetric {
    /// Globally unique instance/metric identity.
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

/// Complete sanitized snapshot for one provider instance. The legacy raw
/// windows are retained for protocol compatibility; new widgets should
/// consume `metrics`.
#[derive(Clone, Debug, Serialize)]
pub struct ProviderSnapshot {
    /// Provider instance id; the primary instance keeps the driver id.
    pub id: String,
    /// Driver id, e.g. `claude`, shared by every instance of a driver.
    pub kind: String,
    pub source_id: String,
    pub name: String,
    /// Instance badge, present while a driver has several enabled instances.
    pub badge: Option<String>,
    pub badge_rgb: Option<[u8; 3]>,
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

/// Globally unique identity of one instance's metric.
pub fn metric_source_id(provider: ProviderId, metric_id: &str) -> String {
    format!("{}:metric:{metric_id}", provider.id())
}

/// Resolve a configured widget metric from the shared source.
pub fn resolve_metric(
    provider: ProviderId,
    limits: &RateLimits,
    configured_id: &str,
) -> Option<WidgetMetric> {
    let kind = provider.kind();
    let (resolved_id, label, raw_window) = registry::resolve_metric(kind, limits, configured_id)?;
    let metric_ids = [configured_id, resolved_id.as_str()];
    let five_hour_reserve = codex_five_hour_reserve_override(kind, limits, &metric_ids);
    let (label, window) = if let Some(reserve_window) = five_hour_reserve {
        let mut window = reserve_window.clone();
        window.resets_at = limits.primary.resets_at;
        window.duration_minutes = limits.primary.duration_minutes;
        (crate::i18n::tr("luna-reserve").to_owned(), window)
    } else {
        (
            label,
            canonical_widget_window(kind, limits, &metric_ids, raw_window),
        )
    };
    Some(WidgetMetric {
        source_id: metric_source_id(provider, configured_id),
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
                registry::metric(provider, id).map(|metric| metric.source),
                Some(registry::MetricSource::Primary)
            )
        })
    {
        return None;
    }

    limits.luna_reserve().map(|reserve| &reserve.window)
}

/// Every metric a provider offers, as `(id, label)`: its declared metrics,
/// then the additional windows it reported.
pub fn metric_catalog(provider: ProviderKind, limits: &RateLimits) -> Vec<(String, String)> {
    let mut catalog = registry::descriptor(provider)
        .metrics
        .iter()
        .map(|metric| (metric.id.to_owned(), metric.localized_label().to_owned()))
        .collect::<Vec<_>>();
    for additional in &limits.additional_limits {
        let id = registry::additional_limit_brick_id(provider, &additional.id);
        if catalog.iter().all(|(known, _)| *known != id) {
            catalog.push((id, registry::additional_label(additional)));
        }
    }
    catalog
}

/// Build the shared sanitized snapshot consumed by external widget clients.
pub fn snapshot(provider: ProviderId, limits: &ProviderLimits) -> ProviderSnapshot {
    let kind = provider.kind();
    let provider_limits = limits.get(provider);
    let descriptor = registry::descriptor(kind);
    let metrics = metric_catalog(kind, provider_limits)
        .into_iter()
        .filter_map(|(id, _)| metric_snapshot(provider, provider_limits, &id))
        .collect();

    let additional = provider_limits
        .additional_limits
        .iter()
        .map(|additional| {
            let metric_id = registry::additional_limit_brick_id(kind, &additional.id);
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

    let badge = provider.badge();
    ProviderSnapshot {
        id: provider.id().into(),
        kind: descriptor.id.into(),
        source_id: provider.id().into(),
        name: provider.qualified_name(),
        badge_rgb: badge
            .as_ref()
            .and_then(|badge| badge.color.rgb())
            .map(|(red, green, blue)| [red, green, blue]),
        badge: badge.map(|badge| badge.text),
        icon: registry::icon(kind).into(),
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
    provider: ProviderId,
    limits: &RateLimits,
    metric_id: &str,
) -> Option<MetricSnapshot> {
    let metric = resolve_metric(provider, limits, metric_id)?;
    Some(MetricSnapshot {
        source_id: metric.source_id,
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
    let reserve_ids = [
        registry::additional_limit_brick_id(provider, crate::quota::limits::GPT_RESERVE_LIMIT_ID),
        registry::dynamic_metric_id(provider, crate::quota::limits::GPT_RESERVE_LIMIT_ID),
    ];
    reserve_ids
        .iter()
        .any(|id| metric_id.eq_ignore_ascii_case(id))
        || registry::luna_reserve_override(provider, limits, metric_id).is_some()
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

    const CODEX: ProviderId = ProviderId::primary(ProviderKind::Codex);

    #[test]
    fn displayed_percent_shows_left_or_used_and_clamps_overruns() {
        for (used, value, expected) in [
            (Some(30), LimitValue::Remaining, Some(70)),
            (Some(30), LimitValue::Used, Some(30)),
            (Some(130), LimitValue::Remaining, Some(0)),
            (Some(130), LimitValue::Used, Some(100)),
            (None, LimitValue::Remaining, None),
            (None, LimitValue::Used, None),
        ] {
            let window = LimitWindow {
                used_percent: used,
                ..Default::default()
            };
            assert_eq!(
                displayed_percent(&window, value),
                expected,
                "{used:?} {value:?}"
            );
        }
        assert_eq!(percent_text(Some(42)), "42%");
        assert_eq!(percent_text(None), NO_VALUE);
    }

    #[test]
    fn indicator_colors_follow_each_mode_and_status_thresholds() {
        let palette = IndicatorPalette {
            critical: [1, 0, 0],
            warning: [2, 0, 0],
            healthy: [3, 0, 0],
            text: [4, 0, 0],
            accent: [5, 0, 0],
        };
        let tint = [6, 0, 0];
        let indicator = |color_mode| TrayIndicator {
            color_mode,
            fixed_color: crate::settings::TrayFixedColor {
                red: 7,
                green: 8,
                blue: 9,
            },
            ..TrayIndicator::new(CODEX, "codex.session")
        };
        let status = indicator(TrayColorMode::Status);
        for (remaining, expected) in [
            (Some(0), ([1, 0, 0], true)),
            (Some(15), ([1, 0, 0], true)),
            (Some(16), ([2, 0, 0], true)),
            (Some(50), ([2, 0, 0], true)),
            (Some(51), ([3, 0, 0], true)),
            (Some(100), ([3, 0, 0], true)),
            (None, ([4, 0, 0], false)),
        ] {
            assert_eq!(
                palette.color(&status, remaining, tint),
                expected,
                "{remaining:?}"
            );
        }
        // Every other mode ignores how much is left.
        for remaining in [Some(5), Some(90), None] {
            for (mode, expected) in [
                (TrayColorMode::Fixed, ([7, 8, 9], true)),
                (TrayColorMode::Provider, (tint, true)),
                (TrayColorMode::Accent, ([5, 0, 0], true)),
                (TrayColorMode::Monochrome, ([4, 0, 0], false)),
            ] {
                assert_eq!(
                    palette.color(&indicator(mode), remaining, tint),
                    expected,
                    "{mode:?} {remaining:?}"
                );
            }
        }
    }

    #[test]
    fn instances_have_independent_metric_identities() {
        let work = ProviderId::new(ProviderKind::Claude, "claude-work");
        let primary = ProviderId::primary(ProviderKind::Claude);
        let mut limits = ProviderLimits::default();
        limits.get_mut(primary).primary.used_percent = Some(7);
        limits.get_mut(work).primary.used_percent = Some(80);
        let metric_id = registry::descriptor(ProviderKind::Claude).default_tray_metrics[0];
        let first = resolve_metric(primary, limits.get(primary), metric_id).unwrap();
        let second = resolve_metric(work, limits.get(work), metric_id).unwrap();
        assert_eq!(first.window.used_percent, Some(7));
        assert_eq!(second.window.used_percent, Some(80));
        assert_ne!(first.source_id, second.source_id);
        let exported = snapshot(work, &limits);
        assert_eq!(exported.id, "claude-work");
        assert_eq!(exported.kind, "claude");
        assert_eq!(exported.primary.used_percent, Some(80));
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
            additional_limits: vec![crate::quota::limits::AdditionalLimit {
                id: crate::quota::limits::GPT_RESERVE_LIMIT_ID.into(),
                title: crate::quota::limits::LUNA_RESERVE_TITLE.into(),
                window: LimitWindow {
                    used_percent: Some(12),
                    resets_at: Some(reserve_reset),
                    duration_minutes: Some(300),
                },
            }],
            ..RateLimits::default()
        };

        let metric = resolve_metric(CODEX, &limits, "codex.lunaReserve").unwrap();
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
            additional_limits: vec![crate::quota::limits::AdditionalLimit {
                id: crate::quota::limits::GPT_RESERVE_LIMIT_ID.into(),
                title: crate::quota::limits::LUNA_RESERVE_TITLE.into(),
                window: LimitWindow {
                    used_percent: Some(12),
                    resets_at: Some(Utc.timestamp_opt(1_700_003_600, 0).unwrap()),
                    duration_minutes: Some(300),
                },
            }],
            ..RateLimits::default()
        };

        let session = resolve_metric(CODEX, &limits, "codex.session").unwrap();
        let weekly = resolve_metric(CODEX, &limits, "codex.weekly").unwrap();
        assert_eq!(session.window, weekly.window);
        assert_eq!(session.window.resets_at, Some(weekly_reset));
    }

    #[test]
    fn snapshot_exposes_the_same_luna_policy_to_external_widgets() {
        let weekly_reset = Utc.timestamp_opt(1_700_475_600, 0).unwrap();
        let limits = ProviderLimits::from_entries([(
            CODEX,
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
                additional_limits: vec![crate::quota::limits::AdditionalLimit {
                    id: crate::quota::limits::GPT_RESERVE_LIMIT_ID.into(),
                    title: crate::quota::limits::LUNA_RESERVE_TITLE.into(),
                    window: LimitWindow {
                        used_percent: Some(12),
                        resets_at: Some(Utc.timestamp_opt(1_700_003_600, 0).unwrap()),
                        duration_minutes: Some(300),
                    },
                }],
                ..RateLimits::default()
            },
        )]);

        let snapshot = snapshot(CODEX, &limits);
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
