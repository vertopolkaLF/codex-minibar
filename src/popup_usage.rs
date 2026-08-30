//! Usage overview page for the popup Usage tab.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Local, NaiveDate};
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

const USAGE_CARD_PAD: f64 = 12.0;

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
        return vstack((
            body_strong("Usage"),
            caption("Enable a spend provider in Settings to see local API usage.")
                .foreground(ThemeRef::TertiaryText)
                .wrap(),
        ))
        .spacing(8.0)
        .with_key("usage-empty")
        .into();
    }

    let range_label = if snapshot.hourly {
        let start = snapshot
            .daily_series
            .first()
            .map(|point| crate::usage_overview::format_hour_label(point.at))
            .unwrap_or_else(|| snapshot.start_date.format("%b %-d").to_string());
        let end = snapshot
            .daily_series
            .last()
            .map(|point| crate::usage_overview::format_hour_label(point.at))
            .unwrap_or_else(|| snapshot.end_date.format("%b %-d").to_string());
        format!("{start} to {end}")
    } else {
        format!(
            "{} to {}",
            snapshot.start_date.format("%b %-d"),
            snapshot.end_date.format("%b %-d")
        )
    };

    vstack((
        usage_header(
            &range_label,
            metric,
            range,
            set_metric,
            set_range,
            &set_chart_hover,
        ),
        usage_hero(snapshot, metric, color_scheme, use_colored_provider_icons),
        usage_chart_card(
            &snapshot.daily_series,
            snapshot.start_date,
            snapshot.end_date,
            snapshot.hourly,
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
    .spacing(10.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
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
    range_label: &str,
    metric: OverviewMetric,
    range: OverviewRange,
    set_metric: SetState<OverviewMetric>,
    set_range: SetState<OverviewRange>,
    set_chart_hover: &SetState<Option<usize>>,
) -> Element {
    let clear_hover = set_chart_hover.clone();
    vstack((
        grid((
            hstack((
                body_strong("Usage").vertical_alignment(VerticalAlignment::Bottom),
                body(range_label)
                    .foreground(ThemeRef::TertiaryText)
                    .vertical_alignment(VerticalAlignment::Bottom),
            ))
            .spacing(8.0)
            .vertical_alignment(VerticalAlignment::Center),
            segmented_control(
                "usage-metric",
                vec![
                    segmented_tab("Cost", metric == OverviewMetric::Cost, {
                        let set_metric = set_metric.clone();
                        let clear_hover = clear_hover.clone();
                        move || {
                            clear_hover.call(None);
                            set_metric.call(OverviewMetric::Cost);
                        }
                    }),
                    segmented_tab("Tokens", metric == OverviewMetric::Tokens, {
                        let set_metric = set_metric.clone();
                        let clear_hover = clear_hover.clone();
                        move || {
                            clear_hover.call(None);
                            set_metric.call(OverviewMetric::Tokens);
                        }
                    }),
                ],
                false,
            )
            .horizontal_alignment(HorizontalAlignment::Right)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto]),
        period_switcher(range, set_range, clear_hover),
    ))
    .spacing(8.0)
    .into()
}

fn period_switcher(
    range: OverviewRange,
    set_range: SetState<OverviewRange>,
    clear_hover: SetState<Option<usize>>,
) -> Element {
    let periods = [
        OverviewRange::Past24h,
        OverviewRange::SevenDays,
        OverviewRange::ThirtyDays,
        OverviewRange::NinetyDays,
    ];
    segmented_control(
        "usage-range",
        periods
            .into_iter()
            .map(|item| {
                segmented_tab(item.label(), range == item, {
                    let set_range = set_range.clone();
                    let clear_hover = clear_hover.clone();
                    move || {
                        clear_hover.call(None);
                        set_range.call(item);
                    }
                })
            })
            .collect(),
        true,
    )
}

struct SegmentedTab {
    label: String,
    selected: bool,
    on_click: Callback<()>,
}

fn segmented_tab(
    label: impl Into<String>,
    selected: bool,
    on_click: impl IntoUnitCallback,
) -> SegmentedTab {
    SegmentedTab {
        label: label.into(),
        selected,
        on_click: on_click.into_unit_callback(),
    }
}

fn segmented_tab_width(label: &str) -> f64 {
    (label.chars().count() as f64 * 8.0 + 22.0).max(48.0)
}

fn segmented_control(key: &str, tabs: Vec<SegmentedTab>, stretch: bool) -> Element {
    let count = tabs.len().max(1);
    let selected = tabs.iter().position(|tab| tab.selected).unwrap_or(0);
    let anim = crate::theme::duration(crate::theme::CONTROL_FAST_ANIMATION);
    let cell_width = if stretch {
        (f64::from(popup::POPUP_WIDTH) - 2.0 - 32.0 - 6.0) / count as f64
    } else {
        tabs.iter()
            .map(|tab| segmented_tab_width(&tab.label))
            .fold(0.0, f64::max)
    };
    let columns = vec![
        if stretch {
            GridLength::Star(1.0)
        } else {
            GridLength::Pixel(cell_width)
        };
        count
    ];
    let thumb = border(Element::Empty)
        .width(cell_width)
        .corner_radius(6.0)
        .background(ThemeRef::Accent)
        .margin(Thickness {
            left: selected as f64 * cell_width,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        })
        .relative_align_left()
        .relative_align_top()
        .relative_align_bottom()
        .with_translation_transition(anim)
        .with_key(format!("{key}-thumb"));
    let cells = tabs
        .into_iter()
        .enumerate()
        .map(|(index, tab)| {
            let hide_divider = index == 0
                || selected == index
                || selected == index.saturating_sub(1);
            let idle = caption(tab.label.clone())
                .font_weight(600)
                .foreground(ThemeRef::PrimaryText)
                .opacity(if index == selected { 0.0 } else { 1.0 })
                .with_opacity_transition(anim)
                .relative_align_h_center()
                .relative_align_v_center()
                .with_key(format!("{key}-idle-{}", tab.label));
            let active = caption(tab.label.clone())
                .font_weight(600)
                .foreground(ThemeRef::custom("TextOnAccentFillColorPrimaryBrush"))
                .opacity(if index == selected { 1.0 } else { 0.0 })
                .with_opacity_transition(anim)
                .relative_align_h_center()
                .relative_align_v_center()
                .with_key(format!("{key}-on-{}", tab.label));
            let label_layers: Vec<Element> = vec![idle.into(), active.into()];
            let label = border(relative_panel(label_layers))
                .padding(Thickness {
                    left: 10.0,
                    top: 5.0,
                    right: 10.0,
                    bottom: 5.0,
                })
                .background(Color::transparent())
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .on_tapped(tab.on_click);
            let cell: Element = if index == 0 {
                label.into()
            } else {
                grid((
                    border(Element::Empty)
                        .width(1.0)
                        .vertical_alignment(VerticalAlignment::Stretch)
                        .background(ThemeRef::DividerStroke)
                        .opacity(if hide_divider { 0.0 } else { 1.0 })
                        .with_opacity_transition(anim)
                        .margin(Thickness {
                            left: 0.0,
                            top: 6.0,
                            right: 0.0,
                            bottom: 6.0,
                        })
                        .with_key(format!("{key}-rule-{index}")),
                    label.grid_column(1),
                ))
                .columns([GridLength::Pixel(1.0), GridLength::Star(1.0)])
                .into()
            };
            cell.horizontal_alignment(HorizontalAlignment::Stretch)
                .grid_column(index as i32)
                .with_key(format!("{key}-tab-{}", tab.label))
        })
        .collect::<Vec<_>>();
    let labels = grid(cells)
        .columns(columns)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .relative_align_left()
        .relative_align_right()
        .relative_align_top()
        .relative_align_bottom()
        .with_key(format!("{key}-labels"));
    let layers: Vec<Element> = vec![thumb.into(), labels.into()];
    let track = relative_panel(layers).horizontal_alignment(HorizontalAlignment::Stretch);
    let track = if stretch {
        track
    } else {
        track.width(cell_width * count as f64)
    };

    border(track)
        .padding(Thickness::uniform(3.0))
        .corner_radius(8.0)
        .background(ThemeRef::ControlFill)
        .horizontal_alignment(if stretch {
            HorizontalAlignment::Stretch
        } else {
            HorizontalAlignment::Left
        })
        .with_key(format!("{key}-{count}"))
        .into()
}

fn usage_card(content: impl Into<Element>) -> Element {
    border(content.into())
        .corner_radius(f64::from(popup::WINDOW_CORNER_RADIUS_DIP))
        .padding(Thickness::uniform(USAGE_CARD_PAD))
        .background(ThemeRef::CardBackground)
        .border_thickness(Thickness::uniform(1.0))
        .border_brush(ThemeRef::CardStroke)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into()
}

fn usage_hero(
    snapshot: &OverviewSnapshot,
    metric: OverviewMetric,
    color_scheme: ColorScheme,
    use_colored_provider_icons: bool,
) -> Element {
    let headline = match metric {
        OverviewMetric::Cost => format_spend_full(snapshot.totals.estimated_cost_microusd),
        OverviewMetric::Tokens => format_token_count(snapshot.totals.total_tokens()),
    };
    let subtitle = format!(
        "{} sessions · API estimate",
        snapshot.total_sessions
    );

    usage_card(
        vstack((
            vstack((
                text_block(headline).font_size(28.0).font_weight(600),
                caption(subtitle).foreground(ThemeRef::TertiaryText),
            ))
            .spacing(0.0),
            provider_grid(
                snapshot,
                metric,
                color_scheme,
                use_colored_provider_icons,
            ),
        ))
        .spacing(8.0),
    )
}

fn provider_grid(
    snapshot: &OverviewSnapshot,
    metric: OverviewMetric,
    color_scheme: ColorScheme,
    use_colored_provider_icons: bool,
) -> Element {
    const COLUMNS: usize = 2;
    let row_count = snapshot.providers.len().div_ceil(COLUMNS).max(1);
    let set_key = snapshot
        .providers
        .iter()
        .map(|entry| entry.provider.id())
        .collect::<Vec<_>>()
        .join("+");
    let cells = snapshot
        .providers
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            provider_row(entry, metric, color_scheme, use_colored_provider_icons)
                .grid_row((index / COLUMNS) as i32)
                .grid_column((index % COLUMNS) as i32)
        })
        .collect::<Vec<_>>();

    grid(cells)
        .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
        .rows(vec![GridLength::Auto; row_count])
        .row_spacing(8.0)
        .column_spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .with_key(format!(
            "usage-hero-providers-{}-{}",
            set_key,
            color_scheme as i32
        ))
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
            .vertical_alignment(VerticalAlignment::Center)
            .with_key(format!(
                "usage-hero-icon-{}-{}-{:02X}{:02X}{:02X}",
                entry.provider.id(),
                icon_name,
                color.r,
                color.g,
                color.b
            )),
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
    .with_key(format!("usage-hero-provider-{}", entry.provider.id()))
    .into()
}

fn usage_chart_card(
    series: &[DailySeriesPoint],
    start_date: NaiveDate,
    end_date: NaiveDate,
    hourly: bool,
    providers: &[ProviderOverview],
    metric: OverviewMetric,
    hover: Option<usize>,
    color_scheme: ColorScheme,
    set_hover: SetState<Option<usize>>,
) -> Element {
    let title = match (hourly, metric) {
        (true, OverviewMetric::Cost) => "Hourly cost",
        (true, OverviewMetric::Tokens) => "Hourly processed tokens",
        (false, OverviewMetric::Cost) => "Cost",
        (false, OverviewMetric::Tokens) => "Tokens",
    };
    let filled = if hourly {
        fill_hourly_series(series)
    } else {
        fill_daily_series(series, start_date, end_date)
    };
    let chart = usage_area_chart(
        &filled,
        providers,
        metric,
        hourly,
        hover,
        color_scheme,
        set_hover,
    );
    let tooltip = hover
        .and_then(|index| filled.get(index).map(|point| (index, point)))
        .map(|(index, point)| {
            chart_tooltip(point, providers, metric, hourly)
                .margin(Thickness {
                    left: 48.0,
                    top: 8.0,
                    right: 8.0,
                    bottom: 0.0,
                })
                .relative_align_left()
                .relative_align_top()
                .with_key(format!(
                    "usage-tip-{}-{}-{}-{}",
                    metric as i32,
                    hourly,
                    index,
                    point.at.to_rfc3339()
                ))
        });

    usage_card(
        vstack((
            body_strong(title),
            relative_panel({
                let mut layers = vec![chart.with_key("usage-chart-plot")];
                if let Some(tooltip) = tooltip {
                    layers.push(tooltip);
                }
                layers
            }),
        ))
        .spacing(6.0),
    )
}

const CHART_PLOT_HEIGHT: f64 = 132.0;
const CHART_Y_AXIS_WIDTH: f64 = 40.0;
const CHART_Y_GAP: f64 = 6.0;
const CHART_PAD_X: f64 = 4.0;
const CHART_PAD_TOP: f64 = 6.0;
const CHART_PAD_BOTTOM: f64 = 3.0;

fn usage_area_chart(
    series: &[DailySeriesPoint],
    providers: &[ProviderOverview],
    metric: OverviewMetric,
    hourly: bool,
    hover: Option<usize>,
    color_scheme: ColorScheme,
    set_hover: SetState<Option<usize>>,
) -> Element {
    // Popup stroke + body 16px + card stroke/pad, then the Y axis.
    let plot_width = f64::from(popup::POPUP_WIDTH)
        - 2.0
        - 32.0
        - 2.0
        - USAGE_CARD_PAD * 2.0
        - CHART_Y_AXIS_WIDTH
        - CHART_Y_GAP;
    if series.is_empty() {
        return border(
            caption("No activity in this range")
                .foreground(ThemeRef::TertiaryText)
                .horizontal_alignment(HorizontalAlignment::Center),
        )
        .height(CHART_PLOT_HEIGHT)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into();
    }

    let raw_max = series.iter().map(|point| point.total).max().unwrap_or(0);
    let max_value = nice_ceiling(raw_max);
    let xaml = usage_area_chart_xaml(series, providers, max_value, plot_width, color_scheme);
    let plot = usage_area_chart_host(
        &xaml,
        series,
        providers,
        metric,
        color_scheme,
        plot_width,
    );
    let hits = usage_chart_hit_targets(series.len(), plot_width, hover, set_hover);
    let y_axis = usage_y_axis(max_value, metric);
    let x_axis = usage_x_axis(series, hourly);

    grid((
        y_axis,
        relative_panel({
            let mut layers = vec![plot];
            layers.extend(hits);
            layers
        })
        .grid_column(1),
        x_axis.grid_column(1).grid_row(1),
    ))
    .columns([GridLength::Pixel(CHART_Y_AXIS_WIDTH), GridLength::Star(1.0)])
    .rows([GridLength::Pixel(CHART_PLOT_HEIGHT), GridLength::Auto])
    .column_spacing(CHART_Y_GAP)
    .row_spacing(4.0)
    .into()
}

fn usage_area_chart_host(
    xaml: &str,
    series: &[DailySeriesPoint],
    providers: &[ProviderOverview],
    metric: OverviewMetric,
    color_scheme: ColorScheme,
    plot_width: f64,
) -> Element {
    thread_local! {
        static CHART_MOUNTS: std::cell::RefCell<
            std::collections::HashMap<String, windows_core::IInspectable>,
        > = std::cell::RefCell::new(std::collections::HashMap::new());
        static CHART_SERIES: std::cell::RefCell<std::collections::HashMap<String, u64>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
    }

    let host_key = format!(
        "usage-area-{}-{}-{}-{}",
        metric as i32,
        color_scheme as i32,
        series.len(),
        providers
            .iter()
            .fold(0_u64, |hash, entry| hash
                .wrapping_mul(31)
                .wrapping_add(entry.provider as u64)),
    );
    let fingerprint = series.iter().fold(0_u64, |hash, point| {
        let provider_hash = point.by_provider.iter().fold(0_u64, |inner, (kind, value)| {
            inner
                .wrapping_mul(31)
                .wrapping_add(*kind as u64)
                .wrapping_add(*value)
        });
        hash.wrapping_mul(31)
            .wrapping_add(point.total)
            .wrapping_add(provider_hash)
    });

    let series_changed = CHART_SERIES.with(|cache| {
        let mut cache = cache.borrow_mut();
        match cache.get(&host_key) {
            Some(previous) if *previous == fingerprint => false,
            _ => {
                cache.insert(host_key.clone(), fingerprint);
                true
            }
        }
    });
    if series_changed {
        CHART_MOUNTS.with(|mounts| {
            if let Some(native) = mounts.borrow().get(&host_key).cloned()
                && let Err(error) = crate::acrylic::install_usage_chart_into(native, xaml)
            {
                eprintln!("Could not update usage area chart: {error:?}");
            }
        });
    }

    let xaml_for_mount = xaml.to_string();
    let key_for_mount = host_key.clone();
    let key_for_unmount = host_key.clone();
    let mut host = swap_chain_panel()
        .width(plot_width)
        .height(CHART_PLOT_HEIGHT);
    host.mounted = Some(Callback::new(
        move |native: Option<windows_core::IInspectable>| {
            if let Some(native) = native {
                if let Err(error) =
                    crate::acrylic::install_usage_chart_into(native.clone(), &xaml_for_mount)
                {
                    eprintln!("Could not install usage area chart: {error:?}");
                }
                CHART_MOUNTS.with(|mounts| {
                    mounts.borrow_mut().insert(key_for_mount.clone(), native);
                });
            }
        },
    ));
    host.unmounted = Some(Callback::new(
        move |native: Option<windows_core::IInspectable>| {
            if let Some(native) = native {
                let _ = crate::acrylic::clear_children(native);
            }
            CHART_MOUNTS.with(|mounts| {
                mounts.borrow_mut().remove(&key_for_unmount);
            });
            CHART_SERIES.with(|cache| {
                cache.borrow_mut().remove(&key_for_unmount);
            });
        },
    ));
    host.with_key(host_key).into()
}

fn usage_chart_hit_targets(
    count: usize,
    plot_width: f64,
    hover: Option<usize>,
    set_hover: SetState<Option<usize>>,
) -> Vec<Element> {
    let step = plot_width / count.max(1) as f64;
    let mut layers = Vec::with_capacity(count * 2);
    for index in 0..count {
        let set_hover_enter = set_hover.clone();
        let set_hover_exit = set_hover.clone();
        let selected = hover == Some(index);
        layers.push(
            border(Element::Empty)
                .width(step.max(4.0))
                .height(CHART_PLOT_HEIGHT)
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
                .with_key(format!("usage-hit-{index}"))
                .into(),
        );
        layers.push(
            border(Element::Empty)
                .width(1.0)
                .height(CHART_PLOT_HEIGHT)
                .background(ThemeRef::CardStroke)
                .opacity(if selected { 1.0 } else { 0.0 })
                .margin(Thickness {
                    left: step * index as f64 + step / 2.0,
                    top: 0.0,
                    right: 0.0,
                    bottom: 0.0,
                })
                .relative_align_left()
                .relative_align_top()
                .with_key(format!("usage-hairline-{index}"))
                .into(),
        );
    }
    layers
}

fn usage_y_axis(max_value: u64, metric: OverviewMetric) -> Element {
    let ticks = [max_value, max_value * 2 / 3, max_value / 3, 0];
    let plot_h = CHART_PLOT_HEIGHT - CHART_PAD_TOP - CHART_PAD_BOTTOM;
    const LABEL_HEIGHT: f64 = 16.0;
    relative_panel(
        ticks
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                let y = CHART_PAD_TOP + plot_h * index as f64 / 3.0;
                let top = (y - LABEL_HEIGHT / 2.0).clamp(0.0, CHART_PLOT_HEIGHT - LABEL_HEIGHT);
                caption(format_axis_value(value, metric))
                    .foreground(ThemeRef::TertiaryText)
                    .horizontal_alignment(HorizontalAlignment::Right)
                    .margin(Thickness {
                        left: 0.0,
                        top,
                        right: 0.0,
                        bottom: 0.0,
                    })
                    .relative_align_left()
                    .relative_align_top()
                    .into()
            })
            .collect::<Vec<Element>>(),
    )
    .width(CHART_Y_AXIS_WIDTH)
    .height(CHART_PLOT_HEIGHT)
    .into()
}

