//! The one place that maps a [`ProviderKind`] to the code that runs it.
//! Generic code starts, detects, describes, forgets and resets instances
//! through this module; each provider module owns its [`Driver`].

use std::path::Path;

use anyhow::{Result, bail};

use crate::{
    providers::instances::{PathField, ProviderInstance},
    providers::registry::ProviderKind,
    providers::worker::{WorkerHandle, WorkerOptions},
    quota::banked_reset::Outcome,
    quota::limits::RateLimits,
};

/// A redeemed reset and the quota read right after it.
pub(crate) type ResetResult = Result<(Outcome, Result<RateLimits>)>;

/// One provider's runtime behavior.
pub(crate) struct Driver {
    pub(crate) start: fn(&ProviderInstance, WorkerOptions) -> Result<WorkerHandle>,
    /// What an instance reads from on this PC. Filesystem and protected
    /// storage only: never a process or a request.
    pub(crate) detect: fn(&ProviderInstance) -> InstallStatus,
    /// The sources `detect` reports on.
    pub(crate) sources: Sources,
    pub(crate) copy: fn() -> SettingsCopy,
    /// Deletes what Minibar stored in protected storage for an instance.
    /// Config folders belong to the CLI and stay on disk.
    pub(crate) forget_secrets: Option<fn(&ProviderInstance) -> Result<()>>,
    /// Spends a banked reset credit: the listed grant with this id when one is
    /// given, otherwise the one the provider selects.
    pub(crate) consume_reset: Option<fn(&ProviderInstance, Option<&str>) -> ResetResult>,
}

pub(crate) fn driver(kind: ProviderKind) -> &'static Driver {
    match kind {
        ProviderKind::Codex => &crate::providers::codex::DRIVER,
        ProviderKind::Claude => &crate::providers::claude::DRIVER,
        ProviderKind::Cursor => &crate::providers::cursor::DRIVER,
        ProviderKind::OpenCodeZen => &crate::providers::opencode::ZEN_DRIVER,
        ProviderKind::OpenCodeGo => &crate::providers::opencode::GO_DRIVER,
        ProviderKind::OpenRouter => &crate::providers::openrouter::DRIVER,
        ProviderKind::Antigravity => &crate::providers::antigravity::DRIVER,
        ProviderKind::Grok => &crate::providers::grok::DRIVER,
        ProviderKind::Kiro => &crate::providers::kiro::DRIVER,
        ProviderKind::CommandCode => &crate::providers::commandcode::DRIVER,
    }
}

pub(crate) fn start_worker(
    instance: &ProviderInstance,
    options: WorkerOptions,
) -> Result<WorkerHandle> {
    if crate::providers::fake::serves(instance.driver) {
        return Ok(crate::providers::fake::start(instance.driver, options));
    }
    (driver(instance.driver).start)(instance, options)
}

pub(crate) fn consume_reset(instance: &ProviderInstance, grant: Option<&str>) -> ResetResult {
    if crate::providers::fake::serves(instance.driver) {
        return crate::providers::fake::consume(instance.driver);
    }
    match driver(instance.driver).consume_reset {
        Some(consume) => consume(instance, grant),
        None => bail!("Provider does not support reset credits"),
    }
}

pub(crate) fn detect(instance: &ProviderInstance) -> InstallStatus {
    (driver(instance.driver).detect)(instance)
}

pub(crate) fn settings_copy(kind: ProviderKind) -> SettingsCopy {
    (driver(kind).copy)()
}

/// Forgets the secrets of a removed instance. The instance is gone either
/// way, so a failure only leaves an unused value behind.
pub(crate) fn forget_secrets(instance: &ProviderInstance) {
    if let Some(forget) = driver(instance.driver).forget_secrets
        && let Err(error) = forget(instance)
    {
        crate::platform::logger::info(format!(
            "failed to delete the secrets of {}: {error:#}",
            instance.display_name()
        ));
    }
}

/// The kinds of installation a driver can read from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Sources {
    pub(crate) app: bool,
    pub(crate) crew: bool,
    pub(crate) cli: bool,
}

impl Sources {
    pub(crate) const APP: Self = Self {
        app: true,
        crew: false,
        cli: false,
    };
    pub(crate) const CLI: Self = Self {
        app: false,
        crew: false,
        cli: true,
    };
    pub(crate) const APP_AND_CLI: Self = Self {
        app: true,
        crew: false,
        cli: true,
    };
    pub(crate) const ALL: Self = Self {
        app: true,
        crew: true,
        cli: true,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InstallSource {
    App,
    Crew,
    Cli,
}

/// What [`detect`] found for one instance. Paths are display strings; key
/// based drivers put a short description of the saved credential in `app`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct InstallStatus {
    pub(crate) sources: Sources,
    pub(crate) app: Option<String>,
    pub(crate) crew: Option<String>,
    pub(crate) cli: Option<String>,
    /// The source the worker reads.
    pub(crate) used: Option<InstallSource>,
    /// A sign-in the driver reads directly, even without its app or CLI.
    pub(crate) signed_in: bool,
    /// Detection has not finished yet.
    pub(crate) checking: bool,
}

impl InstallStatus {
    /// Nothing found, reporting on the sources this instance can use.
    pub(crate) fn none(instance: &ProviderInstance) -> Self {
        Self {
            // A pasted credential is the only thing such an instance reads.
            sources: if instance.uses_manual_credential() {
                Sources::APP
            } else {
                driver(instance.driver).sources
            },
            app: None,
            crew: None,
            cli: None,
            used: None,
            signed_in: false,
            checking: false,
        }
    }

