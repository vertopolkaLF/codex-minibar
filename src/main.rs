#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use codex_minibar::{
    app::{AppState, app},
    codex::{CodexActivator, CodexClient, first_available},
    popup::{POPUP_HEIGHT, POPUP_WIDTH},
    scheduler::ActivationState,
    settings::Settings,
    single_instance::SingleInstance,
    worker::start_worker,
};
use windows_reactor::*;

fn run() -> Result<()> {
    let path = Settings::default_path()?;
    let settings = Settings::load_or_create(&path)?;
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

    let state = Arc::new(AppState {
        settings,
        commands,
        worker: Mutex::new(worker),
        startup_error,
        last_activation_at,
    });

    App::new()
        .run_custom(move |_| {
            // Unlike `App::render`, this builds the WinUI host without calling
            // `Window::Activate`. The tray popup is the sole code path that
            // makes its HWND visible.
            let _host = Box::leak(Box::new(ReactorHost::new_with_window_options(
                "Codex Minibar",
                Some(WindowSize {
                    width: f64::from(POPUP_WIDTH),
                    height: f64::from(POPUP_HEIGHT),
                }),
                InnerConstraints {
                    min_width: Some(f64::from(POPUP_WIDTH)),
                    min_height: Some(f64::from(POPUP_HEIGHT)),
                    max_width: Some(f64::from(POPUP_WIDTH)),
                    max_height: Some(f64::from(POPUP_HEIGHT)),
                },
                Box::new(move |_: &(), cx: &mut RenderCx| app(cx, Arc::clone(&state))),
                |_| {},
            )?));
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
    if let Err(error) = run() {
        show_error(&format!("Codex Minibar failed: {error:#}"));
    }
    drop(instance);
}
