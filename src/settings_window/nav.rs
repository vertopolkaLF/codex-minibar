//! Settings navigation model: root tabs and the Providers drill-in.

use crate::settings::{ProviderId, ProviderInstance};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Tab {
    #[default]
    General,
    Appearance,
    Providers,
    Popup,
    Schedule,
    Tray,
    Notifications,
    Advanced,
    Log,
    Integrations,
    About,
}

impl Tab {
    /// Sidebar order.
    pub(crate) const ALL: [Tab; 11] = [
        Tab::General,
        Tab::Providers,
        Tab::Popup,
        Tab::Schedule,
        Tab::Tray,
        Tab::Notifications,
        Tab::Appearance,
        Tab::Advanced,
        Tab::Log,
        Tab::Integrations,
        Tab::About,
    ];

    pub(crate) fn tag(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Appearance => "appearance",
            Self::Providers => "providers",
            Self::Popup => "customize",
            Self::Schedule => "schedule",
            Self::Tray => "tray",
            Self::Notifications => "notifications",
            Self::Advanced => "advanced",
            Self::Log => "log",
            Self::Integrations => "integrations",
            Self::About => "about",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::General => crate::i18n::tr("general"),
            Self::Appearance => crate::i18n::tr("appearance"),
            Self::Providers => crate::i18n::tr("providers"),
            Self::Popup => crate::i18n::tr("customize"),
            Self::Schedule => crate::i18n::tr("limit-activation"),
            Self::Tray => crate::i18n::tr("tray"),
            Self::Notifications => crate::i18n::tr("notifications"),
            Self::Advanced => crate::i18n::tr("advanced"),
            Self::Log => crate::i18n::tr("log"),
            Self::Integrations => crate::i18n::tr("integrations"),
            Self::About => crate::i18n::tr("about-updates"),
        }
    }

    /// Fluent Color sidebar icon (`assets/icons/fluent-color-*.svg`).
    pub(crate) fn color_icon(self) -> &'static str {
        match self {
            Self::General => "color/home-24.svg",
            Self::Providers => "color/apps-list-24.svg",
            Self::Popup => "color/apps-24.svg",
            Self::Schedule => "color/calendar-clock-24.svg",
            Self::Tray => "color/chat-24.svg",
            Self::Notifications => "color/alert-badge-24.svg",
            Self::Appearance => "color/paint-brush-24.svg",
            Self::Advanced => "color/settings-24.svg",
            Self::Log => "color/history-24.svg",
            Self::Integrations => "color/puzzle-piece-24.svg",
            Self::About => "color/book-open-24.svg",
        }
    }

    /// Monochrome Phosphor/Fluent glyph used when color icons are off.
    pub(crate) fn mono_icon(self) -> &'static str {
        match self {
            Self::General => "house-fill",
            Self::Providers => "plugs-connected-fill",
            Self::Popup => "squares-four-fill",
            Self::Schedule => "clock-fill",
            Self::Tray => "chat-centered-text-fill",
            Self::Notifications => "bell-fill",
            Self::Appearance => "paint-brush-fill",
            Self::Advanced => "fluent-settings",
            Self::Log => "scroll-fill",
            Self::Integrations => "package-fill",
            Self::About => "info-fill",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum NavMode {
    #[default]
    Root,
    Providers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Page {
    Root(Tab),
    Provider(ProviderId),
    /// The Providers pane with no instance configured yet.
    NoProviders,
}

impl Default for Page {
    fn default() -> Self {
        Self::Root(Tab::default())
    }
}

impl Page {
    /// Identity of a page: changing it remounts the scroller and replays the
    /// entrance transition.
    pub(crate) fn key(self) -> String {
        match self {
            Self::Root(tab) => format!("settings-page-{}", tab.tag()),
            Self::Provider(provider) => format!("settings-page-provider-{}", provider.id()),
            Self::NoProviders => "settings-page-no-providers".into(),
        }
    }
}

/// First enabled instance in the shared order. If none are on, the first
/// listed instance — same order the Providers pane shows.
pub(crate) fn first_provider_in_order(instances: &[ProviderInstance]) -> Option<ProviderId> {
    instances
        .iter()
        .find(|instance| instance.enabled)
        .or_else(|| instances.first())
        .map(ProviderInstance::provider_id)
}

/// The page shown when entering the Providers pane.
pub(crate) fn first_provider_page(instances: &[ProviderInstance]) -> Page {
    first_provider_in_order(instances).map_or(Page::NoProviders, Page::Provider)
}

/// Provider pane order: enabled instances first, then disabled ones. Both
/// blocks keep the shared instance order, so instances of one driver stay
/// adjacent.
pub(crate) fn provider_nav_order(
    instances: &[ProviderInstance],
) -> (Vec<&ProviderInstance>, Vec<&ProviderInstance>) {
    (
        instances
            .iter()
            .filter(|instance| instance.enabled)
            .collect(),
        instances
            .iter()
            .filter(|instance| !instance.enabled)
            .collect(),
    )
}

/// Badges tell instances of one driver apart, so they show whenever the
/// driver has more than one configured instance.
pub(crate) fn shows_badge(instance: &ProviderInstance, instances: &[ProviderInstance]) -> bool {
    instances
        .iter()
        .filter(|other| other.driver == instance.driver)
        .count()
        > 1
}