    pub(crate) fn checking(instance: &ProviderInstance) -> Self {
        Self {
            checking: true,
            ..Self::none(instance)
        }
    }

    /// Whether the worker has something to read.
    pub(crate) fn ready(&self) -> bool {
        !self.checking && (self.used.is_some() || self.signed_in)
    }
}

/// `fs::canonicalize` on Windows prefixes `\\?\`. Keep the Settings UI on the
/// ordinary drive-letter form the rest of the app already shows.
pub(crate) fn display_path(path: &Path) -> String {
    let raw = path.display().to_string();
    if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = raw.strip_prefix(r"\\?\") {
        rest.to_owned()
    } else {
        raw
    }
}

/// How an instance proves who it is, which picks its Account section and
/// the wording of its status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Credential {
    /// A login on this PC: a CLI config folder, which Settings can sign in
    /// to, or whatever the installed app or CLI holds.
    Login,
    /// An API key Minibar keeps, or the provider's own sign-in on the
    /// primary instance.
    ApiKey,
    /// API keys grouped into accounts, plus an optional management key.
    KeyAccounts,
}

/// How Settings and onboarding describe a driver.
pub(crate) struct SettingsCopy {
    /// What the driver reads, shown under the Runtime heading.
    pub(crate) description: &'static str,
    /// Names of the app, Crew app and CLI sources; empty when not applicable.
    pub(crate) app: &'static str,
    pub(crate) crew: &'static str,
    pub(crate) cli: &'static str,
    pub(crate) folders: Vec<FolderField>,
    /// Onboarding's note when detection found the provider or not.
    pub(crate) found: &'static str,
    pub(crate) not_found: &'static str,
    pub(crate) credential: Credential,
    /// The source whose folder is the instance's binary path, offered a
    /// folder picker when it is not found.
    pub(crate) binary_source: Option<InstallSource>,
    /// Environment variable that hands a config folder to the CLI.
    pub(crate) config_env: &'static str,
    /// Whether an instance can read a pasted credential instead of a login.
    pub(crate) manual_credential: bool,
    /// Shown on the app source when an instance reads its own config folder.
    pub(crate) config_folder_app_note: Option<&'static str>,
}

impl SettingsCopy {
    pub(crate) fn new(description: &'static str) -> Self {
        Self {
            description,
            app: "",
            crew: "",
            cli: "",
            folders: Vec::new(),
            found: crate::i18n::tr("found-on-this-pc"),
            not_found: crate::i18n::tr("not-found-turn-it-on-if-it-s-installed-somewhere-else"),
            credential: Credential::Login,
            binary_source: Some(InstallSource::Cli),
            config_env: "",
            manual_credential: false,
            config_folder_app_note: None,
        }
    }
}

/// A location the user can set for an instance.
pub(crate) struct FolderField {
    pub(crate) field: PathField,
    pub(crate) label: &'static str,
    pub(crate) description: &'static str,
    pub(crate) placeholder: &'static str,
}

impl FolderField {
    pub(crate) fn binary(
        label: &'static str,
        description: &'static str,
        placeholder: &'static str,
    ) -> Self {
        Self {
            field: PathField::Binary,
            label,
            description,
            placeholder,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_driver_agrees_with_its_registry_entry() {
        use crate::providers::registry::{BankedResets, descriptor};
        for kind in ProviderKind::ALL {
            let descriptor = descriptor(kind);
            let copy = settings_copy(kind);
            let keeps_secrets = copy.manual_credential || copy.credential != Credential::Login;
            assert_eq!(
                driver(kind).consume_reset.is_some(),
                descriptor.banked_resets != BankedResets::Unsupported,
                "{kind:?} reset support"
            );
            // Removing an instance must be able to delete what it stored.
            assert_eq!(
                driver(kind).forget_secrets.is_some(),
                keeps_secrets,
                "{kind:?} secrets"
            );
            // Several instances need an isolating folder or credential.
            assert!(
                !descriptor.supports_multiple_instances
                    || descriptor.config_folder
                    || keeps_secrets,
                "{kind:?} instances"
            );
            assert_eq!(
                descriptor.config_folder,
                !copy.config_env.is_empty(),
                "{kind:?} config folder"
            );
        }
    }
}
