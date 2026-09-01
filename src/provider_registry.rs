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
    /// Whether the provider exposes a real operation that starts a fresh
    /// session limit window.
    pub supports_activation: bool,
    /// Whether the provider contributes date-scoped token history to Usage Stats.
    pub include_in_total_spend: bool,
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
    MetricDescriptor {
        id: "cursor.grokBot",
        label: "Grok Bot",
        source: MetricSource::Additional("cursor-grok-bot"),
    },
];

// Zen has no authoritative percentage/reset windows exposed by its API.
// Local spend is rendered in the popup activity card, not as a fake tray
// quota.
const OPENCODE_ZEN_METRICS: &[MetricDescriptor] = &[];

const OPENCODE_GO_METRICS: &[MetricDescriptor] = &[
    MetricDescriptor {
        id: "opencode-go.session",
        label: "5h session",
        source: MetricSource::Primary,
    },
    MetricDescriptor {
        id: "opencode-go.weekly",
        label: "Weekly",
        source: MetricSource::Secondary,
    },
    MetricDescriptor {
        id: "opencode-go.monthly",
        label: "Monthly",
        source: MetricSource::Additional("monthly"),
    },
];

const OPENROUTER_METRICS: &[MetricDescriptor] = &[MetricDescriptor {
    id: "openrouter.limit",
    label: "Spending limit",
    source: MetricSource::Primary,
}];

