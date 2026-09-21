use super::*;

const DIALOG_WIDTH: f64 = 440.0;
const DIALOG_SCRIM: Color = Color {
    a: 102,
    r: 0,
    g: 0,
    b: 0,
};

pub(super) fn picker_overlay(
    picker: &crate::troubleshoot::ToolPickerState,
    set_picker: AsyncSetState<Option<crate::troubleshoot::ToolPickerState>>,
) -> Element {
    let labels = picker
        .tools
        .iter()
        .map(|tool| tool.tool.label())
        .collect::<Vec<_>>();
    let set_for_selection = set_picker.clone();
    let picker_for_selection = picker.clone();
    let combo = ComboBox::new(labels)
        .selected_index(picker.selected_index)
        .header("AI tool")
        .on_selection_changed(move |selected_index| {
            let mut next = picker_for_selection.clone();
            next.selected_index = selected_index;
            set_for_selection.call(Some(next));
        });

    let selected = usize::try_from(picker.selected_index)
        .ok()
        .and_then(|index| picker.tools.get(index))
        .cloned();
    let set_for_run = set_picker.clone();
    let run = Button::new("Open terminal")
        .accent()
        .enabled(selected.is_some())
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .on_click(move || {
            if let Some(tool) = selected.clone() {
                if let Err(error) = crate::troubleshoot::launch_selected(&tool) {
                    eprintln!("failed to start troubleshooting terminal: {error:#}");
                    crate::notifications::show(
                        "Troubleshooting could not start",
                        &error.to_string(),
                    );
                }
            }
            set_for_run.call(None);
        })
        .grid_column(0);
    let set_for_cancel = set_picker.clone();
    let cancel = Button::new("Cancel")
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .on_click(move || set_for_cancel.call(None))
        .grid_column(1);

    let card = border(
        vstack((
            vstack((
                text_block("Run Troubleshoot with AI")
                    .font_size(20.0)
                    .semibold(),
                text_block("Choose which installed AI tool should investigate the problem.")
                    .font_size(12.0)
                    .opacity(0.72)
                    .wrap(),
                combo,
            ))
            .spacing(14.0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .padding(Thickness::uniform(24.0)),
            border(
                grid((run, cancel))
                    .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
                    .column_spacing(8.0)
                    .horizontal_alignment(HorizontalAlignment::Stretch),
            )
            .padding(Thickness::uniform(24.0))
            .background(ThemeRef::LayerFill)
            .border_thickness(Thickness {
                left: 0.0,
                top: 1.0,
                right: 0.0,
                bottom: 0.0,
            })
            .border_brush(ThemeRef::CardStroke),
        ))
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .background(ThemeRef::SolidBackground)
    .corner_radius(8.0)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .width(DIALOG_WIDTH)
    .horizontal_alignment(HorizontalAlignment::Center)
    .vertical_alignment(VerticalAlignment::Center)
    .on_tapped(|| {});

    let dismiss = set_picker.clone();
    let dismiss_escape = set_picker;
    relative_panel::<Vec<Element>>(vec![
        border(Element::Empty)
            .background(DIALOG_SCRIM)
            .relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .relative_align_bottom()
            .on_tapped(move || dismiss.call(None))
            .into(),
        card.relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .relative_align_bottom()
            .into(),
    ])
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Stretch)
    .keyboard_accelerator(KeyboardAccelerator::new(
        VirtualKey::Escape,
        VirtualKeyModifiers::None,
        move || dismiss_escape.call(None),
    ))
    .with_key("troubleshoot-picker-overlay")
    .into()
}
