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
/// Longest frame the open slide advances through; slower frames are hitches.
const SLIDE_MAX_FRAME_STEP: Duration = Duration::from_millis(34);
/// Invisible frames rendered at launch before the popup host is parked.
const PREWARM_FRAMES: u32 = 3;
/// Following prewarm frames park the capsule one pixel from the screen edge,
/// where every open slide starts, and expose that pixel. DWM skips windows
/// with an empty region, so without this the first real open pays for the
/// backdrop's effect compilation and desktop capture inside DWM, a stall the
/// slide clock cannot see.
const PREWARM_EXPOSED_FRAMES: u32 = 6;
/// Pinned profile switcher between the page and the footer.
const PROFILE_COLUMNS: usize = 4;
const PROFILE_ROW_GAP: f32 = 6.0;
const PROFILE_GRID_PADDING: f32 = 12.0;

fn profile_grid_height(count: usize) -> f32 {
    let rows = count.div_ceil(PROFILE_COLUMNS);
    if rows == 0 {
        return 0.0;
    }
    rows as f32 * super::controls::SEGMENT_HEIGHT
        + rows.saturating_sub(1) as f32 * PROFILE_ROW_GAP
        + PROFILE_GRID_PADDING
}

fn profile_rows<T: Copy + PartialEq>(
    members: &[T],
    selected: Option<T>,
) -> Vec<(Vec<T>, Option<usize>)> {
    members
        .chunks(PROFILE_COLUMNS)
        .map(|row| {
            (
                row.to_vec(),
                row.iter().position(|member| Some(*member) == selected),
            )
        })
        .collect()
}
/// Small inner inset keeps page/footer content clear of the capsule stroke.
const CHROME_INSET: f32 = 1.0;
const PAGE_PADDING: f32 = 16.0;

/// One account bar's text and its animated, measured height.
#[derive(Default)]
pub(crate) struct AccountBar {
    spec: Option<AccountBarSpec>,
    height: Rc<Cell<f32>>,
}

#[derive(Clone)]
struct AccountBarSpec {
    title: String,
    message: String,
    severity: Severity,
    action: bool,
}
/// Smooth wheel scrolling: one notch glides over this duration.
const SCROLL_GLIDE: Duration = Duration::from_millis(140);
/// How long the overlay scrollbar stays after the last wheel tick.
const SCROLLBAR_LINGER: Duration = Duration::from_millis(900);
/// Brief scrollbar flash when a page first overflows, hinting it scrolls.
const SCROLLBAR_FLASH: Duration = Duration::from_millis(1100);
/// Pointer zone along the right edge that reveals the scrollbar.
const SCROLLBAR_HOVER_ZONE: f32 = 14.0;
/// Edge shadows hinting at content above or below the viewport.
const SCROLL_SHADE_HEIGHT: f32 = 14.0;

