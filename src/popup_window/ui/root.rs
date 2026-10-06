//! The popup root view: lifecycle, geometry, pager and shell chrome.

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Context, Div, ElementId, Entity, FontWeight,
    InteractiveElement, IntoElement, ParentElement, Pixels, Point, Render, ScrollWheelEvent,
    SharedString, StatefulInteractiveElement, Styled, Subscription, Task, WeakEntity, Window,
    canvas, div, point, px,
};

use super::{
    components::{self, Severity},
    fx::{self, Fx, Spring},
    theme::{self, Palette},
    tooltip::{TipContent, TipRequest},
};
use crate::popup_window::{
    model::{self, *},
    *,
};
use crate::{
    reset_feed::ForcedReset,
    usage_overview::{BreakdownMode, OverviewMetric, OverviewRange, OverviewSnapshot},
};

/// Fluent motion tokens used by Windows edge panels.
const OPEN_ANIMATION: Duration = Duration::from_millis(250);
const CLOSE_ANIMATION: Duration = Duration::from_millis(167);
/// Pinned profile switcher between the page and the footer.
pub(super) const PROFILE_STRIP_HEIGHT: f32 = 46.0;
/// One physical pixel of chrome around the content keeps anti-aliased
/// corners inside the region's aliased edge.
const CHROME_INSET: f32 = 1.0;
const PAGE_PADDING: f32 = 16.0;
const PAGE_SPACING: f32 = 6.0;
/// Smooth wheel scrolling: one notch glides over this duration.
const SCROLL_GLIDE: Duration = Duration::from_millis(140);

