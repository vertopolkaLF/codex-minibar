//! Framework-free popup card plan.
//!
//! The GPUI renderer draws exactly what this module decides to show, so the
//! visibility rules (bricks, surfaces, account profiles, OpenRouter accounts)
//! stay unit-testable without a window.

use super::*;

/// Presentation toggles shared by every limit-like card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CardStyle {
    pub(crate) show_used_percentage: bool,
    pub(crate) show_usage_values: bool,
    pub(crate) show_usage_pace: bool,
    pub(crate) compact: bool,
}

/// Inputs for [`provider_cards`]. Interaction state stays in the renderer.
#[derive(Clone, Copy)]
pub(crate) struct CardOptions<'a> {
    pub(crate) popup_visibility: &'a PopupVisibility,
    pub(crate) surface: PopupSurface,
    pub(crate) show_provider_tabs: bool,
    pub(crate) include_usage_stats: bool,
    pub(crate) show_account_name: bool,
    /// Home widgets carry a reorder handle on their first heading.
    pub(crate) drag_handle: bool,
    /// OpenRouter expired keys expose a remove action.
    pub(crate) openrouter_actions: bool,
    pub(crate) provider_error: Option<&'a str>,
    pub(crate) now: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub(crate) struct HeadingCard<'a> {
    pub(crate) provider: ProviderKind,
    pub(crate) first: bool,
    pub(crate) plan: Option<String>,
    pub(crate) account_name: Option<&'a str>,
    pub(crate) balance_microusd: Option<u64>,
    pub(crate) error: Option<String>,
    pub(crate) drag_handle: bool,
}

#[derive(Clone, Debug)]
pub(crate) enum Card<'a> {
    Heading(HeadingCard<'a>),
    Limit {
        key: String,
        title: String,
        window: &'a LimitWindow,
        usage_amount: Option<&'a UsageAmount>,
        disabled: bool,
    },
    Spending {
        key: String,
        title: String,
        masked_key: Option<&'a str>,
        spending: &'a SpendingSummary,
        has_live_usage: bool,
        expired: bool,
        expires_at: Option<DateTime<Utc>>,
        /// `(account id, key id)` for the remove action of an expired key.
        delete: Option<(String, String)>,
    },
    AccountHeading {
        name: &'a str,
        balance_microusd: Option<u64>,
    },
    ForcedResets(Vec<&'a crate::reset_feed::ForcedReset>),
    BankedResets {
        limits: &'a RateLimits,
        expansion_key: String,
    },
    UsageStatistics {
        provider: ProviderKind,
        statistics: &'a crate::usage::UsageStatistics,
    },
    /// An OpenRouter account with a management key whose analytics have not
    /// arrived yet.
    UsageLoading,
    Credits {
        value: String,
    },
    /// Cards that must stay glued together (an account profile or an
    /// OpenRouter account with its keys).
    Group {
        cards: Vec<Card<'a>>,
    },
}

impl Card<'_> {
    #[cfg(test)]
    pub(crate) fn is_usage_statistics(&self) -> bool {
        matches!(self, Self::UsageStatistics { .. })
    }

    #[cfg(test)]
    pub(crate) fn nested(&self) -> &[Card<'_>] {
        match self {
            Self::Group { cards, .. } => cards,
            _ => &[],
        }
    }

    #[cfg(test)]
    pub(crate) fn limit_title(&self) -> Option<&str> {
        match self {
            Self::Limit { title, .. } => Some(title),
            _ => None,
        }
    }
}

