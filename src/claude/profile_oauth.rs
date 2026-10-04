//! App-owned subscription logins. The ambient CLI/Desktop login is never written.
use std::{
    collections::HashMap,
    path::Path,
    process::{Command, Stdio},
    sync::{
        Arc, LazyLock, Mutex, MutexGuard,
        atomic::{AtomicU8, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[cfg(windows)]
mod login_child;

const REFRESH_URL: &str = "https://platform.claude.com/v1/oauth/token";
const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const SIGN_IN_HINT: &str = "Use Sign in in this account's Claude settings.";
static OPERATIONS: Mutex<()> = Mutex::new(());
static REFRESH_STATE: LazyLock<Mutex<HashMap<String, RefreshState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) fn credential_guard() -> Result<MutexGuard<'static, ()>> {
    OPERATIONS
        .lock()
        .map_err(|_| anyhow::anyhow!("Claude credential operation failed."))
}

/// 0 = waiting; 1 = cancelled; 2 = saving. Cancellation cannot race a commit.
#[derive(Clone, Default)]
pub(crate) struct LoginControl(Arc<AtomicU8>);

impl PartialEq for LoginControl {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl LoginControl {
    pub(crate) fn cancel(&self) {
        let _ = self
            .0
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst);
    }
    pub(crate) fn cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst) == 1
    }
    pub(crate) fn begin_save(&self) -> Result<()> {
        ensure!(
            self.0
                .compare_exchange(0, 2, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok(),
            "Claude sign-in cancelled."
        );
        Ok(())
    }
}

/// Serialized only inside the protected secret slot. Never Debug or log tokens.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Session {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    scopes: Vec<String>,
    #[serde(default)]
    subscription_type: Option<String>,
    #[serde(default)]
    rate_limit_tier: Option<String>,
}

impl Session {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.access_token.starts_with("sk-ant-oat") && !self.refresh_token.trim().is_empty(),
            "Claude Code did not save a refreshable subscription login."
        );
        ensure!(
            self.scopes.iter().any(|scope| scope == "user:profile"),
            "Claude login has no subscription usage access (user:profile)."
        );
        ensure!(
            self.expires_at > 0,
            "Claude login has no expiry information."
        );
        Ok(())
    }
    pub(crate) fn encode(&self) -> Result<String> {
        self.validate()?;
        serde_json::to_string(self).context("encode Claude subscription login")
    }
    fn needs_refresh(&self) -> bool {
        self.expires_at <= (Utc::now() + chrono::Duration::minutes(5)).timestamp_millis()
    }
    fn fingerprint(&self) -> [u8; 32] {
        Sha256::digest(self.refresh_token.as_bytes()).into()
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
pub(crate) fn resolve(agent: &ureq::Agent, profile_id: &str, raw: &str) -> Result<Option<Session>> {
    resolve_using(agent, profile_id, raw, REFRESH_URL, |session| {
        super::save_profile_credential(profile_id, Some(&session.encode()?))
    })
}

fn resolve_using(
    agent: &ureq::Agent,
    profile_id: &str,
    raw: &str,
    refresh_url: &str,
    mut persist: impl FnMut(&Session) -> Result<()>,
) -> Result<Option<Session>> {
    if !raw.trim_start().starts_with('{') {
        return Ok(None);
    }
    let mut session: Session = serde_json::from_str(raw)
        .map_err(|_| anyhow::anyhow!("Saved Claude login is unreadable. {SIGN_IN_HINT}"))?;
    session.validate()?;
    let mut states = REFRESH_STATE
        .lock()
        .map_err(|_| anyhow::anyhow!("Claude refresh state failed."))?;
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
        } else if state.terminal {
            bail!("Claude subscription login was revoked. {SIGN_IN_HINT}");
        } else if state
            .retry_at
            .is_some_and(|deadline| deadline > Instant::now())
            && session.needs_refresh()
        {
            bail!("Claude token refresh is cooling down. Try again in a few minutes.");
        }
    }
    if !session.needs_refresh() {
        return Ok(Some(session));
    }
    let body = serde_json::json!({"grant_type":"refresh_token", "refresh_token":session.refresh_token, "client_id":CLIENT_ID, "scope":session.scopes.join(" ")});
    let response = agent
        .post(refresh_url)
        .set("Content-Type", "application/json")
        .set("Accept", "application/json")
        .set("anthropic-beta", super::OAUTH_BETA)
        .timeout(Duration::from_secs(15))
        .send_string(&body.to_string());
    let refreshed = match response {
        Ok(response) => response
            .into_string()
            .ok()
            .and_then(|body| serde_json::from_str::<RefreshResponse>(&body).ok())
            .and_then(|reply| reply.apply(&session).ok()),
        Err(ureq::Error::Status(status, response)) => {
            let terminal = matches!(status, 400 | 401)
                && response
                    .into_string()
                    .ok()
                    .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
                    .and_then(|body| body["error"].as_str().map(str::to_owned))
                    .is_some_and(|error| error.eq_ignore_ascii_case("invalid_grant"));
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
                bail!("Claude subscription login was revoked. {SIGN_IN_HINT}");
            }
            bail!(
                "Claude token refresh failed (HTTP {status}). Credentials were kept; retrying later."
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
        bail!("Claude token refresh failed. Credentials were kept; retrying later.");
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
    Ok(Some(refreshed))
}

impl Session {
    pub(crate) fn token(&self) -> &str {
        &self.access_token
    }
    pub(crate) fn plan(&self) -> Option<String> {
        super::plan_type_from_account_fields(
            self.subscription_type.clone(),
            self.rate_limit_tier.clone(),
        )
    }
}

#[derive(Deserialize)]
struct RefreshResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: i64,
    scope: Option<String>,
}

