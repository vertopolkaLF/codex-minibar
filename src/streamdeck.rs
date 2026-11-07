//! Local data bridge for the Codex Minibar Stream Deck companion.
//!
//! The Stream Deck plugin deliberately does not know how to authenticate with
//! any provider. It reads a short-lived local endpoint description and asks
//! Minibar for sanitized quota snapshots or popup actions.

use std::{
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    instances::ProviderId,
    popup_window::AppState,
    provider_registry,
    settings::Settings,
    widget_data::{self, ProviderSnapshot},
};

const PLUGIN_FILE_NAME: &str = "com.vertopolkalf.codex-minibar.streamDeckPlugin";
const PLUGIN_DOWNLOAD_URL: &str = "https://raw.githubusercontent.com/vertopolkaLF/codex-minibar/main/data/com.vertopolkalf.codex-minibar.streamDeckPlugin";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallPhase {
    Idle,
    Downloading,
    Launching,
    Launched,
    Failed(String),
}

/// Downloads the latest packaged plugin and hands it to the registered
/// Stream Deck installer without blocking the settings UI.
pub fn install_latest_plugin_async(on_phase: impl Fn(InstallPhase) + Send + 'static) {
    on_phase(InstallPhase::Downloading);
    thread::spawn(move || {
        let result = (|| -> anyhow::Result<()> {
            let plugin_path = download_latest_plugin()?;
            on_phase(InstallPhase::Launching);
            crate::updater::open_path(&plugin_path)
        })();

        match result {
            Ok(()) => on_phase(InstallPhase::Launched),
            Err(error) => {
                let message = error.to_string();
                crate::notifications::show_error("Stream Deck plugin", &message);
                on_phase(InstallPhase::Failed(message));
            }
        }
    });
}

fn download_latest_plugin() -> anyhow::Result<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let directory = std::env::temp_dir().join("Codex Minibar");
    fs::create_dir_all(&directory)?;
    let path = directory.join(format!(
        "codex-minibar-streamdeck-{}-{stamp}-{PLUGIN_FILE_NAME}",
        std::process::id()
    ));
    crate::updater::download_file(PLUGIN_DOWNLOAD_URL, &path)?;
    Ok(path)
}

pub const PROTOCOL_VERSION: u32 = 1;
pub(crate) const ENDPOINT_FILE_NAME: &str = "streamdeck-bridge.json";
const LOOPBACK_HOST: &str = "127.0.0.1";
/// Requests are one small JSON line; anything longer is hostile or broken.
const MAX_REQUEST_BYTES: u64 = 16 * 1024;
const MAX_CONCURRENT_CONNECTIONS: usize = 8;
const IO_TIMEOUT: Duration = Duration::from_secs(3);

/// Commands that must be executed by the existing tray/UI bridge thread.
#[derive(Clone, Debug)]
pub enum Command {
    #[cfg(not(windows))]
    ShowProvider {
        provider: ProviderId,
    },
    OpenPopup {
        provider: Option<ProviderId>,
    },
    RefreshData,
    #[cfg(not(windows))]
    RefreshAfterResume,
    /// Settings on the Command Palette page.
    OpenCommandPaletteSettings,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    Catalog {
        token: String,
    },
    Snapshot {
        token: String,
    },
    Refresh {
        token: String,
    },
    OpenPopup {
        token: String,
        provider: Option<String>,
    },
    /// Dock bands and provider summaries for the Command Palette extension.
    Palette {
        token: String,
    },
    OpenSettings {
        token: String,
    },
    /// The report `codex-minibar status` prints.
    Status {
        token: String,
    },
}

