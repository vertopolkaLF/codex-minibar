use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use directories::BaseDirs;
use serde::Deserialize;
use serde_json::Value;

use crate::{
    claude_desktop,
    instances::{ProviderId, ProviderInstance},
    limits::{
        AdditionalLimit, LimitWindow, RateLimitResetCredit, RateLimitResetCreditsSummary,
        RateLimits, SpendingSummary,
    },
    secrets,
    usage,
    worker::{Activator, LimitProvider, UsageProvider},
};

pub(crate) mod profile_oauth;

/// Removes leftovers of the former temporary sign-in folders.
pub fn cleanup_abandoned_logins() -> Result<()> {
    profile_oauth::cleanup_abandoned_logins()
}

/// `cedar_ember=1` asks the endpoint to include banked usage-limit resets,
/// the same query Claude Code's `/limit-reset` flow sends. `skip_spend=1`
/// drops the spend block, which nothing here reads.
const OAUTH_USAGE_URL: &str =
    "https://api.anthropic.com/api/oauth/usage?cedar_ember=1&skip_spend=1";
const OAUTH_PROFILE_URL: &str = "https://api.anthropic.com/api/oauth/profile";
const OAUTH_BETA: &str = "oauth-2025-04-20";
const WEB_ORGANIZATIONS_URL: &str = "https://claude.ai/api/organizations";
const WEB_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";
const ADMIN_COST_REPORT_URL: &str = "https://api.anthropic.com/v1/organizations/cost_report";
const ADMIN_API_VERSION: &str = "2023-06-01";
const MANUAL_SECRET_PREFIX: &str = "claude-profile-";
const MANUAL_CREDENTIAL_HINT: &str = "Update this instance's credential in Settings > Providers.";
const FOLDER_SIGN_IN_HINT: &str = "Use Sign in for this instance in Settings > Providers.";
/// How long an expired login waits before Claude Code is asked again.
const REFRESH_RETRY_INTERVAL: Duration = Duration::from_secs(10 * 60);
const FALLBACK_CLAUDE_CODE_VERSION: &str = "2.1.280";
const PROFILE_REFRESH_INTERVAL: Duration = Duration::from_secs(30 * 60);
pub const ACTIVATION_MODEL: &str = "haiku";
pub const ACTIVATION_PROMPT: &str = "reply with letter a";

/// Detect a local Claude Code installation without starting it. A signed-in
/// credentials directory is also a useful signal for portable/npm installs
/// whose launcher is no longer present on PATH in the current process, and the
/// desktop app counts too: it ships its own Claude Code and never writes a
/// credentials file.
pub fn is_installed(explicit: Option<&Path>) -> bool {
    first_available(explicit).is_some() || credentials_path().is_some_and(|path| path.is_file())
}

/// Resolves a Claude Code launcher without spawning it. An explicit folder is
/// searched first; legacy explicit file paths remain supported for upgrades.
pub fn first_available(explicit: Option<&Path>) -> Option<PathBuf> {
    claude_desktop::bundled_cli().or_else(|| cli_available(explicit))
}

/// Finds only a standalone Claude Code CLI, excluding the launcher bundled by
/// Claude Desktop. Used by the UI to report both installations clearly.
pub fn cli_available(explicit: Option<&Path>) -> Option<PathBuf> {
    let path_dirs = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default();
    find_cli(explicit, &path_dirs, &common_launcher_dirs())
}

fn find_cli(
    explicit: Option<&Path>,
    path_dirs: &[PathBuf],
    common_dirs: &[PathBuf],
) -> Option<PathBuf> {
    explicit.and_then(explicit_launcher).or_else(|| {
        path_dirs
            .iter()
            .chain(common_dirs)
            .find_map(|directory| launcher_in(directory))
    })
}

fn common_launcher_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(base) = BaseDirs::new() {
        // The native installer uses this location even when the current
        // process inherited PATH before installation completed.
        dirs.push(base.home_dir().join(".local/bin"));
        #[cfg(not(windows))]
        dirs.push(base.home_dir().join(".npm-global/bin"));
    }
    #[cfg(windows)]
    {
        if let Some(roaming) = std::env::var_os("APPDATA") {
            dirs.push(PathBuf::from(roaming).join("npm"));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let local = PathBuf::from(local);
            dirs.extend([local.join("npm"), local.join("pnpm")]);
        }
        if let Some(pnpm) = std::env::var_os("PNPM_HOME") {
            dirs.push(PathBuf::from(pnpm));
        }
        dirs.extend(crate::discovery::node_global_bin_dirs());
    }
    dirs
}

fn explicit_launcher(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    launcher_in(path)
}

fn launcher_in(directory: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    let names = ["claude.exe", "claude.cmd", "claude.ps1", "claude.bat"];
    #[cfg(not(windows))]
    let names = ["claude"];
    names
        .into_iter()
        .map(|name| directory.join(name))
        .find(|path| path.is_file())
}

/// Starts Claude Code's five-hour window with the smallest supported prompt.
pub struct ClaudeActivator {
    timeout: Duration,
    executable: Option<PathBuf>,
    /// The instance's `CLAUDE_CONFIG_DIR`; `None` uses this PC's login.
    config_folder: Option<PathBuf>,
}

impl ClaudeActivator {
    pub fn new(executable: Option<PathBuf>) -> Self {
        Self {
            timeout: Duration::from_secs(120),
            executable,
            config_folder: None,
        }
    }

    pub fn with_config_folder(mut self, folder: Option<PathBuf>) -> Self {
        self.config_folder = folder;
        self
    }