pub(crate) fn provider_cards<'a>(
    provider: ProviderKind,
    is_first: bool,
    limits: &'a RateLimits,
    forced_resets: &'a [crate::reset_feed::ForcedReset],
    options: &CardOptions<'_>,
) -> Vec<Card<'a>> {
    let profiles = limits.account_profiles(provider);
    if !profiles.is_empty() {
        // A profile's own failure marks only that profile; a failure of the
        // whole provider marks them all.
        let any_profile_error = profiles.iter().any(|profile| profile.error.is_some());
        let mut drag_handle = options.drag_handle;
        let mut cards = profiles
            .iter()
            .enumerate()
            .map(|(index, profile)| {
                let card_error = options
                    .provider_error
                    .filter(|_| profile.error.is_some() || !any_profile_error)
                    .map(|error| {
                        cached_profile_error_for_ui(
                            &profile.limits,
                            profile.error.as_deref().unwrap_or(error),
                        )
                    });
                let nested = CardOptions {
                    // Usage statistics come from local logs shared by every
                    // profile, so they are shown once below the profiles.
                    include_usage_stats: false,
                    show_account_name: true,
                    drag_handle: std::mem::take(&mut drag_handle),
                    openrouter_actions: false,
                    provider_error: card_error.as_deref(),
                    ..*options
                };
                let profile_cards = provider_cards(
                    provider,
                    is_first && index == 0,
                    &profile.limits,
                    forced_resets,
                    &nested,
                );
                Card::Group {
                    cards: profile_cards,
                }
            })
            .collect::<Vec<_>>();
        cards.extend(shared_usage_statistics_card(
            provider,
            limits,
            options.include_usage_stats,
            options.popup_visibility,
            options.surface,
            options.show_provider_tabs,
        ));
        return cards;
    }

    let (monthly_label, primary_label, secondary_label) = match provider {
        ProviderKind::Cursor => ("Cursor Models", "Cursor Models", "Cursor Models"),
        ProviderKind::OpenRouter => ("Spending", "Spending", "Spending"),
        ProviderKind::Antigravity => ("Gemini", "Gemini", "Claude + GPT"),
        ProviderKind::Grok => ("Credits", "Credits", "Credits"),
        ProviderKind::Kiro => ("Monthly Credits", "Credits", "Credits"),
        _ => ("Monthly", "5h Session", "Weekly"),
    };
    let single_openrouter_account =
        provider == ProviderKind::OpenRouter && limits.openrouter_accounts.len() == 1;
    let heading = HeadingCard {
        provider,
        first: is_first,
        plan: (!single_openrouter_account)
            .then(|| {
                limits
                    .plan_type
                    .as_deref()
                    .filter(|plan| !plan.trim().is_empty())
                    .map(capitalize_plan_name)
            })
            .flatten(),
        account_name: (options.show_account_name && !single_openrouter_account)
            .then_some(limits.account_name.as_deref())
            .flatten(),
        balance_microusd: single_openrouter_account
            .then(|| limits.openrouter_accounts[0].balance_microusd)
            .flatten(),
        error: options.provider_error.map(str::to_owned),
        drag_handle: options.drag_handle,
    };
    let mut cards = vec![Card::Heading(heading)];
    let visible = |brick: &str| {
        options
            .popup_visibility
            .is_visible(brick, options.surface, options.show_provider_tabs)
    };

    if provider == ProviderKind::OpenRouter {
        let spending_visible = visible(&spending_brick_id(provider));
        let usage_visible = options.include_usage_stats && visible(&usage_brick_id(provider));
        if spending_visible || usage_visible {
            if !limits.openrouter_accounts.is_empty() {
                // Each account is its own group so headings and keys can
                // never be shuffled across account boundaries.
                for account in &limits.openrouter_accounts {
                    let mut strip = Vec::new();
                    if !single_openrouter_account {
                        strip.push(Card::AccountHeading {
                            name: &account.name,
                            balance_microusd: account.balance_microusd,
                        });
                    }
                    for (index, api_key) in account
                        .api_keys
                        .iter()
                        .enumerate()
                        .filter(|_| spending_visible)
                    {
                        let title = api_key
                            .local_name
                            .as_deref()
                            .or(api_key.label.as_deref())
                            .map(str::trim)
                            .filter(|label| !label.is_empty())
                            .map(str::to_owned)
                            .unwrap_or_else(|| format!("Key {}", index + 1));
                        let expired = api_key.is_expired(options.now);
                        strip.push(Card::Spending {
                            key: format!("{}-api-{}", account.id, api_key.id),
                            title: title.to_uppercase(),
                            masked_key: api_key.masked_key.as_deref(),
                            spending: &api_key.spending,
                            has_live_usage: api_key.has_live_usage,
                            expired,
                            expires_at: api_key.expires_at,
                            delete: (expired && options.openrouter_actions)
                                .then(|| (account.id.clone(), api_key.id.clone())),
                        });
                    }
                    if usage_visible {
                        strip.extend(openrouter_account_usage(limits, &account.id));
                    }
                    cards.push(Card::Group { cards: strip });
                }
            } else if spending_visible && let Some(spending) = limits.spending.as_ref() {
                cards.push(spending_card(
                    format!("{}-spending", provider.id()),
                    spending,
                ));
            }
        }
        return cards;
    }

    // Cursor usage is fetched from a remote CSV export rather than scanned
    // from a local session log. Keep its card visible while that export is
    // still empty or delayed, so the feature does not look like it vanished.
    let has_usage_statistics = options.include_usage_stats
        && visible(&usage_brick_id(provider))
        && (limits.usage.has_data() || provider == ProviderKind::Cursor);
    for section in popup_sections(provider, limits, false) {
        if !matches!(
            section,
            PopupSection::Monthly | PopupSection::FiveHour | PopupSection::Weekly
        ) {
            continue;
        }
        if !section_brick_id(provider, section).is_some_and(|brick| visible(&brick)) {
            continue;
        }
        let (title, window, usage_amount, disabled) = match section {
            PopupSection::Monthly => (
                monthly_label,
                &limits.secondary,
                limits.secondary_usage_amount.as_ref(),
                false,
            ),
            PopupSection::FiveHour => (
                primary_label,
                &limits.primary,
                limits.primary_usage_amount.as_ref(),
                limits.five_hour_disabled(),
            ),
            _ => (
                secondary_label,
                &limits.secondary,
                limits.secondary_usage_amount.as_ref(),
                false,
            ),
        };
        cards.push(Card::Limit {
            key: format!("{}-{}", provider.id(), section.key()),
            title: title.to_uppercase(),
            window,
            usage_amount,
            disabled,
        });
    }
    // Claude can return extra windows such as Fable or Opus. They belong with
    // the ordinary limit cards, before banked resets, statistics, or credits.
    for limit in &limits.additional_limits {
        if !visible(&additional_limit_brick_id(provider, &limit.id)) {
            continue;
        }
        cards.push(Card::Limit {
            key: format!("{}-additional-{}", provider.id(), limit.id),
            title: limit.title.to_uppercase(),
            window: &limit.window,
            usage_amount: None,
            disabled: false,
        });
    }
    // A Claude profile backed by an Admin API key reports organization spend
    // instead of subscription windows.
    if provider == ProviderKind::Claude
        && let Some(spending) = limits.spending.as_ref()
    {
        cards.push(spending_card("claude-spending".into(), spending));
    }
    if provider == ProviderKind::Codex {
        let upcoming = upcoming_forced_resets(forced_resets, options.now);
        if !upcoming.is_empty() {
            cards.push(Card::ForcedResets(upcoming));
        }
    }
    if visible(&resets_brick_id(provider)) && limits.available_reset_count() > 0 {
        // The account keeps two Claude profiles from expanding together.
        cards.push(Card::BankedResets {
            limits,
            expansion_key: format!(
                "{}-{:?}-{}",
                provider.id(),
                options.surface,
                limits.account_name.as_deref().unwrap_or_default()
            ),
        });
    }
    if has_usage_statistics {
        cards.push(Card::UsageStatistics {
            provider,
            statistics: &limits.usage,
        });
    }
    if visible(&credits_brick_id(provider))
        && let Some(value) = credits_display_value(limits)
    {
        cards.push(Card::Credits { value });
    }
    cards
}

