//! Reusable controls shared by settings pages.

use windows_reactor::*;

/// Fluent settings card with a status label and a native WinUI toggle pinned
/// to the trailing edge.
///
/// Keep the explicit toggle width constraints here: the default WinUI
/// template reserves an invisible content slot even when its labels are empty.
pub(crate) fn settings_toggle_card(
    label: impl Into<String>,
    value: bool,
    on_toggled: impl IntoCallback<bool>,
) -> Element {
    let children: Vec<Element> = vec![
        border(Element::Empty)
            .background(ThemeRef::CardBackground)
            .corner_radius(8.0)
            .border_thickness(Thickness::uniform(1.0))
            .border_brush(ThemeRef::CardStroke)
            .relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .relative_align_bottom()
            .into(),
        text_block(label)
            .margin(Thickness {
                left: 16.0,
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
            })
            .relative_align_left()
            .relative_align_v_center()
            .into(),
        text_block(if value { "On" } else { "Off" })
            .margin(Thickness {
                left: 0.0,
                top: 0.0,
                right: 78.0,
                bottom: 0.0,
            })
            .relative_align_right()
            .relative_align_v_center()
            .into(),
        ToggleSwitch::new(value)
            .on_content("")
            .off_content("")
            .on_toggled(on_toggled)
            .min_width(0.0)
            .max_width(50.0)
            .width(50.0)
            .margin(Thickness {
                left: 0.0,
                top: 0.0,
                right: 16.0,
                bottom: 0.0,
            })
            .relative_align_right()
            .relative_align_v_center()
            .into(),
    ];

    relative_panel(children)
        .height(60.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into()
}
