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
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

const ACTIVATE: &str = "activate";
const TOGGLE: &str = "toggle";
/// Longest request line: the verb and an activation token.
const MAX_REQUEST: u64 = 8192;
const MAX_TOKEN_BYTES: usize = 4096;
const TOKEN_LIFETIME: Duration = Duration::from_secs(5);
static ACTIVATION_TOKEN: Mutex<Option<(String, Instant)>> = Mutex::new(None);
static STARTUP_ACTIVATION_TOKEN: OnceLock<Mutex<Option<String>>> = OnceLock::new();
/// The command-line flag that toggles the running popup.
pub const TOGGLE_FLAG: &str = "--toggle";
/// Desktop launchers show the app on the first launch too. Autostart entries
/// omit this flag so an ordinary login can stay in the background.
const SHOW_FLAG: &str = "--show";

/// Saves the launcher's one-use focus grant without passing it on to child
/// processes. Call at the very beginning of `main`, before desktop services,
/// provider workers or any other threads have started.
///
/// # Safety
/// No other thread may access the process environment during this call.
pub unsafe fn capture_startup_activation_token() {
    STARTUP_ACTIVATION_TOKEN.get_or_init(|| {
        let token = std::env::var("XDG_ACTIVATION_TOKEN")
            .ok()
            .filter(|token| is_token(token));
        // SAFETY: the caller guarantees exclusive access to the environment.
        unsafe { std::env::remove_var("XDG_ACTIVATION_TOKEN") };
        Mutex::new(token)
    });
}

fn take_startup_activation_token() -> Option<String> {
    STARTUP_ACTIVATION_TOKEN.get()?.lock().ok()?.take()
}

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

fn connect_with_retry(
    mut connect: impl FnMut() -> std::io::Result<UnixStream>,
    timeout: Duration,
) -> std::io::Result<UnixStream> {
    let started = Instant::now();
    loop {
        match connect() {
            Ok(stream) => return Ok(stream),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                ) && started.elapsed() < timeout =>
            {
                // The primary can own the lock before it binds its socket.
                // A stale socket may also exist during this brief interval.
                std::thread::sleep(
                    Duration::from_millis(20).min(timeout.saturating_sub(started.elapsed())),
                );
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::Interrupted
                    && started.elapsed() < timeout =>
            {
                continue;
            }
            Err(error) => return Err(error),
        }
    }
}

fn activate_existing(socket_path: &std::path::Path) {
    match connect_with_retry(|| UnixStream::connect(socket_path), Duration::from_secs(1)) {
        Ok(mut stream) => {
            let toggle = toggle_requested(std::env::args_os().skip(1));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
            let verb = if toggle { TOGGLE } else { ACTIVATE };
            let request = activation_request(verb, take_startup_activation_token().as_deref());
            let _ = stream.write_all(&request);
        }
        Err(error) => eprintln!("could not reach the running Codex Minibar: {error}"),
    }
}

fn toggle_requested(arguments: impl IntoIterator<Item = std::ffi::OsString>) -> bool {
    arguments
        .into_iter()
        .any(|argument| argument == TOGGLE_FLAG)
}

fn popup_requested(arguments: impl IntoIterator<Item = std::ffi::OsString>) -> bool {
    arguments
        .into_iter()
        .any(|argument| argument == SHOW_FLAG || argument == TOGGLE_FLAG)
}

/// Give the launcher's focus grant to the first visible surface. Onboarding
/// takes priority over `--toggle`; ordinary autostart stays in the tray.
/// Call after the popup renderer is initialized.
pub fn show_initial_window(onboarding_needed: bool) {
    if onboarding_needed {
        set_activation_token(take_startup_activation_token());
        crate::settings_window::open_onboarding();
    } else if popup_requested(std::env::args_os().skip(1)) {
        show_with_token(take_startup_activation_token());
    } else {
        // An ordinary tray startup has no window to activate. Do not retain
        // its launch grant for an unrelated later click.
        let _ = take_startup_activation_token();
    }
}

