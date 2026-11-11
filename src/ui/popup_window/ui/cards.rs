//! Rendering of the framework-free card plan from [`crate::ui::popup_window::model`].

use std::{cell::Cell, rc::Rc};

use gpui::{
    AnyElement, Bounds, ClickEvent, Context, Div, Hsla, InteractiveElement, IntoElement,
    ParentElement, PathBuilder, Pixels, SharedString, StatefulInteractiveElement, Styled,
    Transformation, Window, canvas, div, point, px, radians, relative,
};

use super::{
    components::{self, TextMetrics, caption, card, card_metadata, nowrap, status_row},
    root::{PopupRoot, eid},
};
use crate::ui::gui::{color::HslaExt, fx, mark::icon};
use crate::ui::popup_window::{model::*, *};

/// Line boxes of [`components::body`] and [`components::caption`] copy; used
/// to estimate the reveal cap of expanded banked-reset rows.
const BODY_LINE: f32 = 20.0;
const CAPTION_LINE: f32 = 16.0;
/// Vertical gap around the dividers between expanded banked-reset rows.
const RESET_ROW_GAP: f32 = 9.0;
/// Ring gauge diameter and stroke of the Rings layout.
const RING_SIZE: f32 = 46.0;
const RING_PACE_GAP: f32 = 8.0;
const RING_STROKE: f32 = 5.0;
/// Narrower cards stack ring tiles one per row.
const RING_PAIR_MIN_WIDTH: f32 = 260.0;

fn compact_title_fits(available: f32, title: f32, usage: f32, reset: f32) -> bool {
    // Two 10 DIP gaps between the three groups in the compact row.
    title + usage + reset + 20.0 <= available
}

impl PopupRoot {
    /// Compare against the final laid-out label width, not a guessed number
    /// of characters. Only a genuinely clipped name gets a full-value tip.
    fn account_name_label(
        &self,
        key: u64,
        name: String,
        strong: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = &self.palette;
        let display_name = name.replace(['\r', '\n', '\t'], " ");
        let measured = Rc::new(Cell::new(false));
        let probe_state = Rc::clone(&measured);
        let font = palette.font_family.clone();
        let probe_text = display_name.clone();
        let probe = canvas(
            move |bounds, window, _| {
                let full_width = crate::ui::gui::measure_text(
                    window.text_system(),
                    font.clone(),
                    if strong { 14.0 } else { 12.0 },
                    if strong {
                        gpui::FontWeight::SEMIBOLD
                    } else {
                        gpui::FontWeight::NORMAL
                    },
                    &probe_text,
                );
                probe_state.set(full_width > f32::from(bounds.size.width));
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();
        // A text element is only as wide as its text, so `text_right` alone
        // leaves it on the left; the flex container pushes it to the end.
        let label = if strong {
            components::body_strong(display_name, palette.text_secondary)
        } else {
            caption(display_name, palette.text_tertiary)
                .flex()
                .justify_end()
        };
        div()
            .id(eid(format!("account-name-{key}")))
            .relative()
            .w_full()
            .min_w_0()
            .on_hover(cx.listener(move |this, hovered: &bool, window, cx| {
                if *hovered && measured.get() {
                    this.show_tip(
                        key,
                        super::tooltip::TipContent::Text(name.clone().into()),
                        window,
                        cx,
                    );
                } else if this.tip.as_ref().is_some_and(|tip| tip.owner == key) {
                    this.tip = None;
                    cx.notify();
                }
            }))
            .child(nowrap(label).w_full())
            .child(probe)
            .into_any_element()
    }
    fn metrics<'a>(&self, window: &'a Window) -> TextMetrics<'a> {
        TextMetrics::new(window, self.palette.font_family.clone())
    }

    pub(super) fn render_cards(
        &mut self,
        cards: &[Card<'_>],
        surface: PopupSurface,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let style = self.card_style();
        self.render_cards_styled(cards, surface, style, window, cx)
    }

    fn render_cards_styled(
        &mut self,
        cards: &[Card<'_>],
        surface: PopupSurface,
        style: CardStyle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        cards
            .iter()
            .map(|card| self.render_card(card, surface, style, window, cx))
            .collect()
    }

    fn render_card(
        &mut self,
        card_spec: &Card<'_>,
        surface: PopupSurface,
        style: CardStyle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match card_spec {
            Card::Heading(heading) => self.render_heading(heading, surface, cx),
            Card::Limit {
                key,
                title,
                window: limit,
                usage_amount,
                disabled,
                overlay,
            } => self.render_limit_card(
                key,
                title,
                limit,
                *usage_amount,
                *disabled,
                overlay.as_ref(),
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
            Card::OpenRouterKeys { provider, account } => self
                .render_keys_section(*provider, account, window, cx)
                .unwrap_or_else(|| div().into_any_element()),
            Card::AccountHeading {
                id,
                name,
                balance_microusd,
                ..
            } => {
                let palette = &self.palette;
                let name = self.account_name_label(
                    fx::key((
                        "openrouter-account-name",
                        *id,
                        surface == PopupSurface::HomeTab,
                    )),
                    name.to_string(),
                    true,
                    cx,
                );
                let row = match balance_microusd {
                    Some(balance) => components::split_row(
                        name,
                        components::body_strong(format_micro_usd(*balance), palette.accent_text),
                    ),
                    None => div().child(name),
                };
                row.px(px(4.0)).mt(px(8.0)).into_any_element()
            }
            Card::AnnouncedReset(row) => self.render_announced_reset(row, cx),
            Card::ProviderIncidents(status) => {
                self.render_provider_incidents(status, self.card_inner_width(surface), window, cx)
            }
            Card::CloudCredits {
                key,
                title,
                window: limit,
                credits,
            } => self.render_cloud_credits(
                key,
                title,
                limit,
                *credits,
                style,
                self.card_inner_width(surface),
                window,
            ),
            Card::BankedResets {
                provider,
                limits,
                expansion_key,
            } => self.render_banked_resets(
                *provider,
                limits,
                expansion_key,
                self.card_inner_width(surface),
                window,
                cx,
            ),
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
                        components::body_strong(value.clone(), palette.accent_text),
                    ))
                    .into_any_element()
            }
            Card::Group { cards, .. } => div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .children(self.render_cards_styled(cards, surface, style, window, cx))
                .into_any_element(),
        }
    }

