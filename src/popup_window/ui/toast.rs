//! Animated in-app notifications.
//!
//! One transparent, topmost, never-activated GPUI window in the bottom-right
//! corner of the primary monitor's work area stacks the cards. Its region is
//! clipped to the cards, so the rest of the host never swallows clicks. The
//! newest card enters at the bottom and pushes older ones up; each leaves
//! after its lifetime, which hovering pauses.
//!
//! A card is a small popup: the same surface, type, buttons and quota cards,
//! so it reads as part of the app rather than a separate widget.

use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, AppContext, AsyncApp, Bounds, ClickEvent, Context, Div, Hsla, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window,
    WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions, div, point,
    px, relative, size,
};

use crate::{
    limits::LimitWindow,
    notifications::{LimitAlert, LimitFocus, Notification, NotificationKind},
    popup_window::{
        capitalize_plan_name, format_reset_in,
        model::{interval_tick_count, limit_card_presentation, limit_labels},
        navigation::{PopupSection, popup_sections},
    },
};

use super::{
    PopupRoot,
    components::{self, caption, nowrap},
    fx::{self, Fx},
    theme::{HslaExt, Palette, rgb8, rgba8},
};

const CARD_WIDTH: f32 = 360.0;
/// Room around the cards for their shadow.
const EDGE: f32 = 16.0;
const GAP: f32 = 8.0;
const HOST_HEIGHT: f32 = 620.0;
const MAX_CARDS: usize = 4;
/// Inner padding, as on popup pages.
const PADDING: f32 = 12.0;
/// Every card has a 1 px border, transparent when borders are off.
const BORDER: f32 = 1.0;
const GLYPH: f32 = 16.0;
/// Glyph inset and gap match the popup heading (4 px inset, 4 px margin plus
/// 4 px gap after the mark), so the title lines up with the provider name.
const GLYPH_GAP: f32 = 8.0;
const INSET: f32 = 4.0;
/// Room right of the text for the close button.
const CLOSE_ROOM: f32 = 22.0;
/// Popup body text: 14 px on a 20 px line.
const LINE: f32 = 20.0;
const BODY_GAP: f32 = 2.0;
const BODY_MAX_LINES: usize = 4;
const BUTTON_HEIGHT: f32 = 28.0;
/// Space above the buttons and above the quota section.
const SECTION_GAP: f32 = 12.0;
/// Popup limit cards: the regular one and the compact single row.
const LIMIT_CARD: f32 = 82.0;
const COMPACT_LIMIT_CARD: f32 = 44.0;
const ENTER: Duration = Duration::from_millis(360);
const EXIT: Duration = Duration::from_millis(220);
const TICK: Duration = Duration::from_millis(100);
/// Once the card is in, the bar of the window the notification is about
/// fills; the other quotas stand still so it is clear which one it was.
const FILL_DELAY: Duration = Duration::from_millis(200);
const FILL: Duration = Duration::from_millis(1100);

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
    /// Kept until closed by hand, e.g. after opening the release notes.
    pinned: bool,
    leaving: Option<Instant>,
}

impl Card {
    fn exit_progress(&self, now: Instant) -> f32 {
        self.leaving.map_or(0.0, |since| {
            progress(now.saturating_duration_since(since), EXIT)
        })
    }
}

