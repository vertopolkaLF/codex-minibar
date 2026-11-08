use super::nav::*;
use crate::provider_registry::ProviderKind;
use crate::settings::{ProviderInstance, Settings};

#[test]
fn tab_tags_are_unique() {
    let mut tags = Tab::ALL.map(Tab::tag).to_vec();
    tags.sort_unstable();
    tags.dedup();
    assert_eq!(tags.len(), Tab::ALL.len());
    assert_eq!(Tab::Popup.tag(), "customize");
}

#[test]
fn every_sidebar_icon_is_embedded() {
    for tab in Tab::ALL {
        assert!(
            crate::gui::assets::has_asset(tab.color_icon()),
            "{}",
            tab.color_icon()
        );
    }
}

#[test]
fn page_keys_are_unique_per_tab_and_instance() {
    let work = crate::instances::ProviderId::new(ProviderKind::Claude, "claude-work");
    let pages = Tab::ALL
        .map(Page::Root)
        .into_iter()
        .chain([
            Page::Provider(work),
            Page::Provider(ProviderKind::Claude.into()),
            Page::NoProviders,
        ])
        .collect::<Vec<_>>();
    let keys = pages
        .iter()
        .map(|page| page.key())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(keys.len(), pages.len());
}

fn instances(mask: u32, extra_claude: bool) -> Vec<ProviderInstance> {
    let mut instances = Settings::default().instances;
    if extra_claude {
        let mut work = ProviderInstance::new(ProviderKind::Claude, "Work");
        work.id = "claude-work".into();
        let index = instances
            .iter()
            .position(|instance| instance.driver == ProviderKind::Claude)
            .unwrap();
        instances.insert(index + 1, work);
    }
    for (index, instance) in instances.iter_mut().enumerate() {
        instance.enabled = mask & (1 << index) != 0;
    }
    instances
}

#[test]
fn provider_nav_keeps_identity_and_order_for_every_enabled_combination() {
    for extra_claude in [false, true] {
        let count = instances(0, extra_claude).len();
        for mask in 0..(1_u32 << count) {
            let instances = instances(mask, extra_claude);
            let (enabled, disabled) = provider_nav_order(&instances);
            let expected: Vec<_> = instances
                .iter()
                .filter(|i| i.enabled)
                .chain(instances.iter().filter(|i| !i.enabled))
                .collect();
            let listed: Vec<_> = enabled.iter().chain(disabled.iter()).copied().collect();
            assert_eq!(listed.len(), expected.len());
            for (item, instance) in listed.into_iter().zip(expected) {
                assert_eq!(item.id, instance.id);
                let enabled_claudes = instances
                    .iter()
                    .filter(|i| i.enabled && i.driver == ProviderKind::Claude)
                    .count();
                assert_eq!(
                    crate::instances::shares_driver(item, &instances),
                    item.driver == ProviderKind::Claude && enabled_claudes > 1
                );
            }
        }
    }
}

#[test]
fn first_provider_prefers_enabled_nav_order() {
    let mut instances = Settings::default().instances;
    for instance in &mut instances {
        instance.enabled = false;
    }
    assert_eq!(
        first_provider_in_order(&instances),
        Some(instances[0].provider_id())
    );
    instances[2].enabled = true;
    assert_eq!(
        first_provider_in_order(&instances),
        Some(instances[2].provider_id())
    );
    assert_eq!(first_provider_in_order(&[]), None);
    assert!(matches!(first_provider_page(&[]), Page::NoProviders));
}
