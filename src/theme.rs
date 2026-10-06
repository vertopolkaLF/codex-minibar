//! Shared animation tokens for both WinUI surfaces.

use std::{
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
    time::Duration,
};

static APP_ANIMATIONS_ENABLED: AtomicBool = AtomicBool::new(true);
static CURRENT_ACCENT_RGB: AtomicU32 = AtomicU32::new(0x0078D4);

/// WinUI `ControlFasterAnimationDuration` — pointer-over / micro-interactions.
pub const CONTROL_FASTER_ANIMATION: Duration = Duration::from_millis(83);
/// WinUI `ControlFastAnimationDuration`.
pub const CONTROL_FAST_ANIMATION: Duration = Duration::from_millis(167);
/// WinUI `ControlNormalAnimationDuration`.
pub const CONTROL_NORMAL_ANIMATION: Duration = Duration::from_millis(250);

pub fn set_animations_enabled(enabled: bool) {
    APP_ANIMATIONS_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn animations_enabled() -> bool {
    APP_ANIMATIONS_ENABLED.load(Ordering::Relaxed) && crate::popup::system_animations_enabled()
}

pub fn duration(duration: Duration) -> Duration {
    if animations_enabled() {
        duration
    } else {
        Duration::ZERO
    }
}

pub fn current_accent_rgb() -> [u8; 3] {
    let rgb = CURRENT_ACCENT_RGB.load(Ordering::Relaxed);
    [
        ((rgb >> 16) & 0xff) as u8,
        ((rgb >> 8) & 0xff) as u8,
        (rgb & 0xff) as u8,
    ]
}

fn remember_accent((red, green, blue): (u8, u8, u8)) {
    CURRENT_ACCENT_RGB.store(
        (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue),
        Ordering::Relaxed,
    );
}

pub fn apply_appearance(theme: crate::settings::AppTheme, accent: crate::settings::AccentColor) {
    let requested = match theme {
        crate::settings::AppTheme::Auto => windows_reactor::RequestedTheme::Default,
        crate::settings::AppTheme::Light => windows_reactor::RequestedTheme::Light,
        crate::settings::AppTheme::Dark => windows_reactor::RequestedTheme::Dark,
    };
    windows_reactor::set_requested_theme(requested);

    let result = match accent.rgb() {
        Some(color) => {
            remember_accent(color);
            windows_reactor::set_accent_color(color)
        }
        None => system_accent_palette().and_then(|palette| {
            remember_accent(palette.base);
            windows_reactor::set_accent_palette(palette)
        }),
    };
    if let Err(error) = result {
        eprintln!("failed to apply accent color: {error:?}");
    }
}

/// Accent ramp shared by both UI stacks. GPUI uses it directly; WinUI
/// receives the same values through [`windows_reactor::AccentPalette`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccentRamp {
    pub base: (u8, u8, u8),
    pub light1: (u8, u8, u8),
    pub light2: (u8, u8, u8),
    pub light3: (u8, u8, u8),
    pub dark1: (u8, u8, u8),
    pub dark2: (u8, u8, u8),
    pub dark3: (u8, u8, u8),
}

impl AccentRamp {
    /// Windows-style ramp by mixing the accent toward white or black, the
    /// same derivation `windows_reactor` applies for a fixed accent color.
    pub fn from_base(base: (u8, u8, u8)) -> Self {
        let palette = windows_reactor::AccentPalette::from_base(base);
        Self::from(palette)
    }

    /// Fill role (`AccentFillColorDefaultBrush`) for the resolved theme.
    pub const fn fill(self, dark: bool) -> (u8, u8, u8) {
        if dark { self.light2 } else { self.dark1 }
    }

    /// Text role (`AccentTextFillColorPrimaryBrush`) for the resolved theme.
    pub const fn text(self, dark: bool) -> (u8, u8, u8) {
        if dark { self.light3 } else { self.dark2 }
    }
}

impl From<windows_reactor::AccentPalette> for AccentRamp {
    fn from(palette: windows_reactor::AccentPalette) -> Self {
        Self {
            base: palette.base,
            light1: palette.light1,
            light2: palette.light2,
            light3: palette.light3,
            dark1: palette.dark1,
            dark2: palette.dark2,
            dark3: palette.dark3,
        }
    }
}

/// Resolve the configured accent without touching any UI framework state.
pub fn accent_ramp(accent: crate::settings::AccentColor) -> AccentRamp {
    match accent.rgb() {
        Some(color) => AccentRamp::from_base(color),
        None => system_accent_palette()
            .map(AccentRamp::from)
            .unwrap_or_else(|_| AccentRamp::from_base((0, 120, 212))),
    }
}

#[cfg(windows)]
fn system_accent_palette() -> windows_core::Result<windows_reactor::AccentPalette> {
    use windows::UI::ViewManagement::{UIColorType, UISettings};

    let settings = UISettings::new()?;
    let rgb = |kind| {
        settings
            .GetColorValue(kind)
            .map(|color| (color.R, color.G, color.B))
    };

    Ok(windows_reactor::AccentPalette {
        base: rgb(UIColorType::Accent)?,
        light1: rgb(UIColorType::AccentLight1)?,
        light2: rgb(UIColorType::AccentLight2)?,
        light3: rgb(UIColorType::AccentLight3)?,
        dark1: rgb(UIColorType::AccentDark1)?,
        dark2: rgb(UIColorType::AccentDark2)?,
        dark3: rgb(UIColorType::AccentDark3)?,
    })
}

#[cfg(not(windows))]
fn system_accent_palette() -> windows_core::Result<windows_reactor::AccentPalette> {
    Ok(windows_reactor::AccentPalette::from_base((0, 120, 212)))
}
