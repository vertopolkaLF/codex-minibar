//! Settings and onboarding windows, rendered with GPUI.
//!
//! Both windows live in the same GPUI application as the tray popup. Any
//! thread can request them through the functions below; the request is
//! posted to the GPUI thread as a [`crate::popup_window::PopupCommand`].
//! Each window keeps a full [`Settings`] snapshot: edits apply to it
//! immediately and are committed by a serial writer, and every committed
//! change (from any surface) is synced back through [`sync_open_window`].

use std::{
    cell::RefCell,
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use gpui::{
    AppContext, AsyncApp, Bounds, Point, TitlebarOptions, WindowBackgroundAppearance, WindowBounds,
    WindowHandle, WindowKind, WindowOptions, px, size,
};

use crate::popup_window::{AppState, PopupCommand};
use crate::settings::Settings;

mod about;
mod activation;
mod advanced;
mod appearance;
mod backdrop;
mod customize;
mod general;
pub(crate) mod input;
mod integrations;
pub(crate) mod kit;
mod log;
mod nav;
mod notifications;
mod onboarding;
mod persistence;
mod providers;
pub(crate) mod theme;
mod tray;
mod troubleshoot;
mod vscode_themes;
mod window;

#[cfg(test)]
mod tests;

pub(crate) use persistence::{persist_update, try_persist_update_fallible};
pub(crate) use providers::{OpenRouterSettingsSnapshot, persist_openrouter_credentials};

const WINDOW_WIDTH: f32 = 1000.0;
const WINDOW_HEIGHT: f32 = 740.0;
const ONBOARDING_WIDTH: f32 = 780.0;
const ONBOARDING_HEIGHT: f32 = 560.0;
pub(crate) fn settings_window_title() -> &'static str {
    crate::i18n::tr("codex-minibar-settings")
}
pub(crate) fn onboarding_window_title() -> &'static str {
    crate::i18n::tr("welcome-to-codex-minibar")
}

static SETTINGS_OPEN: AtomicBool = AtomicBool::new(false);
static ONBOARDING_OPEN: AtomicBool = AtomicBool::new(false);
static DISCOVERED_POPUP_BRICKS: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());
static OPENROUTER_SNAPSHOT: Mutex<Option<OpenRouterSettingsSnapshot>> = Mutex::new(None);

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
    SignIn(crate::instances::ProviderId),
    Sync(Box<Settings>),
    DiscoveredBricks(BTreeMap<String, String>),
    OpenRouter(OpenRouterSettingsSnapshot),
}

fn post(command: Command) {
    crate::popup_window::send_command(PopupCommand::Settings(command));
}

/// Open (or focus) the Settings window.
pub fn open() {
    post(Command::Open);
}

/// Optimistic local edits own the displayed settings while the serial writer
/// commits earlier snapshots. Do not roll the UI language back mid-queue.
pub(crate) fn has_pending_edits() -> bool {
    persistence::has_pending()
}

