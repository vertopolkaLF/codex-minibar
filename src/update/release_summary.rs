//! A release summary: the supported Markdown subset of a "What's new" note,
//! parsed into rows ready to render, with its decoded images.

use std::{collections::HashMap, ops::Range, sync::Arc};

use pulldown_cmark::{Event, HeadingLevel, Options, Tag, TagEnd};

// ---------------------------------------------------------------------------
// Content
// ---------------------------------------------------------------------------

/// A fetched summary, ready to render.
#[derive(Clone)]
pub struct Content {
    pub version: String,
    pub blocks: Vec<Block>,
    /// Decoded images by their `src` in the Markdown.
    pub images: HashMap<String, Arc<gpui::RenderImage>>,
}

impl std::fmt::Debug for Content {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Content")
            .field("version", &self.version)
            .field("blocks", &self.blocks.len())
            .field("images", &self.images.len())
            .finish()
    }
}

/// The supported Markdown subset, flattened into rows. `depth` is the list
/// nesting a row belongs to; list items sit one level left of their content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Heading {
        level: u8,
        text: Inline,
    },
    Paragraph {
        depth: usize,
        text: Inline,
    },
    Item {
        depth: usize,
        marker: Marker,
        text: Inline,
    },
    Image {
        depth: usize,
        src: String,
        alt: String,
    },
    Code {
        depth: usize,
        text: String,
    },
    Rule,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    Bullet,
    Number(u64),
}

/// Text with styled ranges. Only ranges that differ from plain text are
/// listed, in order and without overlaps.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inline {
    pub text: String,
    pub spans: Vec<Span>,
    /// HTTPS targets referenced by [`Span::link`].
    pub links: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub range: Range<usize>,
    pub style: SpanStyle,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpanStyle {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub link: Option<usize>,
}

impl Inline {
    fn push(&mut self, text: &str, style: SpanStyle) {
        if text.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(text);
        if style == SpanStyle::default() {
            return;
        }
        let end = self.text.len();
        match self.spans.last_mut() {
            Some(last) if last.range.end == start && last.style == style => last.range.end = end,
            _ => self.spans.push(Span {
                range: start..end,
                style,
            }),
        }
    }

    /// Byte ranges of each link, in [`Inline::links`] order.
    pub fn link_ranges(&self) -> Vec<(Range<usize>, usize)> {
        let mut ranges: Vec<(Range<usize>, usize)> = Vec::new();
        for span in &self.spans {
            let Some(link) = span.style.link else {
                continue;
            };
            match ranges.last_mut() {
                Some((range, last)) if *last == link && range.end == span.range.start => {
                    range.end = span.range.end;
                }
                _ => ranges.push((span.range.clone(), link)),
            }
        }
        ranges
    }
}

// ---------------------------------------------------------------------------
// Markdown
// ---------------------------------------------------------------------------

enum Pending {
    Heading(u8),
    Paragraph(usize),
    Item(usize, Marker),
}

#[derive(Default)]
struct Builder {
    blocks: Vec<Block>,
    /// Next number of each open list; None for bullet lists.
    lists: Vec<Option<u64>>,
    current: Option<(Pending, Inline)>,
    bold: usize,
    italic: usize,
    strike: usize,
    /// Open links; None for links that are not followed (non-HTTPS).
    links: Vec<Option<OpenLink>>,
    image: Option<(String, String)>,
    code_block: Option<String>,
}

/// A link being read. An image inside it flushes the paragraph, so the
/// link's slot in [`Inline::links`] is claimed per paragraph.
struct OpenLink {
    url: String,
    index: Option<usize>,
}

impl Builder {
    fn style(&self) -> SpanStyle {
        SpanStyle {
            bold: self.bold > 0,
            italic: self.italic > 0,
            strike: self.strike > 0,
            code: false,
            link: None,
        }
    }

    fn text(&mut self, text: &str, style: SpanStyle) {
        if let Some((_, alt)) = &mut self.image {
            alt.push_str(text);
        } else if let Some(code) = &mut self.code_block {
            code.push_str(text);
        } else {
            let depth = self.lists.len();
            let inline = &mut self
                .current
                .get_or_insert_with(|| (Pending::Paragraph(depth), Inline::default()))
                .1;
            let link = match self.links.last_mut() {
                Some(Some(open)) => Some(*open.index.get_or_insert_with(|| {
                    inline.links.push(open.url.clone());
                    inline.links.len() - 1
                })),
                _ => None,
            };
            inline.push(text, SpanStyle { link, ..style });
        }
    }

