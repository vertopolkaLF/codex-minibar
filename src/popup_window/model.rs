//! Framework-free popup card plan.
//!
//! The GPUI renderer draws exactly what this module decides to show, so the
//! visibility rules (bricks, surfaces, instances, OpenRouter accounts)
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

/// Instance-scoped card identity, so two instances of one driver never share
/// animation state.
fn card_key(provider: ProviderId, metric: &str) -> String {
    format!("{}:{metric}", provider.id())
}

#[derive(Clone, Debug)]
pub(crate) struct HeadingCard {
    pub(crate) provider: ProviderId,
    pub(crate) first: bool,
    /// Draws the driver icon (with the instance badge) before the name.
    pub(crate) show_icon: bool,
    pub(crate) plan: Option<String>,
    pub(crate) account_name: Option<String>,
    pub(crate) balance_microusd: Option<u64>,
    pub(crate) error: Option<String>,
    pub(crate) drag_handle: bool,
}

#[derive(Clone, Debug)]
pub(crate) enum Card<'a> {
    Heading(HeadingCard),
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
    /// Claude's promotional cloud-session credits: a dollar balance that
    /// expires instead of resetting.
    CloudCredits {
        key: String,
        window: &'a LimitWindow,
        credits: Option<&'a crate::limits::CloudSessionCredits>,
    },
    BankedResets {
        limits: &'a RateLimits,
        expansion_key: String,
    },
    UsageStatistics {
        provider: ProviderId,
        statistics: &'a crate::usage::UsageStatistics,
    },
    /// An OpenRouter account with a management key whose analytics have not
    /// arrived yet.
    UsageLoading,
    Credits {
        value: String,
    },
    /// Cards that must stay glued together (an instance section or an
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
    provider: ProviderId,
    is_first: bool,
    show_icon: bool,
    limits: &'a RateLimits,
    forced_resets: &'a [crate::reset_feed::ForcedReset],
    options: &CardOptions<'_>,
) -> Vec<Card<'a>> {
    let kind = provider.kind();
    let (monthly_label, primary_label, secondary_label) = match kind {
        ProviderKind::Cursor => (
            crate::i18n::tr("cursor-models"),
            crate::i18n::tr("cursor-models"),
            crate::i18n::tr("cursor-models"),
        ),
        ProviderKind::OpenRouter => (
            crate::i18n::tr("spending"),
            crate::i18n::tr("spending"),
            crate::i18n::tr("spending"),
        ),
        ProviderKind::Antigravity => (
            crate::i18n::tr("gemini"),
            crate::i18n::tr("gemini"),
            crate::i18n::tr("claude-gpt"),
        ),
        ProviderKind::Grok => (
            crate::i18n::tr("credits"),
            crate::i18n::tr("credits"),
            crate::i18n::tr("credits"),
        ),
        ProviderKind::Kiro => (
            crate::i18n::tr("monthly-credits-976559"),
            crate::i18n::tr("credits"),
            crate::i18n::tr("credits"),
        ),
        _ => (
            crate::i18n::tr("monthly"),
            crate::i18n::tr("msg-5h-session-de7ce8"),
            crate::i18n::tr("weekly"),
        ),
    };
    let single_openrouter_account =
        kind == ProviderKind::OpenRouter && limits.openrouter_accounts.len() == 1;
    let heading = HeadingCard {
        provider,
        first: is_first,
        show_icon,
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
            .then(|| {
                Some(provider.display_name())
                    .filter(|name| name != kind.display_name())
                    .or_else(|| limits.account_name.clone())
            })
            .flatten(),
        balance_microusd: single_openrouter_account
            .then(|| limits.openrouter_accounts[0].balance_microusd)
            .flatten(),
        error: options.provider_error.map(str::to_owned),
        drag_handle: options.drag_handle,
    };
    let mut cards = vec![Card::Heading(heading)];
    let visible = |brick: &str| {
        options.popup_visibility.is_visible_for_instance(
            provider.id(),
            brick,
            options.surface,
            options.show_provider_tabs,
        )
    };

    if kind == ProviderKind::OpenRouter {
        let spending_visible = visible(&spending_brick_id(kind));
        let usage_visible = options.include_usage_stats && visible(&usage_brick_id(kind));
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
                            .unwrap_or_else(|| {
                                crate::i18n::format(
                                    "key-39df89",
                                    &[("v0", (index + 1).to_string())],
                                )
                            });
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
                        strip.extend(openrouter_account_usage(provider, limits, &account.id));
                    }
                    cards.push(Card::Group { cards: strip });
                }
            } else if spending_visible && let Some(spending) = limits.spending.as_ref() {
                cards.push(spending_card(card_key(provider, "spending"), spending));
            }
        }
        return cards;
    }

    // Cursor usage is fetched from a remote CSV export rather than scanned
    // from a local session log. Keep its card visible while that export is
    // still empty or delayed, so the feature does not look like it vanished.
    let has_usage_statistics = options.include_usage_stats
        && visible(&usage_brick_id(kind))
        && (limits.usage.has_data() || kind == ProviderKind::Cursor);
    for section in popup_sections(kind, limits, false) {
        if !matches!(
            section,
            PopupSection::Monthly | PopupSection::FiveHour | PopupSection::Weekly
        ) {
            continue;
        }
        if !section_brick_id(kind, section).is_some_and(|brick| visible(&brick)) {
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
            key: card_key(provider, section.key()),
            title: title.to_uppercase(),
            window,
            usage_amount,
            disabled,
        });
    }
    // Claude can return extra windows such as Fable or Opus. They belong with
    // the ordinary limit cards, before banked resets, statistics, or credits.
    for limit in &limits.additional_limits {
        if !visible(&additional_limit_brick_id(kind, &limit.id)) {
            continue;
        }
        if kind == ProviderKind::Claude && limit.id == CLOUD_SESSION_CREDITS_LIMIT_ID {
            cards.push(Card::CloudCredits {
                key: card_key(provider, &format!("additional-{}", limit.id)),
                window: &limit.window,
                credits: limits.cloud_session_credits.as_ref(),
            });
            continue;
        }
        cards.push(Card::Limit {
            key: card_key(provider, &format!("additional-{}", limit.id)),
            title: crate::provider_registry::additional_label(limit).to_uppercase(),
            window: &limit.window,
            usage_amount: None,
            disabled: false,
        });
    }
    // A Claude instance backed by an Admin API key reports organization
    // spend instead of subscription windows.
    if kind == ProviderKind::Claude
        && let Some(spending) = limits.spending.as_ref()
    {
        cards.push(spending_card(card_key(provider, "spending"), spending));
    }
    if kind == ProviderKind::Codex {
        let upcoming = upcoming_forced_resets(forced_resets, options.now);
        if !upcoming.is_empty() {
            cards.push(Card::ForcedResets(upcoming));
        }
    }
    if visible(&resets_brick_id(kind)) && limits.available_reset_count() > 0 {
        // Use instance identity rather than a mutable, potentially shared name.
        cards.push(Card::BankedResets {
            limits,
            expansion_key: format!(
                "{}-{:?}",
                card_key(provider, "banked-resets"),
                options.surface,
            ),
        });
    }
    if has_usage_statistics {
        cards.push(Card::UsageStatistics {
            provider,
            statistics: &limits.usage,
        });
    }
    if visible(&credits_brick_id(kind))
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