/// Round the independent MaxContent measurement once to physical pixels.
fn measured_page_height(height: f32, scale: f32) -> f32 {
    let scale = scale.max(0.5);
    ((height * scale - 0.01).ceil() / scale).max(PAGE_PADDING * 2.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Phase {
    Hidden,
    Opening(Instant),
    Open,
    /// Closing from the given starting slide offset (DIP).
    Closing(Instant, f32),
}

/// Native host state. All geometry is in DIP unless suffixed `_px`.
pub(super) struct Host {
    #[cfg(windows)]
    pub(super) hwnd: Option<windows_sys::Win32::Foundation::HWND>,
    pub(super) monitor: Option<super::Monitor>,
    pub(super) phase: Phase,
    pub(super) height: Spring,
    pub(super) width: Spring,
    pub(super) offset: f32,
    pub(super) last_region: Option<(i32, i32, i32, i32, i32)>,
    pub(super) scale: f32,
}

impl Host {
    fn new() -> Self {
        Self {
            #[cfg(windows)]
            hwnd: None,
            monitor: None,
            phase: Phase::Hidden,
            height: Spring::at(f64::from(INITIAL_HEIGHT)),
            width: Spring::at(f64::from(crate::popup::POPUP_WIDTH)),
            offset: 0.0,
            last_region: None,
            scale: 1.0,
        }
    }

    pub(super) fn visible(&self) -> bool {
        !matches!(self.phase, Phase::Hidden)
    }

    /// Wide two-column layout fits on the monitor that owns the popup.
    pub(super) fn wide_available(&self) -> bool {
        self.monitor.is_none_or(|monitor| {
            f64::from(monitor.bounds.width() - crate::popup::EDGE_MARGIN_PX * 2) / monitor.scale()
                >= f64::from(crate::popup::POPUP_WIDE_WIDTH)
        })
    }

    pub(super) fn max_height(&self) -> f32 {
        self.monitor.map_or(4096.0, |monitor| {
            #[cfg(windows)]
            {
                (f64::from(super::host_rect(monitor).height()) / monitor.scale()) as f32
            }
            #[cfg(not(windows))]
            {
                (f64::from(monitor.bounds.height()) / monitor.scale()
                    * crate::popup::POPUP_SCREEN_HEIGHT_FRACTION) as f32
            }
        })
    }

    pub(super) fn margin(&self) -> f32 {
        crate::popup::EDGE_MARGIN_PX as f32 / self.scale.max(0.5)
    }
}

/// Baseline height before the first content measurement.
const INITIAL_HEIGHT: f32 = 300.0;

/// Last measured Home block and column bounds (window coordinates).
#[derive(Default)]
pub(super) struct WidgetLayout {
    pub(super) widgets: HashMap<PopupWidgetKind, Bounds<Pixels>>,
    pub(super) columns: [Option<Bounds<Pixels>>; 2],
}

#[derive(Clone, Default)]
pub(super) struct PageMetrics {
    pub(super) content_height: Rc<Cell<f32>>,
    pub(super) scroll_target: f32,
}

pub(super) struct SnapshotCache {
    pub(super) key: Option<String>,
    pub(super) pending: Option<String>,
    pub(super) value: Arc<OverviewSnapshot>,
    pub(super) task: Option<Task<()>>,
}

impl Default for SnapshotCache {
    fn default() -> Self {
        Self {
            key: None,
            pending: None,
            value: Arc::new(OverviewSnapshot::default()),
            task: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum SnapshotSlot {
    Spend,
    Overview,
}

pub(crate) struct PopupRoot {
    pub(super) state: Arc<AppState>,
    pub(super) ui_dispatcher: windows_reactor::UiMarshaller,
    pub(super) ui: Rc<UiState>,
    pub(super) limits: Rc<ProviderLimits>,
    pub(super) forced_resets: Rc<Vec<ForcedReset>>,
    pub(super) palette: Palette,
    pub(super) accent: crate::theme::AccentRamp,
    pub(super) font_family: SharedString,
    pub(super) fx: Fx,
    pub(super) hover: HashSet<u64>,
    pub(super) host: Host,
    pub(super) pager: PagerState,
    pub(super) pager_started: Instant,
    pub(super) claude_profile: Option<String>,
    pub(super) codex_profile: Option<String>,
    pub(super) overview_metric: OverviewMetric,
    pub(super) overview_range: OverviewRange,
    pub(super) overview_breakdown: BreakdownMode,
    pub(super) chart_hover: Option<usize>,
    pub(super) open_reset_card: Option<String>,
    pub(super) tab_scroll: f32,
    pub(super) snapshots: HashMap<SnapshotSlot, SnapshotCache>,
    pub(super) usage_chart_cache: Option<super::usage::UsageChartCache>,
    pub(super) usage_plot: Option<Entity<super::usage::UsagePlot>>,
    pub(super) charts: HashMap<String, super::activity::ChartState>,
    pub(super) tip: Option<TipRequest>,
    pub(super) pages: HashMap<PopupView, PageMetrics>,
    pub(super) widget_bounds: Rc<RefCell<WidgetLayout>>,
    pub(super) usage_spinner_started: Option<Instant>,
    pub(super) widget_drag: Option<PopupWidgetKind>,
    pub(super) widget_drop: Option<(PopupWidgetKind, Option<usize>)>,
    pub(super) tab_drag: Option<ProviderKind>,
    pub(super) tab_drop: Option<ProviderKind>,
    pub(super) refresh_started: Option<Instant>,
    pub(super) profile_layout: String,
    pub(super) profile_fade_started: Option<Instant>,
    pub(super) capsule_origin: Point<Pixels>,
    pub(super) capsule_size: (f32, f32),
    pub(super) _subscriptions: Vec<Subscription>,
    pub(super) _clock: Option<Task<()>>,
}

impl PopupRoot {
    pub(crate) fn new(
        state: Arc<AppState>,
        ui_dispatcher: windows_reactor::UiMarshaller,
        font_family: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let ui = initial_ui_state(&state);
        let accent = theme::accent_ramp(ui.accent_color);
        let dark = theme::resolve_dark(ui.theme, system_dark(window));
        let palette = Palette::new(
            dark,
            accent,
            crate::popup::background_material(),
            font_family.clone(),
        );
        let appearance = cx.observe_window_appearance(window, |this, window, cx| {
            this.refresh_palette(window);
            cx.notify();
        });
        // Alt+F4 on the focused popup dismisses it; the HWND must survive.
        window.on_window_should_close(cx, |_, _| {
            crate::popup::hide();
            false
        });
        let clock = cx.spawn(async move |this: WeakEntity<Self>, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(20))
                    .await;
                // Relative timestamps and reset countdowns age while open.
                let alive = this
                    .update(cx, |this, cx| {
                        if this.host.visible() {
                            cx.notify();
                        }
                    })
                    .is_ok();
                if !alive {
                    break;
                }
            }
        });
        Self {
            limits: Rc::new(state.current_limits()),
            forced_resets: Rc::new(state.current_forced_resets()),
            state,
            ui_dispatcher,
            ui: Rc::new(ui),
            palette,
            accent,
            font_family,
            fx: Fx::default(),
            hover: HashSet::new(),
            host: Host::new(),
            pager: PagerState::default(),
            pager_started: Instant::now(),
            claude_profile: None,
            codex_profile: None,
            overview_metric: OverviewMetric::default(),
            overview_range: OverviewRange::default(),
            overview_breakdown: BreakdownMode::default(),
            chart_hover: None,
            open_reset_card: None,
            tab_scroll: 0.0,
            snapshots: HashMap::new(),
            usage_chart_cache: None,
            usage_plot: None,
            charts: HashMap::new(),
            tip: None,
            pages: HashMap::new(),
            widget_bounds: Rc::new(RefCell::new(WidgetLayout::default())),
            usage_spinner_started: None,
            widget_drag: None,
            widget_drop: None,
            tab_drag: None,
            tab_drop: None,
            refresh_started: None,
            profile_layout: String::new(),
            profile_fade_started: None,
            capsule_origin: Point::default(),
            capsule_size: (0.0, 0.0),
            _subscriptions: vec![appearance],
            _clock: Some(clock),
        }
    }

    // ----- external updates -------------------------------------------------

    pub(crate) fn apply_ui(&mut self, ui: UiState, window: &mut Window, cx: &mut Context<Self>) {
        let accent_changed = ui.accent_color != self.ui.accent_color;
        // Rate-limit data lives only in AppState; re-read it with every
        // published UiState so the popup renders the tray's exact snapshot.
        self.limits = Rc::new(self.state.current_limits());
        self.forced_resets = Rc::new(self.state.current_forced_resets());
        self.ui = Rc::new(ui);
        if accent_changed {
            self.accent = theme::accent_ramp(self.ui.accent_color);
        }
        crate::theme::set_animations_enabled(self.ui.animations_enabled);
        self.ui.time_format.apply();
        self.refresh_palette(window);
        if self.ui.refreshing && self.refresh_started.is_none() {
            self.refresh_started = Some(Instant::now());
        } else if !self.ui.refreshing {
            self.refresh_started = None;
        }
        self.sync_pager_with_settings();
        if self.host.visible() {
            cx.notify();
        }
    }

    pub(super) fn refresh_palette(&mut self, window: &Window) {
        let dark = theme::resolve_dark(self.ui.theme, system_dark(window));
        let material = crate::popup::background_material();
        if self.palette.dark != dark
            || self.palette.material != material
            || self.palette.accent != theme::rgb8(self.accent.fill(dark))
        {
            self.palette = Palette::new(dark, self.accent, material, self.font_family.clone());
        }
    }

    pub(crate) fn appearance_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_palette(window);
        cx.notify();
    }

    pub(crate) fn select_view(&mut self, view: PopupView, cx: &mut Context<Self>) {
        if self.view_available(view) {
            self.navigate(view, cx);
        }
    }

    fn view_available(&self, view: PopupView) -> bool {
        match view {
            PopupView::Home => true,
            PopupView::Usage => self.ui.usage_stats_enabled,
            other => other
                .provider()
                .is_some_and(|provider| model::provider_enabled(&self.ui, provider)),
        }
    }

    fn sync_pager_with_settings(&mut self) {
        let order = provider_order_from_popup(&self.ui.popup_order);
        self.pager = reduce_pager(self.pager.clone(), PagerAction::SetProviderOrder(order));
        if !self.view_available(self.pager.current) {
            self.pager = reduce_pager(self.pager.clone(), PagerAction::Select(PopupView::Home));
            self.pager_started = Instant::now();
            if !self.fx.enabled() {
                self.finish_pager_animation();
            }
        }
    }

    pub(super) fn navigate(&mut self, view: PopupView, cx: &mut Context<Self>) {
        let before = self.pager.animation_id;
        self.pager = reduce_pager(self.pager.clone(), PagerAction::Select(view));
        if self.pager.animation_id != before {
            self.pager_started = Instant::now();
            self.on_page_entered(self.pager.current);
            if !super::animations_enabled(&self.ui) {
                self.finish_pager_animation();
            }
        }
        self.tip = None;
        cx.notify();
    }

    fn on_page_entered(&mut self, view: PopupView) {
        let metrics = self.pages.entry(view).or_default();
        metrics.scroll_target = 0.0;
        self.fx.snap(fx::key(("scroll", view_key(view))), 0.0);
        self.chart_hover = None;
    }

    fn finish_pager_animation(&mut self) {
        while self.pager.outgoing.is_some() {
            let id = self.pager.animation_id;
            self.pager = reduce_pager(self.pager.clone(), PagerAction::AnimationFinished(id));
            if self.pager.outgoing.is_some() {
                self.pager_started = Instant::now();
                self.on_page_entered(self.pager.current);
                if super::animations_enabled(&self.ui) {
                    break;
                }
            }
        }
    }

    // ----- lifecycle --------------------------------------------------------

    /// Prepares geometry for a show; the caller performs the native show
    /// outside of this update so GPUI can paint the first frame.
    #[cfg(windows)]
    pub(crate) fn begin_show(
        &mut self,
        anchor: Option<(i32, i32)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<super::ShowPlan> {
        let hwnd = self.host.hwnd?;
        if let Some(view) = take_popup_view_request() {
            let view = match view {
                PendingPopupView::Home => PopupView::Home,
                PendingPopupView::Provider(provider) => PopupView::from_provider(provider),
            };
            if self.view_available(view) && view != self.pager.current {
                // A fresh open starts directly on the requested page.
                self.pager = reduce_pager(self.pager.clone(), PagerAction::Select(view));
                self.finish_pager_animation_now();
                self.on_page_entered(self.pager.current);
            }
        }
        let already_visible = matches!(self.host.phase, Phase::Open | Phase::Opening(_));
        let was_hidden = matches!(self.host.phase, Phase::Hidden);
        let anchor = anchor.unwrap_or_else(|| {
            let rect = super::win32::window_rect(hwnd);
            (rect.right - 1, rect.bottom - 1)
        });
        let monitor = super::win32::monitor_for_point(anchor.0, anchor.1);
        self.host.monitor = Some(monitor);
        self.host.scale = monitor.scale() as f32;
        // The system accent can change while the popup is hidden.
        self.accent = theme::accent_ramp(self.ui.accent_color);
        self.refresh_palette(window);
        self.limits = Rc::new(self.state.current_limits());
        self.forced_resets = Rc::new(self.state.current_forced_resets());
        let rect = super::host_rect(monitor);
        if !already_visible {
            let closing_offset = match self.host.phase {
                Phase::Closing(started, from) => Some(closing_offset(
                    started,
                    from,
                    self.capsule_size.0,
                    self.host.margin(),
                )),
                _ => None,
            };
            let target_height = self.target_height(self.pager.current);
            self.host.height.snap(f64::from(target_height));
            let target_width = self.target_width(self.pager.current);
            self.host.width.snap(f64::from(target_width));
            self.host.phase = if super::animations_enabled(&self.ui) {
                // Reopening mid-close continues from the current position.
                let start = closing_offset.unwrap_or(target_width + self.host.margin());
                let full = target_width + self.host.margin();
                let progress_offset = 1.0 - (start / full).clamp(0.0, 1.0);
                Phase::Opening(
                    Instant::now()
                        - Duration::from_secs_f32(
                            OPEN_ANIMATION.as_secs_f32() * progress_offset * 0.5,
                        ),
                )
            } else {
                Phase::Open
            };
            self.host.last_region = None;
            self.tip = None;
            self.hover.clear();
        }
        crate::popup::set_lifecycle(true, false);
        cx.notify();
        Some(super::ShowPlan {
            hwnd,
            rect,
            first_show: was_hidden,
        })
    }

    fn finish_pager_animation_now(&mut self) {
        while self.pager.outgoing.is_some() {
            let id = self.pager.animation_id;
            self.pager = reduce_pager(self.pager.clone(), PagerAction::AnimationFinished(id));
        }
    }

    pub(crate) fn begin_hide(&mut self, cx: &mut Context<Self>) {
        match self.host.phase {
            Phase::Hidden | Phase::Closing(..) => {}
            _ => {
                let from = self.host.offset;
                if super::animations_enabled(&self.ui) {
                    self.host.phase = Phase::Closing(Instant::now(), from);
                    crate::popup::set_lifecycle(true, true);
                } else {
                    self.finish_hide(cx);
                }
                self.tip = None;
                cx.notify();
            }
        }
    }

    fn finish_hide(&mut self, cx: &mut Context<Self>) {
        self.host.phase = Phase::Hidden;
        self.host.offset = 0.0;
        self.hover.clear();
        self.tip = None;
        self.widget_drag = None;
        self.tab_drag = None;
        // Keep only interaction choices and tiny page measurements while
        // hidden. Drop derived usage/model data and cancel outstanding loads.
        self.snapshots = HashMap::new();
        self.usage_chart_cache = None;
        self.usage_plot = None;
        for chart in self.charts.values_mut() {
            chart.release_data();
        }
        self.fx = Fx::default();
        self.chart_hover = None;
        self.usage_spinner_started = None;
        crate::popup::set_lifecycle(false, false);
        crate::popup::publish_surface_bounds(0, 0, 0, 0);
        #[cfg(windows)]
        if let Some(hwnd) = self.host.hwnd {
            // ShowWindow re-enters GPUI; run it after this frame.
            cx.spawn(async move |this, cx| {
                // A tray click may reopen between the last closing frame
                // and this task. Never park an already-reopened popup.
                let hidden = this
                    .update(cx, |root, _| !root.host.visible())
                    .unwrap_or(false);
                if hidden {
                    super::win32::set_region(hwnd, None, 0);
                    super::win32::hide(hwnd);
                    super::win32::park_hidden(hwnd);
                }
            })
            .detach();
        }
        self.host.last_region = None;
    }

    // ----- geometry ---------------------------------------------------------

    pub(super) fn two_columns(&self) -> bool {
        self.ui.popup_two_columns && self.host.wide_available()
    }

    /// GPUI's view cache keys include bounds/mask/text, but not inherited
    /// opacity. Repaint during page/profile/drag fades; cache stable frames.
    pub(super) fn cache_graph_paint(&self) -> bool {
        self.pager.outgoing.is_none()
            && self.profile_fade_started.is_none()
            && self.widget_drag.is_none()
            && PopupWidgetKind::ALL.iter().all(|widget| {
                self.fx
                    .is_at_target(fx::key(("widget-dim", widget.id())), 0.0)
            })
    }

    pub(super) fn page_width(&self, view: PopupView) -> f32 {
        if view.uses_two_columns(self.two_columns()) {
            crate::popup::POPUP_WIDE_WIDTH as f32
        } else {
            crate::popup::POPUP_WIDTH as f32
        }
    }

    fn target_width(&self, view: PopupView) -> f32 {
        self.page_width(view)
    }

    pub(super) fn footer_height(&self) -> f32 {
        crate::popup::bottom_bar_size().footer_height_dip() as f32
    }

    pub(super) fn show_profile_strip(&self) -> bool {
        let view = self.pager.current;
        let provider = match view {
            PopupView::Codex => ProviderKind::Codex,
            PopupView::Claude => ProviderKind::Claude,
            _ => return false,
        };
        !self.ui.show_accounts_as_tabs
            && self.limits.get(provider).account_profiles(provider).len() > 1
    }

    pub(super) fn chrome_height(&self) -> f32 {
        self.footer_height()
            + if self.show_profile_strip() {
                PROFILE_STRIP_HEIGHT
            } else {
                0.0
            }
            + CHROME_INSET * 2.0
    }

    fn target_height(&self, view: PopupView) -> f32 {
        let content = self
            .pages
            .get(&view)
            .map(|metrics| metrics.content_height.get())
            .filter(|height| *height > 1.0)
            .unwrap_or(INITIAL_HEIGHT - self.chrome_height());
        (content + self.chrome_height()).clamp(80.0, self.host.max_height())
    }

    /// Advance window motion for this frame. Returns `(width, height, offset)`.
    fn step_motion(&mut self, now: Instant, cx: &mut Context<Self>) -> (f32, f32, f32) {
        let target_h = self.target_height(self.pager.current);
        let target_w = self.target_width(self.pager.current);
        let animate = super::animations_enabled(&self.ui) && self.host.visible();
        let opening_early = matches!(self.host.phase, Phase::Opening(started)
            if now.saturating_duration_since(started).as_secs_f32() < OPEN_ANIMATION.as_secs_f32() * 0.35);
        if animate && !opening_early {
            self.host.height.retarget(f64::from(target_h));
            self.host.width.retarget(f64::from(target_w));
        } else {
            // Hidden pixels never need a glide: the first frames of the open
            // slide are still beyond the monitor edge.
            self.host.height.snap(f64::from(target_h));
            self.host.width.snap(f64::from(target_w));
        }
        let height = self.host.height.step(now) as f32;
        let width = self.host.width.step(now) as f32;
        if !self.host.height.settled || !self.host.width.settled {
            self.fx.mark_animating();
        }
        let travel = width + self.host.margin();
        let offset = match self.host.phase {
            Phase::Hidden | Phase::Open => 0.0,
            Phase::Opening(started) => {
                let progress = now.saturating_duration_since(started).as_secs_f64()
                    / OPEN_ANIMATION.as_secs_f64();
                if progress >= 1.0 {
                    self.host.phase = Phase::Open;
                    #[cfg(windows)]
                    if let Some(hwnd) = self.host.hwnd {
                        cx.spawn(async move |_, _| super::win32::activate(hwnd))
                            .detach();
                    }
                    crate::popup::arm_outside_click_grace();
                    0.0
                } else {
                    self.fx.mark_animating();
                    (1.0 - crate::popup::ease_entrance(progress)) as f32 * travel
                }
            }
            Phase::Closing(started, from) => {
                let offset = closing_offset(started, from, width, self.host.margin());
                if offset >= travel - 0.5 {
                    self.finish_hide(cx);
                    travel
                } else {
                    self.fx.mark_animating();
                    offset
                }
            }
        };
        self.host.offset = offset;
        crate::popup::set_client_width_dip(width.round() as i32);
        (width, height, offset)
    }

    /// Clip the HWND to the capsule and publish its screen bounds.
    fn apply_region(&mut self, window: &Window, x: f32, y: f32, width: f32, height: f32) {
        #[cfg(windows)]
        {
            let Some(hwnd) = self.host.hwnd else {
                return;
            };
            if !self.host.visible() {
                return;
            }
            let scale = window.scale_factor();
            let viewport = window.viewport_size();
            let window_w = (f32::from(viewport.width) * scale).round() as i32;
            let window_h = (f32::from(viewport.height) * scale).round() as i32;
            let left = ((x * scale).round() as i32).clamp(0, window_w);
            let right = (((x + width) * scale).round() as i32).clamp(left, window_w);
            let top = ((y * scale).round() as i32).clamp(0, window_h);
            let bottom = (((y + height) * scale).round() as i32).clamp(top, window_h);
            let radius = (crate::popup::corner_radius_dip() as f32 * scale).round() as i32;
            let region = (left, top, right, bottom, radius);
            if self.host.last_region == Some(region) {
                return;
            }
            self.host.last_region = Some(region);
            super::win32::set_region(
                hwnd,
                (right > left && bottom > top).then_some(super::win32::Rect {
                    left,
                    top,
                    right,
                    bottom,
                }),
                radius,
            );
            let origin = super::win32::window_rect(hwnd);
            crate::popup::publish_surface_bounds(
                origin.left + left,
                origin.top + top,
                origin.left + right,
                origin.top + bottom,
            );
        }
        #[cfg(not(windows))]
        {
            let _ = (window, x, y, width, height);
        }
    }

    // ----- hover / tooltip helpers -------------------------------------------

    pub(super) fn hovered(&self, id: u64) -> bool {
        self.hover.contains(&id)
    }

    pub(super) fn set_hover(&mut self, id: u64, hovered: bool) -> bool {
        if hovered {
            self.hover.insert(id)
        } else {
            self.hover.remove(&id)
        }
    }

    /// Hover listener that records state, animates, and shows a tooltip.
    pub(super) fn hover_listener(
        &self,
        id: u64,
        tooltip: Option<SharedString>,
        cx: &mut Context<Self>,
    ) -> impl Fn(&bool, &mut Window, &mut App) + 'static {
        cx.listener(move |this, hovered: &bool, window, cx| {
            let changed = this.set_hover(id, *hovered);
            if let Some(text) = tooltip.as_ref() {
                if *hovered {
                    this.show_tip(id, TipContent::Text(text.clone()), window, cx);
                } else if this.tip.as_ref().is_some_and(|tip| tip.owner == id) {
                    this.tip = None;
                    cx.notify();
                }
            }
            if changed {
                cx.notify();
            }
        })
    }

    pub(super) fn show_tip(
        &mut self,
        owner: u64,
        content: TipContent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delayed = matches!(content, TipContent::Text(_));
        self.tip = Some(TipRequest {
            owner,
            content,
            anchor: window.mouse_position(),
            since: Instant::now(),
            delayed,
        });
        if delayed {
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(super::tooltip::TOOLTIP_DELAY)
                    .await;
                let _ = this.update(cx, |_, cx| cx.notify());
            })
            .detach();
        }
        cx.notify();
    }

    // ----- actions ------------------------------------------------------------

    pub(super) fn refresh(&mut self, cx: &mut Context<Self>) {
        if refresh_all_workers(&self.state.worker_commands()) {
            let mut ui = (*self.ui).clone();
            ui.refreshing = true;
            self.ui = Rc::new(ui);
            if self.refresh_started.is_none() {
                self.refresh_started = Some(Instant::now());
            }
            cx.notify();
        }
    }

    pub(super) fn open_settings(&mut self) {
        let settings_tx = self.state.settings_tx.clone();
        let usage_actions_tx = self.state.usage_actions_tx.clone();
        let updates = Arc::clone(&self.state.updates);
        self.ui_dispatcher.dispatch(move || {
            if let Err(error) = crate::settings_window::open(settings_tx, usage_actions_tx, updates)
            {
                eprintln!("Could not open settings window: {error:?}");
            }
        });
    }

    pub(super) fn install_update(&mut self) {
        std::thread::spawn(|| {
            if let Err(error) = crate::updater::apply_pending_update() {
                eprintln!("failed to apply update: {error:#}");
                crate::notifications::show("Update failed", &format!("{error:#}"));
            }
        });
    }

    /// Optimistically apply a popup-owned settings change, then persist it.
    pub(super) fn persist(
        &mut self,
        cx: &mut Context<Self>,
        local: impl FnOnce(&mut UiState),
        update: impl FnOnce(&mut Settings) + Send + 'static,
    ) {
        let mut ui = (*self.ui).clone();
        local(&mut ui);
        self.ui = Rc::new(ui);
        cx.notify();
        let settings_tx = self.state.settings_tx.clone();
        // Disk I/O and the settings broadcast stay off the render thread.
        std::thread::spawn(move || crate::settings_window::persist_update(settings_tx, update));
    }

    // ----- snapshots ------------------------------------------------------------

    /// Stale-while-revalidate usage aggregation on the background executor.
    pub(super) fn snapshot(
        &mut self,
        slot: SnapshotSlot,
        key: String,
        enabled: Vec<ProviderKind>,
        query: impl FnOnce(&ProviderLimits, &[ProviderKind]) -> OverviewSnapshot + Send + 'static,
        cx: &mut Context<Self>,
    ) -> Arc<OverviewSnapshot> {
        let cache = self.snapshots.entry(slot).or_default();
        if cache.key.as_deref() == Some(key.as_str()) {
            return Arc::clone(&cache.value);
        }
        // First load also belongs on the background executor: aggregating
        // local usage can read the database and must not stall the open slide.
        if cache.pending.as_deref() != Some(key.as_str()) {
            cache.pending = Some(key.clone());
            let limits = (*self.limits).clone();
            let task_key = key.clone();
            cache.task = Some(cx.spawn(async move |this, cx| {
                let value = cx
                    .background_executor()
                    .spawn(async move { query(&limits, &enabled) })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    let cache = this.snapshots.entry(slot).or_default();
                    if cache.pending.as_deref() == Some(task_key.as_str()) {
                        cache.value = Arc::new(value);
                        cache.key = Some(task_key);
                        cache.pending = None;
                        cx.notify();
                    }
                });
            }));
        }
        Arc::clone(&cache.value)
    }

    // ----- pages ----------------------------------------------------------------

    pub(super) fn page_metrics(&mut self, view: PopupView) -> PageMetrics {
        self.pages.entry(view).or_default().clone()
    }

    pub(super) fn scroll_page(
        &mut self,
        view: PopupView,
        event: &ScrollWheelEvent,
        viewport_height: f32,
        cx: &mut Context<Self>,
    ) {
        let delta = f32::from(event.delta.pixel_delta(px(20.0)).y);
        let metrics = self.pages.entry(view).or_default();
        let max = (metrics.content_height.get() - viewport_height).max(0.0);
        let next = (metrics.scroll_target - delta * 2.0).clamp(0.0, max);
        if (next - metrics.scroll_target).abs() > 0.1 {
            metrics.scroll_target = next;
            cx.notify();
        }
    }
}

