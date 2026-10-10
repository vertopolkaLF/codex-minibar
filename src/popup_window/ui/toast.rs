//! Animated in-app notifications.
//!
//! One transparent, topmost, never-activated GPUI window in the bottom-right
//! corner of the primary monitor's work area stacks the cards. Its region is
//! clipped to the cards, so the rest of the host never swallows clicks. The
//! newest card enters at the bottom and pushes older ones up; each leaves
//! after its kind's lifetime, which hovering pauses.

use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, AppContext, AsyncApp, Bounds, ClickEvent, Context, FontWeight, Hsla,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, WindowBackgroundAppearance, WindowBounds,
    WindowHandle, WindowKind, WindowOptions, div, point, px, size,
};

use crate::notifications::{Notification, NotificationKind};

use super::{
    PopupRoot,
    components::{self, nowrap},
    fx::{self, Fx},
    theme::{HslaExt, Palette, rgb8},
};

const CARD_WIDTH: f32 = 360.0;
/// Room around the cards for their shadow and the slide-in overshoot.
const EDGE: f32 = 16.0;
const GAP: f32 = 10.0;
const HOST_HEIGHT: f32 = 620.0;
const MAX_CARDS: usize = 4;
const PADDING: f32 = 14.0;
const PLATE: f32 = 34.0;
const TITLE_LINE: f32 = 20.0;
const BODY_LINE: f32 = 18.0;
const BODY_MAX_LINES: usize = 3;
const ACTIONS_HEIGHT: f32 = 30.0;
const PROGRESS_HEIGHT: f32 = 3.0;
const ENTER: Duration = Duration::from_millis(560);
const EXIT: Duration = Duration::from_millis(280);
const ICON_POP: Duration = Duration::from_millis(620);
const ICON_DELAY: Duration = Duration::from_millis(120);
const TICK: Duration = Duration::from_millis(100);

thread_local! {
    static HOST: RefCell<Option<WindowHandle<ToastHost>>> = const { RefCell::new(None) };
}

struct Card {
    id: u64,
    notification: Notification,
    height: f32,
    born: Instant,
    /// Lifetime used so far; it does not grow while hovered.
    shown: Duration,
    last_tick: Instant,
    hovered: bool,
    leaving: Option<Instant>,
}

impl Card {
    fn exit_progress(&self, now: Instant) -> f32 {
        self.leaving.map_or(0.0, |since| {
            progress(now.saturating_duration_since(since), EXIT)
        })
    }
}

pub(crate) struct ToastHost {
    popup: WindowHandle<PopupRoot>,
    default_font: SharedString,
    cards: Vec<Card>,
    next_id: u64,
    fx: Fx,
    /// Palette of the popup state it was built from, keyed by that state and
    /// the system appearance; resolving the system accent every frame is slow.
    palette: Option<(*const super::UiState, bool, Palette)>,
    ticking: bool,
    visible: bool,
    #[cfg(windows)]
    hwnd: Option<windows_sys::Win32::Foundation::HWND>,
    #[cfg(windows)]
    last_region: Vec<super::win32::Rect>,
}

fn progress(elapsed: Duration, duration: Duration) -> f32 {
    if duration.is_zero() {
        return 1.0;
    }
    (elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0)
}

/// Back-out easing: overshoots a little, then settles.
fn ease_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

/// Damped spring for the icon pop: 0 → past 1 → settles at 1.
fn ease_out_elastic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t <= 0.0 || t >= 1.0 {
        return t;
    }
    let c4 = (2.0 * std::f32::consts::PI) / 3.2;
    2f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
}

