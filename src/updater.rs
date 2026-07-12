//! In-place updates from GitHub Releases portable zip assets.

use std::{
    fs,
    io::copy,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex, OnceLock},
    thread,
};

use anyhow::{Context, Result, bail};
use semver::Version;
use serde::Deserialize;
use ureq::Agent;
use zip::ZipArchive;

use crate::notifications;
use crate::single_instance::release_for_update;

pub const GITHUB_OWNER: &str = "vertopolkaLF";
pub const GITHUB_REPO: &str = "codex-minibar";
pub const RELEASES_URL: &str = "https://github.com/vertopolkaLF/codex-minibar/releases";
pub const REPO_URL: &str = "https://github.com/vertopolkaLF/codex-minibar";
pub const ISSUES_URL: &str = "https://github.com/vertopolkaLF/codex-minibar/issues";

const LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/vertopolkaLF/codex-minibar/releases/latest";
const APP_EXE: &str = "codex-minibar.exe";
const USER_AGENT: &str = "codex-minibar-updater";
const UPDATE_SUCCESS_MARKER: &str = ".update-success-pending";

fn http_agent() -> Agent {
    static AGENT: OnceLock<Agent> = OnceLock::new();
    AGENT
        .get_or_init(|| {
            // ureq's `native-tls` feature is not auto-selected for `ureq::get`;
            // build an agent that uses the OS TLS stack (schannel on Windows).
            let tls = ureq::native_tls::TlsConnector::new()
                .expect("create native-tls connector");
            ureq::AgentBuilder::new()
                .tls_connector(Arc::new(tls))
                .build()
        })
        .clone()
}

struct UpdateRuntime {
    updates: Arc<UpdateController>,
    before_exit: Arc<dyn Fn() + Send + Sync>,
}

static RUNTIME: OnceLock<UpdateRuntime> = OnceLock::new();

/// Registers the single process-wide hook used by [`apply_pending_update`].
pub fn install_runtime(
    updates: Arc<UpdateController>,
    before_exit: impl Fn() + Send + Sync + 'static,
) {
    let _ = RUNTIME.set(UpdateRuntime {
        updates,
        before_exit: Arc::new(before_exit),
    });
}

/// Single entry point for installing a discovered update. All UI surfaces must
/// call this and only this when the user chooses to update now.
pub fn apply_pending_update() -> Result<()> {
    let runtime = RUNTIME.get().context("update runtime is not installed")?;
    runtime.updates.apply()?;
    (runtime.before_exit)();
    release_for_update();
    std::process::exit(0);
}

/// Opens the GitHub release page for the pending update, or the releases index.
pub fn open_release_notes() -> Result<()> {
    let url = RUNTIME
        .get()
        .and_then(|runtime| runtime.updates.available_update())
        .map(|update| update.html_url)
        .unwrap_or_else(|| RELEASES_URL.to_string());
    open_url(&url)
}

/// Shows a one-shot success toast after an in-place update relaunch.
pub fn show_post_update_success_if_needed() {
    match take_post_update_success_marker() {
        Ok(Some(version)) => notifications::show(
            "Update complete",
            &format!("Codex Minibar was updated to {version} and is running again."),
        ),
        Ok(None) => {}
        Err(error) => eprintln!("failed to read post-update marker: {error:#}"),
    }
}

/// Keeps Windows "Installed apps" DisplayVersion in sync with this binary.
/// Zip updates replace files but do not rewrite the NSIS uninstall registry.
pub fn sync_installed_display_version() {
    #[cfg(windows)]
    if let Err(error) = sync_uninstall_display_version(env!("CARGO_PKG_VERSION")) {
        eprintln!("failed to sync installed display version: {error:#}");
    }
}

