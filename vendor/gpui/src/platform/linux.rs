mod dispatcher;
mod headless;
mod keyboard;
mod platform;
#[cfg(any(feature = "wayland", feature = "x11"))]
mod text_system;
#[cfg(feature = "wayland")]
mod wayland;
#[cfg(feature = "x11")]
mod x11;

#[cfg(any(feature = "wayland", feature = "x11"))]
mod xdg_desktop_portal;

pub(crate) use dispatcher::*;
pub(crate) use headless::*;
pub(crate) use keyboard::*;
pub(crate) use platform::*;
#[cfg(any(feature = "wayland", feature = "x11"))]
pub(crate) use text_system::*;
#[cfg(feature = "wayland")]
pub(crate) use wayland::*;
#[cfg(feature = "x11")]
pub(crate) use x11::*;

#[cfg(all(feature = "screen-capture", any(feature = "wayland", feature = "x11")))]
pub(crate) type PlatformScreenCaptureFrame = scap::frame::Frame;
#[cfg(not(all(feature = "screen-capture", any(feature = "wayland", feature = "x11"))))]
pub(crate) type PlatformScreenCaptureFrame = ();

// codex-minibar patch: opaque X11 windows for software Vulkan drivers that
// present with PutImage.
static X11_OPAQUE_WITHOUT_SHM_PRESENT: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Create X11 windows with the screen's opaque (24-bit) visual instead of an
/// ARGB one when the X server cannot take presented images as shared-memory
/// pixmaps (it lacks DRI3, Present or MIT-SHM, as Xvfb and many remote or
/// virtual X servers do).
///
/// Mesa's software Vulkan driver (lavapipe) before 24.1 then presents every
/// frame with a PutImage request at a hard-coded depth of 24, which a 32-bit
/// window rejects, so the window never shows anything. Enable this before
/// opening windows when rendering with such a driver; affected windows lose
/// per-pixel transparency, which those servers rarely composite anyway.
pub fn set_x11_opaque_without_shm_present(enabled: bool) {
    X11_OPAQUE_WITHOUT_SHM_PRESENT.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(feature = "x11")]
pub(crate) fn x11_opaque_without_shm_present() -> bool {
    X11_OPAQUE_WITHOUT_SHM_PRESENT.load(std::sync::atomic::Ordering::Relaxed)
}