/// Icon and tint of each kind.
fn kind_style(kind: NotificationKind, palette: &Palette) -> (&'static str, Hsla) {
    let dark = palette.dark;
    let pick =
        |on_dark: (u8, u8, u8), on_light: (u8, u8, u8)| rgb8(if dark { on_dark } else { on_light });
    match kind {
        NotificationKind::Info => ("info-fill", palette.accent),
        NotificationKind::Success => (
            "check-circle-fill",
            pick((0x6C, 0xCB, 0x5F), (0x0F, 0x7B, 0x0F)),
        ),
        NotificationKind::Reset => (
            "arrow-clockwise-bold",
            pick((0x4C, 0xD6, 0xC0), (0x00, 0x7A, 0x6C)),
        ),
        NotificationKind::Warning => ("warning-fill", palette.caution),
        NotificationKind::Error => ("x-circle-fill", palette.critical),
        NotificationKind::Update => (
            "download-simple-fill",
            pick((0xB4, 0x9C, 0xFF), (0x6B, 0x45, 0xC8)),
        ),
    }
}

impl ToastHost {
    fn new(popup: WindowHandle<PopupRoot>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe_window_appearance(window, |this, _, cx| {
            this.palette = None;
            cx.notify();
        })
        .detach();
        Self {
            popup,
            default_font: super::theme::default_font_family(cx),
            cards: Vec::new(),
            next_id: 0,
            fx: Fx::default(),
            palette: None,
            ticking: false,
            visible: false,
            #[cfg(windows)]
            hwnd: None,
            #[cfg(windows)]
            last_region: Vec::new(),
        }
    }

    fn palette(&mut self, window: &Window, cx: &gpui::App) -> Palette {
        let system_dark = super::root::system_dark(window);
        let Ok(root) = self.popup.read(cx) else {
            return self
                .palette
                .as_ref()
                .map(|(_, _, palette)| palette.clone())
                .unwrap_or_else(|| fallback_palette(system_dark, &self.default_font));
        };
        let key = Rc::as_ptr(&root.ui);
        if let Some((cached, dark, palette)) = &self.palette
            && *cached == key
            && *dark == system_dark
        {
            return palette.clone();
        }
        let palette = root.palette_for(system_dark);
        self.palette = Some((key, system_dark, palette.clone()));
        palette
    }

