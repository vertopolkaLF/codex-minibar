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
    scheduler::ActivationState,
    settings::Settings,
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

    let (commands, events, startup_error, _worker_join) = match executable {
        Ok(executable) => {
            let worker = start_worker(
                CodexClient::new(&executable),
                CodexActivator::new(executable),
                activation_path,
                settings.automatic_activation,
                Duration::from_secs(60),
            );
            let (commands, events, join) = worker.into_parts();
            (Some(commands), Some(events), None, Some(join))
        }
        Err(error) => (None, None, Some(error.to_string()), None),
    };

    let state = Arc::new(AppState {
        settings,
        commands,
        events: Mutex::new(events),
        startup_error,
        last_activation_at,
    });

    App::new()
        .title("Codex Minibar")
        .inner_size(380.0, 460.0)
        .inner_constraints(InnerConstraints {
            min_width: Some(360.0),
            min_height: Some(420.0),
            max_width: Some(420.0),
            max_height: Some(520.0),
        })
        .backdrop(Backdrop::Acrylic)
        .render(move |cx| app(cx, Arc::clone(&state)))
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
    if let Err(error) = run() {
        show_error(&format!("Codex Minibar failed: {error:#}"));
    }
}
