//! GPUI building blocks for the Settings and onboarding windows.
//!
//! GPUI is immediate-mode, so every component is a function of the current
//! [`Kit`] (theme + animation state) and the values passed in. Ephemeral UI
//! state that belongs to no setting (an open menu, a slider being dragged)
//! lives in shared cells on the kit; changing it refreshes the window.

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
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
    controls::{Segment, SegmentStyle, segmented_track},
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
pub(crate) const CARD_RADIUS: f32 = 10.0;
pub(crate) const ROW_PADDING_X: f32 = 18.0;
pub(crate) const ROW_MIN_HEIGHT: f32 = 64.0;
pub(crate) const CONTROL_RADIUS: f32 = 6.0;

/// Theme and animation state shared by every component in one window.
#[derive(Default)]
pub(crate) struct Kit {
    pub(crate) theme: Theme,
    pub(crate) fx: Fx,
    heights: HashMap<u64, Rc<Cell<f32>>>,
    pub(crate) menus: Menus,
    sliders: Rc<RefCell<Option<SharedString>>>,
    /// Scroll position of each open time picker's columns, and whether
    /// they were already brought to the current value on open.
    pickers: RefCell<HashMap<SharedString, (Vec<gpui::ScrollHandle>, bool)>>,
    /// Hovered segments of segmented controls.
    hovered: Rc<RefCell<std::collections::HashSet<u64>>>,
    hovers: Hovers,
    /// Bounds of the cards being painted, innermost last, so rows can round
    /// their hover fill to the card's corners.
    cards: Rc<RefCell<Vec<Bounds<Pixels>>>>,
}

impl Kit {
    pub(crate) fn begin_frame(&mut self, theme: Theme) {
        self.theme = theme;
        let enabled = crate::theme::animations_enabled();
        self.fx.begin_frame(enabled);
        self.hovers.begin_frame(enabled);
        self.cards.borrow_mut().clear();
    }

    pub(crate) fn animate(&self) -> bool {
        self.fx.enabled()
    }

    /// Request another frame while any tween is still in flight.
    pub(crate) fn end_frame(&self, window: &mut Window) {
        let hovering = self.hovers.end_frame();
        if self.fx.is_animating() || hovering {
            window.request_animation_frame();
        }
    }
}

// ---------------------------------------------------------------------------
// Hover
// ---------------------------------------------------------------------------

pub(crate) const HOVER_DURATION: Duration = Duration::from_millis(150);

#[derive(Clone, Copy)]
struct Fade {
    from: f32,
    to: f32,
    start: Instant,
}

impl Fade {
    fn at(&self, now: Instant) -> f32 {
        let t = now.duration_since(self.start).as_secs_f32() / HOVER_DURATION.as_secs_f32();
        self.from + (self.to - self.from) * fx::ease_out_cubic(t.min(1.0))
    }
}

/// Hover state of every hoverable surface and its 150 ms crossfade. Lives
/// behind shared cells so components that only borrow the kit can use it.
#[derive(Default)]
struct Hovers {
    hovered: Rc<RefCell<HashSet<u64>>>,
    fades: RefCell<HashMap<u64, Fade>>,
    seen: RefCell<HashSet<u64>>,
    enabled: Cell<bool>,
    animating: Cell<bool>,
}

impl Hovers {
    fn begin_frame(&self, enabled: bool) {
        self.enabled.set(enabled);
        self.animating.set(false);
        self.seen.borrow_mut().clear();
    }

    /// Forget surfaces that were not rendered this frame, so a control that
    /// unmounts under the pointer does not come back hovered.
    fn end_frame(&self) -> bool {
        let seen = self.seen.borrow();
        self.hovered.borrow_mut().retain(|key| seen.contains(key));
        self.fades.borrow_mut().retain(|key, _| seen.contains(key));
        self.animating.get()
    }

    fn progress(&self, key: u64) -> f32 {
        self.seen.borrow_mut().insert(key);
        let target = if self.hovered.borrow().contains(&key) {
            1.0
        } else {
            0.0
        };
        let now = Instant::now();
        let mut fades = self.fades.borrow_mut();
        let fade = fades.entry(key).or_insert(Fade {
            from: target,
            to: target,
            start: now,
        });
        if fade.to != target {
            let from = if self.enabled.get() {
                fade.at(now)
            } else {
                target
            };
            *fade = Fade {
                from,
                to: target,
                start: now,
            };
        }
        let value = fade.at(now);
        if (value - fade.to).abs() > 0.001 {
            self.animating.set(true);
        }
        value
    }
}

/// Hover identity of a surface, derived from its element id.
pub(crate) fn hover_key(id: &str) -> u64 {
    fx::key(("hover", id))
}

fn set_hovered(hovered: &RefCell<HashSet<u64>>, key: u64, on: bool) -> bool {
    if on {
        hovered.borrow_mut().insert(key)
    } else {
        hovered.borrow_mut().remove(&key)
    }
}