fn usage_x_axis(series: &[DailySeriesPoint], hourly: bool) -> Element {
    let label = |point: &DailySeriesPoint| {
        if hourly {
            crate::usage_overview::format_hour_label(point.at)
        } else {
            format_axis_date(point.date)
        }
    };
    let first = series.first().map(label);
    let last = series.last().map(label);
    let mid = series.get(series.len() / 2).map(label);
    grid((
        caption(first.unwrap_or_default()).foreground(ThemeRef::TertiaryText),
        caption(mid.unwrap_or_default())
            .foreground(ThemeRef::TertiaryText)
            .horizontal_alignment(HorizontalAlignment::Center)
            .grid_column(1),
        caption(last.unwrap_or_default())
            .foreground(ThemeRef::TertiaryText)
            .horizontal_alignment(HorizontalAlignment::Right)
            .grid_column(2),
    ))
    .columns([
        GridLength::Star(1.0),
        GridLength::Star(1.0),
        GridLength::Star(1.0),
    ])
    .into()
}

fn usage_area_chart_xaml(
    series: &[DailySeriesPoint],
    providers: &[ProviderOverview],
    max_value: u64,
    width: f64,
    color_scheme: ColorScheme,
) -> String {
    let height = CHART_PLOT_HEIGHT;
    let plot_h = (height - CHART_PAD_TOP - CHART_PAD_BOTTOM).max(1.0);
    let baseline = height - CHART_PAD_BOTTOM;
    let grid = match color_scheme {
        ColorScheme::Dark => "#33A89BB8",
        ColorScheme::Light => "#24111111",
    };

    let mut body = String::new();
    for tick in 0..4 {
        let y = CHART_PAD_TOP + plot_h * tick as f64 / 3.0;
        body.push_str(&format!(
            r#"<Line X1="0" Y1="{y:.2}" X2="{width:.2}" Y2="{y:.2}" Stroke="{grid}" StrokeThickness="1" />"#
        ));
    }

    let xs = series_x_positions(series.len(), width);
    let mut fills = String::new();
    let mut strokes = String::new();
    for entry in providers {
        let color = series_color(entry.provider, color_scheme);
        let ys: Vec<f64> = series
            .iter()
            .map(|point| {
                let value = point
                    .by_provider
                    .get(&entry.provider)
                    .copied()
                    .unwrap_or(0);
                let ratio = if max_value == 0 {
                    0.0
                } else {
                    value as f64 / max_value as f64
                };
                baseline - ratio * plot_h
            })
            .collect();
        if ys.iter().all(|y| (*y - baseline).abs() < 0.01) {
            continue;
        }
        let stroke = monotone_stroke_path(&xs, &ys);
        let fill = format!(
            "{stroke} L {last_x:.2},{baseline:.2} L {first_x:.2},{baseline:.2} Z",
            last_x = xs[xs.len() - 1],
            first_x = xs[0],
        );
        fills.push_str(&format!(
            r#"<Path Fill="{}" Data="{fill}" />"#,
            xaml_rgba(color, 0x33),
        ));
        strokes.push_str(&format!(
            r#"<Path Stroke="{}" StrokeThickness="2" StrokeStartLineCap="Round" StrokeEndLineCap="Round" StrokeLineJoin="Round" Data="{stroke}" />"#,
            xaml_rgb(color),
        ));
    }

    body.push_str(&fills);
    body.push_str(&strokes);
    body.push_str(&format!(
        r#"<Line X1="0" Y1="{baseline:.2}" X2="{width:.2}" Y2="{baseline:.2}" Stroke="{{ThemeResource AccentFillColorDefaultBrush}}" StrokeThickness="1.25" />"#
    ));

    format!(
        r#"<Canvas xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Width="{width:.0}" Height="{height:.0}" Background="Transparent">{body}</Canvas>"#
    )
}