fn spending_card(key: String, spending: &SpendingSummary) -> Card<'_> {
    Card::Spending {
        key,
        title: "SPENDING".into(),
        masked_key: None,
        spending,
        has_live_usage: true,
        expired: false,
        expires_at: None,
        delete: None,
    }
}

/// The usage card for a provider whose limit cards come from account profiles.
pub(crate) fn shared_usage_statistics_card<'a>(
    provider: ProviderKind,
    limits: &'a RateLimits,
    include_usage_stats: bool,
    popup_visibility: &PopupVisibility,
    surface: PopupSurface,
    show_provider_tabs: bool,
) -> Option<Card<'a>> {
    (include_usage_stats
        && limits.usage.has_data()
        && popup_visibility.is_visible(&usage_brick_id(provider), surface, show_provider_tabs))
    .then_some(Card::UsageStatistics {
        provider,
        statistics: &limits.usage,
    })
}

fn openrouter_account_usage<'a>(limits: &'a RateLimits, account: &str) -> Option<Card<'a>> {
    let statistics = limits.usage.accounts.get(account);
    if let Some(statistics) = statistics.filter(|s| !s.daily.is_empty()) {
        return Some(Card::UsageStatistics {
            provider: ProviderKind::OpenRouter,
            statistics,
        });
    }
    // Account-level analytics errors are promoted to the provider error bar
    // by the background bridge; do not render a second error surface here.
    if statistics.and_then(|s| s.error.as_deref()).is_some() {
        return None;
    }
    crate::openrouter::management_key_is_configured(account).then_some(Card::UsageLoading)
}

pub(crate) fn upcoming_forced_resets(
    resets: &[crate::reset_feed::ForcedReset],
    now: DateTime<Utc>,
) -> Vec<&crate::reset_feed::ForcedReset> {
    resets
        .iter()
        .filter(|reset| reset.reset_at > now)
        .take(2)
        .collect()
}

