use super::*;

pub(super) fn provider_cards(
    provider: ProviderKind,
    is_first: bool,
    limits: &RateLimits,
    show_used_percentage: bool,
    show_usage_pace: bool,
    popup_visibility: &PopupVisibility,
    surface: PopupSurface,
    show_provider_tabs: bool,
    show_account_name: bool,
    color_scheme: ColorScheme,
    drag_handle: Option<Element>,
    openrouter_actions: Option<OpenRouterPopupActions>,
    provider_error: Option<(&str, Callback<()>)>,
) -> Vec<Element> {
    let (monthly_label, primary_label, secondary_label) = match provider {
        ProviderKind::Cursor => ("Auto + Composer", "Auto + Composer", "Auto + Composer"),
        ProviderKind::OpenRouter => ("Spending", "Spending", "Spending"),
        _ => ("Monthly", "5h Session", "Weekly"),
    };
    let mut trailing: Vec<Element> = Vec::new();
    if show_account_name {
        if let Some(name) = limits.account_name.as_ref() {
            trailing.push(
                caption(name.clone())
                    .foreground(ThemeRef::TertiaryText)
                    .horizontal_alignment(HorizontalAlignment::Right)
                    .vertical_alignment(VerticalAlignment::Center)
                    .into(),
            );
        }
    }
    if let Some(handle) = drag_handle {
        trailing.push(handle);
    }
    let mut title_parts: Vec<Element> = vec![
        body_strong(provider.display_name())
            .foreground(ThemeRef::SecondaryText)
            .vertical_alignment(VerticalAlignment::Center)
            .into(),
    ];
    if let Some(plan) = limits
        .plan_type
        .as_deref()
        .filter(|plan| !plan.trim().is_empty())
    {
        title_parts.push(
            text_block(capitalize_plan_name(plan))
                .font_weight(400)
                .foreground(ThemeRef::TertiaryText)
                .vertical_alignment(VerticalAlignment::Center)
                .into(),
        );
    }
    let has_provider_error = provider_error.is_some();
    if let Some((_, on_error)) = provider_error {
        title_parts.push(
            provider_error_badge(16.0, on_error).vertical_alignment(VerticalAlignment::Center),
        );
    }
    let title_row = hstack(title_parts)
        .spacing(4.0)
        .vertical_alignment(VerticalAlignment::Center)
        .grid_column(0);
    let mut heading_cells: Vec<Element> = vec![title_row.into()];
    if !trailing.is_empty() {
        heading_cells.push(
            hstack(trailing)
                .spacing(4.0)
                .horizontal_alignment(HorizontalAlignment::Right)
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(1)
                .into(),
        );
    }
    let mut cards: Vec<Element> = vec![
        grid(heading_cells)
            .columns([GridLength::Star(1.0), GridLength::Auto])
            .rows([GridLength::Auto])
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .margin(Thickness {
                left: 4.0,
                top: if is_first { 0.0 } else { 8.0 },
                right: 4.0,
                bottom: 2.0,
            })
            .with_key(format!(
                "{}-heading-{}-{}",
                provider.display_name(),
                if is_first { "first" } else { "rest" },
                has_provider_error,
            ))
            .into(),
    ];
    if provider == ProviderKind::OpenRouter {
        let spending_visible =
            popup_visibility.is_visible(&spending_brick_id(provider), surface, show_provider_tabs);
        if spending_visible {
            if !limits.openrouter_accounts.is_empty() {
                // Nest each account as its own keyed strip. A flat list of headings
                // + key cards lets WinUI recycle siblings across account boundaries
                // when membership flickers (keys loading/failing), so TEST2 can
                // visually land under Pixelscan. Nested strips keep heading+keys
                // glued together; remount the strip when its key set changes.
                for account in &limits.openrouter_accounts {
                    let mut account_strip: Vec<Element> = Vec::new();
                    account_strip.push(
                        openrouter_account_heading(account)
                            .with_key(format!("{}-account-heading", account.id)),
                    );
                    let mut key_identity = String::new();
                    for (index, api_key) in account.api_keys.iter().enumerate() {
                        let title = api_key
                            .label
                            .as_deref()
                            .map(str::trim)
                            .filter(|label| !label.is_empty())
                            .map(str::to_owned)
                            .unwrap_or_else(|| format!("Key {}", index + 1));
                        let masked = api_key.masked_key.as_deref().unwrap_or("");
                        let now = openrouter_actions
                            .as_ref()
                            .map(|actions| actions.now)
                            .unwrap_or_else(Utc::now);
                        let expired = api_key.is_expired(now);
                        if !key_identity.is_empty() {
                            key_identity.push('\u{1f}');
                        }
                        key_identity.push_str(&api_key.id);
                        key_identity.push('\u{1e}');
                        key_identity.push_str(&title);
                        key_identity.push('\u{1e}');
                        key_identity.push_str(masked);
                        key_identity.push('\u{1e}');
                        key_identity.push_str(if expired { "expired" } else { "live" });
                        let on_delete = openrouter_actions.as_ref().map(|actions| {
                            let account_id = account.id.clone();
                            let key_id = api_key.id.clone();
                            let settings_tx = actions.settings_tx.clone();
                            move || {
                                remove_openrouter_api_key(
                                    account_id.clone(),
                                    key_id.clone(),
                                    settings_tx.clone(),
                                );
                            }
                        });
                        let delete_chrome = openrouter_actions.as_ref().map(|actions| {
                            (
                                format!("openrouter-delete-{}-{}", account.id, api_key.id),
                                actions.hovered_action.clone(),
                                actions.set_hovered_action.clone(),
                            )
                        });
                        account_strip.push(
                            spending_card_with_title(
                                title.clone(),
                                api_key.masked_key.as_deref(),
                                &api_key.spending,
                                api_key.has_live_usage,
                                show_used_percentage,
                                color_scheme,
                                expired,
                                api_key.expires_at,
                                on_delete,
                                delete_chrome,
                            )
                            // Identity includes glyph-like content (title/mask) so a
                            // recycled native card cannot keep a neighbor's text.
                            .with_key(format!(
                                "{}-api-{}-{}-{}-{}",
                                account.id,
                                api_key.id,
                                title,
                                masked,
                                if expired { "expired" } else { "live" }
                            )),
                        );
                    }
                    cards.push(
                        vstack(account_strip)
                            .spacing(6.0)
                            .with_key(format!(
                                "openrouter-account-strip-{}-{}",
                                account.id, key_identity
                            ))
                            .into(),
                    );
                }
            } else if let Some(spending) = limits.spending.as_ref() {
                cards.push(
                    spending_card(spending, show_used_percentage, color_scheme)
                        .with_key(format!("{}-spending", provider.display_name())),
                );
            }
        }
        return cards;
    }
    // Cursor usage is fetched from a remote CSV export rather than scanned
    // from a local session log. Keep its card visible while that export is
    // still empty or delayed, so the feature does not look like it vanished.
    let usage_brick = usage_brick_id(provider);
    let show_usage_stats = popup_visibility.is_visible(&usage_brick, surface, show_provider_tabs);
    let has_usage_statistics =
        show_usage_stats && (limits.usage.has_data() || provider == ProviderKind::Cursor);
    cards.extend(
        popup_sections(limits, false)
            .into_iter()
            .filter(|section| {
                matches!(
                    section,
                    PopupSection::Monthly | PopupSection::FiveHour | PopupSection::Weekly
                )
            })
            .filter(|section| {
                section_brick_id(provider, *section).is_some_and(|brick_id| {
                    popup_visibility.is_visible(&brick_id, surface, show_provider_tabs)
                })
            })
            .filter_map(|section| {
                let element: Element = match section {
                    PopupSection::Monthly => limit_card(
                        monthly_label,
                        &limits.secondary,
                        show_used_percentage,
                        show_usage_pace,
                        false,
                        color_scheme,
                    ),
                    PopupSection::FiveHour => limit_card(
                        primary_label,
                        &limits.primary,
                        show_used_percentage,
                        show_usage_pace,
                        limits.five_hour_disabled(),
                        color_scheme,
                    ),
                    PopupSection::Weekly => limit_card(
                        secondary_label,
                        &limits.secondary,
                        show_used_percentage,
                        show_usage_pace,
                        false,
                        color_scheme,
                    ),
                    PopupSection::Error => return None,
                    PopupSection::UsageStatistics
                    | PopupSection::BankedResets
                    | PopupSection::Credits => return None,
                };
                Some(element.with_key(format!("{}-{}", provider.display_name(), section.key())))
            }),
    );
    // Claude can return extra windows such as Fable or Opus. They belong with
    // the ordinary limit cards, before banked resets, statistics, or credits.
    let additional_limits = limits.additional_limits.iter().filter_map(|limit| {
        let brick_id = additional_limit_brick_id(provider, &limit.id);
        if !popup_visibility.is_visible(&brick_id, surface, show_provider_tabs) {
            return None;
        }
        Some(
            limit_card(
                &limit.title,
                &limit.window,
                show_used_percentage,
                show_usage_pace,
                false,
                color_scheme,
            )
            .with_key(format!(
                "{}-additional-{}",
                provider.display_name(),
                limit.id
            )),
        )
    });
    cards.extend(additional_limits);
    // Local statistics remain after every rate-limit window.
    if popup_visibility.is_visible(&resets_brick_id(provider), surface, show_provider_tabs)
        && limits.available_reset_count() > 0
    {
        cards.push(
            reset_credits_card(limits)
                .with_key(format!("{}-banked-resets", provider.display_name())),
        );
    }
    if has_usage_statistics {
        cards.push(
            usage_statistics_card(provider, limits)
                .with_key(format!("{}-usage-statistics", provider.display_name())),
        );
    }
    if popup_visibility.is_visible(&credits_brick_id(provider), surface, show_provider_tabs)
        && credits_display_value(limits).is_some()
    {
        cards.push(credits_card(limits).with_key(format!("{}-credits", provider.display_name())));
    }
    cards
}

