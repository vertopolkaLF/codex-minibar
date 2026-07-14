//! Phosphor Regular geometry from Iconify, rendered with caller-selected color.

use windows_reactor::*;

pub fn data(name: &str) -> &'static str {
    let svg = match name {
        "fluent-refresh" => include_str!("../assets/icons/fluent-arrow-sync-20-regular.svg"),
        "fluent-settings" => include_str!("../assets/icons/fluent-settings-20-regular.svg"),
        "fluent-power" => include_str!("../assets/icons/fluent-power-20-regular.svg"),
        "chat-centered-text" => include_str!("../assets/icons/ph-chat-centered-text.svg"),
        "download-simple" => include_str!("../assets/icons/ph-download-simple.svg"),
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
    let start = svg.find(" d=\"").expect("Iconify SVG path") + 4;
    let end = svg[start..].find('"').expect("Iconify SVG path terminator") + start;
    &svg[start..end]
}

/// Render an icon at `size` using exactly the supplied color.
pub fn element(name: &'static str, size: f64, color: Color) -> Element {
    let path = data(name);
    let mut host = swap_chain_panel().width(size).height(size);
    host.mounted = Some(Callback::new(move |native: Option<_>| {
        if let Some(native) = native
            && let Err(error) = crate::acrylic::install_colored_icon_into(
                native,
                path,
                (color.r, color.g, color.b),
            )
        {
            eprintln!("Could not install Phosphor icon: {error:?}");
        }
    }));
    let icon: Element = host.into();
    icon.with_key(format!("ph-{name}-{:02X}{:02X}{:02X}", color.r, color.g, color.b))
}
