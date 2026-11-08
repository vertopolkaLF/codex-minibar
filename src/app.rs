//! The app runtime behind every surface: worker lifecycle, the live quota
//! store, the tray bridge thread and actions that change persisted state.
//! The popup, Settings and the shell surfaces only render what lives here.

pub mod event;
pub mod game_mode;
pub mod hotkey;
pub mod ids;
pub mod single_instance;
pub mod troubleshoot;

pub mod actions;
mod auto_reset;
mod bridge;
mod limits_persist;
mod state;

use std::sync::Arc;

pub use state::AppState;

/// Routes notifications to the popup's toast host (the desktop notification
/// server on Linux) and their buttons to the app.
pub fn install_notification_host() {
    use crate::surfaces::notifications::{Host, NotificationAction, install_host};
    install_host(Host {
        #[cfg(windows)]
        deliver: |notification| {
            use crate::ui::popup_window::{PopupCommand, send_command};
            send_command(PopupCommand::Toast(Box::new(notification)))
        },
        // Linux windows cannot be click-through or (on Wayland) placed in a
        // corner, so cards go to the desktop's notification server instead.
        #[cfg(not(windows))]
        deliver: |notification| {
            let silent = crate::surfaces::notifications::sounds_muted();
            crate::platform::linux::notify::post(notification, silent);
        },
        user_engaged: || crate::ui::popup::is_visible() || crate::ui::settings_window::is_open(),
        game_mode_active: crate::app::game_mode::is_active,
        run_action: |action| match action {
            NotificationAction::InstallUpdate => {
                // Failures are reported on the update card.
                let _ = crate::update::apply_pending_update();
            }
            NotificationAction::OpenUrl(url) => {
                if let Err(error) = crate::platform::shell::open_url(&url) {
                    crate::platform::logger::info(format!("failed to open {url}: {error:#}"));
                }
            }
            NotificationAction::IgnoreGame(path) => crate::app::game_mode::ignore_game(path),
            NotificationAction::AutoReset(choice) => crate::quota::banked_reset::choose(choice),
        },
    });
}

/// Starts the popup's GPUI thread and the tray bridge.
pub fn start(state: Arc<AppState>) {
    if crate::ui::popup_window::start(Arc::clone(&state)) {
        bridge::start(state);
    }
}