fn serve(listener: UnixListener) {
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let request = match read_request(&mut stream, Duration::from_secs(2)) {
            Ok(request) => request,
            Err(error) => {
                crate::logger::info(format!("Ignoring incomplete activation request: {error}"));
                continue;
            }
        };
        if let Some(token) = parse_request(&request, ACTIVATE) {
            set_activation_token(token);
            activate();
        } else if let Some(token) = parse_request(&request, TOGGLE) {
            toggle_with_token(token);
        }
    }
}

// A per-read socket timeout alone lets a peer keep the serial listener busy
// indefinitely by sending a byte before each timeout. Bound the whole frame.
fn read_request(stream: &mut UnixStream, timeout: Duration) -> std::io::Result<Vec<u8>> {
    let started = Instant::now();
    let mut request = Vec::with_capacity(64);
    loop {
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "activation request timed out",
            ));
        }
        stream.set_read_timeout(Some(remaining))?;
        let mut buffer = [0; 512];
        match stream.read(&mut buffer) {
            Ok(0) => return Ok(request),
            Ok(count) => {
                request.extend_from_slice(&buffer[..count]);
                if request.len() > MAX_REQUEST as usize {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "activation request too large",
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
}

fn activation_request(verb: &str, token: Option<&str>) -> Vec<u8> {
    match token.filter(|token| is_token(token)) {
        // Retain the old wire form when possible, so a newly installed CLI
        // can still activate a primary process from the previous release.
        Some(token)
            if token.len() <= 256
                && token.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
                }) =>
        {
            format!("{verb} {token}\n").into_bytes()
        }
        Some(token) => format!(
            "{verb}-token {}\n",
            URL_SAFE_NO_PAD.encode(token.as_bytes())
        )
        .into_bytes(),
        None => format!("{verb}\n").into_bytes(),
    }
}

pub(super) fn is_token(token: &str) -> bool {
    // Wayland activation tokens are opaque UTF-8 strings, not identifiers.
    !token.is_empty() && token.len() <= MAX_TOKEN_BYTES && !token.contains('\0')
}

/// `Some(token)` for the requested verb, `None` for anything else.
fn parse_request(request: &[u8], verb: &str) -> Option<Option<String>> {
    if request.len() > MAX_REQUEST as usize {
        return None;
    }
    let line = std::str::from_utf8(request).ok()?.strip_suffix('\n')?;
    if let Some(encoded) = line.strip_prefix(&format!("{verb}-token ")) {
        let token = String::from_utf8(URL_SAFE_NO_PAD.decode(encoded).ok()?).ok()?;
        return is_token(&token).then_some(Some(token));
    }
    // Accept requests from an older installed CLI, whose framing only
    // supports a single-line token.
    if line.contains(['\n', '\r']) {
        return None;
    }
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
    toggle_command_for(&program, super::autostart::requires_extraction())
}