impl RefreshResponse {
    fn apply(self, current: &Session) -> Result<Session> {
        ensure!(
            self.expires_in > 300 && self.expires_in <= 365 * 24 * 60 * 60,
            "Invalid Claude token lifetime."
        );
        let session = Session {
            access_token: self.access_token.trim().into(),
            refresh_token: self
                .refresh_token
                .filter(|token| !token.trim().is_empty())
                .unwrap_or_else(|| current.refresh_token.clone()),
            expires_at: Utc::now().timestamp_millis() + self.expires_in * 1000,
            scopes: self
                .scope
                .map(|scope| scope.split_whitespace().map(str::to_owned).collect())
                .unwrap_or_else(|| current.scopes.clone()),
            subscription_type: current.subscription_type.clone(),
            rate_limit_tier: current.rate_limit_tier.clone(),
        };
        session.validate()?;
        Ok(session)
    }
}

const AUTH_OVERRIDES: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_PROFILE",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
    "CLAUDECODE",
];

fn login_command(executable: &Path, directory: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .args(["auth", "login", "--claudeai"])
        .env("CLAUDE_CONFIG_DIR", directory)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for key in AUTH_OVERRIDES {
        command.env_remove(key);
    }
    command
}

fn read_login(directory: &Path) -> Result<Session> {
    let bytes = std::fs::read(directory.join(".credentials.json")).context(
        "Claude Code did not save a login. Install native Windows Claude Code and try again.",
    )?;
    let file: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("Claude Code saved an unreadable login."))?;
    let session: Session = serde_json::from_value(file["claudeAiOauth"].clone()).map_err(|_| {
        anyhow::anyhow!("Claude Code did not save a refreshable subscription login.")
    })?;
    session.validate()?;
    Ok(session)
}

fn login_root() -> Result<std::path::PathBuf> {
    Ok(crate::settings::Settings::default_path()?
        .parent()
        .context("settings directory is unavailable")?
        .join("claude-logins"))
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
        Err(error) => return Err(error).context("read Claude sign-in directory"),
    };
    ensure!(
        metadata.is_dir() && !is_link(&metadata),
        "Claude sign-in directory must not be a link."
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
            "Claude sign-in directory escaped its root."
        );
        std::fs::remove_dir_all(path)?;
    }
    Ok(())
}

