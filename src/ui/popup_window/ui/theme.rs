//! Fluent color tokens for the GPUI popup.
//!
//! Values mirror the WinUI 3 theme dictionaries, so cards, strokes and text
//! keep their exact contrast. The accent roles follow the Fluent mapping
//! shared with the Settings window (fill = Light2/Dark1, text = Light3/Dark2).

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

use gpui::{Hsla, Rgba, SharedString};

use crate::providers::registry::ProviderKind;
use crate::settings::{
    POPUP_VSCODE_CONTRAST_DEFAULT, POPUP_VSCODE_TINT_DEFAULT, PopupBackgroundMaterial, PopupTheme,
    Settings,
};
use crate::ui::gui::color::{HslaExt, brand, ink, resolve_dark, rgb8, rgba8};
use crate::ui::gui::mark::Plate;
use crate::ui::gui::theme::GEIST_FAMILY;
use crate::ui::theme::AccentRamp;
use crate::ui::vscode_themes::VsCodeTheme;

/// A family the popup paints with temporarily while Settings hovers it in the
/// font picker: `Some(None)` previews the Windows default. Never saved.
static FONT_PREVIEW: Mutex<Option<Option<String>>> = Mutex::new(None);
static FONT_PREVIEW_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Replace the font preview; returns whether it changed.
pub(crate) fn set_font_preview(preview: Option<Option<String>>) -> bool {
    let Ok(mut current) = FONT_PREVIEW.lock() else {
        return false;
    };
    if *current == preview {
        return false;
    }
    *current = preview;
    FONT_PREVIEW_GENERATION.fetch_add(1, Ordering::SeqCst);
    true
}

fn font_preview() -> Option<Option<String>> {
    FONT_PREVIEW.lock().ok().and_then(|preview| preview.clone())
}

/// Changes whenever a theme or font preview starts or ends, so the popup
/// knows to rebuild its palette.
pub(crate) fn preview_generation() -> u64 {
    crate::ui::vscode_themes::preview_generation()
        .wrapping_add(FONT_PREVIEW_GENERATION.load(Ordering::SeqCst))
}

