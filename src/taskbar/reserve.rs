//! Space reserved for the widgets inside the notification area, for the
//! experimental "inside the notification area" position.
//!
//! Windows 11 lays app buttons out only up to the notification area, so the
//! strip claims room the shell respects by adding blank, inert notification
//! icons and painting itself over them. The icons have no callback and no
//! tooltip and are promoted onto the taskbar through the per-icon
//! `NotifyIconSettings` entries Explorer reads. They are kept at the front of
//! the notification area; while they are not together on the taskbar the
//! strip is hidden rather than drawn anywhere else.
//!
//! The Windows 10 notification area (also restored by ExplorerPatcher) keeps
//! no such entries; there a helper process promotes the icons through
//! Explorer's private interface (see `promote.rs`).
//!
//! `Shell_NotifyIcon` and `Shell_NotifyIconGetRect` call into Explorer
//! synchronously, so they run on their own thread: the host thread shares
//! Explorer's input queue and must never wait on it.

use std::{
    ptr::null_mut,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicI32, AtomicIsize, AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

use windows_sys::{
    Win32::{
        Foundation::{ERROR_SUCCESS, HWND, LPARAM, LRESULT, RECT, WPARAM},
        System::{
            LibraryLoader::GetModuleHandleW,
            Registry::{
                HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_DWORD, RegCloseKey,
                RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
            },
        },
        UI::{
            Shell::{
                NIF_ICON, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, NOTIFYICONIDENTIFIER,
                Shell_NotifyIconGetRect, Shell_NotifyIconW,
            },
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyIcon, DispatchMessageW, GetMessageW, HICON,
                MSG, PostMessageW, RegisterClassW, RegisterWindowMessageW, SetTimer,
                TranslateMessage, WM_APP, WM_TIMER, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
            },
        },
    },
    core::GUID,
};

use super::obstacles::{self, Bounds};

/// First icon id; far above the ids `tray-icon` hands out.
const FIRST_ID: u32 = 0x4D42_0000;
/// Most icons the strip may claim.
const MAX_ICONS: usize = 16;
/// Width of a notification icon slot on Windows 11, in DIPs; a first guess
/// until a slot is measured.
const SLOT_GUESS: f32 = 28.0;
const TICK: u32 = 250;
const TIMER_TICK: usize = 1;
const WM_WAKE: u32 = WM_APP + 1;
/// How long a smaller strip must last before icons are given back, so a
/// label that changes width every minute does not reshuffle the tray.
const SHRINK_AFTER: Duration = Duration::from_secs(30);
/// Time for Explorer to place and promote new icons before their position
/// is judged.
const SETTLE: Duration = Duration::from_millis(2500);
/// Time Explorer's layout takes to settle after an icon moves, before a
/// measurement of the tray reflects it.
const LAYOUT: Duration = Duration::from_millis(300);
/// Times the icons are re-added to bring them to the front before they are
/// left where Explorer puts them, until the strip is hidden or Explorer
/// restarts.
const MAX_REORDERS: u32 = 3;
/// How often the promotion is re-applied, in case it was turned off.
const PROMOTE_EVERY: Duration = Duration::from_secs(5);
/// How often the promotion is re-applied while an icon is in the overflow.
const REPROMOTE: Duration = Duration::from_secs(1);
/// Least time between two Windows 10 promotion helpers.
const HELPER_EVERY: Duration = Duration::from_secs(10);
/// Failed helper runs before Windows 10 promotion stops until Explorer
/// restarts.
const MAX_HELPER_FAILURES: u32 = 3;

const SETTINGS_KEY: &str = r"Control Panel\NotifyIconSettings";

/// Pixels along the taskbar the strip asks for; 0 releases the icons.
static WANTED: AtomicI32 = AtomicI32::new(0);
/// Screen bounds of the reserved block while it is usable.
static LATEST: Mutex<Option<Bounds>> = Mutex::new(None);
/// The owner window and the ids currently added, for [`shutdown`].
static ADDED: Mutex<(isize, Vec<u32>)> = Mutex::new((0, Vec::new()));
static WINDOW: AtomicIsize = AtomicIsize::new(0);
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);
static STARTED: AtomicBool = AtomicBool::new(false);