#[cfg(windows)]
fn sync_uninstall_display_version(version: &str) -> Result<()> {
    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegOpenKeyExW,
            RegSetValueExW,
        },
    };

    // Installer writes HKCU (RequestExecutionLevel user) for each arch package.
    for arch in ["x86", "x64", "arm64"] {
        let subkey = format!(
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Codex Minibar {arch}"
        );
        let subkey_w: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
        let mut key: HKEY = std::ptr::null_mut();
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                subkey_w.as_ptr(),
                0,
                KEY_SET_VALUE,
                &mut key,
            )
        };
        if status != ERROR_SUCCESS {
            continue;
        }
        let name_w: Vec<u16> = "DisplayVersion"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let data: Vec<u16> = version.encode_utf16().chain(std::iter::once(0)).collect();
        let status = unsafe {
            RegSetValueExW(
                key,
                name_w.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * size_of::<u16>()) as u32,
            )
        };
        unsafe { RegCloseKey(key) };
        if status != ERROR_SUCCESS {
            bail!("RegSetValueExW(DisplayVersion) for {arch}: {status}");
        }
    }
    Ok(())
}

fn take_post_update_success_marker() -> Result<Option<String>> {
    let path = install_dir()?.join(UPDATE_SUCCESS_MARKER);
    if !path.exists() {
        return Ok(None);
    }
    let version = fs::read_to_string(&path)
        .with_context(|| format!("read {}", path.display()))?
        .trim()
        .to_string();
    let _ = fs::remove_file(&path);
    if version.is_empty() {
        return Ok(None);
    }
    Ok(Some(version))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvailableUpdate {
    pub version: String,
    pub asset_url: String,
    pub html_url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdatePhase {
    Idle,
    Checking,
    UpToDate,
    Available(AvailableUpdate),
    Applying,
    Failed(String),
}

struct InnerState {
    phase: UpdatePhase,
    notified_version: Option<String>,
}

pub struct UpdateController {
    inner: Mutex<InnerState>,
}

impl UpdateController {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(InnerState {
                phase: UpdatePhase::Idle,
                notified_version: None,
            }),
        })
    }

    pub fn snapshot(&self) -> UpdatePhase {
        self.inner.lock().expect("update controller lock").phase.clone()
    }

    pub fn is_update_available(&self) -> bool {
        matches!(self.snapshot(), UpdatePhase::Available(_))
    }

    pub fn available_update(&self) -> Option<AvailableUpdate> {
        match self.snapshot() {
            UpdatePhase::Available(update) => Some(update),
            _ => None,
        }
    }

    fn set_phase(&self, phase: UpdatePhase) {
        self.inner.lock().expect("update controller lock").phase = phase;
    }

    pub fn check_async(self: &Arc<Self>, notify: bool, notify_enabled: bool) {
        let controller = Arc::clone(self);
        thread::spawn(move || {
            if let Err(error) = controller.check_once(notify, notify_enabled) {
                eprintln!("update check failed: {error:#}");
                controller.set_phase(UpdatePhase::Failed(error.to_string()));
            }
        });
    }

    fn check_once(&self, notify: bool, notify_enabled: bool) -> Result<()> {
        self.set_phase(UpdatePhase::Checking);
        match check_for_update()? {
            Some(update) => {
                let should_notify = notify
                    && notify_enabled
                    && self
                        .inner
                        .lock()
                        .expect("update controller lock")
                        .notified_version
                        .as_deref()
                        != Some(update.version.as_str());
                if should_notify {
                    notifications::show_update_available(&update.version, &update.html_url);
                    self.inner
                        .lock()
                        .expect("update controller lock")
                        .notified_version = Some(update.version.clone());
                }
                self.set_phase(UpdatePhase::Available(update));
            }
            None => self.set_phase(UpdatePhase::UpToDate),
        }
        Ok(())
    }

    pub fn apply(&self) -> Result<()> {
        let update = self.available_update().context("no update is available")?;
        self.set_phase(UpdatePhase::Applying);
        apply_update(&update)?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

pub fn current_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_else(|_| Version::new(0, 0, 0))
}

pub fn host_arch() -> &'static str {
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    {
        return "x64";
    }
    #[cfg(all(target_arch = "x86", target_pointer_width = "32"))]
    {
        return "x86";
    }
    #[cfg(target_arch = "aarch64")]
    {
        return "arm64";
    }
    #[cfg(not(any(
        all(target_arch = "x86_64", target_pointer_width = "64"),
        all(target_arch = "x86", target_pointer_width = "32"),
        target_arch = "aarch64"
    )))]
    {
        "x64"
    }
}