    /// Home cards in the widget's chosen layout. Only quota windows change
    /// shape; every other card renders as usual.
    pub(super) fn render_home_cards(
        &mut self,
        cards: &[Card<'_>],
        layout: HomeCardLayout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let style = self.card_style();
        if matches!(layout, HomeCardLayout::Cards | HomeCardLayout::Classic) {
            // Classic keeps the header, thin bar and footer card design.
            let style = CardStyle {
                compact: layout == HomeCardLayout::Cards,
                ..style
            };
            return self.render_cards_styled(cards, PopupSurface::HomeTab, style, window, cx);
        }
        let paired = self.card_inner_width(PopupSurface::HomeTab) >= RING_PAIR_MIN_WIDTH;
        let mut out = Vec::new();
        let mut rings = Vec::new();
        for card_spec in cards {
            match card_spec {
                Card::Limit {
                    key,
                    title,
                    window: limit,
                    usage_amount,
                    disabled,
                    overlay,
                } => {
                    let overlay = overlay.as_ref();
                    if layout == HomeCardLayout::Rings {
                        rings.push(
                            self.render_limit_ring(key, title, limit, *disabled, overlay, style),
                        );
                    } else if layout == HomeCardLayout::Tiles {
                        rings.push(
                            self.render_limit_tile(
                                key, title, limit, *disabled, overlay, style, cx,
                            ),
                        );
                    } else if overlay.is_some() {
                        // Two limits do not fit a single line: a merged
                        // card always keeps the two-row compact shape.
                        out.push(self.render_limit_card(
                            key,
                            title,
                            limit,
                            *usage_amount,
                            *disabled,
                            overlay,
                            CardStyle {
                                compact: true,
                                ..style
                            },
                            self.card_inner_width(PopupSurface::HomeTab),
                            window,
                        ));
                    } else {
                        out.push(self.render_limit_line(
                            key,
                            title,
                            limit,
                            *usage_amount,
                            *disabled,
                            style,
                            cx,
                        ));
                    }
                }
                Card::BankedResets {
                    provider,
                    limits,
                    expansion_key,
                } => {
                    flush_rings(&mut out, &mut rings, paired);
                    out.push(self.render_banked_resets(
                        *provider,
                        limits,
                        expansion_key,
                        self.card_inner_width(PopupSurface::HomeTab),
                        window,
                        cx,
                    ));
                }
                _ => {
                    flush_rings(&mut out, &mut rings, paired);
                    out.push(self.render_card(card_spec, PopupSurface::HomeTab, style, window, cx));
                }
            }
        }
        flush_rings(&mut out, &mut rings, paired);
        out
    }

    /// Animated segment length and copy of a sub-limit merged into a quota
    /// bar. `parent_progress` is the parent bar's target percent.
    fn limit_overlay(
        &mut self,
        key: &str,
        limit: &LimitWindow,
        overlay: Option<&LimitOverlay<'_>>,
        parent_progress: f64,
        disabled: bool,
        show_used_percentage: bool,
    ) -> Option<MergedLimit> {
        let overlay = overlay.filter(|_| !disabled)?;
        let target = overlay.progress(parent_progress, show_used_percentage);
        let progress = self.fx.value(
            fx::key(("limit-overlay-progress", key)),
            target as f32,
            fx::NORMAL,
        );
        let percent =
            |value: Option<u8>| value.map_or_else(|| "–".to_owned(), |value| format!("{value}%"));
        Some(MergedLimit {
            progress,
            color: self.palette.quota_overlay(),
            legend: overlay.legend(show_used_percentage),
            parent_percent: percent(if show_used_percentage {
                limit.used_percent
            } else {
                limit.remaining_percent()
            }),
            name: overlay.short_title().to_owned(),
            percent: percent(overlay.percent(show_used_percentage)),
            mode: crate::i18n::tr(if show_used_percentage {
                "quota-word-used"
            } else {
                "quota-word-left"
            }),
        })
    }

    /// Both values of a merged card with equal weight, each in the color of
    /// its bar: `All 94%  Fable 97% left`.
    fn merged_usage(&self, merged: &MergedLimit) -> Div {
        let palette = &self.palette;
        let pair = |name: SharedString, value: String, color: Hsla| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(MERGED_USAGE_GAP))
                .child(caption(name, palette.text_tertiary))
                .child(components::body_strong(value, color))
        };
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_x(px(MERGED_USAGE_PAIR_GAP))
            .whitespace_nowrap()
            .child(pair(
                crate::i18n::tr("quota-all").into(),
                merged.parent_percent.clone(),
                palette.accent_text,
            ))
            .child(
                pair(
                    merged.name.clone().into(),
                    merged.percent.clone(),
                    merged.color,
                )
                .child(caption(merged.mode, palette.text_tertiary)),
            )
    }

    fn merged_usage_width(metrics: &TextMetrics<'_>, merged: &MergedLimit) -> f32 {
        metrics.caption(crate::i18n::tr("quota-all"))
            + MERGED_USAGE_GAP
            + metrics.strong(&merged.parent_percent)
            + MERGED_USAGE_PAIR_GAP
            + metrics.caption(&merged.name)
            + MERGED_USAGE_GAP
            + metrics.strong(&merged.percent)
            + MERGED_USAGE_GAP
            + metrics.caption(merged.mode)
    }

    /// Lines layout: title, pace, value and countdown on one row over the
    /// compact full-card fill. The full pace summary stays in a tooltip.
    #[allow(clippy::too_many_arguments)]
    fn render_limit_line(
        &mut self,
        key: &str,
        title: &str,
        limit: &LimitWindow,
        usage_amount: Option<&UsageAmount>,
        disabled: bool,
        style: CardStyle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let (label, progress, show_reset, exhausted) =
            limit_card_presentation(limit, style.show_used_percentage, disabled);
        let progress = self.fx.value(
            fx::key(("limit-progress", key)),
            progress as f32,
            fx::NORMAL,
        );
        let pace = (style.show_usage_pace && !exhausted)
            .then(|| limit.pace_tip(style.show_used_percentage, Utc::now()))
            .flatten();
        let mut row = div()
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .px(px(12.0))
            .py(px(9.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .min_w_0()
                            .child(nowrap(caption(title.to_owned(), palette.text_secondary))),
                    )
                    .children(pace.map(|pace| components::pace_status(pace, &palette))),
            )
            .child(self.usage_label(label, usage_amount, style.show_usage_values));
        if show_reset {
            row = row.child(match limit.resets_at {
                Some(at) => components::caption_icon_status(
                    components::RESET_ICON,
                    format_reset_in(Some(at)),
                    &palette,
                ),
                None => card_metadata(crate::i18n::tr("session-not-started"), &palette),
            });
        }
        let mut element = card(&palette)
            .id(eid(format!("limit-line-{key}")))
            .relative()
            .overflow_hidden()
            .children(components::compact_progress_layers(
                progress,
                pace.map(|pace| pace.percent as f32),
                interval_tick_count(limit),
                palette.accent,
                &palette,
            ));
        if let Some(pace) = pace {
            let hover_id = fx::key(("limit-line-pace", key));
            element =
                element.on_hover(self.hover_listener(hover_id, Some(pace.summary().into()), cx));
        }
        element.child(row).into_any_element()
    }

    /// Rings layout: a gauge with the percentage inside and the countdown
    /// beside it. Pace is a short radial tick across the ring, as on the
    /// Stream Deck ring key.
    fn render_limit_ring(
        &mut self,
        key: &str,
        title: &str,
        limit: &LimitWindow,
        disabled: bool,
        overlay: Option<&LimitOverlay<'_>>,
        style: CardStyle,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let (_, progress, show_reset, exhausted) =
            limit_card_presentation(limit, style.show_used_percentage, disabled);
        let merged = self.limit_overlay(
            key,
            limit,
            overlay,
            progress,
            disabled,
            style.show_used_percentage,
        );
        let overlay_segment = merged
            .as_ref()
            .map(|merged| (merged.progress, merged.color));
        let progress = self.fx.value(
            fx::key(("limit-progress", key)),
            progress as f32,
            fx::NORMAL,
        );
        let pace = (style.show_usage_pace && !exhausted)
            .then(|| limit.pace_tip(style.show_used_percentage, Utc::now()))
            .flatten();
        let percent = if disabled {
            None
        } else if style.show_used_percentage {
            limit.used_percent
        } else {
            limit.remaining_percent()
        };
        let value = percent.map_or_else(|| "–".to_owned(), |value| format!("{value}%"));
        let accent = palette.accent;
        let marker = palette.pace_marker;
        let pace_percent = pace.map(|pace| pace.percent as f32);
        let gauge = div()
            .relative()
            .flex_none()
            .size(px(RING_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        paint_ring(
                            bounds,
                            progress,
                            overlay_segment,
                            pace_percent,
                            accent,
                            marker,
                            window,
                        );
                    },
                )
                .absolute()
                .inset_0(),
            )
            .child(components::caption_strong(value, palette.text_primary));
        let mut details = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .child(nowrap(caption(title.to_owned(), palette.text_secondary)));
        if show_reset {
            details = details.child(match limit.resets_at {
                // Pace follows the countdown and wraps below it when the
                // tile is too narrow for both.
                Some(at) => div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_x(px(RING_PACE_GAP))
                    .child(nowrap(components::body_strong(
                        format_reset_in(Some(at)),
                        palette.text_primary,
                    )))
                    .children(pace.map(|pace| components::pace_status(pace, &palette))),
                // Wraps rather than clipping longer translations in a narrow tile.
                None => caption(
                    crate::i18n::tr("session-not-started"),
                    palette.text_tertiary,
                ),
            });
        } else if disabled {
            details = details.child(nowrap(card_metadata(crate::i18n::tr("disabled"), &palette)));
        }
        if !show_reset && let Some(pace) = pace {
            details = details.child(components::pace_status(pace, &palette));
        }
        if let Some(merged) = merged {
            details = details.child(overlay_legend(merged.legend, merged.color, &palette));
        }
        // Grows to fill its `flush_rings` cell so paired tiles share a height,
        // with the gauge and details centered vertically.
        card(&palette)
            .flex_1()
            .min_w_0()
            .p(px(10.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .child(gauge)
            .child(details)
            .into_any_element()
    }

    /// Tiles layout: the ring tile's two-per-row shape with the compact
    /// full-card fill instead of a gauge. Title and pace sit on top, the
    /// value and countdown below.
    fn render_limit_tile(
        &mut self,
        key: &str,
        title: &str,
        limit: &LimitWindow,
        disabled: bool,
        overlay: Option<&LimitOverlay<'_>>,
        style: CardStyle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let (_, progress, show_reset, exhausted) =
            limit_card_presentation(limit, style.show_used_percentage, disabled);
        let merged = self.limit_overlay(
            key,
            limit,
            overlay,
            progress,
            disabled,
            style.show_used_percentage,
        );
        let overlay_segment = merged
            .as_ref()
            .map(|merged| (merged.progress, merged.color));
        let progress = self.fx.value(
            fx::key(("limit-progress", key)),
            progress as f32,
            fx::NORMAL,
        );
        let pace = (style.show_usage_pace && !exhausted)
            .then(|| limit.pace_tip(style.show_used_percentage, Utc::now()))
            .flatten();
        let percent = if disabled {
            None
        } else if style.show_used_percentage {
            limit.used_percent
        } else {
            limit.remaining_percent()
        };
        let value = percent.map_or_else(|| "–".to_owned(), |value| format!("{value}%"));
        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .min_w_0()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(nowrap(caption(title.to_owned(), palette.text_secondary))),
            )
            .children(pace.map(|pace| components::pace_status(pace, &palette)));
        // The countdown wraps below the value when the tile is too narrow.
        let mut footer = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_x(px(8.0))
            .child(match merged.as_ref() {
                Some(merged) => self.merged_usage(merged),
                None => nowrap(components::body_strong(value, palette.accent_text)),
            });
        if show_reset {
            footer = footer.child(match limit.resets_at {
                Some(at) => components::caption_icon_status(
                    components::RESET_ICON,
                    format_reset_in(Some(at)),
                    &palette,
                ),
                None => card_metadata(crate::i18n::tr("session-not-started"), &palette),
            });
        } else if disabled {
            footer = footer.child(nowrap(card_metadata(crate::i18n::tr("disabled"), &palette)));
        }
        let mut element = card(&palette)
            .id(eid(format!("limit-tile-{key}")))
            .relative()
            .overflow_hidden()
            .flex_1()
            .min_w_0()
            .children(components::compact_progress_layers_with(
                progress,
                overlay_segment,
                pace.map(|pace| pace.percent as f32),
                interval_tick_count(limit),
                palette.accent,
                &palette,
            ));
        if let Some(pace) = pace {
            let hover_id = fx::key(("limit-tile-pace", key));
            element =
                element.on_hover(self.hover_listener(hover_id, Some(pace.summary().into()), cx));
        }
        // Grows to fill its `flush_rings` cell so paired tiles share a height.
        element
            .child(
                div()
                    .relative()
                    .size_full()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .gap(px(4.0))
                    .px(px(12.0))
                    .py(px(9.0))
                    .child(header)
                    .child(footer),
            )
            .into_any_element()
    }

    fn render_heading(
        &mut self,
        heading: &HeadingCard,
        surface: PopupSurface,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let provider = heading.provider;
        self.fx
            .mark_scope(fx::key(("provider-section", provider.kind())));
        // Home headings open their provider's tab, like the Usage Stats title.
        let link_id = fx::key(("heading-link", provider.id()));
        let link = matches!(surface, PopupSurface::HomeTab)
            && self.show_provider_tabs()
            && model::tab_for_provider(&self.ui, provider).is_some();
        let link_hover = self.fx.toggle(
            fx::key(("heading-link-fx", link_id)),
            link && self.hovered(link_id),
            fx::TEXT_FADE,
        );
        let mut title = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .min_w_0();
        let driver = heading.provider.kind();
        if matches!(surface, PopupSurface::HomeTab) || heading.show_icon {
            title = title.child(
                crate::ui::gui::mark::provider_mark(
                    crate::providers::registry::icon(driver),
                    16.0,
                    palette.provider_icon(driver, self.ui.settings.use_colored_provider_icons),
                    heading.provider.badge().as_ref(),
                    palette.plate(),
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
            palette.text_secondary.mix(palette.accent_text, link_hover),
        )));
        if let Some(plan) = heading.plan.as_ref() {
            title = title.child(nowrap(components::body(
                plan.clone(),
                palette.text_tertiary,
            )));
        }
        if let Some(error) = heading.error.as_ref() {
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
                        this.select_view(PopupView::Provider(provider), cx);
                    }))
                    .child(components::error_badge(16.0, &palette)),
            );
        }
        let mut trailing = div().flex().flex_row().items_center().gap(px(4.0));
        let has_account_name = heading.account_name.is_some();
        if let Some(name) = heading.account_name.clone() {
            trailing =
                trailing
                    .min_w_0()
                    .child(div().flex_1().min_w_0().child(self.account_name_label(
                        fx::key((
                            "heading-account-name",
                            provider.id(),
                            surface == PopupSurface::HomeTab,
                        )),
                        name,
                        false,
                        cx,
                    )));
        }
        if let Some(balance) = heading.balance_microusd {
            trailing = trailing.child(components::body_strong(
                format_micro_usd(balance),
                palette.accent_text,
            ));
        }
        if heading.drag_handle {
            trailing =
                self.with_widget_grip(trailing, HomeWidgetId::provider(heading.provider), cx);
        }
        let title = if link {
            div()
                .id(eid(format!("heading-link-{}", provider.id())))
                .min_w_0()
                .on_hover(self.hover_listener(link_id, None, cx))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.select_view(PopupView::Provider(provider), cx);
                }))
                .child(title)
                .into_any_element()
        } else {
            title.into_any_element()
        };
        let row = if has_account_name {
            div()
                .flex()
                .items_center()
                .w_full()
                .gap(px(components::SPLIT_GAP))
                .child(div().flex_1().min_w_0().child(title))
                // Definite width makes ellipsis resolve against the actual
                // available slot while retaining the driver/plan on the left.
                .child(trailing.w(relative(0.5)).flex_none())
        } else {
            components::split_row(title, trailing)
        };
        row.px(px(4.0))
            .mt(px(if heading.first { 0.0 } else { 8.0 }))
            .mb(px(2.0))
            .into_any_element()
    }

    fn reset_status(&self, limit: &LimitWindow) -> Div {
        match limit.resets_at {
            Some(at) => components::icon_status(
                components::RESET_ICON,
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
            .child(components::body_strong(label, palette.accent_text));
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
        overlay: Option<&LimitOverlay<'_>>,
        style: CardStyle,
        available_width: f32,
        window: &Window,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let (label, progress, show_reset, exhausted) =
            limit_card_presentation(limit, style.show_used_percentage, disabled);
        let merged = self.limit_overlay(
            key,
            limit,
            overlay,
            progress,
            disabled,
            style.show_used_percentage,
        );
        let overlay_segment = merged
            .as_ref()
            .map(|merged| (merged.progress, merged.color));
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
        let metrics = self.metrics(window);
        let title_width = metrics.caption(title);
        let mut usage_width = metrics.strong(&label);
        if let Some(value) = usage_amount_label(usage_amount, style.show_usage_values) {
            usage_width += 5.0 + metrics.caption(&value);
        }
        if let Some(merged) = merged.as_ref() {
            usage_width = Self::merged_usage_width(&metrics, merged);
        }
        let usage_el = |this: &Self| match merged.as_ref() {
            Some(merged) => this.merged_usage(merged),
            None => this.usage_label(label.clone(), usage_amount, style.show_usage_values),
        };
        let reset_width = match limit.resets_at {
            Some(at) => metrics.icon_status(&format_reset_in(Some(at))),
            None => metrics.caption(crate::i18n::tr("session-not-started")),
        };
        let single_line = !title.contains(['\n', '\r']);
        // Title, usage and reset on one row; pace or a merged sub-limit forces
        // the two-row layout.
        let one_row_fits = single_line
            && pace.is_none()
            && merged.is_none()
            && compact_title_fits(available_width, title_width, usage_width, reset_width);
        let title_alone_fits = single_line && title_width <= available_width;
        let title_el = || nowrap(caption(title.to_owned(), secondary));
        let one_row = |this: &Self| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .w_full()
                .gap(px(10.0))
                .child(div().flex_1().min_w_0().child(title_el()))
                .child(usage_el(this))
                .child(this.reset_status(limit))
        };
        let title_block =
            components::fit_text(title_alone_fits, caption(title.to_owned(), secondary));
        let header = match pace {
            Some(pace) => {
                let fits = title_alone_fits
                    && TextMetrics::fits_split(
                        available_width,
                        title_width,
                        metrics.pace_status(pace),
                    );
                components::adaptive_split(
                    fits,
                    title_block,
                    components::pace_status(pace, &palette),
                )
            }
            None => div().child(title_block),
        };
        let footer = if show_reset {
            components::adaptive_split(
                TextMetrics::fits_split(available_width, usage_width, reset_width),
                usage_el(self),
                self.reset_status(limit),
            )
        } else {
            usage_el(self)
        };

        if style.compact {
            let mut element = card(&palette).relative().overflow_hidden();
            for layer in components::compact_progress_layers_with(
                progress,
                overlay_segment,
                pace.map(|pace| pace.percent as f32),
                interval_tick_count(limit),
                palette.accent,
                &palette,
            ) {
                element = element.child(layer);
            }
            // Copy that does not fit keeps the compact fill and moves values below the title.
            let content = if one_row_fits {
                one_row(self)
            } else {
                div().flex().flex_col().w_full().child(header).child(footer)
            };
            return element
                .child(div().relative().p(px(12.0)).child(content))
                .into_any_element();
        }
        if exhausted && one_row_fits {
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
            .child(components::progress_track_with(
                progress,
                overlay_segment,
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
        title: &'static str,
        limit: &LimitWindow,
        credits: Option<&crate::quota::limits::CloudSessionCredits>,
        style: CardStyle,
        available_width: f32,
        window: &Window,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let metrics = self.metrics(window);
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
        let mut metadata_width: f32 = 0.0;
        if let Some(expires) = limit.resets_at {
            let local = expires.with_timezone(&Local);
            let date = format!(
                "{}, {}",
                crate::i18n::month_day(local),
                TimeFormat::current().format_hm(local)
            );
            metadata_width = metrics.caption(&date);
            metadata = metadata.child(card_metadata(date, &palette));
            if available {
                let countdown = format_reset_in(Some(expires));
                metadata_width = metadata_width
                    .max(metrics.status_row(crate::i18n::tr("expires-in"), &countdown));
                metadata = metadata.child(status_row(
                    crate::i18n::tr("expires-in"),
                    countdown,
                    &palette,
                ));
            }
        }
        let title_width = metrics.caption(title);
        let label_width = metrics.strong(&label);
        let fits = |width: f32| TextMetrics::fits_split(available_width, width, metadata_width);
        let label_el = nowrap(components::body_strong(label, palette.accent_text));
        // Long copy moves to its own rows instead of being clipped.
        let content = if fits(title_width.max(label_width)) {
            components::split_row(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .child(nowrap(caption(title, palette.text_secondary)))
                    .child(label_el),
                metadata,
            )
        } else {
            div()
                .flex()
                .flex_col()
                .w_full()
                .gap(px(2.0))
                .child(components::fit_text(
                    title_width <= available_width,
                    caption(title, palette.text_secondary),
                ))
                .child(components::adaptive_split(
                    fits(label_width),
                    label_el,
                    metadata,
                ))
        };

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
        let metrics = self.metrics(window);
        let mut right_width;
        let mut right = div().flex().flex_col().items_end().gap(px(2.0));
        let mut show_masked_key = true;
        if expired {
            let label =
                expires_at.map_or_else(|| crate::i18n::tr("expired").into(), format_expired_at);
            right_width = metrics.body(&label);
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
                let used = format_micro_usd(spending.used_microusd);
                spending.limit_microusd.map_or_else(
                    || used.clone(),
                    |limit| format!("{used} / {}", format_micro_usd(limit)),
                )
            } else {
                // Unknown usage — never show a bare limit that looks like spend.
                spending.limit_microusd.map_or_else(
                    || "?.??".into(),
                    |limit| format!("?.?? / {}", format_micro_usd(limit)),
                )
            };
            right_width = metrics.body(crate::i18n::tr("usage")) + 6.0 + metrics.strong(&amount);
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
                    .child(components::body_strong(amount, palette.accent_text)),
            );
            let expires_soon = expires_at.filter(|at| *at > Utc::now());
            show_masked_key = !(spending.resets_at.is_some() && expires_soon.is_some());
            let statuses = [
                spending.resets_at.map(|at| ("resets-in", at)),
                expires_soon.map(|at| ("expires-in", at)),
            ]
            .into_iter()
            .flatten()
            .map(|(label, at)| (crate::i18n::tr(label), format_reset_in(Some(at))))
            .collect::<Vec<_>>();
            if !statuses.is_empty() {
                let widths = statuses
                    .iter()
                    .map(|(label, value)| metrics.status_row(label, value))
                    .collect::<Vec<_>>();
                let bullet = 12.0 + metrics.caption("•");
                let inline_width = widths.iter().sum::<f32>() + bullet * (widths.len() - 1) as f32;
                // Reset and expiry share a row only while it fits the card.
                let inline = inline_width <= available_width;
                let mut meta = if inline {
                    div().flex().flex_row().items_center().gap(px(6.0))
                } else {
                    div().flex().flex_col().items_end().gap(px(2.0))
                };
                for (index, (label, value)) in statuses.into_iter().enumerate() {
                    if inline && index > 0 {
                        meta = meta.child(card_metadata("•", &palette));
                    }
                    meta = meta.child(status_row(label, value, &palette));
                }
                right = right.child(meta);
                right_width = right_width.max(if inline {
                    inline_width
                } else {
                    widths.iter().copied().fold(0.0, f32::max)
                });
            }
        }
        let title_fits = !title.contains(['\n', '\r'])
            && TextMetrics::fits_split(available_width, metrics.caption(title), right_width);
        let title_label = caption(title.to_owned(), palette.text_secondary);
        let mut left = div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(components::fit_text(title_fits, title_label));
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
        let mut button = div()
            .id(eid(format!("openrouter-delete-{account_id}-{key_id}")))
            .relative()
            .size(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(palette.control_radius))
            .on_hover(self.hover_listener(hover_id, Some(crate::i18n::tr("remove-key").into()), cx))
            .on_click(move |_: &ClickEvent, _, _| {
                let account_id = account_id.clone();
                let key_id = key_id.clone();
                std::thread::spawn(move || {
                    crate::app::actions::forget_openrouter_api_key(account_id, key_id)
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

    /// Every layout shows the reset count and countdown without an expiry date.
    fn render_banked_resets(
        &mut self,
        provider: ProviderId,
        limits: &RateLimits,
        expansion_key: &str,
        available_width: f32,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let metrics = self.metrics(window);
        let pad_x = 12.0;
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
        // The nearest expiry sits inline after the count as a clock and
        // countdown; hovering it shows the full expiry date and time.
        let expiration_at = limits.next_reset_credit_expiration();
        let expiration = expiration_at.map(|at| format_reset_in(Some(at)));
        let expiration_tip = expiration_at.map(|at| {
            let local = at.with_timezone(&Local);
            SharedString::from(format!(
                "{}, {}",
                crate::i18n::date_with_year(local),
                TimeFormat::current().format_hm(local)
            ))
        });
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
        // Listed credits differ (a model-only grant versus a full limit
        // reset), so even a single one can be expanded to read its title.
        let expandable = count > 0 && !available.is_empty();
        let open = expandable && self.open_reset_card.as_deref() == Some(expansion_key);
        let reveal = self
            .fx
            .toggle(fx::key(("reset-reveal", expansion_key)), open, fx::NORMAL);

        // Count label, then the 8 DIP gap and clock countdown, then the 8 DIP
        // gap and 16 DIP chevron.
        let title_width = metrics.strong(&count_label)
            + expiration
                .as_deref()
                .map_or(0.0, |value| 8.0 + metrics.icon_status(value))
            + if expandable { 24.0 } else { 0.0 };
        let mut title =
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(components::fit_text(
                    title_width <= available_width,
                    components::body_strong(count_label, palette.accent_text),
                ));
        if let Some(value) = expiration {
            title = title.child(
                components::icon_status("fluent-clock", value, &palette)
                    .id(eid(format!("reset-expiry-{expansion_key}")))
                    .on_hover(self.hover_listener(
                        fx::key(("reset-expiry", expansion_key)),
                        expiration_tip,
                        cx,
                    )),
            );
        }
        if expandable {
            title = title.child(
                icon("fluent-chevron-down", 16.0, theme_gray()).with_transformation(
                    Transformation::rotate(radians(std::f32::consts::PI * reveal)),
                ),
            );
        }
        // Redemption that needs a listed credit re-reads the inventory for the
        // server-selected grant, so a cached snapshot without `next_credit_id`
        // still qualifies.
        let busy = self.reset_busy.contains(&provider);
        let confirming = self.reset_confirm == Some(provider);
        let can_use = count > 0
            && self.ui.settings.instance(provider).is_some_and(|instance| {
                !instance.uses_manual_credential()
                    && match crate::providers::registry::descriptor(provider.kind()).banked_resets {
                        crate::providers::registry::BankedResets::Unsupported => false,
                        crate::providers::registry::BankedResets::ByCount => true,
                        crate::providers::registry::BankedResets::ByListedCredit => {
                            !available.is_empty()
                        }
                    }
            });
        let header_content = if can_use || busy {
            // The action sits on the trailing edge. When the full label does
            // not fit beside the title (long translations), it shortens to
            // just "Use" before the button wraps onto its own row.
            let button_width = |label: &str| metrics.strong(label) + 24.0;
            let label = if busy {
                crate::i18n::tr("reset-using")
            } else {
                let full = crate::i18n::tr("use-reset");
                if TextMetrics::fits_split(available_width, title_width, button_width(full)) {
                    full
                } else {
                    crate::i18n::tr("use-reset-short")
                }
            };
            let button_id = fx::key(("use-reset", expansion_key));
            let button_hover = self.fx.toggle(
                fx::key(("use-reset-hover", button_id)),
                can_use && !busy && self.hovered(button_id),
                fx::FASTER,
            );
            let mut button = div()
                .id(eid(format!("use-reset-{expansion_key}")))
                .flex_none()
                .px(px(12.0))
                .py(px(5.0))
                .rounded(px(palette.control_radius))
                .bg(palette.accent.opacity(if confirming {
                    0.26
                } else {
                    0.14 + 0.1 * button_hover
                }))
                .child(components::nowrap(components::body_strong(
                    label,
                    palette.accent_text,
                )));
            if busy {
                button = button.opacity(0.6);
            } else if can_use {
                button = button
                    .cursor_pointer()
                    .on_hover(self.hover_listener(button_id, None, cx))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.reset_confirm = if this.reset_confirm == Some(provider) {
                            None
                        } else {
                            Some(provider)
                        };
                        cx.notify();
                    }));
            }
            let fits = TextMetrics::fits_split(available_width, title_width, button_width(label));
            // Two 20 DIP line boxes pulled 3 DIP together: the card keeps the
            // height of a two-line block when the split fits on one row.
            components::adaptive_split(fits, title, button).min_h(px(37.0))
        } else {
            title
        };
        let hover_id = fx::key(("reset-card", expansion_key));
        let hover = self.fx.toggle(
            fx::key(("reset-card-hover", hover_id)),
            expandable && self.hovered(hover_id),
            fx::FASTER,
        );
        let mut header = div()
            .id(eid(format!("reset-card-{expansion_key}")))
            .relative()
            .px(px(pad_x))
            .py(px(9.0));
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
                    layer.rounded_t(px(palette.card_radius - 1.0))
                } else {
                    layer.rounded(px(palette.card_radius - 1.0))
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
                .gap(px(RESET_ROW_GAP))
                .child(components::rule(&palette));
            // Each row adds a divider and two gaps; the first gap is shared
            // with the header rule, so drop one.
            let mut content_height = -RESET_ROW_GAP;
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
                let (status, status_width) = match credit.expires_at {
                    Some(at) => {
                        let value = format_reset_in(Some(at));
                        let width = metrics.status_row(crate::i18n::tr("expires-in"), &value);
                        (
                            status_row(crate::i18n::tr("expires-in"), value, &palette),
                            width,
                        )
                    }
                    None => {
                        let label = crate::i18n::tr("available-to-use");
                        (card_metadata(label, &palette), metrics.caption(label))
                    }
                };
                let name_width = metrics.body(&name);
                let date_width = metrics.caption(&date);
                content_height += 1.0 + RESET_ROW_GAP * 2.0;
                if TextMetrics::fits_split(
                    available_width,
                    name_width,
                    date_width.max(status_width),
                ) {
                    // Short title: name on the left, date over the countdown
                    // on the right.
                    content_height += CAPTION_LINE + 1.0 + BODY_LINE;
                    rows = rows.child(components::split_row(
                        components::nowrap(components::body(name, palette.text_secondary)),
                        div()
                            .flex()
                            .flex_col()
                            .flex_none()
                            .items_end()
                            .gap(px(1.0))
                            .child(card_metadata(date, &palette))
                            .child(status),
                    ));
                } else {
                    // Long title: it wraps across the full width, and the date
                    // and countdown share the line under it.
                    let name_lines = (name_width * 1.15 / available_width.max(1.0))
                        .ceil()
                        .max(1.0);
                    let meta_fits =
                        TextMetrics::fits_split(available_width, date_width, status_width);
                    content_height += name_lines * BODY_LINE
                        + 4.0
                        + if meta_fits {
                            BODY_LINE
                        } else {
                            CAPTION_LINE + 4.0 + BODY_LINE
                        };
                    let date = card_metadata(date, &palette);
                    let meta = if meta_fits {
                        components::split_row(date, status)
                    } else {
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(date)
                            .child(div().flex().flex_row().justify_end().child(status))
                    };
                    rows = rows.child(
                        div()
                            .flex()
                            .flex_col()
                            .w_full()
                            .gap(px(4.0))
                            .child(components::body(name, palette.text_secondary))
                            .child(meta),
                    );
                }
            }
            let cap = (content_height + 12.0 + 8.0) * reveal;
            element = element.child(
                div()
                    .overflow_hidden()
                    .max_h(px(cap))
                    .opacity(reveal)
                    .child(
                        // A definite width: an unbounded title would inflate
                        // the section's max-content width and squeeze the
                        // provider heading instead of wrapping.
                        div()
                            .w(px(available_width + pad_x * 2.0))
                            .px(px(pad_x))
                            .pb(px(12.0))
                            .child(rows),
                    ),
            );
        }
        let confirm_reveal = self.fx.toggle(
            fx::key(("reset-confirm", expansion_key)),
            confirming,
            fx::NORMAL,
        );
        if confirm_reveal > 0.001 {
            let message = crate::i18n::tr("reset-confirm-message");
            let height = (metrics.body(message) / available_width.max(1.0)).ceil() * 22.0 + 72.0;
            let confirm_id = fx::key(("confirm-reset", expansion_key));
            let confirm_hover = self.fx.toggle(
                fx::key(("confirm-reset-hover", confirm_id)),
                self.hovered(confirm_id),
                fx::FASTER,
            );
            let cancel_id = fx::key(("cancel-reset", expansion_key));
            let cancel_hover = self.fx.toggle(
                fx::key(("cancel-reset-hover", cancel_id)),
                self.hovered(cancel_id),
                fx::FASTER,
            );
            let actions = div()
                .flex()
                .flex_row()
                .flex_wrap()
                .justify_end()
                .gap(px(6.0))
                .child(
                    div()
                        .id(eid(format!("cancel-reset-{expansion_key}")))
                        .cursor_pointer()
                        .px(px(12.0))
                        .py(px(5.0))
                        .rounded(px(palette.control_radius))
                        .bg(palette.subtle_fill.opacity(cancel_hover))
                        .child(components::nowrap(components::body(
                            crate::i18n::tr("cancel"),
                            palette.text_secondary,
                        )))
                        .on_hover(self.hover_listener(cancel_id, None, cx))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.reset_confirm = None;
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .id(eid(format!("confirm-reset-{expansion_key}")))
                        .cursor_pointer()
                        .px(px(12.0))
                        .py(px(5.0))
                        .rounded(px(palette.control_radius))
                        .bg(palette.accent.opacity(0.85 + 0.15 * confirm_hover))
                        .child(components::nowrap(components::body_strong(
                            crate::i18n::tr("use-reset"),
                            palette.text_on_accent,
                        )))
                        .on_hover(self.hover_listener(confirm_id, None, cx))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            if this.reset_confirm == Some(provider) {
                                this.use_reset(provider, cx);
                            }
                        })),
                );
            element = element.child(
                div()
                    .overflow_hidden()
                    .max_h(px(height * confirm_reveal))
                    .opacity(confirm_reveal)
                    .child(
                        div()
                            .w(px(available_width + pad_x * 2.0))
                            .px(px(pad_x))
                            .pb(px(9.0))
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(components::rule(&palette))
                            .child(components::body(message, palette.text_secondary))
                            .child(actions),
                    ),
            );
        }
        let status = self.reset_status.get(&provider).map(|(status, at)| {
            (
                status.message(),
                at.elapsed() < super::root::RESET_STATUS_TTL,
            )
        });
        let status_reveal = self.fx.toggle(
            fx::key(("reset-status", expansion_key)),
            status.as_ref().is_some_and(|(_, fresh)| *fresh) && !busy,
            fx::NORMAL,
        );
        if let Some((status, _)) = status.filter(|_| status_reveal > 0.001) {
            let height = (metrics.body(&status) / available_width.max(1.0)).ceil() * 24.0 + 64.0;
            element = element.child(
                div()
                    .overflow_hidden()
                    .max_h(px(height * status_reveal))
                    .opacity(status_reveal)
                    .child(
                        // A definite width: unbounded text would inflate the
                        // section's max-content width and squeeze its heading.
                        div()
                            .w(px(available_width + pad_x * 2.0))
                            .px(px(pad_x))
                            .pb(px(9.0))
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(components::rule(&palette))
                            .child(components::body(status, palette.text_secondary)),
                    ),
            );
        }
        element.into_any_element()
    }

    fn render_provider_incidents(
        &mut self,
        status: &crate::providers::status::ProviderStatus,
        available_width: f32,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use crate::providers::status::IncidentImpact;

        let palette = self.palette.clone();
        let metrics = self.metrics(window);
        let card_key = format!("provider-incidents-{:?}", status.kind);
        let hover_id = fx::key(card_key.as_str());
        let hover = self.fx.toggle(
            fx::key(("provider-incidents-hover", hover_id)),
            self.hovered(hover_id),
            fx::FASTER,
        );
        let today = Local::now().date_naive();
        // A definite width: unbounded text would inflate the max-content
        // width of the whole provider section, and its heading resolves the
        // account column's percentage against that (squeezing the name).
        let mut rows = div().flex().flex_col().gap(px(10.0)).w(px(available_width));
        // Reveal cap: card padding, then per row the summary line, the
        // wrapped title, a possible metadata line and the row gap.
        let mut reveal_height = 24.0;
        for incident in &status.incidents {
            let severity = match incident.impact {
                IncidentImpact::Maintenance => palette.text_secondary,
                IncidentImpact::Degraded | IncidentImpact::PartialOutage => palette.caution,
                IncidentImpact::MajorOutage => palette.critical,
            };
            let impact = incident.impact.label();
            let since = incident.started_at.map(|started| {
                let local = started.with_timezone(&Local);
                let time = TimeFormat::current().format_hm(local);
                let time = if local.date_naive() == today {
                    time.to_string()
                } else {
                    format!("{}, {time}", crate::i18n::month_day(local))
                };
                crate::i18n::format("status-since", &[("time", time)])
            });
            let metadata = std::iter::once(incident.stage.label().to_owned())
                .chain(since)
                .collect::<Vec<_>>()
                .join(" · ");
            let affected = (!incident.components.is_empty())
                .then(|| format!(" · {}", incident.components.join(", ")));
            let summary_width = metrics.caption(impact)
                + affected
                    .as_deref()
                    .map_or(0.0, |affected| metrics.caption(affected));
            // The metadata shares the summary line while both fit, and
            // otherwise gets its own line under the title.
            let fits =
                TextMetrics::fits_split(available_width, summary_width, metrics.caption(&metadata));
            let mut summary = div()
                .flex()
                .flex_row()
                .min_w_0()
                .overflow_hidden()
                .child(components::caption_strong(impact, severity).flex_none());
            if let Some(affected) = affected {
                summary = summary.child(nowrap(caption(affected, palette.text_tertiary)));
            }
            let title = components::body_strong(incident.title.clone(), palette.text_primary)
                .w(px(available_width));
            let mut content = div().flex().flex_col().gap(px(1.0));
            content = if fits {
                content
                    .child(components::split_row(
                        summary,
                        card_metadata(metadata, &palette),
                    ))
                    .child(title)
            } else {
                content
                    .child(summary)
                    .child(title)
                    .child(card_metadata(metadata, &palette))
            };
            reveal_height += 16.0
                + (metrics.strong(&incident.title) / available_width.max(1.0)).ceil() * 20.0
                + if fits { 0.0 } else { 16.0 }
                + 10.0;
            let row_id = fx::key(("provider-incident-row", incident.id.as_str()));
            let url = incident.url.clone();
            rows = rows.child(
                div()
                    .id(eid(format!("provider-incident-{}", incident.id)))
                    .on_hover(self.hover_listener(
                        row_id,
                        Some(crate::i18n::tr("open-status-page").into()),
                        cx,
                    ))
                    .on_click(move |_: &ClickEvent, _, _| {
                        if let Err(error) = crate::platform::shell::open_url(&url) {
                            crate::platform::logger::info(format!(
                                "failed to open status page {url}: {error:#}"
                            ));
                        }
                    })
                    .child(content),
            );
        }
        let mut element = card(&palette)
            .id(eid(card_key))
            .relative()
            .on_hover(self.hover_listener(hover_id, None, cx));
        if let Some(layer) = components::hover_layer(&palette, hover, palette.card_radius - 1.0) {
            element = element.child(layer);
        }
        let element = element.child(div().relative().p(px(12.0)).child(rows));
        // The heading renders first, so this only grows in for an incident
        // that arrives while its section is already on screen.
        let reveal = self.fx.appear_in(
            fx::key(("provider-incidents-reveal", hover_id)),
            fx::key(("provider-section", status.kind)),
            fx::NORMAL,
        );
        if reveal >= 1.0 {
            return element.into_any_element();
        }
        div()
            .overflow_hidden()
            .max_h(px(reveal_height * reveal))
            .opacity(reveal)
            .child(element)
            .into_any_element()
    }

    /// One announced reset: a kind tile tinted by status, the title on the
    /// full width, and kind/date with a status chip on the second line.
    fn render_announced_reset(
        &mut self,
        row: &AnnouncedResetRow<'_>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use crate::quota::reset_detection::{ResetKind, ResetTiming};
        /// Edge padding, also used between the tile and the text.
        const INSET: f32 = 12.0;
        /// Title line + line gap + caption line, so the tile spans the text
        /// block exactly and keeps the same inset on every side.
        const TEXT_GAP: f32 = 2.0;
        const TILE: f32 = 20.0 + TEXT_GAP + 16.0;
        const TILE_GLYPH: f32 = 20.0;
        const CHIP_GLYPH: f32 = 12.0;
        const CARD_HEIGHT: f32 = INSET * 2.0 + TILE;

        let palette = self.palette.clone();
        let reset = row.reset;
        let local = reset.reset_at.with_timezone(&Local);
        let when = format!(
            "{}, {}",
            crate::i18n::month_day(local),
            TimeFormat::current().format_hm(local)
        );
        let date = match reset.timing {
            ResetTiming::Exact => when,
            ResetTiming::Before => crate::i18n::format("before-when", &[("when", when)]),
        };
        let (glyph, kind, fallback_label) = match reset.kind {
            ResetKind::Forced => (
                "fluent-arrow-reset",
                crate::i18n::tr("tibo-reset"),
                crate::i18n::tr("codex-limits"),
            ),
            ResetKind::Banked => (
                "fluent-building-bank",
                crate::i18n::tr("tibo-banked-reset"),
                crate::i18n::tr("banked-reset-credit"),
            ),
        };
        let label = reset
            .label
            .as_deref()
            .filter(|label| !label.trim().is_empty())
            .unwrap_or(fallback_label)
            .to_owned();
        let (color, chip_glyph, chip_text, title_color) = match row.phase {
            AnnouncedResetPhase::Upcoming => {
                let countdown = format_reset_in(Some(reset.reset_at));
                let (glyph, text) = match reset.timing {
                    ResetTiming::Exact => ("fluent-clock", countdown),
                    ResetTiming::Before => ("fluent-hourglass", format!("≤ {countdown}")),
                };
                (palette.accent_text, glyph, text, palette.accent_text)
            }
            AnnouncedResetPhase::Missed => (
                palette.caution,
                "fluent-warning",
                format_time_late(reset.reset_at),
                palette.text_primary,
            ),
            AnnouncedResetPhase::Received(at) => (
                palette.positive,
                "fluent-checkmark-circle",
                format_time_ago(at),
                palette.text_primary,
            ),
        };

        let hover_id = fx::key(("forced-reset", reset.id.as_str()));
        let hover = self.fx.toggle(
            fx::key(("forced-reset-hover", hover_id)),
            self.hovered(hover_id),
            fx::FASTER,
        );
        let tip: SharedString = if reset.source_url.is_some() {
            crate::i18n::tr("open-announcement-source").into()
        } else {
            crate::i18n::tr("source-not-provided").into()
        };
        let source = reset.source_url.clone();
        let tile = div()
            .size(px(TILE))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            // Concentric with the card corner across the inset.
            .rounded(px((palette.card_radius - 2.0).max(4.0)))
            .bg(color.opacity(0.14))
            .child(icon(glyph, TILE_GLYPH, color));
        let chip = div()
            .flex()
            .flex_row()
            .flex_none()
            .items_center()
            .gap(px(4.0))
            .px(px(6.0))
            .rounded_full()
            .bg(color.opacity(0.14))
            .child(icon(chip_glyph, CHIP_GLYPH, color))
            .child(nowrap(components::caption_strong(chip_text, color)));
        let details = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(
                nowrap(caption(format!("{kind} · {date}"), palette.text_secondary))
                    .flex_1()
                    .min_w_0(),
            )
            .child(chip);
        let content = div()
            .relative()
            .p(px(INSET))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(INSET))
            .child(tile)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(TEXT_GAP))
                    .child(nowrap(components::body_strong(label, title_color)))
                    .child(details),
            );
        let mut element = card(&palette)
            .id(eid(format!("forced-reset-{}", reset.id)))
            .relative()
            .on_hover(self.hover_listener(hover_id, Some(tip), cx))
            .on_click(move |_: &ClickEvent, _, _| {
                if let Some(url) = source.as_deref()
                    && let Err(error) = crate::platform::shell::open_url(url)
                {
                    crate::platform::logger::info(format!(
                        "failed to open forced reset source {url}: {error:#}"
                    ));
                }
            });
        if let Some(layer) = components::hover_layer(&palette, hover, palette.card_radius - 1.0) {
            element = element.child(layer);
        }
        let element = element.child(content);
        // An announcement that arrives while its provider section is on
        // screen grows in instead of pushing the cards below it down at once.
        let reveal = self.fx.appear_in(
            fx::key(("forced-reset-reveal", reset.id.as_str())),
            fx::key(("provider-section", row.provider.kind())),
            fx::NORMAL,
        );
        if reveal >= 1.0 {
            return element.into_any_element();
        }
        div()
            .overflow_hidden()
            .max_h(px(CARD_HEIGHT * reveal))
            .opacity(reveal)
            .child(element)
            .into_any_element()
    }
}

