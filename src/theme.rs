//! Shared animation tokens for both WinUI surfaces.

use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

static APP_ANIMATIONS_ENABLED: AtomicBool = AtomicBool::new(true);

/// WinUI `ControlFasterAnimationDuration` — pointer-over / micro-interactions.
pub const CONTROL_FASTER_ANIMATION: Duration = Duration::from_millis(83);
/// WinUI `ControlFastAnimationDuration`.
pub const CONTROL_FAST_ANIMATION: Duration = Duration::from_millis(167);
/// WinUI `ControlNormalAnimationDuration`.
pub const CONTROL_NORMAL_ANIMATION: Duration = Duration::from_millis(250);

pub fn set_animations_enabled(enabled: bool) {
    APP_ANIMATIONS_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn animations_enabled() -> bool {
    APP_ANIMATIONS_ENABLED.load(Ordering::Relaxed) && crate::popup::system_animations_enabled()
}

pub fn duration(duration: Duration) -> Duration {
    if animations_enabled() {
        duration
    } else {
        Duration::ZERO
    }
}

pub fn apply_appearance(theme: crate::settings::AppTheme, accent: crate::settings::AccentColor) {
    let requested = match theme {
        crate::settings::AppTheme::Auto => windows_reactor::RequestedTheme::Default,
        crate::settings::AppTheme::Light => windows_reactor::RequestedTheme::Light,
        crate::settings::AppTheme::Dark => windows_reactor::RequestedTheme::Dark,
    };
    windows_reactor::set_requested_theme(requested);
    if let Err(error) = windows_reactor::set_accent_color(accent.rgb()) {
        eprintln!("failed to apply accent color: {error:?}");
    }
}
