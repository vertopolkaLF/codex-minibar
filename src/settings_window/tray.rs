//! Tray: icon widgets, their indicators and a live 32px preview of each.

use std::{collections::HashMap, sync::Arc};

use gpui::{
    AnyElement, Context, FontWeight, ImageFormat, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, div, img, px,
};

use super::kit::{self, Button, Kit, Row, eid};
use super::window::SettingsWindow;
use crate::popup_window::ui::theme::rgb8;
use crate::settings::{
    LimitValue, ProviderId, ProviderKind, TimeFormat, TrayColorMode, TrayFixedColor, TrayIndicator,
    TrayPresentation, TrayWidget, TrayWidgetKind,
};

const PRESENTATION_LABELS: [&str; 5] = [
    "Numbers",
    "Progress bars",
    "Rings",
    "Reset time",
    "Countdown",
];
const COLOR_LABELS: [&str; 5] = ["Status", "Fixed", "Provider", "App accent", "Monochrome"];
const MAX_INDICATORS: usize = 3;

/// Fixed-color presets offered next to the hex field.
const SWATCHES: [(u8, u8, u8); 12] = [
    (0xFF, 0xFF, 0xFF),
    (0x1F, 0x1F, 0x1F),
    (0x00, 0x78, 0xD4),
    (0x88, 0x6C, 0xE4),
    (0xE3, 0x00, 0x8C),
    (0xE8, 0x11, 0x23),
    (0xF7, 0x63, 0x0C),
    (0xFF, 0xB9, 0x00),
    (0x10, 0x7C, 0x10),
    (0x00, 0xB2, 0x94),
    (0x00, 0x99, 0xBC),
    (0x7A, 0x75, 0x74),
];

#[derive(Clone, PartialEq)]
pub(crate) struct PreviewKey {
    widget: TrayWidget,
    accent: [u8; 3],
    light: bool,
    time_format: TimeFormat,
    minute: u64,
}

/// Applies a dropdown choice to an indicator, given the provider and metric
/// options the dropdown listed.
type IndicatorEdit = fn(&mut TrayIndicator, usize, &[ProviderId], &[(String, String)]);

pub(crate) type PreviewCache = HashMap<String, (PreviewKey, Arc<gpui::Image>)>;

/// Instances an indicator can read, in display order: every configured
/// instance with tray metrics, so a disabled one stays selectable.
fn provider_options() -> Vec<ProviderId> {
    crate::instances::published_providers()
        .into_iter()
        .filter(|provider| {
            !crate::provider_registry::descriptor(provider.kind())
                .default_tray_metrics
                .is_empty()
        })
        .collect()
}

fn provider_label(provider: ProviderId) -> String {
    if crate::instances::published_enabled_providers().contains(&provider) {
        provider.qualified_name()
    } else {
        format!("{} (off)", provider.qualified_name())
    }
}

fn color_mode_label(mode: TrayColorMode) -> &'static str {
    COLOR_LABELS[color_mode_index(mode)]
}

fn color_mode_index(mode: TrayColorMode) -> usize {
    match mode {
        TrayColorMode::Status => 0,
        TrayColorMode::Fixed => 1,
        TrayColorMode::Provider => 2,
        TrayColorMode::Accent => 3,
        TrayColorMode::Monochrome => 4,
    }
}

fn color_mode_from_index(index: usize) -> TrayColorMode {
    match index {
        1 => TrayColorMode::Fixed,
        2 => TrayColorMode::Provider,
        3 => TrayColorMode::Accent,
        4 => TrayColorMode::Monochrome,
        _ => TrayColorMode::Status,
    }
}

fn presentation_index(presentation: TrayPresentation) -> usize {
    match presentation.canonical_percentage() {
        TrayPresentation::StackedBars => 1,
        TrayPresentation::NestedRings => 2,
        TrayPresentation::ResetTime => 3,
        TrayPresentation::ResetCountdown => 4,
        _ => 0,
    }
}

fn presentation_from_index(index: usize) -> TrayPresentation {
    match index {
        1 => TrayPresentation::StackedBars,
        2 => TrayPresentation::NestedRings,
        3 => TrayPresentation::ResetTime,
        4 => TrayPresentation::ResetCountdown,
        _ => TrayPresentation::StackedNumbers,
    }
}