pub(super) fn view_key(view: PopupView) -> u8 {
    match view {
        PopupView::Home => 0,
        PopupView::Usage => 1,
        PopupView::Codex => 2,
        PopupView::Claude => 3,
        PopupView::Cursor => 4,
        PopupView::OpenCodeZen => 5,
        PopupView::OpenCodeGo => 6,
        PopupView::OpenRouter => 7,
        PopupView::Antigravity => 8,
        PopupView::Grok => 9,
        PopupView::Kiro => 10,
    }
}

fn closing_offset(started: Instant, from: f32, width: f32, margin: f32) -> f32 {
    let travel = width + margin;
    let progress = started.elapsed().as_secs_f64() / CLOSE_ANIMATION.as_secs_f64();
    let eased = crate::popup::ease_exit(progress) as f32;
    from + (travel - from) * eased
}

fn system_dark(window: &Window) -> bool {
    matches!(
        window.appearance(),
        gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
    )
}

/// Seed identical to the background bridge so the very first frame already
/// reflects persisted settings, before the bridge publishes.
fn initial_ui_state(state: &AppState) -> UiState {
    let settings = &state.settings;
    UiState {
        theme: settings.theme,
        accent_color: settings.accent_color,
        animations_enabled: settings.animations_enabled,
        popup_background_material: settings.popup_background_material,
        time_format: settings.time_format,
        provider_errors: state
            .startup_provider_errors
            .iter()
            .map(|(provider, error)| (*provider, UiState::error_for_ui(error)))
            .collect(),
        last_activation: format_last_activation(&RateLimits::default(), state.last_activation_at),
        show_used_percentage: settings.show_used_percentage,
        show_usage_values: settings.show_usage_values,
        show_usage_pace: settings.show_usage_pace,
        compact_usage_cards: settings.compact_usage_cards,
        popup_visibility: settings.popup_visibility.clone(),
        usage_stats_enabled: settings.usage_stats_enabled,
        show_total_spend_on_all_tab: settings.show_total_spend_on_all_tab,
        total_spend_presentation: settings.total_spend_presentation,
        total_spend_period: settings.total_spend_period,
        show_account_name: settings.show_account_name,
        codex_enabled: settings.providers.is_enabled(ProviderKind::Codex),
        claude_enabled: settings.providers.is_enabled(ProviderKind::Claude),
        cursor_enabled: settings.providers.is_enabled(ProviderKind::Cursor),
        opencode_zen_enabled: settings.providers.is_enabled(ProviderKind::OpenCodeZen),
        opencode_go_enabled: settings.providers.is_enabled(ProviderKind::OpenCodeGo),
        opencode_zen_credentials_revision: settings.opencode_zen_credentials_revision,
        opencode_go_credentials_revision: settings.opencode_go_credentials_revision,
        openrouter_enabled: settings.providers.is_enabled(ProviderKind::OpenRouter),
        antigravity_enabled: settings.providers.is_enabled(ProviderKind::Antigravity),
        grok_enabled: settings.providers.is_enabled(ProviderKind::Grok),
        kiro_enabled: settings.providers.is_enabled(ProviderKind::Kiro),
        openrouter_credentials_revision: settings.openrouter_credentials_revision,
        popup_order: settings.popup_order.clone(),
        use_colored_provider_icons: settings.use_colored_provider_icons,
        replace_chatgpt_logo_with_codex: settings.replace_chatgpt_logo_with_codex,
        update_version: state
            .updates
            .available_update()
            .map(|update| update.version),
        ..UiState::popup_layout_from_settings(settings)
    }
}

