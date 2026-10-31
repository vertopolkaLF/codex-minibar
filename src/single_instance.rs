//! Process-wide single-instance ownership and activation.

#[cfg(windows)]
mod platform {
    use std::ptr;
    use std::sync::{Mutex, OnceLock};

    use anyhow::{Context, Result};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND},
        System::Threading::CreateMutexW,
        UI::WindowsAndMessaging::{
            FindWindowW, PostMessageW, SW_RESTORE, SetForegroundWindow, ShowWindow,
        },
    };

    use crate::popup_window::ui::win32::POPUP_TITLE;

    const MUTEX_NAME: &str = "Local\\CodexMinibar.9F89F5E9-770D-41AA-879F-9B15C12A2E6A";

    static HOLDER: OnceLock<Mutex<Option<isize>>> = OnceLock::new();

    pub struct SingleInstance(HANDLE);

    impl SingleInstance {
        /// Returns `None` after bringing the primary process to the foreground.
        pub fn acquire_or_activate_existing() -> Result<Option<Self>> {
            let name = wide(MUTEX_NAME);
            let handle = unsafe { CreateMutexW(ptr::null(), 1, name.as_ptr()) };
            if handle.is_null() {
                return Err(std::io::Error::last_os_error())
                    .context("create single-instance mutex");
            }
            if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
                focus_existing_window();
                unsafe { CloseHandle(handle) };
                return Ok(None);
            }
            Ok(Some(Self(handle)))
        }

        /// Keeps the mutex alive for the process lifetime.
        pub fn hold(instance: Self) {
            let handle = instance.0 as isize;
            std::mem::forget(instance);
            HOLDER
                .get_or_init(|| Mutex::new(None))
                .lock()
                .expect("single-instance holder lock")
                .replace(handle);
        }
    }

    impl Drop for SingleInstance {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }

    /// Releases the single-instance mutex before a relaunching update exit.
    pub fn release_for_update() {
        if let Some(holder) = HOLDER.get()
            && let Ok(mut slot) = holder.lock()
            && let Some(handle) = slot.take()
        {
            unsafe { CloseHandle(handle as HANDLE) };
        }
    }

    fn focus_existing_window() {
        // Prefer Settings: it is already an independently focusable surface.
        let hwnd = crate::i18n::Language::SUPPORTED
            .into_iter()
            .find_map(|language| {
                find_window(crate::i18n::tr_in(language, "codex-minibar-settings"))
            })
            .or_else(|| find_window(POPUP_TITLE));
        let Some(hwnd) = hwnd else { return };

        unsafe {
            if hwnd == find_window(POPUP_TITLE).unwrap_or(ptr::null_mut()) {
                // Let the owning process restore its host, saved position and
                // z-order instead of moving a parked 1px HWND from this process.
                PostMessageW(hwnd, crate::popup_window::ui::win32::SHOW_MESSAGE, 0, 0);
                SetForegroundWindow(hwnd);
                return;
            }
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);
        }
    }

    fn find_window(title: &str) -> Option<HWND> {
        let title = wide(title);
        let hwnd = unsafe { FindWindowW(ptr::null(), title.as_ptr()) };
        (!hwnd.is_null()).then_some(hwnd)
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }
}

#[cfg(windows)]
pub use platform::{SingleInstance, release_for_update};

#[cfg(not(windows))]
pub use crate::linux::instance::{SingleInstance, release_for_update};
