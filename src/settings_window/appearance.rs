//! Appearance: theme, accent, font, icons, time format, popup material and motion.

use std::{sync::Arc, time::Duration};

use gpui::{
    AnyElement, App, AppContext, Context, Entity, Focusable, FontWeight, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, SharedString, StatefulInteractiveElement, Styled,
    Subscription, Task, Window, div, prelude::FluentBuilder, px, relative,
};

use super::input::{InputEvent, TextInput};
use super::kit::{self, Button, ButtonSize, Kit, Row, SliderRange, eid};
use super::vscode_themes::{ThemeBrowserUi, ThemeHost};
use super::window::SettingsWindow;
use crate::popup_window::ui::fx;
use crate::popup_window::ui::theme::{self as popup_theme, Palette, rgb8};
use crate::settings::{
    AccentColor, AppTheme, BottomBarSize, POPUP_VSCODE_CONTRAST_DEFAULT, POPUP_VSCODE_CONTRAST_MAX,
    POPUP_VSCODE_CONTRAST_MIN, POPUP_VSCODE_TINT_DEFAULT, POPUP_VSCODE_TINT_MAX,
    POPUP_VSCODE_TINT_MIN, PopupBackgroundMaterial, PopupCornerRadius, PopupTheme, Settings,
    TimeFormat,
};
use crate::vscode_themes::VsCodeTheme;

/// Popup theme cards per row.
const THEME_COLUMNS: u16 = 3;

const FONT_PICKER: &str = "appearance-font";
const FONT_PICKER_WIDTH: f32 = 220.0;
/// Families listed above the rest of the font picker.
const RECENT_FONTS: usize = 5;
/// How long the pointer rests on a family before the popup previews it.
const FONT_PREVIEW_DELAY: Duration = Duration::from_millis(200);

/// One font picker entry; `None` is the Windows default.
type FontChoice = Option<SharedString>;

/// Transient state of the Appearance font picker.
#[derive(Default)]
pub(super) struct FontPicker {
    input: Option<Entity<TextInput>>,
    _subscription: Option<Subscription>,
    /// The list was open last frame.
    open: bool,
    /// Entries as last rendered, for keyboard navigation.
    choices: Vec<FontChoice>,
    selected: Option<usize>,
    highlighted: Option<usize>,
    /// Entry to scroll into view on the next frame.
    reveal: Option<usize>,
    /// The list was opened since the last frame.
    fresh: bool,
    /// Entry under the pointer (or keyboard), waiting to be previewed.
    hovered: Option<FontChoice>,
    preview_task: Option<Task<()>>,
}

impl FontPicker {
    /// Forget a pending preview, e.g. when its page goes away.
    pub(super) fn cancel_preview(&mut self) {
        self.hovered = None;
        self.preview_task = None;
    }
}

/// One card in the popup theme grid.
pub(super) enum ThemeChoice {
    Builtin(PopupTheme),
    VsCode(Arc<VsCodeTheme>),
}

const ACCENTS: [AccentColor; 8] = [
    AccentColor::Windows,
    AccentColor::Blue,
    AccentColor::Purple,
    AccentColor::Pink,
    AccentColor::Red,
    AccentColor::Orange,
    AccentColor::Green,
    AccentColor::Teal,
];

/// A miniature popup painted with `palette`: two usage cards and the footer.
pub(super) fn popup_mock(k: &Kit, palette: &Palette) -> gpui::Div {
    let line = |width: f32, color| div().h(px(4.0)).w(relative(width)).rounded_full().bg(color);
    let mock_card = |fill: f32| {
        div()
            .flex()
            .flex_col()
            .gap(px(5.0))
            .p(px(6.0))
            .rounded(px(palette.card_radius * 0.6))
            .bg(palette.card_background)
            .border_1()
            .border_color(palette.card_stroke)
            .child(line(0.45, palette.text_primary))
            .child(
                div()
                    .h(px(4.0))
                    .w_full()
                    .rounded_full()
                    .bg(palette.control_fill)
                    .child(
                        div()
                            .h_full()
                            .w(relative(fill))
                            .rounded_full()
                            .bg(palette.accent),
                    ),
            )
            .child(line(0.3, palette.text_tertiary))
    };
    div()
        .h(px(96.0))
        .w_full()
        .flex()
        .flex_col()
        .gap(px(5.0))
        .p(px(8.0))
        .rounded(px(6.0))
        .overflow_hidden()
        .bg(palette.solid_background)
        .border_1()
        .border_color(k.theme.divider)
        .child(mock_card(0.62))
        .child(mock_card(0.28))
        .child(
            div()
                .mt_auto()
                .flex()
                .items_center()
                .gap(px(5.0))
                .child(div().size(px(6.0)).rounded_full().bg(palette.accent))
                .child(div().size(px(6.0)).rounded_full().bg(palette.chrome_icon))
                .child(div().size(px(6.0)).rounded_full().bg(palette.chrome_icon)),
        )
}