fn indicator_summary(indicator: &TrayIndicator) -> String {
    let Some(provider) = indicator.provider() else {
        return format!("Unsupported {}", indicator.provider_id);
    };
    let metric = crate::provider_registry::settings_brick_label(
        provider.kind(),
        &indicator.metric_id,
        &super::cached_discovered_popup_bricks(),
    );
    let value = match indicator.limit_value {
        LimitValue::Used => "Used",
        LimitValue::Remaining => "Remaining",
    };
    format!(
        "{} · {metric} · {value} · {}",
        provider.qualified_name(),
        color_mode_label(indicator.color_mode)
    )
}

fn widget_summary(widget: &TrayWidget) -> String {
    if widget.kind == TrayWidgetKind::AppIcon {
        return "App icon".into();
    }
    let labels = widget
        .indicators
        .iter()
        .map(indicator_summary)
        .collect::<Vec<_>>();
    if labels.is_empty() {
        "Empty widget".into()
    } else {
        labels.join("  ·  ")
    }
}

fn default_indicator(provider: ProviderId) -> TrayIndicator {
    let metric = crate::provider_registry::descriptor(provider.kind())
        .default_tray_metrics
        .first()
        .copied()
        .unwrap_or("unknown");
    TrayIndicator::new(provider, metric)
}

/// Sample quotas for previews, one per configured instance so indicators of
/// every instance render.
fn preview_limits() -> crate::limits::ProviderLimits {
    let window = |used_percent| crate::limits::LimitWindow {
        used_percent: Some(used_percent),
        resets_at: None,
        duration_minutes: Some(300),
    };
    let samples = [
        (
            ProviderKind::Codex,
            crate::limits::RateLimits {
                primary: window(38),
                secondary: window(70),
                ..Default::default()
            },
        ),
        (
            ProviderKind::Claude,
            crate::limits::RateLimits {
                primary: window(55),
                secondary: window(12),
                ..Default::default()
            },
        ),
        (
            ProviderKind::Cursor,
            crate::limits::RateLimits {
                secondary: window(18),
                additional_limits: vec![
                    crate::limits::AdditionalLimit {
                        id: "cursor-api".into(),
                        title: "Other Models".into(),
                        window: window(47),
                    },
                    crate::limits::AdditionalLimit {
                        id: "cursor-grok-bot".into(),
                        title: "Grok Bot".into(),
                        window: window(1),
                    },
                ],
                ..Default::default()
            },
        ),
    ];
    crate::limits::ProviderLimits::from_entries(
        crate::instances::published_providers()
            .into_iter()
            .chain(samples.iter().map(|(kind, _)| ProviderId::primary(*kind)))
            .filter_map(|provider| {
                samples
                    .iter()
                    .find(|(kind, _)| *kind == provider.kind())
                    .map(|(_, limits)| (provider, limits.clone()))
            }),
    )
}

fn encode_png(pixels: &[u8]) -> Option<Vec<u8>> {
    let side = ((pixels.len() / 4) as f64).sqrt() as u32;
    if side == 0 || (side * side * 4) as usize != pixels.len() {
        return None;
    }
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, side, side);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(pixels).ok()?;
    }
    Some(out)
}

