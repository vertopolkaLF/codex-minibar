//! Usage overview page for the popup Usage tab.

use std::collections::BTreeMap;

use windows_reactor::*;

use crate::{
    popup,
    provider_registry,
    settings::ProviderKind,
    usage::TokenUsage,
    usage_overview::{
        BreakdownMode, BreakdownRow, DailySeriesPoint, OverviewMetric, OverviewRange,
        OverviewSnapshot, ProviderOverview,
    },
};

pub fn overview_page(
    snapshot: &OverviewSnapshot,
    metric: OverviewMetric,
    range: OverviewRange,
    breakdown: BreakdownMode,
    chart_hover: Option<usize>,
    color_scheme: ColorScheme,
    use_colored_provider_icons: bool,
    set_metric: SetState<OverviewMetric>,
    set_range: SetState<OverviewRange>,
    set_breakdown: SetState<BreakdownMode>,
    set_chart_hover: SetState<Option<usize>>,
) -> Element {
    if snapshot.providers.is_empty() {
        return border(
            vstack((
                body_strong("Usage"),
                caption("Enable a spend provider in Settings to see local API usage.")
                    .foreground(ThemeRef::TertiaryText)
                    .wrap(),
            ))
            .spacing(8.0),
        )
        .corner_radius(f64::from(popup::WINDOW_CORNER_RADIUS_DIP))
        .padding(Thickness::uniform(16.0))
        .background(ThemeRef::CardBackground)
        .border_thickness(Thickness::uniform(1.0))
        .border_brush(ThemeRef::CardStroke)
        .with_key("usage-empty")
        .into();
    }

    let date_label = format!(
        "Usage / {} to {}",
        snapshot.start_date.format("%b %-d"),
        snapshot.end_date.format("%b %-d")
    );

    border(
        vstack((
            usage_header(
                &date_label,
                metric,
                range,
                set_metric,
                set_range,
                &set_chart_hover,
            ),
            usage_hero(snapshot, metric, color_scheme, use_colored_provider_icons),
            usage_chart_card(
                &snapshot.daily_series,
                &snapshot.providers,
                metric,
                chart_hover,
                color_scheme,
                set_chart_hover,
            ),
            usage_totals_card(&snapshot.totals),
            usage_breakdown_card(
                snapshot,
                breakdown,
                metric,
                color_scheme,
                use_colored_provider_icons,
                set_breakdown,
            ),
        ))
        .spacing(10.0),
    )
    .corner_radius(f64::from(popup::WINDOW_CORNER_RADIUS_DIP))
    .padding(Thickness::uniform(12.0))
    .background(ThemeRef::CardBackground)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .with_key(format!(
        "usage-page-{}-{}-{}-{}",
        metric as i32,
        range as i32,
        breakdown as i32,
        color_scheme as i32
    ))
    .into()
}

fn usage_header(
    title: &str,
    metric: OverviewMetric,
    range: OverviewRange,
    set_metric: SetState<OverviewMetric>,
    set_range: SetState<OverviewRange>,
    set_chart_hover: &SetState<Option<usize>>,
) -> Element {
    let clear_hover = set_chart_hover.clone();
    vstack((
        caption(title).foreground(ThemeRef::SecondaryText),
        hstack((
            filter_chip(
                "Cost",
                metric == OverviewMetric::Cost,
                {
                    let set_metric = set_metric.clone();
                    let clear_hover = clear_hover.clone();
                    move || {
                        clear_hover.call(None);
                        set_metric.call(OverviewMetric::Cost);
                    }
                },
            ),
            filter_chip(
                "Tokens",
                metric == OverviewMetric::Tokens,
                {
                    let set_metric = set_metric.clone();
                    let clear_hover = clear_hover.clone();
                    move || {
                        clear_hover.call(None);
                        set_metric.call(OverviewMetric::Tokens);
                    }
                },
            ),
        ))
        .spacing(4.0),
        wrap_chips(
            [
                OverviewRange::Past24h,
                OverviewRange::SevenDays,
                OverviewRange::ThirtyDays,
                OverviewRange::NinetyDays,
            ]
            .into_iter()
            .map(|item| {
                filter_chip(item.label(), range == item, {
                    let set_range = set_range.clone();
                    let clear_hover = clear_hover.clone();
                    move || {
                        clear_hover.call(None);
                        set_range.call(item);
                    }
                })
            })
            .collect(),
        ),
    ))
    .spacing(8.0)
    .into()
}