pub(super) fn spending_card(
    spending: &SpendingSummary,
    show_used_percentage: bool,
    color_scheme: ColorScheme,
) -> Element {
    spending_card_with_title(
        "SPENDING",
        None,
        spending,
        true,
        show_used_percentage,
        color_scheme,
        false,
        None,
        None::<fn()>,
        None,
    )
}

pub(super) fn spending_card_with_title(
    title: impl Into<String>,
    masked_key: Option<&str>,
    spending: &SpendingSummary,
    has_live_usage: bool,
    show_used_percentage: bool,
    color_scheme: ColorScheme,
    expired: bool,
    expires_at: Option<DateTime<Utc>>,
    on_delete: Option<impl IntoUnitCallback>,
    delete_chrome: Option<(String, Option<String>, SetState<Option<String>>)>,
) -> Element {
    let title = title.into().to_uppercase();
    let mut right_side: Vec<Element> = Vec::new();
    // When both reset and expiry countdowns share one row, hide the masked key
    // so the card does not overflow horizontally.
    let mut show_masked_key = true;
    if expired {
        let expired_label = expires_at.map_or_else(|| "expired".into(), format_expired_at);
        let mut expired_row: Vec<Element> = vec![
            text_block(expired_label)
                .foreground(ThemeRef::TertiaryText)
                .vertical_alignment(VerticalAlignment::Center)
                .into(),
        ];
        if let (Some(on_delete), Some((button_id, hovered_action, set_hovered_action))) =
            (on_delete, delete_chrome)
        {
            expired_row.push(openrouter_delete_button(
                button_id,
                color_scheme,
                &hovered_action,
                set_hovered_action,
                on_delete,
            ));
        }
        right_side.push(
            hstack(expired_row)
                .spacing(8.0)
                .horizontal_alignment(HorizontalAlignment::Right)
                .vertical_alignment(VerticalAlignment::Center)
                .into(),
        );
    } else {
        if has_live_usage {
            let used = format_usd(spending.used_microusd as f64 / 1_000_000.0);
            let amount = spending.limit_microusd.map_or_else(
                || used.clone(),
                |limit| format!("{used} / {}", format_usd(limit as f64 / 1_000_000.0)),
            );
            right_side.push(
                hstack((
                    text_block("Usage:")
                        .foreground(ThemeRef::TertiaryText)
                        .vertical_alignment(VerticalAlignment::Center),
                    text_block(amount)
                        .font_weight(600)
                        .foreground(ThemeRef::Accent)
                        .vertical_alignment(VerticalAlignment::Center),
                ))
                .spacing(6.0)
                .horizontal_alignment(HorizontalAlignment::Right)
                .into(),
            );
        } else {
            // Unknown usage — never show a bare limit that looks like spend.
            let amount = spending.limit_microusd.map_or_else(
                || "?.??".into(),
                |limit| format!("?.?? / {}", format_usd(limit as f64 / 1_000_000.0)),
            );
            right_side.push(
                hstack((
                    text_block("Usage:")
                        .foreground(ThemeRef::TertiaryText)
                        .vertical_alignment(VerticalAlignment::Center),
                    text_block(amount)
                        .font_weight(600)
                        .foreground(ThemeRef::Accent)
                        .vertical_alignment(VerticalAlignment::Center),
                ))
                .spacing(6.0)
                .horizontal_alignment(HorizontalAlignment::Right)
                .into(),
            );
        }
        let reset_at = spending.resets_at;
        let expires_soon = expires_at.filter(|at| *at > Utc::now());
        show_masked_key = !(reset_at.is_some() && expires_soon.is_some());
        if reset_at.is_some() || expires_soon.is_some() {
            let mut meta: Vec<Element> = Vec::new();
            if let Some(reset) = reset_at {
                meta.push(
                    text_block("Resets in")
                        .foreground(ThemeRef::TertiaryText)
                        .vertical_alignment(VerticalAlignment::Center)
                        .into(),
                );
                meta.push(
                    text_block(format_reset_in(Some(reset)))
                        .vertical_alignment(VerticalAlignment::Center)
                        .into(),
                );
            }
            if let Some(expires) = expires_soon {
                if reset_at.is_some() {
                    meta.push(
                        text_block("•")
                            .foreground(ThemeRef::TertiaryText)
                            .vertical_alignment(VerticalAlignment::Center)
                            .into(),
                    );
                }
                meta.push(
                    text_block("Expires in")
                        .foreground(ThemeRef::TertiaryText)
                        .vertical_alignment(VerticalAlignment::Center)
                        .into(),
                );
                meta.push(
                    text_block(format_reset_in(Some(expires)))
                        .vertical_alignment(VerticalAlignment::Center)
                        .into(),
                );
            }
            right_side.push(
                hstack(meta)
                    .spacing(6.0)
                    .horizontal_alignment(HorizontalAlignment::Right)
                    .into(),
            );
        }
    }

    let mut title_lines: Vec<Element> =
        vec![caption(title).foreground(ThemeRef::SecondaryText).into()];
    if show_masked_key
        && let Some(masked) = masked_key.map(str::trim).filter(|value| !value.is_empty())
    {
        title_lines.push(
            caption(masked.to_owned())
                .foreground(ThemeRef::TertiaryText)
                .into(),
        );
    }
    let left = vstack(title_lines)
        .spacing(2.0)
        .vertical_alignment(VerticalAlignment::Center);

    let header: Element = if right_side.is_empty() {
        left.into()
    } else {
        grid((
            left,
            vstack(right_side)
                .spacing(2.0)
                .horizontal_alignment(HorizontalAlignment::Right)
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into()
    };

    let mut rows: Vec<Element> = vec![header];
    // Expired keys keep title/mask only — no spend bar that looks "full".
    if !expired && let Some(limit) = spending.limit_microusd.filter(|limit| *limit > 0) {
        let used_percent = if has_live_usage {
            ((spending.used_microusd.min(limit) as f64 / limit as f64) * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };
        let progress = if show_used_percentage {
            used_percent
        } else {
            100.0 - used_percent
        };
        rows.push(rounded_progress(
            progress,
            ThemeRef::Accent,
            None,
            color_scheme,
            0,
        ));
    }

    border(vstack(rows).spacing(8.0))
        .corner_radius(f64::from(popup::WINDOW_CORNER_RADIUS_DIP))
        .padding(Thickness::uniform(12.0))
        .background(ThemeRef::CardBackground)
        .border_thickness(Thickness::uniform(1.0))
        .border_brush(ThemeRef::CardStroke)
        .into()
}

pub(super) fn openrouter_account_heading(account: &OpenRouterAccountSnapshot) -> Element {
    let name = text_block(account.name.clone())
        .font_weight(600)
        .foreground(ThemeRef::SecondaryText)
        .vertical_alignment(VerticalAlignment::Center);
    let content: Element = match account.balance_microusd {
        Some(balance) => grid((
            name,
            text_block(format_usd(balance as f64 / 1_000_000.0))
                .font_weight(600)
                .foreground(ThemeRef::Accent)
                .vertical_alignment(VerticalAlignment::Center)
                .horizontal_alignment(HorizontalAlignment::Right)
                .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into(),
        None => name.into(),
    };
    content
        .margin(Thickness {
            left: 4.0,
            top: 8.0,
            right: 4.0,
            bottom: 0.0,
        })
        .into()
}

/// Stable membership fingerprint for OpenRouter popup chrome. Include account
/// and API-key identities (not live usage) so the provider strip remounts when
/// keys are added/removed/moved, preventing recycled cards under the wrong
/// account heading.
pub(super) fn openrouter_accounts_strip_key(limits: &RateLimits) -> String {
    let mut key = String::new();
    for account in &limits.openrouter_accounts {
        if !key.is_empty() {
            key.push('\u{1d}');
        }
        key.push_str(&account.id);
        key.push('\u{1f}');
        key.push_str(&account.name);
        for api_key in &account.api_keys {
            key.push('\u{1e}');
            key.push_str(&api_key.id);
            if let Some(label) = api_key.label.as_deref() {
                key.push(':');
                key.push_str(label);
            }
            if let Some(masked) = api_key.masked_key.as_deref() {
                key.push('@');
                key.push_str(masked);
            }
            if api_key.expires_at.is_some() || api_key.disabled {
                key.push('#');
                if let Some(expires_at) = api_key.expires_at {
                    key.push_str(&expires_at.timestamp().to_string());
                }
                if api_key.disabled {
                    key.push('!');
                }
            }
        }
    }
    key
}

/// Membership + expired chrome that change popup body height. Usage dollars
/// stay out so a poll cannot remount swap-chain hosts every minute.
pub(super) fn popup_body_height_key(limits: &ProviderLimits, view: PopupView) -> String {
    let mut key = String::new();
    let providers: Vec<ProviderKind> = match view {
        PopupView::Home => crate::provider_registry::PROVIDERS
            .iter()
            .map(|descriptor| descriptor.kind)
            .collect(),
        other => other.provider().into_iter().collect(),
    };
    for provider in providers {
        let snapshot = limits.get(provider);
        if !key.is_empty() {
            key.push('|');
        }
        key.push_str(provider.id());
        key.push(':');
        if provider == ProviderKind::OpenRouter {
            key.push_str(&openrouter_accounts_strip_key(snapshot));
        } else {
            key.push(if snapshot.five_hour_disabled() {
                '0'
            } else {
                '1'
            });
            key.push(if snapshot.spending.is_some() {
                's'
            } else {
                '-'
            });
            key.push(if snapshot.usage.has_data() { 'u' } else { '-' });
        }
    }
    key
}

pub(super) fn latest_sampled_at(limits: &ProviderLimits) -> chrono::DateTime<Utc> {
    crate::provider_registry::PROVIDERS
        .iter()
        .map(|descriptor| limits.get(descriptor.kind).sampled_at)
        .max()
        .unwrap_or_default()
}

/// CSS `#0003` → `#00000033` interval ticks on the usage track.
const INTERVAL_TICK_COLOR: Color = Color {
    a: 0x33,
    r: 0,
    g: 0,
    b: 0,
};

/// Interior ticks that divide a quota window into equal buckets.
///
/// 5-hour bars get hour marks 1–4, weekly bars get day marks 1–6, and
/// monthly bars get three quarter marks (skipping 0% and 100%).
pub(super) fn interval_tick_count(window: &LimitWindow) -> u32 {
    match window.duration_minutes {
        Some(minutes) if minutes <= 12 * 60 => 4,
        Some(minutes) if minutes <= 8 * 24 * 60 => 6,
        Some(_) => 3,
        None => 0,
    }
}

/// Dots at the end of each interior bucket, right-aligned in equal columns.
pub(super) fn interval_ticks_layer(tick_count: u32) -> Option<Element> {
    if tick_count == 0 {
        return None;
    }
    const DOT: f64 = 4.0;
    let segments = (tick_count + 1) as usize;
    let dots: Vec<Element> = (0..tick_count)
        .map(|index| {
            border(Element::Empty)
                .width(DOT)
                .height(DOT)
                .corner_radius(DOT / 2.0)
                .background(INTERVAL_TICK_COLOR)
                .horizontal_alignment(HorizontalAlignment::Right)
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(index as i32)
                .into()
        })
        .collect();
    Some(
        grid(dots)
            .columns(vec![GridLength::Star(1.0); segments])
            .rows([GridLength::Star(1.0)])
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch)
            .grid_column(0)
            .grid_row(0)
            .with_key(format!("interval-ticks-{tick_count}"))
            .into(),
    )
}

/// Thin pill progress track with a rounded fill and optional pace marker.
pub(super) fn rounded_progress(
    value: f64,
    fill: ThemeRef,
    pace: Option<PaceTip>,
    color_scheme: ColorScheme,
    interval_ticks: u32,
) -> Element {
    const HEIGHT: f64 = 6.0;
    let radius = HEIGHT / 2.0;
    let filled = value.clamp(0.0, 100.0);
    let (fill_star, rest_star) = if filled <= 0.0 {
        (0.0001, 100.0)
    } else if filled >= 100.0 {
        (100.0, 0.0001)
    } else {
        (filled, 100.0 - filled)
    };

    let fill_layer = grid((border(Element::Empty)
        .background(fill.clone())
        .corner_radius(radius)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Stretch)
        .grid_column(0),))
    .columns([GridLength::Star(fill_star), GridLength::Star(rest_star)])
    .rows([GridLength::Star(1.0)])
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Stretch)
    .grid_column(0)
    .grid_row(0);

    let track_layer: Element = border(Element::Empty)
        .background(fill)
        .opacity(0.2)
        .corner_radius(radius)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Stretch)
        .grid_column(0)
        .grid_row(0)
        .into();
    let mut layers: Vec<Element> = vec![track_layer, fill_layer.into()];
    if let Some(ticks) = interval_ticks_layer(interval_ticks) {
        layers.push(ticks);
    }
    if let Some(pace) = pace {
        layers.push(pace_marker_layer(pace, color_scheme));
    }

    border(
        grid(layers)
            .columns([GridLength::Star(1.0)])
            .rows([GridLength::Star(1.0)])
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch),
    )
    .corner_radius(radius)
    .height(HEIGHT)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

/// High-contrast vertical tick showing the expected even-burn position.
pub(super) fn pace_marker_layer(pace: PaceTip, color_scheme: ColorScheme) -> Element {
    // Keep the indicator legible against the theme-specific accent track.
    const LINE_WIDTH: f64 = 2.0;
    let marker_color = match color_scheme {
        ColorScheme::Light => Color {
            a: 255,
            r: 0,
            g: 0,
            b: 0,
        },
        ColorScheme::Dark => Color {
            a: 255,
            r: 255,
            g: 255,
            b: 255,
        },
    };
    let percent = pace.percent.clamp(0.0, 100.0);
    let (left_star, right_star) = if percent <= 0.0 {
        (0.0001, 100.0)
    } else if percent >= 100.0 {
        (100.0, 0.0001)
    } else {
        (percent, 100.0 - percent)
    };

    grid((border(Element::Empty)
        .width(LINE_WIDTH)
        .background(marker_color)
        .horizontal_alignment(HorizontalAlignment::Left)
        .vertical_alignment(VerticalAlignment::Stretch)
        .grid_column(1),))
    .columns([
        GridLength::Star(left_star),
        GridLength::Auto,
        GridLength::Star(right_star),
    ])
    .rows([GridLength::Star(1.0)])
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Stretch)
    .grid_column(0)
    .grid_row(0)
    .into()
}

