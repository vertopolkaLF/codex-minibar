//! System-wide shortcut that shows or hides the popup.
//!
//! The shortcut is registered with `RegisterHotKey` on the bridge thread,
//! which already pumps the tray's messages. `WM_HOTKEY` arrives as a thread
//! message (no window), so [`observe_message`] picks it out of the pump and
//! the bridge consumes it with [`take_pressed`].

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

/// A key plus modifiers. `key` is a Windows virtual-key code, so the
/// shortcut follows the physical key regardless of the keyboard layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hotkey {
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub win: bool,
    pub key: u16,
}

const VK_F1: u16 = 0x70;
const VK_F13: u16 = 0x7C;
const VK_F24: u16 = 0x87;

impl Hotkey {
    /// Ctrl, Alt or Win is required so the shortcut cannot swallow ordinary
    /// typing (Shift+A) or a key that other apps rely on (F5). F13–F24 are
    /// free on standard keyboards and macro pads, so they may stand alone.
    pub fn is_valid(&self) -> bool {
        !is_modifier_key(self.key)
            && self.key != 0
            && (self.ctrl || self.alt || self.win || (VK_F13..=VK_F24).contains(&self.key))
    }

    /// Display text such as `Ctrl+Alt+M`.
    pub fn label(&self) -> String {
        let mut parts = Vec::with_capacity(5);
        if self.ctrl {
            parts.push("Ctrl".to_string());
        }
        if self.alt {
            parts.push("Alt".to_string());
        }
        if self.shift {
            parts.push("Shift".to_string());
        }
        if self.win {
            parts.push("Win".to_string());
        }
        parts.push(key_name(self.key));
        parts.join("+")
    }
}

fn is_modifier_key(key: u16) -> bool {
    matches!(
        key,
        // Shift, Ctrl, Alt, both Win keys and their left/right variants.
        0x10 | 0x11 | 0x12 | 0x5B | 0x5C | 0xA0..=0xA5
    )
}

fn key_name(key: u16) -> String {
    match key {
        0x30..=0x39 | 0x41..=0x5A => char::from(key as u8).to_string(),
        VK_F1..=VK_F24 => format!("F{}", key - VK_F1 + 1),
        _ => system_key_name(key).unwrap_or_else(|| format!("Key {key:#04X}")),
    }
}

#[cfg(windows)]
fn system_key_name(key: u16) -> Option<String> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyNameTextW, MAPVK_VK_TO_VSC, MapVirtualKeyW,
    };
    let scan = unsafe { MapVirtualKeyW(u32::from(key), MAPVK_VK_TO_VSC) };
    if scan == 0 {
        return None;
    }
    // Navigation keys share scan codes with the numeric keypad; the extended
    // bit selects "Delete" instead of "Num Del".
    let extended = matches!(key, 0x21..=0x28 | 0x2D | 0x2E | 0x6F | 0x90 | 0x5D);
    let lparam = ((scan as i32) << 16) | if extended { 1 << 24 } else { 0 };
    let mut buffer = [0u16; 64];
    let len = unsafe { GetKeyNameTextW(lparam, buffer.as_mut_ptr(), buffer.len() as i32) };
    (len > 0).then(|| String::from_utf16_lossy(&buffer[..len as usize]))
}

#[cfg(not(windows))]
fn system_key_name(_key: u16) -> Option<String> {
    None
}

/// Reads the shortcut being pressed while a key-down message is handled.
///
/// GPUI reports layout-dependent key text (and folds Shift into symbols), so
/// the recorder reads the virtual-key state Windows keeps for the message
/// being processed instead.
#[cfg(windows)]
pub fn pressed_now() -> Option<Hotkey> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    };
    let down = |key: u16| unsafe { GetKeyState(i32::from(key)) } < 0;
    let key = (0x08..=0xFE_u16).find(|&key| {
        !is_modifier_key(key)
            // Mouse buttons, lock keys, IME processing and injected Unicode.
            && !matches!(key, 0x14 | 0x90 | 0x91 | 0xE5 | 0xE7)
            && down(key)
    })?;
    Some(Hotkey {
        ctrl: down(VK_CONTROL),
        alt: down(VK_MENU),
        shift: down(VK_SHIFT),
        win: down(VK_LWIN) || down(VK_RWIN),
        key,
    })
}

#[cfg(not(windows))]
pub fn pressed_now() -> Option<Hotkey> {
    None
}

/// Registration outcome, shown under the shortcut in Settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Off,
    Active,
    /// Another application (or Windows) already owns the combination.
    Unavailable,
}

static STATUS: AtomicU8 = AtomicU8::new(0);
static PRESSED: AtomicBool = AtomicBool::new(false);
/// Set while Settings records a new shortcut, so pressing the current one
/// records it instead of toggling the popup.
static SUSPENDED: AtomicBool = AtomicBool::new(false);

