use std::{
    ffi::OsStr,
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, mpsc},
    thread,
    time::{Duration as StdDuration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, TimeZone, Utc};
use directories::BaseDirs;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::limits::{
    AdditionalLimit, Credits, LimitWindow, RateLimitResetCredit, RateLimitResetCreditsSummary,
    RateLimits,
};
use crate::usage;
use crate::worker::{Activator, LimitProvider, UsageProvider};

pub const ACTIVATION_PROMPT: &str = "Reply exactly: a";
pub const ACTIVATION_MODEL: &str = "gpt-5.6-luna";

/// ChatGPT WHAM quota endpoint used by CodexBar and the Codex CLI account
/// path. Prefer this over spawning `codex app-server` so periodic polls do not
/// touch the Windows sandbox / LSASS path (issue #25).
const WHAM_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const WHAM_RESET_CREDITS_URL: &str =
    "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits";

pub struct CodexClient {
    executable: PathBuf,
    timeout: StdDuration,
}

impl LimitProvider for CodexClient {
    fn read_limits(&mut self) -> Result<RateLimits> {
        self.read_rate_limits()
    }
}

impl UsageProvider for CodexClient {
    fn account_identity(&self) -> Option<String> {
        crate::store::codex_accounts::poll_identity()
    }
    fn identity_poll_interval(&self) -> StdDuration {
        StdDuration::from_secs(1)
    }

    fn load_cached_usage_statistics(
        &mut self,
        history_days: u16,
    ) -> Result<usage::UsageStatistics> {
        usage::load_cached_usage_statistics(history_days)
    }

    fn refresh_usage_statistics(&mut self, history_days: u16) -> Result<usage::UsageStatistics> {
        usage::refresh_usage_statistics(history_days)
    }
}

pub struct CodexActivator {
    executable: PathBuf,
    timeout: StdDuration,
}

impl CodexActivator {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            timeout: StdDuration::from_secs(120),
        }
    }

    pub fn activate_minimal(&self) -> Result<()> {
        // Persist a real (tiny) session so the ChatGPT 5-hour window starts.
        // `--ephemeral` previously returned exit 0 without opening quota.
        let workspace = activation_workspace_dir()?;
        let last_message_path = workspace.join("last-message.txt");
        crate::logger::info(format!(
            "Codex activation exec model={} reasoning_effort=low",
            ACTIVATION_MODEL
        ));
        let args = activation_args(&workspace, &last_message_path);
        let mut child = command_for_codex(&self.executable, &args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("launch activation through {}", self.executable.display()))?;
        let stderr = child.stderr.take();
        let stderr_reader = thread::spawn(move || {
            let mut buf = String::new();
            if let Some(mut stderr) = stderr {
                let _ = stderr.read_to_string(&mut buf);
            }
            buf
        });
        let deadline = Instant::now() + self.timeout;
        let status = loop {
            if let Some(status) = child.try_wait().context("wait for Codex activation")? {
                break status;
            }
            if Instant::now() >= deadline {
                terminate(&mut child);
                let stderr = stderr_reader.join().unwrap_or_default().trim().to_owned();
                if stderr.is_empty() {
                    bail!("Codex activation timed out after {:?}", self.timeout);
                }
                bail!(
                    "Codex activation timed out after {:?}: {stderr}",
                    self.timeout
                );
            }
            thread::sleep(StdDuration::from_millis(100));
        };
        let stderr = stderr_reader.join().unwrap_or_default();
        let stderr = stderr.trim();
        if !status.success() {
            if stderr.is_empty() {
                bail!("Codex activation exited with {status}");
            }
            bail!("Codex activation exited with {status}: {stderr}");
        }
        let reply = fs::read_to_string(&last_message_path).unwrap_or_default();
        let reply = reply.trim();
        if reply.is_empty() {
            if stderr.is_empty() {
                bail!("Codex activation produced no model output");
            }
            bail!("Codex activation produced no model output: {stderr}");
        }
        crate::logger::info(format!("Codex activation reply: {reply}"));
        Ok(())
    }
}

