//! PowerToys Command Palette extension support.
//!
//! The extension (`cmdpal/`) is a thin client like the Stream Deck plugin: it
//! reads ready-to-show text from the local widget bridge and asks Minibar to
//! open the popup or refresh. This module resolves Dock bands and provider
//! summaries for it, writes the provider marks it shows, and installs the
//! unsigned MSIX package.

use std::{
    fs,
    path::PathBuf,
    sync::Mutex,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::Utc;
use serde::Serialize;

use crate::{
    instances::ProviderId,
    limits::{ProviderLimits, RateLimits},
    settings::{Settings, TaskbarResetDisplay, TaskbarWidget},
    taskbar::model::{self, Appearance, Row, WidgetView},
};

/// Identity name in `cmdpal/CodexMinibar.CommandPalette/Package.appxmanifest`.
pub const PACKAGE_NAME: &str = "CodexMinibar.CommandPalette";
/// `PACKAGE_NAME` plus the publisher hash of the unsigned identity.
pub const PACKAGE_FAMILY: &str = "CodexMinibar.CommandPalette_3pr41ftjnk01m";
const PACKAGE_DOWNLOAD_BASE: &str =
    "https://raw.githubusercontent.com/vertopolkaLF/codex-minibar/main/data";
/// Installs this package instead of the published one (development).
const PACKAGE_OVERRIDE_ENV: &str = "CODEX_MINIBAR_CMDPAL_MSIX";
const ICON_PIXELS: u32 = 64;
const SEPARATOR: &str = " \u{00b7} ";

/// Settings the extension reads, published by the bridge thread.
struct Published {
    widgets: Vec<TaskbarWidget>,
    colored_icons: bool,
}

static PUBLISHED: Mutex<Option<Published>> = Mutex::new(None);

/// Publishes the latest settings; the next bridge request reads them.
pub fn publish_settings(settings: &Settings) {
    if let Ok(mut published) = PUBLISHED.lock() {
        *published = Some(Published {
            widgets: settings.command_palette.widgets.clone(),
            colored_icons: settings.use_colored_provider_icons,
        });
    }
}

/// A provider mark for each theme, as PNG paths the palette can load.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct IconPair {
    pub light: String,
    pub dark: String,
}

/// One Dock band: a Glance widget written as the Dock's two text lines.
#[derive(Debug, Serialize)]
pub struct Band {
    pub id: String,
    /// The instance a click opens.
    pub provider: Option<String>,
    /// How the band is listed in the Dock settings.
    pub name: String,
    pub title: String,
    pub subtitle: String,
    pub icon: Option<IconPair>,
}

/// One quota of a provider, for the palette's lists.
#[derive(Debug, Serialize)]
pub struct Quota {
    pub label: String,
    pub short_label: String,
    /// `46%`, remaining or used as the user chose for the tray.
    pub value: String,
    /// `46% left` / `46% used`.
    pub value_text: String,
    pub progress: Option<f32>,
    pub reset_in: Option<String>,
    pub reset_at: Option<String>,
}

/// Everything the palette shows about one enabled instance.
#[derive(Debug, Serialize)]
pub struct ProviderSummary {
    pub id: String,
    pub kind: String,
    pub name: String,
    /// Words the palette also matches: the driver and instance names.
    pub keywords: Vec<String>,
    pub plan: Option<String>,
    /// The tray's default quotas on one line: `5h 46% · 7d 25%`.
    pub headline: String,
    pub quotas: Vec<Quota>,
    pub today: Option<String>,
    pub icon: Option<IconPair>,
}

#[derive(Debug, Serialize)]
pub struct Payload {
    pub bands: Vec<Band>,
    pub providers: Vec<ProviderSummary>,
}

