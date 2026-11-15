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

use std::{collections::HashMap, fs, path::Path, sync::Arc, thread, time::Duration};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};

use serde::{Deserialize, Serialize};
use ureq::Agent;

use crate::i18n::Language;
use crate::settings::Settings;
use crate::surfaces::notifications::{Notification, NotificationAction, NotificationKind, notify};
use crate::update::release_summary::{Block, Content, is_release_relative, parse};

pub const BASE_URL: &str =
    "https://raw.githubusercontent.com/vertopolkaLF/codex-minibar/main/data/releases";
const USER_AGENT: &str = "codex-minibar-whats-new";
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

#[derive(Clone, Debug)]
pub enum Fetched {
    Ready(Content),
    /// No summary is published for this version.
    Missing,
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
    format!("{}/tag/v{version}", crate::update::RELEASES_URL)
}

fn http_agent() -> Agent {
    crate::platform::net::native_tls_agent()
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
    crate::platform::net::read_limited(response, limit)
        .with_context(|| format!("read {url}"))?
        .with_context(|| format!("{url} is larger than {limit} bytes"))
        .map(Some)
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
        crate::platform::logger::info(format!("What's new: no summary for {version}"));
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
                None => {
                    crate::platform::logger::info(format!("What's new: could not decode {url}"))
                }
            },
            Ok(None) => crate::platform::logger::info(format!("What's new: {url} is missing")),
            Err(error) => crate::platform::logger::info(format!("What's new: {error:#}")),
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
    let result = crate::platform::paths::whats_new_state().and_then(|path| {
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

/// The summary as its own card.
fn notification(content: Content) -> Notification {
    let version = content.version.clone();
    Notification::new(
        NotificationKind::Info,
        crate::i18n::tr("what-s-new"),
        &crate::i18n::format("version", &[("v0", version.clone())]),
    )
    .with_summary(content)
    .with_action(
        crate::i18n::tr("full-release-notes"),
        NotificationAction::OpenUrl(release_url(&version)),
    )
}

/// Show the running version's summary now, e.g. from About.
pub fn show_on_demand() {
    let spawned = thread::Builder::new().name("whats-new".into()).spawn(|| {
        let version = crate::update::current_version().to_string();
        match fetch(&version, crate::i18n::current_language()) {
            Ok(Fetched::Ready(content)) => notify(notification(content)),
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
                crate::platform::logger::info(format!("What's new fetch failed: {error:#}"));
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
pub fn start(settings: &Settings) {
    let onboarding_completed = settings.onboarding_completed;
    let spawned = thread::Builder::new()
        .name("whats-new".into())
        .spawn(move || run(onboarding_completed));
    if let Err(error) = spawned {
        eprintln!("could not start the what's new check: {error}");
    }
}

fn run(onboarding_completed: bool) {
    let current = crate::update::current_version();
    let version = current.to_string();
    let path = match crate::platform::paths::whats_new_state() {
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
    for delay in ATTEMPT_DELAYS {
        thread::sleep(delay);
        // Read the language per attempt: it may change while we wait.
        match fetch(&version, crate::i18n::current_language()) {
            Ok(Fetched::Ready(content)) => {
                mark_seen(&version);
                // While the "Update complete" card of an in-place update is
                // up the summary fills that card instead.
                notify(
                    notification(content)
                        .filling(crate::surfaces::notifications::UPDATE_COMPLETE_KEY),
                );
                return;
            }
            Ok(Fetched::Missing) if Utc::now() - pending_since > MISSING_GRACE => {
                mark_seen(&version);
                return;
            }
            Ok(Fetched::Missing) => {}
            Err(error) => {
                crate::platform::logger::info(format!("What's new fetch failed: {error:#}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summaries_use_their_catalog_tags() {
        assert_eq!(locale_tag(Language::Korean), "ko-KR");
        assert_eq!(locale_tag(Language::Turkish), "tr");
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
        let path = directory.path().join("whats-new.json");
        let state = State {
            seen_version: Some("1.2.0".into()),
            pending_since: Some(Utc::now()),
        };
        save_state(&path, &state).unwrap();
        assert_eq!(load_state(&path), Some(state));
    }
}
