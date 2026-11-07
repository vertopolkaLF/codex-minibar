//! Popup actions that mutate persisted state outside the render thread.

use super::*;

/// Remove an expired OpenRouter key from the vault and the saved account.
/// Runs on a worker thread: it touches the credential store and settings file.
pub(crate) fn remove_openrouter_api_key(account_id: String, key_id: String) {
    if let Err(error) = try_remove_openrouter_api_key(account_id, key_id) {
        notifications::show_error("OpenRouter key not removed", &format!("{error:#}"));
    }
}

/// Stops tracking an OpenRouter API key: its secret leaves the vault and its
/// id leaves the saved account. The key itself keeps working on OpenRouter.
/// Blocking; call it off the UI thread.
pub(crate) fn try_remove_openrouter_api_key(
    account_id: String,
    key_id: String,
) -> anyhow::Result<()> {
    let change =
        crate::openrouter::AccountSecretChange::api_key(account_id.clone(), key_id.clone(), None);
    let rollback = crate::openrouter::apply_account_secret_changes(&[change])?;
    if let Err(error) = crate::settings::store::update_blocking(move |settings| {
        let instance = settings
            .instances
            .iter_mut()
            .find(|instance| {
                instance
                    .openrouter
                    .as_ref()
                    .is_some_and(|account| account.id == account_id)
            })
            .ok_or_else(|| anyhow::anyhow!("OpenRouter account no longer exists"))?;
        let account = instance
            .openrouter
            .as_mut()
            .expect("matched an OpenRouter account");
        let before = account.api_key_ids.len();
        account.api_key_ids.retain(|id| id != &key_id);
        anyhow::ensure!(
            account.api_key_ids.len() != before,
            "OpenRouter API key no longer exists"
        );
        instance.credentials_revision = instance.credentials_revision.wrapping_add(1);
        Ok(())
    }) {
        return Err(match rollback.restore() {
            Ok(()) => error,
            Err(rollback_error) => error.context(format!(
                "restoring the protected key also failed: {rollback_error:#}"
            )),
        });
    }
    Ok(())
}
