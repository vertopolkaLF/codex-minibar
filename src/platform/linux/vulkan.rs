//! Software Vulkan fallback for the AppImage.
//!
//! GPUI renders through blade, which needs a Vulkan device with a few
//! extensions; without one GPUI cannot start at all. Virtual machines, hosts
//! without Vulkan drivers and headless test machines (the AppImage catalog's
//! Xvfb) have none. The AppImage therefore carries Mesa's lavapipe, a CPU
//! Vulkan driver, plus a Vulkan loader (`build-linux.sh`). At startup the host
//! is probed the way GPUI will use it, and only when that fails is the loader
//! pointed at lavapipe. Distribution packages carry no bundle, so this is a
//! no-op for them.

use std::{
    ffi::{CString, OsStr},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    sync::OnceLock,
};

/// Directory of the bundle, relative to the executable's directory.
const BUNDLE_DIR: &str = "../lib/lavapipe";
const MANIFEST: &str = "lvp_icd.json";
const LOADER: &str = "libvulkan.so.1";
/// Driver manifest variables: the current name and the one older loaders read.
const DRIVER_VARIABLES: [&str; 2] = ["VK_DRIVER_FILES", "VK_ICD_FILENAMES"];

static OUTCOME: OnceLock<String> = OnceLock::new();

/// Points the Vulkan loader at the bundled software driver when the host has
/// no device GPUI can render with.
///
/// # Safety
/// Must run before any other thread can read or write the environment.
pub unsafe fn select_driver() {
    let Some(bundle) = std::env::current_exe()
        .ok()
        .and_then(|exe| bundle_dir(&exe))
    else {
        return;
    };
    ensure_loader(&bundle);
    // A driver chosen by the user (or a launcher) wins.
    if DRIVER_VARIABLES
        .iter()
        .any(|name| std::env::var_os(name).is_some())
    {
        return;
    }
    if host_renders() {
        return;
    }
    let manifest = bundle.join(MANIFEST);
    for name in DRIVER_VARIABLES {
        // SAFETY: the caller guarantees the process is still single-threaded.
        unsafe { std::env::set_var(name, &manifest) };
    }
    let outcome = if host_renders() {
        "no usable Vulkan device; rendering with the bundled software driver (lavapipe)"
    } else {
        "no usable Vulkan device, and the bundled software driver (lavapipe) failed too"
    };
    eprintln!("Codex Minibar: {outcome}");
    let _ = OUTCOME.set(outcome.to_owned());
}

/// Writes the startup decision to the log once the logger is up.
pub fn log_outcome() {
    if let Some(outcome) = OUTCOME.get() {
        crate::platform::logger::info(format!("Vulkan: {outcome}"));
    }
}

fn bundle_dir(exe: &Path) -> Option<PathBuf> {
    let dir = exe.parent()?.join(BUNDLE_DIR);
    dir.join(MANIFEST).is_file().then_some(dir)
}

/// Makes `libvulkan.so.1` loadable, from the bundle if the host has none.
/// blade loads the loader by its soname, which then resolves to the copy
/// already loaded here. The bundled loader still finds the host's drivers.
fn ensure_loader(bundle: &Path) {
    if !dlopen(OsStr::new(LOADER), libc::RTLD_LOCAL) {
        dlopen(bundle.join(LOADER).as_os_str(), libc::RTLD_GLOBAL);
    }
}

/// Loads a library for the process lifetime.
fn dlopen(name: &OsStr, scope: libc::c_int) -> bool {
    let Ok(name) = CString::new(name.as_bytes()) else {
        return false;
    };
    // SAFETY: `name` is a valid C string. The handle is never closed, so the
    // library stays loaded for later lookups by soname.
    !unsafe { libc::dlopen(name.as_ptr(), libc::RTLD_NOW | scope) }.is_null()
}

/// Whether GPUI's renderer can start: the same context GPUI creates.
fn host_renders() -> bool {
    // SAFETY: a throwaway context; it is dropped before GPUI creates its own.
    unsafe {
        blade_graphics::Context::init(blade_graphics::ContextDesc {
            presentation: true,
            validation: false,
            ..Default::default()
        })
    }
    .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_is_found_next_to_the_appimage_binary_only() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("usr/bin/codex-minibar");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        assert_eq!(bundle_dir(&exe), None);

        let bundle = root.path().join("usr/lib/lavapipe");
        std::fs::create_dir_all(&bundle).unwrap();
        assert_eq!(bundle_dir(&exe), None);

        std::fs::write(bundle.join(MANIFEST), "{}").unwrap();
        let found = bundle_dir(&exe).unwrap();
        assert!(found.join(MANIFEST).is_file());
    }
}
