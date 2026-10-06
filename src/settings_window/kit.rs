//! GPUI building blocks for the Settings and onboarding windows.
//!
//! GPUI is immediate-mode, so every component is a function of the current
//! [`Kit`] (theme + animation state) and the values passed in. Ephemeral UI
//! state that belongs to no setting (an open menu, a slider being dragged)
//! lives in shared cells on the kit; changing it refreshes the window.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    Animation, AnimationExt, AnyElement, App, AppContext, Bounds, ClickEvent, CursorStyle,
    ElementId, Entity, FontWeight, Hsla, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Render, SharedString,
    StatefulInteractiveElement, Styled, Transformation, Window, anchored, canvas, deferred, div,
    img, point, px, radians, relative, svg,
};

use super::input::TextInput;
use super::theme::Theme;
use crate::popup_window::ui::{
    assets,
    fx::{self, Fx},
    theme::HslaExt,
};

pub(crate) type Handler<T> = Rc<dyn Fn(T, &mut Window, &mut App)>;

pub(crate) fn handler<T>(f: impl Fn(T, &mut Window, &mut App) + 'static) -> Handler<T> {
    Rc::new(f)
}

pub(crate) fn eid(value: impl Into<SharedString>) -> ElementId {
    ElementId::Name(value.into())
}

pub(crate) const CONTROL_HEIGHT: f32 = 32.0;
pub(crate) const CARD_RADIUS: f32 = 8.0;
pub(crate) const ROW_PADDING_X: f32 = 16.0;
pub(crate) const ROW_MIN_HEIGHT: f32 = 62.0;

/// Theme and animation state shared by every component in one window.
#[derive(Default)]
pub(crate) struct Kit {
    pub(crate) theme: Theme,
    pub(crate) fx: Fx,
    heights: HashMap<u64, Rc<Cell<f32>>>,
    pub(crate) menus: Menus,
    sliders: Rc<RefCell<Option<SharedString>>>,
}

impl Kit {
    pub(crate) fn begin_frame(&mut self, theme: Theme) {
        self.theme = theme;
        self.fx.begin_frame(crate::theme::animations_enabled());
    }

    pub(crate) fn animate(&self) -> bool {
        self.fx.enabled()
    }

    /// Request another frame while any tween is still in flight.
    pub(crate) fn end_frame(&self, window: &mut Window) {
        if self.fx.is_animating() {
            window.request_animation_frame();
        }
    }
}

/// The one open popover menu of a window.
#[derive(Clone, Default)]
pub(crate) struct Menus {
    open: Rc<RefCell<Option<SharedString>>>,
    closed: Rc<RefCell<Option<(SharedString, Instant)>>>,
}

impl Menus {
    pub(crate) fn is_open(&self, id: &str) -> bool {
        self.open
            .borrow()
            .as_ref()
            .is_some_and(|open| open.as_ref() == id)
    }

    pub(crate) fn any_open(&self) -> bool {
        self.open.borrow().is_some()
    }

    pub(crate) fn toggle(&self, id: SharedString, window: &mut Window) {
        let just_closed =
            self.closed.borrow().as_ref().is_some_and(|(closed, at)| {
                *closed == id && at.elapsed() < Duration::from_millis(250)
            });
        let mut open = self.open.borrow_mut();
        *open = if open.as_ref() == Some(&id) || just_closed {
            None
        } else {
            Some(id)
        };
        window.refresh();
    }

    /// Close without a repaint (the caller is about to notify anyway).
    pub(crate) fn close_silently(&self) {
        self.open.borrow_mut().take();
    }

    pub(crate) fn close(&self, window: &mut Window) {
        if let Some(id) = self.open.borrow_mut().take() {
            *self.closed.borrow_mut() = Some((id, Instant::now()));
            window.refresh();
        }
    }
}

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

/// A monochrome Iconify glyph tinted with `color`.
pub(crate) fn icon(name: &str, size: f32, color: Hsla) -> gpui::Svg {
    svg()
        .path(assets::icon_path(name))
        .size(px(size))
        .flex_none()
        .text_color(color)
}

/// A full-color SVG or bitmap asset (sidebar Fluent Color icons, app icon).
pub(crate) fn image(path: &'static str, size: f32) -> gpui::Img {
    img(gpui::ImageSource::Resource(gpui::Resource::Embedded(
        path.into(),
    )))
    .size(px(size))
    .flex_none()
}

pub(crate) fn text(value: impl Into<SharedString>, size: f32, color: Hsla) -> gpui::Div {
    div()
        .text_size(px(size))
        .text_color(color)
        .child(value.into())
}

