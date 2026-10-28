//! Where Explorer's own taskbar buttons are, read through UI Automation.
//!
//! Windows 11 does not reserve taskbar space for anyone else, so the strip
//! measures the free span between the app buttons and its anchor, compacts
//! widgets that do not fit and then drops those that still do not, the way
//! the notification area overflows. UI
//! Automation calls into Explorer synchronously; they run on their own MTA
//! thread because the host thread shares Explorer's input queue and must
//! never wait on it.
//!
//! The Windows 10 taskbar (also restored by ExplorerPatcher) exposes its app
//! buttons and notification icons only through MSAA, so they are read from
//! the toolbars' `IAccessible` instead.

use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use windows::Win32::{
    Foundation::HWND,
    System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx},
    System::Variant::VARIANT,
    UI::Accessibility::{
        AccessibleObjectFromWindow, CUIAutomation, IAccessible, IUIAutomation,
        TreeScope_Descendants, UIA_AutomationIdPropertyId, UIA_BoundingRectanglePropertyId,
        UIA_ClassNamePropertyId,
    },
};
use windows_core::Interface;

/// Screen rectangle: left, top, right, bottom.
pub(super) type Bounds = (i32, i32, i32, i32);

/// Explorer's buttons, in screen pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Obstacles {
    /// Bounds of Start, Search, Task View and the app buttons together.
    pub(super) cluster: Option<Bounds>,
    /// The Windows widgets button at the leading edge.
    pub(super) widgets: Option<Bounds>,
}

fn union(a: Option<Bounds>, b: Bounds) -> Bounds {
    match a {
        Some(a) => (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)),
        None => b,
    }
}

const INTERVAL: Duration = Duration::from_secs(2);
/// Granularity of the wait between measurements, so a requested refresh
/// does not wait out the whole interval.
const STEP: Duration = Duration::from_millis(100);

static LATEST: Mutex<Option<Obstacles>> = Mutex::new(None);
/// Notification icons on the taskbar, in order, and when they were measured.
static TRAY: Mutex<Option<(Instant, Vec<Bounds>)>> = Mutex::new(None);
static WANTED: AtomicBool = AtomicBool::new(false);
static STARTED: AtomicBool = AtomicBool::new(false);
static REFRESH: AtomicBool = AtomicBool::new(false);

/// Starts or pauses the measurements; the worker starts on first use.
pub(super) fn set_wanted(wanted: bool) {
    WANTED.store(wanted, Ordering::SeqCst);
    if !wanted {
        if let Ok(mut latest) = LATEST.lock() {
            *latest = None;
        }
        if let Ok(mut tray) = TRAY.lock() {
            *tray = None;
        }
        return;
    }
    if !STARTED.swap(true, Ordering::SeqCst) {
        let spawned = std::thread::Builder::new()
            .name("taskbar-obstacles".into())
            .spawn(run);
        if spawned.is_err() {
            STARTED.store(false, Ordering::SeqCst);
        }
    }
}

pub(super) fn latest() -> Option<Obstacles> {
    LATEST.lock().ok().and_then(|latest| *latest)
}

/// Measures again within a step instead of at the next interval.
pub(super) fn refresh() {
    REFRESH.store(true, Ordering::SeqCst);
}

/// The notification icons shown on the taskbar, leading one first, and when
/// they were measured.
pub(super) fn tray_icons() -> Option<(Instant, Vec<Bounds>)> {
    TRAY.lock().ok().and_then(|tray| tray.clone())
}

fn run() {
    let automation: Option<IUIAutomation> = unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()
    };
    let Some(automation) = automation else {
        return;
    };
    loop {
        if WANTED.load(Ordering::SeqCst) {
            let measured = measure(&automation);
            let tray = measured
                .as_ref()
                .map(|(_, tray)| (Instant::now(), tray.clone()));
            if let Ok(mut latest) = LATEST.lock() {
                *latest = measured.map(|(obstacles, _)| obstacles);
            }
            if let Ok(mut latest) = TRAY.lock() {
                *latest = tray;
            }
        }
        let mut waited = Duration::ZERO;
        while waited < INTERVAL && !REFRESH.swap(false, Ordering::SeqCst) {
            std::thread::sleep(STEP);
            waited += STEP;
        }
    }
}

