//! Win32, shell, network, paths, secrets, logging and local time helpers.

#[cfg(not(windows))]
pub mod linux;
pub mod local_time;
pub mod logger;
pub mod net;
pub mod paths;
pub mod secrets;
pub mod shell;
pub mod win32;