pub(crate) fn page_title(k: &Kit, value: impl Into<SharedString>) -> AnyElement {
    div()
        .text_size(px(28.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(k.theme.text)
        .pb(px(4.0))
        .child(value.into())
        .into_any_element()
}

pub(crate) fn section_heading(k: &Kit, value: impl Into<SharedString>) -> AnyElement {
    div()
        .pt(px(18.0))
        .pb(px(6.0))
        .text_size(px(14.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(k.theme.text)
        .child(value.into())
        .into_any_element()
}

/// Heading with a caption and an optional trailing action (e.g. "Add").
pub(crate) fn section_header(
    k: &Kit,
    title: impl Into<SharedString>,
    caption: Option<SharedString>,
    action: Option<AnyElement>,
) -> AnyElement {
    let mut text_col = div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .flex_1()
        .min_w_0()
        .child(
            div()
                .text_size(px(14.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(k.theme.text)
                .child(title.into()),
        );
    if let Some(caption) = caption {
        text_col = text_col.child(text(caption, 12.0, k.theme.text_secondary));
    }
    div()
        .flex()
        .items_end()
        .gap(px(16.0))
        .pt(px(18.0))
        .pb(px(6.0))
        .child(text_col)
        .children(action)
        .into_any_element()
}

pub(crate) fn caption(k: &Kit, value: impl Into<SharedString>) -> AnyElement {
    text(value, 12.0, k.theme.text_secondary).into_any_element()
}

pub(crate) fn status_dot(color: Hsla) -> AnyElement {
    div()
        .size(px(8.0))
        .rounded_full()
        .bg(color)
        .flex_none()
        .into_any_element()
}

/// A small rounded chip (e.g. `Limits only`).
pub(crate) fn chip(k: &Kit, label: impl Into<SharedString>) -> AnyElement {
    div()
        .px(px(6.0))
        .py(px(1.0))
        .rounded(px(4.0))
        .border_1()
        .border_color(k.theme.card_stroke)
        .bg(k.theme.subtle_hover)
        .text_size(px(11.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(k.theme.text_secondary)
        .child(label.into())
        .into_any_element()
}

/// An instance badge plate as drawn in the popup and sidebar.
pub(crate) fn badge_plate(k: &Kit, badge: &crate::instances::Badge) -> AnyElement {
    let (bg, fg) = k.theme.badge(badge.color);
    div()
        .flex()
        .items_center()
        .justify_center()
        .min_w(px(20.0))
        .h(px(18.0))
        .px(px(4.0))
        .rounded(px(4.0))
        .bg(bg)
        .text_color(fg)
        .text_size(px(11.0))
        .font_weight(FontWeight::BOLD)
        .flex_none()
        .child(badge.text.clone())
        .into_any_element()
}

pub(crate) fn divider(k: &Kit) -> AnyElement {
    div()
        .h(px(1.0))
        .w_full()
        .flex_none()
        .bg(k.theme.divider)
        .into_any_element()
}

/// Fade + rise entrance for content that appears (pages, list items,
/// dialogs). Restarts whenever `id` changes.
pub(crate) fn appear(k: &Kit, id: impl Into<SharedString>, element: AnyElement) -> AnyElement {
    appear_in(k, id, element, false)
}

/// [`appear`] for content that must fill its parent (sidebars, panes).
pub(crate) fn appear_fill(k: &Kit, id: impl Into<SharedString>, element: AnyElement) -> AnyElement {
    appear_in(k, id, element, true)
}

fn appear_in(k: &Kit, id: impl Into<SharedString>, element: AnyElement, fill: bool) -> AnyElement {
    if !k.animate() {
        return element;
    }
    // The wrapper must size exactly like the element it replaces: full
    // width (and height when filling), and an offset that never moves its
    // siblings.
    div()
        .relative()
        .w_full()
        .when(fill, |el| el.h_full())
        .child(element)
        .with_animation(
            eid(id),
            Animation::new(Duration::from_millis(220)).with_easing(fx::ease_out_cubic),
            |el, delta| el.opacity(delta).top(px((1.0 - delta) * 6.0)),
        )
        .into_any_element()
}

// ---------------------------------------------------------------------------
// Cards and rows
// ---------------------------------------------------------------------------

/// A rounded group of rows separated by hairline dividers.
pub(crate) fn card(k: &Kit, rows: Vec<AnyElement>) -> AnyElement {
    let count = rows.len();
    let mut group = div()
        .flex()
        .flex_col()
        .w_full()
        .rounded(px(CARD_RADIUS))
        .border_1()
        .border_color(k.theme.card_stroke)
        .bg(k.theme.card)
        .overflow_hidden();
    for (index, row) in rows.into_iter().enumerate() {
        group = group.child(row);
        if index + 1 < count {
            group = group.child(div().px(px(ROW_PADDING_X)).child(divider(k)));
        }
    }
    group.into_any_element()
}

/// [`card`] whose rows are built with mutable access to the kit.
pub(crate) fn card_of(k: &mut Kit, rows: impl FnOnce(&mut Kit) -> Vec<AnyElement>) -> AnyElement {
    let rows = rows(k);
    card(k, rows)
}

/// Leading icon, title and description, trailing control.
pub(crate) struct Row {
    id: SharedString,
    icon: Option<AnyElement>,
    title: SharedString,
    description: Vec<AnyElement>,
    trailing: Vec<AnyElement>,
    on_click: Option<Handler<()>>,
    disabled: bool,
}

impl Row {
    pub(crate) fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            icon: None,
            title: title.into(),
            description: Vec::new(),
            trailing: Vec::new(),
            on_click: None,
            disabled: false,
        }
    }

    pub(crate) fn icon(mut self, icon: AnyElement) -> Self {
        self.icon = Some(icon);
        self
    }

    pub(crate) fn description(mut self, k: &Kit, text: impl Into<SharedString>) -> Self {
        self.description.push(caption(k, text));
        self
    }

    pub(crate) fn description_opt(self, k: &Kit, text: Option<impl Into<SharedString>>) -> Self {
        match text {
            Some(text) => self.description(k, text),
            None => self,
        }
    }

    pub(crate) fn detail(mut self, element: AnyElement) -> Self {
        self.description.push(element);
        self
    }

    pub(crate) fn trailing(mut self, element: AnyElement) -> Self {
        self.trailing.push(element);
        self
    }

    /// Whole-row click target (navigation rows, expanders).
    pub(crate) fn on_click(mut self, handler: Handler<()>) -> Self {
        self.on_click = Some(handler);
        self
    }

    pub(crate) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub(crate) fn render(self, k: &Kit) -> AnyElement {
        let theme = &k.theme;
        let mut text_col = div()
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(2.0))
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(if self.disabled {
                        theme.text_disabled
                    } else {
                        theme.text
                    })
                    .child(self.title),
            );
        for detail in self.description {
            text_col = text_col.child(detail);
        }
        let mut row = div()
            .id(eid(self.id))
            .flex()
            .items_center()
            .gap(px(14.0))
            .min_h(px(ROW_MIN_HEIGHT))
            .px(px(ROW_PADDING_X))
            .py(px(10.0))
            .w_full()
            .children(self.icon)
            .child(text_col)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .flex_none()
                    .children(self.trailing),
            );
        if let Some(on_click) = self.on_click.filter(|_| !self.disabled) {
            let hover = theme.card_hover;
            row = row
                .cursor_pointer()
                .hover(move |style| style.bg(hover))
                .on_click(move |_, window, cx| on_click((), window, cx));
        }
        row.into_any_element()
    }
}

/// A single-row card.
pub(crate) fn row_card(k: &Kit, row: Row) -> AnyElement {
    card(k, vec![row.render(k)])
}

/// A leading 20px glyph slot for rows.
pub(crate) fn row_icon(k: &Kit, name: &str) -> AnyElement {
    div()
        .size(px(20.0))
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .child(icon(name, 18.0, k.theme.glyph()))
        .into_any_element()
}

/// Height-animated reveal. The content is measured every frame it is shown,
/// so the glide always lands on its natural height.
pub(crate) fn collapsible(
    k: &mut Kit,
    id: u64,
    open: bool,
    content: impl FnOnce(&mut Kit) -> AnyElement,
) -> Option<AnyElement> {
    let progress = k.fx.toggle(fx::key(("collapse", id)), open, fx::NORMAL);
    if !open && progress <= 0.001 {
        return None;
    }
    let cell = Rc::clone(k.heights.entry(id).or_default());
    let measure = Rc::clone(&cell);
    let inner = div()
        .relative()
        .flex_none()
        .w_full()
        .child(content(k))
        .child(
            canvas(
                move |bounds, _, _| measure.set(f32::from(bounds.size.height)),
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        );
    let settled = (progress - 1.0).abs() <= 0.001;
    let mut outer = div().w_full().overflow_hidden().child(inner);
    if !settled {
        outer = outer
            .h(px(cell.get() * progress))
            .opacity(progress.powf(0.6));
    }
    Some(outer.into_any_element())
}

/// Card whose header toggles an animated body.
pub(crate) fn expander(
    k: &mut Kit,
    id: impl Into<SharedString>,
    header: Row,
    expanded: bool,
    on_toggle: Handler<bool>,
    body: impl FnOnce(&mut Kit) -> AnyElement,
) -> AnyElement {
    let id: SharedString = id.into();
    let key = fx::key(("expander", id.as_ref()));
    let turn = k.fx.toggle(key, expanded, fx::NORMAL);
    let chevron = icon("caret-down-bold", 12.0, k.theme.text_secondary)
        .with_transformation(Transformation::rotate(radians(std::f32::consts::PI * turn)));
    let toggle = Rc::clone(&on_toggle);
    let header = header
        .trailing(
            div()
                .size(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .child(chevron)
                .into_any_element(),
        )
        .on_click(handler(move |(), window, cx| toggle(!expanded, window, cx)))
        .render(k);
    let theme_divider = k.theme.divider;
    let body = collapsible(k, key, expanded, move |k| {
        div()
            .flex()
            .flex_col()
            .child(div().h(px(1.0)).bg(theme_divider))
            .child(
                div()
                    .px(px(ROW_PADDING_X))
                    .py(px(14.0))
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(body(k)),
            )
            .into_any_element()
    });
    div()
        .flex()
        .flex_col()
        .w_full()
        .rounded(px(CARD_RADIUS))
        .border_1()
        .border_color(k.theme.card_stroke)
        .bg(k.theme.card)
        .overflow_hidden()
        .child(header)
        .children(body)
        .into_any_element()
}

// ---------------------------------------------------------------------------
// Buttons
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ButtonKind {
    Standard,
    Accent,
    Subtle,
    Danger,
    Link,
}

pub(crate) struct Button {
    id: SharedString,
    label: Option<SharedString>,
    icon: Option<&'static str>,
    kind: ButtonKind,
    disabled: bool,
    full_width: bool,
    tooltip: Option<SharedString>,
    on_click: Option<Handler<()>>,
}

impl Button {
    pub(crate) fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: Some(label.into()),
            icon: None,
            kind: ButtonKind::Standard,
            disabled: false,
            full_width: false,
            tooltip: None,
            on_click: None,
        }
    }

    pub(crate) fn icon_only(id: impl Into<SharedString>, icon: &'static str) -> Self {
        Self {
            label: None,
            icon: Some(icon),
            kind: ButtonKind::Subtle,
            ..Self::new(id, "")
        }
    }

    pub(crate) fn accent(mut self) -> Self {
        self.kind = ButtonKind::Accent;
        self
    }

    pub(crate) fn danger(mut self) -> Self {
        self.kind = ButtonKind::Danger;
        self
    }

    pub(crate) fn link(mut self) -> Self {
        self.kind = ButtonKind::Link;
        self
    }

    pub(crate) fn kind(mut self, kind: ButtonKind) -> Self {
        self.kind = kind;
        self
    }

    pub(crate) fn with_icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }

    pub(crate) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub(crate) fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    pub(crate) fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub(crate) fn on_click(mut self, on_click: Handler<()>) -> Self {
        self.on_click = Some(on_click);
        self
    }

    pub(crate) fn render(self, k: &Kit) -> AnyElement {
        let theme = &k.theme;
        let (bg, hover, pressed, fg, border) = match self.kind {
            ButtonKind::Accent => (
                theme.accent,
                theme.accent_hover,
                theme.accent_pressed,
                theme.on_accent,
                theme.accent,
            ),
            ButtonKind::Standard => (
                theme.control,
                theme.control_hover,
                theme.control_pressed,
                theme.text,
                theme.control_stroke,
            ),
            ButtonKind::Danger => (
                theme.control,
                theme.critical_bg,
                theme.critical_bg.alpha(0.7),
                theme.critical,
                theme.control_stroke,
            ),
            ButtonKind::Subtle => (
                gpui::transparent_black(),
                theme.subtle_hover,
                theme.subtle_pressed,
                theme.text,
                gpui::transparent_black(),
            ),
            ButtonKind::Link => (
                gpui::transparent_black(),
                theme.subtle_hover,
                theme.subtle_pressed,
                theme.accent_text,
                gpui::transparent_black(),
            ),
        };
        let fg = if self.disabled {
            if self.kind == ButtonKind::Accent {
                theme.on_accent.alpha(0.7)
            } else {
                theme.text_disabled
            }
        } else {
            fg
        };
        let bg = if self.disabled && self.kind == ButtonKind::Accent {
            theme.control_disabled
        } else {
            bg
        };
        let icon_only = self.label.is_none();
        let mut button = div()
            .id(eid(self.id))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .h(px(CONTROL_HEIGHT))
            .rounded(px(5.0))
            .border_1()
            .border_color(border)
            .bg(bg)
            .text_color(fg)
            .text_size(px(13.0))
            .when_some(self.icon, |el, name| el.child(icon(name, 14.0, fg)))
            .when_some(self.label, |el, label| el.child(label));
        button = if icon_only {
            button.w(px(CONTROL_HEIGHT))
        } else {
            button.px(px(if self.kind == ButtonKind::Link {
                6.0
            } else {
                14.0
            }))
        };
        if self.full_width {
            button = button.w_full().flex_1();
        }
        if self.disabled {
            button = button.cursor(CursorStyle::Arrow);
        } else {
            button = button
                .cursor_pointer()
                .hover(move |style| style.bg(hover))
                .active(move |style| style.bg(pressed));
            if let Some(on_click) = self.on_click {
                button = button.on_click(move |_: &ClickEvent, window, cx| {
                    cx.stop_propagation();
                    on_click((), window, cx)
                });
            }
        }
        if let Some(tooltip) = self.tooltip {
            button = with_tooltip(k, button, tooltip);
        }
        button.into_any_element()
    }
}

use gpui::prelude::FluentBuilder;

// ---------------------------------------------------------------------------
// Toggle, checkbox, segmented
// ---------------------------------------------------------------------------

/// Fluent toggle switch with a gliding, stretching knob.
pub(crate) fn toggle(
    k: &mut Kit,
    id: impl Into<SharedString>,
    on: bool,
    disabled: bool,
    on_toggle: Handler<bool>,
) -> AnyElement {
    let id: SharedString = id.into();
    let t = k.fx.toggle(fx::key(("toggle", id.as_ref())), on, fx::FAST);
    let theme = &k.theme;
    let (track, stroke, knob) = if disabled {
        (
            if on {
                theme.control_disabled
            } else {
                gpui::transparent_black()
            },
            theme.control_disabled,
            theme.text_disabled,
        )
    } else {
        (
            theme.accent.alpha(t),
            theme.control_strong.mix(theme.accent, t),
            theme.text_secondary.mix(theme.on_accent, t),
        )
    };
    let knob_size = 12.0 + 2.0 * t;
    let travel = 40.0 - 8.0 - knob_size;
    let mut el = div()
        .id(eid(format!("toggle-{id}")))
        .relative()
        .w(px(40.0))
        .h(px(20.0))
        .flex_none()
        .rounded_full()
        .border_1()
        .border_color(stroke)
        .bg(track)
        .child(
            div()
                .absolute()
                .top(px((18.0 - knob_size) / 2.0))
                .left(px(3.0 + travel * t))
                .size(px(knob_size))
                .rounded_full()
                .bg(knob),
        );
    if !disabled {
        let hover = theme.subtle_hover;
        el = el
            .cursor_pointer()
            .hover(move |style| if on { style } else { style.bg(hover) })
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                on_toggle(!on, window, cx)
            });
    }
    el.into_any_element()
}