fn measure(automation: &IUIAutomation) -> Option<(Obstacles, Vec<Bounds>)> {
    unsafe {
        let class = "Shell_TrayWnd\0".encode_utf16().collect::<Vec<_>>();
        let taskbar = windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(
            class.as_ptr(),
            std::ptr::null(),
        );
        if taskbar.is_null() {
            return None;
        }
        let taskbar = HWND(taskbar);
        let cache = automation.CreateCacheRequest().ok()?;
        for property in [
            UIA_ClassNamePropertyId,
            UIA_AutomationIdPropertyId,
            UIA_BoundingRectanglePropertyId,
        ] {
            cache.AddProperty(property).ok().ok()?;
        }
        let root = automation.ElementFromHandle(taskbar).ok()?;
        let condition = automation.CreateTrueCondition().ok()?;
        let elements = root
            .FindAllBuildCache(TreeScope_Descendants, &condition, &cache)
            .ok()?;
        let mut obstacles = Obstacles::default();
        let mut tray = Vec::new();
        for index in 0..elements.Length().unwrap_or(0) {
            let Ok(element) = elements.GetElement(index) else {
                continue;
            };
            let class = String::from_utf16_lossy(&element.CachedClassName().unwrap_or_default());
            let id = String::from_utf16_lossy(&element.CachedAutomationId().unwrap_or_default());
            let Ok(rect) = element.CachedBoundingRectangle() else {
                continue;
            };
            if rect.right <= rect.left || rect.bottom <= rect.top {
                continue;
            }
            let bounds = (rect.left, rect.top, rect.right, rect.bottom);
            match classify(&class, &id) {
                Some(Kind::Widgets) => obstacles.widgets = Some(union(obstacles.widgets, bounds)),
                Some(Kind::Cluster) => obstacles.cluster = Some(union(obstacles.cluster, bounds)),
                Some(Kind::TrayIcon) => tray.push(bounds),
                None => {}
            }
        }
        if obstacles.cluster.is_none() {
            obstacles.cluster = classic_cluster(taskbar);
        }
        if tray.is_empty() {
            tray = classic_tray(taskbar);
        }
        tray.sort_by_key(|bounds| (bounds.0, bounds.1));
        Some((obstacles, tray))
    }
}

fn find_child(parent: HWND, class: &str) -> Option<HWND> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowExW, IsWindowVisible};
    let class = class.encode_utf16().chain([0]).collect::<Vec<_>>();
    let child = unsafe {
        FindWindowExW(
            parent.0,
            std::ptr::null_mut(),
            class.as_ptr(),
            std::ptr::null(),
        )
    };
    (!child.is_null() && unsafe { IsWindowVisible(child) } != 0).then_some(HWND(child))
}

fn window_bounds(window: HWND) -> Option<Bounds> {
    let mut rect = unsafe { std::mem::zeroed::<windows_sys::Win32::Foundation::RECT>() };
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(window.0, &mut rect) };
    (rect.right > rect.left && rect.bottom > rect.top).then_some((
        rect.left,
        rect.top,
        rect.right,
        rect.bottom,
    ))
}

