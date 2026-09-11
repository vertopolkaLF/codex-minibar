use super::*;

const PAGE_SIZE: usize = 8;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct ModelData {
    days: BTreeMap<NaiveDate, BTreeMap<String, TokenUsage>>,
}

pub(super) fn group(
    rows: Vec<(String, NaiveDate, TokenUsage)>,
    bounds: &[Bucket],
    provider: ProviderKind,
) -> ModelData {
    let mut result = ModelData::default();
    for (model, date, usage) in rows {
        if let Some(bucket) = bounds.iter().find(|b| b.first <= date && date <= b.last) {
            let model = if provider == ProviderKind::Cursor {
                crate::cursor::normalize_cursor_model_name(&model)
            } else {
                model
            };
            result
                .days
                .entry(bucket.first)
                .or_default()
                .entry(model)
                .or_default()
                .add(&usage);
        }
    }
    result
}

fn value(usage: &TokenUsage, cost: bool) -> u64 {
    if cost {
        usage.estimated_cost_microusd
    } else {
        usage.total_tokens()
    }
}

// A model keeps its color across dates, refreshes, metric switches and new models.
fn color(model: &str, scheme: ColorScheme) -> Color {
    let mut hash = model.bytes().fold(0xcbf29ce484222325_u64, |hash, b| {
        (hash ^ u64::from(b)).wrapping_mul(0x100000001b3)
    });
    // Avalanche similar suffixes so model revisions do not get identical hues.
    hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d049bb133111eb);
    hash ^= hash >> 31;
    let hue = (hash >> 32) as f64 / u32::MAX as f64 * 6.0;
    let lightness = if scheme == ColorScheme::Dark {
        0.69
    } else {
        0.40
    };
    let chroma = (1.0_f64 - (2.0_f64 * lightness - 1.0).abs()) * 0.70;
    let x = chroma * (1.0 - (hue % 2.0 - 1.0).abs());
    let (r, g, b) = match hue as u8 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = lightness - chroma / 2.0;
    Color::rgb(
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    )
}

