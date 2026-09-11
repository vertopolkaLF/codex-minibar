use super::*;
use gpui::{prelude::*, *};
use crate::{
    limits::LimitWindow,
    provider_registry as registry,
    settings::{
        PopupSurface, PopupWidgetKind, ProviderKind, TotalSpendPeriod, TotalSpendPresentation,
    },
    usage_overview::{self, BreakdownMode, OverviewMetric, OverviewRange},
};
use gpui_component::{tooltip::Tooltip, ActiveTheme};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Home,
    Usage,
    Provider(ProviderKind),
}

pub(super) struct PopupView {
    model: Entity<Model>,
    tab: Tab,
    range: OverviewRange,
    metric: OverviewMetric,
    breakdown: BreakdownMode,
    target_height: f32,
    measured_height: std::rc::Rc<std::cell::Cell<f32>>,
    motion: bool,
    _subscription: Subscription,
    _activation: Subscription,
}

impl PopupView {
    pub fn new(model: Entity<Model>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&model, |this, _, cx| {
            let settings = &this.model.read(cx).snapshot.settings;
            if matches!(this.tab, Tab::Provider(provider) if !settings.providers.is_enabled(provider))
                || (this.tab == Tab::Usage && !settings.usage_stats_enabled)
            {
                this.tab = Tab::Home;
            }
            cx.notify();
        });
        window.on_window_should_close(cx, |window, _| {
            crate::popup::hide(window);
            false
        });
        let activation = cx.observe_window_activation(window, |_, window, _| {
            if !window.is_window_active() {
                crate::popup::hide(window);
            }
        });
        Self {
            model,
            tab: Tab::Home,
            range: OverviewRange::ThirtyDays,
            metric: OverviewMetric::Cost,
            breakdown: BreakdownMode::Model,
            target_height: 520.0,
            measured_height: std::rc::Rc::new(std::cell::Cell::new(0.0)),
            motion: false,
            _subscription: subscription,
            _activation: activation,
        }
    }

    fn select(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.tab = tab;
        cx.notify();
    }

    fn resize(&mut self, height: f32, window: &mut Window, cx: &mut Context<Self>) {
        self.target_height = height.clamp(100.0, crate::popup::max_height(window));
        if !crate::theme::animations_enabled() || !crate::popup::visible(window) {
            crate::popup::resize_pinned(window, self.target_height);
            return;
        }
        if !self.motion {
            self.motion = true;
            self.step_resize(window, cx);
        }
    }

    fn step_resize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current: f32 = window.bounds().size.height.into();
        let delta = self.target_height - current;
        let next = if delta.abs() < 1.0 {
            self.motion = false;
            self.target_height
        } else {
            current + delta * 0.25
        };
        crate::popup::resize_pinned(window, next);
        if self.motion {
            cx.on_next_frame(window, |this, window, cx| this.step_resize(window, cx));
        }
    }

    fn icon_path(&self, provider: ProviderKind, cx: &App) -> SharedString {
        let settings = &self.model.read(cx).snapshot.settings;
        if provider == ProviderKind::Codex && !settings.replace_chatgpt_logo_with_codex {
            "chatgpt".into()
        } else {
            provider.id().into()
        }
    }

    fn tab_button(
        &self,
        id: SharedString,
        icon: SharedString,
        label: &'static str,
        tab: Tab,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.tab == tab;
        let settings = &self.model.read(cx).snapshot.settings;
        let color = if selected {
            cx.theme().primary
        } else if settings.use_colored_provider_icons {
            if let Tab::Provider(provider) = tab {
                let (r, g, b) = registry::descriptor(provider).brand_rgb;
                rgb((r as u32) << 16 | (g as u32) << 8 | b as u32).into()
            } else {
                cx.theme().muted_foreground
            }
        } else {
            cx.theme().muted_foreground
        };
        let size = settings.bottom_bar_size.icon_button_size() as f32;
        let glyph = settings.bottom_bar_size.icon_glyph_size() as f32;
        div()
            .id(id)
            .flex_none()
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .size(px(size))
            .rounded_md()
            .cursor_pointer()
            .hover(|style| style.bg(cx.theme().secondary))
            .text_color(color)
            .child(svg().path(icon).size(px(glyph)))
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left(px(9.))
                    .right(px(9.))
                    .h(px(2.))
                    .bg(color)
                    .opacity(if selected { 1. } else { 0. }),
            )
            .tooltip(move |window, cx| Tooltip::new(label).build(window, cx))
            .on_click(cx.listener(move |this, _, _, cx| this.select(tab, cx)))
    }

    fn provider_card(&self, provider: ProviderKind, snapshot: &Snapshot, cx: &App) -> AnyElement {
        let settings = &snapshot.settings;
        let limits = snapshot.limits.get(provider);
        let surface = if self.tab == Tab::Home {
            PopupSurface::HomeTab
        } else {
            PopupSurface::ProviderTab
        };
        let show = |id: &str| settings.popup_visibility.is_visible(id, surface, true);
        let mut content = div().flex().flex_col().gap_2().child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    svg()
                        .path(self.icon_path(provider, cx))
                        .size(px(18.))
                        .text_color(if settings.use_colored_provider_icons {
                            let (r, g, b) = registry::descriptor(provider).brand_rgb;
                            rgb((r as u32) << 16 | (g as u32) << 8 | b as u32).into()
                        } else {
                            cx.theme().foreground
                        }),
                )
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(provider.display_name()),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(cx.theme().muted_foreground)
                        .child(limits.plan_type.clone().unwrap_or_default()),
                ),
        );
        if settings.show_account_name {
            if let Some(name) = &limits.account_name {
                content = content.child(div().text_size(px(11.)).child(name.clone()));
            }
        }
        if let Some(error) = snapshot.errors.get(&provider) {
            content = content.child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(0xe16d73))
                    .child(error.clone()),
            );
        }
        for metric in registry::descriptor(provider).metrics {
            if show(metric.id) {
                if let Some(window) = registry::metric_window(provider, limits, metric.id) {
                    if !window.is_empty() {
                        content = content.child(quota(
                            &registry::metric_label(provider, limits, metric.id),
                            window,
                            settings,
                            cx,
                        ));
                    }
                }
            }
        }
        for extra in &limits.additional_limits {
            if registry::descriptor(provider).metrics.iter().any(|metric| {
                matches!(metric.source, registry::MetricSource::Additional(id) if id == extra.id)
            }) {
                continue;
            }
            if show(&registry::additional_limit_brick_id(provider, &extra.id)) {
                content = content.child(quota(&extra.title, &extra.window, settings, cx));
            }
        }
        if show(&registry::credits_brick_id(provider))
            && (limits.credits.has_credits || limits.credits.unlimited)
        {
            content = content.child(info_card(
                "Credits",
                if limits.credits.unlimited {
                    "Unlimited".into()
                } else {
                    limits
                        .credits
                        .balance
                        .clone()
                        .unwrap_or_else(|| "Unavailable".into())
                },
                cx,
            ));
        }
        if show(&registry::resets_brick_id(provider)) && limits.reset_credits.is_some() {
            content = content.child(info_card(
                "Banked resets",
                limits.available_reset_count().to_string(),
                cx,
            ));
        }
        if show(&registry::spending_brick_id(provider)) {
            if let Some(spend) = &limits.spending {
                content = content.child(info_card("Spent", money(spend.used_microusd), cx));
            }
        }
        for account in &limits.openrouter_accounts {
            let mut account_ui = card(cx).child(account.name.clone());
            if let Some(balance) = account.balance_microusd {
                account_ui = account_ui.child(info_card("Balance", money(balance), cx));
            }
            for key in &account.api_keys {
                account_ui = account_ui.child(info_card(
                    &key.label.clone().unwrap_or_else(|| "API key".into()),
                    money(key.spending.used_microusd),
                    cx,
                ));
            }
            content = content.child(account_ui);
        }
        if settings.usage_stats_enabled && show(&registry::usage_brick_id(provider)) {
            content = content.child(info_card(
                "Estimated API value",
                money(limits.usage.history.estimated_cost_microusd),
                cx,
            ));
        }
        if let Some(error) = snapshot.usage_errors.get(&provider) {
            content = content.child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(0xe16d73))
                    .child(error.clone()),
            );
        }
        content.into_any_element()
    }

    fn usage_stats_card(&self, snapshot: &Snapshot, cx: &mut Context<Self>) -> AnyElement {
        let enabled = enabled(snapshot);
        let overview = usage_overview::total_spend_snapshot(
            &snapshot.limits,
            &enabled,
            snapshot.settings.total_spend_period,
        );
        let entries = usage_overview::spend_entries(&overview);
        let mut periods = div().flex().gap_1();
        for period in [
            TotalSpendPeriod::Today,
            TotalSpendPeriod::Yesterday,
            TotalSpendPeriod::ThirtyDays,
        ] {
            let selected = snapshot.settings.total_spend_period == period;
            periods = periods.child(
                div()
                    .id(period.label())
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(if selected {
                        cx.theme().secondary
                    } else {
                        cx.theme().background
                    })
                    .child(period.label())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.model.update(cx, |model, cx| {
                            model.edit(|settings| settings.total_spend_period = period, cx)
                        });
                    })),
            );
        }
        let mut card = card(cx)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Usage Stats"),
                    )
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(money(
                        overview.totals.estimated_cost_microusd,
                    ))),
            )
            .child(periods);
        let total = entries.iter().map(|(_, value)| *value).sum::<u64>().max(1);
        if snapshot.settings.total_spend_presentation == TotalSpendPresentation::Donut {
            let mut bar = div().flex().h(px(10.)).w_full().rounded_full().overflow_hidden();
            for (provider, value) in &entries {
                let (r, g, b) = registry::descriptor(*provider).brand_rgb;
                bar = bar.child(
                    div()
                        .h_full()
                        .w(relative(*value as f32 / total as f32))
                        .bg(rgb((r as u32) << 16 | (g as u32) << 8 | b as u32)),
                );
            }
            card = card.child(bar);
        }
        for (provider, value) in entries {
            let (r, g, b) = registry::descriptor(provider).brand_rgb;
            card = card.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .size(px(8.))
                            .rounded_full()
                            .bg(rgb((r as u32) << 16 | (g as u32) << 8 | b as u32)),
                    )
                    .child(div().flex_1().child(provider.display_name()))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(money(value))),
            );
        }
        card.into_any_element()
    }

    fn usage(&self, snapshot: &Snapshot, cx: &mut Context<Self>) -> AnyElement {
        let enabled = enabled(snapshot);
        let overview =
            usage_overview::build_overview_snapshot(&snapshot.limits, &enabled, self.metric, self.range);
        let mut ranges = div().flex().gap_1();
        for range in [
            OverviewRange::Past24h,
            OverviewRange::SevenDays,
            OverviewRange::ThirtyDays,
            OverviewRange::NinetyDays,
        ] {
            ranges = ranges.child(
                div()
                    .id(range.label())
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(if range == self.range {
                        cx.theme().secondary
                    } else {
                        cx.theme().background
                    })
                    .child(range.label())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.range = range;
                        cx.notify();
                    })),
            );
        }
        let mut chart = div()
            .flex()
            .items_end()
            .gap(px(2.))
            .h(px(110.))
            .w_full();
        let max = overview
            .daily_series
            .iter()
            .map(|point| point.total)
            .max()
            .unwrap_or(1)
            .max(1) as f32;
        for point in &overview.daily_series {
            chart = chart.child(
                div()
                    .flex_1()
                    .min_w(px(1.))
                    .h(px(2. + point.total as f32 / max * 100.))
                    .rounded_t_sm()
                    .bg(cx.theme().primary),
            );
        }
        let mut result = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Usage"),
            )
            .child(ranges)
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .id("cost")
                            .cursor_pointer()
                            .font_weight(if self.metric == OverviewMetric::Cost {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .child("Cost")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.metric = OverviewMetric::Cost;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("tokens")
                            .cursor_pointer()
                            .font_weight(if self.metric == OverviewMetric::Tokens {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .child("Tokens")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.metric = OverviewMetric::Tokens;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                card(cx)
                    .child(div().text_2xl().child(if self.metric == OverviewMetric::Cost {
                        money(overview.totals.estimated_cost_microusd)
                    } else {
                        overview.totals.total_tokens().to_string()
                    }))
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(cx.theme().muted_foreground)
                            .child("Estimated API value · local session history"),
                    )
                    .child(chart),
            );
        for row in &overview.providers {
            result = result.child(info_card(
                row.provider.display_name(),
                if self.metric == OverviewMetric::Cost {
                    money(row.usage.estimated_cost_microusd)
                } else {
                    row.usage.total_tokens().to_string()
                },
                cx,
            ));
        }
        result = result.child(
            div()
                .flex()
                .gap_3()
                .child(
                    div()
                        .id("by-model")
                        .cursor_pointer()
                        .child("By model")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.breakdown = BreakdownMode::Model;
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .id("by-day")
                        .cursor_pointer()
                        .child("By day")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.breakdown = BreakdownMode::Day;
                            cx.notify();
                        })),
                ),
        );
        let rows = if self.breakdown == BreakdownMode::Model {
            &overview.model_rows
        } else {
            &overview.day_rows
        };
        for row in rows {
            result = result.child(info_card(
                &row.label,
                if self.metric == OverviewMetric::Cost {
                    money(row.cost_microusd)
                } else {
                    row.tokens.to_string()
                },
                cx,
            ));
        }
        result.into_any_element()
    }
}

