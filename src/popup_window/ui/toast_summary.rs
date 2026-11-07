//! The release summary drawn inside an "Update complete" or "What's new"
//! notification card.
//!
//! Cards have a fixed height so the stack can glide, so every block is
//! measured here with the same fonts, sizes and widths it is laid out with.

use std::sync::Arc;

use gpui::{
    AnyElement, App, Div, FontStyle, FontWeight, HighlightStyle, InteractiveText, IntoElement,
    ObjectFit, ParentElement, RenderImage, SharedString, StrikethroughStyle, Styled, StyledImage,
    StyledText, UnderlineStyle, Window, div, img, px,
};

use crate::release_summary::{Block, Content, Inline, Marker, SpanStyle};

use super::theme::Palette;

const TEXT: f32 = 13.0;
const LINE: f32 = 19.0;
const BLOCK_GAP: f32 = 6.0;
/// Extra room above a heading that follows other content.
const HEADING_GAP: f32 = 4.0;
/// Indent of each list level, and the width of a list marker.
const INDENT: f32 = 16.0;
const IMAGE_MAX_HEIGHT: f32 = 200.0;
const IMAGE_GAP: f32 = 2.0;
const CODE_PADDING: f32 = 6.0;

fn indent(depth: usize) -> f32 {
    depth as f32 * INDENT
}

fn font(family: &SharedString, style: SpanStyle, heading: bool) -> gpui::Font {
    let mut font = gpui::font(family.clone());
    if style.bold || heading {
        font.weight = FontWeight::SEMIBOLD;
    }
    if style.italic {
        font.style = FontStyle::Italic;
    }
    font
}

/// Lines `inline` wraps to at `width`.
fn lines(
    window: &Window,
    inline: &Inline,
    width: f32,
    family: &SharedString,
    heading: bool,
) -> usize {
    let text = &inline.text;
    let mut runs = Vec::new();
    let mut at = 0;
    let mut push = |len: usize, style: SpanStyle| {
        if len > 0 {
            runs.push(gpui::TextRun {
                len,
                font: font(family, style, heading),
                color: gpui::black(),
                background_color: None,
                underline: None,
                strikethrough: None,
            });
        }
    };
    for span in &inline.spans {
        push(span.range.start - at, SpanStyle::default());
        push(span.range.len(), span.style);
        at = span.range.end;
    }
    push(text.len() - at, SpanStyle::default());
    window
        .text_system()
        .shape_text(
            SharedString::from(text.clone()),
            px(TEXT),
            &runs,
            Some(px(width.max(1.0))),
            None,
        )
        .map(|lines| {
            lines
                .iter()
                .map(|line| line.wrap_boundaries().len() + 1)
                .sum::<usize>()
        })
        .unwrap_or(1)
        .max(1)
}

/// `(width, height)` of an image fitted into `width`.
fn image_size(image: &gpui::RenderImage, width: f32) -> (f32, f32) {
    let natural = image.size(0);
    let (w, h) = (
        natural.width.0.max(1) as f32,
        natural.height.0.max(1) as f32,
    );
    let fitted = width.min(w);
    let height = (fitted * h / w).min(IMAGE_MAX_HEIGHT);
    (height * w / h, height)
}

/// Height of one block at `width`; None when it is left out.
fn block_height(
    window: &Window,
    content: &Content,
    block: &Block,
    width: f32,
    family: &SharedString,
) -> Option<f32> {
    let text = |inline: &Inline, width: f32, heading: bool| {
        lines(window, inline, width, family, heading) as f32 * LINE
    };
    Some(match block {
        Block::Heading { text: inline, .. } => text(inline, width, true),
        Block::Paragraph {
            depth,
            text: inline,
        } => text(inline, width - indent(*depth), false),
        Block::Item {
            depth,
            text: inline,
            ..
        } => text(inline, width - indent(*depth) - INDENT, false),
        Block::Image { depth, src, .. } => {
            let image = content.images.get(src)?;
            image_size(image, width - indent(*depth)).1 + IMAGE_GAP * 2.0
        }
        Block::Code { depth, text: code } => {
            let inline = Inline {
                text: code.clone(),
                ..Inline::default()
            };
            text(&inline, width - indent(*depth) - CODE_PADDING * 2.0, false) + CODE_PADDING * 2.0
        }
        Block::Rule => 1.0,
    })
}