/// Color theme, accent and popup theme pickers, shared by every
/// [`ThemeHost`].
pub(super) trait ThemePicker: ThemeHost {
    /// The palette a VS Code theme gives the popup, for previews. `accent`
    /// comes from the caller: resolving the Windows accent goes through
    /// WinRT, too slow to repeat for every card of a frame.
    fn vscode_palette(
        &self,
        vscode: Arc<VsCodeTheme>,
        accent: crate::theme::AccentRamp,
    ) -> Palette {
        let font = popup_theme::popup_font_family(
            PopupTheme::VsCode,
            self.settings().font_family.as_deref(),
            &self.fonts().text,
        );
        let dark = popup_theme::palette_dark(Some(&vscode), false);
        Palette::new(
            PopupTheme::VsCode,
            Some(vscode),
            dark,
            accent,
            PopupBackgroundMaterial::Solid,
            font,
        )
        .with_contrast(self.settings().popup_vscode_contrast)
        .with_borders(self.settings().popup_borders)
    }

    /// A miniature window preview for one theme choice.
    fn theme_card(
        &self,
        k: &Kit,
        value: AppTheme,
        label: &'static str,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = &k.theme;
        let accent = theme.accent;
        let mock = |dark: bool| {
            let (bg, card, line) = if dark {
                (
                    rgb8((0x20, 0x20, 0x20)),
                    rgb8((0x2D, 0x2D, 0x2D)),
                    rgb8((0x55, 0x55, 0x55)),
                )
            } else {
                (
                    rgb8((0xF3, 0xF3, 0xF3)),
                    rgb8((0xFF, 0xFF, 0xFF)),
                    rgb8((0xD0, 0xD0, 0xD0)),
                )
            };
            div()
                .flex_1()
                .h_full()
                .bg(bg)
                .p(px(8.0))
                .flex()
                .flex_col()
                .gap(px(5.0))
                .child(div().h(px(6.0)).w(px(28.0)).rounded_full().bg(accent))
                .child(
                    div()
                        .flex_1()
                        .rounded(px(4.0))
                        .bg(card)
                        .p(px(6.0))
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(div().h(px(4.0)).w(relative(0.7)).rounded_full().bg(line))
                        .child(div().h(px(4.0)).w(relative(0.45)).rounded_full().bg(line)),
                )
        };
        let preview = div()
            .h(px(78.0))
            .w_full()
            .flex()
            .rounded(px(6.0))
            .overflow_hidden()
            .map(|el| match value {
                AppTheme::Auto => el.child(mock(false)).child(mock(true)),
                AppTheme::Light => el.child(mock(false)),
                AppTheme::Dark => el.child(mock(true)),
            });
        let hover = theme.card_hover;
        let card_id = format!("theme-card-{label}");
        let rest = if selected {
            theme.accent_soft
        } else {
            theme.card
        };
        kit::hover_bg(
            k,
            div().id(eid(card_id.clone())),
            kit::hover_key(&card_id),
            rest,
            if selected { rest } else { hover },
        )
        .flex_1()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .p(px(8.0))
        .rounded(px(kit::CARD_RADIUS))
        .shadow(kit::card_shadow(theme))
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _, cx| {
            if this.settings().theme != value {
                this.edit_settings(cx, move |settings| settings.theme = value);
            }
        }))
        .child(preview)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .text_size(px(13.0))
                .when(selected, |el| el.font_weight(FontWeight::SEMIBOLD))
                .child(label),
        )
        .into_any_element()
    }

    /// Built-in designs followed by every installed VS Code theme, in a grid
    /// of equal columns.
    fn popup_theme_grid(&self, k: &Kit, cx: &mut Context<Self>) -> AnyElement {
        let installed = crate::vscode_themes::installed();
        let choices = PopupTheme::ALL
            .into_iter()
            .map(ThemeChoice::Builtin)
            .chain(installed.iter().cloned().map(ThemeChoice::VsCode));
        // Once per frame, not per card: see `vscode_palette`.
        let accent = crate::theme::accent_ramp(self.settings().accent_color);
        let cards = choices
            .map(|choice| self.popup_theme_card(k, choice, accent, cx))
            .collect::<Vec<_>>();
        div()
            .grid()
            .grid_cols(THEME_COLUMNS)
            .gap(px(12.0))
            .w_full()
            .children(cards)
            .into_any_element()
    }

    /// A miniature popup painted with one popup theme's own palette, in the
    /// light or dark mode Settings currently resolves to (VS Code themes
    /// always show their own base).
    fn popup_theme_card(
        &self,
        k: &Kit,
        choice: ThemeChoice,
        accent: crate::theme::AccentRamp,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = &k.theme;
        let (value, vscode) = match choice {
            ThemeChoice::Builtin(value) => (value, None),
            ThemeChoice::VsCode(vscode) => (PopupTheme::VsCode, Some(vscode)),
        };
        let selected = self.settings().popup_theme == value
            && vscode.as_ref().is_none_or(|vscode| {
                self.settings().popup_vscode_theme.as_deref() == Some(vscode.id.as_str())
            });
        let label: SharedString = match value {
            PopupTheme::Fluent => crate::i18n::tr("popup-theme-fluent").into(),
            PopupTheme::Vercel => crate::i18n::tr("popup-theme-vercel").into(),
            PopupTheme::VsCode => vscode.as_ref().map_or_else(
                || crate::i18n::tr("popup-theme-vscode").into(),
                |vscode| vscode.label.clone().into(),
            ),
        };
        // Every card carries a caption line so a row's cards stay level.
        let caption: SharedString = match &vscode {
            Some(vscode) => vscode
                .extension
                .clone()
                .unwrap_or_else(|| crate::i18n::tr("popup-theme-vscode").to_owned())
                .into(),
            None => crate::i18n::tr("popup-theme-built-in").into(),
        };
        let font = popup_theme::popup_font_family(
            value,
            self.settings().font_family.as_deref(),
            &self.fonts().text,
        );
        let palette = Palette::new(
            value,
            vscode.clone(),
            popup_theme::palette_dark(vscode.as_deref(), theme.dark),
            accent,
            PopupBackgroundMaterial::Solid,
            font.clone(),
        )
        .with_contrast(self.settings().popup_vscode_contrast)
        .with_borders(self.settings().popup_borders);
        let vscode_id = vscode.as_ref().map(|vscode| vscode.id.clone());
        let preview = popup_mock(k, &palette);
        let hover = theme.card_hover;
        let card_id = match &vscode_id {
            Some(id) => format!("popup-theme-card-vscode-{id}"),
            None => format!("popup-theme-card-{label}"),
        };
        let rest = if selected {
            theme.accent_soft
        } else {
            theme.card
        };
        let remove = vscode_id.clone().map(|id| {
            Button::icon_only(format!("{card_id}-remove"), "trash-fill")
                .ghost()
                .size(ButtonSize::Small)
                .tooltip(crate::i18n::tr("remove-theme"))
                .on_click(Self::h(cx, move |this, (), _, cx| {
                    this.remove_vscode_theme(id.clone(), cx)
                }))
                .render(k)
        });
        let title = div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .truncate()
                            .text_size(px(13.0))
                            .font_family(font)
                            .when(selected, |el| el.font_weight(FontWeight::SEMIBOLD))
                            .child(label),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(11.0))
                            .text_color(theme.text_tertiary)
                            .child(caption),
                    ),
            )
            .children(remove);
        let card = kit::hover_bg(
            k,
            div().id(eid(card_id.clone())),
            kit::hover_key(&card_id),
            rest,
            if selected { rest } else { hover },
        )
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .p(px(8.0))
        .rounded(px(kit::CARD_RADIUS))
        .shadow(kit::card_shadow(theme))
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _, cx| {
            if selected {
                return;
            }
            match vscode_id.clone() {
                Some(id) => this.select_vscode_theme(id, cx),
                None => this.edit_settings(cx, move |settings| settings.popup_theme = value),
            }
        }))
        .child(preview)
        .child(title)
        .into_any_element();
        // Newly installed themes fade in rather than popping into the grid.
        match &vscode {
            Some(_) => kit::appear(k, card_id, card),
            None => card,
        }
    }

    fn accent_swatches(
        &self,
        k: &mut Kit,
        current: AccentColor,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        fn labels() -> [&'static str; 8] {
            [
                crate::i18n::tr("windows"),
                crate::i18n::tr("blue"),
                crate::i18n::tr("purple"),
                crate::i18n::tr("pink"),
                crate::i18n::tr("red"),
                crate::i18n::tr("orange"),
                crate::i18n::tr("green"),
                crate::i18n::tr("teal"),
            ]
        }
        let (dark, on_accent) = (k.theme.dark, k.theme.on_accent);
        let fill = |accent| rgb8(crate::theme::accent_ramp(accent).fill(dark));
        kit::ColorSwatches {
            id: "accent".into(),
            presets: ACCENTS
                .into_iter()
                .zip(labels())
                .map(|(accent, label)| kit::Swatch {
                    label: label.into(),
                    fill: fill(accent),
                    ink: on_accent,
                    icon: (accent == AccentColor::Windows).then_some("desktop-fill"),
                })
                .collect(),
            selected: ACCENTS.iter().position(|accent| *accent == current),
            custom: matches!(current, AccentColor::Custom(_)).then(|| (fill(current), on_accent)),
            picker_rgb: crate::theme::accent_ramp(current).base,
            on_select: Self::h(cx, |this, index: usize, _, cx| {
                let accent = ACCENTS[index];
                if this.settings().accent_color != accent {
                    this.edit_settings(cx, move |settings| settings.accent_color = accent);
                }
            }),
            on_custom: Self::h(cx, |this, rgb: (u8, u8, u8), _, cx| {
                let next = AccentColor::custom(rgb);
                if this.settings().accent_color != next {
                    this.edit_settings(cx, move |settings| settings.accent_color = next);
                }
            }),
        }
        .render(k)
    }

    /// Windows, Light and Dark cards side by side.
    fn theme_cards(&self, k: &Kit, cx: &mut Context<Self>) -> AnyElement {
        let current = self.settings().theme;
        div()
            .flex()
            .gap(px(12.0))
            .w_full()
            .children(
                [
                    (AppTheme::Auto, crate::i18n::tr("windows")),
                    (AppTheme::Light, crate::i18n::tr("light")),
                    (AppTheme::Dark, crate::i18n::tr("dark")),
                ]
                .into_iter()
                .map(|(value, label)| self.theme_card(k, value, label, value == current, cx)),
            )
            .into_any_element()
    }
}