    /// Card height for `notification`: one title line, up to three wrapped
    /// body lines and the action row. Cards have a fixed height so the stack
    /// can glide without waiting for layout.
    fn measure(&self, notification: &Notification, window: &Window, font: &SharedString) -> f32 {
        let text_width = CARD_WIDTH - PADDING * 2.0 - PLATE - 12.0 - 20.0;
        let lines = if notification.body.trim().is_empty() {
            0
        } else {
            let run = gpui::TextRun {
                len: notification.body.len(),
                font: gpui::font(font.clone()),
                color: gpui::black(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            notification
                .body
                .lines()
                .map(|line| {
                    let width = f32::from(
                        window
                            .text_system()
                            .shape_line(
                                SharedString::from(line.to_owned()),
                                px(13.0),
                                &[gpui::TextRun {
                                    len: line.len(),
                                    ..run.clone()
                                }],
                                None,
                            )
                            .width,
                    );
                    // Word wrapping leaves ragged line ends; leave slack.
                    ((width * 1.08) / text_width).ceil().max(1.0) as usize
                })
                .sum::<usize>()
                .clamp(1, BODY_MAX_LINES)
        };
        let actions = if notification.actions.is_empty() {
            0.0
        } else {
            ACTIONS_HEIGHT + 10.0
        };
        let text = TITLE_LINE + if lines > 0 { 2.0 } else { 0.0 } + lines as f32 * BODY_LINE;
        PADDING * 2.0 + text.max(PLATE) + actions + PROGRESS_HEIGHT
    }

    /// Adds a card. Returns the native work to do outside this borrow when
    /// the host has to appear.
    fn push(
        &mut self,
        notification: Notification,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        crate::notifications::play_sound(&notification);
        let font = self.palette(window, cx).font_family;
        let height = self.measure(&notification, window, &font);
        let now = Instant::now();
        // Identical notifications (e.g. a burst of the same error) replace
        // the visible one instead of piling up.
        self.cards.retain(|card| {
            card.leaving.is_some()
                || card.notification.title != notification.title
                || card.notification.body != notification.body
        });
        self.next_id += 1;
        self.cards.push(Card {
            id: self.next_id,
            notification,
            height,
            born: now,
            shown: Duration::ZERO,
            last_tick: now,
            hovered: false,
            leaving: None,
        });
        // Too many: the oldest ones leave early.
        let mut staying = self
            .cards
            .iter()
            .filter(|card| card.leaving.is_none())
            .count();
        for card in &mut self.cards {
            if staying <= MAX_CARDS {
                break;
            }
            if card.leaving.is_none() {
                card.leaving = Some(now);
                staying -= 1;
            }
        }
        if !self.ticking {
            self.ticking = true;
            cx.spawn(async move |this, cx| tick(this, cx).await)
                .detach();
        }
        cx.notify();
        let appear = !self.visible;
        self.visible = true;
        appear
    }

    fn dismiss(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(card) = self.cards.iter_mut().find(|card| card.id == id)
            && card.leaving.is_none()
        {
            card.leaving = Some(Instant::now());
            cx.notify();
        }
    }

    fn set_hovered(&mut self, id: u64, hovered: bool, cx: &mut Context<Self>) {
        if let Some(card) = self.cards.iter_mut().find(|card| card.id == id) {
            card.hovered = hovered;
            cx.notify();
        }
    }

    /// Advances lifetimes, starts exits and drops finished cards.
    fn advance(&mut self, now: Instant, animate: bool) {
        for card in &mut self.cards {
            let delta = now.saturating_duration_since(card.last_tick);
            card.last_tick = now;
            let entered = !animate || now.saturating_duration_since(card.born) >= ENTER / 2;
            if card.leaving.is_none() && !card.hovered && entered {
                card.shown += delta;
                if card.shown >= card.notification.kind.lifetime() {
                    card.leaving = Some(now);
                }
            }
        }
        self.cards.retain(|card| {
            card.leaving
                .is_none_or(|since| animate && now.saturating_duration_since(since) < EXIT)
        });
    }
}

/// Wakes the host while cards are up: lifetimes advance even when motion is
/// off and no animation frame is pending. Hides the host after the last card.
async fn tick(this: gpui::WeakEntity<ToastHost>, cx: &mut AsyncApp) {
    loop {
        cx.background_executor().timer(TICK).await;
        let Ok(state) = this.update(cx, |this, cx| {
            if this.cards.is_empty() {
                this.ticking = false;
                this.visible = false;
                #[cfg(windows)]
                {
                    this.last_region.clear();
                    return Some(this.hwnd);
                }
                #[cfg(not(windows))]
                return Some(());
            }
            cx.notify();
            None
        }) else {
            return;
        };
        if let Some(_hwnd) = state {
            #[cfg(windows)]
            if let Some(hwnd) = _hwnd {
                super::win32::set_region(hwnd, None, 0);
                super::win32::hide(hwnd);
            }
            return;
        }
    }
}

fn fallback_palette(dark: bool, font: &SharedString) -> Palette {
    Palette::new(
        crate::settings::PopupTheme::Fluent,
        None,
        dark,
        super::theme::accent_ramp(crate::settings::AccentColor::default()),
        crate::settings::PopupBackgroundMaterial::Solid,
        font.clone(),
    )
}

impl Render for ToastHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let animate = crate::theme::animations_enabled();
        let now = Instant::now();
        self.fx.begin_frame(animate);
        self.advance(now, animate);
        let palette = self.palette(window, cx);

        // Bottom-up stack: the newest card sits lowest. Leaving cards keep
        // their slot while they fade so nothing jumps under the pointer.
        // The host can be shorter than `HOST_HEIGHT` on a small work area.
        let mut bottom = f32::from(window.viewport_size().height) - EDGE;
        let mut frames = Vec::with_capacity(self.cards.len());
        for card in self.cards.iter().rev() {
            let target = bottom - card.height;
            bottom = target - GAP;
            let y = self
                .fx
                .value(fx::key(("toast-y", card.id)), target, fx::SETTLE);
            frames.push((card.id, y));
        }
        frames.reverse();

        let mut animating = self.fx.is_animating();
        let mut elements: Vec<AnyElement> = Vec::with_capacity(self.cards.len());
        #[cfg(windows)]
        let mut region = Vec::with_capacity(self.cards.len());
        let scale = window.scale_factor();
        for (card, (_, y)) in self.cards.iter().zip(frames) {
            let enter = if animate {
                progress(now.saturating_duration_since(card.born), ENTER)
            } else {
                1.0
            };
            let exit = if animate {
                card.exit_progress(now)
            } else {
                0.0
            };
            let shown = card.notification.kind.lifetime();
            if animate && (enter < 1.0 || card.leaving.is_some() || !card.hovered) {
                animating = true;
            }
            let enter_eased = ease_out_back(enter);
            let exit_eased = fx::ease_out_cubic(exit);
            let slide = (1.0 - enter_eased) * (CARD_WIDTH * 0.55) + exit_eased * (CARD_WIDTH * 0.4);
            let alpha = (enter * 2.5).min(1.0) * (1.0 - exit_eased);
            let x = EDGE + slide;
            #[cfg(windows)]
            if alpha > 0.0 {
                // Visible part only: the card slides out past the host edge.
                let left = ((x - 4.0) * scale).floor() as i32;
                let right = ((EDGE + CARD_WIDTH + 4.0) * scale).ceil() as i32;
                region.push(super::win32::Rect {
                    left,
                    top: ((y - 4.0) * scale).floor() as i32,
                    right: right.max(left),
                    bottom: ((y + card.height + 8.0) * scale).ceil() as i32,
                });
            }
            let icon_t = if animate {
                progress(
                    now.saturating_duration_since(card.born)
                        .saturating_sub(ICON_DELAY),
                    ICON_POP,
                )
            } else {
                1.0
            };
            let remaining = if card.leaving.is_some() {
                0.0
            } else {
                1.0 - card.shown.as_secs_f32() / shown.as_secs_f32()
            };
            let hover = self.fx.value(
                fx::key(("toast-hover", card.id)),
                if card.hovered { 1.0 } else { 0.0 },
                fx::FAST,
            );
            elements.push(self.card(
                card,
                x,
                y,
                alpha,
                ease_out_elastic(icon_t),
                remaining.clamp(0.0, 1.0),
                hover,
                &palette,
                cx,
            ));
        }
        #[cfg(windows)]
        if let Some(hwnd) = self.hwnd
            && region != self.last_region
        {
            set_region(hwnd, &region);
            self.last_region = region;
        }
        #[cfg(not(windows))]
        let _ = scale;
        if animating || self.fx.is_animating() {
            window.request_animation_frame();
        }
        div()
            .size_full()
            .relative()
            .font_family(palette.font_family.clone())
            .children(elements)
    }
}

