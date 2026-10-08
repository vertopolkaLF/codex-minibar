//! General: startup, Usage Stats and the reset feed.

use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px, relative};

use super::kit::{self, Kit, Row};
use super::window::SettingsWindow;
use crate::settings::{ProviderKind, ResetAnnouncementRefreshInterval, UsageRefreshInterval};

pub(super) fn usage_refresh_labels() -> [&'static str; 7] {
    [
        crate::i18n::tr("msg-1-minute"),
        crate::i18n::tr("msg-5-minutes"),
        crate::i18n::tr("msg-10-minutes"),
        crate::i18n::tr("msg-15-minutes"),
        crate::i18n::tr("msg-30-minutes"),
        crate::i18n::tr("msg-45-minutes"),
        crate::i18n::tr("msg-60-minutes"),
    ]
}

impl SettingsWindow {
    pub(super) fn general_page(
        &mut self,
        k: &mut Kit,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let s = &self.settings;
        let start_at_login = s.start_at_login;
        let (usage_enabled, usage_interval) = (s.usage_stats_enabled, s.usage_refresh_interval);
        let feed = s.notifications.forced_reset_feed_enabled;
        let feed_toasts = s.notifications.forced_reset_notifications;
        let feed_interval = s.reset_announcement_refresh_interval;

        let language = kit::card_of(k, |k| {
            vec![kit::dropdown_row(
                k,
                "general-language",
                crate::i18n::tr("language"),
                Some(crate::i18n::tr(
                    "applies-immediately-to-every-app-window-and-notification-auto-fol",
                )),
                kit::options(&crate::i18n::Language::labels()),
                s.language.index() as i32,
                false,
                Self::h(cx, |this, index: usize, _, cx| {
                    let language = crate::i18n::Language::from_index(index);
                    this.edit(cx, move |settings| settings.language = language);
                }),
            )]
        });

        let startup = kit::card_of(k, |k| {
            vec![kit::toggle_row(
                k,
                "general-startup",
                crate::i18n::tr("start-with-windows"),
                Some(crate::i18n::tr("open-codex-minibar-in-the-tray-when-you-sign-in").into()),
                start_at_login,
                Self::h(cx, |this, value: bool, _, cx| {
                    this.edit(cx, move |settings| settings.start_at_login = value)
                }),
            )]
        });

        let usage = kit::card_of(k, |k| {
            vec![
                kit::toggle_row(
                    k,
                    "general-usage-stats",
                    crate::i18n::tr("enable-usage-stats"),
                    Some(
                        crate::i18n::tr(
                            "scan-local-provider-history-for-the-usage-tab-and-cost-totals",
                        )
                        .into(),
                    ),
                    usage_enabled,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |settings| settings.usage_stats_enabled = value)
                    }),
                ),
                self.usage_providers(k, usage_enabled, cx),
                kit::dropdown_row(
                    k,
                    "general-usage-refresh",
                    crate::i18n::tr("collection-period"),
                    Some(crate::i18n::tr(
                        "how-often-local-provider-history-is-scanned",
                    )),
                    kit::options(&usage_refresh_labels()),
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
                    crate::i18n::tr("check-for-confirmed-tibo-resets"),
                    Some(
                        crate::i18n::tr(
                            "reads-the-app-s-public-github-feed-and-keeps-the-latest-announcem",
                        )
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
                    crate::i18n::tr("notify-when-new-reset-info-arrives"),
                    Some(
                        crate::i18n::tr(
                            "shows-a-notification-when-the-feed-reports-a-possible-reset-never",
                        )
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
                    crate::i18n::tr("check-every"),
                    Some(crate::i18n::tr(
                        "the-feed-is-also-checked-immediately-when-the-app-starts-or-this",
                    )),
                    kit::options(&[
                        crate::i18n::tr("msg-15-minutes"),
                        crate::i18n::tr("msg-30-minutes"),
                        crate::i18n::tr("msg-1-hour"),
                        crate::i18n::tr("msg-3-hours"),
                        crate::i18n::tr("msg-6-hours"),
                        crate::i18n::tr("msg-12-hours"),
                        crate::i18n::tr("msg-24-hours"),
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
            language,
            startup,
            kit::section_heading(k, crate::i18n::tr("usage-stats")),
            usage,
            kit::section_heading(k, crate::i18n::tr("tibo-resets")),
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
                crate::i18n::tr("enable-a-provider-in-the-providers-tab-to-include-it-here"),
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
                .then_some(crate::i18n::tr("add-a-management-key"))
            })
            .or_else(|| {
                (!instance.usage_stats).then_some(crate::i18n::tr(
                    "usage-statistics-are-off-on-this-provider-s-page",
                ))
            });
            let checked = reason.is_none() && instance.in_usage_overview;
            let check = kit::checkbox(
                k,
                format!("usage-provider-{}", provider.id()),
                checked,
                !usage_enabled || reason.is_some(),
                Some(provider.qualified_name().into()),
                Self::h(cx, move |this, checked: bool, _, cx| {
                    this.edit(cx, move |settings| {
                        settings.set_usage_overview_included(provider, checked)
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
        Row::new(
            "general-usage-providers",
            crate::i18n::tr("included-providers"),
        )
        .description(
            k,
            crate::i18n::tr("choose-which-accounts-count-toward-this-machine-s-usage-tab-and-h"),
        )
        .detail(div().pt(px(10.0)).child(grid).into_any_element())
        .disabled(!usage_enabled)
        .render(k)
    }
}
