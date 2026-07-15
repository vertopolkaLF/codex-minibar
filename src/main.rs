#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::{
    rc::Rc,
    sync::{mpsc, Arc, Mutex},
};

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use codex_minibar::{
    app::{app, AppState},
    notifications,
    popup::{self, FALLBACK_CLIENT_HEIGHT_LIMIT, POPUP_WIDTH},
    scheduler::ActivationState,
    settings::Settings,
    single_instance::{self, SingleInstance},
    updater::{
        show_post_update_success_if_needed, sync_installed_display_version, UpdateController,
    },
    provider::start_enabled_workers,
    worker::WorkerEvent,
};
use windows_reactor::*;

fn run() -> Result<()> {
    notifications::initialize();
    sync_installed_display_version();
    show_post_update_success_if_needed();
    let path = Settings::default_path()?;
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
    let (workers, startup_errors) =
        start_enabled_workers(&settings, activation_path.clone(), worker_events_tx.clone());
    let commands = workers
        .iter()
        .map(|(provider, worker)| (*provider, worker.commands.clone()))
        .collect();
    let startup_error = (!startup_errors.is_empty()).then(|| startup_errors.join("\n"));

    let (settings_tx, settings_rx) = mpsc::channel();
    let updates = UpdateController::new();
    if settings.check_for_updates {
        updates.check_async(true, settings.notifications.update_available);
    }
    let initial_height = popup::height_for(settings.hide_plan_credits, startup_error.as_deref())
        // Oversize the first frame so Auto content can measure without clipping;
        // SizeChanged then shrinks the HWND to the real content height.
        .saturating_add(80)
        .min(FALLBACK_CLIENT_HEIGHT_LIMIT);
    popup::set_client_height_dip(initial_height);
    let state = Arc::new(AppState {
        settings,
        limits: Mutex::new(Default::default()),
        commands: Mutex::new(commands),
        workers: Mutex::new(workers),
        worker_events_rx: Mutex::new(Some(worker_events_rx)),
        worker_events_tx,
        activation_path,
        startup_error,
        last_activation_at,
        settings_tx,
        settings_rx: Mutex::new(Some(settings_rx)),
        updates: Arc::clone(&updates),
    });
    codex_minibar::updater::install_runtime(Arc::clone(&updates), {
        let state = Arc::clone(&state);
        move || state.shutdown_worker()
    });

    App::new()
        .run_custom(move |_| {
            // Unlike `App::render`, this builds the WinUI host without calling
            // `Window::Activate`. The tray popup is the sole code path that
            // makes its HWND visible.
            let host = Rc::new(ReactorHost::new_with_window_options(
                "Codex Minibar",
                Some(WindowSize {
                    width: f64::from(POPUP_WIDTH),
                    height: f64::from(initial_height),
                }),
                InnerConstraints {
                    min_width: Some(f64::from(POPUP_WIDTH)),
                    // Keep min tiny — OverlappedPresenter preferred-min was blocking shrink.
                    min_height: Some(80.0),
                    max_width: Some(f64::from(POPUP_WIDTH)),
                    // The actual 80% cap is selected from the monitor at
                    // popup-show time. A fixed 640 DIP creation constraint
                    // cannot be raised reliably by AppWindow later.
                    max_height: Some(f64::from(FALLBACK_CLIENT_HEIGHT_LIMIT)),
                },
                Box::new(move |_: &(), cx: &mut RenderCx| app(cx, Arc::clone(&state))),
                |_| {},
            )?);
            popup::register_host(Rc::clone(&host));
            let _host = Box::leak(Box::new(host));
            Ok(())
        })
        .map_err(|error| anyhow!("windows-reactor failed: {error:?}"))
}

fn show_error(message: &str) {
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

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
