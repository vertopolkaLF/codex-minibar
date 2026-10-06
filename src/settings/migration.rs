//! Version 39: accounts inside providers become provider instances.
//!
//! The document-level part runs inside [`super::migrate`] and is pure. Moving
//! credentials out of protected storage into instance folders needs the
//! secret store and the file system, so it runs once afterwards from
//! [`super::Settings::load_or_create`] through [`finish_instance_migration`].

use std::{collections::BTreeMap, path::PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::{
    HomeWidgetId, InstanceSource, OpenRouterAccount, PopupTabMode, ProviderInstance, ProviderKind,
    Settings,
};

/// First settings version that stores provider instances.
pub(super) const INSTANCES_VERSION: u32 = 39;

/// The built-in profile that followed this PC's login.
const DEFAULT_PROFILE_ID: &str = "default";

#[derive(Default, Deserialize)]
#[serde(default)]
struct LegacyProviders {
    enabled: Vec<String>,
}

#[derive(Clone, Deserialize)]
struct LegacyProfile {
    id: String,
    name: String,
    #[serde(default = "enabled_by_default")]
    enabled: bool,
}

fn enabled_by_default() -> bool {
    true
}

#[derive(Clone, Deserialize)]
struct LegacyHomeWidget {
    kind: String,
    #[serde(default)]
    profile: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct LegacyPopupVisibility {
    provider_all_tab: BTreeMap<String, bool>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct LegacyV38 {
    providers: LegacyProviders,
    popup_order: Vec<String>,
    popup_right_column: Option<Vec<String>>,
    popup_home_order: Vec<LegacyHomeWidget>,
    popup_home_right_column: Option<Vec<LegacyHomeWidget>>,
    popup_visibility: LegacyPopupVisibility,
    show_accounts_as_tabs: bool,
    automatic_activation: bool,
    usage_stats_excluded_providers: Vec<String>,
    codex_path: Option<PathBuf>,
    claude_path: Option<PathBuf>,
    cursor_path: Option<PathBuf>,
    antigravity_path: Option<PathBuf>,
    grok_path: Option<PathBuf>,
    kiro_path: Option<PathBuf>,
    kiro_crew_path: Option<PathBuf>,
    kiro_cli_path: Option<PathBuf>,
    openrouter_accounts: Vec<OpenRouterAccount>,
    codex_profiles: Vec<LegacyProfile>,
    claude_profiles: Vec<LegacyProfile>,
    codex_home_excluded_profiles: Vec<String>,
    claude_home_excluded_profiles: Vec<String>,
}

/// Keys that only existed before instances.
const LEGACY_KEYS: &[&str] = &[
    "providers",
    "popup_order",
    "popup_right_column",
    "show_accounts_as_tabs",
    "automatic_activation",
    "usage_stats_excluded_providers",
    "codex_path",
    "claude_path",
    "cursor_path",
    "antigravity_path",
    "grok_path",
    "kiro_path",
    "kiro_crew_path",
    "kiro_cli_path",
    "opencode_zen_credentials_revision",
    "opencode_go_credentials_revision",
    "openrouter_credentials_revision",
    "openrouter_accounts",
    "codex_profiles",
    "codex_home_excluded_profiles",
    "codex_credentials_revision",
    "codex_profile_credential_revisions",
    "claude_profiles",
    "claude_home_excluded_profiles",
    "claude_credentials_revision",
    "claude_profile_credential_revisions",
];

/// Former Home-widget ids, which differ from provider ids for OpenCode.
fn widget_driver(kind: &str) -> Option<ProviderKind> {
    match kind {
        "open_code_zen" => Some(ProviderKind::OpenCodeZen),
        "open_code_go" => Some(ProviderKind::OpenCodeGo),
        other => ProviderKind::from_id(other),
    }
}

fn to_value<T: Serialize>(value: &T) -> Result<toml::Value> {
    toml::Value::try_from(value).context("encode migrated settings")
}

pub(super) fn migrate_to_instances(document: &mut toml::Value) -> Result<()> {
    let legacy: LegacyV38 = document.clone().try_into().unwrap_or_default();
    let root = document
        .as_table_mut()
        .context("settings root must be a TOML table")?;
    let migrated = instances_from_legacy(&legacy);

    root.insert("instances".into(), to_value(&migrated.instances)?);
    root.insert(
        "popup_tab_mode".into(),
        to_value(&PopupTabMode::default())?,
    );
    root.insert(
        "popup_home_order".into(),
        to_value(&migrated.home_order)?,
    );
    match &migrated.home_right_column {
        Some(right) => {
            root.insert("popup_home_right_column".into(), to_value(right)?);
        }
        None => {
            root.remove("popup_home_right_column");
        }
    }
    if let Some(toml::Value::Table(visibility)) = root.get_mut("popup_visibility") {
        visibility.remove("provider_all_tab");
    }
    migrate_tray_profiles(root);
    for key in LEGACY_KEYS {
        root.remove(*key);
    }
    root.insert(
        "version".into(),
        toml::Value::Integer(i64::from(INSTANCES_VERSION)),
    );
    Ok(())
}

struct Migrated {
    instances: Vec<ProviderInstance>,
    home_order: Vec<HomeWidgetId>,
    home_right_column: Option<Vec<HomeWidgetId>>,
}

/// Maps one former account to its instance id. The Default account becomes
/// the driver's primary instance, which keeps the legacy provider id.
fn profile_instance_id(driver: ProviderKind, profile: Option<&str>) -> String {
    match profile {
        Some(profile) if profile != DEFAULT_PROFILE_ID => profile.to_owned(),
        _ => driver.id().to_owned(),
    }
}

fn profiles_with_default(saved: &[LegacyProfile]) -> Vec<LegacyProfile> {
    let mut profiles = saved.to_vec();
    if !profiles.iter().any(|profile| profile.id == DEFAULT_PROFILE_ID) {
        profiles.insert(
            0,
            LegacyProfile {
                id: DEFAULT_PROFILE_ID.into(),
                name: "Default".into(),
                enabled: true,
            },
        );
    }
    profiles
}

fn instances_from_legacy(legacy: &LegacyV38) -> Migrated {
    let mut drivers = legacy
        .popup_order
        .iter()
        .filter_map(|kind| widget_driver(kind))
        .collect::<Vec<_>>();
    for driver in ProviderKind::ALL {
        if !drivers.contains(&driver) {
            drivers.push(driver);
        }
    }
    let driver_enabled = |driver: ProviderKind| {
        legacy
            .providers
            .enabled
            .iter()
            .any(|id| id == driver.id())
    };
    let shown_on_home = |driver: ProviderKind| {
        legacy
            .popup_visibility
            .provider_all_tab
            .get(driver.id())
            .copied()
            .unwrap_or(true)
    };
    let usage_stats = |driver: ProviderKind| {
        !legacy
            .usage_stats_excluded_providers
            .iter()
            .any(|id| id == driver.id())
    };

    let mut instances = Vec::new();
    for driver in drivers {
        let base = ProviderInstance {
            enabled: driver_enabled(driver),
            show_on_home: shown_on_home(driver),
            usage_stats: usage_stats(driver),
            ..ProviderInstance::primary(driver)
        };
        match driver {
            ProviderKind::Claude | ProviderKind::Codex => {
                let (saved, excluded, binary_path) = if driver == ProviderKind::Claude {
                    (
                        &legacy.claude_profiles,
                        &legacy.claude_home_excluded_profiles,
                        &legacy.claude_path,
                    )
                } else {
                    (
                        &legacy.codex_profiles,
                        &legacy.codex_home_excluded_profiles,
                        &legacy.codex_path,
                    )
                };
                for profile in profiles_with_default(saved) {
                    let primary = profile.id == DEFAULT_PROFILE_ID;
                    let name = profile.name.trim();
                    let name = if primary && (name.is_empty() || name == "Default") {
                        driver.display_name().to_owned()
                    } else {
                        name.to_owned()
                    };
                    instances.push(ProviderInstance {
                        id: profile_instance_id(driver, Some(&profile.id)),
                        name,
                        enabled: base.enabled && profile.enabled,
                        show_on_home: base.show_on_home && !excluded.contains(&profile.id),
                        // Activation only ever ran with the local login.
                        auto_activation: primary && legacy.automatic_activation,
                        binary_path: binary_path.clone(),
                        // Saved Claude accounts are resolved once the secret
                        // store can say whether they hold a sign-in session.
                        source: if !primary && driver == ProviderKind::Claude {
                            InstanceSource::Manual
                        } else {
                            InstanceSource::default()
                        },
                        ..base.clone()
                    });
                }
            }
            ProviderKind::OpenRouter => {
                let mut accounts = legacy.openrouter_accounts.iter();
                instances.push(ProviderInstance {
                    openrouter: Some(
                        accounts
                            .next()
                            .cloned()
                            .unwrap_or_else(OpenRouterAccount::legacy),
                    ),
                    ..base.clone()
                });
                for account in accounts {
                    instances.push(ProviderInstance {
                        id: account.id.clone(),
                        name: account.name.clone(),
                        openrouter: Some(account.clone()),
                        ..base.clone()
                    });
                }
            }
            ProviderKind::Cursor => instances.push(ProviderInstance {
                binary_path: legacy.cursor_path.clone(),
                ..base
            }),
            ProviderKind::Antigravity => instances.push(ProviderInstance {
                binary_path: legacy.antigravity_path.clone(),
                ..base
            }),
            ProviderKind::Grok => instances.push(ProviderInstance {
                binary_path: legacy.grok_path.clone(),
                ..base
            }),
            ProviderKind::Kiro => instances.push(ProviderInstance {
                binary_path: legacy.kiro_path.clone(),
                kiro_crew_path: legacy.kiro_crew_path.clone(),
                kiro_cli_path: legacy.kiro_cli_path.clone(),
                ..base
            }),
            ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => instances.push(base),
        }
    }

    let widget_ids = |widget: &LegacyHomeWidget| -> Vec<HomeWidgetId> {
        if widget.kind == HomeWidgetId::TOTAL_SPEND {
            return vec![HomeWidgetId::total_spend()];
        }
        let Some(driver) = widget_driver(&widget.kind) else {
            return Vec::new();
        };
        let id = profile_instance_id(driver, widget.profile.as_deref());
        if instances.iter().any(|instance| instance.id == id) {
            vec![HomeWidgetId(id)]
        } else {
            Vec::new()
        }
    };
    let driver_widgets = |kind: &str| -> Vec<HomeWidgetId> {
        if kind == HomeWidgetId::TOTAL_SPEND {
            return vec![HomeWidgetId::total_spend()];
        }
        let Some(driver) = widget_driver(kind) else {
            return Vec::new();
        };
        instances
            .iter()
            .filter(|instance| instance.driver == driver)
            .map(|instance| HomeWidgetId(instance.id.clone()))
            .collect()
    };

    let mut home_order = Vec::<HomeWidgetId>::new();
    let push_unique = |order: &mut Vec<HomeWidgetId>, ids: Vec<HomeWidgetId>| {
        for id in ids {
            if !order.contains(&id) {
                order.push(id);
            }
        }
    };
    if legacy.popup_home_order.is_empty() {
        for kind in &legacy.popup_order {
            push_unique(&mut home_order, driver_widgets(kind));
        }
    } else {
        for widget in &legacy.popup_home_order {
            push_unique(&mut home_order, widget_ids(widget));
        }
    }
    let home_right_column = match (&legacy.popup_home_right_column, &legacy.popup_right_column) {
        (Some(right), _) => Some(right.iter().flat_map(widget_ids).collect()),
        (None, Some(right)) => Some(
            right
                .iter()
                .flat_map(|kind| driver_widgets(kind))
                .collect(),
        ),
        (None, None) => None,
    };
    Migrated {
        instances,
        home_order,
        home_right_column,
    }
}

/// Tray indicators named an account with `profile_id`; instances replace that.
fn migrate_tray_profiles(root: &mut toml::map::Map<String, toml::Value>) {
    let Some(toml::Value::Array(widgets)) = root.get_mut("tray_widgets") else {
        return;
    };
    for widget in widgets {
        let Some(toml::Value::Array(indicators)) = widget.get_mut("indicators") else {
            continue;
        };
        for indicator in indicators {
            let Some(indicator) = indicator.as_table_mut() else {
                continue;
            };
            let profile = indicator
                .remove("profile_id")
                .and_then(|value| value.as_str().map(str::to_owned));
            let Some(driver) = indicator
                .get("provider")
                .and_then(toml::Value::as_str)
                .and_then(ProviderKind::from_id)
            else {
                continue;
            };
            if matches!(driver, ProviderKind::Claude | ProviderKind::Codex) {
                indicator.insert(
                    "provider".into(),
                    toml::Value::String(profile_instance_id(driver, profile.as_deref())),
                );
            }
        }
    }
}

/// Former protected-storage slots for saved accounts.
fn claude_secret(id: &str) -> String {
    format!("claude-profile-{id}")
}

fn codex_secret(id: &str) -> String {
    format!("codex.profile.{id}")
}

/// Moves saved sign-in sessions into the instances' config folders, where the
/// CLI keeps and refreshes them from now on. Pasted Claude credentials stay in
/// protected storage as manual credentials. Returns whether settings changed.
pub(super) fn finish_instance_migration(settings: &mut Settings) -> bool {
    finish_with(
        settings,
        |name| crate::secrets::load(name).ok().flatten(),
        |name| {
            let _ = crate::secrets::save(name, None);
        },
        crate::openrouter::key_is_configured(),
        |instance| crate::instances::managed_folder(&instance.id),
    )
}

fn finish_with(
    settings: &mut Settings,
    load: impl Fn(&str) -> Option<String>,
    forget: impl Fn(&str),
    legacy_openrouter_key: bool,
    folder: impl Fn(&ProviderInstance) -> Result<PathBuf>,
) -> bool {
    let mut changed = false;
    for instance in &mut settings.instances {
        if instance.is_primary() {
            continue;
        }
        let exported = match instance.driver {
            ProviderKind::Claude => {
                let secret = claude_secret(&instance.id);
                let Some(raw) = load(&secret) else {
                    continue;
                };
                // Sign-in sessions are JSON; pasted tokens and cookies are not.
                if !raw.trim_start().starts_with('{') {
                    continue;
                }
                folder(instance)
                    .and_then(|folder| export_claude_session(&folder, &raw))
                    .map(|()| secret)
            }
            ProviderKind::Codex => {
                let secret = codex_secret(&instance.id);
                let Some(raw) = load(&secret) else {
                    continue;
                };
                folder(instance)
                    .and_then(|folder| export_codex_session(&folder, &raw))
                    .map(|()| secret)
            }
            _ => continue,
        };
        match exported {
            Ok(secret) => {
                instance.source = InstanceSource::ConfigFolder { path: None };
                forget(&secret);
                changed = true;
            }
            Err(error) => crate::logger::info(format!(
                "Could not move the {} login for {} into its folder: {error:#}",
                instance.driver.display_name(),
                instance.display_name()
            )),
        }
    }
    // Before accounts existed, a lone OpenRouter key was implied; keep it.
    if legacy_openrouter_key
        && !settings.instances.iter().any(|instance| {
            instance
                .openrouter
                .as_ref()
                .is_some_and(|account| account.id == OpenRouterAccount::legacy().id)
        })
    {
        let mut instance = ProviderInstance::new(ProviderKind::OpenRouter, "OpenRouter key");
        instance.enabled = settings
            .instances
            .iter()
            .any(|existing| existing.driver == ProviderKind::OpenRouter && existing.enabled);
        instance.openrouter = Some(OpenRouterAccount::legacy());
        settings.add_instance(instance);
        changed = true;
    }
    changed
}

fn write_private(folder: &std::path::Path, file: &str, contents: &[u8]) -> Result<()> {
    std::fs::create_dir_all(folder).with_context(|| format!("create {}", folder.display()))?;
    let target = folder.join(file);
    if target.exists() {
        // Never overwrite a login the CLI already keeps there.
        return Ok(());
    }
    let mut temporary =
        tempfile::NamedTempFile::new_in(folder).context("create temporary login file")?;
    use std::io::Write;
    temporary.write_all(contents)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(&target)
        .with_context(|| format!("write {}", target.display()))?;
    Ok(())
}

fn export_claude_session(folder: &std::path::Path, raw: &str) -> Result<()> {
    let session: serde_json::Value = serde_json::from_str(raw).context("read saved login")?;
    let file = serde_json::json!({ "claudeAiOauth": session });
    write_private(
        folder,
        ".credentials.json",
        serde_json::to_string_pretty(&file)?.as_bytes(),
    )
}

fn export_codex_session(folder: &std::path::Path, raw: &str) -> Result<()> {
    let session: serde_json::Value = serde_json::from_str(raw).context("read saved login")?;
    let file = serde_json::json!({
        "OPENAI_API_KEY": null,
        "tokens": {
            "id_token": session["id_token"],
            "access_token": session["access_token"],
            "refresh_token": session["refresh_token"],
            "account_id": session["account_id"],
        },
        "last_refresh": session["last_refresh"],
    });
    write_private(
        folder,
        "auth.json",
        serde_json::to_string_pretty(&file)?.as_bytes(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn migrate(raw: &str) -> Settings {
        let mut document: toml::Value = toml::from_str(raw).unwrap();
        migrate_to_instances(&mut document).unwrap();
        document.try_into().unwrap()
    }

    #[test]
    fn providers_and_accounts_become_ordered_instances() {
        let settings = migrate(
            r#"
version = 38
automatic_activation = true
usage_stats_excluded_providers = ["cursor"]
claude_path = 'C:\tools\claude.exe'
popup_order = ["total_spend", "claude", "codex", "cursor", "open_code_zen"]
claude_home_excluded_profiles = ["work"]

[providers]
enabled = ["claude", "cursor"]

[[claude_profiles]]
id = "default"
name = "Personal"
enabled = true

[[claude_profiles]]
id = "work"
name = "Work"
enabled = true

[[claude_profiles]]
id = "old"
name = "Old"
enabled = false
"#,
        );
        let ids = settings
            .instances
            .iter()
            .map(|instance| instance.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(&ids[..5], ["claude", "work", "old", "codex", "cursor"]);
        assert_eq!(ids.len(), ProviderKind::ALL.len() + 2);
        let claude = settings.instance_by_id("claude").unwrap();
        assert_eq!(claude.name, "Personal");
        assert!(claude.enabled && claude.auto_activation && claude.show_on_home);
        assert_eq!(claude.binary_path, Some(PathBuf::from(r"C:\tools\claude.exe")));
        let work = settings.instance_by_id("work").unwrap();
        assert!(work.enabled && !work.auto_activation && !work.show_on_home);
        assert_eq!(work.source, InstanceSource::Manual);
        assert_eq!(work.binary_path, claude.binary_path);
        assert!(!settings.instance_by_id("old").unwrap().enabled);
        let codex = settings.instance_by_id("codex").unwrap();
        assert!(!codex.enabled && !codex.auto_activation);
        assert_eq!(codex.name, "Codex");
        assert!(!settings.instance_by_id("cursor").unwrap().usage_stats);
        assert_eq!(settings.popup_tab_mode, PopupTabMode::Separate);
        assert_eq!(
            settings
                .popup_home_order
                .iter()
                .map(HomeWidgetId::id)
                .take(5)
                .collect::<Vec<_>>(),
            ["total_spend", "claude", "work", "old", "codex"]
        );
    }

    #[test]
    fn home_layout_tray_and_openrouter_accounts_map_to_instances() {
        let settings = migrate(
            r#"
version = 38
popup_order = ["codex", "openrouter"]

[[popup_home_order]]
kind = "codex"
profile = "acct"

[[popup_home_order]]
kind = "total_spend"

[[popup_home_order]]
kind = "codex"
profile = "default"

[[popup_home_right_column]]
kind = "codex"
profile = "acct"

[[codex_profiles]]
id = "acct"
name = "Team"

[[openrouter_accounts]]
id = "first"
name = "Main"
api_key_ids = ["k1"]

[[openrouter_accounts]]
id = "second"
name = "Side"
api_key_ids = ["k2"]

[popup_visibility.provider_all_tab]
codex = false

[[tray_widgets]]
id = "tray"
kind = "limits"
presentation = "number"

[[tray_widgets.indicators]]
provider = "codex"
metric_id = "codex.session"
profile_id = "acct"
"#,
        );
        assert_eq!(
            settings
                .popup_home_order
                .iter()
                .map(HomeWidgetId::id)
                .collect::<Vec<_>>(),
            ["acct", "total_spend", "codex"]
        );
        assert_eq!(
            settings.popup_home_right_column,
            Some(vec![HomeWidgetId("acct".into())])
        );
        assert!(!settings.instance_by_id("acct").unwrap().show_on_home);
        assert_eq!(settings.tray_widgets[0].indicators[0].provider_id, "acct");
        let primary = settings.instance_by_id("openrouter").unwrap();
        assert_eq!(primary.openrouter.as_ref().unwrap().id, "first");
        let side = settings.instance_by_id("second").unwrap();
        assert_eq!(side.name, "Side");
        assert_eq!(side.openrouter.as_ref().unwrap().api_key_ids, ["k2"]);
    }

    #[test]
    fn saved_sessions_move_into_folders_and_pasted_credentials_stay_manual() {
        let mut settings = Settings::default();
        let mut session = ProviderInstance::new(ProviderKind::Claude, "Session");
        session.source = InstanceSource::Manual;
        let mut pasted = ProviderInstance::new(ProviderKind::Claude, "Pasted");
        pasted.source = InstanceSource::Manual;
        let mut codex = ProviderInstance::new(ProviderKind::Codex, "Team");
        codex.source = InstanceSource::ConfigFolder { path: None };
        let (session_id, pasted_id, codex_id) =
            (session.id.clone(), pasted.id.clone(), codex.id.clone());
        settings.add_instance(session);
        settings.add_instance(pasted);
        settings.add_instance(codex);
        let root = tempfile::tempdir().unwrap();
        let forgotten = std::cell::RefCell::new(Vec::new());
        let changed = finish_with(
            &mut settings,
            |name| {
                if name == claude_secret(&session_id) {
                    Some(r#"{"accessToken":"sk-ant-oat-x","refreshToken":"r","expiresAt":1,"scopes":["user:profile"]}"#.into())
                } else if name == claude_secret(&pasted_id) {
                    Some("sk-ant-oat-pasted".into())
                } else if name == codex_secret(&codex_id) {
                    Some(r#"{"access_token":"a","refresh_token":"r","id_token":"i","account_id":"acc","last_refresh":null}"#.into())
                } else {
                    None
                }
            },
            |name| forgotten.borrow_mut().push(name.to_owned()),
            false,
            |instance| Ok(root.path().join(&instance.id)),
        );
        assert!(changed);
        let session = settings.instance_by_id(&session_id).unwrap();
        assert_eq!(session.source, InstanceSource::ConfigFolder { path: None });
        let pasted = settings.instance_by_id(&pasted_id).unwrap();
        assert_eq!(pasted.source, InstanceSource::Manual);
        assert_eq!(
            *forgotten.borrow(),
            [claude_secret(&session_id), codex_secret(&codex_id)]
        );
        let claude: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.path().join(&session_id).join(".credentials.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(claude["claudeAiOauth"]["accessToken"], "sk-ant-oat-x");
        let codex: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.path().join(&codex_id).join("auth.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(codex["tokens"]["account_id"], "acc");
        assert!(!root.path().join(&pasted_id).exists());
    }
}