fn wrap_chips(chips: Vec<Element>) -> Element {
    hstack(chips).spacing(4.0).into()
}

fn filter_chip(label: &str, selected: bool, on_click: impl IntoUnitCallback) -> Element {
    border(
        caption(label).foreground(if selected {
            ThemeRef::Accent
        } else {
            ThemeRef::SecondaryText
        }),
    )
    .padding(Thickness {
        left: 8.0,
        top: 4.0,
        right: 8.0,
        bottom: 4.0,
    })
    .corner_radius(12.0)
    .background(if selected {
        ThemeRef::SubtleFill
    } else {
        Color::transparent()
    })
    .border_thickness(Thickness::uniform(if selected { 1.0 } else { 0.0 }))
    .border_brush(ThemeRef::Accent)
    .on_tapped(on_click)
    .into()
}

fn usage_hero(
    snapshot: &OverviewSnapshot,
    metric: OverviewMetric,
    color_scheme: ColorScheme,
    use_colored_provider_icons: bool,
) -> Element {
    let headline = match metric {
        OverviewMetric::Cost => format_spend(snapshot.totals.estimated_cost_microusd),
        OverviewMetric::Tokens => format_token_count(snapshot.totals.total_tokens()),
    };
    let subtitle = format!(
        "{} sessions · API estimate",
        snapshot.total_sessions
    );

    vstack((
        text_block(headline).font_size(28.0).font_weight(600),
        caption(subtitle).foreground(ThemeRef::TertiaryText),
        vstack(
            snapshot
                .providers
                .iter()
                .map(|entry| provider_row(entry, metric, color_scheme, use_colored_provider_icons))
                .collect::<Vec<_>>(),
        )
        .spacing(8.0),
    ))
    .spacing(8.0)
    .into()
}

fn provider_row(
    entry: &ProviderOverview,
    metric: OverviewMetric,
    color_scheme: ColorScheme,
    use_colored_provider_icons: bool,
) -> Element {
    let descriptor = provider_registry::descriptor(entry.provider);
    let icon_name = descriptor.icon;
    let color = provider_brand_color(entry.provider, color_scheme, use_colored_provider_icons);
    let value = match metric {
        OverviewMetric::Cost => format_spend(entry.usage.estimated_cost_microusd),
        OverviewMetric::Tokens => format_token_count(entry.usage.total_tokens()),
    };
    let share = match metric {
        OverviewMetric::Cost => entry.share_cost,
        OverviewMetric::Tokens => entry.share_tokens,
    };
    let detail = format!(
        "{:.1}% of {} · {}",
        share,
        match metric {
            OverviewMetric::Cost => "cost",
            OverviewMetric::Tokens => "tokens",
        },
        format_token_count(entry.usage.total_tokens())
    );

    grid((
        crate::icons::element(icon_name, 16.0, color)
            .vertical_alignment(VerticalAlignment::Center),
        vstack((
            body_strong(descriptor.display_name),
            caption(format!("{} sessions · {}", entry.sessions, value))
                .foreground(ThemeRef::SecondaryText),
            caption(detail).foreground(ThemeRef::TertiaryText),
        ))
        .spacing(1.0)
        .margin(Thickness {
            left: 8.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        })
        .grid_column(1),
    ))
    .columns([GridLength::Auto, GridLength::Star(1.0)])
    .rows([GridLength::Auto])
    .into()
}