impl ToastHost {
    #[allow(clippy::too_many_arguments)]
    fn card(
        &self,
        card: &Card,
        x: f32,
        y: f32,
        alpha: f32,
        icon_scale: f32,
        remaining: f32,
        hover: f32,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = card.id;
        let notification = &card.notification;
        let (icon, tint) = kind_style(notification.kind, palette);
        let radius = (palette.card_radius + 4.0).min(16.0);
        let plate_glyph = 18.0 * icon_scale.max(0.0);
        let plate = div()
            .size(px(PLATE))
            .flex_none()
            .rounded(px(PLATE / 2.0))
            .bg(tint.alpha(0.12 + 0.06 * hover))
            .flex()
            .items_center()
            .justify_center()
            .child(components::icon(icon, plate_glyph, tint));

        let mut text = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(2.0))
            .child(nowrap(
                components::text(
                    notification.title.clone(),
                    14.0,
                    TITLE_LINE,
                    palette.text_primary,
                )
                .font_weight(FontWeight::SEMIBOLD),
            ));
        if !notification.body.trim().is_empty() {
            text = text.child(
                components::text(
                    notification.body.clone(),
                    13.0,
                    BODY_LINE,
                    palette.text_secondary,
                )
                .line_clamp(BODY_MAX_LINES),
            );
        }
        if !notification.actions.is_empty() {
            let buttons =
                notification
                    .actions
                    .iter()
                    .enumerate()
                    .map(|(index, (label, action))| {
                        let primary = index == 0;
                        let action = action.clone();
                        let (background, foreground) = if primary {
                            (tint, palette.text_on_accent)
                        } else {
                            (palette.control_fill, palette.text_primary)
                        };
                        div()
                            .id(SharedString::from(format!("toast-{id}-action-{index}")))
                            .h(px(ACTIONS_HEIGHT))
                            .px(px(14.0))
                            .flex()
                            .items_center()
                            .rounded(px(palette.control_radius))
                            .bg(background)
                            .when_border(palette)
                            .cursor_pointer()
                            .hover(|style| style.opacity(0.88))
                            .child(
                                components::text(label.clone(), 13.0, 18.0, foreground)
                                    .font_weight(FontWeight::MEDIUM),
                            )
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                action.run();
                                this.dismiss(id, cx);
                            }))
                            .into_any_element()
                    });
            text = text.child(
                div()
                    .pt(px(10.0))
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .children(buttons),
            );
        }

        let close = div()
            .id(SharedString::from(format!("toast-{id}-close")))
            .absolute()
            .top(px(8.0))
            .right(px(8.0))
            .size(px(24.0))
            .rounded(px(12.0))
            .flex()
            .items_center()
            .justify_center()
            .opacity(0.35 + 0.65 * hover)
            .cursor_pointer()
            .hover(|style| style.bg(palette.subtle_fill))
            .child(components::icon("x-bold", 12.0, palette.text_secondary))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                cx.stop_propagation();
                this.dismiss(id, cx);
            }));

        // The countdown drains from the right and pauses under the pointer.
        let bar = div()
            .absolute()
            .left_0()
            .bottom_0()
            .h(px(PROGRESS_HEIGHT))
            .w(px(CARD_WIDTH * remaining))
            .bg(tint.alpha(0.55 + 0.3 * hover));

        // A soft wash of the kind's color behind the icon.
        let glow = div()
            .absolute()
            .left(px(-40.0))
            .top(px(-40.0))
            .size(px(140.0))
            .rounded(px(70.0))
            .bg(tint.alpha(if palette.dark { 0.10 } else { 0.07 }));

        div()
            .id(SharedString::from(format!("toast-{id}")))
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(CARD_WIDTH))
            .h(px(card.height))
            .opacity(alpha)
            .rounded(px(radius))
            .overflow_hidden()
            .bg(palette.solid_background)
            .border_1()
            .border_color(if palette.borders {
                palette.card_stroke
            } else {
                gpui::transparent_black()
            })
            .shadow(vec![gpui::BoxShadow {
                color: gpui::black().alpha(if palette.dark { 0.32 } else { 0.12 }),
                offset: point(px(0.0), px(4.0)),
                blur_radius: px(10.0),
                spread_radius: px(0.0),
            }])
            .cursor_pointer()
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.set_hovered(id, *hovered, cx);
            }))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.dismiss(id, cx);
            }))
            .child(glow)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(px(12.0))
                    .p(px(PADDING))
                    .pr(px(PADDING + 20.0))
                    .child(plate)
                    .child(text),
            )
            .child(close)
            .child(bar)
            .into_any_element()
    }
}

