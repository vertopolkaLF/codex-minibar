//! Reads live Kiro subscription credits, preferring the IDE, then Kiro Crew,
//! and using the CLI installation when neither desktop app is available.

#[cfg(windows)]
use std::env;
use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime},
};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Months, Utc};
use rusqlite::{Connection, OpenFlags, params};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    limits::{LimitWindow, RateLimits, UsageAmount},
    usage::UsageStatistics,
    worker::{Activator, LimitProvider, UsageProvider},
};

const STATE_KEY: &str = "kiro.kiroAgent";
const USAGE_STATE_KEY: &str = "kiro.resourceNotifications.usageState";
const USAGE_COMMAND: &str = "GetUsageLimitsCommand";

#[derive(Clone, Default)]
pub struct KiroClient {
    app_path: Option<PathBuf>,
    crew_path: Option<PathBuf>,
    cli_path: Option<PathBuf>,
    state_path: Option<PathBuf>,
    cli_identity_cache: Option<(Instant, Option<String>)>,
}

impl KiroClient {
    pub fn new() -> Self {
        Self::with_paths(None, None, None)
    }

    pub fn with_paths(
        app_path: Option<&Path>,
        crew_path: Option<&Path>,
        cli_explicit_path: Option<&Path>,
    ) -> Self {
        let app_path = installation_path(app_path);
        let crew_path = crew_installation_path(crew_path);
        let cli_path = cli_path(cli_explicit_path);
        let state_path = state_database_path();
        Self {
            app_path,
            crew_path,
            cli_path,
            state_path,
            cli_identity_cache: None,
        }
    }

    fn read_usage_limits(&mut self) -> Result<RateLimits> {
        if self.app_path.is_none()
            && self.crew_path.is_none()
            && self.cli_path.is_none()
            && !self.state_path.as_ref().is_some_and(|path| path.is_file())
        {
            return Err(anyhow!("Kiro IDE, Kiro Crew, and Kiro CLI were not found"));
        }

        match read_live_usage_limits() {
            Ok(limits) => Ok(self.with_cli_identity(limits)),
            Err(error) if crate::worker::is_rate_limited_error(&error) => Err(error),
            Err(live_error)
                if self.app_path.is_some()
                    || self.state_path.as_ref().is_some_and(|path| path.is_file()) =>
            {
                self.read_cached_usage_limits()
                    .map(|limits| self.with_cli_identity(limits))
                    .map_err(|cache_error| {
                    anyhow!(
                        "Kiro's live quota request failed ({live_error:#}) and its local cache could not be read ({cache_error:#})"
                    )
                    })
            }
            Err(error) => Err(error),
        }
    }

    fn with_cli_identity(&mut self, mut limits: RateLimits) -> RateLimits {
        if limits.account_name.is_none() {
            let now = Instant::now();
            let cached = self
                .cli_identity_cache
                .as_ref()
                .and_then(|(sampled_at, name)| {
                    let ttl = if name.is_some() {
                        Duration::from_secs(600)
                    } else {
                        Duration::from_secs(60)
                    };
                    (now.duration_since(*sampled_at) < ttl).then(|| name.clone())
                });
            limits.account_name = cached.unwrap_or_else(|| {
                let name = self.cli_path.as_deref().and_then(cli_identity_account_name);
                self.cli_identity_cache = Some((Instant::now(), name.clone()));
                name
            });
        }
        limits
    }

    fn read_cached_usage_limits(&self) -> Result<RateLimits> {
        let path = self
            .state_path
            .as_deref()
            .ok_or_else(|| anyhow!("Could not locate the Kiro IDE user-data directory"))?;
        if !path.is_file() {
            return Err(anyhow!(
                "Kiro IDE usage data was not found. Sign in to Kiro and open it once to load your credits."
            ));
        }

        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .context("Could not open Kiro IDE's local usage cache read-only")?;
        connection
            .busy_timeout(Duration::from_millis(750))
            .context("Could not wait for Kiro IDE's usage cache")?;
        let raw: String = connection
            .query_row(
                "SELECT value FROM ItemTable WHERE key = ?1",
                params![STATE_KEY],
                |row| row.get(0),
            )
            .context("Kiro IDE has not saved its local usage state yet")?;
        let state: Value = serde_json::from_str(&raw)
            .context("Kiro IDE's local usage state has an unsupported format")?;
        let usage_state = state
            .get(USAGE_STATE_KEY)
            .and_then(Value::as_object)
            .ok_or_else(|| {
                anyhow!(
                    "Kiro IDE has not cached subscription credits yet. Sign in and open Kiro to refresh its usage panel."
                )
            })?;
        let sampled_at = usage_state
            .get("timestamp")
            .and_then(Value::as_i64)
            .and_then(DateTime::<Utc>::from_timestamp_millis)
            .ok_or_else(|| anyhow!("Kiro IDE's usage cache has no valid sample time"))?;
        let breakdowns = usage_state
            .get("usageBreakdowns")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("Kiro IDE's usage cache has no credit breakdowns"))?;
        let credits = breakdowns
            .iter()
            .find(|breakdown| is_credit_breakdown(breakdown))
            .ok_or_else(|| anyhow!("Kiro IDE's usage cache has no monthly credit entry"))?;

