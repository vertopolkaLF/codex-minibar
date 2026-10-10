use super::nav::*;
use crate::settings::{ProviderInstance, ProviderKind, Settings};

#[test]
fn tab_tags_are_unique() {
    let mut tags = Tab::ALL.map(Tab::tag).to_vec();
    tags.sort_unstable();
    tags.dedup();
    assert_eq!(tags.len(), Tab::ALL.len());
    assert_eq!(Tab::Popup.tag(), "customize");
}

#[test]
fn provider_pages_are_keyed_by_instance() {
    let work = crate::instances::ProviderId::new(ProviderKind::Claude, "claude-work");
    assert_eq!(
        Page::Provider(work).key(),
        "settings-page-provider-claude-work"
    );
    assert_ne!(
        Page::Provider(work).key(),
        Page::Provider(ProviderKind::Claude.into()).key()
    );
    assert_eq!(Page::NoProviders.key(), "settings-page-no-providers");
    assert_eq!(Page::Root(Tab::Popup).key(), "settings-page-customize");
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
                assert_eq!(
                    shows_badge(item, &instances),
                    extra_claude && item.driver == ProviderKind::Claude
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
