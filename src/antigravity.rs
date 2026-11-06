//! Antigravity subscription quota using the official `agy` session: Windows
//! Credential Manager on Windows, the Secret Service login keyring on Linux.

use std::{
    env,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};

use crate::{
    limits::{LimitWindow, RateLimits},
    provider_cli,
    usage::UsageStatistics,
    worker::{Activator, LimitProvider, UsageProvider},
};

/// `agy` stores its session with go-keyring under this service and user:
/// the Windows credential target is `service:user`, and the Secret Service
/// item carries them as `service` and `username` attributes.
const CREDENTIAL_SERVICE: &str = "gemini";
const CREDENTIAL_USER: &str = "antigravity";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const API_BASE: &str = "https://cloudcode-pa.googleapis.com";
const QUOTA_API_BASE: &str = "https://daily-cloudcode-pa.googleapis.com";
// The generic client identity returns model availability, including synthetic
// full quotas. The Hub client family exposes Antigravity subscription usage.
const USER_AGENT: &str = "antigravity/hub/2.9.1 windows/amd64";
#[cfg(windows)]
const CLI_NAMES: &[&str] = &["agy.exe", "agy.cmd", "agy.ps1", "agy.bat"];
#[cfg(not(windows))]
const CLI_NAMES: &[&str] = &["agy"];

pub struct AntigravityClient {
    agent: ureq::Agent,
    cli_folder: Option<PathBuf>,
}

pub struct AntigravityActivator;

impl AntigravityClient {
    pub fn new(cli_folder: Option<PathBuf>) -> Self {
        Self {
            agent: ureq::AgentBuilder::new().timeout(REQUEST_TIMEOUT).build(),
            cli_folder,
        }
    }

    fn fresh_session(&self) -> Result<AntigravitySession> {
        recover_stale_session(parse_credential_blob(&credential_blob()?)?, || {
            provider_cli::refresh_session(
                "Antigravity",
                &cli_candidates(self.cli_folder.as_deref()),
                &["models"],
                REQUEST_TIMEOUT,
            )
            .map_err(|_| {
                anyhow::anyhow!("Antigravity session expired; run `agy` and sign in again")
            })?;
            parse_credential_blob(&credential_blob()?)
        })
    }
}

impl Default for AntigravityClient {
    fn default() -> Self {
        Self::new(None)
    }
}

impl LimitProvider for AntigravityClient {
    fn read_limits(&mut self) -> Result<RateLimits> {
        let session = self.fresh_session()?;
        read_remote_limits(&self.agent, API_BASE, QUOTA_API_BASE, &session.access_token)
    }
}

impl UsageProvider for AntigravityClient {
    fn load_cached_usage_statistics(&mut self, _history_days: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }

    fn refresh_usage_statistics(&mut self, _history_days: u16) -> Result<UsageStatistics> {
        // Provider quota is intentionally separate from local token/API billing.
        Ok(UsageStatistics::default())
    }
}

impl Activator for AntigravityActivator {
    fn activate(&mut self) -> Result<()> {
        bail!("Antigravity does not support session-window activation")
    }
}

pub fn is_installed(explicit: Option<&Path>) -> bool {
    signed_in() || cli_available(explicit).is_some() || desktop_app(explicit).is_some()
}

/// Resolves a standalone `agy` launcher. An explicit folder is searched first;
/// a file path remains supported so a copied binary still counts.
pub fn cli_available(explicit: Option<&Path>) -> Option<PathBuf> {
    cli_candidates(explicit).into_iter().next()
}

/// Resolves the Antigravity desktop app without launching it. An explicit
/// folder is searched first; a file path remains supported for upgrades.
pub fn desktop_app(explicit: Option<&Path>) -> Option<PathBuf> {
    explicit
        .and_then(|path| {
            if path.is_file() {
                is_desktop_app_name(path).then(|| path.to_path_buf())
            } else {
                desktop_app_in(path)
            }
        })
        .or_else(known_desktop_app)
}

struct AntigravitySession {
    access_token: String,
    expires_at: Option<DateTime<Utc>>,
}

