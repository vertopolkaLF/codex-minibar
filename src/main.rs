#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use anyhow::Result;
use codex_minibar::settings::Settings;

fn main() -> Result<()> {
    let path = Settings::default_path()?;
    let _settings = Settings::load_or_create(&path)?;
    Ok(())
}
