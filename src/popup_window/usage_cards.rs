use super::*;

pub(super) fn combined_usage_card(
    limits: &ProviderLimits,
    is_first: bool,
    codex_enabled: bool,
    claude_enabled: bool,
    cursor_enabled: bool,
    opencode_zen_enabled: bool,
    opencode_go_enabled: bool,
    openrouter_enabled: bool,
    period: TotalSpendPeriod,
    on_period: impl Fn(TotalSpendPeriod) + Clone + 'static,
    hovered_period: Option<TotalSpendPeriod>,
    set_hovered_period: SetState<Option<TotalSpendPeriod>>,
    color_scheme: ColorScheme,
    use_colored_provider_icons: bool,
    presentation: TotalSpendPresentation,
    drag_handle: Option<Element>,
    on_open_usage: impl Fn() + Clone + 'static,
    hovered_chrome: Option<UsageStatsHover>,
    set_hovered_chrome: SetState<Option<UsageStatsHover>>,
) -> Element {
    let enabled: Vec<_> = crate::provider_registry::PROVIDERS
        .iter()
        .filter_map(|descriptor| {
            if !descriptor.include_in_total_spend {
                return None;
            }
            let enabled = match descriptor.kind {
                ProviderKind::Codex => codex_enabled,
                ProviderKind::Claude => claude_enabled,
                ProviderKind::Cursor => cursor_enabled,
                ProviderKind::OpenCodeZen => opencode_zen_enabled,
                ProviderKind::OpenCodeGo => opencode_go_enabled,
                ProviderKind::OpenRouter => openrouter_enabled,
            };
            enabled.then_some(descriptor.kind)
        })
        .collect();
    let snapshot = crate::usage_overview::total_spend_snapshot(limits, &enabled, period);
    let entries = crate::usage_overview::spend_entries(&snapshot);
    let total_spend = entries
        .iter()
        .fold(0_u64, |total, (_, spend)| total.saturating_add(*spend));
    let content = match presentation {
        TotalSpendPresentation::Donut => combined_usage_donut_content(
            &entries,
            total_spend,
            period,
            color_scheme,
            use_colored_provider_icons,
        ),
        TotalSpendPresentation::ProgressBar => combined_usage_hero_content(
            &entries,
            total_spend,
            color_scheme,
            use_colored_provider_icons,
        ),
    };

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

    let on_open_title = on_open_usage.clone();
    let set_hovered_title = set_hovered_chrome.clone();
    vstack((
        grid((
            usage_stats_title(
                hovered_chrome == Some(UsageStatsHover::Title),
                set_hovered_title,
                on_open_title,
            )
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
        usage_stats_card(
            content,
            hovered_chrome == Some(UsageStatsHover::Card),
            set_hovered_chrome,
            on_open_usage,
        ),
    ))
    .spacing(6.0)
    .into()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UsageStatsHover {
    Title,
    Card,
}

fn usage_stats_title(
    hovered: bool,
    set_hovered: SetState<Option<UsageStatsHover>>,
    on_open: impl Fn() + Clone + 'static,
) -> Element {
    let anim = crate::theme::duration(Duration::from_millis(200));
    let set_on_enter = set_hovered.clone();
    let set_on_exit = set_hovered;
    let layers = vec![
        body_strong("Usage Stats")
            .foreground(ThemeRef::SecondaryText)
            .opacity(if hovered { 0.0 } else { 1.0 })
            .with_opacity_transition(anim)
            .relative_align_left()
            .relative_align_v_center()
            .into(),
        body_strong("Usage Stats")
            .foreground(ThemeRef::Accent)
            .opacity(if hovered { 1.0 } else { 0.0 })
            .with_opacity_transition(anim)
            .relative_align_left()
            .relative_align_v_center()
            .into(),
        usage_stats_hit_layer(
            on_open,
            move |_| set_on_enter.call(Some(UsageStatsHover::Title)),
            move || set_on_exit.call(None),
            "usage-stats-title",
        ),
    ];
    relative_panel(layers)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Center)
        .with_key("usage-stats-title")
        .into()
}

fn usage_stats_card(
    content: Element,
    hovered: bool,
    set_hovered: SetState<Option<UsageStatsHover>>,
    on_open: impl Fn() + Clone + 'static,
) -> Element {
    let radius = f64::from(popup::CARD_CORNER_RADIUS_DIP);
    let hover_anim = crate::theme::duration(crate::theme::CONTROL_FASTER_ANIMATION);
    let set_on_enter = set_hovered.clone();
    let set_on_exit = set_hovered;
    relative_panel(vec![
        border(Element::Empty)
            .background(ThemeRef::CardBackground)
            .corner_radius(radius)
            .border_thickness(Thickness::uniform(1.0))
            .border_brush(ThemeRef::CardStroke)
            .relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .relative_align_bottom()
            .into(),
        border(Element::Empty)
            .background(ThemeRef::SubtleFill)
            .opacity(if hovered { 1.0 } else { 0.0 })
            .with_opacity_transition(hover_anim)
            .corner_radius(radius)
            .relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .relative_align_bottom()
            .into(),
        border(content)
            .padding(Thickness::uniform(12.0))
            .background(Color::transparent())
            .relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .relative_align_bottom()
            .into(),
        usage_stats_hit_layer(
            on_open,
            move |_| set_on_enter.call(Some(UsageStatsHover::Card)),
            move || set_on_exit.call(None),
            "usage-stats-card",
        ),
    ])
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .with_key("usage-stats-card")
    .into()
}

fn usage_stats_hit_layer(
    on_open: impl Fn() + Clone + 'static,
    on_enter: impl Fn(PointerEventInfo) + Clone + 'static,
    on_exit: impl Fn() + Clone + 'static,
    key: &str,
) -> Element {
    border(Element::Empty)
        .background(Color::transparent())
        .relative_align_left()
        .relative_align_right()
        .relative_align_top()
        .relative_align_bottom()
        .on_pointer_entered(on_enter)
        .on_pointer_exited(on_exit)
        .on_tapped(move || on_open())
        .with_key(format!("{key}-hit"))
        .into()
}

pub(super) fn combined_usage_donut_content(
    entries: &[(ProviderKind, u64)],
    total_spend: u64,
    period: TotalSpendPeriod,
    color_scheme: ColorScheme,
    use_colored_provider_icons: bool,
) -> Element {
    let set_key = spend_provider_set_key(entries);
    let provider_totals = vstack(
        entries
            .iter()
            .map(|(provider, spend)| {
                combined_usage_row(
                    *provider,
                    *spend,
                    color_scheme,
                    use_colored_provider_icons,
                )
            })
            .collect::<Vec<_>>(),
    )
    .spacing(12.0)
    .vertical_alignment(VerticalAlignment::Center)
    .with_key(format!(
        "spend-legend-{set_key}-{}-{}",
        color_scheme as i32, use_colored_provider_icons
    ));

    grid((
        combined_usage_donut(entries, total_spend, period, color_scheme).margin(Thickness {
            left: 0.0,
            top: 0.0,
            right: 16.0,
            bottom: 0.0,
        }),
        provider_totals
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(1),
    ))
    .columns([GridLength::Auto, GridLength::Star(1.0)])
    .rows([GridLength::Auto])
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

pub(super) fn combined_usage_hero_content(
    entries: &[(ProviderKind, u64)],
    total_spend: u64,
    color_scheme: ColorScheme,
    use_colored_provider_icons: bool,
) -> Element {
    let set_key = spend_provider_set_key(entries);
    vstack((
        vstack((
            text_block(format_spend_full(total_spend))
                .font_size(28.0)
                .font_weight(600)
                .vertical_alignment(VerticalAlignment::Center),
            combined_usage_progress_bar(entries, color_scheme)
                .with_key(format!("spend-hero-bar-{set_key}")),
        ))
        .spacing(8.0),
        spend_provider_tiles(entries, color_scheme, use_colored_provider_icons),
    ))
    .spacing(16.0)
    .into()
}

pub(super) fn combined_usage_progress_bar(
    entries: &[(ProviderKind, u64)],
    color_scheme: ColorScheme,
) -> Element {
    let total_spend = entries
        .iter()
        .fold(0_u64, |total, (_, spend)| total.saturating_add(*spend));
    let mut columns = Vec::with_capacity(entries.len().saturating_mul(2).saturating_sub(1));
    for (index, (_, spend)) in entries.iter().enumerate() {
        if index > 0 {
            columns.push(GridLength::Pixel(4.0));
        }
        let weight = if total_spend == 0 { 1 } else { *spend.max(&1) };
        columns.push(GridLength::Star(weight as f64));
    }
    let segments: Vec<Element> = entries
        .iter()
        .enumerate()
        .map(|(index, (provider, _))| {
            border(Element::Empty)
                .background(combined_usage_color(*provider, color_scheme))
                .height(10.0)
                .corner_radius(4.0)
                .grid_column((index * 2) as i32)
                .into()
        })
        .collect();

    grid(segments)
        .columns(columns)
        .rows([GridLength::Pixel(10.0)])
        .height(10.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into()
}

pub(super) fn combined_usage_period_selector(
    selected: TotalSpendPeriod,
    on_select: impl Fn(TotalSpendPeriod) + Clone + 'static,
    hovered: Option<TotalSpendPeriod>,
    set_hovered: SetState<Option<TotalSpendPeriod>>,
) -> Element {
    let buttons: Vec<Element> = [
        TotalSpendPeriod::Today,
        TotalSpendPeriod::Yesterday,
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

/// Draws a true circular ring with native WinUI arc paths.
///
/// The swap-chain host key stays stable across spend refreshes. Remounting it
/// on every usage update recreated unmanaged XAML children and grew the WinUI
/// compositor working set over long runs. Geometry is reinstalled in place
/// when the series fingerprint changes.
pub(super) fn combined_usage_donut(
    entries: &[(ProviderKind, u64)],
    total_spend: u64,
    period: TotalSpendPeriod,
    color_scheme: ColorScheme,
) -> Element {
    const SIZE: f64 = 124.0;
    thread_local! {
        static DONUT_MOUNTS: std::cell::RefCell<
            std::collections::HashMap<String, windows_core::IInspectable>,
        > = std::cell::RefCell::new(std::collections::HashMap::new());
        static DONUT_SERIES: std::cell::RefCell<std::collections::HashMap<String, u64>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
    }

    let xaml = combined_usage_donut_xaml(entries, total_spend, color_scheme);
    let series_key = entries.iter().fold(0_u64, |hash, (provider, spend)| {
        hash.wrapping_mul(31)
            .wrapping_add(*spend)
            .wrapping_add(*provider as u64)
    });
    // Stable host identity — theme/period changes remount; spend updates do not.
    let host_key = format!("spend-donut-{}-{:?}", period.key(), color_scheme);
    let series_fingerprint = series_key.wrapping_add(total_spend);

    let series_changed = DONUT_SERIES.with(|cache| {
        let mut cache = cache.borrow_mut();
        match cache.get(&host_key) {
            Some(previous) if *previous == series_fingerprint => false,
            _ => {
                cache.insert(host_key.clone(), series_fingerprint);
                true
            }
        }
    });
    if series_changed {
        DONUT_MOUNTS.with(|mounts| {
            if let Some(native) = mounts.borrow().get(&host_key).cloned()
                && let Err(error) = crate::acrylic::install_spend_donut_into(native, &xaml)
            {
                eprintln!("Could not update spend donut: {error:?}");
            }
        });
    }

    let xaml_for_mount = xaml.clone();
    let key_for_mount = host_key.clone();
    let key_for_unmount = host_key.clone();
    let mut host = swap_chain_panel().width(SIZE).height(SIZE);
    host.mounted = Some(Callback::new(
        move |native: Option<windows_core::IInspectable>| {
            if let Some(native) = native {
                if let Err(error) =
                    crate::acrylic::install_spend_donut_into(native.clone(), &xaml_for_mount)
                {
                    eprintln!("Could not install spend donut: {error:?}");
                }
                DONUT_MOUNTS.with(|mounts| {
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
            DONUT_MOUNTS.with(|mounts| {
                mounts.borrow_mut().remove(&key_for_unmount);
            });
            DONUT_SERIES.with(|cache| {
                cache.borrow_mut().remove(&key_for_unmount);
            });
        },
    ));
    let donut: Element = host.with_key(host_key).into();

    grid((
        donut,
        text_block(format_spend_compact(total_spend))
            .font_size(18.0)
            .font_weight(600)
            .horizontal_alignment(HorizontalAlignment::Center)
            .vertical_alignment(VerticalAlignment::Center),
    ))
    .columns([GridLength::Auto])
    .rows([GridLength::Auto])
    .width(SIZE)
    .height(SIZE)
    .vertical_alignment(VerticalAlignment::Center)
    .into()
}

pub(super) fn combined_usage_donut_xaml(
    entries: &[(ProviderKind, u64)],
    total_spend: u64,
    color_scheme: ColorScheme,
) -> String {
    const CENTER: f64 = 62.0;
    const OUTER_RADIUS: f64 = 53.0;
    const INNER_RADIUS: f64 = 34.0;
    const GAP_DEGREES: f64 = 2.0;

    let paths = if total_spend == 0 {
        donut_path("#787878", -90.0, 270.0, CENTER, OUTER_RADIUS, INNER_RADIUS)
    } else {
        let mut start = -90.0;
        entries
            .iter()
            .filter(|(_, spend)| *spend > 0)
            .map(|(provider, spend)| {
                let end = start + *spend as f64 / total_spend as f64 * 360.0;
                let path = donut_path(
                    &xaml_color(combined_usage_color(*provider, color_scheme)),
                    start + GAP_DEGREES / 2.0,
                    end - GAP_DEGREES / 2.0,
                    CENTER,
                    OUTER_RADIUS,
                    INNER_RADIUS,
                );
                start = end;
                path
            })
            .collect::<String>()
    };

    format!(
        r#"<Grid xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Width="124" Height="124">{paths}</Grid>"#
    )
}

pub(super) fn donut_path(
    color: &str,
    start: f64,
    end: f64,
    center: f64,
    outer: f64,
    inner: f64,
) -> String {
    let sweep = (end - start).max(0.0);
    if sweep <= 0.0 {
        return String::new();
    }
    if sweep >= 359.0 {
        return format!(
            r#"<Path Fill="{color}" Data="M {center:.2} {outer_top:.2} A {outer:.2} {outer:.2} 0 1 1 {center:.2} {outer_bottom:.2} A {outer:.2} {outer:.2} 0 1 1 {center:.2} {outer_top:.2} M {center:.2} {inner_top:.2} A {inner:.2} {inner:.2} 0 1 0 {center:.2} {inner_bottom:.2} A {inner:.2} {inner:.2} 0 1 0 {center:.2} {inner_top:.2} Z" />"#,
            outer_top = center - outer,
            outer_bottom = center + outer,
            inner_top = center - inner,
            inner_bottom = center + inner,
        );
    }
    let (outer_start_x, outer_start_y) = donut_point(center, outer, start);
    let (outer_end_x, outer_end_y) = donut_point(center, outer, end);
    let (inner_start_x, inner_start_y) = donut_point(center, inner, start);
    let (inner_end_x, inner_end_y) = donut_point(center, inner, end);
    let large_arc = u8::from(sweep > 180.0);
    format!(
        r#"<Path Fill="{color}" Data="M {outer_start_x:.2} {outer_start_y:.2} A {outer:.2} {outer:.2} 0 {large_arc} 1 {outer_end_x:.2} {outer_end_y:.2} L {inner_end_x:.2} {inner_end_y:.2} A {inner:.2} {inner:.2} 0 {large_arc} 0 {inner_start_x:.2} {inner_start_y:.2} Z" />"#
    )
}

pub(super) fn donut_point(center: f64, radius: f64, degrees: f64) -> (f64, f64) {
    let radians = degrees.to_radians();
    (
        center + radius * radians.cos(),
        center + radius * radians.sin(),
    )
}

pub(super) fn xaml_color(color: Color) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
}

fn spend_provider_set_key(entries: &[(ProviderKind, u64)]) -> String {
    entries
        .iter()
        .map(|(provider, _)| provider.id())
        .collect::<Vec<_>>()
        .join("+")
}

fn spend_provider_icon_color(
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
    let (red, green, blue) = crate::provider_registry::descriptor(provider).brand_rgb;
    Color::rgb(red, green, blue)
}

fn spend_provider_icon(
    provider: ProviderKind,
    color_scheme: ColorScheme,
    use_colored: bool,
    slot: &str,
) -> Element {
    let descriptor = crate::provider_registry::descriptor(provider);
    let color = spend_provider_icon_color(provider, color_scheme, use_colored);
    crate::icons::element(descriptor.icon, 16.0, color)
        .vertical_alignment(VerticalAlignment::Center)
        .with_key(format!(
            "spend-{slot}-icon-{}-{}-{:02X}{:02X}{:02X}",
            provider.id(),
            descriptor.icon,
            color.r,
            color.g,
            color.b
        ))
}

fn combined_usage_row(
    provider: ProviderKind,
    spend: u64,
    color_scheme: ColorScheme,
    use_colored_provider_icons: bool,
) -> Element {
    grid((
        spend_provider_icon(provider, color_scheme, use_colored_provider_icons, "row"),
        body_strong(provider.display_name())
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(1),
        body_strong(format_spend_compact(spend))
            .horizontal_alignment(HorizontalAlignment::Right)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(2),
    ))
    .columns([
        GridLength::Auto,
        GridLength::Star(1.0),
        GridLength::Auto,
    ])
    .column_spacing(8.0)
    .rows([GridLength::Auto])
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .with_key(format!("spend-row-{}", provider.id()))
    .into()
}

fn spend_provider_tiles(
    entries: &[(ProviderKind, u64)],
    color_scheme: ColorScheme,
    use_colored_provider_icons: bool,
) -> Element {
    const COLUMNS: usize = 3;
    let row_count = entries.len().div_ceil(COLUMNS).max(1);
    let set_key = spend_provider_set_key(entries);
    let cells = entries
        .iter()
        .enumerate()
        .map(|(index, (provider, spend))| {
            spend_provider_tile(
                *provider,
                *spend,
                color_scheme,
                use_colored_provider_icons,
            )
            .grid_row((index / COLUMNS) as i32)
            .grid_column((index % COLUMNS) as i32)
        })
        .collect::<Vec<_>>();

    grid(cells)
        .columns(vec![GridLength::Star(1.0); COLUMNS])
        .rows(vec![GridLength::Auto; row_count])
        .row_spacing(16.0)
        .column_spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .with_key(format!(
            "spend-hero-providers-{COLUMNS}-{set_key}-{}-{}",
            color_scheme as i32, use_colored_provider_icons
        ))
        .into()
}

fn spend_provider_tile(
    provider: ProviderKind,
    spend: u64,
    color_scheme: ColorScheme,
    use_colored_provider_icons: bool,
) -> Element {
    let descriptor = crate::provider_registry::descriptor(provider);
    let color = spend_provider_icon_color(provider, color_scheme, use_colored_provider_icons);
    vstack((
        grid((
            crate::icons::element(descriptor.icon, 16.0, color)
                .vertical_alignment(VerticalAlignment::Center)
                .with_key(format!(
                    "spend-hero-icon-{}-{}-{:02X}{:02X}{:02X}",
                    provider.id(),
                    descriptor.icon,
                    color.r,
                    color.g,
                    color.b
                )),
            body_strong(descriptor.display_name)
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(1),
        ))
        .columns([GridLength::Auto, GridLength::Star(1.0)])
        .column_spacing(8.0)
        .rows([GridLength::Auto]),
        caption(format_spend_full(spend))
            .font_weight(600)
            .foreground(ThemeRef::PrimaryText),
    ))
    .spacing(4.0)
    .with_key(format!("spend-hero-provider-{}", provider.id()))
    .into()
}

pub(super) fn combined_usage_color(provider: ProviderKind, color_scheme: ColorScheme) -> Color {
    match provider {
        ProviderKind::Codex => Color::rgb(128, 159, 255),
        ProviderKind::Claude => Color::rgb(217, 119, 87),
        ProviderKind::Cursor => match color_scheme {
            ColorScheme::Light => Color::rgb(18, 18, 18),
            ColorScheme::Dark => Color::rgb(230, 230, 230),
        },
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => match color_scheme {
            ColorScheme::Light => Color::rgb(75, 75, 75),
            ColorScheme::Dark => Color::rgb(205, 205, 205),
        },
        ProviderKind::OpenRouter => Color::rgb(200, 255, 0),
    }
}

pub(super) fn format_spend(microusd: u64) -> String {
    format_usd(microusd as f64 / 1_000_000.0)
}

fn format_spend_full(microusd: u64) -> String {
    let cents = spend_display_cents(microusd);
    format!("${}.{:02}", format_thousands(cents / 100), cents % 100)
}

fn format_spend_compact(microusd: u64) -> String {
    format_spend_compact_dollars((microusd as f64 / 1_000_000.0).round() as u64)
}

fn format_spend_compact_dollars(dollars: u64) -> String {
    if dollars >= 1_000_000 {
        format!("${:.1}M", dollars as f64 / 1_000_000.0)
    } else if dollars >= 1_000 {
        format!("${:.1}K", dollars as f64 / 1_000.0)
    } else {
        format!("${dollars}")
    }
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
    (microusd as f64 / 10_000.0)
        .round()
        .clamp(0.0, u64::MAX as f64) as u64
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
