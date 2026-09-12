//! Local data bridge for the Codex Minibar Stream Deck companion.
//!
//! The Stream Deck plugin deliberately does not know how to authenticate with
//! any provider. It reads a short-lived local endpoint description and asks
//! Minibar for sanitized quota snapshots or popup actions.

use std::{
    fs,
    io::{self, BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{Arc, mpsc},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    limits::{LimitWindow, ProviderLimits},
    popup_window::AppState,
    provider_registry,
    settings::{ProviderKind, Settings},
};

const PLUGIN_ASSET_SUFFIX: &str = ".streamDeckPlugin";

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
                crate::notifications::show("Stream Deck plugin", &message);
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
        "codex-minibar-streamdeck-{}-{stamp}{PLUGIN_ASSET_SUFFIX}",
        std::process::id()
    ));
    crate::updater::download_latest_release_asset(PLUGIN_ASSET_SUFFIX, &path)?;
    Ok(path)
}

pub const PROTOCOL_VERSION: u32 = 1;
const ENDPOINT_FILE_NAME: &str = "streamdeck-bridge.json";
const LOOPBACK_HOST: &str = "127.0.0.1";

/// Commands that must be executed by the existing tray/UI bridge thread.
#[derive(Clone, Debug)]
pub enum Command {
    OpenPopup { provider: Option<ProviderKind> },
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
    OpenPopup {
        token: String,
        provider: Option<String>,
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
    Accepted {
        ok: bool,
    },
    Error {
        ok: bool,
        error: String,
    },
}

#[derive(Debug, Serialize)]
struct ProviderInfo {
    id: String,
    name: String,
    icon: String,
    metrics: Vec<MetricInfo>,
}

#[derive(Debug, Serialize)]
struct MetricInfo {
    id: String,
    label: String,
}

#[derive(Debug, Serialize)]
struct ProviderSnapshot {
    id: String,
    name: String,
    icon: String,
    brand_rgb: [u8; 3],
    account_name: Option<String>,
    plan_type: Option<String>,
    sampled_at: DateTime<Utc>,
    primary: WindowSnapshot,
    secondary: WindowSnapshot,
    additional: Vec<AdditionalSnapshot>,
}

#[derive(Debug, Serialize)]
struct WindowSnapshot {
    used_percent: Option<u8>,
    remaining_percent: Option<u8>,
    resets_at: Option<DateTime<Utc>>,
    duration_minutes: Option<u32>,
}

#[derive(Debug, Serialize)]
struct AdditionalSnapshot {
    id: String,
    metric_id: String,
    label: String,
    window: WindowSnapshot,
}

/// Starts the loopback server and returns the queue consumed by the UI-owned
/// background bridge. Starting it from that bridge keeps popup activation on
/// the same thread as tray activation and avoids a second application runtime.
pub fn start_server(state: Arc<AppState>) -> mpsc::Receiver<Command> {
    let (commands_tx, commands_rx) = mpsc::channel();
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
    let token = format!(
        "{:x}-{:x}",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    );
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

    if let Some(parent) = endpoint_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let endpoint_json = serde_json::to_vec_pretty(&endpoint)
        .map_err(|error| io::Error::other(format!("serialize endpoint: {error}")))?;
    fs::write(&endpoint_path, endpoint_json)?;

    for incoming in listener.incoming() {
        match incoming {
            Ok(stream) => {
                let state = Arc::clone(&state);
                let commands_tx = commands_tx.clone();
                let token = token.clone();
                thread::spawn(move || serve_connection(stream, state, commands_tx, &token));
            }
            Err(error) => eprintln!("Stream Deck bridge connection failed: {error}"),
        }
    }
    Ok(())
}

fn serve_connection(
    mut stream: TcpStream,
    state: Arc<AppState>,
    commands_tx: mpsc::Sender<Command>,
    expected_token: &str,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let mut line = String::new();
    let read_result = BufReader::new(&mut stream).read_line(&mut line);
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
        | Request::OpenPopup { token, .. } => token,
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
        Request::OpenPopup { provider, .. } => {
            let provider = match provider {
                Some(id) => match ProviderKind::from_id(&id) {
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
    ProviderKind::ALL
        .into_iter()
        .map(|provider| {
            let descriptor = provider_registry::descriptor(provider);
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
                let metric_id = provider_registry::additional_limit_brick_id(provider, &additional.id);
                if metrics.iter().all(|metric| metric.id != metric_id) {
                    metrics.push(MetricInfo {
                        id: metric_id,
                        label: additional.title.clone(),
                    });
                }
            }
            ProviderInfo {
                id: descriptor.id.into(),
                name: descriptor.display_name.into(),
                icon: descriptor.icon.into(),
                metrics,
            }
        })
        .collect()
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
        providers: ProviderKind::ALL
            .into_iter()
            .map(|provider| provider_snapshot(provider, &limits))
            .collect(),
    }
}

fn provider_snapshot(provider: ProviderKind, limits: &ProviderLimits) -> ProviderSnapshot {
    let descriptor = provider_registry::descriptor(provider);
    let snapshot = limits.get(provider);
    ProviderSnapshot {
        id: descriptor.id.into(),
        name: descriptor.display_name.into(),
        icon: descriptor.icon.into(),
        brand_rgb: [
            descriptor.brand_rgb.0,
            descriptor.brand_rgb.1,
            descriptor.brand_rgb.2,
        ],
        account_name: snapshot.account_name.clone(),
        plan_type: snapshot.plan_type.clone(),
        sampled_at: snapshot.sampled_at,
        primary: window_snapshot(&snapshot.primary),
        secondary: window_snapshot(&snapshot.secondary),
        additional: snapshot
            .additional_limits
            .iter()
            .map(|additional| AdditionalSnapshot {
                id: additional.id.clone(),
                metric_id: provider_registry::additional_limit_brick_id(provider, &additional.id),
                label: additional.title.clone(),
                window: window_snapshot(&additional.window),
            })
            .collect(),
    }
}

fn window_snapshot(window: &LimitWindow) -> WindowSnapshot {
    WindowSnapshot {
        used_percent: window.used_percent,
        remaining_percent: window.remaining_percent(),
        resets_at: window.resets_at,
        duration_minutes: window.duration_minutes,
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
    fn window_snapshot_exposes_remaining_without_provider_credentials() {
        let window = LimitWindow {
            used_percent: Some(25),
            resets_at: None,
            duration_minutes: Some(300),
        };
        let snapshot = window_snapshot(&window);
        assert_eq!(snapshot.used_percent, Some(25));
        assert_eq!(snapshot.remaining_percent, Some(75));
    }

    #[test]
    fn catalog_uses_provider_metric_ids() {
        let provider = provider_registry::descriptor(ProviderKind::Codex);
        assert_eq!(
            provider.metrics[0].source,
            provider_registry::MetricSource::Primary
        );
        assert_eq!(provider.metrics[0].id, "codex.session");
    }
}