pub fn status() -> Status {
    match STATUS.load(Ordering::SeqCst) {
        1 => Status::Active,
        2 => Status::Unavailable,
        _ => Status::Off,
    }
}

fn set_status(status: Status) -> bool {
    let value = match status {
        Status::Off => 0,
        Status::Active => 1,
        Status::Unavailable => 2,
    };
    STATUS.swap(value, Ordering::SeqCst) != value
}

pub fn set_suspended(suspended: bool) {
    SUSPENDED.store(suspended, Ordering::SeqCst);
}

pub fn is_suspended() -> bool {
    SUSPENDED.load(Ordering::SeqCst)
}

#[cfg(windows)]
const HOTKEY_ID: i32 = 0x4D42; // "MB"

/// Notes a `WM_HOTKEY` for the popup shortcut. Called by the bridge's
/// message pump for every message it removes from the queue.
#[cfg(windows)]
pub fn observe_message(message: &windows_sys::Win32::UI::WindowsAndMessaging::MSG) {
    use windows_sys::Win32::UI::WindowsAndMessaging::WM_HOTKEY;
    if message.message == WM_HOTKEY && message.wParam == HOTKEY_ID as usize {
        PRESSED.store(true, Ordering::SeqCst);
    }
}

/// Whether the shortcut was pressed since the last call.
pub fn take_pressed() -> bool {
    PRESSED.swap(false, Ordering::SeqCst)
}

/// Owns the registration. Must live on, and be dropped by, the thread whose
/// message queue receives `WM_HOTKEY` (the bridge thread).
#[derive(Default)]
pub struct Registration {
    current: Option<Hotkey>,
    /// Last requested shortcut, so a failed registration is not retried on
    /// every bridge tick.
    requested: Option<Hotkey>,
}

impl Registration {
    /// Registers `desired`, replacing the previous shortcut. Returns whether
    /// the visible status changed.
    pub fn sync(&mut self, desired: Option<Hotkey>) -> bool {
        let desired = desired.filter(Hotkey::is_valid);
        if desired == self.requested {
            return false;
        }
        self.requested = desired;
        self.unregister();
        let Some(hotkey) = desired else {
            return set_status(Status::Off);
        };
        if register(hotkey) {
            self.current = Some(hotkey);
            set_status(Status::Active)
        } else {
            crate::logger::info(format!(
                "Popup shortcut {} is already in use by another application",
                hotkey.label()
            ));
            set_status(Status::Unavailable)
        }
    }

    fn unregister(&mut self) {
        if self.current.take().is_some() {
            unregister();
        }
        PRESSED.store(false, Ordering::SeqCst);
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        self.unregister();
    }
}

#[cfg(windows)]
fn register(hotkey: Hotkey) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey,
    };
    let mut modifiers = MOD_NOREPEAT;
    if hotkey.ctrl {
        modifiers |= MOD_CONTROL;
    }
    if hotkey.alt {
        modifiers |= MOD_ALT;
    }
    if hotkey.shift {
        modifiers |= MOD_SHIFT;
    }
    if hotkey.win {
        modifiers |= MOD_WIN;
    }
    unsafe {
        RegisterHotKey(
            std::ptr::null_mut(),
            HOTKEY_ID,
            modifiers,
            u32::from(hotkey.key),
        ) != 0
    }
}

#[cfg(windows)]
fn unregister() {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::UnregisterHotKey;
    unsafe {
        UnregisterHotKey(std::ptr::null_mut(), HOTKEY_ID);
    }
}

#[cfg(not(windows))]
fn register(_hotkey: Hotkey) -> bool {
    false
}

#[cfg(not(windows))]
fn unregister() {}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(ctrl: bool, alt: bool, shift: bool, win: bool, key: u16) -> Hotkey {
        Hotkey {
            ctrl,
            alt,
            shift,
            win,
            key,
        }
    }

    #[test]
    fn requires_a_modifier_except_for_extended_function_keys() {
        assert!(key(true, true, false, false, 0x4D).is_valid());
        assert!(key(false, false, false, true, 0x4D).is_valid());
        assert!(!key(false, false, true, false, 0x41).is_valid());
        assert!(!key(false, false, false, false, 0x74).is_valid());
        assert!(key(false, false, false, false, 0x7C).is_valid());
        assert!(!key(true, false, false, false, 0x11).is_valid());
    }

    #[test]
    fn labels_letters_digits_and_function_keys() {
        assert_eq!(key(true, true, false, false, 0x4D).label(), "Ctrl+Alt+M");
        assert_eq!(key(false, false, true, true, 0x31).label(), "Shift+Win+1");
        assert_eq!(key(false, false, false, false, 0x87).label(), "F24");
    }
}