fn recover_stale_session(
    initial: AntigravitySession,
    refresh: impl FnOnce() -> Result<AntigravitySession>,
) -> Result<AntigravitySession> {
    if !session_is_stale(initial.expires_at) {
        return Ok(initial);
    }
    let refreshed = refresh()?;
    if session_is_stale(refreshed.expires_at) {
        bail!("Antigravity session expired; run `agy` and sign in again")
    }
    Ok(refreshed)
}

fn session_is_stale(expires_at: Option<DateTime<Utc>>) -> bool {
    expires_at.is_none_or(|expires| expires <= Utc::now() + chrono::Duration::minutes(1))
}

fn signed_in() -> bool {
    credential_blob()
        .and_then(|blob| parse_credential_blob(&blob))
        .is_ok()
}

fn cli_candidates(explicit: Option<&Path>) -> Vec<PathBuf> {
    let explicit = explicit.filter(|path| !(path.is_file() && is_desktop_app_name(path)));
    let mut known = Vec::new();
    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        known.push(PathBuf::from(local_app_data).join("agy/bin/agy.exe"));
    }
    #[cfg(not(windows))]
    if let Some(home) = env::var_os("HOME").map(PathBuf::from) {
        known.push(home.join(".local/bin/agy"));
        known.push(home.join(".agy/bin/agy"));
    }
    provider_cli::candidates_from(explicit, known, CLI_NAMES)
}

fn is_desktop_app_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.eq_ignore_ascii_case("Antigravity.exe")
                || name.eq_ignore_ascii_case("Antigravity IDE.exe")
                || (cfg!(not(windows)) && name == "antigravity")
        })
}

