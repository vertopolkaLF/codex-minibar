//! The taskbar host thread: a layered child window inside `Shell_TrayWnd`.
//!
//! A message-only controller window owns the timers and wakes on published
//! messages; the visible strip is created, re-parented after an Explorer
//! restart, and destroyed as the settings require. Window procedures only
//! borrow the host state for posted messages (mouse, timers, wake-ups), so
//! sent messages that Win32 calls generate re-enter without a borrow.

use std::{
    cell::RefCell,
    collections::HashMap,
    ptr::null_mut,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicIsize, Ordering},
    },
    time::{Duration, Instant},
};

use chrono::Utc;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM},
    Graphics::Gdi::{
        AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
        CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC,
        HBITMAP, HDC, HGDIOBJ, MapWindowPoints, ReleaseDC, SelectObject,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        HiDpi::GetDpiForWindow,
        Input::KeyboardAndMouse::{
            ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
        },
        WindowsAndMessaging::{
            AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
            DestroyWindow, DispatchMessageW, FindWindowExW, FindWindowW, GW_CHILD, GWL_STYLE,
            GetClientRect, GetCursorPos, GetMessageW, GetWindow, GetWindowRect, HMENU,
            HWND_MESSAGE, HWND_TOP, IDC_ARROW, IsWindow, KillTimer, LoadCursorW, MF_GRAYED,
            MF_SEPARATOR, MF_STRING, MSG, PostMessageW, RegisterClassW, SW_SHOWNA, SWP_NOACTIVATE,
            SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow, SetParent, SetTimer,
            SetWindowLongPtrW, SetWindowPos, ShowWindow, TPM_BOTTOMALIGN, TPM_LEFTALIGN,
            TPM_RETURNCMD, TPM_RIGHTALIGN, TPM_RIGHTBUTTON, TPM_TOPALIGN, TrackPopupMenuEx,
            TranslateMessage, ULW_ALPHA, UpdateLayeredWindow, WM_APP, WM_LBUTTONDOWN, WM_LBUTTONUP,
            WM_MOUSEMOVE, WM_RBUTTONUP, WM_TIMER, WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS,
            WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
            WS_POPUP,
        },
    },
};

use super::{
    Action, Config, WIDGET_GAP,
    model::{self, Appearance, WidgetView},
    obstacles::Obstacles,
    render::{CARD_MARGIN, Item, Metrics, Renderer},
};
use crate::{limits::ProviderLimits, settings::TaskbarAlignment};

pub(super) enum Message {
    Config(Config),
    Limits(ProviderLimits),
}

const WM_WAKE: u32 = WM_APP + 1;
const WM_MOUSELEAVE: u32 = 0x02A3;
const WM_MOUSEWHEEL: u32 = 0x020A;
/// One wheel notch.
const WHEEL_DELTA: i32 = 120;
const TIMER_POLL: usize = 1;
const TIMER_FRAME: usize = 2;
const TIMER_CARD: usize = 3;
/// Hover time before the card opens, like a taskbar thumbnail.
const CARD_DELAY: u32 = 450;
const CARD_GAP: f32 = 8.0;
const CARD_RISE: f32 = 8.0;
const POLL: u32 = 500;
const FRAME: u32 = 15;

const MENU_OPEN: usize = 1;
const MENU_SETTINGS: usize = 2;
const MENU_HIDE: usize = 3;
const MENU_EDIT: usize = 4;
const MENU_UPDATE: usize = 5;
const MENU_APP_SETTINGS: usize = 6;
const MENU_EXIT: usize = 7;

/// Space between the strip and its anchor, in DIPs.
const TRAY_MARGIN: f32 = 2.0;
/// Space kept inside each end of the reserved tray block, in DIPs.
const RESERVE_PADDING: f32 = 2.0;
const START_MARGIN: f32 = 6.0;

const HOVER_IN: Duration = Duration::from_millis(100);
const HOVER_OUT: Duration = Duration::from_millis(167);
const PRESS: Duration = Duration::from_millis(50);
const RESIZE: Duration = Duration::from_millis(250);
const FADE: Duration = Duration::from_millis(167);
const PROGRESS: Duration = Duration::from_millis(500);
const CARD_IN: Duration = Duration::from_millis(167);
const CARD_OUT: Duration = Duration::from_millis(100);

static QUEUE: Mutex<Vec<Message>> = Mutex::new(Vec::new());
static CONTROLLER: AtomicIsize = AtomicIsize::new(0);
static STARTED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
}

pub(super) fn publish(message: Message) {
    let start = matches!(&message, Message::Config(config) if config.taskbar.enabled);
    if let Ok(mut queue) = QUEUE.lock() {
        // Only the newest value of each kind matters.
        queue.retain(|queued| {
            !matches!(
                (queued, &message),
                (Message::Config(_), Message::Config(_)) | (Message::Limits(_), Message::Limits(_))
            )
        });
        queue.push(message);
    }
    if start && !STARTED.swap(true, Ordering::SeqCst) {
        let spawned = std::thread::Builder::new()
            .name("taskbar-widgets".into())
            .spawn(run);
        if let Err(error) = spawned {
            STARTED.store(false, Ordering::SeqCst);
            trace(format!("Could not start the taskbar widgets: {error}"));
        }
    }
    let controller = CONTROLLER.load(Ordering::SeqCst);
    if controller != 0 {
        unsafe { PostMessageW(controller as HWND, WM_WAKE, 0, 0) };
    }
}

