//! Reusable controls shared by settings pages.

use std::{
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

use windows_reactor::*;

use crate::theme::{CONTROL_FASTER_ANIMATION, CONTROL_NORMAL_ANIMATION, duration};

const CARD_RADIUS: f64 = 8.0;
/// Fallback bound for expandable bodies without a known intrinsic height.
const EXPANDABLE_BODY_MAX_HEIGHT: f64 = 512.0;
/// Shared inset for settings cards and supporting panels.
pub(crate) const SETTINGS_CARD_PADDING: f64 = 16.0;

/// Create the standard settings-card inset without repeating raw dimensions.
pub(crate) fn settings_card_padding() -> Thickness {
    Thickness::uniform(SETTINGS_CARD_PADDING)
}

const CARD_PADDING_X: f64 = SETTINGS_CARD_PADDING;
const CARD_ROW_HEIGHT: f64 = 60.0;
const CARD_CONTENT_PADDING_Y: f64 = SETTINGS_CARD_PADDING;
/// Keep wrapped labels clear of the status text and toggle switch.
const CARD_TRAILING_RESERVE: f64 = 148.0;
/// Space between the toggle and the expander chevron (Windows Settings ≈ 6px).
const TOGGLE_CHEVRON_GAP: f64 = 6.0;
const CHEVRON_SIZE: f64 = 28.0;
/// Generation counter so overlapping expand animations don't fight.
static EXPAND_ANIM_GEN: AtomicU64 = AtomicU64::new(0);

pub(crate) fn card_is_hovered(hovered_id: &Option<String>, id: &str) -> bool {
    hovered_id.as_deref() == Some(id)
}

fn card_hover_handlers(
    card_id: impl Into<String>,
    set_hovered_id: SetState<Option<String>>,
) -> (
    impl Fn(PointerEventInfo) + Clone + 'static,
    impl Fn() + Clone + 'static,
) {
    let card_id = card_id.into();
    let enter = {
        let set_hovered_id = set_hovered_id.clone();
        move |_: PointerEventInfo| set_hovered_id.call(Some(card_id.clone()))
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
        .with_opacity_transition(duration(CONTROL_FASTER_ANIMATION))
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
            text_block(label).font_size(14.0).wrap(),
            text_block(description).font_size(12.0).opacity(0.72).wrap(),
        ))
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into(),
        None => text_block(label).wrap().into(),
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
                top: CARD_CONTENT_PADDING_Y,
                right: CARD_TRAILING_RESERVE,
                bottom: CARD_CONTENT_PADDING_Y,
            })
            .relative_align_left()
            .relative_align_right()
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
        .min_height(CARD_ROW_HEIGHT)
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
    animate_expand_progress_between(
        if expanded { 1.0 } else { 0.0 },
        if next { 1.0 } else { 0.0 },
        set_progress,
    );
}

