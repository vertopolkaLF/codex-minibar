use super::*;
use crate::usage::{TokenUsage, UsageStatistics};
use chrono::NaiveDate;
use std::collections::BTreeMap;

const HEIGHT: f64 = 56.0;
const MAX_BARS: usize = 60;

#[derive(Clone, PartialEq)]
struct ChartProps {
    statistics: UsageStatistics,
    cost_based: bool,
    today: NaiveDate,
    scheme: ColorScheme,
    transition: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Selection {
    mask: u8,
    cost: bool,
}

impl Selection {
    fn selected(self, series: Series) -> bool {
        !self.cost && self.mask & (1 << series as u8) != 0
    }

    fn with_series(mut self, series: Series, checked: bool) -> Self {
        // WinUI also raises Checked/Unchecked when reconciliation sets IsChecked.
        // Ignore those echoes so changing modes cannot erase the token selection.
        if self.selected(series) == checked {
            return self;
        }
        let bit = 1 << series as u8;
        self.mask = if self.cost {
            bit
        } else if checked {
            self.mask | bit
        } else {
            self.mask & !bit
        };
        self.cost = false;
        self
    }
}

#[derive(Clone, Debug)]
struct Bucket {
    first: NaiveDate,
    last: NaiveDate,
    usage: TokenUsage,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Availability {
    series: u8,
    cost: bool,
}

impl Availability {
    fn from_buckets(data: &[Bucket]) -> Self {
        let mut available = Self::default();
        for bucket in data {
            for series in Series::ALL {
                if series.value(&bucket.usage) > 0 {
                    available.series |= 1 << series as u8;
                }
            }
            // A priced, free request is a known zero cost, not missing data.
            available.cost |= bucket.usage.priced_requests > 0;
        }
        available
    }

    fn selection(self, requested: Selection) -> Selection {
        Selection {
            mask: requested.mask & self.series,
            cost: self.cost && (requested.cost || self.series == 0),
        }
    }
}

#[derive(Clone, Copy)]
enum Series {
    Input = 0,
    Cache = 1,
    Output = 2,
}

impl Series {
    const ALL: [Self; 3] = [Self::Input, Self::Cache, Self::Output];

    fn label(self) -> &'static str {
        match self {
            Self::Input => "Input",
            Self::Cache => "Cache",
            Self::Output => "Output",
        }
    }

    fn value(self, usage: &TokenUsage) -> u64 {
        // Cached input is a subset of input, not an additional token stream.
        match self {
            Self::Input => usage.input_tokens.saturating_sub(usage.cached_input_tokens),
            Self::Cache => usage.cached_input_tokens.min(usage.input_tokens),
            Self::Output => usage.output_tokens,
        }
    }

    fn menu_label(self) -> &'static str {
        match self {
            Self::Input => "Input (uncached)",
            Self::Cache => "Cached input",
            Self::Output => "Output",
        }
    }

    fn brush(self, scheme: ColorScheme) -> BrushBinding {
        match (self, scheme) {
            (Self::Cache, _) => ThemeRef::Accent.into(),
            (Self::Input, ColorScheme::Dark) => Color::rgb(155, 166, 244).into(),
            (Self::Input, ColorScheme::Light) => Color::rgb(96, 89, 186).into(),
            (Self::Output, ColorScheme::Dark) => Color::rgb(93, 211, 171).into(),
            (Self::Output, ColorScheme::Light) => Color::rgb(0, 126, 96).into(),
        }
    }

    fn xaml_brush(self, scheme: ColorScheme) -> String {
        match self.brush(scheme) {
            BrushBinding::Direct(color) => {
                format!(
                    "#{:02X}{:02X}{:02X}{:02X}",
                    color.a, color.r, color.g, color.b
                )
            }
            BrushBinding::Theme(theme) => format!("{{ThemeResource {}}}", theme.resource_key()),
        }
    }
}