fn series_x_positions(count: usize, width: f64) -> Vec<f64> {
    let count = count.max(1);
    let plot_w = (width - CHART_PAD_X * 2.0).max(1.0);
    if count == 1 {
        return vec![CHART_PAD_X, width - CHART_PAD_X];
    }
    (0..count)
        .map(|index| CHART_PAD_X + plot_w * index as f64 / (count - 1) as f64)
        .collect()
}

fn monotone_stroke_path(xs: &[f64], ys: &[f64]) -> String {
    if xs.is_empty() || ys.is_empty() {
        return String::new();
    }
    if xs.len() == 1 {
        return format!("M {x:.2},{y:.2} L {x:.2},{y:.2}", x = xs[0], y = ys[0]);
    }
    // A single daily sample is stretched across the plot so the area still reads.
    let ys = if xs.len() == 2 && ys.len() == 1 {
        vec![ys[0], ys[0]]
    } else {
        ys.to_vec()
    };
    let tangents = monotone_tangents(xs, &ys);
    let mut path = format!("M {:.2},{:.2}", xs[0], ys[0]);
    for index in 0..xs.len() - 1 {
        let dx = xs[index + 1] - xs[index];
        path.push_str(&format!(
            " C {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}",
            xs[index] + dx / 3.0,
            ys[index] + tangents[index] * dx / 3.0,
            xs[index + 1] - dx / 3.0,
            ys[index + 1] - tangents[index + 1] * dx / 3.0,
            xs[index + 1],
            ys[index + 1],
        ));
    }
    path
}

