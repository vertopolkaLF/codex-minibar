//! Appearance: theme, accent, icons, time format, popup material and motion.

use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px, relative,
};

use super::kit::{self, Kit, Row, SliderRange, eid};
use super::window::SettingsWindow;
use crate::popup_window::ui::theme::{HslaExt, rgb8};
use crate::settings::{
    AccentColor, AppTheme, BottomBarSize, PopupBackgroundMaterial, PopupCornerRadius, TimeFormat,
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
                (AppTheme::Auto, "Windows"),
                (AppTheme::Light, "Light"),
                (AppTheme::Dark, "Dark"),
            ]
            .into_iter()
            .map(|(value, label)| self.theme_card(k, value, label, value == theme, cx)),
        );
        let colors = self.accent_swatches(k, accent, cx);
        let look = kit::card_of(k, |k| {
            vec![
                Row::new("appearance-theme", "Color theme")
                    .description(k, "Applies to Settings, the popup and its tray menu.")
                    .detail(div().pt(px(12.0)).child(theme_cards).into_any_element())
                    .render(k),
                Row::new("appearance-accent", "Accent color")
                    .description(k, "Windows follows your system accent.")
                    .detail(div().pt(px(12.0)).child(colors).into_any_element())
                    .render(k),
                kit::toggle_row(
                    k,
                    "appearance-mono-sidebar",
                    "Use monochrome icons",
                    Some("Show single-color glyphs in the Settings sidebar.".into()),
                    !colored_sidebar,
                    Self::h(cx, |this, mono: bool, _, cx| {
                        this.edit(cx, move |settings| {
                            settings.use_colored_sidebar_icons = !mono
                        })
                    }),
                ),
                Row::new("appearance-time", "Time format")
                    .trailing(kit::segmented(
                        k,
                        "appearance-time-format",
                        &["12-hour", "24-hour"],
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

        let radius_value = radius.dip();
        let popup = kit::card_of(k, |k| {
            vec![
                Row::new("appearance-material", "Popup background")
                    .trailing(kit::segmented(
                        k,
                        "appearance-material",
                        &["Acrylic", "Mica"],
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
                Row::new("appearance-bar", "Bottom bar size")
                    .trailing(kit::segmented(
                        k,
                        "appearance-bar",
                        &["Comfortable", "Compact"],
                        bar.index().max(0) as usize,
                        false,
                        Self::h(cx, |this, index: usize, _, cx| {
                            let value = BottomBarSize::from_index(index as i32);
                            this.edit(cx, move |settings| settings.bottom_bar_size = value)
                        }),
                    ))
                    .render(k),
                Row::new("appearance-radius", "Popup corner radius")
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
                "Animation effects",
                Some(
                    "Glide transitions in the popup and Settings. Windows' own animation setting is also respected."
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
            kit::section_heading(k, "Popup"),
            popup,
            kit::section_heading(k, "Motion"),
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
            .border_1()
            .border_color(theme.card_stroke)
            .map(|el| match value {
                AppTheme::Auto => el.child(mock(false)).child(mock(true)),
                AppTheme::Light => el.child(mock(false)),
                AppTheme::Dark => el.child(mock(true)),
            });
        let hover = theme.card_hover;
        div()
            .id(eid(format!("theme-card-{label}")))
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .p(px(8.0))
            .rounded(px(kit::CARD_RADIUS))
            .border_2()
            .border_color(if selected {
                theme.accent
            } else {
                theme.card_stroke
            })
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
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

    fn accent_swatches(&self, k: &Kit, current: AccentColor, cx: &mut Context<Self>) -> AnyElement {
        let theme = &k.theme;
        let mut row = div().flex().flex_wrap().gap(px(10.0));
        for (index, accent) in ACCENTS.into_iter().enumerate() {
            let ramp = crate::theme::accent_ramp(accent);
            let fill = rgb8(ramp.fill(theme.dark));
            let selected = accent == current;
            let label = [
                "Windows", "Blue", "Purple", "Pink", "Red", "Orange", "Green", "Teal",
            ][index];
            let swatch = div()
                .id(eid(format!("accent-{label}")))
                .size(px(34.0))
                .rounded_full()
                .p(px(3.0))
                .border_2()
                .border_color(if selected {
                    theme.text
                } else {
                    gpui::transparent_black()
                })
                .cursor_pointer()
                .hover({
                    let ring = theme.text.alpha(0.4);
                    move |style| {
                        if selected {
                            style
                        } else {
                            style.border_color(ring)
                        }
                    }
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.settings.accent_color != accent {
                        this.edit(cx, move |settings| settings.accent_color = accent);
                    }
                }))
                .child(
                    div()
                        .size_full()
                        .rounded_full()
                        .bg(fill)
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(accent == AccentColor::Windows, |el| {
                            el.child(kit::icon("desktop-fill", 12.0, theme.on_accent))
                        })
                        .when(selected && accent != AccentColor::Windows, |el| {
                            el.child(kit::icon("check-bold", 12.0, theme.on_accent))
                        }),
                );
            row = row.child(kit::with_tooltip(k, swatch, label));
        }
        row.into_any_element()
    }
}
