//! Embedded SVG icons for the GPUI popup.
//!
//! GPUI paints SVGs as alpha masks tinted by `text_color`, so the Iconify
//! sources are used as-is for their geometry. Every icon is normalized to a
//! square, centered view box first: GPUI scales by width only, and marks such
//! as Kiro (256×312) would otherwise overflow their slot vertically.

use std::borrow::Cow;

use gpui::{AssetSource, SharedString};

pub(crate) struct PopupAssets;

/// Asset path for a popup icon name (the same names `crate::icons` uses).
pub(crate) fn icon_path(name: &str) -> SharedString {
    SharedString::from(format!("icons/{name}.svg"))
}

fn source(name: &str) -> Option<&'static str> {
    Some(match name {
        "fluent-refresh" => include_str!("../../../assets/icons/fluent-arrow-sync-24-filled.svg"),
        "fluent-arrow-download" => {
            include_str!("../../../assets/icons/fluent-arrow-download-24-filled.svg")
        }
        "fluent-settings" => include_str!("../../../assets/icons/fluent-settings-20-filled.svg"),
        "fluent-power" => include_str!("../../../assets/icons/fluent-power-20-filled.svg"),
        "fluent-delete" => include_str!("../../../assets/icons/fluent-delete-20-regular.svg"),
        "fluent-drag" => {
            include_str!("../../../assets/icons/fluent-re-order-dots-vertical-16-filled.svg")
        }
        "fluent-chevron-down" => {
            include_str!("../../../assets/icons/fluent-chevron-down-16-regular.svg")
        }
        "fluent-chart" => include_str!("../../../assets/icons/fluent-data-histogram-24-filled.svg"),
        "fluent-home" => include_str!("../../../assets/icons/fluent-home-24-filled.svg"),
        "fluent-error-circle" => {
            include_str!("../../../assets/icons/fluent-error-circle-16-filled.svg")
        }
        "fluent-warning" => include_str!("../../../assets/icons/fluent-warning-16-filled.svg"),
        "fluent-add" => include_str!("../../../assets/icons/fluent-add-16-regular.svg"),
        "fluent-arrow-left" => {
            include_str!("../../../assets/icons/fluent-arrow-left-16-regular.svg")
        }
        "fluent-copy" => include_str!("../../../assets/icons/fluent-copy-16-regular.svg"),
        "fluent-checkmark-circle" => {
            include_str!("../../../assets/icons/fluent-checkmark-circle-16-filled.svg")
        }
        "codex" => include_str!("../../../assets/icons/openai-iconify.svg"),
        "claude" => include_str!("../../../assets/icons/claude-iconify.svg"),
        "cursor" => include_str!("../../../assets/icons/cursor-iconify.svg"),
        "opencode" => include_str!("../../../assets/icons/opencode-iconify.svg"),
        "openrouter" => include_str!("../../../assets/icons/openrouter-iconify.svg"),
        "antigravity" => include_str!("../../../assets/icons/antigravity.svg"),
        "grok" => include_str!("../../../assets/icons/grok.svg"),
        "kiro" => include_str!("../../../assets/icons/kiro-iconify.svg"),
        "chatgpt" => include_str!("../../../assets/icons/chatgpt-iconify.svg"),
        "fluent-folder" => include_str!("../../../assets/icons/fluent-folder-16-filled.svg"),
        "github" => include_str!("../../../assets/icons/github-iconify.svg"),
        // Phosphor glyphs used by the Settings window, keyed by file stem.
        "arrow-clockwise-bold" => include_str!("../../../assets/icons/ph-arrow-clockwise-bold.svg"),
        "arrow-counter-clockwise-bold" => {
            include_str!("../../../assets/icons/ph-arrow-counter-clockwise-bold.svg")
        }
        "arrow-down-bold" => include_str!("../../../assets/icons/ph-arrow-down-bold.svg"),
        "arrow-square-out" => include_str!("../../../assets/icons/ph-arrow-square-out.svg"),
        "arrow-up-bold" => include_str!("../../../assets/icons/ph-arrow-up-bold.svg"),
        "at-fill" => include_str!("../../../assets/icons/ph-at-fill.svg"),
        "bell-fill" => include_str!("../../../assets/icons/ph-bell-fill.svg"),
        "broom-fill" => include_str!("../../../assets/icons/ph-broom-fill.svg"),
        "caret-down" => include_str!("../../../assets/icons/ph-caret-down.svg"),
        "caret-down-bold" => include_str!("../../../assets/icons/ph-caret-down-bold.svg"),
        "caret-left" => include_str!("../../../assets/icons/ph-caret-left.svg"),
        "caret-right" => include_str!("../../../assets/icons/ph-caret-right.svg"),
        "caret-up" => include_str!("../../../assets/icons/ph-caret-up.svg"),
        "chat-centered-text-fill" => {
            include_str!("../../../assets/icons/ph-chat-centered-text-fill.svg")
        }
        "check-bold" => include_str!("../../../assets/icons/ph-check-bold.svg"),
        "check-circle-fill" => include_str!("../../../assets/icons/ph-check-circle-fill.svg"),
        "circle-half-fill" => include_str!("../../../assets/icons/ph-circle-half-fill.svg"),
        "clock-fill" => include_str!("../../../assets/icons/ph-clock-fill.svg"),
        "copy" => include_str!("../../../assets/icons/ph-copy.svg"),
        "desktop-fill" => include_str!("../../../assets/icons/ph-desktop-fill.svg"),
        "dots-three-bold" => include_str!("../../../assets/icons/ph-dots-three-bold.svg"),
        "download-simple-fill" => include_str!("../../../assets/icons/ph-download-simple-fill.svg"),
        "export-fill" => include_str!("../../../assets/icons/ph-export-fill.svg"),
        "file-text-fill" => include_str!("../../../assets/icons/ph-file-text-fill.svg"),
        "flag-fill" => include_str!("../../../assets/icons/ph-flag-fill.svg"),
        "folder-fill" => include_str!("../../../assets/icons/ph-folder-fill.svg"),
        "folder-open-fill" => include_str!("../../../assets/icons/ph-folder-open-fill.svg"),
        "git-pull-request-fill" => {
            include_str!("../../../assets/icons/ph-git-pull-request-fill.svg")
        }
        "github-logo-fill" => include_str!("../../../assets/icons/ph-github-logo-fill.svg"),
        "house-fill" => include_str!("../../../assets/icons/ph-house-fill.svg"),
        "info-fill" => include_str!("../../../assets/icons/ph-info-fill.svg"),
        "key-fill" => include_str!("../../../assets/icons/ph-key-fill.svg"),
        "package-fill" => include_str!("../../../assets/icons/ph-package-fill.svg"),
        "paint-brush-fill" => include_str!("../../../assets/icons/ph-paint-brush-fill.svg"),
        "pencil-simple-fill" => include_str!("../../../assets/icons/ph-pencil-simple-fill.svg"),
        "plugs-connected-fill" => include_str!("../../../assets/icons/ph-plugs-connected-fill.svg"),
        "plus-bold" => include_str!("../../../assets/icons/ph-plus-bold.svg"),
        "plus-fill" => include_str!("../../../assets/icons/ph-plus-fill.svg"),
        "puzzle-piece-fill" => include_str!("../../../assets/icons/ph-puzzle-piece-fill.svg"),
        "scroll-fill" => include_str!("../../../assets/icons/ph-scroll-fill.svg"),
        "sign-in-bold" => include_str!("../../../assets/icons/ph-sign-in-bold.svg"),
        "sparkle-fill" => include_str!("../../../assets/icons/ph-sparkle-fill.svg"),
        "squares-four-fill" => include_str!("../../../assets/icons/ph-squares-four-fill.svg"),
        "terminal-window-fill" => include_str!("../../../assets/icons/ph-terminal-window-fill.svg"),
        "trash-fill" => include_str!("../../../assets/icons/ph-trash-fill.svg"),
        "upload-simple-fill" => include_str!("../../../assets/icons/ph-upload-simple-fill.svg"),
        "user-fill" => include_str!("../../../assets/icons/ph-user-fill.svg"),
        "warning-fill" => include_str!("../../../assets/icons/ph-warning-fill.svg"),
        "x-circle-fill" => include_str!("../../../assets/icons/ph-x-circle-fill.svg"),
        _ => return None,
    })
}

fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!(" {name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(&tag[start..end])
}

fn strip_attribute(tag: &str, name: &str) -> String {
    let needle = format!(" {name}=\"");
    let Some(start) = tag.find(&needle) else {
        return tag.to_owned();
    };
    let value_start = start + needle.len();
    let Some(end) = tag[value_start..].find('"') else {
        return tag.to_owned();
    };
    format!("{}{}", &tag[..start], &tag[value_start + end + 1..])
}

/// Rewrite the root `<svg>` tag to a square view box centered on the original
/// artwork, with explicit pixel dimensions so usvg never resolves `em` units.
pub(crate) fn normalize_svg(svg: &str) -> String {
    let Some(open) = svg.find("<svg") else {
        return svg.to_owned();
    };
    let Some(close) = svg[open..].find('>').map(|index| index + open) else {
        return svg.to_owned();
    };
    let tag = &svg[open..close];
    let numbers = attribute(tag, "viewBox")
        .map(|value| {
            value
                .split(|c: char| c.is_whitespace() || c == ',')
                .filter(|part| !part.is_empty())
                .filter_map(|part| part.parse::<f64>().ok())
                .collect::<Vec<_>>()
        })
        .filter(|numbers| numbers.len() == 4)
        .unwrap_or_else(|| vec![0.0, 0.0, 24.0, 24.0]);
    let (min_x, min_y, width, height) = (numbers[0], numbers[1], numbers[2], numbers[3]);
    let side = width.max(height).max(f64::EPSILON);
    let x = min_x - (side - width) / 2.0;
    let y = min_y - (side - height) / 2.0;
    let mut rewritten = tag.to_owned();
    for name in ["viewBox", "width", "height"] {
        rewritten = strip_attribute(&rewritten, name);
    }
    format!(
        "{}{} width=\"{side}\" height=\"{side}\" viewBox=\"{x} {y} {side} {side}\"{}",
        &svg[..open],
        rewritten,
        &svg[close..]
    )
}