/// A setting row with a trailing toggle; the whole row toggles.
pub(crate) fn toggle_row(
    k: &mut Kit,
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    description: Option<SharedString>,
    on: bool,
    on_toggle: Handler<bool>,
) -> AnyElement {
    toggle_row_with(k, id, title, description, on, None, on_toggle)
}

/// [`toggle_row`] that may be disabled with an explanation.
pub(crate) fn toggle_row_with(
    k: &mut Kit,
    id: impl Into<SharedString>,
    title: impl Into<SharedString>,
    description: Option<SharedString>,
    on: bool,
    disabled_reason: Option<SharedString>,
    on_toggle: Handler<bool>,
) -> AnyElement {
    let id: SharedString = id.into();
    let disabled = disabled_reason.is_some();
    let state_label = text(
        if on && !disabled { "On" } else { "Off" },
        13.0,
        k.theme.text_secondary,
    )
    .into_any_element();
    let switch = toggle(
        k,
        id.clone(),
        on && !disabled,
        disabled,
        Rc::clone(&on_toggle),
    );
    let mut row = Row::new(id, title)
        .description_opt(k, description)
        .trailing(state_label)
        .trailing(switch)
        .disabled(disabled);
    if let Some(reason) = disabled_reason {
        row = row.detail(text(reason, 12.0, k.theme.caution).into_any_element());
    } else {
        row = row.on_click(handler(move |(), window, cx| on_toggle(!on, window, cx)));
    }
    row.render(k)
}