/// Asks for `pixels` along the taskbar, or none.
pub(super) fn request(pixels: i32) {
    let pixels = pixels.max(0);
    if WANTED.swap(pixels, Ordering::SeqCst) == pixels {
        return;
    }
    if pixels > 0 && !STARTED.swap(true, Ordering::SeqCst) {
        let spawned = std::thread::Builder::new()
            .name("taskbar-reserve".into())
            .spawn(run);
        if spawned.is_err() {
            STARTED.store(false, Ordering::SeqCst);
        }
    }
    let window = WINDOW.load(Ordering::SeqCst);
    if window != 0 {
        unsafe { PostMessageW(window as HWND, WM_WAKE, 0, 0) };
    }
}

/// Screen bounds of the reserved block while its icons sit together on the
/// taskbar.
pub(super) fn latest() -> Option<Bounds> {
    LATEST.lock().ok().and_then(|latest| *latest)
}

/// Removes the icons before the process exits; Explorer would otherwise
/// keep blank slots until the mouse passes over them.
pub(crate) fn shutdown() {
    WANTED.store(0, Ordering::SeqCst);
    let (window, ids) = ADDED
        .lock()
        .map(|mut added| (added.0, std::mem::take(&mut added.1)))
        .unwrap_or_default();
    if window == 0 {
        return;
    }
    for id in &ids {
        delete_icon(window as HWND, *id);
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

struct Reserve {
    window: HWND,
    icon: HICON,
    /// Ids added, in order.
    ids: Vec<u32>,
    /// Width of one icon slot, once measured.
    pitch: Option<i32>,
    shrink_since: Option<Instant>,
    /// When icons were last added or removed.
    changed: Instant,
    /// The icons' bounds at the last tick, and since when they are there.
    rects: Vec<Bounds>,
    moved: Instant,
    promoted: Option<Instant>,
    /// `NotifyIconSettings` entries found for the icons at the last
    /// promotion.
    entries: usize,
    reorders: u32,
    /// Some icon is in the overflow.
    hidden: bool,
    /// A Windows 10 promotion helper still running, when the last one
    /// started and how many failed in a row.
    helper: Option<super::promote::Run>,
    helper_started: Option<Instant>,
    helper_failures: u32,
}

thread_local! {
    static RESERVE: std::cell::RefCell<Option<Reserve>> = const { std::cell::RefCell::new(None) };
}

fn run() {
    unsafe {
        let instance = GetModuleHandleW(null_mut());
        let class = wide("CodexMinibarTaskbarReserve");
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            ..std::mem::zeroed()
        });
        // A hidden top-level window: message-only windows miss the
        // TaskbarCreated broadcast.
        let window = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class.as_ptr(),
            null_mut(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            instance,
            null_mut(),
        );
        if window.is_null() {
            STARTED.store(false, Ordering::SeqCst);
            return;
        }
        let icon = blank_icon();
        RESERVE.with(|reserve| {
            *reserve.borrow_mut() = Some(Reserve {
                window,
                icon,
                ids: Vec::new(),
                pitch: None,
                shrink_since: None,
                changed: Instant::now(),
                rects: Vec::new(),
                moved: Instant::now(),
                promoted: None,
                entries: 0,
                reorders: 0,
                hidden: false,
                helper: None,
                helper_started: None,
                helper_failures: 0,
            })
        });
        TASKBAR_CREATED.store(
            RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
            Ordering::SeqCst,
        );
        if let Ok(mut added) = ADDED.lock() {
            added.0 = window as isize;
        }
        WINDOW.store(window as isize, Ordering::SeqCst);
        SetTimer(window, TIMER_TICK, TICK, None);
        with_reserve(Reserve::tick);
        let mut message = std::mem::zeroed::<MSG>();
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        if !icon.is_null() {
            DestroyIcon(icon);
        }
    }
}

