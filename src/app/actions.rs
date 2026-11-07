//! Changes to OpenRouter credentials, shared by the popup and Settings.
//! Each commits protected secrets and the matching settings as one change.
//! All of them block on the settings writer: call them off the UI thread.

use crate::{
    instances::ProviderInstance, openrouter::AccountSecretChange, settings::OpenRouterAccount,
};

/// The OpenRouter instance that owns `account_id`.
pub(crate) fn find_account_instance<'a>(
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

/// Apply a change to one OpenRouter account. Accounts are addressed by stable
/// id so list shifts cannot move keys between instances. Renaming key labels
/// must not bump the credentials revision; popup headings overlay names.
pub(crate) fn persist_openrouter_account(
    account_id: String,
    bump_credentials: bool,
    previous_availability: Option<bool>,
    mutate: impl FnOnce(&mut OpenRouterAccount) -> anyhow::Result<()> + Send + 'static,
) -> anyhow::Result<()> {
    crate::settings::store::update_blocking(move |settings| {
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
    account_id: String,
    changes: Vec<AccountSecretChange>,
    mutate: impl FnOnce(&mut OpenRouterAccount) -> anyhow::Result<()> + Send + 'static,
) -> anyhow::Result<()> {
    // Availability depends only on the protected store, so no settings read
    // (and no race with queued writes) is needed to know the previous state.
    let before = crate::openrouter::management_key_is_configured(&account_id);
    let rollback = crate::openrouter::apply_account_secret_changes(&changes)?;
    if let Err(error) = persist_openrouter_account(account_id, true, Some(before), mutate) {
        return match rollback.restore() {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(anyhow::anyhow!(
                "could not save account metadata ({error:#}); restoring protected credentials also failed ({rollback_error:#})"
            )),
        };
    }
    Ok(())
}

/// Tracks an API key under the account: its secret goes to the vault, its
/// id and optional name to the account. `key_id` replaces an existing key.
pub(crate) fn track_openrouter_api_key(
    account_id: String,
    key_id: Option<String>,
    secret: String,
    name: String,
) -> anyhow::Result<()> {
    let key_id = key_id.unwrap_or_else(OpenRouterAccount::new_api_key_id);
    let change = AccountSecretChange::api_key(account_id.clone(), key_id.clone(), Some(secret));
    persist_openrouter_credentials(account_id, vec![change], move |account| {
        if !account.api_key_ids.contains(&key_id) {
            account.api_key_ids.push(key_id.clone());
        }
        if !name.is_empty() {
            account.api_key_names.insert(key_id, name);
        }
        Ok(())
    })
}

/// Stops tracking an OpenRouter API key: its secret leaves the vault, its id
/// and name leave the account. The key itself keeps working on OpenRouter.
pub(crate) fn untrack_openrouter_api_key(account_id: String, key_id: String) -> anyhow::Result<()> {
    let change = AccountSecretChange::api_key(account_id.clone(), key_id.clone(), None);
    persist_openrouter_credentials(account_id, vec![change], move |account| {
        let before = account.api_key_ids.len();
        account.api_key_ids.retain(|id| id != &key_id);
        account.api_key_names.remove(&key_id);
        anyhow::ensure!(
            account.api_key_ids.len() != before,
            crate::i18n::tr("openrouter-api-key-no-longer-exists")
        );
        Ok(())
    })
}

/// [`untrack_openrouter_api_key`] for a key that expired or was deleted on
/// OpenRouter; a failure only shows a notification.
pub(crate) fn forget_openrouter_api_key(account_id: String, key_id: String) {
    if let Err(error) = untrack_openrouter_api_key(account_id, key_id) {
        crate::notifications::show_error("OpenRouter key not removed", &format!("{error:#}"));
    }
}
