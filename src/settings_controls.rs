//! Reusable controls shared by settings pages.

use std::{
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

use windows_reactor::*;

use crate::theme::{CONTROL_FASTER_ANIMATION, CONTROL_NORMAL_ANIMATION};

const CARD_RADIUS: f64 = 8.0;
const CARD_PADDING_X: f64 = 16.0;
const CARD_ROW_HEIGHT: f64 = 60.0;
/// Space between the toggle and the expander chevron (Windows Settings ≈ 6px).
const TOGGLE_CHEVRON_GAP: f64 = 6.0;
const CHEVRON_SIZE: f64 = 28.0;
/// Divider + slider block height used while animating expand/collapse.
const EXPAND_BODY_HEIGHT: f64 = 78.0;

/// Generation counter so overlapping expand animations don't fight.
static EXPAND_ANIM_GEN: AtomicU64 = AtomicU64::new(0);

pub(crate) fn card_is_hovered(hovered_id: &Option<String>, id: &str) -> bool {
    hovered_id.as_deref() == Some(id)
}

fn card_hover_handlers(
    card_id: &'static str,
    set_hovered_id: SetState<Option<String>>,
) -> (
    impl Fn(PointerEventInfo) + Clone + 'static,
    impl Fn() + Clone + 'static,
) {
    let enter = {
        let set_hovered_id = set_hovered_id.clone();
        move |_: PointerEventInfo| set_hovered_id.call(Some(card_id.to_string()))
    };
    let exit = move || set_hovered_id.call(None);
    (enter, exit)
}

/// Base card fill + stroke (Fluent card chrome) and WinUI-timed hover tint.
fn card_background_layers(hovered: bool) -> (Element, Element) {
    let base = border(Element::Empty)
        .background(ThemeRef::CardBackground)
        .corner_radius(CARD_RADIUS)
        .border_thickness(Thickness::uniform(1.0))
        .border_brush(ThemeRef::CardStroke)
        .relative_align_left()
        .relative_align_right()
        .relative_align_top()
        .relative_align_bottom()
        .into();
    let hover = border(Element::Empty)
        .background(ThemeRef::SubtleFill)
        .opacity(if hovered { 1.0 } else { 0.0 })
        .with_opacity_transition(CONTROL_FASTER_ANIMATION)
        .corner_radius(CARD_RADIUS)
        .relative_align_left()
        .relative_align_right()
        .relative_align_top()
        .relative_align_bottom()
        .into();
    (base, hover)
}

/// Fluent settings card with a status label and a native WinUI toggle pinned
/// to the trailing edge.
///
/// Keep the explicit toggle width constraints here: the default WinUI
/// template reserves an invisible content slot even when its labels are empty.
/// Tapping anywhere on the card (except the switch itself) flips the value.
pub(crate) fn settings_toggle_card(
    label: impl Into<String>,
    value: bool,
    on_toggled: impl IntoCallback<bool>,
    card_id: &'static str,
    hovered_id: &Option<String>,
    set_hovered_id: SetState<Option<String>>,
) -> Element {
    settings_toggle_card_with_description(
        label,
        None,
        value,
        on_toggled,
        card_id,
        hovered_id,
        set_hovered_id,
    )
}

/// Fluent settings card with an optional explanatory line beneath its label.
pub(crate) fn settings_toggle_card_with_description(
    label: impl Into<String>,
    description: Option<&str>,
    value: bool,
    on_toggled: impl IntoCallback<bool>,
    card_id: &'static str,
    hovered_id: &Option<String>,
    set_hovered_id: SetState<Option<String>>,
) -> Element {
    let hovered = card_is_hovered(hovered_id, card_id);
    let (on_enter, on_exit) = card_hover_handlers(card_id, set_hovered_id);
    let (base, hover) = card_background_layers(hovered);
    let on_toggled = on_toggled.into_callback();
    let label = label.into();
    let label_content: Element = match description {
        Some(description) => vstack((
            text_block(label).font_size(14.0),
            text_block(description).font_size(12.0).opacity(0.72),
        ))
        .into(),
        None => text_block(label).into(),
    };
    let toggle_card = {
        let on_toggled = on_toggled.clone();
        move || on_toggled.invoke(!value)
    };

    let children: Vec<Element> = vec![
        base,
        hover,
        // Transparent fill so empty card space is hit-testable (null bg is not).
        border(Element::Empty)
            .background(Color::transparent())
            .relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .relative_align_bottom()
            .on_tapped({
                let toggle_card = toggle_card.clone();
                move || toggle_card()
            })
            .into(),
        label_content
            .margin(Thickness {
                left: CARD_PADDING_X,
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
            })
            .relative_align_left()
            .relative_align_v_center()
            .on_tapped({
                let toggle_card = toggle_card.clone();
                move || toggle_card()
            })
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
            .on_tapped(move || toggle_card())
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
                right: CARD_PADDING_X,
                bottom: 0.0,
            })
            .relative_align_right()
            .relative_align_v_center()
            .into(),
    ];

    relative_panel(children)
        .height(CARD_ROW_HEIGHT)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .background(Color::transparent())
        .on_pointer_entered(on_enter)
        .on_pointer_exited(on_exit)
        .into()
}