fn animate_expand_progress_between(from: f64, to: f64, set_progress: AsyncSetState<f64>) {
    let anim_id = EXPAND_ANIM_GEN.fetch_add(1, Ordering::Relaxed) + 1;
    let duration = duration(CONTROL_NORMAL_ANIMATION);
    if duration.is_zero() {
        set_progress.call(to);
        return;
    }
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

/// Shared expandable settings-card shell used by notification and Customize
/// cards. The trailing control is deliberately injected so switches and
/// checkboxes keep exactly the same card chrome and hit-test geometry.
fn settings_expander_card_with_header(
    header: impl Into<Element>,
    trailing: Option<Element>,
    expanded: bool,
    expand_progress: f64,
    expanded_body_height: Option<f64>,
    toggle_expand: impl Fn() + Clone + 'static,
    header_is_tappable: bool,
    card_id: impl Into<String>,
    hovered_id: &Option<String>,
    set_hovered_id: SetState<Option<String>>,
    content: impl Into<Element>,
) -> Element {
    let card_id = card_id.into();
    let hovered = card_is_hovered(hovered_id, &card_id);
    let (on_enter, on_exit) = card_hover_handlers(card_id, set_hovered_id);
    let progress = expand_progress.clamp(0.0, 1.0);

    let trailing_reserve = if trailing.is_some() { 80.0 } else { 0.0 };
    let trailing = trailing.map(|trailing| {
        border(trailing)
            .background(Color::transparent())
            .margin(Thickness {
                left: 0.0,
                top: 0.0,
                right: CARD_PADDING_X + CHEVRON_SIZE + TOGGLE_CHEVRON_GAP,
                bottom: 0.0,
            })
            .relative_align_right()
            .relative_align_v_center()
    });
    let chevron = border(
        crate::icons::element("caret-down", 16.0, Color::rgb(138, 138, 138))
            .horizontal_alignment(HorizontalAlignment::Center)
            .vertical_alignment(VerticalAlignment::Center),
    )
    .width(CHEVRON_SIZE)
    .height(CHEVRON_SIZE)
    .background(Color::transparent())
    .rotation(progress * 180.0)
    .margin(Thickness {
        left: 0.0,
        top: 0.0,
        right: CARD_PADDING_X,
        bottom: 0.0,
    })
    .relative_align_right()
    .relative_align_v_center()
    .on_tapped({
        let toggle_expand = toggle_expand.clone();
        move || toggle_expand()
    });

    let header_content = header
        .into()
        .margin(Thickness {
            left: CARD_PADDING_X,
            top: CARD_CONTENT_PADDING_Y,
            right: CARD_PADDING_X + CHEVRON_SIZE + TOGGLE_CHEVRON_GAP + trailing_reserve,
            bottom: CARD_CONTENT_PADDING_Y,
        })
        .relative_align_left()
        .relative_align_v_center();
    let header_content: Element = if header_is_tappable {
        header_content
            .on_tapped({
                let toggle_expand = toggle_expand.clone();
                move || toggle_expand()
            })
            .into()
    } else {
        header_content.into()
    };
    let mut header_children: Vec<Element> = vec![
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
        header_content,
    ];
    if let Some(trailing) = trailing {
        header_children.push(trailing.into());
    }
    header_children.push(chevron.into());
    let header = relative_panel(header_children)
    .min_height(CARD_ROW_HEIGHT)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .background(Color::transparent());

    let body_content = border(
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
                    top: CARD_CONTENT_PADDING_Y,
                    right: CARD_PADDING_X,
                    bottom: CARD_CONTENT_PADDING_Y,
                })
                .horizontal_alignment(HorizontalAlignment::Stretch),
        ))
        .spacing(0.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .opacity(progress),
    )
    .horizontal_alignment(HorizontalAlignment::Stretch);
    let body: Element = match expanded_body_height {
        Some(expanded_body_height) => {
            let body_height = expanded_body_height * progress;
            body_content
                .height(body_height)
                .max_height(body_height)
                .into()
        }
        None if expanded || progress > 0.0 => body_content
            .max_height(EXPANDABLE_BODY_MAX_HEIGHT * progress)
            .into(),
        None => Element::Empty,
    };

    let (base, hover) = card_background_layers(hovered);
    relative_panel(vec![
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
    ])
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .background(Color::transparent())
    .on_pointer_entered(on_enter)
    .on_pointer_exited(on_exit)
    .into()
}

