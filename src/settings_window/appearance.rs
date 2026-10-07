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
                        &["Acrylic", "Mica", "Solid"],
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

    fn accent_swatches(&self, k: &mut Kit, current: AccentColor, cx: &mut Context<Self>) -> AnyElement {
        let theme = &k.theme;
        let mut row = div().flex().flex_wrap().gap(px(10.0));
        for (index, accent) in ACCENTS.into_iter().enumerate() {
            let ramp = crate::theme::accent_ramp(accent);
            let fill = rgb8(ramp.fill(theme.dark));
            let selected = accent == current;
            let label = [
                "Windows", "Blue", "Purple", "Pink", "Red", "Orange", "Green", "Teal",
            ][index];
            let swatch_id = format!("accent-{label}");
            // Selection is a tinted halo, not an outline.
            let rest = if selected {
                fill.alpha(0.35)
            } else {
                gpui::transparent_black()
            };
            let swatch = kit::hover_bg(
                k,
                div().id(eid(swatch_id.clone())),
                kit::hover_key(&swatch_id),
                rest,
                if selected { rest } else { theme.subtle_hover },
            )
            .size(px(34.0))
            .rounded_full()
            .p(px(4.0))
            .cursor_pointer()
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
        row.child(self.custom_accent_swatch(k, current, cx))
            .into_any_element()
    }

    /// Swatch that opens a flyout with red/green/blue sliders for any color.
    fn custom_accent_swatch(
        &self,
        k: &mut Kit,
        current: AccentColor,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        const MENU: &str = "accent-custom";
        let custom = matches!(current, AccentColor::Custom(_));
        let open = k.menus.is_open(MENU);
        let base = crate::theme::accent_ramp(current).base;
        let (dark, on_accent, subtle_hover, control, text_secondary) = {
            let theme = &k.theme;
            (
                theme.dark,
                theme.on_accent,
                theme.subtle_hover,
                theme.control,
                theme.text_secondary,
            )
        };
        let fill = rgb8(crate::theme::accent_ramp(current).fill(dark));
        let rest = if custom {
            fill.alpha(0.35)
        } else {
            gpui::transparent_black()
        };
        let menus = k.menus.clone();
        let trigger = kit::hover_bg(
            k,
            div().id(eid("accent-custom")),
            kit::hover_key("accent-custom"),
            rest,
            if custom { rest } else { subtle_hover },
        )
        .size(px(34.0))
        .rounded_full()
        .p(px(4.0))
        .cursor_pointer()
        .on_click(move |_, window, _| menus.toggle(MENU.into(), window))
        .child(
            div()
                .size_full()
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .map(|el| {
                    if custom {
                        el.bg(fill).child(kit::icon("check-bold", 12.0, on_accent))
                    } else {
                        el.bg(control)
                            .child(kit::icon("plus-bold", 12.0, text_secondary))
                    }
                }),
        );
        let trigger = kit::with_tooltip(k, trigger, "Custom color").into_any_element();

        let content = open.then(|| {
            // Keep the picked hue/saturation while the color is grey or black,
            // where RGB alone cannot tell them.
            let hsv = resolve_hsv(base);
            let pick = |this: &mut SettingsWindow, hsv: [f32; 3], cx: &mut Context<SettingsWindow>| {
                let rgb = kit::hsv_to_rgb(hsv[0], hsv[1], hsv[2]);
                let next = AccentColor::custom(rgb);
                remember_hsv(rgb, hsv);
                if this.settings.accent_color != next {
                    this.edit(cx, move |settings| settings.accent_color = next);
                }
            };
            let area = kit::color_area(
                k,
                "accent-custom-area",
                (240.0, 160.0),
                hsv,
                Self::h(cx, move |this, (x, y): (f32, f32), _, cx| {
                    let current = resolve_hsv(crate::theme::accent_ramp(this.settings.accent_color).base);
                    pick(this, [current[0], x, 1.0 - y], cx)
                }),
            );
            let strip = kit::hue_strip(
                k,
                "accent-custom-hue",
                240.0,
                hsv[0],
                Self::h(cx, move |this, hue: f32, _, cx| {
                    let current = resolve_hsv(crate::theme::accent_ramp(this.settings.accent_color).base);
                    pick(this, [hue, current[1], current[2]], cx)
                }),
            );
            div()
                .flex()
                .flex_col()
                .gap(px(14.0))
                .child(area)
                .child(strip)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(div().size(px(24.0)).rounded_full().bg(rgb8(base)))
                        .child(kit::text(
                            format!("#{:02X}{:02X}{:02X}", base.0, base.1, base.2),
                            13.0,
                            text_secondary,
                        )),
                )
                .into_any_element()
        });
        kit::flyout(k, MENU.into(), trigger, content)
    }
}

thread_local! {
    /// The last picked color with its exact HSV, so dragging through grey or
    /// black does not lose hue and round-trip rounding does not jitter.
    static LAST_HSV: std::cell::Cell<(u32, [f32; 3])> = const { std::cell::Cell::new((u32::MAX, [0.0; 3])) };
}

fn pack(rgb: (u8, u8, u8)) -> u32 {
    (u32::from(rgb.0) << 16) | (u32::from(rgb.1) << 8) | u32::from(rgb.2)
}

fn remember_hsv(rgb: (u8, u8, u8), hsv: [f32; 3]) {
    LAST_HSV.with(|last| last.set((pack(rgb), hsv)));
}

fn resolve_hsv(rgb: (u8, u8, u8)) -> [f32; 3] {
    let (packed, hsv) = LAST_HSV.with(|last| last.get());
    if packed == pack(rgb) {
        hsv
    } else {
        kit::rgb_to_hsv(rgb)
    }
}