/// The app buttons of the Windows 10 taskbar plus any other toolbars on its
/// rebar. The task list itself spans up to the tray, so only its buttons
/// count.
fn classic_cluster(taskbar: HWND) -> Option<Bounds> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowExW, IsWindowVisible};
    let rebar = find_child(taskbar, "ReBarWindow32")?;
    let switcher = find_child(rebar, "MSTaskSwWClass")?;
    let list = find_child(switcher, "MSTaskListWClass")?;
    let mut cluster = None;
    let mut band = std::ptr::null_mut();
    loop {
        band = unsafe { FindWindowExW(rebar.0, band, std::ptr::null(), std::ptr::null()) };
        if band.is_null() {
            break;
        }
        if band != switcher.0
            && unsafe { IsWindowVisible(band) } != 0
            && let Some(bounds) = window_bounds(HWND(band))
        {
            cluster = Some(union(cluster, bounds));
        }
    }
    unsafe {
        let mut object = std::ptr::null_mut();
        // OBJID_CLIENT
        if AccessibleObjectFromWindow(list, 0xFFFF_FFFC, &IAccessible::IID, &mut object).is_err() {
            return cluster;
        }
        let list = IAccessible::from_raw(object);
        for child in 1..=list.accChildCount().unwrap_or(0) {
            let (mut left, mut top, mut width, mut height) = (0, 0, 0, 0);
            let child = VARIANT::from(child);
            if list
                .accLocation(&mut left, &mut top, &mut width, &mut height, &child)
                .is_ok()
                && width > 0
                && height > 0
            {
                cluster = Some(union(cluster, (left, top, left + width, top + height)));
            }
        }
    }
    cluster
}

/// The app icons of the Windows 10 notification area: the buttons of the
/// first toolbar in `SysPager`; the system icons live in a second one.
fn classic_tray(taskbar: HWND) -> Vec<Bounds> {
    let mut icons = Vec::new();
    let Some(list) = find_child(taskbar, "TrayNotifyWnd")
        .and_then(|tray| find_child(tray, "SysPager"))
        .and_then(|pager| find_child(pager, "ToolbarWindow32"))
    else {
        return icons;
    };
    unsafe {
        let mut object = std::ptr::null_mut();
        // OBJID_CLIENT
        if AccessibleObjectFromWindow(list, 0xFFFF_FFFC, &IAccessible::IID, &mut object).is_err() {
            return icons;
        }
        let list = IAccessible::from_raw(object);
        for child in 1..=list.accChildCount().unwrap_or(0) {
            let (mut left, mut top, mut width, mut height) = (0, 0, 0, 0);
            let child = VARIANT::from(child);
            if list
                .accLocation(&mut left, &mut top, &mut width, &mut height, &child)
                .is_ok()
                && width > 0
                && height > 0
            {
                icons.push((left, top, left + width, top + height));
            }
        }
    }
    icons.sort_by_key(|bounds| (bounds.0, bounds.1));
    icons
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Widgets,
    Cluster,
    /// An app's notification icon, not one of the system's.
    TrayIcon,
}

fn classify(class: &str, id: &str) -> Option<Kind> {
    if id == "WidgetsButton" {
        return Some(Kind::Widgets);
    }
    if id == "NotifyItemIcon" {
        return Some(Kind::TrayIcon);
    }
    let cluster = class == "Taskbar.TaskListButtonAutomationPeer"
        || class == "ToggleButton"
        || matches!(
            id,
            "StartButton" | "SearchButton" | "TaskViewButton" | "SearchBoxButton"
        );
    cluster.then_some(Kind::Cluster)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explorer_buttons_are_classified() {
        assert_eq!(
            classify("Taskbar.TaskListButtonAutomationPeer", ""),
            Some(Kind::Cluster)
        );
        assert_eq!(classify("ToggleButton", "StartButton"), Some(Kind::Cluster));
        assert_eq!(
            classify("ToggleButton", "WidgetsButton"),
            Some(Kind::Widgets)
        );
        assert_eq!(classify("SystemTray.NormalButton", ""), None);
        assert_eq!(
            classify("SystemTray.NormalButton", "NotifyItemIcon"),
            Some(Kind::TrayIcon)
        );
        assert_eq!(classify("SystemTray.NormalButton", "SystemTrayIcon"), None);
        assert_eq!(classify("Taskbar.TaskbarFrameAutomationPeer", ""), None);
    }
}