fn settings_expander_label(label: impl Into<String>, description: Option<&str>) -> Element {
    match description {
        Some(description) => vstack((
            text_block(label).font_size(14.0).wrap(),
            text_block(description).font_size(12.0).opacity(0.72).wrap(),
        ))
        .spacing(2.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into(),
        None => text_block(label).wrap().into(),
    }
}

fn settings_expander_card(
    label: impl Into<String>,
    description: Option<&str>,
    trailing: impl Into<Element>,
    expanded: bool,
    expand_progress: f64,
    expanded_body_height: Option<f64>,
    toggle_expand: impl Fn() + Clone + 'static,
    card_id: impl Into<String>,
    hovered_id: &Option<String>,
    set_hovered_id: SetState<Option<String>>,
    content: impl Into<Element>,
) -> Element {
    settings_expander_card_with_header(
        settings_expander_label(label, description),
        Some(trailing.into()),
        expanded,
        expand_progress,
        expanded_body_height,
        toggle_expand,
        true,
        card_id,
        hovered_id,
        set_hovered_id,
        content,
    )
}

#[derive(Clone, PartialEq)]
struct CheckboxExpanderProps {
    label: String,
    checked: bool,
    on_checked: Callback<bool>,
    expanded: bool,
    on_expanding: Callback<bool>,
    expanded_body_height: Option<f64>,
    card_id: String,
    hovered_id: Option<String>,
    set_hovered_id: SetState<Option<String>>,
    content: Element,
}

struct CheckboxExpander;

impl Component<CheckboxExpanderProps> for CheckboxExpander {
    fn render(&self, props: &CheckboxExpanderProps, cx: &mut RenderCx) -> Element {
        let (progress, set_progress) =
            cx.use_async_state(if props.expanded { 1.0_f64 } else { 0.0_f64 });
        let previous_expanded = cx.use_ref(props.expanded);
        if previous_expanded.get_cloned() != props.expanded {
            previous_expanded.set(props.expanded);
            animate_expand_progress_between(
                progress,
                if props.expanded { 1.0 } else { 0.0 },
                set_progress.clone(),
            );
        }

        let next_expanded = !props.expanded;
        let toggle_expand = {
            let on_expanding = props.on_expanding.clone();
            let set_progress = set_progress.clone();
            let from = progress;
            move || {
                on_expanding.invoke(next_expanded);
                animate_expand_progress_between(
                    from,
                    if next_expanded { 1.0 } else { 0.0 },
                    set_progress.clone(),
                );
            }
        };

        settings_expander_card(
            props.label.clone(),
            None,
            settings_section_all_toggle(props.checked, props.on_checked.clone()),
            props.expanded,
            progress,
            props.expanded_body_height,
            toggle_expand,
            props.card_id.clone(),
            &props.hovered_id,
            props.set_hovered_id.clone(),
            props.content.clone(),
        )
    }
}

#[derive(Clone, PartialEq)]
struct ContentExpanderProps {
    header: Element,
    trailing: Option<Element>,
    expanded: bool,
    on_expanding: Callback<bool>,
    card_id: String,
    hovered_id: Option<String>,
    set_hovered_id: SetState<Option<String>>,
    content: Element,
}

struct ContentExpander;

impl Component<ContentExpanderProps> for ContentExpander {
    fn render(&self, props: &ContentExpanderProps, cx: &mut RenderCx) -> Element {
        let (progress, set_progress) =
            cx.use_async_state(if props.expanded { 1.0_f64 } else { 0.0_f64 });
        let content_ref = cx.use_ref(props.content.clone());
        if props.content != Element::Empty {
            content_ref.set(props.content.clone());
        }
        let previous_expanded = cx.use_ref(props.expanded);
        if previous_expanded.get_cloned() != props.expanded {
            previous_expanded.set(props.expanded);
            animate_expand_progress_between(
                progress,
                if props.expanded { 1.0 } else { 0.0 },
                set_progress.clone(),
            );
        }

        let next_expanded = !props.expanded;
        let toggle_expand = {
            let on_expanding = props.on_expanding.clone();
            let set_progress = set_progress.clone();
            let from = progress;
            move || {
                on_expanding.invoke(next_expanded);
                animate_expand_progress_between(
                    from,
                    if next_expanded { 1.0 } else { 0.0 },
                    set_progress.clone(),
                );
            }
        };

        settings_expander_card_with_header(
            props.header.clone(),
            props.trailing.clone(),
            props.expanded,
            progress,
            None,
            toggle_expand,
            false,
            props.card_id.clone(),
            &props.hovered_id,
            props.set_hovered_id.clone(),
            content_ref.get_cloned(),
        )
    }
}

/// Settings-style expanding option card: toggle in the header, nested content
/// below. Toggle never hides or disables `content`. Tapping anywhere on the
/// header row (except the switch itself) flips the expansion state.
pub(crate) fn settings_toggle_expander(
    label: impl Into<String>,
    description: Option<&str>,
    enabled: bool,
    on_toggled: impl IntoCallback<bool>,
    expanded: bool,
    expand_progress: f64,
    expanded_body_height: Option<f64>,
    set_expanded: SetState<bool>,
    set_expand_progress: AsyncSetState<f64>,
    card_id: &'static str,
    hovered_id: &Option<String>,
    set_hovered_id: SetState<Option<String>>,
    content: impl Into<Element>,
) -> Element {
    let toggle_expand = {
        let set_expanded = set_expanded.clone();
        let set_expand_progress = set_expand_progress.clone();
        move || animate_expand_progress(expanded, set_expanded.clone(), set_expand_progress.clone())
    };
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
    ));

    settings_expander_card(
        label,
        description,
        trailing,
        expanded,
        expand_progress,
        expanded_body_height,
        toggle_expand,
        card_id,
        hovered_id,
        set_hovered_id,
        content,
    )
}