impl<T: ThemeHost> ThemePicker for T {}

impl SettingsWindow {
    pub(super) fn appearance_page(
        &mut self,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let s = &self.settings;
        let accent = s.accent_color;
        let time_format = s.time_format;
        let material = s.popup_background_material;
        let borders = s.popup_borders;
        let bar = s.bottom_bar_size;
        let radius = s.popup_corner_radius;
        let animations = s.animations_enabled;
        let vscode_selected = s.popup_theme == PopupTheme::VsCode;

        let theme_cards = self.theme_cards(k, cx);
        let colors = self.accent_swatches(k, accent, cx);
        let font = self.font_picker(k, window, cx);
        let look = kit::card_of(k, |k| {
            vec![
                Row::new("appearance-theme", crate::i18n::tr("color-theme"))
                    .description(
                        k,
                        crate::i18n::tr("applies-to-settings-the-popup-and-its-tray-menu"),
                    )
                    .detail(div().pt(px(12.0)).child(theme_cards).into_any_element())
                    .render(k),
                Row::new("appearance-accent", crate::i18n::tr("accent-color"))
                    .description(k, crate::i18n::tr("windows-follows-your-system-accent"))
                    .detail(div().pt(px(12.0)).child(colors).into_any_element())
                    .render(k),
                Row::new("appearance-font", crate::i18n::tr("font"))
                    .description(k, crate::i18n::tr("any-font-installed-on-this-pc"))
                    .trailing(font)
                    .render(k),
                Row::new("appearance-time", crate::i18n::tr("time-format"))
                    .trailing(kit::segmented(
                        k,
                        "appearance-time-format",
                        &[
                            crate::i18n::tr("msg-12-hour"),
                            crate::i18n::tr("msg-24-hour"),
                        ],
                        time_format.index().max(0) as usize,
                        false,
                        Self::h(cx, |this, index: usize, _, cx| {
                            let value = TimeFormat::from_index(index as i32);
                            value.apply();
                            this.edit(cx, move |settings| settings.time_format = value)
                        }),
                    ))
                    .render(k),
            ]
        });

        let popup_theme_cards = self.popup_theme_grid(k, cx);
        let vscode_row = self.vscode_themes_row(k, cx);
        let theme_row = Row::new("appearance-popup-theme", crate::i18n::tr("popup-theme"))
            .description(k, crate::i18n::tr("popup-theme-description"))
            .detail(
                div()
                    .pt(px(12.0))
                    .child(popup_theme_cards)
                    .into_any_element(),
            )
            .render(k);
        // These only tune VS Code themes, so they glide in with one; the
        // tint also needs a translucent backdrop.
        let contrast_row = self.vscode_percent_row(
            k,
            "appearance-vscode-contrast",
            crate::i18n::tr("popup-theme-contrast"),
            crate::i18n::tr("popup-theme-contrast-description"),
            self.settings.popup_vscode_contrast,
            (
                POPUP_VSCODE_CONTRAST_MIN,
                POPUP_VSCODE_CONTRAST_MAX,
                POPUP_VSCODE_CONTRAST_DEFAULT,
            ),
            |settings, value| settings.popup_vscode_contrast = value,
            cx,
        );
        let tint_row = self.vscode_percent_row(
            k,
            "appearance-vscode-tint",
            crate::i18n::tr("popup-theme-tint"),
            crate::i18n::tr("popup-theme-tint-description"),
            self.settings.popup_vscode_tint,
            (
                POPUP_VSCODE_TINT_MIN,
                POPUP_VSCODE_TINT_MAX,
                POPUP_VSCODE_TINT_DEFAULT,
            ),
            |settings, value| settings.popup_vscode_tint = value,
            cx,
        );
        let row_divider = |k: &Kit| {
            div()
                .px(px(kit::ROW_PADDING_X))
                .child(kit::divider(k))
                .into_any_element()
        };
        let contrast_divider = row_divider(k);
        let tint_divider = row_divider(k);
        let vscode_divider = row_divider(k);
        let tint = kit::collapsible(
            k,
            fx::key("appearance-vscode-tint"),
            material != PopupBackgroundMaterial::Solid,
            move |_| {
                div()
                    .flex()
                    .flex_col()
                    .child(tint_divider)
                    .child(tint_row)
                    .into_any_element()
            },
        );
        let contrast = kit::collapsible(
            k,
            fx::key("appearance-vscode-contrast"),
            vscode_selected,
            move |_| {
                div()
                    .flex()
                    .flex_col()
                    .child(contrast_divider)
                    .child(contrast_row)
                    .children(tint)
                    .into_any_element()
            },
        );
        let themes = kit::card_surface(
            k,
            std::iter::once(theme_row)
                .chain(contrast)
                .chain([vscode_divider, vscode_row]),
        )
        .into_any_element();

        let radius_value = radius.dip();
        let popup = kit::card_of(k, |k| {
            vec![
                Row::new("appearance-material", crate::i18n::tr("popup-background"))
                    .trailing(kit::segmented(
                        k,
                        "appearance-material",
                        &[
                            crate::i18n::tr("acrylic"),
                            crate::i18n::tr("mica"),
                            crate::i18n::tr("solid"),
                        ],
                        material.index().max(0) as usize,
                        false,
                        Self::h(cx, |this, index: usize, _, cx| {
                            let value = PopupBackgroundMaterial::from_index(index as i32);
                            this.edit(cx, move |settings| {
                                settings.popup_background_material = value
                            })
                        }),
                    ))
                    .render(k),
                Row::new("appearance-bar", crate::i18n::tr("bottom-bar-size"))
                    .trailing(kit::segmented(
                        k,
                        "appearance-bar",
                        &[crate::i18n::tr("comfortable"), crate::i18n::tr("compact")],
                        bar.index().max(0) as usize,
                        false,
                        Self::h(cx, |this, index: usize, _, cx| {
                            let value = BottomBarSize::from_index(index as i32);
                            this.edit(cx, move |settings| settings.bottom_bar_size = value)
                        }),
                    ))
                    .render(k),
                Row::new("appearance-radius", crate::i18n::tr("popup-corner-radius"))
                    .trailing(kit::slider(
                        k,
                        "appearance-radius",
                        radius_value as f32,
                        SliderRange {
                            min: 0.0,
                            max: 20.0,
                            step: 4.0,
                        },
                        160.0,
                        Self::h(cx, move |this, value: f32, _, cx| {
                            let value = PopupCornerRadius::from_dip(value.round() as i32);
                            if this.settings.popup_corner_radius != value {
                                this.edit(cx, move |settings| settings.popup_corner_radius = value)
                            }
                        }),
                    ))
                    .trailing(
                        div()
                            .w(px(40.0))
                            .text_right()
                            .text_size(px(13.0))
                            .text_color(k.theme.text_secondary)
                            .child(format!("{radius_value} px"))
                            .into_any_element(),
                    )
                    .render(k),
                kit::toggle_row(
                    k,
                    "appearance-popup-borders",
                    crate::i18n::tr("popup-borders"),
                    Some(crate::i18n::tr("popup-borders-description").into()),
                    borders,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |settings| settings.popup_borders = value)
                    }),
                ),
            ]
        });

        let motion = kit::card_of(k, |k| {
            vec![kit::toggle_row(
                k,
                "appearance-animations",
                crate::i18n::tr("animation-effects"),
                Some(
                    crate::i18n::tr(
                        "glide-transitions-in-the-popup-and-settings-windows-own-animation",
                    )
                    .into(),
                ),
                animations,
                Self::h(cx, |this, value: bool, _, cx| {
                    crate::theme::set_animations_enabled(value);
                    this.edit(cx, move |settings| settings.animations_enabled = value)
                }),
            )]
        });

        vec![
            look,
            kit::section_heading(k, crate::i18n::tr("popup")),
            themes,
            popup,
            kit::section_heading(k, crate::i18n::tr("motion")),
            motion,
        ]
    }

    /// A percent slider for VS Code popup themes, with a reset to `default`.
    #[allow(clippy::too_many_arguments)]
    fn vscode_percent_row(
        &self,
        k: &mut Kit,
        id: &'static str,
        title: &'static str,
        description: &'static str,
        value: u16,
        (min, max, default): (u16, u16, u16),
        set: fn(&mut Settings, u16),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let reset = Button::icon_only(format!("{id}-reset"), "arrow-counter-clockwise-bold")
            .ghost()
            .size(ButtonSize::Small)
            .tooltip(crate::i18n::tr("reset"))
            .disabled(value == default)
            .on_click(Self::h(cx, move |this, (), _, cx| {
                this.edit(cx, move |settings| set(settings, default))
            }))
            .render(k);
        let slider = kit::slider(
            k,
            id,
            f32::from(value),
            SliderRange {
                min: f32::from(min),
                max: f32::from(max),
                step: 5.0,
            },
            160.0,
            Self::h(cx, move |this, next: f32, _, cx| {
                let next = next.round() as u16;
                if next != value {
                    this.edit(cx, move |settings| set(settings, next))
                }
            }),
        );
        let value_label = kit::text(format!("{value}%"), 13.0, k.theme.text_secondary)
            .w(px(40.0))
            .text_right()
            .into_any_element();
        Row::new(id, title)
            .description(k, description)
            .trailing(reset)
            .trailing(slider)
            .trailing(value_label)
            .render(k)
    }

    /// Searchable font ComboBox: recently picked families, the Windows
    /// default, then every installed family. Hovering a family for a moment
    /// previews it in the popup.
    fn font_picker(
        &mut self,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let input = self.font_search_input(window, cx);
        let open = k.menus.is_open(FONT_PICKER);
        // The list closes from many places (a pick, Escape, a click outside,
        // another menu); settle the preview and the focus whichever it was.
        if self.font_picker.open && !open {
            self.font_picker.open = false;
            self.font_picker.hovered = None;
            self.font_picker.preview_task = None;
            cx.defer(end_font_preview);
            if input.read(cx).is_focused(window) {
                window.focus(&self.focus_handle(cx));
            }
        }
        let query = input.read(cx).text().trim().to_lowercase();
        let current = self.settings.font_family.clone();
        let default_label = SharedString::from(crate::i18n::tr("windows-default"));
        let mut families = self.font_families.clone();
        // A font removed since it was chosen stays visible as the selection.
        if let Some(name) = &current
            && !families.iter().any(|family| family.as_ref() == name)
        {
            families.push(name.clone().into());
        }
        let mut choices: Vec<FontChoice> = Vec::new();
        let mut recent_count = 0;
        if query.is_empty() {
            let recent = self
                .settings
                .recent_font_families
                .iter()
                .filter_map(|name| families.iter().find(|family| family.as_ref() == name))
                .take(RECENT_FONTS)
                .cloned()
                .map(Some)
                .collect::<Vec<_>>();
            recent_count = recent.len();
            choices.extend(recent);
            choices.push(None);
            choices.extend(families.iter().cloned().map(Some));
        } else {
            if default_label.to_lowercase().contains(&query) {
                choices.push(None);
            }
            choices.extend(
                families
                    .iter()
                    .filter(|family| family.to_lowercase().contains(&query))
                    .cloned()
                    .map(Some),
            );
        }
        let selected = choices
            .iter()
            .position(|choice| choice.as_ref().map(|name| name.as_ref()) == current.as_deref());
        let picker = &mut self.font_picker;
        picker.highlighted = picker
            .highlighted
            .filter(|index| *index < choices.len())
            .or_else(|| (!query.is_empty() && !choices.is_empty()).then_some(0));
        picker.selected = selected;
        picker.choices = choices.clone();
        let highlighted = picker.highlighted;
        let reveal = picker.reveal.take();
        let fresh = std::mem::take(&mut picker.fresh);

        let items = if choices.is_empty() {
            vec![kit::MenuItem::new(crate::i18n::tr("no-matching-fonts")).disabled()]
        } else {
            choices
                .iter()
                .enumerate()
                .map(|(index, choice)| {
                    let item =
                        kit::MenuItem::new(choice.clone().unwrap_or_else(|| default_label.clone()));
                    if index + 1 == recent_count {
                        item.separator()
                    } else {
                        item
                    }
                })
                .collect()
        };
        let label = current.map_or(default_label, SharedString::from);
        let on_select = {
            let choices = choices.clone();
            Self::h(cx, move |this, index: usize, _, cx| {
                if let Some(choice) = choices.get(index).cloned() {
                    this.choose_font(choice, cx);
                }
            })
        };
        let on_hover = Self::h(cx, move |this, (index, hovered): (usize, bool), _, cx| {
            let Some(choice) = choices.get(index).cloned() else {
                return;
            };
            if hovered {
                this.hover_font(Some(choice), cx);
            } else if this.font_picker.hovered.as_ref() == Some(&choice) {
                this.hover_font(None, cx);
            }
        });
        let on_open = {
            let input = input.clone();
            Self::h(cx, move |this, (), window, cx| {
                let picker = &mut this.font_picker;
                picker.open = true;
                picker.fresh = true;
                picker.highlighted = None;
                picker.reveal = None;
                input.update(cx, |input, cx| input.set_text("", cx));
                window.focus(&input.focus_handle(cx));
                cx.notify();
            })
        };
        let combo = kit::SearchDropdown {
            id: FONT_PICKER.into(),
            label,
            input,
            items,
            selected,
            highlighted,
            reveal,
            fresh,
            width: FONT_PICKER_WIDTH,
            on_open,
            on_select,
            on_hover,
        }
        .render(k, window, cx);
        div()
            .on_key_down(cx.listener(Self::font_picker_key))
            .child(combo)
            .into_any_element()
    }

    /// The picker's search field, created on first use.
    fn font_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        if let Some(input) = &self.font_picker.input {
            return input.clone();
        }
        let input = cx.new(|cx| TextInput::new(window, cx));
        let subscription = cx.subscribe_in(
            &input,
            window,
            |this: &mut Self, _, event: &InputEvent, window, cx| match event {
                InputEvent::Changed => {
                    // A new query starts from its best match at the top.
                    this.font_picker.highlighted = None;
                    this.font_picker.reveal = Some(0);
                    cx.notify();
                }
                InputEvent::Submit => {
                    let choice = this
                        .font_picker
                        .highlighted
                        .and_then(|index| this.font_picker.choices.get(index).cloned());
                    this.kit.menus.close(window);
                    if let Some(choice) = choice {
                        this.choose_font(choice, cx);
                    }
                    cx.notify();
                }
                InputEvent::Blur => {}
            },
        );
        self.font_picker.input = Some(input.clone());
        self.font_picker._subscription = Some(subscription);
        input
    }

    /// Up and Down walk the list while its field has focus.
    fn font_picker_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.font_picker.open {
            return;
        }
        let picker = &mut self.font_picker;
        let Some(last) = picker.choices.len().checked_sub(1) else {
            return;
        };
        let from = picker.highlighted.or(picker.selected);
        let next = match event.keystroke.key.as_str() {
            "down" => from.map_or(0, |index| (index + 1).min(last)),
            "up" => from.map_or(0, |index| index.saturating_sub(1)),
            "pagedown" => from.map_or(0, |index| (index + 8).min(last)),
            "pageup" => from.map_or(0, |index| index.saturating_sub(8)),
            _ => return,
        };
        picker.highlighted = Some(next);
        picker.reveal = Some(next);
        let choice = picker.choices[next].clone();
        self.hover_font(Some(choice), cx);
        cx.stop_propagation();
        cx.notify();
    }

    /// Point the popup preview at `target` once it has stayed put for
    /// [`FONT_PREVIEW_DELAY`]; `None` returns the popup to the saved font.
    fn hover_font(&mut self, target: Option<FontChoice>, cx: &mut Context<Self>) {
        if self.font_picker.hovered == target {
            return;
        }
        self.font_picker.hovered = target.clone();
        self.font_picker.preview_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FONT_PREVIEW_DELAY).await;
            let _ = this.update(cx, |this, cx| {
                if this.font_picker.hovered != target || !this.font_picker.open {
                    return;
                }
                let preview = target.map(|choice| choice.map(|name| name.to_string()));
                if popup_theme::set_font_preview(preview) {
                    cx.refresh_windows();
                }
            });
        }));
    }

    /// Save `choice` and move it to the front of the recent families.
    fn choose_font(&mut self, choice: FontChoice, cx: &mut Context<Self>) {
        self.font_picker.hovered = None;
        self.font_picker.preview_task = None;
        end_font_preview(cx);
        let next = choice.map(|name| name.to_string());
        let recent_first =
            next.is_none() || self.settings.recent_font_families.first() == next.as_ref();
        if self.settings.font_family == next && recent_first {
            return;
        }
        self.edit(cx, move |settings| {
            settings.font_family = next.clone();
            if let Some(name) = &next {
                settings
                    .recent_font_families
                    .retain(|recent| recent != name);
                settings.recent_font_families.insert(0, name.clone());
                settings.recent_font_families.truncate(RECENT_FONTS);
            }
        });
    }
}

/// Drop a font preview and repaint the popup with the saved font.
pub(super) fn end_font_preview(cx: &mut App) {
    if popup_theme::set_font_preview(None) {
        cx.refresh_windows();
    }
}
