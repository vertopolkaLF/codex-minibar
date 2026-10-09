//! Fluent color tokens for the GPUI popup.
//!
//! Values mirror the WinUI 3 theme dictionaries the WinUI popup used through
//! `ThemeRef`, so cards, strokes and text keep their exact contrast. The
//! accent roles follow the Fluent mapping shared with the
//! Settings window (fill = Light2/Dark1, text = Light3/Dark2).

use std::sync::Arc;

use gpui::{Hsla, Rgba, SharedString};

use crate::settings::{AccentColor, AppTheme, PopupBackgroundMaterial, PopupTheme, ProviderKind};
use crate::vscode_themes::VsCodeTheme;

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
        // VS Code themes carry colors only.
        PopupTheme::VsCode => ui_font_family(custom, default),
    }
}

/// The design the popup paints with: a theme previewed from Settings wins
/// over the saved choice; a saved VS Code theme applies only while it is
/// still installed.
pub(crate) fn popup_design(
    theme: PopupTheme,
    id: Option<&str>,
) -> (PopupTheme, Option<Arc<VsCodeTheme>>) {
    if let Some(preview) = crate::vscode_themes::preview() {
        return (PopupTheme::VsCode, Some(preview));
    }
    if theme != PopupTheme::VsCode {
        return (theme, None);
    }
    (theme, id.and_then(crate::vscode_themes::get))
}