/// Checkbox variant of the notification expander card. Customize uses it for
/// provider visibility so those rows share the notification card's chrome.
pub(crate) fn settings_checkbox_expander(
    label: impl Into<String>,
    checked: bool,
    on_checked: impl IntoCallback<bool>,
    expanded: bool,
    on_expanding: impl IntoCallback<bool>,
    expanded_body_height: Option<f64>,
    card_id: impl Into<String>,
    hovered_id: &Option<String>,
    set_hovered_id: SetState<Option<String>>,
    content: impl Into<Element>,
) -> Element {
    component(
        CheckboxExpander,
        CheckboxExpanderProps {
            label: label.into(),
            checked,
            on_checked: on_checked.into_callback(),
            expanded,
            on_expanding: on_expanding.into_callback(),
            expanded_body_height,
            card_id: card_id.into(),
            hovered_id: hovered_id.clone(),
            set_hovered_id,
            content: content.into(),
        },
    )
}

/// Settings-style expandable card with arbitrary header content (no toggle).
///
/// Matches [`settings_toggle_card`] chrome: 8px radius, 16px horizontal padding,
/// 60px header row. Used by tray widget rows so they share option-card sizing.
pub(crate) fn settings_content_expander(
    header: impl Into<Element>,
    expanded: bool,
    on_expanding: impl IntoCallback<bool>,
    card_id: impl Into<String>,
    hovered_id: &Option<String>,
    set_hovered_id: SetState<Option<String>>,
    content: impl Into<Element>,
) -> Element {
    settings_content_expander_with_trailing(
        header,
        None,
        expanded,
        on_expanding,
        card_id,
        hovered_id,
        set_hovered_id,
        content,
    )
}