impl Activator for CodexActivator {
    fn activate(&mut self) -> Result<()> {
        self.activate_minimal()
    }
}

impl CodexClient {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            timeout: StdDuration::from_secs(10),
        }
    }

    pub fn with_timeout(mut self, timeout: StdDuration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn read_rate_limits(&self) -> Result<RateLimits> {
        match self.read_rate_limits_via_oauth() {
            Ok(limits) => Ok(limits),
            Err(oauth_error) => {
                crate::logger::info(format!(
                    "Codex OAuth quota unavailable ({oauth_error:#}); falling back to app-server"
                ));
                self.read_rate_limits_via_app_server().map_err(|cli_error| {
                    cli_error.context(format!("OAuth quota failed: {oauth_error:#}"))
                })
            }
        }
    }

    fn read_rate_limits_via_oauth(&self) -> Result<RateLimits> {
        let credentials = load_oauth_credentials()?;
        let tls = ureq::native_tls::TlsConnector::new().context("create Windows TLS connector")?;
        let agent = ureq::AgentBuilder::new()
            .timeout(self.timeout)
            .tls_connector(Arc::new(tls))
            .build();
        let mut request = agent
            .get(WHAM_USAGE_URL)
            .set(
                "Authorization",
                &format!("Bearer {}", credentials.access_token),
            )
            .set("Accept", "application/json")
            .set(
                "User-Agent",
                &format!("Codex-Minibar/{}", env!("CARGO_PKG_VERSION")),
            );
        if let Some(account_id) = credentials.account_id.as_deref() {
            request = request.set("ChatGPT-Account-Id", account_id);
        }
        let body = match request.call() {
            Ok(response) => response
                .into_string()
                .context("read Codex OAuth usage response")?,
            Err(ureq::Error::Status(401, _)) => {
                bail!("Codex OAuth token expired or invalid. Run `codex login`.")
            }
            Err(ureq::Error::Status(429, _)) => {
                bail!("Codex usage endpoint is rate limited. Try again in a few minutes.")
            }
            Err(ureq::Error::Status(status, _)) => {
                bail!("Codex OAuth usage request failed with HTTP {status}")
            }
            Err(error) => return Err(error).context("request Codex OAuth usage"),
        };
        let value: Value =
            serde_json::from_str(&body).context("parse Codex OAuth usage response")?;
        let mut limits = parse_wham_usage(&value, Utc::now())?;
        if let Ok(reset_credits) = fetch_wham_reset_credits(&agent, &credentials) {
            limits.reset_credits = reset_credits;
        }
        if let Some(account_name) = local_account_name() {
            limits.account_name = Some(account_name);
        }
        Ok(limits)
    }

    fn read_rate_limits_via_app_server(&self) -> Result<RateLimits> {
        // Desktop Codex dropped the legacy `untrusted` approval policy; only
        // `never` / `on-request` remain. Keep `-a never` so rate-limit polls
        // never block on an interactive approval prompt.
        let mut child = spawn_codex(
            &self.executable,
            &["-s", "read-only", "-a", "never", "app-server"],
        )?;
        let stderr = child.stderr.take();
        let result = self.exchange(&mut child);
        terminate(&mut child);
        let result = match (result, stderr) {
            (Err(error), Some(stderr)) => Err(enrich_with_stderr(error, stderr)),
            (result, _) => result,
        };
        result.map(|mut limits| {
            // The account name is a display-only claim from the locally
            // authenticated Codex session. Never let a missing or malformed
            // identity token make otherwise valid quota data unavailable.
            limits.account_name = local_account_name();
            limits
        })
    }

    fn exchange(&self, child: &mut Child) -> Result<RateLimits> {
        let mut stdin = child
            .stdin
            .take()
            .context("Codex app-server stdin unavailable")?;
        let stdout = child
            .stdout
            .take()
            .context("Codex app-server stdout unavailable")?;
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(line) => {
                        if sender.send(line).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        send_request(
            &mut stdin,
            1,
            "initialize",
            json!({"clientInfo": {"name": "Codex Minibar", "version": env!("CARGO_PKG_VERSION")}}),
        )?;
        wait_for_response(&receiver, 1, self.timeout)?;
        // Complete the JSON-RPC initialize handshake required by current
        // Codex app-server builds before any other method is accepted.
        send_notification(&mut stdin, "initialized", json!({}))?;
        send_request(&mut stdin, 2, "account/rateLimits/read", Value::Null)?;
        let response = wait_for_response(&receiver, 2, self.timeout)?;
        parse_rate_limits(&response, Utc::now())
    }
}

#[derive(Deserialize)]
struct AuthFile {
    tokens: Option<AuthTokens>,
}

#[derive(Deserialize)]
struct AuthTokens {
    id_token: Option<String>,
    access_token: Option<String>,
    account_id: Option<String>,
}

#[derive(Clone)]
struct OAuthCredentials {
    access_token: String,
    account_id: Option<String>,
}

#[derive(Deserialize)]
struct IdTokenClaims {
    name: Option<String>,
    email: Option<String>,
}

fn load_oauth_credentials() -> Result<OAuthCredentials> {
    let path = auth_json_path().context("resolve Codex auth.json")?;
    let contents = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    let auth: AuthFile =
        serde_json::from_slice(&contents).with_context(|| format!("parse {}", path.display()))?;
    let tokens = auth
        .tokens
        .context("Codex auth.json has no OAuth tokens; run `codex login`")?;
    let access_token = tokens
        .access_token
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .context("Codex auth.json has no access token; run `codex login`")?;
    let account_id = tokens
        .account_id
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    Ok(OAuthCredentials {
        access_token,
        account_id,
    })
}

fn auth_json_path() -> Option<PathBuf> {
    BaseDirs::new().map(|dirs| dirs.home_dir().join(".codex").join("auth.json"))
}

fn local_account_name() -> Option<String> {
    let contents = fs::read(auth_json_path()?).ok()?;
    let auth: AuthFile = serde_json::from_slice(&contents).ok()?;
    let token = auth.tokens?.id_token?;
    account_name_from_id_token(&token)
}

fn account_name_from_id_token(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let claims: IdTokenClaims = serde_json::from_slice(&decoded).ok()?;
    non_empty(claims.name).or_else(|| non_empty(claims.email))
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

fn fetch_wham_reset_credits(
    agent: &ureq::Agent,
    credentials: &OAuthCredentials,
) -> Result<Option<RateLimitResetCreditsSummary>> {
    let mut request = agent
        .get(WHAM_RESET_CREDITS_URL)
        .set(
            "Authorization",
            &format!("Bearer {}", credentials.access_token),
        )
        .set("Accept", "application/json")
        .set(
            "User-Agent",
            &format!("Codex-Minibar/{}", env!("CARGO_PKG_VERSION")),
        );
    if let Some(account_id) = credentials.account_id.as_deref() {
        request = request.set("ChatGPT-Account-Id", account_id);
    }
    let body = request
        .call()
        .context("request Codex reset credits")?
        .into_string()
        .context("read Codex reset credits response")?;
    let value: Value = serde_json::from_str(&body).context("parse Codex reset credits response")?;
    Ok(parse_wham_reset_credits(Some(&value)))
}

pub fn parse_wham_usage(response: &Value, sampled_at: DateTime<Utc>) -> Result<RateLimits> {
    let rate_limit = response
        .get("rate_limit")
        .context("missing rate_limit in Codex OAuth usage response")?;
    let primary = parse_wham_window(rate_limit.get("primary_window"));
    let primary_window_is_unactivated = primary.looks_like_unactivated_five_hour(sampled_at);
    let account_name = response
        .get("email")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    Ok(RateLimits {
        primary,
        secondary: parse_wham_window(rate_limit.get("secondary_window")),
        sampled_at,
        primary_window_is_unactivated,
        account_name,
        plan_type: response
            .get("plan_type")
            .and_then(Value::as_str)
            .map(str::to_owned),
        limit_name: None,
        secondary_limit_name: None,
        credits: parse_wham_credits(response.get("credits")),
        reset_credits: parse_wham_reset_credits(response.get("rate_limit_reset_credits")),
        additional_limits: parse_wham_additional_limits(response.get("additional_rate_limits")),
        spending: None,
        openrouter_accounts: Default::default(),
        usage: Default::default(),
    }
    .normalized(sampled_at))
}

fn parse_wham_window(value: Option<&Value>) -> LimitWindow {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return LimitWindow::default();
    };
    let used_percent = value
        .get("used_percent")
        .and_then(json_u64)
        .and_then(|value| u8::try_from(value.min(100)).ok());
    let resets_at = value
        .get("reset_at")
        .and_then(json_i64)
        .and_then(|timestamp| Utc.timestamp_opt(timestamp, 0).single());
    let duration_minutes = value
        .get("limit_window_seconds")
        .and_then(json_u64)
        .map(|seconds| (seconds / 60) as u32)
        .filter(|minutes| *minutes > 0);
    LimitWindow {
        used_percent,
        resets_at,
        duration_minutes,
    }
}

fn parse_wham_credits(value: Option<&Value>) -> Credits {
    Credits {
        has_credits: value
            .and_then(|v| v.get("has_credits"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        unlimited: value
            .and_then(|v| v.get("unlimited"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        balance: value.and_then(|v| v.get("balance")).and_then(|balance| {
            balance
                .as_str()
                .map(str::to_owned)
                .or_else(|| balance.as_f64().map(|value| value.to_string()))
                .or_else(|| balance.as_i64().map(|value| value.to_string()))
        }),
    }
}

fn parse_wham_reset_credits(value: Option<&Value>) -> Option<RateLimitResetCreditsSummary> {
    let value = value.filter(|value| !value.is_null())?;
    let available_count = value
        .get("available_count")
        .or_else(|| value.get("availableCount"))
        .and_then(json_u64)
        .and_then(|count| u32::try_from(count).ok())
        .unwrap_or(0);
    let credits = value
        .get("credits")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|credit| RateLimitResetCredit {
            reset_type: credit
                .get("reset_type")
                .or_else(|| credit.get("resetType"))
                .and_then(Value::as_str)
                .map(str::to_owned),
            status: credit
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            granted_at: parse_flexible_timestamp(
                credit.get("granted_at").or_else(|| credit.get("grantedAt")),
            ),
            expires_at: parse_flexible_timestamp(
                credit.get("expires_at").or_else(|| credit.get("expiresAt")),
            ),
            title: credit
                .get("title")
                .and_then(Value::as_str)
                .map(str::to_owned),
            description: credit
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
        .collect();
    Some(RateLimitResetCreditsSummary {
        available_count,
        credits,
    })
}

fn parse_wham_additional_limits(value: Option<&Value>) -> Vec<AdditionalLimit> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let id = entry
                .get("limit_name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?
                .to_owned();
            let window = parse_wham_window(
                entry
                    .pointer("/rate_limit/primary_window")
                    .or_else(|| entry.get("primary_window")),
            );
            if window == LimitWindow::default() {
                return None;
            }
            Some(AdditionalLimit {
                title: additional_limit_title(&id),
                id,
                window,
            })
        })
        .collect()
}

fn additional_limit_title(id: &str) -> String {
    if id.eq_ignore_ascii_case(crate::limits::GPT_RESERVE_LIMIT_ID) {
        crate::limits::LUNA_RESERVE_TITLE.to_owned()
    } else {
        id.to_owned()
    }
}

fn json_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|value| u64::try_from(value).ok()))
        .or_else(|| {
            value
                .as_f64()
                .and_then(|value| (value >= 0.0).then_some(value as u64))
        })
}

fn json_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_f64().map(|value| value as i64))
}