/// Logs to the app log, and to stderr for the visual check example.
fn trace(message: impl AsRef<str>) {
    #[cfg(debug_assertions)]
    eprintln!("taskbar: {}", message.as_ref());
    crate::logger::info(message);
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

const CONTROLLER_CLASS: &str = "CodexMinibarTaskbarController";
const WIDGET_CLASS: &str = "CodexMinibarTaskbarWidgets";
const CARD_CLASS: &str = "CodexMinibarTaskbarCard";

fn run() {
    unsafe {
        let instance = GetModuleHandleW(null_mut());
        let controller_class = wide(CONTROLLER_CLASS);
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(controller_proc),
            hInstance: instance,
            lpszClassName: controller_class.as_ptr(),
            ..std::mem::zeroed()
        });
        let widget_class = wide(WIDGET_CLASS);
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(widget_proc),
            hInstance: instance,
            lpszClassName: widget_class.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            ..std::mem::zeroed()
        });
        let card_class = wide(CARD_CLASS);
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(DefWindowProcW),
            hInstance: instance,
            lpszClassName: card_class.as_ptr(),
            ..std::mem::zeroed()
        });
        let controller = CreateWindowExW(
            0,
            controller_class.as_ptr(),
            null_mut(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            null_mut(),
            instance,
            null_mut(),
        );
        if controller.is_null() {
            trace("Could not create the taskbar widget controller");
            STARTED.store(false, Ordering::SeqCst);
            return;
        }
        HOST.with(|host| *host.borrow_mut() = Some(Host::new(controller)));
        CONTROLLER.store(controller as isize, Ordering::SeqCst);
        SetTimer(controller, TIMER_POLL, POLL, None);
        with_host(|host| host.drain());

        let mut message = std::mem::zeroed::<MSG>();
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

fn with_host<R>(f: impl FnOnce(&mut Host) -> R) -> Option<R> {
    HOST.with(|host| {
        let mut host = host.try_borrow_mut().ok()?;
        host.as_mut().map(f)
    })
}

unsafe extern "system" fn controller_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_WAKE => {
            with_host(Host::drain);
            0
        }
        WM_TIMER if wparam == TIMER_POLL => {
            with_host(Host::poll);
            0
        }
        WM_TIMER if wparam == TIMER_FRAME => {
            with_host(Host::frame);
            0
        }
        WM_TIMER if wparam == TIMER_CARD => {
            unsafe { KillTimer(hwnd, TIMER_CARD) };
            with_host(Host::open_card);
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn point_from(lparam: LPARAM) -> (i32, i32) {
    (
        (lparam & 0xFFFF) as u16 as i16 as i32,
        ((lparam >> 16) & 0xFFFF) as u16 as i16 as i32,
    )
}

unsafe extern "system" fn widget_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_MOUSEMOVE => {
            let point = point_from(lparam);
            let track = with_host(|host| host.mouse_move(point)).unwrap_or(false);
            if track {
                let mut event = TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                unsafe { TrackMouseEvent(&mut event) };
            }
            0
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam >> 16) & 0xFFFF) as u16 as i16;
            with_host(|host| host.scroll(i32::from(delta)));
            0
        }
        WM_MOUSELEAVE => {
            with_host(Host::mouse_leave);
            0
        }
        WM_LBUTTONDOWN => {
            let point = point_from(lparam);
            if with_host(|host| host.press(point)).unwrap_or(false) {
                unsafe { SetCapture(hwnd) };
            }
            0
        }
        WM_LBUTTONUP => {
            let point = point_from(lparam);
            unsafe { ReleaseCapture() };
            if let Some(Some(action)) = with_host(|host| host.release(point)) {
                super::post_action(action);
            }
            0
        }
        WM_RBUTTONUP => {
            let point = point_from(lparam);
            with_host(Host::close_card);
            let target = with_host(|host| MenuTarget {
                page: host.click_page(),
                light: host.appearance.light,
                edge: host.geometry.map_or(Edge::Bottom, |geometry| geometry.edge),
                widget: host.hit(point).and_then(|id| {
                    host.slots
                        .iter()
                        .find(|slot| slot.view.id == id)
                        .map(|slot| (id, slot.view.provider))
                }),
            });
            if let Some(target) = target
                && let Some(action) = context_menu(hwnd, target)
            {
                super::post_action(action);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

/// What a right click landed on.
struct MenuTarget {
    page: crate::settings::TaskbarClick,
    light: bool,
    edge: Edge,
    /// The widget under the pointer and its provider.
    widget: Option<(String, Option<crate::instances::ProviderId>)>,
}

/// The tray icon's menu with the widget's own commands on top, as a native
/// menu themed to match the taskbar rather than the app.
fn context_menu(owner: HWND, target: MenuTarget) -> Option<Action> {
    unsafe {
        theme_menus(owner, target.light);
        let menu: HMENU = CreatePopupMenu();
        if menu.is_null() {
            return None;
        }
        let append = |flags, id: usize, text: &str| {
            let text = wide(text);
            AppendMenuW(menu, flags, id, text.as_ptr());
        };
        let header = format!("Codex Minibar - {}", env!("CARGO_PKG_VERSION"));
        append(MF_STRING | MF_GRAYED, 0, &header);
        AppendMenuW(menu, MF_SEPARATOR, 0, null_mut());
        if target.widget.is_some() {
            append(MF_STRING, MENU_OPEN, crate::i18n::tr("taskbar-menu-open"));
            append(MF_STRING, MENU_EDIT, crate::i18n::tr("taskbar-menu-edit"));
        }
        append(
            MF_STRING,
            MENU_SETTINGS,
            crate::i18n::tr("taskbar-menu-settings"),
        );
        append(MF_STRING, MENU_HIDE, crate::i18n::tr("taskbar-menu-hide"));
        AppendMenuW(menu, MF_SEPARATOR, 0, null_mut());
        if super::update_available() {
            append(
                MF_STRING,
                MENU_UPDATE,
                crate::i18n::tr("update-available-67fd3a"),
            );
        }
        append(MF_STRING, MENU_APP_SETTINGS, crate::i18n::tr("settings"));
        append(MF_STRING, MENU_EXIT, crate::i18n::tr("exit"));
        let mut cursor = POINT { x: 0, y: 0 };
        GetCursorPos(&mut cursor);
        // The menu dismisses on an outside click only while its owner's
        // top-level window is in the foreground.
        SetForegroundWindow(owner);
        // Opens away from the taskbar, like Explorer's own menus.
        let align = match target.edge {
            Edge::Bottom => TPM_BOTTOMALIGN,
            Edge::Top => TPM_TOPALIGN,
            Edge::Left => TPM_LEFTALIGN,
            Edge::Right => TPM_RIGHTALIGN,
        };
        let command = TrackPopupMenuEx(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | align,
            cursor.x,
            cursor.y,
            owner,
            null_mut(),
        );
        DestroyMenu(menu);
        let (id, provider) = target.widget.unzip();
        match command as usize {
            MENU_OPEN => Some(Action::Open {
                provider: provider.flatten(),
                anchor: (cursor.x, cursor.y),
                page: target.page,
            }),
            MENU_EDIT => id.map(Action::Edit),
            MENU_SETTINGS => Some(Action::Settings),
            MENU_HIDE => Some(Action::Hide),
            MENU_UPDATE => Some(Action::Update),
            MENU_APP_SETTINGS => Some(Action::AppSettings),
            MENU_EXIT => Some(Action::Exit),
            _ => None,
        }
    }
}

/// Dark or light Win32 menus through uxtheme's unnamed exports (Windows 10
/// 1903+). Missing exports leave the system default.
fn theme_menus(owner: HWND, light: bool) {
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    type SetPreferredAppMode = unsafe extern "system" fn(i32) -> i32;
    type AllowDarkModeForWindow = unsafe extern "system" fn(HWND, i32) -> i32;
    type FlushMenuThemes = unsafe extern "system" fn();
    unsafe {
        let module = LoadLibraryW(wide("uxtheme.dll").as_ptr());
        if module.is_null() {
            return;
        }
        let ordinal = |number: usize| GetProcAddress(module, number as *const u8);
        if let Some(allow) = ordinal(133) {
            let allow: AllowDarkModeForWindow = std::mem::transmute(allow);
            allow(owner, i32::from(!light));
        }
        if let Some(mode) = ordinal(135) {
            let mode: SetPreferredAppMode = std::mem::transmute(mode);
            // ForceDark = 2, ForceLight = 3.
            mode(if light { 3 } else { 2 });
        }
        if let Some(flush) = ordinal(136) {
            let flush: FlushMenuThemes = std::mem::transmute(flush);
            flush();
        }
    }
}

/// Eased value moving toward a target.
#[derive(Clone, Copy, Debug)]
struct Tween {
    from: f32,
    to: f32,
    start: Instant,
    duration: Duration,
}

impl Tween {
    fn at(value: f32) -> Self {
        Self {
            from: value,
            to: value,
            start: Instant::now(),
            duration: Duration::ZERO,
        }
    }

    fn value(&self, now: Instant) -> f32 {
        if self.duration.is_zero() {
            return self.to;
        }
        let t = (now.saturating_duration_since(self.start).as_secs_f32()
            / self.duration.as_secs_f32())
        .clamp(0.0, 1.0);
        // Fluent decelerate: fast start, gentle landing.
        let eased = 1.0 - (1.0 - t).powi(3);
        self.from + (self.to - self.from) * eased
    }

    fn done(&self, now: Instant) -> bool {
        self.duration.is_zero() || now.saturating_duration_since(self.start) >= self.duration
    }

    fn set(&mut self, to: f32, now: Instant, duration: Duration, animate: bool) {
        if (to - self.to).abs() < 0.0005 {
            return;
        }
        if animate {
            self.from = self.value(now);
            self.start = now;
            self.duration = duration;
        } else {
            self.from = to;
            self.duration = Duration::ZERO;
        }
        self.to = to;
    }
}

struct Slot {
    view: WidgetView,
    /// Measured width in DIPs.
    full: f32,
    width: Tween,
    opacity: Tween,
    hover: Tween,
    press: Tween,
    progress: Vec<Option<Tween>>,
    leaving: bool,
}

impl Slot {
    fn tweens(&self) -> impl Iterator<Item = &Tween> {
        [&self.width, &self.opacity, &self.hover, &self.press]
            .into_iter()
            .chain(self.progress.iter().flatten())
    }
}

/// The screen edge the taskbar is docked to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Edge {
    Bottom,
    Top,
    Left,
    Right,
}

impl Edge {
    fn vertical(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

/// Where the strip lives. "Along" is the taskbar's long axis: x on a
/// horizontal taskbar, y on a vertical one.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Geometry {
    taskbar: HWND,
    scale: f32,
    edge: Edge,
    /// Taskbar client extent across its long axis, in pixels.
    thickness: i32,
    /// End (tray) or start (start) of the strip along the taskbar, in
    /// taskbar client pixels.
    anchor: i32,
    /// Where the Start button ends along the taskbar, on shells that have
    /// a Win32 one.
    start_button: Option<i32>,
    alignment: TaskbarAlignment,
    /// Screen position of the taskbar's client origin.
    origin: (i32, i32),
}

impl Geometry {
    /// A screen rectangle's extent along the taskbar, in client pixels.
    fn along(self, bounds: super::obstacles::Bounds) -> (i32, i32) {
        if self.edge.vertical() {
            (bounds.1 - self.origin.1, bounds.3 - self.origin.1)
        } else {
            (bounds.0 - self.origin.0, bounds.2 - self.origin.0)
        }
    }
}

struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    size: (i32, i32),
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}

impl Surface {
    fn new(size: (i32, i32)) -> Option<Self> {
        unsafe {
            let dc = CreateCompatibleDC(null_mut());
            if dc.is_null() {
                return None;
            }
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = size.0;
            info.bmiHeader.biHeight = -size.1;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB;
            let mut bits = null_mut();
            let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
            if bitmap.is_null() {
                DeleteDC(dc);
                return None;
            }
            let previous = SelectObject(dc, bitmap);
            Some(Self {
                dc,
                bitmap,
                previous,
                size,
            })
        }
    }
}

/// The hover card: a topmost layered popup above the hovered widget.
struct Card {
    window: HWND,
    surface: Option<Surface>,
    id: String,
    opacity: Tween,
    /// Screen position and pixel size of the window.
    rect: (i32, i32, i32, i32),
    closing: bool,
}

impl Drop for Card {
    fn drop(&mut self) {
        self.surface = None;
        unsafe { DestroyWindow(self.window) };
    }
}

struct Host {
    card: Option<Card>,
    /// Explorer's buttons as last measured.
    obstacles: Option<Obstacles>,
    /// Screen bounds of the tray block reserved for the strip, as last read.
    reserved_screen: Option<super::obstacles::Bounds>,
    /// The reserved block's extent along the taskbar, in client pixels.
    reserved: Option<(i32, i32)>,
    /// The last reserved block, where widgets fade out while it is gone.
    block: Option<(i32, i32)>,
    /// Start of a start-aligned strip, past the widgets button.
    leading: i32,
    /// Since when a first show waits for Explorer's buttons to be measured.
    awaiting_obstacles: Option<Instant>,
    controller: HWND,
    window: HWND,
    config: Option<Config>,
    limits: ProviderLimits,
    renderer: Option<Renderer>,
    surface: Option<Surface>,
    geometry: Option<Geometry>,
    appearance: Appearance,
    slots: Vec<Slot>,
    hover: Option<String>,
    /// Every widget the wheel cycles through, in order, while cycling.
    cycle_ids: Vec<String>,
    /// The widget shown while cycling.
    cycled: Option<String>,
    /// Wheel travel not yet turned into a step.
    wheel: i32,
    pressed: Option<String>,
    tracking: bool,
    frame_timer: bool,
    minute: i64,
    /// Widget id, offset and length along the taskbar in DIPs of the last
    /// frame, for hit testing.
    hits: Vec<(String, f32, f32)>,
    placed: Option<(i32, i32, i32, i32)>,
    shown: bool,
}

impl Host {
    fn new(controller: HWND) -> Self {
        Self {
            card: None,
            obstacles: None,
            reserved_screen: None,
            reserved: None,
            block: None,
            leading: 0,
            awaiting_obstacles: None,
            controller,
            window: null_mut(),
            config: None,
            limits: ProviderLimits::default(),
            renderer: None,
            surface: None,
            geometry: None,
            appearance: current_appearance(false),
            slots: Vec::new(),
            hover: None,
            cycle_ids: Vec::new(),
            cycled: None,
            wheel: 0,
            pressed: None,
            tracking: false,
            frame_timer: false,
            minute: Utc::now().timestamp() / 60,
            hits: Vec::new(),
            placed: None,
            shown: false,
        }
    }

    fn drain(&mut self) {
        let messages = QUEUE
            .lock()
            .map(|mut queue| std::mem::take(&mut *queue))
            .unwrap_or_default();
        if messages.is_empty() {
            return;
        }
        for message in messages {
            match message {
                Message::Config(config) => self.config = Some(config),
                Message::Limits(limits) => self.limits = limits,
            }
        }
        self.reconcile();
    }

    fn poll(&mut self) {
        let geometry = self
            .config
            .as_ref()
            .and_then(|config| locate(config.taskbar.alignment));
        let appearance = current_appearance(
            self.config
                .as_ref()
                .is_some_and(|config| config.colored_icons),
        );
        let minute = Utc::now().timestamp() / 60;
        let obstacles = super::obstacles::latest();
        let reserved = super::reserve::latest();
        let window_lost = !self.window.is_null() && unsafe { IsWindow(self.window) } == 0;
        if window_lost {
            // Explorer restarted and destroyed the child with its taskbar.
            self.window = null_mut();
            self.placed = None;
            self.shown = false;
        }
        if geometry != self.geometry
            || appearance != self.appearance
            || minute != self.minute
            || obstacles != self.obstacles
            || reserved != self.reserved_screen
            || window_lost
            || self
                .awaiting_obstacles
                .is_some_and(|since| since.elapsed() >= OBSTACLE_WAIT)
        {
            self.minute = minute;
            self.reconcile();
        } else if !self.window.is_null() {
            self.keep_on_top();
        }
    }

    fn animate(&self) -> bool {
        crate::popup::animations_enabled() && crate::popup::system_animations_enabled()
    }

    /// Brings the strip, its slots and the window in line with the config.
    fn reconcile(&mut self) {
        let now = Instant::now();
        let colored = self
            .config
            .as_ref()
            .is_some_and(|config| config.colored_icons);
        self.appearance = current_appearance(colored);
        let enabled = self
            .config
            .as_ref()
            .is_some_and(|config| config.taskbar.enabled);
        self.geometry = if enabled {
            self.config
                .as_ref()
                .and_then(|config| locate(config.taskbar.alignment))
        } else {
            None
        };
        let views = match (&self.config, self.geometry) {
            (Some(config), Some(_)) if config.taskbar.enabled => model::resolve_visible(
                &config.taskbar.widgets,
                &crate::instances::published_enabled_providers(),
                &self.limits,
                self.appearance,
            ),
            _ => Vec::new(),
        };
        let views = self.cycle(views);
        #[cfg(debug_assertions)]
        trace(format!(
            "reconcile: enabled={enabled} geometry={:?} views={}",
            self.geometry,
            views.len()
        ));
        let Some(geometry) = self.geometry else {
            self.teardown();
            return;
        };
        if !self.window.is_null()
            && unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetParent(self.window) }
                != geometry.taskbar
        {
            // A new taskbar after an Explorer restart: start over on it.
            self.teardown();
        }
        if views.is_empty() && self.window.is_null() {
            super::reserve::request(0);
            self.reserved = None;
            self.slots.clear();
            return;
        }
        if self.renderer.is_none() {
            match Renderer::new() {
                Ok(renderer) => self.renderer = Some(renderer),
                Err(error) => {
                    trace(format!("Taskbar widget renderer failed: {error}"));
                    self.teardown();
                    return;
                }
            }
        }
        let metrics = metrics_for(geometry);
        let animate = self.animate() && self.shown;
        super::obstacles::set_wanted(!views.is_empty());
        self.obstacles = super::obstacles::latest();
        // A first show waits briefly for the measurement instead of flashing
        // over Explorer's buttons.
        if self.window.is_null() && self.obstacles.is_none() && !views.is_empty() {
            let since = *self.awaiting_obstacles.get_or_insert_with(Instant::now);
            if since.elapsed() < OBSTACLE_WAIT {
                return;
            }
        }
        self.awaiting_obstacles = None;
        self.reserve_space(geometry, &views, metrics);
        let (leading, available) = match (geometry.alignment, self.reserved) {
            // Inside the tray the strip only ever shows over its block, and
            // fades out where it was while the block is gone.
            (TaskbarAlignment::InTray, Some((start, end))) => (
                0,
                ((end - start) as f32 - 2.0 * RESERVE_PADDING * metrics.scale).max(0.0),
            ),
            (TaskbarAlignment::InTray, None) => (0, 0.0),
            _ => free_span(geometry, self.obstacles),
        };
        self.leading = leading;
        let (views, widths) = match self.renderer.as_mut() {
            Some(renderer) => fit(views, available / metrics.scale, |view| {
                renderer.measure(view, metrics).unwrap_or(0.0)
            }),
            None => (Vec::new(), Vec::new()),
        };

        let mut previous = std::mem::take(&mut self.slots)
            .into_iter()
            .map(|slot| (slot.view.id.clone(), slot))
            .collect::<HashMap<_, _>>();
        let mut slots = Vec::with_capacity(views.len());
        for (view, full) in views.into_iter().zip(widths) {
            let progress_targets = view.rows.iter().map(|row| row.progress).collect::<Vec<_>>();
            let slot = match previous.remove(&view.id) {
                Some(mut slot) => {
                    slot.leaving = false;
                    slot.full = full;
                    slot.width.set(full, now, RESIZE, animate);
                    slot.opacity.set(1.0, now, FADE, animate);
                    slot.progress.resize(progress_targets.len(), None);
                    for (tween, target) in slot.progress.iter_mut().zip(&progress_targets) {
                        match (tween.as_mut(), target) {
                            (Some(tween), Some(target)) => {
                                tween.set(*target, now, PROGRESS, animate)
                            }
                            (None, Some(target)) => {
                                let mut fresh = Tween::at(0.0);
                                fresh.set(*target, now, PROGRESS, animate);
                                *tween = Some(fresh);
                            }
                            (_, None) => *tween = None,
                        }
                    }
                    slot.view = view;
                    slot
                }
                None => {
                    let mut width = Tween::at(if animate { 0.0 } else { full });
                    width.set(full, now, RESIZE, animate);
                    let mut opacity = Tween::at(if self.animate() { 0.0 } else { 1.0 });
                    opacity.set(1.0, now, FADE, self.animate());
                    let progress = progress_targets
                        .iter()
                        .map(|target| {
                            target.map(|target| {
                                let mut tween = Tween::at(0.0);
                                tween.set(target, now, PROGRESS, self.animate());
                                tween
                            })
                        })
                        .collect();
                    Slot {
                        view,
                        full,
                        width,
                        opacity,
                        hover: Tween::at(0.0),
                        press: Tween::at(0.0),
                        progress,
                        leaving: false,
                    }
                }
            };
            slots.push(slot);
        }
        // Widgets that went away shrink and fade where they were.
        for (_, mut slot) in previous {
            if !animate {
                continue;
            }
            slot.leaving = true;
            slot.width.set(0.0, now, RESIZE, true);
            slot.opacity.set(0.0, now, FADE, true);
            slot.hover.set(0.0, now, HOVER_OUT, true);
            let index = self
                .hits
                .iter()
                .position(|(id, _, _)| *id == slot.view.id)
                .unwrap_or(slots.len())
                .min(slots.len());
            slots.insert(index, slot);
        }
        self.slots = slots;
        if self.hover.as_ref().is_some_and(|id| {
            !self
                .slots
                .iter()
                .any(|slot| &slot.view.id == id && !slot.leaving)
        }) {
            self.hover = None;
        }
        self.render();
    }

    /// Asks for tray space the full strip fits in and adopts the block
    /// while its icons sit together on the taskbar. Only a horizontal
    /// taskbar with the strip inside the tray reserves space.
    fn reserve_space(&mut self, geometry: Geometry, views: &[WidgetView], metrics: Metrics) {
        let wanted = match (geometry.alignment, self.renderer.as_mut()) {
            (TaskbarAlignment::InTray, Some(renderer))
                if !geometry.edge.vertical() && !views.is_empty() =>
            {
                let widths = views
                    .iter()
                    .map(|view| renderer.measure(view, metrics).unwrap_or(0.0))
                    .sum::<f32>();
                let length = widths
                    + WIDGET_GAP * views.len().saturating_sub(1) as f32
                    + 2.0 * RESERVE_PADDING;
                (length * metrics.scale).ceil() as i32
            }
            _ => 0,
        };
        super::reserve::request(wanted);
        self.reserved_screen = if wanted > 0 {
            super::reserve::latest()
        } else {
            None
        };
        self.reserved = self
            .reserved_screen
            .map(|bounds| geometry.along(bounds))
            .filter(|(start, end)| *start >= 0 && end > start);
        if self.reserved.is_some() {
            self.block = self.reserved;
        }
    }

    fn click_page(&self) -> crate::settings::TaskbarClick {
        self.config
            .as_ref()
            .map(|config| config.taskbar.click)
            .unwrap_or_default()
    }

    /// Narrows `views` to the one being shown while cycling is on.
    fn cycle(&mut self, views: Vec<WidgetView>) -> Vec<WidgetView> {
        let cycling = self
            .config
            .as_ref()
            .is_some_and(|config| config.taskbar.cycle);
        if !cycling || views.len() < 2 {
            self.cycle_ids.clear();
            return views;
        }
        self.cycle_ids = views.iter().map(|view| view.id.clone()).collect();
        let index = self
            .cycled
            .as_ref()
            .and_then(|id| self.cycle_ids.iter().position(|shown| shown == id))
            .unwrap_or(0);
        self.cycled = Some(self.cycle_ids[index].clone());
        views.into_iter().skip(index).take(1).collect()
    }

    /// Scroll wheel over the strip: steps through the widgets while cycling,
    /// down or toward the user for the next one.
    fn scroll(&mut self, delta: i32) {
        if self.cycle_ids.len() < 2 {
            self.wheel = 0;
            return;
        }
        self.wheel += delta;
        let steps = self.wheel / WHEEL_DELTA;
        if steps == 0 {
            return;
        }
        self.wheel -= steps * WHEEL_DELTA;
        let count = self.cycle_ids.len() as i32;
        let current = self
            .cycled
            .as_ref()
            .and_then(|id| self.cycle_ids.iter().position(|shown| shown == id))
            .unwrap_or(0) as i32;
        // A wheel notch away from the user is positive: go back.
        let next = (current - steps).rem_euclid(count) as usize;
        let id = self.cycle_ids[next].clone();
        self.cycled = Some(id.clone());
        let hovered = self.hover.is_some();
        let card_open = self.card.as_ref().is_some_and(|card| !card.closing);
        self.reconcile();
        if hovered {
            self.set_hover(Some(id));
            if card_open {
                // The open card follows at once to the widget now shown.
                self.open_card();
            }
        }
    }

    fn teardown(&mut self) {
        super::obstacles::set_wanted(false);
        super::reserve::request(0);
        self.obstacles = None;
        self.reserved_screen = None;
        self.reserved = None;
        self.block = None;
        self.hide();
    }

    /// Destroys the strip but keeps measuring Explorer's buttons, so widgets
    /// squeezed out by app buttons return once there is room again.
    fn hide(&mut self) {
        self.card = None;
        unsafe { KillTimer(self.controller, TIMER_CARD) };
        self.slots.clear();
        self.hits.clear();
        self.hover = None;
        self.pressed = None;
        self.tracking = false;
        self.surface = None;
        self.placed = None;
        self.shown = false;
        self.stop_frames();
        if !self.window.is_null() {
            let window = std::mem::replace(&mut self.window, null_mut());
            unsafe { DestroyWindow(window) };
        }
    }

    fn frame(&mut self) {
        self.render();
    }

    fn start_frames(&mut self) {
        if !self.frame_timer {
            unsafe { SetTimer(self.controller, TIMER_FRAME, FRAME, None) };
            self.frame_timer = true;
        }
    }

    fn stop_frames(&mut self) {
        if self.frame_timer {
            unsafe { KillTimer(self.controller, TIMER_FRAME) };
            self.frame_timer = false;
        }
    }

    fn keep_on_top(&self) {
        let Some(geometry) = self.geometry else {
            return;
        };
        unsafe {
            if GetWindow(geometry.taskbar, GW_CHILD) != self.window {
                SetWindowPos(
                    self.window,
                    HWND_TOP,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }
    }

    fn ensure_window(&mut self, geometry: Geometry, rect: RECT) -> bool {
        if !self.window.is_null() {
            return true;
        }
        unsafe {
            let class = wide(WIDGET_CLASS);
            let title = wide("Codex Minibar Taskbar Widgets");
            // Created top-level, then re-parented: Windows refuses a child
            // created directly under another process's window.
            let window = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                class.as_ptr(),
                title.as_ptr(),
                WS_POPUP,
                rect.left,
                rect.top,
                (rect.right - rect.left).max(1),
                (rect.bottom - rect.top).max(1),
                null_mut(),
                null_mut(),
                GetModuleHandleW(null_mut()),
                null_mut(),
            );
            if !window.is_null() {
                SetWindowLongPtrW(window, GWL_STYLE, (WS_CHILD | WS_CLIPSIBLINGS) as _);
                if SetParent(window, geometry.taskbar).is_null() {
                    trace(format!(
                        "Could not attach the taskbar widgets: {}",
                        std::io::Error::last_os_error()
                    ));
                    DestroyWindow(window);
                    return false;
                }
                SetWindowPos(
                    window,
                    null_mut(),
                    rect.left,
                    rect.top,
                    (rect.right - rect.left).max(1),
                    (rect.bottom - rect.top).max(1),
                    SWP_NOACTIVATE | SWP_NOZORDER,
                );
            }
            if window.is_null() {
                trace(format!(
                    "Could not create the taskbar widget window: {}",
                    std::io::Error::last_os_error()
                ));
                return false;
            }
            self.window = window;
            self.placed = Some(key(rect));
            self.shown = false;
            SetWindowPos(
                window,
                HWND_TOP,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        true
    }

    fn render(&mut self) {
        let now = Instant::now();
        let Some(geometry) = self.geometry else {
            self.teardown();
            return;
        };
        self.slots
            .retain(|slot| !(slot.leaving && slot.width.done(now) && slot.opacity.done(now)));
        let metrics = metrics_for(geometry);
        let mut x = 0.0;
        let mut hits = Vec::with_capacity(self.slots.len());
        let mut placed = Vec::with_capacity(self.slots.len());
        for (index, slot) in self.slots.iter().enumerate() {
            let width = slot.width.value(now).max(0.0);
            let share = if slot.full > 0.0 {
                (width / slot.full).clamp(0.0, 1.0)
            } else {
                0.0
            };
            if index > 0 {
                x += WIDGET_GAP * share;
            }
            hits.push((slot.view.id.clone(), x, width));
            placed.push(x);
            x += width;
        }
        let total = x;
        if total <= 0.5 {
            if self.slots.is_empty() {
                self.hide();
            }
            return;
        }
        // Over a reserved tray block the window covers all of it, so Explorer
        // never highlights the blank icons, and centers the strip.
        let reserved = match geometry.alignment {
            TaskbarAlignment::InTray => self.reserved.or(self.block),
            _ => None,
        };
        let (start, length) = match (reserved, geometry.alignment) {
            (Some((start, end)), _) => (start, end - start),
            (None, TaskbarAlignment::InTray) => {
                self.hide();
                return;
            }
            (None, TaskbarAlignment::Tray) => {
                let length = (total * metrics.scale).ceil() as i32;
                (
                    geometry.anchor - (TRAY_MARGIN * metrics.scale).round() as i32 - length,
                    length,
                )
            }
            (None, TaskbarAlignment::Start) => {
                (self.leading, (total * metrics.scale).ceil() as i32)
            }
        };
        let (rect, width, height) = if geometry.edge.vertical() {
            (
                RECT {
                    left: 0,
                    top: start,
                    right: geometry.thickness,
                    bottom: start + length,
                },
                geometry.thickness,
                length,
            )
        } else {
            (
                RECT {
                    left: start,
                    top: 0,
                    right: start + length,
                    bottom: geometry.thickness,
                },
                length,
                geometry.thickness,
            )
        };
        // Tray alignment grows toward the start: content keeps its far end.
        let shift = match (reserved, geometry.alignment) {
            (Some(_), _) => ((length as f32 / metrics.scale - total) / 2.0).max(0.0),
            (None, TaskbarAlignment::Tray) => (length as f32 / metrics.scale - total).max(0.0),
            (None, _) => 0.0,
        };
        if !self.ensure_window(geometry, rect) {
            return;
        }
        if self
            .surface
            .as_ref()
            .is_none_or(|surface| surface.size != (width, height))
        {
            self.surface = Surface::new((width, height));
        }
        let Some(surface) = self.surface.as_ref() else {
            return;
        };
        let progress = self
            .slots
            .iter()
            .map(|slot| {
                slot.progress
                    .iter()
                    .map(|tween| tween.map(|tween| tween.value(now)))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let items = self
            .slots
            .iter()
            .zip(&placed)
            .zip(&progress)
            .map(|((slot, offset), progress)| Item {
                view: &slot.view,
                offset: offset + shift,
                length: slot.width.value(now).max(0.0),
                hover: slot.hover.value(now),
                press: slot.press.value(now),
                opacity: slot.opacity.value(now),
                progress,
            })
            .collect::<Vec<_>>();
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        if let Err(error) = renderer.paint(
            surface.dc,
            (width, height),
            metrics,
            self.appearance,
            &items,
            reserved.is_some(),
        ) {
            trace(format!("Taskbar widget paint failed: {error}"));
            return;
        }
        drop(items);
        unsafe {
            if self.placed != Some(key(rect)) {
                SetWindowPos(
                    self.window,
                    null_mut(),
                    rect.left,
                    rect.top,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                );
                self.placed = Some(key(rect));
            }
            let screen = GetDC(null_mut());
            let size = SIZE {
                cx: width,
                cy: height,
            };
            let source = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            UpdateLayeredWindow(
                self.window,
                screen,
                null_mut(),
                &size,
                surface.dc,
                &source,
                0,
                &blend,
                ULW_ALPHA,
            );
            ReleaseDC(null_mut(), screen);
            if !self.shown {
                ShowWindow(self.window, SW_SHOWNA);
                self.shown = true;
            }
        }
        self.hits = hits
            .into_iter()
            .map(|(id, x, width)| (id, x + shift, width))
            .collect();
        self.render_card(now);
        let animating = self
            .slots
            .iter()
            .any(|slot| slot.tweens().any(|tween| !tween.done(now)))
            || self.slots.iter().any(|slot| slot.leaving)
            || self
                .card
                .as_ref()
                .is_some_and(|card| !card.opacity.done(now) || card.closing);
        if animating {
            self.start_frames();
        } else {
            self.stop_frames();
        }
    }

    fn hit(&self, point: (i32, i32)) -> Option<String> {
        let geometry = self.geometry?;
        let along = if geometry.edge.vertical() {
            point.1
        } else {
            point.0
        } as f32
            / geometry.scale;
        self.hits
            .iter()
            .find(|(id, start, length)| {
                along >= *start
                    && along < start + length
                    && self
                        .slots
                        .iter()
                        .any(|slot| &slot.view.id == id && !slot.leaving)
            })
            .map(|(id, _, _)| id.clone())
    }

    fn set_hover(&mut self, id: Option<String>) {
        if id == self.hover {
            return;
        }
        let now = Instant::now();
        let animate = self.animate();
        for slot in &mut self.slots {
            let target = if Some(&slot.view.id) == id.as_ref() {
                1.0
            } else {
                0.0
            };
            let duration = if target > 0.5 { HOVER_IN } else { HOVER_OUT };
            slot.hover.set(target, now, duration, animate);
        }
        let reopen = id.is_some()
            && self
                .card
                .as_ref()
                .is_some_and(|card| !card.closing && Some(&card.id) != id.as_ref());
        unsafe { KillTimer(self.controller, TIMER_CARD) };
        if id.is_none() {
            self.close_card();
        } else if reopen {
            // Moving between widgets swaps the card without the delay.
            self.hover = id;
            self.open_card();
            return;
        } else if self.card.as_ref().is_none_or(|card| card.closing) {
            unsafe { SetTimer(self.controller, TIMER_CARD, CARD_DELAY, None) };
        }
        self.hover = id;
        self.render();
    }

    fn open_card(&mut self) {
        let Some(id) = self.hover.clone() else {
            return;
        };
        if self.pressed.is_some() {
            return;
        }
        let now = Instant::now();
        let animate = self.animate();
        match self.card.as_mut() {
            Some(card) => {
                card.id = id;
                card.closing = false;
                card.opacity.set(1.0, now, CARD_IN, animate);
            }
            None => {
                let window = unsafe {
                    let class = wide(CARD_CLASS);
                    CreateWindowExW(
                        WS_EX_LAYERED
                            | WS_EX_TRANSPARENT
                            | WS_EX_TOPMOST
                            | WS_EX_TOOLWINDOW
                            | WS_EX_NOACTIVATE,
                        class.as_ptr(),
                        null_mut(),
                        WS_POPUP,
                        0,
                        0,
                        1,
                        1,
                        null_mut(),
                        null_mut(),
                        GetModuleHandleW(null_mut()),
                        null_mut(),
                    )
                };
                if window.is_null() {
                    return;
                }
                let mut opacity = Tween::at(0.0);
                opacity.set(1.0, now, CARD_IN, animate);
                self.card = Some(Card {
                    window,
                    surface: None,
                    id,
                    opacity,
                    rect: (0, 0, 0, 0),
                    closing: false,
                });
            }
        }
        self.render();
    }

    fn close_card(&mut self) {
        unsafe { KillTimer(self.controller, TIMER_CARD) };
        let now = Instant::now();
        let animate = self.animate();
        let Some(card) = self.card.as_mut() else {
            return;
        };
        if !animate {
            self.card = None;
            return;
        }
        if !card.closing {
            card.closing = true;
            card.opacity.set(0.0, now, CARD_OUT, true);
            self.start_frames();
        }
    }

    /// Draws the hover card above its widget.
    fn render_card(&mut self, now: Instant) {
        let Some(geometry) = self.geometry else {
            self.card = None;
            return;
        };
        if self
            .card
            .as_ref()
            .is_some_and(|card| card.closing && card.opacity.done(now))
        {
            self.card = None;
        }
        let Some(card) = self.card.as_ref() else {
            return;
        };
        let Some(slot) = self
            .slots
            .iter()
            .find(|slot| slot.view.id == card.id && !slot.leaving)
        else {
            self.card = None;
            return;
        };
        let Some(&(_, hit_start, hit_length)) = self.hits.iter().find(|(id, _, _)| *id == card.id)
        else {
            return;
        };
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let scale = geometry.scale;
        let stamp = Utc::now();
        let Ok((width_dip, height_dip)) = renderer.card_size(&slot.view, scale, stamp) else {
            return;
        };
        let width = (width_dip * scale).ceil() as i32;
        let height = (height_dip * scale).ceil() as i32;
        let mut strip = std::mem::MaybeUninit::<RECT>::zeroed();
        unsafe { GetWindowRect(self.window, strip.as_mut_ptr()) };
        let strip = unsafe { strip.assume_init() };
        let monitor = monitor_bounds(geometry.taskbar);
        let margin = (CARD_MARGIN * scale).round() as i32;
        let gap = (CARD_GAP * scale).round() as i32;
        let middle = ((hit_start + hit_length / 2.0) * scale).round() as i32;
        let edge = (8.0 * scale).round() as i32;
        let opacity = card.opacity.value(now);
        // The card rises out of the taskbar: up from a bottom one, down from
        // a top one, sideways from a vertical one.
        let travel = CARD_RISE * (1.0 - opacity);
        let slide = (travel * scale).round() as i32;
        let (left, top, rise) = match geometry.edge {
            Edge::Bottom | Edge::Top => {
                let left = (strip.left + middle - width / 2)
                    .min(monitor.right - edge - width + margin)
                    .max(monitor.left + edge - margin);
                if geometry.edge == Edge::Top {
                    (left, strip.bottom + gap - margin, -travel)
                } else {
                    (left, strip.top - gap - height + margin, travel)
                }
            }
            Edge::Left | Edge::Right => {
                let top = (strip.top + middle - height / 2)
                    .min(monitor.bottom - edge - height + margin)
                    .max(monitor.top + edge - margin);
                if geometry.edge == Edge::Left {
                    (strip.right + gap - margin - slide, top, 0.0)
                } else {
                    (strip.left - gap - width + margin + slide, top, 0.0)
                }
            }
        };
        let view = slot.view.clone();
        let appearance = self.appearance;
        let Some(card) = self.card.as_mut() else {
            return;
        };
        if card
            .surface
            .as_ref()
            .is_none_or(|surface| surface.size != (width, height))
        {
            card.surface = Surface::new((width, height));
        }
        let Some(surface) = card.surface.as_ref() else {
            return;
        };
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        if renderer
            .paint_card(
                surface.dc,
                (width, height),
                scale,
                appearance,
                &view,
                opacity,
                rise,
                stamp,
            )
            .is_err()
        {
            return;
        }
        unsafe {
            let screen = GetDC(null_mut());
            let position = POINT { x: left, y: top };
            let size = SIZE {
                cx: width,
                cy: height,
            };
            let source = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            UpdateLayeredWindow(
                card.window,
                screen,
                &position,
                &size,
                surface.dc,
                &source,
                0,
                &blend,
                ULW_ALPHA,
            );
            ReleaseDC(null_mut(), screen);
            if card.rect == (0, 0, 0, 0) {
                ShowWindow(card.window, SW_SHOWNA);
            }
            card.rect = (left, top, width, height);
        }
    }

    /// Returns whether leave tracking must be requested.
    fn mouse_move(&mut self, point: (i32, i32)) -> bool {
        let id = self.hit(point);
        self.set_hover(id);
        let track = !self.tracking;
        self.tracking = true;
        track
    }

    fn mouse_leave(&mut self) {
        self.tracking = false;
        if self.pressed.is_none() {
            self.set_hover(None);
        }
    }

    fn set_press(&mut self, id: Option<&String>) {
        let now = Instant::now();
        let animate = self.animate();
        for slot in &mut self.slots {
            let target = if Some(&slot.view.id) == id { 1.0 } else { 0.0 };
            slot.press.set(target, now, PRESS, animate);
        }
        self.render();
    }

    fn press(&mut self, point: (i32, i32)) -> bool {
        let Some(id) = self.hit(point) else {
            return false;
        };
        self.close_card();
        self.set_press(Some(&id));
        self.pressed = Some(id);
        true
    }

    fn release(&mut self, point: (i32, i32)) -> Option<Action> {
        let pressed = self.pressed.take()?;
        self.set_press(None);
        let over = self.hit(point);
        if over.as_ref() != Some(&pressed) {
            self.set_hover(over);
            return None;
        }
        let provider = self
            .slots
            .iter()
            .find(|slot| slot.view.id == pressed)
            .and_then(|slot| slot.view.provider);
        let mut cursor = POINT { x: 0, y: 0 };
        unsafe { GetCursorPos(&mut cursor) };
        Some(Action::Open {
            provider,
            anchor: (cursor.x, cursor.y),
            page: self.click_page(),
        })
    }
}

/// Longest a first show waits for Explorer's buttons to be measured.
const OBSTACLE_WAIT: Duration = Duration::from_millis(1500);

/// Clearance kept from Explorer's buttons, in DIPs.
const OBSTACLE_GAP: f32 = 8.0;

/// Left edge of a start-aligned strip and the width available to the strip,
/// both in taskbar client pixels.
fn free_span(geometry: Geometry, obstacles: Option<Obstacles>) -> (i32, f32) {
    let scale = geometry.scale;
    let gap = (OBSTACLE_GAP * scale).round() as i32;
    let cluster = obstacles
        .and_then(|obstacles| obstacles.cluster)
        .map(|bounds| geometry.along(bounds));
    match geometry.alignment {
        // The strip lives in its reserved block instead.
        TaskbarAlignment::InTray => (0, 0.0),
        TaskbarAlignment::Tray => {
            let end = geometry.anchor - (TRAY_MARGIN * scale).round() as i32;
            // Buttons that reach the tray leave no room rather than all of it.
            let start = cluster
                .filter(|(start, _)| *start < end)
                .map(|(_, end)| end + gap)
                .unwrap_or(i32::MIN / 2);
            (0, (end - start).max(0) as f32)
        }
        TaskbarAlignment::Start => {
            let leading = obstacles
                .and_then(|obstacles| obstacles.widgets)
                .map(|bounds| geometry.along(bounds).1)
                .into_iter()
                .chain(geometry.start_button)
                .fold(geometry.anchor, i32::max)
                + (START_MARGIN * scale).round() as i32;
            let end = cluster
                .map(|(start, _)| start - gap)
                .unwrap_or(i32::MAX / 2);
            (leading, (end - leading).max(0) as f32)
        }
    }
}

/// Keeps as many widgets, in order, as fit `available` DIPs, giving up
/// detail on all of them (see [`WidgetView::compacted`]) before a widget
/// overflows the way notification icons do. Without room for even the first
/// widget at its most compact, nothing is shown rather than covering
/// Explorer's buttons.
fn fit(
    views: Vec<WidgetView>,
    available: f32,
    mut measure: impl FnMut(&WidgetView) -> f32,
) -> (Vec<WidgetView>, Vec<f32>) {
    if views.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let length = |layout: &[(WidgetView, f32)], count: usize| {
        layout[..count].iter().map(|(_, width)| width).sum::<f32>()
            + WIDGET_GAP * count.saturating_sub(1) as f32
    };
    // Measured lazily: a roomy taskbar only ever needs the full layout.
    let mut layouts: Vec<Vec<(WidgetView, f32)>> = Vec::new();
    for count in (1..=views.len()).rev() {
        for level in 0..=model::COMPACT_LEVELS {
            let index = usize::from(level);
            if layouts.len() <= index {
                layouts.push(
                    views
                        .iter()
                        .map(|view| {
                            let view = view.compacted(level);
                            let width = measure(&view);
                            (view, width)
                        })
                        .collect(),
                );
            }
            if length(&layouts[index], count) <= available + 0.5 {
                return layouts.swap_remove(index).into_iter().take(count).unzip();
            }
        }
    }
    (Vec::new(), Vec::new())
}

/// Bounds of the monitor showing `window`.
fn monitor_bounds(window: HWND) -> RECT {
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
    };
    unsafe {
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(
            MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST),
            &mut info,
        );
        info.rcMonitor
    }
}

fn key(rect: RECT) -> (i32, i32, i32, i32) {
    (rect.left, rect.top, rect.right, rect.bottom)
}

fn current_appearance(colored_icons: bool) -> Appearance {
    Appearance {
        light: crate::taskbar::uses_light_theme(),
        colored_icons,
        accent: crate::theme::current_accent_rgb(),
    }
}

fn metrics_for(geometry: Geometry) -> Metrics {
    Metrics {
        scale: geometry.scale,
        thickness: geometry.thickness as f32 / geometry.scale,
        vertical: geometry.edge.vertical(),
    }
}

/// A child window's extent along the taskbar, in taskbar client pixels.
unsafe fn child_span(taskbar: HWND, child: HWND, vertical: bool) -> (i32, i32) {
    let mut rect = unsafe { std::mem::zeroed::<RECT>() };
    unsafe { GetWindowRect(child, &mut rect) };
    let mut points = [
        POINT {
            x: rect.left,
            y: rect.top,
        },
        POINT {
            x: rect.right,
            y: rect.bottom,
        },
    ];
    unsafe { MapWindowPoints(null_mut(), taskbar, points.as_mut_ptr(), 2) };
    if vertical {
        (points[0].y, points[1].y)
    } else {
        (points[0].x, points[1].x)
    }
}

/// Finds the primary taskbar, the edge it is docked to and where the strip
/// belongs on it.
fn locate(alignment: TaskbarAlignment) -> Option<Geometry> {
    use windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible;
    unsafe {
        let taskbar = FindWindowW(wide("Shell_TrayWnd").as_ptr(), null_mut());
        if taskbar.is_null() {
            return None;
        }
        let mut client = std::mem::zeroed::<RECT>();
        GetClientRect(taskbar, &mut client);
        let (width, height) = (client.right - client.left, client.bottom - client.top);
        if width <= 0 || height <= 0 {
            return None;
        }
        // Windows 11 docks only to the bottom; older shells and shell
        // replacements also use the other edges.
        let vertical = height > width;
        let mut frame = std::mem::zeroed::<RECT>();
        GetWindowRect(taskbar, &mut frame);
        let monitor = monitor_bounds(taskbar);
        let edge = if vertical {
            if frame.left + frame.right < monitor.left + monitor.right {
                Edge::Left
            } else {
                Edge::Right
            }
        } else if frame.top + frame.bottom < monitor.top + monitor.bottom {
            Edge::Top
        } else {
            Edge::Bottom
        };
        let (thickness, long) = if vertical {
            (width, height)
        } else {
            (height, width)
        };
        let dpi = GetDpiForWindow(taskbar);
        let scale = if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 };
        let anchor = match alignment {
            TaskbarAlignment::Tray | TaskbarAlignment::InTray => {
                let tray = FindWindowExW(
                    taskbar,
                    null_mut(),
                    wide("TrayNotifyWnd").as_ptr(),
                    null_mut(),
                );
                if tray.is_null() {
                    long - (200.0 * scale) as i32
                } else {
                    child_span(taskbar, tray, vertical).0
                }
            }
            TaskbarAlignment::Start => 0,
        };
        let start = FindWindowExW(taskbar, null_mut(), wide("Start").as_ptr(), null_mut());
        let start_button = (!start.is_null() && IsWindowVisible(start) != 0)
            .then(|| child_span(taskbar, start, vertical))
            .filter(|(from, to)| to > from)
            .map(|(_, to)| to);
        let mut origin = POINT { x: 0, y: 0 };
        MapWindowPoints(taskbar, null_mut(), &mut origin, 1);
        Some(Geometry {
            taskbar,
            scale,
            edge,
            thickness,
            anchor,
            start_button,
            alignment,
            origin: (origin.x, origin.y),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        instances::ProviderId,
        settings::{ProviderKind, TaskbarResetDisplay, TaskbarWidget, TaskbarWidgetStyle},
    };

    fn bars(count: usize) -> Vec<WidgetView> {
        let appearance = Appearance {
            light: false,
            colored_icons: true,
            accent: [0, 120, 212],
        };
        (0..count)
            .map(|_| {
                let widget = TaskbarWidget {
                    style: TaskbarWidgetStyle::Bars,
                    reset: TaskbarResetDisplay::Countdown,
                    ..TaskbarWidget::for_provider(ProviderId::primary(ProviderKind::Codex))
                };
                model::resolve(&widget, &ProviderLimits::default(), appearance)
            })
            .collect()
    }

    /// A stand-in for the renderer: 20 DIPs of graphics (10 for meters)
    /// plus the extras.
    fn width(view: &WidgetView) -> f32 {
        let graphic = if view.style == TaskbarWidgetStyle::Meters {
            10.0
        } else {
            20.0
        };
        graphic
            + if view.icon.is_some() { 20.0 } else { 0.0 }
            + if view.show_labels { 10.0 } else { 0.0 }
            + if view.show_values { 10.0 } else { 0.0 }
    }

    #[test]
    fn widgets_shrink_before_they_overflow_and_hide_without_room() {
        let full = width(&bars(1)[0]);
        let all = bars(2);
        let (views, _) = fit(all.clone(), full * 2.0 + WIDGET_GAP, width);
        assert_eq!(views, all);

        let (views, widths) = fit(bars(2), 2.0 * 20.0 + WIDGET_GAP, width);
        assert_eq!(views.len(), 2);
        assert!(
            views
                .iter()
                .all(|view| view.icon.is_none() && !view.has_text())
        );
        assert!(
            views
                .iter()
                .all(|view| view.style == TaskbarWidgetStyle::Bars)
        );
        assert_eq!(widths, [20.0, 20.0]);

        let (views, widths) = fit(bars(2), 2.0 * 10.0 + WIDGET_GAP, width);
        assert_eq!(views.len(), 2);
        assert!(
            views
                .iter()
                .all(|view| view.style == TaskbarWidgetStyle::Meters)
        );
        assert_eq!(widths, [10.0, 10.0]);

        let (views, widths) = fit(bars(2), 15.0, width);
        assert_eq!(views.len(), 1);
        assert_eq!(widths, [10.0]);

        let (views, widths) = fit(bars(2), 5.0, width);
        assert!(views.is_empty() && widths.is_empty());
    }

    #[test]
    fn buttons_reaching_the_tray_leave_no_room() {
        let geometry = Geometry {
            taskbar: null_mut(),
            scale: 1.0,
            edge: Edge::Bottom,
            thickness: 40,
            anchor: 1000,
            start_button: None,
            alignment: TaskbarAlignment::Tray,
            origin: (0, 0),
        };
        let buttons = |end| {
            Some(Obstacles {
                cluster: Some((0, 0, end, 40)),
                widgets: None,
            })
        };
        assert_eq!(free_span(geometry, buttons(800)).1, 190.0);
        assert_eq!(free_span(geometry, buttons(995)).1, 0.0);
        assert_eq!(free_span(geometry, buttons(1200)).1, 0.0);
    }
}
