//! Settings and onboarding windows, rendered with GPUI.
//!
//! Both windows live in the same GPUI application as the tray popup. Any
//! thread can request them through the functions below; the request is
//! posted to the GPUI thread as a [`crate::ui::popup_window::PopupCommand`].
//! Each window keeps a full [`Settings`] snapshot: edits apply to it
//! immediately and are committed by [`crate::settings::store`], and every
//! committed change (from any surface) is synced back through
//! [`sync_open_window`].

use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use gpui::{
    AppContext, AsyncApp, Bounds, Point, TitlebarOptions, WindowBackgroundAppearance, WindowBounds,
    WindowHandle, WindowKind, WindowOptions, px, size,
};

use crate::app::AppState;
use crate::settings::Settings;
use crate::ui::gui::{kit::Handler, theme::Fonts};
use crate::ui::popup_window::PopupCommand;

mod about;
mod activation;
mod advanced;
mod appearance;
mod backdrop;
mod command_palette;
mod customize;
mod general;
mod integrations;
mod log;
mod nav;
mod notifications;
mod onboarding;
mod providers;
#[cfg(not(windows))]
mod shortcut;
mod taskbar;
mod tray;
mod troubleshoot;
mod vscode_themes;
mod widget_list;
mod window;

#[cfg(test)]
mod tests;

const WINDOW_WIDTH: f32 = 1000.0;
const WINDOW_HEIGHT: f32 = 740.0;
// The layouts are not built for full-screen widths, so the windows cannot be
// maximized and stop growing at this size.
const WINDOW_MAX_SIZE: (f32, f32) = (1240.0, 960.0);
/// Smallest Settings window that keeps rows, descriptions and controls readable.
const WINDOW_MIN_SIZE: (f32, f32) = (860.0, 560.0);
const ONBOARDING_WIDTH: f32 = 780.0;
const ONBOARDING_HEIGHT: f32 = 560.0;
/// What the Settings and onboarding windows share: a full settings snapshot
/// that edits apply to at once, and handlers bound to the window.
pub(super) trait Host: Sized + 'static {
    fn settings(&self) -> &Settings;
    fn settings_mut(&mut self) -> &mut Settings;
    fn fonts(&self) -> &Fonts;

    /// Apply an edit to this window's snapshot right away and queue what it
    /// changed for the settings file.
    fn edit(&mut self, cx: &mut gpui::Context<Self>, edit: impl FnOnce(&mut Settings)) {
        let before = crate::settings::store::edit_snapshot(self.settings_mut(), edit);
        crate::settings::runtime::apply_presentation_changes(&before, self.settings());
        cx.refresh_windows();
        cx.notify();
    }

    /// Wrap a method into a component handler.
    fn h<T: 'static>(
        cx: &gpui::Context<Self>,
        f: impl Fn(&mut Self, T, &mut gpui::Window, &mut gpui::Context<Self>) + 'static,
    ) -> Handler<T> {
        let this = cx.weak_entity();
        std::rc::Rc::new(move |value, window, cx| {
            let _ = this.update(cx, |this, cx| f(this, value, window, cx));
        })
    }
}

pub(crate) fn settings_window_title() -> &'static str {
    crate::i18n::tr(crate::app::ids::SETTINGS_TITLE_KEY)
}
pub(crate) fn onboarding_window_title() -> &'static str {
    crate::i18n::tr("welcome-to-codex-minibar")
}

static SETTINGS_OPEN: AtomicBool = AtomicBool::new(false);
static ONBOARDING_OPEN: AtomicBool = AtomicBool::new(false);

thread_local! {
    static SETTINGS_WINDOW: RefCell<Option<WindowHandle<window::SettingsWindow>>> =
        const { RefCell::new(None) };
    static ONBOARDING_WINDOW: RefCell<Option<WindowHandle<onboarding::OnboardingWindow>>> =
        const { RefCell::new(None) };
}

/// Requests handled on the GPUI thread.
pub(crate) enum Command {
    Open,
    OpenOnboarding,
    SignIn(crate::providers::instances::ProviderId),
    Sync(Box<Settings>),
    Live(providers::LiveProviderData),
    /// The popup shortcut's registration status changed.
    #[cfg(windows)]
    HotkeyStatus,
    /// Open Settings on the Taskbar page, with one widget's editor open.
    OpenTaskbar(Option<String>),
    /// Open Settings on Integrations, where the Command Palette lives.
    OpenCommandPalette,
}

fn post(command: Command) {
    crate::ui::popup_window::send_command(PopupCommand::Settings(command));
}

/// Open (or focus) the Settings window.
pub fn open() {
    post(Command::Open);
}

/// Open (or focus) Settings on the Taskbar page.
pub fn open_taskbar() {
    post(Command::OpenTaskbar(None));
}

/// Open Settings on the Command Palette extension and its Dock bands.
pub fn open_command_palette() {
    post(Command::OpenCommandPalette);
}

