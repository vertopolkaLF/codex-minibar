use crate::{
    limits::{LimitWindow, RateLimits},
    settings::ProviderKind,
};

/// Provider-independent identity for a quota window exposed to popup, tray,
/// settings, and future provider surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricSource {
    Primary,
    Secondary,
    Additional(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MetricDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub source: MetricSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderDescriptor {
    pub kind: ProviderKind,
    pub id: &'static str,
    pub display_name: &'static str,
    pub icon: &'static str,
    pub brand_rgb: (u8, u8, u8),
    /// Stable metrics shown before runtime-discovered provider-specific lanes.
    pub metrics: &'static [MetricDescriptor],
    /// Ordered metrics used by onboarding and the provider preset.
    pub default_tray_metrics: &'static [&'static str],
}

const CODEX_METRICS: &[MetricDescriptor] = &[
    MetricDescriptor {
        id: "codex.session",
        label: "5h session",
        source: MetricSource::Primary,
    },
    MetricDescriptor {
        id: "codex.weekly",
        label: "Weekly",
        source: MetricSource::Secondary,
    },
];

const CLAUDE_METRICS: &[MetricDescriptor] = &[
    MetricDescriptor {
        id: "claude.session",
        label: "5h session",
        source: MetricSource::Primary,
    },
    MetricDescriptor {
        id: "claude.weekly",
        label: "Weekly",
        source: MetricSource::Secondary,
    },
];

const CURSOR_METRICS: &[MetricDescriptor] = &[
    MetricDescriptor {
        id: "cursor.auto",
        label: "Auto + Composer",
        source: MetricSource::Secondary,
    },
    MetricDescriptor {
        id: "cursor.api",
        label: "API",
        source: MetricSource::Additional("cursor-api"),
    },
];

pub const PROVIDERS: &[ProviderDescriptor] = &[
    ProviderDescriptor {
        kind: ProviderKind::Codex,
        id: "codex",
        display_name: "Codex",
        icon: "codex",
        brand_rgb: (128, 159, 255),
        metrics: CODEX_METRICS,
        default_tray_metrics: &["codex.session", "codex.weekly"],
    },
    ProviderDescriptor {
        kind: ProviderKind::Claude,
        id: "claude",
        display_name: "Claude",
        icon: "claude",
        brand_rgb: (217, 119, 87),
        metrics: CLAUDE_METRICS,
        default_tray_metrics: &["claude.session", "claude.weekly"],
    },
    ProviderDescriptor {
        kind: ProviderKind::Cursor,
        id: "cursor",
        display_name: "Cursor",
        icon: "cursor",
        brand_rgb: (145, 151, 164),
        metrics: CURSOR_METRICS,
        default_tray_metrics: &["cursor.auto", "cursor.api"],
    },
];

pub fn descriptor(provider: ProviderKind) -> &'static ProviderDescriptor {
    PROVIDERS
        .iter()
        .find(|descriptor| descriptor.kind == provider)
        .expect("every ProviderKind must have a registry descriptor")
}

pub fn metric(provider: ProviderKind, id: &str) -> Option<&'static MetricDescriptor> {
    descriptor(provider)
        .metrics
        .iter()
        .find(|metric| metric.id == id)
}

pub fn provider_for_metric(id: &str) -> Option<ProviderKind> {
    PROVIDERS
        .iter()
        .find(|provider| provider.metrics.iter().any(|metric| metric.id == id))
        .map(|provider| provider.kind)
}

pub fn dynamic_metric_id(provider: ProviderKind, source_id: &str) -> String {
    format!("{}.additional.{source_id}", descriptor(provider).id)
}

pub fn metric_label(provider: ProviderKind, limits: &RateLimits, id: &str) -> String {
    if let Some(metric) = metric(provider, id) {
        return metric.label.into();
    }
    limits
        .additional_limits
        .iter()
        .find(|limit| dynamic_metric_id(provider, &limit.id) == id)
        .map(|limit| limit.title.clone())
        .unwrap_or_else(|| id.rsplit('.').next().unwrap_or(id).replace('-', " "))
}

pub fn metric_window<'a>(
    provider: ProviderKind,
    limits: &'a RateLimits,
    id: &str,
) -> Option<&'a LimitWindow> {
    if let Some(metric) = metric(provider, id) {
        return match metric.source {
            MetricSource::Primary => Some(&limits.primary),
            MetricSource::Secondary => Some(&limits.secondary),
            MetricSource::Additional(source_id) => limits
                .additional_limits
                .iter()
                .find(|limit| limit.id == source_id)
                .map(|limit| &limit.window),
        };
    }
    limits
        .additional_limits
        .iter()
        .find(|limit| dynamic_metric_id(provider, &limit.id) == id)
        .map(|limit| &limit.window)
}

/// Resolves the configured metric, temporarily falling back to the first live
/// compatible metric exposed by the same provider. The configured ID is never
/// mutated, so it automatically returns when the provider reports it again.
pub fn resolve_metric<'a>(
    provider: ProviderKind,
    limits: &'a RateLimits,
    configured_id: &str,
) -> Option<(String, String, &'a LimitWindow)> {
    if let Some(window) = metric_window(provider, limits, configured_id)
        && !window.is_empty()
    {
        return Some((
            configured_id.into(),
            metric_label(provider, limits, configured_id),
            window,
        ));
    }

    for metric in descriptor(provider).metrics {
        if let Some(window) = metric_window(provider, limits, metric.id)
            && !window.is_empty()
        {
            return Some((metric.id.into(), metric.label.into(), window));
        }
    }
    limits.additional_limits.iter().find_map(|limit| {
        (!limit.window.is_empty()).then(|| {
            (
                dynamic_metric_id(provider, &limit.id),
                limit.title.clone(),
                &limit.window,
            )
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_covers_every_provider_exactly_once() {
        assert_eq!(PROVIDERS.len(), ProviderKind::ALL.len());
        for provider in ProviderKind::ALL {
            assert_eq!(
                PROVIDERS
                    .iter()
                    .filter(|descriptor| descriptor.kind == provider)
                    .count(),
                1
            );
        }
    }

    #[test]
    fn metric_ids_are_unique_and_namespaced() {
        let mut ids = std::collections::HashSet::new();
        for provider in PROVIDERS {
            for metric in provider.metrics {
                assert!(metric.id.starts_with(provider.id));
                assert!(ids.insert(metric.id));
            }
        }
    }
}
