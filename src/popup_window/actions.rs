//! Popup actions that mutate persisted state outside the render thread.

use super::*;

/// Remove an expired OpenRouter key from the vault and the saved account.
/// Runs on a worker thread: it touches the credential store and settings file.
pub(crate) fn remove_openrouter_api_key(
    account_id: String,
    key_id: String,
    settings_tx: Sender<Settings>,
) {
    let change =
        crate::openrouter::AccountSecretChange::api_key(account_id.clone(), key_id.clone(), None);
    let rollback = match crate::openrouter::apply_account_secret_changes(&[change]) {
        Ok(rollback) => rollback,
        Err(error) => {
            notifications::show_error("OpenRouter key not removed", &format!("{error:#}"));
            return;
        }
    };
    if let Err(error) =
        crate::settings_window::try_persist_update_fallible(settings_tx, move |settings| {
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
        })
    {
        let message = match rollback.restore() {
            Ok(()) => format!("{error:#}"),
            Err(rollback_error) => {
                format!("{error:#}; restoring the protected key also failed: {rollback_error:#}")
            }
        };
        notifications::show_error("OpenRouter key not removed", &message);
    }
}
