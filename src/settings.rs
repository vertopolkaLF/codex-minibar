use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

pub const SETTINGS_VERSION: u32 = 2;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraySource {
    Combined,
    Primary,
    Secondary,
    PrimaryReset,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayPresentation {
    StackedNumbers,
    StackedBars,
    NestedRings,
    Number,
    Bar,
    Ring,
    ResetTime,
    ResetCountdown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitValue {
    #[default]
    Remaining,
    Used,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayWidget {
    pub source: TraySource,
    pub presentation: TrayPresentation,
    #[serde(default)]
    pub limit_value: LimitValue,
}

impl TrayWidget {
    pub fn default_user_widget() -> Self {
        Self {
            source: TraySource::Combined,
            presentation: TrayPresentation::StackedNumbers,
            limit_value: LimitValue::Remaining,
        }
    }

    pub fn uses_limit_value(&self) -> bool {
        !matches!(self.source, TraySource::PrimaryReset)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationSettings {
    pub activation_success: bool,
    pub activation_failure: bool,
    pub codex_unavailable: bool,
    pub approaching_reset: bool,
    /// Notify when a rate-limit window resets (`resets_at` changes).
    pub limits_changed: bool,
    /// Notify when remaining usage drops to [`Self::low_usage_threshold_percent`].
    pub low_usage_enabled: bool,
    /// Remaining-percent threshold for low-usage notifications (1–99).
    pub low_usage_threshold_percent: u8,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            activation_success: false,
            activation_failure: true,
            codex_unavailable: true,
            approaching_reset: false,
            limits_changed: false,
            low_usage_enabled: false,
            low_usage_threshold_percent: 20,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub automatic_activation: bool,
    pub start_at_login: bool,
    pub show_used_percentage: bool,
    pub hide_plan_credits: bool,
    pub codex_path: Option<PathBuf>,
    pub tray_widgets: Vec<TrayWidget>,
    pub notifications: NotificationSettings,
    pub history_retention_days: u16,
    pub check_for_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            automatic_activation: true,
            start_at_login: true,
            show_used_percentage: false,
            hide_plan_credits: false,
            codex_path: None,
            // An empty list intentionally means "show the ordinary app icon".
            tray_widgets: Vec::new(),
            notifications: NotificationSettings::default(),
            history_retention_days: 90,
            check_for_updates: true,
        }
    }
}

impl Settings {
    pub fn default_path() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("dev", "Codex Minibar", "Codex Minibar")
            .context("could not resolve the application config directory")?;
        Ok(dirs.config_dir().join("settings.toml"))
    }

    pub fn load_or_create(path: &Path) -> Result<Self> {
        if !path.exists() {
            let settings = Self::default();
            settings.save(path)?;
            return Ok(settings);
        }
        let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let mut document: toml::Value = toml::from_str(&raw).context("parse settings TOML")?;
        let original_version = document
            .get("version")
            .and_then(toml::Value::as_integer)
            .unwrap_or(0);
        // A settings file must never prevent the application from starting.
        // Serde intentionally ignores unknown fields, so a newer file can still
        // supply every option this build understands. Only migrate older files.
        let original_version = u32::try_from(original_version).unwrap_or(u32::MAX);
        if original_version < SETTINGS_VERSION {
            migrate(&mut document, original_version)?;
        }
        let settings: Self = document.try_into().context("decode migrated settings")?;
        settings.validate()?;
        if original_version < SETTINGS_VERSION {
            settings.save(path)?;
        }
        Ok(settings)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        anyhow::ensure!(
            self.version >= SETTINGS_VERSION,
            "refusing to save obsolete settings version {}",
            self.version
        );
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        let parent = path
            .parent()
            .context("settings path has no parent directory")?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .with_context(|| format!("create temporary settings in {}", parent.display()))?;
        use std::io::Write;
        temporary
            .write_all(toml::to_string_pretty(self)?.as_bytes())
            .context("write temporary settings")?;
        temporary
            .as_file()
            .sync_all()
            .context("flush temporary settings")?;
        temporary
            .persist(path)
            .with_context(|| format!("commit {}", path.display()))?;
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            (1..=365).contains(&self.history_retention_days),
            "history retention must be between 1 and 365 days"
        );
        anyhow::ensure!(
            (1..=99).contains(&self.notifications.low_usage_threshold_percent),
            "low usage threshold must be between 1 and 99 percent"
        );
        Ok(())
    }

    /// Applies settings whose effect lives outside the render tree.
    pub fn apply_runtime_effects(&self) -> Result<()> {
        apply_startup_registration(self.start_at_login)
    }

    /// If the installer (or another tool) registered us in HKCU Run while
    /// settings still say off, adopt that into settings before we apply them —
    /// otherwise `apply_runtime_effects` would delete the Run value on launch.
    pub fn reconcile_startup_from_registry(&mut self, path: &Path) -> Result<()> {
        if self.start_at_login || !startup_registration_present()? {
            return Ok(());
        }
        self.start_at_login = true;
        self.save(path)
    }
}

#[cfg(windows)]
fn startup_registration_present() -> Result<bool> {
    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_READ, RegCloseKey, RegGetValueW, RegOpenKeyExW, RRF_RT_REG_SZ,
        },
    };

    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let value_name: Vec<u16> = "Codex Minibar"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut key: HKEY = std::ptr::null_mut();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            KEY_READ,
            &mut key,
        )
    };
    if status != ERROR_SUCCESS {
        return Ok(false);
    }
    let mut data_size = 0u32;
    let result = unsafe {
        RegGetValueW(
            key,
            std::ptr::null(),
            value_name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut data_size,
        )
    };
    unsafe { RegCloseKey(key) };
    Ok(result == ERROR_SUCCESS)
}

