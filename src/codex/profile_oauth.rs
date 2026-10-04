//! App-owned Codex sessions. Never writes the ambient CLI/Desktop login.
pub(crate) use crate::claude::profile_oauth::LoginControl;
use anyhow::{Context, Result, bail, ensure};
use base64::Engine;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{LazyLock, Mutex, MutexGuard},
    thread,
    time::{Duration, Instant},
};
const REFRESH_URL: &str = "https://auth.openai.com/oauth/token";
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const SIGN_IN_HINT: &str = "Use Sign in in this account's Codex settings.";
static OPERATIONS: Mutex<()> = Mutex::new(());
static REFRESH_STATE: LazyLock<Mutex<HashMap<String, RefreshState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
pub(crate) fn credential_guard() -> Result<MutexGuard<'static, ()>> {
    OPERATIONS
        .lock()
        .map_err(|_| anyhow::anyhow!("Codex credential operation failed."))
}

// Serialized only in the DPAPI secret slot. Never Debug or log token material.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Session {
    access_token: String,
    refresh_token: String,
    id_token: String,
    account_id: String,
    last_refresh: Option<DateTime<Utc>>,
}
impl Session {
    fn validate(&self) -> Result<()> {
        ensure!(
            !self.access_token.trim().is_empty()
                && !self.refresh_token.trim().is_empty()
                && !self.account_id.trim().is_empty(),
            "Codex did not save a refreshable ChatGPT login."
        );
        Ok(())
    }
    pub(crate) fn encode(&self) -> Result<String> {
        self.validate()?;
        serde_json::to_string(self).context("encode saved Codex login")
    }
    pub(super) fn credentials(&self) -> super::OAuthCredentials {
        super::OAuthCredentials {
            access_token: self.access_token.clone(),
            account_id: Some(self.account_id.clone()),
        }
    }
    fn fingerprint(&self) -> [u8; 32] {
        Sha256::digest(self.refresh_token.as_bytes()).into()
    }
    fn needs_refresh(&self) -> bool {
        if let Some(exp) = token_claims(&self.access_token).and_then(|v| v["exp"].as_i64()) {
            return exp <= (Utc::now() + chrono::Duration::minutes(5)).timestamp();
        }
        self.last_refresh
            .is_none_or(|at| Utc::now() - at > chrono::Duration::days(8))
    }
}
fn token_claims(token: &str) -> Option<serde_json::Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}
#[derive(Deserialize)]
struct RefreshResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    id_token: Option<String>,
}
impl RefreshResponse {
    fn apply(self, previous: &Session) -> Result<Session> {
        let mut session = previous.clone();
        session.access_token = self
            .access_token
            .filter(|v| !v.trim().is_empty())
            .context("Codex refresh returned no access token")?;
        if let Some(token) = self.refresh_token.filter(|v| !v.trim().is_empty()) {
            session.refresh_token = token;
        }
        if let Some(token) = self.id_token.filter(|v| !v.trim().is_empty()) {
            session.id_token = token;
        }
        session.last_refresh = Some(Utc::now());
        session.validate()?;
        Ok(session)
    }
}

struct RefreshState {
    fingerprint: [u8; 32],
    pending: Option<Session>,
    retry_at: Option<Instant>,
    terminal: bool,
}

pub(crate) fn forget(profile_id: &str) {
    if let Ok(mut state) = REFRESH_STATE.lock() {
        state.remove(profile_id);
    }
}

/// Caller holds credential_guard through loading/refreshing/persisting a slot.
pub(crate) fn resolve(
    agent: &ureq::Agent,
    profile_id: &str,
    raw: &str,
    force: bool,
) -> Result<Session> {
    resolve_using(agent, profile_id, raw, force, REFRESH_URL, |session| {
        super::save_profile_credential(profile_id, Some(&session.encode()?))
    })
}