/// Bands and summaries for the current limits and published settings.
pub fn payload(limits: &ProviderLimits) -> Payload {
    let (widgets, colored_icons) = PUBLISHED
        .lock()
        .ok()
        .and_then(|published| {
            published
                .as_ref()
                .map(|published| (published.widgets.clone(), published.colored_icons))
        })
        .unwrap_or_else(|| (Vec::new(), true));
    let accent = crate::theme::current_accent_rgb();
    let appearance = |light| Appearance {
        light,
        colored_icons,
        accent,
    };
    let icons = |view_light: &WidgetView, view_dark: &WidgetView| {
        Some(IconPair {
            light: icon_file(view_light.icon.as_ref()?)?,
            dark: icon_file(view_dark.icon.as_ref()?)?,
        })
    };
    let enabled = crate::instances::published_enabled_providers();
    let bands = widgets
        .iter()
        // Like the taskbar, a band shows while any of its instances is on.
        .filter(|widget| {
            widget
                .indicators
                .iter()
                .filter_map(crate::settings::TrayIndicator::provider)
                .any(|provider| enabled.contains(&provider))
        })
        .filter_map(|widget| {
            let light = model::resolve(widget, limits, appearance(true));
            let dark = model::resolve(widget, limits, appearance(false));
            let (title, subtitle) = band_lines(widget, &dark, limits);
            if title.is_empty() && subtitle.is_empty() {
                return None;
            }
            Some(Band {
                id: widget.id.clone(),
                provider: dark.provider.map(|provider| provider.id().to_owned()),
                name: band_name(widget),
                title,
                subtitle,
                icon: icons(&light, &dark),
            })
        })
        .collect();
    let providers = enabled
        .into_iter()
        .filter(|provider| {
            !crate::provider_registry::descriptor(provider.kind())
                .default_tray_metrics
                .is_empty()
        })
        .filter_map(|provider| {
            let widget = TaskbarWidget {
                reset: TaskbarResetDisplay::Countdown,
                ..TaskbarWidget::for_provider(provider)
            };
            let light = model::resolve(&widget, limits, appearance(true));
            let dark = model::resolve(&widget, limits, appearance(false));
            let icon = icons(&light, &dark);
            if crate::provider_registry::descriptor(provider.kind()).balance_only {
                return Some(balance_summary(provider, limits.get(provider), icon));
            }
            summary(provider, &dark, icon)
        })
        .collect();
    Payload { bands, providers }
}

/// `Minibar · Claude` / `Minibar · Codex, Cursor`.
pub(crate) fn band_name(widget: &TaskbarWidget) -> String {
    let mut names: Vec<String> = Vec::new();
    for provider in widget
        .indicators
        .iter()
        .filter_map(crate::settings::TrayIndicator::provider)
    {
        let name = provider.qualified_name();
        if !names.contains(&name) {
            names.push(name);
        }
    }
    if names.is_empty() {
        "Minibar".to_owned()
    } else {
        format!("Minibar{SEPARATOR}{}", names.join(", "))
    }
}

/// The Dock's title and subtitle for a band. A band of only balance-only
/// instances shows their balance.
pub(crate) fn band_lines(
    widget: &TaskbarWidget,
    view: &WidgetView,
    limits: &ProviderLimits,
) -> (String, String) {
    let mut providers = widget
        .indicators
        .iter()
        .filter_map(crate::settings::TrayIndicator::provider)
        .peekable();
    if providers.peek().is_none()
        || !providers
            .clone()
            .all(|provider| crate::provider_registry::descriptor(provider.kind()).balance_only)
    {
        return band_text(view);
    }
    let mut balances = Vec::new();
    for provider in providers {
        if !balances.iter().any(|(seen, _)| *seen == provider) {
            balances.push((provider, openrouter_balance(limits.get(provider))));
        }
    }
    let total = balances
        .iter()
        .map(|(_, balance)| *balance)
        .try_fold(0_u64, |total, balance| Some(total.saturating_add(balance?)));
    (
        total.map_or_else(|| "\u{2014}".to_owned(), format_balance),
        crate::i18n::tr("available-balance").to_owned(),
    )
}

/// The account balance of an OpenRouter instance, in micro-dollars.
fn openrouter_balance(limits: &RateLimits) -> Option<u64> {
    limits
        .spending
        .as_ref()
        .and_then(|spending| spending.balance_microusd)
        .or_else(|| {
            limits
                .openrouter_accounts
                .iter()
                .filter_map(|account| account.balance_microusd)
                .reduce(u64::saturating_add)
        })
}

fn format_balance(microusd: u64) -> String {
    crate::format::format_micro_usd(microusd)
}