/// Ring tiles sit two to a row while the card is wide enough. Cells stretch
/// to the row's height and each tile stretches inside its cell, so paired
/// tiles always match heights.
fn flush_rings(out: &mut Vec<AnyElement>, rings: &mut Vec<AnyElement>, paired: bool) {
    let per_row = if paired { 2 } else { 1 };
    let mut tiles = std::mem::take(rings).into_iter().peekable();
    while tiles.peek().is_some() {
        let mut row = div().flex().flex_row().gap(px(6.0)).w_full();
        for tile in tiles.by_ref().take(per_row) {
            row = row.child(div().flex_1().min_w_0().flex().child(tile));
        }
        out.push(row.into_any_element());
    }
}

/// Track, clockwise value arc from 12 o'clock, and a radial pace tick the
/// width of the stroke.
#[allow(clippy::too_many_arguments)]
fn paint_ring(
    bounds: Bounds<Pixels>,
    percent: f32,
    overlay: Option<(f32, Hsla)>,
    pace: Option<f32>,
    color: Hsla,
    marker: Hsla,
    window: &mut Window,
) {
    use std::f32::consts::PI;
    let cx = f32::from(bounds.origin.x) + f32::from(bounds.size.width) / 2.0;
    let cy = f32::from(bounds.origin.y) + f32::from(bounds.size.height) / 2.0;
    let radius = (RING_SIZE - RING_STROKE) / 2.0;
    let at = |radius: f32, fraction: f32| {
        let angle = -PI / 2.0 + fraction * 2.0 * PI;
        point(px(cx + radius * angle.cos()), px(cy + radius * angle.sin()))
    };
    let arc = |fraction: f32, color: Hsla, window: &mut Window| {
        let fraction = fraction.clamp(0.0, 1.0);
        if fraction <= 0.0 {
            return;
        }
        let mut builder = PathBuilder::stroke(px(RING_STROKE));
        let size = point(px(radius), px(radius));
        builder.move_to(at(radius, 0.0));
        if fraction >= 0.999 {
            builder.arc_to(size, px(0.0), false, true, at(radius, 0.5));
            builder.arc_to(size, px(0.0), false, true, at(radius, 0.0));
        } else {
            builder.arc_to(size, px(0.0), fraction > 0.5, true, at(radius, fraction));
        }
        if let Ok(path) = builder.build() {
            window.paint_path(path, color);
        }
    };
    arc(1.0, color.opacity(0.2), window);
    arc(percent / 100.0, color, window);
    if let Some((overlay, overlay_color)) = overlay {
        arc(overlay.min(percent) / 100.0, overlay_color, window);
    }
    if let Some(pace) = pace {
        let fraction = pace.clamp(0.0, 100.0) / 100.0;
        let mut builder = PathBuilder::stroke(px(2.0));
        builder.move_to(at(radius - RING_STROKE / 2.0, fraction));
        builder.line_to(at(radius + RING_STROKE / 2.0, fraction));
        if let Ok(path) = builder.build() {
            window.paint_path(path, marker);
        }
    }
}

