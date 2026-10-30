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
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, AppContext, AsyncApp, Bounds, ClickEvent, Context, Div, Hsla, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled,
    StyledImage as _, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind,
    WindowOptions, div, point, prelude::FluentBuilder as _, px, relative, size,
};

use crate::{
    limits::LimitWindow,
    notifications::{LimitAlert, LimitFocus, Notification, NotificationKind, Progress},
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
/// Card shadow. GPUI's blur radius is the Gaussian sigma; past two sigmas
/// the shadow is invisible, so the host region keeps that much around cards.
const SHADOW_BLUR: f32 = 12.0;
const SHADOW_OFFSET: f32 = 4.0;
const SHADOW_REACH: f32 = SHADOW_BLUR * 2.0;
/// Room around the cards for their shadow.
const EDGE: f32 = SHADOW_REACH + SHADOW_OFFSET;
const GAP: f32 = 8.0;
/// Tall enough for a release summary with its hero image and button.
const HOST_HEIGHT: f32 = 900.0;
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
/// Popup limit card: the compact single row.
const LIMIT_CARD: f32 = 44.0;
const ENTER: Duration = Duration::from_millis(360);
const EXIT: Duration = Duration::from_millis(220);
const TICK: Duration = Duration::from_millis(100);
/// Once the card is in, the bar of the window the notification is about
/// fills; the other quotas stand still so it is clear which one it was.
const FILL_DELAY: Duration = Duration::from_millis(200);
const FILL: Duration = Duration::from_millis(1100);
/// Space between the text and a progress bar.
const PROGRESS_GAP: f32 = 10.0;
/// One sweep of an indeterminate bar, and the share of the track it covers.
const SWEEP: Duration = Duration::from_millis(1600);
const SWEEP_WIDTH: f32 = 0.35;
/// A taller release summary scrolls inside the card.
const SUMMARY_MAX_HEIGHT: f32 = 360.0;
/// The scrolling summary never shrinks below this on a short screen.
const MIN_SUMMARY_HEIGHT: f32 = 80.0;
/// Release summaries use the card's full inner width.
const SUMMARY_WIDTH: f32 = CARD_WIDTH - BORDER * 2.0 - PADDING * 2.0 - INSET;
/// A summary's first image spans the top of the card, edge to edge.
const HERO_WIDTH: f32 = CARD_WIDTH - BORDER * 2.0;
const HERO_HEIGHT: f32 = 250.0;
/// Travel before a press becomes a swipe instead of a click.
const SWIPE_SLOP: f32 = 6.0;
/// A swipe this far right, or released moving right this fast (px/ms),
/// closes the card.
const SWIPE_DISMISS: f32 = CARD_WIDTH * 0.3;
const SWIPE_FLING: f32 = 0.6;
/// Leftward drags only give a little: cards leave to the right.
const SWIPE_LEFT_RESISTANCE: f32 = 0.2;

thread_local! {
    static HOST: RefCell<Option<WindowHandle<ToastHost>>> = const { RefCell::new(None) };
}

struct Card {
    id: u64,
    notification: Notification,
    height: f32,
    /// Height of the release summary's scroll area, when it has one.
    summary_height: f32,
    /// The summary's first image, cropped for the top of the card, and the
    /// index of the block it came from.
    hero: Option<(usize, Arc<gpui::RenderImage>)>,
    born: Instant,
    /// Lifetime used so far; it does not grow while hovered.
    shown: Duration,
    last_tick: Instant,
    hovered: bool,
    /// The pointer (mouse, pen or touch) holding the card for a swipe.
    drag: Option<Drag>,
    /// Horizontal offset the card rests at: 0, or where a swipe let go.
    swiped: f32,
    /// The last press became a swipe; its click must not close the card.
    swallow_click: bool,
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
}

pub(crate) struct ToastHost {
    popup: WindowHandle<PopupRoot>,
    default_font: SharedString,
    cards: Vec<Card>,
    next_id: u64,
    /// Keys of progress cards closed by hand; their further progress stays
    /// hidden until the operation reports its result.
    dismissed_keys: Vec<&'static str>,
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

#[derive(Clone, Copy)]
struct Drag {
    /// Pointer x when pressed, in host coordinates.
    start: f32,
    /// Current offset, after resistance.
    offset: f32,
    moved: bool,
    /// Latest pointer x and when it was seen, for the release speed.
    last: (f32, Instant),
    /// Pointer speed in px/ms, positive to the right.
    velocity: f32,
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
            dismissed_keys: Vec::new(),
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
            })
            .unwrap_or(DisplayPrefs {
                used: false,
                colored_icons: true,
            })
    }

    /// Card height for `notification`, and the height of its summary area,
    /// from the same metrics the card is laid out with. Cards have a fixed
    /// height so the stack can glide without waiting for layout.
    fn measure(
        &self,
        notification: &Notification,
        hero: Option<usize>,
        window: &Window,
        palette: &Palette,
    ) -> (f32, f32) {
        let mut summary_height = 0.0;
        let mut height = BORDER * 2.0 + PADDING * 2.0 + LINE;
        if hero.is_some() {
            height += HERO_HEIGHT;
        }
        let alert = notification.limits.as_deref();
        if alert.is_none() && !notification.body.trim().is_empty() {
            let width =
                CARD_WIDTH - BORDER * 2.0 - PADDING * 2.0 - INSET - GLYPH - GLYPH_GAP - CLOSE_ROOM;
            let lines = body_lines(window, &notification.body, width, &palette.font_family);
            height += BODY_GAP + lines as f32 * LINE;
        }
        if let Some(summary) = notification.summary.as_deref() {
            let natural = super::toast_summary::measure(
                window,
                summary,
                SUMMARY_WIDTH,
                &palette.font_family,
                hero,
            );
            summary_height = natural.min(SUMMARY_MAX_HEIGHT);
            height += SECTION_GAP + summary_height;
        }
        if !notification.actions.is_empty() {
            height += SECTION_GAP + BUTTON_HEIGHT;
        }
        // The host is bottom-anchored and may be shorter than HOST_HEIGHT on
        // a small work area: shrink the scrolling summary so the card never
        // grows past the top edge, where it would be cropped.
        let room = HOST_HEIGHT.min(f32::from(window.viewport_size().height)) - EDGE * 2.0;
        if summary_height > 0.0 && height > room {
            let shrunk = (summary_height - (height - room)).max(MIN_SUMMARY_HEIGHT);
            height -= summary_height - shrunk;
            summary_height = shrunk;
        }
        if notification.progress.is_some() {
            height += PROGRESS_GAP + components::PROGRESS_TRACK_HEIGHT;
        }
        if let Some(alert) = alert {
            let rows = quota_rows(alert).len() as f32;
            height += SECTION_GAP + LINE + rows * (palette.card_gap + LIMIT_CARD);
        }
        (height, summary_height)
    }

    /// Adds a card. Returns the native work to do outside this borrow when
    /// the host has to appear.
    fn push(
        &mut self,
        notification: Notification,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let palette = self.palette(window, cx);
        let hero = notification
            .summary
            .as_deref()
            .and_then(|summary| super::toast_summary::hero(summary, HERO_WIDTH, HERO_HEIGHT));
        let (height, summary_height) = self.measure(
            &notification,
            hero.as_ref().map(|(index, _)| *index),
            window,
            &palette,
        );
        let now = Instant::now();
        if let Some(key) = notification.key {
            let finished = notification.progress.is_none();
            // The same operation moves on: update its card in place.
            if let Some(card) = self
                .cards
                .iter_mut()
                .find(|card| card.leaving.is_none() && card.notification.key == Some(key))
            {
                // A finished operation sounds once; a summary filling in an
                // already finished card stays quiet.
                if finished && card.notification.progress.is_some() {
                    crate::notifications::play_sound(&notification);
                    // The result gets its full lifetime from now.
                    card.shown = Duration::ZERO;
                    card.last_tick = now;
                }
                // A summary is read at leisure: it stays until closed.
                card.pinned |= notification.stays_until_closed();
                card.notification = notification;
                card.height = height;
                card.summary_height = summary_height;
                card.hero = hero;
                cx.notify();
                return false;
            }
            if !finished && self.dismissed_keys.contains(&key) {
                return false;
            }
            self.dismissed_keys.retain(|dismissed| *dismissed != key);
        }
        // Progress steps are silent; the result plays the sound.
        if notification.progress.is_none() {
            crate::notifications::play_sound(&notification);
        }
        // Identical notifications (e.g. a burst of the same error) replace
        // the visible one instead of piling up.
        self.cards.retain(|card| {
            card.leaving.is_some()
                || card.notification.title != notification.title
                || card.notification.body != notification.body
        });
        self.next_id += 1;
        let pinned = notification.stays_until_closed();
        self.cards.push(Card {
            id: self.next_id,
            notification,
            height,
            summary_height,
            hero,
            born: now,
            shown: Duration::ZERO,
            last_tick: now,
            hovered: false,
            drag: None,
            swiped: 0.0,
            swallow_click: false,
            pinned,
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
            if let Some(key) = card.notification.key
                && card.notification.progress.is_some()
                && !self.dismissed_keys.contains(&key)
            {
                self.dismissed_keys.push(key);
            }
            cx.notify();
        }
    }

    fn pin(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(card) = self.cards.iter_mut().find(|card| card.id == id) {
            card.pinned = true;
            cx.notify();
        }
    }

    fn begin_drag(&mut self, id: u64, x: f32, cx: &mut Context<Self>) {
        if let Some(card) = self
            .cards
            .iter_mut()
            .find(|card| card.id == id && card.leaving.is_none())
        {
            card.swallow_click = false;
            card.drag = Some(Drag {
                start: x,
                offset: 0.0,
                moved: false,
                last: (x, Instant::now()),
                velocity: 0.0,
            });
            cx.notify();
        }
    }

    /// Follows the pointer holding a card.
    fn drag_to(&mut self, x: f32, cx: &mut Context<Self>) {
        let Some(drag) = self.cards.iter_mut().find_map(|card| card.drag.as_mut()) else {
            return;
        };
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(drag.last.1).as_secs_f32() * 1000.0;
        if elapsed > 0.0 {
            drag.velocity = (x - drag.last.0) / elapsed;
        }
        drag.last = (x, now);
        let delta = x - drag.start;
        drag.moved |= delta.abs() > SWIPE_SLOP;
        drag.offset = if delta < 0.0 {
            delta * SWIPE_LEFT_RESISTANCE
        } else {
            delta
        };
        cx.notify();
    }

    /// Lets go of the held card: far or fast enough closes it, otherwise it
    /// springs back.
    fn end_drag(&mut self, cx: &mut Context<Self>) {
        let Some(card) = self.cards.iter_mut().find(|card| card.drag.is_some()) else {
            return;
        };
        let Some(drag) = card.drag.take() else {
            return;
        };
        card.swallow_click = drag.moved;
        let recent = drag.last.1.elapsed() < Duration::from_millis(80);
        let fling = recent && drag.velocity > SWIPE_FLING;
        if drag.moved && (drag.offset > SWIPE_DISMISS || fling) && card.leaving.is_none() {
            card.swiped = drag.offset.max(0.0);
            card.leaving = Some(Instant::now());
        }
        cx.notify();
    }

    /// A click closes the card, unless the press was a swipe.
    fn click(&mut self, id: u64, cx: &mut Context<Self>) {
        let swiped = self
            .cards
            .iter_mut()
            .find(|card| card.id == id)
            .is_some_and(|card| std::mem::take(&mut card.swallow_click));
        if !swiped {
            self.dismiss(id, cx);
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
            // A card with progress waits for its operation's result.
            let running = card.notification.progress.is_some();
            let held = card.hovered || card.drag.is_some();
            if card.leaving.is_none() && !held && !card.pinned && !running && entered {
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
        // A card that changes content (an operation moving to its result)
        // glides to its new height and the stack above follows.
        let mut bottom = f32::from(window.viewport_size().height) - EDGE;
        let mut frames = Vec::with_capacity(self.cards.len());
        for card in self.cards.iter().rev() {
            let height = self
                .fx
                .value(fx::key(("toast-h", card.id)), card.height, fx::SETTLE);
            let target = bottom - height;
            bottom = target - GAP;
            let y = self
                .fx
                .value(fx::key(("toast-y", card.id)), target, fx::SETTLE);
            frames.push((y, height));
        }
        frames.reverse();

        let mut animating = false;
        let mut elements: Vec<AnyElement> = Vec::with_capacity(self.cards.len());
        #[cfg(windows)]
        let mut region = Vec::with_capacity(self.cards.len());
        let scale = window.scale_factor();
        // The host can reach past the work area; the shadow stops at its
        // bottom so it never makes the taskbar unclickable.
        #[cfg(windows)]
        let floor = f32::from(window.viewport_size().height)
            - (EDGE - crate::popup::EDGE_MARGIN_PX as f32 / scale).max(0.0);
        for (card, (y, height)) in self.cards.iter().zip(frames) {
            let age = now.saturating_duration_since(card.born);
            let (enter, exit) = if animate {
                (progress(age, ENTER), card.exit_progress(now))
            } else {
                (1.0, 0.0)
            };
            let filling = card.notification.limits.is_some() && age < FILL_DELAY + FILL;
            let sweeping = card.notification.progress == Some(Progress::Indeterminate);
            if animate && (enter < 1.0 || card.leaving.is_some() || filling || sweeping) {
                animating = true;
            }
            let enter_eased = fx::ease_out_cubic(enter);
            let exit_eased = fx::ease_out_cubic(exit);
            let slide = (1.0 - enter_eased) * (CARD_WIDTH * 0.3) + exit_eased * (CARD_WIDTH * 0.25);
            // A held card follows the pointer; let go, it springs back or
            // leaves from where it was swiped to.
            let drag_key = fx::key(("toast-drag", card.id));
            let offset = match card.drag {
                Some(drag) => {
                    self.fx.snap(drag_key, drag.offset);
                    drag.offset
                }
                None => self.fx.value(drag_key, card.swiped, fx::NORMAL),
            };
            let fade = 1.0 - (offset.max(0.0) / CARD_WIDTH).min(1.0) * 0.6;
            let alpha = (enter * 2.0).min(1.0) * (1.0 - exit_eased) * fade;
            let x = EDGE + slide + offset;
            #[cfg(windows)]
            if alpha > 0.0 {
                // Card and shadow; the card slides out past the host edge.
                let left = ((x - SHADOW_REACH).max(0.0) * scale).floor() as i32;
                let right = ((EDGE * 2.0 + CARD_WIDTH) * scale).ceil() as i32;
                let bottom = (y + height + SHADOW_OFFSET + SHADOW_REACH).min(floor);
                region.push(super::win32::Rect {
                    left,
                    top: ((y + SHADOW_OFFSET - SHADOW_REACH) * scale).floor() as i32,
                    right: right.max(left),
                    bottom: (bottom * scale).ceil() as i32,
                });
            }
            let hover = self
                .fx
                .toggle(fx::key(("toast-hover", card.id)), card.hovered, fx::FASTER);
            let progress = card.notification.progress.map(|progress| match progress {
                // Keyed by step so a new step starts from its own value
                // instead of draining the previous bar.
                Progress::Fraction(fraction) => BarFrame::Fraction(self.fx.value(
                    fx::key(("toast-progress", card.id, &card.notification.title)),
                    fraction.clamp(0.0, 1.0),
                    fx::NORMAL,
                )),
                Progress::Indeterminate => BarFrame::Sweep(
                    animate.then(|| sweep_cycle(now.saturating_duration_since(card.born))),
                ),
            });
            let summary = if card.notification.summary.is_some() {
                self.fx
                    .appear(fx::key(("toast-summary", card.id)), fx::NORMAL)
            } else {
                0.0
            };
            elements.push(self.card(
                card,
                CardFrame {
                    x,
                    y,
                    height,
                    alpha,
                    hover,
                    age: animate.then_some(age),
                    progress,
                    summary,
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
        // The window captures the pointer while it is pressed, so a swipe is
        // followed even past the card or the host's edge.
        let host = cx.entity().downgrade();
        let swipes = gpui::canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                let moves = host.clone();
                window.on_mouse_event(move |event: &gpui::MouseMoveEvent, phase, _, cx| {
                    if phase != gpui::DispatchPhase::Capture {
                        return;
                    }
                    let _ = moves.update(cx, |this, cx| {
                        if event.pressed_button == Some(gpui::MouseButton::Left) {
                            this.drag_to(f32::from(event.position.x), cx);
                        } else {
                            // Released where no up event reached us.
                            this.end_drag(cx);
                        }
                    });
                });
                let ups = host.clone();
                window.on_mouse_event(move |event: &gpui::MouseUpEvent, phase, _, cx| {
                    if phase == gpui::DispatchPhase::Capture
                        && event.button == gpui::MouseButton::Left
                    {
                        let _ = ups.update(cx, |this, cx| this.end_drag(cx));
                    }
                });
            },
        )
        .absolute()
        .size_full();
        div()
            .size_full()
            .relative()
            .font_family(palette.font_family.clone())
            .text_color(palette.text_primary)
            .children(elements)
            .child(swipes)
    }
}

/// Per-frame placement and animation state of one card.
struct CardFrame {
    x: f32,
    y: f32,
    height: f32,
    alpha: f32,
    hover: f32,
    /// Time since the card appeared; `None` when motion is off.
    age: Option<Duration>,
    progress: Option<BarFrame>,
    /// Opacity of the release summary; it fades in when it fills a card.
    summary: f32,
}

/// Per-frame state of a card's progress bar.
#[derive(Clone, Copy)]
enum BarFrame {
    /// Filled share, gliding toward the latest report.
    Fraction(f32),
    /// Position in the sweep cycle, `0..1`; `None` when motion is off.
    Sweep(Option<f32>),
}

fn sweep_cycle(elapsed: Duration) -> f32 {
    (elapsed.as_secs_f32() / SWEEP.as_secs_f32()).fract()
}

/// A popup progress track; an unknown total sweeps a segment across it.
fn progress_bar(bar: BarFrame, palette: &Palette) -> Div {
    match bar {
        BarFrame::Fraction(fraction) => {
            components::progress_track(fraction * 100.0, None, 0, palette.accent, palette)
        }
        BarFrame::Sweep(cycle) => {
            let radius = components::PROGRESS_TRACK_HEIGHT / 2.0;
            // Without motion the segment rests in the middle of the track.
            let start = cycle.map_or((1.0 - SWEEP_WIDTH) / 2.0, |cycle| {
                fx::cubic_bezier(0.65, 0.0, 0.35, 1.0, f64::from(cycle)) as f32
                    * (1.0 + SWEEP_WIDTH)
                    - SWEEP_WIDTH
            });
            // Clip to the track by hand: GPUI masks are rectangular and would
            // square off the rounded ends.
            let left = start.clamp(0.0, 1.0);
            let width = (start + SWEEP_WIDTH).clamp(0.0, 1.0) - left;
            let mut track = div()
                .relative()
                .w_full()
                .h(px(components::PROGRESS_TRACK_HEIGHT))
                .flex_none()
                .rounded(px(radius))
                .bg(palette.accent.opacity(0.2));
            if width > 0.0 {
                track = track.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(relative(left))
                        .w(relative(width))
                        .rounded(px(radius))
                        .bg(palette.accent),
                );
            }
            track
        }
    }
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
        let summary = notification.summary.as_deref();
        let mut buttons = None;
        if !notification.actions.is_empty() {
            buttons = Some(
                div()
                    .mt(px(SECTION_GAP))
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .children(notification.actions.iter().enumerate().map(
                        |(index, (label, action))| {
                            self.button(id, index, label, action.clone(), palette, cx)
                        },
                    )),
            );
        }
        // With a summary the buttons go under it, at the card's edge.
        if summary.is_none()
            && let Some(buttons) = buttons.take()
        {
            text = text.child(buttons);
        }
        if let Some(bar) = frame.progress {
            text = text.child(progress_bar(bar, palette).mt(px(PROGRESS_GAP)));
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
            // Stays legible over the summary's image.
            .when(card.hero.is_some(), |close| {
                close.bg(palette.solid_background.opacity(0.85))
            })
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

        let radius = crate::popup::corner_radius_dip() as f32;
        let hero = card.hero.as_ref().map(|(_, image)| {
            // Out through the padding to the border, and back in below.
            div()
                .mt(px(-PADDING))
                .mx(px(-PADDING))
                .mb(px(PADDING))
                .w(px(HERO_WIDTH))
                .h(px(HERO_HEIGHT))
                .flex_none()
                .opacity(frame.summary)
                .child(
                    gpui::img(Arc::clone(image))
                        .size_full()
                        .object_fit(gpui::ObjectFit::Fill)
                        .rounded_t(px((radius - BORDER).max(0.0))),
                )
        });
        let mut content = div().flex().flex_col().children(hero).child(
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
        if let Some(summary) = summary {
            let skip = card.hero.as_ref().map(|(index, _)| *index);
            let column = super::toast_summary::render(id, summary, SUMMARY_WIDTH, skip, palette);
            // Fixed at its measured height; a taller summary scrolls.
            content = content.child(
                div()
                    .id(SharedString::from(format!("toast-{id}-summary")))
                    .mt(px(SECTION_GAP))
                    .pl(px(INSET))
                    .h(px(card.summary_height))
                    .flex_none()
                    .overflow_y_scroll()
                    .opacity(frame.summary)
                    .child(column),
            );
            if let Some(buttons) = buttons.take() {
                content = content.child(buttons.pl(px(INSET)));
            }
        }

        div()
            .id(SharedString::from(format!("toast-{id}")))
            .absolute()
            .left(px(frame.x))
            .top(px(frame.y))
            .w(px(CARD_WIDTH))
            .h(px(frame.height))
            .p(px(PADDING))
            .opacity(frame.alpha)
            .rounded(px(crate::popup::corner_radius_dip() as f32))
            .bg(palette.solid_background)
            // The popup capsule's hairline.
            .border_1()
            .border_color(match (palette.window_border(), palette.dark) {
                (false, _) => gpui::transparent_black(),
                (true, true) => rgba8(255, 255, 255, 0x14),
                (true, false) => rgba8(0, 0, 0, 0x12),
            })
            .shadow(vec![gpui::BoxShadow {
                color: gpui::black().alpha(if palette.dark { 0.3 } else { 0.14 }),
                offset: point(px(0.0), px(SHADOW_OFFSET)),
                blur_radius: px(SHADOW_BLUR),
                spread_radius: px(0.0),
            }])
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.set_hovered(id, *hovered, cx);
            }))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    this.begin_drag(id, f32::from(event.position.x), cx);
                }),
            )
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.click(id, cx);
            }))
            // Middle click closes the card, as it closes a browser tab.
            .on_mouse_up(
                gpui::MouseButton::Middle,
                cx.listener(move |this, _: &gpui::MouseUpEvent, _, cx| {
                    cx.stop_propagation();
                    this.dismiss(id, cx);
                }),
            )
            // The summary follows the height glide instead of spilling past it.
            .when(summary.is_some(), |card| card.overflow_hidden())
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

    let mut card = components::card(palette)
        .relative()
        .overflow_hidden()
        .h(px(LIMIT_CARD));
    for layer in components::compact_progress_layers(value, None, ticks, color, palette) {
        card = card.child(layer);
    }
    card.child(
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
    )
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