/// The popup's family: a family previewed from Settings, else the user's
/// pick, else the theme's own typeface.
pub(crate) fn popup_font_family(
    theme: PopupTheme,
    custom: Option<&str>,
    default: &SharedString,
) -> SharedString {
    let preview = font_preview();
    let custom = match &preview {
        Some(preview) => preview.as_deref(),
        None => custom,
    };
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
    if let Some(preview) = crate::ui::vscode_themes::preview() {
        return (PopupTheme::VsCode, Some(preview));
    }
    if theme != PopupTheme::VsCode {
        return (theme, None);
    }
    (theme, id.and_then(crate::ui::vscode_themes::get))
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

#[derive(Clone, Debug)]
pub(crate) struct Palette {
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
    /// The accent where it colors text; [`Palette::with_contrast`] brightens
    /// it without touching accent fills.
    pub(crate) accent_text: Hsla,
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
    /// Favorable status ink, e.g. a usage pace in reserve.
    pub(crate) positive: Hsla,
    pub(crate) chrome_icon: Hsla,
    pub(crate) chrome_icon_hover: Hsla,
    pub(crate) pace_marker: Hsla,
    pub(crate) interval_tick: Hsla,
    pub(crate) chart_grid: Hsla,
    pub(crate) mono_provider_icon: Hsla,
    pub(crate) material: PopupBackgroundMaterial,
    /// Card and control outlines are drawn.
    pub(crate) borders: bool,
    /// The window hairline is drawn, independently of `borders`.
    pub(crate) window_outline: bool,
    /// Opacity in percent of a VS Code theme's canvas over Mica or Acrylic.
    pub(crate) tint: u16,
    /// The VS Code theme this palette was mapped from.
    pub(crate) vscode: Option<Arc<VsCodeTheme>>,
}

impl Palette {
    /// Luminosity controls backdrop brightness independently of tint opacity,
    /// so a lighter tint can preserve wallpaper hue without washing out on white.
    pub(crate) fn capsule_background(&self, frosted: bool) -> Hsla {
        // The backdrop keeps the desktop's hue; a VS Code theme's canvas
        // covers it as much as the user's tint asks.
        if self.vscode.is_some() && frosted && self.material != PopupBackgroundMaterial::Solid {
            return self.solid_background.opacity(f32::from(self.tint) / 100.0);
        }
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

    /// Lightness source of the Mica/Acrylic backdrop: a VS Code theme's
    /// canvas, else the Fluent base (#202020 dark, near-white light).
    #[cfg(windows)]
    pub(crate) fn backdrop_luminosity(&self) -> (u8, u8, u8) {
        if self.vscode.is_none() {
            let level = if self.dark { 32 } else { 245 };
            return (level, level, level);
        }
        let rgb = self.solid_background.to_rgb();
        let byte = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
        (byte(rgb.r), byte(rgb.g), byte(rgb.b))
    }

    /// The popup design `settings` choose, for a window whose system
    /// appearance is `system_dark`.
    pub(crate) fn from_settings(
        settings: &Settings,
        system_dark: bool,
        accent: AccentRamp,
        material: PopupBackgroundMaterial,
        default_font: &SharedString,
    ) -> Self {
        let (theme, vscode) =
            popup_design(settings.popup_theme, settings.popup_vscode_theme.as_deref());
        Self::design(
            theme,
            vscode,
            resolve_dark(settings.theme, system_dark),
            accent,
            material,
            settings,
            default_font,
        )
    }

    /// `theme` dressed in the font, contrast, tint and borders `settings`
    /// choose. `vscode` is the theme from [`popup_design`]; a VS Code
    /// selection whose theme is gone renders as Fluent.
    pub(crate) fn design(
        theme: PopupTheme,
        vscode: Option<Arc<VsCodeTheme>>,
        app_dark: bool,
        accent: AccentRamp,
        material: PopupBackgroundMaterial,
        settings: &Settings,
        default_font: &SharedString,
    ) -> Self {
        // A VS Code theme has a fixed base; built-in designs follow the app.
        let dark = vscode.as_deref().map_or(app_dark, VsCodeTheme::is_dark);
        let font_family = popup_font_family(theme, settings.font_family.as_deref(), default_font);
        let palette = match theme {
            PopupTheme::Fluent => Self::fluent(dark, accent, material, font_family),
            PopupTheme::Vercel => Self::vercel(dark, material, font_family),
            PopupTheme::VsCode => match vscode {
                Some(vscode) => Self::vscode(vscode, material, font_family),
                None => Self::fluent(dark, accent, material, font_family),
            },
        };
        let mut palette = palette.with_contrast(settings.popup_vscode_contrast);
        palette.tint = settings.popup_vscode_tint;
        palette.window_outline = settings.popup_window_border;
        // Without borders every outline drawn with `card_stroke` disappears;
        // dividers between rows stay.
        if !settings.popup_borders {
            palette.card_stroke = gpui::transparent_black();
        }
        palette.borders = settings.popup_borders;
        palette
    }

    /// Scales how far a VS Code palette's text, glyphs and outlines stand
    /// out: above 100 % they blend toward pure white (dark) or black
    /// (light), below it they fade into the surface. Built-in designs keep
    /// their tuned tokens.
    fn with_contrast(mut self, percent: u16) -> Self {
        if self.vscode.is_none() || percent == POPUP_VSCODE_CONTRAST_DEFAULT {
            return self;
        }
        let amount = (f32::from(percent) / 100.0 - 1.0).clamp(-0.5, 1.0);
        let ink = if self.dark {
            gpui::white()
        } else {
            gpui::black()
        };
        // Text keeps part of its hue and hierarchy even at the maximum;
        // hairlines move less so cards don't turn into boxes.
        let adjust = |color: Hsla, weight: f32| {
            if color.a <= 0.0 {
                color
            } else if amount >= 0.0 {
                color.mix(ink, amount * weight)
            } else {
                color.alpha(1.0 + amount * weight)
            }
        };
        for (color, weight) in [
            (&mut self.text_primary, 0.7),
            (&mut self.accent_text, 0.5),
            (&mut self.text_secondary, 0.6),
            (&mut self.text_tertiary, 0.5),
            (&mut self.chrome_icon, 0.6),
            (&mut self.chrome_icon_hover, 0.7),
            (&mut self.mono_provider_icon, 0.6),
            (&mut self.pace_marker, 0.7),
            (&mut self.card_stroke, 0.25),
            (&mut self.divider, 0.25),
            (&mut self.chart_grid, 0.2),
            (&mut self.control_fill, 0.15),
        ] {
            *color = adjust(*color, weight);
        }
        self
    }

    /// Whether the popup and toast windows draw their edge hairline.
    pub(crate) fn window_border(&self) -> bool {
        self.window_outline
    }

    /// Badge plates on this theme's cards: a neutral plate for `Auto`.
    pub(crate) fn plate(&self) -> Plate {
        Plate {
            auto_fill: self.text_secondary,
            auto_text: self.solid_background.opacity(1.0),
            outline: self.solid_background.opacity(0.9),
        }
    }

    fn fluent(
        dark: bool,
        accent: crate::ui::theme::AccentRamp,
        material: PopupBackgroundMaterial,
        font_family: SharedString,
    ) -> Self {
        let accent_fill = rgb8(accent.fill(dark));
        Self {
            dark,
            card_radius: crate::ui::popup::CARD_CORNER_RADIUS_DIP as f32,
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
            accent_text: accent_fill,
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
            // SystemFillColorSuccess.
            positive: if dark {
                rgba8(0x6C, 0xCB, 0x5F, 0xFF)
            } else {
                rgba8(0x0F, 0x7B, 0x0F, 0xFF)
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
            window_outline: true,
            tint: POPUP_VSCODE_TINT_DEFAULT,
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
            accent_text: gray_1000,
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
            // --vbg-green-900.
            positive: pick((0x10, 0x7D, 0x32), (0x62, 0xC0, 0x73)),
            chrome_icon: gray_900,
            chrome_icon_hover: gray_1000,
            pace_marker: gray_1000,
            interval_tick: gray_alpha(0.21, 0.24),
            chart_grid: gray_alpha(0.081, 0.09),
            mono_provider_icon: gray_900,
            material,
            borders: true,
            window_outline: true,
            tint: POPUP_VSCODE_TINT_DEFAULT,
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
        let positive = color(&[
            "testing.iconPassed",
            "gitDecoration.addedResourceForeground",
            "editorGutter.addedBackground",
        ])
        .map(|ink| flatten(ink, card_background))
        .unwrap_or_else(|| pick((0x38, 0x8A, 0x34), (0x89, 0xD1, 0x85)));
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
            dark,
            card_radius: crate::ui::popup::CARD_CORNER_RADIUS_DIP as f32,
            control_radius: 4.0,
            card_gap: 6.0,
            font_family,
            text_primary: foreground,
            text_secondary,
            text_tertiary: tone(0.58),
            text_on_accent,
            accent,
            accent_text: accent,
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
            positive,
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
            window_outline: true,
            tint: POPUP_VSCODE_TINT_DEFAULT,
            vscode: Some(theme),
        }
    }

    /// Brand tint of a small provider mark on this theme's cards.
    pub(crate) fn provider_icon(&self, provider: ProviderKind, colored: bool) -> Hsla {
        if !colored {
            return self.mono_provider_icon;
        }
        brand(provider, self.dark)
    }

    /// Footer tab mark: brand tint, or the neutral chrome glyph.
    pub(crate) fn tab_icon(&self, provider: Option<ProviderKind>, colored: bool) -> Hsla {
        match provider {
            Some(provider) if colored => brand(provider, self.dark),
            _ => self.chrome_icon,
        }
    }

    /// Share-bar / donut / legend color of one instance. The primary instance
    /// keeps the driver color; others use their badge color, or a shade of
    /// the driver color, so instances of one driver stay distinguishable.
    pub(crate) fn spend_color(&self, provider: crate::providers::instances::ProviderId) -> Hsla {
        let rgb = crate::providers::registry::descriptor(provider.kind()).spend_rgb;
        self.instance_shade(provider, rgb8(rgb.pick(self.dark)))
    }

    /// Area-chart series color of one instance on the Usage tab.
    pub(crate) fn series_color(&self, provider: crate::providers::instances::ProviderId) -> Hsla {
        let rgb = crate::providers::registry::descriptor(provider.kind()).series_rgb;
        self.instance_shade(provider, rgb8(rgb.pick(self.dark)))
    }

    fn instance_shade(
        &self,
        provider: crate::providers::instances::ProviderId,
        base: Hsla,
    ) -> Hsla {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::vscode_themes::{ThemeKind, ThemeSource};

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
            if cfg!(windows) { "Segoe UI" } else { "Sans" }.into(),
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
