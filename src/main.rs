#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::{
    sync::{
        mpsc,
        Arc, Mutex,
    },
    rc::Rc,
    time::Duration,
};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use codex_minibar::{
    app::{AppState, app},
    codex::{CodexActivator, CodexClient, first_available},
    notifications,
    popup::{self, POPUP_HEIGHT_MAX, POPUP_WIDTH},
    scheduler::ActivationState,
    settings::Settings,
    single_instance::{self, SingleInstance},
    updater::{UpdateController, show_post_update_success_if_needed, sync_installed_display_version},
    worker::start_worker,
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
    let executable = first_available(settings.codex_path.as_deref());

    let activation_path = path.with_file_name("activation.toml");
    let last_activation_at: Option<DateTime<Utc>> =
        ActivationState::load_or_default(&activation_path)
            .ok()
            .and_then(|state| state.last_attempt_at);

    let (commands, worker, startup_error) = match executable {
        Ok(executable) => {
            let worker = start_worker(
                CodexClient::new(&executable),
                CodexActivator::new(executable),
                activation_path,
                settings.automatic_activation,
                Duration::from_secs(60),
            );
            (Some(worker.commands.clone()), Some(worker), None)
        }
        Err(error) => (None, None, Some(error.to_string())),
    };

    let (settings_tx, settings_rx) = mpsc::channel();
    let updates = UpdateController::new();
    if settings.check_for_updates {
        updates.check_async(
            true,
            settings.notifications.update_available,
        );
    }
    let initial_height = popup::height_for(settings.hide_plan_credits, startup_error.as_deref())
    // Oversize the first frame so Auto content can measure without clipping;
    // SizeChanged then shrinks the HWND to the real content height.
    .saturating_add(80)
    .min(popup::POPUP_HEIGHT_MAX);
    popup::set_client_height_dip(initial_height);
    let state = Arc::new(AppState {
        settings,
        limits: Mutex::new(Default::default()),
        commands,
        worker: Mutex::new(worker),
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
                    max_height: Some(f64::from(POPUP_HEIGHT_MAX)),
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