fn with_reserve<R>(f: impl FnOnce(&mut Reserve) -> R) -> Option<R> {
    RESERVE.with(|reserve| {
        let mut reserve = reserve.try_borrow_mut().ok()?;
        reserve.as_mut().map(f)
    })
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_WAKE => {
            with_reserve(Reserve::tick);
            0
        }
        WM_TIMER if wparam == TIMER_TICK => {
            with_reserve(Reserve::tick);
            0
        }
        _ if message != 0 && message == TASKBAR_CREATED.load(Ordering::SeqCst) => {
            // Missed while busy, the lost icons are noticed on a later tick.
            with_reserve(Reserve::explorer_restarted);
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

impl Reserve {
    fn tick(&mut self) {
        let wanted = WANTED.load(Ordering::SeqCst);
        if wanted == 0 {
            self.reorders = 0;
            self.resize(0);
            publish(None);
            return;
        }
        let pitch = self.pitch.unwrap_or_else(guess_pitch).max(1);
        let target = ((wanted + pitch - 1) / pitch).clamp(1, MAX_ICONS as i32) as usize;
        let count = if target < self.ids.len() && self.pitch.is_some() {
            let since = *self.shrink_since.get_or_insert_with(Instant::now);
            if since.elapsed() >= SHRINK_AFTER {
                target
            } else {
                self.ids.len()
            }
        } else {
            self.shrink_since = None;
            target
        };
        self.resize(count);
        self.promote();
        self.judge();
    }

    fn explorer_restarted(&mut self) {
        // Explorer dropped every icon along with the old notification area.
        self.ids.clear();
        self.pitch = None;
        self.promoted = None;
        self.reorders = 0;
        self.helper_failures = 0;
        self.changed = Instant::now();
        self.sync_added();
        publish(None);
        self.tick();
    }

    /// Adds or removes icons at the end until there are `count`.
    fn resize(&mut self, count: usize) {
        if count == self.ids.len() {
            return;
        }
        while self.ids.len() > count {
            if let Some(id) = self.ids.pop() {
                delete_icon(self.window, id);
            }
        }
        while self.ids.len() < count {
            let id = FIRST_ID + self.ids.len() as u32;
            if !self.add_icon(id) {
                break;
            }
            self.ids.push(id);
            // Promote new icons right away.
            self.promoted = None;
        }
        self.changed = Instant::now();
        self.sync_added();
        obstacles::refresh();
    }

    fn add_icon(&self, id: u32) -> bool {
        unsafe {
            let mut data: NOTIFYICONDATAW = std::mem::zeroed();
            data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
            data.hWnd = self.window;
            data.uID = id;
            data.uFlags = NIF_ICON;
            data.hIcon = self.icon;
            Shell_NotifyIconW(NIM_ADD, &data) != 0
        }
    }

    fn sync_added(&self) {
        if let Ok(mut added) = ADDED.lock() {
            added.1 = self.ids.clone();
        }
    }

    fn promote(&mut self) {
        if classic_tray() {
            self.promote_classic();
            return;
        }
        if self.ids.is_empty() || self.promoted.is_some_and(|at| at.elapsed() < PROMOTE_EVERY) {
            return;
        }
        let ids = self.ids.clone();
        self.entries = promote_settings(|id| ids.contains(&id));
        // Explorer writes the entries a moment after an icon is added; try
        // again on the next tick until every one is there.
        if self.entries >= ids.len() {
            self.promoted = Some(Instant::now());
        }
    }

    /// Windows 10 has no entries to write; a helper process asks Explorer
    /// instead (see `promote.rs`). It runs only when icons were added or
    /// found hidden, at most every [`HELPER_EVERY`], and stops after
    /// [`MAX_HELPER_FAILURES`] failed runs.
    fn promote_classic(&mut self) {
        if let Some(helper) = self.helper.as_mut() {
            match helper.finished() {
                None => return,
                Some(true) => self.helper_failures = 0,
                Some(false) => self.helper_failures += 1,
            }
            self.helper = None;
        }
        let due = self.promoted.is_none()
            && self.helper_failures < MAX_HELPER_FAILURES
            && self
                .helper_started
                .is_none_or(|at| at.elapsed() >= HELPER_EVERY);
        if self.ids.is_empty() || !due {
            return;
        }
        self.helper = super::promote::Run::start(self.window as isize, &self.ids);
        self.helper_started = Some(Instant::now());
        self.promoted = Some(Instant::now());
    }

    /// Publishes the block wherever the icons sit together on the taskbar,
    /// following the tray as it shifts, and brings them back to the front or
    /// out of the overflow once a fresh measurement shows them misplaced.
    fn judge(&mut self) {
        let settled = self.changed.elapsed() >= SETTLE;
        let rects = self
            .ids
            .iter()
            .map(|id| icon_rect(self.window, *id))
            .collect::<Option<Vec<_>>>();
        let Some(mut rects) = rects else {
            if settled {
                // Lost without a TaskbarCreated we could see: start over.
                crate::logger::info("Reserved tray icons have no place; adding them again");
                publish(None);
                self.readd();
            }
            return;
        };
        rects.sort_by_key(|bounds| (bounds.0, bounds.1));
        if rects != self.rects {
            self.rects = rects.clone();
            self.moved = Instant::now();
        }
        let on_taskbar = taskbar_bounds().is_some_and(|taskbar| {
            rects.iter().all(|rect| {
                rect.0 >= taskbar.0
                    && rect.1 >= taskbar.1
                    && rect.2 <= taskbar.2
                    && rect.3 <= taskbar.3
            })
        });
        let together = rects
            .windows(2)
            .all(|pair| (pair[1].0 - pair[0].2).abs() <= 2 && pair[1].1 == pair[0].1);
        if on_taskbar && let Some(first) = rects.first() {
            // Only a slot on the taskbar tells its width; the overflow
            // flyout lays icons out on its own grid.
            let pitch = first.2 - first.0;
            if pitch > 0 {
                self.pitch = Some(pitch);
            }
        }
        // Explorer reports the icons' current place directly, so the block
        // moves with the tray at once; only their order needs UI Automation.
        if on_taskbar && together {
            publish(Some(union(&rects)));
        } else if settled {
            publish(None);
        }
        let fresh = obstacles::tray_icons().filter(|(measured, _)| {
            *measured >= self.changed + SETTLE && *measured >= self.moved + LAYOUT
        });
        let Some((_, tray)) = fresh else {
            if settled {
                obstacles::refresh();
            }
            return;
        };
        let listed = |rect: &Bounds| tray.iter().any(|other| near(rect, other));
        if !on_taskbar || !rects.iter().all(listed) {
            // Moved to the overflow: promote them again, which brings them
            // back onto the taskbar.
            if !self.hidden {
                crate::logger::info(format!(
                    "Reserved tray icons hidden: ours={:?} tray={:?} entries={}",
                    rects.iter().map(|rect| rect.0).collect::<Vec<_>>(),
                    tray.iter().map(|rect| rect.0).collect::<Vec<_>>(),
                    self.entries,
                ));
            }
            self.hidden = true;
            if self.promoted.is_some_and(|at| at.elapsed() >= REPROMOTE) {
                self.promoted = None;
            }
            return;
        }
        self.hidden = false;
        let leading = together
            && tray.len() >= rects.len()
            && rects
                .iter()
                .zip(&tray)
                .all(|(ours, other)| near(ours, other));
        if leading {
            if self.reorders > 0 {
                crate::logger::info("Reserved tray icons lead the notification area");
            }
            self.reorders = 0;
            return;
        }
        if self.reorders >= MAX_REORDERS {
            // Explorer keeps placing them elsewhere: stay where they are.
            return;
        }
        crate::logger::info(format!(
            "Reserved tray icons misplaced: ours={:?} tray={:?}",
            rects.iter().map(|rect| rect.0).collect::<Vec<_>>(),
            tray.iter().map(|rect| rect.0).collect::<Vec<_>>(),
        ));
        self.reorders += 1;
        self.readd();
    }

    /// Adds the icons again for Explorer to place anew.
    fn readd(&mut self) {
        let count = self.ids.len();
        self.resize(0);
        self.resize(count);
    }
}

/// Screen bounds of the primary taskbar.
fn taskbar_bounds() -> Option<Bounds> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowRect};
    unsafe {
        let taskbar = FindWindowW(wide("Shell_TrayWnd").as_ptr(), null_mut());
        if taskbar.is_null() {
            return None;
        }
        let mut rect = std::mem::zeroed::<RECT>();
        (GetWindowRect(taskbar, &mut rect) != 0).then_some((
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
        ))
    }
}

