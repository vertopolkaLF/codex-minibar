//! Linux desktop integration: the freedesktop counterparts of the Win32
//! services the Windows build uses. Everything here talks to the session
//! D-Bus (StatusNotifierItem tray, notifications, XDG portal, Secret Service)
//! or to XDG base directories, so it works on GNOME, KDE and other
//! freedesktop-compliant desktops under both X11 and Wayland.

pub mod appimage_launch;
pub mod apps;
pub mod autostart;
pub(crate) mod command;
pub mod dbus;
pub mod desktop;
pub mod identity;
pub mod instance;
pub mod keybinding;
pub mod keyring;
pub(crate) mod login_child;
pub(crate) mod node_discovery;
pub mod notify;
pub(crate) mod provider_env;
pub(crate) mod resume;
pub mod update;

pub mod wake;

use std::io::Read;

/// Fills `buffer` from the kernel CSPRNG.
pub fn random_bytes(buffer: &mut [u8]) -> std::io::Result<()> {
    std::fs::File::open("/dev/urandom")?.read_exact(buffer)
}
