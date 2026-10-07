//! Provider pages: install detection, credential persistence and the
//! shared model behind the page (`page`) and its dialogs (`dialog`).

use std::{path::PathBuf, sync::mpsc::Sender};

use super::persistence::try_persist_update_fallible;
use crate::limits::{OpenRouterAccountSnapshot, OpenRouterApiKeySnapshot, SpendingSummary};
use crate::settings::{
    InstanceSource, OpenRouterAccount, ProviderId, ProviderInstance, ProviderKind, Settings,
};

mod dialog;
mod page;

pub(crate) use dialog::{ProviderDialog, ProviderDialogKind};

#[derive(Clone, PartialEq)]
pub(crate) struct ProviderInstallStatus {
    app: Option<String>,
    crew: Option<String>,
    cli: Option<String>,
    used: Option<ProviderInstallSource>,
    app_applicable: bool,
    crew_applicable: bool,
    cli_applicable: bool,
    checking: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum ProviderInstallSource {
    App,
    Crew,
    Cli,
}

impl ProviderInstallStatus {
    /// The "Checking…" placeholder with the sources a driver can report.
    pub(crate) fn checking_for(provider: ProviderKind) -> Self {
        match provider {
            ProviderKind::Kiro => Self::checking_kiro(),
            ProviderKind::Grok => Self::checking_cli(),
            ProviderKind::Codex | ProviderKind::Claude | ProviderKind::Antigravity => {
                Self::checking()
            }
            ProviderKind::Cursor
            | ProviderKind::OpenCodeZen
            | ProviderKind::OpenCodeGo
            | ProviderKind::OpenRouter => Self::checking_app(),
        }
    }

    pub(crate) fn checking() -> Self {
        Self {
            app: None,
            crew: None,
            cli: None,
            used: None,
            app_applicable: true,
            crew_applicable: false,
            cli_applicable: true,
            checking: true,
        }
    }

    pub(crate) fn checking_app() -> Self {
        Self {
            app: None,
            crew: None,
            cli: None,
            used: None,
            app_applicable: true,
            crew_applicable: false,
            cli_applicable: false,
            checking: true,
        }
    }

    pub(crate) fn checking_cli() -> Self {
        Self {
            app: None,
            crew: None,
            cli: None,
            used: None,
            app_applicable: false,
            crew_applicable: false,
            cli_applicable: true,
            checking: true,
        }
    }