// ----- render -------------------------------------------------------------------

impl Render for PopupRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.host.visible() {
            return div().id("popup-root").size_full();
        }
        let now = Instant::now();
        self.fx.begin_frame(super::animations_enabled(&self.ui));
        if !cx.has_active_drag() {
            self.widget_drag = None;
            self.widget_drop = None;
            self.tab_drag = None;
            self.tab_drop = None;
        }
        // Complete or advance page transitions.
        if self.pager.outgoing.is_some() {
            if !self.fx.enabled()
                || now.saturating_duration_since(self.pager_started) >= PAGER_ANIMATION_DURATION
            {
                let id = self.pager.animation_id;
                self.pager = reduce_pager(self.pager.clone(), PagerAction::AnimationFinished(id));
                if self.pager.animation_id != id {
                    self.pager_started = now;
                    self.on_page_entered(self.pager.current);
                }
            } else {
                self.fx.mark_animating();
            }
        }

        let palette = self.palette.clone();
        let ui = Rc::clone(&self.ui);
        let limits = Rc::clone(&self.limits);

        // Account switches with a different card layout fade the page in so
        // the remeasured height is not seen as cards jumping.
        let profile_layout = format!(
            "{}|{}",
            profile_layout_key(
                ProviderKind::Claude,
                limits.get(ProviderKind::Claude),
                self.claude_profile.as_deref(),
                ui.show_used_percentage,
                ui.show_usage_pace,
            ),
            profile_layout_key(
                ProviderKind::Codex,
                limits.get(ProviderKind::Codex),
                self.codex_profile.as_deref(),
                ui.show_used_percentage,
                ui.show_usage_pace,
            )
        );
        if profile_layout != self.profile_layout {
            if !self.profile_layout.is_empty()
                && matches!(self.pager.current, PopupView::Claude | PopupView::Codex)
            {
                self.profile_fade_started = Some(now);
            }
            self.profile_layout = profile_layout;
        }

        let (capsule_w, capsule_h, offset) = self.step_motion(now, cx);
        // Closing can finish inside step_motion. Submit an empty last frame,
        // so GPUI also drops scene listeners holding chart/snapshot Arcs.
        if !self.host.visible() {
            return div().id("popup-root").size_full();
        }
        let viewport = window.viewport_size();
        let viewport_w = f32::from(viewport.width);
        let viewport_h = f32::from(viewport.height);
        // A top/side taskbar can leave less work area than 80% of the monitor.
        let capsule_h = capsule_h.min(viewport_h);
        let margin = self.host.margin();
        let capsule_x = viewport_w - margin - capsule_w + offset;
        let capsule_y = (viewport_h - capsule_h).max(0.0);
        self.capsule_origin = point(px(capsule_x), px(capsule_y));
        self.capsule_size = (capsule_w, capsule_h);
        self.apply_region(window, capsule_x, capsule_y, capsule_w, capsule_h);

        let radius = crate::popup::corner_radius_dip() as f32;
        let body_viewport_h = (capsule_h - self.chrome_height()).max(0.0);

        // Pages: the outgoing page slides out while the current slides in.
        let mut pages: Vec<AnyElement> = Vec::with_capacity(2);
        let slide_width = self
            .pager
            .outgoing
            .map_or(crate::popup::POPUP_WIDTH, |from| {
                pager_slide_width(from, self.pager.current, self.two_columns())
            }) as f32;
        let transition = self.pager.outgoing.map(|_| {
            let progress = now
                .saturating_duration_since(self.pager_started)
                .as_secs_f64()
                / PAGER_ANIMATION_DURATION.as_secs_f64();
            fx::fluent(progress) as f32
        });
        if let (Some(outgoing), Some(progress)) = (self.pager.outgoing, transition) {
            let end = self.pager.direction.outgoing_offset(slide_width as i32);
            let element = self.render_page(
                outgoing,
                end * progress,
                1.0 - progress,
                body_viewport_h,
                false,
                window,
                cx,
            );
            pages.push(element);
        }
        let current = self.pager.current;
        let (current_x, current_opacity) = match transition {
            Some(progress) => {
                let start = self.pager.direction.incoming_offset(slide_width as i32);
                (start * (1.0 - progress), progress)
            }
            None => {
                let fade = self
                    .profile_fade_started
                    .map(|started| {
                        let t = now.saturating_duration_since(started).as_secs_f64()
                            / PAGER_ANIMATION_DURATION.as_secs_f64();
                        if t >= 1.0 {
                            self.profile_fade_started = None;
                            1.0
                        } else {
                            self.fx.mark_animating();
                            fx::fluent(t) as f32
                        }
                    })
                    .unwrap_or(1.0);
                (0.0, fade)
            }
        };
        pages.push(self.render_page(
            current,
            current_x,
            current_opacity,
            body_viewport_h,
            true,
            window,
            cx,
        ));

        let shell = div()
            .id("popup-page-viewport")
            .absolute()
            .top(px(CHROME_INSET))
            .left(px(CHROME_INSET))
            .right(px(CHROME_INSET))
            .bottom(px(self.chrome_height() - CHROME_INSET))
            .overflow_hidden()
            .children(pages);

        // Anchor persistent chrome to the FIXED technical host, not to the
        // animated capsule's top/height or a flex page layout. Otherwise
        // rounding top and height separately can move the footer by a pixel
        // as the height spring progresses. Only horizontal open/width motion
        // is shared with the capsule; the footer's vertical bounds stay fixed.
        let chrome_right = margin - offset + CHROME_INSET;
        let chrome_width = (capsule_w - CHROME_INSET * 2.0).max(0.0);
        let footer = div()
            .id("popup-pinned-footer")
            .absolute()
            .right(px(chrome_right))
            .bottom(px(CHROME_INSET))
            .w(px(chrome_width))
            .h(px(self.footer_height()))
            .child(self.render_footer(capsule_w, window, cx));
        let mut chrome = vec![footer.into_any_element()];
        if self.show_profile_strip() {
            chrome.push(
                div()
                    .id("popup-pinned-profiles")
                    .absolute()
                    .right(px(chrome_right))
                    .bottom(px(CHROME_INSET + self.footer_height()))
                    .w(px(chrome_width))
                    .h(px(PROFILE_STRIP_HEIGHT))
                    .child(self.render_profile_strip(window, cx))
                    .into_any_element(),
            );
        }

        let mut capsule = div()
            .id("popup-capsule")
            .absolute()
            .left(px(capsule_x))
            .top(px(capsule_y))
            .w(px(capsule_w))
            .h(px(capsule_h))
            .rounded(px(radius))
            .overflow_hidden()
            .font_family(palette.font_family.clone())
            .text_color(palette.text_primary)
            .text_size(px(14.0))
            .line_height(px(20.0))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !*hovered {
                    // Leaving the capsule must not strand any hover state.
                    this.hover.clear();
                    this.tip = None;
                    this.chart_hover = None;
                    cx.notify();
                }
            }))
            // Paint the entire capsule opaquely; the technical host stays
            // transparent outside it without any native blur/backdrop.
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(radius))
                    .bg(palette.solid_background),
            )
            .child(shell)
            // A hairline keeps the capsule edge crisp against any wallpaper.
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(radius))
                    .border_1()
                    .border_color(if palette.dark {
                        theme::rgba8(255, 255, 255, 0x14)
                    } else {
                        theme::rgba8(0, 0, 0, 0x12)
                    }),
            );
        if let Some(tip) = self.render_tip(capsule_w, capsule_h, window, cx) {
            capsule = capsule.child(tip);
        }

        if self.fx.is_animating() || (ui.refreshing && self.host.visible()) {
            window.request_animation_frame();
        }

        div()
            .id("popup-root")
            .size_full()
            .relative()
            .font_weight(FontWeight::NORMAL)
            .font_family(palette.font_family.clone())
            .text_color(palette.text_primary)
            .text_size(px(14.0))
            .line_height(px(20.0))
            .child(capsule)
            .children(chrome)
    }
}

