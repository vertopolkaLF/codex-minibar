//! Color helpers shared by the popup palette and the Settings theme.

use gpui::{Hsla, Rgba, Window, WindowAppearance};

use crate::providers::registry::ProviderKind;
use crate::settings::AppTheme;

pub(crate) fn rgba8(r: u8, g: u8, b: u8, a: u8) -> Hsla {
    Rgba {
        r: f32::from(r) / 255.0,
        g: f32::from(g) / 255.0,
        b: f32::from(b) / 255.0,
        a: f32::from(a) / 255.0,
    }
    .into()
}

pub(crate) fn rgb8((r, g, b): (u8, u8, u8)) -> Hsla {
    rgba8(r, g, b, 255)
}

/// Black/white with a WinUI-style alpha byte.
pub(crate) fn ink(dark: bool, alpha: u8) -> Hsla {
    if dark {
        rgba8(255, 255, 255, alpha)
    } else {
        rgba8(0, 0, 0, alpha)
    }
}

/// Brand tint of a provider mark on a light or dark surface.
pub(crate) fn brand(provider: ProviderKind, dark: bool) -> Hsla {
    rgb8(
        crate::providers::registry::descriptor(provider)
            .icon_rgb
            .pick(dark),
    )
}

/// Whether the system paints `window` dark.
pub(crate) fn system_dark(window: &Window) -> bool {
    appearance_dark(window.appearance())
}

pub(crate) fn appearance_dark(appearance: WindowAppearance) -> bool {
    matches!(
        appearance,
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    )
}

/// Resolve Auto against the window's system appearance.
pub(crate) fn resolve_dark(theme: AppTheme, system_dark: bool) -> bool {
    match theme {
        AppTheme::Auto => system_dark,
        AppTheme::Light => false,
        AppTheme::Dark => true,
    }
}

pub(crate) trait HslaExt {
    fn alpha(self, alpha: f32) -> Hsla;
    fn mix(self, other: Hsla, amount: f32) -> Hsla;
}

impl HslaExt for Hsla {
    fn alpha(mut self, alpha: f32) -> Hsla {
        self.a *= alpha.clamp(0.0, 1.0);
        self
    }

    /// Linear blend in RGB space; used for hover color crossfades.
    fn mix(self, other: Hsla, amount: f32) -> Hsla {
        let t = amount.clamp(0.0, 1.0);
        if t <= 0.0 {
            return self;
        }
        if t >= 1.0 {
            return other;
        }
        let a = self.to_rgb();
        let b = other.to_rgb();
        Rgba {
            r: a.r + (b.r - a.r) * t,
            g: a.g + (b.g - a.g) * t,
            b: a.b + (b.b - a.b) * t,
            a: a.a + (b.a - a.a) * t,
        }
        .into()
    }
}