trait BorderExt {
    fn when_border(self, palette: &Palette) -> Self;
}

impl BorderExt for gpui::Stateful<gpui::Div> {
    fn when_border(self, palette: &Palette) -> Self {
        if palette.borders {
            self.border_1().border_color(palette.card_stroke)
        } else {
            self
        }
    }
}

/// Union of rounded card rectangles as the host's visible and clickable area.
#[cfg(windows)]
fn set_region(hwnd: windows_sys::Win32::Foundation::HWND, rects: &[super::win32::Rect]) {
    use windows_sys::Win32::Graphics::Gdi::{
        CombineRgn, CreateRectRgn, DeleteObject, RGN_OR, SetWindowRgn,
    };
    unsafe {
        let region = CreateRectRgn(0, 0, 0, 0);
        if region.is_null() {
            return;
        }
        for rect in rects {
            let part = CreateRectRgn(rect.left, rect.top, rect.right, rect.bottom);
            if !part.is_null() {
                CombineRgn(region, region, part, RGN_OR);
                DeleteObject(part);
            }
        }
        // Ownership of the region passes to the system.
        SetWindowRgn(hwnd, region, 0);
    }
}

/// Bottom-right host rectangle on the primary monitor, left of the popup
/// when it is open so the cards never cover it.
#[cfg(windows)]
fn host_rect(popup_left: Option<i32>) -> super::win32::Rect {
    let monitor = super::win32::monitor_for_point(0, 0);
    let scale = monitor.scale();
    let margin = crate::popup::EDGE_MARGIN_PX - (EDGE * scale as f32).round() as i32;
    let width = (f64::from(CARD_WIDTH + EDGE * 2.0) * scale).round() as i32;
    let height = ((f64::from(HOST_HEIGHT) * scale).round() as i32).min(monitor.work.height());
    let right = popup_left
        .filter(|left| *left > monitor.work.left + width && *left <= monitor.work.right)
        .map_or(monitor.work.right - margin, |left| left - margin.max(0));
    let bottom = monitor.work.bottom - margin;
    super::win32::Rect {
        left: right - width,
        top: bottom - height,
        right,
        bottom,
    }
}