/// The Dock's title and subtitle for a Glance, line for line as the taskbar
/// draws it: one quota as a headline over `label · reset`, several as one
/// line each.
pub(crate) fn band_text(view: &WidgetView) -> (String, String) {
    match view.rows.as_slice() {
        [] => (String::new(), String::new()),
        [row] => headline(view, row),
        [first, second, ..] => (row_line(view, first), row_line(view, second)),
    }
}

fn row_line(view: &WidgetView, row: &Row) -> String {
    let mut parts = Vec::with_capacity(3);
    if view.show_labels {
        parts.push(row.label.clone());
    }
    if view.show_values || (parts.is_empty() && row.reset.is_none()) {
        parts.push(row.value.clone());
    }
    parts.extend(row.reset.clone());
    parts.join(" ")
}

fn headline(view: &WidgetView, row: &Row) -> (String, String) {
    let label = view.show_labels.then(|| row.label.clone());
    let mut caption = Vec::new();
    let title = if view.show_values {
        caption.extend(label);
        caption.extend(row.reset.clone());
        row.value.clone()
    } else if let Some(reset) = &row.reset {
        caption.extend(label);
        reset.clone()
    } else if let Some(label) = label {
        label
    } else {
        row.value.clone()
    };
    (title, caption.join(SEPARATOR))
}

fn summary(
    provider: ProviderId,
    view: &WidgetView,
    icon: Option<IconPair>,
) -> Option<ProviderSummary> {
    let section = view
        .card
        .iter()
        .find(|section| section.provider == provider)?;
    let now = Utc::now();
    let quotas = section
        .rows
        .iter()
        .map(|row| {
            let reset = row.resets_at.filter(|at| *at > now);
            Quota {
                label: row.full_label.clone(),
                short_label: row.label.clone(),
                value: row.value.clone(),
                value_text: if row.progress.is_none() {
                    crate::i18n::tr("taskbar-card-no-data").to_owned()
                } else {
                    crate::i18n::format(
                        match row.limit_value {
                            crate::settings::LimitValue::Used => "taskbar-card-used",
                            crate::settings::LimitValue::Remaining => "taskbar-card-left",
                        },
                        &[("value", row.value.clone())],
                    )
                },
                progress: row.progress,
                reset_in: reset.map(|at| crate::format::countdown(at, now)),
                reset_at: reset.map(model::reset_clock),
            }
        })
        .collect();
    let headline = view
        .rows
        .iter()
        .map(|row| format!("{} {}", row.label, row.value))
        .collect::<Vec<_>>()
        .join(SEPARATOR);
    Some(ProviderSummary {
        id: provider.id().to_owned(),
        kind: provider.kind().id().to_owned(),
        name: provider.qualified_name(),
        keywords: keywords(provider),
        plan: section.plan.clone(),
        headline,
        quotas,
        today: section.today.clone(),
        icon,
    })
}

/// OpenRouter's summary is its account balance alone.
fn balance_summary(
    provider: ProviderId,
    limits: &RateLimits,
    icon: Option<IconPair>,
) -> ProviderSummary {
    let label = crate::i18n::tr("available-balance").to_owned();
    let value = openrouter_balance(limits).map_or_else(|| "\u{2014}".to_owned(), format_balance);
    ProviderSummary {
        id: provider.id().to_owned(),
        kind: provider.kind().id().to_owned(),
        name: provider.qualified_name(),
        keywords: keywords(provider),
        plan: None,
        headline: format!("{label} {value}"),
        quotas: vec![Quota {
            short_label: label.clone(),
            label,
            value_text: value.clone(),
            value,
            progress: None,
            reset_in: None,
            reset_at: None,
        }],
        today: None,
        icon,
    }
}

fn keywords(provider: ProviderId) -> Vec<String> {
    let kind = provider.kind();
    let mut keywords = vec![
        kind.display_name().to_owned(),
        kind.id().to_owned(),
        provider.display_name(),
    ];
    keywords.dedup();
    keywords
}

/// A tinted PNG of the mark, written once per name and tint.
fn icon_file(icon: &model::Icon) -> Option<String> {
    let [red, green, blue] = icon.tint;
    let directory = crate::app_paths::command_palette_icons().ok()?;
    let path = directory.join(format!("{}-{red:02x}{green:02x}{blue:02x}.png", icon.name));
    if !path.exists() {
        let png = rasterize_icon(icon.name, ICON_PIXELS, icon.tint)?;
        fs::create_dir_all(&directory).ok()?;
        // Write beside, then move, so the palette never reads half a file.
        let partial = path.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&partial, png).ok()?;
        if fs::rename(&partial, &path).is_err() {
            let _ = fs::remove_file(&partial);
        }
    }
    Some(path.to_string_lossy().into_owned())
}

