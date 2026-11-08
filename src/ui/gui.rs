//! GPUI building blocks shared by the popup, the toast host and the Settings
//! and onboarding windows: colors, fonts, motion, embedded assets and the
//! component kit. Nothing here knows which window it paints.

pub(crate) mod assets;
pub(crate) mod color;
pub(crate) mod fx;
pub(crate) mod input;
pub(crate) mod kit;
pub(crate) mod mark;
pub(crate) mod segmented;
pub(crate) mod theme;

use gpui::{FontWeight, SharedString, px};

/// Measure with the same font, size and weight used to render the label.
pub(crate) fn measure_text(
    text_system: &gpui::WindowTextSystem,
    family: SharedString,
    size: f32,
    weight: FontWeight,
    value: &str,
) -> f32 {
    let mut font = gpui::font(family);
    font.weight = weight;
    value
        .lines()
        .map(|line| {
            let run = gpui::TextRun {
                len: line.len(),
                font: font.clone(),
                color: gpui::black(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            f32::from(
                text_system
                    .shape_line(line.to_owned().into(), px(size), &[run], None)
                    .width,
            )
        })
        .fold(0.0_f32, f32::max)
        .ceil()
}

#[cfg(windows)]
pub(crate) fn hwnd_from_window(
    window: &gpui::Window,
) -> Option<windows_sys::Win32::Foundation::HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as _),
        _ => None,
    }
}