    pub fn activate_minimal(&self) -> Result<()> {
        let mut command = activation_command(self.executable.as_deref());
        scope_command(&mut command, self.config_folder.as_deref());
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("launch Claude activation through `claude`")?;
        let deadline = Instant::now() + self.timeout;
        loop {
            if let Some(status) = child.try_wait().context("wait for Claude activation")? {
                anyhow::ensure!(status.success(), "Claude activation exited with {status}");
                return Ok(());
            }
            if Instant::now() >= deadline {
                terminate(&mut child);
                bail!("Claude activation timed out after {:?}", self.timeout);
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}

impl Default for ClaudeActivator {
    fn default() -> Self {
        Self::new(None)
    }
}

impl Activator for ClaudeActivator {
    fn activate(&mut self) -> Result<()> {
        self.activate_minimal()
    }
}

fn activation_command(explicit: Option<&Path>) -> Command {
    activation_command_for(activation_program(explicit))
}

/// Prefer the launcher on PATH, then the `claude.exe` the desktop app unpacks
/// for its embedded Claude Code. Falling back to the bare name keeps the error
/// message about a missing CLI rather than a missing desktop install.
fn activation_program(explicit: Option<&Path>) -> PathBuf {
    first_available(explicit).unwrap_or_else(|| PathBuf::from("claude"))
}

fn activation_command_for(program: PathBuf) -> Command {
    let mut command = Command::new(program);
    command.args([
        "-p",
        ACTIVATION_PROMPT,
        "--model",
        ACTIVATION_MODEL,
        "--effort=low",
    ]);
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

pub fn save_manual_credential(instance_id: &str, value: Option<&str>) -> Result<()> {
    secrets::save(&manual_secret_name(instance_id), value)
}

pub(crate) fn load_manual_credential(instance_id: &str) -> Result<Option<String>> {
    secrets::load(&manual_secret_name(instance_id))
}

/// Saved accounts used this slot before instances; the name is unchanged so
/// migrated manual credentials keep working.
fn manual_secret_name(instance_id: &str) -> String {
    format!("{MANUAL_SECRET_PREFIX}{instance_id}")
}

/// Pasted credentials accepted by an instance whose Source is Manual. Admin
/// API keys are read for compatibility but not offered for new credentials.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ProfileCredentialMethod {
    #[default]
    BrowserSession,
    OAuthToken,
}

impl ProfileCredentialMethod {
    pub(crate) fn validate(self, raw: &str) -> Result<()> {
        match (self, Credential::parse(raw)?) {
            (Self::BrowserSession, Credential::Cookie(cookie)) => {
                anyhow::ensure!(
                    cookie.split(';').any(|part| part
                        .trim()
                        .strip_prefix("sessionKey=")
                        .is_some_and(|value| !value.trim().is_empty())),
                    "Paste the sessionKey value or a Cookie header containing sessionKey."
                );
                Ok(())
            }
            (Self::OAuthToken, Credential::OAuth(_)) => Ok(()),
            (_, Credential::AdminKey(_)) => bail!(
                "Admin API keys report API organization spending, not Claude subscription limits. Use a browser session or an OAuth token."
            ),
            (Self::BrowserSession, _) => {
                bail!("This is an OAuth token. Select the OAuth token tab.")
            }
            (Self::OAuthToken, _) => bail!(
                "Paste an OAuth access token starting with sk-ant-oat. For a sessionKey, select Browser session."
            ),
        }
    }
}

/// Checks a pasted credential with one live read before it is saved.
pub fn verify_credential(credential: &str) -> Result<()> {
    let mut client = ClaudeClient::new();
    let agent = client.agent()?;
    client.read_credential(&agent, credential).map(drop)
}

/// How a pasted credential is used. Like CodexBar, the kind is inferred from
/// the value's shape instead of asking the user to pick one.
#[derive(Debug, PartialEq, Eq)]
enum Credential {
    /// `sk-ant-admin…`: organization spend from the Admin API.
    AdminKey(String),
    /// `sk-ant-oat…`: the same OAuth usage endpoint as a signed-in folder.
    OAuth(String),
    /// A claude.ai `Cookie` header; a bare value is taken as the `sessionKey`.
    Cookie(String),
}

impl Credential {
    fn parse(raw: &str) -> Result<Self> {
        let value = raw.trim();
        let lower = value.to_ascii_lowercase();
        if lower.starts_with("cookie:") {
            return Ok(Self::Cookie(value["cookie:".len()..].trim().to_owned()));
        }
        if value.contains('=') {
            return Ok(Self::Cookie(value.to_owned()));
        }
        let token = if lower.starts_with("bearer ") {
            value["bearer ".len()..].trim()
        } else {
            value
        };
        let lower = token.to_ascii_lowercase();
        if lower.starts_with("sk-ant-admin") {
            Ok(Self::AdminKey(token.to_owned()))
        } else if lower.starts_with("sk-ant-oat") {
            Ok(Self::OAuth(token.to_owned()))
        } else if lower.starts_with("sk-ant-api") {
            bail!(
                "Standard API keys cannot report Claude subscription limits. Paste a sessionKey or an OAuth access token."
            )
        } else if token.is_empty() {
            bail!("Paste a credential first.")
        } else {
            Ok(Self::Cookie(format!("sessionKey={token}")))
        }
    }
}

/// Where one Claude instance reads its login.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClaudeLogin {
    /// This PC's standard login: Claude Desktop's session or `~/.claude`.
    Ambient,
    /// An instance's own `CLAUDE_CONFIG_DIR`.
    Folder(PathBuf),
    /// A pasted credential in protected storage, keyed by instance id.
    Manual(String),
}

impl ClaudeLogin {
    pub fn for_instance(instance: &ProviderInstance) -> Self {
        if instance.uses_manual_credential() {
            return Self::Manual(instance.id.clone());
        }
        instance
            .config_folder()
            .map_or(Self::Ambient, Self::Folder)
    }

    fn folder(&self) -> Option<&Path> {
        match self {
            Self::Folder(folder) => Some(folder),
            _ => None,
        }
    }
}

/// Reads one Claude instance's usage from Claude's OAuth usage endpoint, the
/// same one CodexBar queries. Folder logins stay in Claude's own
/// `.credentials.json`; Minibar only ever reads them.
pub struct ClaudeClient {
    timeout: Duration,
    provider: ProviderId,
    login: ClaudeLogin,
    executable: Option<PathBuf>,
    account_cache: ClaudeAccountCache,
    /// Expired logins are handed to `claude auth status` at most this often.
    last_refresh_attempt: Option<Instant>,
    rate_limited: bool,
}

#[derive(Default)]
struct ClaudeAccountCache {
    account_name: Option<String>,
    plan_type: Option<String>,
    checked_at: Option<Instant>,
    reset_schedule: Vec<(String, Option<DateTime<Utc>>)>,
}

impl ClaudeAccountCache {
    fn needs_refresh(&self, reset_schedule: &[(String, Option<DateTime<Utc>>)]) -> bool {
        self.checked_at
            .is_none_or(|checked_at| checked_at.elapsed() >= PROFILE_REFRESH_INTERVAL)
            || self.reset_schedule != reset_schedule
    }

    fn record(
        &mut self,
        account_name: Option<String>,
        plan_type: Option<String>,
        reset_schedule: Vec<(String, Option<DateTime<Utc>>)>,
    ) {
        self.account_name = account_name;
        self.plan_type = plan_type;
        self.checked_at = Some(Instant::now());
        self.reset_schedule = reset_schedule;
    }
}

impl ClaudeClient {
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(15),
            provider: ProviderId::primary(crate::settings::ProviderKind::Claude),
            login: ClaudeLogin::Ambient,
            executable: None,
            account_cache: ClaudeAccountCache::default(),
            last_refresh_attempt: None,
            rate_limited: false,
        }
    }