        let used = numeric_field(credits, "currentUsage")
            .filter(|value| value.is_finite() && *value >= 0.0)
            .ok_or_else(|| anyhow!("Kiro IDE's monthly credit usage is unavailable"))?;
        let limit = numeric_field(credits, "usageLimit")
            .filter(|value| value.is_finite() && *value > 0.0)
            .ok_or_else(|| anyhow!("Kiro IDE's monthly credit limit is unavailable"))?;
        let used_percent = ((used / limit) * 100.0).round().clamp(0.0, 100.0) as u8;
        let resets_at = credits
            .get("resetDate")
            .and_then(Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc));
        let duration_minutes = resets_at.and_then(month_duration_minutes);
        let metadata = local_runtime_metadata();

        Ok(RateLimits {
            sampled_at,
            account_name: metadata.account_name,
            plan_type: metadata.plan_name,
            secondary: LimitWindow {
                used_percent: Some(used_percent),
                resets_at,
                duration_minutes,
            },
            secondary_usage_amount: Some(UsageAmount { used, limit }),
            ..RateLimits::default()
        })
    }
}

/// Kiro CLI can still identify its account after the IDE's older shared token
/// expires. Only invoke `whoami` when the CLI's own cached token is unexpired;
/// otherwise some CLI versions launch browser sign-in automatically.
fn cli_identity_account_name(executable: &Path) -> Option<String> {
    if !cli_auth_cache_is_fresh() {
        return None;
    }

    let mut command = Command::new(executable);
    command
        .args(["whoami", "--format", "json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    crate::provider_cli::disable_auto_update(&mut command);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }

    let mut child = command.spawn().ok()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let output = child.wait_with_output().ok()?;
                if !output.status.success() {
                    return None;
                }
                let response: Value = serde_json::from_slice(&output.stdout).ok()?;
                let user = response
                    .get("userInfo")
                    .or_else(|| response.get("user_info"))
                    .unwrap_or(&response);
                let user: KiroUserInfo = serde_json::from_value(user.clone()).ok()?;
                return account_display_name(&user);
            }
            Ok(None) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(50));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

fn cli_auth_cache_is_fresh() -> bool {
    #[cfg(windows)]
    {
        let Some(path) = directories::BaseDirs::new()
            .map(|base| base.data_local_dir().join("Kiro-Cli/data.sqlite3"))
            .filter(|path| path.is_file())
        else {
            return false;
        };
        let Ok(connection) = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        else {
            return false;
        };
        if connection.busy_timeout(Duration::from_millis(250)).is_err() {
            return false;
        }
        let Ok(mut statement) =
            connection.prepare("SELECT value FROM auth_kv WHERE key LIKE 'kirocli:%:token'")
        else {
            return false;
        };
        let Ok(rows) = statement.query_map([], |row| row.get::<_, String>(0)) else {
            return false;
        };
        return rows.flatten().any(|raw| {
            serde_json::from_str::<Value>(&raw)
                .ok()
                .is_some_and(|token| {
                    let has_access_token = token
                        .get("access_token")
                        .and_then(Value::as_str)
                        .is_some_and(|value| !value.trim().is_empty());
                    let not_expired = token
                        .get("expires_at")
                        .and_then(timestamp_value)
                        .is_some_and(|expires_at| {
                            expires_at > Utc::now() + chrono::Duration::seconds(30)
                        });
                    has_access_token && not_expired
                })
        });
    }
    #[cfg(not(windows))]
    {
        false
    }
}

impl LimitProvider for KiroClient {
    fn read_limits(&mut self) -> Result<RateLimits> {
        self.read_usage_limits()
    }
}

impl UsageProvider for KiroClient {
    fn load_cached_usage_statistics(&mut self, _history_days: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }

    fn refresh_usage_statistics(&mut self, _history_days: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }
}

pub struct KiroActivator;

impl Activator for KiroActivator {
    fn activate(&mut self) -> Result<()> {
        Err(anyhow!(
            "Kiro does not expose a supported session activation command"
        ))
    }
}

/// Used for Settings and onboarding readiness without making a network call.
pub fn app_source_is_ready(explicit: Option<&Path>) -> bool {
    let app_found = installation_path(explicit).is_some();
    app_found
        && (read_kiro_auth_token().is_ok()
            || KiroClient::with_paths(explicit, None, None)
                .read_cached_usage_limits()
                .is_ok())
}

pub fn crew_source_is_ready(explicit: Option<&Path>) -> bool {
    crew_installation_path(explicit).is_some() && read_kiro_auth_token().is_ok()
}

pub fn cli_source_is_ready(explicit: Option<&Path>) -> bool {
    cli_path(explicit).is_some() && read_kiro_auth_token().is_ok()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KiroSource {
    Ide,
    Crew,
    Cli,
}

pub fn selected_source(
    app_path: Option<&Path>,
    crew_path: Option<&Path>,
    cli_path: Option<&Path>,
) -> Option<KiroSource> {
    if app_source_is_ready(app_path) {
        Some(KiroSource::Ide)
    } else if crew_source_is_ready(crew_path) {
        Some(KiroSource::Crew)
    } else if cli_source_is_ready(cli_path) {
        Some(KiroSource::Cli)
    } else {
        None
    }
}

pub fn source_is_ready(
    app_path: Option<&Path>,
    crew_path: Option<&Path>,
    cli_path: Option<&Path>,
) -> bool {
    selected_source(app_path, crew_path, cli_path).is_some()
}

pub fn ide_source_path(explicit: Option<&Path>) -> Option<PathBuf> {
    installation_path(explicit).or_else(|| state_database_path().filter(|path| path.is_file()))
}

/// Finds the desktop Kiro Crew app across per-user and all-users installs.
pub fn crew_installation_path(explicit: Option<&Path>) -> Option<PathBuf> {
    explicit
        .and_then(|path| executable_from_crew_path(path, true))
        .or_else(|| {
            #[cfg(windows)]
            {
                let local_app_data = env::var_os("LOCALAPPDATA").map(PathBuf::from);
                let program_files = env::var_os("ProgramFiles").map(PathBuf::from);
                let program_files_x86 = env::var_os("ProgramFiles(x86)").map(PathBuf::from);
                let mut roots = Vec::new();
                if let Some(root) = local_app_data {
                    roots.push(root.join("Programs"));
                }
                roots.extend(program_files);
                roots.extend(program_files_x86);
                roots
                    .into_iter()
                    .flat_map(|root| {
                        [
                            "KiroCrew",
                            "Kiro Crew",
                            "KiroCrew Nightly",
                            "Kiro Crew Nightly",
                        ]
                        .into_iter()
                        .map(move |folder| root.join(folder))
                    })
                    .chain(registered_crew_install_locations())
                    .find_map(|path| executable_from_crew_path(&path, false))
            }
            #[cfg(not(windows))]
            {
                None
            }
        })
}

fn executable_from_crew_path(path: &Path, allow_explicit_file: bool) -> Option<PathBuf> {
    if path.is_file() {
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect::<String>()
            .to_ascii_lowercase();
        if allow_explicit_file || stem.starts_with("kirocrew") {
            return Some(path.to_path_buf());
        }
    }
    [
        "KiroCrew.exe",
        "Kiro Crew.exe",
        "KiroCrew Nightly.exe",
        "Kiro Crew Nightly.exe",
    ]
    .into_iter()
    .map(|name| path.join(name))
    .find(|candidate| candidate.is_file())
}

#[cfg(windows)]
fn registered_crew_install_locations() -> Vec<PathBuf> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_SZ, RegCloseKey,
            RegEnumKeyExW, RegGetValueW, RegOpenKeyExW,
        },
    };

    const UNINSTALL_KEYS: [(HKEY, &str); 3] = [
        (
            HKEY_CURRENT_USER,
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
    ];
    let wide = |value: &str| {
        value
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>()
    };
    let read_value = |key: HKEY, name: &str| -> Option<String> {
        let value_name = wide(name);
        let mut value = [0_u16; 2048];
        let mut size = std::mem::size_of_val(&value) as u32;
        let status = unsafe {
            RegGetValueW(
                key,
                std::ptr::null(),
                value_name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                value.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let len = value
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(value.len());
        Some(String::from_utf16_lossy(&value[..len]).trim().to_owned())
    };

    let mut locations = Vec::new();
    for (hive, key_path) in UNINSTALL_KEYS {
        let mut root: HKEY = std::ptr::null_mut();
        if unsafe { RegOpenKeyExW(hive, wide(key_path).as_ptr(), 0, KEY_READ, &mut root) }
            != ERROR_SUCCESS
        {
            continue;
        }
        let mut index = 0;
        loop {
            let mut name = [0_u16; 512];
            let mut name_len = name.len() as u32;
            if unsafe {
                RegEnumKeyExW(
                    root,
                    index,
                    name.as_mut_ptr(),
                    &mut name_len,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            } != ERROR_SUCCESS
            {
                break;
            }
            index += 1;
            let subkey_name = OsString::from_wide(&name[..name_len as usize]);
            let mut install_key: HKEY = std::ptr::null_mut();
            let subkey = name[..name_len as usize]
                .iter()
                .copied()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            if unsafe { RegOpenKeyExW(root, subkey.as_ptr(), 0, KEY_READ, &mut install_key) }
                != ERROR_SUCCESS
            {
                continue;
            }
            let product_name = read_value(install_key, "DisplayName")
                .unwrap_or_else(|| subkey_name.to_string_lossy().into_owned());
            let normalized_name = product_name
                .chars()
                .filter(char::is_ascii_alphanumeric)
                .collect::<String>()
                .to_ascii_lowercase();
            if normalized_name.contains("kirocrew") {
                if let Some(location) =
                    read_value(install_key, "InstallLocation").filter(|value| !value.is_empty())
                {
                    locations.push(PathBuf::from(location));
                }
                if let Some(icon) = read_value(install_key, "DisplayIcon") {
                    let icon = icon.trim();
                    let icon_path = if let Some(quoted) = icon.strip_prefix('"') {
                        quoted.split_once('"').map_or(quoted, |(path, _)| path)
                    } else {
                        icon.split_once(',').map_or(icon, |(path, _)| path)
                    };
                    if !icon_path.trim().is_empty() {
                        locations.push(PathBuf::from(icon_path.trim()));
                    }
                }
            }
            unsafe { RegCloseKey(install_key) };
        }
        unsafe { RegCloseKey(root) };
    }
    locations
}

#[cfg(not(windows))]
fn registered_crew_install_locations() -> Vec<PathBuf> {
    Vec::new()
}

pub fn cli_path(explicit: Option<&Path>) -> Option<PathBuf> {
    let mut known = Vec::new();
    #[cfg(windows)]
    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        known.push(PathBuf::from(local_app_data).join("kiro-cli/kiro-cli.exe"));
    }
    if let Some(base) = directories::BaseDirs::new() {
        known.push(base.home_dir().join(".local/bin/kiro-cli.exe"));
        known.push(base.home_dir().join(".local/bin/kiro-cli"));
    }
    crate::provider_cli::candidates_from(explicit, known, &["kiro-cli.exe", "kiro-cli"])
        .into_iter()
        .next()
}

pub fn installation_path(explicit: Option<&Path>) -> Option<PathBuf> {
    explicit
        .and_then(|path| {
            if path.is_file() {
                Some(path.to_path_buf())
            } else {
                let executable = path.join("Kiro.exe");
                executable.is_file().then_some(executable)
            }
        })
        .or_else(|| {
            #[cfg(windows)]
            {
                let local_app_data = env::var_os("LOCALAPPDATA").map(PathBuf::from);
                let program_files = env::var_os("ProgramFiles").map(PathBuf::from);
                let program_files_x86 = env::var_os("ProgramFiles(x86)").map(PathBuf::from);
                local_app_data
                    .into_iter()
                    .map(|path| path.join("Programs/Kiro/Kiro.exe"))
                    .chain(
                        program_files
                            .into_iter()
                            .map(|path| path.join("Kiro/Kiro.exe")),
                    )
                    .chain(
                        program_files_x86
                            .into_iter()
                            .map(|path| path.join("Kiro/Kiro.exe")),
                    )
                    .find(|path| path.is_file())
            }
            #[cfg(not(windows))]
            {
                None
            }
        })
}

fn state_database_path() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|base| {
        base.config_dir()
            .join("Kiro")
            .join("User")
            .join("globalStorage")
            .join("state.vscdb")
    })
}

