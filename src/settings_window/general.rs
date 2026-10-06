//! General: startup, refresh cadence, Usage Stats and the reset feed.

use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px, relative};

use super::kit::{self, Kit, Row};
use super::window::SettingsWindow;
use crate::settings::{
    LimitRefreshInterval, ProviderKind, ResetAnnouncementRefreshInterval, UsageRefreshInterval,
};

pub(super) const LIMIT_REFRESH_LABELS: [&str; 5] = [
    "30 seconds",
    "1 minute",
    "5 minutes",
    "10 minutes",
    "15 minutes",
];

pub(super) const USAGE_REFRESH_LABELS: [&str; 7] = [
    "1 minute",
    "5 minutes",
    "10 minutes",
    "15 minutes",
    "30 minutes",
    "45 minutes",
    "60 minutes",
];

impl SettingsWindow {
    pub(super) fn general_page(
        &mut self,
        k: &mut Kit,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let s = &self.settings;
        let (start_at_login, limit_interval) = (s.start_at_login, s.limit_refresh_interval);
        let (usage_enabled, usage_interval) = (s.usage_stats_enabled, s.usage_refresh_interval);
        let feed = s.notifications.forced_reset_feed_enabled;
        let feed_toasts = s.notifications.forced_reset_notifications;
        let feed_interval = s.reset_announcement_refresh_interval;

        let startup = kit::card_of(k, |k| {
            vec![
                kit::toggle_row(
                    k,
                    "general-startup",
                    "Start with Windows",
                    Some("Open Codex Minibar in the tray when you sign in.".into()),
                    start_at_login,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |settings| settings.start_at_login = value)
                    }),
                ),
                kit::dropdown_row(
                    k,
                    "general-limit-refresh",
                    "Refresh limits",
                    Some("How often provider quotas are read."),
                    kit::options(&LIMIT_REFRESH_LABELS),
                    limit_interval.index(),
                    false,
                    Self::h(cx, |this, index: usize, _, cx| {
                        let value = LimitRefreshInterval::from_index(index as i32);
                        this.edit(cx, move |settings| settings.limit_refresh_interval = value)
                    }),
                ),
            ]
        });

        let usage = kit::card_of(k, |k| {
            vec![
                kit::toggle_row(
                    k,
                    "general-usage-stats",
                    "Enable Usage Stats",
                    Some("Scan local provider history for the Usage tab and cost totals.".into()),
                    usage_enabled,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |settings| settings.usage_stats_enabled = value)
                    }),
                ),
                self.usage_providers(k, usage_enabled, cx),
                kit::dropdown_row(
                    k,
                    "general-usage-refresh",
                    "Collection period",
                    Some("How often local provider history is scanned."),
                    kit::options(&USAGE_REFRESH_LABELS),
                    usage_interval.index(),
                    !usage_enabled,
                    Self::h(cx, |this, index: usize, _, cx| {
                        let value = UsageRefreshInterval::from_index(index as i32);
                        this.edit(cx, move |settings| settings.usage_refresh_interval = value)
                    }),
                ),
            ]
        });

        let resets = kit::card_of(k, |k| {
            vec![
                kit::toggle_row(
                    k,
                    "general-tibo-feed",
                    "Check for confirmed Tibo resets",
                    Some(
                        "Reads the app's public GitHub feed and keeps the latest announcement cached."
                            .into(),
                    ),
                    feed,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |settings| {
                            settings.notifications.forced_reset_feed_enabled = value
                        })
                    }),
                ),
                kit::toggle_row(
                    k,
                    "general-tibo-toast",
                    "Notify when new reset info arrives",
                    Some(
                        "Shows a notification when the feed reports a possible reset, never at the reset time."
                            .into(),
                    ),
                    feed_toasts,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |settings| {
                            settings.notifications.forced_reset_notifications = value
                        })
                    }),
                ),
                kit::dropdown_row(
                    k,
                    "general-tibo-interval",
                    "Check every",
                    Some(
                        "The feed is also checked immediately when the app starts or this option is enabled.",
                    ),
                    kit::options(&[
                        "15 minutes",
                        "30 minutes",
                        "1 hour",
                        "3 hours",
                        "6 hours",
                        "12 hours",
                        "24 hours",
                    ]),
                    feed_interval.index(),
                    !feed,
                    Self::h(cx, |this, index: usize, _, cx| {
                        let value = ResetAnnouncementRefreshInterval::from_index(index as i32);
                        this.edit(cx, move |settings| {
                            settings.reset_announcement_refresh_interval = value
                        })
                    }),
                ),
            ]
        });

        vec![
            startup,
            kit::section_heading(k, "Usage Stats"),
            usage,
            kit::section_heading(k, "Tibo Resets™"),
            resets,
        ]
    }

    /// Per-instance Usage Stats switches for enabled instances whose driver
    /// has local history. Unavailable instances stay listed, disabled, with why.
    fn usage_providers(
        &self,
        k: &mut Kit,
        usage_enabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let available = self
            .settings
            .instances
            .iter()
            .filter(|instance| {
                instance.enabled && crate::provider_registry::supports_usage_stats(instance.driver)
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut grid = div().flex().flex_wrap().w_full().gap_y(px(10.0));
        if available.is_empty() {
            grid = grid.child(kit::caption(
                k,
                "Enable a provider in the Providers tab to include it here.",
            ));
        }
        for instance in &available {
            let provider = instance.provider_id();
            let reason = crate::instances::Capabilities::reason(
                instance,
                crate::instances::Capability::UsageStats,
            )
            .or_else(|| {
                (instance.driver == ProviderKind::OpenRouter
                    && !crate::openrouter::has_management_key(instance.openrouter.as_slice()))
                .then_some("Add a management key")
            });
            let checked = reason.is_none() && instance.usage_stats;
            let check = kit::checkbox(
                k,
                format!("usage-provider-{}", provider.id()),
                checked,
                !usage_enabled || reason.is_some(),
                Some(provider.qualified_name().into()),
                Self::h(cx, move |this, checked: bool, _, cx| {
                    this.edit(cx, move |settings| {
                        settings.set_usage_stats_provider_enabled(provider, checked)
                    })
                }),
            );
            let check = match reason {
                Some(reason) => {
                    kit::tooltip_host(k, format!("usage-tip-{}", provider.id()), reason, check)
                }
                None => check,
            };
            grid = grid.child(div().w(relative(1.0 / 3.0)).pr(px(12.0)).child(check));
        }
        Row::new("general-usage-providers", "Included providers")
            .description(
                k,
                "Choose which providers contribute to the Usage tab and Home card. Provider pages keep their usage card, and existing data is never deleted.",
            )
            .detail(div().pt(px(10.0)).child(grid).into_any_element())
            .disabled(!usage_enabled)
            .render(k)
    }
}
