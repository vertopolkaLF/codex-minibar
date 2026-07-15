use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration as StdDuration, Instant},
};

use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{TimeZone, Utc};
use directories::BaseDirs;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::limits::{
    Credits, LimitWindow, RateLimitResetCredit, RateLimitResetCreditsSummary, RateLimits,
};
use crate::usage;
use crate::worker::{Activator, LimitProvider, UsageProvider};

pub const ACTIVATION_MODEL: &str = "gpt-5.4-mini";
pub const ACTIVATION_PROMPT: &str = "Reply exactly: a";

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
    fn load_cached_usage_statistics(
        &mut self,
        history_days: u16,
    ) -> Result<usage::UsageStatistics> {
        usage::load_cached_usage_statistics(history_days)
    }

    fn refresh_usage_statistics(
        &mut self,
        history_days: u16,
    ) -> Result<usage::UsageStatistics> {
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
        let args = [
            "exec",
            ACTIVATION_PROMPT,
            "--model",
            ACTIVATION_MODEL,
            "--config",
            "model_reasoning_effort=\"low\"",
            "--sandbox",
            "read-only",
            "--ephemeral",
            "--ignore-user-config",
            "--ignore-rules",
            "--skip-git-repo-check",
            "--color",
            "never",
            "--disable",
            "plugins",
            "--disable",
            "apps",
            "--disable",
            "browser_use",
            "--disable",
            "in_app_browser",
            "--disable",
            "computer_use",
            "--disable",
            "image_generation",
            "--disable",
            "multi_agent",
            "--disable",
            "goals",
            "--disable",
            "workspace_dependencies",
            "--disable",
            "hooks",
            "--disable",
            "tool_suggest",
        ];
        let mut child = command_for_codex(&self.executable, &args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("launch activation through {}", self.executable.display()))?;
        let deadline = Instant::now() + self.timeout;
        loop {
            if let Some(status) = child.try_wait().context("wait for Codex activation")? {
                anyhow::ensure!(status.success(), "Codex activation exited with {status}");
                return Ok(());
            }
            if Instant::now() >= deadline {
                terminate(&mut child);
                bail!("Codex activation timed out after {:?}", self.timeout);
            }
            thread::sleep(StdDuration::from_millis(100));
        }
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
        let mut child = spawn_codex(
            &self.executable,
            &["-s", "read-only", "-a", "untrusted", "app-server"],
        )?;
        let result = self.exchange(&mut child);
        terminate(&mut child);
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
}

#[derive(Deserialize)]
struct IdTokenClaims {
    name: Option<String>,
    email: Option<String>,
}

fn local_account_name() -> Option<String> {
    let home = BaseDirs::new()?.home_dir().to_path_buf();
    let contents = std::fs::read(home.join(".codex").join("auth.json")).ok()?;
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

fn send_request(stdin: &mut impl Write, id: u64, method: &str, params: Value) -> Result<()> {
    serde_json::to_writer(
        &mut *stdin,
        &json!({"id": id, "method": method, "params": params}),
    )?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;
    Ok(())
}

fn wait_for_response(
    receiver: &mpsc::Receiver<String>,
    id: u64,
    timeout: StdDuration,
) -> Result<Value> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let line = receiver
            .recv_timeout(remaining)
            .context("Codex app-server response timed out")?;
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
    Ok(RateLimits {
        primary: parse_window(limits.get("primary")),
        secondary: parse_window(limits.get("secondary")),
        sampled_at,
        account_name: None,
        plan_type: limits
            .get("planType")
            .and_then(Value::as_str)
            .map(str::to_owned),
        limit_name: limits
            .get("limitName")
            .and_then(Value::as_str)
            .map(str::to_owned),
        credits: parse_credits(limits.get("credits")),
        reset_credits: parse_reset_credits(response.pointer("/result/rateLimitResetCredits")),
        additional_limits: Default::default(),
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

fn spawn_codex(executable: &Path, args: &[&str]) -> Result<Child> {
    command_for_codex(executable, args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("launch {}", executable.display()))
}

fn command_for_codex(executable: &Path, args: &[&str]) -> Command {
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
        assert_eq!(parsed.effective_primary().used_percent, Some(14));
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
            account_name_from_id_token(&token(r#"{"name":"Ada Lovelace","email":"ada@example.com"}"#)),
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
    #[ignore = "requires an installed and authenticated Codex CLI"]
    fn reads_live_rate_limits() {
        let executable = first_available(None).expect("Codex CLI should be discoverable");
        let limits = CodexClient::new(executable)
            .read_rate_limits()
            .expect("Codex app-server should return rate limits");
        assert!(limits.primary.used_percent.is_some() || limits.primary.resets_at.is_some());
    }
}
