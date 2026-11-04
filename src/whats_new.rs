//! "What's new" release summaries, shown once after an update as part of
//! its "Update complete" notification.
//!
//! Summaries are not shipped with the app. Each release keeps a short
//! Markdown file per language (plus any images it references) under
//! `data/releases/<version>/` on the main branch, the same public GitHub
//! location as the reset feed and the Stream Deck plugin. The app fetches the
//! one for its own version after it starts on a newer version than the one
//! the user last saw. A missing summary (404) simply means there is nothing
//! to show for that release.

use std::{
    collections::HashMap,
    fs,
    io::Read,
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use pulldown_cmark::{Event, HeadingLevel, Options, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use ureq::Agent;

use crate::i18n::Language;
use crate::notifications::{Notification, NotificationAction, NotificationKind, notify};
use crate::settings::Settings;

pub const BASE_URL: &str =
    "https://raw.githubusercontent.com/vertopolkaLF/codex-minibar/main/data/releases";
const USER_AGENT: &str = "codex-minibar-whats-new";
const STATE_FILE: &str = "whats-new.json";
const MAX_MARKDOWN_BYTES: u64 = 64 * 1024;
const MAX_IMAGE_BYTES: u64 = 3 * 1024 * 1024;
const MAX_IMAGES: usize = 6;
/// Wider images are scaled down; the window is never wider than this.
const MAX_IMAGE_WIDTH: u32 = 1600;
/// A summary published late still shows if it appears within this window
/// after the update; after that the release is treated as having none.
const MISSING_GRACE: chrono::TimeDelta = chrono::TimeDelta::days(3);
/// Delays before each attempt in one session. The first one is short so the
/// summary lands on the "Update complete" card while it is still up.
const ATTEMPT_DELAYS: [Duration; 4] = [
    Duration::from_secs(1),
    Duration::from_secs(2 * 60),
    Duration::from_secs(15 * 60),
    Duration::from_secs(60 * 60),
];

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

#[derive(Clone, Debug)]
pub enum Fetched {
    Ready(Content),
    /// No summary is published for this version.
    Missing,
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
    links: Vec<Option<usize>>,
    image: Option<(String, String)>,
    code_block: Option<String>,
}

impl Builder {
    fn style(&self) -> SpanStyle {
        SpanStyle {
            bold: self.bold > 0,
            italic: self.italic > 0,
            strike: self.strike > 0,
            code: false,
            link: self.links.last().copied().flatten(),
        }
    }

    fn inline(&mut self) -> &mut Inline {
        let depth = self.lists.len();
        &mut self
            .current
            .get_or_insert_with(|| (Pending::Paragraph(depth), Inline::default()))
            .1
    }

    fn text(&mut self, text: &str, style: SpanStyle) {
        if let Some((_, alt)) = &mut self.image {
            alt.push_str(text);
        } else if let Some(code) = &mut self.code_block {
            code.push_str(text);
        } else {
            self.inline().push(text, style);
        }
    }

    fn flush(&mut self) {
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
                let link = is_https(url).then(|| {
                    let inline = self.inline();
                    inline.links.push(url.to_string());
                    inline.links.len() - 1
                });
                self.links.push(link);
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
fn is_release_relative(src: &str) -> bool {
    !src.is_empty()
        && !src.contains(['\\', '?', '#', ':'])
        && !src.starts_with('/')
        && src
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

// ---------------------------------------------------------------------------
// Fetching
// ---------------------------------------------------------------------------

/// Content file name for a language; every language falls back to English.
fn locale_tag(language: Language) -> &'static str {
    match language.resolved() {
        Language::Russian => "ru",
        Language::BrazilianPortuguese => "pt-BR",
        Language::Spanish => "es",
        Language::SimplifiedChinese => "zh-CN",
        Language::TraditionalChinese => "zh-TW",
        Language::Japanese => "ja",
        Language::German => "de",
        Language::French => "fr",
        Language::Korean => "ko-KR",
        Language::Turkish => "tr",
        Language::English | Language::Auto => "en",
    }
}

pub fn release_url(version: &str) -> String {
    format!("{}/tag/v{version}", crate::updater::RELEASES_URL)
}

fn http_agent() -> Agent {
    crate::http::public_agent_builder()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(20))
        .timeout_write(Duration::from_secs(20))
        .build()
}

/// GET a file, `None` on 404. Bodies over `limit` are rejected.
fn get(agent: &Agent, url: &str, limit: u64) -> Result<Option<Vec<u8>>> {
    let response = match agent
        .get(url)
        .set("User-Agent", USER_AGENT)
        .set("Cache-Control", "no-cache")
        .call()
    {
        Ok(response) => response,
        Err(ureq::Error::Status(404, _)) => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("GET {url}")),
    };
    let mut body = Vec::new();
    response
        .into_reader()
        .take(limit + 1)
        .read_to_end(&mut body)
        .with_context(|| format!("read {url}"))?;
    if body.len() as u64 > limit {
        bail!("{url} is larger than {limit} bytes");
    }
    Ok(Some(body))
}

/// Fetch and decode the summary for `version` in `language`, falling back
/// to English. Blocking; call it off the UI thread.
pub fn fetch(version: &str, language: Language) -> Result<Fetched> {
    let agent = http_agent();
    let base = format!("{BASE_URL}/{version}");
    let mut tags = vec![locale_tag(language)];
    if tags[0] != "en" {
        tags.push("en");
    }
    let mut markdown = None;
    for tag in tags {
        let url = format!("{base}/whats-new.{tag}.md");
        if let Some(body) = get(&agent, &url, MAX_MARKDOWN_BYTES)? {
            markdown = Some(String::from_utf8(body).context("summary is not UTF-8")?);
            break;
        }
    }
    let Some(markdown) = markdown else {
        crate::logger::info(format!("What's new: no summary for {version}"));
        return Ok(Fetched::Missing);
    };
    let blocks = parse(&markdown);
    let mut images = HashMap::new();
    let sources = blocks.iter().filter_map(|block| match block {
        Block::Image { src, .. } if is_release_relative(src) => Some(src.clone()),
        _ => None,
    });
    for src in sources.take(MAX_IMAGES) {
        if images.contains_key(&src) {
            continue;
        }
        // A broken image only loses itself, never the summary.
        let url = format!("{base}/{src}");
        match get(&agent, &url, MAX_IMAGE_BYTES) {
            Ok(Some(bytes)) => match decode_image(&bytes) {
                Some(image) => {
                    images.insert(src, Arc::new(image));
                }
                None => crate::logger::info(format!("What's new: could not decode {url}")),
            },
            Ok(None) => crate::logger::info(format!("What's new: {url} is missing")),
            Err(error) => crate::logger::info(format!("What's new: {error:#}")),
        }
    }
    Ok(Fetched::Ready(Content {
        version: version.to_string(),
        blocks,
        images,
    }))
}

/// The first frame as BGRA pixels, at most [`MAX_IMAGE_WIDTH`] wide.
fn decode_image(bytes: &[u8]) -> Option<gpui::RenderImage> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().ok()?;
    let image = if image.width() > MAX_IMAGE_WIDTH {
        let height = (u64::from(image.height()) * u64::from(MAX_IMAGE_WIDTH)
            / u64::from(image.width()))
        .max(1) as u32;
        image.resize_exact(
            MAX_IMAGE_WIDTH,
            height,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        image
    };
    let mut pixels = image.into_rgba8();
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Some(gpui::RenderImage::new(smallvec::smallvec![
        image::Frame::new(pixels)
    ]))
}

// ---------------------------------------------------------------------------
// Seen state
// ---------------------------------------------------------------------------

/// Which summary the user has already seen. Kept beside the settings file
/// rather than in it: it is bookkeeping, not a preference.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct State {
    seen_version: Option<String>,
    /// When the current version's summary first went unseen.
    pending_since: Option<DateTime<Utc>>,
}

fn state_path() -> Result<PathBuf> {
    Ok(Settings::default_path()?.with_file_name(STATE_FILE))
}

fn load_state(path: &Path) -> Option<State> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn save_state(path: &Path, state: &State) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let content = serde_json::to_vec_pretty(state).context("serialize what's new state")?;
    fs::write(path, content).with_context(|| format!("write {}", path.display()))
}