#[derive(Default)]
struct KiroRuntimeMetadata {
    user_id: Option<String>,
    plan_name: Option<String>,
    account_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KiroLogEntry {
    command_name: Option<String>,
    output: Option<KiroUsageResponse>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KiroUsageResponse {
    subscription_info: Option<KiroSubscriptionInfo>,
    user_info: Option<KiroUserInfo>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KiroSubscriptionInfo {
    subscription_title: Option<String>,
    #[serde(rename = "type")]
    subscription_type: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KiroUserInfo {
    user_id: Option<String>,
    account_name: Option<String>,
    display_name: Option<String>,
    name: Option<String>,
    user_name: Option<String>,
    username: Option<String>,
    email: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KiroAuthToken {
    access_token: Option<String>,
    auth_method: Option<String>,
    provider: Option<String>,
    profile_arn: Option<String>,
    expires_at: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KiroLiveUsageResponse {
    next_date_reset: Option<Value>,
    subscription_info: Option<KiroSubscriptionInfo>,
    #[serde(default)]
    usage_breakdown_list: Vec<KiroUsageBreakdown>,
    user_info: Option<KiroUserInfo>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KiroUsageBreakdown {
    resource_type: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    display_name: Option<String>,
    display_name_plural: Option<String>,
    current_usage: Option<Value>,
    current_usage_with_precision: Option<Value>,
    usage_limit: Option<Value>,
    usage_limit_with_precision: Option<Value>,
    next_date_reset: Option<Value>,
    reset_date: Option<Value>,
}

/// Fetches the same monthly quota response used by Kiro's installed clients. The request
/// only reads Kiro data; credentials are read from its cache and never updated.
fn read_live_usage_limits() -> Result<RateLimits> {
    let token = read_kiro_auth_token()?;
    let profile_arn = token
        .profile_arn
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("Kiro sign-in token has no profile ARN"))?;
    let region = profile_region(profile_arn)
        .ok_or_else(|| anyhow!("Kiro sign-in profile has no supported region"))?;
    let endpoint = usage_endpoint(region)
        .ok_or_else(|| anyhow!("Kiro usage is not available in region {region}"))?;
    let encoded_profile = encode_query_component(profile_arn);
    let url = format!(
        "{endpoint}/getUsageLimits?origin=AI_EDITOR&profileArn={encoded_profile}&resourceType=AGENTIC_REQUEST&isEmailRequired=true"
    );
    let access_token = token
        .access_token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("Kiro sign-in token has no access token"))?;
    let authorization = format!("Bearer {access_token}");
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(10))
        .build();
    let mut request = agent
        .get(&url)
        .set("Authorization", &authorization)
        .set("Accept", "application/json")
        .set(
            "User-Agent",
            concat!("Codex Minibar/", env!("CARGO_PKG_VERSION")),
        );
    if token.auth_method.as_deref().is_some_and(is_external_idp) {
        request = request.set("TokenType", "EXTERNAL_IDP");
    }
    if token
        .provider
        .as_deref()
        .is_some_and(|provider| provider.eq_ignore_ascii_case("internal"))
    {
        request = request.set("redirect-for-internal", "true");
    }

    let response = match request.call() {
        Ok(response) => response,
        Err(ureq::Error::Status(429, _)) => {
            return Err(crate::worker::rate_limit_error(
                "Kiro usage request was rate limited (HTTP 429).",
            ));
        }
        Err(ureq::Error::Status(401 | 403, _)) => {
            return Err(anyhow!(
                "Kiro rejected its saved access token; open Kiro IDE or sign in again with Kiro CLI"
            ));
        }
        Err(ureq::Error::Status(status, _)) => {
            return Err(anyhow!("Kiro usage request failed with HTTP {status}"));
        }
        Err(ureq::Error::Transport(_)) => {
            return Err(anyhow!("Kiro usage endpoint is unreachable"));
        }
    };
    let raw = response
        .into_string()
        .context("could not read Kiro usage response")?;
    let response: KiroLiveUsageResponse =
        serde_json::from_str(&raw).context("Kiro returned an unsupported usage response")?;
    limits_from_live_usage(response)
}

fn limits_from_live_usage(response: KiroLiveUsageResponse) -> Result<RateLimits> {
    let credits = response
        .usage_breakdown_list
        .iter()
        .find(|breakdown| {
            is_credit_type(
                breakdown.resource_type.as_deref(),
                breakdown.kind.as_deref(),
                breakdown.display_name.as_deref(),
                breakdown.display_name_plural.as_deref(),
            )
        })
        .ok_or_else(|| anyhow!("Kiro usage response has no monthly credit breakdown"))?;
    let used = credits
        .current_usage_with_precision
        .as_ref()
        .and_then(numeric_value)
        .or_else(|| credits.current_usage.as_ref().and_then(numeric_value))
        .filter(|value| value.is_finite() && *value >= 0.0)
        .ok_or_else(|| anyhow!("Kiro usage response has no current credit usage"))?;
    let limit = credits
        .usage_limit_with_precision
        .as_ref()
        .and_then(numeric_value)
        .or_else(|| credits.usage_limit.as_ref().and_then(numeric_value))
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| anyhow!("Kiro usage response has no monthly credit limit"))?;
    let resets_at = credits
        .next_date_reset
        .as_ref()
        .or(credits.reset_date.as_ref())
        .or(response.next_date_reset.as_ref())
        .and_then(timestamp_value);
    let mut metadata = KiroRuntimeMetadata {
        user_id: response
            .user_info
            .as_ref()
            .and_then(|user| nonempty(user.user_id.as_deref())),
        plan_name: response
            .subscription_info
            .as_ref()
            .and_then(display_plan_name),
        account_name: response.user_info.as_ref().and_then(account_display_name),
    };
    let local_fallback =
        if resets_at.is_none() || metadata.plan_name.is_none() || metadata.account_name.is_none() {
            KiroClient::new().read_cached_usage_limits().ok()
        } else {
            None
        };
    if let Some(local) = &local_fallback {
        metadata.plan_name = metadata.plan_name.or_else(|| local.plan_type.clone());
        metadata.account_name = metadata.account_name.or_else(|| local.account_name.clone());
    }
    let resets_at = resets_at.or_else(|| {
        local_fallback
            .as_ref()
            .and_then(|local| local.secondary.resets_at.as_ref().cloned())
    });
    let used_percent = ((used / limit) * 100.0).round().clamp(0.0, 100.0) as u8;
    let duration_minutes = resets_at.and_then(month_duration_minutes);

    Ok(RateLimits {
        sampled_at: Utc::now(),
        account_name: metadata.account_name,
        plan_type: metadata.plan_name,
        secondary: LimitWindow {
            used_percent: Some(used_percent),
            resets_at,
            duration_minutes,
        },
        secondary_usage_amount: Some(UsageAmount { used, limit }),
        ..RateLimits::default()
    })
}

fn read_kiro_auth_token() -> Result<KiroAuthToken> {
    let path = directories::BaseDirs::new()
        .map(|base| base.home_dir().join(".aws/sso/cache/kiro-auth-token.json"))
        .ok_or_else(|| anyhow!("could not locate the Kiro sign-in token"))?;
    let metadata = fs::symlink_metadata(&path).context("Kiro sign-in token is unavailable")?;
    if metadata.file_type().is_symlink() {
        return Err(anyhow!("Kiro sign-in token must not be a symbolic link"));
    }
    let raw = fs::read(&path).context("could not read the Kiro sign-in token")?;
    let token: KiroAuthToken =
        serde_json::from_slice(&raw).context("Kiro sign-in token has an unsupported format")?;
    if token
        .expires_at
        .as_ref()
        .and_then(timestamp_value)
        .is_some_and(|expires_at| expires_at <= Utc::now() + chrono::Duration::seconds(30))
    {
        return Err(anyhow!(
            "Kiro's saved access token expired; open Kiro IDE or sign in again with Kiro CLI"
        ));
    }
    Ok(token)
}

fn profile_region(profile_arn: &str) -> Option<&str> {
    let fields: Vec<_> = profile_arn.split(':').collect();
    (fields.len() >= 6 && fields[0] == "arn" && fields[2] == "codewhisperer").then_some(fields[3])
}

/// These trusted q-service endpoints mirror the region table in the installed
/// Kiro extension. Unknown/custom regions fall back to the IDE's local cache.
fn usage_endpoint(region: &str) -> Option<&'static str> {
    match region {
        "us-east-1" => Some("https://q.us-east-1.amazonaws.com"),
        "eu-central-1" => Some("https://q.eu-central-1.amazonaws.com"),
        "us-gov-east-1" => Some("https://q-fips.us-gov-east-1.amazonaws.com"),
        "us-gov-west-1" => Some("https://q-fips.us-gov-west-1.amazonaws.com"),
        "us-iso-east-1" => Some("https://q.us-iso-east-1.c2s.ic.gov"),
        "us-isob-east-1" => Some("https://q.us-isob-east-1.sc2s.sgov.gov"),
        "us-isof-south-1" => Some("https://q.us-isof-south-1.csp.hci.ic.gov"),
        "us-isof-east-1" => Some("https://q.us-isof-east-1.csp.hci.ic.gov"),
        _ => None,
    }
}

fn is_external_idp(auth_method: &str) -> bool {
    auth_method.eq_ignore_ascii_case("external_idp")
        || auth_method.eq_ignore_ascii_case("externalidp")
}

fn encode_query_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            use std::fmt::Write as _;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

fn timestamp_value(value: &Value) -> Option<DateTime<Utc>> {
    if let Some(value) = value.as_str() {
        if let Ok(value) = DateTime::parse_from_rfc3339(value) {
            return Some(value.with_timezone(&Utc));
        }
        return value.parse::<f64>().ok().and_then(unix_timestamp);
    }
    value
        .as_i64()
        .map(|timestamp| timestamp as f64)
        .or_else(|| value.as_f64())
        .and_then(unix_timestamp)
}

fn unix_timestamp(timestamp: f64) -> Option<DateTime<Utc>> {
    if !timestamp.is_finite() {
        return None;
    }
    let milliseconds = if timestamp.abs() >= 100_000_000_000.0 {
        timestamp
    } else {
        timestamp * 1_000.0
    };
    if milliseconds < i64::MIN as f64 || milliseconds > i64::MAX as f64 {
        return None;
    }
    DateTime::from_timestamp_millis(milliseconds.round() as i64)
}

/// Kiro logs the quota response locally. Read only its plan and identity labels;
/// unrelated request fields (including any future auth fields) are ignored by
/// the typed deserializer and are never copied into Minibar state.
fn local_runtime_metadata() -> KiroRuntimeMetadata {
    let mut logs = q_client_log_paths();
    logs.sort_by_key(|path| {
        fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH)
    });
    logs.reverse();

    let mut selected = KiroRuntimeMetadata::default();
    for path in logs {
        let Some(candidate) = latest_usage_metadata_in_log(&path) else {
            continue;
        };
        let same_user = match (&selected.user_id, &candidate.user_id) {
            (Some(selected), Some(candidate)) => selected == candidate,
            (Some(_), None) => false,
            _ => true,
        };
        if selected.plan_name.is_none() {
            selected.plan_name = candidate.plan_name;
        }
        if selected.user_id.is_none() {
            selected.user_id = candidate.user_id;
        }
        if same_user && selected.account_name.is_none() {
            selected.account_name = candidate.account_name;
        }
        if selected.plan_name.is_some() && selected.account_name.is_some() {
            break;
        }
    }
    selected
}

fn q_client_log_paths() -> Vec<PathBuf> {
    let Some(base) = directories::BaseDirs::new() else {
        return Vec::new();
    };
    let logs_root = base.config_dir().join("Kiro").join("logs");
    let Ok(sessions) = fs::read_dir(logs_root) else {
        return Vec::new();
    };

    let mut paths = Vec::new();
    for session in sessions.flatten() {
        if !session.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let Ok(windows) = fs::read_dir(session.path()) else {
            continue;
        };
        for window in windows.flatten() {
            if !window.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            let path = window
                .path()
                .join("exthost")
                .join("kiro.kiroAgent")
                .join("q-client.log");
            if path.is_file() {
                paths.push(path);
            }
        }
    }
    paths
}

fn latest_usage_metadata_in_log(path: &Path) -> Option<KiroRuntimeMetadata> {
    let file = fs::File::open(path).ok()?;
    let mut latest = None;
    for line in BufReader::new(file)
        .lines()
        .map_while(std::result::Result::ok)
    {
        if !line.contains(USAGE_COMMAND) || !line.contains("subscriptionInfo") {
            continue;
        }
        let Some(start) = line.find('{') else {
            continue;
        };
        let mut deserializer = serde_json::Deserializer::from_str(&line[start..]);
        let Ok(entry) = KiroLogEntry::deserialize(&mut deserializer) else {
            continue;
        };
        if entry.command_name.as_deref() != Some(USAGE_COMMAND) {
            continue;
        }
        let Some(output) = entry.output else {
            continue;
        };

        let plan_name = output
            .subscription_info
            .as_ref()
            .and_then(display_plan_name);
        let user = output.user_info;
        let candidate = KiroRuntimeMetadata {
            user_id: user
                .as_ref()
                .and_then(|user| nonempty(user.user_id.as_deref())),
            plan_name,
            account_name: user.as_ref().and_then(account_display_name),
        };
        if candidate.plan_name.is_some()
            || candidate.account_name.is_some()
            || candidate.user_id.is_some()
        {
            if let Some(current) = latest.as_mut() {
                merge_log_metadata(current, candidate);
            } else {
                latest = Some(candidate);
            }
        }
    }
    latest
}

fn merge_log_metadata(current: &mut KiroRuntimeMetadata, candidate: KiroRuntimeMetadata) {
    let user_changed = matches!(
        (&current.user_id, &candidate.user_id),
        (Some(current), Some(candidate)) if current != candidate
    );
    if user_changed {
        *current = candidate;
        return;
    }
    if current.user_id.is_none() && candidate.user_id.is_some() {
        // An earlier label without an id cannot be safely tied to this user.
        current.account_name = None;
        current.user_id = candidate.user_id;
    }
    if candidate.plan_name.is_some() {
        current.plan_name = candidate.plan_name;
    }
    if candidate.account_name.is_some() {
        current.account_name = candidate.account_name;
    }
}

fn display_plan_name(subscription: &KiroSubscriptionInfo) -> Option<String> {
    subscription
        .subscription_title
        .as_deref()
        .and_then(normalize_plan_name)
        .or_else(|| {
            subscription
                .subscription_type
                .as_deref()
                .and_then(known_plan_from_type)
        })
}

fn normalize_plan_name(value: &str) -> Option<String> {
    let value = value.trim();
    let value = if value
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("kiro "))
    {
        &value[5..]
    } else {
        value
    }
    .trim();
    nonempty(Some(value))
}

