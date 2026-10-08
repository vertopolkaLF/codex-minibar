//! Fluent color tokens for the GPUI popup.
//!
//! Values mirror the WinUI 3 theme dictionaries the WinUI popup used through
//! `ThemeRef`, so cards, strokes and text keep their exact contrast. The
//! accent roles follow the Fluent mapping shared with the
//! Settings window (fill = Light2/Dark1, text = Light3/Dark2).

use gpui::{Hsla, Rgba, SharedString};

use crate::settings::{AccentColor, AppTheme, PopupBackgroundMaterial, PopupTheme, ProviderKind};

/// Family name of the bundled Geist faces used by the Vercel popup theme.
pub(crate) const GEIST_FAMILY: &str = "Geist";

/// Registers the bundled fonts with GPUI's text system. Call once at startup,
/// before any window resolves its font family.
pub(crate) fn register_bundled_fonts(cx: &gpui::App) {
    let fonts = vec![
        std::borrow::Cow::Borrowed(
            include_bytes!("../../../assets/fonts/geist/Geist-Regular.ttf").as_slice(),
        ),
        std::borrow::Cow::Borrowed(
            include_bytes!("../../../assets/fonts/geist/Geist-Medium.ttf").as_slice(),
        ),
        std::borrow::Cow::Borrowed(
            include_bytes!("../../../assets/fonts/geist/Geist-SemiBold.ttf").as_slice(),
        ),
        std::borrow::Cow::Borrowed(
            include_bytes!("../../../assets/fonts/geist/Geist-Bold.ttf").as_slice(),
        ),
    ];
    if let Err(error) = cx.text_system().add_fonts(fonts) {
        eprintln!("could not register the bundled Geist font: {error:#}");
    }
}

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

/// The default UI family: Segoe UI Variable on Windows 11, Segoe UI on older
/// systems that do not ship the variable font.
pub(crate) fn default_font_family(cx: &gpui::App) -> SharedString {
    let names = cx.text_system().all_font_names();
    ["Segoe UI Variable Text", "Segoe UI Variable", "Segoe UI"]
        .into_iter()
        .find(|candidate| names.iter().any(|name| name == candidate))
        .unwrap_or("Segoe UI")
        .into()
}

/// Font families installed on this PC, without GPUI's internal aliases and
/// vertical (`@`) variants.
pub(crate) fn installed_font_families(cx: &gpui::App) -> Vec<SharedString> {
    let mut names = cx
        .text_system()
        .all_font_names()
        .into_iter()
        .filter(|name| !name.is_empty() && !name.starts_with(['.', '@']))
        .collect::<Vec<_>>();
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup();
    names.into_iter().map(SharedString::from).collect()
}

/// The popup's family: the user's pick, else the theme's own typeface.
pub(crate) fn popup_font_family(
    theme: PopupTheme,
    custom: Option<&str>,
    default: &SharedString,
) -> SharedString {
    match theme {
        PopupTheme::Fluent => ui_font_family(custom, default),
        PopupTheme::Vercel => ui_font_family(custom, &SharedString::from(GEIST_FAMILY)),
        // SF Pro is licensed for Apple platforms only; Segoe UI Variable is
        // the closest system face on Windows.
        PopupTheme::Apple => ui_font_family(custom, default),
    }
}