/// Track hover on `el`; returns it with its crossfade progress (0..=1).
///
/// Hover comes from a non-blocking hitbox probe rather than `on_hover`,
/// which reports "not hovered" while the button is held and would drop the
/// highlight after every click until the pointer moves again.
pub(crate) fn hoverable<E: ParentElement>(k: &Kit, el: E, key: u64) -> (E, f32) {
    let t = k.hovers.progress(key);
    let hovered = Rc::clone(&k.hovers.hovered);
    let probe = canvas(
        |bounds, window, _| window.insert_hitbox(bounds, gpui::HitboxBehavior::Normal),
        move |_, hitbox, window, _| {
            if set_hovered(&hovered, key, hitbox.is_hovered(window)) {
                window.request_animation_frame();
            }
            let hovered = Rc::clone(&hovered);
            window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, _| {
                if phase == gpui::DispatchPhase::Bubble
                    && set_hovered(&hovered, key, hitbox.is_hovered(window))
                {
                    window.refresh();
                }
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();
    (el.child(probe), t)
}

/// Background that crossfades from `rest` to `hover` while hovered.
pub(crate) fn hover_bg<E: ParentElement + Styled>(
    k: &Kit,
    el: E,
    key: u64,
    rest: Hsla,
    hover: Hsla,
) -> E {
    let (el, t) = hoverable(k, el, key);
    el.bg(blend(rest, hover, t))
}

/// Blend two possibly translucent colors with premultiplied alpha, so a fade
/// from transparent never dips through black.
pub(crate) fn blend(from: Hsla, to: Hsla, t: f32) -> Hsla {
    let t = t.clamp(0.0, 1.0);
    if t <= 0.0 {
        return from;
    }
    if t >= 1.0 {
        return to;
    }
    let (a, b) = (from.to_rgb(), to.to_rgb());
    let alpha = a.a + (b.a - a.a) * t;
    if alpha <= f32::EPSILON {
        return gpui::transparent_black();
    }
    let channel = |x: f32, y: f32| (x * a.a + (y * b.a - x * a.a) * t) / alpha;
    gpui::Rgba {
        r: channel(a.r, b.r),
        g: channel(a.g, b.g),
        b: channel(a.b, b.b),
        a: alpha,
    }
    .into()
}

/// Bracket a card's children so rows inside know its bounds while painting.
/// Place the first element before the rows and the second after them.
fn card_bounds_markers(k: &Kit) -> (AnyElement, AnyElement) {
    let push = Rc::clone(&k.cards);
    let pop = Rc::clone(&k.cards);
    (
        canvas(
            |_, _, _| {},
            move |bounds, (), _, _| push.borrow_mut().push(bounds),
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element(),
        canvas(
            |_, _, _| {},
            move |_, (), _, _| {
                pop.borrow_mut().pop();
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element(),
    )
}

/// A rounded card surface whose rows clip their hover to its corners.
pub(crate) fn card_surface(k: &Kit, children: impl IntoIterator<Item = AnyElement>) -> gpui::Div {
    let (open, close) = card_bounds_markers(k);
    div()
        .relative()
        .flex()
        .flex_col()
        .w_full()
        .rounded(px(CARD_RADIUS))
        .bg(k.theme.card)
        .shadow(card_shadow(&k.theme))
        .overflow_hidden()
        .child(open)
        .children(children)
        .child(close)
}

/// Hover fill of a row, painted under its content with the corners of the
/// card edge it touches.
fn row_hover_fill(k: &Kit, t: f32) -> AnyElement {
    let color = k.theme.subtle_hover.alpha(t);
    let cards = Rc::clone(&k.cards);
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            if t <= 0.001 {
                return;
            }
            let (top, bottom) = match cards.borrow().last() {
                Some(card) => (
                    (f32::from(bounds.top()) - f32::from(card.top())).abs() < 1.0,
                    (f32::from(bounds.bottom()) - f32::from(card.bottom())).abs() < 1.0,
                ),
                None => (true, true),
            };
            let radius = |on: bool| px(if on { CARD_RADIUS } else { 0.0 });
            window.paint_quad(gpui::fill(bounds, color).corner_radii(gpui::Corners {
                top_left: radius(top),
                top_right: radius(top),
                bottom_left: radius(bottom),
                bottom_right: radius(bottom),
            }));
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
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
        .text_size(px(30.0))
        .line_height(px(38.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(k.theme.text)
        .pb(px(10.0))
        .child(value.into())
        .into_any_element()
}

pub(crate) fn section_heading(k: &Kit, value: impl Into<SharedString>) -> AnyElement {
    div()
        .pt(px(24.0))
        .pb(px(8.0))
        .px(px(4.0))
        .text_size(px(13.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(k.theme.text_secondary)
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
                .text_size(px(13.0))
                .line_height(px(18.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(k.theme.text_secondary)
                .child(title.into()),
        );
    if let Some(caption) = caption {
        text_col =
            text_col.child(text(caption, 12.0, k.theme.text_secondary).line_height(px(16.0)));
    }
    div()
        .flex()
        .items_end()
        .gap(px(16.0))
        .pt(px(24.0))
        .pb(px(8.0))
        .px(px(4.0))
        .child(text_col)
        .children(action)
        .into_any_element()
}

pub(crate) fn caption(k: &Kit, value: impl Into<SharedString>) -> AnyElement {
    text(value, 12.0, k.theme.text_secondary)
        .line_height(px(16.0))
        .into_any_element()
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
        .rounded(px(5.0))
        .bg(k.theme.control)
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

/// A driver glyph with the instance badge on its lower-right corner, sized
/// like the popup's provider marks.
pub(crate) fn provider_mark(
    k: &Kit,
    glyph: &str,
    size: f32,
    color: Hsla,
    badge: Option<&crate::instances::Badge>,
) -> AnyElement {
    let mark = div()
        .relative()
        .flex_none()
        .size(px(size))
        .child(icon(glyph, size, color));
    let Some(badge) = badge else {
        return mark.into_any_element();
    };
    let (bg, fg) = k.theme.badge(badge.color);
    let height = (size * 0.62).clamp(9.0, 14.0);
    mark.child(
        div()
            .absolute()
            .right(px(-height * 0.45))
            .bottom(px(-height * 0.3))
            .flex()
            .items_center()
            .justify_center()
            .h(px(height))
            .min_w(px(height))
            .px(px((height * 0.22).max(2.0)))
            .rounded(px(height * 0.3))
            .border_1()
            .border_color(k.theme.window_bg)
            .bg(bg)
            .text_color(fg)
            .text_size(px((height * 0.68).max(7.0)))
            .line_height(px(height))
            .font_weight(FontWeight::BOLD)
            .whitespace_nowrap()
            .child(badge.text.clone()),
    )
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

/// Horizontal slide + fade for panes swapped by navigation; `direction` is
/// +1 to enter from the right, -1 from the left, 0 for the plain [`appear`].
/// The parent must clip, so the travelling pane never paints over its
/// neighbor.
pub(crate) fn slide_in(
    k: &Kit,
    id: impl Into<SharedString>,
    element: AnyElement,
    direction: f32,
    fill: bool,
) -> AnyElement {
    if direction == 0.0 {
        return appear_in(k, id, element, fill);
    }
    if !k.animate() {
        return element;
    }
    div()
        .relative()
        .w_full()
        .when(fill, |el| el.h_full())
        .child(element)
        .with_animation(
            eid(id),
            Animation::new(Duration::from_millis(300)).with_easing(fx::ease_out_cubic),
            move |el, delta| el.opacity(delta).left(px((1.0 - delta) * 32.0 * direction)),
        )
        .into_any_element()
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
    let mut children = Vec::with_capacity(count * 2);
    for (index, row) in rows.into_iter().enumerate() {
        children.push(row);
        if index + 1 < count {
            children.push(
                div()
                    .px(px(ROW_PADDING_X))
                    .child(divider(k))
                    .into_any_element(),
            );
        }
    }
    card_surface(k, children).into_any_element()
}

/// The soft lift under a card (light theme only; dark relies on fills).
pub(crate) fn card_shadow(theme: &Theme) -> Vec<gpui::BoxShadow> {
    vec![gpui::BoxShadow {
        color: theme.card_shadow,
        offset: point(px(0.0), px(1.0)),
        blur_radius: px(3.0),
        spread_radius: px(0.0),
    }]
}

/// Layered shadow for floating surfaces (menus, dialogs, tooltips): a tight
/// contact shadow plus a wide ambient one, so no outline is needed.
pub(crate) fn float_shadow(theme: &Theme, depth: f32) -> Vec<gpui::BoxShadow> {
    vec![
        gpui::BoxShadow {
            color: theme.shadow.alpha(theme.shadow.a * 0.6),
            offset: point(px(0.0), px(1.0)),
            blur_radius: px(3.0),
            spread_radius: px(0.0),
        },
        gpui::BoxShadow {
            color: theme.shadow,
            offset: point(px(0.0), px(depth)),
            blur_radius: px(depth * 2.0),
            spread_radius: px(0.0),
        },
    ]
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
            .gap(px(1.0))
            .flex_1()
            .min_w_0()
            // GPUI's default line height is 1.618em; rows read as one unit
            // only with tight, explicit leading.
            .line_height(px(16.0))
            .child(
                div()
                    .text_size(px(14.0))
                    .line_height(px(20.0))
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
        let hover_key = hover_key(&self.id);
        let on_click = self.on_click.filter(|_| !self.disabled);
        let mut row = div()
            .id(eid(self.id))
            .relative()
            .flex()
            .items_center()
            .gap(px(16.0))
            .min_h(px(ROW_MIN_HEIGHT))
            .px(px(ROW_PADDING_X))
            .py(px(12.0))
            .w_full();
        if let Some(on_click) = on_click {
            let (hovered, t) = hoverable(k, row, hover_key);
            row = hovered
                .cursor_pointer()
                .on_click(move |_, window, cx| on_click((), window, cx))
                .child(row_hover_fill(k, t));
        }
        row = row.children(self.icon).child(text_col).child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .flex_none()
                .children(self.trailing),
        );
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
            .child(
                div()
                    .px(px(ROW_PADDING_X))
                    .child(div().h(px(1.0)).bg(theme_divider)),
            )
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
    card_surface(k, std::iter::once(header).chain(body)).into_any_element()
}

// ---------------------------------------------------------------------------
// Buttons
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ButtonKind {
    Standard,
    Accent,
    Danger,
    Link,
}

pub(crate) struct Button {
    id: SharedString,
    label: Option<SharedString>,
    icon: Option<&'static str>,
    kind: ButtonKind,
    ghost: bool,
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
            ghost: false,
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
            kind: ButtonKind::Standard,
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

    /// No fill at rest; the kind's fill fades in on hover.
    pub(crate) fn ghost(mut self) -> Self {
        self.ghost = true;
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
        let (bg, hover, pressed, fg) = match self.kind {
            ButtonKind::Accent => (
                theme.accent,
                theme.accent_hover,
                theme.accent_pressed,
                theme.on_accent,
            ),
            ButtonKind::Standard => (
                theme.control,
                theme.control_hover,
                theme.control_pressed,
                theme.text,
            ),
            ButtonKind::Danger => (
                theme.control,
                theme.critical_bg,
                theme.critical_bg.alpha(0.7),
                theme.critical,
            ),
            ButtonKind::Link => (
                gpui::transparent_black(),
                theme.subtle_hover,
                theme.subtle_pressed,
                theme.accent_text,
            ),
        };
        let (bg, hover, pressed, fg) = match (self.ghost, self.kind) {
            (false, _) | (true, ButtonKind::Link) => (bg, hover, pressed, fg),
            (true, ButtonKind::Accent) => (
                gpui::transparent_black(),
                theme.accent_soft,
                theme.accent_soft.alpha(0.7),
                theme.accent_text,
            ),
            (true, ButtonKind::Standard) => (
                gpui::transparent_black(),
                theme.control_hover,
                theme.control_pressed,
                fg,
            ),
            (true, ButtonKind::Danger) => (gpui::transparent_black(), hover, pressed, fg),
        };
        let fg = if self.disabled {
            if self.kind == ButtonKind::Accent && !self.ghost {
                theme.on_accent.alpha(0.7)
            } else {
                theme.text_disabled
            }
        } else {
            fg
        };
        let bg = if self.disabled && self.kind == ButtonKind::Accent && !self.ghost {
            theme.control_disabled
        } else {
            bg
        };
        let icon_only = self.label.is_none();
        let key = hover_key(&self.id);
        let mut button = div()
            .id(eid(self.id))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .h(px(CONTROL_HEIGHT))
            .rounded(px(CONTROL_RADIUS))
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
            button = hover_bg(k, button, key, bg, hover)
                .cursor_pointer()
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

/// Round `bounds` to whole device pixels.
fn snap_to_device(bounds: Bounds<Pixels>, scale: f32) -> Bounds<Pixels> {
    let snap = |value: Pixels| px((f32::from(value) * scale).round() / scale);
    gpui::Bounds::from_corners(
        point(snap(bounds.left()), snap(bounds.top())),
        point(snap(bounds.right()), snap(bounds.bottom())),
    )
}

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
    let (track, knob_color) = if disabled {
        (theme.control_disabled, theme.text_disabled)
    } else {
        (
            blend(theme.control_track, theme.accent, t),
            theme.text.mix(theme.on_accent, t),
        )
    };
    let knob_size = 12.0 + 2.0 * t;
    let travel = 40.0 - 8.0 - knob_size;
    let mut el = div()
        .id(eid(format!("toggle-{id}")))
        .relative()
        .w(px(40.0))
        .h(px(20.0))
        .flex_none();
    let mut track = track;
    if !disabled {
        let hover = blend(
            blend(theme.control_track, theme.text_secondary, 0.12),
            theme.accent_hover,
            t,
        );
        let (hovered, h) = hoverable(k, el, hover_key(&format!("toggle-{id}")));
        el = hovered;
        track = blend(track, hover, h);
    }
    // Painted on whole device pixels: GPUI only rasterizes pixels whose
    // centers fall inside a quad, so a pill on a fractional edge loses its
    // antialiased bottom row and looks cropped.
    el = el.child(
        canvas(
            |_, _, _| {},
            move |bounds, (), window, _| {
                let scale = window.scale_factor();
                let track_bounds = snap_to_device(bounds, scale);
                let radius = track_bounds.size.height / 2.0;
                window.paint_quad(gpui::fill(track_bounds, track).corner_radii(radius));
                let knob = Bounds::new(
                    point(
                        bounds.origin.x + px(4.0 + travel * t),
                        bounds.origin.y + px((20.0 - knob_size) / 2.0),
                    ),
                    gpui::size(px(knob_size), px(knob_size)),
                );
                let knob = snap_to_device(knob, scale);
                let radius = knob.size.height / 2.0;
                window.paint_quad(gpui::fill(knob, knob_color).corner_radii(radius));
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full(),
    );
    if !disabled {
        el = el.cursor_pointer().on_click(move |_, window, cx| {
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
    // Same height as the switch, so centering never lands on a half pixel.
    .line_height(px(20.0))
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
    let mut el = div().id(eid(format!("check-{id}")));
    let mut hover = 0.0;
    if !disabled {
        (el, hover) = hoverable(k, el, hover_key(&format!("check-{id}")));
    }
    let fill = if disabled {
        theme.control_disabled
    } else {
        let rest = blend(theme.control_track, theme.accent, t);
        let lifted = blend(
            blend(theme.control_track, theme.text_secondary, 0.12),
            theme.accent_hover,
            t,
        );
        rest.mix(lifted, hover)
    };
    let box_el = div()
        .size(px(18.0))
        .flex_none()
        .rounded(px(5.0))
        .bg(fill)
        .flex()
        .items_center()
        .justify_center()
        .when(t > 0.01, |el| {
            el.child(icon("check-bold", 12.0 * (0.6 + 0.4 * t), theme.on_accent).opacity(t))
        });
    el = el
        .flex()
        .items_center()
        .gap(px(10.0))
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

/// The popup's segmented control ([`segmented_track`]) in Settings colors.
pub(crate) fn segmented(
    k: &mut Kit,
    id: impl Into<SharedString>,
    labels: &[&str],
    selected: usize,
    disabled: bool,
    on_select: Handler<usize>,
) -> AnyElement {
    let id: SharedString = id.into();
    let key = fx::key(("settings-segmented", id.as_ref()));
    let theme = &k.theme;
    let style = SegmentStyle {
        track: theme.control,
        thumb: if disabled {
            theme.control_disabled
        } else {
            theme.accent
        },
        text: if disabled {
            theme.text_disabled
        } else {
            theme.text
        },
        text_on_thumb: if disabled {
            theme.text_disabled
        } else {
            theme.on_accent
        },
        hover: theme.subtle_hover,
        divider: theme.divider,
    };
    let segments = labels
        .iter()
        .map(|label| Segment::text(label.to_string()))
        .collect();
    let hover_set = Rc::clone(&k.hovered);
    let hovered = |index: usize| hover_set.borrow().contains(&fx::key((key, index)));
    let hover_cells = Rc::clone(&k.hovered);
    let mut wire = |index: usize, cell: gpui::Stateful<gpui::Div>| {
        if disabled {
            return cell.cursor_default();
        }
        let on_select = Rc::clone(&on_select);
        let hover_cells = Rc::clone(&hover_cells);
        let hover_id = fx::key((key, index));
        cell.on_hover(move |hovered, window, _| {
            let changed = if *hovered {
                hover_cells.borrow_mut().insert(hover_id)
            } else {
                hover_cells.borrow_mut().remove(&hover_id)
            };
            if changed {
                window.refresh();
            }
        })
        .on_click(move |_, window, cx| on_select(index, window, cx))
    };
    segmented_track(
        &mut k.fx, key, segments, selected, false, style, &hovered, &mut wire,
    )
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
        let rest = if is_selected {
            theme.nav_selected
        } else {
            gpui::transparent_black()
        };
        let accent = theme.accent;
        let mut entry = div()
            .id(eid(format!("menu-{id}-{index}")))
            .relative()
            .flex()
            .items_center()
            .gap(px(10.0))
            .h(px(32.0))
            .px(px(12.0))
            .rounded(px(CONTROL_RADIUS))
            .text_size(px(14.0))
            .text_color(color)
            .bg(rest)
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
            let key = hover_key(&format!("menu-{id}-{index}"));
            entry = hover_bg(k, entry, key, rest, if is_selected { rest } else { hover })
                .cursor_pointer()
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
        .rounded(px(10.0))
        .bg(theme.popover)
        .shadow(float_shadow(theme, 8.0))
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
    let rest = if open { hover } else { theme.control };
    let mut button = div()
        .id(eid(format!("dropdown-{id}")))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.0))
        .h(px(CONTROL_HEIGHT))
        .px(px(12.0))
        .rounded(px(CONTROL_RADIUS))
        .bg(if disabled {
            theme.control_disabled.alpha(0.3)
        } else if open {
            theme.control_hover
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
        let key = hover_key(&format!("dropdown-{id}"));
        button = hover_bg(k, button, key, rest, hover)
            .cursor_pointer()
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
                .bg(theme.control_track)
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
                .bg(theme.control_solid)
                .shadow(vec![gpui::BoxShadow {
                    color: theme.shadow.alpha(0.35),
                    offset: point(px(0.0), px(1.0)),
                    blur_radius: px(2.0),
                    spread_radius: px(0.0),
                }])
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

/// The box around a [`TextInput`]: a fill and the Fluent accent underline
/// while focused.
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
        .px(px(12.0))
        .rounded(px(CONTROL_RADIUS))
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
        .when(focused, |el| {
            el.child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(px(2.0))
                    .bg(theme.accent),
            )
        });
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
        .rounded(px(14.0))
        .bg(theme.dialog)
        .shadow(float_shadow(theme, 24.0))
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
                // GPUI clips children to a rectangle, not the card's radius:
                // round the tinted footer itself so it can't paint past the
                // card's bottom corners.
                .rounded_b(px(14.0))
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
        .line_height(px(28.0))
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
            .rounded(px(CONTROL_RADIUS))
            .bg(self.theme.popover)
            .shadow(float_shadow(&self.theme, 4.0))
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

/// A title bar caption button (minimize, maximize, close).
pub(crate) fn caption_button(
    k: &Kit,
    id: &'static str,
    glyph: &'static str,
    area: gpui::WindowControlArea,
    close: bool,
) -> AnyElement {
    let theme = &k.theme;
    let hover = if close {
        crate::popup_window::ui::theme::rgb8((0xC4, 0x2B, 0x1C))
    } else {
        theme.subtle_hover
    };
    let (el, t) = hoverable(k, div().id(id), hover_key(id));
    el.w(px(46.0))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.0))
        .font_family(theme.icon_font.clone())
        .text_color(if close {
            theme.text.mix(gpui::white(), t)
        } else {
            theme.text
        })
        .bg(blend(gpui::transparent_black(), hover, t))
        .window_control_area(area)
        // Occlude the drag area beneath so the button, not the caption,
        // wins the non-client hit test.
        .occlude()
        .child(glyph)
        .into_any_element()
}

// ---------------------------------------------------------------------------
// Dialog pickers
// ---------------------------------------------------------------------------

/// Numbered step strip for multi-step dialogs. Finished steps show a check
/// and can be revisited; with `free`, every step is reachable and none is
/// marked finished (editors whose pages can be visited in any order).
pub(crate) fn stepper(
    k: &Kit,
    id: &str,
    labels: &[&str],
    current: usize,
    free: bool,
    on_select: Handler<usize>,
) -> AnyElement {
    let theme = &k.theme;
    let steps = labels.iter().enumerate().map(|(index, label)| {
        let active = index == current;
        let done = !free && index < current;
        let reachable = !active && (free || done);
        let marker = div()
            .size(px(22.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .text_size(px(12.0))
            .line_height(px(16.0))
            .font_weight(FontWeight::SEMIBOLD)
            .map(|el| {
                if active || done {
                    el.bg(theme.accent).text_color(theme.on_accent)
                } else {
                    el.bg(theme.control_track).text_color(theme.text_secondary)
                }
            })
            .map(|el| {
                if done {
                    el.child(icon("check-bold", 12.0, theme.on_accent))
                } else {
                    el.child((index + 1).to_string())
                }
            });
        let step_id = format!("{id}-step-{index}");
        let rest = if active {
            theme.card
        } else {
            gpui::transparent_black()
        };
        let on_select = Rc::clone(&on_select);
        div()
            .id(eid(step_id.clone()))
            .flex_1()
            .flex()
            .items_center()
            .gap(px(10.0))
            .h(px(40.0))
            .px(px(12.0))
            .rounded(px(CONTROL_RADIUS))
            .bg(rest)
            .when(active, |el| el.shadow(card_shadow(theme)))
            .when(reachable, |el| {
                hover_bg(k, el, hover_key(&step_id), rest, theme.subtle_hover)
                    .cursor_pointer()
                    .on_click(move |_, window, cx| on_select(index, window, cx))
            })
            .child(marker)
            .child(text(
                label.to_string(),
                14.0,
                if active || done {
                    theme.text
                } else {
                    theme.text_secondary
                },
            ))
            .into_any_element()
    });
    div()
        .flex()
        .gap(px(4.0))
        .p(px(4.0))
        .rounded(px(CARD_RADIUS))
        .bg(theme.subtle_hover)
        .children(steps)
        .into_any_element()
}

/// A selectable card in a dialog picker: leading visual, title, optional
/// subtitle and trailing controls, with a check once selected. Filled with
/// the control tint so it reads on the dialog surface in both themes. The
/// click handler receives `true` on a double click.
pub(crate) struct ChoiceCard {
    id: SharedString,
    leading: Option<AnyElement>,
    title: SharedString,
    trailing: Vec<AnyElement>,
    selected: bool,
    on_click: Option<Handler<bool>>,
}

impl ChoiceCard {
    pub(crate) fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            leading: None,
            title: title.into(),
            trailing: Vec::new(),
            selected: false,
            on_click: None,
        }
    }

    pub(crate) fn leading(mut self, element: AnyElement) -> Self {
        self.leading = Some(element);
        self
    }

    pub(crate) fn trailing(mut self, element: AnyElement) -> Self {
        self.trailing.push(element);
        self
    }

    pub(crate) fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub(crate) fn on_click(mut self, handler: Handler<bool>) -> Self {
        self.on_click = Some(handler);
        self
    }

    pub(crate) fn render(self, k: &Kit) -> gpui::Stateful<gpui::Div> {
        let theme = &k.theme;
        let rest = if self.selected {
            theme.accent_soft
        } else {
            theme.control
        };
        let key = hover_key(&self.id);
        let text_col = div()
            .flex()
            .flex_col()
            .gap(px(1.0))
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .truncate()
                    .text_size(px(14.0))
                    .line_height(px(20.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text)
                    .child(self.title),
            );
        let mut card = div()
            .id(eid(self.id))
            .flex()
            .items_center()
            .gap(px(12.0))
            .min_h(px(56.0))
            .px(px(14.0))
            .py(px(10.0))
            .rounded(px(CARD_RADIUS))
            .bg(rest)
            .children(self.leading)
            .child(text_col)
            .children(self.trailing)
            .when(self.selected, |el| {
                el.child(icon("check-circle-fill", 20.0, theme.accent_text))
            });
        if let Some(on_click) = self.on_click {
            let hover = if self.selected {
                rest
            } else {
                theme.control_hover
            };
            card = hover_bg(k, card, key, rest, hover)
                .cursor_pointer()
                .on_click(move |event, window, cx| on_click(event.click_count() > 1, window, cx));
        }
        card
    }
}

/// A rounded plate behind a choice card's leading glyph or preview.
pub(crate) fn glyph_plate(size: f32, bg: Hsla, content: AnyElement) -> AnyElement {
    div()
        .size(px(size))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(8.0))
        .bg(bg)
        .child(content)
        .into_any_element()
}

// ---------------------------------------------------------------------------
// Time picker
// ---------------------------------------------------------------------------

const PICKER_ITEM: f32 = 32.0;
const PICKER_ROWS: f32 = 7.0;

/// Fluent TimePicker: a field showing the time that opens a flyout with
/// scrollable hour and minute columns (plus AM/PM in 12-hour format). Every
/// pick applies at once; the flyout closes on an outside click.
pub(crate) fn time_picker(
    k: &Kit,
    id: impl Into<SharedString>,
    minutes: u16,
    format: crate::settings::TimeFormat,
    on_change: Handler<u16>,
) -> AnyElement {
    use crate::settings::TimeFormat;
    let id: SharedString = id.into();
    let theme = &k.theme;
    let minutes = minutes.min(23 * 60 + 59);
    let (hour, minute) = (minutes / 60, minutes % 60);
    let open = k.menus.is_open(&id);
    let label = match format {
        TimeFormat::Hour24 => format!("{hour:02}:{minute:02}"),
        TimeFormat::Hour12 => format!(
            "{}:{minute:02} {}",
            match hour % 12 {
                0 => 12,
                value => value,
            },
            if hour < 12 { "AM" } else { "PM" }
        ),
    };
    let menus = k.menus.clone();
    let toggle_id = id.clone();
    let trigger_id = format!("time-{id}");
    let rest = if open {
        theme.control_hover
    } else {
        theme.control
    };
    let trigger = hover_bg(
        k,
        div().id(eid(trigger_id.clone())),
        hover_key(&trigger_id),
        rest,
        theme.control_hover,
    )
    .flex()
    .items_center()
    .gap(px(10.0))
    .h(px(CONTROL_HEIGHT))
    .px(px(12.0))
    .w(px(if format == TimeFormat::Hour12 {
        128.0
    } else {
        104.0
    }))
    .rounded(px(CONTROL_RADIUS))
    .text_size(px(14.0))
    .line_height(px(20.0))
    .text_color(theme.text)
    .cursor_pointer()
    .on_click(move |_, window, _| menus.toggle(toggle_id.clone(), window))
    .child(div().flex_1().child(label))
    .child(icon("clock-fill", 14.0, theme.text_secondary));

    let mut wrapper = div().relative().flex_none().child(trigger);
    if !open {
        k.pickers.borrow_mut().remove(&id);
        return wrapper.into_any_element();
    }

    // Columns: (values, labels, selected index, value -> minutes).
    type Apply = Rc<dyn Fn(u16) -> u16>;
    let mut columns: Vec<(Vec<SharedString>, usize, Apply)> = Vec::new();
    match format {
        TimeFormat::Hour24 => columns.push((
            (0..24).map(|h| format!("{h:02}").into()).collect(),
            hour as usize,
            Rc::new(move |h| h * 60 + minute),
        )),
        TimeFormat::Hour12 => columns.push((
            (1..=12).map(|h| h.to_string().into()).collect(),
            ((hour + 11) % 12) as usize,
            Rc::new(move |index| {
                let hour12 = (index + 1) % 12;
                let hour = if hour >= 12 { hour12 + 12 } else { hour12 };
                hour * 60 + minute
            }),
        )),
    }
    columns.push((
        (0..60).map(|m| format!("{m:02}").into()).collect(),
        minute as usize,
        Rc::new(move |m| hour * 60 + m),
    ));
    if format == TimeFormat::Hour12 {
        columns.push((
            vec!["AM".into(), "PM".into()],
            usize::from(hour >= 12),
            Rc::new(move |period| {
                let hour = hour % 12 + if period == 1 { 12 } else { 0 };
                hour * 60 + minute
            }),
        ));
    }

    let handles = {
        let mut pickers = k.pickers.borrow_mut();
        let entry = pickers.entry(id.clone()).or_insert_with(|| {
            (
                (0..columns.len())
                    .map(|_| gpui::ScrollHandle::new())
                    .collect(),
                false,
            )
        });
        if !entry.1 {
            // Bring each column's value near the top on open.
            for (handle, (_, selected, _)) in entry.0.iter().zip(&columns) {
                handle.scroll_to_top_of_item(selected.saturating_sub(2));
            }
            entry.1 = true;
        }
        entry.0.clone()
    };

    let mut panel_columns = div().flex().gap(px(4.0));
    for (column_index, ((labels, selected, apply), handle)) in
        columns.into_iter().zip(handles).enumerate()
    {
        let scrolls = labels.len() as f32 > PICKER_ROWS;
        let mut list = div()
            .id(eid(format!("time-{id}-col-{column_index}")))
            .flex()
            .flex_col()
            .gap(px(2.0))
            .w(px(60.0))
            .when(scrolls, |el| {
                el.h(px(PICKER_ROWS * (PICKER_ITEM + 2.0)))
                    .overflow_y_scroll()
                    .track_scroll(&handle)
            });
        for (index, label) in labels.into_iter().enumerate() {
            let is_selected = index == selected;
            let item_id = format!("time-{id}-{column_index}-{index}");
            let apply = Rc::clone(&apply);
            let on_change = Rc::clone(&on_change);
            let (rest, hover, fg) = if is_selected {
                (theme.accent, theme.accent_hover, theme.on_accent)
            } else {
                (gpui::transparent_black(), theme.subtle_hover, theme.text)
            };
            list = list.child(
                hover_bg(
                    k,
                    div().id(eid(item_id.clone())),
                    hover_key(&item_id),
                    rest,
                    hover,
                )
                .flex_none()
                .h(px(PICKER_ITEM))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(CONTROL_RADIUS))
                .text_size(px(14.0))
                .line_height(px(20.0))
                .text_color(fg)
                .when(is_selected, |el| el.font_weight(FontWeight::SEMIBOLD))
                .cursor_pointer()
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    on_change(apply(index as u16), window, cx)
                })
                .child(label),
            );
        }
        panel_columns = panel_columns.child(list);
    }

    let menus = k.menus.clone();
    let panel = div()
        .id(eid(format!("time-panel-{id}")))
        .occlude()
        .mt(px(4.0))
        .p(px(4.0))
        .rounded(px(10.0))
        .bg(theme.popover)
        .shadow(float_shadow(theme, 8.0))
        .on_mouse_down_out(move |_, window, _| menus.close(window))
        .child(panel_columns);
    let panel: AnyElement = if k.animate() {
        panel
            .with_animation(
                eid(format!("time-anim-{id}")),
                Animation::new(Duration::from_millis(167)).with_easing(fx::ease_out_cubic),
                |el, delta| el.opacity(delta).mt(px(4.0 - (1.0 - delta) * 8.0)),
            )
            .into_any_element()
    } else {
        panel.into_any_element()
    };
    wrapper = wrapper.child(div().absolute().top_full().left_0().child(
        deferred(anchored().snap_to_window_with_margin(px(8.0)).child(panel)).with_priority(2),
    ));
    wrapper.into_any_element()
}