#[derive(Clone, Debug, Serialize)]
struct Endpoint {
    protocol: u32,
    host: String,
    port: u16,
    token: String,
    executable: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Response {
    Catalog {
        ok: bool,
        protocol: u32,
        providers: Vec<ProviderInfo>,
    },
    Snapshot {
        ok: bool,
        protocol: u32,
        observed_at: DateTime<Utc>,
        executable: String,
        providers: Vec<ProviderSnapshot>,
    },
    Palette {
        ok: bool,
        protocol: u32,
        observed_at: DateTime<Utc>,
        #[serde(flatten)]
        payload: crate::cmdpal::Payload,
    },
    Status {
        ok: bool,
        protocol: u32,
        report: crate::status::Report,
    },
    Accepted {
        ok: bool,
    },
    Error {
        ok: bool,
        error: String,
    },
}

/// One provider instance. `id` is the instance id (the primary instance
/// keeps the driver id) and `kind` the driver id shared by its instances.
#[derive(Debug, Serialize)]
struct ProviderInfo {
    id: String,
    kind: String,
    name: String,
    badge: Option<String>,
    enabled: bool,
    icon: String,
    metrics: Vec<MetricInfo>,
}

#[derive(Debug, Serialize)]
struct MetricInfo {
    id: String,
    label: String,
}

/// Starts the loopback server and returns the queue consumed by the UI-owned
/// background bridge. Starting it from that bridge keeps popup activation on
/// the same thread as tray activation and avoids a second application runtime.
pub fn start_server(state: Arc<AppState>) -> mpsc::Receiver<Command> {
    let (commands_tx, commands_rx) = mpsc::channel();
    #[cfg(not(windows))]
    crate::linux::dbus::start(Arc::clone(&state), commands_tx.clone());
    #[cfg(not(windows))]
    crate::linux::resume::start(commands_tx.clone());
    thread::spawn(move || {
        let endpoint_path = match Settings::default_path() {
            Ok(path) => path.with_file_name(ENDPOINT_FILE_NAME),
            Err(error) => {
                eprintln!("Stream Deck bridge could not resolve its endpoint path: {error:#}");
                return;
            }
        };
        if let Err(error) = run_server(state, commands_tx, endpoint_path) {
            eprintln!("Stream Deck bridge stopped: {error}");
        }
    });
    commands_rx
}

fn run_server(
    state: Arc<AppState>,
    commands_tx: mpsc::Sender<Command>,
    endpoint_path: PathBuf,
) -> io::Result<()> {
    let listener = TcpListener::bind((LOOPBACK_HOST, 0))?;
    let address = listener.local_addr()?;
    let token = generate_token()?;
    let executable = std::env::current_exe()
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    let endpoint = Endpoint {
        protocol: PROTOCOL_VERSION,
        host: LOOPBACK_HOST.into(),
        port: address.port(),
        token: token.clone(),
        executable,
    };

    let endpoint_json = serde_json::to_vec_pretty(&endpoint)
        .map_err(|error| io::Error::other(format!("serialize endpoint: {error}")))?;
    write_endpoint(&endpoint_path, &endpoint_json)?;

    let active = Arc::new(AtomicUsize::new(0));
    for incoming in listener.incoming() {
        match incoming {
            Ok(stream) => {
                if active.fetch_add(1, Ordering::AcqRel) >= MAX_CONCURRENT_CONNECTIONS {
                    active.fetch_sub(1, Ordering::AcqRel);
                    continue;
                }
                let state = Arc::clone(&state);
                let commands_tx = commands_tx.clone();
                let token = token.clone();
                let slot = ConnectionSlot(Arc::clone(&active));
                // A failed spawn drops the closure, and with it the slot.
                let _ = thread::Builder::new().spawn(move || {
                    let _slot = slot;
                    serve_connection(stream, state, commands_tx, &token);
                });
            }
            Err(error) => eprintln!("Stream Deck bridge connection failed: {error}"),
        }
    }
    Ok(())
}

/// The endpoint contains the loopback authorization token. Publish complete
/// JSON in one rename, with private Unix permissions even when replacing an
/// older endpoint created with the user's default umask.
fn write_endpoint(path: &std::path::Path, contents: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("endpoint path has no parent directory"))?;
    fs::create_dir_all(parent)?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(contents)?;
    staged.as_file().sync_all()?;
    publish_endpoint(staged, path)
}

#[cfg(not(windows))]
fn publish_endpoint(staged: tempfile::NamedTempFile, path: &std::path::Path) -> io::Result<()> {
    staged.persist(path).map_err(|error| error.error)?;
    Ok(())
}