fn buckets(statistics: &UsageStatistics, today: NaiveDate) -> Vec<Bucket> {
    let days = usize::from(statistics.history_days.clamp(1, 365));
    let first = today - ChronoDuration::days((days - 1) as i64);
    let mut daily = BTreeMap::<NaiveDate, TokenUsage>::new();
    for entry in &statistics.daily {
        if entry.date >= first && entry.date <= today {
            daily.entry(entry.date).or_default().add(&entry.usage);
        }
    }
    let per_bar = days.div_ceil(MAX_BARS);
    (0..days)
        .step_by(per_bar)
        .map(|offset| {
            let last_offset = (offset + per_bar).min(days) - 1;
            let start = first + ChronoDuration::days(offset as i64);
            let end = first + ChronoDuration::days(last_offset as i64);
            let mut usage = TokenUsage::default();
            for (_, day) in daily.range(start..=end) {
                usage.add(day);
            }
            Bucket {
                first: start,
                last: end,
                usage,
            }
        })
        .collect()
}

fn visible_tokens(usage: &TokenUsage, mask: u8) -> u64 {
    Series::ALL
        .into_iter()
        .filter(|s| mask & (1 << *s as u8) != 0)
        .fold(0_u64, |total, s| total.saturating_add(s.value(usage)))
}

fn cost_label(usage: &TokenUsage) -> String {
    if usage.priced_requests > 0 {
        let value = format_spend(usage.estimated_cost_microusd);
        if usage.priced_requests < usage.requests {
            format!("{value} (partially priced)")
        } else {
            value
        }
    } else if usage.requests == 0 {
        "$0.00".into()
    } else {
        "Unavailable".into()
    }
}

fn bucket_tooltip(bucket: &Bucket) -> String {
    let date = if bucket.first == bucket.last {
        bucket.first.format("%a, %b %-d, %Y").to_string()
    } else {
        format!(
            "{} – {}",
            bucket.first.format("%b %-d, %Y"),
            bucket.last.format("%b %-d, %Y")
        )
    };
    let usage = &bucket.usage;
    let cost = cost_label(usage);
    format!(
        "{date}\n\nInput (uncached): {}\nCached input: {}\nOutput: {}\n\nTotal tokens: {}\nCost: {cost}\n{} requests",
        format_token_count(Series::Input.value(usage)),
        format_token_count(Series::Cache.value(usage)),
        format_token_count(Series::Output.value(usage)),
        format_token_count(usage.total_tokens()),
        usage.requests,
    )
}