/// The summary's first image, center-cropped to `width` x `height` so it can
/// fill the top of the card edge to edge. Returns its block index, which
/// [`measure`] and [`render`] then skip.
pub(super) fn hero(
    content: &Content,
    width: f32,
    height: f32,
) -> Option<(usize, Arc<RenderImage>)> {
    let (index, image) = content
        .blocks
        .iter()
        .enumerate()
        .find_map(|(index, block)| {
            let Block::Image { src, .. } = block else {
                return None;
            };
            content.images.get(src).map(|image| (index, image))
        })?;
    Some((index, Arc::new(crop_to_ratio(image, width / height)?)))
}

/// The centered part of `image` with the aspect ratio `ratio`.
fn crop_to_ratio(image: &RenderImage, ratio: f32) -> Option<RenderImage> {
    let size = image.size(0);
    let (w, h) = (size.width.0 as u32, size.height.0 as u32);
    let buffer = image::RgbaImage::from_raw(w, h, image.as_bytes(0)?.to_vec())?;
    let (cw, ch) = if w as f32 / h as f32 > ratio {
        (((h as f32 * ratio).round() as u32).clamp(1, w), h)
    } else {
        (w, ((w as f32 / ratio).round() as u32).clamp(1, h))
    };
    // Pixels stay BGRA, as GPUI expects; cropping does not care.
    let cropped = image::imageops::crop_imm(&buffer, (w - cw) / 2, (h - ch) / 2, cw, ch).to_image();
    Some(RenderImage::new(smallvec::smallvec![image::Frame::new(
        cropped
    )]))
}

/// Natural height of the summary at `width`, without the `skip` block.
pub(super) fn measure(
    window: &Window,
    content: &Content,
    width: f32,
    family: &SharedString,
    skip: Option<usize>,
) -> f32 {
    let mut height = 0.0;
    let mut first = true;
    for (index, block) in content.blocks.iter().enumerate() {
        if Some(index) == skip {
            continue;
        }
        let Some(block_height) = block_height(window, content, block, width, family) else {
            continue;
        };
        if !first {
            height += BLOCK_GAP;
            if matches!(block, Block::Heading { .. }) {
                height += HEADING_GAP;
            }
        }
        first = false;
        height += block_height;
    }
    height
}

fn highlight(style: SpanStyle, palette: &Palette) -> HighlightStyle {
    let mut highlight = HighlightStyle::default();
    if style.bold {
        highlight.font_weight = Some(FontWeight::SEMIBOLD);
    }
    if style.italic {
        highlight.font_style = Some(FontStyle::Italic);
    }
    if style.strike {
        highlight.strikethrough = Some(StrikethroughStyle {
            thickness: px(1.0),
            color: None,
        });
    }
    if style.code {
        highlight.background_color = Some(palette.control_fill);
    }
    if style.link.is_some() {
        highlight.color = Some(palette.accent_text);
        highlight.underline = Some(UnderlineStyle {
            thickness: px(1.0),
            color: Some(palette.accent_text),
            wavy: false,
        });
    }
    highlight
}

/// Styled text whose links open in the browser without closing the card.
fn inline_text(id: SharedString, inline: &Inline, palette: &Palette) -> AnyElement {
    let styled = StyledText::new(inline.text.clone()).with_highlights(
        inline
            .spans
            .iter()
            .map(|span| (span.range.clone(), highlight(span.style, palette))),
    );
    let ranges = inline.link_ranges();
    if ranges.is_empty() {
        return styled.into_any_element();
    }
    let targets: Vec<String> = ranges
        .iter()
        .map(|(_, link)| inline.links[*link].clone())
        .collect();
    InteractiveText::new(id, styled)
        .on_click(
            ranges.into_iter().map(|(range, _)| range).collect(),
            move |index, _, cx: &mut App| {
                cx.stop_propagation();
                crate::notifications::NotificationAction::OpenUrl(targets[index].clone()).run();
            },
        )
        .into_any_element()
}