fn parse_flexible_timestamp(value: Option<&Value>) -> Option<DateTime<Utc>> {
    let value = value?;
    if let Some(timestamp) = json_i64(value) {
        return Utc.timestamp_opt(timestamp, 0).single();
    }
    value
        .as_str()
        .and_then(|raw| DateTime::parse_from_rfc3339(raw).ok())
        .map(|parsed| parsed.with_timezone(&Utc))
}

fn send_request(stdin: &mut impl Write, id: u64, method: &str, params: Value) -> Result<()> {
    serde_json::to_writer(
        &mut *stdin,
        &json!({"id": id, "method": method, "params": params}),
    )?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;
    Ok(())
}

fn send_notification(stdin: &mut impl Write, method: &str, params: Value) -> Result<()> {
    serde_json::to_writer(&mut *stdin, &json!({"method": method, "params": params}))?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;
    Ok(())
}

fn enrich_with_stderr(error: anyhow::Error, mut stderr: impl std::io::Read) -> anyhow::Error {
    let mut message = String::new();
    let _ = stderr.read_to_string(&mut message);
    let message = message.trim();
    if message.is_empty() {
        error
    } else {
        error.context(format!("codex stderr: {message}"))
    }
}

fn wait_for_response(
    receiver: &mpsc::Receiver<String>,
    id: u64,
    timeout: StdDuration,
) -> Result<Value> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let line = match receiver.recv_timeout(remaining) {
            Ok(line) => line,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                bail!("Codex app-server response timed out");
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                bail!("Codex app-server exited before responding");
            }
        };
        let value: Value =
            serde_json::from_str(&line).context("invalid JSON from Codex app-server")?;
        if value.get("id").and_then(Value::as_u64) == Some(id) {
            if let Some(error) = value.get("error") {
                bail!("Codex app-server error: {error}");
            }
            return Ok(value);
        }
    }
}

