//! Single-instance ownership and activation.
//!
//! The first process holds an exclusive `flock` on a file in the user's
//! runtime directory and listens on a Unix socket next to it. A second
//! launch fails to take the lock, asks the first process through the socket
//! to show itself, and exits — the Linux counterpart of the named mutex and
//! `FindWindow` activation on Windows.
//!
//! `codex-minibar --toggle` asks it to toggle the popup instead. Bound to a
//! keyboard shortcut in the desktop's settings (`linux::keybinding` installs
//! it), that is the global hotkey on Wayland, where applications cannot grab
//! keys themselves. The desktop hands the launched command an
//! `XDG_ACTIVATION_TOKEN`; it travels with the request so the popup may take
//! focus.

use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::OpenOptionsExt,
            net::{UnixListener, UnixStream},
        },
    },
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};

const ACTIVATE: &[u8] = b"activate\n";
const TOGGLE: &str = "toggle";
/// Longest request line: the verb and an activation token.
const MAX_REQUEST: u64 = 512;
const TOKEN_LIFETIME: Duration = Duration::from_secs(5);
static ACTIVATION_TOKEN: Mutex<Option<(String, Instant)>> = Mutex::new(None);
/// The command-line flag that toggles the running popup.
pub const TOGGLE_FLAG: &str = "--toggle";

/// `$XDG_RUNTIME_DIR` (per-user, `0700`), or a per-user temp directory.
fn runtime_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR").filter(|dir| !dir.is_empty()) {
        return PathBuf::from(dir);
    }
    // SAFETY: getuid has no preconditions and cannot fail.
    let uid = unsafe { libc::getuid() };
    std::env::temp_dir().join(format!("codex-minibar-{uid}"))
}

fn paths() -> (PathBuf, PathBuf) {
    let suffix = if crate::fake::enabled() { "-fake" } else { "" };
    let dir = runtime_dir();
    (
        dir.join(format!("codex-minibar{suffix}.lock")),
        dir.join(format!("codex-minibar{suffix}.sock")),
    )
}

pub struct SingleInstance(File);

static HOLDER: Mutex<Option<File>> = Mutex::new(None);

impl SingleInstance {
    /// Returns `None` after asking the primary process to show itself.
    pub fn acquire_or_activate_existing() -> Result<Option<Self>> {
        let (lock_path, socket_path) = paths();
        if let Some(parent) = lock_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .mode(0o600)
            .open(&lock_path)
            .with_context(|| format!("open {}", lock_path.display()))?;
        // SAFETY: the descriptor stays valid for the lifetime of `file`.
        let locked = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;
        if !locked {
            activate_existing(&socket_path);
            return Ok(None);
        }
        // The lock proves no other instance is listening: a socket file left
        // behind by a crash is stale.
        let _ = std::fs::remove_file(&socket_path);
        match UnixListener::bind(&socket_path) {
            Ok(listener) => {
                std::thread::Builder::new()
                    .name("single-instance".into())
                    .spawn(move || serve(listener))
                    .context("spawn single-instance listener")?;
            }
            Err(error) => crate::logger::info(format!(
                "single-instance activation socket unavailable: {error}"
            )),
        }
        Ok(Some(Self(file)))
    }

    /// Keeps the lock alive for the process lifetime.
    pub fn hold(instance: Self) {
        if let Ok(mut holder) = HOLDER.lock() {
            holder.replace(instance.0);
        }
    }
}

/// Releases the lock before a relaunching exit.
pub fn release_for_update() {
    if let Ok(mut holder) = HOLDER.lock() {
        holder.take();
    }
}