impl PopupRoot {
    /// One scrolling page. `measure` marks the page whose natural height
    /// drives the capsule height.
    #[allow(clippy::too_many_arguments)]
    fn render_page(
        &mut self,
        view: PopupView,
        offset_x: f32,
        opacity: f32,
        viewport_height: f32,
        measure: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let body = self.build_body(view, window, cx);
        let metrics = self.page_metrics(view);
        let max_scroll = (metrics.content_height.get() - viewport_height).max(0.0);
        let target = metrics.scroll_target.min(max_scroll);
        if target != metrics.scroll_target {
            self.pages.entry(view).or_default().scroll_target = target;
        }
        let scroll = self
            .fx
            .value(fx::key(("scroll", view_key(view))), target, SCROLL_GLIDE)
            .clamp(0.0, max_scroll.max(0.0));
        let page_width = self.page_width(view) - CHROME_INSET * 2.0;
        let cell = Rc::clone(&metrics.content_height);
        let mut content = div()
            .w(px(page_width))
            .flex()
            .flex_col()
            .gap(px(PAGE_SPACING))
            .p(px(PAGE_PADDING))
            .children(body.into_iter().map(|body| div().flex_none().child(body)))
            .into_any_element();
        // Like GPUI's list elements, lay out the subtree as an independent
        // root. The animated viewport supplies ONLY its width, never height.
        // This breaks the Taffy containing-block -> content -> shell loop.
        let content = canvas(
            move |bounds, window, cx| {
                let measured = content.layout_as_root(
                    gpui::size(
                        AvailableSpace::Definite(px(page_width)),
                        AvailableSpace::MaxContent,
                    ),
                    window,
                    cx,
                );
                let height =
                    measured_page_height(f32::from(measured.height), window.scale_factor());
                if cell.get() != height {
                    cell.set(height);
                    window.request_animation_frame();
                }
                content.prepaint_at(bounds.origin - point(px(0.0), px(scroll)), window, cx);
                content
            },
            |_, mut content, window, cx| content.paint(window, cx),
        )
        .size_full();
        let mut page = div()
            .id(ElementId::Name(
                format!("popup-page-{}", view_key(view)).into(),
            ))
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(offset_x))
            .w(px(page_width))
            .overflow_hidden()
            .opacity(opacity)
            .child(content);
        if measure {
            page =
                page.on_scroll_wheel(cx.listener(move |this, event: &ScrollWheelEvent, _, cx| {
                    this.scroll_page(view, event, viewport_height, cx);
                }));
            // A growing shell can temporarily be shorter than its content.
            // Only show a scrollbar when the final shell cannot fit the page.
            let final_viewport = (self.target_height(view) - self.chrome_height()).max(0.0);
            if metrics.content_height.get() - final_viewport > 1.0 / window.scale_factor()
                && max_scroll > 0.5
            {
                page = page.child(self.render_scrollbar(view, scroll, max_scroll, viewport_height));
            }
        }
        page.into_any_element()
    }

    fn render_scrollbar(
        &mut self,
        view: PopupView,
        scroll: f32,
        max_scroll: f32,
        viewport_height: f32,
    ) -> Div {
        let content = viewport_height + max_scroll;
        let track = (viewport_height - 8.0).max(16.0);
        let thumb = (track * viewport_height / content).clamp(24.0, track);
        let top = 4.0 + (track - thumb) * (scroll / max_scroll.max(1.0));
        let scrolling =
            (scroll - self.pages.get(&view).map_or(0.0, |m| m.scroll_target)).abs() > 0.5;
        let visible = self.fx.toggle(
            fx::key(("scrollbar", view_key(view))),
            scrolling || self.hovered(fx::key(("page-hover", view_key(view)))),
            fx::FAST,
        );
        div()
            .absolute()
            .right(px(3.0))
            .top(px(top))
            .w(px(3.0))
            .h(px(thumb))
            .rounded(px(1.5))
            .bg(self.palette.text_tertiary.opacity(0.35 + 0.35 * visible))
    }

    /// Body sections for one page.
    fn build_body(
        &mut self,
        view: PopupView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let ui = Rc::clone(&self.ui);
        let palette = self.palette.clone();
        let mut body = Vec::new();
        if let Some(error) = ui.error.as_deref() {
            body.push(
                components::info_bar(
                    "Something went wrong",
                    error.to_owned(),
                    Severity::Error,
                    &palette,
                )
                .into_any_element(),
            );
        }
        if let Some(provider) = view.provider()
            && let Some(error) = ui.provider_error(provider)
        {
            body.push(
                components::info_bar(
                    format!("{} error", provider.display_name()),
                    error.to_owned(),
                    Severity::Error,
                    &palette,
                )
                .into_any_element(),
            );
        }
        match view {
            PopupView::Home => body.extend(self.render_home(window, cx)),
            PopupView::Usage => {
                for provider in self.enabled_spend() {
                    if let Some(error) = ui.provider_error(provider) {
                        body.push(
                            components::info_bar(
                                format!("{} error", provider.display_name()),
                                error.to_owned(),
                                Severity::Error,
                                &palette,
                            )
                            .into_any_element(),
                        );
                    }
                }
                body.push(self.render_usage_page(window, cx));
            }
            other => body.extend(self.render_provider_page(other, window, cx)),
        }
        if !model::any_provider_enabled(&ui) {
            body.push(
                components::info_bar(
                    "No providers enabled",
                    "Turn one on in Settings > Providers.",
                    Severity::Informational,
                    &palette,
                )
                .into_any_element(),
            );
        }
        body
    }

    pub(super) fn enabled_provider_order(&self) -> Vec<ProviderKind> {
        provider_order_from_popup(&self.ui.popup_order)
            .into_iter()
            .filter(|provider| model::provider_enabled(&self.ui, *provider))
            .collect()
    }

    pub(super) fn enabled_spend(&self) -> Vec<ProviderKind> {
        self.enabled_provider_order()
            .into_iter()
            .filter(|provider| {
                self.ui.usage_stats_provider_enabled(*provider)
                    && crate::provider_registry::descriptor(*provider).include_in_total_spend
            })
            .collect()
    }

    pub(super) fn claude_tabs(&self) -> Vec<crate::settings::ClaudeProfile> {
        if self.ui.show_accounts_as_tabs && self.ui.claude_enabled {
            claude_account_tabs(&self.ui.claude_profiles)
        } else {
            Vec::new()
        }
    }

    pub(super) fn codex_tabs(&self) -> Vec<crate::settings::CodexProfile> {
        if self.ui.show_accounts_as_tabs && self.ui.codex_enabled {
            codex_account_tabs(&self.ui.codex_profiles)
        } else {
            Vec::new()
        }
    }

    pub(super) fn show_provider_tabs(&self) -> bool {
        provider_icon_tab_count(
            &self.enabled_provider_order(),
            &self.claude_tabs(),
            &self.codex_tabs(),
        ) > 0
    }

    /// Provider tab content for one provider page.
    fn render_provider_page(
        &mut self,
        view: PopupView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(provider) = view.provider() else {
            return Vec::new();
        };
        let ui = Rc::clone(&self.ui);
        let limits = Rc::clone(&self.limits);
        let forced_resets = Rc::clone(&self.forced_resets);
        let palette = self.palette.clone();
        let limits_for_provider = limits.get(provider);
        let profile = match provider {
            ProviderKind::Codex => limits_for_provider.codex_profile(self.codex_profile.as_deref()),
            ProviderKind::Claude => {
                limits_for_provider.claude_profile(self.claude_profile.as_deref())
            }
            _ => None,
        };
        let mut body = Vec::new();
        if let Some(selected) = profile
            && let Some(error) = &selected.error
        {
            body.push(
                components::info_bar(
                    format!("{} error", selected.name),
                    cached_profile_error_for_ui(&selected.limits, error),
                    Severity::Error,
                    &palette,
                )
                .into_any_element(),
            );
        }
        let error_message = ui
            .provider_error(provider)
            .or(profile.and_then(|profile| profile.error.as_deref()))
            .map(|error| {
                profile.map_or_else(
                    || error.to_owned(),
                    |selected| cached_profile_error_for_ui(&selected.limits, error),
                )
            });
        let options = CardOptions {
            popup_visibility: &ui.popup_visibility,
            surface: PopupSurface::ProviderTab,
            show_provider_tabs: self.show_provider_tabs(),
            include_usage_stats: profile.is_none(),
            show_account_name: ui.show_account_name,
            drag_handle: false,
            openrouter_actions: provider == ProviderKind::OpenRouter,
            provider_error: error_message.as_deref(),
            now: Utc::now(),
        };
        let snapshot = profile.map_or(limits_for_provider, |profile| &profile.limits);
        let mut cards = provider_cards(provider, true, snapshot, &forced_resets, &options);
        if profile.is_some() {
            cards.extend(shared_usage_statistics_card(
                provider,
                limits_for_provider,
                true,
                &ui.popup_visibility,
                PopupSurface::ProviderTab,
                options.show_provider_tabs,
            ));
        }
        body.push(
            div()
                .flex()
                .flex_col()
                .gap(px(PAGE_SPACING))
                .children(self.render_cards(&cards, PopupSurface::ProviderTab, window, cx))
                .into_any_element(),
        );
        body
    }

    pub(super) fn card_style(&self) -> CardStyle {
        CardStyle {
            show_used_percentage: self.ui.show_used_percentage,
            show_usage_values: self.ui.show_usage_values,
            show_usage_pace: self.ui.show_usage_pace,
            compact: self.ui.compact_usage_cards,
        }
    }

    /// Account switcher pinned above the footer for multi-account providers.
    fn render_profile_strip(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let provider = if self.pager.current == PopupView::Codex {
            ProviderKind::Codex
        } else {
            ProviderKind::Claude
        };
        let limits = Rc::clone(&self.limits);
        let profiles = limits.get(provider).account_profiles(provider);
        let selected_id = if provider == ProviderKind::Codex {
            self.codex_profile.clone()
        } else {
            self.claude_profile.clone()
        };
        let selected = profiles
            .iter()
            .position(|profile| Some(profile.id.as_str()) == selected_id.as_deref())
            .unwrap_or(0);
        let labels = profiles
            .iter()
            .map(|profile| SharedString::from(profile.name.clone()))
            .collect::<Vec<_>>();
        let ids = profiles
            .iter()
            .map(|profile| profile.id.clone())
            .collect::<Vec<_>>();
        let control = self.segmented_control(
            fx::key(("profiles", provider.id())),
            labels,
            selected,
            true,
            move |this, index, cx| {
                let id = ids.get(index).cloned();
                if provider == ProviderKind::Codex {
                    this.codex_profile = id;
                } else {
                    this.claude_profile = id;
                }
                cx.notify();
            },
            window,
            cx,
        );
        div()
            .flex_none()
            .h(px(PROFILE_STRIP_HEIGHT))
            .px(px(16.0))
            .pb(px(10.0))
            .child(control)
    }
}