/// Same chrome as [`settings_content_expander`], plus a trailing header control
/// that sits above the expand tap target so it does not toggle the card.
pub(crate) fn settings_content_expander_with_trailing(
    header: impl Into<Element>,
    trailing: Option<Element>,
    expanded: bool,
    on_expanding: impl IntoCallback<bool>,
    card_id: impl Into<String>,
    hovered_id: &Option<String>,
    set_hovered_id: SetState<Option<String>>,
    content: impl Into<Element>,
) -> Element {
    component(
        ContentExpander,
        ContentExpanderProps {
            header: header.into(),
            trailing,
            expanded,
            on_expanding: on_expanding.into_callback(),
            card_id: card_id.into(),
            hovered_id: hovered_id.clone(),
            set_hovered_id,
            content: content.into(),
        },
    )
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

/// Fluent settings card with a descriptive label and a trailing native control.
/// Use this for choices such as ComboBox settings so no option floats outside
/// the settings-card layout.
pub(crate) fn settings_control_card(
    label: impl Into<String>,
    description: Option<&str>,
    control: impl Into<Element>,
    card_id: &'static str,
    hovered_id: &Option<String>,
    set_hovered_id: SetState<Option<String>>,
) -> Element {
    const CONTROL_WIDTH: f64 = 160.0;
    let hovered = card_is_hovered(hovered_id, card_id);
    let (on_enter, on_exit) = card_hover_handlers(card_id, set_hovered_id);
    let (base, hover) = card_background_layers(hovered);
    let label = label.into();
    let label_content: Element = match description {
        Some(description) => vstack((
            text_block(label).font_size(14.0).wrap(),
            text_block(description).font_size(12.0).opacity(0.72).wrap(),
        ))
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into(),
        None => text_block(label).wrap().into(),
    };

    relative_panel(vec![
        base,
        hover,
        label_content
            .margin(Thickness {
                left: CARD_PADDING_X,
                top: CARD_CONTENT_PADDING_Y,
                right: CONTROL_WIDTH + CARD_PADDING_X * 2.0,
                bottom: CARD_CONTENT_PADDING_Y,
            })
            .relative_align_left()
            .relative_align_right()
            .relative_align_v_center()
            .into(),
        control
            .into()
            .width(CONTROL_WIDTH)
            .margin(Thickness {
                left: 0.0,
                top: 0.0,
                right: CARD_PADDING_X,
                bottom: 0.0,
            })
            .relative_align_right()
            .relative_align_v_center()
            .into(),
    ])
    .min_height(CARD_ROW_HEIGHT)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .background(Color::transparent())
    .on_pointer_entered(on_enter)
    .on_pointer_exited(on_exit)
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
    .padding(settings_card_padding())
    .background(ThemeRef::CardBackground)
    .corner_radius(CARD_RADIUS)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

/// Accent update action. Text keeps the control accessible without an icon font.
pub(crate) fn update_accent_button(
    label: impl Into<String>,
    on_click: impl IntoUnitCallback,
) -> Button {
    Button::new(label).accent().on_click(on_click)
}

/// Compact nav-pane card: version label stacked above the update action.
pub(crate) fn update_available_nav_card(
    version: impl AsRef<str>,
    on_click: impl IntoUnitCallback,
) -> Element {
    border(
        vstack((
            text_block(format!("{} available!", version.as_ref()))
                .font_size(13.0)
                .horizontal_alignment(HorizontalAlignment::Center),
            update_accent_button("Update", on_click)
                .horizontal_alignment(HorizontalAlignment::Stretch),
        ))
        .spacing(10.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(settings_card_padding())
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

    let action_button = Button::new(button_label).accent().on_click(on_click);

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
        action_button
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

/// Default CheckBox style is 32×120 for touch. Icon-only boxes override
/// both; the 32px chrome is the template's check glyph host.
/// https://github.com/microsoft/microsoft-ui-xaml/issues/2671
const POPUP_BRICK_CHECK_COL_PX: f64 = 32.0;
const POPUP_BRICK_ROW_HEIGHT: f64 = 32.0;
/// Shared Home/Tab column width — wide enough for the "Home" header.
const POPUP_BRICK_LABEL_COL_PX: f64 = 44.0;
/// `NormalRectangle` in the default CheckBox template is 20×20, left-aligned
/// in the 32px host. Headers and checkboxes are optically centered on that glyph.
const POPUP_BRICK_GLYPH_PX: f64 = 20.0;

fn popup_brick_columns() -> [GridLength; 3] {
    [
        GridLength::Star(1.0),
        GridLength::Pixel(POPUP_BRICK_LABEL_COL_PX),
        GridLength::Pixel(POPUP_BRICK_LABEL_COL_PX),
    ]
}

/// Left inset so the 20px check glyph sits in the horizontal center of the column.
fn popup_brick_glyph_center_margin() -> Thickness {
    let inset = (POPUP_BRICK_LABEL_COL_PX - POPUP_BRICK_GLYPH_PX) / 2.0;
    Thickness {
        left: inset,
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
    }
}

fn popup_brick_header_label(label: &str, column: i32) -> Element {
    if column == 0 {
        return text_block(label)
            .font_size(12.0)
            .opacity(0.72)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(0)
            .into();
    }
    text_block(label)
        .font_size(12.0)
        .opacity(0.72)
        .horizontal_alignment(HorizontalAlignment::Center)
        .vertical_alignment(VerticalAlignment::Center)
        .grid_column(column)
        .into()
}

/// Icon-only CheckBox: kill the style MinWidth=120 and Content padding
/// so only the 32×32 glyph host remains. Margin optically centers the
/// 20px check mark inside the wider Home/Tab column.
fn settings_icon_checkbox(
    checked: bool,
    enabled: bool,
    on_checked: impl IntoCallback<bool>,
) -> CheckBox {
    CheckBox::new(checked)
        .enabled(enabled)
        .on_checked(on_checked)
        .min_width(0.0)
        .max_width(POPUP_BRICK_CHECK_COL_PX)
        .width(POPUP_BRICK_CHECK_COL_PX)
        .padding(Thickness::uniform(0.0))
        .margin(popup_brick_glyph_center_margin())
        .horizontal_alignment(HorizontalAlignment::Left)
        .vertical_alignment(VerticalAlignment::Center)
}

/// Estimated body height for the checkbox visibility table, including its
/// divider and vertical card padding. This lets the shared card shell animate
/// content-driven provider bodies with the same height progress as notices.
pub(crate) fn settings_brick_body_height(row_count: usize) -> f64 {
    1.0 + row_count as f64 * POPUP_BRICK_ROW_HEIGHT + CARD_CONTENT_PADDING_Y * 2.0
}

/// Shared Card / Home / Tab header. Home/Tab labels sit centered in their
/// columns, matching the optically centered check glyphs below.
pub(crate) fn settings_brick_table_header(row_key: &str) -> Element {
    grid(vec![
        popup_brick_header_label("Card", 0),
        popup_brick_header_label("Home", 1),
        popup_brick_header_label("Tab", 2),
    ])
    .columns(popup_brick_columns())
    .rows([GridLength::Auto])
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .with_key(format!("popup-brick-header-{row_key}"))
    .into()
}

/// Trailing Home-tab master checkbox. Content is the label so WinUI
/// applies the template's optical padding instead of a sibling TextBlock.
pub(crate) fn settings_section_all_toggle(
    checked: bool,
    on_checked: impl IntoCallback<bool>,
) -> Element {
    CheckBox::new(checked)
        .content("Home")
        .on_checked(on_checked)
        .min_width(0.0)
        .vertical_alignment(VerticalAlignment::Center)
        .into()
}

/// Card name on the left; Home and Tab are icon-only native checkboxes.
pub(crate) fn settings_brick_row(
    label: impl Into<String>,
    all_tab: bool,
    provider_tab: bool,
    all_enabled: bool,
    on_all_tab_changed: impl IntoCallback<bool>,
    on_provider_tab_changed: impl IntoCallback<bool>,
    row_key: &str,
) -> Element {
    let label = label.into();
    grid(vec![
        text_block(label)
            .font_size(14.0)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(0)
            .into(),
        settings_icon_checkbox(all_tab, all_enabled, on_all_tab_changed)
            .tooltip("Show on Home tab")
            .grid_column(1)
            .into(),
        settings_icon_checkbox(provider_tab, true, on_provider_tab_changed)
            .tooltip("Show on provider tab")
            .grid_column(2)
            .into(),
    ])
    .columns(popup_brick_columns())
    .rows([GridLength::Auto])
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .with_key(format!("popup-brick-row-{row_key}"))
    .into()
}
