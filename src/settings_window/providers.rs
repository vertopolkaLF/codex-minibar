//! Provider pages: credential persistence and the
//! shared model behind the page (`page`) and its dialogs (`dialog`).

use std::sync::mpsc::Sender;

use super::persistence::try_persist_update_fallible;
use crate::drivers::{InstallSource, InstallStatus};
use crate::limits::{OpenRouterAccountSnapshot, OpenRouterApiKeySnapshot, SpendingSummary};
use crate::provider_registry::ProviderKind;
use crate::settings::{
    BadgeColor, InstanceSource, OpenRouterAccount, ProviderId, ProviderInstance, Settings,
};

mod dialog;
mod page;

pub(crate) use dialog::{ProviderDialog, ProviderDialogKind};

fn not_openrouter_key() -> &'static str {
    crate::i18n::tr("that-doesn-t-look-like-an-openrouter-key-keys-start-with-sk-or")
}

/// OpenRouter usage the worker last published. Settings only reads labels,
/// masked keys, spend and balances from it; it never holds secrets.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct OpenRouterSettingsSnapshot {
    pub(crate) accounts: Vec<OpenRouterAccountSnapshot>,
    pub(crate) sampled_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Signed-in account name of every instance that reported one, keyed by
    /// instance id. Shown in each instance's Account section.
    pub(crate) identities: std::collections::BTreeMap<String, String>,
}

impl OpenRouterSettingsSnapshot {
    /// Labels, spend and balances of every OpenRouter instance, plus every
    /// instance's signed-in account name.
    pub(crate) fn from_limits(limits: &crate::limits::ProviderLimits) -> Self {
        let mut snapshot = Self::default();
        for (provider, limits) in limits.iter() {
            if let Some(name) = limits
                .account_name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
            {
                snapshot
                    .identities
                    .insert(provider.id().to_owned(), name.to_owned());
            }
            if provider.kind() != ProviderKind::OpenRouter || limits.openrouter_accounts.is_empty()
            {
                continue;
            }
            snapshot
                .accounts
                .extend(limits.openrouter_accounts.iter().cloned());
            snapshot.sampled_at = snapshot.sampled_at.max(Some(limits.sampled_at));
        }
        snapshot
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

/// The OpenRouter instance that owns `account_id`.
fn find_account_instance<'a>(
    instances: &'a [ProviderInstance],
    account_id: &str,
) -> Option<&'a ProviderInstance> {
    instances.iter().find(|instance| {
        instance
            .openrouter
            .as_ref()
            .is_some_and(|account| account.id == account_id)
    })
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

/// Applies a change to one instance against on-disk settings, never a stale
/// UI snapshot. The open window picks the result up through the live sync.
/// Blocks on the serial writer: call from worker threads only, never the
/// GPUI thread.
fn persist_instance(
    settings_tx: Sender<Settings>,
    provider: ProviderId,
    mutate: impl FnOnce(&mut ProviderInstance) -> anyhow::Result<()> + Send + 'static,
) -> anyhow::Result<()> {
    try_persist_update_fallible(settings_tx, move |settings| {
        let instance = settings
            .instance_mut(provider)
            .ok_or_else(|| anyhow::anyhow!(crate::i18n::tr("this-provider-no-longer-exists")))?;
        mutate(instance)?;
        instance.normalize();
        Ok(())
    })
}

/// Apply a change to one OpenRouter account. Accounts are addressed by stable
/// id so list shifts cannot move keys between instances. Renaming key labels
/// must not bump the credentials revision; popup headings overlay names.
fn persist_openrouter_account(
    settings_tx: Sender<Settings>,
    account_id: String,
    bump_credentials: bool,
    previous_availability: Option<bool>,
    mutate: impl FnOnce(&mut OpenRouterAccount) -> anyhow::Result<()> + Send + 'static,
) -> anyhow::Result<()> {
    try_persist_update_fallible(settings_tx, move |settings| {
        let provider = find_account_instance(&settings.instances, &account_id)
            .map(ProviderInstance::provider_id)
            .ok_or_else(|| {
                anyhow::anyhow!(crate::i18n::tr("openrouter-account-no-longer-exists"))
            })?;
        let instance = settings
            .instance_mut(provider)
            .expect("instance just found");
        let account = instance
            .openrouter
            .as_mut()
            .expect("OpenRouter instance has an account");
        mutate(account)?;
        let after = crate::openrouter::has_management_key(std::slice::from_ref(account));
        if bump_credentials {
            instance.credentials_revision = instance.credentials_revision.wrapping_add(1);
        }
        if let Some(before) = previous_availability {
            settings.sync_openrouter_usage_availability(provider, before, after);
        }
        Ok(())
    })
}

/// Commits protected OpenRouter secrets as one file update, then commits the
/// matching account metadata. If the settings write fails, protected storage
/// is restored to its exact previous values so neither side can be orphaned.
pub(crate) fn persist_openrouter_credentials(
    settings_tx: Sender<Settings>,
    account_id: String,
    changes: Vec<crate::openrouter::AccountSecretChange>,
    mutate: impl FnOnce(&mut OpenRouterAccount) -> anyhow::Result<()> + Send + 'static,
) -> anyhow::Result<()> {
    // Availability depends only on the protected store, so no settings read
    // (and no race with queued writes) is needed to know the previous state.
    let before = crate::openrouter::management_key_is_configured(&account_id);
    let rollback = crate::openrouter::apply_account_secret_changes(&changes)?;
    if let Err(error) =
        persist_openrouter_account(settings_tx, account_id, true, Some(before), mutate)
    {
        return match rollback.restore() {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(anyhow::anyhow!(
                "could not save account metadata ({error:#}); restoring protected credentials also failed ({rollback_error:#})"
            )),
        };
    }
    Ok(())
}

fn bump_credentials(settings_tx: Sender<Settings>, provider: ProviderId) -> anyhow::Result<()> {
    persist_instance(settings_tx, provider, |instance| {
        instance.credentials_revision = instance.credentials_revision.wrapping_add(1);
        Ok(())
    })
}

fn persist_opencode_manual_key(
    settings_tx: Sender<Settings>,
    provider: ProviderId,
    value: Option<String>,
) -> anyhow::Result<()> {
    let rollback = crate::opencode::apply_manual_key(provider, value.as_deref())?;
    if let Err(error) = bump_credentials(settings_tx, provider) {
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
fn persist_claude_manual_credential(
    settings_tx: Sender<Settings>,
    provider: ProviderId,
    credential: &str,
) -> anyhow::Result<()> {
    let previous = crate::claude::load_manual_credential(provider.id())?;
    crate::claude::save_manual_credential(provider.id(), Some(credential))?;
    if let Err(error) = bump_credentials(settings_tx, provider) {
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

/// `Claude · Work` when the driver has several instances, else the name.
fn provider_label(instance: &ProviderInstance, instances: &[ProviderInstance]) -> String {
    let shared = instances
        .iter()
        .filter(|other| other.driver == instance.driver)
        .count()
        > 1;
    let name = instance.display_name();
    if shared && name != instance.driver.display_name() {
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
    k: &super::kit::Kit,
    id: impl Into<gpui::SharedString>,
    current: BadgeColor,
    on_pick: super::kit::Handler<BadgeColor>,
) -> gpui::AnyElement {
    let theme = &k.theme;
    let plate = |color: BadgeColor| theme.badge(color);
    let on_select = on_pick.clone();
    super::kit::ColorSwatches {
        id: id.into(),
        presets: BadgeColor::PRESETS
            .into_iter()
            .map(|color| {
                let (fill, ink) = plate(color);
                super::kit::Swatch {
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
