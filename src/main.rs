#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::sync::{Arc, Mutex, mpsc};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use codex_minibar::{
    app::AppState,
    app::event::AppEvent,
    app::single_instance::{self, SingleInstance},
    app::troubleshoot,
    platform::logger,
    platform::paths,
    providers::scheduler::ActivationState,
    providers::start_enabled_workers,
    providers::status,
    quota::reset_feed,
    settings::runtime,
    store,
    surfaces::notifications,
    update::{self, UpdateController},
};

fn run() -> Result<()> {
    codex_minibar::quota::banked_reset::mark_process_start();
    codex_minibar::app::install_notification_host();
    notifications::initialize();
    update::sync_installed_display_version();
    let path = paths::settings()?;
    if paths::fake_sandbox() {
        codex_minibar::providers::fake::seed_settings()?;
    }
    // Bootstrap order: logger, settings and every process-wide effect of
    // them (language first), then workers, then windows.
    logger::initialize(&paths::log()?)?;
    if let Err(error) = codex_minibar::providers::claude::cleanup_abandoned_logins() {
        logger::info(format!(
            "could not clean abandoned Claude sign-in directories: {error:#}"
        ));
    }
    if let Err(error) = codex_minibar::providers::codex::cleanup_abandoned_logins() {
        logger::info(format!(
            "could not clean abandoned Codex sign-in directories: {error:#}"
        ));
    }
    if let Err(error) = codex_minibar::usage::pricing::initialize() {
        logger::info(format!("failed to hydrate pricing catalog: {error:#}"));
    }
    let settings = runtime::load_at_startup(&path)?;
    let hydrated_limits = store::shared()
        .and_then(|shared| {
            let store = shared
                .lock()
                .map_err(|_| anyhow!("provider store lock poisoned"))?;
            // Drop rows of removed instances; disabled instances keep their data.
            if let Err(error) = store.prune_unknown_providers(&settings.provider_ids()) {
                logger::info(format!("failed to prune unknown providers: {error:#}"));
            }
            store.hydrate_provider_limits(&settings.provider_ids(), settings.history_retention_days)
        })
        .unwrap_or_else(|error| {
            logger::info(format!("failed to hydrate provider store: {error:#}"));
            Default::default()
        });
    // Settings start the taskbar widgets; they open on the stored quotas.
    codex_minibar::surfaces::taskbar::publish_limits(&hydrated_limits);
    let (settings_tx, settings_rx) = mpsc::channel();
    runtime::start(&settings, settings_tx);
    update::finish_startup();
    std::thread::spawn(|| {
        if let Err(error) = codex_minibar::providers::codex::discovery::cleanup() {
            logger::info(format!(
                "Codex CLI startup cache cleanup deferred: {error:#}"
            ));
        }
    });
    let activation_path = paths::activation_state()?;
    let last_activation_at: Option<DateTime<Utc>> =
        match ActivationState::load_or_default(&activation_path) {
            Ok(state) => state.last_attempt_at,
            Err(error) => {
                logger::info(format!("failed to load activation state: {error:#}"));
                None
            }
        };

    let (worker_events_tx, worker_events_rx) = mpsc::channel::<AppEvent>();
    let reset_feed_worker = reset_feed::start_worker(&settings, paths::reset_feed_cache()?, {
        let events = worker_events_tx.clone();
        move |snapshot| {
            let _ = events.send(AppEvent::ForcedResetsUpdated(snapshot));
        }
    });
    let provider_status_worker =
        status::start_worker(&settings, paths::provider_status_cache()?, {
            let events = worker_events_tx.clone();
            move |snapshot| {
                let _ = events.send(AppEvent::ProviderStatusUpdated(snapshot));
            }
        });
    let (workers, startup_provider_errors) =
        start_enabled_workers(&settings, activation_path.clone(), worker_events_tx.clone());
    let commands = workers
        .iter()
        .map(|(provider, worker)| (*provider, worker.commands.clone()))
        .collect();
    let (usage_actions_tx, usage_actions_rx) = mpsc::channel();
    let updates = UpdateController::new();
    if settings.check_for_updates {
        updates.check_async(settings.notifications.update_available);
    }
    let onboarding_needed = !settings.onboarding_completed;
    let state = Arc::new(AppState {
        settings,
        limits: Mutex::new(hydrated_limits),
        forced_resets: Mutex::new(Vec::new()),
        provider_statuses: Mutex::new(Vec::new()),
        changes: Default::default(),
        commands: Mutex::new(commands),
        workers: Mutex::new(workers),
        worker_events_rx: Mutex::new(Some(worker_events_rx)),
        worker_events_tx,
        reset_feed_worker: Mutex::new(Some(reset_feed_worker)),
        provider_status_worker: Mutex::new(Some(provider_status_worker)),
        activation_path,
        startup_provider_errors,
        last_activation_at,
        settings_rx: Mutex::new(Some(settings_rx)),
        usage_actions_tx,
        usage_actions_rx: Mutex::new(Some(usage_actions_rx)),
        updates: Arc::clone(&updates),
    });
    codex_minibar::update::install_runtime(Arc::clone(&updates), {
        let state = Arc::clone(&state);
        move || state.shutdown_worker()
    });

    codex_minibar::app::start(Arc::clone(&state));
    codex_minibar::update::whats_new::start(&state.settings);
    if onboarding_needed {
        codex_minibar::ui::settings_window::open_onboarding();
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
        use codex_minibar::platform::win32::wide;
        use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};

        let text = wide(message);
        let caption = wide("Codex Minibar");
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
    if let Err(error) = run() {
        show_error(&format!("Codex Minibar failed: {error:#}"));
    }
    single_instance::release_for_update();
}
