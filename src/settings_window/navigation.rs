use super::providers::ProviderReadiness;
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Tab {
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
    pub(super) fn tag(self) -> &'static str {
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

    pub(super) fn from_tag(tag: &str) -> Self {
        match tag {
            "appearance" => Self::Appearance,
            "tray" => Self::Tray,
            "providers" => Self::Providers,
            "popup" | "customize" => Self::Popup,
            "schedule" | "limit-activation" => Self::Schedule,
            "notifications" => Self::Notifications,
            "advanced" => Self::Advanced,
            "log" => Self::Log,
            "integrations" => Self::Integrations,
            "about" => Self::About,
            _ => Self::General,
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum SettingsNavMode {
    #[default]
    Root,
    Providers,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RenderedPage {
    Root(Tab),
    Provider(ProviderKind),
}

impl Default for RenderedPage {
    fn default() -> Self {
        Self::Root(Tab::default())
    }
}

impl RenderedPage {
    pub(super) fn scroll_key(self) -> String {
        match self {
            Self::Root(tab) => format!("settings-scroll-{}", tab.tag()),
            Self::Provider(provider) => format!("settings-scroll-provider-{}", provider.id()),
        }
    }

    pub(super) fn page_key(self) -> String {
        match self {
            Self::Root(tab) => format!("settings-page-{}", tab.tag()),
            Self::Provider(provider) => format!("settings-page-provider-{}", provider.id()),
        }
    }
}

pub(super) fn provider_order_from_popup(popup_order: &[PopupWidgetKind]) -> Vec<ProviderKind> {
    popup_order
        .iter()
        .filter_map(|widget| widget.as_provider())
        .collect()
}

pub(super) fn first_provider_in_order(popup_order: &[PopupWidgetKind]) -> ProviderKind {
    provider_order_from_popup(popup_order)
        .into_iter()
        .next()
        .unwrap_or(ProviderKind::Codex)
}

pub(super) fn fade_to_rendered_page(
    set_page_visible: AsyncSetState<bool>,
    set_rendered_page: AsyncSetState<RenderedPage>,
    page: RenderedPage,
) {
    set_page_visible.call(false);
    std::thread::spawn(move || {
        std::thread::sleep(duration(Duration::from_millis(180)));
        set_rendered_page.call(page);
        set_page_visible.call(true);
    });
}

pub(super) fn root_nav_items(nav_icon_color: &str, use_colored: bool) -> [NavViewItem; 11] {
    let item = |label: &str, tag: &str| {
        let mut nav = NavViewItem::new(label).tag(tag);
        if use_colored {
            nav = nav.icon_image_uri(crate::icons::fluent_color_uri(tag));
        } else {
            nav = nav.icon_path(
                crate::icons::data(crate::icons::sidebar_mono_icon(tag)),
                nav_icon_color,
            );
        }
        nav
    };
    [
        item("General", "general"),
        item("Providers", "providers")
            .trailing_icon_path(crate::icons::data("caret-right"), nav_icon_color),
        item("Customize", "customize"),
        item("Limit activation", "schedule"),
        item("Tray", "tray"),
        item("Notifications", "notifications"),
        item("Appearance", "appearance"),
        item("Advanced", "advanced"),
        item("Log", "log"),
        item("Integrations", "integrations"),
        item("About & Updates", "about"),
    ]
}

/// Provider pane: enabled providers first, then a divider and the disabled
/// ones dimmed. Both blocks keep the Customize order.
pub(super) fn providers_nav_items(
    popup_order: &[PopupWidgetKind],
    nav_icon_color: &str,
    color_scheme: ColorScheme,
    is_enabled: impl Fn(ProviderKind) -> bool,
    readiness: impl Fn(ProviderKind) -> ProviderReadiness,
    openrouter_account_count: usize,
) -> Vec<NavViewItem> {
    let (ready_color, setup_color) = status_dot_colors(color_scheme);
    let item = |provider: ProviderKind| {
        let descriptor = crate::provider_registry::descriptor(provider);
        NavViewItem::new(descriptor.display_name)
            .tag(provider.id())
            .icon_path(crate::icons::path_icon_data(descriptor.icon), nav_icon_color)
    };
    let order = provider_order_from_popup(popup_order);
    let mut items = Vec::new();
    for provider in order
        .iter()
        .copied()
        .filter(|provider| is_enabled(*provider))
    {
        let mut nav = item(provider);
        if provider == ProviderKind::OpenRouter && openrouter_account_count > 0 {
            nav = nav.info_badge(openrouter_account_count as i32);
        }
        nav = match readiness(provider) {
            ProviderReadiness::Ready => nav.status_dot(ready_color),
            ProviderReadiness::NeedsSetup => nav.status_dot(setup_color),
            ProviderReadiness::Checking => nav,
        };
        items.push(nav);
    }
    let disabled: Vec<ProviderKind> = order
        .iter()
        .copied()
        .filter(|provider| !is_enabled(*provider))
        .collect();
    if !disabled.is_empty() && !items.is_empty() {
        items.push(NavViewItem::separator());
    }
    items.extend(
        disabled
            .into_iter()
            .map(|provider| item(provider).dimmed(true)),
    );
    items
}

/// Identity for the provider pane. Menu items are rebuilt natively on change,
/// so the NavigationView is remounted whenever membership, dots or badges move.
pub(super) fn providers_nav_signature(items: &[NavViewItem]) -> String {
    items
        .iter()
        .map(|item| {
            format!(
                "{}:{}:{}:{:?}:{:?}",
                item.tag
                    .as_deref()
                    .unwrap_or(if item.is_separator { "-" } else { "" }),
                item.dimmed,
                item.is_header,
                item.info_badge,
                item.status_dot
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn status_dot_colors(color_scheme: ColorScheme) -> (&'static str, &'static str) {
    match color_scheme {
        ColorScheme::Dark => ("#6CCB5F", "#FCE100"),
        ColorScheme::Light => ("#0F7B0F", "#9D5D00"),
    }
}

#[cfg(test)]
mod provider_navigation_tests {
    use super::*;

    #[test]
    fn provider_identity_and_order_survive_every_enabled_combination() {
        let popup_order = Settings::default().popup_order;
        let order = provider_order_from_popup(&popup_order);
        assert_eq!(order.len(), crate::provider_registry::PROVIDERS.len());
        for scheme in [ColorScheme::Light, ColorScheme::Dark] {
            for mask in 0..(1_u32 << order.len()) {
                let enabled = |provider| {
                    let index = order.iter().position(|item| *item == provider).unwrap();
                    mask & (1 << index) != 0
                };
                let items = providers_nav_items(
                    &popup_order,
                    "#123456",
                    scheme,
                    enabled,
                    |_| ProviderReadiness::Ready,
                    2,
                );
                let tagged: Vec<_> = items.iter().filter(|item| item.tag.is_some()).collect();
                let expected: Vec<_> = order
                    .iter()
                    .copied()
                    .filter(|p| enabled(*p))
                    .chain(order.iter().copied().filter(|p| !enabled(*p)))
                    .collect();
                assert_eq!(tagged.len(), order.len());
                for (item, provider) in tagged.into_iter().zip(expected) {
                    let descriptor = crate::provider_registry::descriptor(provider);
                    assert_eq!(item.tag.as_deref(), Some(provider.id()));
                    assert_eq!(item.content, descriptor.display_name);
                    assert_eq!(
                        item.icon_path.as_ref().unwrap().0,
                        crate::icons::path_icon_data(descriptor.icon)
                    );
                    assert_eq!(item.dimmed, !enabled(provider));
                    assert_eq!(item.status_dot.is_some(), enabled(provider));
                    assert_eq!(
                        item.info_badge,
                        (enabled(provider) && provider == ProviderKind::OpenRouter).then_some(2)
                    );
                }
                let mixed = mask != 0 && mask != (1 << order.len()) - 1;
                assert_eq!(
                    items.iter().filter(|item| item.is_separator).count(),
                    usize::from(mixed)
                );
            }
        }
    }
}
