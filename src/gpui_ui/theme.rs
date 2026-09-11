//! Shared GPUI appearance tokens. No XAML resources or window-order dependency.
use std::{sync::atomic::{AtomicBool, AtomicU32, Ordering}, time::Duration};
use crate::settings::{AppTheme, AccentColor};
static ANIMATIONS: AtomicBool = AtomicBool::new(true);
static ACCENT: AtomicU32 = AtomicU32::new(0x0078d4);
pub fn set_animations_enabled(value: bool) { ANIMATIONS.store(value, Ordering::Relaxed); }
pub fn animations_enabled() -> bool { ANIMATIONS.load(Ordering::Relaxed) && crate::popup::system_animations_enabled() }
pub fn duration(value: Duration) -> Duration { if animations_enabled() { value } else { Duration::ZERO } }
pub fn apply_appearance(_: AppTheme, accent: AccentColor) {
    let (r,g,b) = accent.rgb().unwrap_or_else(system_accent);
    ACCENT.store((r as u32) << 16 | (g as u32) << 8 | b as u32, Ordering::Relaxed);
}
pub fn current_accent_rgb() -> [u8;3] { let c = ACCENT.load(Ordering::Relaxed); [(c >> 16) as u8,(c >> 8) as u8,c as u8] }
fn system_accent() -> (u8,u8,u8) {
    #[cfg(windows)] unsafe {
        let mut color = 0u32; let mut opaque = 0;
        if windows_sys::Win32::Graphics::Dwm::DwmGetColorizationColor(&mut color, &mut opaque) == 0 {
            return ((color >> 16) as u8, (color >> 8) as u8, color as u8);
        }
    }
    (0,120,212)
}
