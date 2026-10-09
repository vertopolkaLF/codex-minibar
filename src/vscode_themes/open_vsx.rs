//! Open VSX (open-vsx.org) theme search and install.

use std::{
    collections::HashMap,
    io::Read,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use super::ThemeSource;

const API: &str = "https://open-vsx.org/api";
/// Results shown per search.
const PAGE_SIZE: usize = 30;
/// Hits requested per search: filtering out non-color-theme packages
/// (icon packs especially) drops some.
const SEARCH_SIZE: u32 = 50;
const MANIFEST_WORKERS: usize = 16;
const MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
/// Theme packages are small; this only stops a runaway download.
const MAX_VSIX_BYTES: u64 = 100 * 1024 * 1024;
/// Extension icons are 128-256 px images.
const MAX_ICON_BYTES: u64 = 2 * 1024 * 1024;

/// One search hit from the registry's Themes category.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extension {
    pub namespace: String,
    pub name: String,
    pub version: String,
    pub display_name: String,
    pub description: String,
    pub download_count: u64,
    pub verified: bool,
    /// Extension logo, when the package ships one.
    pub icon_url: Option<String>,
    download_url: String,
}

impl Extension {
    /// `namespace.name`, the prefix of every theme id it installs.
    pub fn key(&self) -> String {
        format!("{}.{}", self.namespace, self.name).to_lowercase()
    }
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    extensions: Vec<RawExtension>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawExtension {
    namespace: String,
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    download_count: u64,
    #[serde(default)]
    verified: bool,
    #[serde(default)]
    deprecated: bool,
    #[serde(default)]
    files: Files,
}

#[derive(Deserialize)]
struct Manifest {
    #[serde(default)]
    contributes: Contributes,
}

#[derive(Default, Deserialize)]
struct Contributes {
    /// Color themes; icon packs use `iconThemes` / `productIconThemes`.
    #[serde(default)]
    themes: Vec<serde_json::Value>,
    #[serde(default)]
    languages: Vec<serde_json::Value>,
    #[serde(default)]
    debuggers: Vec<serde_json::Value>,
}

impl Contributes {
    /// A color theme package, not a language tool that bundles a theme.
    fn is_color_theme(&self) -> bool {
        !self.themes.is_empty() && self.languages.is_empty() && self.debuggers.is_empty()
    }
}

#[derive(Default, Deserialize)]
struct Files {
    #[serde(default)]
    download: Option<String>,
    #[serde(default)]
    icon: Option<String>,
}

/// Uses the OS TLS stack (schannel), like the app's other public fetches:
/// it honors the Windows certificate store, which VPNs and proxies that
/// inspect TLS rely on.
/// Shared so its connection pool keeps TLS sessions to the registry warm
/// across the many small manifest and icon requests.
fn agent() -> ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(build_agent).clone()
}