/// Filter a copy for Home only. The canonical quota snapshots remain intact.
pub(crate) fn claude_limits_for_home(
    limits: &RateLimits,
    saved: &[crate::settings::ClaudeProfile],
    excluded: &[String],
) -> Option<RateLimits> {
    let enabled = claude_account_tabs(saved);
    let is_visible = |id: &str| {
        enabled.iter().any(|p| p.id == id) && !excluded.iter().any(|hidden| hidden == id)
    };
    if limits.claude_profiles.is_empty() {
        // Legacy single-Default snapshots, and the placeholder before the
        // first multi-profile read, describe only the first enabled account.
        return enabled
            .first()
            .filter(|p| is_visible(&p.id))
            .map(|_| limits.clone());
    }
    let mut filtered = limits.clone();
    filtered.claude_profiles.retain(|p| is_visible(&p.id));
    (!filtered.claude_profiles.is_empty()).then_some(filtered)
}

pub(crate) fn codex_limits_for_home(
    limits: &RateLimits,
    saved: &[crate::settings::CodexProfile],
    excluded: &[String],
) -> Option<RateLimits> {
    let enabled = codex_account_tabs(saved);
    let is_visible = |id: &str| {
        enabled.iter().any(|p| p.id == id) && !excluded.iter().any(|hidden| hidden == id)
    };
    if limits.codex_profiles.is_empty() {
        return enabled
            .first()
            .filter(|p| is_visible(&p.id))
            .map(|_| limits.clone());
    }
    let mut filtered = limits.clone();
    filtered.codex_profiles.retain(|p| is_visible(&p.id));
    (!filtered.codex_profiles.is_empty()).then_some(filtered)
}

/// Resolve the label/progress pair while keeping compactness tied to the
/// actual quota state rather than the selected label mode.
///
/// An exhausted limit is therefore compact in both modes: `100% used` and
/// `0% left` describe the same underlying state.
pub(crate) fn limit_card_presentation(
    window: &LimitWindow,
    show_used_percentage: bool,
    disabled: bool,
) -> (String, f64, bool, bool) {
    if disabled {
        return ("Disabled".into(), 100.0, false, false);
    }
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
    (
        label,
        f64::from(percentage.unwrap_or(0)),
        true,
        remaining == Some(0),
    )
}

pub(crate) fn usage_amount_label(
    usage_amount: Option<&UsageAmount>,
    enabled: bool,
) -> Option<String> {
    let usage_amount = usage_amount?;
    if !enabled
        || !usage_amount.used.is_finite()
        || usage_amount.used < 0.0
        || !usage_amount.limit.is_finite()
        || usage_amount.limit <= 0.0
    {
        return None;
    }
    Some(format!(
        "({}/{})",
        format_usage_amount(usage_amount.used),
        format_usage_amount(usage_amount.limit),
    ))
}

