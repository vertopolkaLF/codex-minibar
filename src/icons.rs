//! Icon geometry from Iconify (Fluent for chrome actions, Fluent Color for
//! settings navigation, Phosphor for the remaining settings content).

use windows_reactor::*;

/// Path data plus the SVG design canvas so WinUI Viewbox keeps intended padding.
pub struct IconGeom {
    pub path: &'static str,
    pub canvas: f64,
}

pub fn geom(name: &str) -> IconGeom {
    let svg = match name {
        // Fluent glyphs are used for popup chrome and compact controls.
        "fluent-refresh" | "arrows-clockwise" | "popup-refresh" => {
            include_str!("../assets/icons/fluent-arrow-sync-24-filled.svg")
        }
        "fluent-settings" | "sliders" | "popup-settings" => {
            include_str!("../assets/icons/fluent-settings-20-filled.svg")
        }
        "fluent-power" | "power" | "popup-power" => {
            include_str!("../assets/icons/fluent-power-20-filled.svg")
        }
        "fluent-delete" => include_str!("../assets/icons/fluent-delete-20-regular.svg"),
        "fluent-drag" => {
            include_str!("../assets/icons/fluent-re-order-dots-vertical-16-filled.svg")
        }
        "fluent-folder" => include_str!("../assets/icons/fluent-folder-16-filled.svg"),
        "fluent-chart" => {
            include_str!("../assets/icons/fluent-data-histogram-24-filled.svg")
        }
        "fluent-home" => include_str!("../assets/icons/fluent-home-24-filled.svg"),

        // Provider marks are local SVG assets. They are rendered as
        // monochrome paths so they remain legible in either app theme.
        "codex" => include_str!("../assets/icons/openai-iconify.svg"),
        "claude" => include_str!("../assets/icons/claude-iconify.svg"),
        "cursor" => include_str!("../assets/icons/cursor-iconify.svg"),
        "opencode" => include_str!("../assets/icons/opencode-iconify.svg"),
        "openrouter" => include_str!("../assets/icons/openrouter-iconify.svg"),
        // Reserved for the ChatGPT provider when it is added to ProviderKind.
        "chatgpt" => include_str!("../assets/icons/chatgpt-iconify.svg"),
        "chat-centered-text" => include_str!("../assets/icons/ph-chat-centered-text-fill.svg"),
        "download-simple" => include_str!("../assets/icons/ph-download-simple-fill.svg"),
        "plugs-connected" => include_str!("../assets/icons/ph-plugs-connected-fill.svg"),
        "clock" => include_str!("../assets/icons/ph-clock-fill.svg"),
        "scroll" => include_str!("../assets/icons/ph-scroll-fill.svg"),
        "caret-down" => include_str!("../assets/icons/ph-caret-down.svg"),
        "caret-right" => include_str!("../assets/icons/ph-caret-right.svg"),
        "check-circle-fill" => include_str!("../assets/icons/ph-check-circle-fill.svg"),
        "github-logo" => include_str!("../assets/icons/ph-github-logo-fill.svg"),
        "package" => include_str!("../assets/icons/ph-package-fill.svg"),
        "flag" => include_str!("../assets/icons/ph-flag-fill.svg"),
        "at" => include_str!("../assets/icons/ph-at-fill.svg"),
        "house" => include_str!("../assets/icons/ph-house-fill.svg"),
        "squares-four" => include_str!("../assets/icons/ph-squares-four-fill.svg"),
        "browsers" => include_str!("../assets/icons/ph-browsers-fill.svg"),
        "paint-brush" => include_str!("../assets/icons/ph-paint-brush-fill.svg"),
        "bell" => include_str!("../assets/icons/ph-bell-fill.svg"),
        "info" => include_str!("../assets/icons/ph-info-fill.svg"),
        _ => panic!("unknown icon: {name}"),
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

/// Monochrome Phosphor/Fluent path used when Settings sidebar color icons
/// are turned off. Keys match [`fluent_color_uri`].
pub fn sidebar_mono_icon(name: &str) -> &'static str {
    match name {
        "general" => "house",
        "providers" => "plugs-connected",
        "customize" => "squares-four",
        "schedule" => "clock",
        "tray" => "chat-centered-text",
        "widgets" => "browsers",
        "notifications" => "bell",
        "appearance" => "paint-brush",
        "advanced" => "sliders",
        "log" => "scroll",
        "about" => "info",
        _ => panic!("unknown Settings sidebar icon: {name}"),
    }
}

/// Resolve a bundled Fluent UI System Color icon as a file URI.
///
/// The portable package keeps `assets` beside the executable, while local
/// development runs from `target` without copying that directory. Support
/// both layouts so the sidebar stays visible in either workflow.
pub fn fluent_color_uri(name: &str) -> String {
    let file_name = match name {
        "general" => "fluent-color-home-24.svg",
        "providers" => "fluent-color-apps-list-24.svg",
        "customize" => "fluent-color-apps-24.svg",
        "schedule" => "fluent-color-calendar-clock-24.svg",
        "tray" => "fluent-color-chat-24.svg",
        "widgets" => "fluent-color-widget-24.svg",
        "notifications" => "fluent-color-alert-badge-24.svg",
        "appearance" => "fluent-color-paint-brush-24.svg",
        "advanced" => "fluent-color-settings-24.svg",
        "log" => "fluent-color-history-24.svg",
        "about" => "fluent-color-book-open-24.svg",
        _ => panic!("unknown Fluent Color navigation icon: {name}"),
    };
    let packaged = std::env::current_exe().ok().and_then(|path| {
        path.parent()
            .map(|parent| parent.join("assets/icons").join(file_name))
    });
    let path = packaged.filter(|path| path.exists()).unwrap_or_else(|| {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets/icons")
            .join(file_name)
    });
    format!("file:///{}", path.to_string_lossy().replace('\\', "/"))
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
            eprintln!("Could not install filled icon: {error:?}");
        }
    }));
    host.unmounted = Some(Callback::new(move |native: Option<_>| {
        if let Some(native) = native {
            let _ = crate::acrylic::clear_children(native);
        }
    }));
    let icon: Element = host.into();
    icon.with_key(format!(
        "filled-{name}-{:02X}{:02X}{:02X}",
        color.r, color.g, color.b
    ))
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
            eprintln!("Could not install accent filled icon: {error:?}");
        }
    }));
    host.unmounted = Some(Callback::new(move |native: Option<_>| {
        if let Some(native) = native {
            let _ = crate::acrylic::clear_children(native);
        }
    }));
    let icon: Element = host.into();
    icon.with_key(format!("filled-{name}-accent"))
}
