use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

pub const SETTINGS_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayMetric {
    PrimaryRemaining,
    PrimaryReset,
    SecondaryRemaining,
    SecondaryReset,
    Combined,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayWidget {
    pub metric: TrayMetric,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationSettings {
    pub activation_success: bool,
    pub activation_failure: bool,
    pub codex_unavailable: bool,
    pub approaching_reset: bool,
    pub limits_changed: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            activation_success: false,
            activation_failure: true,
            codex_unavailable: true,
            approaching_reset: false,
            limits_changed: false,
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
            tray_widgets: vec![
                TrayWidget {
                    metric: TrayMetric::PrimaryRemaining,
                },
                TrayWidget {
                    metric: TrayMetric::PrimaryReset,
                },
            ],
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
        anyhow::ensure!(
            original_version <= i64::from(SETTINGS_VERSION),
            "settings were created by a newer Codex Minibar"
        );
        migrate(&mut document, original_version as u32)?;
        let settings: Self = document.try_into().context("decode migrated settings")?;
        settings.validate()?;
        if original_version != i64::from(SETTINGS_VERSION) {
            settings.save(path)?;
        }
        Ok(settings)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        anyhow::ensure!(
            self.version == SETTINGS_VERSION,
            "refusing to save unsupported settings version {}",
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
        Ok(())
    }

    /// Applies settings whose effect lives outside the render tree.
    pub fn apply_runtime_effects(&self) -> Result<()> {
        apply_startup_registration(self.start_at_login)
    }
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
    anyhow::ensure!(
        result == ERROR_SUCCESS,
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
        assert_eq!(value.tray_widgets.len(), 2);
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
        assert!(fs::read_to_string(path).unwrap().contains("version = 1"));
    }

    #[test]
    fn rejects_newer_settings_versions() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        fs::write(&path, "version = 999\n").unwrap();
        assert!(Settings::load_or_create(&path).is_err());
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