    pub(crate) fn checking_kiro() -> Self {
        Self {
            app: None,
            crew: None,
            cli: None,
            used: None,
            app_applicable: true,
            crew_applicable: true,
            cli_applicable: true,
            checking: true,
        }
    }
}

/// Detects what one instance reads from: its binary/app (explicit path or
/// discovery) and, for key-based drivers, its own saved credentials.
pub(crate) fn instance_install_status(instance: &ProviderInstance) -> ProviderInstallStatus {
    let path = |path: &Option<PathBuf>| {
        path.as_ref()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let provider = instance.provider_id();
    match instance.driver {
        ProviderKind::Kiro => provider_install_status_kiro(
            &path(&instance.binary_path),
            &path(&instance.kiro_crew_path),
            &path(&instance.kiro_cli_path),
        ),
        ProviderKind::OpenRouter => {
            let detected = instance.openrouter.as_ref().is_some_and(|account| {
                crate::openrouter::is_installed_for_accounts(std::slice::from_ref(account))
            });
            ProviderInstallStatus {
                app: detected.then(|| "OpenRouter account credentials are configured".into()),
                used: detected.then_some(ProviderInstallSource::App),
                ..provider_install_status(ProviderKind::OpenRouter, "")
            }
        }
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo if !provider.is_primary() => {
            // Secondary OpenCode instances read only their own API key.
            let detected = crate::opencode::key_is_configured(provider);
            ProviderInstallStatus {
                app: detected.then(|| "Saved API key".into()),
                used: detected.then_some(ProviderInstallSource::App),
                ..provider_install_status(instance.driver, "")
            }
        }
        ProviderKind::Claude if instance.uses_manual_credential() => {
            let detected = crate::claude::load_manual_credential(&instance.id)
                .ok()
                .flatten()
                .is_some_and(|value| !value.trim().is_empty());
            ProviderInstallStatus {
                app: detected.then(|| "Saved credential".into()),
                cli: None,
                used: detected.then_some(ProviderInstallSource::App),
                cli_applicable: false,
                ..provider_install_status(ProviderKind::Claude, "")
            }
        }
        ProviderKind::Claude if instance.config_folder().is_some() => {
            // Another account's folder never runs the app's bundled copy.
            let status = provider_install_status(ProviderKind::Claude, &path(&instance.binary_path));
            ProviderInstallStatus {
                used: status.cli.as_ref().map(|_| ProviderInstallSource::Cli),
                ..status
            }
        }
        driver => provider_install_status(driver, &path(&instance.binary_path)),
    }
}

pub(crate) fn provider_install_status(
    provider: ProviderKind,
    configured_folder: &str,
) -> ProviderInstallStatus {
    if provider == ProviderKind::Kiro {
        return provider_install_status_kiro(configured_folder, "", "");
    }
    let configured_folder = (!configured_folder.trim().is_empty())
        .then(|| std::path::Path::new(configured_folder.trim()));
    let (app, cli, used) = match provider {
        ProviderKind::Codex => {
            let candidates = crate::discovery::discover(configured_folder);
            let app = candidates
                .iter()
                .find(|candidate| candidate.source == crate::discovery::CandidateSource::DesktopApp)
                .map(|candidate| candidate.path.as_path());
            let cli = candidates
                .iter()
                .find(|candidate| candidate.source != crate::discovery::CandidateSource::DesktopApp)
                .map(|candidate| candidate.path.as_path());
            let used = candidates.first().map(|candidate| match candidate.source {
                crate::discovery::CandidateSource::DesktopApp => ProviderInstallSource::App,
                _ => ProviderInstallSource::Cli,
            });
            (app.map(display_fs_path), cli.map(display_fs_path), used)
        }
        ProviderKind::Claude => {
            let app = crate::claude_desktop::bundled_cli();
            let cli = crate::claude::cli_available(configured_folder);
            let used = if app.is_some() {
                Some(ProviderInstallSource::App)
            } else {
                cli.as_ref().map(|_| ProviderInstallSource::Cli)
            };
            (
                app.as_deref().map(display_fs_path),
                cli.as_deref().map(display_fs_path),
                used,
            )
        }
        ProviderKind::Cursor => {
            let app = crate::cursor::installation_path(configured_folder);
            let used = app.as_ref().map(|_| ProviderInstallSource::App);
            (app.as_deref().map(display_fs_path), None, used)
        }
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => {
            let detected = crate::opencode::is_installed(provider);
            let detail = detected.then(|| "OpenCode auth.json or local database".into());
            (detail, None, detected.then_some(ProviderInstallSource::App))
        }
        ProviderKind::OpenRouter => (None, None, None),
        ProviderKind::Antigravity => {
            let app = crate::antigravity::desktop_app(configured_folder);
            let cli = crate::antigravity::cli_available(configured_folder);
            let used = if cli.is_some() {
                Some(ProviderInstallSource::Cli)
            } else {
                app.as_ref().map(|_| ProviderInstallSource::App)
            };
            (
                app.as_deref().map(display_fs_path),
                cli.as_deref().map(display_fs_path),
                used,
            )
        }
        ProviderKind::Grok => {
            let cli = crate::grok::cli_available(configured_folder);
            (
                None,
                cli.as_deref().map(display_fs_path),
                cli.as_ref().map(|_| ProviderInstallSource::Cli),
            )
        }
        ProviderKind::Kiro => {
            unreachable!("Kiro status is resolved before the provider match")
        }
    };
    ProviderInstallStatus {
        app,
        crew: None,
        cli,
        used,
        app_applicable: provider != ProviderKind::Grok,
        crew_applicable: false,
        cli_applicable: matches!(
            provider,
            ProviderKind::Codex
                | ProviderKind::Claude
                | ProviderKind::Antigravity
                | ProviderKind::Grok
        ),
        checking: false,
    }
}

pub(crate) fn provider_install_status_kiro(
    configured_app_folder: &str,
    configured_crew_folder: &str,
    configured_cli_folder: &str,
) -> ProviderInstallStatus {
    let configured_app_folder = (!configured_app_folder.trim().is_empty())
        .then(|| std::path::Path::new(configured_app_folder.trim()));
    let configured_crew_folder = (!configured_crew_folder.trim().is_empty())
        .then(|| std::path::Path::new(configured_crew_folder.trim()));
    let configured_cli_folder = (!configured_cli_folder.trim().is_empty())
        .then(|| std::path::Path::new(configured_cli_folder.trim()));
    let app_path = crate::kiro::ide_source_path(configured_app_folder);
    let crew_path = crate::kiro::crew_installation_path(configured_crew_folder);
    let cli_path = crate::kiro::cli_path(configured_cli_folder);
    let selected = crate::kiro::selected_source(
        configured_app_folder,
        configured_crew_folder,
        configured_cli_folder,
    );
    ProviderInstallStatus {
        app: app_path.as_deref().map(display_fs_path),
        crew: crew_path.as_deref().map(display_fs_path),
        cli: cli_path.as_deref().map(display_fs_path),
        used: match selected {
            Some(crate::kiro::KiroSource::Ide) => Some(ProviderInstallSource::App),
            Some(crate::kiro::KiroSource::Crew) => Some(ProviderInstallSource::Crew),
            Some(crate::kiro::KiroSource::Cli) => Some(ProviderInstallSource::Cli),
            None => None,
        },
        app_applicable: true,
        crew_applicable: true,
        cli_applicable: true,
        checking: false,
    }
}

/// `fs::canonicalize` on Windows prefixes `\\?\`. Keep the Settings UI on the
/// ordinary drive-letter form the rest of the app already shows.
fn display_fs_path(path: &std::path::Path) -> String {
    let raw = path.display().to_string();
    if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = raw.strip_prefix(r"\\?\") {
        rest.to_owned()
    } else {
        raw
    }
}

const NOT_OPENROUTER_KEY: &str =
    "That doesn't look like an OpenRouter key. Keys start with sk-or-.";

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

pub(crate) fn provider_readiness(status: &ProviderInstallStatus) -> ProviderReadiness {
    if status.checking {
        ProviderReadiness::Checking
    } else if status.used.is_some() {
        ProviderReadiness::Ready
    } else {
        ProviderReadiness::NeedsSetup
    }
}

fn plural(count: usize, word: &str) -> String {
    format!("{count} {word}{}", if count == 1 { "" } else { "s" })
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
fn persist_instance(
    settings_tx: Sender<Settings>,
    provider: ProviderId,
    mutate: impl FnOnce(&mut ProviderInstance) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    try_persist_update_fallible(settings_tx, move |settings| {
        let instance = settings
            .instance_mut(provider)
            .ok_or_else(|| anyhow::anyhow!("This provider no longer exists."))?;
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
    mutate: impl FnOnce(&mut OpenRouterAccount) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    try_persist_update_fallible(settings_tx, move |settings| {
        let provider = find_account_instance(&settings.instances, &account_id)
            .map(ProviderInstance::provider_id)
            .ok_or_else(|| anyhow::anyhow!("OpenRouter account no longer exists"))?;
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
fn persist_openrouter_credentials(
    settings_tx: Sender<Settings>,
    account_id: String,
    changes: Vec<crate::openrouter::AccountSecretChange>,
    mutate: impl FnOnce(&mut OpenRouterAccount) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let current = Settings::load_or_create(&Settings::default_path()?)?;
    let before = find_account_instance(&current.instances, &account_id)
        .and_then(|instance| instance.openrouter.as_ref())
        .is_some_and(|account| {
            crate::openrouter::has_management_key(std::slice::from_ref(account))
        });
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

/// Forgets every secret Minibar stored for a deleted instance. Config folders
/// belong to the CLI and are left on disk.
fn forget_instance_secrets(instance: &ProviderInstance) {
    let provider = instance.provider_id();
    match instance.driver {
        ProviderKind::Claude => {
            if let Err(error) = crate::claude::save_manual_credential(&instance.id, None) {
                eprintln!("failed to delete the Claude credential: {error:#}");
            }
        }
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => {
            if let Err(error) = crate::opencode::save_manual_key(provider, None) {
                eprintln!("failed to delete the OpenCode key: {error:#}");
            }
        }
        ProviderKind::OpenRouter => {
            if let Some(account) = &instance.openrouter {
                let mut changes = account
                    .api_key_ids
                    .iter()
                    .map(|key_id| {
                        crate::openrouter::AccountSecretChange::api_key(
                            account.id.clone(),
                            key_id.clone(),
                            None,
                        )
                    })
                    .collect::<Vec<_>>();
                changes.push(crate::openrouter::AccountSecretChange::management(
                    account.id.clone(),
                    None,
                ));
                if let Err(error) = crate::openrouter::apply_account_secret_changes(&changes) {
                    eprintln!("failed to delete the OpenRouter keys: {error:#}");
                }
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

/// Which path field of an instance a folder picker edits.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PathField {
    /// CLI/app location (Kiro: IDE).
    Binary,
    KiroCrew,
    KiroCli,
    /// `CLAUDE_CONFIG_DIR` / `CODEX_HOME`.
    ConfigFolder,
}

impl PathField {
    fn key(self) -> &'static str {
        match self {
            Self::Binary => "binary",
            Self::KiroCrew => "kiro-crew",
            Self::KiroCli => "kiro-cli",
            Self::ConfigFolder => "config-folder",
        }
    }

    fn read(self, instance: &ProviderInstance) -> String {
        let path = match self {
            Self::Binary => instance.binary_path.as_ref(),
            Self::KiroCrew => instance.kiro_crew_path.as_ref(),
            Self::KiroCli => instance.kiro_cli_path.as_ref(),
            Self::ConfigFolder => match &instance.source {
                InstanceSource::ConfigFolder { path } => path.as_ref(),
                InstanceSource::Manual => None,
            },
        };
        path.map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    fn write(self, instance: &mut ProviderInstance, folder: Option<PathBuf>) {
        match self {
            Self::Binary => instance.binary_path = folder,
            Self::KiroCrew => instance.kiro_crew_path = folder,
            Self::KiroCli => instance.kiro_cli_path = folder,
            Self::ConfigFolder => {
                instance.source = InstanceSource::ConfigFolder { path: folder };
            }
        }
    }
}

pub(crate) struct FolderConfig {
    field: PathField,
    label: &'static str,
    description: &'static str,
    placeholder: &'static str,
}

fn folder_configs(driver: ProviderKind) -> Vec<FolderConfig> {
    let binary = |label, description, placeholder| FolderConfig {
        field: PathField::Binary,
        label,
        description,
        placeholder,
    };
    match driver {
        ProviderKind::Codex => vec![binary(
            "Codex CLI folder",
            "Folder with codex.exe, codex.cmd, or codex.ps1. Leave empty to find it automatically.",
            r"C:\Users\you\AppData\Roaming\npm",
        )],
        ProviderKind::Claude => vec![binary(
            "Claude Code CLI folder",
            "Folder with claude.exe, claude.cmd, or claude.ps1. Leave empty to find it automatically.",
            r"C:\Users\you\AppData\Roaming\npm",
        )],
        ProviderKind::Cursor => vec![binary(
            "Cursor app folder",
            "Folder with Cursor.exe. Leave empty to find it automatically. Usage still comes from the signed-in profile.",
            r"C:\Users\you\AppData\Local\Programs\Cursor",
        )],
        ProviderKind::Antigravity => vec![binary(
            "agy CLI folder",
            "Folder with agy.exe, agy.cmd, or agy.ps1. Leave empty to find it automatically.",
            r"C:\Users\you\AppData\Local\agy\bin",
        )],
        ProviderKind::Grok => vec![binary(
            "Grok CLI folder",
            "Folder with grok.exe, grok.cmd, or grok.ps1. Leave empty to find it automatically.",
            r"C:\Users\you\.grok\bin",
        )],
        ProviderKind::Kiro => vec![
            binary(
                "Kiro IDE folder",
                "Folder containing Kiro.exe, or the executable itself. Leave empty to find it automatically.",
                r"C:\Users\you\AppData\Local\Programs\Kiro",
            ),
            FolderConfig {
                field: PathField::KiroCrew,
                label: "Kiro Crew app path",
                description: "Folder containing KiroCrew.exe, or the executable itself. Leave empty to detect per-user and all-users installs automatically.",
                placeholder: r"C:\Users\you\AppData\Local\Programs\KiroCrew",
            },
            FolderConfig {
                field: PathField::KiroCli,
                label: "Kiro CLI folder",
                description: "Folder containing kiro-cli.exe, or the executable itself. Leave empty to find it automatically.",
                placeholder: r"C:\Users\you\AppData\Local\kiro-cli",
            },
        ],
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo | ProviderKind::OpenRouter => {
            Vec::new()
        }
    }
}

fn provider_description(provider: ProviderKind) -> &'static str {
    match provider {
        ProviderKind::Codex => "Reads the signed-in Codex CLI or desktop app.",
        ProviderKind::Claude => "Reads your existing Claude Code login.",
        ProviderKind::Cursor => "Reads the signed-in Cursor app for this billing cycle.",
        ProviderKind::OpenCodeZen => "Reads Zen auth and local OpenCode history.",
        ProviderKind::OpenCodeGo => "Reads Go quota windows and local OpenCode history.",
        ProviderKind::OpenRouter => {
            "Reads API-key usage and spend limits. A management key also enables usage history and credit balance."
        }
        ProviderKind::Antigravity => {
            "Reads subscription quota from your existing official agy Windows sign-in."
        }
        ProviderKind::Grok => {
            "Reads SuperGrok subscription credits from your existing official Grok CLI sign-in."
        }
        ProviderKind::Kiro => {
            "Fetches Kiro's live monthly credits with its shared sign-in; recognizes IDE, Crew, and CLI installs."
        }
    }
}

/// Display names for the app, Crew app, and CLI sources a provider can read from.
fn source_labels(provider: ProviderKind) -> (&'static str, &'static str, &'static str) {
    match provider {
        ProviderKind::Codex => ("Codex desktop app", "", "Codex CLI"),
        ProviderKind::Claude => ("Claude desktop app", "", "Claude Code CLI"),
        ProviderKind::Cursor => ("Cursor app", "", ""),
        ProviderKind::Antigravity => ("Antigravity app", "", "agy CLI"),
        ProviderKind::Grok => ("", "", "Grok CLI"),
        ProviderKind::Kiro => ("Kiro IDE", "Kiro Crew", "Kiro CLI"),
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo | ProviderKind::OpenRouter => {
            ("", "", "")
        }
    }
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
                (exists && single)
                    .then(|| format!("{} supports one instance", driver.display_name())),
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