pub const PROVIDERS: &[ProviderDescriptor] = &[
    ProviderDescriptor {
        kind: ProviderKind::Codex,
        id: "codex",
        display_name: "Codex",
        icon: "codex",
        brand_rgb: (128, 159, 255),
        supports_activation: true,
        include_in_total_spend: true,
        metrics: CODEX_METRICS,
        default_tray_metrics: &["codex.session", "codex.weekly"],
    },
    ProviderDescriptor {
        kind: ProviderKind::Claude,
        id: "claude",
        display_name: "Claude",
        icon: "claude",
        brand_rgb: (217, 119, 87),
        supports_activation: true,
        include_in_total_spend: true,
        metrics: CLAUDE_METRICS,
        default_tray_metrics: &["claude.session", "claude.weekly"],
    },
    ProviderDescriptor {
        kind: ProviderKind::Cursor,
        id: "cursor",
        display_name: "Cursor",
        icon: "cursor",
        brand_rgb: (145, 151, 164),
        supports_activation: false,
        include_in_total_spend: true,
        metrics: CURSOR_METRICS,
        default_tray_metrics: &["cursor.auto", "cursor.api"],
    },
    ProviderDescriptor {
        kind: ProviderKind::OpenCodeZen,
        id: "opencode",
        display_name: "OpenCode Zen",
        icon: "opencode",
        brand_rgb: (128, 128, 128),
        supports_activation: false,
        include_in_total_spend: true,
        metrics: OPENCODE_ZEN_METRICS,
        default_tray_metrics: &[],
    },
    ProviderDescriptor {
        kind: ProviderKind::OpenCodeGo,
        id: "opencode-go",
        display_name: "OpenCode Go",
        icon: "opencode",
        brand_rgb: (128, 128, 128),
        supports_activation: false,
        include_in_total_spend: true,
        metrics: OPENCODE_GO_METRICS,
        default_tray_metrics: &[
            "opencode-go.session",
            "opencode-go.weekly",
            "opencode-go.monthly",
        ],
    },
    ProviderDescriptor {
        kind: ProviderKind::OpenRouter,
        id: "openrouter",
        display_name: "OpenRouter",
        icon: "openrouter",
        brand_rgb: (200, 255, 0),
        supports_activation: false,
        include_in_total_spend: false,
        metrics: OPENROUTER_METRICS,
        default_tray_metrics: &["openrouter.limit"],
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

/// Maps any popup brick id — catalog, extras, or discovered — onto its provider.
pub fn provider_for_brick_id(brick_id: &str) -> Option<ProviderKind> {
    ProviderKind::ALL.into_iter().find(|provider| {
        brick_id.starts_with(&format!("{}.", descriptor(*provider).id))
    })
}

pub fn dynamic_metric_id(provider: ProviderKind, source_id: &str) -> String {
    format!("{}.additional.{source_id}", descriptor(provider).id)
}

pub fn resets_brick_id(provider: ProviderKind) -> String {
    format!("{}.resets", descriptor(provider).id)
}

pub fn credits_brick_id(provider: ProviderKind) -> String {
    format!("{}.credits", descriptor(provider).id)
}

pub fn usage_brick_id(provider: ProviderKind) -> String {
    format!("{}.usage", descriptor(provider).id)
}

pub fn spending_brick_id(provider: ProviderKind) -> String {
    format!("{}.spending", descriptor(provider).id)
}

/// Whether this provider can expose banked reset credits in the popup.
pub fn supports_banked_resets(provider: ProviderKind) -> bool {
    !matches!(provider, ProviderKind::Cursor | ProviderKind::OpenRouter)
}

/// Whether this provider can expose a credits card in the popup.
pub fn supports_credits(provider: ProviderKind) -> bool {
    !matches!(provider, ProviderKind::OpenRouter)
}

/// Whether this provider can expose local usage statistics in the popup.
pub fn supports_usage_stats(_provider: ProviderKind) -> bool {
    true
}

/// Whether this provider exposes OpenRouter-style spending strips.
pub fn supports_spending_strips(provider: ProviderKind) -> bool {
    provider == ProviderKind::OpenRouter
}

/// Stable popup brick ids declared for a provider before runtime discovery.
pub fn catalog_brick_ids(provider: ProviderKind) -> Vec<String> {
    let mut ids = descriptor(provider)
        .metrics
        .iter()
        .map(|metric| metric.id.to_string())
        .collect::<Vec<_>>();
    if supports_banked_resets(provider) {
        ids.push(resets_brick_id(provider));
    }
    if supports_credits(provider) {
        ids.push(credits_brick_id(provider));
    }
    if supports_usage_stats(provider) {
        ids.push(usage_brick_id(provider));
    }
    if supports_spending_strips(provider) {
        ids.push(spending_brick_id(provider));
    }
    ids
}

/// Human label for a popup brick id shown in Settings.
pub fn brick_label(provider: ProviderKind, brick_id: &str) -> String {
    if let Some(metric) = metric(provider, brick_id) {
        return metric.label.into();
    }
    let provider_id = descriptor(provider).id;
    if brick_id == resets_brick_id(provider) {
        return "Banked resets".into();
    }
    if brick_id == credits_brick_id(provider) {
        return "Credits".into();
    }
    if brick_id == usage_brick_id(provider) {
        return "Usage stats".into();
    }
    if brick_id == spending_brick_id(provider) {
        return "Spending".into();
    }
    if let Some(source_id) = brick_id.strip_prefix(&format!("{provider_id}.additional.")) {
        return source_id.replace('-', " ").replace('_', " ");
    }
    brick_id.rsplit('.').next().unwrap_or(brick_id).replace('-', " ")
}

/// Catalog bricks plus runtime-discovered additional windows for Settings.
pub fn settings_brick_ids(
    provider: ProviderKind,
    extra_ids: impl IntoIterator<Item = impl AsRef<str>>,
) -> Vec<String> {
    let mut ids = catalog_brick_ids(provider);
    let prefix = format!("{}.additional.", descriptor(provider).id);
    for extra in extra_ids {
        let extra = extra.as_ref();
        if extra.starts_with(&prefix) && !ids.iter().any(|id| id == extra) {
            ids.push(extra.to_string());
        }
    }
    ids
}

/// Prefer the live provider title for a discovered additional window.
pub fn settings_brick_label(
    provider: ProviderKind,
    brick_id: &str,
    discovered_labels: &std::collections::BTreeMap<String, String>,
) -> String {
    if let Some(label) = discovered_labels.get(brick_id) {
        let label = label.trim();
        if !label.is_empty() {
            return label.to_string();
        }
    }
    brick_label(provider, brick_id)
}

/// Labels for additional windows that are not already in the static catalog.
pub fn discovered_additional_brick_labels(
    provider: ProviderKind,
    limits: &RateLimits,
) -> Vec<(String, String)> {
    let catalog = catalog_brick_ids(provider);
    limits
        .additional_limits
        .iter()
        .filter_map(|limit| {
            let id = additional_limit_brick_id(provider, &limit.id);
            if catalog.iter().any(|known| known == &id) {
                return None;
            }
            let title = limit.title.trim();
            if title.is_empty() {
                return None;
            }
            Some((id, title.to_string()))
        })
        .collect()
}

/// Resolve a popup limit card to its configured brick id.
pub fn limit_section_brick_id(provider: ProviderKind, section: LimitSectionKind) -> Option<String> {
    match section {
        LimitSectionKind::FiveHour => descriptor(provider)
            .metrics
            .iter()
            .find(|metric| matches!(metric.source, MetricSource::Primary))
            .map(|metric| metric.id.to_string()),
        LimitSectionKind::Weekly | LimitSectionKind::Monthly => descriptor(provider)
            .metrics
            .iter()
            .find(|metric| matches!(metric.source, MetricSource::Secondary))
            .map(|metric| metric.id.to_string()),
    }
}

/// Resolve an additional limit card to its popup brick id.
pub fn additional_limit_brick_id(provider: ProviderKind, source_id: &str) -> String {
    descriptor(provider)
        .metrics
        .iter()
        .find(|metric| {
            matches!(metric.source, MetricSource::Additional(id) if id == source_id)
        })
        .map(|metric| metric.id.to_string())
        .unwrap_or_else(|| dynamic_metric_id(provider, source_id))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitSectionKind {
    FiveHour,
    Weekly,
    Monthly,
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
    fn brick_ids_map_to_the_longest_provider_prefix() {
        assert_eq!(
            provider_for_brick_id("opencode.usage"),
            Some(ProviderKind::OpenCodeZen)
        );
        assert_eq!(
            provider_for_brick_id("opencode-go.session"),
            Some(ProviderKind::OpenCodeGo)
        );
        assert_eq!(
            provider_for_brick_id("codex.additional.runtime_lane"),
            Some(ProviderKind::Codex)
        );
    }

    #[test]
    fn cursor_grok_bot_maps_to_the_catalog_brick() {
        assert_eq!(
            additional_limit_brick_id(ProviderKind::Cursor, "cursor-grok-bot"),
            "cursor.grokBot"
        );
        assert!(
            catalog_brick_ids(ProviderKind::Cursor)
                .iter()
                .any(|id| id == "cursor.grokBot")
        );
        assert_eq!(
            discovered_additional_brick_labels(
                ProviderKind::Cursor,
                &RateLimits {
                    additional_limits: vec![crate::limits::AdditionalLimit {
                        id: "cursor-grok-bot".into(),
                        title: "Grok Bot".into(),
                        window: LimitWindow {
                            used_percent: Some(1),
                            ..Default::default()
                        },
                    }],
                    ..Default::default()
                }
            )
            .len(),
            0
        );
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

    #[test]
    fn only_providers_with_real_session_windows_support_activation() {
        assert!(descriptor(ProviderKind::Codex).supports_activation);
        assert!(descriptor(ProviderKind::Claude).supports_activation);
        assert!(!descriptor(ProviderKind::Cursor).supports_activation);
        assert!(!descriptor(ProviderKind::OpenCodeZen).supports_activation);
        assert!(!descriptor(ProviderKind::OpenCodeGo).supports_activation);
        assert!(!descriptor(ProviderKind::OpenRouter).supports_activation);
        assert!(!descriptor(ProviderKind::OpenRouter).include_in_total_spend);
    }

    #[test]
    fn opencode_registry_keeps_zen_popup_only_and_maps_go_monthly() {
        assert!(descriptor(ProviderKind::OpenCodeZen).metrics.is_empty());
        let limits = RateLimits {
            additional_limits: vec![crate::limits::AdditionalLimit {
                id: "monthly".into(),
                title: "Monthly".into(),
                window: LimitWindow {
                    used_percent: Some(4),
                    ..Default::default()
                },
            }],
            ..Default::default()
        };
        assert_eq!(
            metric_window(ProviderKind::OpenCodeGo, &limits, "opencode-go.monthly")
                .and_then(|window| window.used_percent),
            Some(4)
        );
    }

    #[test]
    fn settings_bricks_include_live_additional_windows_without_catalog_duplicates() {
        let extra_id = additional_limit_brick_id(ProviderKind::Claude, "seven_day_runtime_lane");
        let ids = settings_brick_ids(
            ProviderKind::Claude,
            [extra_id.as_str(), "claude.session"],
        );
        assert!(ids.contains(&"claude.session".into()));
        assert!(ids.contains(&extra_id));
        assert_eq!(
            ids.iter().filter(|id| *id == &extra_id).count(),
            1
        );

        let mut labels = std::collections::BTreeMap::new();
        labels.insert(extra_id.clone(), "Runtime Lane".into());
        assert_eq!(
            settings_brick_label(ProviderKind::Claude, &extra_id, &labels),
            "Runtime Lane"
        );
        assert_eq!(
            discovered_additional_brick_labels(
                ProviderKind::OpenCodeGo,
                &RateLimits {
                    additional_limits: vec![crate::limits::AdditionalLimit {
                        id: "monthly".into(),
                        title: "Monthly".into(),
                        window: LimitWindow {
                            used_percent: Some(4),
                            ..Default::default()
                        },
                    }],
                    ..Default::default()
                }
            )
            .len(),
            0
        );
    }
}