#[cfg(not(windows))]
fn startup_registration_present() -> Result<bool> {
    Ok(false)
}

#[cfg(windows)]
fn apply_startup_registration(enabled: bool) -> Result<()> {
    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
            RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegSetValueExW,
        },
    };

    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let value_name: Vec<u16> = "Codex Minibar"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut key: HKEY = std::ptr::null_mut();
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            std::ptr::null_mut(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        )
    };
    anyhow::ensure!(status == ERROR_SUCCESS, "open Windows startup registry key: {status}");

    let result = if enabled {
        let executable = std::env::current_exe().context("resolve current executable for startup")?;
        let command = format!("\"{}\"", executable.display());
        let data: Vec<u16> = command
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            RegSetValueExW(
                key,
                value_name.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * size_of::<u16>()) as u32,
            )
        }
    } else {
        unsafe { RegDeleteValueW(key, value_name.as_ptr()) }
    };
    unsafe { RegCloseKey(key) };
    // Deleting an already-absent Run value is success: the desired end state is
    // "not registered", whether we removed it now or it was never there.
    const ERROR_FILE_NOT_FOUND: u32 = 2;
    anyhow::ensure!(
        result == ERROR_SUCCESS || (!enabled && result == ERROR_FILE_NOT_FOUND),
        "update Windows startup registration: {result}"
    );
    Ok(())
}

#[cfg(not(windows))]
fn apply_startup_registration(_enabled: bool) -> Result<()> {
    Ok(())
}

fn migrate(document: &mut toml::Value, mut version: u32) -> Result<()> {
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
                        let metric = widget.remove("metric").and_then(|value| value.as_str().map(str::to_owned));
                        let (source, presentation) = match metric.as_deref() {
                            Some("primary_remaining") => ("primary", "number"),
                            Some("secondary_remaining") => ("secondary", "number"),
                            Some("primary_reset") => ("primary_reset", "reset_time"),
                            Some("secondary_reset") => ("primary_reset", "reset_time"),
                            Some("combined") | _ => ("combined", "stacked_numbers"),
                        };
                        widget.insert("source".into(), toml::Value::String(source.into()));
                        widget.insert("presentation".into(), toml::Value::String(presentation.into()));
                        widget.insert("limit_value".into(), toml::Value::String("remaining".into()));
                    }
                }
                root.insert("version".into(), toml::Value::Integer(2));
                version = 2;
            }
            unsupported => anyhow::bail!("no migration path from settings version {unsupported}"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_product_decisions() {
        let value = Settings::default();
        assert!(value.automatic_activation);
        assert!(value.start_at_login);
        assert!(!value.show_used_percentage);
        assert!(!value.hide_plan_credits);
        assert_eq!(value.history_retention_days, 90);
        assert!(value.tray_widgets.is_empty());
        assert!(!value.notifications.limits_changed);
        assert!(!value.notifications.low_usage_enabled);
        assert_eq!(value.notifications.low_usage_threshold_percent, 20);
    }

    #[test]
    fn round_trips_through_disk() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        let expected = Settings::default();
        expected.save(&path).unwrap();
        assert_eq!(Settings::load_or_create(&path).unwrap(), expected);
    }

    #[test]
    fn migrates_pre_versioned_settings_and_rewrites_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        fs::write(
            &path,
            r#"
automatic_activation = false
start_at_login = true
history_retention_days = 30
check_for_updates = false
tray_widgets = []
"#,
        )
        .unwrap();

        let migrated = Settings::load_or_create(&path).unwrap();
        assert_eq!(migrated.version, SETTINGS_VERSION);
        assert!(!migrated.automatic_activation);
        assert!(migrated.start_at_login);
        assert_eq!(migrated.history_retention_days, 30);
        assert!(fs::read_to_string(path).unwrap().contains("version = 2"));
    }

    #[test]
    fn accepts_newer_settings_versions_and_ignores_unknown_options() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        fs::write(
            &path,
            "version = 999\nfuture_option = true\nhistory_retention_days = 30\n",
        )
        .unwrap();
        let settings = Settings::load_or_create(&path).unwrap();
        assert_eq!(settings.version, 999);
        assert_eq!(settings.history_retention_days, 30);
    }

    #[test]
    fn validates_retention_range() {
        let settings = Settings {
            history_retention_days: 0,
            ..Settings::default()
        };
        assert!(settings.validate().is_err());
    }
}