fn resolve_using(
    agent: &ureq::Agent,
    profile_id: &str,
    raw: &str,
    force: bool,
    refresh_url: &str,
    mut persist: impl FnMut(&Session) -> Result<()>,
) -> Result<Session> {
    let mut session: Session = serde_json::from_str(raw)
        .map_err(|_| anyhow::anyhow!("Saved Codex login is unreadable. {SIGN_IN_HINT}"))?;
    session.validate()?;
    let mut states = REFRESH_STATE
        .lock()
        .map_err(|_| anyhow::anyhow!("Codex refresh state failed."))?;
    let fingerprint = session.fingerprint();
    if states
        .get(profile_id)
        .is_some_and(|state| state.fingerprint != fingerprint)
    {
        states.remove(profile_id);
    }
    if let Some(state) = states.get(profile_id) {
        if let Some(pending) = &state.pending {
            session = pending.clone();
            persist(&session)?;
            states.remove(profile_id);
            return Ok(session);
        } else if state.terminal {
            bail!("Codex subscription login was revoked. {SIGN_IN_HINT}");
        } else if state
            .retry_at
            .is_some_and(|deadline| deadline > Instant::now())
            && (force || session.needs_refresh())
        {
            bail!("Codex token refresh is cooling down. Try again in a few minutes.");
        }
    }
    if !force && !session.needs_refresh() {
        return Ok(session);
    }
    let body = serde_json::json!({"grant_type":"refresh_token", "refresh_token":session.refresh_token, "client_id":CLIENT_ID, "scope":"openid profile email"});
    let response = agent
        .post(refresh_url)
        .set("Content-Type", "application/json")
        .set("Accept", "application/json")
        .timeout(Duration::from_secs(15))
        .send_string(&body.to_string());
    let refreshed = match response {
        Ok(response) => response
            .into_string()
            .ok()
            .and_then(|body| serde_json::from_str::<RefreshResponse>(&body).ok())
            .and_then(|reply: RefreshResponse| reply.apply(&session).ok()),
        Err(ureq::Error::Status(status, response)) => {
            let terminal = matches!(status, 400 | 401)
                && response
                    .into_string()
                    .ok()
                    .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
                    .and_then(|body| {
                        body["error"]
                            .as_str()
                            .or(body["error"]["code"].as_str())
                            .map(str::to_owned)
                    })
                    .is_some_and(|error| {
                        matches!(
                            error.as_str(),
                            "invalid_grant"
                                | "refresh_token_reused"
                                | "refresh_token_invalidated"
                                | "refresh_token_expired"
                        )
                    });
            states.insert(
                profile_id.into(),
                RefreshState {
                    fingerprint,
                    pending: None,
                    retry_at: Some(Instant::now() + Duration::from_secs(300)),
                    terminal,
                },
            );
            if terminal {
                bail!("Codex subscription login was revoked. {SIGN_IN_HINT}");
            }
            bail!(
                "Codex token refresh failed (HTTP {status}). Credentials were kept; retrying later."
            );
        }
        Err(_) => None,
    };
    let Some(refreshed) = refreshed else {
        states.insert(
            profile_id.into(),
            RefreshState {
                fingerprint,
                pending: None,
                retry_at: Some(Instant::now() + Duration::from_secs(300)),
                terminal: false,
            },
        );
        bail!("Codex token refresh failed. Credentials were kept; retrying later.");
    };
    // Keep rotated credentials in memory before attempting disk persistence.
    // A failed save must retry this write, not reuse the consumed refresh grant.
    states.insert(
        profile_id.into(),
        RefreshState {
            fingerprint,
            pending: Some(refreshed.clone()),
            retry_at: None,
            terminal: false,
        },
    );
    persist(&refreshed)?;
    states.remove(profile_id);
    Ok(refreshed)
}

const AUTH_OVERRIDES: &[&str] = &[
    "OPENAI_API_KEY",
    "OPENAI_BASE_URL",
    "OPENAI_ORGANIZATION",
    "OPENAI_ORG_ID",
    "OPENAI_PROJECT_ID",
    "CODEX_API_KEY",
    "CODEX_AUTH_TOKEN",
    "CODEX_INTERNAL_ORIGINATOR_OVERRIDE",
];
fn login_command(executable: &Path, directory: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .args(["-c", "cli_auth_credentials_store=\"file\"", "login"])
        .env("CODEX_HOME", directory)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for key in AUTH_OVERRIDES {
        command.env_remove(key);
    }
    command
}
fn native_login_executable(explicit: Option<&Path>) -> Option<PathBuf> {
    crate::discovery::discover(explicit)
        .into_iter()
        .find_map(|candidate| native_for_candidate(&candidate.path))
}