/// Full-color assets painted with `img` (their own colors are kept).
fn color_source(name: &str) -> Option<&'static [u8]> {
    Some(match name {
        "alert-badge-24" => include_bytes!("../../../assets/icons/fluent-color-alert-badge-24.svg"),
        "apps-24" => include_bytes!("../../../assets/icons/fluent-color-apps-24.svg"),
        "apps-list-24" => include_bytes!("../../../assets/icons/fluent-color-apps-list-24.svg"),
        "book-open-24" => include_bytes!("../../../assets/icons/fluent-color-book-open-24.svg"),
        "calendar-clock-24" => {
            include_bytes!("../../../assets/icons/fluent-color-calendar-clock-24.svg")
        }
        "chat-24" => include_bytes!("../../../assets/icons/fluent-color-chat-24.svg"),
        "history-24" => include_bytes!("../../../assets/icons/fluent-color-history-24.svg"),
        "home-24" => include_bytes!("../../../assets/icons/fluent-color-home-24.svg"),
        "paint-brush-24" => include_bytes!("../../../assets/icons/fluent-color-paint-brush-24.svg"),
        "puzzle-piece-24" => {
            include_bytes!("../../../assets/icons/fluent-color-puzzle-piece-24.svg")
        }
        "settings-24" => include_bytes!("../../../assets/icons/fluent-color-settings-24.svg"),
        "app-icon" => include_bytes!("../../../assets/app-icon.png"),
        "app-icon-32" => include_bytes!("../../../assets/icons/app-icon-32.png"),
        _ => return None,
    })
}

/// Whether `path` resolves to an embedded asset.
#[cfg(test)]
pub(crate) fn has_asset(path: &str) -> bool {
    PopupAssets.load(path).ok().flatten().is_some()
}

impl AssetSource for PopupAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if let Some(name) = path.strip_prefix("color/").and_then(|rest| {
            rest.strip_suffix(".svg")
                .or_else(|| rest.strip_suffix(".png"))
        }) {
            return Ok(color_source(name).map(Cow::Borrowed));
        }
        let Some(name) = path
            .strip_prefix("icons/")
            .and_then(|rest| rest.strip_suffix(".svg"))
        else {
            return Ok(None);
        };
        Ok(source(name).map(|svg| Cow::Owned(normalize_svg(svg).into_bytes())))
    }

    fn list(&self, _path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tall_marks_are_centered_in_a_square_view_box() {
        let normalized = normalize_svg(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="0.83em" height="1em" viewBox="0 0 256 312"><path d="M0 0"/></svg>"#,
        );
        assert!(normalized.contains(r#"viewBox="-28 0 312 312""#));
        assert!(normalized.contains(r#"width="312""#));
        assert!(!normalized.contains("em\""));
        assert!(normalized.ends_with("<path d=\"M0 0\"/></svg>"));
    }

    #[test]
    fn every_popup_icon_is_embedded() {
        for name in [
            "fluent-refresh",
            "fluent-arrow-download",
            "fluent-settings",
            "fluent-delete",
            "fluent-drag",
            "fluent-chevron-down",
            "fluent-chart",
            "fluent-home",
            "fluent-error-circle",
            "fluent-warning",
        ] {
            assert!(source(name).is_some(), "{name}");
        }
        for descriptor in crate::provider_registry::PROVIDERS {
            assert!(source(descriptor.icon).is_some(), "{}", descriptor.icon);
        }
        assert!(source("chatgpt").is_some());
    }
}