/// Element id helper.
pub(super) fn eid(value: impl Into<String>) -> ElementId {
    ElementId::Name(SharedString::from(value.into()))
}

#[cfg(test)]
mod geometry_tests {
    use super::*;

    #[test]
    fn page_height_rounds_up_to_physical_pixels_without_layout_noise() {
        assert_eq!(measured_page_height(87.4, 1.5), 88.0);
        assert_eq!(
            measured_page_height(169.001, 1.0),
            measured_page_height(169.0, 1.0)
        );
        assert_eq!(measured_page_height(0.0, 1.0), PAGE_PADDING * 2.0);
        for (scale, expected) in [(1.0, 169.0), (1.25, 169.6), (1.5, 169.33333), (2.0, 169.0)] {
            assert_eq!(measured_page_height(169.0, scale), expected);
        }
    }

    #[test]
    fn repeated_page_measurements_leave_the_shell_spring_settled() {
        let started = Instant::now();
        let mut spring = Spring::at(300.0);
        let target = measured_page_height(169.0, 1.0) + 62.0;
        for frame in 0..180 {
            let measured = measured_page_height(169.0 + (frame % 2) as f32 * 0.001, 1.0) + 62.0;
            spring.retarget(f64::from(measured));
            spring.step(started + Duration::from_millis(frame * 16));
            if frame >= 120 {
                assert!(spring.settled);
                assert_eq!(spring.position, f64::from(target));
                assert_eq!(spring.velocity, 0.0);
            }
        }
    }
}