fn native_for_candidate(candidate: &Path) -> Option<PathBuf> {
    if candidate
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    {
        return Some(candidate.to_owned());
    }
    // npm's shim delegates to either an optional platform package or its own
    // vendor directory. Both current vendor/<target>/bin and legacy /codex
    // layouts are supported. No shell launcher is needed for browser login.
    let root = candidate.parent()?;
    let targets = if cfg!(target_arch = "aarch64") {
        ["aarch64-pc-windows-msvc", "x86_64-pc-windows-msvc"]
    } else {
        ["x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"]
    };
    for target in targets {
        let platform_package = if target.starts_with("aarch64") {
            "codex-win32-arm64"
        } else {
            "codex-win32-x64"
        };
        let main = root.join("node_modules/@openai/codex");
        let roots = [
            main.join("node_modules/@openai").join(platform_package),
            root.join("node_modules/@openai").join(platform_package),
            main,
        ];
        for package in roots {
            for directory in ["bin", "codex"] {
                let native = package
                    .join("vendor")
                    .join(target)
                    .join(directory)
                    .join("codex.exe");
                if native.is_file() {
                    return Some(native);
                }
            }
        }
    }
    None
}

fn read_login(directory: &Path) -> Result<Session> {
    let bytes = std::fs::read(directory.join("auth.json")).context(
        "Codex did not save a login. Install native Codex CLI or Codex desktop and try again.",
    )?;
    let file: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("Codex saved an unreadable login."))?;
    let tokens = &file["tokens"];
    let get = |key: &str| tokens[key].as_str().unwrap_or_default().to_owned();
    let id_token = get("id_token");
    let mut account_id = get("account_id");
    if account_id.trim().is_empty() {
        account_id = token_claims(&id_token)
            .and_then(|v| {
                v["https://api.openai.com/auth"]["chatgpt_account_id"]
                    .as_str()
                    .map(str::to_owned)
            })
            .unwrap_or_default();
    }
    let session = Session {
        access_token: get("access_token"),
        refresh_token: get("refresh_token"),
        id_token,
        account_id,
        last_refresh: file["last_refresh"].as_str().and_then(|v| v.parse().ok()),
    };
    session.validate()?;
    Ok(session)
}

fn login_root() -> Result<std::path::PathBuf> {
    Ok(crate::settings::Settings::default_path()?
        .parent()
        .context("settings directory is unavailable")?
        .join("codex-logins"))
}

/// Primary instance only, before any login starts. Never follow reparse points.
pub(crate) fn cleanup_abandoned_logins() -> Result<()> {
    cleanup_root(&login_root()?)
}

fn is_link(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    metadata.file_type().is_symlink()
}

fn cleanup_root(root: &Path) -> Result<()> {
    let metadata = match std::fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).context("read Codex sign-in directory"),
    };
    ensure!(
        metadata.is_dir() && !is_link(&metadata),
        "Codex sign-in directory must not be a link."
    );
    let root = root.canonicalize()?;
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        if !entry.file_name().to_string_lossy().starts_with("login-") {
            continue;
        }
        let metadata = std::fs::symlink_metadata(entry.path())?;
        if !metadata.is_dir() || is_link(&metadata) {
            continue;
        }
        let path = entry.path().canonicalize()?;
        ensure!(
            path.parent() == Some(root.as_path()),
            "Codex sign-in directory escaped its root."
        );
        std::fs::remove_dir_all(path)?;
    }
    Ok(())
}