/// The UI family to render with: the user's pick, or the system default.
pub(crate) fn ui_font_family(custom: Option<&str>, default: &SharedString) -> SharedString {
    custom
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map_or_else(
            || default.clone(),
            |name| SharedString::from(name.to_owned()),
        )
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
    pub(crate) theme: PopupTheme,
    pub(crate) dark: bool,
    /// Corner radius of popup cards in DIPs.
    pub(crate) card_radius: f32,
    /// Corner radius of buttons, tabs, chips and anchored tooltips.
    pub(crate) control_radius: f32,
    /// Vertical gap between stacked cards and Home widgets.
    pub(crate) card_gap: f32,
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
    pub(crate) caution: Hsla,
    pub(crate) caution_background: Hsla,
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
        match self.material {
            // Opaque #202020 / #F3F3F3 with no backdrop blur behind it.
            PopupBackgroundMaterial::Solid => return self.solid_background,
            // WinUI Mica tint opacity (0.8 dark / 0.5 light) over the
            // luminosity-matched wallpaper; solid where Mica is unsupported.
            PopupBackgroundMaterial::Mica => {
                if !frosted {
                    return self.solid_background;
                }
                return self
                    .solid_background
                    .opacity(if self.dark { 0.8 } else { 0.5 });
            }
            PopupBackgroundMaterial::Acrylic => {}
        }
        let opacity = if frosted {
            if self.dark { 0.35 } else { 0.25 }
        } else if self.dark {
            0.94
        } else {
            0.96
        };
        self.solid_background.opacity(opacity)
    }

    pub(crate) fn new(
        theme: PopupTheme,
        dark: bool,
        accent: crate::theme::AccentRamp,
        material: PopupBackgroundMaterial,
        font_family: SharedString,
    ) -> Self {
        match theme {
            PopupTheme::Fluent => Self::fluent(dark, accent, material, font_family),
            PopupTheme::Vercel => Self::vercel(dark, material, font_family),
            PopupTheme::Apple => Self::apple(dark, material, font_family),
        }
    }

    /// Accent fill this theme paints with; Vercel is monochrome and Apple
    /// always uses Action Blue.
    pub(crate) fn accent_for(
        theme: PopupTheme,
        dark: bool,
        accent: crate::theme::AccentRamp,
    ) -> Hsla {
        match theme {
            PopupTheme::Fluent => rgb8(accent.fill(dark)),
            PopupTheme::Vercel => vercel_gray_1000(dark),
            PopupTheme::Apple => apple_action_blue(dark),
        }
    }

    fn fluent(
        dark: bool,
        accent: crate::theme::AccentRamp,
        material: PopupBackgroundMaterial,
        font_family: SharedString,
    ) -> Self {
        let accent_fill = rgb8(accent.fill(dark));
        Self {
            theme: PopupTheme::Fluent,
            dark,
            card_radius: crate::popup::CARD_CORNER_RADIUS_DIP as f32,
            // ControlCornerRadius.
            control_radius: 4.0,
            card_gap: 6.0,
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
            // SystemFillColorCaution / CautionBackground.
            caution: if dark {
                rgba8(0xFC, 0xE1, 0x00, 0xFF)
            } else {
                rgba8(0x9D, 0x5D, 0x00, 0xFF)
            },
            caution_background: if dark {
                rgba8(0x43, 0x35, 0x19, 0xFF)
            } else {
                rgba8(0xFF, 0xF4, 0xCE, 0xFF)
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

    /// Vercel's Geist tokens (vercel.com/design.md): a monochrome gray
    /// scale, hairline alpha borders, 8 px cards and color reserved for state.
    fn vercel(dark: bool, material: PopupBackgroundMaterial, font_family: SharedString) -> Self {
        let pick =
            |light: (u8, u8, u8), dark_rgb: (u8, u8, u8)| rgb8(if dark { dark_rgb } else { light });
        // --vbg-gray-alpha-N: black on light, white on dark.
        let gray_alpha = |light: f32, dark_alpha: f32| {
            let alpha = if dark { dark_alpha } else { light };
            ink(dark, (alpha * 255.0).round() as u8)
        };
        let gray_1000 = vercel_gray_1000(dark);
        let gray_900 = pick((0x4D, 0x4D, 0x4D), (0xA0, 0xA0, 0xA0));
        let gray_700 = rgb8((0x8F, 0x8F, 0x8F));
        Self {
            theme: PopupTheme::Vercel,
            dark,
            // --vbg-radius / --vbg-radius-small / --vbg-space-2.
            card_radius: 8.0,
            control_radius: 6.0,
            card_gap: 8.0,
            font_family,
            text_primary: gray_1000,
            text_secondary: gray_900,
            text_tertiary: gray_700,
            // --vbg-background-100 on a gray-1000 fill.
            text_on_accent: pick((0xFF, 0xFF, 0xFF), (0x00, 0x00, 0x00)),
            accent: gray_1000,
            // --vbg-surface-primary cards over the background-200 canvas.
            card_background: pick((0xFF, 0xFF, 0xFF), (0x0A, 0x0A, 0x0A)),
            // --vbg-border-default.
            card_stroke: gray_alpha(0.08, 0.14),
            subtle_fill: gray_alpha(0.05, 0.07),
            control_fill: gray_alpha(0.05, 0.07),
            // --vbg-border-subtle.
            divider: gray_alpha(0.10, 0.13),
            solid_background: pick((0xFA, 0xFA, 0xFA), (0x00, 0x00, 0x00)),
            tooltip_background: pick((0xFF, 0xFF, 0xFF), (0x0A, 0x0A, 0x0A)),
            // Spacing, not a tinted band, separates the footer.
            footer_background: rgba8(0, 0, 0, 0),
            // --vbg-red-900 / red-100.
            critical: pick((0xD8, 0x00, 0x1B), (0xFF, 0x56, 0x5F)),
            critical_background: pick((0xFF, 0xEE, 0xEF), (0x33, 0x0A, 0x11)),
            attention_background: gray_alpha(0.05, 0.07),
            // --vbg-amber-900 / amber-100.
            caution: pick((0xAA, 0x4D, 0x00), (0xFF, 0x93, 0x00)),
            caution_background: pick((0xFF, 0xF6, 0xDE), (0x2A, 0x17, 0x00)),
            chrome_icon: gray_900,
            chrome_icon_hover: gray_1000,
            pace_marker: gray_1000,
            interval_tick: gray_alpha(0.21, 0.24),
            chart_grid: gray_alpha(0.081, 0.09),
            mono_provider_icon: gray_900,
            material,
        }
    }

    /// Apple's web tokens: parchment canvas, white (or near-black tile) cards
    /// on soft hairlines, near-black ink and a single Action Blue accent.
    fn apple(dark: bool, material: PopupBackgroundMaterial, font_family: SharedString) -> Self {
        let pick =
            |light: (u8, u8, u8), dark_rgb: (u8, u8, u8)| rgb8(if dark { dark_rgb } else { light });
        // {colors.ink} on light surfaces, {colors.body-on-dark} on tiles.
        let ink_primary = pick((0x1D, 0x1D, 0x1F), (0xFF, 0xFF, 0xFF));
        // {colors.ink-muted-80} / {colors.body-muted}.
        let ink_secondary = pick((0x33, 0x33, 0x33), (0xCC, 0xCC, 0xCC));
        // {colors.ink-muted-48}.
        let ink_tertiary = rgb8((0x7A, 0x7A, 0x7A));
        Self {
            theme: PopupTheme::Apple,
            dark,
            // {rounded.lg} scaled to popup-size cards / {rounded.sm} / {spacing.sm}.
            card_radius: 14.0,
            control_radius: 8.0,
            card_gap: 12.0,
            font_family,
            text_primary: ink_primary,
            text_secondary: ink_secondary,
            text_tertiary: ink_tertiary,
            // {colors.on-primary}.
            text_on_accent: rgb8((0xFF, 0xFF, 0xFF)),
            accent: apple_action_blue(dark),
            // {colors.canvas} utility cards / {colors.surface-tile-1}.
            card_background: pick((0xFF, 0xFF, 0xFF), (0x27, 0x27, 0x29)),
            // {colors.hairline} as a ring rather than a hard line.
            card_stroke: ink(dark, if dark { 0x0F } else { 0x14 }),
            // {colors.divider-soft}.
            subtle_fill: ink(dark, if dark { 0x14 } else { 0x0A }),
            control_fill: ink(dark, if dark { 0x1F } else { 0x14 }),
            // {colors.hairline}.
            divider: ink(dark, 0x1F),
            // {colors.canvas-parchment} / {colors.surface-black}.
            solid_background: pick((0xF5, 0xF5, 0xF7), (0x00, 0x00, 0x00)),
            // {colors.surface-pearl} / {colors.surface-tile-2}.
            tooltip_background: pick((0xFA, 0xFA, 0xFC), (0x2A, 0x2A, 0x2C)),
            // Frosted sticky-bar band.
            footer_background: if dark {
                rgba8(255, 255, 255, 0x0A)
            } else {
                rgba8(255, 255, 255, 0x80)
            },
            // The design carries no state colors; use Apple's system red/orange.
            critical: pick((0xD7, 0x00, 0x15), (0xFF, 0x45, 0x3A)),
            critical_background: pick((0xFF, 0xF0, 0xF0), (0x3A, 0x1A, 0x1A)),
            attention_background: ink(dark, if dark { 0x0D } else { 0x08 }),
            caution: pick((0xC9, 0x34, 0x00), (0xFF, 0x9F, 0x0A)),
            caution_background: pick((0xFF, 0xF5, 0xE5), (0x3A, 0x2A, 0x10)),
            chrome_icon: ink_secondary,
            chrome_icon_hover: ink_primary,
            pace_marker: ink_primary,
            interval_tick: ink(dark, if dark { 0x3D } else { 0x33 }),
            chart_grid: ink(dark, if dark { 0x1A } else { 0x14 }),
            mono_provider_icon: ink_secondary,
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

    /// Share-bar / donut / legend color of one instance. The primary instance
    /// keeps the driver color; others use their badge color, or a shade of
    /// the driver color, so instances of one driver stay distinguishable.
    pub(crate) fn spend_color(&self, provider: crate::instances::ProviderId) -> Hsla {
        self.instance_shade(provider, self.driver_spend_color(provider.kind()))
    }

    /// Area-chart series color of one instance on the Usage tab.
    pub(crate) fn series_color(&self, provider: crate::instances::ProviderId) -> Hsla {
        self.instance_shade(provider, self.driver_series_color(provider.kind()))
    }

    fn instance_shade(&self, provider: crate::instances::ProviderId, base: Hsla) -> Hsla {
        if provider.is_primary() {
            return base;
        }
        if let Some(rgb) = provider.badge().and_then(|badge| badge.color.rgb()) {
            return rgb8(rgb);
        }
        let hash = provider.id().bytes().fold(0_u32, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(u32::from(byte))
        });
        let amount = [0.3, 0.45, 0.6][(hash % 3) as usize];
        let toward = if self.dark {
            gpui::black()
        } else {
            gpui::white()
        };
        base.mix(toward, amount)
    }

    /// Share-bar / donut / legend color used by combined spend surfaces.
    pub(crate) fn driver_spend_color(&self, provider: ProviderKind) -> Hsla {
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
    pub(crate) fn driver_series_color(&self, provider: ProviderKind) -> Hsla {
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

/// --vbg-gray-1000: Vercel's primary text and fill.
fn vercel_gray_1000(dark: bool) -> Hsla {
    rgb8(if dark {
        (0xED, 0xED, 0xED)
    } else {
        (0x17, 0x17, 0x17)
    })
}

/// {colors.primary} on light surfaces, {colors.primary-on-dark} on dark.
fn apple_action_blue(dark: bool) -> Hsla {
    rgb8(if dark {
        (0x29, 0x97, 0xFF)
    } else {
        (0x00, 0x66, 0xCC)
    })
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
