//! Settings file migrations, from the pre-versioned format to
//! [`SETTINGS_VERSION`]. They rewrite the TOML document before it is decoded
//! and are pure.
//!
//! Version 39 turns accounts inside providers into provider instances. Moving
//! their credentials out of protected storage into instance folders needs the
//! secret store and the file system, so startup supplies those to
//! [`finish_instance_migration`], which runs once after the load.

use std::{collections::BTreeMap, path::PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::{
    HomeWidgetId, InstanceSource, OpenRouterAccount, PopupTabMode, PopupVisibility,
    ProviderInstance, ProviderKind, Settings, TimeFormat,
};

/// The settings format this build writes.
pub const SETTINGS_VERSION: u32 = 41;
/// First version where the taskbar widget is shown by default.
pub(super) const TASKBAR_DEFAULT_VERSION: u32 = 41;
/// First version with provider instances.
pub(super) const INSTANCES_VERSION: u32 = 39;

/// Drivers the historical migrations enumerate. Frozen so adding a provider
/// never changes what migrating an old settings file produces.
pub(super) const MIGRATED_DRIVERS: [ProviderKind; 10] = [
    ProviderKind::Codex,
    ProviderKind::Claude,
    ProviderKind::Cursor,
    ProviderKind::OpenCodeZen,
    ProviderKind::OpenCodeGo,
    ProviderKind::OpenRouter,
    ProviderKind::Antigravity,
    ProviderKind::Grok,
    ProviderKind::Kiro,
    ProviderKind::CommandCode,
];

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
    root.insert("popup_tab_mode".into(), to_value(&PopupTabMode::default())?);
    root.insert("popup_home_order".into(), to_value(&migrated.home_order)?);
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
    if !profiles
        .iter()
        .any(|profile| profile.id == DEFAULT_PROFILE_ID)
    {
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
    for driver in MIGRATED_DRIVERS {
        if !drivers.contains(&driver) {
            drivers.push(driver);
        }
    }
    let driver_enabled =
        |driver: ProviderKind| legacy.providers.enabled.iter().any(|id| id == driver.id());
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
                        // Activation only ever ran with the local login of enabled providers.
                        auto_activation: primary && base.enabled && legacy.automatic_activation,
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
            ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo | ProviderKind::CommandCode => {
                instances.push(base)
            }
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
        (None, Some(right)) => Some(right.iter().flat_map(|kind| driver_widgets(kind)).collect()),
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
/// protected storage as manual credentials. `load` and `forget` reach the
/// former secrets, `legacy_openrouter_key` says whether a pre-accounts key is
/// stored, and `folder` names an instance's managed folder. Returns whether
/// settings changed.
pub(crate) fn finish_instance_migration(
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
            Err(error) => crate::platform::logger::info(format!(
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

/// Converts leftover pre-indicator tray widget tables into the current shape.
/// Safe to run on every load: modern widgets (with `indicators`) are left alone.
pub(super) fn heal_legacy_tray_widgets(document: &mut toml::Value) -> bool {
    let Some(root) = document.as_table_mut() else {
        return false;
    };
    let Some(toml::Value::Array(widgets)) = root.get_mut("tray_widgets") else {
        return false;
    };
    let mut changed = false;
    let mut healed = Vec::with_capacity(widgets.len());
    for (index, value) in widgets.iter_mut().enumerate() {
        let Some(widget) = value.as_table_mut() else {
            changed = true;
            continue;
        };
        let looks_legacy = (widget.contains_key("provider")
            || widget.contains_key("source")
            || widget.contains_key("metric")
            || widget.contains_key("limit_value"))
            && !widget.contains_key("indicators");
        if looks_legacy {
            convert_legacy_tray_widget(widget, index);
            changed = true;
        } else if widget.contains_key("source") {
            // Current schema has no widget-level `source`; drop leftovers so a
            // downgrade/upgrade ping-pong cannot keep confusing decoders.
            widget.remove("source");
            widget.remove("provider");
            widget.remove("limit_value");
            widget.remove("metric");
            changed = true;
        }
        healed.push(value.clone());
    }
    if changed {
        *widgets = healed;
    }
    changed
}

fn convert_legacy_tray_widget(widget: &mut toml::map::Map<String, toml::Value>, index: usize) {
    let provider = widget
        .remove("provider")
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "codex".into());
    let source = widget
        .remove("source")
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "combined".into());
    let limit_value = widget
        .remove("limit_value")
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "remaining".into());
    let color_mode = widget
        .remove("color_mode")
        .unwrap_or_else(|| toml::Value::String("status".into()));
    let fixed_color = widget.remove("fixed_color");
    widget.remove("metric");
    let primary = match provider.as_str() {
        "claude" => "claude.session",
        "cursor" => "cursor.auto",
        "openrouter" => "openrouter.limit",
        _ => "codex.session",
    };
    let secondary = match provider.as_str() {
        "claude" => "claude.weekly",
        "cursor" => "cursor.auto",
        "openrouter" => "openrouter.limit",
        _ => "codex.weekly",
    };
    let metric_ids: Vec<&str> = match source.as_str() {
        "primary" | "primary_reset" => vec![primary],
        "secondary" => vec![secondary],
        _ if primary == secondary => vec![primary],
        _ => vec![primary, secondary],
    };
    let indicators = metric_ids
        .into_iter()
        .map(|metric_id| {
            let mut indicator = toml::map::Map::new();
            indicator.insert("provider".into(), toml::Value::String(provider.clone()));
            indicator.insert("metric_id".into(), toml::Value::String(metric_id.into()));
            indicator.insert(
                "limit_value".into(),
                toml::Value::String(limit_value.clone()),
            );
            indicator.insert("color_mode".into(), color_mode.clone());
            if let Some(fixed_color) = fixed_color.clone() {
                indicator.insert("fixed_color".into(), fixed_color);
            }
            toml::Value::Table(indicator)
        })
        .collect();
    widget
        .entry("id")
        .or_insert_with(|| toml::Value::String(format!("legacy-tray-{index}")));
    widget
        .entry("kind")
        .or_insert_with(|| toml::Value::String("limits".into()));
    widget.insert("indicators".into(), toml::Value::Array(indicators));
    if !widget.contains_key("presentation") {
        let presentation = match source.as_str() {
            "primary_reset" => "reset_time",
            "primary" | "secondary" => "number",
            _ => "stacked_numbers",
        };
        widget.insert(
            "presentation".into(),
            toml::Value::String(presentation.into()),
        );
    }
}

pub(super) fn migrate(document: &mut toml::Value, mut version: u32) -> Result<()> {
    while version < SETTINGS_VERSION {
        match version {
            // Version 0 was the pre-versioned format. All its property names remain
            // compatible; serde defaults fill newly introduced notification/update fields.
            0 => {
                document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?
                    .insert("version".into(), toml::Value::Integer(1));
                version = 1;
            }
            1 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                if let Some(toml::Value::Array(widgets)) = root.get_mut("tray_widgets") {
                    for widget in widgets {
                        let Some(widget) = widget.as_table_mut() else {
                            continue;
                        };
                        let metric = widget
                            .remove("metric")
                            .and_then(|value| value.as_str().map(str::to_owned));
                        let (source, presentation) = match metric.as_deref() {
                            Some("primary_remaining") => ("primary", "number"),
                            Some("secondary_remaining") => ("secondary", "number"),
                            Some("primary_reset") => ("primary_reset", "reset_time"),
                            Some("secondary_reset") => ("primary_reset", "reset_time"),
                            _ => ("combined", "stacked_numbers"),
                        };
                        widget.insert("source".into(), toml::Value::String(source.into()));
                        widget.insert(
                            "presentation".into(),
                            toml::Value::String(presentation.into()),
                        );
                        widget.insert(
                            "limit_value".into(),
                            toml::Value::String("remaining".into()),
                        );
                    }
                }
                root.insert("version".into(), toml::Value::Integer(2));
                version = 2;
            }
            2 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                let notifications = root
                    .entry("notifications")
                    .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
                if let Some(table) = notifications.as_table_mut() {
                    table
                        .entry("update_available")
                        .or_insert(toml::Value::Boolean(true));
                }
                root.insert("version".into(), toml::Value::Integer(3));
                version = 3;
            }
            3 => {
                document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?
                    .entry("show_usage_stats")
                    .or_insert(toml::Value::Boolean(true));
                document
                    .as_table_mut()
                    .expect("settings root was checked above")
                    .insert("version".into(), toml::Value::Integer(4));
                version = 4;
            }
            4 => {
                // Usage activity is intentionally a compact recent view. This
                // setting was previously informational-only, so migrate the
                // former default rather than preserving an inaccessible 90-day
                // value.
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.insert("history_retention_days".into(), toml::Value::Integer(30));
                root.insert("version".into(), toml::Value::Integer(5));
                version = 5;
            }
            5 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("show_banked_resets")
                    .or_insert(toml::Value::Boolean(true));
                root.insert("version".into(), toml::Value::Integer(6));
                version = 6;
            }
            6 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("provider")
                    .or_insert(toml::Value::String("codex".into()));
                root.insert("version".into(), toml::Value::Integer(7));
                version = 7;
            }
            7 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                let selected = root
                    .remove("provider")
                    .and_then(|value| value.as_str().map(str::to_owned));
                let claude_enabled = selected.as_deref() == Some("claude");
                let mut providers = toml::map::Map::new();
                providers.insert(
                    "codex_enabled".into(),
                    toml::Value::Boolean(!claude_enabled),
                );
                providers.insert(
                    "claude_enabled".into(),
                    toml::Value::Boolean(claude_enabled),
                );
                root.insert("providers".into(), toml::Value::Table(providers));
                root.insert("version".into(), toml::Value::Integer(8));
                version = 8;
            }
            8 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("limit_refresh_interval")
                    .or_insert(toml::Value::String("minute1".into()));
                root.insert("version".into(), toml::Value::Integer(9));
                version = 9;
            }
            9 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("show_account_name")
                    .or_insert(toml::Value::Boolean(false));
                root.insert("version".into(), toml::Value::Integer(10));
                version = 10;
            }
            10 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                let providers = root
                    .entry("providers")
                    .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
                if let Some(providers) = providers.as_table_mut() {
                    providers
                        .entry("cursor_enabled")
                        .or_insert(toml::Value::Boolean(false));
                }
                root.insert("version".into(), toml::Value::Integer(11));
                version = 11;
            }
            11 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.remove("hide_plan_credits");
                root.insert("version".into(), toml::Value::Integer(12));
                version = 12;
            }
            12 => {
                // Do not surprise existing users with onboarding after an
                // update. Only settings files created by this version start
                // with onboarding incomplete.
                document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?
                    .insert("onboarding_completed".into(), toml::Value::Boolean(true));
                document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?
                    .insert("version".into(), toml::Value::Integer(13));
                version = 13;
            }
            13 => {
                // The Home tab previously had no usage summary at all, so
                // preserve the new feature's default when existing settings
                // files are upgraded.
                document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?
                    .entry("show_combined_usage_on_all_tab")
                    .or_insert(toml::Value::Boolean(true));
                document
                    .as_table_mut()
                    .expect("settings root was checked above")
                    .insert("version".into(), toml::Value::Integer(14));
                version = 14;
            }
            14 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                let previous = root
                    .remove("show_combined_usage_on_all_tab")
                    .unwrap_or(toml::Value::Boolean(true));
                root.entry("show_total_spend_on_all_tab")
                    .or_insert(previous);
                root.insert("version".into(), toml::Value::Integer(15));
                version = 15;
            }
            15 => {
                document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?
                    .entry("total_spend_presentation")
                    .or_insert(toml::Value::String("donut".into()));
                document
                    .as_table_mut()
                    .expect("settings root was checked above")
                    .insert("version".into(), toml::Value::Integer(16));
                version = 16;
            }
            16 => {
                // Preserve the historical Codex → Claude → Cursor presentation
                // order for existing installs when provider reordering lands.
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("provider_order").or_insert_with(|| {
                    toml::Value::Array(
                        MIGRATED_DRIVERS
                            .iter()
                            .map(|provider| toml::Value::String(provider.id().into()))
                            .collect(),
                    )
                });
                root.insert("version".into(), toml::Value::Integer(17));
                version = 17;
            }
            17 => {
                // Promote provider_order into popup_order and pin Usage Stats
                // above the historical provider stack.
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                let providers = root
                    .remove("provider_order")
                    .and_then(|value| value.as_array().cloned())
                    .unwrap_or_else(|| {
                        MIGRATED_DRIVERS
                            .iter()
                            .map(|provider| toml::Value::String(provider.id().into()))
                            .collect()
                    });
                let mut popup_order = vec![toml::Value::String("total_spend".into())];
                for provider in providers {
                    if let Some(id) = provider.as_str()
                        && matches!(id, "codex" | "claude" | "cursor" | "openrouter")
                    {
                        popup_order.push(toml::Value::String(id.into()));
                    }
                }
                for provider in MIGRATED_DRIVERS {
                    // Home widget ids of that era.
                    let id = match provider {
                        ProviderKind::OpenCodeZen => "open_code_zen",
                        ProviderKind::OpenCodeGo => "open_code_go",
                        other => other.id(),
                    };
                    if !popup_order.iter().any(|value| value.as_str() == Some(id)) {
                        popup_order.push(toml::Value::String(id.into()));
                    }
                }
                root.insert("popup_order".into(), toml::Value::Array(popup_order));
                root.insert("version".into(), toml::Value::Integer(18));
                version = 18;
            }
            18 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("theme")
                    .or_insert(toml::Value::String("auto".into()));
                root.entry("accent_color")
                    .or_insert(toml::Value::String("windows".into()));
                root.entry("animations_enabled")
                    .or_insert(toml::Value::Boolean(true));
                root.insert("version".into(), toml::Value::Integer(19));
                version = 19;
            }
            19 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                let provider_flags = root
                    .get("providers")
                    .and_then(toml::Value::as_table)
                    .cloned()
                    .unwrap_or_default();
                let popup_order = root
                    .get("popup_order")
                    .and_then(toml::Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let mut enabled = Vec::new();
                for value in popup_order {
                    let Some(id) = value.as_str() else {
                        continue;
                    };
                    let flag = format!("{id}_enabled");
                    if provider_flags
                        .get(&flag)
                        .and_then(toml::Value::as_bool)
                        .unwrap_or(false)
                    {
                        enabled.push(toml::Value::String(id.into()));
                    }
                }
                for provider in MIGRATED_DRIVERS {
                    let id = provider.id();
                    let flag = format!("{id}_enabled");
                    if provider_flags
                        .get(&flag)
                        .and_then(toml::Value::as_bool)
                        .unwrap_or(false)
                        && !enabled.iter().any(|value| value.as_str() == Some(id))
                    {
                        enabled.push(toml::Value::String(id.into()));
                    }
                }
                let mut providers = toml::map::Map::new();
                providers.insert("enabled".into(), toml::Value::Array(enabled));
                root.insert("providers".into(), toml::Value::Table(providers));
                if let Some(toml::Value::Array(widgets)) = root.get_mut("tray_widgets") {
                    for (index, value) in widgets.iter_mut().enumerate() {
                        let Some(widget) = value.as_table_mut() else {
                            continue;
                        };
                        if widget.contains_key("indicators") {
                            continue;
                        }
                        convert_legacy_tray_widget(widget, index);
                    }
                }
                root.insert("version".into(), toml::Value::Integer(20));
                version = 20;
            }
            20 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                if let Some(toml::Value::Array(widgets)) = root.get_mut("tray_widgets") {
                    for value in widgets.iter_mut() {
                        let Some(widget) = value.as_table_mut() else {
                            continue;
                        };
                        let color_mode = widget
                            .remove("color_mode")
                            .unwrap_or_else(|| toml::Value::String("status".into()));
                        let fixed_color = widget.remove("fixed_color");
                        let Some(toml::Value::Array(indicators)) = widget.get_mut("indicators")
                        else {
                            continue;
                        };
                        for indicator_value in indicators.iter_mut() {
                            let Some(indicator) = indicator_value.as_table_mut() else {
                                continue;
                            };
                            indicator.entry("color_mode").or_insert(color_mode.clone());
                            if let Some(fixed_color) = fixed_color.clone() {
                                indicator.entry("fixed_color").or_insert(fixed_color);
                            }
                        }
                    }
                }
                root.insert("version".into(), toml::Value::Integer(21));
                version = 21;
            }
            21 => {
                // Preserve the historical Thirty Days default for existing installs.
                document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?
                    .entry("total_spend_period")
                    .or_insert(toml::Value::String("thirty_days".into()));
                document
                    .as_table_mut()
                    .expect("settings root was checked above")
                    .insert("version".into(), toml::Value::Integer(22));
                version = 22;
            }
            22 => {
                // `codex_path` remains optional: absent means automatic
                // discovery, which preserves existing installations exactly.
                document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?
                    .insert("version".into(), toml::Value::Integer(23));
                version = 23;
            }
            23 => {
                // The new provider paths are optional. Missing values retain
                // automatic discovery for Claude and Cursor.
                document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?
                    .insert("version".into(), toml::Value::Integer(24));
                version = 24;
            }
            24 | 25 => {
                // No released build wrote these versions. Step through them
                // so the instance migration still runs for older files.
                document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?
                    .insert("version".into(), toml::Value::Integer(26));
                version = 26;
            }
            26 => {
                // Manual OpenCode keys are stored outside settings. These
                // non-secret revisions only wake already-running workers.
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("opencode_zen_credentials_revision")
                    .or_insert(toml::Value::Integer(0));
                root.entry("opencode_go_credentials_revision")
                    .or_insert(toml::Value::Integer(0));
                root.entry("openrouter_credentials_revision")
                    .or_insert(toml::Value::Integer(0));
                root.insert("version".into(), toml::Value::Integer(27));
                version = 27;
            }
            27 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("openrouter_accounts")
                    .or_insert_with(|| toml::Value::Array(Vec::new()));
                root.insert("version".into(), toml::Value::Integer(28));
                version = 28;
            }
            28 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                let show_banked_resets = root
                    .get("show_banked_resets")
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(true);
                let show_usage_stats = root
                    .get("show_usage_stats")
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(true);
                let mut bricks = toml::map::Map::new();
                for provider in MIGRATED_DRIVERS {
                    for brick_id in crate::providers::registry::catalog_brick_ids(provider) {
                        let (all_tab, provider_tab) = if brick_id.ends_with(".usage") {
                            (false, show_usage_stats)
                        } else if brick_id.ends_with(".resets") {
                            (show_banked_resets, show_banked_resets)
                        } else {
                            let default = PopupVisibility::default_brick_visibility(&brick_id);
                            (default.all_tab, default.provider_tab)
                        };
                        let mut entry = toml::map::Map::new();
                        entry.insert("all_tab".into(), toml::Value::Boolean(all_tab));
                        entry.insert("provider_tab".into(), toml::Value::Boolean(provider_tab));
                        bricks.insert(brick_id, toml::Value::Table(entry));
                    }
                }
                let mut popup_visibility = toml::map::Map::new();
                popup_visibility.insert("bricks".into(), toml::Value::Table(bricks));
                root.insert(
                    "popup_visibility".into(),
                    toml::Value::Table(popup_visibility),
                );
                root.remove("show_banked_resets");
                root.remove("show_usage_stats");
                root.insert("version".into(), toml::Value::Integer(29));
                version = 29;
            }
            29 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                if !root.contains_key("time_format") {
                    root.insert(
                        "time_format".into(),
                        toml::Value::String(TimeFormat::from_windows().as_str().into()),
                    );
                }
                root.insert("version".into(), toml::Value::Integer(30));
                version = 30;
            }
            30 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("auto_activation_pauses")
                    .or_insert_with(|| toml::Value::Array(Vec::new()));
                root.insert("version".into(), toml::Value::Integer(31));
                version = 31;
            }
            31 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("bottom_bar_size")
                    .or_insert_with(|| toml::Value::String("comfortable".into()));
                root.entry("popup_corner_radius")
                    .or_insert_with(|| toml::Value::String("small".into()));
                root.insert("version".into(), toml::Value::Integer(32));
                version = 32;
            }
            32 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("popup_background_material")
                    .or_insert_with(|| toml::Value::String("acrylic".into()));
                root.insert("version".into(), toml::Value::Integer(33));
                version = 33;
            }
            33 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("usage_refresh_interval")
                    .or_insert_with(|| toml::Value::String("minutes15".into()));
                root.insert("version".into(), toml::Value::Integer(34));
                version = 34;
            }
            34 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("usage_stats_enabled")
                    .or_insert(toml::Value::Boolean(true));
                root.insert("version".into(), toml::Value::Integer(35));
                version = 35;
            }
            35 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("reset_announcement_refresh_interval")
                    .or_insert_with(|| toml::Value::String("hour1".into()));
                let notifications = root
                    .entry("notifications")
                    .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
                if let Some(table) = notifications.as_table_mut() {
                    table
                        .entry("forced_reset_feed_enabled")
                        .or_insert(toml::Value::Boolean(true));
                    table
                        .entry("forced_reset_notifications")
                        .or_insert(toml::Value::Boolean(true));
                }
                root.insert("version".into(), toml::Value::Integer(36));
                version = 36;
            }
            36 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("usage_stats_excluded_providers")
                    .or_insert_with(|| toml::Value::Array(Vec::new()));
                root.insert("version".into(), toml::Value::Integer(37));
                version = 37;
            }
            37 => {
                // Antigravity and Grok CLI folders are optional. Missing values
                // retain automatic discovery for existing installations.
                document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?
                    .insert("version".into(), toml::Value::Integer(38));
                version = 38;
            }
            38 => {
                migrate_to_instances(document)?;
                version = INSTANCES_VERSION;
            }
            39 => {
                // The global legacy-cards switch became the Classic layout of
                // each Home provider widget that still used ordinary cards.
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                let legacy_cards = root
                    .remove("compact_usage_cards")
                    .and_then(|value| value.as_bool())
                    == Some(false);
                if legacy_cards {
                    let ids = root
                        .get("instances")
                        .and_then(toml::Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(|instance| instance.get("id")?.as_str().map(str::to_owned))
                        .collect::<Vec<_>>();
                    let layouts = root
                        .entry("popup_home_card_layouts")
                        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
                    if let Some(layouts) = layouts.as_table_mut() {
                        for id in ids {
                            layouts
                                .entry(id)
                                .or_insert_with(|| toml::Value::String("classic".into()));
                        }
                    }
                }
                root.insert("version".into(), toml::Value::Integer(40));
                version = 40;
            }
            40 => {
                // The taskbar widget is switched on after decoding, where its
                // default widget can be built from the enabled providers.
                document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?
                    .insert("version".into(), toml::Value::Integer(41));
                version = 41;
            }
            // Unknown future/gap versions: stamp current and keep decoding with
            // serde defaults rather than refusing to start.
            _ => {
                document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?
                    .insert(
                        "version".into(),
                        toml::Value::Integer(i64::from(SETTINGS_VERSION)),
                    );
                version = SETTINGS_VERSION;
            }
        }
    }
    Ok(())
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
        assert_eq!(ids.len(), MIGRATED_DRIVERS.len() + 2);
        let claude = settings.instance_by_id("claude").unwrap();
        assert_eq!(claude.name, "Personal");
        assert!(claude.enabled && claude.auto_activation && claude.show_on_home);
        assert_eq!(
            claude.binary_path,
            Some(PathBuf::from(r"C:\tools\claude.exe"))
        );
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
        let changed = finish_instance_migration(
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
        let mut forgotten = forgotten.take();
        forgotten.sort();
        let mut expected = [claude_secret(&session_id), codex_secret(&codex_id)];
        expected.sort();
        assert_eq!(forgotten, expected);
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
