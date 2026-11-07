//! Fluent color tokens of the component kit, which the Settings and
//! onboarding windows paint with, and the font families every window uses.
//!
//! The layering follows Windows 11 (base, content plane, cards, controls) but
//! surfaces are separated by fill alone: strokes are reserved for dividers. Everything resolves from the live theme and accent on every
//! frame; nothing is cached across appearance changes.

use gpui::{Hsla, SharedString, WindowAppearance};

use super::color::{HslaExt, ink, rgb8, rgba8};
use super::mark::Plate;
use crate::settings::{AccentColor, AppTheme};

/// Family name of the bundled Geist faces used by the Vercel popup theme.
pub(crate) const GEIST_FAMILY: &str = "Geist";

/// Registers the bundled fonts with GPUI's text system. Call once at startup,
/// before any window resolves its font family.
pub(crate) fn register_bundled_fonts(cx: &gpui::App) {
    let fonts = vec![
        std::borrow::Cow::Borrowed(
            include_bytes!("../../assets/fonts/geist/Geist-Regular.ttf").as_slice(),
        ),
        std::borrow::Cow::Borrowed(
            include_bytes!("../../assets/fonts/geist/Geist-Medium.ttf").as_slice(),
        ),
        std::borrow::Cow::Borrowed(
            include_bytes!("../../assets/fonts/geist/Geist-SemiBold.ttf").as_slice(),
        ),
        std::borrow::Cow::Borrowed(
            include_bytes!("../../assets/fonts/geist/Geist-Bold.ttf").as_slice(),
        ),
    ];
    if let Err(error) = cx.text_system().add_fonts(fonts) {
        eprintln!("could not register the bundled Geist font: {error:#}");
    }
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
    pub(crate) control_strong: Hsla,
    /// Flat push button fill; a solid step brighter than controls.
    pub(crate) button: Hsla,
    pub(crate) button_hover: Hsla,
    pub(crate) button_pressed: Hsla,
    /// Destructive button: a soft red wash with saturated red text.
    pub(crate) danger: Hsla,
    pub(crate) danger_hover: Hsla,
    pub(crate) danger_text: Hsla,
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
    pub(crate) accent_text: Hsla,
    pub(crate) accent_soft: Hsla,
    pub(crate) divider: Hsla,
    pub(crate) popover: Hsla,
    pub(crate) dialog: Hsla,
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

/// Font families resolved once per window from the installed fonts.
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
            text: default_font_family(cx),
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
        let dark = super::color::resolve_dark(theme, super::color::appearance_dark(appearance));
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
            control_strong: ink(dark, if dark { 0x8B } else { 0x72 }),
            button: ink(dark, if dark { 0x17 } else { 0x0B }),
            button_hover: ink(dark, if dark { 0x22 } else { 0x13 }),
            button_pressed: ink(dark, if dark { 0x12 } else { 0x08 }),
            danger: rgba8(0xE5, 0x48, 0x4D, if dark { 0x29 } else { 0x1A }),
            danger_hover: rgba8(0xE5, 0x48, 0x4D, if dark { 0x3D } else { 0x29 }),
            danger_text: if dark {
                rgb8((0xFF, 0x6B, 0x6E))
            } else {
                rgb8((0xCE, 0x2C, 0x31))
            },
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
            accent_text: rgb8(accent.text(dark)),
            accent_soft: accent_fill.alpha(if dark { 0.16 } else { 0.12 }),
            divider: ink(dark, if dark { 0x10 } else { 0x0D }),
            popover: if dark {
                rgb8((0x2A, 0x2A, 0x2D))
            } else {
                rgb8((0xFC, 0xFC, 0xFD))
            },
            // Modal cards sit one step above the window, darker than menus
            // so the scrim-dimmed page and the card don't wash out together.
            dialog: if dark {
                rgb8((0x20, 0x20, 0x23))
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

    /// Neutral icon tint used for monochrome marks.
    pub(crate) fn glyph(&self) -> Hsla {
        if self.dark {
            rgb8((0xDC, 0xDC, 0xDC))
        } else {
            rgb8((0x3A, 0x3A, 0x3A))
        }
    }

    /// Badge plates on this theme's surfaces: a neutral plate for `Auto`.
    pub(crate) fn plate(&self) -> Plate {
        let (auto_fill, auto_text) = if self.dark {
            (rgb8((200, 200, 200)), rgb8((28, 28, 28)))
        } else {
            (rgb8((90, 90, 90)), rgb8((255, 255, 255)))
        };
        Plate {
            auto_fill,
            auto_text,
            outline: self.window_bg,
        }
    }
}