pub(crate) fn checkbox(
    k: &mut Kit,
    id: impl Into<SharedString>,
    checked: bool,
    disabled: bool,
    label: Option<SharedString>,
    on_change: Handler<bool>,
) -> AnyElement {
    let id: SharedString = id.into();
    let t =
        k.fx.toggle(fx::key(("check", id.as_ref())), checked, fx::FASTER);
    let theme = &k.theme;
    let fill = if disabled {
        theme.control_disabled.alpha(t)
    } else {
        theme.accent.alpha(t)
    };
    let stroke = if disabled {
        theme.control_disabled
    } else {
        theme.control_strong.mix(theme.accent, t)
    };
    let box_el = div()
        .size(px(18.0))
        .flex_none()
        .rounded(px(4.0))
        .border_1()
        .border_color(stroke)
        .bg(fill)
        .flex()
        .items_center()
        .justify_center()
        .when(t > 0.01, |el| {
            el.child(icon("check-bold", 12.0 * (0.6 + 0.4 * t), theme.on_accent).opacity(t))
        });
    let mut el = div()
        .id(eid(format!("check-{id}")))
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(box_el)
        .when_some(label, |el, label| {
            el.child(text(
                label,
                14.0,
                if disabled {
                    theme.text_disabled
                } else {
                    theme.text
                },
            ))
        });
    if !disabled {
        el = el.cursor_pointer().on_click(move |_, window, cx| {
            cx.stop_propagation();
            on_change(!checked, window, cx)
        });
    }
    el.into_any_element()
}