/// Record that `version`'s summary was shown or deliberately skipped.
pub fn mark_seen(version: &str) {
    let result = state_path().and_then(|path| {
        save_state(
            &path,
            &State {
                seen_version: Some(version.to_string()),
                pending_since: None,
            },
        )
    });
    if let Err(error) = result {
        eprintln!("failed to save what's new state: {error:#}");
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Decision {
    /// Nothing to do; the state is current.
    Done,
    /// Record `current` as seen without showing anything.
    Skip,
    /// Fetch and show the summary.
    Show,
}

/// `state` is None when no state file exists yet: a fresh install still in
/// onboarding has nothing new to see, while an existing install upgrading
/// to the first version with summaries does.
fn decide(
    state: Option<&State>,
    onboarding_completed: bool,
    current: &semver::Version,
) -> Decision {
    let seen = state
        .and_then(|state| state.seen_version.as_deref())
        .and_then(|version| semver::Version::parse(version).ok());
    match seen {
        Some(seen) if seen >= *current => Decision::Done,
        None if state.is_none() && !onboarding_completed => Decision::Skip,
        _ => Decision::Show,
    }
}

/// The summary as a notification. After an in-place update it fills the
/// "Update complete" card; otherwise (an installer update, or on demand) it
/// is its own card.
fn notification(content: Content, just_updated: bool) -> Notification {
    let version = content.version.clone();
    let notification = if just_updated {
        crate::updater::update_complete_notification(&version)
    } else {
        Notification::new(
            NotificationKind::Info,
            crate::i18n::tr("what-s-new"),
            &crate::i18n::format("version", &[("v0", version.clone())]),
        )
    };
    notification.with_summary(content).with_action(
        crate::i18n::tr("full-release-notes"),
        NotificationAction::OpenUrl(release_url(&version)),
    )
}

/// Show the running version's summary now, e.g. from About.
pub fn show_on_demand() {
    let spawned = thread::Builder::new().name("whats-new".into()).spawn(|| {
        let version = crate::updater::current_version().to_string();
        match fetch(&version, crate::i18n::current_language()) {
            Ok(Fetched::Ready(content)) => notify(notification(content, false)),
            Ok(Fetched::Missing) => notify(
                Notification::new(
                    NotificationKind::Info,
                    crate::i18n::tr("what-s-new"),
                    crate::i18n::tr("no-summary-for-this-version"),
                )
                .with_action(
                    crate::i18n::tr("full-release-notes"),
                    NotificationAction::OpenUrl(release_url(&version)),
                ),
            ),
            Err(error) => {
                crate::logger::info(format!("What's new fetch failed: {error:#}"));
                notify(
                    Notification::new(
                        NotificationKind::Warning,
                        crate::i18n::tr("what-s-new"),
                        crate::i18n::tr("couldn-t-load-what-s-new"),
                    )
                    .with_action(
                        crate::i18n::tr("full-release-notes"),
                        NotificationAction::OpenUrl(release_url(&version)),
                    ),
                );
            }
        }
    });
    if let Err(error) = spawned {
        eprintln!("could not load what's new: {error}");
    }
}

/// Decide at startup whether this version's summary is due and, if so,
/// fetch it in the background and post it as a notification.
pub fn start(settings: &Settings, just_updated: bool) {
    let onboarding_completed = settings.onboarding_completed;
    let spawned = thread::Builder::new()
        .name("whats-new".into())
        .spawn(move || run(onboarding_completed, just_updated));
    if let Err(error) = spawned {
        eprintln!("could not start the what's new check: {error}");
    }
}

fn run(onboarding_completed: bool, just_updated: bool) {
    let current = crate::updater::current_version();
    let version = current.to_string();
    let path = match state_path() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("what's new state is unavailable: {error:#}");
            return;
        }
    };
    let mut state = load_state(&path);
    match decide(state.as_ref(), onboarding_completed, &current) {
        Decision::Done => return,
        Decision::Skip => {
            mark_seen(&version);
            return;
        }
        Decision::Show => {}
    }
    let pending_since = match state.as_ref().and_then(|state| state.pending_since) {
        Some(since) => since,
        None => {
            let since = Utc::now();
            let state = state.get_or_insert_with(State::default);
            state.pending_since = Some(since);
            if let Err(error) = save_state(&path, state) {
                eprintln!("failed to save what's new state: {error:#}");
            }
            since
        }
    };
    for (attempt, delay) in ATTEMPT_DELAYS.into_iter().enumerate() {
        thread::sleep(delay);
        // Read the language per attempt: it may change while we wait.
        match fetch(&version, crate::i18n::current_language()) {
            Ok(Fetched::Ready(content)) => {
                mark_seen(&version);
                // Later attempts arrive after the update card is gone.
                notify(notification(content, just_updated && attempt == 0));
                return;
            }
            Ok(Fetched::Missing) if Utc::now() - pending_since > MISSING_GRACE => {
                mark_seen(&version);
                return;
            }
            Ok(Fetched::Missing) => {}
            Err(error) => crate::logger::info(format!("What's new fetch failed: {error:#}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn korean_summary_uses_its_catalog_tag() {
        assert_eq!(locale_tag(Language::Korean), "ko-KR");
    }

    #[test]
    fn turkish_summary_uses_its_catalog_tag() {
        assert_eq!(locale_tag(Language::Turkish), "tr");
    }

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

    #[test]
    fn decides_when_a_summary_is_due() {
        let current = semver::Version::new(1, 2, 0);
        let seen = |version: &str| State {
            seen_version: Some(version.into()),
            pending_since: None,
        };
        assert_eq!(decide(None, false, &current), Decision::Skip);
        assert_eq!(decide(None, true, &current), Decision::Show);
        assert_eq!(decide(Some(&seen("1.1.0")), true, &current), Decision::Show);
        assert_eq!(decide(Some(&seen("1.2.0")), true, &current), Decision::Done);
        assert_eq!(decide(Some(&seen("1.3.0")), true, &current), Decision::Done);
        // Unreadable state never re-enters the fresh-install path.
        assert_eq!(
            decide(Some(&State::default()), false, &current),
            Decision::Show
        );
    }

    #[test]
    fn state_round_trips() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(STATE_FILE);
        let state = State {
            seen_version: Some("1.2.0".into()),
            pending_since: Some(Utc::now()),
        };
        save_state(&path, &state).unwrap();
        assert_eq!(load_state(&path), Some(state));
    }
}