/// Popup display settings the quota cards follow.
#[derive(Clone, Copy)]
struct DisplayPrefs {
    /// Percentages show the used share instead of what is left.
    used: bool,
    colored_icons: bool,
    compact: bool,
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

/// Fluent glyph and its color for each kind.
fn kind_style(kind: NotificationKind, palette: &Palette) -> (&'static str, Hsla) {
    match kind {
        NotificationKind::Info => ("fluent-info", palette.accent),
        NotificationKind::Success => (
            "fluent-checkmark-circle",
            rgb8(if palette.dark {
                (0x6C, 0xCB, 0x5F)
            } else {
                (0x0F, 0x7B, 0x0F)
            }),
        ),
        NotificationKind::Reset => ("fluent-refresh", palette.accent),
        NotificationKind::Warning => ("fluent-warning", palette.caution),
        NotificationKind::Error => ("fluent-error-circle", palette.critical),
        NotificationKind::Update => ("fluent-arrow-download", palette.accent),
    }
}

/// A window that came back refills from empty.
fn refills(kind: NotificationKind) -> bool {
    matches!(kind, NotificationKind::Reset | NotificationKind::Success)
}

/// `(title, window, focused)` of every quota card, in popup order.
fn quota_rows(alert: &LimitAlert) -> Vec<(String, &LimitWindow, bool)> {
    let kind = alert.provider.kind();
    let limits = &alert.limits;
    let (monthly, five_hour, secondary) = limit_labels(kind);
    let secondary = limits
        .secondary_limit_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(secondary);
    popup_sections(kind, limits, false)
        .into_iter()
        .filter_map(|section| {
            let (title, window, focus) = match section {
                PopupSection::Monthly => (monthly, &limits.secondary, LimitFocus::Secondary),
                PopupSection::FiveHour => (five_hour, &limits.primary, LimitFocus::Primary),
                PopupSection::Weekly => (secondary, &limits.secondary, LimitFocus::Secondary),
                _ => return None,
            };
            Some((title.to_uppercase(), window, focus == alert.focus))
        })
        .collect()
}

/// Wrapped line count of `body` at the card's text width, as the text
/// element will lay it out.
fn body_lines(window: &Window, body: &str, width: f32, family: &SharedString) -> usize {
    let run = gpui::TextRun {
        len: body.len(),
        font: gpui::font(family.clone()),
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_text(
            SharedString::from(body.to_owned()),
            px(14.0),
            &[run],
            Some(px(width)),
            None,
        )
        .map(|lines| {
            lines
                .iter()
                .map(|line| line.wrap_boundaries().len() + 1)
                .sum::<usize>()
        })
        .unwrap_or(1)
        .clamp(1, BODY_MAX_LINES)
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

    fn prefs(&self, cx: &gpui::App) -> DisplayPrefs {
        self.popup
            .read(cx)
            .map(|root| DisplayPrefs {
                used: root.ui.show_used_percentage,
                colored_icons: root.ui.use_colored_provider_icons,
                compact: root.ui.compact_usage_cards,
            })
            .unwrap_or(DisplayPrefs {
                used: false,
                colored_icons: true,
                compact: true,
            })
    }

    /// Card height for `notification`, from the same metrics the card is
    /// laid out with. Cards have a fixed height so the stack can glide
    /// without waiting for layout.
    fn measure(
        &self,
        notification: &Notification,
        window: &Window,
        palette: &Palette,
        prefs: DisplayPrefs,
    ) -> f32 {
        let mut height = BORDER * 2.0 + PADDING * 2.0 + LINE;
        let alert = notification.limits.as_deref();
        if alert.is_none() && !notification.body.trim().is_empty() {
            let width =
                CARD_WIDTH - BORDER * 2.0 - PADDING * 2.0 - INSET - GLYPH - GLYPH_GAP - CLOSE_ROOM;
            let lines = body_lines(window, &notification.body, width, &palette.font_family);
            height += BODY_GAP + lines as f32 * LINE;
        }
        if !notification.actions.is_empty() {
            height += SECTION_GAP + BUTTON_HEIGHT;
        }
        if let Some(alert) = alert {
            let card = if prefs.compact {
                COMPACT_LIMIT_CARD
            } else {
                LIMIT_CARD
            };
            let rows = quota_rows(alert).len() as f32;
            height += SECTION_GAP + LINE + rows * (palette.card_gap + card);
        }
        height
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
        let palette = self.palette(window, cx);
        let height = self.measure(&notification, window, &palette, self.prefs(cx));
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
            pinned: false,
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

    fn pin(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(card) = self.cards.iter_mut().find(|card| card.id == id) {
            card.pinned = true;
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
            let entered = !animate || now.saturating_duration_since(card.born) >= ENTER;
            if card.leaving.is_none() && !card.hovered && !card.pinned && entered {
                card.shown += delta;
                if card.shown >= card.notification.lifetime() {
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

/// Wakes the host while cards are up: lifetimes advance even when no
/// animation frame is pending. Hides the host after the last card.
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
        let prefs = self.prefs(cx);

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
            frames.push(y);
        }
        frames.reverse();

        let mut animating = false;
        let mut elements: Vec<AnyElement> = Vec::with_capacity(self.cards.len());
        #[cfg(windows)]
        let mut region = Vec::with_capacity(self.cards.len());
        let scale = window.scale_factor();
        for (card, y) in self.cards.iter().zip(frames) {
            let age = now.saturating_duration_since(card.born);
            let (enter, exit) = if animate {
                (progress(age, ENTER), card.exit_progress(now))
            } else {
                (1.0, 0.0)
            };
            let filling = card.notification.limits.is_some() && age < FILL_DELAY + FILL;
            if animate && (enter < 1.0 || card.leaving.is_some() || filling) {
                animating = true;
            }
            let enter_eased = fx::ease_out_cubic(enter);
            let exit_eased = fx::ease_out_cubic(exit);
            let slide = (1.0 - enter_eased) * (CARD_WIDTH * 0.3) + exit_eased * (CARD_WIDTH * 0.25);
            let alpha = (enter * 2.0).min(1.0) * (1.0 - exit_eased);
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
            let hover = self
                .fx
                .toggle(fx::key(("toast-hover", card.id)), card.hovered, fx::FASTER);
            elements.push(self.card(
                card,
                CardFrame {
                    x,
                    y,
                    alpha,
                    hover,
                    age: animate.then_some(age),
                },
                prefs,
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
            .text_color(palette.text_primary)
            .children(elements)
    }
}

/// Per-frame placement and animation state of one card.
struct CardFrame {
    x: f32,
    y: f32,
    alpha: f32,
    hover: f32,
    /// Time since the card appeared; `None` when motion is off.
    age: Option<Duration>,
}

impl ToastHost {
    fn card(
        &self,
        card: &Card,
        frame: CardFrame,
        prefs: DisplayPrefs,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = card.id;
        let notification = &card.notification;
        let (glyph, tint) = kind_style(notification.kind, palette);
        let alert = notification.limits.as_deref();

        let mut text = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .pr(px(CLOSE_ROOM))
            .child(nowrap(components::body_strong(
                notification.title.clone(),
                palette.text_primary,
            )));
        if alert.is_none() && !notification.body.trim().is_empty() {
            text = text.child(
                components::body(notification.body.clone(), palette.text_secondary)
                    .mt(px(BODY_GAP))
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
                        self.button(id, index, label, action.clone(), palette, cx)
                    });
            text = text.child(
                div()
                    .mt(px(SECTION_GAP))
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .children(buttons),
            );
        }

        let close = div()
            .id(SharedString::from(format!("toast-{id}-close")))
            .absolute()
            .top(px(PADDING - 2.0))
            .right(px(PADDING - 6.0))
            .size(px(24.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .hover(|style| style.bg(palette.subtle_fill))
            .child(components::icon(
                "fluent-dismiss",
                12.0,
                palette
                    .chrome_icon
                    .mix(palette.chrome_icon_hover, frame.hover),
            ))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                cx.stop_propagation();
                this.dismiss(id, cx);
            }));

        let mut content = div().flex().flex_col().child(
            div()
                .flex()
                .flex_row()
                .items_start()
                .pl(px(INSET))
                .gap(px(GLYPH_GAP))
                // The glyph sits centered on the title line.
                .child(
                    div()
                        .pt(px((LINE - GLYPH) / 2.0))
                        .child(components::icon(glyph, GLYPH, tint)),
                )
                .child(text),
        );
        if let Some(alert) = alert {
            content = content.child(quota_section(
                alert,
                notification.kind,
                frame.age,
                prefs,
                palette,
            ));
        }

        div()
            .id(SharedString::from(format!("toast-{id}")))
            .absolute()
            .left(px(frame.x))
            .top(px(frame.y))
            .w(px(CARD_WIDTH))
            .h(px(card.height))
            .p(px(PADDING))
            .opacity(frame.alpha)
            .rounded(px(crate::popup::corner_radius_dip() as f32))
            .bg(palette.solid_background)
            // The popup capsule's hairline.
            .border_1()
            .border_color(match (palette.borders, palette.dark) {
                (false, _) => gpui::transparent_black(),
                (true, true) => rgba8(255, 255, 255, 0x14),
                (true, false) => rgba8(0, 0, 0, 0x12),
            })
            .shadow(vec![gpui::BoxShadow {
                color: gpui::black().alpha(if palette.dark { 0.3 } else { 0.14 }),
                offset: point(px(0.0), px(4.0)),
                blur_radius: px(12.0),
                spread_radius: px(0.0),
            }])
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.set_hovered(id, *hovered, cx);
            }))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.dismiss(id, cx);
            }))
            .child(content)
            .child(close)
            .into_any_element()
    }

