//! The app runtime behind every surface: worker lifecycle, the live quota
//! store, the tray bridge thread and actions that change persisted state.
//! The popup, Settings and the shell surfaces only render what lives here.

pub mod actions;
mod auto_reset;
mod bridge;
mod limits_persist;
mod state;

use std::sync::Arc;

pub use state::AppState;

/// Routes notifications to the popup's toast host and their buttons to the
/// app.
pub fn install_notification_host() {
    use crate::notifications::{Host, NotificationAction, install_host};
    use crate::popup_window::{PopupCommand, send_command};
    install_host(Host {
        deliver: |notification| send_command(PopupCommand::Toast(Box::new(notification))),
        user_engaged: || crate::popup::is_visible() || crate::settings_window::is_open(),
        game_mode_active: crate::game_mode::is_active,
        run_action: |action| match action {
            NotificationAction::InstallUpdate => {
                // Failures are reported on the update card.
                let _ = crate::updater::apply_pending_update();
            }
            NotificationAction::OpenUrl(url) => {
                if let Err(error) = crate::shell::open_url(&url) {
                    crate::logger::info(format!("failed to open {url}: {error:#}"));
                }
            }
            NotificationAction::IgnoreGame(path) => crate::game_mode::ignore_game(path),
            NotificationAction::AutoReset(choice) => crate::banked_reset::choose(choice),
        },
    });
}

/// Starts the popup's GPUI thread and the tray bridge.
pub fn start(state: Arc<AppState>) {
    if crate::popup_window::start(Arc::clone(&state)) {
        bridge::start(state);
    }
}
