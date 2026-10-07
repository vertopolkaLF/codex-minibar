//! Rendering of the framework-free card plan from [`crate::popup_window::model`].

use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, Transformation, Window, div,
    px, radians,
};

use super::{
    components::{self, CARD_RADIUS, caption, card, card_metadata, icon, nowrap, status_row},
    fx,
    root::{PopupRoot, eid},
    theme::HslaExt,
};
use crate::popup_window::{model::*, *};

/// Old WinUI estimate per expanded banked-reset row; used as the reveal cap.
const RESET_ROW_HEIGHT: f32 = 58.0;

fn compact_title_fits(available: f32, title: f32, usage: f32, reset: f32) -> bool {
    // Two 10 DIP gaps between the three groups in the compact row.
    title + usage + reset + 20.0 <= available
}

impl PopupRoot {
    pub(super) fn render_cards(
        &mut self,
        cards: &[Card<'_>],
        surface: PopupSurface,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        cards
            .iter()
            .map(|card| self.render_card(card, surface, window, cx))
            .collect()
    }

    fn render_card(
        &mut self,
        card_spec: &Card<'_>,
        surface: PopupSurface,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let style = self.card_style();
        match card_spec {
            Card::Heading(heading) => self.render_heading(heading, surface, cx),
            Card::Limit {
                key,
                title,
                window: limit,
                usage_amount,
                disabled,
            } => self.render_limit_card(
                key,
                title,
                limit,
                *usage_amount,
                *disabled,
                style,
                self.card_inner_width(surface),
                window,
            ),
            Card::Spending {
                key,
                title,
                masked_key,
                spending,
                has_live_usage,
                expired,
                expires_at,
                delete,
            } => self.render_spending_card(
                key,
                title,
                *masked_key,
                spending,
                *has_live_usage,
                *expired,
                *expires_at,
                delete.clone(),
                style,
                self.card_inner_width(surface),
                window,
                cx,
            ),
            Card::AccountHeading {
                name,
                balance_microusd,
                ..
            } => {
                let palette = &self.palette;
                let name = nowrap(components::body_strong(
                    name.to_string(),
                    palette.text_secondary,
                ));
                let row = match balance_microusd {
                    Some(balance) => components::split_row(
                        name,
                        components::body_strong(
                            format_usd(*balance as f64 / 1_000_000.0),
                            palette.accent,
                        ),
                    ),
                    None => div().child(name),
                };
                row.px(px(4.0)).mt(px(8.0)).into_any_element()
            }
            Card::ForcedResets(resets) => self.render_forced_resets(resets, cx),
            Card::CloudCredits {
                key,
                window: limit,
                credits,
            } => self.render_cloud_credits(key, limit, *credits, style),
            Card::BankedResets {
                limits,
                expansion_key,
            } => self.render_banked_resets(limits, expansion_key, cx),
            Card::UsageStatistics {
                provider,
                statistics,
            } => self.render_activity_card(*provider, statistics, window, cx),
            Card::UsageLoading => caption(
                crate::i18n::tr("loading-usage-statistics"),
                self.palette.text_tertiary,
            )
            .mx(px(4.0))
            .into_any_element(),
            Card::Credits { value } => {
                let palette = &self.palette;
                card(palette)
                    .px(px(16.0))
                    .py(px(12.0))
                    .child(components::split_row(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(components::body(
                                crate::i18n::tr("credits-338f52"),
                                palette.text_tertiary,
                            ))
                            .child(caption(
                                crate::i18n::tr("available-balance"),
                                palette.text_tertiary,
                            )),
                        components::body_strong(value.clone(), palette.accent),
                    ))
                    .into_any_element()
            }
            Card::Group { cards, .. } => div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .children(self.render_cards(cards, surface, window, cx))
                .into_any_element(),
        }
    }

    fn render_heading(
        &mut self,
        heading: &HeadingCard,
        surface: PopupSurface,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let mut title = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .min_w_0();
        let driver = heading.provider.kind();
        if matches!(surface, PopupSurface::HomeTab) || heading.show_icon {
            title = title.child(
                components::provider_mark(
                    crate::provider_registry::icon(driver),
                    16.0,
                    palette.provider_icon(driver, self.ui.use_colored_provider_icons),
                    heading.provider.badge().as_ref(),
                    &palette,
                )
                .mr(px(if heading.provider.badge().is_some() {
                    10.0
                } else {
                    4.0
                })),
            );
        }
        title = title.child(nowrap(components::body_strong(
            driver.display_name(),
            palette.text_secondary,
        )));
        if let Some(plan) = heading.plan.as_ref() {
            title = title.child(nowrap(components::body(
                plan.clone(),
                palette.text_tertiary,
            )));
        }
        if let Some(error) = heading.error.as_ref() {
            let provider = heading.provider;
            let id = fx::key(("heading-error", provider.id(), heading.first));
            title = title.child(
                div()
                    .id(eid(format!(
                        "heading-error-{}-{}",
                        provider.id(),
                        heading.first
                    )))
                    .on_hover(self.hover_listener(
                        id,
                        Some(crate::i18n::localize_error(error).into()),
                        cx,
                    ))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.select_view(PopupView::from_provider(provider), cx);
                    }))
                    .child(components::error_badge(16.0, &palette)),
            );
        }
        let mut trailing = div().flex().flex_row().items_center().gap(px(4.0));
        if let Some(name) = heading.account_name.clone() {
            trailing = trailing.child(card_metadata(name, &palette));
        }
        if let Some(balance) = heading.balance_microusd {
            trailing = trailing.child(components::body_strong(
                format_usd(balance as f64 / 1_000_000.0),
                palette.accent,
            ));
        }
        if heading.drag_handle {
            trailing = trailing
                .child(self.widget_drag_handle(HomeWidgetId::provider(heading.provider), cx));
        }
        components::split_row(title, trailing)
            .px(px(4.0))
            .mt(px(if heading.first { 0.0 } else { 8.0 }))
            .mb(px(2.0))
            .into_any_element()
    }

    fn reset_status(&self, limit: &LimitWindow) -> Div {
        match limit.resets_at {
            Some(at) => status_row(
                crate::i18n::tr("resets-in"),
                format_reset_in(Some(at)),
                &self.palette,
            ),
            None => card_metadata(crate::i18n::tr("session-not-started"), &self.palette),
        }
    }

    fn usage_label(
        &self,
        label: String,
        usage_amount: Option<&UsageAmount>,
        show_values: bool,
    ) -> Div {
        let palette = &self.palette;
        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(5.0))
            .whitespace_nowrap()
            .child(components::body_strong(label, palette.accent));
        if let Some(value) = usage_amount_label(usage_amount, show_values) {
            row = row.child(caption(value, palette.text_tertiary));
        }
        row
    }

    fn render_limit_card(
        &mut self,
        key: &str,
        title: &str,
        limit: &LimitWindow,
        usage_amount: Option<&UsageAmount>,
        disabled: bool,
        style: CardStyle,
        available_width: f32,
        window: &Window,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let (label, progress, show_reset, exhausted) =
            limit_card_presentation(limit, style.show_used_percentage, disabled);
        // Values glide when a poll moves them instead of jumping.
        let progress = self.fx.value(
            fx::key(("limit-progress", key)),
            progress as f32,
            fx::NORMAL,
        );
        let pace = (style.show_usage_pace && !exhausted)
            .then(|| limit.pace_tip(style.show_used_percentage, Utc::now()))
            .flatten();
        let secondary = palette.text_secondary;
        let font_family = self.font_family.clone();
        let measure = |value: &str, size, weight| {
            components::measure_text(
                window.text_system(),
                font_family.clone(),
                size,
                weight,
                value,
            )
        };
        let title_width = measure(title, 12.0, FontWeight::NORMAL);
        let mut usage_width = measure(&label, 14.0, FontWeight::SEMIBOLD);
        if let Some(value) = usage_amount_label(usage_amount, style.show_usage_values) {
            usage_width += 5.0 + measure(&value, 12.0, FontWeight::NORMAL);
        }
        let reset_width = match limit.resets_at {
            Some(at) => {
                measure(crate::i18n::tr("resets-in"), 14.0, FontWeight::NORMAL)
                    + 6.0
                    + measure(&format_reset_in(Some(at)), 14.0, FontWeight::NORMAL)
            }
            None => measure(
                crate::i18n::tr("session-not-started"),
                12.0,
                FontWeight::NORMAL,
            ),
        };
        let title_fits = !title.contains(['\n', '\r'])
            && match pace {
                Some(pace) => {
                    title_width + 8.0 + measure(&pace.summary(), 12.0, FontWeight::NORMAL)
                        <= available_width
                }
                None => compact_title_fits(available_width, title_width, usage_width, reset_width),
            };
        let title_el = || nowrap(caption(title.to_owned(), secondary));
        let one_row = |this: &Self| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .w_full()
                .gap(px(10.0))
                .child(div().flex_1().min_w_0().child(title_el()))
                .child(this.usage_label(label.clone(), usage_amount, style.show_usage_values))
                .child(this.reset_status(limit))
        };
        let header = if !title_fits {
            let mut full = div()
                .flex()
                .flex_col()
                .w_full()
                .gap(px(2.0))
                .child(caption(title.to_owned(), secondary));
            if let Some(pace) = pace {
                full = full.child(card_metadata(pace.summary(), &palette));
            }
            full
        } else {
            match pace {
                Some(pace) => {
                    components::split_row(title_el(), card_metadata(pace.summary(), &palette))
                }
                None => div().child(title_el()),
            }
        };
        let footer = if show_reset && usage_width + 8.0 + reset_width <= available_width {
            components::split_row(
                self.usage_label(label.clone(), usage_amount, style.show_usage_values),
                self.reset_status(limit),
            )
        } else if show_reset {
            div()
                .flex()
                .flex_col()
                .w_full()
                .gap(px(4.0))
                .child(self.usage_label(label.clone(), usage_amount, style.show_usage_values))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .child(self.reset_status(limit)),
                )
        } else {
            self.usage_label(label.clone(), usage_amount, style.show_usage_values)
        };

        if style.compact {
            let mut element = card(&palette).relative().overflow_hidden();
            for layer in components::compact_progress_layers(
                progress,
                pace.map(|pace| pace.percent as f32),
                interval_tick_count(limit),
                palette.accent,
                &palette,
            ) {
                element = element.child(layer);
            }
            // Names that do not fit keep the compact fill but move values below the title.
            let content = if pace.is_none() && title_fits {
                one_row(self)
            } else {
                div().flex().flex_col().w_full().child(header).child(footer)
            };
            return element
                .child(div().relative().p(px(12.0)).child(content))
                .into_any_element();
        }
        if exhausted && title_fits {
            return card(&palette)
                .overflow_hidden()
                .p(px(12.0))
                .child(one_row(self))
                .into_any_element();
        }
        card(&palette)
            .overflow_hidden()
            .p(px(12.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(header)
            .child(components::progress_track(
                progress,
                pace.map(|pace| pace.percent as f32),
                interval_tick_count(limit),
                palette.accent,
                &palette,
            ))
            .child(footer)
            .into_any_element()
    }

    fn render_cloud_credits(
        &mut self,
        key: &str,
        limit: &LimitWindow,
        credits: Option<&crate::limits::CloudSessionCredits>,
        style: CardStyle,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let (label, progress, available) = cloud_session_credits_presentation(
            limit,
            credits,
            style.show_used_percentage,
            Utc::now(),
        );
        let progress = self.fx.value(
            fx::key(("limit-progress", key)),
            progress as f32,
            fx::NORMAL,
        );
        // Name/balance left, expiry date/countdown right, like two-line quota cards.
        let mut metadata = div().flex().flex_col().items_end();
        if let Some(expires) = limit.resets_at {
            let local = expires.with_timezone(&Local);
            metadata = metadata.child(card_metadata(
                format!(
                    "{}, {}",
                    crate::i18n::month_day(local),
                    TimeFormat::current().format_hm(local)
                ),
                &palette,
            ));
            if available {
                metadata = metadata.child(status_row(
                    crate::i18n::tr("expires-in"),
                    format_reset_in(Some(expires)),
                    &palette,
                ));
            }
        }
        let content = components::split_row(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .child(nowrap(caption(
                    crate::i18n::tr("cloud-session-credits"),
                    palette.text_secondary,
                )))
                .child(nowrap(components::body_strong(label, palette.accent))),
            metadata,
        );

        if style.compact {
            let mut element = card(&palette).relative().overflow_hidden();
            if available {
                for layer in
                    components::compact_progress_layers(progress, None, 0, palette.accent, &palette)
                {
                    element = element.child(layer);
                }
            }
            return element
                .child(div().relative().p(px(12.0)).child(content))
                .into_any_element();
        }
        let mut element = card(&palette)
            .overflow_hidden()
            .p(px(12.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(content);
        if available {
            element = element.child(components::progress_track(
                progress,
                None,
                0,
                palette.accent,
                &palette,
            ));
        }
        element.into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn render_spending_card(
        &mut self,
        key: &str,
        title: &str,
        masked_key: Option<&str>,
        spending: &SpendingSummary,
        has_live_usage: bool,
        expired: bool,
        expires_at: Option<DateTime<Utc>>,
        delete: Option<(String, String)>,
        style: CardStyle,
        available_width: f32,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let font_family = self.font_family.clone();
        let measure = |value: &str, size, weight| {
            components::measure_text(
                window.text_system(),
                font_family.clone(),
                size,
                weight,
                value,
            )
        };
        let mut right_width;
        let mut right = div().flex().flex_col().items_end().gap(px(2.0));
        let mut show_masked_key = true;
        if expired {
            let label =
                expires_at.map_or_else(|| crate::i18n::tr("expired").into(), format_expired_at);
            right_width = measure(&label, 14.0, FontWeight::NORMAL);
            let mut row = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(components::body(label, palette.text_tertiary));
            if let Some((account_id, key_id)) = delete {
                right_width += 40.0;
                row = row.child(self.render_delete_button(account_id, key_id, cx));
            }
            right = right.child(row);
        } else {
            let amount = if has_live_usage {
                let used = format_usd(spending.used_microusd as f64 / 1_000_000.0);
                spending.limit_microusd.map_or_else(
                    || used.clone(),
                    |limit| format!("{used} / {}", format_usd(limit as f64 / 1_000_000.0)),
                )
            } else {
                // Unknown usage — never show a bare limit that looks like spend.
                spending.limit_microusd.map_or_else(
                    || "?.??".into(),
                    |limit| format!("?.?? / {}", format_usd(limit as f64 / 1_000_000.0)),
                )
            };
            right_width = measure(crate::i18n::tr("usage"), 14.0, FontWeight::NORMAL)
                + 6.0
                + measure(&amount, 14.0, FontWeight::SEMIBOLD);
            right = right.child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(6.0))
                    .whitespace_nowrap()
                    .child(components::body(
                        crate::i18n::tr("usage"),
                        palette.text_tertiary,
                    ))
                    .child(components::body_strong(amount, palette.accent)),
            );
            let expires_soon = expires_at.filter(|at| *at > Utc::now());
            show_masked_key = !(spending.resets_at.is_some() && expires_soon.is_some());
            if spending.resets_at.is_some() || expires_soon.is_some() {
                let mut meta_width = 0.0;
                let mut meta = div().flex().flex_row().items_center().gap(px(6.0));
                if let Some(reset) = spending.resets_at {
                    meta_width += measure(crate::i18n::tr("resets-in"), 14.0, FontWeight::NORMAL)
                        + 6.0
                        + measure(&format_reset_in(Some(reset)), 14.0, FontWeight::NORMAL);
                    meta = meta.child(status_row(
                        crate::i18n::tr("resets-in"),
                        format_reset_in(Some(reset)),
                        &palette,
                    ));
                }
                if let Some(expires) = expires_soon {
                    if spending.resets_at.is_some() {
                        meta_width += 12.0 + measure("•", 12.0, FontWeight::NORMAL);
                        meta = meta.child(card_metadata("•", &palette));
                    }
                    meta_width += measure(crate::i18n::tr("expires-in"), 14.0, FontWeight::NORMAL)
                        + 6.0
                        + measure(&format_reset_in(Some(expires)), 14.0, FontWeight::NORMAL);
                    meta = meta.child(status_row(
                        crate::i18n::tr("expires-in"),
                        format_reset_in(Some(expires)),
                        &palette,
                    ));
                }
                right = right.child(meta);
                right_width = right_width.max(meta_width);
            }
        }
        let title_fits = !title.contains(['\n', '\r'])
            && measure(title, 12.0, FontWeight::NORMAL) + 8.0 + right_width <= available_width;
        let title_label = caption(title.to_owned(), palette.text_secondary);
        let mut left = div().flex().flex_col().gap(px(2.0)).child(if title_fits {
            nowrap(title_label)
        } else {
            title_label
        });
        if show_masked_key
            && let Some(masked) = masked_key.map(str::trim).filter(|value| !value.is_empty())
        {
            left = left.child(nowrap(caption(masked.to_owned(), palette.text_tertiary)));
        }
        let header = if title_fits {
            components::split_row(left, right)
        } else {
            div()
                .flex()
                .flex_col()
                .w_full()
                .gap(px(6.0))
                .child(left)
                .child(right)
        };
        // Expired keys keep title/mask only — no spend bar that looks "full".
        let progress = (!expired)
            .then(|| {
                spending
                    .limit_microusd
                    .filter(|limit| *limit > 0)
                    .map(|limit| {
                        let used = if has_live_usage {
                            (spending.used_microusd.min(limit) as f64 / limit as f64 * 100.0)
                                .clamp(0.0, 100.0)
                        } else {
                            0.0
                        };
                        if style.show_used_percentage {
                            used
                        } else {
                            100.0 - used
                        }
                    })
            })
            .flatten()
            .map(|value| {
                self.fx
                    .value(fx::key(("spend-progress", key)), value as f32, fx::NORMAL)
            });
        if style.compact {
            let mut element = card(&palette).relative().overflow_hidden();
            if let Some(progress) = progress {
                for layer in
                    components::compact_progress_layers(progress, None, 0, palette.accent, &palette)
                {
                    element = element.child(layer);
                }
            }
            return element
                .child(div().relative().p(px(12.0)).child(header))
                .into_any_element();
        }
        let mut element = card(&palette)
            .p(px(12.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(header);
        if let Some(progress) = progress {
            element = element.child(components::progress_track(
                progress,
                None,
                0,
                palette.accent,
                &palette,
            ));
        }
        element.into_any_element()
    }

    fn render_delete_button(
        &mut self,
        account_id: String,
        key_id: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let hover_id = fx::key(("openrouter-delete", account_id.as_str(), key_id.as_str()));
        let hovered = self.hovered(hover_id);
        let hover = self.fx.toggle(
            fx::key(("openrouter-delete-fx", hover_id)),
            hovered,
            fx::FASTER,
        );
        let settings_tx = self.state.settings_tx.clone();
        let mut button = div()
            .id(eid(format!("openrouter-delete-{account_id}-{key_id}")))
            .relative()
            .size(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.0))
            .on_hover(self.hover_listener(hover_id, Some(crate::i18n::tr("remove-key").into()), cx))
            .on_click(move |_: &ClickEvent, _, _| {
                let account_id = account_id.clone();
                let key_id = key_id.clone();
                let settings_tx = settings_tx.clone();
                std::thread::spawn(move || {
                    crate::popup_window::remove_openrouter_api_key(account_id, key_id, settings_tx)
                });
            });
        if let Some(layer) = components::hover_layer(&palette, hover, 4.0) {
            button = button.child(layer);
        }
        button
            .child(icon(
                "fluent-delete",
                18.0,
                palette.chrome_icon.mix(palette.accent, hover),
            ))
            .into_any_element()
    }

    fn render_banked_resets(
        &mut self,
        limits: &RateLimits,
        expansion_key: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let count = limits.available_reset_count();
        let count_label =
            crate::i18n::format("count-banked-resets", &[("count", count.to_string())]);
        let format_date = |at: DateTime<Utc>| {
            let local = at.with_timezone(&Local);
            format!(
                "{}, {}",
                crate::i18n::month_day(local),
                TimeFormat::current().format_hm(local)
            )
        };
        let expiration = limits.next_reset_credit_expiration();
        let expiration_date = expiration
            .map(format_date)
            .unwrap_or_else(|| crate::i18n::tr("available-to-use").into());
        let expiration_status = match expiration {
            Some(at) => status_row(
                crate::i18n::tr("expires-in"),
                format_reset_in(Some(at)),
                &palette,
            ),
            None => card_metadata(crate::i18n::tr("no-expiration-date"), &palette),
        };
        let available = limits
            .reset_credits
            .as_ref()
            .map(|summary| {
                summary
                    .credits
                    .iter()
                    .filter(|credit| credit.status.eq_ignore_ascii_case("available"))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let expandable = count > 1 && available.len() > 1;
        let open = expandable && self.open_reset_card.as_deref() == Some(expansion_key);
        let reveal = self
            .fx
            .toggle(fx::key(("reset-reveal", expansion_key)), open, fx::NORMAL);

        let mut title = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(nowrap(components::body_strong(count_label, palette.accent)));
        if expandable {
            title = title.child(
                icon("fluent-chevron-down", 16.0, theme_gray()).with_transformation(
                    Transformation::rotate(radians(std::f32::consts::PI * reveal)),
                ),
            );
        }
        let header_content = components::split_row(
            title,
            div()
                .flex()
                .flex_col()
                .items_end()
                .gap(px(1.0))
                .child(card_metadata(expiration_date, &palette))
                .child(expiration_status),
        );
        let hover_id = fx::key(("reset-card", expansion_key));
        let hover = self.fx.toggle(
            fx::key(("reset-card-hover", hover_id)),
            expandable && self.hovered(hover_id),
            fx::FASTER,
        );
        let mut header = div()
            .id(eid(format!("reset-card-{expansion_key}")))
            .relative()
            .px(px(16.0))
            .py(px(12.0));
        if expandable {
            let key = expansion_key.to_owned();
            header = header
                .on_hover(self.hover_listener(hover_id, None, cx))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.open_reset_card = if this.open_reset_card.as_deref() == Some(key.as_str())
                    {
                        None
                    } else {
                        Some(key.clone())
                    };
                    cx.notify();
                }));
            if hover > 0.001 {
                let layer = div()
                    .absolute()
                    .inset_0()
                    .bg(palette.subtle_fill.opacity(hover));
                header = header.child(if reveal > 0.0 {
                    layer.rounded_t(px(CARD_RADIUS - 1.0))
                } else {
                    layer.rounded(px(CARD_RADIUS - 1.0))
                });
            }
        }
        header = header.child(div().relative().child(header_content));
        let mut element = card(&palette)
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(header);
        if expandable && reveal > 0.001 {
            let mut rows = div()
                .flex()
                .flex_col()
                .gap(px(9.0))
                .child(components::rule(&palette));
            for (index, credit) in available.iter().enumerate() {
                if index > 0 {
                    rows = rows.child(components::rule(&palette));
                }
                let name = credit
                    .title
                    .as_deref()
                    .filter(|title| !title.trim().is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        crate::i18n::format("banked-reset", &[("v0", (index + 1).to_string())])
                    });
                let date = credit
                    .expires_at
                    .map_or_else(|| crate::i18n::tr("no-expiration-date").into(), format_date);
                let status = match credit.expires_at {
                    Some(at) => status_row(
                        crate::i18n::tr("expires-in"),
                        format_reset_in(Some(at)),
                        &palette,
                    ),
                    None => card_metadata(crate::i18n::tr("available-to-use"), &palette),
                };
                rows = rows.child(components::split_row(
                    nowrap(components::body(name, palette.text_secondary)),
                    div()
                        .flex()
                        .flex_col()
                        .items_end()
                        .gap(px(1.0))
                        .child(card_metadata(date, &palette))
                        .child(status),
                ));
            }
            let cap = (available.len() as f32 * RESET_ROW_HEIGHT + 16.0) * reveal;
            element = element.child(
                div()
                    .overflow_hidden()
                    .max_h(px(cap))
                    .opacity(reveal)
                    .child(div().px(px(16.0)).pb(px(12.0)).child(rows)),
            );
        }
        element.into_any_element()
    }

    fn render_forced_resets(
        &mut self,
        resets: &[&crate::reset_feed::ForcedReset],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let hover_id = fx::key("forced-resets");
        let hover = self.fx.toggle(
            fx::key(("forced-resets-hover", hover_id)),
            self.hovered(hover_id),
            fx::FASTER,
        );
        let mut rows = div().flex().flex_col().gap(px(8.0));
        for reset in resets {
            let local = reset.reset_at.with_timezone(&Local);
            let date = format!(
                "{}, {}",
                crate::i18n::month_day(local),
                TimeFormat::current().format_hm(local)
            );
            let label = reset
                .label
                .as_deref()
                .filter(|label| !label.trim().is_empty())
                .unwrap_or(crate::i18n::tr("codex-limits"))
                .to_owned();
            let row_id = fx::key(("forced-reset-row", reset.id.as_str()));
            let tip: SharedString = if reset.source_url.is_some() {
                crate::i18n::tr("open-announcement-source").into()
            } else {
                crate::i18n::tr("source-not-provided").into()
            };
            let source = reset.source_url.clone();
            rows = rows.child(
                div()
                    .id(eid(format!("forced-reset-{}", reset.id)))
                    .on_hover(self.hover_listener(row_id, Some(tip), cx))
                    .on_click(move |_: &ClickEvent, _, _| {
                        if let Some(url) = source.as_deref()
                            && let Err(error) = crate::updater::open_url(url)
                        {
                            crate::logger::info(format!(
                                "failed to open forced reset source {url}: {error:#}"
                            ));
                        }
                    })
                    .child(components::split_row(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(1.0))
                            .child(caption(
                                crate::i18n::tr("tibo-reset"),
                                palette.text_secondary,
                            ))
                            .child(components::body_strong(label, palette.accent)),
                        div()
                            .flex()
                            .flex_col()
                            .items_end()
                            .gap(px(1.0))
                            .child(card_metadata(date, &palette))
                            .child(status_row(
                                crate::i18n::tr("resets-in"),
                                format_reset_in(Some(reset.reset_at)),
                                &palette,
                            )),
                    )),
            );
        }
        let mut element = card(&palette)
            .id("forced-resets")
            .relative()
            .on_hover(self.hover_listener(hover_id, None, cx));
        if let Some(layer) = components::hover_layer(&palette, hover, CARD_RADIUS - 1.0) {
            element = element.child(layer);
        }
        element
            .child(div().relative().p(px(12.0)).child(rows))
            .into_any_element()
    }
}

fn theme_gray() -> gpui::Hsla {
    super::theme::rgb8((138, 138, 138))
}

#[cfg(test)]
mod layout_tests {
    use super::compact_title_fits;

    #[test]
    fn compact_card_requires_room_for_the_entire_name() {
        assert!(compact_title_fits(320.0, 80.0, 100.0, 120.0));
        assert!(!compact_title_fits(320.0, 81.0, 100.0, 120.0));
        assert!(compact_title_fits(340.0, 81.0, 100.0, 120.0));
    }
}
