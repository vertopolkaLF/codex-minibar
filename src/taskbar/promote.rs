//! Promotes notification icons onto the Windows 10 taskbar (also restored
//! by ExplorerPatcher), which keeps no `NotifyIconSettings` entries.
//!
//! Explorer's private `ITrayNotify` is the interface behind its "select which
//! icons appear on the taskbar" page. Its `NOTIFYITEM` grows between builds,
//! and marshaling one of the wrong size crashes the caller inside RPC, so the
//! item is never built here: Explorer hands every item to a registered
//! `INotificationCB`, in its own layout, and that same item goes back with
//! only the preference changed. Even so the interface is undocumented, so it
//! runs in a short-lived child process ([`run_from_args`]): a crash there
//! costs one attempt, not the app.

// COM method names follow the interfaces Explorer exposes.
#![allow(non_snake_case)]

use std::{cell::Cell, ffi::c_void, process::Command, rc::Rc};

use windows::Win32::System::Com::{
    CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
};
use windows_core::{GUID, HRESULT, IUnknown, Interface, implement, interface};

/// The argument that turns a launch of the app into the helper.
const FLAG: &str = "--promote-tray-icons";

/// Exit codes of the helper.
const PROMOTED: i32 = 0;
const NOT_FOUND: i32 = 1;
const UNAVAILABLE: i32 = 2;

const CLSID_TRAY_NOTIFY: GUID = GUID::from_u128(0x25DEAD04_1EAC_4911_9E3A_AD0A4AB560FD);
/// "Show icon and notifications".
const SHOW_ALWAYS: u32 = 2;

/// The leading members of Explorer's `NOTIFYITEM`, which have kept their
/// place in every build; later builds append more. Only ever read through a
/// pointer Explorer handed over, never built or copied.
#[repr(C)]
struct NotifyItem {
    exe_name: *mut u16,
    tip: *mut u16,
    icon: *mut c_void,
    window: isize,
    preference: u32,
    id: u32,
}

#[interface("D782CCBA-AFB0-43F1-94DB-FDA3779EACCB")]
unsafe trait INotificationCB: IUnknown {
    unsafe fn Notify(&self, event: u32, item: *mut NotifyItem) -> HRESULT;
}

/// `ITrayNotify` as of Windows 8.
#[interface("D133CE13-3537-48BA-93A7-AFCD5D2053B4")]
unsafe trait ITrayNotify: IUnknown {
    unsafe fn RegisterCallback(&self, callback: *mut c_void, cookie: *mut u32) -> HRESULT;
    unsafe fn UnregisterCallback(&self, cookie: *mut u32) -> HRESULT;
    unsafe fn SetPreference(&self, item: *const NotifyItem) -> HRESULT;
}

#[implement(INotificationCB)]
struct Callback {
    tray: ITrayNotify,
    window: isize,
    ids: Vec<u32>,
    /// Icons found, already shown or promoted now.
    found: Rc<Cell<usize>>,
}

impl INotificationCB_Impl for Callback_Impl {
    unsafe fn Notify(&self, _event: u32, item: *mut NotifyItem) -> HRESULT {
        let Some(item) = (unsafe { item.as_mut() }) else {
            return HRESULT(0);
        };
        if item.window != self.window || !self.ids.contains(&item.id) {
            return HRESULT(0);
        }
        if item.preference != SHOW_ALWAYS {
            item.preference = SHOW_ALWAYS;
            if unsafe { self.tray.SetPreference(item) }.is_err() {
                return HRESULT(0);
            }
        }
        self.found.set(self.found.get() + 1);
        HRESULT(0)
    }
}

/// Runs the helper when the app was launched as one; returns its exit code.
pub fn run_from_args() -> Option<i32> {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some(FLAG) {
        return None;
    }
    let window = args.next().and_then(|value| value.parse::<isize>().ok())?;
    let ids = args
        .filter_map(|value| value.parse::<u32>().ok())
        .collect::<Vec<_>>();
    Some(promote(window, ids))
}

fn promote(window: isize, ids: Vec<u32>) -> i32 {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let Ok(unknown) =
            CoCreateInstance::<_, IUnknown>(&CLSID_TRAY_NOTIFY, None, CLSCTX_LOCAL_SERVER)
        else {
            return UNAVAILABLE;
        };
        let Ok(tray) = unknown.cast::<ITrayNotify>() else {
            return UNAVAILABLE;
        };
        let count = ids.len();
        let found = Rc::new(Cell::new(0));
        let callback: INotificationCB = Callback {
            tray: tray.clone(),
            window,
            ids,
            found: Rc::clone(&found),
        }
        .into();
        let mut cookie = 0;
        // Explorer reports every current item before this returns.
        if tray
            .RegisterCallback(callback.as_raw(), &mut cookie)
            .is_err()
        {
            return UNAVAILABLE;
        }
        let _ = tray.UnregisterCallback(&mut cookie);
        drop(callback);
        if found.get() >= count {
            PROMOTED
        } else {
            NOT_FOUND
        }
    }
}

/// A helper run promoting `ids` of `window`.
pub(super) struct Run {
    child: std::process::Child,
    started: std::time::Instant,
}

/// Longest a helper may take before it is killed.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

impl Run {
    pub(super) fn start(window: isize, ids: &[u32]) -> Option<Self> {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        const NO_WINDOW: u32 = 0x0800_0000;
        let exe = std::env::current_exe().ok()?;
        let child = Command::new(exe)
            .arg(FLAG)
            .arg(window.to_string())
            .args(ids.iter().map(u32::to_string))
            .creation_flags(NO_WINDOW)
            .spawn()
            .ok()?;
        Some(Self {
            child,
            started: std::time::Instant::now(),
        })
    }

    /// `Some(true)` once every icon is shown, `Some(false)` when the run
    /// failed, `None` while it is still running.
    pub(super) fn finished(&mut self) -> Option<bool> {
        match self.child.try_wait() {
            Ok(Some(status)) => {
                let code = status.code();
                if code != Some(PROMOTED) {
                    crate::logger::info(format!("Tray icon promotion helper exited with {code:?}"));
                }
                Some(code == Some(PROMOTED))
            }
            Ok(None) if self.started.elapsed() < TIMEOUT => None,
            _ => {
                let _ = self.child.kill();
                let _ = self.child.wait();
                crate::logger::info("Tray icon promotion helper timed out");
                Some(false)
            }
        }
    }
}