fn xml(text: &str) -> String {
    text.chars()
        .filter(|c| matches!(*c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}'))
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub(super) fn next_page(current: usize, pages: usize, wheel: i32) -> usize {
    let last = pages.saturating_sub(1);
    if wheel < 0 {
        current.saturating_add(1).min(last)
    } else {
        current.min(last).saturating_sub(1)
    }
}

impl ModelData {
    pub(super) fn availability(&self) -> Availability {
        Availability::from_buckets(
            &self
                .days
                .iter()
                .map(|(date, models)| {
                    let mut usage = TokenUsage::default();
                    for model in models.values() {
                        usage.add(model);
                    }
                    Bucket {
                        first: *date,
                        last: *date,
                        usage,
                    }
                })
                .collect::<Vec<_>>(),
        )
    }

    pub(super) fn has_data(&self) -> bool {
        self.days
            .values()
            .flat_map(|models| models.values())
            .any(|usage| {
                usage.requests > 0 || usage.total_tokens() > 0 || usage.priced_requests > 0
            })
    }

    pub(super) fn value(&self, date: NaiveDate, cost: bool) -> u64 {
        self.days
            .get(&date)
            .into_iter()
            .flat_map(|models| models.values())
            .fold(0_u64, |total, usage| {
                total.saturating_add(value(usage, cost))
            })
    }

    pub(super) fn pages(&self, date: NaiveDate) -> usize {
        self.days
            .get(&date)
            .map_or(1, |models| models.len().div_ceil(PAGE_SIZE).max(1))
    }

    fn sorted(&self, date: NaiveDate, cost: bool) -> Vec<(&str, &TokenUsage)> {
        let mut rows: Vec<_> = self
            .days
            .get(&date)
            .into_iter()
            .flat_map(|models| models.iter())
            .map(|(model, usage)| (model.as_str(), usage))
            .collect();
        rows.sort_by(|a, b| {
            value(b.1, cost)
                .cmp(&value(a.1, cost))
                .then_with(|| a.0.cmp(b.0))
        });
        rows
    }

    pub(super) fn description(&self, bucket: &Bucket, cost: bool) -> String {
        let mut result = format!(
            "{} – {} · {} by model",
            bucket.first,
            bucket.last,
            if cost { "Cost (USD)" } else { "Tokens" }
        );
        for (name, usage) in self.sorted(bucket.first, cost) {
            let amount = if cost {
                cost_label(usage)
            } else {
                format_token_count(usage.total_tokens())
            };
            result.push_str(&format!("\n{name}: {amount}"));
        }
        result
    }

    pub(super) fn tooltip(
        &self,
        bucket: &Bucket,
        cost: bool,
        provider: ProviderKind,
        scheme: ColorScheme,
        page: usize,
    ) -> String {
        let all = self.sorted(bucket.first, cost);
        let page = page.min(self.pages(bucket.first).saturating_sub(1));
        let descriptor = crate::provider_registry::descriptor(provider);
        let icon = crate::icons::geom(descriptor.icon);
        let icon_path = xml(icon.path);
        let canvas = icon.canvas;
        let icon_color = xaml_color(combined_usage_color(provider, scheme));
        let mut rows = String::new();
        for (name, usage) in all.iter().skip(page * PAGE_SIZE).take(PAGE_SIZE) {
            let brush = xaml_color(color(name, scheme));
            let name = xml(name);
            let amount = if cost {
                cost_label(usage)
            } else {
                format_token_count(usage.total_tokens())
            };
            let amount = xml(&amount);
            rows.push_str(&format!(r#"<Grid ColumnSpacing="7"><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition Width="Auto"/><ColumnDefinition Width="*"/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions><Border Width="6" Height="6" CornerRadius="3" Background="{brush}" VerticalAlignment="Center"/><Viewbox Grid.Column="1" Width="14" Height="14" VerticalAlignment="Center"><Canvas Width="{canvas}" Height="{canvas}"><Path Data="{icon_path}" Fill="{icon_color}"/></Canvas></Viewbox><TextBlock Grid.Column="2" Text="{name}" FontSize="11" TextWrapping="Wrap" MaxLines="2" TextTrimming="CharacterEllipsis" VerticalAlignment="Center"/><TextBlock Grid.Column="3" Text="{amount}" FontSize="11" FontWeight="SemiBold" VerticalAlignment="Center"/></Grid>"#));
        }
        if all.is_empty() {
            rows.push_str(r#"<TextBlock Text="No model data" FontSize="11"/>"#);
        }
        let date = if bucket.first == bucket.last {
            bucket.first.format("%a, %b %-d, %Y").to_string()
        } else {
            format!(
                "{} – {}",
                bucket.first.format("%b %-d"),
                bucket.last.format("%b %-d, %Y")
            )
        };
        let metric = if cost {
            "Cost (USD) by model"
        } else {
            "Tokens by model"
        };
        let paging = if all.len() > PAGE_SIZE {
            format!(
                r#"<TextBlock Text="{}–{} of {} models · Scroll for more" FontSize="10" Foreground="{{ThemeResource TextFillColorSecondaryBrush}}"/>"#,
                page * PAGE_SIZE + 1,
                ((page + 1) * PAGE_SIZE).min(all.len()),
                all.len()
            )
        } else {
            String::new()
        };
        format!(
            r#"<StackPanel xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Width="270" Spacing="9" IsHitTestVisible="False"><TextBlock Text="{date}" FontSize="12" FontWeight="SemiBold"/><TextBlock Text="{metric}" FontSize="10" Foreground="{{ThemeResource TextFillColorSecondaryBrush}}"/><StackPanel Spacing="7">{rows}</StackPanel>{paging}</StackPanel>"#
        )
    }
}

pub(super) fn bar(
    data: &ModelData,
    date: NaiveDate,
    cost: bool,
    scheme: ColorScheme,
    height: f64,
    transition: Duration,
) -> Element {
    let total = data.value(date, cost);
    let models: Vec<_> = data
        .days
        .get(&date)
        .into_iter()
        .flat_map(|models| models.iter())
        .collect();
    let rows = models
        .iter()
        .map(|(_, usage)| GridLength::Star(value(usage, cost) as f64 / total.max(1) as f64));
    let segments: Vec<Element> = models
        .iter()
        .enumerate()
        .map(|(index, (name, _))| {
            border(Element::Empty)
                .grid_row(index as i32)
                .background(color(name, scheme))
                .with_key(format!("model-{name}"))
                .into()
        })
        .collect();
    border(grid(segments).rows(rows).columns([GridLength::Star(1.0)]))
        .max_width(12.0)
        .height(height)
        .corner_radius(1.5)
        .background(Color::transparent())
        .with_layout_animation(LayoutAnimationConfig::linear(transition).animate_size(true))
        .with_key("model-bar")
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouped_models_preserve_dates_totals_and_do_not_double_count_cache() {
        let date = NaiveDate::from_ymd_opt(2026, 9, 10).unwrap();
        let usage = TokenUsage {
            input_tokens: 100,
            cached_input_tokens: 80,
            output_tokens: 10,
            estimated_cost_microusd: 50,
            priced_requests: 1,
            requests: 1,
            ..Default::default()
        };
        let bounds = vec![Bucket {
            first: date,
            last: date + ChronoDuration::days(1),
            usage: TokenUsage::default(),
        }];
        let data = group(
            vec![
                ("a".into(), date, usage.clone()),
                ("a".into(), date + ChronoDuration::days(1), usage.clone()),
                ("b".into(), date, usage.clone()),
                ("outside".into(), date - ChronoDuration::days(1), usage),
            ],
            &bounds,
            ProviderKind::Codex,
        );
        assert_eq!(data.value(date, false), 330);
        assert_eq!(data.value(date, true), 150);
        assert_eq!(data.days[&date].len(), 2);
        assert!(data.availability().cost);
        assert_eq!(data.pages(date), 1);
    }

    #[test]
    fn tooltip_pages_are_bounded() {
        assert_eq!(next_page(0, 3, 120), 0);
        assert_eq!(next_page(0, 3, -120), 1);
        assert_eq!(next_page(2, 3, -120), 2);
        assert_eq!(next_page(9, 1, -120), 0);
    }

    #[test]
    fn model_colors_separate_revisions_and_cost_only_data_disables_tokens() {
        for scheme in [ColorScheme::Light, ColorScheme::Dark] {
            assert_ne!(color("model-1", scheme), color("model-2", scheme));
        }
        let date = NaiveDate::from_ymd_opt(2026, 9, 10).unwrap();
        let data = group(
            vec![(
                "model".into(),
                date,
                TokenUsage {
                    requests: 1,
                    priced_requests: 1,
                    ..Default::default()
                },
            )],
            &[Bucket {
                first: date,
                last: date,
                usage: TokenUsage::default(),
            }],
            ProviderKind::OpenRouter,
        );
        assert!(data.has_data());
        assert_eq!(
            data.availability(),
            Availability {
                series: 0,
                cost: true
            }
        );
        assert_eq!(xml("x\u{fffe}&\u{0}"), "x&amp;");
    }

    #[test]
    #[cfg(windows)]
    fn model_tooltip_escapes_names_and_paginates_with_existing_provider_icons() {
        let date = NaiveDate::from_ymd_opt(2026, 9, 10).unwrap();
        let bucket = Bucket {
            first: date,
            last: date,
            usage: TokenUsage::default(),
        };
        let data = group(
            (0..10)
                .map(|i| {
                    (
                        format!("model-{i}<\"&"),
                        date,
                        TokenUsage {
                            input_tokens: i + 1,
                            ..Default::default()
                        },
                    )
                })
                .collect(),
            &[bucket.clone()],
            ProviderKind::OpenRouter,
        );
        for provider in crate::provider_registry::PROVIDERS {
            for scheme in [ColorScheme::Light, ColorScheme::Dark] {
                for page in [0, 1] {
                    let markup = data.tooltip(&bucket, false, provider.kind, scheme, page);
                    let xml = windows::Data::Xml::Dom::XmlDocument::new().unwrap();
                    xml.LoadXml(&windows_core::HSTRING::from(markup.as_str()))
                        .unwrap();
                    assert_eq!(
                        markup.matches("<Path ").count(),
                        if page == 0 { 8 } else { 2 }
                    );
                    assert!(markup.contains("&lt;&quot;&amp;"));
                }
            }
        }
    }
}
