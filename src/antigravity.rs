//! Antigravity subscription quota using the official `agy` Windows session.

use std::{
    collections::HashMap,
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

const CREDENTIAL_TARGET: &str = "gemini:antigravity";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const API_BASE: &str = "https://cloudcode-pa.googleapis.com";
const CLI_NAMES: &[&str] = &["agy.exe", "agy.cmd", "agy.ps1", "agy.bat"];

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
        read_remote_limits(&self.agent, API_BASE, &session.access_token)
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
    expires_at.map_or(true, |expires| {
        expires <= Utc::now() + chrono::Duration::minutes(1)
    })
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
    provider_cli::candidates_from(explicit, known, CLI_NAMES)
}

fn is_desktop_app_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.eq_ignore_ascii_case("Antigravity.exe")
                || name.eq_ignore_ascii_case("Antigravity IDE.exe")
        })
}

fn desktop_app_in(directory: &Path) -> Option<PathBuf> {
    [
        directory.join("Antigravity.exe"),
        directory.join("Antigravity IDE.exe"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

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
        if let Some(parsed) = DateTime::parse_from_rfc3339(value)
            .ok()
            .map(|value| value.with_timezone(&Utc))
        {
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

    let target = CREDENTIAL_TARGET
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

#[cfg(not(windows))]
fn credential_blob() -> Result<Vec<u8>> {
    bail!("official Antigravity session reuse is available on Windows")
}

#[derive(Clone)]
struct QuotaObservation {
    id: String,
    label: String,
    remaining_fraction: Option<f64>,
    reset_time: Option<DateTime<Utc>>,
}

fn read_remote_limits(agent: &ureq::Agent, base: &str, access_token: &str) -> Result<RateLimits> {
    let metadata = json!({
        "metadata": {
            "ideType": "ANTIGRAVITY",
            "platform": "PLATFORM_UNSPECIFIED",
            "pluginType": "GEMINI"
        }
    });
    let code_assist = post_json(agent, base, "loadCodeAssist", access_token, &metadata)?;
    let project = project_id(&code_assist);
    let request = project.map_or_else(|| json!({}), |project| json!({ "project": project }));
    let available = post_json(agent, base, "fetchAvailableModels", access_token, &request)?;
    let model_labels = available
        .get("models")
        .and_then(Value::as_object)
        .map(|models| {
            models
                .iter()
                .map(|(id, model)| {
                    let label = model
                        .get("displayName")
                        .or_else(|| model.get("label"))
                        .and_then(Value::as_str)
                        .unwrap_or(id)
                        .to_owned();
                    (id.to_owned(), label)
                })
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();
    let available_observations = observations_from_models(&available);
    let verified = match post_json(agent, base, "retrieveUserQuota", access_token, &request) {
        Ok(value) => observations_from_buckets(&value, &model_labels),
        Err(_) => Vec::new(),
    };
    let observations = choose_observations(verified, available_observations)?;
    let gemini = select_family(&observations, Family::Gemini);
    let third_party = select_family(&observations, Family::ThirdParty);
    if gemini.is_none() && third_party.is_none() {
        bail!("Antigravity returned no recognized subscription quota pools")
    }
    Ok(RateLimits {
        primary: gemini.map(limit_window).unwrap_or_default(),
        secondary: third_party.map(limit_window).unwrap_or_default(),
        sampled_at: Utc::now(),
        plan_type: plan_name(&code_assist),
        limit_name: Some("Gemini".into()),
        secondary_limit_name: Some("Claude + GPT".into()),
        ..RateLimits::default()
    })
}

fn choose_observations(
    verified: Vec<QuotaObservation>,
    available: Vec<QuotaObservation>,
) -> Result<Vec<QuotaObservation>> {
    if verified
        .iter()
        .any(|item| item.remaining_fraction.is_some())
    {
        return Ok(verified);
    }
    if available.iter().any(|item| {
        item.remaining_fraction
            .is_some_and(|remaining| remaining < 0.999)
    }) {
        return Ok(available);
    }
    bail!("Antigravity limits are not available for this account")
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
        .set("User-Agent", "antigravity")
        .send_string(&body.to_string())
    {
        Ok(response) => response,
        Err(ureq::Error::Status(401, _)) => {
            bail!("Antigravity session is not authorized; run `agy` and sign in")
        }
        Err(ureq::Error::Status(403, _)) => {
            bail!("Antigravity limits are not available for this account")
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

fn observations_from_models(value: &Value) -> Vec<QuotaObservation> {
    value
        .get("models")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(id, model)| {
            let quota = model.get("quotaInfo")?;
            Some(QuotaObservation {
                id: id.to_owned(),
                label: model
                    .get("displayName")
                    .or_else(|| model.get("label"))
                    .and_then(Value::as_str)
                    .unwrap_or(id)
                    .to_owned(),
                remaining_fraction: quota.get("remainingFraction").and_then(Value::as_f64),
                reset_time: quota
                    .get("resetTime")
                    .and_then(Value::as_str)
                    .and_then(parse_timestamp),
            })
        })
        .collect()
}

fn observations_from_buckets(
    value: &Value,
    model_labels: &HashMap<String, String>,
) -> Vec<QuotaObservation> {
    value
        .get("buckets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|bucket| {
            let id = bucket.get("modelId")?.as_str()?.to_owned();
            Some(QuotaObservation {
                label: model_labels.get(&id).cloned().unwrap_or_else(|| id.clone()),
                id,
                remaining_fraction: bucket.get("remainingFraction").and_then(Value::as_f64),
                reset_time: bucket
                    .get("resetTime")
                    .and_then(Value::as_str)
                    .and_then(parse_timestamp),
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
                    identity.contains("claude")
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
        duration_minutes: None,
    }
}

fn plan_name(value: &Value) -> Option<String> {
    [
        value.pointer("/planInfo/planType"),
        value.pointer("/paidTier/name"),
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
    fn selects_the_most_constrained_quota_in_each_real_pool() {
        let observations = observations_from_models(&json!({
            "models": {
                "gemini-fast": {"displayName":"Gemini Flash", "quotaInfo":{"remainingFraction":0.8,"resetTime":"2026-09-14T00:00:00Z"}},
                "gemini-pro": {"displayName":"Gemini Pro", "quotaInfo":{"remainingFraction":0.4,"resetTime":"2026-09-15T00:00:00Z"}},
                "claude": {"displayName":"Claude Sonnet", "quotaInfo":{"remainingFraction":0.6,"resetTime":"2026-09-16T00:00:00Z"}}
            }
        }));
        let gemini = select_family(&observations, Family::Gemini).unwrap();
        let third_party = select_family(&observations, Family::ThirdParty).unwrap();
        assert_eq!(gemini.id, "gemini-pro");
        assert_eq!(limit_window(gemini).used_percent, Some(60));
        assert_eq!(third_party.id, "claude");
    }

    #[test]
    fn all_available_placeholders_are_not_reported_as_quota() {
        let observations = observations_from_models(&json!({
            "models": {
                "gemini-pro": {"quotaInfo":{"remainingFraction":1.0}}
            }
        }));
        assert!(choose_observations(Vec::new(), observations).is_err());
    }

    #[test]
    fn prefers_an_explicit_cli_folder_over_desktop_app_names() {
        let directory = tempfile::tempdir().unwrap();
        let cli = directory.path().join("agy.exe");
        let app = directory.path().join("Antigravity.exe");
        std::fs::write(&cli, b"cli").unwrap();
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
}