pub fn parse_rate_limits(
    response: &Value,
    sampled_at: chrono::DateTime<Utc>,
) -> Result<RateLimits> {
    let limits = response
        .pointer("/result/rateLimits")
        .context("missing result.rateLimits")?;
    let primary = parse_window(limits.get("primary"));
    // This only marks a single-response candidate. The scheduler compares two
    // or three neighboring responses before deciding whether the reset is
    // fixed (active) or follows each request by five hours (not activated).
    let primary_window_is_unactivated = primary.looks_like_unactivated_five_hour(sampled_at);
    Ok(RateLimits {
        primary,
        secondary: parse_window(limits.get("secondary")),
        sampled_at,
        primary_window_is_unactivated,
        account_name: None,
        plan_type: limits
            .get("planType")
            .and_then(Value::as_str)
            .map(str::to_owned),
        limit_name: limits
            .get("limitName")
            .and_then(Value::as_str)
            .map(str::to_owned),
        secondary_limit_name: None,
        credits: parse_credits(limits.get("credits")),
        reset_credits: parse_reset_credits(response.pointer("/result/rateLimitResetCredits")),
        additional_limits: Default::default(),
        spending: None,
        openrouter_accounts: Default::default(),
        usage: Default::default(),
    }
    .normalized(sampled_at))
}