fn monotone_tangents(xs: &[f64], ys: &[f64]) -> Vec<f64> {
    let n = ys.len();
    let mut tangents = vec![0.0; n];
    if n < 2 {
        return tangents;
    }
    let mut slopes = vec![0.0; n - 1];
    for index in 0..n - 1 {
        let dx = (xs[index + 1] - xs[index]).max(f64::EPSILON);
        slopes[index] = (ys[index + 1] - ys[index]) / dx;
    }
    tangents[0] = slopes[0];
    tangents[n - 1] = slopes[n - 2];
    for index in 1..n - 1 {
        if slopes[index - 1] * slopes[index] <= 0.0 {
            tangents[index] = 0.0;
        } else {
            tangents[index] = (slopes[index - 1] + slopes[index]) / 2.0;
        }
    }
    for index in 0..n - 1 {
        if slopes[index].abs() < f64::EPSILON {
            tangents[index] = 0.0;
            tangents[index + 1] = 0.0;
            continue;
        }
        let alpha = tangents[index] / slopes[index];
        let beta = tangents[index + 1] / slopes[index];
        let sum = alpha * alpha + beta * beta;
        if sum > 9.0 {
            let scale = 3.0 / sum.sqrt();
            tangents[index] = scale * alpha * slopes[index];
            tangents[index + 1] = scale * beta * slopes[index];
        }
    }
    tangents
}