fn desktop_app_in(directory: &Path) -> Option<PathBuf> {
    [
        directory.join("Antigravity.exe"),
        directory.join("Antigravity IDE.exe"),
        directory.join("antigravity"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

#[cfg(not(windows))]
fn known_desktop_app() -> Option<PathBuf> {
    let home = env::var_os("HOME").map(PathBuf::from);
    [
        Some(PathBuf::from("/usr/share/antigravity/antigravity")),
        Some(PathBuf::from("/opt/Antigravity/antigravity")),
        Some(PathBuf::from("/opt/antigravity/antigravity")),
        home.map(|home| home.join(".local/share/antigravity/antigravity")),
    ]
    .into_iter()
    .flatten()
    .find(|path| path.is_file())
}

#[cfg(windows)]
fn known_desktop_app() -> Option<PathBuf> {
    let local_app_data = env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
    let programs = local_app_data.join("Programs");
    [
        programs.join("antigravity/Antigravity.exe"),
        programs.join("Antigravity/Antigravity.exe"),
        programs.join("Antigravity IDE/Antigravity IDE.exe"),
        programs.join("Antigravity/Antigravity IDE.exe"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn parse_credential_blob(blob: &[u8]) -> Result<AntigravitySession> {
    let text = decode_credential_text(blob)?;
    let root: Value = serde_json::from_str(&text)
        .context("official Antigravity session is invalid; run `agy` and sign in again")?;
    let token = root
        .get("token")
        .filter(|value| value.is_object())
        .unwrap_or(&root);
    let access_token = token
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("official Antigravity session is missing; run `agy` and sign in")?
        .to_owned();
    let expires_at = token
        .get("expiry")
        .or_else(|| token.get("expiry_date"))
        .and_then(parse_expiry);
    Ok(AntigravitySession {
        access_token,
        expires_at,
    })
}

fn decode_credential_text(blob: &[u8]) -> Result<String> {
    let mut text = if blob.starts_with(&[0xff, 0xfe]) || (blob.len() >= 2 && blob[1] == 0) {
        let utf16 = blob
            .chunks_exact(2)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
            .collect::<Vec<_>>();
        String::from_utf16_lossy(&utf16)
            .trim_start_matches('\u{feff}')
            .trim_matches(char::from(0))
            .trim()
            .to_owned()
    } else {
        String::from_utf8_lossy(blob)
            .trim_start_matches('\u{feff}')
            .trim_matches(char::from(0))
            .trim()
            .to_owned()
    };
    if let Some(encoded) = text.strip_prefix("go-keyring-base64:") {
        let decoded = STANDARD
            .decode(encoded)
            .context("decode official Antigravity session")?;
        text = String::from_utf8(decoded).context("official Antigravity session is not UTF-8")?;
    }
    Ok(text)
}

fn parse_expiry(value: &Value) -> Option<DateTime<Utc>> {
    if let Some(value) = value.as_str() {
        if let Some(parsed) = parse_timestamp(value) {
            return Some(parsed);
        }
        if let Ok(numeric) = value.parse::<i64>() {
            return epoch_timestamp(numeric);
        }
    }
    value.as_i64().and_then(epoch_timestamp)
}

fn epoch_timestamp(value: i64) -> Option<DateTime<Utc>> {
    let seconds = if value > 1_000_000_000_000 {
        value / 1_000
    } else {
        value
    };
    DateTime::from_timestamp(seconds, 0)
}

#[cfg(windows)]
fn credential_blob() -> Result<Vec<u8>> {
    use std::ffi::c_void;
    use windows_sys::Win32::Security::Credentials::{
        CRED_TYPE_GENERIC, CREDENTIALW, CredFree, CredReadW,
    };

    let target = format!("{CREDENTIAL_SERVICE}:{CREDENTIAL_USER}")
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
    let found = unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) } != 0;
    if !found || credential.is_null() {
        bail!("official Antigravity session not found; run `agy` and sign in")
    }
    let value = unsafe { &*credential };
    let size = usize::try_from(value.CredentialBlobSize).unwrap_or_default();
    if size == 0 || value.CredentialBlob.is_null() {
        unsafe { CredFree(credential.cast::<c_void>()) };
        bail!("official Antigravity session is empty; run `agy` and sign in again")
    }
    let blob = unsafe { std::slice::from_raw_parts(value.CredentialBlob, size).to_vec() };
    unsafe { CredFree(credential.cast::<c_void>()) };
    Ok(blob)
}

/// The `agy` session from the login keyring, or from the token file `agy`
/// writes where no Secret Service runs.
#[cfg(not(windows))]
fn credential_blob() -> Result<Vec<u8>> {
    if let Some(blob) = futures::executor::block_on(keyring_blob()) {
        return Ok(blob);
    }
    if let Some(blob) = token_file_paths()
        .into_iter()
        .find_map(|path| std::fs::read(path).ok())
        .filter(|blob| !blob.iter().all(u8::is_ascii_whitespace))
    {
        return Ok(blob);
    }
    bail!("official Antigravity session not found; run `agy` and sign in")
}

#[cfg(not(windows))]
async fn keyring_blob() -> Option<Vec<u8>> {
    let keyring = oo7::Keyring::new().await.ok()?;
    // Searching needs no unlock, so people without an Antigravity session are
    // never asked to unlock their keyring for it.
    let items = keyring
        .search_items(&vec![
            ("service", CREDENTIAL_SERVICE),
            ("username", CREDENTIAL_USER),
        ])
        .await
        .ok()?;
    for item in items {
        if item.is_locked().await.ok()? && !may_prompt_once() {
            continue;
        }
        if item.unlock().await.is_err() {
            continue;
        }
        if let Ok(secret) = item.secret().await
            && !secret.is_empty()
        {
            return Some(secret.to_vec());
        }
    }
    None
}

/// A locked keyring prompts once, as it would for `agy` itself. This runs on
/// every refresh, so a dismissed prompt must not come back on the next one.
#[cfg(not(windows))]
fn may_prompt_once() -> bool {
    static PROMPTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    !PROMPTED.swap(true, std::sync::atomic::Ordering::Relaxed)
}

#[cfg(not(windows))]
fn token_file_paths() -> Vec<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| vec![home.join(".gemini/antigravity-cli/antigravity-oauth-token")])
        .unwrap_or_default()
}

#[derive(Clone)]
struct QuotaObservation {
    id: String,
    label: String,
    remaining_fraction: Option<f64>,
    reset_time: Option<DateTime<Utc>>,
    duration_minutes: Option<u32>,
}

fn read_remote_limits(
    agent: &ureq::Agent,
    base: &str,
    quota_base: &str,
    access_token: &str,
) -> Result<RateLimits> {
    let metadata = json!({
        "metadata": {
            "ideType": "ANTIGRAVITY",
            "platform": "PLATFORM_UNSPECIFIED",
            "pluginType": "GEMINI"
        }
    });
    let code_assist = post_json(agent, base, "loadCodeAssist", access_token, &metadata)?;
    let project = project_id(&code_assist);
    if project.is_none()
        && code_assist.get("currentTier").is_none_or(Value::is_null)
        && code_assist
            .get("ineligibleTiers")
            .and_then(Value::as_array)
            .is_some_and(|tiers| {
                tiers.iter().any(|tier| {
                    tier.get("reasonCode").and_then(Value::as_str)
                        == Some("GOOGLE_TOS_NOT_SUPPORTED_BY_CLIENT")
                })
            })
    {
        bail!("Antigravity session cannot read subscription quota; run `agy` and sign in again")
    }
    let request = project.map_or_else(|| json!({}), |project| json!({ "project": project }));
    // Both legacy model endpoints can agree on placeholder 100%-remaining
    // values, even when the account has consumed its weekly allowance.
    let summary = post_json(
        agent,
        quota_base,
        "retrieveUserQuotaSummary",
        access_token,
        &request,
    )?;
    limits_from_summary(&summary, plan_name(&code_assist))
}

fn limits_from_summary(summary: &Value, plan_type: Option<String>) -> Result<RateLimits> {
    let observations = observations_from_summary(summary);
    // Keep the existing family metrics (including saved tray selections), but
    // let either the session or weekly allowance drive each family's usage.
    let gemini = select_family(&observations, Family::Gemini);
    let third_party = select_family(&observations, Family::ThirdParty);
    if gemini.is_none() && third_party.is_none() {
        bail!("Antigravity returned no recognized subscription quota pools")
    }
    Ok(RateLimits {
        primary: gemini.map(limit_window).unwrap_or_default(),
        secondary: third_party.map(limit_window).unwrap_or_default(),
        sampled_at: Utc::now(),
        plan_type,
        limit_name: Some("Gemini".into()),
        secondary_limit_name: Some("Claude + GPT".into()),
        ..RateLimits::default()
    })
}

fn project_id(value: &Value) -> Option<&str> {
    let project = value.get("cloudaicompanionProject")?;
    project
        .as_str()
        .or_else(|| project.get("id").and_then(Value::as_str))
        .or_else(|| project.get("projectId").and_then(Value::as_str))
        .filter(|value| !value.trim().is_empty())
}

fn post_json(
    agent: &ureq::Agent,
    base: &str,
    operation: &str,
    access_token: &str,
    body: &Value,
) -> Result<Value> {
    let url = format!("{base}/v1internal:{operation}");
    let response = match agent
        .post(&url)
        .set("Authorization", &format!("Bearer {access_token}"))
        .set("Content-Type", "application/json")
        .set("Accept", "application/json")
        .set("User-Agent", USER_AGENT)
        .send_string(&body.to_string())
    {
        Ok(response) => response,
        Err(ureq::Error::Status(401, _)) => {
            bail!("Antigravity session is not authorized; run `agy` and sign in")
        }
        Err(ureq::Error::Status(403, _)) => {
            bail!("Antigravity limits are not available for this account")
        }
        Err(ureq::Error::Status(429, _)) => {
            return Err(crate::worker::rate_limit_error(
                "Antigravity quota request was rate limited (HTTP 429).",
            ));
        }
        Err(ureq::Error::Status(status, _)) => {
            bail!("Antigravity quota request failed with HTTP {status}")
        }
        Err(error) => return Err(error).context("request Antigravity subscription quota"),
    };
    let body = response
        .into_string()
        .context("read Antigravity subscription quota response")?;
    serde_json::from_str(&body).context("parse Antigravity subscription quota response")
}

fn observations_from_summary(value: &Value) -> Vec<QuotaObservation> {
    let summary = value.get("response").unwrap_or(value);
    summary
        .get("groups")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|group| {
            group
                .get("buckets")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|bucket| bucket.get("disabled").and_then(Value::as_bool) != Some(true))
                .map(move |bucket| {
                    let id = bucket
                        .get("bucketId")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let cadence = bucket.get("window").and_then(Value::as_str);
                    QuotaObservation {
                        id: id.to_owned(),
                        label: group
                            .get("displayName")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                        remaining_fraction: bucket
                            .get("remainingFraction")
                            .or_else(|| bucket.pointer("/remaining/remainingFraction"))
                            .and_then(Value::as_f64)
                            .filter(|fraction| {
                                fraction.is_finite() && (0.0..=1.0).contains(fraction)
                            }),
                        reset_time: bucket.get("resetTime").and_then(parse_expiry),
                        duration_minutes: match cadence {
                            Some("5h") => Some(300),
                            Some("weekly") => Some(10_080),
                            None if id.ends_with("-5h") => Some(300),
                            None if id.ends_with("-weekly") => Some(10_080),
                            _ => None,
                        },
                    }
                })
        })
        .collect()
}

