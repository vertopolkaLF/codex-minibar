//! Framework-free view model of the taskbar widgets.
//!
//! Resolves configured indicators against the live limits once, so the
//! renderer, the hover card and the Settings previews all read the same
//! labels, values and colors.

// The taskbar strip is a Windows Explorer feature; Linux keeps only the
// settings model it shares with the tray.
#![cfg_attr(not(windows), allow(dead_code))]

use chrono::{DateTime, Local, Utc};

use crate::{
    instances::ProviderId,
    limits::ProviderLimits,
    settings::{
        LimitValue, TaskbarResetDisplay, TaskbarWidget, TaskbarWidgetStyle, TimeFormat,
        TrayColorMode, TrayIndicator,
    },
};

pub(crate) type Rgb = [u8; 3];

/// Everything outside the widget list that changes how widgets look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Appearance {
    /// The taskbar (not the app) uses the light theme.
    pub(crate) light: bool,
    pub(crate) colored_icons: bool,
    pub(crate) accent: Rgb,
}

impl Appearance {
    pub(crate) fn text(self) -> Rgb {
        if self.light {
            [0, 0, 0]
        } else {
            [255, 255, 255]
        }
    }

    /// Fluent `TextFillColorPrimary` opacity on this taskbar.
    pub(crate) fn text_alpha(self) -> f32 {
        if self.light { 0.896 } else { 1.0 }
    }

    /// Fluent `TextFillColorSecondary` opacity on this taskbar.
    pub(crate) fn secondary_alpha(self) -> f32 {
        if self.light { 0.606 } else { 0.786 }
    }

    /// Fluent system status colors, tuned per theme for contrast.
    fn status(self, remaining: Option<u8>) -> Rgb {
        match (remaining, self.light) {
            (Some(0..=15), false) => [255, 153, 164],
            (Some(0..=15), true) => [196, 43, 28],
            (Some(16..=50), false) => [252, 225, 0],
            (Some(16..=50), true) => [157, 93, 0],
            (Some(_), false) => [108, 203, 95],
            (Some(_), true) => [15, 123, 15],
            (None, _) => self.text(),
        }
    }

    fn provider_tint(self, provider: ProviderId) -> Rgb {
        let (red, green, blue) = if self.light {
            crate::provider_registry::light_surface_brand_rgb(provider.kind())
        } else {
            crate::provider_registry::dark_surface_brand_rgb(provider.kind())
        };
        [red, green, blue]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Icon {
    pub(crate) name: &'static str,
    pub(crate) badge: Option<crate::instances::Badge>,
    pub(crate) tint: Rgb,
}

/// One resolved indicator.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Row {
    pub(crate) provider: ProviderId,
    /// Short name for the taskbar: `5h`, `7d`, or the full name.
    pub(crate) label: String,
    /// Full name for the hover card.
    pub(crate) full_label: String,
    /// `42%`, or an em dash while no data has arrived.
    pub(crate) value: String,
    /// Fill fraction of the displayed value, 0..=1.
    pub(crate) progress: Option<f32>,
    /// Where an even burn would put the fill, 0..=1.
    pub(crate) pace: Option<f32>,
    /// Bar, ring and value tint.
    pub(crate) color: Rgb,
    /// Whether `color` is a real tint rather than the text color.
    pub(crate) tinted: bool,
    pub(crate) resets_at: Option<DateTime<Utc>>,
    pub(crate) icon: Icon,
    /// Remaining or used, for the hover card's wording.
    pub(crate) limit_value: LimitValue,
    /// When the quota resets, as the widget is set to write it.
    pub(crate) reset: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WidgetView {
    pub(crate) id: String,
    pub(crate) style: TaskbarWidgetStyle,
    pub(crate) icon: Option<Icon>,
    pub(crate) provider: Option<ProviderId>,
    pub(crate) rows: Vec<Row>,
    pub(crate) show_labels: bool,
    pub(crate) show_values: bool,
    /// Marks the pace on the widget's graphics; the hover card always does.
    pub(crate) show_pace: bool,
    /// The hover card: each provider the widget reads, in full.
    pub(crate) card: Vec<CardSection>,
}

/// One provider in the hover card: every quota it reports, not only the
/// widget's, and what it used today.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CardSection {
    pub(crate) provider: ProviderId,
    pub(crate) icon: Icon,
    pub(crate) plan: Option<String>,
    pub(crate) rows: Vec<Row>,
    /// Today's tokens and their API value, or spend for pay-as-you-go
    /// providers; `None` without usage statistics.
    pub(crate) today: Option<String>,
}

impl WidgetView {
    /// Whether anything is written beside or under the graphic.
    pub(crate) fn has_text(&self) -> bool {
        self.show_values || self.show_labels || self.rows.iter().any(|row| row.reset.is_some())
    }