impl Render for PopupView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snapshot = self.model.read(cx).snapshot.clone();
        let mut body = div().flex().flex_col().gap_4().p_3();
        if let Some(error) = &snapshot.error {
            body = body.child(div().text_color(rgb(0xe16d73)).child(error.clone()));
        }
        match self.tab {
            Tab::Usage => body = body.child(self.usage(&snapshot, cx)),
            Tab::Provider(provider) => {
                body = body.child(self.provider_card(provider, &snapshot, cx))
            }
            Tab::Home => {
                for widget in &snapshot.settings.popup_order {
                    match widget {
                        PopupWidgetKind::TotalSpend
                            if snapshot.settings.usage_stats_enabled
                                && snapshot.settings.show_total_spend_on_all_tab =>
                        {
                            body = body.child(self.usage_stats_card(&snapshot, cx));
                        }
                        other => {
                            if let Some(provider) = other.as_provider() {
                                if snapshot.settings.providers.is_enabled(provider)
                                    && snapshot.settings.popup_visibility.provider_shown_on_all(provider)
                                {
                                    body = body.child(self.provider_card(provider, &snapshot, cx));
                                }
                            }
                        }
                    }
                }
                if enabled(&snapshot).is_empty() {
                    let model = self.model.clone();
                    body = body
                        .child(div().py_8().child("Enable a provider to see your limits."))
                        .child(
                            div()
                                .id("enable-providers")
                                .cursor_pointer()
                                .text_color(cx.theme().primary)
                                .child("Open Settings")
                                .on_click(move |_, _, cx| super::open_settings(model.clone(), cx)),
                        );
                }
            }
        }
        let mut tabs = div().flex().items_center().gap(px(2.));
        tabs = tabs.child(self.tab_button("home".into(), "home".into(), "Home", Tab::Home, cx));
        if snapshot.settings.usage_stats_enabled {
            tabs = tabs.child(self.tab_button(
                "usage".into(),
                "usage".into(),
                "Usage",
                Tab::Usage,
                cx,
            ));
        }
        for provider in enabled(&snapshot) {
            tabs = tabs.child(self.tab_button(
                provider.id().into(),
                self.icon_path(provider, cx),
                provider.display_name(),
                Tab::Provider(provider),
                cx,
            ));
        }
        let model = self.model.clone();
        let refresh = self.model.clone();
        let action_size = snapshot.settings.bottom_bar_size.icon_button_size() as f32;
        let glyph = snapshot.settings.bottom_bar_size.icon_glyph_size() as f32;
        let mut footer = div()
            .flex()
            .items_center()
            .gap(px(2.))
            .flex_none()
            .child(
                div()
                    .id("refresh")
                    .size(px(action_size))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .tooltip(|window, cx| Tooltip::new("Refresh").build(window, cx))
                    .child(svg().path("refresh").size(px(glyph)))
                    .on_click(move |_, _, cx| refresh.read(cx).runtime.send(Command::Refresh)),
            )
            .child(
                div()
                    .id("settings")
                    .size(px(action_size))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .tooltip(|window, cx| Tooltip::new("Settings").build(window, cx))
                    .child(svg().path("settings").size(px(glyph)))
                    .on_click(move |_, _, cx| super::open_settings(model.clone(), cx)),
            );
        if matches!(snapshot.update, crate::updater::UpdatePhase::Available(_)) {
            footer = footer.child(
                div()
                    .id("update")
                    .px_2()
                    .cursor_pointer()
                    .text_color(cx.theme().primary)
                    .child("Update")
                    .on_click(|_, _, _| {
                        if let Err(error) = crate::updater::apply_pending_update() {
                            crate::notifications::show("Update failed", &error.to_string());
                        }
                    }),
            );
        }
        let footer_height = snapshot.settings.bottom_bar_size.footer_height_dip() as f32;
        let measured = self.measured_height.clone();
        let view = cx.weak_entity();
        body = body.relative().child(
            canvas(
                move |bounds, window, cx| {
                    let height: f32 = bounds.size.height.into();
                    if (measured.get() - height).abs() > 0.5 {
                        measured.set(height);
                        let view = view.clone();
                        window.defer(cx, move |window, cx| {
                            let _ = view.update(cx, |this, cx| {
                                this.resize(height + footer_height, window, cx)
                            });
                        });
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        );
        crate::popup::appearance(window, &snapshot.settings, cx.theme().is_dark());
        let mut background = cx.theme().background;
        background.a = 0.86;
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(background)
            .text_color(cx.theme().foreground)
            .text_size(px(12.))
            .rounded(px(snapshot.settings.popup_corner_radius.dip() as f32))
            .overflow_hidden()
            .on_key_down(|event, window, _| {
                if event.keystroke.key == "escape" {
                    crate::popup::hide(window);
                }
            })
            .child(
                div()
                    .id("popup-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(body),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(px(footer_height))
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .px_3()
                    .child(
                        div()
                            .id("provider-tabs")
                            .flex_1()
                            .min_w_0()
                            .overflow_x_scroll()
                            .child(tabs),
                    )
                    .child(footer),
            )
    }
}

fn enabled(snapshot: &Snapshot) -> Vec<ProviderKind> {
    snapshot
        .settings
        .popup_order
        .iter()
        .filter_map(|widget| widget.as_provider())
        .filter(|provider| snapshot.settings.providers.is_enabled(*provider))
        .collect()
}

pub(super) fn money(value: u64) -> String {
    format!("${:.2}", value as f64 / 1_000_000.)
}

pub(super) fn card(cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .rounded(px(10.))
        .bg(cx.theme().secondary)
        .border_1()
        .border_color(cx.theme().border)
}

pub(super) fn info_card(label: &str, value: String, cx: &App) -> Div {
    card(cx).child(
        div()
            .flex()
            .justify_between()
            .gap_2()
            .child(label.to_string())
            .child(div().font_weight(FontWeight::SEMIBOLD).child(value)),
    )
}

fn quota(label: &str, limit: &LimitWindow, settings: &Settings, cx: &App) -> AnyElement {
    let value = if settings.show_used_percentage {
        limit.used_percent
    } else {
        limit.remaining_percent()
    };
    let percent = value.unwrap_or(0) as f32 / 100.;
    let compact = settings.compact_usage_cards;
    let pace = (!compact && settings.show_usage_pace)
        .then(|| limit.pace_tip(settings.show_used_percentage, chrono::Utc::now()))
        .flatten();
    let mut result = card(cx).gap_2();
    result = result.child(
        div()
            .flex()
            .justify_between()
            .gap_2()
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(cx.theme().muted_foreground)
                    .child(label.to_uppercase()),
            )
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(cx.theme().muted_foreground)
                    .child(pace.map(|tip| tip.summary()).unwrap_or_default()),
            ),
    );
    let mut bar = div()
        .w_full()
        .h(px(if compact { 3. } else { 6. }))
        .rounded_full()
        .bg(cx.theme().border)
        .relative()
        .child(
            div()
                .w(relative(percent))
                .h_full()
                .rounded_full()
                .bg(cx.theme().primary),
        );
    if let Some(pace) = pace {
        bar = bar.child(
            div()
                .absolute()
                .top_0()
                .left(relative((pace.percent as f32 / 100.0).clamp(0.0, 1.0)))
                .w(px(2.))
                .h_full()
                .bg(cx.theme().foreground)
                .opacity(0.7),
        );
    }
    result = result.child(bar);
    result = result.child(
        div()
            .flex()
            .justify_between()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(value.map(|value| format!("{value}%")).unwrap_or_else(|| "—".into())),
            )
            .child(div().text_size(px(10.)).text_color(cx.theme().muted_foreground).child(
                limit.resets_at.map(|at| {
                    let minutes = (at - chrono::Utc::now()).num_minutes().max(0);
                    format!("Resets in {}h {}m", minutes / 60, minutes % 60)
                }).unwrap_or_default(),
            )),
    );
    result.into_any_element()
}
