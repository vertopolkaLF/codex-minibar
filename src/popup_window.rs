use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender},
    },
    thread,
    time::Duration,
};

use chrono::{DateTime, Duration as ChronoDuration, Local, Utc};
use windows_reactor::*;

use crate::{
    limits::{
        LimitWindow, OpenRouterAccountSnapshot, PaceTip, ProviderLimits, RateLimits,
        SpendingSummary,
    },
    notifications,
    notifications::LimitNotificationTracker,
    popup,
    provider_registry::{
        LimitSectionKind, additional_limit_brick_id, credits_brick_id, limit_section_brick_id,
        resets_brick_id, spending_brick_id, usage_brick_id,
    },
    settings::{
        AccentColor, AppTheme, NotificationSettings, PopupSurface, PopupVisibility,
        PopupWidgetKind, ProviderKind, Settings, TotalSpendPeriod, TotalSpendPresentation,
        TrayWidget,
    },
    settings_controls::update_accent_button,
    tray::{TrayManager, TrayMenuAction},
    updater::{UpdateController, UpdatePhase},
    worker::{WorkerCommand, WorkerEvent},
};

#[cfg(windows)]
static KEEP_ON_MONITOR_QUEUED: AtomicBool = AtomicBool::new(false);

fn format_activation_at(at: DateTime<Utc>) -> String {
    at.with_timezone(&Local)
        .format("%H:%M:%S %d.%m.%Y")
        .to_string()
}

/// Start of the current 5h window: resets_at minus duration.
fn window_started_at(window: &LimitWindow) -> Option<DateTime<Utc>> {
    match (window.resets_at, window.duration_minutes) {
        (Some(reset), Some(minutes)) => Some(reset - ChronoDuration::minutes(i64::from(minutes))),
        _ => None,
    }
}

fn format_last_activation(limits: &RateLimits, fallback_attempt: Option<DateTime<Utc>>) -> String {
    window_started_at(&limits.primary)
        .or(fallback_attempt)
        .map(format_activation_at)
        .unwrap_or_else(|| "Never".into())
}

fn refresh_all_workers(commands: &[(ProviderKind, Sender<WorkerCommand>)]) -> bool {
    let mut requested = false;
    for (_, commands) in commands {
        requested |= commands.send(WorkerCommand::Refresh).is_ok();
    }
    requested
}

/// Hidden tray popups must not rebuild their WinUI tree on every provider poll.
/// Remounting unmanaged SwapChainPanel/XAML children steadily grows the
/// compositor working set (observed multi-GB after long idle runs).
fn popup_ui_should_publish() -> bool {
    popup::is_visible() || crate::settings_window::is_open()
}

fn publish_popup_ui(set_ui: &AsyncSetState<UiState>, ui: &UiState) {
    if popup_ui_should_publish() {
        set_ui.call(ui.clone());
    }
}

/// Push the latest view state before a show so the first frame is current even
/// after a stretch of suppressed background polls.
fn flush_popup_ui(set_ui: &AsyncSetState<UiState>, ui: &UiState) {
    set_ui.call(ui.clone());
}

/// Shared startup state handed from `main` into the reactor render tree.
pub struct AppState {
    pub settings: Settings,
    /// The sole live rate-limit snapshot. Both the tray and popup read this
    /// store, and worker results replace it atomically before either surface
    /// is repainted.
    pub limits: Mutex<ProviderLimits>,
    pub commands: Mutex<HashMap<ProviderKind, Sender<WorkerCommand>>>,
    pub workers: Mutex<crate::provider::ProviderWorkers>,
    pub worker_events_rx: Mutex<Option<Receiver<WorkerEvent>>>,
    pub worker_events_tx: Sender<WorkerEvent>,
    pub activation_path: std::path::PathBuf,
    pub startup_error: Option<String>,
    /// Last activation attempt loaded from persisted activation state.
    pub last_activation_at: Option<DateTime<Utc>>,
    /// Live settings pushes from the settings window; drained by the tray bridge.
    pub settings_rx: Mutex<Option<Receiver<Settings>>>,
    pub settings_tx: Sender<Settings>,
    pub updates: Arc<UpdateController>,
}

impl AppState {
    fn current_limits(&self) -> ProviderLimits {
        self.limits
            .lock()
            .map(|limits| limits.clone())
            .unwrap_or_default()
    }

    fn replace_limits(&self, provider: ProviderKind, mut limits: RateLimits) {
        let persisted = if let Ok(mut current) = self.limits.lock() {
            // Quota polling must not erase the independently refreshed usage
            // history between its ten-minute scans.
            limits.usage = current.get(provider).usage.clone();
            *current.get_mut(provider) = limits.clone();
            Some(limits)
        } else {
            None
        };
        // Never hold the live UI snapshot while waiting for storage. Usage
        // refreshes can legitimately keep the SQLite writer busy briefly.
        if let Some(limits) = persisted
            && let Err(error) =
                crate::store::with_store(|store| store.save_limits(provider, &limits))
        {
            eprintln!(
                "failed to persist {} limits: {error:#}",
                provider.display_name()
            );
        }
    }

    fn replace_usage(&self, provider: ProviderKind, usage: crate::usage::UsageStatistics) {
        if let Ok(mut current) = self.limits.lock() {
            current.get_mut(provider).usage = usage;
        }
    }

    fn take_worker_events(&self) -> Option<Receiver<WorkerEvent>> {
        self.worker_events_rx.lock().ok()?.take()
    }