fn openrouter_account_usage<'a>(
    provider: ProviderId,
    limits: &'a RateLimits,
    account: &str,
) -> Option<Card<'a>> {
    let statistics = limits.usage.accounts.get(account);
    if let Some(statistics) = statistics.filter(|s| !s.daily.is_empty()) {
        return Some(Card::UsageStatistics {
            provider,
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

/// Claude's additional window that carries cloud-session credit dollars.
pub(crate) const CLOUD_SESSION_CREDITS_LIMIT_ID: &str = "iguana_necktie";

/// `(label, progress, available)` for a cloud-credit balance. Expired or
/// locked credits show no progress; dollars are never estimated from the
/// rounded utilization.
pub(crate) fn cloud_session_credits_presentation(
    window: &LimitWindow,
    credits: Option<&crate::limits::CloudSessionCredits>,
    show_used: bool,
    now: DateTime<Utc>,
) -> (String, f64, bool) {
    if window.resets_at.is_some_and(|at| at <= now) {
        return (crate::i18n::tr("expired").into(), 0.0, false);
    }
    if credits.is_some_and(|credits| credits.locked) {
        return (crate::i18n::tr("unavailable").into(), 0.0, false);
    }
    let (percentage, progress, _, _) = limit_card_presentation(window, show_used, false);
    let label = credits.map_or(percentage, |credits| {
        let amount = if show_used {
            credits.used_dollars
        } else {
            credits.remaining_dollars
        };
        crate::i18n::format(
            if show_used {
                "cloud-amount-used"
            } else {
                "cloud-amount-left"
            },
            &[
                ("amount", format_usd(amount)),
                ("limit", format_usd(credits.limit_dollars)),
            ],
        )
    });
    (label, progress, true)
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
        return (crate::i18n::tr("disabled").into(), 100.0, false, false);
    }
    let remaining = window.remaining_percent();
    let percentage = if show_used_percentage {
        window.used_percent
    } else {
        remaining
    };
    let label = percentage
        .map(|value| {
            crate::i18n::format(
                if show_used_percentage {
                    "quota-percent-used"
                } else {
                    "quota-percent-left"
                },
                &[("value", value.to_string())],
            )
        })
        .unwrap_or_else(|| crate::i18n::tr("unavailable").into());
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
    limits
        .iter()
        .map(|(_, limits)| limits.sampled_at)
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

/// Structural layout key of one instance's cards. A switcher change whose
/// key differs fades the page in instead of letting cards jump.
pub(crate) fn instance_layout_key(
    limits: &RateLimits,
    has_error: bool,
    show_used_percentage: bool,
    show_usage_pace: bool,
) -> String {
    let mut key = String::new();
    key.push(if has_error { '!' } else { ';' });
    // An expiring login adds a warning bar above the cards.
    let expiring = limits
        .login_expires_at
        .is_some_and(|at| at - Utc::now() <= chrono::Duration::days(3));
    key.push(if expiring { '~' } else { '.' });
    push_limits_layout_key(&mut key, limits, show_used_percentage, show_usage_pace);
    key
}

/// Provider tabs for the current settings.
pub(crate) fn provider_tabs(ui: &UiState) -> Vec<PopupView> {
    provider_tab_views(&ui.instances, ui.popup_tab_mode)
}

/// Provider tabs are shown only when there is more than one to choose from.
pub(crate) fn show_provider_tabs(ui: &UiState) -> bool {
    provider_tabs(ui).len() > 1
}

/// The instance a grouped switcher shows: the persisted choice while it is
/// still enabled, otherwise the driver's first enabled instance.
pub(crate) fn selected_group_member(ui: &UiState, driver: ProviderKind) -> Option<ProviderId> {
    let members = ui.enabled_instances_of(driver);
    ui.grouped_tab_selection
        .get(driver.id())
        .and_then(|id| members.iter().copied().find(|member| member.id() == id))
        .or_else(|| members.first().copied())
}

/// The tab that shows `provider`, if any.
pub(crate) fn tab_for_provider(ui: &UiState, provider: ProviderId) -> Option<PopupView> {
    let tabs = provider_tabs(ui);
    if tabs.contains(&PopupView::Provider(provider)) {
        return Some(PopupView::Provider(provider));
    }
    tabs.into_iter().find(|tab| tab.shows(provider))
}

/// Instances counted in the Usage tab and combined total spend.
pub(crate) fn spend_providers(ui: &UiState) -> Vec<ProviderId> {
    ui.enabled_providers()
        .into_iter()
        .filter(|provider| {
            ui.usage_overview_included(*provider)
                && crate::provider_registry::descriptor(provider.kind()).include_in_total_spend
        })
        .collect()
}

pub(crate) fn show_total_spend(ui: &UiState) -> bool {
    ui.usage_stats_enabled && ui.show_total_spend_on_all_tab && spend_providers(ui).len() > 1
}

pub(crate) fn any_provider_enabled(ui: &UiState) -> bool {
    ui.instances.iter().any(|instance| instance.enabled)
}

pub(crate) fn home_widget_label(ui: &UiState, widget: &HomeWidgetId) -> String {
    if widget.is_total_spend() {
        return crate::i18n::tr("home-usage-title").into();
    }
    ui.instances
        .iter()
        .find(|instance| instance.id == widget.id())
        .map(|instance| instance.provider_id().qualified_name())
        .unwrap_or_else(|| widget.id().to_owned())
}

fn widget_driver(ui: &UiState, widget: &HomeWidgetId) -> Option<ProviderKind> {
    ui.instances
        .iter()
        .find(|instance| instance.id == widget.id())
        .map(|instance| instance.driver)
}

/// Every Home block in its saved position, including hidden ones so they
/// keep their slot. New instances join after their driver's last block.
pub(crate) fn home_widget_order(ui: &UiState) -> Vec<HomeWidgetId> {
    let mut available = vec![HomeWidgetId::total_spend()];
    available.extend(
        ui.instances
            .iter()
            .map(|instance| HomeWidgetId::provider(instance.provider_id())),
    );
    if ui.popup_home_order.is_empty() {
        return available;
    }
    let mut order = Vec::new();
    for widget in &ui.popup_home_order {
        if available.contains(widget) && !order.contains(widget) {
            order.push(widget.clone());
        }
    }
    for widget in available {
        if !order.contains(&widget) {
            let driver = widget_driver(ui, &widget);
            let insert = order
                .iter()
                .rposition(|item| driver.is_some() && widget_driver(ui, item) == driver)
                .map_or(order.len(), |index| index + 1);
            order.insert(insert, widget);
        }
    }
    order
}

/// The instance behind a Home block, when it is shown on Home.
pub(crate) fn home_widget_provider(ui: &UiState, widget: &HomeWidgetId) -> Option<ProviderId> {
    let instance = ui
        .instances
        .iter()
        .find(|instance| instance.id == widget.id())?;
    (instance.enabled
        && instance.show_on_home
        && ui
            .popup_visibility
            .instance_visible_on_home(&instance.id, instance.driver))
    .then(|| instance.provider_id())
}

pub(crate) fn visible_home_widgets(ui: &UiState, show_spend: bool) -> Vec<HomeWidgetId> {
    home_widget_order(ui)
        .into_iter()
        .filter(|widget| {
            if widget.is_total_spend() {
                show_spend
            } else {
                home_widget_provider(ui, widget).is_some()
            }
        })
        .collect()
}

pub(crate) fn home_widget_right_column(ui: &UiState, show_spend: bool) -> Vec<HomeWidgetId> {
    if let Some(right) = &ui.popup_home_right_column {
        return right.clone();
    }
    visible_home_widgets(ui, show_spend)
        .into_iter()
        .skip(1)
        .step_by(2)
        .collect()
}

pub(crate) fn home_widget_drop_layout(
    ui: &UiState,
    active: &HomeWidgetId,
    over: &HomeWidgetId,
    column: Option<usize>,
    show_spend: bool,
) -> Option<(Vec<HomeWidgetId>, Vec<HomeWidgetId>)> {
    if column.is_some_and(|column| column > 1) {
        return None;
    }
    let visible = visible_home_widgets(ui, show_spend);
    let from = visible.iter().position(|w| w == active)?;
    let to = visible.iter().position(|w| w == over)?;
    let mut moved = visible.clone();
    let item = moved.remove(from);
    moved.insert(to, item);
    let mut sequence = moved.into_iter();
    let order = home_widget_order(ui)
        .into_iter()
        .map(|w| {
            if visible.contains(&w) {
                sequence.next().expect("visible slot")
            } else {
                w
            }
        })
        .collect::<Vec<_>>();
    let mut right = home_widget_right_column(ui, show_spend);
    let old_right = right.clone();
    if let Some(column) = column
        && right.contains(active) != (column == 1)
    {
        right.retain(|w| w != active);
        if column == 1 {
            right.push(active.clone());
        }
    }
    (order != home_widget_order(ui) || right != old_right).then_some((order, right))
}

/// What an account's bar says about its login.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LoginNotice {
    /// The login is gone; only a new sign-in brings the account back.
    SignInNeeded,
    /// The login stops renewing within Claude Code's three-day warning.
    Expiring { days_left: i64 },
}

/// The login notice for one account, if any. `error` is the account's
/// current popup error; only accounts Minibar can sign in get a notice.
pub(crate) fn login_notice(
    instance: &ProviderInstance,
    error: Option<&str>,
    limits: &RateLimits,
    now: DateTime<Utc>,
) -> Option<LoginNotice> {
    let can_sign_in =
        crate::instances::Capabilities::reason(instance, crate::instances::Capability::SignIn)
            .is_none();
    // The default Claude account can read the desktop app's own session,
    // which the app keeps signed in; no sign-in is offered for it for now.
    let app_account = instance.driver == ProviderKind::Claude && instance.config_folder().is_none();
    if !can_sign_in || app_account {
        return None;
    }
    if let Some(error) = error {
        return error_needs_sign_in(error).then_some(LoginNotice::SignInNeeded);
    }
    let left = limits.login_expires_at? - now;
    let window = chrono::Duration::days(3);
    if left <= chrono::Duration::zero() || left > window {
        return None;
    }
    let day = chrono::Duration::days(1).num_milliseconds();
    let days_left = (left.num_milliseconds() + day - 1) / day;
    Some(LoginNotice::Expiring { days_left })
}

/// Login failures all end by asking for a new sign-in; every other error
/// (network, rate limits, server faults) does not.
pub(crate) fn error_needs_sign_in(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    error.contains("sign in") || error.contains("sign-in")
}