/// ReplaceFile preserves existing readers' handles, unlike MoveFileEx (used by
/// tempfile::persist). Close our staging handle before Windows opens it again.
#[cfg(windows)]
fn publish_endpoint(staged: tempfile::NamedTempFile, path: &std::path::Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{Foundation::ERROR_FILE_NOT_FOUND, Storage::FileSystem::ReplaceFileW};
    let staged = staged.into_temp_path();
    let destination: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let source: Vec<u16> = staged.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: paths are NUL-terminated and live for the call; optional pointers
    // are null. The staging guard removes the temporary file on failure.
    if unsafe {
        ReplaceFileW(
            destination.as_ptr(),
            source.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    } != 0
    {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() != Some(ERROR_FILE_NOT_FOUND as i32) {
        return Err(error);
    }
    // First publication has no destination. Do not overwrite one created by a
    // concurrent writer after ReplaceFile reported a missing file.
    staged
        .persist_noclobber(path)
        .map_err(|error| error.error)?;
    Ok(())
}

/// Releases a connection-handler slot even if the handler panics.
struct ConnectionSlot(Arc<AtomicUsize>);

impl Drop for ConnectionSlot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// 128 bits from the OS CSPRNG, hex encoded.
#[cfg(not(windows))]
fn generate_token() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    crate::linux::random_bytes(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// 128 bits from the OS CSPRNG, hex encoded.
#[cfg(windows)]
fn generate_token() -> io::Result<String> {
    use windows_sys::Win32::Security::Cryptography::{
        BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom,
    };
    let mut bytes = [0u8; 16];
    // SAFETY: the buffer is valid for `bytes.len()` writable bytes and a null
    // algorithm handle is allowed with the system-preferred RNG flag.
    let status = unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            bytes.as_mut_ptr(),
            bytes.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status != 0 {
        return Err(io::Error::other(format!(
            "BCryptGenRandom failed: {status:#x}"
        )));
    }
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn serve_connection(
    mut stream: TcpStream,
    state: Arc<AppState>,
    commands_tx: mpsc::Sender<Command>,
    expected_token: &str,
) {
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    let mut line = String::new();
    let read_result = match stream.try_clone() {
        Ok(reader) => BufReader::new(reader.take(MAX_REQUEST_BYTES)).read_line(&mut line),
        Err(error) => Err(error),
    };
    if read_result.is_ok() && !line.ends_with('\n') && line.len() as u64 >= MAX_REQUEST_BYTES {
        let _ = write_response(
            &mut stream,
            &Response::Error {
                ok: false,
                error: "request too large".into(),
            },
        );
        return;
    }
    let response = match read_result {
        Ok(0) => Response::Error {
            ok: false,
            error: "empty request".into(),
        },
        Ok(_) => match serde_json::from_str::<Request>(line.trim()) {
            Ok(request) => handle_request(request, &state, &commands_tx, expected_token),
            Err(error) => Response::Error {
                ok: false,
                error: format!("invalid request: {error}"),
            },
        },
        Err(error) => Response::Error {
            ok: false,
            error: format!("read request: {error}"),
        },
    };
    let _ = write_response(&mut stream, &response);
}

fn handle_request(
    request: Request,
    state: &AppState,
    commands_tx: &mpsc::Sender<Command>,
    expected_token: &str,
) -> Response {
    let token = match &request {
        Request::Catalog { token }
        | Request::Snapshot { token }
        | Request::Refresh { token }
        | Request::OpenPopup { token, .. }
        | Request::Palette { token }
        | Request::OpenSettings { token }
        | Request::Status { token } => token,
    };
    if token != expected_token {
        return Response::Error {
            ok: false,
            error: "unauthorized bridge request".into(),
        };
    }

    match request {
        Request::Catalog { .. } => Response::Catalog {
            ok: true,
            protocol: PROTOCOL_VERSION,
            providers: build_catalog(state),
        },
        Request::Snapshot { .. } => build_snapshot(state),
        Request::Status { .. } => Response::Status {
            ok: true,
            protocol: PROTOCOL_VERSION,
            report: status_report(state),
        },
        Request::Palette { .. } => {
            let limits = state
                .limits
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            Response::Palette {
                ok: true,
                protocol: PROTOCOL_VERSION,
                observed_at: Utc::now(),
                payload: crate::cmdpal::payload(&limits),
            }
        }
        Request::OpenSettings { .. } => match commands_tx.send(Command::OpenCommandPaletteSettings)
        {
            Ok(()) => Response::Accepted { ok: true },
            Err(error) => Response::Error {
                ok: false,
                error: format!("queue settings action: {error}"),
            },
        },
        Request::Refresh { .. } => match commands_tx.send(Command::RefreshData) {
            Ok(()) => Response::Accepted { ok: true },
            Err(error) => Response::Error {
                ok: false,
                error: format!("queue data refresh: {error}"),
            },
        },
        Request::OpenPopup { provider, .. } => {
            let provider = match provider {
                Some(id) => match ProviderId::lookup(&id) {
                    Some(provider) => Some(provider),
                    None => {
                        return Response::Error {
                            ok: false,
                            error: format!("unknown provider: {id}"),
                        };
                    }
                },
                None => None,
            };
            match commands_tx.send(Command::OpenPopup { provider }) {
                Ok(()) => Response::Accepted { ok: true },
                Err(error) => Response::Error {
                    ok: false,
                    error: format!("queue popup action: {error}"),
                },
            }
        }
    }
}

fn build_catalog(state: &AppState) -> Vec<ProviderInfo> {
    let limits = state
        .limits
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let enabled = crate::instances::published_enabled_providers();
    crate::instances::published_providers()
        .into_iter()
        .map(|provider| {
            let kind = provider.kind();
            let descriptor = provider_registry::descriptor(kind);
            let live_limits = limits.get(provider);
            let mut metrics = descriptor
                .metrics
                .iter()
                .map(|metric| MetricInfo {
                    id: metric.id.into(),
                    label: metric.label.into(),
                })
                .collect::<Vec<_>>();
            for additional in &live_limits.additional_limits {
                let metric_id = provider_registry::additional_limit_brick_id(kind, &additional.id);
                if metrics.iter().all(|metric| metric.id != metric_id) {
                    metrics.push(MetricInfo {
                        id: metric_id,
                        label: additional.title.clone(),
                    });
                }
            }
            ProviderInfo {
                id: provider.id().into(),
                kind: descriptor.id.into(),
                name: provider.qualified_name(),
                badge: provider.badge().map(|badge| badge.text),
                enabled: enabled.contains(&provider),
                icon: provider_registry::icon(kind).into(),
                metrics,
            }
        })
        .collect()
}

/// The report `codex-minibar status` and the Linux D-Bus service publish.
pub(crate) fn status_report(state: &AppState) -> crate::status::Report {
    // Built from a copy: the D-Bus service polls this, and building can
    // rasterize provider marks, so workers must not wait on the lock.
    let limits = state
        .limits
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    crate::status::report(&limits)
}

fn build_snapshot(state: &AppState) -> Response {
    let limits = state
        .limits
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    Response::Snapshot {
        ok: true,
        protocol: PROTOCOL_VERSION,
        observed_at: Utc::now(),
        executable: std::env::current_exe()
            .ok()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default(),
        providers: crate::instances::published_providers()
            .into_iter()
            .map(|provider| widget_data::snapshot(provider, &limits))
            .collect(),
    }
}

fn write_response(stream: &mut TcpStream, response: &Response) -> io::Result<()> {
    serde_json::to_writer(&mut *stream, response)
        .map_err(|error| io::Error::other(format!("serialize response: {error}")))?;
    stream.write_all(b"\n")?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_publication_creates_then_replaces_a_complete_document() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nested").join(ENDPOINT_FILE_NAME);
        write_endpoint(&path, b"first-fixture").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"first-fixture");
        write_endpoint(&path, b"second-fixture").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second-fixture");
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn endpoint_publication_replaces_the_file_instead_of_truncating_a_readers_copy() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(ENDPOINT_FILE_NAME);
        fs::write(&path, br#"{"token":"old-fixture"}"#).unwrap();
        let mut reader = fs::File::open(&path).unwrap();
        let new = br#"{"token":"new-fixture"}"#;
        write_endpoint(&path, new).unwrap();
        assert_eq!(fs::read(&path).unwrap(), new);
        let mut previous = String::new();
        reader.read_to_string(&mut previous).unwrap();
        assert_eq!(previous, r#"{"token":"old-fixture"}"#);
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn endpoint_publication_replaces_a_symlink_without_writing_its_target() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().unwrap();
        let outside = directory.path().join("unrelated.json");
        let endpoint = directory.path().join(ENDPOINT_FILE_NAME);
        fs::write(&outside, b"unchanged").unwrap();
        symlink(&outside, &endpoint).unwrap();
        write_endpoint(&endpoint, br#"{"token":"fixture"}"#).unwrap();
        assert_eq!(fs::read(&outside).unwrap(), b"unchanged");
        assert!(
            !fs::symlink_metadata(&endpoint)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
}