    fn worker_commands(&self) -> Vec<(ProviderKind, Sender<WorkerCommand>)> {
        self.commands
            .lock()
            .map(|commands| {
                commands
                    .iter()
                    .map(|(provider, commands)| (*provider, commands.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Applies provider toggles without disturbing workers that remain enabled.
    fn sync_provider_workers(&self, settings: &Settings, restart: &[ProviderKind]) -> Vec<String> {
        let disabled = crate::provider_registry::PROVIDERS
            .iter()
            .map(|descriptor| descriptor.kind)
            .filter(|provider| !settings.providers.is_enabled(*provider))
            .collect::<Vec<_>>();
        let stopped = self.workers.lock().map_or_else(
            |_| Vec::new(),
            |mut workers| {
                disabled
                    .iter()
                    .chain(
                        restart
                            .iter()
                            .filter(|provider| settings.providers.is_enabled(**provider)),
                    )
                    .filter_map(|provider| workers.remove(provider))
                    .collect()
            },
        );
        for worker in stopped {
            worker.shutdown();
        }
        if let Ok(mut commands) = self.commands.lock() {
            commands.retain(|provider, _| {
                settings.providers.is_enabled(*provider) && !restart.contains(provider)
            });
        }
        if let Ok(mut limits) = self.limits.lock() {
            for provider in &disabled {
                *limits.get_mut(*provider) = RateLimits::default();
            }
        }

        let mut errors = Vec::new();
        for provider in crate::provider_registry::PROVIDERS
            .iter()
            .map(|descriptor| descriptor.kind)
        {
            if !settings.providers.is_enabled(provider)
                || self
                    .workers
                    .lock()
                    .is_ok_and(|workers| workers.contains_key(&provider))
            {
                continue;
            }
            match crate::provider::start_provider_worker(
                provider,
                settings,
                self.activation_path.clone(),
                self.worker_events_tx.clone(),
            ) {
                Ok(worker) => {
                    if let Ok(mut commands) = self.commands.lock() {
                        commands.insert(provider, worker.commands.clone());
                    }
                    if let Ok(mut workers) = self.workers.lock() {
                        workers.insert(provider, worker);
                    }
                }
                Err(error) => errors.push(format!("{}: {error:#}", provider.display_name())),
            }
        }
        errors
    }

    pub fn shutdown_worker(&self) {
        if let Ok(mut workers) = self.workers.lock() {
            for (_, worker) in std::mem::take(&mut *workers) {
                worker.shutdown();
            }
        }
        if let Ok(mut commands) = self.commands.lock() {
            commands.clear();
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct UiState {
    theme: AppTheme,
    accent_color: AccentColor,
    animations_enabled: bool,
    last_activation: String,
    error: Option<String>,
    /// Changes for every successful worker sample.  Rate-limit data lives only
    /// in `AppState`, but this revision makes that external snapshot observable
    /// to the reactive render loop even when all other view metadata is equal.
    limits_revision: u64,
    /// A refresh has been requested and is waiting for the worker's next sample.
    refreshing: bool,
    show_used_percentage: bool,
    show_usage_pace: bool,
    popup_visibility: PopupVisibility,
    show_total_spend_on_all_tab: bool,
    total_spend_presentation: TotalSpendPresentation,
    total_spend_period: TotalSpendPeriod,
    show_account_name: bool,
    codex_enabled: bool,
    claude_enabled: bool,
    cursor_enabled: bool,
    opencode_zen_enabled: bool,
    opencode_go_enabled: bool,
    openrouter_enabled: bool,
    opencode_zen_credentials_revision: u64,
    opencode_go_credentials_revision: u64,
    openrouter_credentials_revision: u64,
    popup_order: Vec<PopupWidgetKind>,
    use_colored_provider_icons: bool,
    replace_chatgpt_logo_with_codex: bool,
    codex_path: Option<std::path::PathBuf>,
    claude_path: Option<std::path::PathBuf>,
    cursor_path: Option<std::path::PathBuf>,
    update_version: Option<String>,
}

/// Sections of the settings window. Keeping this as a small enum makes the
/// sidebar stable while each page grows independently.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SettingsTab {
    #[default]
    General,
    Tray,
    Notifications,
    Advanced,
}

#[allow(dead_code)]
impl SettingsTab {
    fn index(self) -> u8 {
        match self {
            Self::General => 0,
            Self::Tray => 1,
            Self::Notifications => 2,
            Self::Advanced => 3,
        }
    }

    fn tag(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Tray => "tray",
            Self::Notifications => "notifications",
            Self::Advanced => "advanced",
        }
    }

    fn from_tag(tag: &str) -> Self {
        match tag {
            "tray" => Self::Tray,
            "notifications" => Self::Notifications,
            "advanced" => Self::Advanced,
            _ => Self::General,
        }
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            theme: AppTheme::Auto,
            accent_color: AccentColor::Windows,
            animations_enabled: true,
            last_activation: "Never".into(),
            error: None,
            limits_revision: 0,
            refreshing: false,
            show_used_percentage: false,
            show_usage_pace: true,
            popup_visibility: PopupVisibility::build_defaults(),
            show_total_spend_on_all_tab: true,
            total_spend_presentation: TotalSpendPresentation::default(),
            total_spend_period: TotalSpendPeriod::default(),
            show_account_name: false,
            codex_enabled: true,
            claude_enabled: false,
            cursor_enabled: false,
            opencode_zen_enabled: false,
            opencode_go_enabled: false,
            opencode_zen_credentials_revision: 0,
            opencode_go_credentials_revision: 0,
            openrouter_enabled: false,
            openrouter_credentials_revision: 0,
            popup_order: PopupWidgetKind::default_order(),
            use_colored_provider_icons: false,
            replace_chatgpt_logo_with_codex: false,
            codex_path: None,
            claude_path: None,
            cursor_path: None,
            update_version: None,
        }
    }
}

impl UiState {
    /// Shows an error in the popup and records it once per distinct message.
    /// Polling can retry many times a second; duplicate popup errors would make
    /// the diagnostic log unreadable.
    fn set_popup_error(&mut self, error: impl Into<String>) {
        let error = error.into();
        if self.error.as_deref() != Some(error.as_str()) {
            crate::logger::info(format!("Popup error: {error}"));
        }
        self.error = Some(error);
    }
}

impl UiState {
    /// Marks the shared rate-limit snapshot as changed so `AsyncSetState` does
    /// not discard an otherwise identical UI state as a no-op.
    fn observe_limits_update(&mut self) {
        self.limits_revision = self.limits_revision.wrapping_add(1);
    }
}

/// The popup either shows the combined feed or one enabled provider.
///
/// This intentionally stays ephemeral: it is a view choice for the currently
/// open popup, not an application preference that should survive a restart.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PopupView {
    #[default]
    All,
    Codex,
    Claude,
    Cursor,
    OpenCodeZen,
    OpenCodeGo,
    OpenRouter,
}

impl PopupView {
    const fn from_provider(provider: ProviderKind) -> Self {
        match provider {
            ProviderKind::Codex => Self::Codex,
            ProviderKind::Claude => Self::Claude,
            ProviderKind::Cursor => Self::Cursor,
            ProviderKind::OpenCodeZen => Self::OpenCodeZen,
            ProviderKind::OpenCodeGo => Self::OpenCodeGo,
            ProviderKind::OpenRouter => Self::OpenRouter,
        }
    }

    const fn provider(self) -> Option<ProviderKind> {
        match self {
            Self::All => None,
            Self::Codex => Some(ProviderKind::Codex),
            Self::Claude => Some(ProviderKind::Claude),
            Self::Cursor => Some(ProviderKind::Cursor),
            Self::OpenCodeZen => Some(ProviderKind::OpenCodeZen),
            Self::OpenCodeGo => Some(ProviderKind::OpenCodeGo),
            Self::OpenRouter => Some(ProviderKind::OpenRouter),
        }
    }

    fn order(self, provider_order: &[ProviderKind]) -> i32 {
        match self {
            Self::All => 0,
            other => {
                let provider = other.provider().expect("provider view");
                1 + provider_order
                    .iter()
                    .position(|item| *item == provider)
                    .unwrap_or(0) as i32
            }
        }
    }
}

fn enabled_popup_views(
    popup_order: &[PopupWidgetKind],
    codex: bool,
    claude: bool,
    cursor: bool,
    opencode_zen: bool,
    opencode_go: bool,
    openrouter: bool,
) -> Vec<PopupView> {
    let mut views = vec![PopupView::All];
    for widget in popup_order {
        let Some(provider) = widget.as_provider() else {
            continue;
        };
        let enabled = match provider {
            ProviderKind::Codex => codex,
            ProviderKind::Claude => claude,
            ProviderKind::Cursor => cursor,
            ProviderKind::OpenCodeZen => opencode_zen,
            ProviderKind::OpenCodeGo => opencode_go,
            ProviderKind::OpenRouter => openrouter,
        };
        if enabled {
            views.push(PopupView::from_provider(provider));
        }
    }
    views
}

fn provider_order_from_popup(popup_order: &[PopupWidgetKind]) -> Vec<ProviderKind> {
    popup_order
        .iter()
        .filter_map(|widget| widget.as_provider())
        .collect()
}

fn provider_order_key(providers: &[ProviderKind]) -> String {
    providers
        .iter()
        .map(|provider| provider.id())
        .collect::<Vec<_>>()
        .join("-")
}

fn popup_order_key(popup_order: &[PopupWidgetKind]) -> String {
    popup_order
        .iter()
        .map(|widget| widget.id())
        .collect::<Vec<_>>()
        .join("-")
}

fn provider_is_enabled(
    provider: ProviderKind,
    codex: bool,
    claude: bool,
    cursor: bool,
    opencode_zen: bool,
    opencode_go: bool,
    openrouter: bool,
) -> bool {
    match provider {
        ProviderKind::Codex => codex,
        ProviderKind::Claude => claude,
        ProviderKind::Cursor => cursor,
        ProviderKind::OpenCodeZen => opencode_zen,
        ProviderKind::OpenCodeGo => opencode_go,
        ProviderKind::OpenRouter => openrouter,
    }
}

fn total_spend_provider_count(
    codex: bool,
    claude: bool,
    cursor: bool,
    opencode_zen: bool,
    opencode_go: bool,
) -> usize {
    usize::from(codex)
        + usize::from(claude)
        + usize::from(cursor)
        + usize::from(opencode_zen)
        + usize::from(opencode_go)
}

fn visible_popup_widgets(
    popup_order: &[PopupWidgetKind],
    show_total_spend: bool,
    popup_visibility: &PopupVisibility,
    codex: bool,
    claude: bool,
    cursor: bool,
    opencode_zen: bool,
    opencode_go: bool,
    openrouter: bool,
) -> Vec<PopupWidgetKind> {
    popup_order
        .iter()
        .copied()
        .filter(|widget| match widget {
            PopupWidgetKind::TotalSpend => show_total_spend,
            other => other.as_provider().is_some_and(|provider| {
                provider_is_enabled(
                    provider,
                    codex,
                    claude,
                    cursor,
                    opencode_zen,
                    opencode_go,
                    openrouter,
                ) && popup_visibility.provider_visible_on_all(provider)
            }),
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PagerDirection {
    Forward,
    Backward,
}

const PAGER_ANIMATION_DURATION: Duration = Duration::from_millis(250);

impl PagerDirection {
    fn between(from: PopupView, to: PopupView, provider_order: &[ProviderKind]) -> Self {
        if to.order(provider_order) > from.order(provider_order) {
            Self::Forward
        } else {
            Self::Backward
        }
    }

    const fn outgoing_offset(self) -> f32 {
        match self {
            Self::Forward => -(popup::POPUP_WIDTH as f32),
            Self::Backward => popup::POPUP_WIDTH as f32,
        }
    }

    const fn incoming_offset(self) -> f32 {
        -self.outgoing_offset()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PagerState {
    current: PopupView,
    outgoing: Option<PopupView>,
    pending: Option<PopupView>,
    direction: PagerDirection,
    animation_id: u64,
    provider_order: Vec<ProviderKind>,
}

impl Default for PagerState {
    fn default() -> Self {
        Self {
            current: PopupView::All,
            outgoing: None,
            pending: None,
            direction: PagerDirection::Forward,
            animation_id: 0,
            provider_order: ProviderKind::default_order(),
        }
    }
}

#[derive(Clone, Debug)]
enum PagerAction {
    Select(PopupView),
    SetProviderOrder(Vec<ProviderKind>),
    AnimationFinished(u64),
}

fn reduce_pager(mut state: PagerState, action: PagerAction) -> PagerState {
    match action {
        PagerAction::SetProviderOrder(order) => {
            if state.provider_order != order {
                state.provider_order = order;
            }
            state
        }
        PagerAction::Select(target) => {
            if state.outgoing.is_some() {
                if target != state.current {
                    state.pending = Some(target);
                }
                return state;
            }
            if target == state.current {
                return state;
            }
            state.direction = PagerDirection::between(state.current, target, &state.provider_order);
            state.outgoing = Some(state.current);
            state.current = target;
            state.pending = None;
            state.animation_id = state.animation_id.wrapping_add(1);
            state
        }
        PagerAction::AnimationFinished(animation_id) => {
            if state.animation_id != animation_id || state.outgoing.is_none() {
                return state;
            }
            state.outgoing = None;
            let pending = state.pending.take();
            if let Some(target) = pending
                && target != state.current
            {
                state.direction =
                    PagerDirection::between(state.current, target, &state.provider_order);
                state.outgoing = Some(state.current);
                state.current = target;
                state.animation_id = state.animation_id.wrapping_add(1);
            }
            state
        }
    }
}

/// Semantic identity for each independently reconciled popup section.
///
/// Keeping these identities separate from their position prevents the WinUI
/// reconciler from reusing a Monthly or reset card as a Plus-plan card when the
/// response changes the shape of the popup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PopupSection {
    Error,
    Monthly,
    FiveHour,
    Weekly,
    UsageStatistics,
    BankedResets,
    Credits,
}

impl PopupSection {
    const fn key(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Monthly => "monthly",
            Self::FiveHour => "five-hour",
            Self::Weekly => "weekly",
            Self::UsageStatistics => "usage-statistics",
            Self::BankedResets => "banked-resets",
            Self::Credits => "credits",
        }
    }
}

fn popup_sections(limits: &RateLimits, has_error: bool) -> Vec<PopupSection> {
    let mut sections = Vec::with_capacity(6);
    if has_error {
        sections.push(PopupSection::Error);
    }
    if limits.is_free_plan() {
        if !limits.secondary.is_empty() {
            sections.push(PopupSection::Monthly);
        }
    } else {
        if !limits.five_hour_disabled() {
            sections.push(PopupSection::FiveHour);
        }
        if !limits.secondary.is_empty() {
            sections.push(PopupSection::Weekly);
        }
    }
    if limits.available_reset_count() > 0 {
        sections.push(PopupSection::BankedResets);
    }
    if limits.usage.has_data() {
        sections.push(PopupSection::UsageStatistics);
    }
    if credits_display_value(limits).is_some() {
        sections.push(PopupSection::Credits);
    }
    sections
}

fn limit_section_kind(section: PopupSection) -> Option<LimitSectionKind> {
    match section {
        PopupSection::FiveHour => Some(LimitSectionKind::FiveHour),
        PopupSection::Weekly => Some(LimitSectionKind::Weekly),
        PopupSection::Monthly => Some(LimitSectionKind::Monthly),
        _ => None,
    }
}

fn section_brick_id(provider: ProviderKind, section: PopupSection) -> Option<String> {
    match section {
        PopupSection::BankedResets => Some(resets_brick_id(provider)),
        PopupSection::UsageStatistics => Some(usage_brick_id(provider)),
        PopupSection::Credits => Some(credits_brick_id(provider)),
        PopupSection::Error => None,
        limit_section => limit_section_kind(limit_section)
            .and_then(|kind| limit_section_brick_id(provider, kind)),
    }
}

fn popup_visibility_key(visibility: &PopupVisibility) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    visibility.hash(&mut hasher);
    hasher.finish()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WidgetDragState {
    active: PopupWidgetKind,
    over: PopupWidgetKind,
}

fn persist_popup_order(
    settings_tx: Sender<Settings>,
    set_ui: AsyncSetState<UiState>,
    mut ui: UiState,
    next_order: Vec<PopupWidgetKind>,
) {
    ui.popup_order = next_order.clone();
    set_ui.call(ui);
    crate::settings_window::persist_update(settings_tx, move |settings| {
        settings.popup_order = next_order;
        settings.normalize_popup_order();
    });
}

fn persist_total_spend_period(
    settings_tx: Sender<Settings>,
    set_ui: AsyncSetState<UiState>,
    mut ui: UiState,
    period: TotalSpendPeriod,
) {
    if ui.total_spend_period == period {
        return;
    }
    ui.total_spend_period = period;
    set_ui.call(ui);
    crate::settings_window::persist_update(settings_tx, move |settings| {
        settings.total_spend_period = period;
    });
}

fn commit_widget_drag(
    settings_tx: Sender<Settings>,
    set_ui: AsyncSetState<UiState>,
    ui: UiState,
    drag: WidgetDragState,
    set_drag: SetState<Option<WidgetDragState>>,
) {
    // PointerReleased can hit both the section catcher and the page body in one
    // gesture; only the first commit may mutate order.
    thread_local! {
        static COMMITTING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    if COMMITTING.with(|flag| flag.replace(true)) {
        return;
    }

    set_drag.call(None);
    if drag.active == drag.over {
        COMMITTING.with(|flag| flag.set(false));
        return;
    }
    let show_total_spend = ui.show_total_spend_on_all_tab
        && total_spend_provider_count(
            ui.codex_enabled,
            ui.claude_enabled,
            ui.cursor_enabled,
            ui.opencode_zen_enabled,
            ui.opencode_go_enabled,
        ) > 1;
    let mut scratch = Settings {
        popup_order: ui.popup_order.clone(),
        providers: crate::settings::ProviderSettings::from_enabled(
            crate::provider_registry::PROVIDERS
                .iter()
                .filter(|descriptor| match descriptor.kind {
                    ProviderKind::Codex => ui.codex_enabled,
                    ProviderKind::Claude => ui.claude_enabled,
                    ProviderKind::Cursor => ui.cursor_enabled,
                    ProviderKind::OpenCodeZen => ui.opencode_zen_enabled,
                    ProviderKind::OpenCodeGo => ui.opencode_go_enabled,
                    ProviderKind::OpenRouter => ui.openrouter_enabled,
                })
                .map(|descriptor| descriptor.kind),
        ),
        show_total_spend_on_all_tab: ui.show_total_spend_on_all_tab,
        ..Settings::default()
    };
    if !scratch.move_popup_widget(drag.active, drag.over, show_total_spend) {
        COMMITTING.with(|flag| flag.set(false));
        return;
    }
    persist_popup_order(settings_tx, set_ui, ui, scratch.popup_order);
    COMMITTING.with(|flag| flag.set(false));
}

fn drag_handle(
    widget: PopupWidgetKind,
    color_scheme: ColorScheme,
    drag: &Option<WidgetDragState>,
    set_drag: SetState<Option<WidgetDragState>>,
) -> Element {
    let idle = popup_chrome_icon_color(color_scheme, false);
    let active = drag.as_ref().is_some_and(|state| state.active == widget);
    let set_on_press = set_drag.clone();
    relative_panel::<Vec<Element>>(vec![
        border(Element::Empty)
            .background(ThemeRef::SubtleFill)
            .opacity(if active { 1.0 } else { 0.0 })
            .corner_radius(4.0)
            .relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .relative_align_bottom()
            .into(),
        crate::icons::element("fluent-drag", 14.0, idle)
            .relative_align_h_center()
            .relative_align_v_center()
            .into(),
    ])
    .tooltip("Drag to reorder")
    .width(REORDER_BUTTON_SIZE)
    .height(REORDER_BUTTON_SIZE)
    .min_width(REORDER_BUTTON_SIZE)
    .min_height(REORDER_BUTTON_SIZE)
    .max_width(REORDER_BUTTON_SIZE)
    .max_height(REORDER_BUTTON_SIZE)
    .background(Color::transparent())
    .on_pointer_pressed(move |_: PointerEventInfo| {
        set_on_press.call(Some(WidgetDragState {
            active: widget,
            over: widget,
        }));
    })
    .with_key(format!("drag-handle-{}", widget.id()))
    .into()
}

fn with_widget_drop_target(
    widget: PopupWidgetKind,
    content: Element,
    drag: &Option<WidgetDragState>,
    set_drag: SetState<Option<WidgetDragState>>,
    settings_tx: Sender<Settings>,
    set_ui: AsyncSetState<UiState>,
    ui: UiState,
) -> Element {
    let is_active = drag.as_ref().is_some_and(|state| state.active == widget);
    let is_over = drag.as_ref().is_some_and(|state| state.over == widget);
    let show_outline = is_over && !is_active;
    let dragging = drag.is_some();
    let set_on_enter = set_drag.clone();
    let set_on_release = set_drag.clone();
    let drag_for_enter = drag.clone();
    let drag_for_release = drag.clone();

    // Visual ring only — null fill so it does not steal hits on its own.
    let outline: Element = border(Element::Empty)
        .border_thickness(Thickness::uniform(1.0))
        .border_brush(ThemeRef::Accent)
        .corner_radius(6.0)
        .opacity(if show_outline { 1.0 } else { 0.0 })
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Stretch)
        .grid_column(0)
        .grid_row(0)
        .into();

    let mut layers: Vec<Element> = vec![
        content
            .opacity(if is_active { 0.55 } else { 1.0 })
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .grid_column(0)
            .grid_row(0),
        outline,
    ];

    // While dragging, a transparent full-size catcher matches the highlight
    // zone (header + cards) so release on the title row commits the drop.
    // WinUI hit-tests Transparent backgrounds; null backgrounds do not.
    if dragging {
        layers.push(
            border(Element::Empty)
                .background(Color::transparent())
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .vertical_alignment(VerticalAlignment::Stretch)
                .grid_column(0)
                .grid_row(0)
                .on_pointer_entered(move |_: PointerEventInfo| {
                    let Some(current) = drag_for_enter.clone() else {
                        return;
                    };
                    if current.over == widget {
                        return;
                    }
                    set_on_enter.call(Some(WidgetDragState {
                        active: current.active,
                        over: widget,
                    }));
                })
                .on_pointer_released(move |_: PointerEventInfo| {
                    let Some(current) = drag_for_release.clone() else {
                        return;
                    };
                    // This catcher covers the whole section (header + body), so
                    // the drop target is always `widget` — do not trust a possibly
                    // stale `over` captured before the last pointer-enter update.
                    commit_widget_drag(
                        settings_tx.clone(),
                        set_ui.clone(),
                        ui.clone(),
                        WidgetDragState {
                            active: current.active,
                            over: widget,
                        },
                        set_on_release.clone(),
                    );
                })
                .with_key(format!("drop-catcher-{}", widget.id()))
                .into(),
        );
    }

    grid(layers)
        .columns([GridLength::Star(1.0)])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch)
        // Keep the host identity stable across highlight toggles so a remount
        // cannot swallow the in-flight pointer release.
        .with_key(format!("drop-target-{}", widget.id()))
        .into()
}

fn provider_cards(
    provider: ProviderKind,
    is_first: bool,
    limits: &RateLimits,
    show_used_percentage: bool,
    show_usage_pace: bool,
    popup_visibility: &PopupVisibility,
    surface: PopupSurface,
    show_provider_tabs: bool,
    show_account_name: bool,
    color_scheme: ColorScheme,
    drag_handle: Option<Element>,
) -> Vec<Element> {
    let (monthly_label, primary_label, secondary_label) = match provider {
        ProviderKind::Cursor => ("Auto + Composer", "Auto + Composer", "Auto + Composer"),
        ProviderKind::OpenRouter => ("Spending", "Spending", "Spending"),
        _ => ("Monthly", "5h Session", "Weekly"),
    };
    let mut trailing: Vec<Element> = Vec::new();
    if show_account_name {
        if let Some(name) = limits.account_name.as_ref() {
            trailing.push(
                caption(name.clone())
                    .foreground(ThemeRef::TertiaryText)
                    .horizontal_alignment(HorizontalAlignment::Right)
                    .vertical_alignment(VerticalAlignment::Center)
                    .into(),
            );
        }
    }
    if let Some(handle) = drag_handle {
        trailing.push(handle);
    }
    let mut title_parts: Vec<Element> = vec![body_strong(provider.display_name())
        .foreground(ThemeRef::SecondaryText)
        .vertical_alignment(VerticalAlignment::Center)
        .into()];
    if let Some(plan) = limits
        .plan_type
        .as_deref()
        .filter(|plan| !plan.trim().is_empty())
    {
        title_parts.push(
            text_block(capitalize_plan_name(plan))
                .font_weight(400)
                .foreground(ThemeRef::TertiaryText)
                .vertical_alignment(VerticalAlignment::Center)
                .into(),
        );
    }
    let title_row = hstack(title_parts)
        .spacing(4.0)
        .vertical_alignment(VerticalAlignment::Center)
        .grid_column(0);
    let mut heading_cells: Vec<Element> = vec![title_row.into()];
    if !trailing.is_empty() {
        heading_cells.push(
            hstack(trailing)
                .spacing(4.0)
                .horizontal_alignment(HorizontalAlignment::Right)
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(1)
                .into(),
        );
    }
    let mut cards: Vec<Element> = vec![
        grid(heading_cells)
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .margin(Thickness {
            left: 4.0,
            top: if is_first { 0.0 } else { 8.0 },
            right: 4.0,
            bottom: 2.0,
        })
        .with_key(format!(
            "{}-heading-{}",
            provider.display_name(),
            if is_first { "first" } else { "rest" }
        ))
        .into(),
    ];
    if provider == ProviderKind::OpenRouter {
        let spending_visible = popup_visibility.is_visible(
            &spending_brick_id(provider),
            surface,
            show_provider_tabs,
        );
        if spending_visible {
            if !limits.openrouter_accounts.is_empty() {
                // Nest each account as its own keyed strip. A flat list of headings
                // + key cards lets WinUI recycle siblings across account boundaries
                // when membership flickers (keys loading/failing), so TEST2 can
                // visually land under Pixelscan. Nested strips keep heading+keys
                // glued together; remount the strip when its key set changes.
                for account in &limits.openrouter_accounts {
                    let mut account_strip: Vec<Element> = Vec::new();
                    account_strip.push(
                        openrouter_account_heading(account)
                            .with_key(format!("{}-account-heading", account.id)),
                    );
                    let mut key_identity = String::new();
                    for (index, api_key) in account.api_keys.iter().enumerate() {
                        let title = api_key
                            .label
                            .as_deref()
                            .map(str::trim)
                            .filter(|label| !label.is_empty())
                            .map(str::to_owned)
                            .unwrap_or_else(|| format!("Key {}", index + 1));
                        let masked = api_key.masked_key.as_deref().unwrap_or("");
                        if !key_identity.is_empty() {
                            key_identity.push('\u{1f}');
                        }
                        key_identity.push_str(&api_key.id);
                        key_identity.push('\u{1e}');
                        key_identity.push_str(&title);
                        key_identity.push('\u{1e}');
                        key_identity.push_str(masked);
                        account_strip.push(
                            spending_card_with_title(
                                title.clone(),
                                api_key.masked_key.as_deref(),
                                &api_key.spending,
                                api_key.has_live_usage,
                                show_used_percentage,
                                color_scheme,
                            )
                            // Identity includes glyph-like content (title/mask) so a
                            // recycled native card cannot keep a neighbor's text.
                            .with_key(format!(
                                "{}-api-{}-{}-{}",
                                account.id, api_key.id, title, masked
                            )),
                        );
                    }
                    cards.push(
                        vstack(account_strip)
                            .spacing(6.0)
                            .with_key(format!(
                                "openrouter-account-strip-{}-{}",
                                account.id, key_identity
                            ))
                            .into(),
                    );
                }
            } else if let Some(spending) = limits.spending.as_ref() {
                cards.push(
                    spending_card(spending, show_used_percentage, color_scheme)
                        .with_key(format!("{}-spending", provider.display_name())),
                );
            }
        }
        return cards;
    }
    // Cursor usage is fetched from a remote CSV export rather than scanned
    // from a local session log. Keep its card visible while that export is
    // still empty or delayed, so the feature does not look like it vanished.
    let usage_brick = usage_brick_id(provider);
    let show_usage_stats = popup_visibility.is_visible(&usage_brick, surface, show_provider_tabs);
    let has_usage_statistics =
        show_usage_stats && (limits.usage.has_data() || provider == ProviderKind::Cursor);
    cards.extend(
        popup_sections(limits, false)
            .into_iter()
            .filter(|section| {
                matches!(
                    section,
                    PopupSection::Monthly | PopupSection::FiveHour | PopupSection::Weekly
                )
            })
            .filter(|section| {
                section_brick_id(provider, *section).is_some_and(|brick_id| {
                    popup_visibility.is_visible(&brick_id, surface, show_provider_tabs)
                })
            })
            .filter_map(|section| {
                let element: Element = match section {
                    PopupSection::Monthly => limit_card(
                        monthly_label,
                        &limits.secondary,
                        show_used_percentage,
                        show_usage_pace,
                        false,
                        color_scheme,
                    ),
                    PopupSection::FiveHour => limit_card(
                        primary_label,
                        &limits.primary,
                        show_used_percentage,
                        show_usage_pace,
                        limits.five_hour_disabled(),
                        color_scheme,
                    ),
                    PopupSection::Weekly => limit_card(
                        secondary_label,
                        &limits.secondary,
                        show_used_percentage,
                        show_usage_pace,
                        false,
                        color_scheme,
                    ),
                    PopupSection::Error => return None,
                    PopupSection::UsageStatistics
                    | PopupSection::BankedResets
                    | PopupSection::Credits => return None,
                };
                Some(element.with_key(format!("{}-{}", provider.display_name(), section.key())))
            }),
    );
    // Claude can return extra windows such as Fable or Opus. They belong with
    // the ordinary limit cards, before banked resets, statistics, or credits.
    let additional_limits = limits.additional_limits.iter().filter_map(|limit| {
        let brick_id = additional_limit_brick_id(provider, &limit.id);
        if !popup_visibility.is_visible(&brick_id, surface, show_provider_tabs) {
            return None;
        }
        Some(
            limit_card(
                &limit.title,
                &limit.window,
                show_used_percentage,
                show_usage_pace,
                false,
                color_scheme,
            )
            .with_key(format!(
                "{}-additional-{}",
                provider.display_name(),
                limit.id
            )),
        )
    });
    cards.extend(additional_limits);
    // Local statistics remain after every rate-limit window.
    if popup_visibility.is_visible(&resets_brick_id(provider), surface, show_provider_tabs)
        && limits.available_reset_count() > 0
    {
        cards.push(
            reset_credits_card(limits)
                .with_key(format!("{}-banked-resets", provider.display_name())),
        );
    }
    if has_usage_statistics {
        cards.push(
            usage_statistics_card(provider, limits)
                .with_key(format!("{}-usage-statistics", provider.display_name())),
        );
    }
    if popup_visibility.is_visible(&credits_brick_id(provider), surface, show_provider_tabs)
        && credits_display_value(limits).is_some()
    {
        cards.push(credits_card(limits).with_key(format!("{}-credits", provider.display_name())));
    }
    cards
}

fn spending_card(
    spending: &SpendingSummary,
    show_used_percentage: bool,
    color_scheme: ColorScheme,
) -> Element {
    spending_card_with_title("SPENDING", None, spending, true, show_used_percentage, color_scheme)
}

fn spending_card_with_title(
    title: impl Into<String>,
    masked_key: Option<&str>,
    spending: &SpendingSummary,
    has_live_usage: bool,
    show_used_percentage: bool,
    color_scheme: ColorScheme,
) -> Element {
    let title = title.into().to_uppercase();
    let mut right_side: Vec<Element> = Vec::new();
    if has_live_usage {
        let used = format_usd(spending.used_microusd as f64 / 1_000_000.0);
        let amount = spending.limit_microusd.map_or_else(
            || used.clone(),
            |limit| format!("{used} / {}", format_usd(limit as f64 / 1_000_000.0)),
        );
        right_side.push(
            text_block(amount)
                .font_weight(600)
                .foreground(ThemeRef::Accent)
                .horizontal_alignment(HorizontalAlignment::Right)
                .into(),
        );
    } else {
        // Unknown usage — never show a bare limit that looks like spend.
        let amount = spending.limit_microusd.map_or_else(
            || "?.??".into(),
            |limit| format!("?.?? / {}", format_usd(limit as f64 / 1_000_000.0)),
        );
        right_side.push(
            text_block(amount)
                .font_weight(600)
                .foreground(ThemeRef::Accent)
                .horizontal_alignment(HorizontalAlignment::Right)
                .into(),
        );
    }
    if let Some(reset) = spending.resets_at {
        right_side.push(
            hstack((
                text_block("Resets in")
                    .foreground(ThemeRef::TertiaryText)
                    .vertical_alignment(VerticalAlignment::Center),
                text_block(format_reset_in(Some(reset)))
                    .vertical_alignment(VerticalAlignment::Center),
            ))
            .spacing(6.0)
            .horizontal_alignment(HorizontalAlignment::Right)
            .into(),
        );
    }

    let mut title_lines: Vec<Element> = vec![caption(title)
        .foreground(ThemeRef::SecondaryText)
        .into()];
    if let Some(masked) = masked_key.map(str::trim).filter(|value| !value.is_empty()) {
        title_lines.push(
            caption(masked.to_owned())
                .foreground(ThemeRef::TertiaryText)
                .into(),
        );
    }
    let left = vstack(title_lines)
        .spacing(2.0)
        .vertical_alignment(VerticalAlignment::Center);

    let header: Element = if right_side.is_empty() {
        left.into()
    } else {
        grid((
            left,
            vstack(right_side)
                .spacing(2.0)
                .horizontal_alignment(HorizontalAlignment::Right)
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into()
    };

    let mut rows: Vec<Element> = vec![header];
    if let Some(limit) = spending.limit_microusd.filter(|limit| *limit > 0) {
        let used_percent = if has_live_usage {
            ((spending.used_microusd.min(limit) as f64 / limit as f64) * 100.0)
                .clamp(0.0, 100.0)
        } else {
            0.0
        };
        let progress = if show_used_percentage {
            used_percent
        } else {
            100.0 - used_percent
        };
        rows.push(rounded_progress(
            progress,
            ThemeRef::Accent,
            None,
            color_scheme,
        ));
    }

    border(vstack(rows).spacing(8.0))
        .corner_radius(f64::from(popup::WINDOW_CORNER_RADIUS_DIP))
        .padding(Thickness::uniform(12.0))
        .background(ThemeRef::CardBackground)
        .border_thickness(Thickness::uniform(1.0))
        .border_brush(ThemeRef::CardStroke)
        .into()
}

fn openrouter_account_heading(account: &OpenRouterAccountSnapshot) -> Element {
    let name = text_block(account.name.clone())
        .font_weight(600)
        .foreground(ThemeRef::SecondaryText)
        .vertical_alignment(VerticalAlignment::Center);
    let content: Element = match account.balance_microusd {
        Some(balance) => grid((
            name,
            text_block(format_usd(balance as f64 / 1_000_000.0))
                .font_weight(600)
                .foreground(ThemeRef::Accent)
                .vertical_alignment(VerticalAlignment::Center)
                .horizontal_alignment(HorizontalAlignment::Right)
                .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into(),
        None => name.into(),
    };
    content
        .margin(Thickness {
            left: 4.0,
            top: 8.0,
            right: 4.0,
            bottom: 0.0,
        })
        .into()
}

/// Stable membership fingerprint for OpenRouter popup chrome. Include account
/// and API-key identities (not live usage) so the provider strip remounts when
/// keys are added/removed/moved, preventing recycled cards under the wrong
/// account heading.
fn openrouter_accounts_strip_key(limits: &RateLimits) -> String {
    let mut key = String::new();
    for account in &limits.openrouter_accounts {
        if !key.is_empty() {
            key.push('\u{1d}');
        }
        key.push_str(&account.id);
        key.push('\u{1f}');
        key.push_str(&account.name);
        for api_key in &account.api_keys {
            key.push('\u{1e}');
            key.push_str(&api_key.id);
            if let Some(label) = api_key.label.as_deref() {
                key.push(':');
                key.push_str(label);
            }
            if let Some(masked) = api_key.masked_key.as_deref() {
                key.push('@');
                key.push_str(masked);
            }
        }
    }
    key
}

fn latest_sampled_at(limits: &ProviderLimits) -> chrono::DateTime<Utc> {
    crate::provider_registry::PROVIDERS
        .iter()
        .map(|descriptor| limits.get(descriptor.kind).sampled_at)
        .max()
        .unwrap_or_default()
}

/// Root WinUI view for Codex Minibar (hosted in a tray popup shell).
pub fn app(cx: &mut RenderCx, state: Arc<AppState>) -> Element {
    let dpi = cx.use_dpi().max(1);
    // Pin the root to the live client size. Stretch alone is not enough: during
    // shell-height animation the tree otherwise keeps its content DesiredSize
    // and sits top-aligned in a taller HWND, leaving a black band under the footer.
    let window_size = cx.use_inner_size();
    let color_scheme = cx.use_color_scheme();
    let window_corner_radius = f64::from(popup::WINDOW_CORNER_RADIUS_DIP);
    // Keep the visual stroke one physical pixel inside the HWND clip so GDI's
    // aliased region cannot trim its anti-aliased XAML corner pixels.
    let border_inset = 96.0 / f64::from(dpi);
    let inner_corner_radius = (window_corner_radius - border_inset).max(0.0);
    let (ui, set_ui) = cx.use_async_state(UiState {
        theme: state.settings.theme,
        accent_color: state.settings.accent_color,
        animations_enabled: state.settings.animations_enabled,
        error: state.startup_error.clone(),
        last_activation: format_last_activation(&RateLimits::default(), state.last_activation_at),
        show_used_percentage: state.settings.show_used_percentage,
        show_usage_pace: state.settings.show_usage_pace,
        popup_visibility: state.settings.popup_visibility.clone(),
        show_total_spend_on_all_tab: state.settings.show_total_spend_on_all_tab,
        total_spend_presentation: state.settings.total_spend_presentation,
        total_spend_period: state.settings.total_spend_period,
        show_account_name: state.settings.show_account_name,
        codex_enabled: state.settings.providers.is_enabled(ProviderKind::Codex),
        claude_enabled: state.settings.providers.is_enabled(ProviderKind::Claude),
        cursor_enabled: state.settings.providers.is_enabled(ProviderKind::Cursor),
        opencode_zen_enabled: state
            .settings
            .providers
            .is_enabled(ProviderKind::OpenCodeZen),
        opencode_go_enabled: state
            .settings
            .providers
            .is_enabled(ProviderKind::OpenCodeGo),
        opencode_zen_credentials_revision: state.settings.opencode_zen_credentials_revision,
        opencode_go_credentials_revision: state.settings.opencode_go_credentials_revision,
        openrouter_enabled: state
            .settings
            .providers
            .is_enabled(ProviderKind::OpenRouter),
        openrouter_credentials_revision: state.settings.openrouter_credentials_revision,
        popup_order: state.settings.popup_order.clone(),
        use_colored_provider_icons: state.settings.use_colored_provider_icons,
        replace_chatgpt_logo_with_codex: state.settings.replace_chatgpt_logo_with_codex,
        update_version: state
            .updates
            .available_update()
            .map(|update| update.version),
        ..UiState::default()
    });
    cx.use_effect(
        (ui.theme, ui.accent_color, ui.animations_enabled),
        move || {
            crate::theme::set_animations_enabled(ui.animations_enabled);
            crate::theme::apply_appearance(ui.theme, ui.accent_color);
        },
    );
    // Rendering observes the same snapshot that the tray consumes; UiState
    // deliberately contains only view metadata, never a second copy of limits.
    let limits = state.current_limits();
    let commands = state.worker_commands();
    let ui_dispatcher = cx.use_ui_marshaller();
    let settings_tx = state.settings_tx.clone();
    let (hovered_action, set_hovered_action) = cx.use_state(Option::<String>::None);
    let (tab_scroll_x, set_tab_scroll_x) = cx.use_state(0.0_f64);
    let (widget_drag, set_widget_drag) = cx.use_state(None::<WidgetDragState>);
    let (pager, pager_dispatch) = cx.use_reducer_fn(reduce_pager, PagerState::default());
    let (hovered_combined_usage_period, set_hovered_combined_usage_period) =
        cx.use_state(None::<TotalSpendPeriod>);
    // Relative timestamps need an occasional render tick while the popup is
    // visible. `prepare_show_on_ui_thread` requests an immediate render on
    // every open, so there is no reason to reconcile the entire hidden WinUI
    // tree once per second for the lifetime of the process.
    let (clock_tick, set_clock_tick) = cx.use_async_state(0_u64);
    let page_animations_enabled = ui.animations_enabled && popup::system_animations_enabled();

    cx.use_effect_with_cleanup(
        (
            pager.animation_id,
            pager.outgoing.is_some(),
            page_animations_enabled,
        ),
        {
            let pager_dispatch = pager_dispatch.clone();
            move || {
                let timer = pager.outgoing.and_then(|_| {
                    let duration = if page_animations_enabled {
                        PAGER_ANIMATION_DURATION
                    } else {
                        Duration::from_millis(1)
                    };
                    DispatcherTimer::new_one_shot(duration, move || {
                        pager_dispatch.call(PagerAction::AnimationFinished(pager.animation_id));
                    })
                    .ok()
                });
                Some(move || drop(timer))
            }
        },
    );

    cx.use_effect(
        (
            pager.current,
            ui.codex_enabled,
            ui.claude_enabled,
            ui.cursor_enabled,
            ui.opencode_zen_enabled,
            ui.opencode_go_enabled,
            ui.openrouter_enabled,
            popup_order_key(&ui.popup_order),
        ),
        {
            let pager_dispatch = pager_dispatch.clone();
            let order = provider_order_from_popup(&ui.popup_order);
            move || {
                pager_dispatch.call(PagerAction::SetProviderOrder(order.clone()));
                let available = match pager.current {
                    PopupView::All => true,
                    PopupView::Codex => ui.codex_enabled,
                    PopupView::Claude => ui.claude_enabled,
                    PopupView::Cursor => ui.cursor_enabled,
                    PopupView::OpenCodeZen => ui.opencode_zen_enabled,
                    PopupView::OpenCodeGo => ui.opencode_go_enabled,
                    PopupView::OpenRouter => ui.openrouter_enabled,
                };
                if !available {
                    pager_dispatch.call(PagerAction::Select(PopupView::All));
                }
            }
        },
    );

    cx.use_effect((), {
        let state = Arc::clone(&state);
        let set_ui = set_ui.clone();
        let ui_dispatcher = ui_dispatcher.clone();
        move || {
            // Convert the WinUI window into a hidden tray popup as soon as it exists.
            let _ = popup::ensure_configured();
            popup::sync_host_constraints();
            // SystemBackdrop paints square + shadow past SetWindowRgn — keep it off.
            set_backdrop(None);
            start_background_bridge(state, set_ui, ui_dispatcher);
        }
    });

    cx.use_effect((), {
        let set_clock_tick = set_clock_tick.clone();
        move || {
            thread::spawn(move || {
                let mut tick = 0_u64;
                loop {
                    thread::sleep(Duration::from_secs(60));
                    if popup::is_visible() {
                        tick = tick.wrapping_add(1);
                        set_clock_tick.call(tick);
                    }
                }
            });
        }
    });

    let refresh = {
        let commands = commands.clone();
        let set_ui = set_ui.clone();
        let ui = ui.clone();
        move || {
            if refresh_all_workers(&commands) {
                let mut ui = ui.clone();
                ui.refreshing = true;
                set_ui.call(ui);
            }
        }
    };
    // A selector only earns its keep when it can actually switch between
    // providers. With zero or one enabled provider the familiar compact
    // footer remains, sparing us some very professional-looking empty UI.
    let enabled_provider_order = provider_order_from_popup(&ui.popup_order)
        .into_iter()
        .filter(|provider| {
            provider_is_enabled(
                *provider,
                ui.codex_enabled,
                ui.claude_enabled,
                ui.cursor_enabled,
                ui.opencode_zen_enabled,
                ui.opencode_go_enabled,
                ui.openrouter_enabled,
            )
        })
        .collect::<Vec<_>>();
    let enabled_views = enabled_popup_views(
        &ui.popup_order,
        ui.codex_enabled,
        ui.claude_enabled,
        ui.cursor_enabled,
        ui.opencode_zen_enabled,
        ui.opencode_go_enabled,
        ui.openrouter_enabled,
    );
    let enabled_provider_count = enabled_views.len().saturating_sub(1);
    let show_provider_tabs = enabled_provider_count > 1;
    let selected_view = pager.current;
    let show_total_spend = ui.show_total_spend_on_all_tab
        && total_spend_provider_count(
            ui.codex_enabled,
            ui.claude_enabled,
            ui.cursor_enabled,
            ui.opencode_zen_enabled,
            ui.opencode_go_enabled,
        ) > 1;
    let all_tab_widgets = visible_popup_widgets(
        &ui.popup_order,
        show_total_spend && show_provider_tabs,
        &ui.popup_visibility,
        ui.codex_enabled,
        ui.claude_enabled,
        ui.cursor_enabled,
        ui.opencode_zen_enabled,
        ui.opencode_go_enabled,
        ui.openrouter_enabled,
    );
    let can_reorder_widgets = selected_view == PopupView::All && all_tab_widgets.len() > 1;
    let build_body = |view: PopupView, retain_disabled_detail: bool| {
        let surface = if view == PopupView::All {
            PopupSurface::AllTab
        } else {
            PopupSurface::ProviderTab
        };
        let show_total_spend = show_total_spend && view == PopupView::All;

        let mut body: Vec<Element> = Vec::new();
        let mut has_preceding_section = false;
        if let Some(error) = ui.error.clone() {
            body.push(
                InfoBar::new("Something went wrong")
                    .message(error)
                    .error()
                    .is_closable(false)
                    .with_key("popup-error")
                    .into(),
            );
            has_preceding_section = true;
        }

        if view == PopupView::All {
            let widgets = visible_popup_widgets(
                &ui.popup_order,
                show_total_spend,
                &ui.popup_visibility,
                ui.codex_enabled,
                ui.claude_enabled,
                ui.cursor_enabled,
                ui.opencode_zen_enabled,
                ui.opencode_go_enabled,
                ui.openrouter_enabled,
            );
            for (index, widget) in widgets.into_iter().enumerate() {
                let is_first = index == 0 && !has_preceding_section;
                let section = match widget {
                    PopupWidgetKind::TotalSpend => {
                        let on_period = {
                            let settings_tx = settings_tx.clone();
                            let set_ui = set_ui.clone();
                            let ui = ui.clone();
                            move |period| {
                                persist_total_spend_period(
                                    settings_tx.clone(),
                                    set_ui.clone(),
                                    ui.clone(),
                                    period,
                                );
                            }
                        };
                        combined_usage_card(
                            &limits,
                            is_first,
                            ui.codex_enabled,
                            ui.claude_enabled,
                            ui.cursor_enabled,
                            ui.opencode_zen_enabled,
                            ui.opencode_go_enabled,
                            ui.openrouter_enabled,
                            ui.total_spend_period,
                            on_period,
                            hovered_combined_usage_period,
                            set_hovered_combined_usage_period.clone(),
                            color_scheme,
                            ui.total_spend_presentation,
                            can_reorder_widgets.then(|| {
                                drag_handle(
                                    PopupWidgetKind::TotalSpend,
                                    color_scheme,
                                    &widget_drag,
                                    set_widget_drag.clone(),
                                )
                            }),
                        )
                        .with_key(format!(
                            "all-combined-usage-{}-{:?}-{}",
                            ui.total_spend_period.key(),
                            ui.total_spend_presentation,
                            if is_first { "first" } else { "rest" }
                        ))
                    }
                    provider_widget => {
                        let provider = provider_widget.as_provider().expect("provider widget");
                        let limits_for_provider = limits.get(provider);
                        let handle = can_reorder_widgets.then(|| {
                            drag_handle(
                                provider_widget,
                                color_scheme,
                                &widget_drag,
                                set_widget_drag.clone(),
                            )
                        });
                        vstack(provider_cards(
                            provider,
                            is_first,
                            limits_for_provider,
                            ui.show_used_percentage,
                            ui.show_usage_pace,
                            &ui.popup_visibility,
                            PopupSurface::AllTab,
                            show_provider_tabs,
                            ui.show_account_name,
                            color_scheme,
                            handle,
                        ))
                        .spacing(6.0)
                        .with_key(format!(
                            "provider-{}-{}-{}",
                            provider.id(),
                            if is_first { "first" } else { "rest" },
                            if provider == ProviderKind::OpenRouter {
                                openrouter_accounts_strip_key(limits_for_provider)
                            } else {
                                String::new()
                            }
                        ))
                        .into()
                    }
                };
                let section = if can_reorder_widgets {
                    with_widget_drop_target(
                        widget,
                        section,
                        &widget_drag,
                        set_widget_drag.clone(),
                        settings_tx.clone(),
                        set_ui.clone(),
                        ui.clone(),
                    )
                } else {
                    section
                };
                body.push(section);
                has_preceding_section = true;
            }
        } else {
            let providers_for_view: Vec<ProviderKind> = view
                .provider()
                .filter(|provider| {
                    provider_is_enabled(
                        *provider,
                        ui.codex_enabled,
                        ui.claude_enabled,
                        ui.cursor_enabled,
                        ui.opencode_zen_enabled,
                        ui.opencode_go_enabled,
                        ui.openrouter_enabled,
                    ) || retain_disabled_detail
                })
                .into_iter()
                .collect();
            for provider in providers_for_view {
                let limits_for_provider = limits.get(provider);
                body.push(
                    vstack(provider_cards(
                        provider,
                        !has_preceding_section,
                        limits_for_provider,
                        ui.show_used_percentage,
                        ui.show_usage_pace,
                        &ui.popup_visibility,
                        surface,
                        show_provider_tabs,
                        ui.show_account_name,
                        color_scheme,
                        None,
                    ))
                    .spacing(6.0)
                    .with_key(format!(
                        "provider-{}-{}",
                        provider.id(),
                        if provider == ProviderKind::OpenRouter {
                            openrouter_accounts_strip_key(limits_for_provider)
                        } else {
                            String::new()
                        }
                    ))
                    .into(),
                );
                has_preceding_section = true;
            }
        }
        if !ui.codex_enabled
            && !ui.claude_enabled
            && !ui.cursor_enabled
            && !ui.opencode_zen_enabled
            && !ui.opencode_go_enabled
            && !ui.openrouter_enabled
        {
            body.push(
                InfoBar::new("No providers enabled")
                    .message("Enable a provider in Settings > Providers.")
                    .is_closable(false)
                    .with_key("popup-no-providers")
                    .into(),
            );
        }
        body
    };

    let body = build_body(selected_view, false);
    let outgoing_body = pager.outgoing.map(|view| build_body(view, true));

    let footer_background = match color_scheme {
        // CSS shorthand: #0002 = #00000022; #0001 = #00000011.
        ColorScheme::Dark => Color {
            a: 0x30,
            r: 0,
            g: 0,
            b: 0,
        },
        ColorScheme::Light => Color {
            a: 0x11,
            r: 0,
            g: 0,
            b: 0,
        },
    };

    let footer_identity: Element = if show_provider_tabs {
        // Build only live tabs — never pad with Element::Empty. Empty siblings
        // collapse during reconcile and let swap-chain hosts keep a prior
        // provider's pixels in another tab's slot.
        let tab_content_width =
            provider_tab_strip_content_width(enabled_provider_order.len());
        let tab_viewport_width = provider_tab_strip_viewport_width();
        let tab_max_offset = (tab_content_width - tab_viewport_width).max(0.0);
        let tab_scroll_x = tab_scroll_x.clamp(0.0, tab_max_offset);
        let on_tab_wheel = Callback::new({
            let set_tab_scroll_x = set_tab_scroll_x.clone();
            move |info: PointerEventInfo| {
                if info.wheel_delta == 0 {
                    return;
                }
                let step = f64::from(info.wheel_delta) / 120.0 * 48.0;
                let dx = if info.wheel_is_horizontal {
                    step
                } else {
                    -step
                };
                let next = (tab_scroll_x + dx).clamp(0.0, tab_max_offset);
                if (next - tab_scroll_x).abs() > 0.5 {
                    set_tab_scroll_x.call(next);
                }
            }
        });
        let mut provider_tabs = vec![popup_tab_button(
            "provider-tab-all",
            None,
            Some("All"),
            "All providers",
            selected_view == PopupView::All,
            ui.use_colored_provider_icons,
            color_scheme,
            &hovered_action,
            set_hovered_action.clone(),
            on_tab_wheel.clone(),
            {
                let pager_dispatch = pager_dispatch.clone();
                move || pager_dispatch.call(PagerAction::Select(PopupView::All))
            },
        )];
        for provider in &enabled_provider_order {
            let (tab_id, icon_name, tip, view) = match provider {
                ProviderKind::Codex => (
                    "provider-tab-codex",
                    if ui.replace_chatgpt_logo_with_codex {
                        "codex"
                    } else {
                        "chatgpt"
                    },
                    "Codex",
                    PopupView::Codex,
                ),
                ProviderKind::Claude => {
                    ("provider-tab-claude", "claude", "Claude", PopupView::Claude)
                }
                ProviderKind::Cursor => {
                    ("provider-tab-cursor", "cursor", "Cursor", PopupView::Cursor)
                }
                ProviderKind::OpenCodeZen => (
                    "provider-tab-opencode-zen",
                    "opencode",
                    "OpenCode Zen",
                    PopupView::OpenCodeZen,
                ),
                ProviderKind::OpenCodeGo => (
                    "provider-tab-opencode-go",
                    "opencode",
                    "OpenCode Go",
                    PopupView::OpenCodeGo,
                ),
                ProviderKind::OpenRouter => (
                    "provider-tab-openrouter",
                    "openrouter",
                    "OpenRouter",
                    PopupView::OpenRouter,
                ),
            };
            provider_tabs.push(popup_tab_button(
                tab_id,
                Some(icon_name),
                None,
                tip,
                selected_view == view,
                ui.use_colored_provider_icons,
                color_scheme,
                &hovered_action,
                set_hovered_action.clone(),
                on_tab_wheel.clone(),
                {
                    let pager_dispatch = pager_dispatch.clone();
                    move || pager_dispatch.call(PagerAction::Select(view))
                },
            ));
        }
        let tabs_key = format!(
            "provider-tabs-{}-{}-{}",
            provider_order_key(&enabled_provider_order),
            ui.use_colored_provider_icons,
            color_scheme as i32
        );
        horizontal_wheel_strip(
            hstack(provider_tabs)
                .spacing(2.0)
                .horizontal_alignment(HorizontalAlignment::Left)
                .vertical_alignment(VerticalAlignment::Center)
                .margin(Thickness {
                    left: -tab_scroll_x,
                    top: 0.0,
                    right: 0.0,
                    bottom: 0.0,
                })
                // Provider marks are native swap-chain children. Recreate
                // the whole selector when membership, order, tint mode, or
                // theme changes; otherwise WinUI reconciliation can retain
                // a prior tab's text/icon. Scroll offset stays out of the
                // key so panning cannot recycle swap-chain hosts.
                .with_key(tabs_key.clone()),
            ICON_BUTTON_SIZE,
            tabs_key,
            on_tab_wheel,
        )
    } else {
        vstack((
            body_strong("Codex Minibar").foreground(ThemeRef::SecondaryText),
            caption(if ui.refreshing {
                "Refreshing…".into()
            } else {
                format_last_updated(latest_sampled_at(&limits), clock_tick)
            })
            .foreground(ThemeRef::TertiaryText),
        ))
        .spacing(0.0)
        .vertical_alignment(VerticalAlignment::Center)
        .horizontal_alignment(HorizontalAlignment::Left)
        .into()
    };
    let refresh_tooltip = if show_provider_tabs {
        let last_updated = format_last_updated(latest_sampled_at(&limits), clock_tick);
        let relative_time = last_updated
            .strip_prefix("Updated ")
            .unwrap_or(&last_updated);
        format!("Refresh | Last updated {relative_time}")
    } else {
        "Refresh".into()
    };

    let footer = border(
        grid((
            footer_identity.grid_column(0),
            hstack({
                // Build only live actions — never pad with Element::Empty.
                // Empty siblings collapse during reconcile and let swap-chain
                // hosts keep a neighbor's painted icon in another slot.
                let mut actions = vec![
                    icon_button(
                        "refresh",
                        "fluent-refresh",
                        "fluent-refresh",
                        &refresh_tooltip,
                        color_scheme,
                        &hovered_action,
                        set_hovered_action.clone(),
                        refresh,
                    ),
                    icon_button(
                        "settings",
                        "fluent-settings",
                        "fluent-settings",
                        "Settings",
                        color_scheme,
                        &hovered_action,
                        set_hovered_action.clone(),
                        {
                            let settings_tx = settings_tx.clone();
                            let updates = Arc::clone(&state.updates);
                            move || {
                                if let Err(error) = crate::settings_window::open(
                                    settings_tx.clone(),
                                    updates.clone(),
                                ) {
                                    eprintln!("Could not open settings window: {error:?}");
                                }
                            }
                        },
                    ),
                ];
                if ui.update_version.is_some() {
                    actions.push(
                        update_accent_button("Update", || {
                            if let Err(error) = crate::updater::apply_pending_update() {
                                eprintln!("failed to apply update: {error:#}");
                                notifications::show("Update failed", &format!("{error:#}"));
                            }
                        })
                        .height(ICON_BUTTON_SIZE)
                        .min_height(ICON_BUTTON_SIZE)
                        .max_height(ICON_BUTTON_SIZE)
                        .padding(Thickness {
                            left: 12.0,
                            top: 0.0,
                            right: 12.0,
                            bottom: 0.0,
                        })
                        .vertical_alignment(VerticalAlignment::Center)
                        .with_key("footer-update")
                        .into(),
                    );
                }
                actions
            })
            .spacing(4.0)
            .horizontal_alignment(HorizontalAlignment::Right)
            .vertical_alignment(VerticalAlignment::Center)
            // Update membership swaps control kinds; key the whole strip so
            // action swap-chain hosts never inherit a neighbor's painted icon.
            .with_key(format!(
                "footer-actions-{}-{}",
                ui.update_version.is_some(),
                color_scheme as i32
            ))
            .canvas_z_index(1)
            .grid_column(1),
        ))
        .rows([GridLength::Auto])
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .column_spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(Thickness {
        left: if show_provider_tabs { 14.0 } else { 24.0 },
        top: 10.0,
        right: 18.0,
        // Extra bottom padding so content clears the rounded window corners.
        bottom: 14.0,
    })
    .border_thickness(Thickness {
        left: 0.0,
        top: 1.0,
        right: 0.0,
        bottom: 0.0,
    })
    .background(footer_background)
    .border_brush(ThemeRef::CardStroke)
    .horizontal_alignment(HorizontalAlignment::Stretch);

    // The body can outgrow the popup when both providers, statistics, and an
    // error are visible. Give it the flexible row and keep the footer in a
    // separate Auto row so it remains fixed to the bottom edge.
    let build_page = |body: Vec<Element>,
                      view: PopupView,
                      role: &'static str,
                      from_x: f32,
                      to_x: f32,
                      measure_height: bool| {
        // Limit snapshots update for every provider poll. They must update the
        // existing reactive tree rather than remount this entire page: doing
        // so also recreates its unmanaged SwapChainPanel/XAML children and
        // steadily grows the WinUI compositor's retained allocation.
        //
        // Key only error presence, not the message text: PollFailed can emit a
        // new string every minute and would otherwise remount the whole page.
        let body_layout_key = format!(
            "popup-page-{role}-{}-{}-{}-{:?}-{}-{}-{}-{}-{}-{}-{}-{:?}-{:?}",
            ui.error.is_some(),
            popup_visibility_key(&ui.popup_visibility),
            ui.show_total_spend_on_all_tab,
            ui.total_spend_presentation,
            ui.total_spend_period.key(),
            ui.show_account_name,
            ui.codex_enabled,
            ui.claude_enabled,
            ui.cursor_enabled,
            ui.openrouter_enabled,
            popup_order_key(&ui.popup_order),
            color_scheme as i32,
            view,
        );
        let mut content = vstack(body)
            .spacing(6.0)
            .padding(Thickness {
                left: 16.0,
                top: 16.0,
                right: 16.0,
                bottom: 16.0,
            })
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Top);
        if widget_drag.is_some() {
            let set_drag = set_widget_drag.clone();
            let drag = widget_drag.clone();
            let settings_tx = settings_tx.clone();
            let set_ui = set_ui.clone();
            let ui = ui.clone();
            content = content.on_pointer_released(move |_: PointerEventInfo| {
                let Some(current) = drag.clone() else {
                    return;
                };
                commit_widget_drag(
                    settings_tx.clone(),
                    set_ui.clone(),
                    ui.clone(),
                    current,
                    set_drag.clone(),
                );
            });
        }
        if from_x != to_x {
            content.mounted = Some(Callback::new(move |native: Option<_>| {
                if let Some(native) = native
                    && let Err(error) = animate_translation_x(
                        native,
                        from_x,
                        to_x,
                        PAGER_ANIMATION_DURATION,
                        Easing::Fluent,
                    )
                {
                    eprintln!("Could not animate popup page: {error:?}");
                }
            }));
        }
        if measure_height {
            content = content.on_resize(|_width, height| {
                popup::set_client_height_from_body_content(height);
            });
        }
        scroll_viewer(content.with_key(body_layout_key))
            .horizontal_scroll_bar_visibility(ScrollBarVisibility::Disabled)
            .vertical_scroll_bar_visibility(if measure_height {
                ScrollBarVisibility::Auto
            } else {
                ScrollBarVisibility::Hidden
            })
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch)
            .grid_row(0)
            .into()
    };

    let incoming_from = if page_animations_enabled {
        pager
            .outgoing
            .map_or(0.0, |_| pager.direction.incoming_offset())
    } else {
        0.0
    };
    let current_page = build_page(body, selected_view, "current", incoming_from, 0.0, true);
    let outgoing_page = match (pager.outgoing, outgoing_body) {
        (Some(view), Some(body)) => build_page(
            body,
            view,
            "outgoing",
            0.0,
            if page_animations_enabled {
                pager.direction.outgoing_offset()
            } else {
                0.0
            },
            false,
        ),
        _ => Element::Empty,
    };
    let page_viewport = grid((outgoing_page, current_page))
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Stretch)
        .grid_row(0);

    let body_panel = border(
        grid((page_viewport, footer.grid_row(1)))
            .rows([GridLength::Star(1.0), GridLength::Auto])
            .columns([GridLength::Star(1.0)])
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch)
            .background(Color::transparent()),
    )
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::SurfaceStroke)
    .corner_radius(inner_corner_radius)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Stretch);

    // Mica behind content; reconciler does not manage this panel's children.
    // It is element-level Mica rather than `Window.SystemBackdrop`: the latter
    // ignores the popup's Win32 rounded region and paints past its edges.
    // Height is owned solely by the body's desired-size callback above. Using
    // this layer's arranged height as a second source fed ResizeClient back
    // into layout and caused a resize loop / spurious scrollbars.
    let mica = {
        let mut host = swap_chain_panel()
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch);
        host.mounted = Some(Callback::new(|native: Option<_>| {
            if let Some(native) = native {
                if let Err(error) = crate::acrylic::install_mica_into(native) {
                    eprintln!("Could not install popup Mica element: {error:?}");
                }
            }
        }));
        host.unmounted = Some(Callback::new(|native: Option<_>| {
            if let Some(native) = native {
                let _ = crate::acrylic::clear_children(native);
            }
        }));
        let mica: Element = host.into();
        mica.with_key("popup-mica")
    };

    border(
        grid((mica, body_panel))
            .rows([GridLength::Star(1.0)])
            .columns([GridLength::Star(1.0)])
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch)
            // Match card chrome — SolidBackground is near-black in dark mode and
            // reads as the same "black gap" when Mica lags a frame behind resize.
            .background(ThemeRef::CardBackground),
    )
    .padding(Thickness::uniform(border_inset))
    .corner_radius(window_corner_radius)
    .background(ThemeRef::CardBackground)
    .width(window_size.width.max(1.0))
    .height(window_size.height.max(1.0))
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Stretch)
    .into()
}

/// The first settings surface is deliberately a native WinUI shell: persistent
/// sidebar on the left, focused tab content on the right. Persistence wiring
/// follows once every setting has its final interaction model.
#[allow(dead_code)]
pub(crate) fn open_settings_window(
    settings_tx: Sender<Settings>,
    updates: Arc<UpdateController>,
) -> windows_core::Result<()> {
    crate::settings_window::open(settings_tx, updates)
}

fn update_available_from_phase(phase: &UpdatePhase) -> bool {
    matches!(phase, UpdatePhase::Available(_))
}

fn update_version_from_phase(phase: &UpdatePhase) -> Option<String> {
    match phase {
        UpdatePhase::Available(update) => Some(update.version.clone()),
        _ => None,
    }
}

/// A transparent WinUI/Mica window can retain a stale white DWM redirection
/// bitmap, particularly after moving across monitors. The visual symptom is a
/// real window that is lighter than screenshots despite the same XAML tree.
/// Disabling that legacy backing surface preserves Mica and lets the intended
/// `#FFFFFF05` page wash composite correctly.
#[cfg(windows)]
fn disable_settings_redirection_bitmap() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GWL_EXSTYLE, GetWindowLongW, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOSIZE, SWP_NOZORDER, SetWindowLongW, SetWindowPos, WS_EX_NOREDIRECTIONBITMAP,
    };

