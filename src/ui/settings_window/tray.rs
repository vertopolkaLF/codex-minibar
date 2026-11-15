//! Tray: icon widgets, their indicators and a live 32px preview of each.

use std::{collections::HashMap, sync::Arc};

use gpui::{
    AnyElement, Context, ImageFormat, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, Window, div, img, px,
};

use super::Host;
use super::widget_list::{Tray, default_indicator, dialog_section, style_tile, widget_providers};
use super::window::SettingsWindow;
use crate::settings::{
    Choice, ProviderId, TimeFormat, TrayIndicator, TrayPresentation, TrayWidget, TrayWidgetKind,
};
use crate::ui::gui::color::rgb8;
use crate::ui::gui::kit::{self, Button, Kit, Row};

fn presentation_label(presentation: TrayPresentation) -> &'static str {
    match presentation.canonical_percentage() {
        TrayPresentation::StackedBars => crate::i18n::tr("progress-bars"),
        TrayPresentation::NestedRings => crate::i18n::tr("rings"),
        TrayPresentation::ResetTime => crate::i18n::tr("reset-time"),
        TrayPresentation::ResetCountdown => crate::i18n::tr("countdown"),
        _ => crate::i18n::tr("numbers"),
    }
}

fn presentation_hint(presentation: TrayPresentation) -> &'static str {
    match presentation.canonical_percentage() {
        TrayPresentation::StackedBars => crate::i18n::tr("one-bar-per-indicator"),
        TrayPresentation::NestedRings => crate::i18n::tr("nested-rings-one-per-indicator"),
        TrayPresentation::ResetTime => crate::i18n::tr("when-the-limit-resets"),
        TrayPresentation::ResetCountdown => crate::i18n::tr("time-left-until-the-reset"),
        _ => crate::i18n::tr("percentages-as-digits"),
    }
}

const MAX_INDICATORS: usize = 3;

#[derive(Clone, PartialEq)]
pub(crate) struct PreviewKey {
    widget: TrayWidget,
    accent: [u8; 3],
    light: bool,
    time_format: TimeFormat,
    minute: u64,
}

pub(crate) type PreviewCache = HashMap<String, (PreviewKey, Arc<gpui::Image>)>;

fn widget_title(index: usize, widget: &TrayWidget) -> String {
    if widget.kind == TrayWidgetKind::AppIcon {
        crate::i18n::tr("app-icon").into()
    } else {
        crate::i18n::format("widget", &[("v0", (index + 1).to_string())])
    }
}