    /// Popup buttons: the first is the accent one, the rest standard.
    fn button(
        &self,
        id: u64,
        index: usize,
        label: &str,
        action: crate::notifications::NotificationAction,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (background, hover, foreground) = if index == 0 {
            // WinUI accent buttons fade to 90% on hover.
            (
                palette.accent,
                palette.accent.opacity(0.9),
                palette.text_on_accent,
            )
        } else {
            (
                palette.control_fill,
                palette.subtle_fill,
                palette.text_primary,
            )
        };
        let mut button = div()
            .id(SharedString::from(format!("toast-{id}-action-{index}")))
            .h(px(BUTTON_HEIGHT))
            .px(px(12.0))
            .flex()
            .items_center()
            .rounded(px(4.0))
            .bg(background)
            .hover(move |style| style.bg(hover));
        if index > 0 && palette.borders {
            button = button.border_1().border_color(palette.card_stroke);
        }
        button
            .child(components::body_strong(label.to_owned(), foreground))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                cx.stop_propagation();
                action.run();
                if action.pins_card() {
                    this.pin(id, cx);
                } else {
                    this.dismiss(id, cx);
                }
            }))
            .into_any_element()
    }
}

/// The provider heading and its quota cards, as on the popup's home page.
fn quota_section(
    alert: &LimitAlert,
    kind: NotificationKind,
    age: Option<Duration>,
    prefs: DisplayPrefs,
    palette: &Palette,
) -> Div {
    div()
        .mt(px(SECTION_GAP))
        .flex()
        .flex_col()
        .gap(px(palette.card_gap))
        .child(provider_heading(alert, prefs, palette))
        .children(
            quota_rows(alert)
                .into_iter()
                .map(|(title, window, focused)| {
                    limit_card(title, window, focused, kind, age, prefs, palette)
                }),
        )
}