    let title: Vec<u16> = "Codex Minibar Settings"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    if hwnd.is_null() {
        return;
    }

    unsafe {
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        SetWindowLongW(
            hwnd,
            GWL_EXSTYLE,
            (ex_style | WS_EX_NOREDIRECTIONBITMAP) as i32,
        );
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        );
    }
}

#[cfg(not(windows))]
fn disable_settings_redirection_bitmap() {}

#[allow(dead_code)]
fn settings_window(cx: &mut RenderCx, settings: Arc<Settings>) -> Element {
    // Run after the WinUI tree has mounted as well as immediately after window
    // activation. The second pass covers the first-frame compositor path.
    cx.use_effect((), disable_settings_redirection_bitmap);
    let (selected, set_selected) = cx.use_state(SettingsTab::default());
    let content = settings_tab_content(&settings, selected);

    let menu = [
        NavViewItem::new("General")
            .tag(SettingsTab::General.tag())
            .icon_path(crate::icons::data("house"), "#E6E6E6"),
        NavViewItem::new("Tray")
            .tag(SettingsTab::Tray.tag())
            .icon_path(crate::icons::data("chat-centered-text"), "#E6E6E6"),
        NavViewItem::new("Notifications")
            .tag(SettingsTab::Notifications.tag())
            .icon_path(crate::icons::data("bell"), "#E6E6E6"),
        NavViewItem::new("Advanced")
            .tag(SettingsTab::Advanced.tag())
            .icon_path(crate::icons::data("sliders"), "#E6E6E6"),
    ];
    // NavigationView owns the sidebar only. Its generated content presenter
    // is opaque in the current WinUI template, so rendering the page inside it
    // would blend our `#FFFFFF05` wash over white instead of Mica.
    let navigation = NavigationView::new(menu, Element::Empty)
        .selected_tag(selected.tag())
        .on_selection_changed({
            move |tag: String| {
                let next = SettingsTab::from_tag(&tag);
                if next != selected {
                    set_selected.call(next);
                }
            }
        })
        .pane_display_mode(NavigationViewPaneDisplayMode::Left)
        .pane_open(true)
        .open_pane_length(220.0)
        .pane_title("Settings")
        .settings_visible(false)
        .back_button_visible(false)
        .pane_toggle_button_visible(false)
        .background(Color::transparent())
        .width(220.0)
        .horizontal_alignment(HorizontalAlignment::Left)
        .vertical_alignment(VerticalAlignment::Stretch);

    let page = border(
        border(content)
            .with_key(format!("settings-page-{}", selected.tag()))
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch),
    )
    .padding(Thickness {
        left: 32.0,
        top: 24.0,
        right: 32.0,
        bottom: 32.0,
    })
    .background(ThemeRef::LayerFill)
    .corner_radius(12.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Stretch);

    let title_bar = TitleBar::new("Codex Minibar Settings")
        .back_button_visible(false)
        .pane_toggle_button_visible(false);

    let shell = grid((navigation.grid_column(0), page.grid_column(1)))
        .columns([GridLength::Pixel(220.0), GridLength::Star(1.0)])
        .rows([GridLength::Star(1.0)])
        .background(Color::transparent());

    grid((title_bar.grid_row(0), shell.grid_row(1)))
        .rows([GridLength::Auto, GridLength::Star(1.0)])
        .columns([GridLength::Star(1.0)])
        .background(Color::transparent())
        .into()
}