/// Straight-alpha RGBA PNG of a monochrome mark in `tint`.
fn rasterize_icon(name: &str, pixels: u32, tint: model::Rgb) -> Option<Vec<u8>> {
    use resvg::{tiny_skia, usvg};
    let source = crate::popup_window::ui::assets::svg_source(name)?;
    let svg = crate::popup_window::ui::assets::normalize_svg(source);
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).ok()?;
    let size = tree.size();
    let scale = pixels as f32 / size.width().max(size.height());
    let mut pixmap = tiny_skia::Pixmap::new(pixels, pixels)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let rgba = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| [tint[0], tint[1], tint[2], pixel.alpha()])
        .collect::<Vec<_>>();
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, pixels, pixels);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&rgba).ok()?;
    }
    Some(png)
}

// ----- installer -------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallPhase {
    Idle,
    Downloading,
    /// Waiting for the elevated installer (UAC).
    Installing,
    Installed,
    Removing,
    Removed,
    Failed(String),
}

/// Whether the extension package is registered for this user.
#[cfg(windows)]
pub fn is_installed() -> bool {
    use windows_sys::Win32::Storage::Packaging::Appx::GetPackagesByPackageFamily;
    let family = crate::win32::wide(PACKAGE_FAMILY);
    let mut count = 0u32;
    let mut buffer_length = 0u32;
    // SAFETY: null buffers ask only for the counts.
    let status = unsafe {
        GetPackagesByPackageFamily(
            family.as_ptr(),
            &mut count,
            std::ptr::null_mut(),
            &mut buffer_length,
            std::ptr::null_mut(),
        )
    };
    // ERROR_INSUFFICIENT_BUFFER with a count means at least one package.
    (status == 0 || status == 122) && count > 0
}

#[cfg(not(windows))]
pub fn is_installed() -> bool {
    false
}

fn package_url() -> String {
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        _ => "x64",
    };
    format!("{PACKAGE_DOWNLOAD_BASE}/{PACKAGE_NAME}_{arch}.msix")
}

