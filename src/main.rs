#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::sync::{Arc, Mutex, mpsc};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use codex_minibar::{
    app::AppState,
    notifications,
    provider::start_enabled_workers_with_limits,
    reset_feed,
    scheduler::ActivationState,
    settings::Settings,
    single_instance::{self, SingleInstance},
    store, troubleshoot,
    updater::{
        UpdateController, show_post_update_success_if_needed, sync_installed_display_version,
    },
    worker::WorkerEvent,
};
use windows_reactor::*;

fn run() -> Result<()> {
    notifications::initialize();
    sync_installed_display_version();
    show_post_update_success_if_needed();
    let path = Settings::default_path()?;
    codex_minibar::logger::initialize(&path)?;
    if let Err(error) = codex_minibar::pricing::initialize() {
        eprintln!("failed to hydrate pricing catalog: {error:#}");
    }
    let mut settings = Settings::load_or_create(&path)?;
    if let Err(error) = settings.reconcile_startup_from_registry(&path) {
        eprintln!("failed to reconcile startup setting: {error:#}");
    }
    if let Err(error) = settings.apply_runtime_effects() {
        eprintln!("failed to apply startup registration: {error:#}");
    }
    let activation_path = path.with_file_name("activation.toml");
    let last_activation_at: Option<DateTime<Utc>> =
        ActivationState::load_or_default(&activation_path)
            .ok()
            .and_then(|state| state.last_attempt_at);

    let (worker_events_tx, worker_events_rx) = mpsc::channel::<WorkerEvent>();
    let reset_feed_worker = reset_feed::start_worker(
        &settings,
        reset_feed::cache_path(&path),
        worker_events_tx.clone(),
    );
    let mut hydrated_limits = store::shared()
        .and_then(|shared| {
            shared
                .lock()
                .map_err(|_| anyhow!("provider store lock poisoned"))?
                .hydrate_provider_limits(settings.history_retention_days)
        })
        .unwrap_or_else(|error| {
            eprintln!("failed to hydrate provider store: {error:#}");
            Default::default()
        });
    let codex = codex_minibar::settings::ProviderKind::Codex;
    let retained =
        codex_minibar::codex::prepare_startup_limits(hydrated_limits.get(codex), &settings);
    *hydrated_limits.get_mut(codex) = retained;
    let (workers, startup_provider_errors) = start_enabled_workers_with_limits(
        &settings,
        activation_path.clone(),
        worker_events_tx.clone(),
        &hydrated_limits,
    );
    let commands = workers
        .iter()
        .map(|(provider, worker)| (*provider, worker.commands.clone()))
        .collect();
    let (settings_tx, settings_rx) = mpsc::channel();
    let (usage_actions_tx, usage_actions_rx) = mpsc::channel();
    let updates = UpdateController::new();
    if settings.check_for_updates {
        updates.check_async(true, settings.notifications.update_available);
    }
    let onboarding_needed = !settings.onboarding_completed;
    let state = Arc::new(AppState {
        settings,
        limits: Mutex::new(hydrated_limits),
        forced_resets: Mutex::new(Vec::new()),
        commands: Mutex::new(commands),
        workers: Mutex::new(workers),
        worker_events_rx: Mutex::new(Some(worker_events_rx)),
        worker_events_tx,
        reset_feed_worker: Mutex::new(Some(reset_feed_worker)),
        activation_path,
        startup_provider_errors,
        last_activation_at,
        settings_tx,
        settings_rx: Mutex::new(Some(settings_rx)),
        usage_actions_tx,
        usage_actions_rx: Mutex::new(Some(usage_actions_rx)),
        updates: Arc::clone(&updates),
    });
    codex_minibar::updater::install_runtime(Arc::clone(&updates), {
        let state = Arc::clone(&state);
        move || state.shutdown_worker()
    });

    App::new()
        .run_custom(move |_| {
            codex_minibar::theme::apply_appearance(
                state.settings.theme,
                state.settings.accent_color,
            );
            // The popup itself is a GPUI window on its own thread. WinUI keeps
            // this thread for Settings; a never-activated host window keeps
            // the XAML dispatcher alive after the last Settings window closes.
            let keepalive = ReactorHost::new_with_window_options(
                "Codex Minibar Host",
                Some(WindowSize {
                    width: 1.0,
                    height: 1.0,
                }),
                InnerConstraints {
                    min_width: None,
                    min_height: None,
                    max_width: None,
                    max_height: None,
                },
                Box::new(|_: &(), _: &mut RenderCx| Element::Empty),
                |_| {},
            )?;
            let _ = keepalive.set_shown_in_switchers(false);
            let ui_dispatcher = WinUIDispatcher::for_current_thread()?.marshaller();
            codex_minibar::popup_window::start(Arc::clone(&state), ui_dispatcher);
            if onboarding_needed {
                // First launch configures providers before any worker has a
                // chance to poll. The regular popup stays parked until Done.
                codex_minibar::settings_window::open_onboarding(state.settings_tx.clone())?;
            }
            let _host = Box::leak(Box::new(keepalive));
            Ok(())
        })
        .map_err(|error| anyhow!("windows-reactor failed: {error:?}"))
}

fn show_error(message: &str) {
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};

        let text: Vec<u16> = OsStr::new(message)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let caption: Vec<u16> = OsStr::new("Codex Minibar")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                caption.as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }
    #[cfg(not(windows))]
    {
        eprintln!("{message}");
    }
}

fn main() {
    if troubleshoot::is_cli_request() {
        if let Err(error) = troubleshoot::launch_cli_picker() {
            show_error(&format!(
                "Could not start Codex Minibar troubleshooting: {error:#}"
            ));
        }
        return;
    }

    let instance = match SingleInstance::acquire_or_activate_existing() {
        Ok(Some(instance)) => instance,
        Ok(None) => return,
        Err(error) => {
            show_error(&format!(
                "Codex Minibar could not enforce a single instance: {error:#}"
            ));
            return;
        }
    };
    single_instance::SingleInstance::hold(instance);
    if let Err(error) = codex_minibar::claude::cleanup_abandoned_logins() {
        eprintln!("could not clean abandoned Claude sign-in directories: {error:#}");
    }
    if let Err(error) = codex_minibar::codex::cleanup_abandoned_logins() {
        eprintln!("could not clean abandoned Codex sign-in directories: {error:#}");
    }
    if notifications::launched_via_toast_update() {
        let _ = notifications::publish_toast_update_request();
    }
    if let Err(error) = run() {
        show_error(&format!("Codex Minibar failed: {error:#}"));
    }
    single_instance::release_for_update();
}