/// Pill segmented control with a sliding accent thumb.
pub(crate) fn segmented(
    k: &mut Kit,
    id: impl Into<SharedString>,
    labels: &[&str],
    selected: usize,
    disabled: bool,
    on_select: Handler<usize>,
) -> AnyElement {
    let id: SharedString = id.into();
    let count = labels.len().max(1);
    let selected = selected.min(count - 1);
    let thumb = k.fx.value(
        fx::key(("segment", id.as_ref())),
        selected as f32 / count as f32,
        fx::FAST,
    );
    let theme = &k.theme;
    let mut track = div()
        .id(eid(format!("segmented-{id}")))
        .relative()
        .flex()
        .h(px(CONTROL_HEIGHT))
        .p(px(3.0))
        .rounded(px(7.0))
        .border_1()
        .border_color(theme.control_stroke)
        .bg(theme.subtle_hover)
        .child(
            div()
                .absolute()
                .top(px(3.0))
                .bottom(px(3.0))
                .left(px(3.0))
                .right(px(3.0))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(relative(thumb))
                        .w(relative(1.0 / count as f32))
                        .rounded(px(5.0))
                        .bg(if disabled {
                            theme.control_disabled
                        } else {
                            theme.accent
                        }),
                ),
        );
    for (index, label) in labels.iter().enumerate() {
        let on = index == selected;
        let on_select = Rc::clone(&on_select);
        let hover = theme.subtle_hover;
        let mut cell = div()
            .id(eid(format!("segment-{id}-{index}")))
            .relative()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .px(px(12.0))
            .rounded(px(5.0))
            .text_size(px(13.0))
            .text_color(if on { theme.on_accent } else { theme.text })
            .child(SharedString::from(label.to_string()));
        if !disabled && !on {
            cell = cell
                .cursor_pointer()
                .hover(move |style| style.bg(hover))
                .on_click(move |_, window, cx| on_select(index, window, cx));
        }
        track = track.child(cell);
    }
    track.into_any_element()
}

// ---------------------------------------------------------------------------
// Dropdowns and menus
// ---------------------------------------------------------------------------

pub(crate) struct MenuItem {
    pub(crate) label: SharedString,
    pub(crate) icon: Option<&'static str>,
    pub(crate) disabled: bool,
    pub(crate) danger: bool,
}

impl MenuItem {
    pub(crate) fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            disabled: false,
            danger: false,
        }
    }

    pub(crate) fn icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }

    pub(crate) fn danger(mut self) -> Self {
        self.danger = true;
        self
    }
}

fn menu_panel(
    k: &Kit,
    id: &SharedString,
    items: Vec<MenuItem>,
    selected: Option<usize>,
    min_width: f32,
    on_select: Handler<usize>,
) -> AnyElement {
    let theme = &k.theme;
    let menus = k.menus.clone();
    let mut list = div()
        .id(eid(format!("menu-list-{id}")))
        .flex()
        .flex_col()
        .p(px(4.0))
        .gap(px(2.0))
        .max_h(px(340.0))
        .overflow_y_scroll();
    for (index, item) in items.into_iter().enumerate() {
        let on_select = Rc::clone(&on_select);
        let menus = menus.clone();
        let is_selected = selected == Some(index);
        let color = if item.disabled {
            theme.text_disabled
        } else if item.danger {
            theme.critical
        } else {
            theme.text
        };
        let hover = theme.subtle_hover;
        let accent = theme.accent;
        let mut entry = div()
            .id(eid(format!("menu-{id}-{index}")))
            .relative()
            .flex()
            .items_center()
            .gap(px(10.0))
            .h(px(32.0))
            .px(px(12.0))
            .rounded(px(4.0))
            .text_size(px(14.0))
            .text_color(color)
            .when(is_selected, |el| el.bg(hover))
            .when(is_selected, |el| {
                el.child(
                    div()
                        .absolute()
                        .left_0()
                        .top(px(9.0))
                        .h(px(14.0))
                        .w(px(3.0))
                        .rounded_full()
                        .bg(accent),
                )
            })
            .when_some(item.icon, |el, name| el.child(icon(name, 14.0, color)))
            .child(div().whitespace_nowrap().child(item.label));
        if !item.disabled {
            entry = entry
                .cursor_pointer()
                .hover(move |style| style.bg(hover))
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    menus.close(window);
                    on_select(index, window, cx);
                });
        }
        list = list.child(entry);
    }
    let menus = k.menus.clone();
    let panel = div()
        .id(eid(format!("menu-panel-{id}")))
        .occlude()
        .mt(px(4.0))
        .min_w(px(min_width))
        .rounded(px(8.0))
        .border_1()
        .border_color(theme.popover_stroke)
        .bg(theme.popover)
        .shadow(vec![gpui::BoxShadow {
            color: theme.shadow,
            offset: point(px(0.0), px(8.0)),
            blur_radius: px(16.0),
            spread_radius: px(0.0),
        }])
        .on_mouse_down_out(move |_, window, _| menus.close(window))
        .child(list);
    let panel: AnyElement = if k.animate() {
        panel
            .with_animation(
                eid(format!("menu-anim-{id}")),
                Animation::new(Duration::from_millis(167)).with_easing(fx::ease_out_cubic),
                |el, delta| el.opacity(delta).mt(px(4.0 - (1.0 - delta) * 8.0)),
            )
            .into_any_element()
    } else {
        panel.into_any_element()
    };
    div()
        .absolute()
        .top_full()
        .left_0()
        .child(
            deferred(anchored().snap_to_window_with_margin(px(8.0)).child(panel)).with_priority(2),
        )
        .into_any_element()
}

