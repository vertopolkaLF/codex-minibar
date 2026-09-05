use super::persistence::{persist_bool, persist_update};
use super::shared::settings_section_heading;
use super::*;

pub(super) fn render(ctx: &SettingsPageContext<'_>) -> (&'static str, Vec<Element>) {
    let theme = ctx.theme;
    let accent_color = ctx.accent_color;
    let animations_enabled = ctx.animations_enabled;
    let bottom_bar_size = ctx.bottom_bar_size;
    let popup_corner_radius = ctx.popup_corner_radius;
    let time_format = ctx.time_format;
    let use_colored_sidebar_icons = ctx.use_colored_sidebar_icons;
    let set_theme = ctx.set_theme.clone();
    let set_accent_color = ctx.set_accent_color.clone();
    let set_animations_enabled = ctx.set_animations_enabled.clone();
    let set_bottom_bar_size = ctx.set_bottom_bar_size.clone();
    let set_popup_corner_radius = ctx.set_popup_corner_radius.clone();
    let set_time_format = ctx.set_time_format.clone();
    let set_use_colored_sidebar_icons = ctx.set_use_colored_sidebar_icons.clone();
    let theme_navigation_guard = ctx.theme_navigation_guard.clone();
    let theme_navigation_guard_timer = ctx.theme_navigation_guard_timer.clone();
    let hovered_card_id = ctx.hovered_card_id;
    let set_hovered_card_id = ctx.set_hovered_card_id.clone();
    let settings_tx = ctx.settings_tx.clone();
    let apply_theme = settings_tx.clone();
    let apply_accent_color = settings_tx.clone();
    let apply_animations_enabled = settings_tx.clone();
    let apply_bottom_bar_size = settings_tx.clone();
    let apply_popup_corner_radius = settings_tx.clone();
    let apply_time_format = settings_tx.clone();
    let apply_use_colored_sidebar_icons = settings_tx.clone();
    let appearance_rows = vec![
        settings_control_card(
            "Color theme",
            None,
            ComboBox::new(["Windows", "Light", "Dark"])
                .selected_index(theme.index())
                .on_selection_changed(move |choice| {
                    let value = AppTheme::from_index(choice);
                    theme_navigation_guard.set(true);
                    let guard = theme_navigation_guard.clone();
                    match DispatcherTimer::new_one_shot(Duration::from_millis(350), move || {
                        guard.set(false)
                    }) {
                        Ok(timer) => theme_navigation_guard_timer.set(Some(timer)),
                        Err(_) => theme_navigation_guard.set(false),
                    }
                    set_theme.call(value);
                    crate::theme::apply_appearance(value, accent_color);
                    persist_update(apply_theme.clone(), move |settings| {
                        settings.theme = value;
                    });
                }),
            "appearance-theme",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("appearance-theme"),
        settings_control_card(
            "Accent color",
            None,
            ComboBox::new([
                "Windows", "Blue", "Purple", "Pink", "Red", "Orange", "Green", "Teal",
            ])
            .selected_index(accent_color.index())
            .on_selection_changed(move |choice| {
                let value = AccentColor::from_index(choice);
                set_accent_color.call(value);
                crate::theme::apply_appearance(theme, value);
                persist_update(apply_accent_color.clone(), move |settings| {
                    settings.accent_color = value;
                });
            }),
            "appearance-accent",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("appearance-accent"),
        settings_toggle_card(
            "Colored sidebar icons",
            use_colored_sidebar_icons,
            {
                let set_use_colored_sidebar_icons = set_use_colored_sidebar_icons.clone();
                let apply_use_colored_sidebar_icons = apply_use_colored_sidebar_icons.clone();
                move |value| {
                    persist_bool(
                        set_use_colored_sidebar_icons.clone(),
                        apply_use_colored_sidebar_icons.clone(),
                        value,
                        |settings, value| {
                            settings.use_colored_sidebar_icons = value;
                        },
                    );
                }
            },
            "appearance-colored-sidebar-icons",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("appearance-colored-sidebar-icons"),
        settings_control_card(
            "Time format",
            None,
            ComboBox::new(["12-hour", "24-hour"])
                .selected_index(time_format.index())
                .on_selection_changed(move |choice| {
                    let value = TimeFormat::from_index(choice);
                    set_time_format.call(value);
                    value.apply();
                    persist_update(apply_time_format.clone(), move |settings| {
                        settings.time_format = value;
                    });
                }),
            "appearance-time-format",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("appearance-time-format"),
        settings_section_heading("Popup").with_key("appearance-popup-heading"),
        settings_control_card(
            "Bottom bar size",
            None,
            ComboBox::new(["Comfortable", "Compact"])
                .selected_index(bottom_bar_size.index())
                .on_selection_changed(move |choice| {
                    let value = BottomBarSize::from_index(choice);
                    set_bottom_bar_size.call(value);
                    crate::popup::apply_popup_appearance(value, popup_corner_radius);
                    persist_update(apply_bottom_bar_size.clone(), move |settings| {
                        settings.bottom_bar_size = value;
                    });
                }),
            "appearance-bottom-bar-size",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("appearance-bottom-bar-size"),
        settings_control_card(
            "Popup corner radius",
            None,
            grid((
                text_block(format!("{} DIP", popup_corner_radius.dip()))
                    .foreground(ThemeRef::SecondaryText)
                    .grid_row(0)
                    .grid_column(1)
                    .horizontal_alignment(HorizontalAlignment::Right),
                Slider::new(f64::from(popup_corner_radius.dip()))
                    .range(8.0, 20.0)
                    .step(4.0)
                    .on_value_changed(move |raw_value: f64| {
                        let value = PopupCornerRadius::from_dip(raw_value.round() as i32);
                        if value == popup_corner_radius {
                            return;
                        }
                        set_popup_corner_radius.call(value);
                        crate::popup::apply_popup_appearance(bottom_bar_size, value);
                        persist_update(apply_popup_corner_radius.clone(), move |settings| {
                            settings.popup_corner_radius = value;
                        });
                    })
                    .grid_row(1)
                    .grid_column_span(2)
                    .margin(Thickness {
                        left: 0.0,
                        top: 2.0,
                        right: 0.0,
                        bottom: 0.0,
                    }),
            ))
            .columns([GridLength::Star(1.0), GridLength::Auto])
            .rows([GridLength::Auto, GridLength::Auto])
            .horizontal_alignment(HorizontalAlignment::Stretch),
            "appearance-popup-corner-radius",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("appearance-popup-corner-radius"),
        settings_section_heading("Motion").with_key("appearance-motion-heading"),
        settings_toggle_card(
            "Animation effects",
            animations_enabled,
            move |value| {
                crate::theme::set_animations_enabled(value);
                persist_bool(
                    set_animations_enabled.clone(),
                    apply_animations_enabled.clone(),
                    value,
                    |settings, value| settings.animations_enabled = value,
                );
            },
            "appearance-animations",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("appearance-animations"),
    ];
    ("Appearance", appearance_rows)
}
