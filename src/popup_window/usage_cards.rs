use super::*;
use crate::usage_overview::OverviewSnapshot;

pub(super) fn combined_usage_card(
    snapshot: &OverviewSnapshot,
    is_first: bool,
    period: TotalSpendPeriod,
    on_period: impl Fn(TotalSpendPeriod) + Clone + 'static,
    hovered_period: Option<TotalSpendPeriod>,
    set_hovered_period: SetState<Option<TotalSpendPeriod>>,
    color_scheme: ColorScheme,
    use_colored_provider_icons: bool,
    drag_handle: Option<Element>,
) -> Element {
    let content = crate::popup_usage::usage_hero(
        snapshot,
        crate::usage_overview::OverviewMetric::Cost,
        color_scheme,
        use_colored_provider_icons,
    );

    let mut title_trailing_items: Vec<Element> = vec![
        combined_usage_period_selector(period, on_period, hovered_period, set_hovered_period)
            .into(),
    ];
    if let Some(handle) = drag_handle {
        title_trailing_items.push(handle);
    }
    let title_trailing = hstack(title_trailing_items)
        .spacing(4.0)
        .horizontal_alignment(HorizontalAlignment::Right)
        .vertical_alignment(VerticalAlignment::Center)
        .grid_column(1);

    vstack((
        grid((
            body_strong("Total Spend")
                .foreground(ThemeRef::SecondaryText)
                .vertical_alignment(VerticalAlignment::Center),
            title_trailing,
        ))
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
            "total-spend-heading-{}",
            if is_first { "first" } else { "rest" }
        )),
        content,
    ))
    .spacing(6.0)
    .into()
}

pub(super) fn combined_usage_period_selector(
    selected: TotalSpendPeriod,
    on_select: impl Fn(TotalSpendPeriod) + Clone + 'static,
    hovered: Option<TotalSpendPeriod>,
    set_hovered: SetState<Option<TotalSpendPeriod>>,
) -> Element {
    let buttons: Vec<Element> = [
        TotalSpendPeriod::Past24h,
        TotalSpendPeriod::SevenDays,
        TotalSpendPeriod::ThirtyDays,
    ]
    .into_iter()
    .map(|period| {
        combined_usage_period_button(
            period,
            selected,
            hovered == Some(period),
            on_select.clone(),
            set_hovered.clone(),
        )
    })
    .collect();
    hstack(buttons).spacing(12.0).into()
}

pub(super) fn combined_usage_period_button(
    period: TotalSpendPeriod,
    selected: TotalSpendPeriod,
    hovered: bool,
    on_select: impl Fn(TotalSpendPeriod) + Clone + 'static,
    set_hovered: SetState<Option<TotalSpendPeriod>>,
) -> Element {
    let is_selected = period == selected;
    let set_hovered_on_enter = set_hovered.clone();
    let set_hovered_on_exit = set_hovered;
    // Crossfade text colors: tertiary idle, secondary hover, accent selected.
    let layers: Vec<Element> = vec![
        body_strong(period.label())
            .foreground(ThemeRef::TertiaryText)
            .opacity(if !is_selected && !hovered { 1.0 } else { 0.0 })
            .with_opacity_transition(crate::theme::duration(Duration::from_millis(200)))
            .relative_align_h_center()
            .relative_align_v_center()
            .into(),
        body_strong(period.label())
            .foreground(ThemeRef::SecondaryText)
            .opacity(if !is_selected && hovered { 1.0 } else { 0.0 })
            .with_opacity_transition(crate::theme::duration(Duration::from_millis(200)))
            .relative_align_h_center()
            .relative_align_v_center()
            .into(),
        body_strong(period.label())
            .foreground(ThemeRef::Accent)
            .opacity(if is_selected { 1.0 } else { 0.0 })
            .with_opacity_transition(crate::theme::duration(Duration::from_millis(200)))
            .relative_align_h_center()
            .relative_align_v_center()
            .into(),
    ];
    relative_panel(layers)
        .on_pointer_entered(move |_: PointerEventInfo| {
            set_hovered_on_enter.call(Some(period));
        })
        .on_pointer_exited(move || set_hovered_on_exit.call(None))
        .on_tapped(move || on_select(period))
        .with_key(format!("combined-period-{}-{is_selected}", period.key()))
        .into()
}