/// The same button, measured two ways that may round differently.
fn near(a: &Bounds, b: &Bounds) -> bool {
    (a.0 - b.0).abs() <= 1
        && (a.1 - b.1).abs() <= 1
        && (a.2 - b.2).abs() <= 1
        && (a.3 - b.3).abs() <= 1
}

/// Width of a notification icon slot on the Windows 10 taskbar, in DIPs.
const CLASSIC_SLOT_GUESS: f32 = 24.0;

/// Whether Explorer runs the Windows 10 notification area: a Win32 toolbar
/// inside `SysPager` rather than the XAML one.
pub(super) fn classic_tray() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowExW, FindWindowW, IsWindowVisible,
    };
    unsafe {
        let taskbar = FindWindowW(wide("Shell_TrayWnd").as_ptr(), null_mut());
        if taskbar.is_null() {
            return false;
        }
        let tray = FindWindowExW(
            taskbar,
            null_mut(),
            wide("TrayNotifyWnd").as_ptr(),
            null_mut(),
        );
        if tray.is_null() {
            return false;
        }
        let pager = FindWindowExW(tray, null_mut(), wide("SysPager").as_ptr(), null_mut());
        if pager.is_null() {
            return false;
        }
        // Windows 11 keeps an empty SysPager; only the Windows 10 one holds
        // the icon toolbar.
        let icons = FindWindowExW(
            pager,
            null_mut(),
            wide("ToolbarWindow32").as_ptr(),
            null_mut(),
        );
        !icons.is_null() && IsWindowVisible(icons) != 0
    }
}

