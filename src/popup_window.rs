use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender},
    },
    thread,
    time::Duration,
};

use chrono::{DateTime, Duration as ChronoDuration, Local, Utc};
use windows_reactor::*;

use crate::{
    limits::{LimitWindow, PaceTip, ProviderLimits, RateLimits},
    notifications,
    notifications::LimitNotificationTracker,
    popup,
    settings::{
        NotificationSettings, ProviderKind, Settings, TotalSpendPresentation, TrayWidget,
    },
    settings_controls::update_accent_button,
    tray::{TrayManager, TrayMenuAction},
    updater::{UpdateController, UpdatePhase},
    worker::{WorkerCommand, WorkerEvent},
};

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
        if let Ok(mut current) = self.limits.lock() {
            // Quota polling must not erase the independently refreshed usage
            // history between its ten-minute scans.
            limits.usage = current.get(provider).usage.clone();
            *current.get_mut(provider) = limits;
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
    fn sync_provider_workers(&self, settings: &Settings) -> Vec<String> {
        let disabled = [
            ProviderKind::Codex,
            ProviderKind::Claude,
            ProviderKind::Cursor,
        ]
        .into_iter()
        .filter(|provider| !settings.providers.is_enabled(*provider))
        .collect::<Vec<_>>();
        let stopped = self.workers.lock().map_or_else(
            |_| Vec::new(),
            |mut workers| {
                disabled
                    .iter()
                    .filter_map(|provider| workers.remove(provider))
                    .collect()
            },
        );
        for worker in stopped {
            worker.shutdown();
        }
        if let Ok(mut commands) = self.commands.lock() {
            commands.retain(|provider, _| settings.providers.is_enabled(*provider));
        }
        if let Ok(mut limits) = self.limits.lock() {
            for provider in &disabled {
                *limits.get_mut(*provider) = RateLimits::default();
            }
        }

        let mut errors = Vec::new();
        for provider in [
            ProviderKind::Codex,
            ProviderKind::Claude,
            ProviderKind::Cursor,
        ] {
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
    show_banked_resets: bool,
    show_usage_stats: bool,
    show_total_spend_on_all_tab: bool,
    total_spend_presentation: TotalSpendPresentation,
    show_account_name: bool,
    codex_enabled: bool,
    claude_enabled: bool,
    cursor_enabled: bool,
    use_colored_provider_icons: bool,
    replace_chatgpt_logo_with_codex: bool,
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
            last_activation: "Never".into(),
            error: None,
            limits_revision: 0,
            refreshing: false,
            show_used_percentage: false,
            show_usage_pace: true,
            show_banked_resets: true,
            show_usage_stats: true,
            show_total_spend_on_all_tab: true,
            total_spend_presentation: TotalSpendPresentation::default(),
            show_account_name: false,
            codex_enabled: true,
            claude_enabled: false,
            cursor_enabled: false,
            use_colored_provider_icons: false,
            replace_chatgpt_logo_with_codex: false,
            update_version: None,
        }
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
}

/// Ephemeral time range for the compact spend card on the All tab.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum CombinedUsagePeriod {
    Today,
    Yesterday,
    #[default]
    ThirtyDays,
}