    fn flush(&mut self) {
        for open in self.links.iter_mut().flatten() {
            open.index = None;
        }
        let Some((pending, mut text)) = self.current.take() else {
            return;
        };
        let trimmed = text.text.trim_end().len();
        if trimmed == 0 {
            return;
        }
        if trimmed < text.text.len() {
            text.text.truncate(trimmed);
            text.spans.retain_mut(|span| {
                span.range.end = span.range.end.min(trimmed);
                span.range.start < span.range.end
            });
        }
        self.blocks.push(match pending {
            Pending::Heading(level) => Block::Heading { level, text },
            Pending::Paragraph(depth) => Block::Paragraph { depth, text },
            Pending::Item(depth, marker) => Block::Item {
                depth,
                marker,
                text,
            },
        });
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.text(&text, self.style()),
            Event::Code(text) => {
                let style = SpanStyle {
                    code: true,
                    ..self.style()
                };
                self.text(&text, style);
            }
            Event::SoftBreak => self.text(" ", self.style()),
            Event::HardBreak => self.text("\n", self.style()),
            Event::Rule => {
                self.flush();
                self.blocks.push(Block::Rule);
            }
            // Raw HTML, math and footnotes are outside the supported subset.
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {
                // A list item's first paragraph continues the item itself.
                let item_is_empty = matches!(
                    &self.current,
                    Some((Pending::Item(..), inline)) if inline.text.is_empty()
                );
                if !item_is_empty {
                    self.flush();
                }
            }
            Tag::Heading { level, .. } => {
                self.flush();
                let level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    _ => 3,
                };
                self.current = Some((Pending::Heading(level), Inline::default()));
            }
            Tag::List(start) => {
                self.flush();
                self.lists.push(start);
            }
            Tag::Item => {
                self.flush();
                let depth = self.lists.len().saturating_sub(1);
                let marker = match self.lists.last_mut() {
                    Some(Some(next)) => {
                        let number = *next;
                        *next += 1;
                        Marker::Number(number)
                    }
                    _ => Marker::Bullet,
                };
                self.current = Some((Pending::Item(depth, marker), Inline::default()));
            }
            Tag::CodeBlock(_) => {
                self.flush();
                self.code_block = Some(String::new());
            }
            Tag::Image { dest_url, .. } => {
                self.flush();
                self.image = Some((dest_url.to_string(), String::new()));
            }
            Tag::Link { dest_url, .. } => {
                let url = dest_url.trim();
                self.links.push(is_https(url).then(|| OpenLink {
                    url: url.to_string(),
                    index: None,
                }));
            }
            Tag::Emphasis => self.italic += 1,
            Tag::Strong => self.bold += 1,
            Tag::Strikethrough => self.strike += 1,
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::Item => self.flush(),
            TagEnd::List(_) => {
                self.flush();
                self.lists.pop();
            }
            TagEnd::CodeBlock => {
                if let Some(code) = self.code_block.take() {
                    let text = code.trim_end().to_string();
                    if !text.is_empty() {
                        let depth = self.lists.len();
                        self.blocks.push(Block::Code { depth, text });
                    }
                }
            }
            TagEnd::Image => {
                if let Some((src, alt)) = self.image.take() {
                    let depth = self.lists.len();
                    self.blocks.push(Block::Image { depth, src, alt });
                }
            }
            TagEnd::Link => {
                self.links.pop();
            }
            TagEnd::Emphasis => self.italic = self.italic.saturating_sub(1),
            TagEnd::Strong => self.bold = self.bold.saturating_sub(1),
            TagEnd::Strikethrough => self.strike = self.strike.saturating_sub(1),
            _ => {}
        }
    }
}

/// Parse the supported Markdown subset: headings, paragraphs, nested lists,
/// bold, italic, strikethrough, inline and block code, HTTPS links, images
/// and rules. Raw HTML is dropped.
pub fn parse(markdown: &str) -> Vec<Block> {
    let mut builder = Builder::default();
    for event in pulldown_cmark::Parser::new_ext(markdown, Options::ENABLE_STRIKETHROUGH) {
        builder.event(event);
    }
    builder.flush();
    builder.blocks
}