    /// The widget with detail given up to keep its place on a crowded
    /// taskbar: reset times first, then labels, values and the mark. At
    /// [`COMPACT_LEVELS`] every design becomes bare meters, the narrowest
    /// graphic.
    pub(crate) fn compacted(&self, level: u8) -> Self {
        let mut view = self.clone();
        let graphic = view.style.has_graphic();
        if level >= 1 {
            for row in &mut view.rows {
                row.reset = None;
            }
            // A text design whose reset time stood in for the value.
            view.show_values |= !graphic;
        }
        if level >= 2 {
            view.show_labels = false;
        }
        if level >= 3 && graphic {
            view.show_values = false;
        }
        if level >= 4 {
            view.icon = None;
        }
        if level >= 5 {
            view.style = TaskbarWidgetStyle::Meters;
            view.rows
                .truncate(TaskbarWidgetStyle::Meters.max_indicators());
            view.show_labels = false;
            view.show_values = false;
        }
        view
    }
}

/// How many steps [`WidgetView::compacted`] takes to the shortest layout.
pub(crate) const COMPACT_LEVELS: u8 = 5;

/// Compact countdown: `2h 13m`, `3d 4h`, `45m`.
pub(crate) fn countdown(reset: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let minutes = (reset - now).num_minutes().max(0);
    let (days, hours, minutes) = (minutes / 1_440, (minutes % 1_440) / 60, minutes % 60);
    if days > 0 {
        if hours > 0 {
            crate::i18n::format(
                "days-d-hours-h",
                &[("days", days.to_string()), ("hours", hours.to_string())],
            )
        } else {
            crate::i18n::format("days-d", &[("days", days.to_string())])
        }
    } else if hours > 0 {
        if minutes > 0 {
            crate::i18n::format(
                "hours-h-minutes-m",
                &[
                    ("hours", hours.to_string()),
                    ("minutes", minutes.to_string()),
                ],
            )
        } else {
            crate::i18n::format("hours-h", &[("hours", hours.to_string())])
        }
    } else {
        crate::i18n::format("minutes-m", &[("minutes", minutes.to_string())])
    }
}

/// Local wall-clock time of a reset, with the date once it is not today.
pub(crate) fn reset_clock(reset: DateTime<Utc>) -> String {
    let local = reset.with_timezone(&Local);
    let time = TimeFormat::current().format_hm(local);
    if local.date_naive() == Local::now().date_naive() {
        time
    } else {
        format!("{} {time}", local.format("%a"))
    }
}

fn row_color(
    indicator: &TrayIndicator,
    provider: ProviderId,
    remaining: Option<u8>,
    appearance: Appearance,
) -> (Rgb, bool) {
    match indicator.color_mode {
        TrayColorMode::Status => (appearance.status(remaining), remaining.is_some()),
        TrayColorMode::Fixed => (
            [
                indicator.fixed_color.red,
                indicator.fixed_color.green,
                indicator.fixed_color.blue,
            ],
            true,
        ),
        TrayColorMode::Provider => (appearance.provider_tint(provider), true),
        TrayColorMode::Accent => (appearance.accent, true),
        TrayColorMode::Monochrome => (appearance.text(), false),
    }
}

fn icon_for(provider: ProviderId, appearance: Appearance) -> Icon {
    let descriptor = crate::provider_registry::descriptor(provider.kind());
    Icon {
        name: descriptor.icon,
        badge: provider.badge(),
        tint: if appearance.colored_icons {
            appearance.provider_tint(provider)
        } else {
            appearance.text()
        },
    }
}

fn resolve_row(
    indicator: &TrayIndicator,
    limits: &ProviderLimits,
    appearance: Appearance,
    now: DateTime<Utc>,
) -> Option<Row> {
    let provider = indicator.provider()?;
    let metric =
        crate::widget_data::resolve_metric(provider, limits.get(provider), &indicator.metric_id);
    let registered = crate::provider_registry::metric(provider.kind(), &indicator.metric_id);
    let full_label = metric
        .as_ref()
        .map(|metric| metric.label.clone())
        .or_else(|| registered.map(|metric| metric.localized_label().to_owned()))
        .unwrap_or_else(|| indicator.metric_id.clone());
    // Runtime names (a reserve standing in for the session) stay in full.
    let label = match registered {
        Some(metric) if full_label == metric.localized_label() => {
            metric.localized_short_label().to_owned()
        }
        _ => full_label.clone(),
    };
    let window = metric.map(|metric| metric.window).unwrap_or_default();
    let pace = window
        .pace_tip(indicator.limit_value == LimitValue::Used, now)
        .map(|tip| (tip.percent / 100.0) as f32);
    let remaining = window.remaining_percent();
    let displayed = match indicator.limit_value {
        LimitValue::Remaining => remaining,
        LimitValue::Used => window.used_percent.map(|used| used.min(100)),
    };
    let (color, tinted) = row_color(indicator, provider, remaining, appearance);
    Some(Row {
        provider,
        label,
        full_label,
        pace,
        value: displayed.map_or_else(|| "\u{2014}".to_owned(), |value| format!("{value}%")),
        progress: displayed.map(|value| f32::from(value) / 100.0),
        color,
        tinted,
        resets_at: window.resets_at,
        icon: icon_for(provider, appearance),
        limit_value: indicator.limit_value,
        reset: None,
    })
}

pub(crate) fn resolve(
    widget: &TaskbarWidget,
    limits: &ProviderLimits,
    appearance: Appearance,
) -> WidgetView {
    let now = Utc::now();
    let style = widget.style;
    let reset = if style.has_reset() {
        widget.reset
    } else {
        TaskbarResetDisplay::Off
    };
    let mut rows = widget
        .indicators
        .iter()
        .take(style.max_indicators())
        .filter_map(|indicator| resolve_row(indicator, limits, appearance, now))
        .collect::<Vec<_>>();
    for row in &mut rows {
        row.reset = row
            .resets_at
            .filter(|at| *at > now)
            .and_then(|at| match reset {
                TaskbarResetDisplay::Off => None,
                TaskbarResetDisplay::Countdown => Some(countdown(at, now)),
                TaskbarResetDisplay::Clock => Some(reset_clock(at)),
            });
    }
    let provider = widget.provider();
    let icon = widget
        .show_icon
        .then(|| provider.map(|provider| icon_for(provider, appearance)))
        .flatten();
    WidgetView {
        id: widget.id.clone(),
        style,
        icon,
        provider,
        rows,
        show_labels: widget.show_labels && style.has_labels(),
        // Text designs always say something: the percentage, unless the
        // reset time stands in for it.
        show_values: widget.show_values
            || (!style.has_graphic() && reset == TaskbarResetDisplay::Off),
        show_pace: widget.show_pace,
        card: card_sections(widget, limits, appearance, now),
    }
}

/// Every provider the widget reads, each with all of its quotas. Quotas the
/// widget shows keep its colors and remaining/used choice.
fn card_sections(
    widget: &TaskbarWidget,
    limits: &ProviderLimits,
    appearance: Appearance,
    now: DateTime<Utc>,
) -> Vec<CardSection> {
    let mut providers = Vec::new();
    for provider in widget.indicators.iter().filter_map(TrayIndicator::provider) {
        if !providers.contains(&provider) {
            providers.push(provider);
        }
    }
    providers
        .into_iter()
        .map(|provider| {
            let configured = widget
                .indicators
                .iter()
                .filter(|indicator| indicator.provider() == Some(provider))
                .collect::<Vec<_>>();
            let limit_value = configured
                .first()
                .map_or(LimitValue::Remaining, |indicator| indicator.limit_value);
            // A metric without data of its own can resolve to another one
            // (a reserve falling back to the session); list each quota once.
            let mut seen = Vec::new();
            let rows = crate::widget_data::snapshot(provider, limits)
                .metrics
                .into_iter()
                .filter(|metric| {
                    let resolved = crate::widget_data::resolve_metric(
                        provider,
                        limits.get(provider),
                        &metric.id,
                    )
                    .map_or_else(|| metric.id.clone(), |resolved| resolved.resolved_id);
                    let fresh = !seen.contains(&resolved) && !seen.contains(&metric.label);
                    seen.push(resolved);
                    seen.push(metric.label.clone());
                    fresh
                })
                .filter_map(|metric| {
                    let indicator = configured
                        .iter()
                        .find(|indicator| indicator.metric_id == metric.id)
                        .map(|indicator| (*indicator).clone())
                        .unwrap_or_else(|| TrayIndicator {
                            limit_value,
                            ..TrayIndicator::new(provider, metric.id)
                        });
                    resolve_row(&indicator, limits, appearance, now)
                })
                .collect();
            let provider_limits = limits.get(provider);
            CardSection {
                provider,
                icon: icon_for(provider, appearance),
                plan: provider_limits
                    .plan_type
                    .as_deref()
                    .filter(|plan| !plan.trim().is_empty())
                    .map(crate::popup_window::capitalize_plan_name),
                rows,
                today: today_usage(provider, &provider_limits.usage),
            }
        })
        .collect()
}

/// `1.2M tokens · ≈ $3.40`, or `$1.23 · 45 requests` for providers billed
/// per request.
fn today_usage(provider: ProviderId, usage: &crate::usage::UsageStatistics) -> Option<String> {
    if !usage.has_data() {
        return None;
    }
    let today = &usage.today;
    if crate::popup_window::ui::is_cost_provider(provider.kind()) {
        return Some(format!(
            "{} \u{00b7} {}",
            crate::popup_window::format_usd(today.estimated_cost_microusd as f64 / 1e6),
            crate::i18n::format("requests", &[("v0", today.requests.to_string())]),
        ));
    }
    let tokens = crate::i18n::format(
        "taskbar-card-tokens",
        &[(
            "count",
            crate::popup_window::format_token_count(today.total_tokens()),
        )],
    );
    Some(match today.estimated_api_value_usd() {
        Some(value) => format!(
            "{tokens} \u{00b7} \u{2248} {}",
            crate::popup_window::format_usd(value)
        ),
        None => tokens,
    })
}

/// Widgets that have at least one enabled instance, resolved in order.
pub(crate) fn resolve_visible(
    widgets: &[TaskbarWidget],
    enabled: &[ProviderId],
    limits: &ProviderLimits,
    appearance: Appearance,
) -> Vec<WidgetView> {
    widgets
        .iter()
        .filter(|widget| {
            widget
                .indicators
                .iter()
                .filter_map(TrayIndicator::provider)
                .any(|provider| enabled.contains(&provider))
        })
        .map(|widget| resolve(widget, limits, appearance))
        .filter(|view| !view.rows.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        limits::{LimitWindow, RateLimits},
        settings::ProviderKind,
    };

    fn appearance(light: bool) -> Appearance {
        Appearance {
            light,
            colored_icons: true,
            accent: [0, 120, 212],
        }
    }

    fn limits(session_used: u8, weekly_used: u8) -> ProviderLimits {
        let codex = ProviderId::primary(ProviderKind::Codex);
        ProviderLimits::from_entries([(
            codex,
            RateLimits {
                primary: LimitWindow {
                    used_percent: Some(session_used),
                    duration_minutes: Some(300),
                    resets_at: Some(Utc::now() + chrono::Duration::minutes(133)),
                },
                secondary: LimitWindow {
                    used_percent: Some(weekly_used),
                    duration_minutes: Some(10_080),
                    resets_at: Some(Utc::now() + chrono::Duration::days(3)),
                },
                ..RateLimits::default()
            },
        )])
    }

    #[test]
    fn preset_resolves_remaining_percentages_with_status_colors() {
        let widget = TaskbarWidget::for_provider(ProviderId::primary(ProviderKind::Codex));
        let view = resolve(&widget, &limits(58, 95), appearance(false));
        assert_eq!(view.rows.len(), 2);
        assert_eq!(view.rows[0].value, "42%");
        assert_eq!(view.rows[0].progress, Some(0.42));
        assert_eq!(view.rows[0].color, [252, 225, 0]);
        assert_eq!(view.rows[1].value, "5%");
        assert_eq!(view.rows[1].color, [255, 153, 164]);
        assert!(view.icon.is_some());
    }

    #[test]
    fn used_values_and_light_theme_colors() {
        let mut widget = TaskbarWidget::for_provider(ProviderId::primary(ProviderKind::Codex));
        widget.indicators[0].limit_value = LimitValue::Used;
        let view = resolve(&widget, &limits(10, 0), appearance(true));
        assert_eq!(view.rows[0].value, "10%");
        // Status severity still follows what is left.
        assert_eq!(view.rows[0].color, [15, 123, 15]);
    }

    #[test]
    fn missing_data_shows_a_dash_in_the_text_color() {
        let widget = TaskbarWidget::for_provider(ProviderId::primary(ProviderKind::Codex));
        let view = resolve(&widget, &ProviderLimits::default(), appearance(false));
        assert_eq!(view.rows[0].value, "\u{2014}");
        assert_eq!(view.rows[0].progress, None);
        assert!(!view.rows[0].tinted);
    }

    #[test]
    fn style_caps_the_number_of_rows() {
        let mut widget = TaskbarWidget::for_provider(ProviderId::primary(ProviderKind::Codex));
        let provider = ProviderId::primary(ProviderKind::Codex);
        widget.indicators = vec![
            TrayIndicator::new(provider, "codex.session"),
            TrayIndicator::new(provider, "codex.weekly"),
            TrayIndicator::new(provider, "codex.session"),
        ];
        widget.style = TaskbarWidgetStyle::Glance;
        assert_eq!(
            resolve(&widget, &limits(0, 0), appearance(false))
                .rows
                .len(),
            2
        );
        widget.style = TaskbarWidgetStyle::Rings;
        assert_eq!(
            resolve(&widget, &limits(0, 0), appearance(false))
                .rows
                .len(),
            3
        );
    }

    #[test]
    fn hidden_icon_and_disabled_instances() {
        let provider = ProviderId::primary(ProviderKind::Codex);
        let mut widget = TaskbarWidget::for_provider(provider);
        widget.show_icon = false;
        assert!(
            resolve(&widget, &limits(0, 0), appearance(false))
                .icon
                .is_none()
        );
        let widgets = vec![widget];
        assert!(resolve_visible(&widgets, &[], &limits(0, 0), appearance(false)).is_empty());
        assert_eq!(
            resolve_visible(&widgets, &[provider], &limits(0, 0), appearance(false)).len(),
            1
        );
    }

    #[test]
    fn standard_windows_get_short_labels_and_a_pace() {
        let widget = TaskbarWidget::for_provider(ProviderId::primary(ProviderKind::Codex));
        let view = resolve(&widget, &limits(58, 5), appearance(false));
        assert_eq!(view.rows[0].label, crate::i18n::tr("short-5h"));
        assert_eq!(view.rows[1].label, crate::i18n::tr("short-7d"));
        assert_eq!(view.rows[0].full_label, crate::i18n::tr("msg-5h-session"));
        // 167 of 300 minutes gone: an even burn leaves about 44 % now.
        let pace = view.rows[0].pace.expect("the session has a pace");
        assert!((pace - 0.443).abs() < 0.01, "{pace}");
        assert!(view.show_pace);
    }

    #[test]
    fn reset_time_follows_the_widget_setting() {
        let provider = ProviderId::primary(ProviderKind::Codex);
        let mut widget = TaskbarWidget::for_provider(provider);
        let view = resolve(&widget, &limits(10, 0), appearance(false));
        assert!(
            view.rows[0]
                .reset
                .as_deref()
                .is_some_and(|left| left.starts_with("2h"))
        );
        widget.reset = TaskbarResetDisplay::Clock;
        let view = resolve(&widget, &limits(10, 0), appearance(false));
        assert!(
            view.rows[0]
                .reset
                .as_deref()
                .is_some_and(|clock| clock.contains(':'))
        );
        widget.reset = TaskbarResetDisplay::Off;
        widget.show_values = false;
        let view = resolve(&widget, &limits(10, 0), appearance(false));
        assert!(view.rows[0].reset.is_none());
        // A Glance with nothing else to say keeps its percentages.
        assert!(view.show_values);
        widget.style = TaskbarWidgetStyle::Rings;
        let view = resolve(&widget, &limits(10, 0), appearance(false));
        assert!(!view.show_values);
        assert!(view.has_text(), "labels still show");
        widget.show_labels = false;
        let view = resolve(&widget, &limits(10, 0), appearance(false));
        assert!(!view.has_text());
    }

    #[test]
    fn hover_card_shows_every_quota_and_today() {
        let codex = ProviderId::primary(ProviderKind::Codex);
        let mut widget = TaskbarWidget::for_provider(codex);
        // The widget shows only the weekly quota, used.
        widget.indicators = vec![TrayIndicator {
            limit_value: LimitValue::Used,
            ..TrayIndicator::new(codex, "codex.weekly")
        }];
        let mut limits = limits(58, 5);
        let usage = crate::usage::TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 200_000,
            requests: 12,
            ..Default::default()
        };
        limits.get_mut(codex).usage = crate::usage::UsageStatistics {
            today: usage.clone(),
            history: usage,
            ..Default::default()
        };
        let view = resolve(&widget, &limits, appearance(false));
        assert_eq!(view.rows.len(), 1);
        let [section] = view.card.as_slice() else {
            panic!("one provider");
        };
        assert!(section.rows.len() >= 2, "session and weekly");
        let mut labels = section
            .rows
            .iter()
            .map(|row| row.full_label.clone())
            .collect::<Vec<_>>();
        labels.dedup();
        assert_eq!(
            labels.len(),
            section.rows.len(),
            "each quota once: {labels:?}"
        );
        let weekly = section
            .rows
            .iter()
            .find(|row| row.label == crate::i18n::tr("short-7d"))
            .expect("weekly row");
        assert_eq!(weekly.value, "5%", "keeps the widget's used choice");
        let session = &section.rows[0];
        assert_eq!(session.value, "58%", "the others follow it");
        assert!(
            section
                .today
                .as_deref()
                .is_some_and(|today| today.contains("1.2M"))
        );
    }

    #[test]
    fn retired_compact_style_reads_as_glance() {
        let widget: TaskbarWidget = toml::from_str("style = \"compact\"").unwrap();
        assert_eq!(widget.style, TaskbarWidgetStyle::Glance);
    }

    #[test]
    fn countdown_is_compact() {
        let now = Utc::now();
        assert_eq!(countdown(now + chrono::Duration::minutes(45), now), "45m");
        assert_eq!(
            countdown(now + chrono::Duration::minutes(133), now),
            "2h 13m"
        );
        assert_eq!(
            countdown(now + chrono::Duration::minutes(3 * 1440 + 240), now),
            "3d 4h"
        );
    }
}