/// Diameter of the color dot and its gap to the text in [`overlay_legend`].
const OVERLAY_LEGEND_DOT: f32 = 6.0;
const OVERLAY_LEGEND_GAP: f32 = 4.0;
/// Gaps inside and between the name/value pairs of a merged card.
const MERGED_USAGE_GAP: f32 = 4.0;
const MERGED_USAGE_PAIR_GAP: f32 = 10.0;

/// A sub-limit merged into its parent's card, resolved for one frame.
struct MergedLimit {
    /// Animated segment length in parent-bar percent.
    progress: f32,
    color: Hsla,
    /// Ring layout legend, such as `Fable only 61%`.
    legend: String,
    parent_percent: String,
    name: String,
    percent: String,
    /// Localized `left` / `used`.
    mode: &'static str,
}

/// Color dot and label of a sub-limit segment drawn inside a quota bar.
fn overlay_legend(legend: String, color: Hsla, palette: &super::theme::Palette) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(OVERLAY_LEGEND_GAP))
        .whitespace_nowrap()
        .child(
            div()
                .size(px(OVERLAY_LEGEND_DOT))
                .flex_none()
                .rounded_full()
                .bg(color),
        )
        .child(caption(legend, palette.text_tertiary))
}

fn theme_gray() -> gpui::Hsla {
    crate::ui::gui::color::rgb8((138, 138, 138))
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
