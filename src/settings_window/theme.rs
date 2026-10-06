//! Fluent color tokens for the GPUI Settings and onboarding windows.
//!
//! Values follow the Windows 11 theme dictionaries (Mica base, layer, card,
//! control and text fills) so the window sits naturally next to the system
//! Settings app. Everything resolves from the live theme and accent on every
//! frame; nothing is cached across appearance changes.

use gpui::{Hsla, SharedString, WindowAppearance};

use crate::popup_window::ui::theme::{HslaExt, rgb8, rgba8};
use crate::settings::{AccentColor, AppTheme};

fn ink(dark: bool, alpha: u8) -> Hsla {
    if dark {
        rgba8(255, 255, 255, alpha)
    } else {
        rgba8(0, 0, 0, alpha)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Theme {
    pub(crate) dark: bool,
    pub(crate) font: SharedString,
    pub(crate) mono_font: SharedString,
    pub(crate) icon_font: SharedString,
    pub(crate) window_bg: Hsla,
    pub(crate) layer: Hsla,
    pub(crate) layer_stroke: Hsla,
    pub(crate) card: Hsla,
    pub(crate) card_hover: Hsla,
    pub(crate) card_stroke: Hsla,
    pub(crate) control: Hsla,
    pub(crate) control_hover: Hsla,
    pub(crate) control_pressed: Hsla,
    pub(crate) control_stroke: Hsla,
    pub(crate) control_strong: Hsla,
    pub(crate) control_disabled: Hsla,
    pub(crate) input_bg: Hsla,
    pub(crate) input_focus_bg: Hsla,
    pub(crate) subtle_hover: Hsla,
    pub(crate) subtle_pressed: Hsla,
    pub(crate) text: Hsla,
    pub(crate) text_secondary: Hsla,
    pub(crate) text_tertiary: Hsla,
    pub(crate) text_disabled: Hsla,
    pub(crate) on_accent: Hsla,
    pub(crate) accent: Hsla,
    pub(crate) accent_hover: Hsla,
    pub(crate) accent_pressed: Hsla,
    pub(crate) accent_text: Hsla,
    pub(crate) accent_soft: Hsla,
    pub(crate) divider: Hsla,
    pub(crate) popover: Hsla,
    pub(crate) popover_stroke: Hsla,
    pub(crate) scrim: Hsla,
    pub(crate) success: Hsla,
    pub(crate) success_bg: Hsla,
    pub(crate) caution: Hsla,
    pub(crate) critical: Hsla,
    pub(crate) critical_bg: Hsla,
    pub(crate) selection: Hsla,
    pub(crate) shadow: Hsla,
}

impl Default for Theme {
    fn default() -> Self {
        Self::new(
            true,
            crate::theme::AccentRamp::from_base((0, 120, 212)),
            Fonts::default(),
        )
    }
}

/// Font families resolved once per process from the installed fonts.
#[derive(Clone, Debug)]
pub(crate) struct Fonts {
    pub(crate) text: SharedString,
    pub(crate) mono: SharedString,
    pub(crate) icons: SharedString,
}

impl Default for Fonts {
    fn default() -> Self {
        Self {
            text: "Segoe UI".into(),
            mono: "Consolas".into(),
            icons: "Segoe MDL2 Assets".into(),
        }
    }
}

impl Fonts {
    pub(crate) fn resolve(cx: &gpui::App) -> Self {
        let names = cx.text_system().all_font_names();
        let pick = |candidates: &[&'static str], fallback: &'static str| {
            candidates
                .iter()
                .find(|candidate| names.iter().any(|name| name == *candidate))
                .map_or_else(|| SharedString::from(fallback), |name| (*name).into())
        };
        Self {
            text: pick(
                &["Segoe UI Variable Text", "Segoe UI Variable", "Segoe UI"],
                "Segoe UI",
            ),
            mono: pick(&["Cascadia Mono", "Consolas"], "Consolas"),
            icons: pick(
                &["Segoe Fluent Icons", "Segoe MDL2 Assets"],
                "Segoe MDL2 Assets",
            ),
        }
    }
}

impl Theme {
    pub(crate) fn resolve(
        theme: AppTheme,
        accent: AccentColor,
        appearance: WindowAppearance,
        fonts: Fonts,
    ) -> Self {
        let system_dark = matches!(
            appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        let dark = match theme {
            AppTheme::Auto => system_dark,
            AppTheme::Light => false,
            AppTheme::Dark => true,
        };
        Self::new(dark, crate::theme::accent_ramp(accent), fonts)
    }

    pub(crate) fn new(dark: bool, accent: crate::theme::AccentRamp, fonts: Fonts) -> Self {
        let accent_fill = rgb8(accent.fill(dark));
        let window_bg = if dark {
            rgb8((0x20, 0x20, 0x20))
        } else {
            rgb8((0xF3, 0xF3, 0xF3))
        };
        Self {
            dark,
            font: fonts.text,
            mono_font: fonts.mono,
            icon_font: fonts.icons,
            window_bg,
            layer: if dark {
                rgba8(0x3A, 0x3A, 0x3A, 0x4C)
            } else {
                rgba8(255, 255, 255, 0x80)
            },
            layer_stroke: ink(dark, if dark { 0x10 } else { 0x0C }),
            card: if dark {
                rgba8(255, 255, 255, 0x0D)
            } else {
                rgba8(255, 255, 255, 0xB3)
            },
            card_hover: if dark {
                rgba8(255, 255, 255, 0x15)
            } else {
                rgba8(0xF6, 0xF6, 0xF6, 0xD0)
            },
            card_stroke: if dark {
                rgba8(0, 0, 0, 0x30)
            } else {
                rgba8(0, 0, 0, 0x0F)
            },
            control: if dark {
                rgba8(255, 255, 255, 0x0F)
            } else {
                rgba8(255, 255, 255, 0xB3)
            },
            control_hover: if dark {
                rgba8(255, 255, 255, 0x15)
            } else {
                rgba8(0xF9, 0xF9, 0xF9, 0x80)
            },
            control_pressed: if dark {
                rgba8(255, 255, 255, 0x08)
            } else {
                rgba8(0xF9, 0xF9, 0xF9, 0x4D)
            },
            control_stroke: ink(dark, if dark { 0x14 } else { 0x16 }),
            control_strong: ink(dark, if dark { 0x8B } else { 0x72 }),
            control_disabled: ink(dark, if dark { 0x28 } else { 0x37 }),
            input_bg: if dark {
                rgba8(255, 255, 255, 0x0B)
            } else {
                rgba8(255, 255, 255, 0xB3)
            },
            input_focus_bg: if dark {
                rgb8((0x1E, 0x1E, 0x1E))
            } else {
                rgb8((0xFF, 0xFF, 0xFF))
            },
            subtle_hover: ink(dark, if dark { 0x0F } else { 0x09 }),
            subtle_pressed: ink(dark, if dark { 0x0A } else { 0x06 }),
            text: if dark {
                rgba8(255, 255, 255, 0xFF)
            } else {
                rgba8(0, 0, 0, 0xE4)
            },
            text_secondary: ink(dark, if dark { 0xC5 } else { 0x9E }),
            text_tertiary: ink(dark, if dark { 0x87 } else { 0x72 }),
            text_disabled: ink(dark, if dark { 0x5D } else { 0x5C }),
            on_accent: if dark {
                rgba8(0, 0, 0, 0xFF)
            } else {
                rgba8(255, 255, 255, 0xFF)
            },
            accent: accent_fill,
            accent_hover: accent_fill.alpha(0.9),
            accent_pressed: accent_fill.alpha(0.8),
            accent_text: rgb8(accent.text(dark)),
            accent_soft: accent_fill.alpha(if dark { 0.16 } else { 0.12 }),
            divider: ink(dark, if dark { 0x15 } else { 0x0F }),
            popover: if dark {
                rgb8((0x2C, 0x2C, 0x2C))
            } else {
                rgb8((0xF9, 0xF9, 0xF9))
            },
            popover_stroke: if dark {
                rgba8(0, 0, 0, 0x5C)
            } else {
                rgba8(0, 0, 0, 0x17)
            },
            scrim: rgba8(0, 0, 0, if dark { 0x80 } else { 0x4D }),
            success: if dark {
                rgb8((0x6C, 0xCB, 0x5F))
            } else {
                rgb8((0x0F, 0x7B, 0x0F))
            },
            success_bg: if dark {
                rgb8((0x39, 0x3D, 0x1B))
            } else {
                rgb8((0xDF, 0xF6, 0xDD))
            },
            caution: if dark {
                rgb8((0xFC, 0xE1, 0x00))
            } else {
                rgb8((0x9D, 0x5D, 0x00))
            },
            critical: if dark {
                rgb8((0xFF, 0x99, 0xA4))
            } else {
                rgb8((0xC4, 0x2B, 0x1C))
            },
            critical_bg: if dark {
                rgb8((0x44, 0x27, 0x26))
            } else {
                rgb8((0xFD, 0xE7, 0xE9))
            },
            selection: accent_fill.alpha(0.4),
            shadow: rgba8(0, 0, 0, if dark { 0x66 } else { 0x26 }),
        }
    }

    /// Brand tint of a provider mark on this theme's surfaces.
    pub(crate) fn brand(&self, provider: crate::settings::ProviderKind) -> Hsla {
        rgb8(if self.dark {
            crate::provider_registry::dark_surface_brand_rgb(provider)
        } else {
            crate::provider_registry::light_surface_brand_rgb(provider)
        })
    }

    /// Neutral icon tint used for monochrome marks.
    pub(crate) fn glyph(&self) -> Hsla {
        if self.dark {
            rgb8((0xDC, 0xDC, 0xDC))
        } else {
            rgb8((0x3A, 0x3A, 0x3A))
        }
    }

    /// Badge plate colors: the badge color, or a neutral plate for `Auto`.
    pub(crate) fn badge(&self, color: crate::settings::BadgeColor) -> (Hsla, Hsla) {
        match color.rgb() {
            Some(rgb) => (rgb8(rgb), rgb8((255, 255, 255))),
            None if self.dark => (rgb8((200, 200, 200)), rgb8((28, 28, 28))),
            None => (rgb8((90, 90, 90)), rgb8((255, 255, 255))),
        }
    }
}