fn is_https(url: &str) -> bool {
    url.get(..8)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"))
}

/// Images must live next to the summary: a plain relative path without
/// parent segments, query or scheme.
pub(crate) fn is_release_relative(src: &str) -> bool {
    !src.is_empty()
        && !src.contains(['\\', '?', '#', ':'])
        && !src.starts_with('/')
        && src
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inline(text: &str) -> Inline {
        Inline {
            text: text.into(),
            ..Inline::default()
        }
    }

    #[test]
    fn parses_headings_paragraphs_and_lists() {
        let blocks =
            parse("## New\n\nIntro text\nwraps.\n\n- One\n- Two\n  1. Nested\n  2. Again\n\n---\n");
        assert_eq!(
            blocks,
            vec![
                Block::Heading {
                    level: 2,
                    text: inline("New"),
                },
                Block::Paragraph {
                    depth: 0,
                    text: inline("Intro text wraps."),
                },
                Block::Item {
                    depth: 0,
                    marker: Marker::Bullet,
                    text: inline("One"),
                },
                Block::Item {
                    depth: 0,
                    marker: Marker::Bullet,
                    text: inline("Two"),
                },
                Block::Item {
                    depth: 1,
                    marker: Marker::Number(1),
                    text: inline("Nested"),
                },
                Block::Item {
                    depth: 1,
                    marker: Marker::Number(2),
                    text: inline("Again"),
                },
                Block::Rule,
            ]
        );
    }

    #[test]
    fn a_link_around_an_image_keeps_its_caption_linked() {
        let blocks = parse("[![shot](https://example.com/a.png) caption](https://example.com)");
        let [Block::Image { .. }, Block::Paragraph { text, .. }] = blocks.as_slice() else {
            panic!("{blocks:?}");
        };
        assert_eq!(text.text.trim(), "caption");
        assert_eq!(text.links, vec!["https://example.com".to_string()]);
        assert!(text.spans.iter().all(|span| span.style.link == Some(0)));
    }

    #[test]
    fn styles_inline_spans_and_keeps_only_https_links() {
        let blocks = parse(
            "**Bold** and *it* `code` [site](https://example.com) [bad](javascript:x) ~~old~~",
        );
        let [Block::Paragraph { text, .. }] = blocks.as_slice() else {
            panic!("{blocks:?}");
        };
        assert_eq!(text.text, "Bold and it code site bad old");
        assert_eq!(text.links, vec!["https://example.com".to_string()]);
        let styled = |range: Range<usize>| {
            text.spans
                .iter()
                .find(|span| span.range == range)
                .map(|span| span.style)
                .unwrap()
        };
        assert!(styled(0..4).bold);
        assert!(styled(9..11).italic);
        assert!(styled(12..16).code);
        assert_eq!(styled(17..21).link, Some(0));
        assert!(styled(26..29).strike);
        assert_eq!(text.link_ranges(), vec![(17..21, 0)]);
    }

    #[test]
    fn images_inside_list_items_become_indented_blocks() {
        let blocks =
            parse("- Feature text.\n\n  ![Shot](shot.webp)\n\n- Next\n\n<b>raw html</b>\n");
        assert_eq!(
            blocks,
            vec![
                Block::Item {
                    depth: 0,
                    marker: Marker::Bullet,
                    text: inline("Feature text."),
                },
                Block::Image {
                    depth: 1,
                    src: "shot.webp".into(),
                    alt: "Shot".into(),
                },
                Block::Item {
                    depth: 0,
                    marker: Marker::Bullet,
                    text: inline("Next"),
                },
                // Inline HTML tags are dropped; their text stays.
                Block::Paragraph {
                    depth: 0,
                    text: inline("raw html"),
                },
            ]
        );
    }

    #[test]
    fn only_release_relative_images_are_fetched() {
        assert!(is_release_relative("hero.webp"));
        assert!(is_release_relative("media/hero.png"));
        assert!(!is_release_relative("../secret.png"));
        assert!(!is_release_relative("/abs.png"));
        assert!(!is_release_relative("https://example.com/x.png"));
        assert!(!is_release_relative("a//b.png"));
        assert!(!is_release_relative("x.png?y"));
    }
}
