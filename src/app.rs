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

/// Starts the popup's GPUI thread and the tray bridge.
pub fn start(state: Arc<AppState>) {
    if crate::popup_window::start(Arc::clone(&state)) {
        bridge::start(state);
    }
}
