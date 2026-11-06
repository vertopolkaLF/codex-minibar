//! Hands URLs and files to the Windows shell.

use std::path::Path;

use anyhow::{Context, Result, bail};

pub fn open_url(url: &str) -> Result<()> {
    open_shell_target(url.into(), url.to_owned())
}

pub fn open_path(path: &Path) -> Result<()> {
    open_shell_target(path.as_os_str().to_owned(), path.display().to_string())
}

/// Opens the target on a dedicated thread. ShellExecuteW pumps window
/// messages while it runs; on the UI thread that dispatches queued GPUI
/// foreground tasks re-entrantly from inside a click handler, while the app
/// is already borrowed, and the resulting panic inside the window procedure
/// aborts the process. Failures are therefore logged by the worker.
fn open_shell_target(target: std::ffi::OsString, description: String) -> Result<()> {
    std::thread::Builder::new()
        .name("shell-open".into())
        .spawn(move || {
            if let Err(error) = shell_execute(&target, &description) {
                crate::logger::info(format!("{error:#}"));
            }
        })
        .context("spawn shell-open thread")?;
    Ok(())
}

fn shell_execute(target: &std::ffi::OsStr, description: &str) -> Result<()> {
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::System::Com::{
            COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
        };
        use windows_sys::Win32::UI::Shell::ShellExecuteW;

        let operation: Vec<u16> = OsStr::new("open")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let target: Vec<u16> = target.encode_wide().chain(std::iter::once(0)).collect();
        // Shell extensions behind ShellExecuteW may require an STA.
        let com = unsafe {
            CoInitializeEx(
                std::ptr::null(),
                (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
            )
        };
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                target.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if com >= 0 {
            unsafe { CoUninitialize() };
        }
        if result as isize <= 32 {
            bail!("ShellExecuteW failed for {description}");
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = target;
        let _ = description;
        bail!("opening URLs is only supported on Windows")
    }
}