fn usage_chart_card(
    series: &[DailySeriesPoint],
    providers: &[ProviderOverview],
    metric: OverviewMetric,
    hover: Option<usize>,
    color_scheme: ColorScheme,
    set_hover: SetState<Option<usize>>,
) -> Element {
    let title = match metric {
        OverviewMetric::Cost => "Daily cost",
        OverviewMetric::Tokens => "Daily processed tokens",
    };
    let chart = usage_line_chart(series, providers, metric, hover, color_scheme, set_hover);
    let tooltip = hover
        .and_then(|index| series.get(index))
        .map(|point| chart_tooltip(point, providers, metric));

    vstack((
        body_strong(title),
        relative_panel({
            let mut layers = vec![chart];
            if let Some(tooltip) = tooltip {
                layers.push(
                    tooltip
                        .margin(Thickness {
                            left: 8.0,
                            top: 8.0,
                            right: 8.0,
                            bottom: 0.0,
                        })
                        .relative_align_left()
                        .relative_align_top()
                        .into(),
                );
            }
            layers
        }),
    ))
    .spacing(8.0)
    .into()
}

fn usage_line_chart(
    series: &[DailySeriesPoint],
    providers: &[ProviderOverview],
    metric: OverviewMetric,
    hover: Option<usize>,
    color_scheme: ColorScheme,
    set_hover: SetState<Option<usize>>,
) -> Element {
    const CHART_HEIGHT: f64 = 132.0;
    let chart_width = f64::from(popup::POPUP_WIDTH) - 2.0 - 24.0 - 2.0 - 24.0;
    if series.is_empty() {
        return border(
            caption("No activity in this range")
                .foreground(ThemeRef::TertiaryText)
                .horizontal_alignment(HorizontalAlignment::Center),
        )
        .height(CHART_HEIGHT)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into();
    }

    let max_value = series
        .iter()
        .map(|point| point.total)
        .max()
        .unwrap_or(1)
        .max(1);
    let step = chart_width / series.len().max(1) as f64;
    let bar_width = (step / providers.len().max(1) as f64 - 1.0).clamp(1.5, 6.0);

    let mut layers: Vec<Element> = Vec::new();
    for (provider_index, entry) in providers.iter().enumerate() {
        let bars: Vec<Element> = series
            .iter()
            .enumerate()
            .map(|(index, point)| {
                let value = point
                    .by_provider
                    .get(&entry.provider)
                    .copied()
                    .unwrap_or(0);
                let height = if max_value == 0 {
                    2.0
                } else {
                    (CHART_HEIGHT * value as f64 / max_value as f64).max(2.0)
                };
                border(Element::Empty)
                    .width(bar_width)
                    .height(height)
                    .corner_radius(1.0)
                    .background(provider_brand_color(
                        entry.provider,
                        color_scheme,
                        true,
                    ))
                    .opacity(if value == 0 { 0.15 } else { 0.95 })
                    .margin(Thickness {
                        left: step * index as f64
                            + provider_index as f64 * (bar_width + 1.0)
                            + (step - bar_width * providers.len() as f64) / 2.0,
                        top: CHART_HEIGHT - height,
                        right: 0.0,
                        bottom: 0.0,
                    })
                    .relative_align_left()
                    .relative_align_top()
                    .into()
            })
            .collect();
        layers.extend(bars);
    }

    for (index, _) in series.iter().enumerate() {
        let set_hover_enter = set_hover.clone();
        let set_hover_exit = set_hover.clone();
        layers.push(
            border(Element::Empty)
                .width(step.max(4.0))
                .height(CHART_HEIGHT)
                .background(Color::transparent())
                .margin(Thickness {
                    left: step * index as f64,
                    top: 0.0,
                    right: 0.0,
                    bottom: 0.0,
                })
                .relative_align_left()
                .relative_align_top()
                .on_pointer_entered(move |_| set_hover_enter.call(Some(index)))
                .on_pointer_exited(move || set_hover_exit.call(None))
                .into(),
        );
        if hover == Some(index) {
            layers.push(
                border(Element::Empty)
                    .width(1.0)
                    .height(CHART_HEIGHT)
                    .background(ThemeRef::CardStroke)
                    .margin(Thickness {
                        left: step * index as f64 + step / 2.0,
                        top: 0.0,
                        right: 0.0,
                        bottom: 0.0,
                    })
                    .relative_align_left()
                    .into(),
            );
        }
    }

    border(relative_panel(layers))
        .height(CHART_HEIGHT)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into()
}