fn fill_hourly_series(series: &[DailySeriesPoint]) -> Vec<DailySeriesPoint> {
    if series.len() >= 24 {
        return series.to_vec();
    }
    if series.is_empty() {
        return Vec::new();
    }
    series.to_vec()
}

fn fill_daily_series(
    series: &[DailySeriesPoint],
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Vec<DailySeriesPoint> {
    if series.is_empty() || start_date > end_date {
        return Vec::new();
    }
    let mut by_date = BTreeMap::new();
    for point in series {
        by_date.insert(point.date, point.clone());
    }
    let mut filled = Vec::new();
    let mut day = start_date;
    while day <= end_date {
        filled.push(by_date.remove(&day).unwrap_or(DailySeriesPoint {
            at: start_of_local_day(day),
            date: day,
            by_provider: BTreeMap::new(),
            total: 0,
        }));
        day += Duration::days(1);
    }
    filled
}

fn start_of_local_day(date: NaiveDate) -> DateTime<Local> {
    use chrono::TimeZone;
    date.and_hms_opt(0, 0, 0)
        .and_then(|naive| Local.from_local_datetime(&naive).single())
        .unwrap_or_else(Local::now)
}

fn series_color(provider: ProviderKind, color_scheme: ColorScheme) -> Color {
    match provider {
        ProviderKind::Cursor => match color_scheme {
            ColorScheme::Dark => Color::rgb(236, 236, 236),
            ColorScheme::Light => Color::rgb(28, 28, 28),
        },
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => match color_scheme {
            ColorScheme::Dark => Color::rgb(210, 210, 210),
            ColorScheme::Light => Color::rgb(72, 72, 72),
        },
        _ => {
            let (red, green, blue) = provider_registry::descriptor(provider).brand_rgb;
            Color::rgb(red, green, blue)
        }
    }
}

fn nice_ceiling(value: u64) -> u64 {
    if value <= 1 {
        return 1;
    }
    let magnitude = 10f64.powf((value as f64).log10().floor());
    let normalized = value as f64 / magnitude;
    let nice = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 2.5 {
        2.5
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    };
    (nice * magnitude).round().max(1.0) as u64
}

fn format_axis_value(value: u64, metric: OverviewMetric) -> String {
    match metric {
        OverviewMetric::Cost => format_spend(value),
        OverviewMetric::Tokens => format_token_count(value),
    }
}

fn format_axis_date(date: NaiveDate) -> String {
    date.format("%b %-d").to_string()
}

fn xaml_rgb(color: Color) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
}