/// Animate `progress` from its current visual target toward `expanded`.
///
/// Height is driven every frame so siblings in a `vstack` reflow smoothly —
/// layout animations are not wired in the WinUI backend yet.
pub(crate) fn animate_expand_progress(
    expanded: bool,
    set_expanded: SetState<bool>,
    set_progress: AsyncSetState<f64>,
) {
    let next = !expanded;
    set_expanded.call(next);
    let to = if next { 1.0 } else { 0.0 };
    let from = if next { 0.0 } else { 1.0 };
    let anim_id = EXPAND_ANIM_GEN.fetch_add(1, Ordering::Relaxed) + 1;
    let duration = CONTROL_NORMAL_ANIMATION;
    thread::spawn(move || {
        let start = Instant::now();
        loop {
            if EXPAND_ANIM_GEN.load(Ordering::Relaxed) != anim_id {
                return;
            }
            let t = (start.elapsed().as_secs_f64() / duration.as_secs_f64()).min(1.0);
            // Ease-out cubic.
            let eased = 1.0 - (1.0 - t).powi(3);
            set_progress.call(from + (to - from) * eased);
            if t >= 1.0 {
                break;
            }
            thread::sleep(Duration::from_millis(16));
        }
        if EXPAND_ANIM_GEN.load(Ordering::Relaxed) == anim_id {
            set_progress.call(to);
        }
    });
}

/// Settings-style expanding option card: toggle in the header, nested content
/// below. Toggle never hides or disables `content`. Tapping anywhere on the
/// header row (except the toggle itself) expands/collapses with animation.
pub(crate) fn settings_toggle_expander(
    label: impl Into<String>,
    enabled: bool,
    on_toggled: impl IntoCallback<bool>,
    expanded: bool,
    expand_progress: f64,
    set_expanded: SetState<bool>,
    set_expand_progress: AsyncSetState<f64>,
    card_id: &'static str,
    hovered_id: &Option<String>,
    set_hovered_id: SetState<Option<String>>,
    content: impl Into<Element>,
) -> Element {
    let hovered = card_is_hovered(hovered_id, card_id);
    let (on_enter, on_exit) = card_hover_handlers(card_id, set_hovered_id);

    let toggle_expand = {
        let set_expanded = set_expanded.clone();
        let set_expand_progress = set_expand_progress.clone();
        move || animate_expand_progress(expanded, set_expanded.clone(), set_expand_progress.clone())
    };

    let progress = expand_progress.clamp(0.0, 1.0);

    // Trailing controls share one v-centered stack so the chevron cannot drift
    // relative to the toggle (RelativePanel + composition rotation used to).
    let trailing = hstack((
        text_block(if enabled { "On" } else { "Off" })
            .vertical_alignment(VerticalAlignment::Center)
            .margin(Thickness {
                left: 0.0,
                top: 0.0,
                right: 12.0,
                bottom: 0.0,
            })
            .on_tapped({
                let toggle_expand = toggle_expand.clone();
                move || toggle_expand()
            }),
        ToggleSwitch::new(enabled)
            .on_content("")
            .off_content("")
            .on_toggled(on_toggled)
            .min_width(0.0)
            .max_width(50.0)
            .width(50.0)
            .vertical_alignment(VerticalAlignment::Center)
            .margin(Thickness {
                left: 0.0,
                top: 0.0,
                right: TOGGLE_CHEVRON_GAP,
                bottom: 0.0,
            }),
        // Fixed hit-box; glyph centered inside. Rotation pivots on this box.
        border(
            text_block("\u{E70D}")
                .font_family("Segoe Fluent Icons")
                .font_size(12.0)
                .horizontal_alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Center),
        )
        .width(CHEVRON_SIZE)
        .height(CHEVRON_SIZE)
        .background(Color::transparent())
        .rotation(progress * 180.0)
        .vertical_alignment(VerticalAlignment::Center)
        .on_tapped({
            let toggle_expand = toggle_expand.clone();
            move || toggle_expand()
        }),
    ))
    .spacing(0.0)
    .margin(Thickness {
        left: 0.0,
        top: 0.0,
        right: CARD_PADDING_X,
        bottom: 0.0,
    })
    .relative_align_right()
    .relative_align_v_center();

    let header_children: Vec<Element> = vec![
        // Transparent fill so empty header space is hit-testable (null bg is not).
        border(Element::Empty)
            .background(Color::transparent())
            .relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .relative_align_bottom()
            .on_tapped({
                let toggle_expand = toggle_expand.clone();
                move || toggle_expand()
            })
            .into(),
        text_block(label)
            .margin(Thickness {
                left: CARD_PADDING_X,
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
            })
            .relative_align_left()
            .relative_align_v_center()
            .on_tapped({
                let toggle_expand = toggle_expand.clone();
                move || toggle_expand()
            })
            .into(),
        trailing.into(),
    ];

    let header = relative_panel(header_children)
        .height(CARD_ROW_HEIGHT)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .background(Color::transparent());

    let body_height = EXPAND_BODY_HEIGHT * progress;
    let body = border(
        vstack((
            border(Element::Empty)
                .height(1.0)
                .background(ThemeRef::CardStroke)
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .margin(Thickness {
                    left: CARD_PADDING_X,
                    top: 0.0,
                    right: CARD_PADDING_X,
                    bottom: 0.0,
                }),
            border(content.into())
                .padding(Thickness {
                    left: CARD_PADDING_X,
                    top: 8.0,
                    right: CARD_PADDING_X,
                    bottom: 8.0,
                })
                .horizontal_alignment(HorizontalAlignment::Stretch),
        ))
        .spacing(0.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .opacity(progress),
    )
    .height(body_height)
    .max_height(body_height)
    .horizontal_alignment(HorizontalAlignment::Stretch);

    let shell_children: Vec<Element> = {
        let (base, hover) = card_background_layers(hovered);
        // Convert relative-panel layers into a bordered stack overlay by
        // placing fills behind the column content via a RelativePanel shell.
        vec![
            base,
            hover,
            vstack((header, body))
                .spacing(0.0)
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .relative_align_left()
                .relative_align_right()
                .relative_align_top()
                .relative_align_bottom()
                .into(),
        ]
    };

    relative_panel(shell_children)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .background(Color::transparent())
        .on_pointer_entered(on_enter)
        .on_pointer_exited(on_exit)
        .into()
}

