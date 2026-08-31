use super::*;

/// The popup either shows the Home feed or one enabled provider.
///
/// This intentionally stays ephemeral: it is a view choice for the currently
/// open popup, not an application preference that should survive a restart.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum PopupView {
    #[default]
    Home,
    Usage,
    Codex,
    Claude,
    Cursor,
    OpenCodeZen,
    OpenCodeGo,
    OpenRouter,
}

impl PopupView {
    #[cfg(test)]
    pub(super) const fn from_provider(provider: ProviderKind) -> Self {
        match provider {
            ProviderKind::Codex => Self::Codex,
            ProviderKind::Claude => Self::Claude,
            ProviderKind::Cursor => Self::Cursor,
            ProviderKind::OpenCodeZen => Self::OpenCodeZen,
            ProviderKind::OpenCodeGo => Self::OpenCodeGo,
            ProviderKind::OpenRouter => Self::OpenRouter,
        }
    }

    pub(super) const fn provider(self) -> Option<ProviderKind> {
        match self {
            Self::Home | Self::Usage => None,
            Self::Codex => Some(ProviderKind::Codex),
            Self::Claude => Some(ProviderKind::Claude),
            Self::Cursor => Some(ProviderKind::Cursor),
            Self::OpenCodeZen => Some(ProviderKind::OpenCodeZen),
            Self::OpenCodeGo => Some(ProviderKind::OpenCodeGo),
            Self::OpenRouter => Some(ProviderKind::OpenRouter),
        }
    }

    pub(super) fn order(self, provider_order: &[ProviderKind]) -> i32 {
        match self {
            Self::Home => 0,
            Self::Usage => 1,
            other => {
                let provider = other.provider().expect("provider view");
                2 + provider_order
                    .iter()
                    .position(|item| *item == provider)
                    .unwrap_or(0) as i32
            }
        }
    }
}

#[cfg(test)]
pub(super) fn enabled_popup_views(
    popup_order: &[PopupWidgetKind],
    codex: bool,
    claude: bool,
    cursor: bool,
    opencode_zen: bool,
    opencode_go: bool,
    openrouter: bool,
) -> Vec<PopupView> {
    let mut views = vec![PopupView::Home, PopupView::Usage];
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

pub(super) fn provider_order_from_popup(popup_order: &[PopupWidgetKind]) -> Vec<ProviderKind> {
    popup_order
        .iter()
        .filter_map(|widget| widget.as_provider())
        .collect()
}

pub(super) fn provider_order_key(providers: &[ProviderKind]) -> String {
    providers
        .iter()
        .map(|provider| provider.id())
        .collect::<Vec<_>>()
        .join("-")
}

pub(super) fn popup_order_key(popup_order: &[PopupWidgetKind]) -> String {
    popup_order
        .iter()
        .map(|widget| widget.id())
        .collect::<Vec<_>>()
        .join("-")
}

pub(super) fn provider_is_enabled(
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

pub(super) fn total_spend_provider_count(
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

pub(super) fn visible_popup_widgets(
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
pub(super) enum PagerDirection {
    Forward,
    Backward,
}

pub(super) const PAGER_ANIMATION_DURATION: Duration = Duration::from_millis(250);
pub(super) const REFRESH_SPIN_DURATION: Duration = Duration::from_millis(650);
pub(super) const REFRESH_PAUSE_DURATION: Duration = Duration::from_millis(280);

pub(super) fn refresh_spin_ease(progress: f64) -> f64 {
    let progress = progress.clamp(0.0, 1.0);
    if progress < 0.5 {
        4.0 * progress * progress * progress
    } else {
        1.0 - (-2.0 * progress + 2.0).powi(3) / 2.0
    }
}

/// Advance the two-arrow refresh icon by 180 degrees, then hold it still.
/// Keeping the angle continuous makes the next half-turn start without a
/// discontinuity, while the eased progress avoids a robotic constant speed.
pub(super) fn refresh_rotation_at(elapsed: Duration) -> f64 {
    let cycle = REFRESH_SPIN_DURATION + REFRESH_PAUSE_DURATION;
    let cycle_seconds = cycle.as_secs_f64();
    let elapsed_seconds = elapsed.as_secs_f64();
    let cycle_index = (elapsed_seconds / cycle_seconds).floor();
    let phase = elapsed_seconds - cycle_index * cycle_seconds;
    let base_angle = cycle_index * 180.0;

    if phase >= REFRESH_SPIN_DURATION.as_secs_f64() {
        base_angle + 180.0
    } else {
        let progress = phase / REFRESH_SPIN_DURATION.as_secs_f64();
        base_angle + 180.0 * refresh_spin_ease(progress)
    }
}

impl PagerDirection {
    pub(super) fn between(from: PopupView, to: PopupView, provider_order: &[ProviderKind]) -> Self {
        if to.order(provider_order) > from.order(provider_order) {
            Self::Forward
        } else {
            Self::Backward
        }
    }

    pub(super) const fn outgoing_offset(self) -> f32 {
        match self {
            Self::Forward => -(popup::POPUP_WIDTH as f32),
            Self::Backward => popup::POPUP_WIDTH as f32,
        }
    }

    pub(super) const fn incoming_offset(self) -> f32 {
        -self.outgoing_offset()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PagerState {
    pub(super) current: PopupView,
    pub(super) outgoing: Option<PopupView>,
    pub(super) pending: Option<PopupView>,
    pub(super) direction: PagerDirection,
    pub(super) animation_id: u64,
    pub(super) provider_order: Vec<ProviderKind>,
}

impl Default for PagerState {
    fn default() -> Self {
        Self {
            current: PopupView::Home,
            outgoing: None,
            pending: None,
            direction: PagerDirection::Forward,
            animation_id: 0,
            provider_order: ProviderKind::default_order(),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum PagerAction {
    Select(PopupView),
    SetProviderOrder(Vec<ProviderKind>),
    AnimationFinished(u64),
}

pub(super) fn reduce_pager(mut state: PagerState, action: PagerAction) -> PagerState {
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
pub(super) enum PopupSection {
    Error,
    Monthly,
    FiveHour,
    Weekly,
    UsageStatistics,
    BankedResets,
    Credits,
}

impl PopupSection {
    pub(super) const fn key(self) -> &'static str {
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

pub(super) fn popup_sections(limits: &RateLimits, has_error: bool) -> Vec<PopupSection> {
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

pub(super) fn limit_section_kind(section: PopupSection) -> Option<LimitSectionKind> {
    match section {
        PopupSection::FiveHour => Some(LimitSectionKind::FiveHour),
        PopupSection::Weekly => Some(LimitSectionKind::Weekly),
        PopupSection::Monthly => Some(LimitSectionKind::Monthly),
        _ => None,
    }
}

pub(super) fn section_brick_id(provider: ProviderKind, section: PopupSection) -> Option<String> {
    match section {
        PopupSection::BankedResets => Some(resets_brick_id(provider)),
        PopupSection::UsageStatistics => Some(usage_brick_id(provider)),
        PopupSection::Credits => Some(credits_brick_id(provider)),
        PopupSection::Error => None,
        limit_section => limit_section_kind(limit_section)
            .and_then(|kind| limit_section_brick_id(provider, kind)),
    }
}

pub(super) fn popup_visibility_key(visibility: &PopupVisibility) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    visibility.hash(&mut hasher);
    hasher.finish()
}