#[derive(Clone, Copy)]
enum Family {
    Gemini,
    ThirdParty,
}

fn select_family(items: &[QuotaObservation], family: Family) -> Option<QuotaObservation> {
    items
        .iter()
        .filter(|item| {
            let identity = format!("{} {}", item.id, item.label).to_ascii_lowercase();
            match family {
                Family::Gemini => identity.contains("gemini"),
                Family::ThirdParty => {
                    item.id.starts_with("3p-")
                        || identity.contains("claude")
                        || identity.contains("gpt")
                        || identity.contains("oss")
                }
            }
        })
        .filter(|item| item.remaining_fraction.is_some())
        .min_by(|left, right| {
            left.remaining_fraction
                .partial_cmp(&right.remaining_fraction)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    left.duration_minutes
                        .unwrap_or(u32::MAX)
                        .cmp(&right.duration_minutes.unwrap_or(u32::MAX))
                })
        })
        .cloned()
}

fn limit_window(item: QuotaObservation) -> LimitWindow {
    LimitWindow {
        used_percent: item
            .remaining_fraction
            .filter(|value| value.is_finite())
            .map(|remaining| ((1.0 - remaining.clamp(0.0, 1.0)) * 100.0).round() as u8),
        resets_at: item.reset_time,
        duration_minutes: item.duration_minutes,
    }
}