pub(crate) fn login(explicit: Option<&Path>, control: &LoginControl) -> Result<String> {
    ensure!(!control.cancelled(), "Claude sign-in cancelled.");
    let executable = super::cli_available(explicit)
        .context("Install native Windows Claude Code to sign in from Minibar.")?;
    ensure!(
        executable
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe")),
        "Sign-in requires native Windows Claude Code. Install its native executable, or use a pasted credential."
    );
    let root = login_root()?;
    std::fs::create_dir_all(&root).context("create Claude sign-in root")?;
    ensure!(
        !is_link(&std::fs::symlink_metadata(&root)?),
        "Claude sign-in directory must not be a link."
    );
    let directory = tempfile::Builder::new()
        .prefix("login-")
        .tempdir_in(root)
        .context("create isolated Claude sign-in directory")?;
    let command = login_command(&executable, directory.path());
    #[cfg(windows)]
    let mut child = login_child::LoginChild::spawn(&command)?;
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
            ensure!(!control.cancelled(), "Claude sign-in cancelled.");
            bail!(
                "Claude sign-in timed out. Finish the browser login within five minutes. If the browser shows a code, use the manual OAuth token tab instead."
            );
        }
        let status = match child.try_wait() {
            Ok(status) => status,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                bail!("Could not wait for Claude Code sign-in.");
            }
        };
        if let Some(status) = status {
            ensure!(
                status.success(),
                "Claude sign-in did not finish (exit {}). Try again, or use a pasted credential.",
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

    // A fixture-only loopback endpoint; no provider or real secret store is used.
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

    #[test]
    fn rotated_session_is_retried_after_save_failure_without_another_refresh() {
        let _guard = credential_guard().unwrap();
        let mut current = session();
        current.expires_at = 1;
        let raw = current.encode().unwrap();
        let (url, worker) = endpoint(
            "200 OK",
            r#"{"access_token":"sk-ant-oat-new","refresh_token":"rotated-refresh","expires_in":3600,"scope":"user:profile"}"#,
        );
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(5))
            .build();
        let id = "save-failure-fixture";
        forget(id);
        assert!(resolve_using(&agent, id, &raw, &url, |_| bail!("fixture write failure")).is_err());
        let grant = worker.join().unwrap();
        assert_eq!(grant["grant_type"], "refresh_token");
        assert_eq!(grant["refresh_token"], "refresh-fixture");
        let mut stored = None;
        let recovered = resolve_using(&agent, id, &raw, &url, |session| {
            stored = Some(session.encode()?);
            Ok(())
        })
        .unwrap()
        .unwrap();
        assert_eq!(recovered.token(), "sk-ant-oat-new");
        let stored: Session = serde_json::from_str(&stored.unwrap()).unwrap();
        assert_eq!(stored.refresh_token, "rotated-refresh");
        forget(id);
    }

    #[test]
    fn rejected_grant_is_not_retried_and_new_credentials_clear_rejection() {
        let _guard = credential_guard().unwrap();
        let mut current = session();
        current.expires_at = 1;
        let (url, worker) = endpoint("400 Bad Request", r#"{"error":"invalid_grant"}"#);
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(5))
            .build();
        let id = "revoked-fixture";
        forget(id);
        assert!(
            resolve_using(&agent, id, &current.encode().unwrap(), &url, |_| Ok(()))
                .err()
                .unwrap()
                .to_string()
                .contains("revoked")
        );
        worker.join().unwrap();
        assert!(
            resolve_using(&agent, id, &current.encode().unwrap(), &url, |_| Ok(()))
                .err()
                .unwrap()
                .to_string()
                .contains("revoked")
        );
        let mut replacement = session();
        replacement.refresh_token = "replacement-refresh".into();
        assert!(
            resolve_using(&agent, id, &replacement.encode().unwrap(), &url, |_| Ok(()))
                .unwrap()
                .is_some()
        );
        forget(id);
    }

    #[test]
    fn legacy_credentials_and_fresh_sessions_never_start_refresh() {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(1))
            .build();
        for raw in ["sk-ant-oat-pasted", "sessionKey=fixture"] {
            assert!(
                resolve_using(&agent, "legacy", raw, "http://127.0.0.1:1", |_| panic!(
                    "unexpected write"
                ))
                .unwrap()
                .is_none()
            );
        }
        let current = session();
        let fresh = resolve_using(
            &agent,
            "fresh",
            &current.encode().unwrap(),
            "http://127.0.0.1:1",
            |_| panic!("unexpected write"),
        )
        .unwrap()
        .unwrap();
        assert_eq!(fresh.token(), current.token());
    }

    #[test]
    fn abandoned_login_cleanup_keeps_other_directories_and_files() {
        let root = tempfile::tempdir().unwrap();
        let login = root.path().join("login-fixture");
        std::fs::create_dir(&login).unwrap();
        std::fs::write(login.join(".credentials.json"), "fixture").unwrap();
        let keep = root.path().join("unrelated");
        std::fs::create_dir(&keep).unwrap();
        std::fs::write(root.path().join("login-file"), "keep").unwrap();
        cleanup_root(root.path()).unwrap();
        assert!(!login.exists());
        assert!(keep.exists());
        assert!(root.path().join("login-file").exists());
    }
    fn session() -> Session {
        Session {
            access_token: "sk-ant-oat-fixture".into(),
            refresh_token: "refresh-fixture".into(),
            expires_at: 2_000_000_000_000,
            scopes: vec!["user:profile".into()],
            subscription_type: Some("pro".into()),
            rate_limit_tier: None,
        }
    }
    #[test]
    fn login_command_is_isolated_and_removes_auth_overrides() {
        let command = login_command(Path::new("claude.exe"), Path::new("isolated"));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["auth", "login", "--claudeai"]
        );
        assert_eq!(command.get_current_dir(), Some(Path::new("isolated")));
        for key in AUTH_OVERRIDES {
            assert!(
                command
                    .get_envs()
                    .any(|(name, value)| name == *key && value.is_none())
            );
        }
        assert!(
            command
                .get_envs()
                .any(|(name, value)| name == "CLAUDE_CONFIG_DIR"
                    && value == Some(std::ffi::OsStr::new("isolated")))
        );
    }
    #[test]
    fn refresh_rotates_tokens_and_keeps_plan_and_missing_refresh_token() {
        let current = session();
        let next = RefreshResponse {
            access_token: "sk-ant-oat-new".into(),
            refresh_token: Some("new-refresh".into()),
            expires_in: 3600,
            scope: None,
        }
        .apply(&current)
        .unwrap();
        assert_eq!(next.refresh_token, "new-refresh");
        assert_eq!(next.subscription_type, current.subscription_type);
        assert_eq!(next.scopes, current.scopes);
        assert!(!next.needs_refresh());
        let next = RefreshResponse {
            access_token: "sk-ant-oat-new".into(),
            refresh_token: None,
            expires_in: 3600,
            scope: None,
        }
        .apply(&current)
        .unwrap();
        assert_eq!(next.refresh_token, current.refresh_token);
    }
    #[test]
    fn login_requires_refresh_and_usage_scope() {
        let mut current = session();
        current.refresh_token.clear();
        assert!(current.validate().is_err());
        current.refresh_token = "refresh".into();
        current.scopes = vec!["user:inference".into()];
        assert!(current.validate().is_err());
    }
    #[test]
    fn cancellation_and_save_are_mutually_exclusive() {
        let control = LoginControl::default();
        control.cancel();
        assert!(control.begin_save().is_err());
        let control = LoginControl::default();
        control.begin_save().unwrap();
        control.cancel();
        assert!(!control.cancelled());
    }
}
