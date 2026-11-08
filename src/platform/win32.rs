//! Small Win32 helpers shared by every module that talks to the OS directly.

#[cfg(windows)]
use std::ffi::{OsStr, OsString};
#[cfg(windows)]
use std::os::windows::ffi::{OsStrExt, OsStringExt};

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{ERROR_SUCCESS, HWND},
    System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
        RRF_RT_ANY, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteValueW,
        RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, RegSetValueExW,
    },
};

/// `value` as a null-terminated UTF-16 string.
#[cfg(windows)]
pub fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
    value
        .as_ref()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Window classes of the primary and the secondary-monitor taskbars.
#[cfg(windows)]
pub const TASKBAR_CLASSES: [&str; 2] = ["Shell_TrayWnd", "Shell_SecondaryTrayWnd"];

/// The primary taskbar window, or null when Explorer is not running.
#[cfg(windows)]
pub fn primary_taskbar() -> HWND {
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;
    unsafe { FindWindowW(wide(TASKBAR_CLASSES[0]).as_ptr(), std::ptr::null()) }
}

/// Whether the shell (taskbar and notification area) uses the light theme.
/// The shell follows the "Windows mode" (`SystemUsesLightTheme`), not the
/// app mode; Windows 10 and 11 default to a dark taskbar when it is missing.
/// On Linux this is the panel hosting the tray: GNOME-family top bars are dark
/// in both color schemes; other desktops follow the portal color scheme and
/// default to light without a preference.
pub fn shell_uses_light_theme() -> bool {
    #[cfg(windows)]
    {
        RegKey::open(
            HKEY_CURRENT_USER,
            r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        )
        .and_then(|key| key.dword("SystemUsesLightTheme"))
        .is_some_and(|value| value != 0)
    }
    #[cfg(not(windows))]
    {
        !crate::platform::linux::desktop::panel_always_dark()
            && crate::platform::linux::desktop::prefers_dark() != Some(true)
    }
}

/// An open registry key, closed on drop.
#[cfg(windows)]
pub struct RegKey(HKEY);

#[cfg(windows)]
impl RegKey {
    /// Opens `path` under `parent` for reading.
    pub fn open(parent: HKEY, path: impl AsRef<OsStr>) -> Option<Self> {
        Self::open_with(parent, path, KEY_READ)
    }

    /// Opens an existing `path` under `parent` for writing values.
    pub fn open_for_write(parent: HKEY, path: impl AsRef<OsStr>) -> Option<Self> {
        Self::open_with(parent, path, KEY_SET_VALUE)
    }

    fn open_with(parent: HKEY, path: impl AsRef<OsStr>, access: u32) -> Option<Self> {
        let mut key: HKEY = std::ptr::null_mut();
        let status = unsafe { RegOpenKeyExW(parent, wide(path).as_ptr(), 0, access, &mut key) };
        (status == ERROR_SUCCESS).then_some(Self(key))
    }

    /// Opens `path` under HKCU for writing values, creating it if needed.
    pub fn create_hkcu(path: &str) -> anyhow::Result<Self> {
        let mut key: HKEY = std::ptr::null_mut();
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide(path).as_ptr(),
                0,
                std::ptr::null_mut(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        };
        anyhow::ensure!(status == ERROR_SUCCESS, "RegCreateKeyExW({path}): {status}");
        Ok(Self(key))
    }

    pub fn subkey(&self, name: impl AsRef<OsStr>) -> Option<Self> {
        Self::open(self.0, name)
    }

    /// Names of the direct subkeys.
    pub fn subkey_names(&self) -> Vec<OsString> {
        let mut names = Vec::new();
        for index in 0.. {
            let mut name = [0_u16; 512];
            let mut len = name.len() as u32;
            let status = unsafe {
                RegEnumKeyExW(
                    self.0,
                    index,
                    name.as_mut_ptr(),
                    &mut len,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };
            if status != ERROR_SUCCESS {
                break;
            }
            names.push(OsString::from_wide(&name[..len as usize]));
        }
        names
    }

    /// A `REG_SZ` value, up to its first NUL.
    pub fn string(&self, name: &str) -> Option<OsString> {
        let mut data = [0_u16; 2048];
        let mut size = std::mem::size_of_val(&data) as u32;
        let status = unsafe {
            RegGetValueW(
                self.0,
                std::ptr::null(),
                wide(name).as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                data.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let len = data
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(data.len());
        Some(OsString::from_wide(&data[..len]))
    }

    pub fn dword(&self, name: &str) -> Option<u32> {
        let mut value = 0_u32;
        let mut size = size_of::<u32>() as u32;
        let status = unsafe {
            RegGetValueW(
                self.0,
                std::ptr::null(),
                wide(name).as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                (&mut value as *mut u32).cast(),
                &mut size,
            )
        };
        (status == ERROR_SUCCESS).then_some(value)
    }

    /// Whether a value of any type named `name` exists.
    pub fn has_value(&self, name: &str) -> bool {
        let status = unsafe {
            RegGetValueW(
                self.0,
                std::ptr::null(),
                wide(name).as_ptr(),
                RRF_RT_ANY,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        status == ERROR_SUCCESS
    }

    /// Writes a `REG_SZ` value; the Win32 status on failure.
    pub fn set_string(&self, name: &str, value: impl AsRef<OsStr>) -> Result<(), u32> {
        let data = wide(value);
        let status = unsafe {
            RegSetValueExW(
                self.0,
                wide(name).as_ptr(),
                0,
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * size_of::<u16>()) as u32,
            )
        };
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(status)
        }
    }

    /// Deletes a value; the Win32 status on failure.
    pub fn delete_value(&self, name: &str) -> Result<(), u32> {
        let status = unsafe { RegDeleteValueW(self.0, wide(name).as_ptr()) };
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(status)
        }
    }
}

#[cfg(windows)]
impl Drop for RegKey {
    fn drop(&mut self) {
        unsafe { RegCloseKey(self.0) };
    }
}