fn activate_existing(socket_path: &std::path::Path) {
    match UnixStream::connect(socket_path) {
        Ok(mut stream) => {
            let toggle = std::env::args()
                .skip(1)
                .any(|argument| argument == TOGGLE_FLAG);
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
            let request = if toggle {
                match activation_token_from_env() {
                    Some(token) => format!("{TOGGLE} {token}\n").into_bytes(),
                    None => format!("{TOGGLE}\n").into_bytes(),
                }
            } else {
                ACTIVATE.to_vec()
            };
            let _ = stream.write_all(&request);
        }
        Err(error) => eprintln!("could not reach the running Codex Minibar: {error}"),
    }
}

fn serve(listener: UnixListener) {
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let mut request = Vec::with_capacity(64);
        if (&mut stream)
            .take(MAX_REQUEST)
            .read_to_end(&mut request)
            .is_err()
        {
            continue;
        }
        if request == ACTIVATE {
            activate();
        } else if let Some(token) = parse_toggle(&request) {
            if let (Some(token), Ok(mut pending)) = (token, ACTIVATION_TOKEN.lock()) {
                *pending = Some((token, Instant::now()));
            }
            toggle();
        }
    }
}

/// The token the desktop gave the launched `--toggle` process, if any.
fn activation_token_from_env() -> Option<String> {
    std::env::var("XDG_ACTIVATION_TOKEN")
        .ok()
        .filter(|token| is_token(token))
}

fn is_token(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= 256
        && token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

/// `Some(token)` for a toggle request, `None` for anything else.
fn parse_toggle(request: &[u8]) -> Option<Option<String>> {
    let line = std::str::from_utf8(request).ok()?.strip_suffix('\n')?;
    let rest = line.strip_prefix(TOGGLE)?;
    if rest.is_empty() {
        return Some(None);
    }
    let token = rest.strip_prefix(' ')?;
    is_token(token).then(|| Some(token.to_owned()))
}

/// A focus token received with the latest toggle, valid once and briefly.
pub fn take_activation_token() -> Option<String> {
    ACTIVATION_TOKEN
        .lock()
        .ok()?
        .take()
        .filter(|(_, received)| received.elapsed() < TOKEN_LIFETIME)
        .map(|(token, _)| token)
}

/// The shell command that toggles the popup, for a desktop keyboard
/// shortcut. AppImages are addressed by their file, not the mount point.
pub fn toggle_command() -> String {
    let program = std::env::var_os("APPIMAGE")
        .filter(|path| !path.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_exe().ok())
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "codex-minibar".into());
    let program = if program.contains(|c: char| c.is_whitespace() || c == '\'') {
        format!("'{}'", program.replace('\'', "'\\''"))
    } else {
        program
    };
    format!("{program} {TOGGLE_FLAG}")
}

/// Like a tray click: closes the popup, or opens it.
fn toggle() {
    if crate::popup::is_visible() && !crate::popup::is_closing() {
        // Nothing opens, so nothing needs the focus token.
        let _ = take_activation_token();
        if !crate::settings_window::is_open() {
            crate::popup::hide();
        }
    } else {
        crate::popup::show_near_cursor();
    }
    super::wake::wake();
}

/// Mirrors the Windows behavior: focus Settings when it is open, otherwise
/// show the popup.
fn activate() {
    if crate::settings_window::is_open() {
        crate::settings_window::open();
    } else if !crate::popup::is_visible() || crate::popup::is_closing() {
        crate::popup::show_near_cursor();
    }
    super::wake::wake();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_requests_carry_an_optional_valid_focus_token() {
        assert_eq!(parse_toggle(b"toggle\n"), Some(None));
        assert_eq!(
            parse_toggle(b"toggle abc-123_x.y:z\n"),
            Some(Some("abc-123_x.y:z".to_owned()))
        );
        assert_eq!(parse_toggle(b"toggle \n"), None);
        assert_eq!(parse_toggle(b"toggle a b\n"), None);
        assert_eq!(parse_toggle(b"toggle abc"), None);
        assert_eq!(parse_toggle(b"togglex\n"), None);
        assert_eq!(parse_toggle(ACTIVATE), None);
    }
}
