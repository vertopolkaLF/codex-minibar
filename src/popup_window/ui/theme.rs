//! Fluent color tokens for the GPUI popup.
//!
//! Values mirror the WinUI 3 theme dictionaries the WinUI popup used through
//! `ThemeRef`, so cards, strokes and text keep their exact contrast. The
//! accent roles follow the same mapping `windows_reactor` installs for the
//! Settings window (fill = Light2/Dark1, text = Light3/Dark2).

use gpui::{Hsla, Rgba, SharedString};

use crate::settings::{AccentColor, AppTheme, PopupBackgroundMaterial, ProviderKind};

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
fn ink(dark: bool, alpha: u8) -> Hsla {
    if dark {
        rgba8(255, 255, 255, alpha)
    } else {
        rgba8(0, 0, 0, alpha)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Palette {
    pub(crate) dark: bool,
    pub(crate) font_family: SharedString,
    pub(crate) text_primary: Hsla,
    pub(crate) text_secondary: Hsla,
    pub(crate) text_tertiary: Hsla,
    pub(crate) text_on_accent: Hsla,
    pub(crate) accent: Hsla,
    pub(crate) card_background: Hsla,
    pub(crate) card_stroke: Hsla,
    pub(crate) subtle_fill: Hsla,
    pub(crate) control_fill: Hsla,
    pub(crate) divider: Hsla,
    pub(crate) solid_background: Hsla,
    pub(crate) tooltip_background: Hsla,
    pub(crate) footer_background: Hsla,
    pub(crate) critical: Hsla,
    pub(crate) critical_background: Hsla,
    pub(crate) attention_background: Hsla,
    pub(crate) chrome_icon: Hsla,
    pub(crate) chrome_icon_hover: Hsla,
    pub(crate) pace_marker: Hsla,
    pub(crate) interval_tick: Hsla,
    pub(crate) chart_grid: Hsla,
    pub(crate) mono_provider_icon: Hsla,
    pub(crate) material: PopupBackgroundMaterial,
}

impl Palette {
    /// Luminosity controls backdrop brightness independently of tint opacity,
    /// so a lighter tint can preserve wallpaper hue without washing out on white.
    pub(crate) fn capsule_background(&self, frosted: bool) -> Hsla {
        if self.material == PopupBackgroundMaterial::Mica {
            // The experimental native controller crashed the GPUI dispatcher.
            // Keep startup and live material switching safe with a solid fallback.
            return self.solid_background;
        }
        let opacity = if frosted && self.material == PopupBackgroundMaterial::Acrylic {
            if self.dark { 0.35 } else { 0.25 }
        } else if self.dark {
            0.94
        } else {
            0.96
        };
        self.solid_background.opacity(opacity)
    }

    pub(crate) fn new(
        dark: bool,
        accent: crate::theme::AccentRamp,
        material: PopupBackgroundMaterial,
        font_family: SharedString,
    ) -> Self {
        let accent_fill = rgb8(accent.fill(dark));
        Self {
            dark,
            font_family,
            // TextFillColorPrimary/Secondary/Tertiary.
            text_primary: if dark {
                rgba8(255, 255, 255, 0xFF)
            } else {
                rgba8(0, 0, 0, 0xE4)
            },
            text_secondary: if dark {
                rgba8(255, 255, 255, 0xC5)
            } else {
                rgba8(0, 0, 0, 0x9E)
            },
            text_tertiary: if dark {
                rgba8(255, 255, 255, 0x87)
            } else {
                rgba8(0, 0, 0, 0x72)
            },
            // TextOnAccentFillColorPrimary.
            text_on_accent: if dark {
                rgba8(0, 0, 0, 0xFF)
            } else {
                rgba8(255, 255, 255, 0xFF)
            },
            accent: accent_fill,
            // CardBackgroundFillColorDefault / CardStrokeColorDefault.
            card_background: if dark {
                rgba8(255, 255, 255, 0x0D)
            } else {
                rgba8(255, 255, 255, 0xB3)
            },
            card_stroke: if dark {
                rgba8(0, 0, 0, 0x19)
            } else {
                rgba8(0, 0, 0, 0x0F)
            },
            // SubtleFillColorSecondary.
            subtle_fill: ink(dark, if dark { 0x0F } else { 0x09 }),
            // ControlFillColorDefault.
            control_fill: if dark {
                rgba8(255, 255, 255, 0x0F)
            } else {
                rgba8(255, 255, 255, 0xB3)
            },
            // DividerStrokeColorDefault.
            divider: ink(dark, if dark { 0x15 } else { 0x0F }),
            // SolidBackgroundFillColorBase.
            solid_background: if dark {
                rgba8(0x20, 0x20, 0x20, 0xFF)
            } else {
                rgba8(0xF3, 0xF3, 0xF3, 0xFF)
            },
            tooltip_background: if dark {
                rgba8(0x2C, 0x2C, 0x2C, 0xFF)
            } else {
                rgba8(0xF9, 0xF9, 0xF9, 0xFF)
            },
            // Subtle footer tint over the opaque capsule background.
            footer_background: rgba8(0, 0, 0, if dark { 0x24 } else { 0x0D }),
            // SystemFillColorCritical / CriticalBackground / AttentionBackground.
            critical: if dark {
                rgba8(0xFF, 0x99, 0xA4, 0xFF)
            } else {
                rgba8(0xC4, 0x2B, 0x1C, 0xFF)
            },
            critical_background: if dark {
                rgba8(0x44, 0x27, 0x26, 0xFF)
            } else {
                rgba8(0xFD, 0xE7, 0xE9, 0xFF)
            },
            attention_background: if dark {
                rgba8(255, 255, 255, 0x08)
            } else {
                rgba8(0xF6, 0xF6, 0xF6, 0x80)
            },
            chrome_icon: if dark {
                rgb8((190, 190, 190))
            } else {
                rgb8((17, 17, 17))
            },
            chrome_icon_hover: if dark {
                rgb8((230, 230, 230))
            } else {
                rgb8((17, 17, 17))
            },
            pace_marker: if dark {
                rgb8((255, 255, 255))
            } else {
                rgb8((0, 0, 0))
            },
            // CSS `#0004` interval ticks on the usage track.
            interval_tick: rgba8(0, 0, 0, 0x44),
            chart_grid: if dark {
                rgba8(0xA8, 0x9B, 0xB8, 0x33)
            } else {
                rgba8(0x11, 0x11, 0x11, 0x24)
            },
            mono_provider_icon: if dark {
                rgb8((190, 190, 190))
            } else {
                rgb8((96, 96, 96))
            },
            material,
        }
    }

    /// Brand tint of a small provider mark on this theme's cards.
    pub(crate) fn provider_icon(&self, provider: ProviderKind, colored: bool) -> Hsla {
        if !colored {
            return self.mono_provider_icon;
        }
        rgb8(if self.dark {
            crate::provider_registry::dark_surface_brand_rgb(provider)
        } else {
            crate::provider_registry::light_surface_brand_rgb(provider)
        })
    }

    /// Footer tab mark: brand tint, or the neutral chrome glyph.
    pub(crate) fn tab_icon(&self, provider: Option<ProviderKind>, colored: bool) -> Hsla {
        match provider {
            Some(provider) if colored => self.provider_icon(provider, true),
            _ => self.chrome_icon,
        }
    }

    /// Share-bar / donut / legend color used by combined spend surfaces.
    pub(crate) fn spend_color(&self, provider: ProviderKind) -> Hsla {
        let dark = self.dark;
        rgb8(match provider {
            ProviderKind::Codex => (128, 159, 255),
            ProviderKind::Claude => (217, 119, 87),
            ProviderKind::Cursor => {
                if dark {
                    (255, 255, 255)
                } else {
                    (18, 18, 18)
                }
            }
            ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => {
                if dark {
                    (205, 205, 205)
                } else {
                    (75, 75, 75)
                }
            }
            ProviderKind::OpenRouter => (200, 255, 0),
            ProviderKind::Antigravity => (66, 133, 244),
            ProviderKind::Grok => {
                if dark {
                    (255, 255, 255)
                } else {
                    (51, 51, 51)
                }
            }
            ProviderKind::Kiro => (151, 125, 255),
        })
    }

    /// Area-chart series color on the Usage tab.
    pub(crate) fn series_color(&self, provider: ProviderKind) -> Hsla {
        let dark = self.dark;
        rgb8(match provider {
            ProviderKind::Cursor => {
                if dark {
                    (255, 255, 255)
                } else {
                    (28, 28, 28)
                }
            }
            ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => {
                if dark {
                    (210, 210, 210)
                } else {
                    (72, 72, 72)
                }
            }
            _ => crate::provider_registry::descriptor(provider).brand_rgb,
        })
    }
}

/// Resolve Auto against the window's system appearance.
pub(crate) fn resolve_dark(theme: AppTheme, system_dark: bool) -> bool {
    match theme {
        AppTheme::Auto => system_dark,
        AppTheme::Light => false,
        AppTheme::Dark => true,
    }
}

pub(crate) fn accent_ramp(accent: AccentColor) -> crate::theme::AccentRamp {
    crate::theme::accent_ramp(accent)
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
