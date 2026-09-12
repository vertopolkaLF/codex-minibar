use super::*;
use crate::usage::{TokenUsage, UsageStatistics};
use chrono::NaiveDate;
use std::cell::RefCell;
use std::collections::BTreeMap;

const HEIGHT: f64 = 56.0;
const MAX_BARS: usize = 60;
const USAGE_CARD_PAD: f64 = 12.0;
const USAGE_CARD_SECTION_GAP: f64 = 12.0;
const USAGE_CARD_FOOTER_GAP: f64 = 6.0;

mod models;

#[derive(Clone, PartialEq)]
struct ChartProps {
    provider: ProviderKind,
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
            provider,
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
    let (by_model, set_by_model) = cx.use_state(false);
    let (model_page, set_model_page) = cx.use_reducer(0_usize);
    // Only a changed usage snapshot/date refetches; hover and toggles stay in memory.
    let model_resource = cx.use_resource(
        |(provider, statistics, today): (ProviderKind, UsageStatistics, NaiveDate)| {
            let bounds = buckets(&statistics, today);
            crate::store::with_store(|store| {
                store.load_model_daily(provider, bounds[0].first, today)
            })
            .map(|rows| Arc::new(models::group(rows, &bounds, provider)))
            .map_err(|error| format!("Could not load model data: {error:#}"))
        },
        (props.provider, props.statistics.clone(), props.today),
    );
    let data = cx.use_memo((props.statistics.clone(), props.today), || {
        buckets(&props.statistics, props.today)
    });
    let model_data = model_resource.data();
    let availability = if by_model {
        model_data
            .map(|models| models.availability())
            .unwrap_or_default()
    } else {
        Availability::from_buckets(&data)
    };
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
            if by_model {
                model_data.map_or(0, |models| models.value(b.first, cost_mode))
            } else if cost_mode {
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
            let value = if by_model {
                model_data.map_or(0, |models| models.value(bucket.first, cost_mode))
            } else if cost_mode {
                bucket.usage.estimated_cost_microusd
            } else {
                visible_tokens(&bucket.usage, mask)
            };
            let date = bucket.first;
            let enter = set_hovered.clone();
            let tooltip = if by_model {
                model_data.map_or_else(String::new, |models| models.description(bucket, cost_mode))
            } else {
                bucket_tooltip(bucket)
            };
            let height = if maximum == 0 {
                2.0
            } else {
                (HEIGHT * value as f64 / maximum as f64).max(2.0)
            };
            let bar: Element = if by_model && value > 0 {
                models::bar(
                    model_data.expect("model value requires data"),
                    date,
                    cost_mode,
                    props.scheme,
                    bar_width,
                    height,
                    transition,
                )
            } else if cost_mode || value == 0 {
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
                .on_pointer_entered({
                    let page = set_model_page.clone();
                    move |_: PointerEventInfo| {
                        page.call(|_| 0);
                        enter.call(Some(date));
                    }
                })
                .with_key("hover-target");
            if by_model && model_data.is_some_and(|models| models.pages(date) > 1) {
                let pages = model_data.map_or(1, |models| models.pages(date));
                let page = set_model_page.clone();
                hit_target = hit_target.on_pointer_wheel(move |event: PointerEventInfo| {
                    if !event.wheel_is_horizontal && event.wheel_delta != 0 {
                        page.call(move |current| {
                            models::next_page(current, pages, event.wheel_delta)
                        });
                    }
                });
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
    let empty_label = if by_model && model_resource.error().is_some() {
        Some("Model data unavailable")
    } else if by_model && model_resource.is_loading() {
        Some("Loading models…")
    } else if by_model && availability.series == 0 && !availability.cost {
        Some("No model data")
    } else if availability.series == 0 && !availability.cost {
        Some("No usage data")
    } else if !by_model && !cost_mode && mask == 0 {
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
        .on_pointer_exited({
            let provider = props.provider;
            move || dismiss_activity_hover(provider, &exit)
        });

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
    let model_available = model_data.is_some_and(|models| models.has_data());
    let model_hint = if let Some(error) = model_resource.error() {
        error.to_owned()
    } else if model_resource.is_loading() {
        "Loading model breakdown".into()
    } else if !model_available {
        "No model data for this period".into()
    } else {
        "Group tokens or cost by model".into()
    };
    let model_selector = border(
        grid((
            border(Element::Empty)
                .background(ThemeRef::ControlFill)
                .corner_radius(4.0)
                .opacity(if by_model { 1.0 } else { 0.0 })
                .with_opacity_transition(transition),
            HyperlinkButton::new("Model")
                .enabled(model_available || by_model)
                .font_size(10.0)
                .min_width(0.0)
                .min_height(0.0)
                .height(22.0)
                .padding(Thickness::uniform(0.0))
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .foreground(if by_model {
                    ThemeRef::PrimaryText
                } else {
                    ThemeRef::SecondaryText
                })
                .on_click(move || set_by_model.call(!by_model))
                .tooltip(model_hint)
                .automation_name(if by_model {
                    "Ungroup models (selected)"
                } else {
                    "Group by model"
                }),
        ))
        .columns([GridLength::Star(1.0)])
        .rows([GridLength::Star(1.0)])
        .with_key("Model"),
    )
    .width(48.0)
    .height(26.0)
    .padding(Thickness::uniform(2.0))
    .corner_radius(6.0)
    .background(ThemeRef::SubtleFill)
    .with_key("model-selector");
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
    let legend: Element = if by_model {
        // Keep the footer height stable without invisible focusable controls.
        border(Element::Empty)
            .height(24.0)
            .with_key("model-legend-space")
            .into()
    } else {
        hstack(legend)
            .spacing(2.0)
            .tooltip(summary)
            .vertical_alignment(VerticalAlignment::Center)
            .with_key("token-legend")
            .into()
    };
    let selectors = hstack((model_selector, metric_selector))
        .spacing(8.0)
        .grid_column(1);
    let footer = grid((legend, selectors))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .rows([GridLength::Auto]);
    let tip = active_hover.and_then(|date| {
        data.iter().find(|bucket| bucket.first == date).and_then(|bucket| {
            if by_model {
                model_data.map(|models| {
                    activity_tip_from_models(
                        bucket,
                        models,
                        cost_mode,
                        props.provider,
                        props.scheme,
                        model_page,
                    )
                })
            } else {
                Some(activity_tip_from_bucket(bucket, props.provider, props.scheme))
            }
        })
    });
    publish_activity_page_tip(tip);

    let metrics = usage_card_metrics(props.provider, &props.statistics);
    border(
        vstack((
            metrics,
            vstack((chart, footer)).spacing(USAGE_CARD_FOOTER_GAP),
        ))
        .spacing(USAGE_CARD_SECTION_GAP),
    )
    .corner_radius(f64::from(popup::CARD_CORNER_RADIUS_DIP))
    .padding(Thickness::uniform(USAGE_CARD_PAD))
    .background(ThemeRef::CardBackground)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .with_key(format!("activity-card-{}", props.provider.id()))
    .into()
}

const TOOLTIP_PAD_X: f64 = 14.0;
const TOOLTIP_PAD_Y: f64 = 8.0;
const TOOLTIP_CURSOR_GAP: f64 = 10.0;
const TOOLTIP_EDGE_INSET: f64 = 4.0;
const TOOLTIP_ROW_GAP: f64 = 8.0;
const TOOLTIP_VALUE_GAP: f64 = 24.0;
const TOOLTIP_CHAR_CAPTION: f64 = 6.5;
const TOOLTIP_CHAR_TITLE: f64 = 8.0;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ActivityTipData {
    provider: ProviderKind,
    title: String,
    kind: ActivityTipKind,
}

#[derive(Clone, Debug, PartialEq)]
enum ActivityTipKind {
    Usage {
        total: String,
        cost: String,
        requests: String,
        series: Vec<(u8, String)>,
    },
    Model {
        metric: String,
        rows: Vec<(String, String, Color)>,
        footer: Option<String>,
    },
}

struct ActivityTooltipTrack {
    host: Option<windows_core::IInspectable>,
    cursor: Option<(f64, f64)>,
    tip_width: f64,
    tip_height: f64,
}

thread_local! {
    static ACTIVITY_TOOLTIP_TRACK: RefCell<ActivityTooltipTrack> = RefCell::new(ActivityTooltipTrack {
        host: None,
        cursor: None,
        tip_width: 0.0,
        tip_height: 0.0,
    });
    static ACTIVITY_TOOLTIP_MOUNTED: Callback<Option<windows_core::IInspectable>> =
        Callback::new(|native: Option<windows_core::IInspectable>| {
            if let Some(host) = native.clone() {
                let _ = windows_reactor::set_hit_test_visible(host, false);
            }
            ACTIVITY_TOOLTIP_TRACK.with(|track| {
                track.borrow_mut().host = native;
            });
            apply_activity_tooltip_offset();
        });
    static ACTIVITY_PAGE_TIP: RefCell<Option<SetState<Option<ActivityTipData>>>> =
        RefCell::new(None);
    static ACTIVITY_PAGE_TIP_LAST: RefCell<Option<ActivityTipData>> = RefCell::new(None);
}

pub(super) fn install_activity_page_tip(set_tip: SetState<Option<ActivityTipData>>) {
    ACTIVITY_PAGE_TIP.with(|cell| {
        *cell.borrow_mut() = Some(set_tip);
    });
}

fn publish_activity_page_tip(data: Option<ActivityTipData>) {
    ACTIVITY_PAGE_TIP_LAST.with(|last| {
        if *last.borrow() == data {
            return;
        }
        *last.borrow_mut() = data.clone();
        ACTIVITY_PAGE_TIP.with(|cell| {
            if let Some(set_tip) = cell.borrow().as_ref() {
                set_tip.call(data);
            }
        });
    });
}

pub(super) fn remember_activity_page_cursor(x: f64, y: f64) {
    ACTIVITY_TOOLTIP_TRACK.with(|track| {
        track.borrow_mut().cursor = Some((x, y));
    });
    apply_activity_tooltip_offset();
}

pub(super) fn dismiss_activity_page_tip() {
    ACTIVITY_TOOLTIP_TRACK.with(|track| {
        let mut track = track.borrow_mut();
        track.cursor = None;
        track.host = None;
    });
    publish_activity_page_tip(None);
}

fn dismiss_activity_hover(_provider: ProviderKind, set_hover: &SetState<Option<NaiveDate>>) {
    dismiss_activity_page_tip();
    set_hover.call(None);
}

fn apply_activity_tooltip_offset() {
    ACTIVITY_TOOLTIP_TRACK.with(|track| {
        let track = track.borrow();
        let Some(host) = track.host.clone() else {
            return;
        };
        let Some((cursor_x, cursor_y)) = track.cursor else {
            return;
        };
        let page_width = activity_page_width();
        let visible = (popup::body_viewport_height_dip() - 32.0).max(80.0);
        let left = activity_tooltip_offset_x(cursor_x, track.tip_width, page_width);
        let top = activity_tooltip_offset_y(cursor_y, track.tip_height, visible);
        let _ = windows_reactor::set_translation_xy(host, left as f32, top as f32);
    });
}

fn activity_page_width() -> f64 {
    f64::from(popup::POPUP_WIDTH) - 2.0 - 32.0
}

fn usage_card_metrics(provider: ProviderKind, statistics: &UsageStatistics) -> Element {
    if is_cost_provider(provider) {
        return grid((
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
        .horizontal_alignment(HorizontalAlignment::Stretch)
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
    grid((
        usage_tokens_and_cost_metric("Today", today, today_value),
        usage_tokens_and_cost_metric(&format!("Last {period} days"), total, history_value)
            .grid_column(1),
    ))
    .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
    .rows([GridLength::Auto])
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn activity_tooltip_offset_x(cursor_x: f64, tip_width: f64, area_width: f64) -> f64 {
    let min_x = TOOLTIP_EDGE_INSET;
    let max_x = (area_width - TOOLTIP_EDGE_INSET - tip_width).max(min_x);
    let prefer_right = cursor_x + TOOLTIP_CURSOR_GAP + tip_width <= area_width - TOOLTIP_EDGE_INSET;
    let raw = if prefer_right {
        cursor_x + TOOLTIP_CURSOR_GAP
    } else {
        cursor_x - TOOLTIP_CURSOR_GAP - tip_width
    };
    raw.clamp(min_x, max_x)
}

fn activity_tooltip_offset_y(cursor_y: f64, tip_height: f64, area_height: f64) -> f64 {
    let min_y = TOOLTIP_EDGE_INSET;
    let max_y = (area_height - TOOLTIP_EDGE_INSET - tip_height).max(min_y);
    let prefer_below = cursor_y + TOOLTIP_CURSOR_GAP + tip_height <= area_height - TOOLTIP_EDGE_INSET;
    let raw = if prefer_below {
        cursor_y + TOOLTIP_CURSOR_GAP
    } else {
        cursor_y - TOOLTIP_CURSOR_GAP - tip_height
    };
    raw.clamp(min_y, max_y)
}

fn remember_activity_tip_size(tip_width: f64, tip_height: f64) {
    ACTIVITY_TOOLTIP_TRACK.with(|track| {
        let mut track = track.borrow_mut();
        track.tip_width = tip_width;
        track.tip_height = tip_height;
    });
}

fn activity_tooltip_row(label: impl Into<Element>, amount: impl Into<String>) -> Element {
    grid((
        label.into().vertical_alignment(VerticalAlignment::Center),
        caption(amount.into())
            .font_weight(600)
            .foreground(ThemeRef::Accent)
            .horizontal_alignment(HorizontalAlignment::Right)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(1),
    ))
    .columns([GridLength::Star(1.0), GridLength::Auto])
    .column_spacing(TOOLTIP_VALUE_GAP)
    .rows([GridLength::Auto])
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn activity_tooltip_shell(
    title: String,
    rows: Vec<Element>,
    tip_width: f64,
    tip_height: f64,
    body_key: impl Into<String>,
) -> Element {
    remember_activity_tip_size(tip_width, tip_height);
    let body_width = (tip_width - TOOLTIP_PAD_X * 2.0).max(1.0);
    let mut host = vstack((border(
        vstack((
            body_strong(title).with_key("activity-tip-title"),
            vstack(rows)
                .spacing(0.0)
                .width(body_width)
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .with_key(body_key),
        ))
        .spacing(10.0)
        .with_key("activity-tip-body"),
    )
    .padding(Thickness {
        left: TOOLTIP_PAD_X,
        top: TOOLTIP_PAD_Y,
        right: TOOLTIP_PAD_X,
        bottom: TOOLTIP_PAD_Y,
    })
    .corner_radius(6.0)
    .background(ThemeRef::SolidBackground)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .horizontal_alignment(HorizontalAlignment::Left),));
    host.mounted = Some(ACTIVITY_TOOLTIP_MOUNTED.with(Callback::clone));
    apply_activity_tooltip_offset();
    host.horizontal_alignment(HorizontalAlignment::Left)
        .vertical_alignment(VerticalAlignment::Top)
        .relative_align_left()
        .relative_align_top()
        .with_key("activity-tip-host")
        .into()
}

fn bucket_title(bucket: &Bucket) -> String {
    if bucket.first == bucket.last {
        bucket.first.format("%a, %b %-d, %Y").to_string()
    } else {
        format!(
            "{} – {}",
            bucket.first.format("%b %-d, %Y"),
            bucket.last.format("%b %-d, %Y")
        )
    }
}

fn activity_tip_from_bucket(
    bucket: &Bucket,
    provider: ProviderKind,
    _scheme: ColorScheme,
) -> ActivityTipData {
    let usage = &bucket.usage;
    let cost = if usage.priced_requests > 0 {
        format_spend(usage.estimated_cost_microusd)
    } else if usage.requests == 0 {
        "$0.00".into()
    } else {
        "Unavailable".into()
    };
    let requests = if usage.priced_requests > 0 && usage.priced_requests < usage.requests {
        format!(
            "{} requests · {} priced",
            usage.requests, usage.priced_requests
        )
    } else {
        format!("{} requests", usage.requests)
    };
    ActivityTipData {
        provider,
        title: bucket_title(bucket),
        kind: ActivityTipKind::Usage {
            total: format_token_count(usage.total_tokens()),
            cost,
            requests,
            series: Series::ALL
                .into_iter()
                .map(|series| (series as u8, format_token_count(series.value(usage))))
                .collect(),
        },
    }
}

fn activity_tip_from_models(
    bucket: &Bucket,
    models: &models::ModelData,
    cost: bool,
    provider: ProviderKind,
    scheme: ColorScheme,
    page: usize,
) -> ActivityTipData {
    let (entries, total) = models.page_rows(bucket.first, cost, scheme, page);
    let page = page.min(models.pages(bucket.first).saturating_sub(1));
    let footer = if entries.is_empty() {
        Some("No model data".into())
    } else if total > models::PAGE_SIZE {
        Some(format!(
            "{}–{} of {total} models · Scroll for more",
            page * models::PAGE_SIZE + 1,
            ((page + 1) * models::PAGE_SIZE).min(total)
        ))
    } else {
        None
    };
    ActivityTipData {
        provider,
        title: bucket_title(bucket),
        kind: ActivityTipKind::Model {
            metric: if cost {
                "Cost (USD) by model".into()
            } else {
                "Tokens by model".into()
            },
            rows: entries,
            footer,
        },
    }
}

pub(super) fn activity_page_tooltip(data: &ActivityTipData, scheme: ColorScheme) -> Element {
    match &data.kind {
        ActivityTipKind::Usage {
            total,
            cost,
            requests,
            series,
        } => activity_usage_tooltip(
            data.provider,
            data.title.clone(),
            total,
            cost,
            requests,
            series,
            scheme,
        ),
        ActivityTipKind::Model {
            metric,
            rows,
            footer,
        } => activity_models_tooltip(
            data.provider,
            data.title.clone(),
            metric,
            rows,
            footer.as_deref(),
            scheme,
        ),
    }
}

fn activity_usage_tooltip(
    _provider: ProviderKind,
    title: String,
    total: &str,
    cost: &str,
    requests: &str,
    series: &[(u8, String)],
    scheme: ColorScheme,
) -> Element {
    let mut name_width = 16.0 * TOOLTIP_CHAR_CAPTION;
    let mut amount_width = total.chars().count().max(cost.chars().count()) as f64 * TOOLTIP_CHAR_CAPTION;
    let mut rows: Vec<Element> = vec![
        grid((
            vstack((
                caption("Tokens").foreground(ThemeRef::SecondaryText),
                text_block(total.clone())
                    .font_size(20.0)
                    .font_weight(600)
                    .with_key("activity-tip-tokens"),
            ))
            .spacing(2.0),
            vstack((
                caption("Cost")
                    .foreground(ThemeRef::SecondaryText)
                    .horizontal_alignment(HorizontalAlignment::Right),
                text_block(cost.clone())
                    .font_size(16.0)
                    .font_weight(600)
                    .foreground(ThemeRef::Accent)
                    .horizontal_alignment(HorizontalAlignment::Right)
                    .with_key("activity-tip-cost"),
            ))
            .spacing(2.0)
            .horizontal_alignment(HorizontalAlignment::Right)
            .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .column_spacing(12.0)
        .rows([GridLength::Auto])
        .margin(Thickness {
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: 6.0,
        })
        .with_key("activity-tip-hero")
        .into(),
        border(Element::Empty)
            .height(1.0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .background(ThemeRef::DividerStroke)
            .margin(Thickness {
                left: 0.0,
                top: 0.0,
                right: 0.0,
                bottom: 6.0,
            })
            .with_key("activity-tip-rule")
            .into(),
    ];

    for (index, amount) in series {
        let Some(series) = Series::ALL.get(*index as usize).copied() else {
            continue;
        };
        name_width = name_width.max(series.menu_label().chars().count() as f64 * TOOLTIP_CHAR_CAPTION);
        amount_width = amount_width.max(amount.chars().count() as f64 * TOOLTIP_CHAR_CAPTION);
        let label = hstack((
            border(Element::Empty)
                .width(6.0)
                .height(6.0)
                .corner_radius(3.0)
                .background(series.brush(scheme))
                .vertical_alignment(VerticalAlignment::Center)
                .with_key(format!("activity-tip-dot-{}", series.label())),
            caption(series.menu_label())
                .foreground(ThemeRef::SecondaryText)
                .vertical_alignment(VerticalAlignment::Center),
        ))
        .spacing(TOOLTIP_ROW_GAP)
        .with_key(format!("activity-tip-label-{}", series.label()));
        rows.push(
            border(activity_tooltip_row(label, amount.clone()).with_key(format!(
                "activity-tip-row-{}",
                series.label()
            )))
            .padding(Thickness {
                left: 0.0,
                top: 0.0,
                right: 0.0,
                bottom: 6.0,
            })
            .with_key(format!("activity-tip-slot-{}", series.label()))
            .into(),
        );
    }
    rows.push(
        caption(requests.to_owned())
            .foreground(ThemeRef::SecondaryText)
            .with_key("activity-tip-requests")
            .into(),
    );

    let inner_width = (6.0 + TOOLTIP_ROW_GAP + name_width + TOOLTIP_VALUE_GAP + amount_width)
        .max(5.0 * TOOLTIP_CHAR_CAPTION + 12.0 + amount_width);
    let tip_width =
        (title.chars().count() as f64 * TOOLTIP_CHAR_TITLE).max(inner_width) + TOOLTIP_PAD_X * 2.0;
    let tip_height = TOOLTIP_PAD_Y * 2.0 + 20.0 + 10.0 + 44.0 + 7.0 + 3.0 * 26.0 + 16.0;
    activity_tooltip_shell(
        title,
        rows,
        tip_width,
        tip_height,
        format!("activity-tip-rows-{}", scheme as i32),
    )
}

fn activity_models_tooltip(
    provider: ProviderKind,
    title: String,
    metric: &str,
    entries: &[(String, String, Color)],
    footer: Option<&str>,
    scheme: ColorScheme,
) -> Element {
    let mut name_width = 14.0 * TOOLTIP_CHAR_CAPTION;
    let mut amount_width = 6.0 * TOOLTIP_CHAR_CAPTION;
    let mut rows: Vec<Element> = vec![
        caption(metric.to_owned())
            .foreground(ThemeRef::SecondaryText)
            .margin(Thickness {
                left: 0.0,
                top: 0.0,
                right: 0.0,
                bottom: 6.0,
            })
            .with_key("activity-tip-metric")
            .into(),
    ];
    for slot in 0..models::PAGE_SIZE {
        let hidden = slot >= entries.len();
        let (name, amount, color) = entries.get(slot).cloned().unwrap_or_else(|| {
            (String::new(), String::new(), Color::transparent())
        });
        if !hidden {
            name_width = name_width.max(name.chars().count().min(28) as f64 * TOOLTIP_CHAR_CAPTION);
            amount_width = amount_width.max(amount.chars().count() as f64 * TOOLTIP_CHAR_CAPTION);
        }
        let label = hstack((
            border(Element::Empty)
                .width(6.0)
                .height(6.0)
                .corner_radius(3.0)
                .background(color)
                .vertical_alignment(VerticalAlignment::Center)
                .with_key(format!("activity-tip-model-dot-{slot}")),
            caption(name)
                .foreground(ThemeRef::SecondaryText)
                .vertical_alignment(VerticalAlignment::Center),
        ))
        .spacing(TOOLTIP_ROW_GAP)
        .with_key(format!("activity-tip-model-label-{slot}"));
        let mut slot_el = border(
            activity_tooltip_row(label, amount).with_key(format!("activity-tip-model-row-{slot}")),
        )
        .opacity(if hidden { 0.0 } else { 1.0 })
        .with_key(format!("activity-tip-model-slot-{slot}"));
        if hidden {
            slot_el = slot_el.height(0.0);
        } else {
            slot_el = slot_el.padding(Thickness {
                left: 0.0,
                top: 0.0,
                right: 0.0,
                bottom: 6.0,
            });
        }
        rows.push(slot_el.into());
    }
    if let Some(footer) = footer {
        rows.push(
            caption(footer.to_owned())
                .foreground(ThemeRef::SecondaryText)
                .with_key("activity-tip-model-footer")
                .into(),
        );
    }
    let visible = if entries.is_empty() { 1 } else { entries.len() };
    let paging = usize::from(footer.is_some() && !entries.is_empty());
    let inner_width = 6.0 + TOOLTIP_ROW_GAP + name_width + TOOLTIP_VALUE_GAP + amount_width;
    let tip_width =
        (title.chars().count() as f64 * TOOLTIP_CHAR_TITLE).max(inner_width) + TOOLTIP_PAD_X * 2.0;
    let tip_height = TOOLTIP_PAD_Y * 2.0
        + 20.0
        + 10.0
        + 22.0
        + visible as f64 * 26.0
        + paging as f64 * 16.0;
    activity_tooltip_shell(
        title,
        rows,
        tip_width,
        tip_height,
        format!(
            "activity-tip-models-{}-{}",
            provider.id(),
            scheme as i32
        ),
    )
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