fn download_package() -> anyhow::Result<PathBuf> {
    if let Some(path) = std::env::var_os(PACKAGE_OVERRIDE_ENV).map(PathBuf::from) {
        anyhow::ensure!(path.is_file(), "{} does not exist", path.display());
        return Ok(path);
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let directory = std::env::temp_dir().join("Codex Minibar");
    fs::create_dir_all(&directory)?;
    let path = directory.join(format!("{PACKAGE_NAME}-{stamp}.msix"));
    crate::net::download(&package_url(), &path)?;
    Ok(path)
}

/// Downloads the package and registers it through an elevated PowerShell:
/// unsigned packages with executable content install for all users.
pub fn install_async(on_phase: impl Fn(InstallPhase) + Send + 'static) {
    on_phase(InstallPhase::Downloading);
    thread::spawn(move || {
        let result = download_package().and_then(|path| {
            on_phase(InstallPhase::Installing);
            let script = format!(
                "Add-AppxPackage -Path '{}' -AllowUnsigned -ForceApplicationShutdown \
                 -ForceUpdateFromAnyVersion",
                path.to_string_lossy().replace('\'', "''")
            );
            run_elevated_powershell(&script)
        });
        on_phase(match result {
            Ok(()) if is_installed() => InstallPhase::Installed,
            Ok(()) => InstallPhase::Failed("the package was not registered".into()),
            Err(error) => {
                crate::logger::info(format!(
                    "Command Palette extension install failed: {error:#}"
                ));
                InstallPhase::Failed(error.to_string())
            }
        });
    });
}

/// Unregisters the package for every user, as it was installed.
pub fn uninstall_async(on_phase: impl Fn(InstallPhase) + Send + 'static) {
    on_phase(InstallPhase::Removing);
    thread::spawn(move || {
        let script = format!(
            "Get-AppxPackage -AllUsers -Name '{PACKAGE_NAME}' | Remove-AppxPackage -AllUsers"
        );
        on_phase(match run_elevated_powershell(&script) {
            Ok(()) if !is_installed() => InstallPhase::Removed,
            Ok(()) => InstallPhase::Failed("the package is still registered".into()),
            Err(error) => InstallPhase::Failed(error.to_string()),
        });
    });
}

#[cfg(windows)]
fn run_elevated_powershell(script: &str) -> anyhow::Result<()> {
    use crate::win32::wide;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, WAIT_OBJECT_0},
        System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject},
        UI::{
            Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW},
            WindowsAndMessaging::SW_HIDE,
        },
    };
    // A failing cmdlet must set the exit code the caller reads.
    let wrapped = format!("$ErrorActionPreference = 'Stop'; try {{ {script} }} catch {{ exit 1 }}");
    let encoded = {
        use base64::Engine;
        let utf16le = wrapped
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<u8>>();
        base64::engine::general_purpose::STANDARD.encode(utf16le)
    };
    let verb = wide("runas");
    let file = wide("powershell.exe");
    let parameters = wide(format!(
        "-NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand {encoded}"
    ));
    // SAFETY: zeroed is a valid empty SHELLEXECUTEINFOW; every pointer set
    // below outlives the call.
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = parameters.as_ptr();
    info.nShow = SW_HIDE;
    // SAFETY: `info` is fully initialized above.
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        // ERROR_CANCELLED when the user declines the UAC prompt.
        return Err(std::io::Error::last_os_error().into());
    }
    anyhow::ensure!(!info.hProcess.is_null(), "the installer did not start");
    let mut code = 1u32;
    // SAFETY: the handle is owned here and closed once.
    unsafe {
        if WaitForSingleObject(info.hProcess, INFINITE) == WAIT_OBJECT_0 {
            GetExitCodeProcess(info.hProcess, &mut code);
        }
        CloseHandle(info.hProcess);
    }
    anyhow::ensure!(code == 0, "PowerShell exited with code {code}");
    Ok(())
}

#[cfg(not(windows))]
fn run_elevated_powershell(_script: &str) -> anyhow::Result<()> {
    anyhow::bail!("the Command Palette is only available on Windows")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{LimitValue, TaskbarWidgetStyle};
    use crate::taskbar::model::Icon;

    fn row(label: &str, value: &str, reset: Option<&str>) -> Row {
        Row {
            provider: ProviderId::primary(crate::provider_registry::ProviderKind::Codex),
            label: label.into(),
            full_label: label.into(),
            value: value.into(),
            progress: Some(0.5),
            pace: None,
            color: [0, 0, 0],
            tinted: false,
            resets_at: None,
            icon: Icon {
                name: "codex",
                badge: None,
                tint: [0, 0, 0],
            },
            limit_value: LimitValue::Remaining,
            reset: reset.map(str::to_owned),
        }
    }

    fn view(rows: Vec<Row>, labels: bool, values: bool) -> WidgetView {
        WidgetView {
            id: "dock".into(),
            style: TaskbarWidgetStyle::Glance,
            icon: None,
            provider: None,
            rows,
            show_labels: labels,
            show_values: values,
            show_pace: false,
            card: Vec::new(),
        }
    }

    #[test]
    fn one_quota_reads_as_a_headline_over_its_caption() {
        let one = vec![row("5h", "46%", Some("18m"))];
        assert_eq!(
            band_text(&view(one.clone(), true, true)),
            ("46%".into(), "5h \u{00b7} 18m".into())
        );
        assert_eq!(
            band_text(&view(one.clone(), true, false)),
            ("18m".into(), "5h".into())
        );
        assert_eq!(
            band_text(&view(vec![row("5h", "46%", None)], false, false)),
            ("46%".into(), String::new())
        );
    }

    #[test]
    fn two_quotas_take_a_line_each() {
        let rows = vec![row("5h", "46%", Some("18m")), row("7d", "25%", None)];
        assert_eq!(
            band_text(&view(rows.clone(), true, true)),
            ("5h 46% 18m".into(), "7d 25%".into())
        );
        assert_eq!(
            band_text(&view(rows, false, false)),
            ("18m".into(), "25%".into())
        );
    }
}