/// Open Settings on the editor of the taskbar widget `id`.
pub fn open_taskbar_widget(id: String) {
    post(Command::OpenTaskbar(Some(id)));
}

/// Open Settings on one account's page with its sign-in already started.
pub fn open_sign_in(provider: crate::providers::instances::ProviderId) {
    post(Command::SignIn(provider));
}

/// Open the first-launch flow beside the popup, which previews each choice.
/// A dismissed onboarding window restores the settings it found, so it never
/// half-configures provider workers.
pub fn open_onboarding() {
    post(Command::OpenOnboarding);
}

/// Whether a Settings or onboarding window is currently alive.
///
/// The tray popup uses this to stay visible as a live preview while a user
/// navigates settings and changes popup-related options.
pub(crate) fn is_open() -> bool {
    SETTINGS_OPEN.load(Ordering::SeqCst) || ONBOARDING_OPEN.load(Ordering::SeqCst)
}

/// Repaint Settings after the popup shortcut was (un)registered.
#[cfg(windows)]
pub(crate) fn refresh_hotkey_status() {
    if SETTINGS_OPEN.load(Ordering::SeqCst) {
        post(Command::HotkeyStatus);
    }
}

/// Push a committed settings snapshot into the open window.
pub fn sync_open_window(settings: Settings) {
    if is_open() {
        post(Command::Sync(Box::new(settings)));
    }
}

/// Hands the live quota snapshot to the open Settings window.
pub fn publish_limits(limits: &crate::quota::limits::ProviderLimits) {
    if SETTINGS_OPEN.load(Ordering::SeqCst) {
        post(Command::Live(providers::LiveProviderData::from_limits(
            limits,
        )));
    }
}

/// Execute a [`Command`] on the GPUI thread.
pub(crate) fn handle(command: Command, state: &Arc<AppState>, cx: &mut AsyncApp) {
    match command {
        Command::Open => {
            if focus_existing(&ONBOARDING_WINDOW, cx) || focus_existing(&SETTINGS_WINDOW, cx) {
                return;
            }
            open_settings_window(Arc::clone(state), cx);
        }
        Command::SignIn(provider) => {
            if focus_existing(&ONBOARDING_WINDOW, cx) {
                return;
            }
            if !focus_existing(&SETTINGS_WINDOW, cx) {
                open_settings_window(Arc::clone(state), cx);
            }
            let Some(handle) = SETTINGS_WINDOW.with(|slot| *slot.borrow()) else {
                return;
            };
            let _ = handle.update(cx, |root, window, cx| {
                root.begin_sign_in(provider, window, cx)
            });
        }
        Command::OpenOnboarding => {
            if focus_existing(&ONBOARDING_WINDOW, cx) {
                return;
            }
            close_existing(&SETTINGS_WINDOW, cx);
            open_onboarding_window(cx);
        }
        Command::Sync(settings) => {
            with_settings_window(cx, |root, cx| root.apply_sync((*settings).clone(), cx));
            if let Some(handle) = ONBOARDING_WINDOW.with(|slot| *slot.borrow()) {
                let _ = handle.update(cx, |root, _, cx| root.apply_sync(*settings, cx));
            }
        }
        Command::Live(live) => with_settings_window(cx, |root, cx| {
            if root.live != live {
                root.live = live;
                cx.notify();
            }
        }),
        #[cfg(windows)]
        Command::HotkeyStatus => with_settings_window(cx, |_, cx| cx.notify()),
        Command::OpenTaskbar(widget) => {
            if focus_existing(&ONBOARDING_WINDOW, cx) {
                return;
            }
            if !focus_existing(&SETTINGS_WINDOW, cx) {
                open_settings_window(Arc::clone(state), cx);
            }
            with_settings_window(cx, |root, cx| {
                root.open_tab(nav::Tab::Taskbar, cx);
                if let Some(widget) = widget {
                    root.open_widget::<widget_list::Taskbar>(widget, cx);
                }
            });
        }
        Command::OpenCommandPalette => {
            if focus_existing(&ONBOARDING_WINDOW, cx) {
                return;
            }
            if !focus_existing(&SETTINGS_WINDOW, cx) {
                open_settings_window(Arc::clone(state), cx);
            }
            with_settings_window(cx, |root, cx| root.open_tab(nav::Tab::Integrations, cx));
        }
    }
}

fn with_settings_window(
    cx: &mut AsyncApp,
    f: impl FnOnce(&mut window::SettingsWindow, &mut gpui::Context<window::SettingsWindow>),
) {
    let Some(handle) = SETTINGS_WINDOW.with(|slot| *slot.borrow()) else {
        return;
    };
    if handle.update(cx, |root, _, cx| f(root, cx)).is_err() {
        SETTINGS_WINDOW.with(|slot| slot.borrow_mut().take());
        SETTINGS_OPEN.store(false, Ordering::SeqCst);
    }
}