/// One slot at the taskbar's DPI.
fn guess_pitch() -> i32 {
    use windows_sys::Win32::UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::FindWindowW};
    let taskbar = unsafe { FindWindowW(wide("Shell_TrayWnd").as_ptr(), null_mut()) };
    let dpi = if taskbar.is_null() {
        0
    } else {
        unsafe { GetDpiForWindow(taskbar) }
    };
    let scale = if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 };
    let slot = if classic_tray() {
        CLASSIC_SLOT_GUESS
    } else {
        SLOT_GUESS
    };
    (slot * scale).round() as i32
}

/// An invisible 32×32 icon. Its pixels are not fully transparent: the
/// Windows 11 tray keeps showing whatever image a recycled slot held before
/// when an icon has nothing to draw, so each pixel keeps 1/255 alpha.
fn blank_icon() -> HICON {
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
        DeleteObject,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, ICONINFO};
    const SIZE: i32 = 32;
    unsafe {
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = SIZE;
        info.bmiHeader.biHeight = -SIZE;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut bits = null_mut();
        let color = CreateDIBSection(null_mut(), &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if color.is_null() || bits.is_null() {
            return null_mut();
        }
        // Premultiplied BGRA: black at 1/255 alpha.
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u32>(), (SIZE * SIZE) as usize);
        pixels.fill(0x0100_0000);
        let mask_bits = vec![0u8; (SIZE * SIZE / 8) as usize];
        let mask = CreateBitmap(SIZE, SIZE, 1, 1, mask_bits.as_ptr().cast());
        let icon = CreateIconIndirect(&ICONINFO {
            fIcon: 1,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        });
        DeleteObject(mask);
        DeleteObject(color);
        icon
    }
}

fn publish(bounds: Option<Bounds>) {
    if let Ok(mut latest) = LATEST.lock() {
        *latest = bounds;
    }
}