fn chart_tooltip(
    point: &DailySeriesPoint,
    providers: &[ProviderOverview],
    metric: OverviewMetric,
) -> Element {
    let rows: Vec<Element> = providers
        .iter()
        .filter_map(|entry| {
            let value = point.by_provider.get(&entry.provider).copied()?;
            Some(
                grid((
                    crate::icons::element(
                        provider_registry::descriptor(entry.provider).icon,
                        14.0,
                        provider_brand_color(entry.provider, ColorScheme::Dark, true),
                    ),
                    caption(provider_registry::descriptor(entry.provider).display_name)
                        .grid_column(1),
                    caption(match metric {
                        OverviewMetric::Cost => format_spend(value),
                        OverviewMetric::Tokens => format_token_count(value),
                    })
                    .horizontal_alignment(HorizontalAlignment::Right)
                    .grid_column(2),
                ))
                .columns([
                    GridLength::Auto,
                    GridLength::Star(1.0),
                    GridLength::Auto,
                ])
                .into(),
            )
        })
        .collect();

    border(
        vstack({
            let mut items = vec![
                body_strong(point.date.format("%b %-d").to_string()).into(),
            ];
            items.extend(rows);
            items.push(
                caption(format!(
                    "Total {}",
                    match metric {
                        OverviewMetric::Cost => format_spend(point.total),
                        OverviewMetric::Tokens => format_token_count(point.total),
                    }
                ))
                .foreground(ThemeRef::SecondaryText)
                .into(),
            );
            items
        })
        .spacing(4.0),
    )
    .padding(Thickness::uniform(8.0))
    .corner_radius(6.0)
    .background(ThemeRef::CardBackground)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .into()
}

fn usage_totals_card(totals: &TokenUsage) -> Element {
    let processed = totals.total_tokens();
    let uncached = totals
        .input_tokens
        .saturating_sub(totals.cached_input_tokens);
    vstack((
        body_strong("Totals"),
        grid((
            total_metric("Processed tokens", format_token_count(processed)),
            total_metric(
                "Cached input",
                format_token_count(totals.cached_input_tokens),
            )
            .grid_column(1),
            total_metric("Uncached input", format_token_count(uncached)),
            total_metric("Output", format_token_count(totals.output_tokens)).grid_column(1),
            total_metric(
                "Cache savings",
                format_spend(totals.cache_savings_microusd),
            )
            .grid_column_span(2),
        ))
        .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
        .row_spacing(8.0)
        .column_spacing(8.0),
    ))
    .spacing(8.0)
    .into()
}

fn total_metric(label: &str, value: String) -> Element {
    vstack((
        caption(label).foreground(ThemeRef::TertiaryText),
        body_strong(value),
    ))
    .spacing(2.0)
    .into()
}