fn format_usage_amount(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    format!("{value:.2}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

/// Interior ticks that divide a quota window into equal buckets.
///
/// 5-hour bars get hour marks 1–4, weekly bars get day marks 1–6, and
/// monthly bars get three quarter marks (skipping 0% and 100%).
pub(crate) fn interval_tick_count(window: &LimitWindow) -> u32 {
    match window.duration_minutes {
        Some(minutes) if minutes <= 12 * 60 => 4,
        Some(minutes) if minutes <= 8 * 24 * 60 => 6,
        Some(_) => 3,
        None => 0,
    }
}

pub(crate) fn latest_sampled_at(limits: &ProviderLimits) -> DateTime<Utc> {
    crate::provider_registry::PROVIDERS
        .iter()
        .map(|descriptor| limits.get(descriptor.kind).sampled_at)
        .max()
        .unwrap_or_default()
}

fn push_limits_layout_key(
    key: &mut String,
    snapshot: &RateLimits,
    show_used_percentage: bool,
    show_usage_pace: bool,
) {
    key.push(if snapshot.five_hour_disabled() {
        '0'
    } else {
        '1'
    });
    key.push(if snapshot.spending.is_some() {
        's'
    } else {
        '-'
    });
    key.push(if snapshot.usage.has_data() { 'u' } else { '-' });
    key.push(if snapshot.is_free_plan() { 'f' } else { 'p' });
    let windows: Vec<&LimitWindow> = if snapshot.is_free_plan() {
        vec![&snapshot.secondary]
    } else {
        vec![&snapshot.primary, &snapshot.secondary]
    };
    for window in windows.into_iter().filter(|window| !window.is_empty()) {
        key.push(pace_label_layout_key(
            window,
            show_used_percentage,
            show_usage_pace,
        ));
    }
    for limit in &snapshot.additional_limits {
        key.push('|');
        key.push_str(&limit.id);
        key.push(pace_label_layout_key(
            &limit.window,
            show_used_percentage,
            show_usage_pace,
        ));
    }
}

/// Only the structural part of the pace state: whether a pace label shows.
fn pace_label_layout_key(
    window: &LimitWindow,
    show_used_percentage: bool,
    show_usage_pace: bool,
) -> char {
    let (_, _, _, compact) = limit_card_presentation(window, show_used_percentage, false);
    if show_usage_pace && !compact && window.pace_tip(show_used_percentage, Utc::now()).is_some() {
        'p'
    } else {
        '-'
    }
}

/// Layout key of the account selected on a Claude/Codex provider tab.
pub(crate) fn profile_layout_key(
    provider: ProviderKind,
    limits: &RateLimits,
    selected: Option<&str>,
    show_used_percentage: bool,
    show_usage_pace: bool,
) -> String {
    let profile = match provider {
        ProviderKind::Claude => limits.claude_profile(selected),
        ProviderKind::Codex => limits.codex_profile(selected),
        _ => None,
    };
    let Some(profile) = profile else {
        return String::new();
    };
    let mut key = String::new();
    key.push(if profile.error.is_some() { '!' } else { ';' });
    push_limits_layout_key(
        &mut key,
        &profile.limits,
        show_used_percentage,
        show_usage_pace,
    );
    key
}

/// Auto-distribute only the currently visible blocks until the user first moves one.
/// Once assigned, hidden providers keep their saved column when they return.
pub(crate) fn home_right_column(ui: &UiState, show_total_spend: bool) -> Vec<PopupWidgetKind> {
    ui.popup_right_column.clone().unwrap_or_else(|| {
        visible_popup_widgets(
            &ui.popup_order,
            show_total_spend,
            &ui.popup_visibility,
            ui.codex_enabled,
            ui.claude_enabled,
            ui.cursor_enabled,
            ui.opencode_zen_enabled,
            ui.opencode_go_enabled,
            ui.openrouter_enabled,
            ui.antigravity_enabled,
            ui.grok_enabled,
            ui.kiro_enabled,
        )
        .into_iter()
        .skip(1)
        .step_by(2)
        .collect()
    })
}

pub(crate) fn show_total_spend(ui: &UiState) -> bool {
    ui.usage_stats_enabled
        && ui.show_total_spend_on_all_tab
        && total_spend_provider_count(
            ui.codex_enabled,
            ui.claude_enabled,
            ui.cursor_enabled,
            ui.opencode_zen_enabled,
            ui.opencode_go_enabled,
            ui.openrouter_enabled,
            &ui.usage_stats_excluded_providers,
        ) > 1
}

pub(crate) fn provider_enabled(ui: &UiState, provider: ProviderKind) -> bool {
    provider_is_enabled(
        provider,
        ui.codex_enabled,
        ui.claude_enabled,
        ui.cursor_enabled,
        ui.opencode_zen_enabled,
        ui.opencode_go_enabled,
        ui.openrouter_enabled,
        ui.antigravity_enabled,
        ui.grok_enabled,
        ui.kiro_enabled,
    )
}

pub(crate) fn any_provider_enabled(ui: &UiState) -> bool {
    ProviderKind::ALL
        .into_iter()
        .any(|provider| provider_enabled(ui, provider))
}

/// Apply a finished Home widget drag to a scratch copy of the layout.
/// Returns the new `(order, right column)` when anything moved.
pub(crate) fn widget_drop_layout(
    ui: &UiState,
    active: PopupWidgetKind,
    over: PopupWidgetKind,
    column: Option<usize>,
) -> Option<(Vec<PopupWidgetKind>, Vec<PopupWidgetKind>)> {
    let show_total_spend = show_total_spend(ui);
    let mut scratch = Settings {
        popup_order: ui.popup_order.clone(),
        popup_right_column: Some(home_right_column(ui, show_total_spend)),
        providers: crate::settings::ProviderSettings::from_enabled(
            crate::provider_registry::PROVIDERS
                .iter()
                .filter(|descriptor| provider_enabled(ui, descriptor.kind))
                .map(|descriptor| descriptor.kind),
        ),
        show_total_spend_on_all_tab: ui.show_total_spend_on_all_tab,
        ..Settings::default()
    };
    let reordered = scratch.move_popup_widget(active, over, show_total_spend);
    let moved_column =
        column.is_some_and(|column| scratch.assign_popup_widget_column(active, column));
    (reordered || moved_column).then(|| {
        (
            scratch.popup_order,
            scratch.popup_right_column.unwrap_or_default(),
        )
    })
}