/// Driver mark, name and plan, with the account on the right.
fn provider_heading(alert: &LimitAlert, prefs: DisplayPrefs, palette: &Palette) -> Div {
    let provider = alert.provider;
    let driver = provider.kind();
    let badge = provider.badge();
    let mut title = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .min_w_0()
        .child(
            components::provider_mark(
                crate::provider_registry::icon(driver),
                16.0,
                palette.provider_icon(driver, prefs.colored_icons),
                badge.as_ref(),
                palette,
            )
            .mr(px(if badge.is_some() { 10.0 } else { 4.0 })),
        )
        .child(nowrap(components::body_strong(
            driver.display_name(),
            palette.text_secondary,
        )));
    if let Some(plan) = alert
        .limits
        .plan_type
        .as_deref()
        .filter(|plan| !plan.trim().is_empty())
    {
        title = title.child(nowrap(components::body(
            capitalize_plan_name(plan),
            palette.text_tertiary,
        )));
    }
    let account = Some(provider.display_name())
        .filter(|name| name != driver.display_name())
        .or_else(|| alert.limits.account_name.clone());
    div()
        .h(px(LINE))
        .px(px(INSET))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(components::SPLIT_GAP))
        .child(div().flex_1().min_w_0().child(title))
        .children(account.map(|account| {
            div()
                .max_w(relative(0.5))
                .child(nowrap(components::body(account, palette.text_tertiary)))
        }))
}

/// A popup limit card. The one the notification is about fills in; a
/// refilled window starts empty (0% left, 100% used).
fn limit_card(
    title: String,
    window: &LimitWindow,
    focused: bool,
    kind: NotificationKind,
    age: Option<Duration>,
    prefs: DisplayPrefs,
    palette: &Palette,
) -> Div {
    let (label, target, show_reset, _) = limit_card_presentation(window, prefs.used, false);
    let known = window.used_percent.is_some();
    let start = if focused && refills(kind) && prefs.used {
        100.0
    } else {
        0.0
    };
    let fill = match age {
        Some(age) if focused => progress(age.saturating_sub(FILL_DELAY), FILL),
        _ => 1.0,
    };
    let value = if known {
        start + (target as f32 - start) * fx::ease_out_cubic(fill)
    } else {
        0.0
    };
    // The number counts along with the bar.
    let label = if known && fill < 1.0 {
        crate::i18n::format(
            if prefs.used {
                "quota-percent-used"
            } else {
                "quota-percent-left"
            },
            &[("value", (value.round() as i32).to_string())],
        )
    } else {
        label
    };
    let color = if focused && kind == NotificationKind::Warning {
        palette.caution
    } else {
        palette.accent_text
    };
    let usage = nowrap(components::body_strong(label, color));
    let reset = show_reset.then(|| match window.resets_at {
        Some(at) => {
            components::icon_status(components::RESET_ICON, format_reset_in(Some(at)), palette)
        }
        None => components::card_metadata(crate::i18n::tr("session-not-started"), palette),
    });
    let title = nowrap(caption(title, palette.text_secondary));
    let ticks = interval_tick_count(window);

    if prefs.compact {
        let mut card = components::card(palette)
            .relative()
            .overflow_hidden()
            .h(px(COMPACT_LIMIT_CARD));
        for layer in components::compact_progress_layers(value, None, ticks, color, palette) {
            card = card.child(layer);
        }
        return card.child(
            div()
                .relative()
                .size_full()
                .px(px(12.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(10.0))
                .child(div().flex_1().min_w_0().child(title))
                .child(usage)
                .children(reset),
        );
    }
    components::card(palette)
        .overflow_hidden()
        .h(px(LIMIT_CARD))
        .px(px(12.0))
        .flex()
        .flex_col()
        .justify_center()
        .gap(px(8.0))
        .child(title)
        .child(components::progress_track(
            value, None, ticks, color, palette,
        ))
        .child(match reset {
            Some(reset) => components::split_row(usage, reset),
            None => div().child(usage),
        })
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
        window_max_size: None,
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
