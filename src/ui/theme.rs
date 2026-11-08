//! Animation preference and accent shared by every surface: the GPUI
//! windows, the taskbar widget and the tray glyphs.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

static APP_ANIMATIONS_ENABLED: AtomicBool = AtomicBool::new(true);
static CURRENT_ACCENT_RGB: AtomicU32 = AtomicU32::new(0x0078D4);

pub fn set_animations_enabled(enabled: bool) {
    APP_ANIMATIONS_ENABLED.store(enabled, Ordering::Relaxed);
}

/// Whether motion runs: the app's setting and the Windows "Animation
/// effects" accessibility preference.
pub fn animations_enabled() -> bool {
    APP_ANIMATIONS_ENABLED.load(Ordering::Relaxed) && system_animations_enabled()
}

#[cfg(windows)]
fn system_animations_enabled() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
    };
    let mut enabled = 1i32;
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            &mut enabled as *mut i32 as *mut _,
            0,
        )
    };
    ok == 0 || enabled != 0
}

/// The desktop's animation switch (GNOME "Animations", read via the portal).
#[cfg(not(windows))]
fn system_animations_enabled() -> bool {
    crate::platform::linux::desktop::animations_enabled().unwrap_or(true)
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

/// Record the configured accent for surfaces painted outside GPUI (the tray
/// glyphs). The GPUI windows resolve theme and accent from settings on every
/// frame, so nothing else has to be pushed anywhere.
pub fn apply_appearance(accent: crate::settings::AccentColor) {
    remember_accent(accent_ramp(accent).base);
}

/// Accent ramp shared by every UI surface.
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

fn tone((r, g, b): (u8, u8, u8), amount: f64, lighter: bool) -> (u8, u8, u8) {
    let target = if lighter { 255.0 } else { 0.0 };
    let channel = |value: u8| {
        (f64::from(value) + (target - f64::from(value)) * amount)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    (channel(r), channel(g), channel(b))
}

impl AccentRamp {
    /// Ramp for an explicitly chosen color. The fill roles (`light2` in dark,
    /// `dark1` in light) are the chosen color itself, so the swatch, preview
    /// and every accent-filled control show exactly what was picked; the
    /// other steps are mixed toward white or black.
    pub fn from_base(base: (u8, u8, u8)) -> Self {
        Self {
            base,
            light1: tone(base, 0.25, true),
            light2: base,
            light3: tone(base, 0.70, true),
            dark1: base,
            dark2: tone(base, 0.45, false),
            dark3: tone(base, 0.70, false),
        }
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

/// Resolve the configured accent without touching any UI framework state.
pub fn accent_ramp(accent: crate::settings::AccentColor) -> AccentRamp {
    match accent.rgb() {
        Some(color) => AccentRamp::from_base(color),
        None => system_accent_ramp().unwrap_or_else(|| AccentRamp::from_base((0, 120, 212))),
    }
}

#[cfg(windows)]
fn system_accent_ramp() -> Option<AccentRamp> {
    use windows::UI::ViewManagement::{UIColorType, UISettings};

    let settings = UISettings::new().ok()?;
    let rgb = |kind| {
        settings
            .GetColorValue(kind)
            .ok()
            .map(|color| (color.R, color.G, color.B))
    };
    Some(AccentRamp {
        base: rgb(UIColorType::Accent)?,
        light1: rgb(UIColorType::AccentLight1)?,
        light2: rgb(UIColorType::AccentLight2)?,
        light3: rgb(UIColorType::AccentLight3)?,
        dark1: rgb(UIColorType::AccentDark1)?,
        dark2: rgb(UIColorType::AccentDark2)?,
        dark3: rgb(UIColorType::AccentDark3)?,
    })
}

/// The desktop accent from the XDG portal (GNOME 47+, KDE Plasma).
#[cfg(not(windows))]
fn system_accent_ramp() -> Option<AccentRamp> {
    crate::platform::linux::desktop::accent_color().map(AccentRamp::from_base)
}