/// Style and the providers a widget reads, for list rows and the editor.
fn widget_short_summary(widget: &TrayWidget) -> String {
    if widget.kind == TrayWidgetKind::AppIcon {
        return crate::i18n::tr("the-codex-minibar-icon").into();
    }
    let style = presentation_label(widget.presentation);
    let mut providers: Vec<String> = Vec::new();
    for provider in widget.indicators.iter().filter_map(TrayIndicator::provider) {
        let name = provider.qualified_name().to_string();
        if !providers.contains(&name) {
            providers.push(name);
        }
    }
    if providers.is_empty() {
        crate::i18n::format("style-no-indicators", &[("style", style.to_string())])
    } else {
        format!("{style} · {}", providers.join(", "))
    }
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
            accent: crate::ui::theme::current_accent_rgb(),
            light: crate::platform::win32::shell_uses_light_theme(),
            time_format: TimeFormat::current(),
            minute: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_secs() / 60),
        };
        let image = match self.tray_previews.get(&widget.id) {
            Some((cached, image)) if *cached == key => Some(Arc::clone(image)),
            _ => {
                let pixels = crate::surfaces::tray::render_widget_with_accent(
                    widget,
                    &super::widget_list::preview_limits(&self.settings.instances),
                    key.accent,
                );
                encode_png(&pixels).map(|png| {
                    let image = Arc::new(gpui::Image::from_bytes(ImageFormat::Png, png));
                    self.tray_previews
                        .insert(widget.id.clone(), (key, Arc::clone(&image)));
                    image
                })
            }
        };
        // The tray paints over the taskbar; give the preview the same plate.
        let plate = if crate::platform::win32::shell_uses_light_theme() {
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

    pub(super) fn tray_page(
        &mut self,
        k: &mut Kit,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let enabled = widget_providers(&self.settings.instances, true);
        let mut rows = vec![kit::caption(
            k,
            crate::i18n::tr("each-widget-is-one-icon-in-the-notification-area-indicators-show"),
        )];
        let empty = Row::new("tray-empty", crate::i18n::tr("tray-icon"))
            .description(k, crate::i18n::tr("shows-the-app-icon"));
        rows.extend(
            self.widget_list::<Tray>(k, cx, empty, |this, k, index, widget| {
                Row::new(
                    format!("tray-row-{}", widget.id),
                    widget_title(index, widget),
                )
                .icon(this.tray_preview(widget, 24.0))
                .description(k, widget_short_summary(widget))
            }),
        );
        let first = enabled.first().copied();
        let add_widget = Self::h(cx, move |this, (), _, cx| {
            if let Some(provider) = first {
                this.add_widget::<Tray>(TrayWidget::custom_for_provider(provider), cx);
            }
        });
        let add_icon = Self::h(cx, |this, (), _, cx| {
            this.update_widgets::<Tray>(cx, |widgets| widgets.push(TrayWidget::app_icon()))
        });
        rows.push(
            div()
                .flex()
                .gap(px(8.0))
                .pt(px(8.0))
                .child(
                    Button::new("tray-add-widget", crate::i18n::tr("add-widget"))
                        .accent()
                        .with_icon("plus-bold")
                        .disabled(enabled.is_empty())
                        .on_click(add_widget)
                        .render(k),
                )
                .child(
                    Button::new("tray-add-icon", crate::i18n::tr("add-app-icon"))
                        .on_click(add_icon)
                        .render(k),
                )
                .into_any_element(),
        );
        rows
    }

    /// The widget editor: a live preview header, then one page that reads top
    /// to bottom — how the icon is drawn, then which quotas it shows.
    pub(super) fn tray_dialog_overlay(
        &mut self,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (dialog, index, widget, phase) = self.widget_dialog::<Tray>(k)?;
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
                crate::i18n::tr(
                    "shows-the-codex-minibar-icon-in-the-notification-area-it-has-no-i",
                ),
            ));
        } else {
            body.push(dialog_section(
                k,
                crate::i18n::tr("style"),
                presentation_hint(widget.presentation),
            ));
            body.push(self.tray_style_picker(k, index, &widget, cx));
            let reset_clock = widget.presentation.is_reset_clock();
            let hint = if reset_clock {
                crate::i18n::tr("a-reset-clock-follows-one-quota").to_string()
            } else {
                crate::i18n::format(
                    "up-to-max-indicators-quotas-drawn-in-this-order-expand-one-to-cha",
                    &[("max_indicators", MAX_INDICATORS.to_string())],
                )
            };
            body.push(dialog_section(k, crate::i18n::tr("indicators"), hint));
            let indicators = self.indicator_editor::<Tray>(
                k,
                index,
                &widget.id,
                &widget.indicators,
                MAX_INDICATORS,
                reset_clock,
                dialog.expanded,
                window,
                cx,
            );
            body.push(kit::appear(
                k,
                format!("tray-dlg-{}-{reset_clock}", widget.id),
                indicators,
            ));
        }
        Some(self.widget_dialog_frame::<Tray>(k, cx, &widget.id, index, phase, 600.0, body))
    }

    /// One tile per presentation, each previewing this widget drawn that way.
    fn tray_style_picker(
        &mut self,
        k: &Kit,
        index: usize,
        widget: &TrayWidget,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let first_enabled = widget_providers(&self.settings.instances, true)
            .first()
            .copied()
            .unwrap_or_default();
        let current = widget.presentation.index();
        let tiles = (0..TrayPresentation::ALL.len())
            .map(|choice| {
                let preview = if choice == current {
                    self.tray_preview(widget, 24.0)
                } else {
                    let mut variant = widget.clone();
                    variant.id = format!("{}::style-{choice}", widget.id);
                    variant.presentation = TrayPresentation::from_index(choice);
                    if variant.presentation.is_reset_clock() {
                        variant.indicators.truncate(1);
                    }
                    variant.normalize();
                    self.tray_preview(&variant, 24.0)
                };
                style_tile(
                    k,
                    format!("tray-style-{choice}"),
                    choice == current,
                    preview,
                    presentation_label(TrayPresentation::from_index(choice)),
                )
                .relative()
                .flex_1()
                .min_w_0()
                .items_center()
                .pt(px(12.0))
                .pb(px(10.0))
                .px(px(4.0))
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
        self.update_widgets::<Tray>(cx, |widgets| {
            let Some(widget) = widgets.get_mut(index) else {
                return;
            };
            widget.presentation = TrayPresentation::from_index(choice);
            if widget.presentation.is_reset_clock() {
                widget.indicators.truncate(1);
                if widget.indicators.is_empty() {
                    widget.indicators.push(default_indicator(fallback));
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_encode_only_square_rgba_icons() {
        let pixels = vec![255_u8; 32 * 32 * 4];
        assert!(encode_png(&pixels).is_some_and(|png| png.starts_with(&[0x89, b'P', b'N', b'G'])));
        assert!(encode_png(&[]).is_none());
        assert!(encode_png(&vec![255_u8; 32 * 16 * 4]).is_none());
    }
}
