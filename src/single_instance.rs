//! Process-wide single-instance ownership and activation for Windows.

#[cfg(windows)]
mod platform {
    use std::ptr;

    use anyhow::{Context, Result};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, POINT, RECT},
        Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint},
        System::Threading::CreateMutexW,
        UI::WindowsAndMessaging::{
            FindWindowW, GetCursorPos, GetWindowRect, HWND_TOPMOST, SW_RESTORE, SWP_NOACTIVATE,
            SWP_NOSIZE, SWP_SHOWWINDOW, SetForegroundWindow, SetWindowPos, ShowWindow,
        },
    };

    const MUTEX_NAME: &str = "Local\\CodexMinibar.9F89F5E9-770D-41AA-879F-9B15C12A2E6A";
    const POPUP_TITLE: &str = "Codex Minibar";
    const SETTINGS_TITLE: &str = "Codex Minibar Settings";
    const EDGE_MARGIN: i32 = 20;

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
    }

    impl Drop for SingleInstance {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }

    fn focus_existing_window() {
        // Prefer Settings: it is already an independently focusable surface.
        let hwnd = find_window(SETTINGS_TITLE).or_else(|| find_window(POPUP_TITLE));
        let Some(hwnd) = hwnd else { return };

        unsafe {
            ShowWindow(hwnd, SW_RESTORE);
            if hwnd == find_window(POPUP_TITLE).unwrap_or(ptr::null_mut()) {
                position_popup_at_cursor(hwnd);
            }
            SetForegroundWindow(hwnd);
        }
    }

    fn position_popup_at_cursor(hwnd: HWND) {
        let mut cursor = POINT { x: 0, y: 0 };
        let monitor = unsafe {
            GetCursorPos(&mut cursor);
            MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST)
        };
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            rcMonitor: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            rcWork: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            dwFlags: 0,
        };
        if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
            return;
        }
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        unsafe { GetWindowRect(hwnd, &mut rect) };
        let width = (rect.right - rect.left).max(1);
        let height = (rect.bottom - rect.top).max(1);
        unsafe {
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                info.rcWork.right - width - EDGE_MARGIN,
                info.rcWork.bottom - height - EDGE_MARGIN,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOSIZE | SWP_SHOWWINDOW,
            );
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
pub use platform::SingleInstance;

#[cfg(not(windows))]
pub struct SingleInstance;

#[cfg(not(windows))]
impl SingleInstance {
    pub fn acquire_or_activate_existing() -> anyhow::Result<Option<Self>> {
        Ok(Some(Self))
    }
}