fn activate_window(window: &gpui::Window) {
    #[cfg(not(windows))]
    if let Some(token) = crate::platform::linux::instance::take_activation_token() {
        window.activate_window_with_token(token);
        return;
    }
    window.activate_window();
}

fn focus_existing<V: 'static>(
    slot: &'static std::thread::LocalKey<RefCell<Option<WindowHandle<V>>>>,
    cx: &mut AsyncApp,
) -> bool {
    let Some(handle) = slot.with(|slot| *slot.borrow()) else {
        return false;
    };
    let alive = handle
        .update(cx, |_, window, _| activate_window(window))
        .is_ok();
    if !alive {
        slot.with(|slot| slot.borrow_mut().take());
    }
    alive
}

fn close_existing<V: 'static>(
    slot: &'static std::thread::LocalKey<RefCell<Option<WindowHandle<V>>>>,
    cx: &mut AsyncApp,
) {
    if let Some(handle) = slot.with(|slot| slot.borrow_mut().take()) {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
    }
}

fn window_options(
    title: &'static str,
    width: f32,
    height: f32,
    min: (f32, f32),
    max: (f32, f32),
    cx: &gpui::App,
) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some(title.into()),
            appears_transparent: true,
            traffic_light_position: Some(Point::default()),
        }),
        focus: true,
        show: true,
        kind: WindowKind::Normal,
        is_movable: true,
        is_resizable: true,
        is_minimizable: true,
        display_id: None,
        window_background: WindowBackgroundAppearance::Opaque,
        #[cfg(windows)]
        app_id: Some("CodexMinibar".into()),
        #[cfg(not(windows))]
        app_id: Some(crate::platform::linux::identity::APP_ID.into()),
        window_min_size: Some(size(px(min.0), px(min.1))),
        window_max_size: Some(size(px(max.0), px(max.1))),
        // Linux: our own caption is the title bar. GPUI would otherwise ask
        // X11 and KDE window managers for a second, native one.
        window_decorations: (!cfg!(windows)).then_some(gpui::WindowDecorations::Client),
        tabbing_identifier: None,
    }
}

fn open_settings_window(state: Arc<AppState>, cx: &mut AsyncApp) {
    let result = cx.update(|cx| {
        let options = window_options(
            settings_window_title(),
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            WINDOW_MIN_SIZE,
            WINDOW_MAX_SIZE,
            cx,
        );
        cx.open_window(options, |window, cx| {
            cx.new(|cx| window::SettingsWindow::new(state, window, cx))
        })
    });
    match result {
        Ok(Ok(handle)) => {
            SETTINGS_OPEN.store(true, Ordering::SeqCst);
            SETTINGS_WINDOW.with(|slot| *slot.borrow_mut() = Some(handle));
            let _ = handle.update(cx, |_, window, _| activate_window(window));
        }
        Ok(Err(error)) => eprintln!("Could not open settings window: {error:#}"),
        Err(error) => eprintln!("Could not open settings window: {error:#}"),
    }
}

fn open_onboarding_window(cx: &mut AsyncApp) {
    let result = cx.update(|cx| {
        let options = window_options(
            onboarding_window_title(),
            ONBOARDING_WIDTH,
            ONBOARDING_HEIGHT,
            (560.0, 420.0),
            WINDOW_MAX_SIZE,
            cx,
        );
        cx.open_window(options, |window, cx| {
            cx.new(|cx| onboarding::OnboardingWindow::new(window, cx))
        })
    });
    match result {
        Ok(Ok(handle)) => {
            ONBOARDING_OPEN.store(true, Ordering::SeqCst);
            ONBOARDING_WINDOW.with(|slot| *slot.borrow_mut() = Some(handle));
            // The popup stays up beside onboarding as a live preview of
            // every choice (`is_open` keeps it from being dismissed).
            // Home lists every provider onboarding turns on.
            crate::ui::popup_window::request_view(crate::ui::popup_window::PendingPopupView::Home);
            crate::ui::popup_window::ensure_shown(crate::ui::popup::Origin::PrimaryMonitor);
            let _ = handle.update(cx, |_, window, _| activate_window(window));
        }
        Ok(Err(error)) => eprintln!("Could not open onboarding: {error:#}"),
        Err(error) => eprintln!("Could not open onboarding: {error:#}"),
    }
}

/// Called by a window as it closes.
pub(crate) fn window_closed(onboarding: bool) {
    if onboarding {
        ONBOARDING_OPEN.store(false, Ordering::SeqCst);
        ONBOARDING_WINDOW.with(|slot| slot.borrow_mut().take());
    } else {
        SETTINGS_OPEN.store(false, Ordering::SeqCst);
        SETTINGS_WINDOW.with(|slot| slot.borrow_mut().take());
    }
}