fn parse_window(value: Option<&Value>) -> LimitWindow {
    let used_percent = value
        .and_then(|v| v.get("usedPercent"))
        .and_then(Value::as_u64)
        .and_then(|value| u8::try_from(value.min(100)).ok());
    let resets_at = value
        .and_then(|v| v.get("resetsAt"))
        .and_then(Value::as_i64)
        .and_then(|timestamp| Utc.timestamp_opt(timestamp, 0).single());
    let duration_minutes = value
        .and_then(|v| v.get("windowDurationMins"))
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok());
    LimitWindow {
        used_percent,
        resets_at,
        duration_minutes,
    }
}

fn parse_credits(value: Option<&Value>) -> Credits {
    Credits {
        has_credits: value
            .and_then(|v| v.get("hasCredits"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        unlimited: value
            .and_then(|v| v.get("unlimited"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        balance: value
            .and_then(|v| v.get("balance"))
            .and_then(Value::as_str)
            .map(str::to_owned),
    }
}

fn parse_reset_credits(value: Option<&Value>) -> Option<RateLimitResetCreditsSummary> {
    let value = value?;
    let available_count = value
        .get("availableCount")
        .and_then(Value::as_u64)
        .and_then(|count| u32::try_from(count).ok())
        .unwrap_or(0);
    let credits = value
        .get("credits")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|credit| RateLimitResetCredit {
            reset_type: credit
                .get("resetType")
                .and_then(Value::as_str)
                .map(str::to_owned),
            status: credit
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            granted_at: parse_timestamp(credit.get("grantedAt")),
            expires_at: parse_timestamp(credit.get("expiresAt")),
            title: credit
                .get("title")
                .and_then(Value::as_str)
                .map(str::to_owned),
            description: credit
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
        .collect();
    Some(RateLimitResetCreditsSummary {
        available_count,
        credits,
    })
}

fn parse_timestamp(value: Option<&Value>) -> Option<chrono::DateTime<Utc>> {
    value
        .and_then(Value::as_i64)
        .and_then(|timestamp| Utc.timestamp_opt(timestamp, 0).single())
}

fn activation_workspace_dir() -> Result<PathBuf> {
    let dir = BaseDirs::new()
        .context("resolve home directory")?
        .home_dir()
        .join(".codex")
        .join("minibar-activation");
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    Ok(dir)
}

fn activation_args(workspace: &Path, last_message: &Path) -> Vec<String> {
    // Keep quota activation deterministic and cheap. Do not inherit the
    // user's interactive model or service tier from ~/.codex/config.toml.
    let mut args = vec![
        "exec".into(),
        ACTIVATION_PROMPT.into(),
        "--model".into(),
        ACTIVATION_MODEL.into(),
    ];
    args.extend([
        "--config".into(),
        "model_reasoning_effort=\"low\"".into(),
        "--config".into(),
        "mcp_servers={}".into(),
        "--config".into(),
        "notify=[]".into(),
        "--sandbox".into(),
        "read-only".into(),
        "--ignore-rules".into(),
        "--skip-git-repo-check".into(),
        "--color".into(),
        "never".into(),
        "--cd".into(),
        workspace.to_string_lossy().into_owned(),
        "--output-last-message".into(),
        last_message.to_string_lossy().into_owned(),
        "--disable".into(),
        "plugins".into(),
        "--disable".into(),
        "apps".into(),
        "--disable".into(),
        "browser_use".into(),
        "--disable".into(),
        "in_app_browser".into(),
        "--disable".into(),
        "computer_use".into(),
        "--disable".into(),
        "image_generation".into(),
        "--disable".into(),
        "multi_agent".into(),
        "--disable".into(),
        "goals".into(),
        "--disable".into(),
        "workspace_dependencies".into(),
        "--disable".into(),
        "hooks".into(),
        "--disable".into(),
        "tool_suggest".into(),
    ]);
    args
}

fn spawn_codex(executable: &Path, args: &[&str]) -> Result<Child> {
    command_for_codex(executable, args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("launch {}", executable.display()))
}

fn command_for_codex(
    executable: &Path,
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
) -> Command {
    let extension = executable
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let mut command =
        if cfg!(windows) && matches!(extension.to_ascii_lowercase().as_str(), "cmd" | "bat") {
            let mut command = Command::new("cmd.exe");
            command.args(["/D", "/C"]).arg(executable).args(args);
            command
        } else if cfg!(windows) && extension.eq_ignore_ascii_case("ps1") {
            let mut command = Command::new("powershell.exe");
            command
                .args([
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                ])
                .arg(executable)
                .args(args);
            command
        } else {
            let mut command = Command::new(executable);
            command.args(args);
            command
        };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    command
}

fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

pub fn first_available(explicit: Option<&Path>) -> Result<PathBuf> {
    crate::discovery::discover(explicit)
        .into_iter()
        .next()
        .map(|candidate| candidate.path)
        .ok_or_else(|| anyhow!("Codex executable was not found"))
}

/// Returns whether a local Codex CLI or the Codex desktop-app CLI bridge is
/// present. This is intentionally filesystem-only so onboarding never starts
/// a provider process merely to identify an installation.
pub fn is_installed(explicit: Option<&Path>) -> bool {
    first_available(explicit).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_present_and_missing_windows() {
        let now = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
        let value = json!({"id": 2, "result": {"rateLimits": {
            "primary": {"usedPercent": 27, "resetsAt": 1_700_003_600, "windowDurationMins": 300},
            "secondary": null
        }}});
        let parsed = parse_rate_limits(&value, now).unwrap();
        assert_eq!(parsed.primary.used_percent, Some(27));
        assert_eq!(parsed.primary.remaining_percent(), Some(73));
        assert_eq!(parsed.primary.duration_minutes, Some(300));
        assert_eq!(parsed.secondary, LimitWindow::default());
        assert!(!parsed.primary_window_is_unactivated);
        assert!(!parsed.five_hour_disabled());
    }

    #[test]
    fn remaps_weekly_primary_when_five_hour_is_gone() {
        let now = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
        let value = json!({"result": {"rateLimits": {
            "primary": {
                "usedPercent": 14,
                "resetsAt": 1_700_475_600,
                "windowDurationMins": 10080
            },
            "secondary": null
        }}});
        let parsed = parse_rate_limits(&value, now).unwrap();
        assert!(parsed.five_hour_disabled());
        assert_eq!(parsed.primary, LimitWindow::default());
        assert_eq!(parsed.secondary.used_percent, Some(14));
        assert_eq!(parsed.secondary.duration_minutes, Some(10_080));
        assert!(!parsed.primary_window_is_unactivated);
        assert_eq!(parsed.effective_primary().used_percent, Some(14));
    }

    #[test]
    fn marks_codex_unactivated_five_hour_window() {
        let sampled_at = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
        let value = json!({"result": {"rateLimits": {
            "primary": {
                "usedPercent": 0,
                "resetsAt": sampled_at.timestamp() + 5 * 60 * 60,
                "windowDurationMins": 300
            },
            "secondary": {
                "usedPercent": 0,
                "resetsAt": sampled_at.timestamp() + 7 * 24 * 60 * 60,
                "windowDurationMins": 10080
            }
        }}});

        let parsed = parse_rate_limits(&value, sampled_at).unwrap();
        assert!(parsed.primary_window_is_unactivated);
        assert_eq!(parsed.primary.duration_minutes, Some(300));
        assert_eq!(parsed.secondary.duration_minutes, Some(10_080));
    }

    #[test]
    fn clamps_out_of_range_percentages() {
        let value = json!({"result": {"rateLimits": {"primary": {"usedPercent": 999}}}});
        let parsed = parse_rate_limits(&value, Utc::now()).unwrap();
        assert_eq!(parsed.primary.used_percent, Some(100));
    }

    #[test]
    fn account_name_prefers_name_and_falls_back_to_email() {
        let token = |claims: &str| {
            format!(
                "header.{}.signature",
                URL_SAFE_NO_PAD.encode(claims.as_bytes())
            )
        };

        assert_eq!(
            account_name_from_id_token(&token(
                r#"{"name":"Ada Lovelace","email":"ada@example.com"}"#
            )),
            Some("Ada Lovelace".into())
        );
        assert_eq!(
            account_name_from_id_token(&token(r#"{"name":"  ","email":"ada@example.com"}"#)),
            Some("ada@example.com".into())
        );
    }

    #[test]
    fn parses_banked_reset_credits_and_expiration() {
        let value = json!({"result": {
            "rateLimits": {"primary": null, "secondary": null},
            "rateLimitResetCredits": {
                "availableCount": 1,
                "credits": [{
                    "resetType": "codexRateLimits",
                    "status": "available",
                    "grantedAt": 1_783_965_251_i64,
                    "expiresAt": 1_786_557_251_i64,
                    "title": "Full reset",
                    "description": "One free rate limit reset."
                }]
            }
        }});

        let parsed = parse_rate_limits(&value, Utc::now()).unwrap();
        let summary = parsed.reset_credits.as_ref().unwrap();
        assert_eq!(summary.available_count, 1);
        assert_eq!(summary.credits[0].status, "available");
        assert_eq!(summary.credits[0].title.as_deref(), Some("Full reset"));
        assert_eq!(
            parsed.next_reset_credit_expiration(),
            Utc.timestamp_opt(1_786_557_251, 0).single()
        );
    }

    #[test]
    fn parses_wham_oauth_usage_windows_and_credits() {
        let sampled_at = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
        let value = json!({
            "email": "ada@example.com",
            "plan_type": "plus",
            "rate_limit": {
                "primary_window": {
                    "used_percent": 12,
                    "limit_window_seconds": 18_000,
                    "reset_at": 1_700_003_600
                },
                "secondary_window": {
                    "used_percent": 44,
                    "limit_window_seconds": 604_800,
                    "reset_at": 1_700_475_600
                }
            },
            "credits": {
                "has_credits": true,
                "unlimited": false,
                "balance": "12.5"
            },
            "rate_limit_reset_credits": {
                "available_count": 2
            },
            "additional_rate_limits": [{
                "limit_name": "gpt-reserve",
                "rate_limit": {
                    "primary_window": {
                        "used_percent": 0,
                        "limit_window_seconds": 604_800,
                        "reset_at": 1_700_604_800
                    }
                }
            }]
        });

        let parsed = parse_wham_usage(&value, sampled_at).unwrap();
        assert_eq!(parsed.account_name.as_deref(), Some("ada@example.com"));
        assert_eq!(parsed.plan_type.as_deref(), Some("plus"));
        assert_eq!(parsed.primary.used_percent, Some(12));
        assert_eq!(parsed.primary.duration_minutes, Some(300));
        assert_eq!(parsed.secondary.used_percent, Some(44));
        assert_eq!(parsed.secondary.duration_minutes, Some(10_080));
        assert!(parsed.credits.has_credits);
        assert_eq!(parsed.credits.balance.as_deref(), Some("12.5"));
        assert_eq!(parsed.reset_credits.as_ref().unwrap().available_count, 2);
        assert_eq!(parsed.additional_limits.len(), 1);
        assert_eq!(parsed.additional_limits[0].id, "gpt-reserve");
        assert_eq!(
            parsed.additional_limits[0].title,
            crate::limits::LUNA_RESERVE_TITLE
        );
        assert_eq!(parsed.additional_limits[0].window.used_percent, Some(0));
    }

    #[test]
    fn parses_wham_reset_credit_iso_timestamps() {
        let value = json!({
            "available_count": 1,
            "credits": [{
                "reset_type": "codex_rate_limits",
                "status": "available",
                "granted_at": "2026-09-04T01:14:53.340415Z",
                "expires_at": "2026-10-04T01:14:53.340415Z",
                "title": "Full reset"
            }]
        });
        let summary = parse_wham_reset_credits(Some(&value)).unwrap();
        assert_eq!(summary.available_count, 1);
        assert_eq!(summary.credits[0].status, "available");
        assert_eq!(summary.credits[0].title.as_deref(), Some("Full reset"));
        assert!(summary.credits[0].expires_at.is_some());
    }

    #[test]
    #[ignore = "requires an installed and authenticated Codex CLI"]
    fn reads_live_rate_limits() {
        let executable = first_available(None).expect("Codex CLI should be discoverable");
        let limits = CodexClient::new(executable)
            .read_rate_limits()
            .expect("Codex quota should return rate limits");
        assert!(limits.primary.used_percent.is_some() || limits.primary.resets_at.is_some());
    }

    #[test]
    fn activation_exec_persists_a_real_session_and_captures_the_reply() {
        let args = activation_args(
            Path::new("C:\\tmp\\act"),
            Path::new("C:\\tmp\\act\\out.txt"),
        );
        assert!(args.contains(&"--output-last-message".into()));
        assert!(args.contains(&"--cd".into()));
        assert!(!args.iter().any(|arg| arg == "--ephemeral"));
        assert!(!args.iter().any(|arg| arg == "--ignore-user-config"));
        assert!(args.contains(&"--model".into()));
        assert!(args.contains(&ACTIVATION_MODEL.into()));
        assert!(args.contains(&"model_reasoning_effort=\"low\"".into()));
        assert!(!args.iter().any(|arg| arg == "service_tier=\"priority\""));
    }
}
