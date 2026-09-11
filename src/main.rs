#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::{
    
    sync::{Arc, Mutex, mpsc},
};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use codex_minibar::{
    gpui_ui::AppState,
    notifications,
    
    provider::start_enabled_workers,
    scheduler::ActivationState,
    settings::Settings,
    single_instance::{self, SingleInstance},
    store,
    updater::{
        UpdateController, show_post_update_success_if_needed, sync_installed_display_version,
    },
    worker::WorkerEvent,
};


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
    let hydrated_limits = store::shared()
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
    let (workers, startup_provider_errors) =
        start_enabled_workers(&settings, activation_path.clone(), worker_events_tx.clone());
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

    let state = Arc::new(AppState {
        settings,
        limits: Mutex::new(hydrated_limits),
        commands: Mutex::new(commands),
        workers: Mutex::new(workers),
        worker_events_rx: Mutex::new(Some(worker_events_rx)),
        worker_events_tx,
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

    codex_minibar::gpui_ui::run(state)
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
    if notifications::launched_via_toast_update() {
        let _ = notifications::publish_toast_update_request();
    }
    if let Err(error) = run() {
        show_error(&format!("Codex Minibar failed: {error:#}"));
    }
    single_instance::release_for_update();
}

