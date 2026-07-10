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
            start_at_login: false,
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
        let settings: Self = toml::from_str(&raw).context("parse settings TOML")?;
        anyhow::ensure!(
            settings.version <= SETTINGS_VERSION,
            "settings were created by a newer Codex Minibar"
        );
        Ok(settings)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_product_decisions() {
        let value = Settings::default();
        assert!(value.automatic_activation);
        assert!(!value.start_at_login);
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
}
