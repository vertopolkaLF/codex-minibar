use super::*;

/// The popup either shows the Home feed, the Usage page, one provider
/// instance, or one driver whose instances share a grouped tab.
///
/// This intentionally stays ephemeral: it is a view choice for the currently
/// open popup, not an application preference that should survive a restart.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum PopupView {
    #[default]
    Home,
    Usage,
    /// One provider instance.
    Provider(ProviderId),
    /// Every enabled instance of one driver behind a single grouped tab.
    Group(ProviderKind),
}

impl PopupView {
    pub(super) const fn uses_two_columns(self, enabled: bool) -> bool {
        enabled && matches!(self, Self::Home | Self::Usage)
    }

    pub(super) const fn from_provider(provider: ProviderId) -> Self {
        Self::Provider(provider)
    }

    pub(super) const fn provider(self) -> Option<ProviderId> {
        match self {
            Self::Provider(provider) => Some(provider),
            _ => None,
        }
    }

    /// Whether this view shows `provider`, directly or through its group.
    pub(super) fn shows(self, provider: ProviderId) -> bool {
        match self {
            Self::Provider(shown) => shown == provider,
            Self::Group(kind) => kind == provider.kind(),
            _ => false,
        }
    }

    /// Stable string identity for animation and element keys.
    pub(super) fn key(self) -> String {
        match self {
            Self::Home => "home".into(),
            Self::Usage => "usage".into(),
            Self::Provider(provider) => format!("provider:{}", provider.id()),
            Self::Group(kind) => format!("group:{}", kind.id()),
        }
    }

    pub(super) fn order(self, tab_order: &[PopupView]) -> i32 {
        match self {
            Self::Home => 0,
            Self::Usage => 1,
            other => {
                let position = tab_order
                    .iter()
                    .position(|item| *item == other)
                    .or_else(|| {
                        // An instance inside a grouped tab sits at its group.
                        other.provider().and_then(|provider| {
                            tab_order.iter().position(|item| item.shows(provider))
                        })
                    })
                    .unwrap_or(0);
                2 + position as i32
            }
        }
    }
}

/// Provider tabs in display order for the enabled instances.
///
/// Separate mode yields one tab per instance. Grouped modes yield one tab per
/// driver at the slot of its first enabled instance; a driver with a single
/// enabled instance keeps its plain instance tab.
pub(super) fn provider_tab_views(
    instances: &[ProviderInstance],
    mode: PopupTabMode,
) -> Vec<PopupView> {
    let enabled = instances
        .iter()
        .filter(|instance| instance.enabled)
        .collect::<Vec<_>>();
    let mut views = Vec::with_capacity(enabled.len());
    for instance in &enabled {
        let shared = enabled
            .iter()
            .filter(|other| other.driver == instance.driver)
            .count()
            > 1;
        let view = if mode.is_grouped() && shared {
            PopupView::Group(instance.driver)
        } else {
            PopupView::Provider(instance.provider_id())
        };
        if !views.contains(&view) {
            views.push(view);
        }
    }
    views
}

/// Instances shown by one provider tab, in display order.
pub(super) fn tab_members(instances: &[ProviderInstance], view: PopupView) -> Vec<ProviderId> {
    instances
        .iter()
        .filter(|instance| instance.enabled && view.shows(instance.provider_id()))
        .map(ProviderInstance::provider_id)
        .collect()
}

/// New instance order after dragging tab `from` onto tab `to`. Every tab
/// moves as one block, so a grouped driver keeps its instances together;
/// disabled instances keep their slots.
pub(super) fn reordered_instance_ids(
    instances: &[ProviderInstance],
    tabs: &[PopupView],
    from: PopupView,
    to: PopupView,
) -> Option<Vec<String>> {
    let source = tabs.iter().position(|tab| *tab == from)?;
    let target = tabs.iter().position(|tab| *tab == to)?;
    if source == target {
        return None;
    }
    let blocks = tabs
        .iter()
        .map(|tab| tab_members(instances, *tab))
        .collect::<Vec<_>>();
    let mut moved = blocks.clone();
    let block = moved.remove(source);
    moved.insert(target, block);
    let mut order = Vec::with_capacity(instances.len());
    for instance in instances {
        let provider = instance.provider_id();
        match blocks.iter().position(|block| block.contains(&provider)) {
            Some(index) if blocks[index].first() == Some(&provider) => {
                order.extend(moved[index].iter().map(|member| member.id().to_owned()));
            }
            Some(_) => {}
            None => order.push(instance.id.clone()),
        }
    }
    Some(order)
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
    pub(super) fn between(from: PopupView, to: PopupView, provider_order: &[PopupView]) -> Self {
        if to.order(provider_order) > from.order(provider_order) {
            Self::Forward
        } else {
            Self::Backward
        }
    }

    pub(super) const fn outgoing_offset(self, page_width: i32) -> f32 {
        match self {
            Self::Forward => -(page_width as f32),
            Self::Backward => page_width as f32,
        }
    }

    pub(super) const fn incoming_offset(self, page_width: i32) -> f32 {
        -self.outgoing_offset(page_width)
    }
}

/// Both pages must travel past the widest surface in a mixed-width transition.
/// Using only the incoming width leaves half of an outgoing two-column page visible.
pub(super) const fn pager_slide_width(from: PopupView, to: PopupView, two_columns: bool) -> i32 {
    if from.uses_two_columns(two_columns) || to.uses_two_columns(two_columns) {
        popup::POPUP_WIDE_WIDTH
    } else {
        popup::POPUP_WIDTH
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PagerState {
    pub(super) current: PopupView,
    pub(super) outgoing: Option<PopupView>,
    pub(super) pending: Option<PopupView>,
    pub(super) direction: PagerDirection,
    pub(super) animation_id: u64,
    pub(super) provider_order: Vec<PopupView>,
}

impl Default for PagerState {
    fn default() -> Self {
        Self {
            current: PopupView::Home,
            outgoing: None,
            pending: None,
            direction: PagerDirection::Forward,
            animation_id: 0,
            provider_order: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum PagerAction {
    Select(PopupView),
    SetProviderOrder(Vec<PopupView>),
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

pub(super) fn popup_sections(
    provider: ProviderKind,
    limits: &RateLimits,
    has_error: bool,
) -> Vec<PopupSection> {
    let mut sections = Vec::with_capacity(6);
    if has_error {
        sections.push(PopupSection::Error);
    }
    if provider == ProviderKind::Kiro || limits.is_free_plan() {
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
