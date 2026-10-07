//! Tray: icon widgets, their indicators and a live 32px preview of each.

use std::{collections::HashMap, sync::Arc};

use gpui::{
    AnyElement, Context, FontWeight, ImageFormat, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, div, img, prelude::FluentBuilder, px,
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
const PRESENTATION_HINTS: [&str; 5] = [
    "Percentages as digits",
    "One bar per indicator",
    "Nested rings, one per indicator",
    "When the limit resets",
    "Time left until the reset",
];
const MAX_INDICATORS: usize = 3;

/// The open widget editor: which widget and which indicator is expanded.
#[derive(Clone)]
pub(crate) struct TrayDialog {
    widget_id: String,
    expanded: Option<usize>,
}

impl TrayDialog {
    fn new(widget_id: String) -> Self {
        Self {
            widget_id,
            expanded: Some(0),
        }
    }
}

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

/// Metric, value and color of an indicator, under its provider name.
fn indicator_detail(indicator: &TrayIndicator) -> String {
    let metric = indicator.provider().map_or_else(
        || indicator.metric_id.clone(),
        |provider| {
            crate::provider_registry::settings_brick_label(
                provider.kind(),
                &indicator.metric_id,
                &super::cached_discovered_popup_bricks(),
            )
            .to_string()
        },
    );
    let value = match indicator.limit_value {
        LimitValue::Used => "Used",
        LimitValue::Remaining => "Remaining",
    };
    format!(
        "{metric} · {value} · {}",
        color_mode_label(indicator.color_mode)
    )
}

/// A dialog section title with a one-line hint under it.
fn dialog_section(k: &Kit, title: &'static str, hint: impl Into<SharedString>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .pt(px(6.0))
        .child(
            div()
                .text_size(px(14.0))
                .line_height(px(20.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(k.theme.text)
                .child(title),
        )
        .child(kit::caption(k, hint))
        .into_any_element()
}

fn widget_title(index: usize, widget: &TrayWidget) -> String {
    if widget.kind == TrayWidgetKind::AppIcon {
        "App icon".into()
    } else {
        format!("Widget {}", index + 1)
    }
}

/// Style and the providers a widget reads, for list rows and the editor.
fn widget_short_summary(widget: &TrayWidget) -> String {
    if widget.kind == TrayWidgetKind::AppIcon {
        return "The Codex Minibar icon".into();
    }
    let style = PRESENTATION_LABELS[presentation_index(widget.presentation)];
    let mut providers: Vec<String> = Vec::new();
    for provider in widget.indicators.iter().filter_map(TrayIndicator::provider) {
        let name = provider.qualified_name().to_string();
        if !providers.contains(&name) {
            providers.push(name);
        }
    }
    if providers.is_empty() {
        format!("{style} · No indicators")
    } else {
        format!("{style} · {}", providers.join(", "))
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
    // A reset a little over two hours out, so reset clocks show a time.
    let resets_at = Some(chrono::Utc::now() + chrono::Duration::minutes(137));
    let window = |used_percent| crate::limits::LimitWindow {
        used_percent: Some(used_percent),
        resets_at,
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
    fn tray_preview(&mut self, widget: &TrayWidget, size: f32) -> AnyElement {
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
        _window: &mut Window,
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
        } else {
            let count = widgets.len();
            let items = widgets
                .iter()
                .enumerate()
                .map(|(index, widget)| {
                    let row = self.tray_widget_row(k, index, count, widget, cx);
                    kit::appear(k, format!("tray-widget-{}", widget.id), row)
                })
                .collect();
            rows.push(kit::card(k, items));
        }
        let first = enabled.first().copied();
        let add_widget = Self::h(cx, move |this, (), _, cx| {
            let Some(provider) = first else {
                return;
            };
            let widget = TrayWidget::custom_for_provider(provider);
            this.tray_dialog = Some(TrayDialog::new(widget.id.clone()));
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

    /// One widget in the page list: live preview, name, a short summary and
    /// an action menu. Clicking the row opens the widget editor.
    fn tray_widget_row(
        &mut self,
        k: &Kit,
        index: usize,
        count: usize,
        widget: &TrayWidget,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = widget.id.clone();
        let open_id = id.clone();
        let open = Self::h(cx, move |this, (), _, cx| {
            this.tray_dialog = Some(TrayDialog::new(open_id.clone()));
            cx.notify();
        });
        let menu_id = id.clone();
        let mut up = kit::MenuItem::new("Move up").icon("arrow-up-bold");
        up.disabled = index == 0;
        let mut down = kit::MenuItem::new("Move down").icon("arrow-down-bold");
        down.disabled = index + 1 >= count;
        let menu = kit::more_menu(
            k,
            format!("tray-{id}-menu"),
            vec![
                kit::MenuItem::new("Edit").icon("pencil-simple-fill"),
                up,
                down,
                kit::MenuItem::new("Duplicate").icon("copy"),
                kit::MenuItem::new("Remove").icon("trash-fill").danger(),
            ],
            Self::h(cx, move |this, choice: usize, _, cx| match choice {
                0 => {
                    this.tray_dialog = Some(TrayDialog::new(menu_id.clone()));
                    cx.notify();
                }
                1 => this.move_widget(index, -1, cx),
                2 => this.move_widget(index, 1, cx),
                3 => this.duplicate_widget(index, cx),
                _ => this.remove_widget(index, cx),
            }),
        );
        let preview = self.tray_preview(widget, 24.0);
        Row::new(format!("tray-row-{id}"), widget_title(index, widget))
            .icon(preview)
            .description(k, widget_short_summary(widget))
            .trailing(menu)
            .trailing(kit::icon("caret-right", 12.0, k.theme.text_secondary).into_any_element())
            .on_click(open)
            .render(k)
    }

    fn move_widget(&mut self, index: usize, delta: isize, cx: &mut Context<Self>) {
        self.update_widgets(cx, |widgets| {
            let target = index as isize + delta;
            if index < widgets.len() && target >= 0 && (target as usize) < widgets.len() {
                widgets.swap(index, target as usize);
            }
        });
    }

    fn duplicate_widget(&mut self, index: usize, cx: &mut Context<Self>) {
        self.update_widgets(cx, |widgets| {
            if let Some(widget) = widgets.get(index) {
                let copy = if widget.kind == TrayWidgetKind::AppIcon {
                    TrayWidget::app_icon()
                } else {
                    widget.duplicate_with_new_id()
                };
                widgets.insert(index + 1, copy);
            }
        });
    }

    fn remove_widget(&mut self, index: usize, cx: &mut Context<Self>) {
        let mut next = self.settings.tray_widgets.clone();
        if index >= next.len() {
            return;
        }
        let removed = next.remove(index);
        if self
            .tray_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.widget_id == removed.id)
        {
            self.tray_dialog = None;
        }
        self.removed_widget = Some((index, removed));
        self.set_widgets(next, cx);
    }

    fn set_tray_dialog(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut TrayDialog)) {
        if let Some(dialog) = self.tray_dialog.as_mut() {
            f(dialog);
        }
        cx.notify();
    }

    /// The widget editor: a live preview header, then one page that reads top
    /// to bottom — how the icon is drawn, then which quotas it shows. Every
    /// choice applies to the tray immediately; "Done" only closes it.
    pub(super) fn tray_dialog_overlay(
        &mut self,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let dialog = self.tray_dialog.clone()?;
        let Some((index, widget)) = self
            .settings
            .tray_widgets
            .iter()
            .cloned()
            .enumerate()
            .find(|(_, widget)| widget.id == dialog.widget_id)
        else {
            self.tray_dialog = None;
            return None;
        };
        let close = Self::h(cx, |this, (), _, cx| {
            this.tray_dialog = None;
            cx.notify();
        });
        let preview = self.tray_preview(&widget, 32.0);
        let header = div()
            .flex()
            .items_center()
            .gap(px(14.0))
            .child(preview)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.0))
                    .child(kit::dialog_title(k, widget_title(index, &widget)))
                    .child(kit::caption(k, widget_short_summary(&widget))),
            )
            .into_any_element();
        let mut body = vec![header];
        if widget.kind == TrayWidgetKind::AppIcon {
            body.push(kit::caption(
                k,
                "Shows the Codex Minibar icon in the notification area. It has no indicators to set up.",
            ));
        } else {
            let style = presentation_index(widget.presentation);
            body.push(dialog_section(k, "Style", PRESENTATION_HINTS[style]));
            body.push(self.tray_style_picker(k, index, &widget, cx));
            let reset_clock = widget.presentation.is_reset_clock();
            let hint = if reset_clock {
                "A reset clock follows one quota.".to_string()
            } else {
                format!(
                    "Up to {MAX_INDICATORS} quotas, drawn in this order. Expand one to change it."
                )
            };
            body.push(dialog_section(k, "Indicators", hint));
            let indicators = self.tray_indicators(k, index, &widget, dialog.expanded, window, cx);
            body.push(kit::appear(
                k,
                format!("tray-dlg-{}-{reset_clock}", widget.id),
                indicators,
            ));
        }
        let remove = Button::new("tray-dlg-remove", "Remove widget")
            .danger()
            .with_icon("trash-fill")
            .full_width()
            .on_click(Self::h(cx, move |this, (), _, cx| {
                this.remove_widget(index, cx)
            }));
        let done = Button::new("tray-dlg-done", "Done")
            .accent()
            .full_width()
            .on_click(close.clone());
        Some(kit::dialog(
            k,
            format!("tray-dialog-{}", widget.id),
            600.0,
            body,
            vec![remove.render(k), done.render(k)],
            Some(close),
        ))
    }

    /// One tile per presentation, each previewing this widget drawn that way.
    fn tray_style_picker(
        &mut self,
        k: &Kit,
        index: usize,
        widget: &TrayWidget,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let first_enabled = self
            .enabled_tray_providers()
            .first()
            .copied()
            .unwrap_or_default();
        let current = presentation_index(widget.presentation);
        let tiles = (0..PRESENTATION_LABELS.len())
            .map(|choice| {
                let selected = choice == current;
                let preview = if selected {
                    self.tray_preview(widget, 24.0)
                } else {
                    let mut variant = widget.clone();
                    variant.id = format!("{}::style-{choice}", widget.id);
                    variant.presentation = presentation_from_index(choice);
                    if variant.presentation.is_reset_clock() {
                        variant.indicators.truncate(1);
                    }
                    variant.normalize();
                    self.tray_preview(&variant, 24.0)
                };
                let id = format!("tray-style-{choice}");
                let theme = &k.theme;
                let rest = if selected {
                    theme.accent_soft
                } else {
                    theme.control
                };
                let hover = if selected { rest } else { theme.control_hover };
                let tile = div()
                    .id(eid(id.clone()))
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(8.0))
                    .pt(px(12.0))
                    .pb(px(10.0))
                    .px(px(4.0))
                    .rounded(px(kit::CARD_RADIUS))
                    .border_1()
                    .border_color(if selected {
                        theme.accent
                    } else {
                        gpui::transparent_black()
                    })
                    .child(preview)
                    .child(
                        div()
                            .text_size(px(12.0))
                            .line_height(px(16.0))
                            .whitespace_nowrap()
                            .when(selected, |el| el.font_weight(FontWeight::SEMIBOLD))
                            .text_color(if selected {
                                theme.accent_text
                            } else {
                                theme.text
                            })
                            .child(PRESENTATION_LABELS[choice]),
                    );
                kit::hover_bg(k, tile, kit::hover_key(&id), rest, hover)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_presentation(index, choice, first_enabled, cx)
                    }))
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div().flex().gap(px(8.0)).children(tiles).into_any_element()
    }

    fn set_presentation(
        &mut self,
        index: usize,
        choice: usize,
        fallback: ProviderId,
        cx: &mut Context<Self>,
    ) {
        self.update_widgets(cx, |widgets| {
            let Some(widget) = widgets.get_mut(index) else {
                return;
            };
            widget.presentation = presentation_from_index(choice);
            if widget.presentation.is_reset_clock() {
                widget.indicators.truncate(1);
                if widget.indicators.is_empty() {
                    widget.indicators.push(default_indicator(fallback));
                }
            }
        });
    }

    /// The widget's indicators as expandable cards, in draw order, with an
    /// "Add" button below. Reset clocks follow a single indicator, so they
    /// show its fields directly.
    #[allow(clippy::too_many_arguments)]
    fn tray_indicators(
        &mut self,
        k: &mut Kit,
        index: usize,
        widget: &TrayWidget,
        expanded: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let first_enabled = self
            .enabled_tray_providers()
            .first()
            .copied()
            .unwrap_or_default();
        if widget.presentation.is_reset_clock() {
            let indicator = widget
                .indicators
                .first()
                .cloned()
                .unwrap_or_else(|| default_indicator(first_enabled));
            let fields = self.indicator_fields(k, index, 0, &indicator, false, window, cx);
            return kit::card_surface(
                k,
                [div()
                    .px(px(kit::ROW_PADDING_X))
                    .py(px(14.0))
                    .child(fields)
                    .into_any_element()],
            )
            .into_any_element();
        }
        let count = widget.indicators.len();
        let mut column = div().flex().flex_col().gap(px(8.0));
        for (slot, indicator) in widget.indicators.iter().enumerate() {
            let card = self.indicator_card(
                k,
                index,
                slot,
                count,
                &widget.id,
                indicator,
                expanded == Some(slot),
                window,
                cx,
            );
            column = column.child(kit::appear(
                k,
                format!("tray-{}-indicator-{slot}-in", widget.id),
                card,
            ));
        }
        if count < MAX_INDICATORS {
            let fallback = widget
                .indicators
                .last()
                .and_then(TrayIndicator::provider)
                .unwrap_or(first_enabled);
            column = column.child(
                div().flex().child(
                    Button::new(format!("tray-{}-add-indicator", widget.id), "Add indicator")
                        .with_icon("plus-bold")
                        .on_click(Self::h(cx, move |this, (), _, cx| {
                            this.update_widgets(cx, |widgets| {
                                if let Some(widget) = widgets.get_mut(index) {
                                    widget.indicators.push(default_indicator(fallback));
                                }
                            });
                            this.set_tray_dialog(cx, |dialog| dialog.expanded = Some(count));
                        }))
                        .render(k),
                ),
            );
        }
        column.into_any_element()
    }

    /// One indicator: provider mark, provider name and a one-line summary,
    /// with reorder/remove controls. Its header expands the fields in place.
    #[allow(clippy::too_many_arguments)]
    fn indicator_card(
        &mut self,
        k: &mut Kit,
        widget_index: usize,
        slot: usize,
        count: usize,
        widget_id: &str,
        indicator: &TrayIndicator,
        expanded: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = format!("tray-{widget_id}-indicator-{slot}");
        let swap = |delta: isize| {
            Self::h(cx, move |this, (), _, cx| {
                let target = slot as isize + delta;
                if target < 0 {
                    return;
                }
                let target = target as usize;
                this.update_widgets(cx, |widgets| {
                    let Some(widget) = widgets.get_mut(widget_index) else {
                        return;
                    };
                    if target < widget.indicators.len() {
                        widget.indicators.swap(slot, target);
                    }
                });
                // The expanded card travels with its indicator.
                this.set_tray_dialog(cx, |dialog| {
                    dialog.expanded = match dialog.expanded {
                        Some(open) if open == slot => Some(target),
                        Some(open) if open == target => Some(slot),
                        other => other,
                    }
                });
            })
        };
        let remove = Self::h(cx, move |this, (), _, cx| {
            this.update_widgets(cx, |widgets| {
                if let Some(widget) = widgets.get_mut(widget_index)
                    && widget.indicators.len() > 1
                    && slot < widget.indicators.len()
                {
                    widget.indicators.remove(slot);
                }
            });
            this.set_tray_dialog(cx, |dialog| {
                dialog.expanded = match dialog.expanded {
                    Some(open) if open == slot => None,
                    Some(open) if open > slot => Some(open - 1),
                    other => other,
                }
            });
        });
        let toggle = Self::h(cx, move |this, open: bool, _, cx| {
            this.set_tray_dialog(cx, |dialog| dialog.expanded = open.then_some(slot))
        });
        let theme = &k.theme;
        let (glyph, tint, title) = match indicator.provider() {
            Some(provider) => (
                crate::provider_registry::icon(provider.kind()),
                theme.brand(provider.kind()),
                provider.qualified_name().to_string(),
            ),
            None => (
                "warning-fill",
                theme.caution,
                format!("Unsupported {}", indicator.provider_id),
            ),
        };
        let mark = div()
            .relative()
            .child(kit::glyph_plate(
                36.0,
                theme.subtle_hover,
                kit::icon(glyph, 18.0, tint).into_any_element(),
            ))
            .when(indicator.color_mode == TrayColorMode::Fixed, |el| {
                let color = indicator.fixed_color;
                el.child(
                    div()
                        .absolute()
                        .right(px(-2.0))
                        .bottom(px(-2.0))
                        .size(px(12.0))
                        .rounded_full()
                        .bg(rgb8((color.red, color.green, color.blue))),
                )
            })
            .into_any_element();
        let controls = div()
            .flex()
            .gap(px(2.0))
            .child(
                Button::icon_only(format!("{id}-up"), "arrow-up-bold")
                    .ghost()
                    .tooltip("Move up")
                    .disabled(slot == 0)
                    .on_click(swap(-1))
                    .render(k),
            )
            .child(
                Button::icon_only(format!("{id}-down"), "arrow-down-bold")
                    .ghost()
                    .tooltip("Move down")
                    .disabled(slot + 1 >= count)
                    .on_click(swap(1))
                    .render(k),
            )
            .child(
                Button::icon_only(format!("{id}-remove"), "trash-fill")
                    .ghost()
                    .tooltip("Remove indicator")
                    .disabled(count <= 1)
                    .on_click(remove)
                    .render(k),
            )
            .into_any_element();
        let header = Row::new(format!("{id}-header"), title)
            .icon(mark)
            .description(k, indicator_detail(indicator))
            .trailing(controls);
        let fields = self.indicator_fields(k, widget_index, slot, indicator, true, window, cx);
        kit::expander(k, id, header, expanded, toggle, move |_| fields)
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
            second = second.child(kit::field(k, "Show", value_box));
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
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(selected, |el| {
                        let luma = 0.299 * f32::from(red)
                            + 0.587 * f32::from(green)
                            + 0.114 * f32::from(blue);
                        let mark = if luma > 150.0 {
                            rgb8((0, 0, 0))
                        } else {
                            rgb8((255, 255, 255))
                        };
                        el.child(kit::icon("check-bold", 14.0, mark))
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
        let preview = div()
            .size(px(kit::CONTROL_HEIGHT))
            .rounded(px(6.0))
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