/// Shows `notification`, creating the host on first use. Runs on the GPUI
/// thread from the command loop, outside any window borrow.
pub(super) fn post(notification: Notification, popup: &WindowHandle<PopupRoot>, cx: &mut AsyncApp) {
    let handle = match HOST.with(|slot| *slot.borrow()) {
        Some(handle) => handle,
        None => {
            let Some(handle) = open_host(*popup, cx) else {
                return;
            };
            HOST.with(|slot| *slot.borrow_mut() = Some(handle));
            handle
        }
    };
    let appear = handle
        .update(cx, |host, window, cx| host.push(notification, window, cx))
        .unwrap_or(false);
    #[cfg(windows)]
    if appear {
        let popup_left = popup
            .update(cx, |root, _, _| root.capsule_screen_left())
            .ok()
            .flatten();
        let hwnd = handle.update(cx, |host, _, _| host.hwnd).ok().flatten();
        if let Some(hwnd) = hwnd {
            let rect = host_rect(popup_left);
            super::win32::place(hwnd, rect);
            // A DPI change during placement makes GPUI apply Windows'
            // suggested rect; place again with the final geometry.
            if super::win32::window_rect(hwnd) != rect {
                super::win32::place(hwnd, rect);
            }
            let _ = handle.update(cx, |host, window, cx| {
                host.last_region.clear();
                window.refresh();
                cx.notify();
            });
            super::win32::show(hwnd);
        }
    }
    #[cfg(not(windows))]
    let _ = appear;
}

fn open_host(popup: WindowHandle<PopupRoot>, cx: &mut AsyncApp) -> Option<WindowHandle<ToastHost>> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(CARD_WIDTH + EDGE * 2.0), px(HOST_HEIGHT)),
        })),
        titlebar: None,
        focus: false,
        show: false,
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id: None,
        window_background: WindowBackgroundAppearance::Transparent,
        app_id: None,
        window_min_size: None,
        window_decorations: None,
        tabbing_identifier: None,
    };
    let handle = match cx.update(|cx| {
        cx.open_window(options, |window, cx| {
            cx.new(|cx| ToastHost::new(popup, window, cx))
        })
    }) {
        Ok(Ok(handle)) => handle,
        Ok(Err(error)) | Err(error) => {
            eprintln!("could not create the notification window: {error:#}");
            return None;
        }
    };
    #[cfg(windows)]
    {
        let hwnd = handle
            .update(cx, |_, window, _| super::win32::hwnd_from_window(window))
            .ok()
            .flatten();
        if let Some(hwnd) = hwnd {
            super::win32::configure_toast(hwnd);
            super::win32::set_region(hwnd, None, 0);
            let _ = handle.update(cx, |host, _, _| host.hwnd = Some(hwnd));
        }
    }
    Some(handle)
}