fn render_block(
    card: u64,
    index: usize,
    content: &Content,
    block: &Block,
    width: f32,
    palette: &Palette,
) -> Option<Div> {
    let id = || SharedString::from(format!("toast-{card}-summary-{index}"));
    Some(match block {
        Block::Heading { text, .. } => {
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child(inline_text(id(), text, palette))
        }
        Block::Paragraph { depth, text } => {
            div()
                .pl(px(indent(*depth)))
                .child(inline_text(id(), text, palette))
        }
        Block::Item {
            depth,
            marker,
            text,
        } => {
            let marker: AnyElement = match marker {
                Marker::Bullet => div()
                    .mt(px((LINE - 4.0) / 2.0))
                    .size(px(4.0))
                    .rounded_full()
                    .bg(palette.text_secondary)
                    .into_any_element(),
                Marker::Number(number) => div()
                    .text_color(palette.text_secondary)
                    .child(format!("{number}."))
                    .into_any_element(),
            };
            div()
                .pl(px(indent(*depth)))
                .flex()
                .flex_row()
                .items_start()
                .child(div().w(px(INDENT)).flex_none().child(marker))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(inline_text(id(), text, palette)),
                )
        }
        Block::Image { depth, src, .. } => {
            let image = content.images.get(src)?;
            let (w, h) = image_size(image, width - indent(*depth));
            div().pl(px(indent(*depth))).py(px(IMAGE_GAP)).child(
                img(Arc::clone(image))
                    .w(px(w))
                    .h(px(h))
                    .object_fit(ObjectFit::Contain)
                    .rounded(px(palette.control_radius)),
            )
        }
        Block::Code { depth, text } => div().pl(px(indent(*depth))).child(
            div()
                .p(px(CODE_PADDING))
                .rounded(px(palette.control_radius))
                .bg(palette.control_fill)
                .child(text.clone()),
        ),
        Block::Rule => div().h(px(1.0)).bg(palette.divider),
    })
}

/// The summary at `width`, without the `skip` block, laid out to match
/// [`measure`].
pub(super) fn render(
    card: u64,
    content: &Content,
    width: f32,
    skip: Option<usize>,
    palette: &Palette,
) -> Div {
    let mut column = div()
        .w(px(width))
        .flex()
        .flex_col()
        .text_size(px(TEXT))
        .line_height(px(LINE))
        .text_color(palette.text_primary);
    let mut first = true;
    for (index, block) in content.blocks.iter().enumerate() {
        if Some(index) == skip {
            continue;
        }
        let Some(element) = render_block(card, index, content, block, width, palette) else {
            continue;
        };
        let gap = if first {
            0.0
        } else if matches!(block, Block::Heading { .. }) {
            BLOCK_GAP + HEADING_GAP
        } else {
            BLOCK_GAP
        };
        first = false;
        column = column.child(element.mt(px(gap)).flex_none());
    }
    column
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: u32, height: u32) -> RenderImage {
        let buffer =
            image::RgbaImage::from_fn(width, height, |x, _| image::Rgba([x as u8, 0, 0, 255]));
        RenderImage::new(smallvec::smallvec![image::Frame::new(buffer)])
    }

    #[test]
    fn hero_crop_keeps_the_center() {
        let wide = crop_to_ratio(&image(8, 2), 1.0).unwrap();
        assert_eq!(wide.size(0).width.0, 2);
        assert_eq!(wide.size(0).height.0, 2);
        // Columns 3 and 4 of 0..8 are the middle pair.
        assert_eq!(wide.as_bytes(0).unwrap()[0], 3);

        let tall = crop_to_ratio(&image(4, 8), 2.0).unwrap();
        assert_eq!(tall.size(0).width.0, 4);
        assert_eq!(tall.size(0).height.0, 2);
    }
}
