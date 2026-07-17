//! Phosphor Regular geometry from Iconify, rendered with caller-selected color.

use windows_reactor::*;

/// Path data plus the SVG design canvas so WinUI Viewbox keeps intended padding.
pub struct IconGeom {
    pub path: &'static str,
    pub canvas: f64,
}

pub fn geom(name: &str) -> IconGeom {
    let svg = match name {
        "fluent-refresh" => include_str!("../assets/icons/fluent-arrow-sync-20-regular.svg"),
        "fluent-settings" => include_str!("../assets/icons/fluent-settings-20-regular.svg"),
        "fluent-power" => include_str!("../assets/icons/fluent-power-20-regular.svg"),
        "fluent-drag" => include_str!("../assets/icons/fluent-line-horizontal-3-16-regular.svg"),

        // Provider marks are sourced from Iconify. They are rendered as
        // monochrome paths so they remain legible in either app theme.
        "codex" => include_str!("../assets/icons/openai-iconify.svg"),
        "claude" => include_str!("../assets/icons/claude-iconify.svg"),
        "cursor" => include_str!("../assets/icons/cursor-iconify.svg"),
        // Reserved for the ChatGPT provider when it is added to ProviderKind.
        "chatgpt" => include_str!("../assets/icons/chatgpt-iconify.svg"),
        "chat-centered-text" => include_str!("../assets/icons/ph-chat-centered-text.svg"),
        "download-simple" => include_str!("../assets/icons/ph-download-simple.svg"),
        "plugs-connected" => include_str!("../assets/icons/ph-plugs-connected.svg"),
        "arrows-clockwise" | "popup-refresh" => include_str!("../assets/icons/ph-arrows-clockwise.svg"),
        "sliders" | "popup-settings" => include_str!("../assets/icons/ph-sliders.svg"),
        "power" | "popup-power" => include_str!("../assets/icons/ph-power.svg"),
        "caret-down" => include_str!("../assets/icons/ph-caret-down.svg"),
        "github-logo" => include_str!("../assets/icons/ph-github-logo.svg"),
        "package" => include_str!("../assets/icons/ph-package.svg"),
        "flag" => include_str!("../assets/icons/ph-flag.svg"),
        "at" => include_str!("../assets/icons/ph-at.svg"),
        "house" => include_str!("../assets/icons/ph-house.svg"),
        "bell" => include_str!("../assets/icons/ph-bell.svg"),
        "info" => include_str!("../assets/icons/ph-info.svg"),
        _ => panic!("unknown Phosphor icon: {name}"),
    };
    let canvas = viewbox_size(svg);
    let start = svg.find(" d=\"").expect("Iconify SVG path") + 4;
    let end = svg[start..].find('"').expect("Iconify SVG path terminator") + start;
    IconGeom {
        path: &svg[start..end],
        canvas,
    }
}

pub fn data(name: &str) -> &'static str {
    geom(name).path
}

fn viewbox_size(svg: &str) -> f64 {
    let start = svg.find("viewBox=\"").expect("SVG viewBox") + 9;
    let end = svg[start..].find('"').expect("SVG viewBox terminator") + start;
    let mut parts = svg[start..end].split_whitespace();
    let _min_x = parts.next();
    let _min_y = parts.next();
    parts
        .next()
        .expect("SVG viewBox width")
        .parse::<f64>()
        .expect("SVG viewBox width number")
}

/// Render an icon at `size` using exactly the supplied color.
///
/// The host is keyed by glyph + tint. Swap-chain painters run only on mount,
/// so any identity change must remount — never rely on in-place updates.
pub fn element(name: &'static str, size: f64, color: Color) -> Element {
    let icon = geom(name);
    let mut host = swap_chain_panel().width(size).height(size);
    host.mounted = Some(Callback::new(move |native: Option<_>| {
        if let Some(native) = native
            && let Err(error) = crate::acrylic::install_colored_icon_into(
                native,
                icon.path,
                icon.canvas,
                (color.r, color.g, color.b),
            )
        {
            eprintln!("Could not install Phosphor icon: {error:?}");
        }
    }));
    let icon: Element = host.into();
    icon.with_key(format!("ph-{name}-{:02X}{:02X}{:02X}", color.r, color.g, color.b))
}

/// Render an icon filled with the live Windows accent theme brush.
///
/// Same mount-only paint rule as [`element`]: key changes must remount the host.
pub fn accent_element(name: &'static str, size: f64) -> Element {
    let icon = geom(name);
    let mut host = swap_chain_panel().width(size).height(size);
    host.mounted = Some(Callback::new(move |native: Option<_>| {
        if let Some(native) = native
            && let Err(error) =
                crate::acrylic::install_accent_icon_into(native, icon.path, icon.canvas)
        {
            eprintln!("Could not install accent Phosphor icon: {error:?}");
        }
    }));
    let icon: Element = host.into();
    icon.with_key(format!("ph-{name}-accent"))
}
