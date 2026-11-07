//! Icons, provider marks and instance badge plates.

use gpui::{Div, FontWeight, Hsla, ParentElement, SharedString, Styled, div, img, px, svg};

use super::color::rgb8;
use crate::instances::Badge;
use crate::settings::BadgeColor;

/// A monochrome Iconify glyph tinted with `color`.
pub(crate) fn icon(name: &str, size: f32, color: Hsla) -> gpui::Svg {
    svg()
        .path(super::assets::icon_path(name))
        .size(px(size))
        .flex_none()
        .text_color(color)
}

/// A full-color SVG or bitmap asset (sidebar Fluent Color icons, app icon).
pub(crate) fn image(path: &'static str, size: f32) -> gpui::Img {
    img(gpui::ImageSource::Resource(gpui::Resource::Embedded(
        path.into(),
    )))
    .size(px(size))
    .flex_none()
}

/// How a surface paints instance badge plates: the colors of an `Auto`
/// badge, which follow the surface, and the outline that sets a plate off
/// the glyph under it.
#[derive(Clone, Copy)]
pub(crate) struct Plate {
    pub(crate) auto_fill: Hsla,
    pub(crate) auto_text: Hsla,
    pub(crate) outline: Hsla,
}

impl Plate {
    /// Fill and text of a plate in `color`.
    pub(crate) fn colors(&self, color: BadgeColor) -> (Hsla, Hsla) {
        match color.rgb().zip(color.text_rgb()) {
            Some((fill, text)) => (rgb8(fill), rgb8(text)),
            None => (self.auto_fill, self.auto_text),
        }
    }
}

/// Instance badge plate: up to three letters on the badge color, sized to
/// sit on a provider mark or in a segmented control.
pub(crate) fn badge_plate(badge: &Badge, height: f32, plate: Plate) -> Div {
    let (fill, text) = plate.colors(badge.color);
    div()
        .flex_none()
        .h(px(height))
        .min_w(px(height))
        .px(px((height * 0.22).max(2.0)))
        .rounded(px(height * 0.3))
        .bg(fill)
        .border_1()
        .border_color(plate.outline)
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .text_size(px((height * 0.68).max(7.0)))
                .line_height(px(height))
                .text_color(text)
                .font_weight(FontWeight::BOLD)
                .whitespace_nowrap()
                .child(SharedString::from(badge.text.clone())),
        )
}

/// A driver icon with the instance badge in its lower-right corner. Callers
/// pass a badge only while the driver has several enabled instances.
pub(crate) fn provider_mark(
    icon_name: &str,
    size: f32,
    color: Hsla,
    badge: Option<&Badge>,
    plate: Plate,
) -> Div {
    let mut mark = div()
        .relative()
        .flex_none()
        .size(px(size))
        .child(icon(icon_name, size, color));
    if let Some(badge) = badge {
        let height = (size * 0.62).clamp(9.0, 14.0);
        mark = mark.child(
            div()
                .absolute()
                .right(px(-height * 0.45))
                .bottom(px(-height * 0.3))
                .child(badge_plate(badge, height, plate)),
        );
    }
    mark
}
