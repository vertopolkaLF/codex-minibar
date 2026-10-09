//! Open VSX (open-vsx.org) theme search and install.

use std::{io::Read, time::Duration};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use super::ThemeSource;

const API: &str = "https://open-vsx.org/api";
const PAGE_SIZE: u32 = 30;
/// Theme packages are small; this only stops a runaway download.
const MAX_VSIX_BYTES: u64 = 100 * 1024 * 1024;

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

#[derive(Default, Deserialize)]
struct Files {
    #[serde(default)]
    download: Option<String>,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .user_agent(concat!("codex-minibar/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// Color theme extensions matching `query`; the most downloaded first when
/// the query is empty.
pub fn search(query: &str) -> Result<Vec<Extension>> {
    let query = query.trim();
    let size = PAGE_SIZE.to_string();
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
        .context("could not reach Open VSX")?
        .into_string()
        .context("the Open VSX response was interrupted")?;
    let response: SearchResponse =
        serde_json::from_str(&body).context("Open VSX returned an unexpected response")?;
    Ok(response
        .extensions
        .into_iter()
        .filter(|raw| !raw.deprecated)
        .filter_map(|raw| {
            let download_url = raw
                .files
                .download
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
                download_url,
            })
        })
        .collect())
}

/// Download `extension` and install every color theme it contributes.
pub fn install(extension: &Extension) -> Result<Vec<String>> {
    let response = agent()
        .get(&extension.download_url)
        .call()
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
    let themes = super::themes_from_vsix(
        &bytes,
        ThemeSource::OpenVsx {
            namespace: extension.namespace.clone(),
            name: extension.name.clone(),
            version: extension.version.clone(),
        },
    )?;
    super::install(themes)
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
        assert!(!search("").unwrap().is_empty());
    }
}
