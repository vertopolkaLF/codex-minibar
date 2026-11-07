//! Provider pages: credential persistence and the
//! shared model behind the page (`page`) and its dialogs (`dialog`).

use crate::drivers::{InstallSource, InstallStatus};
use crate::limits::{OpenRouterAccountSnapshot, OpenRouterApiKeySnapshot, SpendingSummary};
use crate::provider_registry::ProviderKind;
use crate::settings::{
    BadgeColor, InstanceSource, OpenRouterAccount, ProviderId, ProviderInstance, Settings,
};

mod dialog;
mod page;

use crate::app::actions::{
    find_account_instance, persist_openrouter_account, persist_openrouter_credentials,
};

pub(crate) use dialog::{ProviderDialog, ProviderDialogKind};

fn not_openrouter_key() -> &'static str {
    crate::i18n::tr("that-doesn-t-look-like-an-openrouter-key-keys-start-with-sk-or")
}

/// What Settings shows from the live quota snapshot, derived once per
/// change so pages never fetch anything themselves. Holds labels, masked
/// keys, spend and balances, never secrets.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct LiveProviderData {
    /// Titles of API-discovered additional windows, by brick id, so Settings
    /// lists them with the provider's own titles.
    pub(crate) discovered_bricks: std::collections::BTreeMap<String, String>,
    /// Signed-in account name of every instance that reported one, by
    /// instance id. Shown in each instance's Account section.
    pub(crate) identities: std::collections::BTreeMap<String, String>,
    pub(crate) openrouter_accounts: Vec<OpenRouterAccountSnapshot>,
    pub(crate) openrouter_sampled_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl LiveProviderData {
    pub(crate) fn from_limits(limits: &crate::limits::ProviderLimits) -> Self {
        let mut live = Self::default();
        for (provider, limits) in limits.iter() {
            live.discovered_bricks.extend(
                crate::provider_registry::discovered_additional_brick_labels(
                    provider.kind(),
                    limits,
                ),
            );
            if let Some(name) = limits
                .account_name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
            {
                live.identities
                    .insert(provider.id().to_owned(), name.to_owned());
            }
            if limits.openrouter_accounts.is_empty() {
                continue;
            }
            live.openrouter_accounts
                .extend(limits.openrouter_accounts.iter().cloned());
            live.openrouter_sampled_at = live.openrouter_sampled_at.max(Some(limits.sampled_at));
        }
        live
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderReadiness {
    Checking,
    Ready,
    NeedsSetup,
}

pub(crate) fn provider_readiness(status: &InstallStatus) -> ProviderReadiness {
    if status.checking {
        ProviderReadiness::Checking
    } else if status.ready() {
        ProviderReadiness::Ready
    } else {
        ProviderReadiness::NeedsSetup
    }
}

fn money(microusd: u64) -> String {
    format!("${:.2}", microusd as f64 / 1_000_000.0)
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

/// Applies a change to one instance against on-disk settings, never a stale
/// UI snapshot. The open window picks the result up through the live sync.
/// Blocks on the serial writer: call from worker threads only, never the
/// GPUI thread.
fn persist_instance(
    provider: ProviderId,
    mutate: impl FnOnce(&mut ProviderInstance) -> anyhow::Result<()> + Send + 'static,
) -> anyhow::Result<()> {
    crate::settings::store::update_blocking(move |settings| {
        let instance = settings
            .instance_mut(provider)
            .ok_or_else(|| anyhow::anyhow!(crate::i18n::tr("this-provider-no-longer-exists")))?;
        mutate(instance)?;
        instance.normalize();
        Ok(())
    })
}

fn bump_credentials(provider: ProviderId) -> anyhow::Result<()> {
    persist_instance(provider, |instance| {
        instance.credentials_revision = instance.credentials_revision.wrapping_add(1);
        Ok(())
    })
}

fn persist_opencode_manual_key(provider: ProviderId, value: Option<String>) -> anyhow::Result<()> {
    let rollback = crate::opencode::apply_manual_key(provider, value.as_deref())?;
    if let Err(error) = bump_credentials(provider) {
        return match crate::opencode::restore_manual_key(rollback) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(anyhow::anyhow!(
                "could not save credential revision ({error:#}); restoring the protected key also failed ({rollback_error:#})"
            )),
        };
    }
    Ok(())
}

/// Keep the existing credential when validation or the settings commit fails.
fn persist_claude_manual_credential(provider: ProviderId, credential: &str) -> anyhow::Result<()> {
    let previous = crate::claude::load_manual_credential(provider.id())?;
    crate::claude::save_manual_credential(provider.id(), Some(credential))?;
    if let Err(error) = bump_credentials(provider) {
        return match crate::claude::save_manual_credential(provider.id(), previous.as_deref()) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(anyhow::anyhow!(
                "Could not save provider settings ({error:#}); restoring its previous credential also failed ({rollback_error:#})."
            )),
        };
    }
    Ok(())
}

/// Only the provider knows the account-wide purchased credit. Tracked keys
/// can be a subset, so their spend plus the balance is not a valid total.
fn account_credit_total(snapshot: Option<&OpenRouterAccountSnapshot>) -> Option<u64> {
    snapshot?.total_credits_microusd.filter(|total| *total > 0)
}

fn available_key_spending(key: &OpenRouterApiKeySnapshot) -> Option<&SpendingSummary> {
    key.has_live_usage.then_some(&key.spending)
}

/// Drivers offered by "Add provider", with the reason a driver can't be added.
fn add_instance_choices(instances: &[ProviderInstance]) -> Vec<(ProviderKind, Option<String>)> {
    ProviderKind::ALL
        .into_iter()
        .map(|driver| {
            let exists = instances.iter().any(|instance| instance.driver == driver);
            let single = !crate::provider_registry::descriptor(driver).supports_multiple_instances;
            (
                driver,
                (exists && single).then(|| {
                    crate::i18n::format(
                        "supports-one-instance",
                        &[("v0", driver.display_name().to_string())],
                    )
                }),
            )
        })
        .collect()
}

/// `Claude · Work` when the driver has several enabled instances, else the
/// name, as every other surface labels it.
fn provider_label(instance: &ProviderInstance, instances: &[ProviderInstance]) -> String {
    let name = instance.display_name();
    if crate::instances::shares_driver(instance, instances)
        && name != instance.driver.display_name()
    {
        format!("{} \u{00b7} {name}", instance.driver.display_name())
    } else {
        name
    }
}

fn looks_like_openrouter_key(value: &str) -> bool {
    value.starts_with("sk-or-")
}

/// Badge color presets plus a custom color, in the accent picker's style.
fn badge_color_swatches(
    k: &crate::gui::kit::Kit,
    id: impl Into<gpui::SharedString>,
    current: BadgeColor,
    on_pick: crate::gui::kit::Handler<BadgeColor>,
) -> gpui::AnyElement {
    let theme = &k.theme;
    let plate = |color: BadgeColor| theme.plate().colors(color);
    let on_select = on_pick.clone();
    crate::gui::kit::ColorSwatches {
        id: id.into(),
        presets: BadgeColor::PRESETS
            .into_iter()
            .map(|color| {
                let (fill, ink) = plate(color);
                crate::gui::kit::Swatch {
                    label: color.label().into(),
                    fill,
                    ink,
                    icon: (color == BadgeColor::Auto).then_some("circle-half-fill"),
                }
            })
            .collect(),
        selected: BadgeColor::PRESETS
            .iter()
            .position(|color| *color == current),
        custom: matches!(current, BadgeColor::Custom(_)).then(|| plate(current)),
        // Auto opens the picker on a neutral grey rather than the theme plate.
        picker_rgb: current.rgb().unwrap_or((0x80, 0x80, 0x80)),
        on_select: std::rc::Rc::new(move |index, window, cx| {
            on_select(BadgeColor::PRESETS[index], window, cx)
        }),
        on_custom: std::rc::Rc::new(move |rgb, window, cx| {
            on_pick(BadgeColor::custom(rgb), window, cx)
        }),
    }
    .render(k)
}

#[cfg(test)]
mod providers_snapshot_tests {
    use super::*;

    #[test]
    fn account_credit_bar_uses_reported_total_not_tracked_key_spend() {
        let mut account = OpenRouterAccountSnapshot {
            balance_microusd: Some(25_000_000),
            total_credits_microusd: Some(100_000_000),
            ..Default::default()
        };
        assert_eq!(account_credit_total(Some(&account)), Some(100_000_000));
        account.api_keys.push(OpenRouterApiKeySnapshot {
            spending: SpendingSummary {
                used_microusd: 5_000_000,
                ..Default::default()
            },
            has_live_usage: true,
            ..Default::default()
        });
        assert_eq!(account_credit_total(Some(&account)), Some(100_000_000));
        account.total_credits_microusd = None;
        assert_eq!(account_credit_total(Some(&account)), None);
        account.total_credits_microusd = Some(0);
        assert_eq!(account_credit_total(Some(&account)), None);
    }

    #[test]
    fn cached_accounts_without_purchased_credits_still_deserialize() {
        let account: OpenRouterAccountSnapshot = serde_json::from_str(
            r#"{"id":"account","name":"Example","api_keys":[],"balance_microusd":25000000}"#,
        )
        .unwrap();
        assert_eq!(account.balance_microusd, Some(25_000_000));
        assert_eq!(account.total_credits_microusd, None);
    }

    #[test]
    fn unavailable_usage_is_not_presented_as_zero_spend() {
        let mut key = OpenRouterApiKeySnapshot::default();
        key.spending.limit_microusd = Some(50_000_000);
        assert!(available_key_spending(&key).is_none());
        key.has_live_usage = true;
        assert_eq!(available_key_spending(&key).unwrap().used_microusd, 0);
        key.spending.used_microusd = 12_400_000;
        assert_eq!(
            available_key_spending(&key).unwrap().used_microusd,
            12_400_000
        );
        key.has_live_usage = false;
        assert!(available_key_spending(&key).is_none());
        assert_eq!(key.spending.limit_microusd, Some(50_000_000));
    }
}