fn check_for_update() -> Result<Option<AvailableUpdate>> {
    let release: GhRelease = github_get(LATEST_RELEASE_API)?;
    let remote = parse_release_version(&release.tag_name)?;
    let current = current_version();
    if remote <= current {
        return Ok(None);
    }

    let arch = host_arch();
    let expected_suffix = format!("-{arch}-portable.zip");
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name.ends_with(&expected_suffix))
        .with_context(|| {
            format!(
                "release {} has no portable zip asset for {arch} (*{expected_suffix})",
                release.tag_name
            )
        })?;

    Ok(Some(AvailableUpdate {
        version: remote.to_string(),
        asset_url: asset.browser_download_url.clone(),
        html_url: release.html_url,
    }))
}

fn parse_release_version(tag: &str) -> Result<Version> {
    let trimmed = tag.trim();
    // Accept "v1.2.3", "V1.2.3", "v.1.2.3", "V.1.2.3", and bare "1.2.3".
    let without_prefix = trimmed
        .strip_prefix("v.")
        .or_else(|| trimmed.strip_prefix("V."))
        .or_else(|| trimmed.strip_prefix('v'))
        .or_else(|| trimmed.strip_prefix('V'))
        .unwrap_or(trimmed)
        .trim_start_matches('.');
    Version::parse(without_prefix).with_context(|| format!("parse release tag {tag:?}"))
}

fn github_get(url: &str) -> Result<GhRelease> {
    let response = http_agent()
        .get(url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .call()
        .with_context(|| format!("GET {url}"))?;
    let status = response.status();
    let body = response
        .into_string()
        .with_context(|| format!("read GitHub response body ({status})"))?;
    if status / 100 != 2 {
        bail!("GitHub API returned {status}: {body}");
    }
    serde_json::from_str(&body).context("parse GitHub release JSON")
}

fn install_dir() -> Result<PathBuf> {
    std::env::current_exe()
        .context("resolve current executable")
        .and_then(|path| {
            path.parent()
                .map(Path::to_path_buf)
                .context("executable has no parent directory")
        })
}

fn install_dir_writable(dir: &Path) -> bool {
    let probe = dir.join(".codex-minibar-update-write-test");
    match fs::write(&probe, b"") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn apply_update(update: &AvailableUpdate) -> Result<()> {
    let install_dir = install_dir()?;
    if !install_dir_writable(&install_dir) {
        open_url(&update.html_url)?;
        bail!(
            "cannot update in place because {} is not writable; opened the release page in your browser",
            install_dir.display()
        );
    }

    let staging_root = install_dir.join(".update-staging");
    let run_dir = staging_root.join(format!("run-{}", std::process::id()));
    if run_dir.exists() {
        fs::remove_dir_all(&run_dir)
            .with_context(|| format!("clear previous update staging at {}", run_dir.display()))?;
    }
    fs::create_dir_all(&run_dir)
        .with_context(|| format!("create update staging at {}", run_dir.display()))?;

    let zip_path = run_dir.join("package.zip");
    download_file(&update.asset_url, &zip_path)?;
    let extracted = run_dir.join("extracted");
    fs::create_dir_all(&extracted).context("create extraction directory")?;
    let payload_root = extract_portable_zip(&zip_path, &extracted)?;

    let pid = std::process::id();
    let script_path = run_dir.join("apply-update.ps1");
    let script = format!(
        r#"$ErrorActionPreference = 'Stop'
$pidToWait = {pid}
$install = '{install}'
$source = '{source}'
$staging = '{staging}'
$exe = Join-Path $install '{APP_EXE}'

while (Get-Process -Id $pidToWait -ErrorAction SilentlyContinue) {{
    Start-Sleep -Milliseconds 250
}}

try {{
    Get-ChildItem -LiteralPath $source -Force | ForEach-Object {{
        Copy-Item -LiteralPath $_.FullName -Destination $install -Recurse -Force
    }}
    Set-Content -LiteralPath (Join-Path $install '{marker}') -Value '{version}' -Encoding utf8 -NoNewline
    foreach ($arch in @('x86', 'x64', 'arm64')) {{
        $key = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Codex Minibar $arch"
        if (Test-Path -LiteralPath $key) {{
            Set-ItemProperty -LiteralPath $key -Name DisplayVersion -Value '{version}'
        }}
    }}
    Start-Process -FilePath $exe -WorkingDirectory $install
}} finally {{
    Start-Sleep -Seconds 2
    if (Test-Path -LiteralPath $staging) {{
        Remove-Item -LiteralPath $staging -Recurse -Force -ErrorAction SilentlyContinue
    }}
}}
"#,
        install = escape_ps_single_quoted(&install_dir),
        source = escape_ps_single_quoted(&payload_root),
        staging = escape_ps_single_quoted(&run_dir),
        marker = UPDATE_SUCCESS_MARKER,
        version = escape_ps_single_quoted_str(&update.version),
    );
    fs::write(&script_path, script).context("write update helper script")?;
    spawn_hidden_helper(&script_path).context("spawn update helper")?;

    Ok(())
}

#[cfg(windows)]
fn spawn_hidden_helper(script_path: &Path) -> Result<()> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-File",
        ])
        .arg(script_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .context("spawn hidden PowerShell update helper")
}

