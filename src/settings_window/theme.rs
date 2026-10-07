//! Fluent color tokens for the GPUI Settings and onboarding windows.
//!
//! The layering follows Windows 11 (base, content plane, cards, controls) but
//! surfaces are separated by fill alone: strokes are reserved for dividers. Everything resolves from the live theme and accent on every
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
    pub(crate) card: Hsla,
    pub(crate) card_hover: Hsla,
    pub(crate) control: Hsla,
    pub(crate) control_hover: Hsla,
    pub(crate) control_pressed: Hsla,
    pub(crate) control_strong: Hsla,
    pub(crate) control_disabled: Hsla,
    /// Unfilled track of toggles and checkboxes.
    pub(crate) control_track: Hsla,
    /// Opaque raised fill for slider thumbs; brighter than any card.
    pub(crate) control_solid: Hsla,
    pub(crate) input_bg: Hsla,
    pub(crate) input_focus_bg: Hsla,
    pub(crate) subtle_hover: Hsla,
    pub(crate) subtle_pressed: Hsla,
    pub(crate) nav_selected: Hsla,
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
    pub(crate) scrim: Hsla,
    pub(crate) success: Hsla,
    pub(crate) success_bg: Hsla,
    pub(crate) caution: Hsla,
    pub(crate) critical: Hsla,
    pub(crate) critical_bg: Hsla,
    pub(crate) selection: Hsla,
    pub(crate) shadow: Hsla,
    /// Soft lift under cards; transparent where fills already separate.
    pub(crate) card_shadow: Hsla,
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
            rgb8((0x19, 0x19, 0x1B))
        } else {
            rgb8((0xEC, 0xEC, 0xEF))
        };
        Self {
            dark,
            font: fonts.text,
            mono_font: fonts.mono,
            icon_font: fonts.icons,
            window_bg,
            // The content plane: one step up from the window, no stroke.
            layer: if dark {
                rgba8(255, 255, 255, 0x08)
            } else {
                rgba8(255, 255, 255, 0x8C)
            },
            card: if dark {
                rgba8(255, 255, 255, 0x0B)
            } else {
                rgba8(255, 255, 255, 0xFF)
            },
            card_hover: if dark {
                rgba8(255, 255, 255, 0x12)
            } else {
                rgba8(0xF7, 0xF7, 0xF9, 0xFF)
            },
            // Controls are filled, never outlined: a tint that reads on both
            // the plane and a card.
            control: ink(dark, if dark { 0x12 } else { 0x0A }),
            control_hover: ink(dark, if dark { 0x1A } else { 0x10 }),
            control_pressed: ink(dark, if dark { 0x0C } else { 0x07 }),
            control_strong: ink(dark, if dark { 0x8B } else { 0x72 }),
            control_disabled: ink(dark, if dark { 0x20 } else { 0x18 }),
            control_track: rgba8(0, 0, 0, if dark { 0x73 } else { 0x1F }),
            control_solid: if dark {
                rgb8((0x4A, 0x4A, 0x4E))
            } else {
                rgb8((0xFF, 0xFF, 0xFF))
            },
            input_bg: ink(dark, if dark { 0x0E } else { 0x08 }),
            input_focus_bg: if dark {
                rgba8(0, 0, 0, 0x40)
            } else {
                rgba8(0, 0, 0, 0x04)
            },
            subtle_hover: ink(dark, if dark { 0x0E } else { 0x08 }),
            subtle_pressed: ink(dark, if dark { 0x09 } else { 0x05 }),
            nav_selected: ink(dark, if dark { 0x13 } else { 0x0B }),
            text: if dark {
                rgba8(255, 255, 255, 0xF2)
            } else {
                rgba8(0, 0, 0, 0xE4)
            },
            text_secondary: ink(dark, if dark { 0xB0 } else { 0x99 }),
            text_tertiary: ink(dark, if dark { 0x80 } else { 0x70 }),
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
            divider: ink(dark, if dark { 0x10 } else { 0x0D }),
            popover: if dark {
                rgb8((0x2A, 0x2A, 0x2D))
            } else {
                rgb8((0xFC, 0xFC, 0xFD))
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
            shadow: rgba8(0, 0, 0, if dark { 0x73 } else { 0x29 }),
            card_shadow: rgba8(0, 0, 0, if dark { 0x00 } else { 0x0A }),
        }
    }

    /// Let a Mica backdrop show through: the window base becomes transparent
    /// and the content plane takes the Windows 11 layer fill, which is
    /// tuned to sit on Mica.
    pub(crate) fn with_mica(mut self) -> Self {
        self.window_bg = gpui::transparent_black();
        self.layer = if self.dark {
            rgba8(0x3A, 0x3A, 0x3A, 0x4C)
        } else {
            rgba8(255, 255, 255, 0x80)
        };
        self
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