impl SettingsWindow {
    /// The widget rendered exactly as the tray draws it, with sample quotas.
    fn tray_preview(&mut self, k: &Kit, widget: &TrayWidget, size: f32) -> AnyElement {
        let key = PreviewKey {
            widget: widget.clone(),
            accent: crate::theme::current_accent_rgb(),
            light: crate::tray::system_uses_light_theme(),
            time_format: TimeFormat::current(),
            minute: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_secs() / 60),
        };
        let image = match self.tray_previews.get(&widget.id) {
            Some((cached, image)) if *cached == key => Some(Arc::clone(image)),
            _ => {
                let pixels =
                    crate::tray::render_widget_with_accent(widget, &preview_limits(), key.accent);
                encode_png(&pixels).map(|png| {
                    let image = Arc::new(gpui::Image::from_bytes(ImageFormat::Png, png));
                    self.tray_previews
                        .insert(widget.id.clone(), (key, Arc::clone(&image)));
                    image
                })
            }
        };
        // The tray paints over the taskbar; give the preview the same plate.
        let plate = if crate::tray::system_uses_light_theme() {
            rgb8((0xEE, 0xEE, 0xEE))
        } else {
            rgb8((0x1C, 0x1C, 0x1C))
        };
        div()
            .size(px(size + 12.0))
            .flex_none()
            .rounded(px(6.0))
            .bg(plate)
            .border_1()
            .border_color(k.theme.card_stroke)
            .flex()
            .items_center()
            .justify_center()
            .children(image.map(|image| img(image).size(px(size))))
            .into_any_element()
    }

    fn set_widgets(&mut self, mut widgets: Vec<TrayWidget>, cx: &mut Context<Self>) {
        for widget in &mut widgets {
            widget.normalize();
        }
        self.edit(cx, move |settings| settings.tray_widgets = widgets.clone());
    }

    fn update_widgets(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Vec<TrayWidget>)) {
        let mut next = self.settings.tray_widgets.clone();
        f(&mut next);
        self.set_widgets(next, cx);
    }

    fn enabled_tray_providers(&self) -> Vec<ProviderId> {
        self.settings
            .instances
            .iter()
            .filter(|instance| instance.enabled)
            .filter(|instance| {
                !crate::provider_registry::descriptor(instance.driver)
                    .default_tray_metrics
                    .is_empty()
            })
            .map(|instance| instance.provider_id())
            .collect()
    }

    pub(super) fn tray_page(
        &mut self,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let widgets = self.settings.tray_widgets.clone();
        let enabled = self.enabled_tray_providers();
        let mut rows = vec![kit::caption(
            k,
            "Each widget is one icon in the notification area. Indicators show a provider's quota as numbers, bars, rings or a reset clock.",
        )];
        if let Some((index, removed)) = self.removed_widget.clone() {
            let undo = Self::h(cx, move |this, (), _, cx| {
                let removed = removed.clone();
                this.update_widgets(cx, |widgets| {
                    widgets.insert(index.min(widgets.len()), removed)
                });
                this.removed_widget = None;
            });
            rows.push(kit::appear(
                k,
                "tray-undo",
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .px(px(16.0))
                    .py(px(8.0))
                    .rounded(px(kit::CARD_RADIUS))
                    .bg(k.theme.accent_soft)
                    .child(kit::text("Widget removed", 13.0, k.theme.text).flex_1())
                    .child(Button::new("tray-undo", "Undo").on_click(undo).render(k))
                    .into_any_element(),
            ));
        }
        if widgets.is_empty() {
            rows.push(kit::row_card(
                k,
                Row::new("tray-empty", "Tray icon").description(k, "Shows the app icon."),
            ));
        }
        let count = widgets.len();
        for (index, widget) in widgets.iter().enumerate() {
            let card = self.tray_widget_card(k, index, count, widget, &enabled, window, cx);
            rows.push(kit::appear(k, format!("tray-widget-{}", widget.id), card));
        }
        let first = enabled.first().copied();
        let add_widget = Self::h(cx, move |this, (), _, cx| {
            let Some(provider) = first else {
                return;
            };
            let widget = TrayWidget::custom_for_provider(provider);
            this.set_expanded(format!("tray-{}", widget.id), true);
            this.update_widgets(cx, |widgets| widgets.push(widget));
        });
        let add_icon = Self::h(cx, |this, (), _, cx| {
            this.update_widgets(cx, |widgets| widgets.push(TrayWidget::app_icon()))
        });
        rows.push(
            div()
                .flex()
                .gap(px(8.0))
                .pt(px(8.0))
                .child(
                    Button::new("tray-add-widget", "Add widget")
                        .accent()
                        .with_icon("plus-bold")
                        .disabled(enabled.is_empty())
                        .on_click(add_widget)
                        .render(k),
                )
                .child(
                    Button::new("tray-add-icon", "Add app icon")
                        .on_click(add_icon)
                        .render(k),
                )
                .into_any_element(),
        );
        rows
    }

    #[allow(clippy::too_many_arguments)]
    fn tray_widget_card(
        &mut self,
        k: &mut Kit,
        index: usize,
        count: usize,
        widget: &TrayWidget,
        enabled: &[ProviderId],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let card_id = format!("tray-{}", widget.id);
        let expanded = self.is_expanded(&card_id);
        let move_up = Self::h(cx, move |this, (), _, cx| {
            this.update_widgets(cx, |widgets| {
                if index > 0 {
                    widgets.swap(index, index - 1)
                }
            })
        });
        let move_down = Self::h(cx, move |this, (), _, cx| {
            this.update_widgets(cx, |widgets| {
                if index + 1 < widgets.len() {
                    widgets.swap(index, index + 1)
                }
            })
        });
        let reorder = div()
            .flex()
            .flex_col()
            .child(
                Button::icon_only(format!("{card_id}-up"), "caret-up")
                    .tooltip("Move widget up")
                    .disabled(index == 0)
                    .on_click(move_up)
                    .render(k),
            )
            .child(
                Button::icon_only(format!("{card_id}-down"), "caret-down-bold")
                    .tooltip("Move widget down")
                    .disabled(index + 1 >= count)
                    .on_click(move_down)
                    .render(k),
            )
            .into_any_element();
        let preview = self.tray_preview(k, widget, 32.0);
        let header = Row::new(format!("{card_id}-header"), format!("Widget {}", index + 1))
            .icon(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(reorder)
                    .child(preview)
                    .into_any_element(),
            )
            .description(k, widget_summary(widget));
        let body = self.tray_widget_body(k, index, widget, enabled, window, cx);
        let on_toggle = Self::expand_handler(cx, card_id.clone());
        kit::expander(k, card_id, header, expanded, on_toggle, move |_| body)
    }

    fn tray_widget_body(
        &mut self,
        k: &mut Kit,
        index: usize,
        widget: &TrayWidget,
        enabled: &[ProviderId],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = widget.id.clone();
        let remove = Self::h(cx, move |this, (), _, cx| {
            let mut next = this.settings.tray_widgets.clone();
            if index < next.len() {
                let removed = next.remove(index);
                this.removed_widget = Some((index, removed));
                this.tray_editing = None;
                this.set_widgets(next, cx);
            }
        });
        let duplicate = Self::h(cx, move |this, (), _, cx| {
            this.update_widgets(cx, |widgets| {
                if let Some(widget) = widgets.get(index) {
                    let copy = if widget.kind == TrayWidgetKind::AppIcon {
                        TrayWidget::app_icon()
                    } else {
                        widget.duplicate_with_new_id()
                    };
                    widgets.insert(index + 1, copy);
                }
            })
        });
        let actions = |k: &Kit, extra: Option<AnyElement>| {
            div()
                .flex()
                .gap(px(8.0))
                .children(extra)
                .child(
                    Button::new(format!("tray-{id}-duplicate"), "Duplicate")
                        .with_icon("copy")
                        .on_click(duplicate.clone())
                        .render(k),
                )
                .child(
                    Button::new(format!("tray-{id}-remove"), "Remove")
                        .danger()
                        .with_icon("trash-fill")
                        .on_click(remove.clone())
                        .render(k),
                )
                .into_any_element()
        };
        if widget.kind == TrayWidgetKind::AppIcon {
            return actions(k, None);
        }
        let mut body = div().flex().flex_col().gap(px(14.0));
        let first_enabled = enabled.first().copied().unwrap_or_default();
        body = body.child(kit::field(
            k,
            "Appearance",
            kit::dropdown(
                k,
                format!("tray-{id}-presentation"),
                kit::options(&PRESENTATION_LABELS),
                Some(presentation_index(widget.presentation)),
                false,
                240.0,
                Self::h(cx, move |this, choice: usize, _, cx| {
                    this.update_widgets(cx, |widgets| {
                        let Some(widget) = widgets.get_mut(index) else {
                            return;
                        };
                        widget.presentation = presentation_from_index(choice);
                        if widget.presentation.is_reset_clock() {
                            widget.indicators.truncate(1);
                            if widget.indicators.is_empty() {
                                widget.indicators.push(default_indicator(first_enabled));
                            }
                        }
                    })
                }),
            ),
        ));
        let mut extra = None;
        if widget.presentation.is_reset_clock() {
            let indicator = widget
                .indicators
                .first()
                .cloned()
                .unwrap_or_else(|| default_indicator(first_enabled));
            body = body.child(self.indicator_fields(k, index, 0, &indicator, false, window, cx));
        } else {
            body = body.child(kit::text("Indicators", 12.0, k.theme.text_secondary));
            let count = widget.indicators.len();
            for (slot, indicator) in widget.indicators.iter().enumerate() {
                body = body
                    .child(self.indicator_row(k, index, slot, count, &widget.id, indicator, cx));
            }
            if count < MAX_INDICATORS {
                let fallback = widget
                    .indicators
                    .last()
                    .and_then(TrayIndicator::provider)
                    .unwrap_or(first_enabled);
                let widget_id = widget.id.clone();
                extra = Some(
                    Button::new(format!("tray-{id}-add-indicator"), "Add indicator")
                        .with_icon("plus-bold")
                        .on_click(Self::h(cx, move |this, (), _, cx| {
                            let slot = this
                                .settings
                                .tray_widgets
                                .get(index)
                                .map_or(0, |widget| widget.indicators.len());
                            this.update_widgets(cx, |widgets| {
                                if let Some(widget) = widgets.get_mut(index) {
                                    widget.indicators.push(default_indicator(fallback));
                                }
                            });
                            this.tray_editing = Some((widget_id.clone(), slot));
                        }))
                        .render(k),
                );
            }
        }
        body.child(actions(k, extra)).into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn indicator_row(
        &mut self,
        k: &mut Kit,
        widget_index: usize,
        slot: usize,
        count: usize,
        widget_id: &str,
        indicator: &TrayIndicator,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let swap = |delta: isize| {
            Self::h(cx, move |this, (), _, cx| {
                this.update_widgets(cx, |widgets| {
                    let Some(widget) = widgets.get_mut(widget_index) else {
                        return;
                    };
                    let target = slot as isize + delta;
                    if target >= 0 && (target as usize) < widget.indicators.len() {
                        widget.indicators.swap(slot, target as usize);
                    }
                })
            })
        };
        let id = format!("tray-{widget_id}-indicator-{slot}");
        let edit_id = widget_id.to_owned();
        let edit = Self::h(cx, move |this, (), _, cx| {
            this.tray_editing = Some((edit_id.clone(), slot));
            cx.notify();
        });
        let remove = Self::h(cx, move |this, (), _, cx| {
            let mut next = this.settings.tray_widgets.clone();
            let Some(widget) = next.get_mut(widget_index) else {
                return;
            };
            if slot < widget.indicators.len() {
                widget.indicators.remove(slot);
            }
            if widget.indicators.is_empty() {
                let removed = next.remove(widget_index);
                this.removed_widget = Some((widget_index, removed));
            }
            this.tray_editing = None;
            this.set_widgets(next, cx);
        });
        let theme = &k.theme;
        let swatch = (indicator.color_mode == TrayColorMode::Fixed).then(|| {
            let color = indicator.fixed_color;
            div()
                .size(px(10.0))
                .rounded_full()
                .bg(rgb8((color.red, color.green, color.blue)))
                .into_any_element()
        });
        div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .p(px(8.0))
            .pr(px(12.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.card_stroke)
            .bg(theme.subtle_hover)
            .child(
                div()
                    .flex()
                    .child(
                        Button::icon_only(format!("{id}-up"), "arrow-up-bold")
                            .tooltip("Move indicator up")
                            .disabled(slot == 0)
                            .on_click(swap(-1))
                            .render(k),
                    )
                    .child(
                        Button::icon_only(format!("{id}-down"), "arrow-down-bold")
                            .tooltip("Move indicator down")
                            .disabled(slot + 1 >= count)
                            .on_click(swap(1))
                            .render(k),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .children(swatch)
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(indicator_summary(indicator)),
                            ),
                    )
                    .child(kit::caption(k, format!("Indicator {}", slot + 1))),
            )
            .child(
                Button::new(format!("{id}-edit"), "Edit")
                    .with_icon("pencil-simple-fill")
                    .on_click(edit)
                    .render(k),
            )
            .child(
                Button::icon_only(format!("{id}-remove"), "trash-fill")
                    .tooltip("Remove indicator")
                    .on_click(remove)
                    .render(k),
            )
            .into_any_element()
    }

    /// Provider, metric, value and color for one indicator.
    fn indicator_fields(
        &mut self,
        k: &mut Kit,
        widget_index: usize,
        slot: usize,
        indicator: &TrayIndicator,
        with_value: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let options = provider_options();
        let known = indicator.provider();
        let mut provider_labels = options
            .iter()
            .map(|provider| SharedString::from(provider_label(*provider)))
            .collect::<Vec<_>>();
        let provider_index = known
            .and_then(|provider| options.iter().position(|option| *option == provider))
            .unwrap_or_else(|| {
                provider_labels.push(format!("Unsupported ({})", indicator.provider_id).into());
                provider_labels.len() - 1
            });
        let metrics = crate::provider_registry::tray_metric_options(
            known.unwrap_or_default().kind(),
            &super::cached_discovered_popup_bricks(),
        );
        let mut metric_labels = metrics
            .iter()
            .map(|(_, label)| SharedString::from(label.clone()))
            .collect::<Vec<_>>();
        let metric_index = metrics
            .iter()
            .position(|(id, _)| id == &indicator.metric_id)
            .unwrap_or_else(|| {
                metric_labels.push(format!("Unavailable ({})", indicator.metric_id).into());
                metric_labels.len() - 1
            });
        let key = format!("tray-{widget_index}-{slot}");
        let edit = |cx: &mut Context<Self>,
                    f: IndicatorEdit,
                    options: Vec<ProviderId>,
                    metrics: Vec<(String, String)>| {
            Self::h(cx, move |this, choice: usize, _, cx| {
                let options = options.clone();
                let metrics = metrics.clone();
                this.update_widgets(cx, move |widgets| {
                    let Some(widget) = widgets.get_mut(widget_index) else {
                        return;
                    };
                    if widget.indicators.is_empty() {
                        widget.indicators.push(default_indicator(
                            options.first().copied().unwrap_or_default(),
                        ));
                    }
                    if let Some(indicator) = widget.indicators.get_mut(slot) {
                        f(indicator, choice, &options, &metrics);
                    }
                })
            })
        };
        let provider_box = kit::dropdown(
            k,
            format!("{key}-provider"),
            provider_labels,
            Some(provider_index),
            false,
            0.0,
            edit(
                cx,
                |indicator, choice, options, _| {
                    if let Some(provider) = options.get(choice).copied() {
                        *indicator = TrayIndicator {
                            provider_id: provider.id().into(),
                            metric_id: default_indicator(provider).metric_id,
                            ..indicator.clone()
                        };
                    }
                },
                options.clone(),
                Vec::new(),
            ),
        );
        let metric_box = kit::dropdown(
            k,
            format!("{key}-metric-{}", indicator.provider_id),
            metric_labels,
            Some(metric_index),
            false,
            0.0,
            edit(
                cx,
                |indicator, choice, _, metrics| {
                    if let Some((id, _)) = metrics.get(choice) {
                        indicator.metric_id = id.clone();
                    }
                },
                Vec::new(),
                metrics.clone(),
            ),
        );
        let color_box = kit::dropdown(
            k,
            format!("{key}-color"),
            kit::options(&COLOR_LABELS),
            Some(color_mode_index(indicator.color_mode)),
            false,
            0.0,
            edit(
                cx,
                |indicator, choice, _, _| indicator.color_mode = color_mode_from_index(choice),
                Vec::new(),
                Vec::new(),
            ),
        );
        let mut grid = div().flex().flex_col().gap(px(14.0)).child(
            div()
                .flex()
                .gap(px(12.0))
                .child(kit::field(k, "Provider", provider_box))
                .child(kit::field(k, "Metric", metric_box)),
        );
        let mut second = div().flex().gap(px(12.0));
        if with_value {
            let value_box = kit::dropdown(
                k,
                format!("{key}-value"),
                kit::options(&["Remaining", "Used"]),
                Some(usize::from(indicator.limit_value == LimitValue::Used)),
                false,
                0.0,
                edit(
                    cx,
                    |indicator, choice, _, _| {
                        indicator.limit_value = if choice == 1 {
                            LimitValue::Used
                        } else {
                            LimitValue::Remaining
                        }
                    },
                    Vec::new(),
                    Vec::new(),
                ),
            );
            second = second.child(kit::field(k, "Value", value_box));
        }
        second = second.child(kit::field(k, "Color", color_box));
        grid = grid.child(second);
        if indicator.color_mode == TrayColorMode::Fixed {
            grid = grid.child(self.color_picker(
                k,
                widget_index,
                slot,
                indicator.fixed_color,
                window,
                cx,
            ));
        }
        grid.into_any_element()
    }

    fn color_picker(
        &mut self,
        k: &mut Kit,
        widget_index: usize,
        slot: usize,
        current: TrayFixedColor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let set = move |this: &mut Self, color: TrayFixedColor, cx: &mut Context<Self>| {
            this.update_widgets(cx, |widgets| {
                if let Some(indicator) = widgets
                    .get_mut(widget_index)
                    .and_then(|widget| widget.indicators.get_mut(slot))
                {
                    indicator.fixed_color = color;
                }
            })
        };
        let theme = &k.theme;
        let mut swatches = div().flex().flex_wrap().gap(px(8.0));
        for (index, (red, green, blue)) in SWATCHES.into_iter().enumerate() {
            let color = TrayFixedColor { red, green, blue };
            let selected = color == current;
            swatches = swatches.child(
                div()
                    .id(eid(format!("tray-swatch-{widget_index}-{slot}-{index}")))
                    .size(px(26.0))
                    .rounded(px(6.0))
                    .bg(rgb8((red, green, blue)))
                    .border_2()
                    .border_color(if selected {
                        theme.accent
                    } else {
                        theme.card_stroke
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| set(this, color, cx))),
            );
        }
        let hex = format!(
            "#{:02X}{:02X}{:02X}",
            current.red, current.green, current.blue
        );
        let hex_input = self.input(
            format!("tray-hex-{widget_index}-{slot}"),
            &hex,
            "#RRGGBB",
            false,
            false,
            window,
            cx,
            move |this, value, _, _, cx| {
                if let Some(color) = parse_hex(&value)
                    && color != current
                {
                    set(this, color, cx);
                }
            },
        );
        let hex_field = kit::text_field(k, &hex_input, Some(120.0), window, cx);
        let theme = &k.theme;
        let preview = div()
            .size(px(kit::CONTROL_HEIGHT))
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.card_stroke)
            .bg(rgb8((current.red, current.green, current.blue)));
        kit::field(
            k,
            "Fixed color",
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(swatches)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(preview)
                        .child(hex_field),
                )
                .into_any_element(),
        )
    }

    /// Modal editor for one indicator.
    pub(super) fn tray_editor_overlay(
        &mut self,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (widget_id, slot) = self.tray_editing.clone()?;
        let Some((widget_index, widget)) = self
            .settings
            .tray_widgets
            .iter()
            .cloned()
            .enumerate()
            .find(|(_, widget)| widget.id == widget_id)
        else {
            self.tray_editing = None;
            return None;
        };
        let Some(indicator) = widget.indicators.get(slot).cloned() else {
            self.tray_editing = None;
            return None;
        };
        let close = Self::h(cx, |this, (), _, cx| {
            this.tray_editing = None;
            cx.notify();
        });
        let preview = self.tray_preview(k, &widget, 32.0);
        let fields = self.indicator_fields(k, widget_index, slot, &indicator, true, window, cx);
        Some(kit::dialog(
            k,
            format!("tray-editor-{widget_id}-{slot}"),
            540.0,
            vec![
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.0))
                    .child(preview)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(kit::dialog_title(k, format!("Edit indicator {}", slot + 1)))
                            .child(kit::caption(k, indicator_summary(&indicator))),
                    )
                    .into_any_element(),
                fields,
            ],
            vec![
                Button::new("tray-editor-done", "Done")
                    .accent()
                    .full_width()
                    .on_click(close.clone())
                    .render(k),
            ],
            Some(close),
        ))
    }
}

fn parse_hex(value: &str) -> Option<TrayFixedColor> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |range: std::ops::Range<usize>| u8::from_str_radix(&hex[range], 16).ok();
    Some(TrayFixedColor {
        red: channel(0..2)?,
        green: channel(2..4)?,
        blue: channel(4..6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_colors_parse_with_or_without_hash() {
        assert_eq!(
            parse_hex("#0078d4"),
            Some(TrayFixedColor {
                red: 0,
                green: 0x78,
                blue: 0xD4
            })
        );
        assert!(parse_hex("12345").is_none());
        assert!(parse_hex("zzzzzz").is_none());
    }

    #[test]
    fn presentation_and_color_indices_round_trip() {
        for index in 0..PRESENTATION_LABELS.len() {
            assert_eq!(presentation_index(presentation_from_index(index)), index);
        }
        for index in 0..COLOR_LABELS.len() {
            assert_eq!(color_mode_index(color_mode_from_index(index)), index);
        }
    }

    #[test]
    fn previews_encode_as_png() {
        let pixels = vec![255_u8; 32 * 32 * 4];
        assert!(encode_png(&pixels).is_some_and(|png| png.starts_with(&[0x89, b'P', b'N', b'G'])));
    }
}