/// Light or dark for a palette: a VS Code theme has a fixed base, every
/// built-in design follows the app theme.
pub(crate) fn palette_dark(vscode: Option<&VsCodeTheme>, app_dark: bool) -> bool {
    vscode.map_or(app_dark, VsCodeTheme::is_dark)
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
    /// Card, control and window outlines are drawn.
    pub(crate) borders: bool,
    /// The VS Code theme this palette was mapped from.
    pub(crate) vscode: Option<Arc<VsCodeTheme>>,
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

    /// `vscode` is the theme from [`popup_design`]; a VS Code
    /// selection whose theme is gone renders as Fluent.
    pub(crate) fn new(
        theme: PopupTheme,
        vscode: Option<Arc<VsCodeTheme>>,
        dark: bool,
        accent: crate::theme::AccentRamp,
        material: PopupBackgroundMaterial,
        font_family: SharedString,
    ) -> Self {
        match theme {
            PopupTheme::Fluent => Self::fluent(dark, accent, material, font_family),
            PopupTheme::Vercel => Self::vercel(dark, material, font_family),
            PopupTheme::VsCode => match vscode {
                Some(vscode) => Self::vscode(vscode, material, font_family),
                None => Self {
                    theme: PopupTheme::VsCode,
                    ..Self::fluent(dark, accent, material, font_family)
                },
            },
        }
    }

    /// Without borders every outline drawn with `card_stroke` disappears;
    /// dividers between rows stay.
    pub(crate) fn with_borders(mut self, borders: bool) -> Self {
        if !borders {
            self.card_stroke = gpui::transparent_black();
        }
        self.borders = borders;
        self
    }

    /// Accent fill this theme paints with; Vercel is monochrome.
    pub(crate) fn accent_for(
        theme: PopupTheme,
        dark: bool,
        accent: crate::theme::AccentRamp,
    ) -> Hsla {
        match theme {
            PopupTheme::Fluent | PopupTheme::VsCode => rgb8(accent.fill(dark)),
            PopupTheme::Vercel => vercel_gray_1000(dark),
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
            borders: true,
            vscode: None,
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
            borders: true,
            vscode: None,
        }
    }

    /// A VS Code theme's workbench colors mapped onto popup roles: the side
    /// bar is the canvas, editor surfaces are cards, the activity bar drives
    /// the footer glyphs and the primary button is the accent. Keys a theme
    /// omits fall back to VS Code's own Dark/Light Modern defaults.
    fn vscode(
        theme: Arc<VsCodeTheme>,
        material: PopupBackgroundMaterial,
        font_family: SharedString,
    ) -> Self {
        let dark = theme.is_dark();
        let pick =
            |light: (u8, u8, u8), dark_rgb: (u8, u8, u8)| rgb8(if dark { dark_rgb } else { light });
        let color = |keys: &[&str]| {
            keys.iter()
                .find_map(|key| theme.color(key))
                .map(|[r, g, b, a]| rgba8(r, g, b, a))
        };
        let editor = flatten(
            color(&["editor.background"])
                .unwrap_or_else(|| pick((0xFF, 0xFF, 0xFF), (0x1F, 0x1F, 0x1F))),
            pick((0xFF, 0xFF, 0xFF), (0x00, 0x00, 0x00)),
        );
        let canvas = flatten(
            color(&[
                "sideBar.background",
                "panel.background",
                "editorGroupHeader.tabsBackground",
            ])
            .unwrap_or(editor),
            editor,
        );
        // Cards lift off the canvas; themes that paint both alike get a tint.
        let card_background = if contrast(editor, canvas) < 1.04 {
            let ink = color(&["foreground"]).unwrap_or_else(|| pick((0, 0, 0), (255, 255, 255)));
            canvas.mix(flatten(ink, canvas), if dark { 0.05 } else { 0.03 })
        } else {
            editor
        };
        // Many themes keep `foreground` muted and brighten only editor text;
        // card titles take whichever reads best.
        let mut foreground = ["foreground", "editor.foreground", "sideBar.foreground"]
            .into_iter()
            .filter_map(|key| color(&[key]))
            .map(|ink| flatten(ink, card_background))
            .max_by(|a, b| contrast(*a, card_background).total_cmp(&contrast(*b, card_background)))
            .unwrap_or_else(|| pick((0x3B, 0x3B, 0x3B), (0xCC, 0xCC, 0xCC)));
        // A theme whose text barely shows on its own cards is unreadable here.
        if contrast(foreground, card_background) < 3.0 {
            foreground = pick((0x1F, 0x1F, 0x1F), (0xF0, 0xF0, 0xF0));
        }
        let tone = |alpha: f32| foreground.alpha(alpha);
        // Usage bars must stand out from their card, and many themes use a
        // muted gray for buttons: take the first accent role that does.
        let accents = [
            ("button.background", "button.foreground"),
            ("activityBarBadge.background", "activityBarBadge.foreground"),
            ("progressBar.background", ""),
            ("focusBorder", ""),
            ("textLink.foreground", ""),
        ]
        .into_iter()
        .filter_map(|(fill, ink)| {
            let fill = flatten(color(&[fill])?, card_background);
            Some((fill, contrast(fill, card_background), ink))
        })
        .collect::<Vec<_>>();
        let (accent, accent_ink) = accents
            .iter()
            .find(|(_, contrast, _)| *contrast >= 2.2)
            .or_else(|| accents.iter().max_by(|a, b| a.1.total_cmp(&b.1)))
            .map_or((rgb8((0x00, 0x78, 0xD4)), ""), |&(fill, _, ink)| {
                (fill, ink)
            });
        let text_on_accent = color(&[accent_ink])
            .map(|ink| flatten(ink, accent))
            .filter(|ink| contrast(*ink, accent) >= 2.5)
            .unwrap_or_else(|| {
                let white = rgb8((0xFF, 0xFF, 0xFF));
                let black = rgb8((0x00, 0x00, 0x00));
                if contrast(white, accent) >= contrast(black, accent) {
                    white
                } else {
                    black
                }
            });
        let critical = color(&[
            "errorForeground",
            "editorError.foreground",
            "list.errorForeground",
        ])
        .map(|ink| flatten(ink, card_background))
        .unwrap_or_else(|| pick((0xA1, 0x26, 0x0D), (0xF4, 0x87, 0x71)));
        let caution = color(&["editorWarning.foreground", "list.warningForeground"])
            .map(|ink| flatten(ink, card_background))
            .unwrap_or_else(|| pick((0xBF, 0x88, 0x03), (0xCC, 0xA7, 0x00)));
        // Secondary text has to sit visibly below the titles.
        let primary_contrast = contrast(foreground, card_background);
        let text_secondary = color(&["descriptionForeground", "foreground"])
            .map(|ink| flatten(ink, card_background))
            .filter(|ink| {
                let ratio = contrast(*ink, card_background);
                ratio >= 2.5 && ratio < primary_contrast * 0.9
            })
            .unwrap_or_else(|| tone(0.78));
        Self {
            theme: PopupTheme::VsCode,
            dark,
            card_radius: crate::popup::CARD_CORNER_RADIUS_DIP as f32,
            control_radius: 4.0,
            card_gap: 6.0,
            font_family,
            text_primary: foreground,
            text_secondary,
            text_tertiary: tone(0.58),
            text_on_accent,
            accent,
            card_background,
            card_stroke: color(&[
                "widget.border",
                "editorWidget.border",
                "panel.border",
                "contrastBorder",
            ])
            .unwrap_or_else(|| tone(if dark { 0.08 } else { 0.10 })),
            subtle_fill: color(&["list.hoverBackground", "toolbar.hoverBackground"])
                .unwrap_or_else(|| tone(if dark { 0.06 } else { 0.04 })),
            // Usage tracks and hover plates must read on any card color, so
            // they are tinted from the text rather than taken from input
            // backgrounds that often match the editor.
            control_fill: tone(if dark { 0.10 } else { 0.08 }),
            divider: color(&[
                "editorGroup.border",
                "panel.border",
                "sideBarSectionHeader.border",
            ])
            .filter(|line| line.a > 0.0)
            .unwrap_or_else(|| tone(0.10)),
            solid_background: canvas,
            tooltip_background: flatten(
                color(&[
                    "editorHoverWidget.background",
                    "editorWidget.background",
                    "menu.background",
                ])
                .unwrap_or(card_background),
                card_background,
            ),
            // A tint toward the activity bar, so translucent materials keep
            // showing through the footer band.
            footer_background: color(&["activityBar.background"])
                .map(|band| band.alpha(0.55))
                .unwrap_or_else(|| rgba8(0, 0, 0, 0)),
            critical,
            critical_background: color(&["inputValidation.errorBackground"])
                .map(|fill| flatten(fill, card_background))
                .unwrap_or_else(|| card_background.mix(critical, 0.16)),
            attention_background: tone(if dark { 0.04 } else { 0.03 }),
            caution,
            caution_background: color(&["inputValidation.warningBackground"])
                .map(|fill| flatten(fill, card_background))
                .unwrap_or_else(|| card_background.mix(caution, 0.16)),
            chrome_icon: color(&["activityBar.inactiveForeground"])
                .map(|ink| flatten(ink, canvas))
                .filter(|ink| contrast(*ink, canvas) >= 2.0)
                .unwrap_or(text_secondary),
            chrome_icon_hover: color(&["activityBar.foreground"])
                .map(|ink| flatten(ink, canvas))
                .filter(|ink| contrast(*ink, canvas) >= 3.0)
                .unwrap_or(foreground),
            pace_marker: foreground,
            interval_tick: rgba8(0, 0, 0, 0x44),
            chart_grid: tone(if dark { 0.12 } else { 0.10 }),
            mono_provider_icon: text_secondary,
            material,
            borders: true,
            vscode: Some(theme),
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

/// `color` composited over an opaque `base`.
fn flatten(color: Hsla, base: Hsla) -> Hsla {
    let top = color.to_rgb();
    let under = base.to_rgb();
    let t = top.a.clamp(0.0, 1.0);
    Rgba {
        r: under.r + (top.r - under.r) * t,
        g: under.g + (top.g - under.g) * t,
        b: under.b + (top.b - under.b) * t,
        a: 1.0,
    }
    .into()
}

/// WCAG contrast ratio between two opaque colors.
fn contrast(a: Hsla, b: Hsla) -> f32 {
    fn luminance(color: Hsla) -> f32 {
        let rgb = color.to_rgb();
        let channel = |value: f32| {
            if value <= 0.039_28 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(rgb.r) + 0.7152 * channel(rgb.g) + 0.0722 * channel(rgb.b)
    }
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vscode_themes::{ThemeKind, ThemeSource};

    fn vscode(kind: ThemeKind, colors: &[(&str, &str)]) -> Palette {
        let theme = VsCodeTheme {
            id: "test".into(),
            label: "Test".into(),
            extension: None,
            kind,
            source: ThemeSource::File {
                name: "test.json".into(),
            },
            colors: colors
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        };
        Palette::vscode(
            Arc::new(theme),
            PopupBackgroundMaterial::Solid,
            "Segoe UI".into(),
        )
    }

    fn bytes(color: Hsla) -> (u8, u8, u8) {
        let rgb = color.to_rgb();
        let byte = |value: f32| (value * 255.0).round() as u8;
        (byte(rgb.r), byte(rgb.g), byte(rgb.b))
    }

    #[test]
    fn themes_without_colors_stay_readable() {
        for kind in [ThemeKind::Dark, ThemeKind::Light] {
            let palette = vscode(kind, &[]);
            assert_eq!(palette.dark, kind.is_dark());
            assert!(contrast(palette.text_primary, palette.card_background) >= 4.5);
            assert!(contrast(palette.accent, palette.card_background) >= 2.2);
        }
    }

    #[test]
    fn muted_buttons_hand_the_accent_to_a_visible_role() {
        // Dracula: gray buttons, purple badge.
        let palette = vscode(
            ThemeKind::Dark,
            &[
                ("editor.background", "#282a36"),
                ("sideBar.background", "#21222c"),
                ("foreground", "#f8f8f2"),
                ("button.background", "#44475a"),
                ("activityBarBadge.background", "#bd93f9"),
                ("activityBarBadge.foreground", "#f8f8f2"),
            ],
        );
        assert_eq!(bytes(palette.accent), (0xbd, 0x93, 0xf9));
        assert_eq!(bytes(palette.solid_background), (0x21, 0x22, 0x2c));
        assert_eq!(bytes(palette.card_background), (0x28, 0x2a, 0x36));
    }

    #[test]
    fn titles_use_the_brightest_foreground() {
        let palette = vscode(
            ThemeKind::Dark,
            &[
                ("editor.background", "#1a1b26"),
                ("foreground", "#787c99"),
                ("editor.foreground", "#a9b1d6"),
            ],
        );
        assert_eq!(bytes(palette.text_primary), (0xa9, 0xb1, 0xd6));
        let secondary = flatten(palette.text_secondary, palette.card_background);
        assert!(
            contrast(secondary, palette.card_background)
                < contrast(palette.text_primary, palette.card_background)
        );
    }
}
