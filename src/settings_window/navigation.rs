use super::providers::mutate_openrouter_accounts;
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

pub(super) fn root_nav_items(nav_icon_color: &str, use_colored: bool) -> [NavViewItem; 10] {
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
        item("About & Updates", "about"),
    ]
}

pub(super) fn providers_nav_items(
    popup_order: &[PopupWidgetKind],
    nav_icon_color: &str,
) -> Vec<NavViewItem> {
    let mut items = vec![NavViewItem::header("Providers")];
    for provider in provider_order_from_popup(popup_order) {
        let descriptor = crate::provider_registry::descriptor(provider);
        items.push(
            NavViewItem::new(descriptor.display_name)
                .tag(provider.id())
                .icon_path(crate::icons::data(descriptor.icon), nav_icon_color),
        );
    }
    items
}

pub(super) fn providers_pane_add_footer(
    set_openrouter_accounts: SetState<Vec<OpenRouterAccount>>,
    settings_tx: Sender<Settings>,
    set_selected_provider: SetState<ProviderKind>,
    set_page_visible: AsyncSetState<bool>,
    set_rendered_page: AsyncSetState<RenderedPage>,
) -> Element {
    let add_account_setter = set_openrouter_accounts;
    let add_account_tx = settings_tx;
    let select_openrouter = set_selected_provider;
    let page_visible = set_page_visible;
    let rendered_page = set_rendered_page;
    border(
        vstack((
            text_block("Only OpenRouter supports multiple accounts.")
                .font_size(11.0)
                .opacity(0.72)
                .wrap(),
            Button::new("Add")
                .icon(Symbol::Add)
                .menu_flyout(vec![
                    menu_item("OpenRouter account"),
                    menu_separator(),
                    menu_item("Codex (single session)"),
                    menu_item("Claude (single session)"),
                    menu_item("Cursor (single session)"),
                    menu_item("OpenCode Zen (single session)"),
                    menu_item("OpenCode Go (single session)"),
                ])
                .on_item_clicked(move |choice: String| match choice.as_str() {
                    "OpenRouter account" => {
                        mutate_openrouter_accounts(
                            add_account_setter.clone(),
                            add_account_tx.clone(),
                            move |accounts| {
                                let next_index = accounts.len() + 1;
                                accounts
                                    .push(OpenRouterAccount::new(format!("Account {next_index}")));
                                true
                            },
                        );
                        select_openrouter.call(ProviderKind::OpenRouter);
                        fade_to_rendered_page(
                            page_visible.clone(),
                            rendered_page.clone(),
                            RenderedPage::Provider(ProviderKind::OpenRouter),
                        );
                    }
                    _ => {
                        crate::notifications::show(
                            "One signed-in session",
                            "Open this provider's page to set it up.",
                        );
                    }
                }),
        ))
        .spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(Thickness {
        left: 12.0,
        top: 0.0,
        right: 12.0,
        bottom: 2.0,
    })
    .background(Color::transparent())
    .into()
}