pub(super) fn toggle_command_for(program: &str, extract: bool) -> String {
    let program = quote_command_argument(program);
    if extract {
        format!(
            "{program} {} {TOGGLE_FLAG}",
            super::autostart::EXTRACT_AND_RUN_FLAG
        )
    } else {
        format!("{program} {TOGGLE_FLAG}")
    }
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

/// Shows the popup after a notification click using the desktop's focus grant.
/// Unlike a toggle, clicking a notification must keep an open popup visible.
pub(crate) fn show_with_token(token: Option<String>) {
    set_activation_token(token);
    crate::popup::show_near_cursor();
    super::wake::wake();
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
    fn activation_request_deadline_bounds_a_trickling_peer() {
        let (mut reader, mut writer) = UnixStream::pair().unwrap();
        let peer = std::thread::spawn(move || {
            for _ in 0..100 {
                if writer.write_all(b"a").is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });
        let started = Instant::now();
        let result = read_request(&mut reader, Duration::from_millis(100));
        drop(reader);
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
        peer.join().unwrap();
    }

    #[test]
    fn activation_request_reader_preserves_frames_and_rejects_oversize_input() {
        for (data, valid) in [
            (b"activate\n".to_vec(), true),
            (vec![b'a'; MAX_REQUEST as usize + 1], false),
        ] {
            let (mut reader, mut writer) = UnixStream::pair().unwrap();
            let peer = std::thread::spawn(move || {
                let _ = writer.write_all(&data);
            });
            let result = read_request(&mut reader, Duration::from_secs(2));
            if valid {
                assert_eq!(result.unwrap(), b"activate\n");
            } else {
                assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
            }
            drop(reader);
            peer.join().unwrap();
        }
    }

    #[test]
    fn toggle_request_detection_accepts_non_utf8_unrelated_arguments() {
        use std::os::unix::ffi::OsStringExt;
        let unrelated = std::ffi::OsString::from_vec(b"non-utf8-\xff".to_vec());
        assert!(!toggle_requested([unrelated.clone()]));
        assert!(toggle_requested([unrelated.clone(), TOGGLE_FLAG.into()]));
        assert!(popup_requested([unrelated, SHOW_FLAG.into()]));
        // Showing an existing window must not be interpreted as a toggle.
        assert!(!toggle_requested([SHOW_FLAG.into()]));
        assert!(!toggle_requested(["--toggle-other".into()]));
        assert!(!popup_requested(["--show-other".into()]));
    }

    #[test]
    fn activation_retries_until_the_primary_socket_is_ready() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("activate.sock");
        let mut listener = None;
        let mut attempts = 0;
        let stream = connect_with_retry(
            || {
                attempts += 1;
                if attempts == 2 {
                    listener = Some(UnixListener::bind(&path).unwrap());
                }
                UnixStream::connect(&path)
            },
            Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(attempts, 2);
        assert!(listener.unwrap().accept().is_ok());
        drop(stream);
    }

    #[test]
    fn activation_does_not_retry_permission_errors_or_exceed_its_deadline() {
        let mut attempts = 0;
        assert!(
            connect_with_retry(
                || {
                    attempts += 1;
                    Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
                },
                Duration::from_secs(1)
            )
            .is_err()
        );
        assert_eq!(attempts, 1);
        attempts = 0;
        assert!(
            connect_with_retry(
                || {
                    attempts += 1;
                    Err(std::io::Error::from(std::io::ErrorKind::NotFound))
                },
                Duration::ZERO
            )
            .is_err()
        );
        assert_eq!(attempts, 1);
    }

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
    fn extracted_appimage_shortcuts_keep_runtime_and_popup_flags_separate() {
        assert_eq!(
            toggle_command_for("/opt/My AppImage", true),
            "'/opt/My AppImage' --appimage-extract-and-run --toggle"
        );
        assert_eq!(
            toggle_command_for("/opt/My AppImage", false),
            "'/opt/My AppImage' --toggle"
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
        set_activation_token(Some("invalid\0token".into()));
        assert_eq!(take_activation_token(), None);
    }

    #[test]
    fn opaque_focus_tokens_round_trip_without_changing_socket_framing() {
        for verb in [ACTIVATE, TOGGLE] {
            for token in [
                "{uuid}/grant+value=",
                "token with spaces",
                "строка\nс\tтокеном",
            ] {
                let request = activation_request(verb, Some(token));
                assert_eq!(request.iter().filter(|byte| **byte == b'\n').count(), 1);
                assert_eq!(parse_request(&request, verb), Some(Some(token.to_owned())));
            }
            assert_eq!(
                parse_request(format!("{verb}-token invalid*\n").as_bytes(), verb),
                None
            );
            assert_eq!(
                parse_request(
                    activation_request(verb, Some(&"x".repeat(MAX_TOKEN_BYTES))).as_slice(),
                    verb
                ),
                Some(Some("x".repeat(MAX_TOKEN_BYTES)))
            );
        }
        assert!(!is_token("a\0b"));
        assert!(!is_token(&"x".repeat(MAX_TOKEN_BYTES + 1)));
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
        assert_eq!(
            parse_request(b"toggle a b\n", TOGGLE),
            Some(Some("a b".into()))
        );
        assert_eq!(parse_request(b"toggle abc", TOGGLE), None);
        assert_eq!(parse_request(b"togglex\n", TOGGLE), None);
        assert_eq!(parse_request(b"activate\n", TOGGLE), None);
    }
}