fn build_agent() -> ureq::Agent {
    let builder = ureq::AgentBuilder::new().max_idle_connections_per_host(MANIFEST_WORKERS);
    let builder = match ureq::native_tls::TlsConnector::new() {
        Ok(tls) => builder.tls_connector(Arc::new(tls)),
        Err(error) => {
            eprintln!("Open VSX: falling back to bundled TLS: {error}");
            builder
        }
    };
    builder
        .timeout_connect(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .user_agent(concat!("codex-minibar/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// A short reason for a failed request. ureq's own message leads with the
/// full request URL, which is noise in a settings row.
fn describe(error: ureq::Error) -> anyhow::Error {
    match error {
        ureq::Error::Status(code, _) => anyhow::anyhow!("Open VSX answered with HTTP {code}"),
        ureq::Error::Transport(transport) => {
            let detail = transport
                .message()
                .map(str::to_owned)
                .or_else(|| std::error::Error::source(&transport).map(ToString::to_string));
            match detail {
                Some(detail) => anyhow::anyhow!("{}: {detail}", transport.kind()),
                None => anyhow::anyhow!("{}", transport.kind()),
            }
        }
    }
}

/// Color theme extensions matching `query`; the most downloaded first when
/// the query is empty.
pub fn search(query: &str) -> Result<Vec<Extension>> {
    let query = query.trim();
    let size = SEARCH_SIZE.to_string();
    let mut request = agent()
        .get(&format!("{API}/-/search"))
        .query("category", "Themes")
        .query("size", &size)
        .query("includeAllVersions", "false");
    request = if query.is_empty() {
        request
            .query("sortBy", "downloadCount")
            .query("sortOrder", "desc")
    } else {
        request.query("query", query).query("sortBy", "relevance")
    };
    let body = request
        .call()
        .map_err(describe)?
        .into_string()
        .context("the Open VSX response was interrupted")?;
    let response: SearchResponse =
        serde_json::from_str(&body).context("Open VSX returned an unexpected response")?;
    let extensions = response
        .extensions
        .into_iter()
        .filter(|raw| !raw.deprecated)
        .filter_map(|raw| {
            let download_url = raw
                .files
                .download
                .clone()
                .filter(|url| url.starts_with("https://"))?;
            Some(Extension {
                display_name: raw
                    .display_name
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| raw.name.clone()),
                description: raw.description.unwrap_or_default(),
                namespace: raw.namespace,
                name: raw.name,
                version: raw.version,
                download_count: raw.download_count,
                verified: raw.verified,
                icon_url: raw.files.icon.filter(|url| url.starts_with("https://")),
                download_url,
            })
        })
        .collect::<Vec<_>>();
    Ok(color_theme_extensions(extensions))
}

/// The Themes category also lists icon packs, product icon themes and the
/// odd extension that bundles a theme as an extra (PowerShell); keep only
/// packages whose manifest contributes color themes. Manifests are fetched
/// in parallel; one that cannot be read falls back to the extension's name.
fn color_theme_extensions(extensions: Vec<Extension>) -> Vec<Extension> {
    let verdicts = std::thread::scope(|scope| {
        let workers = extensions
            .chunks(extensions.len().div_ceil(MANIFEST_WORKERS).max(1))
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(contributes_color_themes)
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().unwrap_or_default())
            .collect::<Vec<_>>()
    });
    extensions
        .into_iter()
        .zip(verdicts.into_iter().chain(std::iter::repeat(true)))
        .filter_map(|(extension, keep)| keep.then_some(extension))
        .take(PAGE_SIZE)
        .collect()
}

/// Verdicts per package version; manifests never change once published.
fn manifest_verdicts() -> &'static Mutex<HashMap<String, bool>> {
    static VERDICTS: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();
    VERDICTS.get_or_init(Default::default)
}

fn contributes_color_themes(extension: &Extension) -> bool {
    let key = extension.download_url.clone();
    let known = manifest_verdicts()
        .lock()
        .ok()
        .and_then(|verdicts| verdicts.get(&key).copied());
    if let Some(known) = known {
        return known;
    }
    // The manifest sits beside the package: `.../<version>/file/package.json`.
    let Some((base, _)) = extension.download_url.rsplit_once('/') else {
        return true;
    };
    let url = format!("{base}/package.json");
    // One retry: a flaky connection drops the odd request out of the burst.
    let verdict = (0..2)
        .find_map(|_| fetch_manifest(&url))
        .map(|manifest| manifest.contributes.is_color_theme());
    match verdict {
        Some(verdict) => {
            if let Ok(mut verdicts) = manifest_verdicts().lock() {
                verdicts.insert(key, verdict);
            }
            verdict
        }
        None => !looks_like_icon_pack(extension),
    }
}

fn fetch_manifest(url: &str) -> Option<Manifest> {
    let response = agent().get(url).call().ok()?;
    let mut body = Vec::new();
    response
        .into_reader()
        .take(MAX_MANIFEST_BYTES)
        .read_to_end(&mut body)
        .ok()?;
    serde_json::from_slice(&body).ok()
}

/// Best guess when the manifest is unreachable: icon packs name themselves
/// after their icons ("Material Product Icons", "vscode-icons").
fn looks_like_icon_pack(extension: &Extension) -> bool {
    [&extension.name, &extension.display_name]
        .iter()
        .any(|name| name.to_lowercase().contains("icon"))
}

/// Download `extension` and install every color theme it contributes.
pub fn install(extension: &Extension) -> Result<Vec<String>> {
    super::install(download(extension)?)
}

/// The color themes `extension` contributes, parsed in memory without
/// touching the library, for previews.
pub fn download(extension: &Extension) -> Result<Vec<super::VsCodeTheme>> {
    let response = agent()
        .get(&extension.download_url)
        .call()
        .map_err(describe)
        .with_context(|| format!("could not download {}", extension.display_name))?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_VSIX_BYTES + 1)
        .read_to_end(&mut bytes)
        .context("the download was interrupted")?;
    if bytes.len() as u64 > MAX_VSIX_BYTES {
        bail!(
            "{} is too large to be a theme package",
            extension.display_name
        );
    }
    super::themes_from_vsix(
        &bytes,
        ThemeSource::OpenVsx {
            namespace: extension.namespace.clone(),
            name: extension.name.clone(),
            version: extension.version.clone(),
        },
    )
}

/// The raw bytes of an extension icon.
pub fn fetch_icon(url: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    agent()
        .get(url)
        .call()
        .map_err(describe)?
        .into_reader()
        .take(MAX_ICON_BYTES + 1)
        .read_to_end(&mut bytes)
        .context("the icon download was interrupted")?;
    if bytes.len() as u64 > MAX_ICON_BYTES {
        bail!("the icon is too large");
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live registry round trip without touching the installed library:
    /// `cargo test --lib open_vsx -- --ignored`.
    #[test]
    #[ignore = "needs network access to open-vsx.org"]
    fn searches_and_parses_a_live_theme_package() {
        let results = search("one dark pro").unwrap();
        let extension = results
            .iter()
            .find(|extension| extension.key() == "zhuangtongfa.material-theme")
            .expect("One Dark Pro is listed");
        let response = agent().get(&extension.download_url).call().unwrap();
        let mut bytes = Vec::new();
        response.into_reader().read_to_end(&mut bytes).unwrap();
        let themes = super::super::themes_from_vsix(
            &bytes,
            ThemeSource::OpenVsx {
                namespace: extension.namespace.clone(),
                name: extension.name.clone(),
                version: extension.version.clone(),
            },
        )
        .unwrap();
        assert!(themes.len() >= 3);
        assert!(themes.iter().all(|theme| theme.kind.is_dark()));
        assert!(themes[0].color("editor.background").is_some());
        let icon = fetch_icon(extension.icon_url.as_deref().expect("has an icon")).unwrap();
        assert!(icon.starts_with(b"\x89PNG"));
        let popular = search("").unwrap();
        assert!(!popular.is_empty());
        for gone in ["pkief.material-icon-theme", "ms-vscode.powershell"] {
            assert!(
                popular.iter().all(|extension| extension.key() != gone),
                "{gone}"
            );
        }
        assert!(
            popular
                .iter()
                .any(|extension| extension.key() == "zhuangtongfa.material-theme")
        );
    }
}