/// ComboBox: a button showing the current choice that opens a list.
pub(crate) fn dropdown(
    k: &Kit,
    id: impl Into<SharedString>,
    options: Vec<SharedString>,
    selected: Option<usize>,
    disabled: bool,
    width: f32,
    on_select: Handler<usize>,
) -> AnyElement {
    dropdown_with_placeholder(k, id, options, selected, disabled, width, None, on_select)
}

pub(crate) fn dropdown_with_placeholder(
    k: &Kit,
    id: impl Into<SharedString>,
    options: Vec<SharedString>,
    selected: Option<usize>,
    disabled: bool,
    width: f32,
    placeholder: Option<SharedString>,
    on_select: Handler<usize>,
) -> AnyElement {
    let id: SharedString = id.into();
    let theme = &k.theme;
    let open = k.menus.is_open(&id);
    let label = selected
        .and_then(|index| options.get(index).cloned())
        .or(placeholder.clone())
        .unwrap_or_default();
    let label_color = if disabled {
        theme.text_disabled
    } else if selected.is_none() {
        theme.text_secondary
    } else {
        theme.text
    };
    let menus = k.menus.clone();
    let toggle_id = id.clone();
    let hover = theme.control_hover;
    let mut button = div()
        .id(eid(format!("dropdown-{id}")))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.0))
        .h(px(CONTROL_HEIGHT))
        .px(px(11.0))
        .rounded(px(5.0))
        .border_1()
        .border_color(if open {
            theme.accent
        } else {
            theme.control_stroke
        })
        .bg(if disabled {
            theme.control_disabled.alpha(0.3)
        } else {
            theme.control
        })
        .text_size(px(13.0))
        .text_color(label_color)
        .child(div().flex_1().min_w_0().truncate().child(label))
        .child(icon("caret-down-bold", 10.0, theme.text_secondary));
    if width > 0.0 {
        button = button.w(px(width));
    } else {
        button = button.w_full();
    }
    if !disabled {
        button = button
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .on_click(move |_, window, _| menus.toggle(toggle_id.clone(), window));
    }
    let items = options.into_iter().map(MenuItem::new).collect::<Vec<_>>();
    let mut wrapper = div().relative().flex_none().child(button);
    if width <= 0.0 {
        wrapper = wrapper.w_full().flex_1();
    }
    if open && !disabled {
        wrapper = wrapper.child(menu_panel(
            k,
            &id,
            items,
            selected,
            width.max(160.0),
            on_select,
        ));
    }
    wrapper.into_any_element()
}

pub(crate) fn options(labels: &[&str]) -> Vec<SharedString> {
    labels
        .iter()
        .map(|label| SharedString::from(label.to_string()))
        .collect()
}

/// A setting row whose trailing control is a dropdown.
#[allow(clippy::too_many_arguments)]
pub(crate) fn dropdown_row(
    k: &Kit,
    id: &str,
    title: impl Into<SharedString>,
    description: Option<&str>,
    labels: Vec<SharedString>,
    selected: i32,
    disabled: bool,
    on_select: Handler<usize>,
) -> AnyElement {
    let control = dropdown(
        k,
        id.to_owned(),
        labels,
        usize::try_from(selected).ok(),
        disabled,
        200.0,
        on_select,
    );
    Row::new(format!("row-{id}"), title)
        .description_opt(k, description.map(|text| text.to_owned()))
        .trailing(control)
        .disabled(disabled)
        .render(k)
}

/// A labeled dropdown stacked under a caption (dialog and editor forms).
pub(crate) fn field(k: &Kit, label: impl Into<SharedString>, control: AnyElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .flex_1()
        .min_w_0()
        .child(text(label, 12.0, k.theme.text_secondary))
        .child(control)
        .into_any_element()
}

/// "⋯" button opening an action menu.
pub(crate) fn more_menu(
    k: &Kit,
    id: impl Into<SharedString>,
    items: Vec<MenuItem>,
    on_select: Handler<usize>,
) -> AnyElement {
    let id: SharedString = id.into();
    let menus = k.menus.clone();
    let toggle_id = id.clone();
    let open = k.menus.is_open(&id);
    let button = Button::icon_only(format!("more-{id}"), "dots-three-bold")
        .tooltip("More options")
        .on_click(handler(move |(), window, _| {
            menus.toggle(toggle_id.clone(), window)
        }))
        .render(k);
    let mut wrapper = div().relative().flex_none().child(button);
    if open {
        wrapper = wrapper.child(menu_panel(k, &id, items, None, 170.0, on_select));
    }
    wrapper.into_any_element()
}