    pub fn for_instance(instance: &ProviderInstance) -> Self {
        Self {
            provider: instance.provider_id(),
            login: ClaudeLogin::for_instance(instance),
            executable: first_available(instance.binary_path.as_deref()),
            ..Self::new()
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn read_rate_limits(&mut self) -> Result<RateLimits> {
        let agent = self.agent()?;
        match self.login.clone() {
            ClaudeLogin::Ambient => {
                let credentials = self.with_refresh(None, load_credentials)?;
                let hint = credentials.source.sign_in_hint();
                // Both credential files record the plan next to the token, so
                // the common case needs no request at all to label it.
                let plan = plan_type_from_account_fields(
                    credentials.subscription_type,
                    credentials.rate_limit_tier,
                );
                self.read_oauth(&agent, &credentials.access_token, hint, plan)
            }
            ClaudeLogin::Folder(folder) => {
                let file = folder.join(".credentials.json");
                let credentials =
                    self.with_refresh(Some(&folder), || load_cli_credentials_at(&file))?;
                let plan = plan_type_from_account_fields(
                    credentials.subscription_type,
                    credentials.rate_limit_tier,
                );
                self.read_oauth(&agent, &credentials.access_token, FOLDER_SIGN_IN_HINT, plan)
            }
            ClaudeLogin::Manual(instance_id) => {
                let credential = load_manual_credential(&instance_id)?.with_context(|| {
                    format!("This Claude instance has no saved credential. {MANUAL_CREDENTIAL_HINT}")
                })?;
                self.read_credential(&agent, &credential)
            }
        }
    }

    /// Reads a login; when it has expired, lets Claude Code refresh it once
    /// through `claude auth status`, which never contacts a model and so can
    /// never start a session window, then reads it again.
    fn with_refresh(
        &mut self,
        folder: Option<&Path>,
        load: impl Fn() -> Result<Credentials>,
    ) -> Result<Credentials> {
        let credentials = load()?;
        if !credentials.is_expired() {
            return Ok(credentials);
        }
        let refreshable = credentials.source == CredentialSource::Cli
            && self
                .last_refresh_attempt
                .is_none_or(|at| at.elapsed() >= REFRESH_RETRY_INTERVAL);
        if refreshable && let Some(executable) = self.executable.clone() {
            self.last_refresh_attempt = Some(Instant::now());
            if let Err(error) = refresh_login(&executable, folder) {
                crate::logger::info(format!("Claude login refresh failed: {error:#}"));
            }
            let refreshed = load()?;
            if !refreshed.is_expired() {
                return Ok(refreshed);
            }
        }
        let hint = if folder.is_some() {
            FOLDER_SIGN_IN_HINT
        } else {
            credentials.source.sign_in_hint()
        };
        bail!("Claude login has expired. {hint}")
    }

    fn read_credential(&mut self, agent: &ureq::Agent, credential: &str) -> Result<RateLimits> {
        // A sign-in session kept here by an interrupted migration still has
        // a usable access token until it expires.
        if credential.trim_start().starts_with('{') {
            let session: Value = serde_json::from_str(credential)
                .map_err(|_| anyhow::anyhow!("Saved Claude login is unreadable."))?;
            let token = session["accessToken"]
                .as_str()
                .context("Saved Claude login has no access token.")?
                .to_owned();
            return self.read_oauth(agent, &token, MANUAL_CREDENTIAL_HINT, None);
        }
        match Credential::parse(credential)? {
            Credential::OAuth(token) => {
                self.read_oauth(agent, &token, MANUAL_CREDENTIAL_HINT, None)
            }
            // Cloudflare challenges the Schannel handshake of the shared
            // agent, so claude.ai is read through ureq's default TLS backend.
            Credential::Cookie(cookie) => read_web_usage(
                &ureq::AgentBuilder::new().timeout(self.timeout).build(),
                &cookie,
            ),
            Credential::AdminKey(key) => read_admin_spending(agent, &key),
        }
    }

    fn agent(&self) -> Result<ureq::Agent> {
        // `ureq` is built without its default Rustls backend. Configure the
        // native TLS adapter explicitly so Claude's HTTPS endpoint uses the
        // Windows certificate store (Schannel), as the updater already does.
        let tls = ureq::native_tls::TlsConnector::new().context("create Windows TLS connector")?;
        Ok(ureq::AgentBuilder::new()
            .timeout(self.timeout)
            .tls_connector(Arc::new(tls))
            .build())
    }

    fn read_oauth(
        &mut self,
        agent: &ureq::Agent,
        access_token: &str,
        sign_in_hint: &str,
        local_plan_type: Option<String>,
    ) -> Result<RateLimits> {
        let body = send(
            agent
                .get(OAUTH_USAGE_URL)
                .set("Authorization", &format!("Bearer {access_token}"))
                .set("Accept", "application/json")
                .set("Content-Type", "application/json")
                .set("anthropic-beta", OAUTH_BETA)
                // Banked resets are only reported to the CLI surface; any other
                // User-Agent gets `cedar_ember.ineligible_reason = "surface"`.
                .set("User-Agent", &cli_user_agent()),
            "Claude OAuth usage request",
            sign_in_hint,
        )
        .inspect_err(|error| {
            self.rate_limited |= crate::worker::is_rate_limited_error(error);
        })?;
        let mut limits = parse_usage_response(&body, Utc::now())?;
        let reset_schedule = reset_schedule(&limits);
        let cache = &mut self.account_cache;
        if cache.needs_refresh(&reset_schedule) {
            // Account metadata stays separate from quota reads. Cache it for
            // 30 minutes and refresh immediately when any reset changes. One
            // profile request covers both the name and the plan fallback.
            let profile = match fetch_profile(agent, access_token) {
                Ok(profile) => Some(profile),
                Err(error) if crate::worker::is_rate_limited_error(&error) => return Err(error),
                Err(_) => None,
            };
            cache.record(
                profile
                    .as_ref()
                    .and_then(|profile| profile.account_name.clone()),
                profile.and_then(|profile| profile.plan_type),
                reset_schedule,
            );
        }
        // Newer usage responses omit organization_name, but older responses
        // still expose it. The cached profile is authoritative whenever it
        // provides an identity value.
        if let Some(account_name) = cache.account_name.clone() {
            limits.account_name = Some(account_name);
        }
        limits.plan_type = local_plan_type.or_else(|| cache.plan_type.clone());
        Ok(limits)
    }
}

/// Environment variables that would make Claude Code ignore the config
/// folder's login.
pub(crate) const AUTH_OVERRIDES: &[&str] = &[
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

/// Points a Claude Code command at one instance's login.
pub(crate) fn scope_command(command: &mut Command, folder: Option<&Path>) {
    if let Some(folder) = folder {
        command.env("CLAUDE_CONFIG_DIR", folder);
        for key in AUTH_OVERRIDES {
            command.env_remove(key);
        }
    }
}

fn refresh_command(executable: &Path, folder: Option<&Path>) -> Command {
    let mut command = Command::new(executable);
    command
        .args(["auth", "status"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(folder) = folder {
        command.current_dir(folder);
    }
    scope_command(&mut command, folder);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    command
}

/// `claude auth status` refreshes an expired OAuth login under Claude Code's
/// own lock and writes it back, without sending any model request.
fn refresh_login(executable: &Path, folder: Option<&Path>) -> Result<()> {
    let mut child = refresh_command(executable, folder)
        .spawn()
        .context("start `claude auth status`")?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            terminate(&mut child);
            bail!("`claude auth status` timed out");
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Sends a Claude request and maps the failures every endpoint shares.
fn send(request: ureq::Request, what: &str, sign_in_hint: &str) -> Result<String> {
    match request.call() {
        Ok(response) => response.into_string().with_context(|| what.to_owned()),
        // claude.ai answers an expired session with 403, the API with 401.
        Err(ureq::Error::Status(401 | 403, _)) => {
            bail!("{what} was unauthorized. {sign_in_hint}")
        }
        Err(ureq::Error::Status(429, _)) => Err(crate::worker::rate_limit_error(
            "Claude usage endpoint is rate limited. Try again in a few minutes.",
        )),
        Err(ureq::Error::Status(status, _)) => bail!("{what} failed with HTTP {status}"),
        Err(error) => Err(error).with_context(|| what.to_owned()),
    }
}

#[derive(Deserialize)]
struct WebOrganization {
    uuid: String,
    name: Option<String>,
    #[serde(default)]
    capabilities: Vec<String>,
    rate_limit_tier: Option<String>,
    billing_type: Option<String>,
}

impl WebOrganization {
    /// The tier names Max and Team plans. A Pro plan only shows up as a
    /// capability or, like CodexBar infers it, as a Stripe-billed Claude tier.
    fn plan_type(&self) -> Option<String> {
        let tier = self.rate_limit_tier.clone().unwrap_or_default();
        let stripe_billed = self
            .billing_type
            .as_deref()
            .is_some_and(|billing| billing.contains("stripe"));
        plan_type_from_tier(Some(tier.clone()))
            .or_else(|| {
                self.capabilities
                    .iter()
                    .find_map(|capability| plan_type_from_tier(Some(capability.clone())))
            })
            .or_else(|| (stripe_billed && tier.contains("claude")).then(|| "pro".to_owned()))
    }
}

/// Reads subscription limits with a claude.ai session cookie. The web usage
/// endpoint returns the same window shape as the OAuth one.
fn read_web_usage(agent: &ureq::Agent, cookie: &str) -> Result<RateLimits> {
    let get = |url: &str| {
        send(
            agent
                .get(url)
                .set("Cookie", cookie)
                .set("Accept", "application/json")
                // Cloudflare challenges obvious non-browser clients before
                // they ever reach the API.
                .set("User-Agent", WEB_USER_AGENT),
            "Claude web usage request",
            MANUAL_CREDENTIAL_HINT,
        )
    };
    let organization = pick_organization(&get(WEB_ORGANIZATIONS_URL)?)?;
    let body = get(&format!(
        "{WEB_ORGANIZATIONS_URL}/{}/usage",
        organization.uuid
    ))?;
    let mut limits = parse_usage_response(&body, Utc::now())?;
    limits.plan_type = organization.plan_type();
    limits.account_name = non_empty(organization.name);
    Ok(limits)
}

/// A login can belong to several organizations; subscription limits live on
/// the one with chat access, not on an API-only organization.
fn pick_organization(response: &str) -> Result<WebOrganization> {
    let mut organizations: Vec<WebOrganization> =
        serde_json::from_str(response).context("parse Claude organizations")?;
    anyhow::ensure!(
        !organizations.is_empty(),
        "Claude session has no organization"
    );
    let index = organizations
        .iter()
        .position(|organization| organization.capabilities.iter().any(|name| name == "chat"))
        .unwrap_or(0);
    Ok(organizations.swap_remove(index))
}

#[derive(Deserialize)]
struct CostReport {
    data: Vec<CostBucket>,
}

#[derive(Deserialize)]
struct CostBucket {
    results: Vec<CostResult>,
}

#[derive(Deserialize)]
struct CostResult {
    amount: String,
}

/// Admin API keys belong to an API organization, which has spend rather than
/// subscription windows. Reports the last 30 days of cost.
fn read_admin_spending(agent: &ureq::Agent, api_key: &str) -> Result<RateLimits> {
    let now = Utc::now();
    let starting_at = (now - chrono::Duration::days(30)).format("%Y-%m-%dT00:00:00Z");
    let body = send(
        agent
            .get(ADMIN_COST_REPORT_URL)
            .query("starting_at", &starting_at.to_string())
            .query("bucket_width", "1d")
            .query("limit", "31")
            .set("x-api-key", api_key)
            .set("anthropic-version", ADMIN_API_VERSION),
        "Claude Admin API cost report",
        MANUAL_CREDENTIAL_HINT,
    )?;
    parse_cost_report(&body, now)
}

fn parse_cost_report(response: &str, sampled_at: DateTime<Utc>) -> Result<RateLimits> {
    let report: CostReport =
        serde_json::from_str(response).context("parse Claude Admin API cost report")?;
    // `amount` is a decimal string in cents.
    let cents = report
        .data
        .iter()
        .flat_map(|bucket| &bucket.results)
        .filter_map(|result| result.amount.parse::<f64>().ok())
        .sum::<f64>();
    Ok(RateLimits {
        sampled_at,
        spending: Some(SpendingSummary {
            used_microusd: (cents * 10_000.0).round() as u64,
            ..SpendingSummary::default()
        }),
        ..RateLimits::default()
    })
}

impl Default for ClaudeClient {
    fn default() -> Self {
        Self::new()
    }
}

impl LimitProvider for ClaudeClient {
    fn take_rate_limit_response(&mut self) -> bool {
        std::mem::take(&mut self.rate_limited)
    }
    fn read_limits(&mut self) -> Result<RateLimits> {
        self.read_rate_limits()
    }
}

/// Claude Code's own User-Agent, versioned after the CLI bundled with Claude
/// Desktop when present so the server sees a current client.
fn cli_user_agent() -> String {
    let version = claude_desktop::bundled_cli()
        .as_deref()
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| FALLBACK_CLAUDE_CODE_VERSION.to_owned());
    format!("claude-cli/{version} (external, cli)")
}

fn reset_schedule(limits: &RateLimits) -> Vec<(String, Option<DateTime<Utc>>)> {
    let mut schedule = vec![
        ("primary".into(), limits.primary.resets_at),
        ("secondary".into(), limits.secondary.resets_at),
    ];
    schedule.extend(
        limits
            .additional_limits
            .iter()
            .map(|limit| (limit.id.clone(), limit.window.resets_at)),
    );
    schedule
}

impl UsageProvider for ClaudeClient {
    fn load_cached_usage_statistics(
        &mut self,
        history_days: u16,
    ) -> Result<usage::UsageStatistics> {
        usage::load_cached_claude_usage_statistics(self.provider, history_days)
    }

    fn refresh_usage_statistics(&mut self, history_days: u16) -> Result<usage::UsageStatistics> {
        if matches!(self.login, ClaudeLogin::Manual(_)) {
            return usage::load_cached_claude_usage_statistics(self.provider, history_days);
        }
        usage::refresh_claude_usage_statistics(self.provider, self.login.folder(), history_days)
    }
}

#[derive(Deserialize)]
struct CredentialFile {
    #[serde(rename = "claudeAiOauth")]
    oauth: Option<OAuthCredentials>,
}

#[derive(Deserialize)]
struct OAuthCredentials {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "expiresAt")]
    expires_at_millis: Option<i64>,
    #[serde(rename = "subscriptionType")]
    subscription_type: Option<String>,
    #[serde(rename = "rateLimitTier")]
    rate_limit_tier: Option<String>,
}

struct Credentials {
    access_token: String,
    expires_at: Option<DateTime<Utc>>,
    /// Both the CLI credentials file and the desktop token cache record the
    /// plan next to the token, so neither path needs a request to label it.
    subscription_type: Option<String>,
    rate_limit_tier: Option<String>,
    source: CredentialSource,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CredentialSource {
    Cli,
    Desktop,
}

impl CredentialSource {
    /// How to get a fresh session back for this source.
    fn sign_in_hint(self) -> &'static str {
        match self {
            Self::Cli => "Run `claude` to sign in again.",
            Self::Desktop => "Open the Claude desktop app to sign in again.",
        }
    }
}

impl Credentials {
    fn is_expired(&self) -> bool {
        self.expires_at
            .is_some_and(|expires_at| expires_at <= Utc::now())
    }
}

fn credentials_path() -> Option<PathBuf> {
    BaseDirs::new().map(|directories| {
        directories
            .home_dir()
            .join(".claude")
            .join(".credentials.json")
    })
}

/// Collects every local Claude session and prefers a live one. Both sources
/// yield byte-identical usage data, so ordering only decides whose expiry we
/// follow: the desktop app refreshes its token whenever it is open, while the
/// CLI refreshes only when `claude` runs, so the desktop session goes stale
/// less often. A stale session on either side never masks a live one.
///
/// When every session has expired the CLI one is returned, because only it
/// can be refreshed without opening an app; the caller reports expiry.
fn load_credentials() -> Result<Credentials> {
    let attempts = [load_desktop_credentials(), load_cli_credentials()];
    let mut errors = Vec::new();
    let mut expired = None;
    for attempt in attempts {
        match attempt {
            Ok(credentials) if !credentials.is_expired() => return Ok(credentials),
            Ok(credentials) => expired = Some(credentials),
            Err(error) => errors.push(format!("{error:#}")),
        }
    }
    if let Some(credentials) = expired {
        return Ok(credentials);
    }
    bail!(
        "no Claude login found. Install Claude Code or the Claude desktop app and sign in first ({})",
        errors.join("; ")
    )
}

fn load_cli_credentials() -> Result<Credentials> {
    let path = credentials_path()
        .context("could not resolve the home directory for Claude credentials")?;
    load_cli_credentials_at(&path)
}

fn load_cli_credentials_at(path: &Path) -> Result<Credentials> {
    let path = path.to_path_buf();
    let contents = fs::read(&path).with_context(|| {
        format!(
            "read {} (install Claude Code and sign in first)",
            path.display()
        )
    })?;
    let file: CredentialFile =
        serde_json::from_slice(&contents).with_context(|| format!("parse {}", path.display()))?;
    let oauth = file.oauth.context(
        "Claude credentials do not contain a Claude OAuth session; run `claude` to sign in",
    )?;
    let access_token = oauth.access_token.trim().to_owned();
    anyhow::ensure!(
        !access_token.is_empty(),
        "Claude OAuth access token is empty"
    );
    let expires_at = oauth
        .expires_at_millis
        .and_then(DateTime::from_timestamp_millis);
    Ok(Credentials {
        access_token,
        expires_at,
        subscription_type: oauth.subscription_type,
        rate_limit_tier: oauth.rate_limit_tier,
        source: CredentialSource::Cli,
    })
}

fn load_desktop_credentials() -> Result<Credentials> {
    let session = claude_desktop::load_session()?;
    Ok(Credentials {
        access_token: session.access_token,
        expires_at: session.expires_at,
        subscription_type: session.subscription_type,
        rate_limit_tier: session.rate_limit_tier,
        source: CredentialSource::Desktop,
    })
}

#[derive(Deserialize)]
struct OAuthUsageResponse {
    five_hour: Option<OAuthUsageWindow>,
    seven_day: Option<OAuthUsageWindow>,
    organization_name: Option<String>,
    #[serde(default)]
    limits: Vec<OAuthLimitEntry>,
    /// Banked usage-limit resets. Kept out of `additional_windows` because it
    /// is not a quota window.
    cedar_ember: Option<OAuthBankedResets>,
    /// Claude regularly adds model- and feature-specific quota windows (for
    /// example `seven_day_fable`). Keep every window-shaped field instead of
    /// silently throwing newer limits away.
    #[serde(flatten)]
    additional_windows: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
struct OAuthProfileResponse {
    account: Option<OAuthProfileAccount>,
    organization: Option<OAuthProfileOrganization>,
}

#[derive(Deserialize)]
struct OAuthProfileAccount {
    full_name: Option<String>,
    display_name: Option<String>,
    email: Option<String>,
    has_claude_max: Option<bool>,
    has_claude_pro: Option<bool>,
}

#[derive(Deserialize)]
struct OAuthProfileOrganization {
    name: Option<String>,
    organization_type: Option<String>,
    seat_tier: Option<String>,
    rate_limit_tier: Option<String>,
}

#[derive(Deserialize)]
struct OAuthLimitEntry {
    kind: Option<String>,
    group: Option<String>,
    percent: Option<f64>,
    resets_at: Option<String>,
    scope: Option<OAuthLimitScope>,
}

#[derive(Deserialize)]
struct OAuthLimitScope {
    model: Option<OAuthLimitScopeModel>,
}

#[derive(Deserialize)]
struct OAuthLimitScopeModel {
    id: Option<String>,
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct OAuthBankedResets {
    #[serde(default)]
    eligible: bool,
    #[serde(default)]
    grants: Vec<OAuthResetGrant>,
}

#[derive(Deserialize)]
struct OAuthResetGrant {
    id: Option<String>,
    label: Option<String>,
    #[serde(default)]
    resets_left: u32,
    starts_at: Option<String>,
    ends_at: Option<String>,
    #[serde(default)]
    clears: Vec<String>,
    #[serde(default)]
    paused: bool,
}

#[derive(Clone, Deserialize)]
struct OAuthUsageWindow {
    utilization: Option<f64>,
    resets_at: Option<String>,
    #[serde(default, alias = "windowDurationMins", alias = "window_duration_mins")]
    duration_minutes: Option<u32>,
}

pub fn parse_usage_response(response: &str, sampled_at: DateTime<Utc>) -> Result<RateLimits> {
    let response: OAuthUsageResponse =
        serde_json::from_str(response).context("parse Claude OAuth usage")?;
    let primary = parse_window(response.five_hour, Some(5 * 60));
    let secondary = parse_window(response.seven_day, Some(7 * 24 * 60));
    let mut additional_limits = response
        .additional_windows
        .into_iter()
        .filter_map(|(id, value)| {
            let window = serde_json::from_value::<OAuthUsageWindow>(value).ok()?;
            let window = parse_window(Some(window), inferred_duration_minutes(&id));
            (!window.is_empty()).then(|| AdditionalLimit {
                title: additional_limit_title(&id),
                id,
                window,
            })
        })
        .collect::<Vec<_>>();
    additional_limits.extend(scoped_weekly_limits(response.limits));
    additional_limits.sort_by(|left, right| left.id.cmp(&right.id));
    additional_limits.dedup_by(|left, right| left.id == right.id);
    anyhow::ensure!(
        !primary.is_empty() || !secondary.is_empty() || !additional_limits.is_empty(),
        "Claude OAuth response does not contain usage windows"
    );
    Ok(RateLimits {
        primary,
        secondary,
        additional_limits,
        sampled_at,
        account_name: non_empty(response.organization_name),
        reset_credits: response
            .cedar_ember
            .and_then(|banked| banked_resets(banked, sampled_at)),
        // The OAuth usage payload does not contain a subscription tier. Do
        // not present the provider name as if it were a plan.
        plan_type: None,
        ..RateLimits::default()
    }
    .normalized(sampled_at))
}

/// Maps Claude's reset grants onto the shared banked-reset summary. Each grant
/// becomes one credit row; the count is the resets still left across grants
/// that have not expired. Paused grants still count, as they stay banked.
fn banked_resets(
    banked: OAuthBankedResets,
    now: DateTime<Utc>,
) -> Option<RateLimitResetCreditsSummary> {
    if !banked.eligible {
        return None;
    }
    let credits = banked
        .grants
        .into_iter()
        .filter(|grant| grant.resets_left > 0)
        .filter_map(|grant| {
            let expires_at = parse_timestamp(grant.ends_at.as_deref());
            if expires_at.is_some_and(|expires_at| expires_at <= now) {
                return None;
            }
            Some((
                grant.resets_left,
                RateLimitResetCredit {
                    reset_type: (!grant.clears.is_empty()).then(|| grant.clears.join(",")),
                    status: if grant.paused { "paused" } else { "available" }.to_owned(),
                    granted_at: parse_timestamp(grant.starts_at.as_deref()),
                    expires_at,
                    title: non_empty(grant.label),
                    description: non_empty(grant.id),
                },
            ))
        })
        .collect::<Vec<_>>();
    Some(RateLimitResetCreditsSummary {
        available_count: credits.iter().map(|(count, _)| count).sum(),
        credits: credits.into_iter().map(|(_, credit)| credit).collect(),
    })
}

fn parse_timestamp(value: Option<&str>) -> Option<DateTime<Utc>> {
    value
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
}

/// Identity and plan both come from the profile endpoint. The former
/// `account/settings` endpoint no longer reports `subscriptionType` or
/// `rateLimitTier` at all, so it is not consulted.
struct AccountProfile {
    account_name: Option<String>,
    plan_type: Option<String>,
}

fn fetch_profile(agent: &ureq::Agent, access_token: &str) -> Result<AccountProfile> {
    let response = agent
        .get(OAUTH_PROFILE_URL)
        .set("Authorization", &format!("Bearer {access_token}"))
        .set("Accept", "application/json")
        .set("anthropic-beta", OAUTH_BETA)
        .set(
            "User-Agent",
            &format!("claude-code/{FALLBACK_CLAUDE_CODE_VERSION}"),
        )
        .call()
        .context("request Claude OAuth profile")?;
    let body = response
        .into_string()
        .context("read Claude OAuth profile response")?;
    parse_profile(&body)
}

fn parse_profile(response: &str) -> Result<AccountProfile> {
    let profile: OAuthProfileResponse =
        serde_json::from_str(response).context("parse Claude OAuth profile")?;
    let (person_name, email, has_max, has_pro) =
        profile
            .account
            .map_or((None, None, false, false), |account| {
                (
                    non_empty(account.full_name).or_else(|| non_empty(account.display_name)),
                    non_empty(account.email),
                    account.has_claude_max.unwrap_or_default(),
                    account.has_claude_pro.unwrap_or_default(),
                )
            });
    let (organization_name, plan_type) =
        profile.organization.map_or((None, None), |organization| {
            (
                non_empty(organization.name),
                // A seat or organization type names the plan directly; the
                // rate-limit tier is the weakest signal and usually opaque
                // (`default_raven`), so it only fills a gap or adds the Max
                // multiplier to a matching plan.
                plan_type_from_account_fields(
                    plan_type_from_tier(organization.seat_tier)
                        .or_else(|| plan_type_from_tier(organization.organization_type)),
                    organization.rate_limit_tier,
                ),
            )
        });
    Ok(AccountProfile {
        account_name: person_name.or(organization_name).or(email),
        plan_type: plan_type.or_else(|| {
            // Personal entitlement flags only matter when no organization
            // named a plan, so a team seat is not relabelled by them.
            has_max
                .then(|| "max".to_owned())
                .or_else(|| has_pro.then(|| "pro".to_owned()))
        }),
    })
}

/// Shared by both credential files, which report the same two fields.
fn plan_type_from_account_fields(
    subscription_type: Option<String>,
    rate_limit_tier: Option<String>,
) -> Option<String> {
    let from_tier = plan_type_from_tier(rate_limit_tier);
    match (non_empty(subscription_type), from_tier) {
        // The tier agrees with the subscription and adds the Max multiplier.
        (Some(subscription), Some(tier)) if tier.starts_with(&subscription) => Some(tier),
        (subscription, tier) => subscription.or(tier),
    }
}

/// Infers a visible subscription tier from an internal tier identifier such as
/// `team_standard` or `claude_team`. Only unambiguous identifiers count;
/// generic values like `default_raven` stay intentionally absent from the UI.
fn plan_type_from_tier(tier: Option<String>) -> Option<String> {
    non_empty(tier).and_then(|tier| {
        let tier = tier.to_ascii_lowercase();
        let parts = tier
            .split(|character: char| !character.is_alphanumeric())
            .collect::<Vec<_>>();
        let plan = ["enterprise", "team", "max", "pro"]
            .into_iter()
            .find(|plan| parts.contains(plan))?;
        // Max tiers carry their usage multiplier: `default_claude_max_20x`.
        let multiplier = parts
            .iter()
            .find(|part| plan == "max" && matches!(**part, "5x" | "20x"));
        Some(match multiplier {
            Some(multiplier) => format!("{plan} {multiplier}"),
            None => plan.to_owned(),
        })
    })
}

/// The OAuth endpoint's current shape puts promotional/model-only weekly
/// quotas in `limits[]`. A Fable limit, for example, is a `weekly_scoped`
/// entry with its visible name at `scope.model.display_name`.
fn scoped_weekly_limits(limits: Vec<OAuthLimitEntry>) -> Vec<AdditionalLimit> {
    let mut seen_ids = BTreeSet::new();
    limits
        .into_iter()
        .filter_map(|limit| {
            if limit.kind.as_deref() != Some("weekly_scoped")
                || limit.group.as_deref() != Some("weekly")
            {
                return None;
            }
            let model = limit.scope?.model?;
            let title = non_empty(model.display_name)?;
            if title.eq_ignore_ascii_case("all models") {
                return None;
            }
            let identity = non_empty(model.id).unwrap_or_else(|| title.clone());
            let identity_slug = limit_slug(&identity);
            if identity_slug == "all-models" || identity_slug.ends_with("-all-models") {
                return None;
            }
            let id = format!("claude-weekly-scoped-{identity_slug}");
            if identity_slug.is_empty() || !seen_ids.insert(id.clone()) {
                return None;
            }
            let used_percent = limit
                .percent
                .filter(|value| value.is_finite())
                .map(|value| value.round().clamp(0.0, 100.0) as u8);
            let resets_at = limit
                .resets_at
                .as_deref()
                .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
                .map(|value| value.with_timezone(&Utc));
            let window = LimitWindow {
                used_percent,
                resets_at,
                duration_minutes: Some(7 * 24 * 60),
            };
            (!window.is_empty()).then(|| AdditionalLimit {
                id,
                title: format!("{title} only"),
                window,
            })
        })
        .collect()
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

fn limit_slug(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for character in value.chars() {
        if character.is_alphanumeric() {
            slug.extend(character.to_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }
    slug.trim_matches('-').to_owned()
}

fn parse_window(window: Option<OAuthUsageWindow>, duration_minutes: Option<u32>) -> LimitWindow {
    let Some(window) = window else {
        return LimitWindow::default();
    };
    LimitWindow {
        used_percent: window
            .utilization
            .filter(|value| value.is_finite())
            .map(|value| value.round().clamp(0.0, 100.0) as u8),
        resets_at: window
            .resets_at
            .as_deref()
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc)),
        duration_minutes: window.duration_minutes.or(duration_minutes),
    }
}

fn inferred_duration_minutes(id: &str) -> Option<u32> {
    match id {
        name if name.starts_with("five_hour") => Some(5 * 60),
        name if name.starts_with("seven_day") => Some(7 * 24 * 60),
        name if name.starts_with("monthly") => Some(30 * 24 * 60),
        _ => None,
    }
}

fn additional_limit_title(id: &str) -> String {
    let name = id
        .strip_prefix("seven_day_")
        .or_else(|| id.strip_prefix("five_hour_"))
        .or_else(|| id.strip_prefix("monthly_"))
        .unwrap_or(id);
    name.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            let Some(first) = characters.next() else {
                return String::new();
            };
            format!(
                "{}{}",
                first.to_uppercase(),
                characters.as_str().to_lowercase()
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn finds_native_cli_when_path_is_missing_and_honors_overrides() {
        let root = tempfile::tempdir().unwrap();
        let native = root.path().join(".local/bin");
        let on_path = root.path().join("path");
        let explicit = root.path().join("custom");
        #[cfg(windows)]
        let name = "claude.exe";
        #[cfg(not(windows))]
        let name = "claude";
        for directory in [&native, &on_path, &explicit] {
            fs::create_dir_all(directory).unwrap();
            fs::write(directory.join(name), b"fixture, never executed").unwrap();
        }
        let common = std::slice::from_ref(&native);
        assert_eq!(find_cli(None, &[], common), Some(native.join(name)));
        assert_eq!(
            find_cli(None, std::slice::from_ref(&on_path), common),
            Some(on_path.join(name))
        );
        assert_eq!(
            find_cli(Some(&explicit), std::slice::from_ref(&on_path), common),
            Some(explicit.join(name))
        );
        fs::remove_file(native.join(name)).unwrap();
        assert_eq!(find_cli(None, &[], common), None);
    }

    #[test]
    fn parses_oauth_session_and_weekly_windows() {
        let sampled_at = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();
        let limits = parse_usage_response(
            r#"{"five_hour":{"utilization":12.5,"resets_at":"2026-07-15T15:00:00.000Z"},"seven_day":{"utilization":30,"resets_at":"2026-07-21T00:00:00.000Z"}}"#,
            sampled_at,
        )
        .unwrap();
        assert_eq!(limits.primary.used_percent, Some(13));
        assert_eq!(limits.primary.duration_minutes, Some(300));
        assert_eq!(limits.secondary.used_percent, Some(30));
        assert_eq!(limits.secondary.duration_minutes, Some(10_080));
    }

    #[test]
    fn accepts_a_weekly_only_oauth_response() {
        let limits =
            parse_usage_response(r#"{"seven_day":{"utilization":42}}"#, Utc::now()).unwrap();
        assert!(limits.primary.is_empty());
        assert_eq!(limits.secondary.used_percent, Some(42));
    }

    #[test]
    fn profile_name_falls_back_to_organization_then_email() {
        let name = parse_profile(
            r#"{"account":{"full_name":"Ada Lovelace","email":"ada@example.com"},"organization":{"name":"Example Studio"}}"#,
        )
        .unwrap();
        assert_eq!(name.account_name.as_deref(), Some("Ada Lovelace"));

        let organization_name = parse_profile(
            r#"{"account":{"full_name":" ","email":"ada@example.com"},"organization":{"name":"Example Studio"}}"#,
        )
        .unwrap();
        assert_eq!(
            organization_name.account_name.as_deref(),
            Some("Example Studio")
        );

        let email =
            parse_profile(r#"{"account":{"email":"ada@example.com"},"organization":{}}"#).unwrap();
        assert_eq!(email.account_name.as_deref(), Some("ada@example.com"));
    }

    #[test]
    fn profile_plan_prefers_the_seat_tier_over_personal_entitlements() {
        // Shape observed live: a team seat whose rate-limit tier is opaque.
        let team = parse_profile(
            r#"{"account":{"full_name":"Ada","has_claude_max":false,"has_claude_pro":false},"organization":{"name":"Example","organization_type":"claude_team","seat_tier":"team_standard","rate_limit_tier":"default_raven"}}"#,
        )
        .unwrap();
        assert_eq!(team.plan_type.as_deref(), Some("team"));

        // A team seat is not relabelled by a personal Max entitlement.
        let both = parse_profile(
            r#"{"account":{"has_claude_max":true},"organization":{"seat_tier":"team_standard"}}"#,
        )
        .unwrap();
        assert_eq!(both.plan_type.as_deref(), Some("team"));

        let personal = parse_profile(
            r#"{"account":{"has_claude_max":true,"has_claude_pro":false},"organization":{"rate_limit_tier":"default_raven"}}"#,
        )
        .unwrap();
        assert_eq!(personal.plan_type.as_deref(), Some("max"));

        let pro =
            parse_profile(r#"{"account":{"has_claude_pro":true},"organization":{}}"#).unwrap();
        assert_eq!(pro.plan_type.as_deref(), Some("pro"));

        let unknown =
            parse_profile(r#"{"account":{},"organization":{"rate_limit_tier":"default"}}"#)
                .unwrap();
        assert_eq!(unknown.plan_type, None);
    }

    #[test]
    fn credential_files_prefer_the_explicit_subscription_type() {
        assert_eq!(
            plan_type_from_account_fields(
                Some("pro".into()),
                Some("default_claude_max_20x".into())
            )
            .as_deref(),
            Some("pro")
        );
        assert_eq!(
            plan_type_from_account_fields(None, Some("default_claude_max_20x".into())).as_deref(),
            Some("max 20x")
        );
        // A matching tier adds the multiplier to the explicit subscription.
        assert_eq!(
            plan_type_from_account_fields(Some("max".into()), Some("default_claude_max_5x".into()))
                .as_deref(),
            Some("max 5x")
        );
        assert_eq!(
            plan_type_from_account_fields(None, Some("default".into())),
            None
        );
        // The live value on a team account carries no usable tier keyword.
        assert_eq!(
            plan_type_from_account_fields(None, Some("default_raven".into())),
            None
        );
    }

    #[test]
    fn cli_credentials_carry_the_plan_alongside_the_token() {
        let file: CredentialFile = serde_json::from_str(
            r#"{"claudeAiOauth":{"accessToken":"token","expiresAt":1785183619245,"subscriptionType":"team","rateLimitTier":"default_raven"}}"#,
        )
        .unwrap();
        let oauth = file.oauth.unwrap();
        assert_eq!(oauth.subscription_type.as_deref(), Some("team"));
        assert_eq!(oauth.rate_limit_tier.as_deref(), Some("default_raven"));
    }

    #[test]
    fn account_cache_refreshes_after_30_minutes_or_a_reset_change() {
        let schedule = vec![("primary".into(), None), ("secondary".into(), None)];
        let mut cache = ClaudeAccountCache::default();
        assert!(cache.needs_refresh(&schedule));

        cache.record(
            Some("Ada Lovelace".into()),
            Some("pro".into()),
            schedule.clone(),
        );
        assert!(!cache.needs_refresh(&schedule));
        assert_eq!(cache.plan_type.as_deref(), Some("pro"));

        let changed_schedule = vec![
            ("primary".into(), None),
            ("secondary".into(), Some(Utc::now())),
        ];
        assert!(cache.needs_refresh(&changed_schedule));

        // Instant::now() - 30min panics on Windows when uptime is shorter
        // (GitHub Actions runners). Skip the elapsed path in that case; the
        // reset-schedule change above already covers needs_refresh == true.
        if let Some(stale) = Instant::now().checked_sub(PROFILE_REFRESH_INTERVAL) {
            cache.checked_at = Some(stale);
            assert!(cache.needs_refresh(&schedule));
        }
    }

    #[test]
    fn preserves_every_additional_claude_limit_including_fable() {
        let limits = parse_usage_response(
            r#"{"five_hour":{"utilization":12},"seven_day":{"utilization":30},"seven_day_opus":{"utilization":7},"limits":[{"kind":"weekly_scoped","group":"weekly","percent":42,"resets_at":"2026-07-21T00:00:00.000Z","scope":{"model":{"id":"claude/fable.5:promo","display_name":"Fable"}}},{"kind":"weekly_scoped","group":"weekly","percent":30,"scope":{"model":{"display_name":"All models"}}}],"organization_name":"example"}"#,
            Utc::now(),
        )
        .unwrap();

        assert_eq!(limits.additional_limits.len(), 2);
        assert_eq!(
            limits.additional_limits[0].id,
            "claude-weekly-scoped-claude-fable-5-promo"
        );
        assert_eq!(limits.additional_limits[0].title, "Fable only");
        assert_eq!(limits.additional_limits[0].window.used_percent, Some(42));
        assert_eq!(
            limits.additional_limits[0].window.duration_minutes,
            Some(10_080)
        );
        assert_eq!(limits.additional_limits[1].title, "Opus");
        assert_eq!(limits.account_name.as_deref(), Some("example"));
        assert_eq!(limits.plan_type, None);
    }

    #[test]
    fn parses_banked_resets_from_cedar_ember() {
        let sampled_at = Utc.with_ymd_and_hms(2026, 9, 22, 18, 0, 0).unwrap();
        let limits = parse_usage_response(
            r#"{"five_hour":{"utilization":1},"seven_day":{"utilization":60},"cedar_ember":{"eligible":true,"ineligible_reason":null,"at_limit":false,"exhausted":[],"grants":[{"id":"opus55-launch-team-20260921","label":"Claude Opus 5.5 launch: one usage-limit reset for Team members","resets_total":1,"resets_left":1,"starts_at":"2026-09-22T16:00:00+00:00","ends_at":"2026-10-22T16:00:00+00:00","clears":["five_hour","seven_day","seven_day_overage_included"],"paused":false,"usable_now":true,"use_requires_limit":false,"percent_used":{"five_hour":1,"seven_day":60},"blocking":[],"arm":null},{"id":"spent","resets_left":0,"ends_at":"2026-10-01T00:00:00Z"},{"id":"expired","resets_left":2,"ends_at":"2026-09-01T00:00:00Z"}],"next_grant_id":"opus55-launch-team-20260921","weekly_resets_at":"2026-09-25T14:00:00+00:00","cooldown_until":null}}"#,
            sampled_at,
        )
        .unwrap();

        assert!(limits.additional_limits.is_empty());
        assert_eq!(limits.available_reset_count(), 1);
        assert_eq!(
            limits.next_reset_credit_expiration(),
            Some(Utc.with_ymd_and_hms(2026, 10, 22, 16, 0, 0).unwrap())
        );
        let credit = &limits.reset_credits.as_ref().unwrap().credits[0];
        assert_eq!(
            credit.title.as_deref(),
            Some("Claude Opus 5.5 launch: one usage-limit reset for Team members")
        );
    }

    #[test]
    fn ineligible_surface_reports_no_banked_resets() {
        let limits = parse_usage_response(
            r#"{"five_hour":{"utilization":1},"cedar_ember":{"eligible":false,"ineligible_reason":"surface","grants":[]}}"#,
            Utc::now(),
        )
        .unwrap();
        assert!(limits.reset_credits.is_none());
        assert_eq!(limits.available_reset_count(), 0);
    }

    #[test]
    fn activation_uses_the_minimal_haiku_command() {
        let command = activation_command_for(PathBuf::from("claude"));
        assert_eq!(command.get_program().to_string_lossy(), "claude");
        assert_eq!(
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            [
                "-p",
                "reply with letter a",
                "--model",
                "haiku",
                "--effort=low",
            ]
        );
    }

    #[test]
    fn pasted_credentials_are_routed_by_their_shape() {
        let parse = |raw| Credential::parse(raw).unwrap();
        assert_eq!(
            parse(" Bearer sk-ant-admin01-x "),
            Credential::AdminKey("sk-ant-admin01-x".into())
        );
        assert_eq!(
            parse("sk-ant-oat01-x"),
            Credential::OAuth("sk-ant-oat01-x".into())
        );
        // A bare value is a sessionKey; anything cookie-shaped is sent as is.
        assert_eq!(
            parse("sk-ant-sid01-x"),
            Credential::Cookie("sessionKey=sk-ant-sid01-x".into())
        );
        assert_eq!(
            parse("Cookie: sessionKey=abc; other=1"),
            Credential::Cookie("sessionKey=abc; other=1".into())
        );
        // A standard API key would otherwise be sent to claude.ai as a cookie.
        assert!(Credential::parse("sk-ant-api03-x").is_err());
        assert!(Credential::parse("  ").is_err());
    }

    #[test]
    fn subscription_setup_validates_the_selected_method_before_fetching() {
        use ProfileCredentialMethod::{BrowserSession, OAuthToken};
        assert!(BrowserSession.validate("sk-ant-sid01-fixture").is_ok());
        assert!(
            BrowserSession
                .validate("Cookie: other=1; sessionKey=fixture")
                .is_ok()
        );
        assert!(OAuthToken.validate("Bearer sk-ant-oat01-fixture").is_ok());
        assert!(BrowserSession.validate("sk-ant-oat01-fixture").is_err());
        assert!(OAuthToken.validate("sessionKey=fixture").is_err());
        assert!(BrowserSession.validate("other=1").is_err());
        assert!(BrowserSession.validate("sessionKey=").is_err());
        for method in [BrowserSession, OAuthToken] {
            assert!(method.validate("sk-ant-admin01-fixture").is_err());
            assert!(method.validate("sk-ant-api03-fixture").is_err());
            assert!(method.validate(" ").is_err());
        }
    }

    #[test]
    fn renaming_the_only_default_profile_promotes_its_live_sample_immediately() {
        let mut limits = RateLimits {
            primary: LimitWindow {
                used_percent: Some(42),
                ..Default::default()
            },
            account_name: Some("Service name".into()),
            ..Default::default()
        };
        assert!(!apply_profile_names(&mut limits, &Settings::default()));
        let mut settings = Settings::default();
        settings.claude_profiles = profiles_for_settings(&settings);
        settings.claude_profiles[0].name = "Personal".into();
        assert!(apply_profile_names(&mut limits, &settings));
        let profile = &limits.claude_profiles[0];
        assert_eq!(profile.name, "Personal");
        assert_eq!(profile.limits.account_name.as_deref(), Some("Personal"));
        assert_eq!(profile.limits.primary.used_percent, Some(42));
        assert!(profile.limits.claude_profiles.is_empty());
        assert!(!apply_profile_names(&mut limits, &settings));
        settings.claude_profiles[0].name = "Work".into();
        assert!(apply_profile_names(&mut limits, &settings));
        assert_eq!(
            limits.claude_profiles[0].limits.account_name.as_deref(),
            Some("Work")
        );
    }

    #[test]
    fn one_manual_profile_keeps_its_identity_when_default_is_disabled() {
        let profile = ClaudeProfile {
            id: "work".into(),
            name: "Work".into(),
            enabled: true,
        };
        let sample = RateLimits {
            primary: LimitWindow {
                used_percent: Some(20),
                ..Default::default()
            },
            account_name: Some("Organization".into()),
            ..Default::default()
        };
        let mut limits =
            merge_profiles(vec![(profile.clone(), Ok(sample))], &[], Utc::now()).unwrap();
        assert_eq!(limits.claude_profiles.len(), 1);
        assert_eq!(limits.claude_profiles[0].id, "work");
        assert_eq!(
            limits.claude_profiles[0].limits.account_name.as_deref(),
            Some("Work")
        );
        let mut settings = Settings::default();
        settings.claude_profiles = profiles_for_settings(&settings);
        settings.claude_profiles[0].enabled = false;
        settings.claude_profiles.push(ClaudeProfile {
            name: "Renamed".into(),
            ..profile
        });
        assert!(apply_profile_names(&mut limits, &settings));
        assert_eq!(limits.claude_profiles[0].name, "Renamed");
        assert_eq!(
            limits.claude_profiles[0].limits.primary.used_percent,
            Some(20)
        );
    }

    #[test]
    fn adding_profile_and_restarting_reader_keeps_default_sample_on_429() {
        let previous = Settings::default();
        let mut next = previous.clone();
        next.claude_profiles = profiles_for_settings(&previous);
        let added = ClaudeProfile::new("Account 1");
        next.claude_profiles.push(added.clone());
        let sample = RateLimits {
            primary: LimitWindow {
                used_percent: Some(0),
                ..Default::default()
            },
            secondary: LimitWindow {
                used_percent: Some(3),
                ..Default::default()
            },
            sampled_at: Utc::now() - chrono::Duration::minutes(1),
            ..Default::default()
        };
        let visible = prepare_profile_refresh(&sample, &previous, &next);
        let mut restarted =
            ClaudeClient::with_profiles(profiles_for_settings(&next)).with_cached_limits(&visible);
        let merged = restarted
            .merge_profile_results(vec![
                (
                    profiles_for_settings(&next)[0].clone(),
                    Err(crate::worker::rate_limit_error("fixture 429")),
                ),
                (added, Ok(sample.clone())),
            ])
            .unwrap();
        let default = merged
            .claude_profiles
            .iter()
            .find(|profile| profile.id == ClaudeProfile::DEFAULT_ID)
            .unwrap();
        assert_eq!(default.limits.primary.used_percent, Some(0));
        assert_eq!(default.limits.secondary.used_percent, Some(3));
        assert_eq!(default.limits.sampled_at, sample.sampled_at);
        assert!(default.error.is_some());
        assert!(restarted.take_rate_limit_response());
        assert!(!restarted.take_rate_limit_response());
    }

    #[test]
    fn profile_refresh_discards_only_disabled_removed_and_replaced_credentials() {
        let mut previous = Settings::default();
        let profile = |id: &str| ClaudeProfile {
            id: id.into(),
            name: id.into(),
            enabled: true,
        };
        previous.claude_profiles = vec![
            profile("default"),
            profile("changed"),
            profile("disabled"),
            profile("removed"),
            profile("untouched"),
        ];
        let sample = RateLimits {
            primary: LimitWindow {
                used_percent: Some(25),
                ..Default::default()
            },
            sampled_at: Utc::now(),
            ..Default::default()
        };
        let limits = merge_profiles(
            previous
                .claude_profiles
                .iter()
                .cloned()
                .map(|profile| (profile, Ok(sample.clone())))
                .collect(),
            &[],
            Utc::now(),
        )
        .unwrap();
        let mut next = previous.clone();
        next.claude_profiles
            .retain(|profile| profile.id != "removed");
        next.claude_profiles
            .iter_mut()
            .find(|profile| profile.id == "disabled")
            .unwrap()
            .enabled = false;
        next.claude_profiles
            .iter_mut()
            .find(|profile| profile.id == "untouched")
            .unwrap()
            .name = "Renamed".into();
        next.claude_profile_credential_revisions
            .insert("changed".into(), 1);
        next.claude_credentials_revision += 1;
        let retained = prepare_profile_refresh(&limits, &previous, &next);
        assert_eq!(retained.claude_profiles.len(), 3);
        for id in ["default", "untouched"] {
            let cached = retained
                .claude_profiles
                .iter()
                .find(|profile| profile.id == id)
                .unwrap();
            assert_eq!(cached.limits.primary.used_percent, Some(25));
            assert_eq!(cached.limits.sampled_at, sample.sampled_at);
        }
        let changed = retained
            .claude_profiles
            .iter()
            .find(|profile| profile.id == "changed")
            .unwrap();
        assert_eq!(changed.limits.primary.used_percent, None);
        assert_eq!(changed.limits.sampled_at.timestamp(), 0);
        assert_eq!(
            retained
                .claude_profiles
                .iter()
                .find(|profile| profile.id == "untouched")
                .unwrap()
                .name,
            "Renamed"
        );
    }

    #[test]
    fn legacy_default_sample_never_moves_to_a_new_first_profile() {
        let previous = Settings::default();
        let mut next = previous.clone();
        next.claude_profiles = vec![
            ClaudeProfile::new("First"),
            profiles_for_settings(&previous)[0].clone(),
        ];
        let sample = RateLimits {
            primary: LimitWindow {
                used_percent: Some(42),
                ..Default::default()
            },
            ..Default::default()
        };
        let retained = prepare_profile_refresh(&sample, &previous, &next);
        assert_eq!(
            retained.claude_profiles[0].limits.primary.used_percent,
            None
        );
        assert_eq!(retained.claude_profiles[1].id, "default");
        assert_eq!(
            retained.claude_profiles[1].limits.primary.used_percent,
            Some(42)
        );
    }

    #[test]
    fn failed_first_profile_without_cache_uses_a_successful_profile_for_provider_quota() {
        let default = profiles_for_settings(&Settings::default())[0].clone();
        let work = ClaudeProfile::new("Work");
        let sample = RateLimits {
            primary: LimitWindow {
                used_percent: Some(25),
                ..Default::default()
            },
            sampled_at: Utc::now(),
            account_name: Some("Work identity".into()),
            ..Default::default()
        };
        let merged = merge_profiles(
            vec![
                (default, Err(anyhow::anyhow!("fixture failure"))),
                (work, Ok(sample)),
            ],
            &[],
            Utc::now(),
        )
        .unwrap();
        assert_eq!(merged.primary.used_percent, Some(25));
        assert_eq!(merged.account_name.as_deref(), Some("Work identity"));
        assert!(merged.claude_profiles[0].error.is_some());
        assert_eq!(merged.claude_profiles[0].limits.primary.used_percent, None);
    }

    #[test]
    fn failed_first_profile_with_cache_keeps_its_provider_quota() {
        let default = profiles_for_settings(&Settings::default())[0].clone();
        let sample = |percent| RateLimits {
            primary: LimitWindow {
                used_percent: Some(percent),
                ..Default::default()
            },
            sampled_at: Utc::now(),
            ..Default::default()
        };
        let previous = vec![ClaudeProfileSnapshot {
            id: default.id.clone(),
            name: default.name.clone(),
            limits: sample(10),
            error: None,
        }];
        let merged = merge_profiles(
            vec![
                (default, Err(anyhow::anyhow!("fixture failure"))),
                (ClaudeProfile::new("Work"), Ok(sample(40))),
            ],
            &previous,
            Utc::now(),
        )
        .unwrap();
        assert_eq!(merged.primary.used_percent, Some(10));
        assert_eq!(
            merged.claude_profiles[1].limits.primary.used_percent,
            Some(40)
        );
    }

    #[test]
    fn a_failing_profile_keeps_its_previous_limits_and_reports_the_error() {
        let profile = |id: &str, name: &str| ClaudeProfile {
            id: id.into(),
            name: name.into(),
            enabled: true,
        };
        let limits = |percent| RateLimits {
            primary: LimitWindow {
                used_percent: Some(percent),
                ..LimitWindow::default()
            },
            account_name: Some("Real Name".into()),
            ..RateLimits::default()
        };
        let now = Utc::now();
        let first = merge_profiles(
            vec![
                (profile("default", "Default"), Ok(limits(10))),
                (profile("work", "Work"), Ok(limits(40))),
            ],
            &[],
            now,
        )
        .unwrap();
        // The provider-level fields describe the first profile, unrenamed.
        assert_eq!(first.primary.used_percent, Some(10));
        assert_eq!(first.account_name.as_deref(), Some("Real Name"));

        let second = merge_profiles(
            vec![
                (profile("default", "Default"), Ok(limits(20))),
                (
                    profile("work", "Work"),
                    Err(anyhow::anyhow!("session expired")),
                ),
            ],
            &first.claude_profiles,
            now,
        )
        .unwrap();
        let [default, work] = second.claude_profiles.as_slice() else {
            panic!("expected both profiles");
        };
        assert_eq!(default.limits.primary.used_percent, Some(20));
        assert_eq!(default.error, None);
        assert_eq!(work.limits.primary.used_percent, Some(40));
        assert_eq!(work.limits.account_name.as_deref(), Some("Work"));
        assert_eq!(work.error.as_deref(), Some("session expired"));

        // With nothing readable the provider fails, and a rate limit wins so
        // the worker pauses polling.
        let error = merge_profiles(
            vec![
                (
                    profile("default", "Default"),
                    Err(anyhow::anyhow!("session expired")),
                ),
                (
                    profile("work", "Work"),
                    Err(crate::worker::rate_limit_error("slow down")),
                ),
            ],
            &second.claude_profiles,
            now,
        )
        .unwrap_err();
        assert!(crate::worker::is_rate_limited_error(&error));
    }

    #[test]
    fn web_usage_reads_the_organization_with_chat_access() {
        let organization = pick_organization(
            r#"[{"uuid":"api","name":"API","capabilities":["api"]},{"uuid":"chat","name":"Team","capabilities":["chat","claude_max"]}]"#,
        )
        .unwrap();
        assert_eq!(organization.uuid, "chat");
        assert!(pick_organization("[]").is_err());

        let plan = |json: &str| pick_organization(json).unwrap().plan_type();
        assert_eq!(
            plan(r#"[{"uuid":"a","rate_limit_tier":"default_claude_max_5x"}]"#).as_deref(),
            Some("max 5x")
        );
        assert_eq!(
            plan(r#"[{"uuid":"a","rate_limit_tier":"default_claude_ai","capabilities":["chat","claude_pro"]}]"#)
                .as_deref(),
            Some("pro")
        );
        assert_eq!(
            plan(r#"[{"uuid":"a","rate_limit_tier":"default_claude_ai","billing_type":"stripe_subscription"}]"#)
                .as_deref(),
            Some("pro")
        );
        assert_eq!(
            plan(r#"[{"uuid":"a","rate_limit_tier":"default_claude_ai"}]"#),
            None
        );
    }

    #[test]
    fn admin_cost_report_sums_cents_into_spending() {
        let limits = parse_cost_report(
            r#"{"data":[{"results":[{"amount":"123.45"},{"amount":"0.55"}]},{"results":[{"amount":"76"}]}],"has_more":false}"#,
            Utc::now(),
        )
        .unwrap();
        assert_eq!(limits.spending.unwrap().used_microusd, 2_000_000);
    }
}