fn usage_breakdown_card(
    snapshot: &OverviewSnapshot,
    breakdown: BreakdownMode,
    metric: OverviewMetric,
    color_scheme: ColorScheme,
    use_colored_provider_icons: bool,
    set_breakdown: SetState<BreakdownMode>,
) -> Element {
    let rows = match breakdown {
        BreakdownMode::Model => &snapshot.model_rows,
        BreakdownMode::Day => &snapshot.day_rows,
    };
    vstack((
        grid((
            body_strong("Breakdown"),
            hstack((
                filter_chip("Model", breakdown == BreakdownMode::Model, {
                    let set_breakdown = set_breakdown.clone();
                    move || set_breakdown.call(BreakdownMode::Model)
                }),
                filter_chip("Day", breakdown == BreakdownMode::Day, {
                    let set_breakdown = set_breakdown.clone();
                    move || set_breakdown.call(BreakdownMode::Day)
                }),
            ))
            .horizontal_alignment(HorizontalAlignment::Right)
            .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto]),
        breakdown_header(),
        vstack(
            rows.iter()
                .take(12)
                .map(|row| breakdown_row(row, metric, color_scheme, use_colored_provider_icons))
                .collect::<Vec<_>>(),
        )
        .spacing(6.0),
    ))
    .spacing(8.0)
    .into()
}

fn breakdown_header() -> Element {
    grid((
        caption("Model").foreground(ThemeRef::TertiaryText),
        caption("Cost")
            .foreground(ThemeRef::TertiaryText)
            .horizontal_alignment(HorizontalAlignment::Right)
            .grid_column(1),
        caption("Share")
            .foreground(ThemeRef::TertiaryText)
            .horizontal_alignment(HorizontalAlignment::Right)
            .grid_column(2),
        caption("Tokens")
            .foreground(ThemeRef::TertiaryText)
            .horizontal_alignment(HorizontalAlignment::Right)
            .grid_column(3),
    ))
    .columns([
        GridLength::Star(1.0),
        GridLength::Pixel(56.0),
        GridLength::Pixel(44.0),
        GridLength::Pixel(56.0),
    ])
    .into()
}

fn breakdown_row(
    row: &BreakdownRow,
    metric: OverviewMetric,
    color_scheme: ColorScheme,
    use_colored_provider_icons: bool,
) -> Element {
    let leading: Element = if let Some(provider) = row.provider {
        crate::icons::element(
            provider_registry::descriptor(provider).icon,
            14.0,
            provider_brand_color(provider, color_scheme, use_colored_provider_icons),
        )
        .into()
    } else {
        Element::Empty
    };
    grid((
        hstack((leading, caption(&row.label).margin(Thickness {
            left: 6.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        })))
            .vertical_alignment(VerticalAlignment::Center),
        caption(format_spend(row.cost_microusd))
            .horizontal_alignment(HorizontalAlignment::Right)
            .grid_column(1),
        caption(format!("{:.1}%", row.share))
            .horizontal_alignment(HorizontalAlignment::Right)
            .grid_column(2),
        caption(format_token_count(row.tokens))
            .horizontal_alignment(HorizontalAlignment::Right)
            .grid_column(3),
    ))
    .columns([
        GridLength::Star(1.0),
        GridLength::Pixel(56.0),
        GridLength::Pixel(44.0),
        GridLength::Pixel(56.0),
    ])
    .into()
}

fn provider_brand_color(
    provider: ProviderKind,
    color_scheme: ColorScheme,
    use_colored: bool,
) -> Color {
    if !use_colored {
        return match color_scheme {
            ColorScheme::Dark => Color::rgb(190, 190, 190),
            ColorScheme::Light => Color::rgb(96, 96, 96),
        };
    }
    let (r, g, b) = provider_registry::descriptor(provider).brand_rgb;
    Color::rgb(r, g, b)
}

fn format_spend(microusd: u64) -> String {
    let value = microusd as f64 / 1_000_000.0;
    if value >= 1_000_000.0 {
        format!("${:.1}M", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("${:.1}K", value / 1_000.0)
    } else {
        format!("${value:.2}")
    }
}

fn format_token_count(tokens: u64) -> String {
    match tokens {
        0..=999 => tokens.to_string(),
        1_000..=999_999 => format!("{:.1}K", tokens as f64 / 1_000.0),
        1_000_000..=999_999_999 => format!("{:.1}M", tokens as f64 / 1_000_000.0),
        _ => format!("{:.1}B", tokens as f64 / 1_000_000_000.0),
    }
}