/// Open Settings on one account's page with its sign-in already started.
pub fn open_sign_in(provider: crate::instances::ProviderId) {
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

/// Push a committed settings snapshot into the open window.
pub fn sync_open_window(settings: Settings) {
    if SETTINGS_OPEN.load(Ordering::SeqCst) {
        post(Command::Sync(Box::new(settings)));
    }
}

pub(crate) fn cached_discovered_popup_bricks() -> BTreeMap<String, String> {
    DISCOVERED_POPUP_BRICKS
        .lock()
        .map(|labels| labels.clone())
        .unwrap_or_default()
}

fn discovered_popup_brick_labels(
    limits: &crate::limits::ProviderLimits,
) -> BTreeMap<String, String> {
    let mut labels = BTreeMap::new();
    for (provider, snapshot) in limits.iter() {
        for (brick_id, title) in
            crate::provider_registry::discovered_additional_brick_labels(provider.kind(), snapshot)
        {
            labels.insert(brick_id, title);
        }
    }
    labels
}

/// Publishes API-discovered additional windows so Settings can list them
/// immediately, using the provider-supplied titles rather than a hardcoded catalog.
pub fn publish_discovered_popup_bricks(limits: &crate::limits::ProviderLimits) {
    let labels = discovered_popup_brick_labels(limits);
    let changed = DISCOVERED_POPUP_BRICKS
        .lock()
        .map(|mut slot| {
            let changed = *slot != labels;
            *slot = labels.clone();
            changed
        })
        .unwrap_or(false);
    if changed && SETTINGS_OPEN.load(Ordering::SeqCst) {
        post(Command::DiscoveredBricks(labels));
    }
}

pub(crate) fn cached_openrouter_snapshot() -> OpenRouterSettingsSnapshot {
    OPENROUTER_SNAPSHOT
        .lock()
        .ok()
        .and_then(|snapshot| snapshot.clone())
        .unwrap_or_default()
}

/// Publishes the latest OpenRouter key labels, spend and balances of every
/// OpenRouter instance, plus each instance's signed-in account name, so
/// provider pages can show them without fetching anything themselves.
pub fn publish_openrouter_snapshot(limits: &crate::limits::ProviderLimits) {
    let snapshot = OpenRouterSettingsSnapshot::from_limits(limits);
    let changed = OPENROUTER_SNAPSHOT
        .lock()
        .map(|mut slot| {
            let changed = slot.as_ref() != Some(&snapshot);
            *slot = Some(snapshot.clone());
            changed
        })
        .unwrap_or(false);
    if changed && SETTINGS_OPEN.load(Ordering::SeqCst) {
        post(Command::OpenRouter(snapshot));
    }
}

/// Register key bindings used by the Settings windows. Called once when the
/// GPUI application starts.
pub(crate) fn init(cx: &mut gpui::App) {
    input::bind_keys(cx);
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
            open_onboarding_window(Arc::clone(state), cx);
        }
        Command::Sync(settings) => with_settings_window(cx, |root, cx| {
            root.apply_sync(*settings, cx);
        }),
        Command::DiscoveredBricks(labels) => with_settings_window(cx, |root, cx| {
            root.discovered_bricks = labels;
            cx.notify();
        }),
        Command::OpenRouter(snapshot) => with_settings_window(cx, |root, cx| {
            root.openrouter = snapshot;
            cx.notify();
        }),
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

fn focus_existing<V: 'static>(
    slot: &'static std::thread::LocalKey<RefCell<Option<WindowHandle<V>>>>,
    cx: &mut AsyncApp,
) -> bool {
    let Some(handle) = slot.with(|slot| *slot.borrow()) else {
        return false;
    };
    let alive = handle
        .update(cx, |_, window, _| window.activate_window())
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
        app_id: Some("CodexMinibar".into()),
        window_min_size: Some(size(px(min.0), px(min.1))),
        window_decorations: None,
        tabbing_identifier: None,
    }
}

fn open_settings_window(state: Arc<AppState>, cx: &mut AsyncApp) {
    let result = cx.update(|cx| {
        let options = window_options(
            settings_window_title(),
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            (640.0, 460.0),
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
            let _ = handle.update(cx, |_, window, _| window.activate_window());
        }
        Ok(Err(error)) => eprintln!("Could not open settings window: {error:#}"),
        Err(error) => eprintln!("Could not open settings window: {error:#}"),
    }
}

fn open_onboarding_window(state: Arc<AppState>, cx: &mut AsyncApp) {
    let result = cx.update(|cx| {
        let options = window_options(
            onboarding_window_title(),
            ONBOARDING_WIDTH,
            ONBOARDING_HEIGHT,
            (560.0, 420.0),
            cx,
        );
        cx.open_window(options, |window, cx| {
            cx.new(|cx| onboarding::OnboardingWindow::new(state, window, cx))
        })
    });
    match result {
        Ok(Ok(handle)) => {
            ONBOARDING_OPEN.store(true, Ordering::SeqCst);
            ONBOARDING_WINDOW.with(|slot| *slot.borrow_mut() = Some(handle));
            // The popup stays up beside onboarding as a live preview of
            // every choice (`is_open` keeps it from being dismissed).
            // Home lists every provider onboarding turns on.
            crate::popup_window::request_home_view();
            if !crate::popup::is_visible() || crate::popup::is_closing() {
                crate::popup::show_on_primary();
            }
            let _ = handle.update(cx, |_, window, _| window.activate_window());
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
