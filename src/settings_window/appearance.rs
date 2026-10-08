//! Appearance: theme, accent, font, icons, time format, popup material and motion.

use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px, relative,
};

use super::kit::{self, Kit, Row, SliderRange, eid};
use super::window::SettingsWindow;
use crate::popup_window::ui::theme::{self as popup_theme, Palette, rgb8};
use crate::settings::{
    AccentColor, AppTheme, BottomBarSize, PopupBackgroundMaterial, PopupCornerRadius, PopupTheme,
    TimeFormat,
};

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

impl SettingsWindow {
    pub(super) fn appearance_page(
        &mut self,
        k: &mut Kit,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let s = &self.settings;
        let (theme, accent) = (s.theme, s.accent_color);
        let colored_sidebar = s.use_colored_sidebar_icons;
        let time_format = s.time_format;
        let material = s.popup_background_material;
        let bar = s.bottom_bar_size;
        let radius = s.popup_corner_radius;
        let animations = s.animations_enabled;

        let theme_cards = div().flex().gap(px(12.0)).w_full().children(
            [
                (AppTheme::Auto, crate::i18n::tr("windows")),
                (AppTheme::Light, crate::i18n::tr("light")),
                (AppTheme::Dark, crate::i18n::tr("dark")),
            ]
            .into_iter()
            .map(|(value, label)| self.theme_card(k, value, label, value == theme, cx)),
        );
        let colors = self.accent_swatches(k, accent, cx);
        let font = self.font_picker(k, cx);
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
                Row::new("appearance-icon-style", crate::i18n::tr("icons-style"))
                    .description(k, crate::i18n::tr("glyph-style-in-the-settings-sidebar"))
                    .trailing(kit::segmented(
                        k,
                        "appearance-icon-style",
                        &[crate::i18n::tr("colored"), crate::i18n::tr("monochrome")],
                        if colored_sidebar { 0 } else { 1 },
                        false,
                        Self::h(cx, |this, index: usize, _, cx| {
                            this.edit(cx, move |settings| {
                                settings.use_colored_sidebar_icons = index == 0
                            })
                        }),
                    ))
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

        let popup_theme_cards = div().flex().gap(px(12.0)).w_full().children(
            PopupTheme::ALL
                .into_iter()
                .map(|value| self.popup_theme_card(k, value, cx)),
        );
        let themes = kit::card_of(k, |k| {
            vec![
                Row::new("appearance-popup-theme", crate::i18n::tr("popup-theme"))
                    .description(k, crate::i18n::tr("popup-theme-description"))
                    .detail(
                        div()
                            .pt(px(12.0))
                            .child(popup_theme_cards)
                            .into_any_element(),
                    )
                    .render(k),
            ]
        });

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
            if this.settings.theme != value {
                this.edit(cx, move |settings| settings.theme = value);
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

    /// A miniature popup painted with one popup theme's own palette, in the
    /// light or dark mode Settings currently resolves to.
    fn popup_theme_card(&self, k: &Kit, value: PopupTheme, cx: &mut Context<Self>) -> AnyElement {
        let theme = &k.theme;
        let selected = self.settings.popup_theme == value;
        let label = match value {
            PopupTheme::Fluent => crate::i18n::tr("popup-theme-fluent"),
            PopupTheme::Vercel => crate::i18n::tr("popup-theme-vercel"),
        };
        let font = popup_theme::popup_font_family(
            value,
            self.settings.font_family.as_deref(),
            &popup_theme::default_font_family(cx),
        );
        let palette = Palette::new(
            value,
            theme.dark,
            crate::theme::accent_ramp(self.settings.accent_color),
            PopupBackgroundMaterial::Solid,
            font.clone(),
        );
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
        let preview = div()
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
            .border_color(theme.divider)
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
            );
        let hover = theme.card_hover;
        let card_id = format!("popup-theme-card-{label}");
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
            if this.settings.popup_theme != value {
                this.edit(cx, move |settings| settings.popup_theme = value);
            }
        }))
        .child(preview)
        .child(
            div()
                .text_size(px(13.0))
                .font_family(font)
                .when(selected, |el| el.font_weight(FontWeight::SEMIBOLD))
                .child(label),
        )
        .into_any_element()
    }

    /// Installed font families, led by the Windows default.
    fn font_picker(&self, k: &Kit, cx: &mut Context<Self>) -> AnyElement {
        let current = self.settings.font_family.clone();
        let mut families = self.font_families.clone();
        // A font removed since it was chosen stays visible as the selection.
        if let Some(name) = &current
            && !families.iter().any(|family| family.as_ref() == name)
        {
            families.push(name.clone().into());
        }
        let selected = match &current {
            None => 0,
            Some(name) => {
                1 + families
                    .iter()
                    .position(|family| family.as_ref() == name)
                    .unwrap_or_default()
            }
        };
        let mut options = vec![SharedString::from(crate::i18n::tr("windows-default"))];
        options.extend(families.iter().cloned());
        kit::dropdown(
            k,
            "appearance-font",
            options,
            Some(selected),
            false,
            200.0,
            Self::h(cx, move |this, index: usize, _, cx| {
                let next = index
                    .checked_sub(1)
                    .and_then(|index| families.get(index))
                    .map(ToString::to_string);
                if this.settings.font_family != next {
                    this.edit(cx, move |settings| settings.font_family = next.clone());
                }
            }),
        )
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
                if this.settings.accent_color != accent {
                    this.edit(cx, move |settings| settings.accent_color = accent);
                }
            }),
            on_custom: Self::h(cx, |this, rgb: (u8, u8, u8), _, cx| {
                let next = AccentColor::custom(rgb);
                if this.settings.accent_color != next {
                    this.edit(cx, move |settings| settings.accent_color = next);
                }
            }),
        }
        .render(k)
    }
}