/// Round the independent MaxContent measurement once to physical pixels.
fn measured_page_height(height: f32, scale: f32) -> f32 {
    let scale = scale.max(0.5);
    ((height * scale - 0.01).ceil() / scale).max(PAGE_PADDING * 2.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Phase {
    Hidden,
    /// Launch-time invisible render (empty region) that warms the renderer,
    /// composition, glyph/icon atlases and usage snapshots before first open.
    Prewarm,
    Opening(Instant),
    Open,
    /// Closing from the given starting slide offset (DIP).
    Closing(Instant, f32),
}

/// Native host state. All geometry is in DIP unless suffixed `_px`.
pub(super) struct Host {
    #[cfg(windows)]
    pub(super) hwnd: Option<windows_sys::Win32::Foundation::HWND>,
    #[cfg(windows)]
    backdrop: Option<super::backdrop::Backdrop>,
    pub(super) monitor: Option<super::Monitor>,
    pub(super) phase: Phase,
    pub(super) height: Spring,
    pub(super) width: Spring,
    pub(super) offset: f32,
    pub(super) last_region: Option<(i32, i32, i32, i32, i32)>,
    pub(super) scale: f32,
    /// Previous frame of the open slide; long gaps are not counted as motion.
    pub(super) slide_frame: Option<Instant>,
    /// Frames rendered during `Phase::Prewarm`.
    pub(super) prewarm_frames: u32,
}

impl Host {
    fn new() -> Self {
        Self {
            #[cfg(windows)]
            hwnd: None,
            #[cfg(windows)]
            backdrop: None,
            monitor: None,
            phase: Phase::Hidden,
            height: Spring::at(f64::from(INITIAL_HEIGHT)),
            width: Spring::at(f64::from(crate::popup::POPUP_WIDTH)),
            offset: 0.0,
            last_region: None,
            scale: 1.0,
            slide_frame: None,
            prewarm_frames: 0,
        }
    }

    pub(super) fn visible(&self) -> bool {
        !matches!(self.phase, Phase::Hidden)
    }

    /// The prewarm frame that shows DWM the capsule's edge pixel.
    fn prewarm_exposed(&self) -> bool {
        self.phase == Phase::Prewarm
            && (PREWARM_FRAMES..PREWARM_FRAMES + PREWARM_EXPOSED_FRAMES)
                .contains(&self.prewarm_frames)
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
    pub(super) widgets: HashMap<HomeWidgetId, Bounds<Pixels>>,
    pub(super) columns: [Option<Bounds<Pixels>>; 2],
}

#[derive(Clone, Default)]
pub(super) struct PageMetrics {
    pub(super) content_height: Rc<Cell<f32>>,
    pub(super) scroll_target: f32,
    /// Whether the page overflowed on its last frame; a rising edge flashes
    /// the scrollbar.
    pub(super) overflowing: bool,
    /// The overlay scrollbar stays visible until this moment.
    pub(super) reveal_until: Option<Instant>,
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
    pub(super) ui: Rc<UiState>,
    pub(super) limits: Rc<ProviderLimits>,
    pub(super) forced_resets: Rc<Vec<ForcedReset>>,
    pub(super) palette: Palette,
    pub(super) accent: crate::theme::AccentRamp,
    /// Windows UI family used when no custom font is selected.
    pub(super) default_font: SharedString,
    /// The `vscode_themes::preview_generation` the palette was built for.
    pub(super) preview_generation: u64,
    pub(super) fx: Fx,
    pub(super) hover: HashSet<u64>,
    pub(super) host: Host,
    pub(super) pager: PagerState,
    pub(super) pager_started: Instant,
    pub(super) overview_metric: OverviewMetric,
    pub(super) overview_range: OverviewRange,
    pub(super) overview_breakdown: BreakdownMode,
    /// Providers toggled off on the Usage tab for this app session.
    pub(super) usage_excluded: std::collections::BTreeSet<ProviderId>,
    pub(super) chart_hover: Option<usize>,
    pub(super) open_reset_card: Option<String>,
    pub(super) reset_confirm: Option<ProviderId>,
    pub(super) reset_busy: HashSet<ProviderId>,
    pub(super) reset_status: HashMap<ProviderId, crate::banked_reset::Status>,
    /// OpenRouter key administration on the OpenRouter tab.
    pub(super) keys: super::keys::KeyAdmin,
    /// Settings-window controls (dropdowns) reused by popup forms.
    pub(super) kit: crate::settings_window::kit::Kit,
    kit_fonts: crate::settings_window::theme::Fonts,
    pub(super) tab_scroll: f32,
    pub(super) snapshots: HashMap<SnapshotSlot, SnapshotCache>,
    pub(super) usage_filtered: Option<super::usage::UsageFilterCache>,
    pub(super) usage_chart_cache: Option<super::usage::UsageChartCache>,
    pub(super) usage_plot: Option<Entity<super::usage::UsagePlot>>,
    pub(super) charts: HashMap<String, super::activity::ChartState>,
    pub(super) tip: Option<TipRequest>,
    language: crate::i18n::Language,
    pub(super) pages: HashMap<PopupView, PageMetrics>,
    pub(super) widget_bounds: Rc<RefCell<WidgetLayout>>,
    pub(super) usage_spinner_started: Option<Instant>,
    /// Home edit mode: reveals the widget reorder grips.
    pub(super) home_editing: bool,
    pub(super) widget_drag: Option<HomeWidgetId>,
    pub(super) widget_drop: Option<(HomeWidgetId, Option<usize>)>,
    pub(super) tab_drag: Option<PopupView>,
    pub(super) tab_drop: Option<PopupView>,
    pub(super) refresh_started: Option<Instant>,
    pub(super) profile_layout: String,
    pub(super) profile_fade_started: Option<Instant>,
    profile_height: f32,
    profile_scroll: gpui::ScrollHandle,
    profile_members: Vec<ProviderId>,
    profile_selected: Option<ProviderId>,
    /// Account bars per page, kept while they collapse after clearing.
    pub(super) account_bars: HashMap<(PopupView, ProviderId), AccountBar>,
    /// Body items that grow in and out, with how present each one is.
    pub(super) body_presence: Vec<(usize, f32)>,
    pub(super) capsule_origin: Point<Pixels>,
    pub(super) capsule_size: (f32, f32),
    pub(super) _subscriptions: Vec<Subscription>,
    pub(super) _clock: Option<Task<()>>,
    /// Repaints once the scrollbar reveal expires so it can fade out.
    pub(super) scrollbar_timer: Option<Task<()>>,
}

impl PopupRoot {
    pub(crate) fn new(
        state: Arc<AppState>,
        default_font: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let ui = initial_ui_state(&state);
        let accent = theme::accent_ramp(ui.accent_color);
        let (popup_theme, vscode) =
            theme::popup_design(ui.popup_theme, ui.popup_vscode_theme.as_deref());
        let dark = theme::palette_dark(
            vscode.as_deref(),
            theme::resolve_dark(ui.theme, system_dark(window)),
        );
        let palette = Palette::new(
            popup_theme,
            vscode,
            dark,
            accent,
            crate::popup::background_material(),
            theme::popup_font_family(popup_theme, ui.font_family.as_deref(), &default_font),
        )
        .with_contrast(ui.popup_vscode_contrast)
        .with_borders(ui.popup_borders);
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
            ui: Rc::new(ui),
            palette,
            accent,
            default_font,
            preview_generation: crate::vscode_themes::preview_generation(),
            fx: Fx::default(),
            hover: HashSet::new(),
            host: Host::new(),
            pager: PagerState::default(),
            pager_started: Instant::now(),
            overview_metric: OverviewMetric::default(),
            overview_range: OverviewRange::default(),
            overview_breakdown: BreakdownMode::default(),
            usage_excluded: Default::default(),
            chart_hover: None,
            open_reset_card: None,
            reset_confirm: None,
            reset_busy: HashSet::new(),
            reset_status: HashMap::new(),
            keys: Default::default(),
            kit: crate::settings_window::kit::Kit::with_caret("fluent-chevron-down"),
            kit_fonts: crate::settings_window::theme::Fonts::resolve(cx),
            tab_scroll: 0.0,
            snapshots: HashMap::new(),
            usage_filtered: None,
            usage_chart_cache: None,
            usage_plot: None,
            charts: HashMap::new(),
            tip: None,
            language: crate::i18n::current_language(),
            pages: HashMap::new(),
            widget_bounds: Rc::new(RefCell::new(WidgetLayout::default())),
            usage_spinner_started: None,
            home_editing: false,
            widget_drag: None,
            widget_drop: None,
            tab_drag: None,
            tab_drop: None,
            refresh_started: None,
            profile_layout: String::new(),
            profile_fade_started: None,
            profile_height: 0.0,
            profile_scroll: gpui::ScrollHandle::new(),
            profile_members: Vec::new(),
            profile_selected: None,
            account_bars: HashMap::new(),
            body_presence: Vec::new(),
            capsule_origin: Point::default(),
            capsule_size: (0.0, 0.0),
            _subscriptions: vec![appearance],
            _clock: Some(clock),
            scrollbar_timer: None,
        }
    }

    // ----- external updates -------------------------------------------------

    pub(crate) fn apply_ui(&mut self, ui: UiState, window: &mut Window, cx: &mut Context<Self>) {
        let accent_changed = ui.accent_color != self.ui.accent_color;
        let same_account = |provider| {
            ui.instance(provider)
                .zip(self.ui.instance(provider))
                .is_some_and(|(new, old)| new.enabled && new.runtime_key() == old.runtime_key())
        };
        self.reset_status
            .retain(|provider, _| same_account(*provider));
        if self
            .reset_confirm
            .is_some_and(|provider| !same_account(provider))
        {
            self.reset_confirm = None;
        }
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
        self.preview_generation = crate::vscode_themes::preview_generation();
        // Looked up on every refresh: reinstalling a theme replaces its entry.
        let (popup_theme, vscode) =
            theme::popup_design(self.ui.popup_theme, self.ui.popup_vscode_theme.as_deref());
        let dark = theme::palette_dark(
            vscode.as_deref(),
            theme::resolve_dark(self.ui.theme, system_dark(window)),
        );
        let material = crate::popup::background_material();
        let font = theme::popup_font_family(
            popup_theme,
            self.ui.font_family.as_deref(),
            &self.default_font,
        );
        let same_vscode = match (&self.palette.vscode, &vscode) {
            (Some(current), Some(next)) => Arc::ptr_eq(current, next),
            (None, None) => true,
            _ => false,
        };
        if self.palette.theme != popup_theme
            || !same_vscode
            || self.palette.dark != dark
            || self.palette.font_family != font
            || self.palette.material != material
            || self.palette.borders != self.ui.popup_borders
            || self.palette.contrast != self.ui.popup_vscode_contrast
            || (vscode.is_none()
                && self.palette.accent != Palette::accent_for(popup_theme, dark, self.accent))
        {
            #[cfg(windows)]
            if (self.palette.material != material || self.palette.dark != dark)
                && let Some(backdrop) = &mut self.host.backdrop
            {
                backdrop.set_appearance(material, dark);
            }
            self.palette = Palette::new(popup_theme, vscode, dark, self.accent, material, font)
                .with_contrast(self.ui.popup_vscode_contrast)
                .with_borders(self.ui.popup_borders);
        }
    }

    /// The popup's design for another surface (the notification host) with
    /// its own system appearance, on an opaque background.
    pub(super) fn palette_for(&self, system_dark: bool) -> Palette {
        let (popup_theme, vscode) =
            theme::popup_design(self.ui.popup_theme, self.ui.popup_vscode_theme.as_deref());
        let dark = theme::palette_dark(
            vscode.as_deref(),
            theme::resolve_dark(self.ui.theme, system_dark),
        );
        Palette::new(
            popup_theme,
            vscode,
            dark,
            theme::accent_ramp(self.ui.accent_color),
            crate::settings::PopupBackgroundMaterial::Solid,
            theme::popup_font_family(
                popup_theme,
                self.ui.font_family.as_deref(),
                &self.default_font,
            ),
        )
        .with_contrast(self.ui.popup_vscode_contrast)
        .with_borders(self.ui.popup_borders)
    }

    /// Screen x of the visible popup capsule's left edge, in pixels.
    #[cfg(windows)]
    pub(super) fn capsule_screen_left(&self) -> Option<i32> {
        let hwnd = self.host.hwnd.filter(|_| self.host.visible())?;
        let rect = super::win32::window_rect(hwnd);
        Some(rect.left + (f32::from(self.capsule_origin.x) * self.host.scale).round() as i32)
    }

    /// Gap between stacked cards; glides when the popup theme changes it.
    pub(super) fn card_gap(&mut self) -> f32 {
        self.fx.value(
            fx::key("card-gap"),
            self.palette.card_gap,
            crate::theme::CONTROL_NORMAL_ANIMATION,
        )
    }

    pub(crate) fn appearance_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_palette(window);
        cx.notify();
    }

    /// Opens a page. An instance inside a grouped tab opens its group with
    /// that instance selected in the switcher.
    pub(crate) fn select_view(&mut self, view: PopupView, cx: &mut Context<Self>) {
        let view = match view {
            PopupView::Provider(provider) => {
                let Some(tab) = model::tab_for_provider(&self.ui, provider) else {
                    return;
                };
                if let PopupView::Group(driver) = tab {
                    self.select_group_member(driver, provider, cx);
                }
                tab
            }
            other => other,
        };
        if self.view_available(view) {
            self.navigate(view, cx);
        }
    }

    fn view_available(&self, view: PopupView) -> bool {
        match view {
            PopupView::Home => true,
            PopupView::Usage => self.ui.usage_stats_enabled,
            other => model::provider_tabs(&self.ui).contains(&other),
        }
    }

    /// Persists the instance shown by a grouped tab's switcher.
    pub(super) fn select_group_member(
        &mut self,
        driver: ProviderKind,
        provider: ProviderId,
        cx: &mut Context<Self>,
    ) {
        if model::selected_group_member(&self.ui, driver) == Some(provider) {
            return;
        }
        let driver_id = driver.id().to_owned();
        let instance_id = provider.id().to_owned();
        let (local_driver, local_id) = (driver_id.clone(), instance_id.clone());
        self.persist(
            cx,
            move |ui| {
                ui.grouped_tab_selection.insert(local_driver, local_id);
            },
            move |settings| {
                settings
                    .grouped_tab_selection
                    .insert(driver_id, instance_id);
            },
        );
    }

    fn sync_pager_with_settings(&mut self) {
        let order = model::provider_tabs(&self.ui);
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
        if view != PopupView::Home {
            self.home_editing = false;
        }
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
        metrics.overflowing = false;
        metrics.reveal_until = None;
        self.fx.snap(fx::key(("scroll", view_key(view))), 0.0);
        self.fx
            .snap(fx::key(("scroll-shade-top", view_key(view))), 0.0);
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

    #[cfg(windows)]
    pub(crate) fn attach_native_host(&mut self, hwnd: windows_sys::Win32::Foundation::HWND) {
        self.host.hwnd = Some(hwnd);
        self.host.backdrop =
            match super::backdrop::Backdrop::new(hwnd, self.palette.material, self.palette.dark) {
                Ok(backdrop) => Some(backdrop),
                Err(error) => {
                    // Keep plain transparency if this system cannot host a clipped
                    // backdrop. Never fall back to blur across the whole HWND.
                    eprintln!("could not create capsule backdrop: {error:#}");
                    None
                }
            };
    }

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
        let was_hidden = matches!(self.host.phase, Phase::Hidden | Phase::Prewarm);
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
            // The first frame after a cold start (swap-chain growth, glyph
            // and icon rasterization) can outlast the whole slide. Count the
            // wait until that frame as a hitch so the entrance still plays.
            self.host.slide_frame = Some(Instant::now());
            self.host.last_region = None;
            self.tip = None;
            self.hover.clear();
            self.keys.mark_stale();
            // Every open flashes the scrollbar again on overflowing pages.
            for metrics in self.pages.values_mut() {
                metrics.overflowing = false;
                metrics.reveal_until = None;
            }
        }
        crate::popup::set_lifecycle(true, false);
        cx.notify();
        Some(super::ShowPlan {
            hwnd,
            rect,
            first_show: was_hidden,
        })
    }

    /// Prepares an invisible launch-time render at full host size, so the
    /// first real open does not pay for swap-chain growth, composition setup,
    /// glyph/icon rasterization or the first usage aggregation mid-slide.
    #[cfg(windows)]
    pub(crate) fn begin_prewarm(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<super::ShowPlan> {
        let hwnd = self.host.hwnd?;
        if self.host.phase != Phase::Hidden {
            return None;
        }
        // The primary monitor owns the taskbar tray in the common setup.
        let monitor = super::win32::monitor_for_point(0, 0);
        self.host.monitor = Some(monitor);
        self.host.scale = monitor.scale() as f32;
        self.refresh_palette(window);
        self.limits = Rc::new(self.state.current_limits());
        self.forced_resets = Rc::new(self.state.current_forced_resets());
        let target_height = self.target_height(self.pager.current);
        self.host.height.snap(f64::from(target_height));
        let target_width = self.target_width(self.pager.current);
        self.host.width.snap(f64::from(target_width));
        self.host.phase = Phase::Prewarm;
        self.host.prewarm_frames = 0;
        self.host.last_region = None;
        cx.notify();
        Some(super::ShowPlan {
            hwnd,
            rect: super::host_rect(monitor),
            first_show: true,
        })
    }

    /// The prewarm rendered enough frames and its usage snapshots landed, or
    /// a real open already took over.
    pub(crate) fn prewarm_settled(&self) -> bool {
        self.host.phase != Phase::Prewarm
            || (self.host.prewarm_frames >= PREWARM_FRAMES + PREWARM_EXPOSED_FRAMES
                && self.snapshots.values().all(|cache| cache.pending.is_none()))
    }

    pub(crate) fn end_prewarm(&mut self, cx: &mut Context<Self>) {
        if self.host.phase == Phase::Prewarm {
            self.finish_hide(cx);
            cx.notify();
        }
    }

    fn finish_pager_animation_now(&mut self) {
        while self.pager.outgoing.is_some() {
            let id = self.pager.animation_id;
            self.pager = reduce_pager(self.pager.clone(), PagerAction::AnimationFinished(id));
        }
    }

    pub(crate) fn begin_hide(&mut self, cx: &mut Context<Self>) {
        match self.host.phase {
            Phase::Hidden | Phase::Prewarm | Phase::Closing(..) => {}
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
        #[cfg(windows)]
        if let Some(backdrop) = &mut self.host.backdrop {
            backdrop.hide();
        }
        self.host.offset = 0.0;
        self.keys.on_hidden();
        self.reset_confirm = None;
        self.kit.menus.close_silently();
        self.hover.clear();
        self.tip = None;
        self.home_editing = false;
        self.widget_drag = None;
        self.tab_drag = None;
        // Keep only interaction choices, tiny page measurements and the last
        // committed usage snapshots while hidden. The snapshots are small and
        // let the next open paint the previous totals (stale-while-revalidate)
        // instead of a "Loading" placeholder that shifts the layout. Cancel
        // outstanding loads; a reopen recomputes against fresh data.
        self.snapshots.retain(|_, cache| cache.key.is_some());
        for cache in self.snapshots.values_mut() {
            cache.pending = None;
            cache.task = None;
        }
        self.usage_filtered = None;
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
            && self.widget_bounds.borrow().widgets.keys().all(|widget| {
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

    pub(super) fn card_inner_width(&self, surface: PopupSurface) -> f32 {
        let home = surface == PopupSurface::HomeTab;
        let width = if home {
            self.page_width(PopupView::Home)
        } else {
            crate::popup::POPUP_WIDTH as f32
        };
        let content = width - CHROME_INSET * 2.0 - PAGE_PADDING * 2.0;
        let column = if home && self.two_columns() {
            (content - super::home::COLUMN_GAP) / 2.0
        } else {
            content
        };
        // Card border and the 12 DIP padding on each side.
        (column - 26.0).max(0.0)
    }

    fn target_width(&self, view: PopupView) -> f32 {
        self.page_width(view)
    }

    pub(super) fn footer_height(&self) -> f32 {
        crate::popup::bottom_bar_size().footer_height_dip() as f32
    }

    /// The instance switcher shows on a grouped tab in switcher mode.
    pub(super) fn show_profile_strip(&self) -> bool {
        matches!(self.pager.current, PopupView::Group(_))
            && self.ui.popup_tab_mode == PopupTabMode::GroupedSwitcher
    }

    pub(super) fn chrome_height(&self) -> f32 {
        self.footer_height() + self.profile_height + CHROME_INSET * 2.0
    }

    fn target_profile_strip_height(&self) -> f32 {
        let PopupView::Group(driver) = self.pager.current else {
            return 0.0;
        };
        if !self.show_profile_strip() {
            return 0.0;
        }
        // Keep the account page usable; large grids scroll within the strip.
        profile_grid_height(self.ui.enabled_instances_of(driver).len())
            .min((self.host.max_height() * 0.5).max(profile_grid_height(1)))
    }

    fn target_height(&self, view: PopupView) -> f32 {
        let chrome = self.footer_height() + self.target_profile_strip_height() + CHROME_INSET * 2.0;
        // Usage loads asynchronously and its content height swings with the
        // range and provider count; a fixed full-height shell never jumps.
        if view == PopupView::Usage {
            return self.host.max_height();
        }
        let content = self
            .pages
            .get(&view)
            .map(|metrics| metrics.content_height.get())
            .filter(|height| *height > 1.0)
            .unwrap_or(INITIAL_HEIGHT - chrome);
        (content + chrome).clamp(80.0, self.host.max_height())
    }

    /// Advance window motion for this frame. Returns `(width, height, offset)`.
    fn step_motion(&mut self, now: Instant, cx: &mut Context<Self>) -> (f32, f32, f32) {
        if let Phase::Opening(started) = self.host.phase {
            // Advance the slide by at most one long frame per frame: a stalled
            // first paint must not skip the entrance.
            if let Some(last) = self.host.slide_frame {
                let stall = now
                    .saturating_duration_since(last)
                    .saturating_sub(SLIDE_MAX_FRAME_STEP);
                if !stall.is_zero() {
                    self.host.phase = Phase::Opening(started + stall);
                }
            }
            self.host.slide_frame = Some(now);
        } else {
            self.host.slide_frame = None;
        }
        let target_h = self.target_height(self.pager.current);
        let target_w = self.target_width(self.pager.current);
        let animate = super::animations_enabled(&self.ui)
            && self.host.visible()
            && self.host.phase != Phase::Prewarm;
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
            Phase::Prewarm if self.host.prewarm_exposed() => travel - 1.0,
            Phase::Hidden | Phase::Prewarm | Phase::Open => 0.0,
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
            if let Some(backdrop) = &mut self.host.backdrop {
                backdrop.update(
                    x * scale,
                    y * scale,
                    width * scale,
                    height * scale,
                    crate::popup::corner_radius_dip() as f32 * scale,
                );
            }
            let viewport = window.viewport_size();
            let window_w = (f32::from(viewport.width) * scale).round() as i32;
            let window_h = (f32::from(viewport.height) * scale).round() as i32;
            if self.host.phase == Phase::Prewarm {
                // Render everything, show at most the capsule's edge pixel at
                // the screen edge, publish no hit bounds.
                let region = if self.host.prewarm_exposed() {
                    let y = (((y + height * 0.5) * scale).round() as i32).clamp(0, window_h - 1);
                    (window_w - 1, y, window_w, y + 1, 0)
                } else {
                    (0, 0, 0, 0, 0)
                };
                if self.host.last_region != Some(region) {
                    self.host.last_region = Some(region);
                    let (left, top, right, bottom, _) = region;
                    super::win32::set_region(
                        hwnd,
                        Some(super::win32::Rect {
                            left,
                            top,
                            right,
                            bottom,
                        }),
                        0,
                    );
                }
                return;
            }
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

    pub(super) fn use_reset(&mut self, provider: ProviderId, cx: &mut Context<Self>) {
        let Some(instance) = self.ui.instance(provider).filter(|i| i.enabled).cloned() else {
            return;
        };
        if !self.reset_busy.insert(provider) {
            return;
        }
        self.reset_confirm = None;
        let revision = instance.credentials_revision;
        let runtime_key = instance.runtime_key();
        let events = self.state.worker_events_tx.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { crate::banked_reset::consume(&instance) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.reset_busy.remove(&provider);
                // A credential edit/removal while the request ran must not
                // attach the previous account's result to the new profile.
                if this
                    .ui
                    .instance(provider)
                    .is_some_and(|i| i.runtime_key() == runtime_key && i.enabled)
                {
                    if let Ok((_, Ok(limits))) = &result {
                        let _ = events.send(WorkerEvent::ProviderLimitsUpdated(
                            provider,
                            revision,
                            limits.clone(),
                        ));
                    }
                    let message = match result {
                        Ok((crate::banked_reset::Outcome::Reset, Err(_))) => {
                            crate::banked_reset::Status::RefreshFailed
                        }
                        Ok((outcome, _)) => crate::banked_reset::Status::Outcome(outcome),
                        Err(error) => crate::banked_reset::Status::from_error(error),
                    };
                    this.reset_status.insert(provider, message);
                    // Include sibling instances sharing the account. Normal
                    // worker publication updates tray, Home and provider tabs.
                    this.refresh(cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn refresh(&mut self, cx: &mut Context<Self>) {
        self.keys.mark_stale();
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
        crate::settings_window::open();
    }

    pub(super) fn install_update(&mut self) {
        std::thread::spawn(|| {
            if let Err(error) = crate::updater::apply_pending_update() {
                eprintln!("failed to apply update: {error:#}");
                crate::notifications::show_error(
                    crate::i18n::tr("update-failed"),
                    &format!("{error:#}"),
                );
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
        // Disk I/O and the settings broadcast run on the serial settings
        // writer, off the render thread and ordered with every other write.
        crate::settings_window::persist_update(settings_tx, update);
    }

    // ----- snapshots ------------------------------------------------------------

    /// Stale-while-revalidate usage aggregation on the background executor.
    pub(super) fn snapshot(
        &mut self,
        slot: SnapshotSlot,
        key: String,
        enabled: Vec<ProviderId>,
        query: impl FnOnce(&ProviderLimits, &[ProviderId]) -> OverviewSnapshot + Send + 'static,
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
            self.reveal_scrollbar(view, SCROLLBAR_LINGER, cx);
            cx.notify();
        }
    }

    /// Keeps the overlay scrollbar visible for `duration`, then lets it fade.
    fn reveal_scrollbar(&mut self, view: PopupView, duration: Duration, cx: &mut Context<Self>) {
        let until = Instant::now() + duration;
        let metrics = self.pages.entry(view).or_default();
        if metrics.reveal_until.is_some_and(|at| at >= until) {
            return;
        }
        metrics.reveal_until = Some(until);
        self.scrollbar_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(duration).await;
            let _ = this.update(cx, |_, cx| cx.notify());
        }));
    }
}

pub(super) fn view_key(view: PopupView) -> String {
    view.key()
}

fn closing_offset(started: Instant, from: f32, width: f32, margin: f32) -> f32 {
    let travel = width + margin;
    let progress = started.elapsed().as_secs_f64() / CLOSE_ANIMATION.as_secs_f64();
    let eased = crate::popup::ease_exit(progress) as f32;
    from + (travel - from) * eased
}

pub(super) fn system_dark(window: &Window) -> bool {
    matches!(
        window.appearance(),
        gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
    )
}

/// Seed identical to the background bridge so the very first frame already
/// reflects persisted settings, before the bridge publishes.
fn initial_ui_state(state: &AppState) -> UiState {
    let settings = &state.settings;
    let mut ui = UiState {
        provider_errors: state
            .startup_provider_errors
            .iter()
            .map(|(provider, error)| (*provider, UiState::error_for_ui(error)))
            .collect(),
        last_activation: format_last_activation(&RateLimits::default(), state.last_activation_at),
        update_version: state
            .updates
            .available_update()
            .map(|update| update.version),
        ..UiState::popup_layout_from_settings(settings)
    };
    ui.apply_settings(settings);
    ui
}

// ----- render -------------------------------------------------------------------

impl Render for PopupRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let language = crate::i18n::current_language();
        if self.language != language {
            self.language = language;
            // Tooltips retain formatted chart text while hovered. Rebuild on
            // the next pointer event rather than retaining the old language.
            self.tip = None;
        }
        if !self.host.visible() {
            return div().id("popup-root").size_full();
        }
        // Settings previews a theme by swapping a shared slot, then
        // refreshing every window.
        if self.preview_generation != crate::vscode_themes::preview_generation() {
            self.refresh_palette(window);
        }
        let now = Instant::now();
        self.sync_key_pin();
        self.fx.begin_frame(super::animations_enabled(&self.ui));
        self.profile_height = self.fx.value(
            fx::key("profile-grid-height"),
            self.target_profile_strip_height(),
            fx::FAST,
        );
        let kit_theme = crate::settings_window::theme::Theme::new(
            self.palette.dark,
            self.accent,
            crate::settings_window::theme::Fonts {
                text: self.palette.font_family.clone(),
                ..self.kit_fonts.clone()
            },
        );
        self.kit.begin_frame(kit_theme, window);
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

        // Switcher changes with a different card layout fade the page in so
        // the remeasured height is not seen as cards jumping.
        let profile_layout = match self.pager.current {
            PopupView::Group(driver) if self.show_profile_strip() => {
                model::selected_group_member(&ui, driver).map_or_else(String::new, |member| {
                    format!(
                        "{}|{}",
                        member.id(),
                        instance_layout_key(
                            limits.get(member),
                            ui.has_provider_error(member),
                            ui.show_used_percentage,
                            ui.show_usage_pace,
                        )
                    )
                })
            }
            _ => String::new(),
        };
        if profile_layout != self.profile_layout {
            if !self.profile_layout.is_empty() && !profile_layout.is_empty() {
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
        if self.profile_height > 0.001 {
            chrome.push(
                div()
                    .id("popup-pinned-profiles")
                    .absolute()
                    .right(px(chrome_right))
                    .bottom(px(CHROME_INSET + self.footer_height()))
                    .w(px(chrome_width))
                    .h(px(self.profile_height))
                    .overflow_hidden()
                    .child(self.render_profile_strip(window, cx))
                    .into_any_element(),
            );
        }

        #[cfg(windows)]
        let frosted = self
            .host
            .backdrop
            .as_ref()
            .is_some_and(|backdrop| backdrop.supports(palette.material));
        #[cfg(not(windows))]
        let frosted = false;
        let capsule = div()
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
            // A rounded composition visual below GPUI supplies blur only
            // inside the capsule. This tint leaves that backdrop visible.
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(radius))
                    .bg(palette.capsule_background(frosted)),
            )
            .child(shell)
            // A hairline keeps the capsule edge crisp against any wallpaper.
            .children(palette.borders.then(|| {
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(radius))
                    .border_1()
                    .border_color(if palette.dark {
                        theme::rgba8(255, 255, 255, 0x14)
                    } else {
                        theme::rgba8(0, 0, 0, 0x12)
                    })
            }));
        // Paint tooltips after pages, pinned chrome and any other deferred UI.
        // Their coordinates are window-relative; render_tip still clamps them
        // to the capsule so the native region cannot cut off the bubble.
        let tip = self
            .render_tip(capsule_w, capsule_h, window, cx)
            .map(|tip| gpui::deferred(tip).with_priority(usize::MAX));

        if self.host.phase == Phase::Prewarm {
            self.host.prewarm_frames = self.host.prewarm_frames.saturating_add(1);
            if self.host.prewarm_frames <= PREWARM_FRAMES + PREWARM_EXPOSED_FRAMES {
                window.request_animation_frame();
            }
        }
        if self.fx.is_animating() || (ui.refreshing && self.host.visible()) {
            window.request_animation_frame();
        }
        self.kit.end_frame(window);

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
            .children(tip)
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
        let gap = self.card_gap();
        let presence = std::mem::take(&mut self.body_presence);
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
            .gap(px(gap))
            .p(px(PAGE_PADDING))
            .children(body.into_iter().enumerate().map(|(index, body)| {
                let item = div().flex_none().child(body);
                // A growing item also grows its gap: the negative margin
                // cancels the gap while the item is still collapsed.
                match presence.iter().find(|(at, _)| *at == index) {
                    Some((_, shown)) => item.mb(px(-gap * (1.0 - shown))),
                    None => item,
                }
            }))
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
            let overflowing = metrics.content_height.get() - final_viewport
                > 1.0 / window.scale_factor()
                && max_scroll > 0.5;
            let page_state = self.pages.entry(view).or_default();
            let newly_overflowing = overflowing && !page_state.overflowing;
            page_state.overflowing = overflowing;
            if newly_overflowing {
                self.reveal_scrollbar(view, SCROLLBAR_FLASH, cx);
            }
            page = page.children(self.render_scroll_shades(
                view,
                overflowing && scroll > 0.5,
                overflowing && max_scroll - scroll > 0.5,
            ));
            if overflowing {
                page = page
                    .child(self.render_scrollbar(view, scroll, max_scroll, viewport_height))
                    .child(self.render_scrollbar_zone(view, cx));
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
        let metrics = self.pages.get(&view);
        let scrolling = (scroll - metrics.map_or(0.0, |m| m.scroll_target)).abs() > 0.5;
        let revealed = metrics
            .and_then(|m| m.reveal_until)
            .is_some_and(|until| Instant::now() < until);
        let shown =
            scrolling || revealed || self.hovered(fx::key(("scrollbar-zone", view_key(view))));
        // Hidden at rest; appears quickly and fades out more gently.
        let visible = self.fx.value(
            fx::key(("scrollbar", view_key(view))),
            if shown { 1.0 } else { 0.0 },
            if shown { fx::FAST } else { fx::NORMAL },
        );
        div()
            .absolute()
            .right(px(3.0))
            .top(px(top))
            .w(px(3.0))
            .h(px(thumb))
            .rounded(px(1.5))
            .bg(self.palette.text_tertiary.opacity(0.7 * visible))
    }

    /// Invisible strip along the right edge; pointing at it shows the
    /// scrollbar without revealing it for every pointer move over the page.
    fn render_scrollbar_zone(&self, view: PopupView, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id(ElementId::Name(
                format!("popup-scrollbar-zone-{}", view_key(view)).into(),
            ))
            .absolute()
            .top_0()
            .bottom_0()
            .right_0()
            .w(px(SCROLLBAR_HOVER_ZONE))
            .on_hover(self.hover_listener(fx::key(("scrollbar-zone", view_key(view))), None, cx))
    }

    /// Soft shadows at the viewport edges while content continues beyond
    /// them. Shadows blend with every backdrop material, unlike a tint fade.
    fn render_scroll_shades(&mut self, view: PopupView, above: bool, below: bool) -> Vec<Div> {
        let top = self.fx.toggle(
            fx::key(("scroll-shade-top", view_key(view))),
            above,
            fx::FAST,
        );
        let bottom = self.fx.toggle(
            fx::key(("scroll-shade-bottom", view_key(view))),
            below,
            fx::FAST,
        );
        let strength = if self.palette.dark { 0.32 } else { 0.10 };
        let shade = |alpha: f32| gpui::hsla(0.0, 0.0, 0.0, alpha);
        let mut shades = Vec::new();
        if top > 0.001 {
            shades.push(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(SCROLL_SHADE_HEIGHT))
                    .bg(gpui::linear_gradient(
                        180.0,
                        gpui::linear_color_stop(shade(strength * top), 0.0),
                        gpui::linear_color_stop(shade(0.0), 1.0),
                    )),
            );
        }
        if bottom > 0.001 {
            shades.push(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(px(SCROLL_SHADE_HEIGHT))
                    .bg(gpui::linear_gradient(
                        180.0,
                        gpui::linear_color_stop(shade(0.0), 0.0),
                        gpui::linear_color_stop(shade(strength * bottom), 1.0),
                    )),
            );
        }
        shades
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
                    crate::i18n::tr("something-went-wrong"),
                    crate::i18n::localize_error(error),
                    Severity::Error,
                    &palette,
                )
                .into_any_element(),
            );
        }
        // One account's page shows its error; pages with several accounts
        // show only bars that need a new sign-in, since their headings
        // already mark other errors. Usage reads local logs, so provider
        // errors don't apply to it.
        let (accounts, every_error) = match view {
            PopupView::Usage => (Vec::new(), false),
            PopupView::Home => (ui.enabled_providers(), false),
            other => match self.page_provider(other) {
                Some(provider) => (vec![provider], true),
                None => (tab_members(&ui.instances, other), false),
            },
        };
        self.body_presence.clear();
        for provider in accounts {
            if let Some((bar, shown)) = self.account_bar(view, provider, every_error, cx) {
                if shown < 1.0 {
                    self.body_presence.push((body.len(), shown));
                }
                body.push(bar);
            }
        }
        match view {
            PopupView::Home => body.extend(self.render_home(window, cx)),
            PopupView::Usage => body.push(self.render_usage_page(window, cx)),
            other => body.extend(self.render_provider_page(other, window, cx)),
        }
        if !model::any_provider_enabled(&ui) {
            body.push(
                components::info_bar(
                    crate::i18n::tr("no-providers-enabled"),
                    crate::i18n::tr("turn-one-on-in-settings-providers"),
                    Severity::Informational,
                    &palette,
                )
                .into_any_element(),
            );
        }
        body
    }

    pub(super) fn enabled_spend(&self) -> Vec<ProviderId> {
        model::spend_providers(&self.ui)
    }

    pub(super) fn show_provider_tabs(&self) -> bool {
        model::show_provider_tabs(&self.ui)
    }

    /// The single instance a page is about: the instance tab itself, or the
    /// switcher's selection on a grouped tab. Stacked groups have none.
    pub(super) fn page_provider(&self, view: PopupView) -> Option<ProviderId> {
        match view {
            PopupView::Provider(provider) => Some(provider),
            PopupView::Group(driver) if self.ui.popup_tab_mode == PopupTabMode::GroupedSwitcher => {
                model::selected_group_member(&self.ui, driver)
            }
            _ => None,
        }
    }

    /// The bar for one account: its error, or a warning that its login is
    /// about to end. Login problems carry a button that signs the account in
    /// again into its own folder. `every_error` also shows other errors.
    ///
    /// Bars grow in and collapse out at their measured height; a cleared bar
    /// keeps its last text while it collapses. Returns how shown it is.
    fn account_bar(
        &mut self,
        view: PopupView,
        provider: ProviderId,
        every_error: bool,
        cx: &mut Context<Self>,
    ) -> Option<(AnyElement, f32)> {
        let current = self.account_bar_spec(provider, every_error);
        // Track every account every frame, so a bar that appears later
        // animates in instead of starting fully shown.
        let shown = self.fx.toggle(
            fx::key(("account-bar", view_key(view), provider.id())),
            current.is_some(),
            fx::NORMAL,
        );
        let key = (view, provider);
        let bar = match current {
            Some(spec) => {
                let bar = self.account_bars.entry(key).or_default();
                bar.spec = Some(spec);
                bar
            }
            None if shown > 0.001 => self.account_bars.get_mut(&key)?,
            None => {
                self.account_bars.remove(&key);
                return None;
            }
        };
        let spec = bar.spec.clone()?;
        let measured = Rc::clone(&bar.height);
        let palette = self.palette.clone();
        let action = spec.action.then(|| self.sign_in_button(provider, cx));
        let content = components::info_bar_with_action(
            spec.title,
            spec.message,
            spec.severity,
            &palette,
            action,
        );
        let height = measured.get();
        let probe = canvas(
            move |bounds, window, _| {
                let natural = f32::from(bounds.size.height);
                if measured.get() != natural {
                    measured.set(natural);
                    window.request_animation_frame();
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        let mut element = div()
            .overflow_hidden()
            .opacity(shown)
            .child(div().relative().flex_shrink_0().child(content).child(probe));
        if shown < 1.0 {
            element = element.h(px(height * shown));
        }
        Some((element.into_any_element(), shown))
    }

    /// What the account's bar should say now, if anything.
    fn account_bar_spec(&self, provider: ProviderId, every_error: bool) -> Option<AccountBarSpec> {
        let ui = &self.ui;
        let error = ui.provider_error(provider);
        let notice = ui.instance(provider).and_then(|instance| {
            model::login_notice(instance, error, self.limits.get(provider), Utc::now())
        });
        let name = provider.qualified_name();
        let (title, message, severity) = match (notice, error) {
            (Some(LoginNotice::Expiring { days_left }), _) => (
                crate::i18n::format(
                    "name-login-expires-in-days-left",
                    &[
                        ("name", name.to_string()),
                        ("days_left", days_left.to_string()),
                    ],
                ),
                crate::i18n::tr("sign-in-again-to-keep-limits-updating").to_owned(),
                Severity::Caution,
            ),
            (Some(LoginNotice::SignInNeeded), Some(error)) => (
                crate::i18n::format("name-error", &[("name", name.to_string())]),
                crate::i18n::localize_error(error),
                Severity::Error,
            ),
            (None, Some(error)) if every_error => (
                crate::i18n::format("name-error", &[("name", name.to_string())]),
                crate::i18n::localize_error(error),
                Severity::Error,
            ),
            _ => return None,
        };
        Some(AccountBarSpec {
            title,
            message,
            severity,
            action: notice.is_some(),
        })
    }

    fn sign_in_button(&mut self, provider: ProviderId, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette.clone();
        let hover_id = fx::key(("sign-in-again", provider.id()));
        let hovered = self.hovered(hover_id);
        let hover = self
            .fx
            .toggle(fx::key(("sign-in-again-fx", hover_id)), hovered, fx::FASTER);
        div()
            .id(eid(format!("sign-in-again-{}", provider.id())))
            .h(px(28.0))
            .px(px(12.0))
            .flex()
            .items_center()
            .rounded(px(4.0))
            // WinUI accent buttons fade to 90% on hover.
            .bg(palette.accent.opacity(1.0 - 0.1 * hover))
            .on_hover(self.hover_listener(hover_id, None, cx))
            .on_click(move |_: &gpui::ClickEvent, _, _| {
                crate::settings_window::open_sign_in(provider);
            })
            .child(components::body_strong(
                crate::i18n::tr("sign-in-again"),
                palette.text_on_accent,
            ))
            .into_any_element()
    }

    /// Provider tab content: one instance, or every instance of a stacked
    /// group with a badge + name header per instance.
    fn render_provider_page(
        &mut self,
        view: PopupView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let gap = self.card_gap();
        let members = match self.page_provider(view) {
            Some(provider) => vec![provider],
            None => tab_members(&self.ui.instances, view),
        };
        let stacked = members.len() > 1;
        let ui = Rc::clone(&self.ui);
        let limits = Rc::clone(&self.limits);
        let forced_resets = Rc::clone(&self.forced_resets);
        let show_tabs = self.show_provider_tabs();
        let mut sections = Vec::with_capacity(members.len());
        for (index, provider) in members.into_iter().enumerate() {
            // A stacked page has no page-wide bar for one instance; each
            // section's heading carries its own error marker.
            let error_message = ui.provider_error(provider).map(str::to_owned);
            let options = CardOptions {
                keep_reset_card: self.reset_busy.contains(&provider)
                    || self.reset_status.contains_key(&provider),
                popup_visibility: &ui.popup_visibility,
                surface: PopupSurface::ProviderTab,
                show_provider_tabs: show_tabs,
                include_usage_stats: ui.usage_stats_provider_enabled(provider),
                show_account_name: ui.show_account_name,
                drag_handle: false,
                openrouter_actions: provider.kind() == ProviderKind::OpenRouter,
                provider_error: error_message.as_deref(),
                now: Utc::now(),
            };
            let openrouter = provider.kind() == ProviderKind::OpenRouter;
            // A New key or reveal page replaces this account's section.
            if openrouter && let Some(page) = self.render_key_page(provider, window, cx) {
                sections.push(
                    div()
                        .id(eid(format!("provider-key-page-{}", provider.id())))
                        .child(page)
                        .into_any_element(),
                );
                continue;
            }
            let cards = provider_cards(
                provider,
                index == 0,
                stacked,
                limits.get(provider),
                &forced_resets,
                &options,
            );
            let mut children = self.render_cards(&cards, PopupSurface::ProviderTab, window, cx);
            if openrouter && let Some(keys) = self.render_keys_section(provider, window, cx) {
                children.push(keys);
            }
            sections.push(
                div()
                    .id(eid(format!("provider-section-{}", provider.id())))
                    .flex()
                    .flex_col()
                    .gap(px(gap))
                    .children(children)
                    .into_any_element(),
            );
        }
        sections
    }

    pub(super) fn card_style(&self) -> CardStyle {
        CardStyle {
            show_used_percentage: self.ui.show_used_percentage,
            show_usage_values: self.ui.show_usage_values,
            show_usage_pace: self.ui.show_usage_pace,
            compact: self.ui.compact_usage_cards,
        }
    }

    /// Instance switcher pinned above the footer for a grouped tab.
    fn render_profile_strip(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let PopupView::Group(driver) = self.pager.current else {
            return div();
        };
        let members = self.ui.enabled_instances_of(driver);
        let selected_id = model::selected_group_member(&self.ui, driver);
        let membership_changed = self.profile_members != members;
        if membership_changed {
            self.profile_members = members.clone();
            self.profile_scroll.set_offset(point(px(0.0), px(0.0)));
        }
        if membership_changed || self.profile_selected != selected_id {
            self.profile_selected = selected_id;
            if let Some(index) = members
                .iter()
                .position(|member| Some(*member) == selected_id)
            {
                self.profile_scroll.scroll_to_item(index / PROFILE_COLUMNS);
            }
        }
        let mut grid = div()
            .id("profile-grid-scroll")
            .h_full()
            .w_full()
            .overflow_y_scroll()
            .track_scroll(&self.profile_scroll)
            .flex()
            .flex_col()
            .gap(px(PROFILE_ROW_GAP));
        for (row, selected) in profile_rows(&members, selected_id) {
            let key = fx::key((
                "profiles",
                driver.id(),
                row.iter().map(|member| member.id()).collect::<Vec<_>>(),
            ));
            grid = grid.child(self.profile_row_control(key, row, selected, driver, window, cx));
        }
        div()
            .flex_none()
            .h(px(self.profile_height))
            .px(px(16.0))
            .pt(px(2.0))
            .pb(px(10.0))
            .child(grid)
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
    fn profile_rows_fill_each_row_and_select_only_the_matching_stable_id() {
        for count in [0_usize, 1, 2, 3, 4, 5, 6, 7, 8, 9, 1_000] {
            let members = (0..count).collect::<Vec<_>>();
            let selected = count.checked_sub(1);
            let rows = profile_rows(&members, selected);
            assert_eq!(rows.len(), count.div_ceil(4));
            assert_eq!(
                rows.iter()
                    .flat_map(|(row, _)| row)
                    .copied()
                    .collect::<Vec<_>>(),
                members
            );
            for (row, active) in &rows {
                assert!((1..=4).contains(&row.len()));
                if let Some(index) = active {
                    assert_eq!(Some(row[*index]), selected);
                }
            }
            assert_eq!(
                rows.iter().filter(|(_, active)| active.is_some()).count(),
                usize::from(count > 0)
            );
        }
        let rows = profile_rows(&[5, 4, 3, 2, 1], Some(1));
        assert_eq!(rows[0].1, None);
        assert_eq!(rows[1], (vec![1], Some(0)));
        assert_eq!(profile_grid_height(4), 46.0);
        assert_eq!(profile_grid_height(5), 86.0);
        assert_eq!(profile_grid_height(9), 126.0);
    }

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