// ---------------------------------------------------------------------------
// Slider
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub(crate) struct SliderRange {
    pub(crate) min: f32,
    pub(crate) max: f32,
    pub(crate) step: f32,
}

pub(crate) fn slider(
    k: &mut Kit,
    id: impl Into<SharedString>,
    value: f32,
    range: SliderRange,
    width: f32,
    on_change: Handler<f32>,
) -> AnyElement {
    let id: SharedString = id.into();
    let span = (range.max - range.min).max(f32::EPSILON);
    let fraction = k.fx.value(
        fx::key(("slider", id.as_ref())),
        ((value - range.min) / span).clamp(0.0, 1.0),
        fx::FASTER,
    );
    let theme = &k.theme;
    let bounds = Rc::new(Cell::new(None::<Bounds<Pixels>>));
    let dragging = Rc::clone(&k.sliders);
    let is_dragging = dragging.borrow().as_ref().is_some_and(|open| *open == id);
    let quantize = move |x: Pixels, bounds: Bounds<Pixels>| {
        let width = f32::from(bounds.size.width).max(1.0);
        let t = (f32::from(x - bounds.left()) / width).clamp(0.0, 1.0);
        let raw = range.min + t * span;
        let stepped = ((raw - range.min) / range.step).round() * range.step + range.min;
        stepped.clamp(range.min, range.max)
    };
    let track_bounds = Rc::clone(&bounds);
    let paint_bounds = Rc::clone(&bounds);
    let down_id = id.clone();
    let down_dragging = Rc::clone(&dragging);
    let down_change = Rc::clone(&on_change);
    let move_id = id.clone();
    let thumb = if is_dragging { 12.0 } else { 10.0 };
    div()
        .id(eid(format!("slider-{id}")))
        .relative()
        .w(px(width))
        .h(px(CONTROL_HEIGHT))
        .flex_none()
        .cursor_pointer()
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(px(CONTROL_HEIGHT / 2.0 - 2.0))
                .h(px(4.0))
                .rounded_full()
                .bg(theme.control_strong.alpha(0.6))
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(relative(fraction))
                        .rounded_full()
                        .bg(theme.accent),
                ),
        )
        .child(
            div()
                .absolute()
                .top(px(CONTROL_HEIGHT / 2.0 - 10.0))
                .left(relative(fraction))
                .ml(px(-10.0))
                .size(px(20.0))
                .rounded_full()
                .bg(theme.popover)
                .border_1()
                .border_color(theme.control_stroke)
                .flex()
                .items_center()
                .justify_center()
                .child(div().size(px(thumb)).rounded_full().bg(theme.accent)),
        )
        .child(
            canvas(
                move |b, _, _| track_bounds.set(Some(b)),
                move |_, _, window, _| {
                    let Some(_) = paint_bounds.get() else {
                        return;
                    };
                    let move_bounds = Rc::clone(&paint_bounds);
                    let move_dragging = Rc::clone(&dragging);
                    let move_change = Rc::clone(&on_change);
                    let move_id = move_id.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, _, window, cx| {
                        if move_dragging.borrow().as_ref() != Some(&move_id) {
                            return;
                        }
                        if event.pressed_button != Some(MouseButton::Left) {
                            *move_dragging.borrow_mut() = None;
                            window.refresh();
                            return;
                        }
                        if let Some(bounds) = move_bounds.get() {
                            move_change(quantize(event.position.x, bounds), window, cx);
                        }
                    });
                    let up_dragging = Rc::clone(&dragging);
                    window.on_mouse_event(move |_: &MouseUpEvent, _, window, _| {
                        if up_dragging.borrow_mut().take().is_some() {
                            window.refresh();
                        }
                    });
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .on_mouse_down(
            MouseButton::Left,
            move |event: &MouseDownEvent, window, cx| {
                *down_dragging.borrow_mut() = Some(down_id.clone());
                if let Some(bounds) = bounds.get() {
                    down_change(quantize(event.position.x, bounds), window, cx);
                }
                window.refresh();
            },
        )
        .into_any_element()
}

// ---------------------------------------------------------------------------
// Text fields
// ---------------------------------------------------------------------------

/// The box around a [`TextInput`]: fill, stroke and the Fluent accent
/// underline while focused.
pub(crate) fn text_field(
    k: &Kit,
    input: &Entity<TextInput>,
    width: Option<f32>,
    window: &Window,
    cx: &mut App,
) -> AnyElement {
    let theme = &k.theme;
    let colors = super::input::InputColors {
        text: theme.text,
        placeholder: theme.text_tertiary,
        caret: theme.text,
        selection: theme.selection,
    };
    let (focused, disabled) = input.update(cx, |input, _| {
        input.colors = colors;
        (input.is_focused(window), input.disabled)
    });
    let mut field = div()
        .relative()
        .flex()
        .items_center()
        .h(px(CONTROL_HEIGHT))
        .px(px(11.0))
        .rounded(px(5.0))
        .border_1()
        .border_color(theme.control_stroke)
        .bg(if focused {
            theme.input_focus_bg
        } else {
            theme.input_bg
        })
        .text_size(px(14.0))
        .line_height(px(20.0))
        .overflow_hidden()
        .when(disabled, |el| el.opacity(0.5))
        .child(input.clone())
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .h(px(if focused { 2.0 } else { 1.0 }))
                .bg(if focused {
                    theme.accent
                } else {
                    theme.control_strong.alpha(0.5)
                }),
        );
    field = match width {
        Some(width) => field.w(px(width)).flex_none(),
        None => field.w_full(),
    };
    field.into_any_element()
}

// ---------------------------------------------------------------------------
// Overlays
// ---------------------------------------------------------------------------