impl CombinedUsagePeriod {
    const fn label(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Yesterday => "Yesterday",
            Self::ThirtyDays => "30 Days",
        }
    }

    const fn key(self) -> &'static str {
        match self {
            Self::Today => "today",
            Self::Yesterday => "yesterday",
            Self::ThirtyDays => "30-days",
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

fn popup_sections(
    limits: &RateLimits,
    show_banked_resets: bool,
    show_usage_stats: bool,
    has_error: bool,
) -> Vec<PopupSection> {
    let mut sections = Vec::with_capacity(6);
    if has_error {
        sections.push(PopupSection::Error);
    }
    if limits.is_free_plan() {
        sections.push(PopupSection::Monthly);
    } else {
        if !limits.five_hour_disabled() {
            sections.push(PopupSection::FiveHour);
        }
        sections.push(PopupSection::Weekly);
    }
    if show_banked_resets && limits.available_reset_count() > 0 {
        sections.push(PopupSection::BankedResets);
    }
    if show_usage_stats && limits.usage.has_data() {
        sections.push(PopupSection::UsageStatistics);
    }
    if credits_display_value(limits).is_some() {
        sections.push(PopupSection::Credits);
    }
    sections
}

fn provider_cards(
    provider: ProviderKind,
    is_first: bool,
    limits: &RateLimits,
    show_used_percentage: bool,
    show_usage_pace: bool,
    show_banked_resets: bool,
    show_usage_stats: bool,
    show_account_name: bool,
    color_scheme: ColorScheme,
) -> Vec<Element> {
    let (monthly_label, primary_label, secondary_label) = match provider {
        ProviderKind::Cursor => ("Auto + Composer", "Auto + Composer", "Auto + Composer"),
        _ => ("Monthly", "5h Session", "Weekly"),
    };
    let account_heading: Element = if show_account_name {
        limits
            .account_name
            .as_ref()
            .map(|name| {
                caption(name.clone())
                    .foreground(ThemeRef::TertiaryText)
                    .horizontal_alignment(HorizontalAlignment::Right)
                    .vertical_alignment(VerticalAlignment::Center)
                    .grid_column(1)
                    .into()
            })
            .unwrap_or(Element::Empty)
    } else {
        Element::Empty
    };
    let mut cards: Vec<Element> = vec![
        grid((
            hstack((
                body_strong(provider.display_name())
                    .foreground(ThemeRef::SecondaryText)
                    .vertical_alignment(VerticalAlignment::Center),
                limits
                    .plan_type
                    .as_deref()
                    .filter(|plan| !plan.trim().is_empty())
                    .map(|plan| {
                        text_block(capitalize_plan_name(plan))
                            .font_weight(400)
                            .foreground(ThemeRef::TertiaryText)
                            .vertical_alignment(VerticalAlignment::Center)
                            .into()
                    })
                    .unwrap_or(Element::Empty),
            ))
            .spacing(4.0)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(0),
            account_heading,
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
        .with_key(format!("{}-heading", provider.display_name()))
        .into(),
    ];
    // Cursor usage is fetched from a remote CSV export rather than scanned
    // from a local session log. Keep its card visible while that export is
    // still empty or delayed, so the feature does not look like it vanished.
    let has_usage_statistics =
        show_usage_stats && (limits.usage.has_data() || provider == ProviderKind::Cursor);
    cards.extend(
        popup_sections(limits, show_banked_resets, show_usage_stats, false)
            .into_iter()
            .filter(|section| {
                matches!(
                    section,
                    PopupSection::Monthly | PopupSection::FiveHour | PopupSection::Weekly
                )
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
    let additional_limits = limits.additional_limits.iter().map(|limit| {
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
        ))
    });
    cards.extend(additional_limits);
    // Local statistics remain after every rate-limit window.
    if show_banked_resets && limits.available_reset_count() > 0 {
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
    if credits_display_value(limits).is_some() {
        cards.push(credits_card(limits).with_key(format!("{}-credits", provider.display_name())));
    }
    cards
}

fn latest_sampled_at(limits: &ProviderLimits) -> chrono::DateTime<Utc> {
    [
        limits.codex.sampled_at,
        limits.claude.sampled_at,
        limits.cursor.sampled_at,
    ]
    .into_iter()
    .max()
    .unwrap_or_default()
}

/// Root WinUI view for Codex Minibar (hosted in a tray popup shell).
pub fn app(cx: &mut RenderCx, state: Arc<AppState>) -> Element {
    let dpi = cx.use_dpi().max(1);
    let color_scheme = cx.use_color_scheme();
    let window_corner_radius = f64::from(popup::WINDOW_CORNER_RADIUS_DIP);
    // Keep the visual stroke one physical pixel inside the HWND clip so GDI's
    // aliased region cannot trim its anti-aliased XAML corner pixels.
    let border_inset = 96.0 / f64::from(dpi);
    let inner_corner_radius = (window_corner_radius - border_inset).max(0.0);
    let (ui, set_ui) = cx.use_async_state(UiState {
        error: state.startup_error.clone(),
        last_activation: format_last_activation(&RateLimits::default(), state.last_activation_at),
        show_used_percentage: state.settings.show_used_percentage,
        show_usage_pace: state.settings.show_usage_pace,
        show_banked_resets: state.settings.show_banked_resets,
        show_usage_stats: state.settings.show_usage_stats,
        show_total_spend_on_all_tab: state.settings.show_total_spend_on_all_tab,
        show_account_name: state.settings.show_account_name,
        codex_enabled: state.settings.providers.codex_enabled,
        claude_enabled: state.settings.providers.claude_enabled,
        cursor_enabled: state.settings.providers.cursor_enabled,
        use_colored_provider_icons: state.settings.use_colored_provider_icons,
        replace_chatgpt_logo_with_codex: state.settings.replace_chatgpt_logo_with_codex,
        update_version: state
            .updates
            .available_update()
            .map(|update| update.version),
        ..UiState::default()
    });
    // Rendering observes the same snapshot that the tray consumes; UiState
    // deliberately contains only view metadata, never a second copy of limits.
    let limits = state.current_limits();
    let commands = state.worker_commands();
    let ui_dispatcher = cx.use_ui_marshaller();
    let settings_tx = state.settings_tx.clone();
    let (hovered_action, set_hovered_action) = cx.use_state(Option::<String>::None);
    let (selected_view, set_selected_view) = cx.use_state(PopupView::All);
    let (combined_usage_period, set_combined_usage_period) =
        cx.use_state(CombinedUsagePeriod::default());
    let (hovered_combined_usage_period, set_hovered_combined_usage_period) =
        cx.use_state(None::<CombinedUsagePeriod>);
    // Relative timestamps need a render tick even while the popup receives no
    // input or provider event. This changes only the elapsed-time label; it
    // never requests fresh limits.
    let (clock_tick, set_clock_tick) = cx.use_async_state(0_u64);

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
                    thread::sleep(Duration::from_secs(1));
                    tick = tick.wrapping_add(1);
                    set_clock_tick.call(tick);
                }
            });
        }
    });

    let refresh = {
        let commands = commands.clone();
        let set_ui = set_ui.clone();
        let ui = ui.clone();
        move || {
            if commands
                .iter()
                .any(|(_, commands)| commands.send(WorkerCommand::Refresh).is_ok())
            {
                let mut ui = ui.clone();
                ui.refreshing = true;
                set_ui.call(ui);
            }
        }
    };
    let quit = move || std::process::exit(0);

    // A selector only earns its keep when it can actually switch between
    // providers. With zero or one enabled provider the familiar compact
    // footer remains, sparing us some very professional-looking empty UI.
    let enabled_provider_count = [ui.codex_enabled, ui.claude_enabled, ui.cursor_enabled]
        .into_iter()
        .filter(|enabled| *enabled)
        .count();
    let show_provider_tabs = enabled_provider_count > 1;
    let selected_view = match selected_view {
        PopupView::Codex if !ui.codex_enabled => PopupView::All,
        PopupView::Claude if !ui.claude_enabled => PopupView::All,
        PopupView::Cursor if !ui.cursor_enabled => PopupView::All,
        view => view,
    };
    let show_codex = ui.codex_enabled
        && (!show_provider_tabs || matches!(selected_view, PopupView::All | PopupView::Codex));
    let show_claude = ui.claude_enabled
        && (!show_provider_tabs || matches!(selected_view, PopupView::All | PopupView::Claude));
    let show_cursor = ui.cursor_enabled
        && (!show_provider_tabs || matches!(selected_view, PopupView::All | PopupView::Cursor));
    // Individual activity remains a provider-detail view. The All tab has its
    // own compact, provider-by-provider summary when explicitly enabled.
    let show_usage_stats =
        ui.show_usage_stats && (!show_provider_tabs || selected_view != PopupView::All);
    let show_total_spend = ui.show_total_spend_on_all_tab
        && show_provider_tabs
        && selected_view == PopupView::All;

    let mut body: Vec<Element> = Vec::new();
    // Provider headings add a small top gap unless they are the first visible
    // section. Total Spend and an error banner count as sections too.
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
    if show_total_spend {
        body.push(
            combined_usage_card(
                &limits,
                ui.codex_enabled,
                ui.claude_enabled,
                ui.cursor_enabled,
                combined_usage_period,
                set_combined_usage_period.clone(),
                hovered_combined_usage_period,
                set_hovered_combined_usage_period.clone(),
                color_scheme,
                ui.total_spend_presentation,
            )
            .with_key(format!(
                "all-combined-usage-{}-{:?}",
                combined_usage_period.key(),
                ui.total_spend_presentation
            )),
        );
        has_preceding_section = true;
    }
    if show_codex {
        body.push(
            vstack(provider_cards(
                ProviderKind::Codex,
                !has_preceding_section,
                &limits.codex,
                ui.show_used_percentage,
                ui.show_usage_pace,
                ui.show_banked_resets,
                show_usage_stats,
                ui.show_account_name,
                color_scheme,
            ))
            .spacing(6.0)
            .with_key("provider-codex")
            .into(),
        );
        has_preceding_section = true;
    }
    if show_claude {
        body.push(
            vstack(provider_cards(
                ProviderKind::Claude,
                !has_preceding_section,
                &limits.claude,
                ui.show_used_percentage,
                ui.show_usage_pace,
                ui.show_banked_resets,
                show_usage_stats,
                ui.show_account_name,
                color_scheme,
            ))
            .spacing(6.0)
            .with_key("provider-claude")
            .into(),
        );
        has_preceding_section = true;
    }
    if show_cursor {
        body.push(
            vstack(provider_cards(
                ProviderKind::Cursor,
                !has_preceding_section,
                &limits.cursor,
                ui.show_used_percentage,
                ui.show_usage_pace,
                false,
                show_usage_stats,
                ui.show_account_name,
                color_scheme,
            ))
            .spacing(6.0)
            .with_key("provider-cursor")
            .into(),
        );
    }
    if !ui.codex_enabled && !ui.claude_enabled && !ui.cursor_enabled {
        body.push(
            InfoBar::new("No providers enabled")
                .message("Enable Codex, Claude, or Cursor in Settings > Providers.")
                .is_closable(false)
                .with_key("popup-no-providers")
                .into(),
        );
    }

    let quit_or_update = if ui.update_version.is_some() {
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
        .into()
    } else {
        icon_button(
            "quit",
            "fluent-power",
            "fluent-power",
            "Quit",
            &hovered_action,
            set_hovered_action.clone(),
            quit,
        )
    };
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
        hstack((
            popup_tab_button(
                "provider-tab-all",
                None,
                Some("All"),
                "All providers",
                selected_view == PopupView::All,
                ui.use_colored_provider_icons,
                &hovered_action,
                set_hovered_action.clone(),
                {
                    let set_selected_view = set_selected_view.clone();
                    move || set_selected_view.call(PopupView::All)
                },
            ),
            if ui.codex_enabled {
                popup_tab_button(
                    "provider-tab-codex",
                    Some(if ui.replace_chatgpt_logo_with_codex {
                        "codex"
                    } else {
                        "chatgpt"
                    }),
                    None,
                    "Codex",
                    selected_view == PopupView::Codex,
                    ui.use_colored_provider_icons,
                    &hovered_action,
                    set_hovered_action.clone(),
                    {
                        let set_selected_view = set_selected_view.clone();
                        move || set_selected_view.call(PopupView::Codex)
                    },
                )
            } else {
                Element::Empty
            },
            if ui.claude_enabled {
                popup_tab_button(
                    "provider-tab-claude",
                    Some("claude"),
                    None,
                    "Claude",
                    selected_view == PopupView::Claude,
                    ui.use_colored_provider_icons,
                    &hovered_action,
                    set_hovered_action.clone(),
                    {
                        let set_selected_view = set_selected_view.clone();
                        move || set_selected_view.call(PopupView::Claude)
                    },
                )
            } else {
                Element::Empty
            },
            if ui.cursor_enabled {
                popup_tab_button(
                    "provider-tab-cursor",
                    Some("cursor"),
                    None,
                    "Cursor",
                    selected_view == PopupView::Cursor,
                    ui.use_colored_provider_icons,
                    &hovered_action,
                    set_hovered_action.clone(),
                    {
                        let set_selected_view = set_selected_view.clone();
                        move || set_selected_view.call(PopupView::Cursor)
                    },
                )
            } else {
                Element::Empty
            },
        ))
        .spacing(2.0)
        .horizontal_alignment(HorizontalAlignment::Left)
        .vertical_alignment(VerticalAlignment::Center)
        // Provider marks are native swap-chain children. Recreate the whole
        // selector when its membership changes; otherwise WinUI reconciliation
        // can retain a prior tab's text/icon in a newly occupied slot.
        .with_key(format!(
            "provider-tabs-{}-{}-{}",
            ui.codex_enabled, ui.claude_enabled, ui.cursor_enabled
        ))
        .into()
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
            hstack((
                icon_button(
                    "refresh",
                    "fluent-refresh",
                    "fluent-refresh",
                    &refresh_tooltip,
                    &hovered_action,
                    set_hovered_action.clone(),
                    refresh,
                ),
                icon_button(
                    "settings",
                    "fluent-settings",
                    "fluent-settings",
                    "Settings",
                    &hovered_action,
                    set_hovered_action.clone(),
                    {
                        let settings_tx = settings_tx.clone();
                        let updates = Arc::clone(&state.updates);
                        move || {
                            if let Err(error) =
                                crate::settings_window::open(settings_tx.clone(), updates.clone())
                            {
                                eprintln!("Could not open settings window: {error:?}");
                            }
                        }
                    },
                ),
                quit_or_update,
            ))
            .spacing(4.0)
            .horizontal_alignment(HorizontalAlignment::Right)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(1),
        ))
        .rows([GridLength::Auto])
        .columns([GridLength::Star(1.0), GridLength::Auto])
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
    let body_layout_key = format!(
        "popup-scroll-{}-{:?}-{}-{}-{}-{:?}-{}-{}-{}-{}-{:?}-{:?}",
        ui.limits_revision,
        ui.error,
        ui.show_banked_resets,
        ui.show_usage_stats,
        ui.show_total_spend_on_all_tab,
        ui.total_spend_presentation,
        ui.show_account_name,
        ui.codex_enabled,
        ui.claude_enabled,
        ui.cursor_enabled,
        color_scheme as i32,
        selected_view,
    );
    let scrollable_body = scroll_viewer(
        vstack(body)
            .spacing(6.0)
            .padding(Thickness {
                left: 16.0,
                top: 16.0,
                right: 16.0,
                bottom: 16.0,
            })
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Top)
            .on_resize(|_width, height| {
                popup::set_client_height_from_body_content(height);
            })
            .with_key(body_layout_key),
    )
    .horizontal_scroll_bar_visibility(ScrollBarVisibility::Disabled)
    .vertical_scroll_bar_visibility(ScrollBarVisibility::Auto)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Stretch)
    .grid_row(0);

    let body_panel = border(
        grid((scrollable_body, footer.grid_row(1)))
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
    .vertical_alignment(VerticalAlignment::Top);

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
        host
    };

    let chrome = border(
        grid((mica, body_panel))
            .rows([GridLength::Star(1.0)])
            .columns([GridLength::Star(1.0)])
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Top)
            .background(Color::transparent()),
    )
    .padding(Thickness::uniform(border_inset))
    .corner_radius(window_corner_radius)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Top);

    chrome.into()
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
    let mut widgets = state.settings.tray_widgets.clone();
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
            error: state.startup_error.clone(),
            last_activation: format_last_activation(&RateLimits::default(), fallback_attempt),
            show_used_percentage: state.settings.show_used_percentage,
            show_usage_pace: state.settings.show_usage_pace,
            show_banked_resets: state.settings.show_banked_resets,
            show_usage_stats: state.settings.show_usage_stats,
            show_total_spend_on_all_tab: state.settings.show_total_spend_on_all_tab,
            total_spend_presentation: state.settings.total_spend_presentation,
            show_account_name: state.settings.show_account_name,
            codex_enabled: state.settings.providers.codex_enabled,
            claude_enabled: state.settings.providers.claude_enabled,
            cursor_enabled: state.settings.providers.cursor_enabled,
            use_colored_provider_icons: state.settings.use_colored_provider_icons,
            replace_chatgpt_logo_with_codex: state.settings.replace_chatgpt_logo_with_codex,
            update_version: update_version_from_phase(&update_phase),
            ..UiState::default()
        };

        if let Err(error) = tray.sync(
            &widgets,
            &state.current_limits(),
            update_available_from_phase(&update_phase),
        ) {
            ui.error = Some(error.to_string());
            set_ui.call(ui.clone());
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
            let phase = updates.snapshot();
            let providers_changed = ui.codex_enabled != settings.providers.codex_enabled
                || ui.claude_enabled != settings.providers.claude_enabled
                || ui.cursor_enabled != settings.providers.cursor_enabled;
            ui.show_used_percentage = settings.show_used_percentage;
            ui.show_usage_pace = settings.show_usage_pace;
            ui.show_banked_resets = settings.show_banked_resets;
            ui.show_usage_stats = settings.show_usage_stats;
            ui.show_total_spend_on_all_tab = settings.show_total_spend_on_all_tab;
            ui.total_spend_presentation = settings.total_spend_presentation;
            ui.show_account_name = settings.show_account_name;
            ui.codex_enabled = settings.providers.codex_enabled;
            ui.claude_enabled = settings.providers.claude_enabled;
            ui.cursor_enabled = settings.providers.cursor_enabled;
            ui.use_colored_provider_icons = settings.use_colored_provider_icons;
            ui.replace_chatgpt_logo_with_codex = settings.replace_chatgpt_logo_with_codex;
            *notification_settings = settings.notifications.clone();
            *widgets = settings.tray_widgets.clone();
            ui.update_version = update_version_from_phase(&phase);
            // Presentation settings must visibly apply before any background
            // work. In particular, changing provider icons must never wait on
            // a worker lock, network request, or provider lifecycle change.
            set_ui.call(ui.clone());
            if providers_changed {
                let provider_errors = state.sync_provider_workers(&settings);
                if !provider_errors.is_empty() {
                    ui.error = Some(provider_errors.join("\n"));
                }
            }
            // Repaint the existing native icons in place. Recreating them makes
            // Explorer animate a remove/add sequence and causes a visible flash.
            if let Err(error) = tray.sync(
                widgets,
                &state.current_limits(),
                update_available_from_phase(&phase),
            ) {
                ui.error = Some(error.to_string());
            }
            for (_, commands) in state.worker_commands() {
                let _ = commands.send(WorkerCommand::SetAutomaticActivation(
                    settings.automatic_activation,
                ));
                let _ = commands.send(WorkerCommand::SetLimitRefreshInterval(Duration::from_secs(
                    settings.limit_refresh_interval.seconds(),
                )));
                // The worker refreshes immediately after receiving this command,
                // so the selected history range is reflected in the open popup
                // without asking the user to restart the application.
                let _ = commands.send(WorkerCommand::SetHistoryRetentionDays(
                    settings.history_retention_days,
                ));
            }
            set_ui.call(ui.clone());
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
                ui.error = Some(error.to_string());
            }
            set_ui.call(ui.clone());
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
            set_ui.call(ui.clone());
            loop {
                popup::pump_messages();
                drain_toast_update();
                if let Err(error) = tray.refresh_system_theme(&widgets, &state.current_limits()) {
                    ui.error = Some(error.to_string());
                    set_ui.call(ui.clone());
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
                ui.error = Some(error.to_string());
                set_ui.call(ui.clone());
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
                    {
                        continue;
                    }
                    // Publish once, then let both native tray and WinUI render
                    // from that exact snapshot.
                    state.replace_limits(provider, limits);
                    let limits = state.current_limits();
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
                        ui.error = Some(error.to_string());
                    } else {
                        ui.error = None;
                    }
                    if provider == ProviderKind::Codex {
                        ui.last_activation =
                            format_last_activation(limits.get(provider), fallback_attempt);
                    }
                    ui.observe_limits_update();
                    ui.refreshing = false;
                    set_ui.call(ui.clone());
                }
                Ok(WorkerEvent::ProviderUsageUpdated(provider, usage)) => {
                    if (provider == ProviderKind::Codex && !ui.codex_enabled)
                        || (provider == ProviderKind::Claude && !ui.claude_enabled)
                        || (provider == ProviderKind::Cursor && !ui.cursor_enabled)
                    {
                        continue;
                    }
                    state.replace_usage(provider, usage);
                    // Usage stats affect only the popup, but they share the
                    // reactive snapshot revision with quota updates.
                    ui.observe_limits_update();
                    set_ui.call(ui.clone());
                }
                Ok(WorkerEvent::ProviderActivationSucceeded(provider)) => {
                    ui.last_activation = format!(
                        "{} succeeded at {}",
                        provider.display_name(),
                        format_activation_at(Utc::now())
                    );
                    set_ui.call(ui.clone());
                }
                Ok(WorkerEvent::ProviderActivationFailed(provider, error)) => {
                    ui.last_activation = format!(
                        "{} failed at {}: {error}",
                        provider.display_name(),
                        format_activation_at(Utc::now())
                    );
                    set_ui.call(ui.clone());
                }
                Ok(WorkerEvent::ProviderPollFailed(provider, error)) => {
                    ui.error = Some(format!("{}: {error}", provider.display_name()));
                    ui.refreshing = false;
                    set_ui.call(ui.clone());
                }
                // All live provider workers are forwarded as scoped events.
                Ok(
                    WorkerEvent::LimitsUpdated(_)
                    | WorkerEvent::UsageUpdated(_)
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
    _ui: &mut UiState,
    _set_ui: &AsyncSetState<UiState>,
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
                    popup::hide();
                }
            } else {
                // Native showing is allowed only after synchronous WinUI
                // reactivation; otherwise XAML can remain dormant indefinitely.
                let (ready_tx, ready_rx) = std::sync::mpsc::channel();
                ui_dispatcher.dispatch(move || {
                    let _ = ready_tx.send(popup::prepare_show_on_ui_thread());
                });
                match ready_rx.recv_timeout(std::time::Duration::from_millis(500)) {
                    Ok(true) => popup::show_near(x, y),
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

    popup::keep_on_monitor();

    // Settings are a live editor for this surface. Treat the separate settings
    // window as part of the popup interaction so navigating or toggling a
    // setting cannot dismiss the preview beneath it.
    if !crate::settings_window::is_open() && popup::clicked_outside() {
        popup::hide();
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

/// Compact footer selector item for choosing the combined or provider view.
fn popup_tab_button(
    id: &'static str,
    icon_name: Option<&'static str>,
    label: Option<&'static str>,
    tip: &'static str,
    selected: bool,
    use_colored_provider_icons: bool,
    hovered_action: &Option<String>,
    set_hovered_action: SetState<Option<String>>,
    on_click: impl IntoUnitCallback,
) -> Element {
    let hovered = hovered_action.as_deref() == Some(id);
    let set_on_enter = set_hovered_action.clone();
    let set_on_exit = set_hovered_action;
    let neutral_icon_color = if hovered {
        Color::rgb(230, 230, 230)
    } else {
        Color::rgb(190, 190, 190)
    };
    let icon_color = if use_colored_provider_icons {
        match icon_name {
            Some("codex") | Some("chatgpt") => Color::rgb(128, 159, 255),
            Some("claude") => Color::rgb(217, 119, 87),
            Some("cursor") => Color::rgb(255, 255, 255),
            _ => neutral_icon_color,
        }
    } else {
        neutral_icon_color
    };
    let tab_width = if label.is_some() {
        44.0
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
    let content: Element = if let Some(label) = label {
        body_strong(label)
            .foreground(if selected {
                ThemeRef::AccentText
            } else if hovered {
                ThemeRef::PrimaryText
            } else {
                ThemeRef::SecondaryText
            })
            .relative_align_h_center()
            .relative_align_v_center()
            .into()
    } else {
        crate::icons::element(icon_name.expect("provider tab icon"), 18.0, icon_color)
            .relative_align_h_center()
            .relative_align_v_center()
            .into()
    };

    // `SwapChainPanel` runs its icon painter only on mount. Key the complete
    // tab by its appearance so changing either provider mark or tint replaces
    // that native host immediately instead of leaving stale pixels on screen.
    relative_panel(vec![hover_background, content, selection_marker])
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
        .on_tapped(on_click)
        .with_key(format!(
            "{id}-{}-{:02X}{:02X}{:02X}",
            icon_name.unwrap_or("label"),
            icon_color.r,
            icon_color.g,
            icon_color.b
        ))
        .into()
}

/// Icon-only action using a neutral Phosphor SVG that adopts the accent on hover.
fn icon_button(
    id: &'static str,
    normal_icon: &'static str,
    hover_icon: &'static str,
    tip: &str,
    hovered_action: &Option<String>,
    set_hovered_action: SetState<Option<String>>,
    on_click: impl IntoUnitCallback,
) -> Element {
    let hovered = hovered_action.as_deref() == Some(id);
    let set_on_enter = set_hovered_action.clone();
    let set_on_exit = set_hovered_action;
    let hover_background: Element = border(Element::Empty)
        .background(ThemeRef::SubtleFill)
        .opacity(if hovered { 1.0 } else { 0.0 })
        .corner_radius(4.0)
        .relative_align_left()
        .relative_align_right()
        .relative_align_top()
        .relative_align_bottom()
        .into();
    let icon: Element = crate::icons::element(
        if hovered { hover_icon } else { normal_icon },
        18.0,
        if hovered {
            Color::rgb(0, 120, 212)
        } else {
            Color::rgb(230, 230, 230)
        },
    )
    .relative_align_h_center()
    .relative_align_v_center()
    .into();
    relative_panel(vec![hover_background, icon])
        .tooltip(tip)
        .width(ICON_BUTTON_SIZE)
        .height(ICON_BUTTON_SIZE)
        .min_width(ICON_BUTTON_SIZE)
        .min_height(ICON_BUTTON_SIZE)
        .max_width(ICON_BUTTON_SIZE)
        .max_height(ICON_BUTTON_SIZE)
        .background(Color::transparent())
        .on_pointer_entered(move |_: PointerEventInfo| {
            set_on_enter.call(Some(id.to_string()));
        })
        .on_pointer_exited(move || set_on_exit.call(None))
        .on_tapped(on_click)
        .into()
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
    let accent = ThemeRef::SystemAttention;
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
    let reset = format_reset_in(window.resets_at);

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

    let footer: Element = if show_reset {
        grid((
            hstack((text_block(remaining_label)
                .font_weight(600)
                .foreground(accent.clone())
                .vertical_alignment(VerticalAlignment::Center),))
            .vertical_alignment(VerticalAlignment::Center),
            hstack((
                text_block("Resets in")
                    .foreground(ThemeRef::TertiaryText)
                    .vertical_alignment(VerticalAlignment::Center),
                text_block(reset).vertical_alignment(VerticalAlignment::Center),
            ))
            .spacing(6.0)
            .horizontal_alignment(HorizontalAlignment::Right)
            .vertical_alignment(VerticalAlignment::Center),
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
                .foreground(ThemeRef::SystemAttention)
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
                .foreground(ThemeRef::SystemAttention)
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
    let chart = usage_activity_chart(statistics);

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
    codex_enabled: bool,
    claude_enabled: bool,
    cursor_enabled: bool,
    period: CombinedUsagePeriod,
    set_period: SetState<CombinedUsagePeriod>,
    hovered_period: Option<CombinedUsagePeriod>,
    set_hovered_period: SetState<Option<CombinedUsagePeriod>>,
    color_scheme: ColorScheme,
    presentation: TotalSpendPresentation,
) -> Element {
    let providers = [
        (ProviderKind::Cursor, cursor_enabled, &limits.cursor),
        (ProviderKind::Claude, claude_enabled, &limits.claude),
        (ProviderKind::Codex, codex_enabled, &limits.codex),
    ];
    let mut entries: Vec<_> = providers
        .into_iter()
        .filter(|(_, enabled, _)| *enabled)
        .map(|(provider, _, provider_limits)| {
            (provider, combined_usage_spend(&provider_limits.usage, period))
        })
        .collect();
    entries.sort_by(|(_, left), (_, right)| right.cmp(left));
    let total_spend = entries
        .iter()
        .fold(0_u64, |total, (_, spend)| total.saturating_add(*spend));
    let content = match presentation {
        TotalSpendPresentation::Donut => combined_usage_donut_content(
            &entries,
            total_spend,
            period,
            set_period,
            hovered_period,
            set_hovered_period,
            color_scheme,
        ),
        TotalSpendPresentation::ProgressBar => combined_usage_progress_content(
            &entries,
            total_spend,
            period,
            set_period,
            hovered_period,
            set_hovered_period,
            color_scheme,
        ),
    };

    vstack((
        body_strong("Total Spend")
            .foreground(ThemeRef::SecondaryText)
            .margin(Thickness {
                left: 4.0,
                top: 0.0,
                right: 4.0,
                bottom: 0.0,
            }),
        border(
            content,
        )
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
    period: CombinedUsagePeriod,
    set_period: SetState<CombinedUsagePeriod>,
    hovered_period: Option<CombinedUsagePeriod>,
    set_hovered_period: SetState<Option<CombinedUsagePeriod>>,
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

    vstack((
        combined_usage_period_selector(period, set_period, hovered_period, set_hovered_period),
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
        .horizontal_alignment(HorizontalAlignment::Stretch),
    ))
    .spacing(10.0)
    .into()
}

fn combined_usage_progress_content(
    entries: &[(ProviderKind, u64)],
    total_spend: u64,
    _period: CombinedUsagePeriod,
    set_period: SetState<CombinedUsagePeriod>,
    hovered_period: Option<CombinedUsagePeriod>,
    set_hovered_period: SetState<Option<CombinedUsagePeriod>>,
    color_scheme: ColorScheme,
) -> Element {
    let mut sorted_entries = entries.to_vec();
    sorted_entries.sort_by(|(_, left), (_, right)| right.cmp(left));

    vstack((
        combined_usage_period_selector(
            _period,
            set_period,
            hovered_period,
            set_hovered_period,
        ),
        text_block(format_spend(total_spend)).font_size(22.0).font_weight(600),
        combined_usage_progress_bar(&sorted_entries, color_scheme),
        combined_usage_grouped_totals(&sorted_entries, color_scheme),
    ))
    .spacing(10.0)
    .into()
}

fn combined_usage_progress_bar(entries: &[(ProviderKind, u64)], color_scheme: ColorScheme) -> Element {
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
    period: CombinedUsagePeriod,
) -> u64 {
    match period {
        CombinedUsagePeriod::Today => statistics.today.estimated_cost_microusd,
        CombinedUsagePeriod::Yesterday => statistics
            .daily
            .iter()
            .find(|entry| entry.date == Local::now().date_naive() - ChronoDuration::days(1))
            .map(|entry| entry.usage.estimated_cost_microusd)
            .unwrap_or_default(),
        CombinedUsagePeriod::ThirtyDays => statistics.history.estimated_cost_microusd,
    }
}

fn combined_usage_period_selector(
    selected: CombinedUsagePeriod,
    set_selected: SetState<CombinedUsagePeriod>,
    hovered: Option<CombinedUsagePeriod>,
    set_hovered: SetState<Option<CombinedUsagePeriod>>,
) -> Element {
    let buttons: Vec<Element> = [
        CombinedUsagePeriod::Today,
        CombinedUsagePeriod::Yesterday,
        CombinedUsagePeriod::ThirtyDays,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, period)| {
        combined_usage_period_button(
            period,
            selected,
            hovered == Some(period),
            set_selected.clone(),
            set_hovered.clone(),
        )
            .grid_column(index as i32)
    })
    .collect();
    border(
        grid(buttons)
            .columns([
                GridLength::Star(1.0),
                GridLength::Star(1.0),
                GridLength::Star(1.0),
            ])
            .rows([GridLength::Auto])
            .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(Thickness::uniform(4.0))
    .corner_radius(6.0)
    .background(ThemeRef::SubtleFill)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn combined_usage_period_button(
    period: CombinedUsagePeriod,
    selected: CombinedUsagePeriod,
    hovered: bool,
    set_selected: SetState<CombinedUsagePeriod>,
    set_hovered: SetState<Option<CombinedUsagePeriod>>,
) -> Element {
    let is_selected = period == selected;
    let set_hovered_on_enter = set_hovered.clone();
    let set_hovered_on_exit = set_hovered;
    let layers: Vec<Element> = vec![
        border(Element::Empty)
            .background(ThemeRef::Accent)
            .opacity(if is_selected { 1.0 } else { 0.0 })
            .with_opacity_transition(Duration::from_millis(200))
            .corner_radius(4.0)
            .relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .relative_align_bottom()
            .into(),
        border(Element::Empty)
            .background(ThemeRef::CardBackground)
            .opacity(if !is_selected && hovered { 1.0 } else { 0.0 })
            .with_opacity_transition(Duration::from_millis(200))
            .corner_radius(4.0)
            .relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .relative_align_bottom()
            .into(),
        body_strong(period.label())
            .foreground(ThemeRef::SecondaryText)
            .opacity(if is_selected { 0.0 } else { 1.0 })
            .with_opacity_transition(Duration::from_millis(200))
            .relative_align_h_center()
            .relative_align_v_center()
            .into(),
        body_strong(period.label())
            .foreground(Color::rgb(0, 0, 0))
            .opacity(if is_selected { 1.0 } else { 0.0 })
            .with_opacity_transition(Duration::from_millis(200))
            .relative_align_h_center()
            .relative_align_v_center()
            .into(),
    ];
    relative_panel(layers)
    .height(24.0)
    .min_height(24.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .on_pointer_entered(move |_: PointerEventInfo| {
        set_hovered_on_enter.call(Some(period));
    })
    .on_pointer_exited(move || set_hovered_on_exit.call(None))
    .on_tapped(move || set_selected.call(period))
    .with_key(format!("combined-period-{}-{is_selected}", period.key()))
    .into()
}

/// Draws a true circular ring with native WinUI arc paths. The XAML host is
/// keyed by its data so a live refresh replaces its geometry as well as text.
fn combined_usage_donut(
    entries: &[(ProviderKind, u64)],
    total_spend: u64,
    period: CombinedUsagePeriod,
    color_scheme: ColorScheme,
) -> Element {
    const SIZE: f64 = 124.0;
    let xaml = combined_usage_donut_xaml(entries, total_spend, color_scheme);
    let series_key = entries.iter().fold(0_u64, |hash, (provider, spend)| {
        hash.wrapping_mul(31)
            .wrapping_add(*spend)
            .wrapping_add(*provider as u64)
    });
    let key = format!(
        "spend-donut-{}-{total_spend}-{series_key}-{:?}",
        period.key(),
        color_scheme
    );
    let mut host = swap_chain_panel().width(SIZE).height(SIZE);
    host.mounted = Some(Callback::new(move |native: Option<_>| {
        if let Some(native) = native
            && let Err(error) = crate::acrylic::install_spend_donut_into(native, &xaml)
        {
            eprintln!("Could not install spend donut: {error:?}");
        }
    }));
    let donut: Element = host.with_key(key).into();

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

/// Compact, screenshot-style activity chart. For long histories, adjacent days
/// are grouped into a single bar so the chart stays legible in the tray popup.
fn usage_activity_chart(statistics: &crate::usage::UsageStatistics) -> Element {
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
        .map(|index| statistics.tokens_on(first_day + ChronoDuration::days(index as i64)))
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
                .background(ThemeRef::SystemAttention)
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
            combined_usage_spend(&statistics, CombinedUsagePeriod::Today),
            1_250_000
        );
        assert_eq!(
            combined_usage_spend(&statistics, CombinedUsagePeriod::Yesterday),
            2_500_000
        );
        assert_eq!(
            combined_usage_spend(&statistics, CombinedUsagePeriod::ThirtyDays),
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
        let mut limits = plan_limits("plus");
        limits.usage.history.requests = 1;

        assert!(
            popup_sections(&limits, true, true, false).contains(&PopupSection::UsageStatistics)
        );
        assert!(
            !popup_sections(&limits, true, false, false).contains(&PopupSection::UsageStatistics)
        );
    }

    #[test]
    fn banked_reset_count_and_expiration_are_formatted() {
        assert_eq!(
            format_reset_in(Some(
                Utc::now() + ChronoDuration::days(2) + ChronoDuration::minutes(1),
            )),
            "2d"
        );
    }

    #[test]
    fn free_to_plus_replaces_monthly_with_session_and_weekly_sections() {
        let free = popup_sections(&plan_limits("free"), true, true, false);
        assert_eq!(free, vec![PopupSection::Monthly]);
        assert_unique_section_keys(&free);

        let plus = popup_sections(&plan_limits("plus"), true, true, false);
        assert_eq!(plus, vec![PopupSection::FiveHour, PopupSection::Weekly,]);
        assert_unique_section_keys(&plus);
    }

    #[test]
    fn disabled_five_hour_session_is_omitted_from_popup() {
        let mut limits = plan_limits("plus");
        limits.primary = LimitWindow::default();

        let sections = popup_sections(&limits, true, true, false);
        assert_eq!(sections, vec![PopupSection::Weekly]);
        assert_unique_section_keys(&sections);
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
        assert!(!popup_sections(&limits, true, true, false).contains(&PopupSection::Credits));

        limits.credits.balance = Some("$12.50".into());
        assert_eq!(credits_display_value(&limits).as_deref(), Some("$12.50"));
        assert!(popup_sections(&limits, true, true, false).contains(&PopupSection::Credits));

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
            true,
            true,
            false,
            ColorScheme::Dark,
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

        let sections = popup_sections(&limits, true, true, true);
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
    fn banked_resets_section_respects_its_live_toggle() {
        let mut limits = plan_limits("plus");
        limits.reset_credits = Some(crate::limits::RateLimitResetCreditsSummary {
            available_count: 1,
            ..Default::default()
        });

        assert!(popup_sections(&limits, true, true, false).contains(&PopupSection::BankedResets));
        assert!(!popup_sections(&limits, false, true, false).contains(&PopupSection::BankedResets));
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
}
