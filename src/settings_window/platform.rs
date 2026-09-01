use super::*;

#[cfg(windows)]
pub(super) fn install_settings_close_hide() {
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SC_CLOSE, SW_HIDE, ShowWindow, WM_CLOSE, WM_NCDESTROY, WM_SYSCOMMAND,
    };

    const SUBCLASS_ID: usize = 0xC0DE_5E77;

    let hwnd = find_settings_window();
    if hwnd.is_null() {
        return;
    }

    unsafe extern "system" fn subclass_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _uid: usize,
        _data: usize,
    ) -> LRESULT {
        let is_close =
            msg == WM_CLOSE || (msg == WM_SYSCOMMAND && (wparam & 0xFFF0) as u32 == SC_CLOSE);
        if is_close {
            // Hide while fully painted; default processing then destroys.
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
        if msg == WM_NCDESTROY {
            unsafe {
                RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
            }
        }
        unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
    }

    unsafe {
        let _ = SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, 0);
    }
}

#[cfg(windows)]
pub(super) fn set_settings_window_icon() {
    use windows_sys::Win32::{
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{ICON_BIG, ICON_SMALL, LoadIconW, SendMessageW, WM_SETICON},
    };

    let hwnd = find_settings_window();
    if hwnd.is_null() {
        return;
    }

    // `winresource` embeds the application icon as resource 1.
    let module = unsafe { GetModuleHandleW(std::ptr::null()) };
    let icon = unsafe { LoadIconW(module, 1usize as *const u16) };
    if !icon.is_null() {
        unsafe {
            SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, icon as isize);
            SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, icon as isize);
        }
    }
}

/// The caption buttons are painted by DWM, outside the XAML `TitleBar` tree.
/// Keep their light/dark glyphs in lockstep with the live WinUI theme.
#[cfg(windows)]
pub(super) fn sync_settings_caption_button_theme(color_scheme: ColorScheme) {
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;

    const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
    let hwnd = find_settings_window();
    if hwnd.is_null() {
        return;
    }

    let use_dark_caption_buttons = i32::from(matches!(color_scheme, ColorScheme::Dark));
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &use_dark_caption_buttons as *const i32 as *const _,
            size_of::<i32>() as u32,
        );
    }
}

#[cfg(windows)]
pub(super) fn find_settings_window() -> windows_sys::Win32::Foundation::HWND {
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;

    [SETTINGS_WINDOW_TITLE, ONBOARDING_WINDOW_TITLE]
        .into_iter()
        .map(|title| {
            let title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
            unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) }
        })
        .find(|hwnd| !hwnd.is_null())
        .unwrap_or(std::ptr::null_mut())
}

#[cfg(not(windows))]
pub(super) fn sync_settings_caption_button_theme(_color_scheme: ColorScheme) {}

#[cfg(not(windows))]
pub(super) fn set_settings_window_icon() {}

#[cfg(not(windows))]
pub(super) fn install_settings_close_hide() {}

pub(super) fn load_settings_for_window() -> Settings {
    match Settings::default_path().and_then(|path| Settings::load_or_create(&path)) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("failed to load settings for window: {error:#}");
            Settings::default()
        }
    }
}

/// Whether the independent settings surface is currently alive.
///
/// The tray popup uses this to stay visible as a live preview while a user
/// navigates settings and changes popup-related options.
#[cfg(windows)]
pub(crate) fn is_open() -> bool {
    !find_settings_window().is_null()
}

#[cfg(not(windows))]
pub(crate) fn is_open() -> bool {
    false
}

#[cfg(windows)]
pub(super) fn close_open_window() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
    let hwnd = find_settings_window();
    if !hwnd.is_null() {
        unsafe {
            let _ = PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
    }
}

#[cfg(not(windows))]
pub(super) fn close_open_window() {}