fn bucket_tooltip_xaml(bucket: &Bucket, scheme: ColorScheme) -> String {
    let date = if bucket.first == bucket.last {
        bucket.first.format("%a, %b %-d, %Y").to_string()
    } else {
        format!(
            "{} – {}",
            bucket.first.format("%b %-d, %Y"),
            bucket.last.format("%b %-d, %Y")
        )
    };
    let usage = &bucket.usage;
    let total = format_token_count(usage.total_tokens());
    let cost = if usage.priced_requests > 0 {
        format_spend(usage.estimated_cost_microusd)
    } else if usage.requests == 0 {
        "$0.00".into()
    } else {
        "Unavailable".into()
    };
    let rows: String = Series::ALL.into_iter().map(|series| {
        let label = series.menu_label();
        let amount = format_token_count(series.value(usage));
        let brush = series.xaml_brush(scheme);
        // Labels are constants; amounts and dates contain only formatted numbers/date text.
        format!(r#"<Grid ColumnSpacing="8"><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition Width="*"/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions><Border Width="6" Height="6" CornerRadius="3" Background="{brush}" VerticalAlignment="Center"/><TextBlock Grid.Column="1" Text="{label}" FontSize="12" Foreground="{{ThemeResource TextFillColorSecondaryBrush}}"/><TextBlock Grid.Column="2" Text="{amount}" FontSize="12" FontWeight="SemiBold" HorizontalAlignment="Right"/></Grid>"#)
    }).collect();
    let requests = if usage.priced_requests > 0 && usage.priced_requests < usage.requests {
        format!(
            "{} requests · {} priced",
            usage.requests, usage.priced_requests
        )
    } else {
        format!("{} requests", usage.requests)
    };
    format!(
        r#"<StackPanel xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Width="210" Spacing="10" IsHitTestVisible="False">
        <TextBlock Text="{date}" FontSize="12" FontWeight="SemiBold"/>
        <Grid ColumnSpacing="12">
            <Grid.ColumnDefinitions><ColumnDefinition Width="*"/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions>
            <StackPanel Spacing="2"><TextBlock Text="Tokens" FontSize="10" Foreground="{{ThemeResource TextFillColorSecondaryBrush}}"/><TextBlock Text="{total}" FontSize="20" FontWeight="SemiBold"/></StackPanel>
            <StackPanel Grid.Column="1" Spacing="2" HorizontalAlignment="Right"><TextBlock Text="Cost" FontSize="10" HorizontalAlignment="Right" Foreground="{{ThemeResource TextFillColorSecondaryBrush}}"/><TextBlock Text="{cost}" FontSize="16" FontWeight="SemiBold" HorizontalAlignment="Right" Foreground="{{ThemeResource AccentTextFillColorPrimaryBrush}}"/></StackPanel>
        </Grid>
        <Border Height="1" Background="{{ThemeResource DividerStrokeColorDefaultBrush}}"/>
        <StackPanel Spacing="5">{rows}</StackPanel>
        <TextBlock Text="{requests}" FontSize="10" Foreground="{{ThemeResource TextFillColorSecondaryBrush}}"/>
    </StackPanel>"#
    )
}

pub(super) fn usage_activity_chart(
    provider: ProviderKind,
    statistics: &UsageStatistics,
    cost_based: bool,
) -> Element {
    component(
        render_chart,
        ChartProps {
            statistics: statistics.clone(),
            cost_based,
            today: Local::now().date_naive(),
            scheme: current_color_scheme(),
            transition: crate::theme::duration(crate::theme::CONTROL_FAST_ANIMATION),
        },
    )
    .with_key(format!("activity-chart-{}", provider.id()))
}

fn render_chart(props: &ChartProps, cx: &mut RenderCx) -> Element {
    let (requested_selection, update_selection) = cx.use_reducer(Selection {
        mask: 7,
        cost: props.cost_based,
    });
    let (hovered, set_hovered) = cx.use_state(None::<NaiveDate>);
    let data = cx.use_memo((props.statistics.clone(), props.today), || {
        buckets(&props.statistics, props.today)
    });
    let availability = Availability::from_buckets(&data);
    // Resolve against the displayed period without erasing the user's choices
    // while data is loading or a metric is temporarily unavailable.
    let selection = availability.selection(requested_selection);
    let Selection {
        mask,
        cost: cost_mode,
    } = selection;
    let chart_width = f64::from(popup::POPUP_WIDTH) - 2.0 - 32.0 - 2.0 - 24.0;
    let slot_width = chart_width / data.len() as f64;
    let bar_width = (slot_width - 2.0).clamp(1.0, 12.0);
    let maximum = data
        .iter()
        .map(|b| {
            if cost_mode {
                b.usage.estimated_cost_microusd
            } else {
                visible_tokens(&b.usage, mask)
            }
        })
        .max()
        .unwrap_or(0);
    let transition = props.transition;
    let active_hover = hovered.filter(|date| data.iter().any(|b| b.first == *date));
    let bars: Vec<Element> = data
        .iter()
        .map(|bucket| {
            let value = if cost_mode {
                bucket.usage.estimated_cost_microusd
            } else {
                visible_tokens(&bucket.usage, mask)
            };
            let date = bucket.first;
            let enter = set_hovered.clone();
            let tooltip = bucket_tooltip(bucket);
            let height = if maximum == 0 {
                2.0
            } else {
                (HEIGHT * value as f64 / maximum as f64).max(2.0)
            };
            let bar: Element = if cost_mode || value == 0 {
                border(Element::Empty)
                    .width(bar_width)
                    .height(height)
                    .corner_radius(1.5)
                    .background(ThemeRef::Accent)
                    .opacity(if value == 0 { 0.2 } else { 1.0 })
                    .with_layout_animation(
                        LayoutAnimationConfig::linear(transition).animate_size(true),
                    )
                    .with_key("single-bar")
                    .into()
            } else {
                // Keep every series host in place when a series is hidden. Native
                // layout divides the available height, including pixel rounding.
                let series = Series::ALL.into_iter().rev().collect::<Vec<_>>();
                let rows = series.iter().map(|series| {
                    GridLength::Star(if selection.selected(*series) {
                        series.value(&bucket.usage) as f64 / value as f64
                    } else {
                        0.0
                    })
                });
                let segments: Vec<Element> = series
                    .iter()
                    .enumerate()
                    .map(|(index, series)| {
                        border(Element::Empty)
                            .grid_row(index as i32)
                            .background(series.brush(props.scheme))
                            .with_layout_animation(
                                LayoutAnimationConfig::linear(transition).animate_size(true),
                            )
                            .with_key(series.label())
                            .into()
                    })
                    .collect();
                // Round the complete bar so tiny end segments cannot flatten it.
                border(grid(segments).rows(rows).columns([GridLength::Star(1.0)]))
                    .width(bar_width)
                    .height(height)
                    .corner_radius(1.5)
                    .background(Color::transparent())
                    .with_layout_animation(
                        LayoutAnimationConfig::linear(transition).animate_size(true),
                    )
                    .with_key("token-bar")
                    .into()
            };
            let column = grid([bar
                .vertical_alignment(VerticalAlignment::Bottom)
                .horizontal_alignment(HorizontalAlignment::Center)])
            .rows([GridLength::Star(1.0)])
            .columns([GridLength::Star(1.0)])
            .height(HEIGHT)
            .width(slot_width)
            .opacity(if active_hover.is_none_or(|h| h == date) {
                1.0
            } else {
                0.75
            })
            .with_opacity_transition(transition);
            // A transparent overlay owns pointer input for the entire day. The
            // colored segments must not become separate hover targets.
            let mut hit_target = border(Element::Empty)
                .width(slot_width)
                .height(HEIGHT)
                .background(Color {
                    a: 0,
                    r: 0,
                    g: 0,
                    b: 0,
                })
                .automation_name(tooltip)
                .on_pointer_entered(move |_: PointerEventInfo| enter.call(Some(date)))
                .with_key("hover-target");
            if active_hover == Some(date) {
                hit_target = hit_target.tooltip_with(
                    Tooltip::xaml(bucket_tooltip_xaml(bucket, props.scheme))
                        .placement(TooltipPlacement::Top)
                        .open(true),
                );
            }
            grid((column, hit_target))
                .rows([GridLength::Star(1.0)])
                .columns([GridLength::Star(1.0)])
                .width(slot_width)
                .height(HEIGHT)
                .with_key(format!("bar-{date}"))
                .into()
        })
        .collect();
    let exit = set_hovered.clone();
    let mut chart_children: Vec<Element> = vec![hstack(bars).spacing(0.0).height(HEIGHT).into()];
    let empty_label = if availability.series == 0 && !availability.cost {
        Some("No usage data")
    } else if !cost_mode && mask == 0 {
        Some("No series selected")
    } else {
        None
    };
    if let Some(label) = empty_label {
        chart_children.push(
            text_block(label)
                .font_size(11.0)
                .foreground(ThemeRef::TertiaryText)
                .horizontal_alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Center)
                .with_key("empty-state")
                .into(),
        );
    }
    let chart = grid(chart_children)
        .rows([GridLength::Auto])
        .columns([GridLength::Auto])
        .height(HEIGHT)
        .on_pointer_exited(move || exit.call(None));

    let legend: Vec<Element> = Series::ALL
        .into_iter()
        .map(|series| {
            let available = availability.series & (1 << series as u8) != 0;
            let selected = selection.selected(series);
            let update = update_selection.clone();
            HyperlinkButton::new(format!("● {}", series.label()))
                .enabled(available)
                .font_size(11.0)
                .min_width(0.0)
                .min_height(0.0)
                .height(24.0)
                .padding(Thickness {
                    left: 3.0,
                    top: 0.0,
                    right: 3.0,
                    bottom: 0.0,
                })
                .foreground(series.brush(props.scheme))
                .opacity(if selected { 1.0 } else { 0.45 })
                .with_opacity_transition(transition)
                .on_click(move || {
                    update.call(move |current| {
                        let current = Selection {
                            cost: availability.selection(current).cost,
                            ..current
                        };
                        current.with_series(series, !current.selected(series))
                    })
                })
                .tooltip(if available {
                    format!(
                        "{}: {} tokens",
                        series.menu_label(),
                        format_token_count(series.value(&props.statistics.history))
                    )
                } else {
                    format!(
                        "No {} tokens in this period",
                        series.menu_label().to_lowercase()
                    )
                })
                .automation_name(if available {
                    format!(
                        "{} {} series",
                        if selected { "Hide" } else { "Show" },
                        series.menu_label()
                    )
                } else {
                    format!("{} series unavailable", series.menu_label())
                })
                .with_key(series.label())
                .into()
        })
        .collect();
    let modes: Vec<Element> = [("Tokens", false), ("Cost", true)]
        .into_iter()
        .enumerate()
        .map(|(index, (label, cost))| {
            let available = if cost {
                availability.cost
            } else {
                availability.series != 0
            };
            let selected = available && cost_mode == cost;
            let update = update_selection.clone();
            let highlight = border(Element::Empty)
                .background(ThemeRef::ControlFill)
                .corner_radius(4.0)
                .opacity(if selected { 1.0 } else { 0.0 })
                .with_opacity_transition(transition);
            let control = HyperlinkButton::new(label)
                .enabled(available)
                .font_size(10.0)
                .foreground(match (props.scheme, selected) {
                    (ColorScheme::Dark, true) => Color::rgb(245, 245, 245),
                    (ColorScheme::Dark, false) => Color::rgb(155, 155, 155),
                    (ColorScheme::Light, true) => Color::rgb(24, 24, 24),
                    (ColorScheme::Light, false) => Color::rgb(100, 100, 100),
                })
                .min_width(0.0)
                .min_height(0.0)
                .height(22.0)
                .padding(Thickness::uniform(0.0))
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .on_click(move || update.call(move |current| Selection { cost, ..current }))
                .tooltip(if !available && cost {
                    "No cost data for this period"
                } else if !available {
                    "No token data for this period"
                } else if cost {
                    "Daily cost in USD"
                } else {
                    "Daily token volume"
                })
                .automation_name(if available {
                    format!(
                        "Show {}{}",
                        label.to_lowercase(),
                        if selected { " (selected)" } else { "" }
                    )
                } else {
                    format!("{label} unavailable")
                });
            grid((highlight, control))
                .columns([GridLength::Star(1.0)])
                .rows([GridLength::Star(1.0)])
                .grid_column(index as i32)
                .with_key(label)
                .into()
        })
        .collect();
    let metric_selector = border(
        grid(modes)
            .columns([GridLength::Pixel(46.0), GridLength::Pixel(38.0)])
            .rows([GridLength::Star(1.0)]),
    )
    .background(match props.scheme {
        ColorScheme::Dark => Color {
            a: 35,
            r: 0,
            g: 0,
            b: 0,
        },
        ColorScheme::Light => Color {
            a: 14,
            r: 0,
            g: 0,
            b: 0,
        },
    })
    .padding(Thickness::uniform(2.0))
    .corner_radius(6.0)
    .height(26.0)
    .grid_column(1)
    .with_key("chart-metric");
    let total = if cost_mode {
        cost_label(&props.statistics.history)
    } else {
        format!(
            "{} tokens",
            format_token_count(visible_tokens(&props.statistics.history, mask))
        )
    };
    let summary = format!("{total} · {} requests", props.statistics.history.requests);
    let footer = grid((
        hstack(legend)
            .spacing(2.0)
            .tooltip(summary)
            .vertical_alignment(VerticalAlignment::Center),
        metric_selector,
    ))
    .columns([GridLength::Star(1.0), GridLength::Auto])
    .rows([GridLength::Auto]);
    vstack((chart, footer)).spacing(6.0).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn availability_for(usage: TokenUsage) -> Availability {
        let date = NaiveDate::from_ymd_opt(2026, 9, 10).unwrap();
        Availability::from_buckets(&[Bucket {
            first: date,
            last: date,
            usage,
        }])
    }

    #[test]
    fn cost_only_usage_disables_tokens_but_keeps_known_zero_cost() {
        let available = availability_for(TokenUsage {
            requests: 2,
            priced_requests: 2,
            ..Default::default()
        });
        assert_eq!(
            available,
            Availability {
                series: 0,
                cost: true
            }
        );
        assert_eq!(
            available.selection(Selection {
                mask: 7,
                cost: false
            }),
            Selection {
                mask: 0,
                cost: true
            }
        );
    }

    #[test]
    fn unpriced_tokens_enable_only_series_with_data() {
        let available = availability_for(TokenUsage {
            input_tokens: 100,
            output_tokens: 20,
            requests: 2,
            ..Default::default()
        });
        assert_eq!(
            available,
            Availability {
                series: 5,
                cost: false
            }
        );
        assert_eq!(
            available.selection(Selection {
                mask: 7,
                cost: true
            }),
            Selection {
                mask: 5,
                cost: false
            }
        );
        let cached = availability_for(TokenUsage {
            input_tokens: 100,
            cached_input_tokens: 100,
            ..Default::default()
        });
        assert_eq!(cached.series, 2);
    }

    #[test]
    fn metric_availability_does_not_erase_selection_or_reenable_hidden_series() {
        let requested = Selection {
            mask: 5,
            cost: false,
        };
        let available = Availability {
            series: 7,
            cost: true,
        };
        assert_eq!(
            Availability::default().selection(requested),
            Selection {
                mask: 0,
                cost: false
            }
        );
        assert_eq!(available.selection(requested), requested);
        assert_eq!(
            available.selection(Selection {
                mask: 0,
                cost: false
            }),
            Selection {
                mask: 0,
                cost: false
            }
        );
    }

    #[test]
    fn availability_uses_only_the_displayed_days() {
        let today = NaiveDate::from_ymd_opt(2026, 9, 10).unwrap();
        let usage = TokenUsage {
            input_tokens: 100,
            requests: 1,
            priced_requests: 1,
            ..Default::default()
        };
        let statistics = UsageStatistics {
            history_days: 30,
            history: usage.clone(),
            daily: vec![crate::usage::DailyTokenUsage {
                date: today - ChronoDuration::days(30),
                usage,
            }],
            ..Default::default()
        };
        assert_eq!(
            Availability::from_buckets(&buckets(&statistics, today)),
            Availability::default()
        );
    }

    #[test]
    #[cfg(windows)]
    fn hover_card_markup_is_valid_for_empty_partial_and_grouped_usage() {
        let first = NaiveDate::from_ymd_opt(2026, 9, 10).unwrap();
        for (days, requests, priced_requests) in [(0, 0, 0), (0, 2, 0), (1, 3, 1), (1, 3, 3)] {
            let bucket = Bucket {
                first,
                last: first + ChronoDuration::days(days),
                usage: TokenUsage {
                    input_tokens: 100,
                    cached_input_tokens: 80,
                    output_tokens: 5,
                    requests,
                    priced_requests,
                    estimated_cost_microusd: 123_456,
                    ..Default::default()
                },
            };
            for scheme in [ColorScheme::Light, ColorScheme::Dark] {
                let markup = bucket_tooltip_xaml(&bucket, scheme);
                let xml = windows::Data::Xml::Dom::XmlDocument::new().unwrap();
                xml.LoadXml(&windows_core::HSTRING::from(markup.as_str()))
                    .unwrap();
                assert!(markup.contains("Text=\"105\""));
                assert!(markup.contains("Text=\"20\""));
                if requests > 0 && priced_requests == 0 {
                    assert!(markup.contains("Unavailable"));
                }
            }
        }
    }

    #[test]
    fn native_checked_echoes_preserve_mode_and_selection() {
        let mut selection = Selection {
            mask: 5,
            cost: true,
        };
        for series in Series::ALL {
            selection = selection.with_series(series, false);
        }
        assert_eq!(
            selection,
            Selection {
                mask: 5,
                cost: true
            }
        );

        // Returning from cost restores the previous series; native check echoes are inert.
        selection.cost = false;
        for series in Series::ALL {
            selection = selection.with_series(series, selection.selected(series));
        }
        assert_eq!(selection.mask, 5);
        selection = selection.with_series(Series::Input, false);
        selection = selection.with_series(Series::Output, false);
        assert_eq!(selection.mask, 0);
        selection = selection.with_series(Series::Cache, true);
        assert_eq!(selection.mask, 2);

        // Choosing a token series while cost is shown starts with that series alone.
        selection.cost = true;
        selection = selection.with_series(Series::Output, true);
        assert_eq!(
            selection,
            Selection {
                mask: 4,
                cost: false
            }
        );
    }

    #[test]
    fn token_series_do_not_count_cache_twice() {
        let usage = TokenUsage {
            input_tokens: 100,
            cached_input_tokens: 80,
            output_tokens: 20,
            ..Default::default()
        };
        assert_eq!(visible_tokens(&usage, 7), 120);
        assert_eq!(visible_tokens(&usage, 1), 20);
        assert_eq!(visible_tokens(&usage, 2), 80);
        assert_eq!(visible_tokens(&usage, 4), 20);
        assert_eq!(visible_tokens(&usage, 0), 0);
    }

    #[test]
    fn missing_prices_are_distinct_from_zero_cost() {
        let mut usage = TokenUsage::default();
        assert_eq!(cost_label(&usage), "$0.00");
        usage.requests = 2;
        assert_eq!(cost_label(&usage), "Unavailable");
        usage.priced_requests = 1;
        assert!(cost_label(&usage).contains("partially priced"));
        usage.priced_requests = 2;
        assert!(!cost_label(&usage).contains("partially priced"));
    }

    #[test]
    fn daily_bars_merge_duplicates_and_exclude_outside_dates() {
        let today = NaiveDate::from_ymd_opt(2026, 9, 10).unwrap();
        let statistics = UsageStatistics {
            history_days: 30,
            daily: [-30, -29, 0, 0, 1]
                .into_iter()
                .map(|offset| crate::usage::DailyTokenUsage {
                    date: today + ChronoDuration::days(offset),
                    usage: TokenUsage {
                        input_tokens: 5,
                        requests: 1,
                        ..Default::default()
                    },
                })
                .collect(),
            ..Default::default()
        };
        let data = buckets(&statistics, today);
        assert_eq!(data.len(), 30);
        assert!(data.iter().all(|bucket| bucket.first == bucket.last));
        assert_eq!(data[0].usage.requests, 1);
        assert_eq!(data[29].usage.requests, 2);
        assert_eq!(data.iter().map(|b| b.usage.total_tokens()).sum::<u64>(), 15);
    }

    #[test]
    fn grouping_preserves_dates_gaps_and_usage() {
        let today = NaiveDate::from_ymd_opt(2026, 9, 10).unwrap();
        let mut statistics = UsageStatistics {
            history_days: 90,
            ..Default::default()
        };
        for offset in [0, 1, 89] {
            statistics.daily.push(crate::usage::DailyTokenUsage {
                date: today - ChronoDuration::days(offset),
                usage: TokenUsage {
                    input_tokens: 12,
                    output_tokens: 3,
                    requests: 1,
                    ..Default::default()
                },
            });
        }
        let data = buckets(&statistics, today);
        assert_eq!(data.len(), 45);
        assert_eq!(
            data.first().unwrap().first,
            today - ChronoDuration::days(89)
        );
        assert_eq!(data.last().unwrap().last, today);
        assert_eq!(data.iter().map(|b| b.usage.total_tokens()).sum::<u64>(), 45);
        assert_eq!(data.iter().map(|b| b.usage.requests).sum::<u64>(), 3);
        assert!(data.iter().any(|b| b.usage.requests == 0));
        assert!(bucket_tooltip(&data[0]).contains('–'));
    }
}