fn union(rects: &[Bounds]) -> Bounds {
    rects.iter().fold(rects[0], |a, b| {
        (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
    })
}

fn delete_icon(window: HWND, id: u32) {
    unsafe {
        let mut data: NOTIFYICONDATAW = std::mem::zeroed();
        data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = window;
        data.uID = id;
        Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

fn icon_rect(window: HWND, id: u32) -> Option<Bounds> {
    unsafe {
        let identifier = NOTIFYICONIDENTIFIER {
            cbSize: size_of::<NOTIFYICONIDENTIFIER>() as u32,
            hWnd: window,
            uID: id,
            guidItem: GUID::from_u128(0),
        };
        let mut rect = std::mem::zeroed::<RECT>();
        if Shell_NotifyIconGetRect(&identifier, &mut rect) < 0 {
            return None;
        }
        (rect.right > rect.left && rect.bottom > rect.top).then_some((
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
        ))
    }
}

/// Promotes this executable's `NotifyIconSettings` entries whose icon id
/// matches onto the taskbar; returns how many matched.
///
/// The entries are never deleted: Explorer creates them once per session
/// and does not write them again for an icon it already knows, so a deleted
/// entry leaves its icon in the overflow until Explorer restarts. The fixed
/// ids keep reusing the same few entries instead.
fn promote_settings(matches: impl Fn(u32) -> bool) -> usize {
    let Some(exe) = std::env::current_exe()
        .ok()
        .map(|path| path.to_string_lossy().to_lowercase())
    else {
        return 0;
    };
    let mut matched = 0;
    unsafe {
        let mut root: HKEY = null_mut();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            wide(SETTINGS_KEY).as_ptr(),
            0,
            KEY_READ,
            &mut root,
        ) != ERROR_SUCCESS
        {
            return 0;
        }
        let mut index = 0;
        loop {
            let mut name = [0u16; 256];
            let mut length = name.len() as u32;
            if RegEnumKeyExW(
                root,
                index,
                name.as_mut_ptr(),
                &mut length,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
            ) != ERROR_SUCCESS
            {
                break;
            }
            index += 1;
            let name = String::from_utf16_lossy(&name[..length as usize]);
            let mut entry: HKEY = null_mut();
            if RegOpenKeyExW(
                root,
                wide(&name).as_ptr(),
                0,
                KEY_READ | KEY_WRITE,
                &mut entry,
            ) != ERROR_SUCCESS
            {
                continue;
            }
            let ours = read_dword(entry, "UID").is_some_and(&matches)
                && read_string(entry, "ExecutablePath")
                    .is_some_and(|path| same_executable(&path, &exe));
            if ours {
                matched += 1;
                if read_dword(entry, "IsPromoted") != Some(1) {
                    let one = 1u32.to_le_bytes();
                    RegSetValueExW(
                        entry,
                        wide("IsPromoted").as_ptr(),
                        0,
                        REG_DWORD,
                        one.as_ptr(),
                        one.len() as u32,
                    );
                }
            }
            RegCloseKey(entry);
        }
        RegCloseKey(root);
    }
    matched
}

/// Explorer stores paths under known folders as `{FOLDERID}\rest`.
fn same_executable(stored: &str, exe: &str) -> bool {
    let stored = stored.to_lowercase();
    match stored
        .strip_prefix('{')
        .and_then(|rest| rest.split_once('}'))
    {
        Some((_, rest)) => exe.ends_with(rest),
        None => stored == exe,
    }
}

unsafe fn read_dword(key: HKEY, name: &str) -> Option<u32> {
    let mut value = 0u32;
    let mut size = size_of::<u32>() as u32;
    let mut kind = 0;
    let status = unsafe {
        RegQueryValueExW(
            key,
            wide(name).as_ptr(),
            null_mut(),
            &mut kind,
            (&mut value as *mut u32).cast(),
            &mut size,
        )
    };
    (status == ERROR_SUCCESS && kind == REG_DWORD).then_some(value)
}

unsafe fn read_string(key: HKEY, name: &str) -> Option<String> {
    let mut buffer = [0u16; 1024];
    let mut size = (buffer.len() * 2) as u32;
    let status = unsafe {
        RegQueryValueExW(
            key,
            wide(name).as_ptr(),
            null_mut(),
            null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let length = (size as usize / 2).min(buffer.len());
    let text = String::from_utf16_lossy(&buffer[..length]);
    Some(text.trim_end_matches('\0').to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_folder_paths_match_the_executable() {
        let exe = r"c:\users\me\appdata\local\codex minibar\codex-minibar.exe";
        assert!(same_executable(
            r"{F1B32785-6FBA-4FCF-9D55-7B8E7F157091}\Codex Minibar\codex-minibar.exe",
            exe
        ));
        assert!(same_executable(
            r"C:\Users\me\AppData\Local\Codex Minibar\codex-minibar.exe",
            exe
        ));
        assert!(!same_executable(r"C:\Other\codex-minibar.exe", exe));
    }
}