/// Nested slider row for use inside [`settings_toggle_expander`] content.
///
/// Label and percent sit on opposite sides (space-between); the slider spans
/// the full content width.
pub(crate) fn settings_slider_content(
    label: impl Into<String>,
    value: u8,
    minimum: u8,
    maximum: u8,
    step: u8,
    on_changed: impl IntoCallback<f64>,
) -> Element {
    grid((
        text_block(label)
            .grid_row(0)
            .grid_column(0)
            .horizontal_alignment(HorizontalAlignment::Left)
            .vertical_alignment(VerticalAlignment::Center),
        text_block(format!("{value}%"))
            .foreground(ThemeRef::SecondaryText)
            .grid_row(0)
            .grid_column(1)
            .horizontal_alignment(HorizontalAlignment::Right)
            .vertical_alignment(VerticalAlignment::Center),
        Slider::new(f64::from(value))
            .range(f64::from(minimum), f64::from(maximum))
            .step(f64::from(step))
            .on_value_changed(on_changed)
            .grid_row(1)
            .grid_column_span(2)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .margin(Thickness {
                left: 0.0,
                top: 6.0,
                right: 0.0,
                bottom: 0.0,
            }),
    ))
    .columns([GridLength::Star(1.0), GridLength::Auto])
    .rows([GridLength::Auto, GridLength::Auto])
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

/// Read-only settings row (no hover — it isn't interactive).
pub(crate) fn settings_info_card(label: impl Into<String>, value: impl Into<String>) -> Element {
    border(
        grid((
            text_block(label)
                .grid_column(0)
                .vertical_alignment(VerticalAlignment::Center),
            text_block(value)
                .foreground(ThemeRef::SecondaryText)
                .grid_column(1)
                .horizontal_alignment(HorizontalAlignment::Right)
                .vertical_alignment(VerticalAlignment::Center),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(Thickness {
        left: CARD_PADDING_X,
        top: 10.0,
        right: CARD_PADDING_X,
        bottom: 10.0,
    })
    .background(ThemeRef::CardBackground)
    .corner_radius(CARD_RADIUS)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

/// Action card with trailing button + WinUI-timed hover.
pub(crate) fn settings_action_card(
    label: impl Into<String>,
    button_label: impl Into<String>,
    on_click: impl IntoUnitCallback,
    card_id: &'static str,
    hovered_id: &Option<String>,
    set_hovered_id: SetState<Option<String>>,
) -> Element {
    let hovered = card_is_hovered(hovered_id, card_id);
    let (on_enter, on_exit) = card_hover_handlers(card_id, set_hovered_id);
    let (base, hover) = card_background_layers(hovered);

    let children: Vec<Element> = vec![
        base,
        hover,
        text_block(label)
            .margin(Thickness {
                left: CARD_PADDING_X,
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
            })
            .relative_align_left()
            .relative_align_v_center()
            .into(),
        Button::new(button_label)
            .accent()
            .on_click(on_click)
            .margin(Thickness {
                left: 0.0,
                top: 0.0,
                right: CARD_PADDING_X,
                bottom: 0.0,
            })
            .relative_align_right()
            .relative_align_v_center()
            .into(),
    ];

    relative_panel(children)
        .height(CARD_ROW_HEIGHT)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .background(Color::transparent())
        .on_pointer_entered(on_enter)
        .on_pointer_exited(on_exit)
        .into()
}