fn xaml_rgba(color: Color, alpha: u8) -> String {
    format!("#{:02X}{:02X}{:02X}{:02X}", alpha, color.r, color.g, color.b)
}

fn chart_tooltip(
    point: &DailySeriesPoint,
    providers: &[ProviderOverview],
    metric: OverviewMetric,
    hourly: bool,
) -> Element {
    let mut total_cents = 0_u64;
    let mut total_tokens = 0_u64;
    let mut rows: Vec<Element> = Vec::new();
    for entry in providers {
        let Some(value) = point.by_provider.get(&entry.provider).copied() else {
            continue;
        };
        let amount = match metric {
            OverviewMetric::Cost => {
                let cents = spend_display_cents(value);
                total_cents = total_cents.saturating_add(cents);
                format_spend_cents(cents)
            }
            OverviewMetric::Tokens => {
                total_tokens = total_tokens.saturating_add(value);
                format_token_count(value)
            }
        };
        rows.push(
            grid((
                crate::icons::element(
                    provider_registry::descriptor(entry.provider).icon,
                    14.0,
                    provider_brand_color(entry.provider, ColorScheme::Dark, true),
                ),
                caption(provider_registry::descriptor(entry.provider).display_name)
                    .grid_column(1),
                caption(amount)
                    .horizontal_alignment(HorizontalAlignment::Right)
                    .grid_column(2),
            ))
            .columns([
                GridLength::Auto,
                GridLength::Star(1.0),
                GridLength::Auto,
            ])
            .with_key(format!("usage-tip-row-{}", entry.provider.id()))
            .into(),
        );
    }

    let mut items = vec![body_strong(if hourly {
        format!(
            "{} · {}",
            point.date.format("%b %-d"),
            crate::usage_overview::format_hour_label(point.at)
        )
    } else {
        point.date.format("%b %-d").to_string()
    })
    .with_key("usage-tip-title")
    .into()];
    items.extend(rows);
    items.push(
        caption(format!(
            "Total {}",
            match metric {
                OverviewMetric::Cost => format_spend_cents(total_cents),
                OverviewMetric::Tokens => format_token_count(total_tokens),
            }
        ))
        .foreground(ThemeRef::SecondaryText)
        .with_key("usage-tip-total")
        .into(),
    );

    border(vstack(items).spacing(4.0).with_key("usage-tip-rows"))
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
    usage_card(
        vstack((
            body_strong("Totals"),
            grid((
                total_metric("Processed tokens", format_token_count(processed))
                    .grid_column(0)
                    .grid_row(0),
                total_metric(
                    "Cached input",
                    format_token_count(totals.cached_input_tokens),
                )
                .grid_column(1)
                .grid_row(0),
                total_metric("Uncached input", format_token_count(uncached))
                    .grid_column(0)
                    .grid_row(1),
                total_metric("Output", format_token_count(totals.output_tokens))
                    .grid_column(1)
                    .grid_row(1),
                total_metric(
                    "Cache savings",
                    format_spend(totals.cache_savings_microusd),
                )
                .grid_column(0)
                .grid_row(2)
                .grid_column_span(2),
            ))
            .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
            .rows([
                GridLength::Auto,
                GridLength::Auto,
                GridLength::Auto,
            ])
            .row_spacing(8.0)
            .column_spacing(8.0),
        ))
        .spacing(8.0),
    )
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
    usage_card(
        vstack((
            grid((
                body_strong("Breakdown").vertical_alignment(VerticalAlignment::Center),
                segmented_control(
                    "usage-breakdown",
                    vec![
                        segmented_tab("Model", breakdown == BreakdownMode::Model, {
                            let set_breakdown = set_breakdown.clone();
                            move || set_breakdown.call(BreakdownMode::Model)
                        }),
                        segmented_tab("Day", breakdown == BreakdownMode::Day, {
                            let set_breakdown = set_breakdown.clone();
                            move || set_breakdown.call(BreakdownMode::Day)
                        }),
                    ],
                    false,
                )
                .horizontal_alignment(HorizontalAlignment::Right)
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(1),
            ))
            .columns([GridLength::Star(1.0), GridLength::Auto]),
            vstack((
                breakdown_header(),
                vstack(
                    rows.iter()
                        .map(|row| {
                            breakdown_row(row, metric, color_scheme, use_colored_provider_icons)
                        })
                        .collect::<Vec<_>>(),
                )
                .spacing(6.0),
            ))
            .spacing(6.0),
        ))
        .spacing(14.0),
    )
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
    _metric: OverviewMetric,
    color_scheme: ColorScheme,
    use_colored_provider_icons: bool,
) -> Element {
    let mut title = Vec::new();
    if let Some(provider) = row.provider {
        title.push(
            crate::icons::element(
                provider_registry::descriptor(provider).icon,
                14.0,
                provider_brand_color(provider, color_scheme, use_colored_provider_icons),
            )
            .into(),
        );
    }
    title.push(
        caption(&row.label)
            .margin(Thickness {
                left: if row.provider.is_some() { 6.0 } else { 0.0 },
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
            })
            .into(),
    );
    if let Some(weekday) = &row.weekday {
        title.push(
            caption(weekday)
                .foreground(ThemeRef::TertiaryText)
                .vertical_alignment(VerticalAlignment::Bottom)
                .into(),
        );
    }
    grid((
        hstack(title)
            .spacing(4.0)
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
    format_spend_dollars((microusd as f64 / 1_000_000.0).round() as u64)
}

fn format_spend_full(microusd: u64) -> String {
    let cents = spend_display_cents(microusd);
    format!(
        "${}.{:02}",
        format_thousands(cents / 100),
        cents % 100
    )
}

fn format_thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    grouped.chars().rev().collect()
}

fn spend_display_cents(microusd: u64) -> u64 {
    (microusd as f64 / 10_000.0).round().clamp(0.0, u64::MAX as f64) as u64
}

fn format_spend_cents(cents: u64) -> String {
    format_spend_dollars(((cents as f64) / 100.0).round() as u64)
}

fn format_spend_dollars(dollars: u64) -> String {
    if dollars >= 1_000_000 {
        format!("${}M", dollars / 1_000_000)
    } else if dollars >= 1_000 {
        format!("${}K", dollars / 1_000)
    } else {
        format!("${dollars}")
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