/// Modal dialog: scrim, centered card, body and a button footer.
pub(crate) fn dialog(
    k: &Kit,
    id: impl Into<SharedString>,
    width: f32,
    body: Vec<AnyElement>,
    buttons: Vec<AnyElement>,
    on_dismiss: Option<Handler<()>>,
) -> AnyElement {
    let id: SharedString = id.into();
    let theme = &k.theme;
    let card = div()
        .id(eid(format!("dialog-card-{id}")))
        .relative()
        .occlude()
        .flex()
        .flex_col()
        .w(px(width))
        .max_h(relative(0.92))
        .rounded(px(10.0))
        .border_1()
        .border_color(theme.popover_stroke)
        .bg(theme.popover)
        .shadow(vec![gpui::BoxShadow {
            color: theme.shadow,
            offset: point(px(0.0), px(16.0)),
            blur_radius: px(32.0),
            spread_radius: px(0.0),
        }])
        .overflow_hidden()
        .on_click(|_, _, cx| cx.stop_propagation())
        .child(
            div()
                .id(eid(format!("dialog-body-{id}")))
                .flex()
                .flex_col()
                .gap(px(14.0))
                .p(px(24.0))
                .flex_shrink()
                .min_h_0()
                .overflow_y_scroll()
                .children(body),
        )
        .child(
            div()
                .flex()
                .gap(px(8.0))
                .p(px(24.0))
                .flex_none()
                .bg(theme.subtle_hover)
                .border_t_1()
                .border_color(theme.divider)
                .children(buttons),
        );
    let card: AnyElement = if k.animate() {
        card.with_animation(
            eid(format!("dialog-anim-{id}")),
            Animation::new(Duration::from_millis(220)).with_easing(fx::ease_out_cubic),
            |el, delta| el.opacity(delta).top(px((1.0 - delta) * 16.0)),
        )
        .into_any_element()
    } else {
        card.into_any_element()
    };
    let scrim = theme.scrim;
    let mut overlay = div()
        .id(eid(format!("dialog-{id}")))
        .occlude()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p(px(16.0))
        .bg(scrim)
        .child(card);
    if let Some(on_dismiss) = on_dismiss {
        overlay = overlay.on_click(move |_, window, cx| on_dismiss((), window, cx));
    }
    let overlay: AnyElement = if k.animate() {
        overlay
            .with_animation(
                eid(format!("dialog-scrim-{id}")),
                Animation::new(Duration::from_millis(167)),
                |el, delta| el.opacity(delta),
            )
            .into_any_element()
    } else {
        overlay.into_any_element()
    };
    overlay
}

pub(crate) fn dialog_title(k: &Kit, title: impl Into<SharedString>) -> AnyElement {
    div()
        .text_size(px(20.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(k.theme.text)
        .child(title.into())
        .into_any_element()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Severity {
    Success,
    Critical,
    Info,
}

/// InfoBar: tinted strip with a status glyph.
pub(crate) fn info_bar(
    k: &Kit,
    severity: Severity,
    message: impl Into<SharedString>,
) -> AnyElement {
    let theme = &k.theme;
    let (bg, fg, glyph) = match severity {
        Severity::Success => (theme.success_bg, theme.success, "check-circle-fill"),
        Severity::Critical => (theme.critical_bg, theme.critical, "x-circle-fill"),
        Severity::Info => (theme.accent_soft, theme.accent_text, "info-fill"),
    };
    div()
        .flex()
        .items_center()
        .gap(px(10.0))
        .px(px(14.0))
        .py(px(10.0))
        .rounded(px(CARD_RADIUS))
        .border_1()
        .border_color(theme.card_stroke)
        .bg(bg)
        .child(icon(glyph, 16.0, fg))
        .child(text(message, 13.0, theme.text).flex_1())
        .into_any_element()
}

struct TooltipView {
    text: SharedString,
    theme: Theme,
}

impl Render for TooltipView {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .font_family(self.theme.font.clone())
            .max_w(px(320.0))
            .px(px(8.0))
            .py(px(5.0))
            .rounded(px(5.0))
            .border_1()
            .border_color(self.theme.popover_stroke)
            .bg(self.theme.popover)
            .text_size(px(12.0))
            .text_color(self.theme.text)
            .child(self.text.clone())
    }
}

pub(crate) fn with_tooltip(
    k: &Kit,
    element: gpui::Stateful<gpui::Div>,
    tooltip: impl Into<SharedString>,
) -> gpui::Stateful<gpui::Div> {
    let text = tooltip.into();
    let theme = k.theme.clone();
    element.tooltip(move |_, cx| {
        cx.new(|_| TooltipView {
            text: text.clone(),
            theme: theme.clone(),
        })
        .into()
    })
}

/// Wrap any element so it shows a tooltip on hover.
pub(crate) fn tooltip_host(
    k: &Kit,
    id: impl Into<SharedString>,
    tooltip: impl Into<SharedString>,
    element: AnyElement,
) -> AnyElement {
    with_tooltip(k, div().id(eid(id)).child(element), tooltip).into_any_element()
}

/// Thin overlay scrollbar for a [`gpui::ScrollHandle`] container.
pub(crate) fn scrollbar(k: &Kit, handle: &gpui::ScrollHandle) -> Option<AnyElement> {
    let bounds = handle.bounds();
    let max = f32::from(handle.max_offset().height);
    let view = f32::from(bounds.size.height);
    if max <= 0.5 || view <= 0.0 {
        return None;
    }
    let offset = (-f32::from(handle.offset().y)).clamp(0.0, max);
    let content = view + max;
    let thumb = (view / content * view).max(32.0);
    let top = offset / max * (view - thumb);
    Some(
        div()
            .absolute()
            .top(px(top + 2.0))
            .right(px(3.0))
            .w(px(3.0))
            .h(px(thumb - 4.0))
            .rounded_full()
            .bg(k.theme.control_strong.alpha(0.55))
            .into_any_element(),
    )
}