pub(crate) fn login(explicit: Option<&Path>, control: &LoginControl) -> Result<String> {
    ensure!(!control.cancelled(), "Codex sign-in cancelled.");
    let executable = native_login_executable(explicit)
        .context("Install native Windows Codex CLI or Codex desktop to sign in from Minibar.")?;
    ensure!(
        executable
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe")),
        "Sign-in requires native Windows Codex CLI or Codex desktop. Install Codex desktop or the native CLI and try again."
    );
    let root = login_root()?;
    std::fs::create_dir_all(&root).context("create Codex sign-in root")?;
    ensure!(
        !is_link(&std::fs::symlink_metadata(&root)?),
        "Codex sign-in directory must not be a link."
    );
    let directory = tempfile::Builder::new()
        .prefix("login-")
        .tempdir_in(root)
        .context("create isolated Codex sign-in directory")?;
    let command = login_command(&executable, directory.path());
    #[cfg(windows)]
    let mut child = crate::claude::profile_oauth::login_child::LoginChild::spawn(&command)?;
    #[cfg(not(windows))]
    let mut child = {
        let mut command = command;
        command.spawn()?
    };
    let started = Instant::now();
    loop {
        if control.cancelled() || started.elapsed() >= Duration::from_secs(300) {
            let _ = child.kill();
            let _ = child.wait();
            ensure!(!control.cancelled(), "Codex sign-in cancelled.");
            bail!(
                "Codex sign-in timed out. Finish the browser login within five minutes. Try again after closing an unfinished sign-in."
            );
        }
        let status = match child.try_wait() {
            Ok(status) => status,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                bail!("Could not wait for Codex sign-in.");
            }
        };
        if let Some(status) = status {
            ensure!(
                status.success(),
                "Codex sign-in did not finish (exit {}). Try signing in again.",
                status.code().unwrap_or(-1)
            );
            return read_login(directory.path())?.encode();
        }
        thread::sleep(Duration::from_millis(150));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    fn endpoint(status: &str, body: &str) -> (String, std::thread::JoinHandle<serde_json::Value>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/token", listener.local_addr().unwrap());
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let worker = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut data = Vec::new();
            let body_start;
            loop {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                data.push(byte[0]);
                assert!(data.len() < 16384);
                if data.ends_with(b"\r\n\r\n") {
                    body_start = data.len();
                    break;
                }
            }
            let headers = String::from_utf8(data).unwrap();
            let length: usize = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|length| length.trim().parse().unwrap())
                })
                .unwrap();
            let mut payload = vec![0; length];
            socket.read_exact(&mut payload).unwrap();
            assert!(headers.starts_with("POST /token "));
            assert!(body_start > 0);
            socket.write_all(response.as_bytes()).unwrap();
            serde_json::from_slice(&payload).unwrap()
        });
        (url, worker)
    }

    fn session() -> Session {
        Session {
            access_token: "fixture-access".into(),
            refresh_token: "fixture-refresh".into(),
            id_token: "fixture-id".into(),
            account_id: "workspace-fixture".into(),
            last_refresh: Some(Utc::now()),
        }
    }
    fn agent() -> ureq::Agent {
        ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(3))
            .build()
    }

    #[test]
    fn login_is_isolated_and_removes_auth_overrides() {
        let command = login_command(Path::new("codex.exe"), Path::new("isolated"));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["-c", "cli_auth_credentials_store=\"file\"", "login"]
        );
        assert_eq!(command.get_current_dir(), Some(Path::new("isolated")));
        assert!(
            command.get_envs().any(|(key, value)| key == "CODEX_HOME"
                && value == Some(std::ffi::OsStr::new("isolated")))
        );
        for key in AUTH_OVERRIDES {
            assert!(
                command
                    .get_envs()
                    .any(|(name, value)| name == *key && value.is_none())
            );
        }
    }
    #[test]
    fn resolves_native_npm_binaries_in_current_legacy_and_nested_layouts() {
        let root = tempfile::tempdir().unwrap();
        let candidate = root.path().join("codex.cmd");
        for relative in [
            "node_modules/@openai/codex/vendor/x86_64-pc-windows-msvc/codex/codex.exe",
            "node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe",
            "node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe",
        ] {
            let path = root.path().join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"fixture").unwrap();
            assert_eq!(native_for_candidate(&candidate), Some(path.clone()));
            std::fs::remove_file(path).unwrap();
        }
        assert_eq!(native_for_candidate(&candidate), None);
    }

    #[test]
    fn reads_full_login_and_rejects_api_key_only_auth() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("auth.json");
        std::fs::write(&path, r#"{"tokens":{"access_token":"fixture-access","refresh_token":"fixture-refresh","id_token":"fixture-id","account_id":"workspace-fixture"}}"#).unwrap();
        let read = read_login(directory.path()).unwrap();
        assert_eq!(read.refresh_token, "fixture-refresh");
        assert_eq!(
            read.credentials().account_id.as_deref(),
            Some("workspace-fixture")
        );
        std::fs::write(&path, r#"{"OPENAI_API_KEY":"fixture-api-key"}"#).unwrap();
        assert!(read_login(directory.path()).is_err());
    }
    #[test]
    fn rotated_session_is_retried_after_save_failure_without_reusing_grant() {
        let _guard = credential_guard().unwrap();
        let mut current = session();
        current.last_refresh = None;
        let raw = current.encode().unwrap();
        let id = "codex-save-failure";
        forget(id);
        let (url, worker) = endpoint(
            "200 OK",
            r#"{"access_token":"new-access","refresh_token":"new-refresh"}"#,
        );
        assert!(
            resolve_using(&agent(), id, &raw, false, &url, |_| bail!(
                "fixture write failure"
            ))
            .is_err()
        );
        let grant = worker.join().unwrap();
        assert_eq!(grant["refresh_token"], "fixture-refresh");
        assert_eq!(grant["client_id"], CLIENT_ID);
        let mut saved = None;
        let recovered = resolve_using(&agent(), id, &raw, true, &url, |session| {
            saved = Some(session.clone());
            Ok(())
        })
        .unwrap();
        assert_eq!(recovered.access_token, "new-access");
        assert_eq!(saved.unwrap().refresh_token, "new-refresh");
        forget(id);
    }
    #[test]
    fn terminal_refresh_failure_does_not_retry_until_credentials_change() {
        let _guard = credential_guard().unwrap();
        let id = "codex-revoked";
        forget(id);
        let current = session();
        let (url, worker) = endpoint(
            "400 Bad Request",
            r#"{"error":{"code":"refresh_token_reused"}}"#,
        );
        assert!(
            resolve_using(
                &agent(),
                id,
                &current.encode().unwrap(),
                true,
                &url,
                |_| Ok(())
            )
            .err()
            .unwrap()
            .to_string()
            .contains("revoked")
        );
        worker.join().unwrap();
        assert!(
            resolve_using(
                &agent(),
                id,
                &current.encode().unwrap(),
                true,
                &url,
                |_| panic!("unexpected save")
            )
            .is_err()
        );
        let mut replacement = current;
        replacement.refresh_token = "replacement-refresh".into();
        assert!(
            resolve_using(
                &agent(),
                id,
                &replacement.encode().unwrap(),
                false,
                &url,
                |_| panic!("unexpected save")
            )
            .is_ok()
        );
        forget(id);
    }
    #[test]
    fn transient_refresh_failure_keeps_credentials_and_cools_down() {
        let _guard = credential_guard().unwrap();
        let id = "codex-transient";
        forget(id);
        let current = session();
        let (url, worker) = endpoint("503 Service Unavailable", "{}");
        assert!(
            resolve_using(
                &agent(),
                id,
                &current.encode().unwrap(),
                true,
                &url,
                |_| panic!("unexpected save")
            )
            .is_err()
        );
        worker.join().unwrap();
        let error = resolve_using(&agent(), id, &current.encode().unwrap(), true, &url, |_| {
            panic!("unexpected save")
        })
        .err()
        .unwrap();
        assert!(error.to_string().contains("cooling down"));
        forget(id);
    }
    #[test]
    fn refresh_retains_workspace_and_missing_optional_tokens() {
        let previous = session();
        let next = RefreshResponse {
            access_token: Some("new-access".into()),
            refresh_token: None,
            id_token: None,
        }
        .apply(&previous)
        .unwrap();
        assert_eq!(next.account_id, previous.account_id);
        assert_eq!(next.refresh_token, previous.refresh_token);
        assert_eq!(next.id_token, previous.id_token);
    }
    #[test]
    fn fresh_session_never_requests_refresh() {
        let current = session();
        let result = resolve_using(
            &agent(),
            "codex-fresh",
            &current.encode().unwrap(),
            false,
            "http://127.0.0.1:1",
            |_| panic!("unexpected save"),
        )
        .unwrap();
        assert_eq!(result.access_token, current.access_token);
    }
    #[test]
    fn jwt_expiry_takes_priority_over_refresh_age() {
        let mut current = session();
        current.access_token = format!(
            "fixture.{}.fixture",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"exp":1}"#)
        );
        assert!(current.needs_refresh());
    }
}
