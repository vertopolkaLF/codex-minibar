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
            fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};

const ACTIVATE: &str = "activate";
const TOGGLE: &str = "toggle";
/// Longest request line: the verb and an activation token.
const MAX_REQUEST: u64 = 512;
const TOKEN_LIFETIME: Duration = Duration::from_secs(5);
static ACTIVATION_TOKEN: Mutex<Option<(String, Instant)>> = Mutex::new(None);
/// The command-line flag that toggles the running popup.
pub const TOGGLE_FLAG: &str = "--toggle";

/// `$XDG_RUNTIME_DIR` (per-user, `0700`), or a per-user temp directory.
fn runtime_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
    {
        return Ok(dir);
    }
    // SAFETY: getuid has no preconditions and cannot fail.
    let uid = unsafe { libc::getuid() };
    let dir = std::env::temp_dir().join(format!("codex-minibar-{uid}"));
    prepare_fallback_dir(&dir, uid)?;
    Ok(dir)
}

fn prepare_fallback_dir(dir: &std::path::Path, uid: u32) -> Result<()> {
    match std::fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error).context("create single-instance runtime directory"),
    }
    let metadata = std::fs::symlink_metadata(dir).context("inspect runtime directory")?;
    ensure!(
        metadata.is_dir() && metadata.uid() == uid,
        "runtime directory must be owned by the current user and must not be a symlink"
    );
    // Repair the app-owned fallback created by older versions with default
    // permissions. Never change permissions on the desktop's runtime dir.
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
        .context("make single-instance runtime directory private")
}

fn paths() -> Result<(PathBuf, PathBuf)> {
    let suffix = if crate::fake::enabled() { "-fake" } else { "" };
    let dir = runtime_dir()?;
    Ok((
        dir.join(format!("codex-minibar{suffix}.lock")),
        dir.join(format!("codex-minibar{suffix}.sock")),
    ))
}

pub struct SingleInstance(File);

static HOLDER: Mutex<Option<File>> = Mutex::new(None);