fn plan_name(value: &Value) -> Option<String> {
    [
        value.pointer("/paidTier/name"),
        value.pointer("/planInfo/planType"),
        value.pointer("/currentTier/name"),
    ]
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
    .find(|value| !value.trim().is_empty())
    .map(str::to_owned)
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    type RecordedRequests = Vec<(String, Value)>;

    #[test]
    fn parses_go_keyring_credential_wrapper() {
        let raw = r#"{"token":{"access_token":"secret","expiry":"2099-01-01T00:00:00Z"}}"#;
        let wrapped = format!("go-keyring-base64:{}", STANDARD.encode(raw));
        let parsed = parse_credential_blob(wrapped.as_bytes()).unwrap();
        assert_eq!(parsed.access_token, "secret");
        assert!(parsed.expires_at.is_some());
    }

    #[test]
    fn expired_session_uses_the_owner_cli_result_once() {
        let initial = test_session("2000-01-01T00:00:00Z");
        let mut calls = 0;
        let refreshed = recover_stale_session(initial, || {
            calls += 1;
            Ok(test_session("2099-01-01T00:00:00Z"))
        })
        .unwrap();
        assert_eq!(calls, 1);
        assert!(!session_is_stale(refreshed.expires_at));
    }

    #[test]
    fn fresh_session_does_not_start_the_owner_cli() {
        let initial = test_session("2099-01-01T00:00:00Z");
        let mut calls = 0;
        recover_stale_session(initial, || {
            calls += 1;
            Ok(test_session("2099-01-01T00:00:00Z"))
        })
        .unwrap();
        assert_eq!(calls, 0);
    }

    #[test]
    fn still_expired_owner_result_keeps_the_manual_login_message() {
        let error = recover_stale_session(test_session("2000-01-01T00:00:00Z"), || {
            Ok(test_session("2000-01-01T00:00:00Z"))
        })
        .err()
        .expect("stale refreshed session should fail");
        assert_eq!(
            error.to_string(),
            "Antigravity session expired; run `agy` and sign in again"
        );
    }

    fn test_session(expiry: &str) -> AntigravitySession {
        AntigravitySession {
            access_token: "secret".into(),
            expires_at: parse_timestamp(expiry),
        }
    }

    #[test]
    fn parses_utf16_windows_credential_and_wire_project_shapes() {
        let raw = r#"{"access_token":"secret"}"#;
        let utf16 = raw
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(
            parse_credential_blob(&utf16).unwrap().access_token,
            "secret"
        );
        assert_eq!(
            project_id(&json!({"cloudaicompanionProject":{"id":"project-a"}})),
            Some("project-a")
        );
        assert_eq!(
            project_id(&json!({"cloudaicompanionProject":"project-b"})),
            Some("project-b")
        );
    }

    #[test]
    fn weekly_consumption_is_not_hidden_by_an_unused_session() {
        let limits = limits_from_summary(&quota_summary(), None).unwrap();
        assert_eq!(limits.primary.used_percent, Some(3));
        assert_eq!(limits.primary.duration_minutes, Some(10_080));
        assert_eq!(
            limits.primary.resets_at,
            parse_timestamp("2026-10-27T14:58:17Z")
        );
        assert_eq!(limits.secondary.used_percent, Some(60));
        assert_eq!(limits.secondary.duration_minutes, Some(300));
        assert_eq!(
            limits.secondary.resets_at,
            parse_timestamp("2026-10-21T06:00:00Z")
        );
    }

    #[test]
    fn disabled_session_buckets_do_not_override_enabled_weekly_allowances() {
        let mut summary = quota_summary();
        for group in summary["groups"].as_array_mut().unwrap() {
            group["buckets"][0]["disabled"] = json!(false);
            group["buckets"][1]["disabled"] = json!(true);
            group["buckets"][1]["remainingFraction"] = json!(0.5);
        }
        let limits = limits_from_summary(&summary, None).unwrap();
        assert_eq!(limits.primary.used_percent, Some(3));
        assert_eq!(limits.primary.duration_minutes, Some(10_080));
        assert_eq!(
            limits.primary.resets_at,
            parse_timestamp("2026-10-27T14:58:17Z")
        );
        assert_eq!(limits.secondary.used_percent, Some(20));
        assert_eq!(limits.secondary.duration_minutes, Some(10_080));
        assert_eq!(
            limits.secondary.resets_at,
            parse_timestamp("2026-10-28T01:59:37Z")
        );
    }

    #[test]
    fn disabled_only_families_remain_unavailable() {
        for disabled_group in [0, 1] {
            let mut summary = quota_summary();
            for bucket in summary["groups"][disabled_group]["buckets"]
                .as_array_mut()
                .unwrap()
            {
                bucket["disabled"] = json!(true);
            }
            let limits = limits_from_summary(&summary, None).unwrap();
            assert_eq!(limits.primary.is_empty(), disabled_group == 0);
            assert_eq!(limits.secondary.is_empty(), disabled_group == 1);

            for group in summary["groups"].as_array_mut().unwrap() {
                for bucket in group["buckets"].as_array_mut().unwrap() {
                    bucket["disabled"] = json!(true);
                }
            }
            assert!(limits_from_summary(&summary, None).is_err());
        }
    }

    #[test]
    fn legacy_model_availability_is_not_reported_as_subscription_usage() {
        for legacy in [
            json!({
                "models": {
                    "gemini-pro": {"quotaInfo":{"remainingFraction":1.0}}
                }
            }),
            json!({
                "buckets": [{"modelId":"gemini-pro","remainingFraction":1.0}]
            }),
        ] {
            assert!(limits_from_summary(&legacy, None).is_err());
        }
    }

    #[test]
    fn real_unused_and_exhausted_summary_buckets_are_valid() {
        let mut summary = quota_summary();
        for remaining in [0.0, 1.0] {
            for group in summary["groups"].as_array_mut().unwrap() {
                for bucket in group["buckets"].as_array_mut().unwrap() {
                    bucket["remainingFraction"] = json!(remaining);
                }
            }
            let limits = limits_from_summary(&summary, None).unwrap();
            let expected = if remaining == 0.0 { Some(100) } else { Some(0) };
            assert_eq!(limits.primary.used_percent, expected);
            assert_eq!(limits.secondary.used_percent, expected);
        }
    }

    #[test]
    fn family_and_equal_usage_selection_do_not_depend_on_response_order() {
        let mut summary = quota_summary();
        for group in summary["groups"].as_array_mut().unwrap() {
            for bucket in group["buckets"].as_array_mut().unwrap() {
                bucket["remainingFraction"] = json!(0.5);
            }
        }
        for reverse in [false, true] {
            if reverse {
                let groups = summary["groups"].as_array_mut().unwrap();
                groups.reverse();
                for group in groups {
                    group["buckets"].as_array_mut().unwrap().reverse();
                }
            }
            let limits = limits_from_summary(&summary, None).unwrap();
            assert_eq!(limits.primary.duration_minutes, Some(300));
            assert_eq!(limits.secondary.duration_minutes, Some(300));
            assert_eq!(
                limits.primary.resets_at,
                parse_timestamp("2026-10-21T06:59:37Z")
            );
            assert_eq!(
                limits.secondary.resets_at,
                parse_timestamp("2026-10-21T06:00:00Z")
            );
        }
    }

    #[test]
    fn missing_or_invalid_fractions_do_not_become_zero_usage() {
        for fraction in [Value::Null, json!("1"), json!(-0.1), json!(1.1)] {
            let summary = json!({"groups":[{
                "displayName":"Gemini Models",
                "buckets":[{"bucketId":"gemini-weekly","remainingFraction":fraction}]
            }]});
            assert!(limits_from_summary(&summary, None).is_err());
        }
        let summary = json!({"groups":[{
            "displayName":"Gemini Models",
            "buckets":[{"bucketId":"gemini-weekly","remainingFraction":0.7}]
        }]});
        let limits = limits_from_summary(&summary, None).unwrap();
        assert_eq!(limits.primary.used_percent, Some(30));
        assert!(limits.secondary.is_empty());
    }

    #[test]
    fn parses_nested_summary_and_does_not_borrow_another_familys_quota() {
        let summary = json!({"response":{"groups":[{
            "displayName":"Claude and GPT models",
            "buckets":[{
                "bucketId":"3p-weekly",
                "remaining":{"remainingFraction":0.25},
                "resetTime":1_800_000_000
            }]
        }]}});
        let limits = limits_from_summary(&summary, None).unwrap();
        assert!(limits.primary.is_empty());
        assert_eq!(limits.secondary.used_percent, Some(75));
        assert_eq!(limits.secondary.duration_minutes, Some(10_080));
        assert_eq!(
            limits.secondary.resets_at,
            DateTime::from_timestamp(1_800_000_000, 0)
        );
    }

    fn quota_summary() -> Value {
        // Sanitized wire shape observed from retrieveUserQuotaSummary. Legacy
        // retrieveUserQuota returned 1.0 for every model for the same account.
        json!({"groups":[
            {"displayName":"Gemini Models","buckets":[
                {"bucketId":"gemini-weekly","window":"weekly","remainingFraction":0.97032213,"resetTime":"2026-10-27T14:58:17Z"},
                {"bucketId":"gemini-5h","window":"5h","remainingFraction":1.0,"resetTime":"2026-10-21T06:59:37Z"}
            ]},
            {"displayName":"Claude and GPT models","buckets":[
                {"bucketId":"3p-weekly","window":"weekly","remainingFraction":0.8,"resetTime":"2026-10-28T01:59:37Z"},
                {"bucketId":"3p-5h","window":"5h","remainingFraction":0.4,"resetTime":"2026-10-21T06:00:00Z"}
            ]}
        ]})
    }

    #[test]
    fn remote_requests_use_hub_identity_project_and_subscription_summary() {
        let assist = json!({
            "cloudaicompanionProject":{"id":"test-project"},
            "currentTier":{"name":"Antigravity"},
            "planInfo":{"planType":"Free"},
            "paidTier":{"name":"Google AI Pro"}
        });
        let (base, peer) = mock_api(vec![(200, assist), (200, quota_summary())]);
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(5))
            .build();
        let limits = read_remote_limits(&agent, &base, &base, "test-token").unwrap();
        let requests = peer.join().unwrap();
        assert_eq!(requests.len(), 2);
        for (headers, _) in &requests {
            let headers = headers.to_ascii_lowercase();
            assert!(headers.contains("user-agent: antigravity/hub/2.9.1 windows/amd64"));
            assert!(headers.contains("authorization: bearer test-token"));
        }
        assert!(
            requests[0]
                .0
                .starts_with("POST /v1internal:loadCodeAssist ")
        );
        assert_eq!(requests[0].1["metadata"]["ideType"], "ANTIGRAVITY");
        assert!(
            requests[1]
                .0
                .starts_with("POST /v1internal:retrieveUserQuotaSummary ")
        );
        assert_eq!(requests[1].1, json!({"project":"test-project"}));
        assert_eq!(limits.plan_type.as_deref(), Some("Google AI Pro"));
        assert_eq!(limits.primary.used_percent, Some(3));
        assert_eq!(limits.limit_name.as_deref(), Some("Gemini"));
        assert_eq!(limits.secondary_limit_name.as_deref(), Some("Claude + GPT"));
    }

    #[test]
    fn summary_errors_propagate_without_falling_back_to_availability() {
        for status in [401, 403, 429, 500] {
            let (base, peer) = mock_api(vec![
                (200, json!({"cloudaicompanionProject":"test-project"})),
                (status, json!({})),
            ]);
            let agent = ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(5))
                .build();
            let error = read_remote_limits(&agent, &base, &base, "test-token").unwrap_err();
            assert_eq!(crate::worker::is_rate_limited_error(&error), status == 429);
            assert_eq!(peer.join().unwrap().len(), 2);
        }
    }

    #[test]
    fn incompatible_oauth_client_does_not_accept_placeholder_summary() {
        let (base, peer) = mock_api(vec![(
            200,
            json!({
                "ineligibleTiers":[{"reasonCode":"GOOGLE_TOS_NOT_SUPPORTED_BY_CLIENT"}]
            }),
        )]);
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(5))
            .build();
        let error = read_remote_limits(&agent, &base, &base, "test-token").unwrap_err();
        assert!(error.to_string().contains("run `agy` and sign in again"));
        assert_eq!(peer.join().unwrap().len(), 1);
    }

    fn mock_api(
        responses: Vec<(u16, Value)>,
    ) -> (String, std::thread::JoinHandle<RecordedRequests>) {
        use std::io::{BufRead, BufReader, Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let peer = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for (status, response) in responses {
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                std::time::Instant::now() < deadline,
                                "quota request timed out"
                            );
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("accept quota request: {error}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut headers = String::new();
                loop {
                    let mut line = String::new();
                    assert!(reader.read_line(&mut line).unwrap() > 0);
                    if line == "\r\n" {
                        break;
                    }
                    headers.push_str(&line);
                }
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .map(|length| length.parse::<usize>().unwrap())
                    })
                    .unwrap();
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                requests.push((headers, serde_json::from_slice(&body).unwrap()));
                let response = response.to_string();
                write!(stream, "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len()).unwrap();
            }
            requests
        });
        (base, peer)
    }

    #[test]
    fn prefers_an_explicit_cli_folder_over_desktop_app_names() {
        let directory = tempfile::tempdir().unwrap();
        let cli = directory.path().join(CLI_NAMES[0]);
        let app = directory.path().join("Antigravity.exe");
        std::fs::write(&cli, b"cli").unwrap();
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::fs::write(&app, b"app").unwrap();

        assert_eq!(
            cli_available(Some(directory.path())),
            Some(std::fs::canonicalize(&cli).unwrap())
        );
        assert_eq!(
            desktop_app(Some(directory.path())).as_deref(),
            Some(app.as_path())
        );
    }

    #[test]
    fn summary_unknown_cadence_keeps_quota_and_offset_without_inventing_duration() {
        let summary = json!({"groups":[null, {"displayName":"Gemini Models","buckets":[
            {"bucketId":"gemini-future","window":"future-cadence",
             "remainingFraction":0.25,"resetTime":"2026-11-01T01:30:00-04:00"},
            {"bucketId":"gemini-weekly","remainingFraction":null}
        ]}]});
        let limits = limits_from_summary(&summary, None).unwrap();
        assert_eq!(limits.primary.used_percent, Some(75));
        assert_eq!(limits.primary.duration_minutes, None);
        assert_eq!(
            limits.primary.resets_at,
            Some("2026-11-01T05:30:00Z".parse().unwrap())
        );
        assert!(limits.secondary.is_empty());
        for summary in [
            json!({}),
            json!({"groups":null}),
            json!({"groups":[{"buckets":null}]}),
        ] {
            assert!(limits_from_summary(&summary, None).is_err());
        }
    }
}