fn known_plan_from_type(value: &str) -> Option<String> {
    match value
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '_', '-'], "")
        .as_str()
    {
        "free" => Some("Free".into()),
        "pro" => Some("Pro".into()),
        "proplus" => Some("Pro+".into()),
        "promax" => Some("Pro Max".into()),
        "power" => Some("Power".into()),
        _ => None,
    }
}

fn account_display_name(user: &KiroUserInfo) -> Option<String> {
    first_nonempty([
        user.account_name.as_deref(),
        user.display_name.as_deref(),
        user.name.as_deref(),
        user.user_name.as_deref(),
        user.username.as_deref(),
        user.email.as_deref(),
    ])
}

fn first_nonempty<'a>(values: impl IntoIterator<Item = Option<&'a str>>) -> Option<String> {
    values.into_iter().find_map(nonempty)
}

fn nonempty(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() || is_redaction_marker(value) {
        None
    } else {
        Some(value.to_owned())
    }
}

fn is_redaction_marker(value: &str) -> bool {
    let normalized: String = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    matches!(
        normalized.as_str(),
        "sensitiveinformation" | "redacted" | "redactedvalue"
    )
}

fn is_credit_breakdown(value: &Value) -> bool {
    is_credit_type(
        value.get("resourceType").and_then(Value::as_str),
        value.get("type").and_then(Value::as_str),
        value.get("displayName").and_then(Value::as_str),
        value.get("displayNamePlural").and_then(Value::as_str),
    )
}

fn is_credit_type(
    resource_type: Option<&str>,
    kind: Option<&str>,
    display_name: Option<&str>,
    display_name_plural: Option<&str>,
) -> bool {
    [resource_type, kind]
        .into_iter()
        .flatten()
        .any(|value| value.eq_ignore_ascii_case("CREDIT"))
        || [display_name, display_name_plural]
            .into_iter()
            .flatten()
            .any(|value| value.to_ascii_lowercase().contains("credit"))
}

fn numeric_field(value: &Value, name: &str) -> Option<f64> {
    value.get(name).and_then(numeric_value)
}

fn numeric_value(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
}

fn month_duration_minutes(resets_at: DateTime<Utc>) -> Option<u32> {
    let start = resets_at.checked_sub_months(Months::new(1))?;
    u32::try_from((resets_at - start).num_minutes()).ok()
}