pub(super) fn format_spend(microusd: u64) -> String {
    crate::usage_overview::format_spend(microusd)
}

pub(super) fn usage_tokens_and_cost_metric(label: &str, tokens: String, cost: String) -> Element {
    vstack((
        caption(label).foreground(ThemeRef::TertiaryText),
        hstack((
            text_block(tokens).font_weight(600),
            caption(format!("≈ {cost}"))
                .foreground(ThemeRef::TertiaryText)
                .vertical_alignment(VerticalAlignment::Center),
        ))
        .spacing(5.0)
        .vertical_alignment(VerticalAlignment::Center),
    ))
    .spacing(1.0)
    .vertical_alignment(VerticalAlignment::Center)
    .into()
}

pub(super) fn usage_value_metric(label: &str, value: String, requests: u64) -> Element {
    vstack((
        caption(label).foreground(ThemeRef::TertiaryText),
        hstack((
            text_block(value).font_weight(600),
            caption(format!("{requests} requests"))
                .foreground(ThemeRef::TertiaryText)
                .vertical_alignment(VerticalAlignment::Center),
        ))
        .spacing(5.0)
        .vertical_alignment(VerticalAlignment::Center),
    ))
    .spacing(1.0)
    .vertical_alignment(VerticalAlignment::Center)
    .into()
}

pub(super) fn is_cost_provider(provider: ProviderKind) -> bool {
    matches!(
        provider,
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo
    )
}

/// Compact, screenshot-style activity chart. For long histories, adjacent days
/// are grouped into a single bar so the chart stays legible in the tray popup.
pub(super) fn usage_activity_chart(
    statistics: &crate::usage::UsageStatistics,
    cost_based: bool,
) -> Element {
    const MAX_BARS: usize = 60;
    const CHART_HEIGHT: f64 = 56.0;
    const BAR_GAP: f64 = 2.0;

    // The popup width is fixed. Subtract its outer stroke, the body padding,
    // and this card's stroke/padding so the first and last bars sit at the
    // same inset as the rest of the card content.
    let chart_width = f64::from(popup::POPUP_WIDTH) - 2.0 - 32.0 - 2.0 - 24.0;

    let days = usize::from(statistics.history_days.max(1));
    let today = Local::now().date_naive();
    let first_day = today - ChronoDuration::days(days.saturating_sub(1) as i64);
    let daily: Vec<u64> = (0..days)
        .map(|index| {
            let date = first_day + ChronoDuration::days(index as i64);
            statistics
                .daily
                .iter()
                .find(|entry| entry.date == date)
                .map(|entry| {
                    if cost_based {
                        entry.usage.estimated_cost_microusd
                    } else {
                        entry.usage.total_tokens()
                    }
                })
                .unwrap_or_default()
        })
        .collect();
    let values = compact_activity_bars(&daily, MAX_BARS);
    let max_value = values.iter().copied().max().unwrap_or(0);
    let bar_width = ((chart_width - BAR_GAP * values.len().saturating_sub(1) as f64)
        / values.len().max(1) as f64)
        .clamp(2.0, 12.0);

    let bars: Vec<Element> = values
        .into_iter()
        .map(|tokens| {
            let height = if max_value == 0 {
                2.0
            } else {
                (CHART_HEIGHT * tokens as f64 / max_value as f64).max(2.0)
            };
            border(Element::Empty)
                .width(bar_width)
                .height(height)
                .corner_radius(1.5)
                .background(ThemeRef::Accent)
                .opacity(if tokens == 0 { 0.2 } else { 1.0 })
                .vertical_alignment(VerticalAlignment::Bottom)
                .into()
        })
        .collect();

    border(
        hstack(bars)
            .spacing(BAR_GAP)
            .height(CHART_HEIGHT)
            .vertical_alignment(VerticalAlignment::Bottom),
    )
    .height(CHART_HEIGHT)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Bottom)
    .into()
}
