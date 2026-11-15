//! Embedded assets for the GPUI windows.
//!
//! GPUI paints SVGs as alpha masks tinted by `text_color`, so the Iconify
//! sources from [`crate::ui::icons`] are used as-is for their geometry.

use std::borrow::Cow;

use gpui::{AssetSource, SharedString};

use crate::ui::icons::{normalize_svg, svg_source};

pub(crate) struct Assets;

/// Asset path for a popup icon name (the same names `crate::ui::icons` uses).
pub(crate) fn icon_path(name: &str) -> SharedString {
    SharedString::from(format!("icons/{name}.svg"))
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
        "gauge-24" => include_bytes!("../../../assets/icons/fluent-color-gauge-24.svg"),
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
    Assets.load(path).ok().flatten().is_some()
}

impl AssetSource for Assets {
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
        Ok(svg_source(name).map(|svg| Cow::Owned(normalize_svg(svg).into_bytes())))
    }

    fn list(&self, _path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}
