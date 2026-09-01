use super::*;

#[test]
fn tab_tags_keep_legacy_aliases() {
    assert_eq!(Tab::from_tag("popup"), Tab::Popup);
    assert_eq!(Tab::from_tag("customize"), Tab::Popup);
    assert_eq!(Tab::from_tag("schedule"), Tab::Schedule);
    assert_eq!(Tab::from_tag("limit-activation"), Tab::Schedule);
    assert_eq!(Tab::Appearance.tag(), "appearance");
}

#[test]
fn rendered_page_keys_keep_root_and_provider_identity() {
    assert_eq!(
        RenderedPage::Root(Tab::Popup).scroll_key(),
        "settings-scroll-customize"
    );
    assert_eq!(
        RenderedPage::Provider(ProviderKind::OpenRouter).page_key(),
        "settings-page-provider-openrouter"
    );
}

#[test]
fn provider_order_follows_popup_order() {
    let order = vec![
        PopupWidgetKind::OpenRouter,
        PopupWidgetKind::TotalSpend,
        PopupWidgetKind::Claude,
        PopupWidgetKind::OpenCodeGo,
    ];
    assert_eq!(
        navigation::provider_order_from_popup(&order),
        vec![
            ProviderKind::OpenRouter,
            ProviderKind::Claude,
            ProviderKind::OpenCodeGo,
        ]
    );
}
