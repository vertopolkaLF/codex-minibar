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

fn run() -> Result<()> {
    notifications::initialize();
    sync_installed_display_version();
    let path = Settings::default_path()?;
    codex_minibar::logger::initialize(&path)?;
    if let Err(error) = codex_minibar::pricing::initialize() {
        eprintln!("failed to hydrate pricing catalog: {error:#}");
    }
    let mut settings = Settings::load_or_create(&path)?;
    settings.language.apply();
    show_post_update_success_if_needed();
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
    let hydrated_limits = store::shared()
        .and_then(|shared| {
            shared
                .lock()
                .map_err(|_| anyhow!("provider store lock poisoned"))?
                .hydrate_provider_limits(&settings.provider_ids(), settings.history_retention_days)
        })
        .unwrap_or_else(|error| {
            eprintln!("failed to hydrate provider store: {error:#}");
            Default::default()
        });
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

    // Initializes the tray accent before any popup can be shown.
    codex_minibar::theme::apply_appearance(state.settings.theme, state.settings.accent_color);
    codex_minibar::popup_window::start(Arc::clone(&state));
    if onboarding_needed {
        codex_minibar::settings_window::open_onboarding();
    }
    // The GPUI thread owns every window and the bridge thread owns the tray;
    // both exit the process directly, so the main thread only stays alive.
    loop {
        std::thread::park();
    }
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