pub(super) fn limit_card(
    title: &str,
    window: &LimitWindow,
    show_used_percentage: bool,
    show_usage_pace: bool,
    disabled: bool,
    color_scheme: ColorScheme,
) -> Element {
    let accent = ThemeRef::Accent;
    let (remaining_label, progress, show_reset, pace) = if disabled {
        ("Disabled".into(), 100.0, false, None)
    } else {
        let remaining = window.remaining_percent();
        let percentage = if show_used_percentage {
            window.used_percent
        } else {
            remaining
        };
        let suffix = if show_used_percentage { "used" } else { "left" };
        let label = percentage
            .map(|value| format!("{value}% {suffix}"))
            .unwrap_or_else(|| "Unavailable".into());
        let pace = show_usage_pace
            .then(|| window.pace_tip(show_used_percentage, Utc::now()))
            .flatten();
        (label, f64::from(percentage.unwrap_or(0)), true, pace)
    };
    let reset = window.resets_at.map(|at| format_reset_in(Some(at)));

    let header: Element = if let Some(pace) = pace {
        grid((
            caption(title.to_uppercase())
                .foreground(ThemeRef::SecondaryText)
                .vertical_alignment(VerticalAlignment::Center),
            caption(pace.summary())
                .foreground(ThemeRef::SecondaryText)
                .horizontal_alignment(HorizontalAlignment::Right)
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Center)
        .into()
    } else {
        grid((caption(title.to_uppercase()).foreground(ThemeRef::SecondaryText),))
            .columns([GridLength::Star(1.0)])
            .rows([GridLength::Auto])
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Center)
            .into()
    };

    let reset_status: Element = match reset {
        Some(reset) => hstack((
            text_block("Resets in")
                .foreground(ThemeRef::TertiaryText)
                .vertical_alignment(VerticalAlignment::Center),
            text_block(reset).vertical_alignment(VerticalAlignment::Center),
        ))
        .spacing(6.0)
        .horizontal_alignment(HorizontalAlignment::Right)
        .vertical_alignment(VerticalAlignment::Center)
        .into(),
        None => text_block("Session not started")
            .foreground(Color::rgb(255, 255, 255))
            .horizontal_alignment(HorizontalAlignment::Right)
            .vertical_alignment(VerticalAlignment::Center)
            .into(),
    };

    let footer: Element = if show_reset {
        grid((
            hstack((text_block(remaining_label)
                .font_weight(600)
                .foreground(accent.clone())
                .vertical_alignment(VerticalAlignment::Center),))
            .vertical_alignment(VerticalAlignment::Center),
            reset_status.grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Center)
        .into()
    } else {
        hstack((text_block(remaining_label)
            .font_weight(600)
            .foreground(accent.clone())
            .vertical_alignment(VerticalAlignment::Center),))
        .vertical_alignment(VerticalAlignment::Center)
        .into()
    };

    border(
        vstack((
            header,
            rounded_progress(
                progress,
                accent,
                pace,
                color_scheme,
                interval_tick_count(window),
            ),
            footer,
        ))
        .spacing(8.0),
    )
    .corner_radius(f64::from(popup::WINDOW_CORNER_RADIUS_DIP))
    .padding(Thickness::uniform(12.0))
    .background(ThemeRef::CardBackground)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .into()
}

pub(super) fn credits_card(limits: &RateLimits) -> Element {
    let value = credits_display_value(limits)
        .expect("credits card is only rendered for a displayable credit balance");

    border(
        grid((
            vstack((
                text_block("CREDITS").foreground(ThemeRef::TertiaryText),
                caption("Available balance").foreground(ThemeRef::TertiaryText),
            ))
            .spacing(2.0)
            .vertical_alignment(VerticalAlignment::Center),
            text_block(value)
                .font_weight(600)
                .foreground(ThemeRef::Accent)
                .vertical_alignment(VerticalAlignment::Center)
                .horizontal_alignment(HorizontalAlignment::Right)
                .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .corner_radius(f64::from(popup::WINDOW_CORNER_RADIUS_DIP))
    .padding(Thickness {
        left: 16.0,
        top: 12.0,
        right: 16.0,
        bottom: 12.0,
    })
    .background(ThemeRef::CardBackground)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .into()
}

pub(super) fn reset_credits_card(limits: &RateLimits) -> Element {
    let count = limits.available_reset_count();
    let count_label = if count == 1 {
        "1 Banked Reset".into()
    } else {
        format!("{count} Banked Resets")
    };
    let expiration = limits.next_reset_credit_expiration();
    let expiration_label = expiration
        .map(|expires_at| format!("Expires in {}", format_reset_in(Some(expires_at))))
        .unwrap_or_else(|| "No expiration date".into());
    let expiration_date = expiration
        .map(|expires_at| {
            let local = expires_at.with_timezone(&Local);
            format!(
                "{}, {}",
                local.format("%b %-d"),
                TimeFormat::current().format_hm(local)
            )
        })
        .unwrap_or_else(|| "Available to use".into());

    border(
        grid((
            text_block(count_label)
                .font_weight(600)
                .foreground(ThemeRef::Accent)
                .vertical_alignment(VerticalAlignment::Center),
            vstack((
                text_block(expiration_label),
                caption(expiration_date)
                    .foreground(ThemeRef::TertiaryText)
                    .horizontal_alignment(HorizontalAlignment::Right),
            ))
            .spacing(1.0)
            .horizontal_alignment(HorizontalAlignment::Right)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .corner_radius(f64::from(popup::WINDOW_CORNER_RADIUS_DIP))
    .padding(Thickness {
        left: 16.0,
        top: 12.0,
        right: 16.0,
        bottom: 12.0,
    })
    .background(ThemeRef::CardBackground)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .into()
}

pub(super) fn usage_statistics_card(provider: ProviderKind, limits: &RateLimits) -> Element {
    let statistics = &limits.usage;
    if provider == ProviderKind::Cursor && !statistics.has_data() {
        return border(
            vstack((
                body_strong("Usage activity"),
                caption(
                    "Waiting for Cursor's usage export. Refresh to retry.",
                )
                .foreground(ThemeRef::TertiaryText)
                .wrap(),
            ))
            .spacing(6.0),
        )
        .corner_radius(f64::from(popup::WINDOW_CORNER_RADIUS_DIP))
        .padding(Thickness::uniform(12.0))
        .background(ThemeRef::CardBackground)
        .border_thickness(Thickness::uniform(1.0))
        .border_brush(ThemeRef::CardStroke)
        .into();
    }
    if is_cost_provider(provider) {
        let metrics = grid((
            usage_value_metric(
                "Today",
                format_spend(statistics.today.estimated_cost_microusd),
                statistics.today.requests,
            ),
            usage_value_metric(
                &format!("Last {} days", statistics.history_days),
                format_spend(statistics.history.estimated_cost_microusd),
                statistics.history.requests,
            )
            .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch);
        let detail = format!(
            "{} requests · {} tokens",
            statistics.history.requests,
            format_token_count(statistics.history.total_tokens()),
        );
        return border(
            vstack((
                metrics,
                usage_activity_chart(statistics, true),
                caption(detail).foreground(ThemeRef::TertiaryText),
            ))
            .spacing(12.0),
        )
        .corner_radius(f64::from(popup::WINDOW_CORNER_RADIUS_DIP))
        .padding(Thickness::uniform(12.0))
        .background(ThemeRef::CardBackground)
        .border_thickness(Thickness::uniform(1.0))
        .border_brush(ThemeRef::CardStroke)
        .into();
    }
    let period = statistics.history_days;
    let total = format_token_count(statistics.history.total_tokens());
    let today = format_token_count(statistics.today.total_tokens());
    let today_value = statistics
        .today
        .estimated_api_value_usd()
        .map(format_usd)
        .unwrap_or_else(|| "No data".into());
    let history_value = statistics
        .history
        .estimated_api_value_usd()
        .map(format_usd)
        .unwrap_or_else(|| "No data".into());
    let detail = format!(
        "{} in · {} out · {} cached · {} requests",
        format_token_count(statistics.history.input_tokens),
        format_token_count(statistics.history.output_tokens),
        format_token_count(statistics.history.cached_input_tokens),
        statistics.history.requests,
    );
    let metrics = grid((
        usage_tokens_and_cost_metric("Today", today, today_value),
        usage_tokens_and_cost_metric(&format!("Last {period} days"), total, history_value)
            .grid_column(1),
    ))
    .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
    .rows([GridLength::Auto])
    .horizontal_alignment(HorizontalAlignment::Stretch);
    let chart = usage_activity_chart(statistics, false);

    border(
        vstack((
            metrics,
            chart,
            caption(detail).foreground(ThemeRef::TertiaryText),
        ))
        .spacing(12.0),
    )
    .corner_radius(f64::from(popup::WINDOW_CORNER_RADIUS_DIP))
    .padding(Thickness::uniform(12.0))
    .background(ThemeRef::CardBackground)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .into()
}