#[cfg(windows)]
pub(super) fn choose_settings_file(save: bool) -> anyhow::Result<Option<PathBuf>> {
    use windows_sys::Win32::UI::Controls::Dialogs::{
        GetOpenFileNameW, GetSaveFileNameW, OFN_FILEMUSTEXIST, OFN_OVERWRITEPROMPT,
        OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };

    let mut filename = "codex-minibar-settings.toml"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    filename.resize(32_768, 0);
    let filter = "Codex Minibar settings (*.toml)\0*.toml\0\0"
        .encode_utf16()
        .collect::<Vec<_>>();
    let default_extension = "toml\0".encode_utf16().collect::<Vec<_>>();
    let title = if save {
        "Export settings\0"
    } else {
        "Import settings\0"
    }
    .encode_utf16()
    .collect::<Vec<_>>();
    let mut dialog: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    dialog.lpstrFilter = filter.as_ptr();
    dialog.lpstrFile = filename.as_mut_ptr();
    dialog.nMaxFile = filename.len() as u32;
    dialog.lpstrTitle = title.as_ptr();
    dialog.lpstrDefExt = default_extension.as_ptr();
    dialog.Flags = OFN_PATHMUSTEXIST
        | if save {
            OFN_OVERWRITEPROMPT
        } else {
            OFN_FILEMUSTEXIST
        };

    let accepted = unsafe {
        if save {
            GetSaveFileNameW(&mut dialog)
        } else {
            GetOpenFileNameW(&mut dialog)
        }
    } != 0;
    if !accepted {
        return Ok(None);
    }
    let length = filename.iter().position(|&unit| unit == 0).unwrap_or(0);
    Ok(Some(PathBuf::from(String::from_utf16(
        &filename[..length],
    )?)))
}

#[cfg(not(windows))]
pub(super) fn choose_settings_file(_save: bool) -> anyhow::Result<Option<PathBuf>> {
    anyhow::bail!("settings import and export are only available on Windows")
}

#[cfg(windows)]
pub(super) fn choose_provider_folder() -> anyhow::Result<Option<PathBuf>> {
    use windows_sys::Win32::UI::Shell::{
        BIF_EDITBOX, BIF_NEWDIALOGSTYLE, BIF_RETURNONLYFSDIRS, BROWSEINFOW, ILFree,
        SHBrowseForFolderW, SHGetPathFromIDListW,
    };

    let title = "Select the folder with the provider files\0"
        .encode_utf16()
        .collect::<Vec<_>>();
    let mut display_name = [0_u16; 260];
    let mut dialog: BROWSEINFOW = unsafe { std::mem::zeroed() };
    dialog.pszDisplayName = display_name.as_mut_ptr();
    dialog.lpszTitle = title.as_ptr();
    dialog.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE | BIF_EDITBOX;

    let item_id_list = unsafe { SHBrowseForFolderW(&dialog) };
    if item_id_list.is_null() {
        return Ok(None);
    }

    let mut path = [0_u16; 32_768];
    let found_path = unsafe { SHGetPathFromIDListW(item_id_list, path.as_mut_ptr()) } != 0;
    unsafe { ILFree(item_id_list) };
    if !found_path {
        return Ok(None);
    }

    let length = path.iter().position(|&unit| unit == 0).unwrap_or(0);
    Ok(Some(PathBuf::from(String::from_utf16(&path[..length])?)))
}

#[cfg(not(windows))]
pub(super) fn choose_provider_folder() -> anyhow::Result<Option<PathBuf>> {
    anyhow::bail!("provider folder selection is only available on Windows")
}

#[cfg(windows)]
pub(super) fn confirm_settings_reset() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IDYES, MB_ICONWARNING, MB_YESNO, MessageBoxW,
    };

    let message = "Reset all Codex Minibar settings to their defaults?\0"
        .encode_utf16()
        .collect::<Vec<_>>();
    let title = "Reset settings\0".encode_utf16().collect::<Vec<_>>();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_YESNO | MB_ICONWARNING,
        ) == IDYES
    }
}

#[cfg(not(windows))]
pub(super) fn confirm_settings_reset() -> bool {
    false
}
