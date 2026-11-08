//! Identifiers a second launch uses to find and wake the running instance.

/// Title of the popup host window.
pub const POPUP_TITLE: &str = "Codex Minibar";
/// Posted to the popup host to show it (`WM_APP + 0x034D`).
pub const SHOW_MESSAGE: u32 = 0x8000 + 0x034D;
/// Catalog key of the Settings window title, which is localized.
pub const SETTINGS_TITLE_KEY: &str = "codex-minibar-settings";