impl SingleInstance {
    /// Returns `None` after asking the primary process to show itself.
    pub fn acquire_or_activate_existing() -> Result<Option<Self>> {
        let (lock_path, socket_path) = paths()?;
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
        if !try_lock(&file).context("lock single-instance file")? {
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

fn try_lock(file: &File) -> std::io::Result<bool> {
    loop {
        // SAFETY: the descriptor stays valid for the lifetime of `file`.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            return Ok(true);
        }
        let error = std::io::Error::last_os_error();
        match error.kind() {
            std::io::ErrorKind::Interrupted => continue,
            std::io::ErrorKind::WouldBlock => return Ok(false),
            _ => return Err(error),
        }
    }
}

fn activate_existing(socket_path: &std::path::Path) {
    match UnixStream::connect(socket_path) {
        Ok(mut stream) => {
            let toggle = std::env::args()
                .skip(1)
                .any(|argument| argument == TOGGLE_FLAG);
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
            let verb = if toggle { TOGGLE } else { ACTIVATE };
            let request = activation_request(verb, activation_token_from_env().as_deref());
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
        if let Some(token) = parse_request(&request, ACTIVATE) {
            set_activation_token(token);
            activate();
        } else if let Some(token) = parse_request(&request, TOGGLE) {
            toggle_with_token(token);
        }
    }
}

fn activation_request(verb: &str, token: Option<&str>) -> Vec<u8> {
    match token.filter(|token| is_token(token)) {
        Some(token) => format!("{verb} {token}\n").into_bytes(),
        None => format!("{verb}\n").into_bytes(),
    }
}

/// The token the desktop gave the launched process, if any.
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

/// `Some(token)` for the requested verb, `None` for anything else.
fn parse_request(request: &[u8], verb: &str) -> Option<Option<String>> {
    let line = std::str::from_utf8(request).ok()?.strip_suffix('\n')?;
    let rest = line.strip_prefix(verb)?;
    if rest.is_empty() {
        return Some(None);
    }
    let token = rest.strip_prefix(' ')?;
    is_token(token).then(|| Some(token.to_owned()))
}

/// A focus token received with the latest activation request, valid once and briefly.
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
    let program = super::autostart::executable()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "codex-minibar".into());
    let program = quote_command_argument(&program);
    format!("{program} {TOGGLE_FLAG}")
}

// GNOME parses this with GLib shell quoting. Quote every path, including
// paths containing metacharacters without whitespace.
fn quote_command_argument(program: &str) -> String {
    format!("'{}'", program.replace('\'', "'\\''"))
}

/// Toggles the popup for an outside request, such as `--toggle` or the
/// D-Bus service, with the desktop's focus token for it when there is one.
pub(crate) fn toggle_with_token(token: Option<String>) {
    set_activation_token(token);
    toggle();
}

fn set_activation_token(token: Option<String>) {
    if let Ok(mut pending) = ACTIVATION_TOKEN.lock() {
        *pending = token
            .filter(|token| is_token(token))
            .map(|token| (token, Instant::now()));
    }
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
    } else {
        // Show also activates an already visible (for example pinned) popup.
        crate::popup::show_near_cursor();
    }
    super::wake::wake();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_runtime_directory_is_private_and_rejects_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("runtime");
        // SAFETY: getuid has no preconditions.
        let uid = unsafe { libc::getuid() };
        prepare_fallback_dir(&dir, uid).unwrap();
        assert_eq!(std::fs::metadata(&dir).unwrap().mode() & 0o777, 0o700);
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        prepare_fallback_dir(&dir, uid).unwrap();
        assert_eq!(std::fs::metadata(&dir).unwrap().mode() & 0o777, 0o700);
        let link = root.path().join("link");
        std::os::unix::fs::symlink(&dir, &link).unwrap();
        assert!(prepare_fallback_dir(&link, uid).is_err());
        assert!(prepare_fallback_dir(&dir, uid.wrapping_add(1)).is_err());
    }

    #[test]
    fn only_lock_contention_reports_an_existing_instance() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let first = OpenOptions::new().write(true).open(file.path()).unwrap();
        let second = OpenOptions::new().write(true).open(file.path()).unwrap();
        assert!(try_lock(&first).unwrap());
        assert!(!try_lock(&second).unwrap());
        drop(first);
        assert!(try_lock(&second).unwrap());
    }

    #[test]
    fn shortcut_paths_are_always_shell_quoted() {
        assert_eq!(quote_command_argument("/opt/$apps/a;b"), "'/opt/$apps/a;b'");
        assert_eq!(
            quote_command_argument("/opt/it's/minibar"),
            "'/opt/it'\\''s/minibar'"
        );
    }

    #[test]
    fn focus_tokens_are_consumed_once_and_replaced_by_each_request() {
        set_activation_token(Some("first".into()));
        assert_eq!(take_activation_token().as_deref(), Some("first"));
        assert_eq!(take_activation_token(), None);
        set_activation_token(Some("stale".into()));
        set_activation_token(None);
        assert_eq!(take_activation_token(), None);
        set_activation_token(Some("stale".into()));
        set_activation_token(Some("invalid token".into()));
        assert_eq!(take_activation_token(), None);
    }

    #[test]
    fn launcher_and_shortcut_requests_preserve_focus_tokens() {
        for verb in [ACTIVATE, TOGGLE] {
            for token in [None, Some("desktop-token:123")] {
                let request = activation_request(verb, token);
                assert_eq!(
                    parse_request(&request, verb),
                    Some(token.map(str::to_owned))
                );
            }
            assert_eq!(parse_request(b"activatex\n", verb), None);
            assert_eq!(parse_request(b"activate token\nextra\n", verb), None);
        }
    }

    #[test]
    fn toggle_requests_carry_an_optional_valid_focus_token() {
        assert_eq!(parse_request(b"toggle\n", TOGGLE), Some(None));
        assert_eq!(
            parse_request(b"toggle abc-123_x.y:z\n", TOGGLE),
            Some(Some("abc-123_x.y:z".to_owned()))
        );
        assert_eq!(parse_request(b"toggle \n", TOGGLE), None);
        assert_eq!(parse_request(b"toggle a b\n", TOGGLE), None);
        assert_eq!(parse_request(b"toggle abc", TOGGLE), None);
        assert_eq!(parse_request(b"togglex\n", TOGGLE), None);
        assert_eq!(parse_request(b"activate\n", TOGGLE), None);
    }
}