#[allow(dead_code)]
fn settings_tab_content(settings: &Settings, tab: SettingsTab) -> Element {
    let (title, subtitle, rows): (&str, &str, Vec<Element>) = match tab {
        SettingsTab::General => (
            "General",
            "Core behavior for Codex Minibar.",
            vec![
                settings_row(
                    "Automatic activation",
                    if settings.automatic_activation {
                        "On"
                    } else {
                        "Off"
                    },
                ),
                settings_row(
                    "Start at sign-in",
                    if settings.start_at_login { "On" } else { "Off" },
                ),
                settings_row(
                    "Check for updates",
                    if settings.check_for_updates {
                        "On"
                    } else {
                        "Off"
                    },
                ),
            ],
        ),
        SettingsTab::Tray => (
            "Tray",
            "Choose what Codex Minibar shows in the notification area.",
            vec![settings_row(
                "Active tray widgets",
                format!("{} configured", settings.tray_widgets.len()),
            )],
        ),
        SettingsTab::Notifications => (
            "Notifications",
            "Decide which important events deserve your attention.",
            vec![
                settings_row(
                    "Activation failures",
                    if settings.notifications.activation_failure {
                        "On"
                    } else {
                        "Off"
                    },
                ),
                settings_row(
                    "Codex unavailable",
                    if settings.notifications.codex_unavailable {
                        "On"
                    } else {
                        "Off"
                    },
                ),
                settings_row(
                    "Activation successes",
                    if settings.notifications.activation_success {
                        "On"
                    } else {
                        "Off"
                    },
                ),
            ],
        ),
        SettingsTab::Advanced => (
            "Advanced",
            "Storage and integration settings that should stay out of the way.",
            vec![
                settings_row(
                    "History retention",
                    format!("{} days", settings.history_retention_days),
                ),
                settings_row(
                    "Codex executable",
                    settings
                        .codex_path
                        .as_ref()
                        .map_or("Automatic".into(), |path| path.display().to_string()),
                ),
            ],
        ),
    };

    vstack((
        text_block(title).font_size(28.0).bold(),
        text_block(subtitle).foreground(ThemeRef::SecondaryText),
        vstack(rows).spacing(8.0),
    ))
    .spacing(10.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Top)
    .into()
}

