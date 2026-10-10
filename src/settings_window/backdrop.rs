//! Mica backdrop for the Settings and onboarding windows.
//!
//! GPUI has no Mica option, but on Windows it presents through a
//! premultiplied-alpha DirectComposition swap chain cleared to transparent,
//! so asking DWM for the system backdrop and extending the frame into the
//! client area lets Mica show wherever the UI leaves pixels uncovered.
//!
//! Mica's tint follows the window's immersive dark mode, which GPUI sets from
//! the system theme (and resets on every system theme change). [`sync`]
//! re-applies the app's resolved theme whenever either side changes, so a
//! forced Light/Dark theme tints the backdrop correctly from the first frame.
//!
//! With "Show accent color on title bars and window borders" on, DWM paints
//! the window border and the 1–2px caption strip GPUI keeps above the client
//! area in the accent color. The border is turned off and the caption gets
//! an explicit neutral color, so neither reads as a stripe over Mica. (The
//! strip itself must stay: giving it to the client turns the whole extended
//! frame into a native caption, with DWM's own buttons drawn under ours.)
//!
//! [`sync`]: Backdrop::sync

use gpui::{Window, WindowAppearance};

#[derive(Default)]
pub(crate) struct Backdrop {
    #[cfg(windows)]
    hwnd: Option<windows_sys::Win32::Foundation::HWND>,
    mica: bool,
    applied: Option<(bool, WindowAppearance)>,
}

impl Backdrop {
    /// Ask DWM for Mica. Falls back to the opaque palette where the system
    /// backdrop is unavailable (before Windows 11 22H2).
    pub(crate) fn install(window: &Window) -> Self {
        #[cfg(windows)]
        {
            use windows_sys::Win32::Graphics::Dwm::{
                DWMSBT_MAINWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DwmExtendFrameIntoClientArea,
                DwmSetWindowAttribute,
            };
            use windows_sys::Win32::UI::Controls::MARGINS;

            let Some(hwnd) = crate::popup_window::ui::win32::hwnd_from_window(window) else {
                return Self::default();
            };
            let backdrop = DWMSBT_MAINWINDOW;
            let margins = MARGINS {
                cxLeftWidth: -1,
                cxRightWidth: -1,
                cyTopHeight: -1,
                cyBottomHeight: -1,
            };
            hide_border(hwnd);
            // SAFETY: `hwnd` is this window's live handle; both calls only
            // read the pointed-to values for the duration of the call.
            let mica = unsafe {
                DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_SYSTEMBACKDROP_TYPE as u32,
                    (&raw const backdrop).cast(),
                    size_of_val(&backdrop) as u32,
                ) >= 0
                    && DwmExtendFrameIntoClientArea(hwnd, &margins) >= 0
            };
            Self {
                hwnd: Some(hwnd),
                mica,
                applied: None,
            }
        }
        #[cfg(not(windows))]
        {
            let _ = window;
            Self::default()
        }
    }

    pub(crate) fn mica(&self) -> bool {
        self.mica
    }

    /// Tint the backdrop, frame and caption strip for the resolved theme. Cheap to call
    /// every frame: DWM is only touched when the theme or the system
    /// appearance (which GPUI mirrors into the frame) changes.
    pub(crate) fn sync(&mut self, dark: bool, appearance: WindowAppearance) {
        if self.applied == Some((dark, appearance)) {
            return;
        }
        self.applied = Some((dark, appearance));
        #[cfg(windows)]
        if let Some(hwnd) = self.hwnd {
            use windows_sys::Win32::Graphics::Dwm::{
                DWMWA_CAPTION_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute,
            };
            let value: i32 = dark.into();
            // Mica's base tone, as a COLORREF (0x00BBGGRR): keeps the caption
            // strip neutral instead of the system accent.
            let caption: u32 = if dark { 0x0020_2020 } else { 0x00F3_F3F3 };
            // SAFETY: as above; the values are read during each call only.
            unsafe {
                DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
                    (&raw const value).cast(),
                    size_of_val(&value) as u32,
                );
                DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_CAPTION_COLOR as u32,
                    (&raw const caption).cast(),
                    size_of_val(&caption) as u32,
                );
            }
        }
    }
}

/// Turn off the accent-tinted 1px window border (Windows 11 only; a no-op
/// where DWM has no border control).
#[cfg(windows)]
fn hide_border(hwnd: windows_sys::Win32::Foundation::HWND) {
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DwmSetWindowAttribute,
    };
    let none = DWMWA_COLOR_NONE;
    // SAFETY: live handle; the value is read during the call only.
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR as u32,
            (&raw const none).cast(),
            size_of_val(&none) as u32,
        );
    }
}