#[cfg(not(windows))]
fn spawn_hidden_helper(_script_path: &Path) -> Result<()> {
    bail!("in-place updates are only supported on Windows")
}

fn escape_ps_single_quoted(value: &Path) -> String {
    value.display().to_string().replace('\'', "''")
}

fn escape_ps_single_quoted_str(value: &str) -> String {
    value.replace('\'', "''")
}

fn download_file(url: &str, destination: &Path) -> Result<()> {
    let response = http_agent()
        .get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .with_context(|| format!("download {url}"))?;
    let status = response.status();
    if status / 100 != 2 {
        bail!("download failed with status {status}");
    }
    let mut reader = response.into_reader();
    let mut file =
        fs::File::create(destination).with_context(|| format!("create {}", destination.display()))?;
    copy(&mut reader, &mut file).context("write downloaded update package")?;
    Ok(())
}

fn extract_portable_zip(archive_path: &Path, destination: &Path) -> Result<PathBuf> {
    let file = fs::File::open(archive_path).with_context(|| format!("open {}", archive_path.display()))?;
    let mut archive = ZipArchive::new(file).context("open update zip archive")?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).context("read zip entry")?;
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let target = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target).with_context(|| format!("create {}", target.display()))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        let mut output =
            fs::File::create(&target).with_context(|| format!("create {}", target.display()))?;
        copy(&mut entry, &mut output).with_context(|| format!("extract {}", target.display()))?;
    }

    payload_root_from_extracted(destination)
}

fn payload_root_from_extracted(destination: &Path) -> Result<PathBuf> {
    let mut entries = fs::read_dir(destination)
        .with_context(|| format!("read {}", destination.display()))?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    if entries.len() == 1 {
        let only = entries.remove(0);
        let file_type = only.file_type().context("inspect extracted entry")?;
        if file_type.is_dir() {
            return Ok(only.path());
        }
    }
    Ok(destination.to_path_buf())
}

pub fn open_url(url: &str) -> Result<()> {
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::Shell::ShellExecuteW;

        let operation: Vec<u16> = OsStr::new("open")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let target: Vec<u16> = OsStr::new(url)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                target.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if result as isize <= 32 {
            bail!("ShellExecuteW failed for {url}");
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        bail!("opening URLs is only supported on Windows")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_release_tags() {
        assert_eq!(
            parse_release_version("v1.2.3").unwrap(),
            Version::new(1, 2, 3)
        );
        assert_eq!(
            parse_release_version("V1.2.3").unwrap(),
            Version::new(1, 2, 3)
        );
        assert_eq!(
            parse_release_version("v.1.2.3").unwrap(),
            Version::new(1, 2, 3)
        );
        assert_eq!(
            parse_release_version("V.0.1.0").unwrap(),
            Version::new(0, 1, 0)
        );
        assert_eq!(
            parse_release_version("1.2.3").unwrap(),
            Version::new(1, 2, 3)
        );
        assert_eq!(
            parse_release_version("  v.2.0.0  ").unwrap(),
            Version::new(2, 0, 0)
        );
    }
}