#[allow(dead_code)]
fn settings_row(label: impl Into<String>, value: impl Into<String>) -> Element {
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
        left: 12.0,
        top: 10.0,
        right: 12.0,
        bottom: 10.0,
    })
    .background(ThemeRef::CardBackground)
    .corner_radius(6.0)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn start_background_bridge(
    state: Arc<AppState>,
    set_ui: AsyncSetState<UiState>,
    ui_dispatcher: UiMarshaller,
) {
    let events = state.take_worker_events();
    let mut widgets = state
        .settings
        .tray_widgets
        .iter()
        .filter(|widget| widget.is_visible_for(&state.settings.providers))
        .cloned()
        .collect::<Vec<_>>();
    let settings_rx = state
        .settings_rx
        .lock()
        .ok()
        .and_then(|mut slot| slot.take());
    let settings_tx = state.settings_tx.clone();
    let updates = Arc::clone(&state.updates);
    let mut check_for_updates = state.settings.check_for_updates;
    let mut notify_on_update = state.settings.notifications.update_available;

    thread::spawn(move || {
        let mut tray = TrayManager::new();
        let fallback_attempt = state.last_activation_at;
        let mut notification_settings = state.settings.notifications.clone();
        let mut limit_notifications = HashMap::<ProviderKind, LimitNotificationTracker>::new();
        let mut update_phase = updates.snapshot();
        let mut ui = UiState {
            theme: state.settings.theme,
            accent_color: state.settings.accent_color,
            animations_enabled: state.settings.animations_enabled,
            error: state.startup_error.clone(),
            last_activation: format_last_activation(&RateLimits::default(), fallback_attempt),
            show_used_percentage: state.settings.show_used_percentage,
            show_usage_pace: state.settings.show_usage_pace,
            popup_visibility: state.settings.popup_visibility.clone(),
            show_total_spend_on_all_tab: state.settings.show_total_spend_on_all_tab,
            total_spend_presentation: state.settings.total_spend_presentation,
            total_spend_period: state.settings.total_spend_period,
            show_account_name: state.settings.show_account_name,
            codex_enabled: state.settings.providers.is_enabled(ProviderKind::Codex),
            claude_enabled: state.settings.providers.is_enabled(ProviderKind::Claude),
            cursor_enabled: state.settings.providers.is_enabled(ProviderKind::Cursor),
            opencode_zen_enabled: state
                .settings
                .providers
                .is_enabled(ProviderKind::OpenCodeZen),
            opencode_go_enabled: state
                .settings
                .providers
                .is_enabled(ProviderKind::OpenCodeGo),
            opencode_zen_credentials_revision: state.settings.opencode_zen_credentials_revision,
            opencode_go_credentials_revision: state.settings.opencode_go_credentials_revision,
            openrouter_enabled: state
                .settings
                .providers
                .is_enabled(ProviderKind::OpenRouter),
            openrouter_credentials_revision: state.settings.openrouter_credentials_revision,
            popup_order: state.settings.popup_order.clone(),
            use_colored_provider_icons: state.settings.use_colored_provider_icons,
            replace_chatgpt_logo_with_codex: state.settings.replace_chatgpt_logo_with_codex,
            codex_path: state.settings.codex_path.clone(),
            claude_path: state.settings.claude_path.clone(),
            cursor_path: state.settings.cursor_path.clone(),
            update_version: update_version_from_phase(&update_phase),
            ..UiState::default()
        };
        if let Some(error) = ui.error.as_deref() {
            crate::logger::info(format!("Popup error: {error}"));
        }

        if let Err(error) = tray.sync(
            &widgets,
            &state.current_limits(),
            update_available_from_phase(&update_phase),
        ) {
            ui.set_popup_error(error.to_string());
            flush_popup_ui(&set_ui, &ui);
        }

        // Keep trying until the WinUI window exists, then park it as a popup.
        for _ in 0..50 {
            if popup::ensure_configured().is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        let apply_settings = |ui: &mut UiState,
                              set_ui: &AsyncSetState<UiState>,
                              notification_settings: &mut NotificationSettings,
                              widgets: &mut Vec<TrayWidget>,
                              tray: &mut TrayManager,
                              settings: Settings| {
            crate::settings_window::sync_open_window(settings.clone(), ui_dispatcher.clone());
            let phase = updates.snapshot();
            let providers_changed = ui.codex_enabled
                != settings.providers.is_enabled(ProviderKind::Codex)
                || ui.claude_enabled != settings.providers.is_enabled(ProviderKind::Claude)
                || ui.cursor_enabled != settings.providers.is_enabled(ProviderKind::Cursor)
                || ui.opencode_zen_enabled
                    != settings.providers.is_enabled(ProviderKind::OpenCodeZen)
                || ui.opencode_go_enabled
                    != settings.providers.is_enabled(ProviderKind::OpenCodeGo)
                || ui.openrouter_enabled != settings.providers.is_enabled(ProviderKind::OpenRouter);
            let opencode_zen_credentials_changed =
                ui.opencode_zen_credentials_revision != settings.opencode_zen_credentials_revision;
            let opencode_go_credentials_changed =
                ui.opencode_go_credentials_revision != settings.opencode_go_credentials_revision;
            let openrouter_credentials_changed =
                ui.openrouter_credentials_revision != settings.openrouter_credentials_revision;
            ui.theme = settings.theme;
            ui.accent_color = settings.accent_color;
            ui.animations_enabled = settings.animations_enabled;
            ui.show_used_percentage = settings.show_used_percentage;
            ui.show_usage_pace = settings.show_usage_pace;
            ui.popup_visibility = settings.popup_visibility.clone();
            ui.show_total_spend_on_all_tab = settings.show_total_spend_on_all_tab;
            ui.total_spend_presentation = settings.total_spend_presentation;
            ui.total_spend_period = settings.total_spend_period;
            ui.show_account_name = settings.show_account_name;
            ui.codex_enabled = settings.providers.is_enabled(ProviderKind::Codex);
            ui.claude_enabled = settings.providers.is_enabled(ProviderKind::Claude);
            ui.cursor_enabled = settings.providers.is_enabled(ProviderKind::Cursor);
            ui.opencode_zen_enabled = settings.providers.is_enabled(ProviderKind::OpenCodeZen);
            ui.opencode_go_enabled = settings.providers.is_enabled(ProviderKind::OpenCodeGo);
            ui.opencode_zen_credentials_revision = settings.opencode_zen_credentials_revision;
            ui.opencode_go_credentials_revision = settings.opencode_go_credentials_revision;
            ui.openrouter_enabled = settings.providers.is_enabled(ProviderKind::OpenRouter);
            ui.openrouter_credentials_revision = settings.openrouter_credentials_revision;
            if openrouter_credentials_changed && let Ok(mut limits) = state.limits.lock() {
                *limits.get_mut(ProviderKind::OpenRouter) = RateLimits::default();
            }
            ui.popup_order = settings.popup_order.clone();
            ui.use_colored_provider_icons = settings.use_colored_provider_icons;
            ui.replace_chatgpt_logo_with_codex = settings.replace_chatgpt_logo_with_codex;
            *notification_settings = settings.notifications.clone();
            *widgets = settings
                .tray_widgets
                .iter()
                .filter(|widget| widget.is_visible_for(&settings.providers))
                .cloned()
                .collect();
            ui.update_version = update_version_from_phase(&phase);
            // Presentation settings must visibly apply before any background
            // work. In particular, changing provider icons must never wait on
            // a worker lock, network request, or provider lifecycle change.
            flush_popup_ui(set_ui, ui);
            let restart = [
                (ProviderKind::Codex, settings.codex_path != ui.codex_path),
                (ProviderKind::Claude, settings.claude_path != ui.claude_path),
                (ProviderKind::Cursor, settings.cursor_path != ui.cursor_path),
                (ProviderKind::OpenRouter, openrouter_credentials_changed),
            ]
            .into_iter()
            .filter_map(|(provider, changed)| changed.then_some(provider))
            .collect::<Vec<_>>();
            ui.codex_path = settings.codex_path.clone();
            ui.claude_path = settings.claude_path.clone();
            ui.cursor_path = settings.cursor_path.clone();
            if providers_changed || !restart.is_empty() {
                let provider_errors = state.sync_provider_workers(&settings, &restart);
                if !provider_errors.is_empty() {
                    ui.set_popup_error(provider_errors.join("\n"));
                }
            }
            // Repaint the existing native icons in place. Recreating them makes
            // Explorer animate a remove/add sequence and causes a visible flash.
            if let Err(error) = tray.sync(
                widgets,
                &state.current_limits(),
                update_available_from_phase(&phase),
            ) {
                ui.set_popup_error(error.to_string());
            }
            for (provider, commands) in state.worker_commands() {
                let _ = commands.send(WorkerCommand::SetAutomaticActivation(
                    settings.automatic_activation
                        && crate::provider_registry::descriptor(provider).supports_activation,
                ));
                let schedules = settings
                    .scheduled_activations
                    .iter()
                    .filter(|rule| {
                        crate::provider_registry::descriptor(provider).supports_activation
                            && rule.provider() == Some(provider)
                    })
                    .cloned()
                    .collect();
                let _ = commands.send(WorkerCommand::SetScheduledActivations(schedules));
                let _ = commands.send(WorkerCommand::SetLimitRefreshInterval(Duration::from_secs(
                    settings.limit_refresh_interval.seconds(),
                )));
                // The worker refreshes immediately after receiving this command,
                // so the selected history range is reflected in the open popup
                // without asking the user to restart the application.
                let _ = commands.send(WorkerCommand::SetHistoryRetentionDays(
                    settings.history_retention_days,
                ));
                if (provider == ProviderKind::OpenCodeZen && opencode_zen_credentials_changed)
                    || (provider == ProviderKind::OpenCodeGo && opencode_go_credentials_changed)
                    || (provider == ProviderKind::OpenRouter && openrouter_credentials_changed)
                {
                    let _ = commands.send(WorkerCommand::Refresh);
                }
            }
            flush_popup_ui(set_ui, ui);
        };

        let drain_settings = |ui: &mut UiState,
                              set_ui: &AsyncSetState<UiState>,
                              notification_settings: &mut NotificationSettings,
                              widgets: &mut Vec<TrayWidget>,
                              tray: &mut TrayManager,
                              check_for_updates: &mut bool,
                              notify_on_update: &mut bool| {
            let Some(settings_rx) = settings_rx.as_ref() else {
                return;
            };
            while let Ok(settings) = settings_rx.try_recv() {
                if settings.check_for_updates && !*check_for_updates {
                    updates.check_async(false, settings.notifications.update_available);
                }
                *check_for_updates = settings.check_for_updates;
                *notify_on_update = settings.notifications.update_available;
                apply_settings(ui, set_ui, notification_settings, widgets, tray, settings);
            }
        };

        let drain_updates = |ui: &mut UiState,
                             set_ui: &AsyncSetState<UiState>,
                             tray: &mut TrayManager,
                             update_phase: &mut UpdatePhase,
                             widgets: &mut Vec<TrayWidget>| {
            let next = updates.snapshot();
            if next == *update_phase {
                return;
            }
            *update_phase = next;
            ui.update_version = update_version_from_phase(update_phase);
            if let Err(error) = tray.sync(
                widgets,
                &state.current_limits(),
                update_available_from_phase(update_phase),
            ) {
                ui.set_popup_error(error.to_string());
            }
            publish_popup_ui(set_ui, ui);
        };

        let drain_toast_update = || {
            if crate::notifications::take_toast_update_request()
                && let Err(error) = crate::updater::apply_pending_update()
            {
                eprintln!("failed to apply update from toast: {error:#}");
                notifications::show("Update failed", &format!("{error:#}"));
            }
        };

        let Some(events) = events else {
            publish_popup_ui(&set_ui, &ui);
            loop {
                popup::pump_messages();
                drain_toast_update();
                if let Err(error) = tray.refresh_system_theme(&widgets, &state.current_limits()) {
                    ui.set_popup_error(error.to_string());
                    publish_popup_ui(&set_ui, &ui);
                }
                drain_settings(
                    &mut ui,
                    &set_ui,
                    &mut notification_settings,
                    &mut widgets,
                    &mut tray,
                    &mut check_for_updates,
                    &mut notify_on_update,
                );
                drain_updates(&mut ui, &set_ui, &mut tray, &mut update_phase, &mut widgets);
                if pump_tray_and_dismiss(
                    &tray,
                    &ui_dispatcher,
                    &settings_tx,
                    &state,
                    &mut ui,
                    &set_ui,
                ) {
                    drop(tray);
                    state.shutdown_worker();
                    std::process::exit(0);
                }
                thread::sleep(Duration::from_millis(16));
            }
        };

        loop {
            popup::pump_messages();
            drain_toast_update();
            if let Err(error) = tray.refresh_system_theme(&widgets, &state.current_limits()) {
                ui.set_popup_error(error.to_string());
                publish_popup_ui(&set_ui, &ui);
            }
            drain_settings(
                &mut ui,
                &set_ui,
                &mut notification_settings,
                &mut widgets,
                &mut tray,
                &mut check_for_updates,
                &mut notify_on_update,
            );
            drain_updates(&mut ui, &set_ui, &mut tray, &mut update_phase, &mut widgets);
            if pump_tray_and_dismiss(
                &tray,
                &ui_dispatcher,
                &settings_tx,
                &state,
                &mut ui,
                &set_ui,
            ) {
                drop(tray);
                state.shutdown_worker();
                std::process::exit(0);
            }
            match events.recv_timeout(Duration::from_millis(16)) {
                Ok(WorkerEvent::ProviderLimitsUpdated(provider, limits)) => {
                    if (provider == ProviderKind::Codex && !ui.codex_enabled)
                        || (provider == ProviderKind::Claude && !ui.claude_enabled)
                        || (provider == ProviderKind::Cursor && !ui.cursor_enabled)
                        || (provider == ProviderKind::OpenCodeZen && !ui.opencode_zen_enabled)
                        || (provider == ProviderKind::OpenCodeGo && !ui.opencode_go_enabled)
                        || (provider == ProviderKind::OpenRouter && !ui.openrouter_enabled)
                    {
                        continue;
                    }
                    crate::logger::info(format!(
                        "{} limits received: session used={:?}%, reset={:?}; weekly used={:?}%, reset={:?}",
                        provider.display_name(),
                        limits.primary.used_percent,
                        limits.primary.resets_at,
                        limits.secondary.used_percent,
                        limits.secondary.resets_at,
                    ));
                    // Publish once, then let both native tray and WinUI render
                    // from that exact snapshot.
                    state.replace_limits(provider, limits);
                    let limits = state.current_limits();
                    crate::settings_window::publish_discovered_popup_bricks(
                        &limits,
                        ui_dispatcher.clone(),
                    );
                    if ui
                        .popup_visibility
                        .absorb_discovered_bricks(&limits)
                    {
                        let limits_for_settings = limits.clone();
                        crate::settings_window::persist_update(
                            settings_tx.clone(),
                            move |settings| {
                                settings.absorb_discovered_popup_bricks(&limits_for_settings);
                            },
                        );
                    }
                    limit_notifications.entry(provider).or_default().observe(
                        limits.get(provider),
                        &notification_settings,
                        provider,
                    );
                    if let Err(error) = tray.sync(
                        &widgets,
                        &limits,
                        update_available_from_phase(&update_phase),
                    ) {
                        ui.set_popup_error(error.to_string());
                    } else {
                        ui.error = None;
                    }
                    if provider == ProviderKind::Codex {
                        ui.last_activation =
                            format_last_activation(limits.get(provider), fallback_attempt);
                    }
                    ui.observe_limits_update();
                    ui.refreshing = false;
                    publish_popup_ui(&set_ui, &ui);
                }
                Ok(WorkerEvent::ProviderUsageUpdated(provider, usage)) => {
                    if (provider == ProviderKind::Codex && !ui.codex_enabled)
                        || (provider == ProviderKind::Claude && !ui.claude_enabled)
                        || (provider == ProviderKind::Cursor && !ui.cursor_enabled)
                        || (provider == ProviderKind::OpenCodeZen && !ui.opencode_zen_enabled)
                        || (provider == ProviderKind::OpenCodeGo && !ui.opencode_go_enabled)
                        || (provider == ProviderKind::OpenRouter && !ui.openrouter_enabled)
                    {
                        continue;
                    }
                    crate::logger::info(format!(
                        "{} usage received: today={} tokens, history={} tokens",
                        provider.display_name(),
                        usage.today.total_tokens(),
                        usage.history.total_tokens()
                    ));
                    state.replace_usage(provider, usage);
                    // Usage stats affect only the popup, but they share the
                    // reactive snapshot revision with quota updates.
                    ui.observe_limits_update();
                    publish_popup_ui(&set_ui, &ui);
                }
                Ok(WorkerEvent::ProviderActivationStarted(provider)) => {
                    crate::logger::info(format!("{} activation started", provider.display_name()));
                }
                Ok(WorkerEvent::ProviderActivationSucceeded(provider)) => {
                    crate::logger::info(format!(
                        "{} activation succeeded",
                        provider.display_name()
                    ));
                    ui.last_activation = format!(
                        "{} succeeded at {}",
                        provider.display_name(),
                        format_activation_at(Utc::now())
                    );
                    if notification_settings.activation_success {
                        notifications::show_activation_succeeded(provider);
                    }
                    publish_popup_ui(&set_ui, &ui);
                }
                Ok(WorkerEvent::ProviderActivationFailed(provider, error)) => {
                    crate::logger::info(format!(
                        "{} activation failed: {error}",
                        provider.display_name()
                    ));
                    ui.last_activation = format!(
                        "{} failed at {}: {error}",
                        provider.display_name(),
                        format_activation_at(Utc::now())
                    );
                    publish_popup_ui(&set_ui, &ui);
                }
                Ok(WorkerEvent::ProviderPollFailed(provider, error)) => {
                    crate::logger::info(format!(
                        "{} polling failed: {error}",
                        provider.display_name()
                    ));
                    ui.set_popup_error(format!("{}: {error}", provider.display_name()));
                    ui.refreshing = false;
                    publish_popup_ui(&set_ui, &ui);
                }
                // All live provider workers are forwarded as scoped events.
                Ok(
                    WorkerEvent::LimitsUpdated(_)
                    | WorkerEvent::UsageUpdated(_)
                    | WorkerEvent::ActivationStarted
                    | WorkerEvent::ActivationSucceeded
                    | WorkerEvent::ActivationFailed(_)
                    | WorkerEvent::PollFailed(_),
                ) => {}
                Ok(WorkerEvent::Stopped) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

#[cfg(windows)]
fn pump_tray_and_dismiss(
    tray: &TrayManager,
    ui_dispatcher: &UiMarshaller,
    settings_tx: &Sender<Settings>,
    state: &AppState,
    ui: &mut UiState,
    set_ui: &AsyncSetState<UiState>,
) -> bool {
    use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};

    while let Ok(event) = TrayIconEvent::receiver().try_recv() {
        if let TrayIconEvent::Click {
            id,
            position,
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
            && tray.contains(&id)
        {
            let x = position.x as i32;
            let y = position.y as i32;
            if popup::is_visible() {
                // While Settings is open the popup is a live preview, not a
                // transient tray flyout. Keep it available until Settings closes.
                if !crate::settings_window::is_open() {
                    ui_dispatcher.dispatch(popup::hide);
                }
            } else {
                // Activation and motion publication both belong to WinUI's
                // thread. Publishing the animation from this tray worker used
                // to strand the HWND just beyond the monitor edge forever.
                // Flush suppressed background UiState so the first frame sees
                // the latest limits/error/activation text.
                flush_popup_ui(set_ui, ui);
                let (ready_tx, ready_rx) = std::sync::mpsc::channel();
                ui_dispatcher.dispatch(move || {
                    let ready = popup::prepare_show_on_ui_thread();
                    if ready {
                        popup::show_near(x, y);
                    }
                    let _ = ready_tx.send(ready);
                });
                match ready_rx.recv_timeout(std::time::Duration::from_millis(500)) {
                    Ok(true) => {}
                    Ok(false) => {
                        eprintln!("popup host was unavailable during synchronous reactivation");
                    }
                    Err(error) => eprintln!("popup reactivation timed out: {error}"),
                }
            }
            ui_dispatcher.dispatch(popup::hide_from_switchers);
        }
    }

    for action in tray.drain_menu_actions() {
        match action {
            TrayMenuAction::Update => {
                if let Err(error) = crate::updater::apply_pending_update() {
                    eprintln!("failed to apply update: {error:#}");
                    notifications::show("Update failed", &format!("{error:#}"));
                }
            }
            TrayMenuAction::Settings => {
                let settings_tx = settings_tx.clone();
                let updates = Arc::clone(&state.updates);
                flush_popup_ui(set_ui, ui);
                ui_dispatcher.dispatch(move || {
                    // Opening Settings from the tray menu should provide the
                    // same always-visible live preview as opening it from the
                    // popup footer.
                    if !popup::is_visible() && popup::prepare_show_on_ui_thread() {
                        popup::show_near_cursor();
                    }
                    if let Err(error) = crate::settings_window::open(settings_tx, updates) {
                        eprintln!("Could not open settings window: {error:?}");
                    }
                });
            }
            TrayMenuAction::Exit => return true,
        }
    }

    // HWND geometry belongs to the WinUI thread. Coalesce the 60 Hz tray pump
    // into at most one pending UI task so a busy dispatcher cannot accumulate
    // an unbounded tail of stale SetWindowPos calls.
    if popup::is_visible() && !KEEP_ON_MONITOR_QUEUED.swap(true, Ordering::SeqCst) {
        ui_dispatcher.dispatch(|| {
            popup::keep_on_monitor();
            KEEP_ON_MONITOR_QUEUED.store(false, Ordering::SeqCst);
        });
    }

    // Settings are a live editor for this surface. Treat the separate settings
    // window as part of the popup interaction so navigating or toggling a
    // setting cannot dismiss the preview beneath it.
    if !crate::settings_window::is_open()
        && !popup::is_closing()
        && (popup::clicked_outside() || popup::escape_pressed())
    {
        ui_dispatcher.dispatch(popup::hide);
    }
    false
}

#[cfg(not(windows))]
fn pump_tray_and_dismiss(
    _tray: &TrayManager,
    _ui_dispatcher: &UiMarshaller,
    _settings_tx: &Sender<Settings>,
    _state: &AppState,
    _ui: &mut UiState,
    _set_ui: &AsyncSetState<UiState>,
) -> bool {
    false
}

const ICON_BUTTON_SIZE: f64 = 36.0;
const REORDER_BUTTON_SIZE: f64 = 28.0;
const ALL_TAB_WIDTH: f64 = 44.0;
const TAB_STRIP_SPACING: f64 = 2.0;
const FOOTER_TAB_PADDING_LEFT: f64 = 14.0;
const FOOTER_PADDING_RIGHT: f64 = 18.0;
const FOOTER_COLUMN_SPACING: f64 = 8.0;
const FOOTER_ACTION_SPACING: f64 = 4.0;
const FOOTER_ACTION_COUNT: f64 = 2.0;

fn provider_tab_strip_content_width(provider_count: usize) -> f64 {
    ALL_TAB_WIDTH + provider_count as f64 * (ICON_BUTTON_SIZE + TAB_STRIP_SPACING)
}

fn provider_tab_strip_viewport_width() -> f64 {
    f64::from(popup::POPUP_WIDTH)
        - FOOTER_TAB_PADDING_LEFT
        - FOOTER_PADDING_RIGHT
        - FOOTER_COLUMN_SPACING
        - (ICON_BUTTON_SIZE * FOOTER_ACTION_COUNT
            + FOOTER_ACTION_SPACING * (FOOTER_ACTION_COUNT - 1.0))
}

/// Compact footer selector item for choosing the combined or provider view.
fn popup_tab_button(
    id: &'static str,
    icon_name: Option<&'static str>,
    label: Option<&'static str>,
    tip: &'static str,
    selected: bool,
    use_colored_provider_icons: bool,
    color_scheme: ColorScheme,
    hovered_action: &Option<String>,
    set_hovered_action: SetState<Option<String>>,
    on_wheel: impl IntoCallback<PointerEventInfo>,
    on_click: impl IntoUnitCallback,
) -> Element {
    let hovered = hovered_action.as_deref() == Some(id);
    let set_on_enter = set_hovered_action.clone();
    let set_on_exit = set_hovered_action;
    let idle_icon_color = popup_chrome_icon_color(color_scheme, false);
    let hover_icon_color = popup_chrome_icon_color(color_scheme, true);
    let brand_icon_color = match icon_name {
        Some("codex") | Some("chatgpt") => Color::rgb(128, 159, 255),
        Some("claude") => Color::rgb(217, 119, 87),
        // Match Total Spend: Cursor mark flips with the Windows text theme.
        Some("cursor") => combined_usage_color(ProviderKind::Cursor, color_scheme),
        Some("opencode") => combined_usage_color(ProviderKind::OpenCodeZen, color_scheme),
        Some("openrouter") => combined_usage_color(ProviderKind::OpenRouter, color_scheme),
        _ => idle_icon_color,
    };
    let tab_width = if label.is_some() {
        ALL_TAB_WIDTH
    } else {
        ICON_BUTTON_SIZE
    };
    let hover_background: Element = border(Element::Empty)
        .background(ThemeRef::SubtleFill)
        .opacity(if hovered { 1.0 } else { 0.0 })
        .corner_radius(4.0)
        .relative_align_left()
        .relative_align_right()
        .relative_align_top()
        .relative_align_bottom()
        .into();
    let selection_marker: Element = border(Element::Empty)
        .height(2.0)
        .background(ThemeRef::Accent)
        .opacity(if selected { 1.0 } else { 0.0 })
        .corner_radius(1.0)
        .margin(Thickness {
            left: 9.0,
            top: 0.0,
            right: 9.0,
            bottom: 0.0,
        })
        .relative_align_left()
        .relative_align_right()
        .relative_align_bottom()
        .into();
    let mut layers: Vec<Element> = vec![hover_background];
    if let Some(label) = label {
        layers.push(
            body_strong(label)
                .foreground(if selected {
                    ThemeRef::Accent
                } else if hovered {
                    ThemeRef::PrimaryText
                } else {
                    ThemeRef::SecondaryText
                })
                .relative_align_h_center()
                .relative_align_v_center()
                .into(),
        );
    } else {
        let icon_name = icon_name.expect("provider tab icon");
        if use_colored_provider_icons {
            layers.push(
                crate::icons::element(icon_name, 18.0, brand_icon_color)
                    .relative_align_h_center()
                    .relative_align_v_center()
                    .into(),
            );
        } else {
            // Crossfade idle/emphasized hosts instead of remounting on hover.
            layers.push(
                crate::icons::element(icon_name, 18.0, idle_icon_color)
                    .opacity(if hovered { 0.0 } else { 1.0 })
                    .relative_align_h_center()
                    .relative_align_v_center()
                    .into(),
            );
            layers.push(
                crate::icons::element(icon_name, 18.0, hover_icon_color)
                    .opacity(if hovered { 1.0 } else { 0.0 })
                    .relative_align_h_center()
                    .relative_align_v_center()
                    .into(),
            );
        }
    }
    layers.push(selection_marker);
    // Cover swap-chain icons so wheel hits a normal XAML element and
    // bubbles here. SwapChainPanel often swallows wheel input.
    layers.push(
        border(Element::Empty)
            .background(Color::transparent())
            .relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .relative_align_bottom()
            .into(),
    );

    // `SwapChainPanel` paints only on mount. Keep the tab key stable across
    // hover so reconciliation cannot recycle another tab's native icon host.
    relative_panel(layers)
        .tooltip(tip)
        .width(tab_width)
        .height(ICON_BUTTON_SIZE)
        .min_width(tab_width)
        .min_height(ICON_BUTTON_SIZE)
        .max_width(tab_width)
        .max_height(ICON_BUTTON_SIZE)
        .background(Color::transparent())
        .on_pointer_entered(move |_: PointerEventInfo| {
            set_on_enter.call(Some(id.to_string()));
        })
        .on_pointer_exited(move || set_on_exit.call(None))
        .on_pointer_wheel(on_wheel)
        .on_tapped(on_click)
        .with_key(format!(
            "{id}-{}-{}-{}",
            icon_name.unwrap_or("label"),
            use_colored_provider_icons,
            color_scheme as i32
        ))
        .into()
}

/// Icon-only action using a neutral Phosphor SVG that adopts the accent on hover.
fn icon_button(
    id: &'static str,
    normal_icon: &'static str,
    hover_icon: &'static str,
    tip: &str,
    color_scheme: ColorScheme,
    hovered_action: &Option<String>,
    set_hovered_action: SetState<Option<String>>,
    on_click: impl IntoUnitCallback,
) -> Element {
    chrome_icon_button(
        id,
        normal_icon,
        hover_icon,
        tip,
        ICON_BUTTON_SIZE,
        18.0,
        color_scheme,
        hovered_action,
        set_hovered_action,
        on_click,
    )
}

fn chrome_icon_button(
    id: &'static str,
    normal_icon: &'static str,
    hover_icon: &'static str,
    tip: &str,
    size: f64,
    glyph_size: f64,
    color_scheme: ColorScheme,
    hovered_action: &Option<String>,
    set_hovered_action: SetState<Option<String>>,
    on_click: impl IntoUnitCallback,
) -> Element {
    let hovered = hovered_action.as_deref() == Some(id);
    let set_on_enter = set_hovered_action.clone();
    let set_on_exit = set_hovered_action;
    let idle_color = popup_chrome_icon_color(color_scheme, false);
    let hover_background: Element = border(Element::Empty)
        .background(ThemeRef::SubtleFill)
        .opacity(if hovered { 1.0 } else { 0.0 })
        .corner_radius(4.0)
        .relative_align_left()
        .relative_align_right()
        .relative_align_top()
        .relative_align_bottom()
        .into();
    // Keep both swap-chain hosts mounted and crossfade opacity on hover.
    // Remounting the icon host on every hover recycles native panels and can
    // leave a neighbor's painted glyph in this slot.
    let idle_icon: Element = crate::icons::element(normal_icon, glyph_size, idle_color)
        .opacity(if hovered { 0.0 } else { 1.0 })
        .relative_align_h_center()
        .relative_align_v_center()
        .into();
    let accent_icon: Element = crate::icons::accent_element(hover_icon, glyph_size)
        .opacity(if hovered { 1.0 } else { 0.0 })
        .relative_align_h_center()
        .relative_align_v_center()
        .into();
    // Stable across hover; remount only when theme tint changes.
    relative_panel(vec![hover_background, idle_icon, accent_icon])
        .tooltip(tip)
        .width(size)
        .height(size)
        .min_width(size)
        .min_height(size)
        .max_width(size)
        .max_height(size)
        .background(Color::transparent())
        .on_pointer_entered(move |_: PointerEventInfo| {
            set_on_enter.call(Some(id.to_string()));
        })
        .on_pointer_exited(move || set_on_exit.call(None))
        .on_tapped(on_click)
        .with_key(format!(
            "{id}-{size}-{glyph_size}-{:02X}{:02X}{:02X}",
            idle_color.r, idle_color.g, idle_color.b
        ))
        .into()
}

/// Approximate WinUI primary/secondary text for swap-chain icons that cannot
/// bind ThemeRef brushes directly.
fn popup_chrome_icon_color(color_scheme: ColorScheme, emphasized: bool) -> Color {
    match color_scheme {
        ColorScheme::Light => {
            if emphasized {
                Color::rgb(0, 0, 0)
            } else {
                Color::rgb(96, 96, 96)
            }
        }
        ColorScheme::Dark => {
            if emphasized {
                Color::rgb(230, 230, 230)
            } else {
                Color::rgb(190, 190, 190)
            }
        }
    }
}

/// Thin pill progress track with a rounded fill and optional pace marker.
fn rounded_progress(
    value: f64,
    fill: ThemeRef,
    pace: Option<PaceTip>,
    color_scheme: ColorScheme,
) -> Element {
    const HEIGHT: f64 = 6.0;
    let radius = HEIGHT / 2.0;
    let filled = value.clamp(0.0, 100.0);
    let (fill_star, rest_star) = if filled <= 0.0 {
        (0.0001, 100.0)
    } else if filled >= 100.0 {
        (100.0, 0.0001)
    } else {
        (filled, 100.0 - filled)
    };

    let fill_layer = grid((border(Element::Empty)
        .background(fill.clone())
        .corner_radius(radius)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Stretch)
        .grid_column(0),))
    .columns([GridLength::Star(fill_star), GridLength::Star(rest_star)])
    .rows([GridLength::Star(1.0)])
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Stretch)
    .grid_column(0)
    .grid_row(0);

    let track_layer: Element = border(Element::Empty)
        .background(fill)
        .opacity(0.2)
        .corner_radius(radius)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Stretch)
        .grid_column(0)
        .grid_row(0)
        .into();
    let mut layers: Vec<Element> = vec![track_layer, fill_layer.into()];
    if let Some(pace) = pace {
        layers.push(pace_marker_layer(pace, color_scheme));
    }

    border(
        grid(layers)
            .columns([GridLength::Star(1.0)])
            .rows([GridLength::Star(1.0)])
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch),
    )
    .corner_radius(radius)
    .height(HEIGHT)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

/// High-contrast vertical tick showing the expected even-burn position.
fn pace_marker_layer(pace: PaceTip, color_scheme: ColorScheme) -> Element {
    // Keep the indicator legible against the theme-specific accent track.
    const LINE_WIDTH: f64 = 2.0;
    let marker_color = match color_scheme {
        ColorScheme::Light => Color {
            a: 255,
            r: 0,
            g: 0,
            b: 0,
        },
        ColorScheme::Dark => Color {
            a: 255,
            r: 255,
            g: 255,
            b: 255,
        },
    };
    let percent = pace.percent.clamp(0.0, 100.0);
    let (left_star, right_star) = if percent <= 0.0 {
        (0.0001, 100.0)
    } else if percent >= 100.0 {
        (100.0, 0.0001)
    } else {
        (percent, 100.0 - percent)
    };

    grid((border(Element::Empty)
        .width(LINE_WIDTH)
        .background(marker_color)
        .horizontal_alignment(HorizontalAlignment::Left)
        .vertical_alignment(VerticalAlignment::Stretch)
        .grid_column(1),))
    .columns([
        GridLength::Star(left_star),
        GridLength::Auto,
        GridLength::Star(right_star),
    ])
    .rows([GridLength::Star(1.0)])
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Stretch)
    .grid_column(0)
    .grid_row(0)
    .into()
}

fn limit_card(
    title: &str,
    window: &LimitWindow,
    show_used_percentage: bool,
    show_usage_pace: bool,
    disabled: bool,
    color_scheme: ColorScheme,
) -> Element {
    let accent = ThemeRef::Accent;
    let (remaining_label, progress, show_reset, pace) = if disabled {
        ("Disabled".into(), 100.0, false, None)
    } else {
        let remaining = window.remaining_percent();
        let percentage = if show_used_percentage {
            window.used_percent
        } else {
            remaining
        };
        let suffix = if show_used_percentage { "used" } else { "left" };
        let label = percentage
            .map(|value| format!("{value}% {suffix}"))
            .unwrap_or_else(|| "Unavailable".into());
        let pace = show_usage_pace
            .then(|| window.pace_tip(show_used_percentage, Utc::now()))
            .flatten();
        (label, f64::from(percentage.unwrap_or(0)), true, pace)
    };
    let reset = window.resets_at.map(|at| format_reset_in(Some(at)));

    let header: Element = if let Some(pace) = pace {
        grid((
            caption(title.to_uppercase())
                .foreground(ThemeRef::SecondaryText)
                .vertical_alignment(VerticalAlignment::Center),
            caption(pace.summary())
                .foreground(ThemeRef::SecondaryText)
                .horizontal_alignment(HorizontalAlignment::Right)
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Center)
        .into()
    } else {
        grid((caption(title.to_uppercase()).foreground(ThemeRef::SecondaryText),))
            .columns([GridLength::Star(1.0)])
            .rows([GridLength::Auto])
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Center)
            .into()
    };

    let reset_status: Element = match reset {
        Some(reset) => hstack((
            text_block("Resets in")
                .foreground(ThemeRef::TertiaryText)
                .vertical_alignment(VerticalAlignment::Center),
            text_block(reset).vertical_alignment(VerticalAlignment::Center),
        ))
        .spacing(6.0)
        .horizontal_alignment(HorizontalAlignment::Right)
        .vertical_alignment(VerticalAlignment::Center)
        .into(),
        None => text_block("Session is not activated")
            .foreground(Color::rgb(255, 255, 255))
            .horizontal_alignment(HorizontalAlignment::Right)
            .vertical_alignment(VerticalAlignment::Center)
            .into(),
    };

    let footer: Element = if show_reset {
        grid((
            hstack((text_block(remaining_label)
                .font_weight(600)
                .foreground(accent.clone())
                .vertical_alignment(VerticalAlignment::Center),))
            .vertical_alignment(VerticalAlignment::Center),
            reset_status.grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Center)
        .into()
    } else {
        hstack((text_block(remaining_label)
            .font_weight(600)
            .foreground(accent.clone())
            .vertical_alignment(VerticalAlignment::Center),))
        .vertical_alignment(VerticalAlignment::Center)
        .into()
    };

    border(
        vstack((
            header,
            rounded_progress(progress, accent, pace, color_scheme),
            footer,
        ))
        .spacing(8.0),
    )
    .corner_radius(f64::from(popup::WINDOW_CORNER_RADIUS_DIP))
    .padding(Thickness::uniform(12.0))
    .background(ThemeRef::CardBackground)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .into()
}

fn credits_card(limits: &RateLimits) -> Element {
    let value = credits_display_value(limits)
        .expect("credits card is only rendered for a displayable credit balance");

    border(
        grid((
            vstack((
                text_block("CREDITS").foreground(ThemeRef::TertiaryText),
                caption("Available balance").foreground(ThemeRef::TertiaryText),
            ))
            .spacing(2.0)
            .vertical_alignment(VerticalAlignment::Center),
            text_block(value)
                .font_weight(600)
                .foreground(ThemeRef::Accent)
                .vertical_alignment(VerticalAlignment::Center)
                .horizontal_alignment(HorizontalAlignment::Right)
                .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .corner_radius(f64::from(popup::WINDOW_CORNER_RADIUS_DIP))
    .padding(Thickness {
        left: 16.0,
        top: 12.0,
        right: 16.0,
        bottom: 12.0,
    })
    .background(ThemeRef::CardBackground)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .into()
}

fn reset_credits_card(limits: &RateLimits) -> Element {
    let count = limits.available_reset_count();
    let count_label = if count == 1 {
        "1 Banked Reset".into()
    } else {
        format!("{count} Banked Resets")
    };
    let expiration = limits.next_reset_credit_expiration();
    let expiration_label = expiration
        .map(|expires_at| format!("Expires in {}", format_reset_in(Some(expires_at))))
        .unwrap_or_else(|| "No expiration date".into());
    let expiration_date = expiration
        .map(|expires_at| {
            expires_at
                .with_timezone(&Local)
                .format("%b %-d, %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "Available to use".into());

    border(
        grid((
            text_block(count_label)
                .font_weight(600)
                .foreground(ThemeRef::Accent)
                .vertical_alignment(VerticalAlignment::Center),
            vstack((
                text_block(expiration_label),
                caption(expiration_date)
                    .foreground(ThemeRef::TertiaryText)
                    .horizontal_alignment(HorizontalAlignment::Right),
            ))
            .spacing(1.0)
            .horizontal_alignment(HorizontalAlignment::Right)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .corner_radius(f64::from(popup::WINDOW_CORNER_RADIUS_DIP))
    .padding(Thickness {
        left: 16.0,
        top: 12.0,
        right: 16.0,
        bottom: 12.0,
    })
    .background(ThemeRef::CardBackground)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .into()
}

fn usage_statistics_card(provider: ProviderKind, limits: &RateLimits) -> Element {
    let statistics = &limits.usage;
    if provider == ProviderKind::Cursor && !statistics.has_data() {
        return border(
            vstack((
                body_strong("Usage activity"),
                caption(
                    "Waiting for Cursor usage export. Refresh to retry; Cursor can delay new rows.",
                )
                .foreground(ThemeRef::TertiaryText)
                .wrap(),
            ))
            .spacing(6.0),
        )
        .corner_radius(f64::from(popup::WINDOW_CORNER_RADIUS_DIP))
        .padding(Thickness::uniform(12.0))
        .background(ThemeRef::CardBackground)
        .border_thickness(Thickness::uniform(1.0))
        .border_brush(ThemeRef::CardStroke)
        .into();
    }
    if is_cost_provider(provider) {
        let metrics = grid((
            usage_value_metric(
                "Today spend",
                format_spend(statistics.today.estimated_cost_microusd),
                statistics.today.requests,
            ),
            usage_value_metric(
                &format!("Last {} days spend", statistics.history_days),
                format_spend(statistics.history.estimated_cost_microusd),
                statistics.history.requests,
            )
            .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch);
        let detail = format!(
            "{} requests · {} tokens",
            statistics.history.requests,
            format_token_count(statistics.history.total_tokens()),
        );
        return border(
            vstack((
                metrics,
                usage_activity_chart(statistics, true),
                caption(detail).foreground(ThemeRef::TertiaryText),
            ))
            .spacing(12.0),
        )
        .corner_radius(f64::from(popup::WINDOW_CORNER_RADIUS_DIP))
        .padding(Thickness::uniform(12.0))
        .background(ThemeRef::CardBackground)
        .border_thickness(Thickness::uniform(1.0))
        .border_brush(ThemeRef::CardStroke)
        .into();
    }
    let period = statistics.history_days;
    let total = format_token_count(statistics.history.total_tokens());
    let today = format_token_count(statistics.today.total_tokens());
    let today_value = statistics
        .today
        .estimated_api_value_usd()
        .map(format_usd)
        .unwrap_or_else(|| "No data".into());
    let history_value = statistics
        .history
        .estimated_api_value_usd()
        .map(format_usd)
        .unwrap_or_else(|| "No data".into());
    let detail = format!(
        "{} in · {} out · {} cached · {} requests",
        format_token_count(statistics.history.input_tokens),
        format_token_count(statistics.history.output_tokens),
        format_token_count(statistics.history.cached_input_tokens),
        statistics.history.requests,
    );
    let metrics = grid((
        usage_tokens_and_cost_metric("Today tokens", today, today_value),
        usage_tokens_and_cost_metric(&format!("Last {period} days tokens"), total, history_value)
            .grid_column(1),
    ))
    .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
    .rows([GridLength::Auto])
    .horizontal_alignment(HorizontalAlignment::Stretch);
    let chart = usage_activity_chart(statistics, false);

    border(
        vstack((
            metrics,
            chart,
            caption(detail).foreground(ThemeRef::TertiaryText),
        ))
        .spacing(12.0),
    )
    .corner_radius(f64::from(popup::WINDOW_CORNER_RADIUS_DIP))
    .padding(Thickness::uniform(12.0))
    .background(ThemeRef::CardBackground)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .into()
}

fn combined_usage_card(
    limits: &ProviderLimits,
    is_first: bool,
    codex_enabled: bool,
    claude_enabled: bool,
    cursor_enabled: bool,
    opencode_zen_enabled: bool,
    opencode_go_enabled: bool,
    openrouter_enabled: bool,
    period: TotalSpendPeriod,
    on_period: impl Fn(TotalSpendPeriod) + Clone + 'static,
    hovered_period: Option<TotalSpendPeriod>,
    set_hovered_period: SetState<Option<TotalSpendPeriod>>,
    color_scheme: ColorScheme,
    presentation: TotalSpendPresentation,
    drag_handle: Option<Element>,
) -> Element {
    let mut entries: Vec<_> = crate::provider_registry::PROVIDERS
        .iter()
        .filter_map(|descriptor| {
            if !descriptor.include_in_total_spend {
                return None;
            }
            let enabled = match descriptor.kind {
                ProviderKind::Codex => codex_enabled,
                ProviderKind::Claude => claude_enabled,
                ProviderKind::Cursor => cursor_enabled,
                ProviderKind::OpenCodeZen => opencode_zen_enabled,
                ProviderKind::OpenCodeGo => opencode_go_enabled,
                ProviderKind::OpenRouter => openrouter_enabled,
            };
            enabled.then(|| (descriptor.kind, limits.get(descriptor.kind)))
        })
        .map(|(provider, provider_limits)| {
            (
                provider,
                combined_usage_spend(&provider_limits.usage, period),
            )
        })
        .collect();
    entries.sort_by(|(_, left), (_, right)| right.cmp(left));
    let total_spend = entries
        .iter()
        .fold(0_u64, |total, (_, spend)| total.saturating_add(*spend));
    let content = match presentation {
        TotalSpendPresentation::Donut => {
            combined_usage_donut_content(&entries, total_spend, period, color_scheme)
        }
        TotalSpendPresentation::ProgressBar => {
            combined_usage_progress_content(&entries, total_spend, color_scheme)
        }
    };

    let mut title_trailing_items: Vec<Element> = vec![
        combined_usage_period_selector(period, on_period, hovered_period, set_hovered_period)
            .into(),
    ];
    if let Some(handle) = drag_handle {
        title_trailing_items.push(handle);
    }
    let title_trailing = hstack(title_trailing_items)
        .spacing(4.0)
        .horizontal_alignment(HorizontalAlignment::Right)
        .vertical_alignment(VerticalAlignment::Center)
        .grid_column(1);

    vstack((
        grid((
            body_strong("Total Spend")
                .foreground(ThemeRef::SecondaryText)
                .vertical_alignment(VerticalAlignment::Center),
            title_trailing,
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .margin(Thickness {
            left: 4.0,
            top: if is_first { 0.0 } else { 8.0 },
            right: 4.0,
            bottom: 2.0,
        })
        .with_key(format!(
            "total-spend-heading-{}",
            if is_first { "first" } else { "rest" }
        )),
        border(content)
            .corner_radius(f64::from(popup::WINDOW_CORNER_RADIUS_DIP))
            .padding(Thickness::uniform(10.0))
            .background(ThemeRef::CardBackground)
            .border_thickness(Thickness::uniform(1.0))
            .border_brush(ThemeRef::CardStroke),
    ))
    .spacing(6.0)
    .into()
}

fn combined_usage_donut_content(
    entries: &[(ProviderKind, u64)],
    total_spend: u64,
    period: TotalSpendPeriod,
    color_scheme: ColorScheme,
) -> Element {
    let provider_totals = vstack(
        entries
            .iter()
            .map(|(provider, spend)| combined_usage_row(*provider, *spend, color_scheme))
            .collect::<Vec<_>>(),
    )
    .spacing(10.0)
    .vertical_alignment(VerticalAlignment::Center);

    grid((
        combined_usage_donut(entries, total_spend, period, color_scheme).margin(Thickness {
            left: 0.0,
            top: 0.0,
            right: 16.0,
            bottom: 0.0,
        }),
        provider_totals
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(1),
    ))
    .columns([GridLength::Auto, GridLength::Star(1.0)])
    .rows([GridLength::Auto])
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn combined_usage_progress_content(
    entries: &[(ProviderKind, u64)],
    total_spend: u64,
    color_scheme: ColorScheme,
) -> Element {
    let mut sorted_entries = entries.to_vec();
    sorted_entries.sort_by(|(_, left), (_, right)| right.cmp(left));

    vstack((
        text_block(format_spend(total_spend))
            .font_size(22.0)
            .font_weight(600),
        combined_usage_progress_bar(&sorted_entries, color_scheme),
        combined_usage_grouped_totals(&sorted_entries, color_scheme),
    ))
    .spacing(10.0)
    .into()
}

fn combined_usage_progress_bar(
    entries: &[(ProviderKind, u64)],
    color_scheme: ColorScheme,
) -> Element {
    let total_spend = entries
        .iter()
        .fold(0_u64, |total, (_, spend)| total.saturating_add(*spend));
    let mut columns = Vec::with_capacity(entries.len().saturating_mul(2).saturating_sub(1));
    for (index, (_, spend)) in entries.iter().enumerate() {
        if index > 0 {
            columns.push(GridLength::Pixel(4.0));
        }
        let weight = if total_spend == 0 { 1 } else { *spend.max(&1) };
        columns.push(GridLength::Star(weight as f64));
    }
    let segments: Vec<Element> = entries
        .iter()
        .enumerate()
        .map(|(index, (provider, _))| {
            border(Element::Empty)
                .background(combined_usage_color(*provider, color_scheme))
                .height(10.0)
                .corner_radius(5.0)
                .grid_column((index * 2) as i32)
                .into()
        })
        .collect();

    grid(segments)
        .columns(columns)
        .rows([GridLength::Pixel(10.0)])
        .height(10.0)
        .into()
}

fn combined_usage_spend(
    statistics: &crate::usage::UsageStatistics,
    period: TotalSpendPeriod,
) -> u64 {
    match period {
        TotalSpendPeriod::Today => statistics.today.estimated_cost_microusd,
        TotalSpendPeriod::Yesterday => statistics
            .daily
            .iter()
            .find(|entry| entry.date == Local::now().date_naive() - ChronoDuration::days(1))
            .map(|entry| entry.usage.estimated_cost_microusd)
            .unwrap_or_default(),
        TotalSpendPeriod::ThirtyDays => statistics.history.estimated_cost_microusd,
    }
}

fn combined_usage_period_selector(
    selected: TotalSpendPeriod,
    on_select: impl Fn(TotalSpendPeriod) + Clone + 'static,
    hovered: Option<TotalSpendPeriod>,
    set_hovered: SetState<Option<TotalSpendPeriod>>,
) -> Element {
    let buttons: Vec<Element> = [
        TotalSpendPeriod::Today,
        TotalSpendPeriod::Yesterday,
        TotalSpendPeriod::ThirtyDays,
    ]
    .into_iter()
    .map(|period| {
        combined_usage_period_button(
            period,
            selected,
            hovered == Some(period),
            on_select.clone(),
            set_hovered.clone(),
        )
    })
    .collect();
    hstack(buttons).spacing(12.0).into()
}

fn combined_usage_period_button(
    period: TotalSpendPeriod,
    selected: TotalSpendPeriod,
    hovered: bool,
    on_select: impl Fn(TotalSpendPeriod) + Clone + 'static,
    set_hovered: SetState<Option<TotalSpendPeriod>>,
) -> Element {
    let is_selected = period == selected;
    let set_hovered_on_enter = set_hovered.clone();
    let set_hovered_on_exit = set_hovered;
    // Crossfade text colors: tertiary idle, secondary hover, accent selected.
    let layers: Vec<Element> = vec![
        body_strong(period.label())
            .foreground(ThemeRef::TertiaryText)
            .opacity(if !is_selected && !hovered { 1.0 } else { 0.0 })
            .with_opacity_transition(crate::theme::duration(Duration::from_millis(200)))
            .relative_align_h_center()
            .relative_align_v_center()
            .into(),
        body_strong(period.label())
            .foreground(ThemeRef::SecondaryText)
            .opacity(if !is_selected && hovered { 1.0 } else { 0.0 })
            .with_opacity_transition(crate::theme::duration(Duration::from_millis(200)))
            .relative_align_h_center()
            .relative_align_v_center()
            .into(),
        body_strong(period.label())
            .foreground(ThemeRef::Accent)
            .opacity(if is_selected { 1.0 } else { 0.0 })
            .with_opacity_transition(crate::theme::duration(Duration::from_millis(200)))
            .relative_align_h_center()
            .relative_align_v_center()
            .into(),
    ];
    relative_panel(layers)
        .on_pointer_entered(move |_: PointerEventInfo| {
            set_hovered_on_enter.call(Some(period));
        })
        .on_pointer_exited(move || set_hovered_on_exit.call(None))
        .on_tapped(move || on_select(period))
        .with_key(format!("combined-period-{}-{is_selected}", period.key()))
        .into()
}

/// Draws a true circular ring with native WinUI arc paths.
///
/// The swap-chain host key stays stable across spend refreshes. Remounting it
/// on every usage update recreated unmanaged XAML children and grew the WinUI
/// compositor working set over long runs. Geometry is reinstalled in place
/// when the series fingerprint changes.
fn combined_usage_donut(
    entries: &[(ProviderKind, u64)],
    total_spend: u64,
    period: TotalSpendPeriod,
    color_scheme: ColorScheme,
) -> Element {
    const SIZE: f64 = 124.0;
    thread_local! {
        static DONUT_MOUNTS: std::cell::RefCell<
            std::collections::HashMap<String, windows_core::IInspectable>,
        > = std::cell::RefCell::new(std::collections::HashMap::new());
        static DONUT_SERIES: std::cell::RefCell<std::collections::HashMap<String, u64>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
    }

    let xaml = combined_usage_donut_xaml(entries, total_spend, color_scheme);
    let series_key = entries.iter().fold(0_u64, |hash, (provider, spend)| {
        hash.wrapping_mul(31)
            .wrapping_add(*spend)
            .wrapping_add(*provider as u64)
    });
    // Stable host identity — theme/period changes remount; spend updates do not.
    let host_key = format!("spend-donut-{}-{:?}", period.key(), color_scheme);
    let series_fingerprint = series_key.wrapping_add(total_spend);

    let series_changed = DONUT_SERIES.with(|cache| {
        let mut cache = cache.borrow_mut();
        match cache.get(&host_key) {
            Some(previous) if *previous == series_fingerprint => false,
            _ => {
                cache.insert(host_key.clone(), series_fingerprint);
                true
            }
        }
    });
    if series_changed {
        DONUT_MOUNTS.with(|mounts| {
            if let Some(native) = mounts.borrow().get(&host_key).cloned()
                && let Err(error) = crate::acrylic::install_spend_donut_into(native, &xaml)
            {
                eprintln!("Could not update spend donut: {error:?}");
            }
        });
    }

    let xaml_for_mount = xaml.clone();
    let key_for_mount = host_key.clone();
    let key_for_unmount = host_key.clone();
    let mut host = swap_chain_panel().width(SIZE).height(SIZE);
    host.mounted = Some(Callback::new(
        move |native: Option<windows_core::IInspectable>| {
            if let Some(native) = native {
                if let Err(error) =
                    crate::acrylic::install_spend_donut_into(native.clone(), &xaml_for_mount)
                {
                    eprintln!("Could not install spend donut: {error:?}");
                }
                DONUT_MOUNTS.with(|mounts| {
                    mounts.borrow_mut().insert(key_for_mount.clone(), native);
                });
            }
        },
    ));
    host.unmounted = Some(Callback::new(
        move |native: Option<windows_core::IInspectable>| {
            if let Some(native) = native {
                let _ = crate::acrylic::clear_children(native);
            }
            DONUT_MOUNTS.with(|mounts| {
                mounts.borrow_mut().remove(&key_for_unmount);
            });
            DONUT_SERIES.with(|cache| {
                cache.borrow_mut().remove(&key_for_unmount);
            });
        },
    ));
    let donut: Element = host.with_key(host_key).into();

    grid((
        donut,
        text_block(format_spend(total_spend))
            .font_size(18.0)
            .font_weight(600)
            .horizontal_alignment(HorizontalAlignment::Center)
            .vertical_alignment(VerticalAlignment::Center),
    ))
    .columns([GridLength::Auto])
    .rows([GridLength::Auto])
    .width(SIZE)
    .height(SIZE)
    .vertical_alignment(VerticalAlignment::Center)
    .into()
}

fn combined_usage_donut_xaml(
    entries: &[(ProviderKind, u64)],
    total_spend: u64,
    color_scheme: ColorScheme,
) -> String {
    const CENTER: f64 = 62.0;
    const OUTER_RADIUS: f64 = 53.0;
    const INNER_RADIUS: f64 = 34.0;
    const GAP_DEGREES: f64 = 2.0;

    let paths = if total_spend == 0 {
        donut_path("#787878", -90.0, 270.0, CENTER, OUTER_RADIUS, INNER_RADIUS)
    } else {
        let mut start = -90.0;
        entries
            .iter()
            .filter(|(_, spend)| *spend > 0)
            .map(|(provider, spend)| {
                let end = start + *spend as f64 / total_spend as f64 * 360.0;
                let path = donut_path(
                    &xaml_color(combined_usage_color(*provider, color_scheme)),
                    start + GAP_DEGREES / 2.0,
                    end - GAP_DEGREES / 2.0,
                    CENTER,
                    OUTER_RADIUS,
                    INNER_RADIUS,
                );
                start = end;
                path
            })
            .collect::<String>()
    };

    format!(
        r#"<Grid xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Width="124" Height="124">{paths}</Grid>"#
    )
}

fn donut_path(color: &str, start: f64, end: f64, center: f64, outer: f64, inner: f64) -> String {
    let sweep = (end - start).max(0.0);
    if sweep <= 0.0 {
        return String::new();
    }
    if sweep >= 359.0 {
        return format!(
            r#"<Path Fill="{color}" Data="M {center:.2} {outer_top:.2} A {outer:.2} {outer:.2} 0 1 1 {center:.2} {outer_bottom:.2} A {outer:.2} {outer:.2} 0 1 1 {center:.2} {outer_top:.2} M {center:.2} {inner_top:.2} A {inner:.2} {inner:.2} 0 1 0 {center:.2} {inner_bottom:.2} A {inner:.2} {inner:.2} 0 1 0 {center:.2} {inner_top:.2} Z" />"#,
            outer_top = center - outer,
            outer_bottom = center + outer,
            inner_top = center - inner,
            inner_bottom = center + inner,
        );
    }
    let (outer_start_x, outer_start_y) = donut_point(center, outer, start);
    let (outer_end_x, outer_end_y) = donut_point(center, outer, end);
    let (inner_start_x, inner_start_y) = donut_point(center, inner, start);
    let (inner_end_x, inner_end_y) = donut_point(center, inner, end);
    let large_arc = u8::from(sweep > 180.0);
    format!(
        r#"<Path Fill="{color}" Data="M {outer_start_x:.2} {outer_start_y:.2} A {outer:.2} {outer:.2} 0 {large_arc} 1 {outer_end_x:.2} {outer_end_y:.2} L {inner_end_x:.2} {inner_end_y:.2} A {inner:.2} {inner:.2} 0 {large_arc} 0 {inner_start_x:.2} {inner_start_y:.2} Z" />"#
    )
}

fn donut_point(center: f64, radius: f64, degrees: f64) -> (f64, f64) {
    let radians = degrees.to_radians();
    (
        center + radius * radians.cos(),
        center + radius * radians.sin(),
    )
}

fn xaml_color(color: Color) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
}

fn combined_usage_row(provider: ProviderKind, spend: u64, color_scheme: ColorScheme) -> Element {
    grid((
        hstack((
            Shape::ellipse()
                .fill(combined_usage_color(provider, color_scheme))
                .width(9.0)
                .height(9.0)
                .vertical_alignment(VerticalAlignment::Center),
            body_strong(provider.display_name()),
        ))
        .spacing(8.0)
        .vertical_alignment(VerticalAlignment::Center),
        body_strong(format_spend(spend))
            .horizontal_alignment(HorizontalAlignment::Right)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(1),
    ))
    .columns([GridLength::Star(1.0), GridLength::Auto])
    .rows([GridLength::Auto])
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn combined_usage_grouped_totals(
    entries: &[(ProviderKind, u64)],
    color_scheme: ColorScheme,
) -> Element {
    let column_count = entries.len().clamp(1, 3);
    let row_count = entries.len().div_ceil(column_count);
    let cells: Vec<Element> = entries
        .iter()
        .enumerate()
        .map(|(index, (provider, spend))| {
            vstack((
                hstack((
                    Shape::ellipse()
                        .fill(combined_usage_color(*provider, color_scheme))
                        .width(9.0)
                        .height(9.0)
                        .vertical_alignment(VerticalAlignment::Center),
                    body_strong(provider.display_name()),
                ))
                .spacing(7.0)
                .vertical_alignment(VerticalAlignment::Center),
                body_strong(format_spend(*spend)),
            ))
            .spacing(4.0)
            .grid_row((index / column_count) as i32)
            .grid_column((index % column_count) as i32)
            .into()
        })
        .collect();

    grid(cells)
        .columns(vec![GridLength::Star(1.0); column_count])
        .rows(vec![GridLength::Auto; row_count])
        .row_spacing(10.0)
        .column_spacing(14.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into()
}

fn combined_usage_color(provider: ProviderKind, color_scheme: ColorScheme) -> Color {
    match provider {
        ProviderKind::Codex => Color::rgb(128, 159, 255),
        ProviderKind::Claude => Color::rgb(217, 119, 87),
        ProviderKind::Cursor => match color_scheme {
            ColorScheme::Light => Color::rgb(18, 18, 18),
            ColorScheme::Dark => Color::rgb(230, 230, 230),
        },
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => match color_scheme {
            ColorScheme::Light => Color::rgb(75, 75, 75),
            ColorScheme::Dark => Color::rgb(205, 205, 205),
        },
        ProviderKind::OpenRouter => Color::rgb(200, 255, 0),
    }
}

fn format_spend(microusd: u64) -> String {
    format_usd(microusd as f64 / 1_000_000.0)
}

fn usage_tokens_and_cost_metric(label: &str, tokens: String, cost: String) -> Element {
    vstack((
        caption(label).foreground(ThemeRef::TertiaryText),
        hstack((
            text_block(tokens).font_weight(600),
            caption(format!("≈ {cost}"))
                .foreground(ThemeRef::TertiaryText)
                .vertical_alignment(VerticalAlignment::Center),
        ))
        .spacing(5.0)
        .vertical_alignment(VerticalAlignment::Center),
    ))
    .spacing(1.0)
    .vertical_alignment(VerticalAlignment::Center)
    .into()
}

fn usage_value_metric(label: &str, value: String, requests: u64) -> Element {
    vstack((
        caption(label).foreground(ThemeRef::TertiaryText),
        hstack((
            text_block(value).font_weight(600),
            caption(format!("{requests} requests"))
                .foreground(ThemeRef::TertiaryText)
                .vertical_alignment(VerticalAlignment::Center),
        ))
        .spacing(5.0)
        .vertical_alignment(VerticalAlignment::Center),
    ))
    .spacing(1.0)
    .vertical_alignment(VerticalAlignment::Center)
    .into()
}

fn is_cost_provider(provider: ProviderKind) -> bool {
    matches!(
        provider,
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo
    )
}

/// Compact, screenshot-style activity chart. For long histories, adjacent days
/// are grouped into a single bar so the chart stays legible in the tray popup.
fn usage_activity_chart(statistics: &crate::usage::UsageStatistics, cost_based: bool) -> Element {
    const MAX_BARS: usize = 60;
    const CHART_HEIGHT: f64 = 56.0;
    const BAR_GAP: f64 = 2.0;

    // The popup width is fixed. Subtract its outer stroke, the body padding,
    // and this card's stroke/padding so the first and last bars sit at the
    // same inset as the rest of the card content.
    let chart_width = f64::from(popup::POPUP_WIDTH) - 2.0 - 32.0 - 2.0 - 24.0;

    let days = usize::from(statistics.history_days.max(1));
    let today = Local::now().date_naive();
    let first_day = today - ChronoDuration::days(days.saturating_sub(1) as i64);
    let daily: Vec<u64> = (0..days)
        .map(|index| {
            let date = first_day + ChronoDuration::days(index as i64);
            statistics
                .daily
                .iter()
                .find(|entry| entry.date == date)
                .map(|entry| {
                    if cost_based {
                        entry.usage.estimated_cost_microusd
                    } else {
                        entry.usage.total_tokens()
                    }
                })
                .unwrap_or_default()
        })
        .collect();
    let values = compact_activity_bars(&daily, MAX_BARS);
    let max_value = values.iter().copied().max().unwrap_or(0);
    let bar_width = ((chart_width - BAR_GAP * values.len().saturating_sub(1) as f64)
        / values.len().max(1) as f64)
        .clamp(2.0, 12.0);

    let bars: Vec<Element> = values
        .into_iter()
        .map(|tokens| {
            let height = if max_value == 0 {
                2.0
            } else {
                (CHART_HEIGHT * tokens as f64 / max_value as f64).max(2.0)
            };
            border(Element::Empty)
                .width(bar_width)
                .height(height)
                .corner_radius(1.5)
                .background(ThemeRef::Accent)
                .opacity(if tokens == 0 { 0.2 } else { 1.0 })
                .vertical_alignment(VerticalAlignment::Bottom)
                .into()
        })
        .collect();

    border(
        hstack(bars)
            .spacing(BAR_GAP)
            .height(CHART_HEIGHT)
            .vertical_alignment(VerticalAlignment::Bottom),
    )
    .height(CHART_HEIGHT)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Bottom)
    .into()
}

fn compact_activity_bars(values: &[u64], max_bars: usize) -> Vec<u64> {
    if values.len() <= max_bars || max_bars == 0 {
        return values.to_vec();
    }
    let per_bar = values.len().div_ceil(max_bars);
    values
        .chunks(per_bar)
        .map(|chunk| chunk.iter().copied().sum())
        .collect()
}

fn format_token_count(tokens: u64) -> String {
    match tokens {
        0..=999 => tokens.to_string(),
        1_000..=999_999 => format!("{:.1}K", tokens as f64 / 1_000.0),
        1_000_000..=999_999_999 => format!("{:.1}M", tokens as f64 / 1_000_000.0),
        _ => format!("{:.1}B", tokens as f64 / 1_000_000_000.0),
    }
}

fn format_usd(value: f64) -> String {
    if value >= 1_000_000.0 {
        format!("${:.1}M", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("${:.1}K", value / 1_000.0)
    } else {
        format!("${value:.2}")
    }
}

fn credits_display_value(limits: &RateLimits) -> Option<String> {
    if limits.credits.unlimited {
        return Some("Unlimited".into());
    }
    if !limits.credits.has_credits {
        return None;
    }

    let balance = limits.credits.balance.as_deref()?.trim();
    if balance.is_empty()
        || matches!(
            balance.to_ascii_lowercase().as_str(),
            "none" | "undefined" | "null" | "n/a" | "unavailable"
        )
    {
        None
    } else if limits.credits.has_credits {
        Some(balance.into())
    } else {
        None
    }
}

fn capitalize_plan_name(plan: &str) -> String {
    let plan = plan.trim();
    let mut characters = plan.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };
    format!(
        "{}{}",
        first.to_uppercase(),
        characters.as_str().to_lowercase()
    )
}

fn format_reset_in(reset: Option<DateTime<Utc>>) -> String {
    let Some(reset) = reset else {
        return "Unavailable".into();
    };

    let remaining_minutes = (reset - Utc::now()).num_minutes().max(0);
    let days = remaining_minutes / 1_440;
    let hours = (remaining_minutes % 1_440) / 60;
    let minutes = remaining_minutes % 60;

    if days > 0 {
        if hours > 0 {
            format!("{days}d {hours}h")
        } else {
            format!("{days}d")
        }
    } else if hours > 0 {
        if minutes > 0 {
            format!("{hours}h {minutes}m")
        } else {
            format!("{hours}h")
        }
    } else {
        format!("{minutes}m")
    }
}

fn format_last_updated(sampled_at: DateTime<Utc>, _clock_tick: u64) -> String {
    if sampled_at.timestamp() == 0 {
        return "Waiting for first update...".into();
    }
    let seconds = (Utc::now() - sampled_at).num_seconds().max(0);
    let elapsed = match seconds {
        0..=4 => "just now".into(),
        5..=59 => format!("{seconds} seconds ago"),
        _ => format!("{} minutes ago", seconds / 60),
    };
    format!("Updated {elapsed}")
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::settings::{PopupSurface, PopupVisibility};

    fn plan_limits(plan_type: &str) -> RateLimits {
        RateLimits {
            plan_type: Some(plan_type.into()),
            primary: LimitWindow {
                used_percent: Some(20),
                ..Default::default()
            },
            secondary: LimitWindow {
                used_percent: Some(40),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn all_visible() -> PopupVisibility {
        PopupVisibility::build_defaults()
    }

    fn visibility_with(brick_id: &str, all_tab: bool, provider_tab: bool) -> PopupVisibility {
        let mut visibility = PopupVisibility::build_defaults();
        visibility.set_brick(brick_id, all_tab, provider_tab);
        visibility
    }

    fn assert_unique_section_keys(sections: &[PopupSection]) {
        let keys: HashSet<_> = sections.iter().map(|section| section.key()).collect();
        assert_eq!(
            keys.len(),
            sections.len(),
            "popup sections must not duplicate"
        );
    }

    #[test]
    fn last_activation_uses_window_start() {
        let primary = LimitWindow {
            used_percent: Some(1),
            resets_at: Some(
                chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 7, 10, 16, 8, 0).unwrap(),
            ),
            duration_minutes: Some(300),
        };
        assert_eq!(
            window_started_at(&primary),
            Some(chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 7, 10, 11, 8, 0).unwrap())
        );
        assert_eq!(
            format_last_activation(&RateLimits::default(), None),
            "Never"
        );
    }

    #[test]
    fn unavailable_sample_has_clear_copy() {
        assert_eq!(
            format_last_updated(DateTime::default(), 0),
            "Waiting for first update..."
        );
        assert_eq!(format_reset_in(None), "Unavailable");
    }

    #[test]
    fn popup_refresh_is_sent_to_every_provider_worker() {
        let (codex_tx, codex_rx) = std::sync::mpsc::channel();
        let (claude_tx, claude_rx) = std::sync::mpsc::channel();
        let (cursor_tx, cursor_rx) = std::sync::mpsc::channel();
        let commands = vec![
            (ProviderKind::Codex, codex_tx),
            (ProviderKind::Claude, claude_tx),
            (ProviderKind::Cursor, cursor_tx),
        ];

        assert!(refresh_all_workers(&commands));
        assert_eq!(codex_rx.try_recv(), Ok(WorkerCommand::Refresh));
        assert_eq!(claude_rx.try_recv(), Ok(WorkerCommand::Refresh));
        assert_eq!(cursor_rx.try_recv(), Ok(WorkerCommand::Refresh));
    }

    #[test]
    fn activity_chart_groups_long_histories_without_losing_tokens() {
        assert_eq!(compact_activity_bars(&[2, 3, 5], 60), vec![2, 3, 5]);
        assert_eq!(compact_activity_bars(&[2, 3, 5, 7, 11], 2), vec![10, 18]);
    }

    #[test]
    fn combined_spend_uses_the_selected_time_range() {
        let mut statistics = crate::usage::UsageStatistics::default();
        statistics.today.estimated_cost_microusd = 1_250_000;
        statistics.history.estimated_cost_microusd = 9_750_000;
        statistics.daily.push(crate::usage::DailyTokenUsage {
            date: Local::now().date_naive() - ChronoDuration::days(1),
            usage: crate::usage::TokenUsage {
                estimated_cost_microusd: 2_500_000,
                ..Default::default()
            },
        });

        assert_eq!(
            combined_usage_spend(&statistics, TotalSpendPeriod::Today),
            1_250_000
        );
        assert_eq!(
            combined_usage_spend(&statistics, TotalSpendPeriod::Yesterday),
            2_500_000
        );
        assert_eq!(
            combined_usage_spend(&statistics, TotalSpendPeriod::ThirtyDays),
            9_750_000
        );
        assert_eq!(format_spend(1_250_000), "$1.25");
    }

    #[test]
    fn spend_donut_uses_native_arc_geometry() {
        let xaml = combined_usage_donut_xaml(
            &[
                (ProviderKind::Cursor, 2_000_000),
                (ProviderKind::Claude, 1_000_000),
                (ProviderKind::Codex, 500_000),
            ],
            3_500_000,
            ColorScheme::Dark,
        );

        assert!(xaml.starts_with("<Grid"));
        assert_eq!(xaml.matches("<Path ").count(), 3);
        assert!(xaml.contains(" A 53.00 53.00 "));
        assert!(!xaml.contains("Rectangle"));
    }

    #[test]
    fn usage_statistics_section_respects_its_live_toggle() {
        let mut limits = RateLimits {
            usage: crate::usage::UsageStatistics {
                history: crate::usage::TokenUsage {
                    requests: 1,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        assert!(
            popup_sections(&limits, false).contains(&PopupSection::UsageStatistics)
        );
        let mut hidden_usage = all_visible();
        hidden_usage.set_brick("opencode.usage", false, false);
        let cards = provider_cards(
            ProviderKind::OpenCodeZen,
            true,
            &limits,
            false,
            true,
            &hidden_usage,
            PopupSurface::ProviderTab,
            true,
            false,
            ColorScheme::Dark,
            None,
        );
        assert_eq!(cards.len(), 1);
    }

    #[test]
    fn popup_visibility_hides_codex_resets_on_all_but_shows_on_provider_tab() {
        let mut limits = plan_limits("plus");
        limits.reset_credits = Some(crate::limits::RateLimitResetCreditsSummary {
            available_count: 1,
            ..Default::default()
        });
        let visibility = visibility_with("codex.resets", false, true);
        let all_cards = provider_cards(
            ProviderKind::Codex,
            true,
            &limits,
            false,
            true,
            &visibility,
            PopupSurface::AllTab,
            true,
            false,
            ColorScheme::Dark,
            None,
        );
        let tab_cards = provider_cards(
            ProviderKind::Codex,
            true,
            &limits,
            false,
            true,
            &visibility,
            PopupSurface::ProviderTab,
            true,
            false,
            ColorScheme::Dark,
            None,
        );
        assert_eq!(all_cards.len(), 1);
        assert_eq!(tab_cards.len(), 3);
    }

    #[test]
    fn popup_visibility_union_applies_when_provider_tabs_are_hidden() {
        let visibility = visibility_with("codex.usage", false, true);
        assert!(visibility.is_visible(
            "codex.usage",
            PopupSurface::AllTab,
            false
        ));
    }

    #[test]
    fn popup_section_all_off_drops_provider_from_all_tab() {
        let mut visibility = all_visible();
        visibility.set_provider_all_tab(ProviderKind::Codex, false);
        let widgets = visible_popup_widgets(
            &PopupWidgetKind::default_order(),
            false,
            &visibility,
            true,
            false,
            false,
            false,
            false,
            false,
        );
        assert!(!widgets.contains(&PopupWidgetKind::Codex));
        let limits = plan_limits("plus");
        let tab_cards = provider_cards(
            ProviderKind::Codex,
            true,
            &limits,
            false,
            true,
            &visibility,
            PopupSurface::ProviderTab,
            true,
            false,
            ColorScheme::Dark,
            None,
        );
        assert!(!tab_cards.is_empty());
    }

    #[test]
    fn assert_unique_section_keys(sections: &[PopupSection]) {
        assert_eq!(
            format_reset_in(Some(
                Utc::now() + ChronoDuration::days(2) + ChronoDuration::minutes(1),
            )),
            "2d"
        );
    }

    #[test]
    fn free_to_plus_replaces_monthly_with_session_and_weekly_sections() {
        let free = popup_sections(&plan_limits("free"), false);
        assert_eq!(free, vec![PopupSection::Monthly]);
        assert_unique_section_keys(&free);

        let plus = popup_sections(&plan_limits("plus"), false);
        assert_eq!(plus, vec![PopupSection::FiveHour, PopupSection::Weekly,]);
        assert_unique_section_keys(&plus);
    }

    #[test]
    fn disabled_five_hour_session_is_omitted_from_popup() {
        let mut limits = plan_limits("plus");
        limits.primary = LimitWindow::default();

        let sections = popup_sections(&limits, false);
        assert_eq!(sections, vec![PopupSection::Weekly]);
        assert_unique_section_keys(&sections);
    }

    #[test]
    fn zen_without_quota_windows_does_not_render_placeholder_limit_cards() {
        let limits = RateLimits {
            plan_type: Some("Zen · 2 models".into()),
            usage: crate::usage::UsageStatistics {
                history: crate::usage::TokenUsage {
                    requests: 1,
                    estimated_cost_microusd: 1_250_000,
                    priced_requests: 1,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            popup_sections(&limits, false),
            vec![PopupSection::UsageStatistics]
        );
    }

    #[test]
    fn plan_names_use_sentence_case() {
        assert_eq!(capitalize_plan_name("PLUS"), "Plus");
        assert_eq!(capitalize_plan_name("  pro  "), "Pro");
    }

    #[test]
    fn credits_only_render_for_a_real_balance_or_unlimited_access() {
        let mut limits = plan_limits("plus");
        limits.credits.has_credits = true;
        limits.credits.balance = Some("undefined".into());
        assert_eq!(credits_display_value(&limits), None);
        assert!(!popup_sections(&limits, false).contains(&PopupSection::Credits));

        limits.credits.balance = Some("$12.50".into());
        assert_eq!(credits_display_value(&limits).as_deref(), Some("$12.50"));
        assert!(popup_sections(&limits, false).contains(&PopupSection::Credits));

        limits.credits = Default::default();
        limits.credits.unlimited = true;
        assert_eq!(credits_display_value(&limits).as_deref(), Some("Unlimited"));
    }

    #[test]
    fn provider_cards_include_each_additional_limit() {
        let mut limits = plan_limits("plus");
        limits
            .additional_limits
            .push(crate::limits::AdditionalLimit {
                id: "seven_day_fable".into(),
                title: "Fable".into(),
                window: LimitWindow {
                    used_percent: Some(42),
                    ..Default::default()
                },
            });

        let cards = provider_cards(
            ProviderKind::Claude,
            true,
            &limits,
            false,
            true,
            &all_visible(),
            PopupSurface::ProviderTab,
            true,
            false,
            ColorScheme::Dark,
            None,
        );
        // Heading + 5h + weekly + Fable (no separate plan metadata row).
        assert_eq!(cards.len(), 4);
    }

    #[test]
    fn sections_keep_banked_resets_singleton() {
        let mut limits = plan_limits("plus");
        limits.reset_credits = Some(crate::limits::RateLimitResetCreditsSummary {
            available_count: 1,
            ..Default::default()
        });

        let sections = popup_sections(&limits, true);
        assert_eq!(
            sections,
            vec![
                PopupSection::Error,
                PopupSection::FiveHour,
                PopupSection::Weekly,
                PopupSection::BankedResets,
            ]
        );
        assert_unique_section_keys(&sections);
    }

    #[test]
    fn banked_resets_section_is_available_when_data_exists() {
        let mut limits = plan_limits("plus");
        limits.reset_credits = Some(crate::limits::RateLimitResetCreditsSummary {
            available_count: 1,
            ..Default::default()
        });

        assert!(popup_sections(&limits, false).contains(&PopupSection::BankedResets));
    }

    #[test]
    fn every_limits_sample_forces_a_reactive_state_change() {
        let mut ui = UiState::default();
        let initial = ui.clone();

        ui.observe_limits_update();
        assert_ne!(ui, initial);
        assert_eq!(ui.limits_revision, 1);

        // A Plus sample can have the same footer metadata as the preceding
        // Free sample; the revision still guarantees a rerender of the shared
        // snapshot.
        ui.observe_limits_update();
        assert_eq!(ui.limits_revision, 2);
        assert_eq!(ui.last_activation, initial.last_activation);
        assert_eq!(ui.error, initial.error);
    }

    #[test]
    fn pager_queues_only_the_latest_destination() {
        let state = reduce_pager(PagerState::default(), PagerAction::Select(PopupView::Codex));
        assert_eq!(state.outgoing, Some(PopupView::All));
        assert_eq!(state.current, PopupView::Codex);
        assert_eq!(state.direction, PagerDirection::Forward);

        let animation_id = state.animation_id;
        let state = reduce_pager(state, PagerAction::Select(PopupView::Claude));
        let state = reduce_pager(state, PagerAction::Select(PopupView::Cursor));
        assert_eq!(state.pending, Some(PopupView::Cursor));

        let state = reduce_pager(state, PagerAction::AnimationFinished(animation_id));
        assert_eq!(state.outgoing, Some(PopupView::Codex));
        assert_eq!(state.current, PopupView::Cursor);
        assert_eq!(state.pending, None);
        assert_eq!(state.direction, PagerDirection::Forward);
    }

    #[test]
    fn pager_uses_reverse_motion_for_an_earlier_tab() {
        let state = PagerState {
            current: PopupView::Cursor,
            ..PagerState::default()
        };
        let state = reduce_pager(state, PagerAction::Select(PopupView::All));
        assert_eq!(state.outgoing, Some(PopupView::Cursor));
        assert_eq!(state.current, PopupView::All);
        assert_eq!(state.direction, PagerDirection::Backward);
        assert!(state.direction.outgoing_offset() > 0.0);
        assert!(state.direction.incoming_offset() < 0.0);
    }

    #[test]
    fn every_provider_membership_has_the_expected_tab_order() {
        let default_order = PopupWidgetKind::default_order();
        for mask in 0_u8..64 {
            let codex = mask & 0b001 != 0;
            let claude = mask & 0b010 != 0;
            let cursor = mask & 0b100 != 0;
            let opencode_zen = mask & 0b01000 != 0;
            let opencode_go = mask & 0b10000 != 0;
            let openrouter = mask & 0b100000 != 0;
            let views = enabled_popup_views(
                &default_order,
                codex,
                claude,
                cursor,
                opencode_zen,
                opencode_go,
                openrouter,
            );
            let providers = provider_order_from_popup(&default_order);

            assert_eq!(views.first(), Some(&PopupView::All));
            assert_eq!(views.contains(&PopupView::Codex), codex);
            assert_eq!(views.contains(&PopupView::Claude), claude);
            assert_eq!(views.contains(&PopupView::Cursor), cursor);
            assert_eq!(views.contains(&PopupView::OpenCodeZen), opencode_zen);
            assert_eq!(views.contains(&PopupView::OpenCodeGo), opencode_go);
            assert_eq!(views.contains(&PopupView::OpenRouter), openrouter);
            assert!(
                views
                    .windows(2)
                    .all(|pair| pair[0].order(&providers) < pair[1].order(&providers))
            );
            assert_eq!(
                views.len(),
                1 + usize::from(codex)
                    + usize::from(claude)
                    + usize::from(cursor)
                    + usize::from(opencode_zen)
                    + usize::from(opencode_go)
                    + usize::from(openrouter)
            );
        }

        let reversed = vec![
            PopupWidgetKind::TotalSpend,
            PopupWidgetKind::Cursor,
            PopupWidgetKind::Claude,
            PopupWidgetKind::Codex,
            PopupWidgetKind::OpenCodeZen,
            PopupWidgetKind::OpenCodeGo,
            PopupWidgetKind::OpenRouter,
        ];
        let views = enabled_popup_views(&reversed, true, true, true, true, true, true);
        assert_eq!(
            views,
            vec![
                PopupView::All,
                PopupView::Cursor,
                PopupView::Claude,
                PopupView::Codex,
                PopupView::OpenCodeZen,
                PopupView::OpenCodeGo,
                PopupView::OpenRouter,
            ]
        );
    }

    #[test]
    fn stale_pager_completion_cannot_end_a_newer_transition() {
        let state = reduce_pager(PagerState::default(), PagerAction::Select(PopupView::Codex));
        let unchanged = reduce_pager(
            state.clone(),
            PagerAction::AnimationFinished(state.animation_id.wrapping_sub(1)),
        );
        assert_eq!(unchanged, state);
    }
}
