//! VS Code color themes for the popup.
//!
//! Themes come from `.vsix` packages (a local file or Open VSX) or from a
//! bare theme `.json`. Only the workbench `colors` map matters to the popup,
//! so each theme is stored resolved (its `include` chain merged) as one JSON
//! file under `<config>/vscode-themes`. The library is loaded on first use
//! and kept in memory; every window reads it when it builds a palette.

pub mod open_vsx;

use std::{
    collections::{BTreeMap, HashMap},
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock, RwLock},
};

use anyhow::{Context, Result, bail};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Larger theme files exist (token-heavy ones run ~200 KB), but nothing
/// legitimate comes close to this.
const MAX_THEME_FILE_BYTES: u64 = 8 * 1024 * 1024;
/// Upper bound on the JSON a package may expand to while importing.
const MAX_PACKAGE_JSON_BYTES: u64 = 64 * 1024 * 1024;
const MAX_INCLUDE_DEPTH: usize = 8;

/// Light or dark base of a theme (VS Code's `uiTheme` / `type`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeKind {
    Light,
    Dark,
    HighContrastDark,
    HighContrastLight,
}

impl ThemeKind {
    pub const fn is_dark(self) -> bool {
        matches!(self, Self::Dark | Self::HighContrastDark)
    }

    /// `contributes.themes[].uiTheme`.
    fn from_ui_theme(value: &str) -> Option<Self> {
        match value {
            "vs" => Some(Self::Light),
            "vs-dark" => Some(Self::Dark),
            "hc-black" => Some(Self::HighContrastDark),
            "hc-light" => Some(Self::HighContrastLight),
            _ => None,
        }
    }

    /// The theme file's own `type`.
    fn from_type(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "light" | "vs" => Some(Self::Light),
            "dark" | "vs-dark" => Some(Self::Dark),
            "hc" | "hcdark" | "hc-black" | "highcontrast" => Some(Self::HighContrastDark),
            "hclight" | "hc-light" => Some(Self::HighContrastLight),
            _ => None,
        }
    }
}

/// Where an installed theme came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ThemeSource {
    /// Imported from a file on this PC.
    File { name: String },
    /// Installed from the Open VSX registry.
    OpenVsx {
        namespace: String,
        name: String,
        version: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VsCodeTheme {
    /// Stable id, also the stored file name; the setting references it.
    pub id: String,
    pub label: String,
    /// Extension display name or publisher, for the picker caption.
    #[serde(default)]
    pub extension: Option<String>,
    pub kind: ThemeKind,
    pub source: ThemeSource,
    /// Workbench color id -> `#RRGGBB[AA]`, with includes already merged.
    pub colors: BTreeMap<String, String>,
}

impl VsCodeTheme {
    /// A workbench color as RGBA bytes; `None` when missing or malformed.
    pub fn color(&self, key: &str) -> Option<[u8; 4]> {
        parse_hex_color(self.colors.get(key)?)
    }
}

/// `#RGB`, `#RGBA`, `#RRGGBB` or `#RRGGBBAA`.
pub fn parse_hex_color(value: &str) -> Option<[u8; 4]> {
    let hex = value.trim().strip_prefix('#')?;
    if !hex.is_ascii() {
        return None;
    }
    let nibble = |index: usize| u8::from_str_radix(&hex[index..=index], 16).ok();
    let byte = |index: usize| u8::from_str_radix(&hex[index..index + 2], 16).ok();
    match hex.len() {
        3 | 4 => {
            let mut rgba = [255; 4];
            for (slot, index) in rgba.iter_mut().zip(0..hex.len()) {
                *slot = nibble(index)? * 17;
            }
            Some(rgba)
        }
        6 | 8 => {
            let mut rgba = [255; 4];
            for (slot, index) in rgba.iter_mut().zip((0..hex.len()).step_by(2)) {
                *slot = byte(index)?;
            }
            Some(rgba)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Library
// ---------------------------------------------------------------------------

pub type Library = Arc<Vec<Arc<VsCodeTheme>>>;

fn library_cell() -> &'static RwLock<Library> {
    static LIBRARY: OnceLock<RwLock<Library>> = OnceLock::new();
    LIBRARY.get_or_init(|| RwLock::new(Arc::new(load_library())))
}

/// Every installed theme, sorted by label.
pub fn installed() -> Library {
    library_cell()
        .read()
        .map(|library| Arc::clone(&library))
        .unwrap_or_default()
}

/// One installed theme by id.
pub fn get(id: &str) -> Option<Arc<VsCodeTheme>> {
    installed().iter().find(|theme| theme.id == id).cloned()
}

fn themes_dir() -> Result<PathBuf> {
    ProjectDirs::from("dev", "Codex Minibar", "Codex Minibar")
        .map(|dirs| dirs.config_dir().join("vscode-themes"))
        .context("could not resolve the application config directory")
}

fn load_library() -> Vec<Arc<VsCodeTheme>> {
    let Ok(dir) = themes_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut themes = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            match serde_json::from_str::<VsCodeTheme>(&text) {
                Ok(theme) => Some(Arc::new(theme)),
                Err(error) => {
                    eprintln!("skipping VS Code theme {}: {error:#}", path.display());
                    None
                }
            }
        })
        .collect::<Vec<_>>();
    sort_library(&mut themes);
    themes
}

fn sort_library(themes: &mut [Arc<VsCodeTheme>]) {
    themes.sort_by(|a, b| {
        a.label
            .to_lowercase()
            .cmp(&b.label.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
}

/// Store `themes`, replacing installed ones with the same id. Returns their ids.
pub fn install(themes: Vec<VsCodeTheme>) -> Result<Vec<String>> {
    let dir = themes_dir()?;
    std::fs::create_dir_all(&dir).with_context(|| format!("could not create {}", dir.display()))?;
    for theme in &themes {
        let path = dir.join(format!("{}.json", theme.id));
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, serde_json::to_vec_pretty(theme)?)
            .with_context(|| format!("could not write {}", temp.display()))?;
        std::fs::rename(&temp, &path)
            .with_context(|| format!("could not write {}", path.display()))?;
    }
    let ids = themes
        .iter()
        .map(|theme| theme.id.clone())
        .collect::<Vec<_>>();
    let mut library = library_cell()
        .write()
        .map_err(|_| anyhow::anyhow!("theme library lock poisoned"))?;
    let mut next = library
        .iter()
        .filter(|theme| !ids.contains(&theme.id))
        .cloned()
        .collect::<Vec<_>>();
    next.extend(themes.into_iter().map(Arc::new));
    sort_library(&mut next);
    *library = Arc::new(next);
    Ok(ids)
}

/// Delete an installed theme.
pub fn remove(id: &str) -> Result<()> {
    let path = themes_dir()?.join(format!("{}.json", sanitize_id(id)));
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("could not delete {}", path.display()));
        }
    }
    let mut library = library_cell()
        .write()
        .map_err(|_| anyhow::anyhow!("theme library lock poisoned"))?;
    let next = library
        .iter()
        .filter(|theme| theme.id != id)
        .cloned()
        .collect();
    *library = Arc::new(next);
    Ok(())
}

/// Import every color theme in a `.vsix` package or a theme `.json` file.
pub fn import_file(path: &Path) -> Result<Vec<String>> {
    let is_package = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("vsix") || ext.eq_ignore_ascii_case("zip"));
    let themes = if is_package {
        let bytes =
            std::fs::read(path).with_context(|| format!("could not read {}", path.display()))?;
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        themes_from_vsix(&bytes, ThemeSource::File { name: file_name })?
    } else {
        vec![theme_from_json_file(path)?]
    };
    install(themes)
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Package metadata a theme's id and caption are built from.
struct PackageInfo {
    publisher: String,
    name: String,
    display_name: Option<String>,
}

/// Read every `contributes.themes` entry of a `.vsix` package.
pub(crate) fn themes_from_vsix(bytes: &[u8], source: ThemeSource) -> Result<Vec<VsCodeTheme>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .context("the file is not a valid .vsix package")?;
    // Only JSON under `extension/` can be a manifest, NLS table or theme.
    let mut files = HashMap::new();
    let mut budget = MAX_PACKAGE_JSON_BYTES;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().replace('\\', "/");
        let Some(relative) = name.strip_prefix("extension/") else {
            continue;
        };
        let lower = relative.to_ascii_lowercase();
        if !entry.is_file() || !(lower.ends_with(".json") || lower.ends_with(".jsonc")) {
            continue;
        }
        if entry.size() > MAX_THEME_FILE_BYTES || entry.size() > budget {
            continue;
        }
        let mut data = Vec::new();
        entry
            .by_ref()
            .take(MAX_THEME_FILE_BYTES)
            .read_to_end(&mut data)?;
        budget = budget.saturating_sub(data.len() as u64);
        files.insert(normalize_path(relative), data);
    }
    let manifest = files
        .get("package.json")
        .context("the package has no extension/package.json")?;
    let manifest = parse_jsonc(manifest).context("could not read the package manifest")?;
    let nls = files
        .get("package.nls.json")
        .and_then(|data| parse_jsonc(data).ok());
    let text = |key: &str| {
        manifest
            .get(key)
            .and_then(Value::as_str)
            .map(|value| localize(value, nls.as_ref()))
    };
    let info = PackageInfo {
        publisher: text("publisher").unwrap_or_else(|| "unknown".into()),
        name: text("name").unwrap_or_else(|| "theme".into()),
        display_name: text("displayName").filter(|name| !name.trim().is_empty()),
    };
    let contributions = manifest
        .pointer("/contributes/themes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if contributions.is_empty() {
        bail!("this extension does not contribute any color themes");
    }
    let read = |path: &str| files.get(path).cloned();
    let mut themes = Vec::new();
    let mut failures = Vec::new();
    for contribution in contributions {
        let Some(path) = contribution.get("path").and_then(Value::as_str) else {
            continue;
        };
        let path = normalize_path(path);
        let resolved = match resolve_theme(&read, &path, 0) {
            Ok(resolved) => resolved,
            Err(error) => {
                failures.push(format!("{path}: {error:#}"));
                continue;
            }
        };
        let label = contribution
            .get("label")
            .and_then(Value::as_str)
            .map(|label| localize(label, nls.as_ref()))
            .or(resolved.name.clone())
            .unwrap_or_else(|| info.name.clone());
        let theme_id = contribution
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| label.clone());
        let kind = contribution
            .get("uiTheme")
            .and_then(Value::as_str)
            .and_then(ThemeKind::from_ui_theme)
            .or(resolved.kind)
            .unwrap_or_else(|| infer_kind(&resolved.colors));
        themes.push(VsCodeTheme {
            id: sanitize_id(&format!("{}.{}.{}", info.publisher, info.name, theme_id)),
            label,
            extension: info
                .display_name
                .clone()
                .or_else(|| Some(info.publisher.clone())),
            kind,
            source: source.clone(),
            colors: resolved.colors,
        });
    }
    if themes.is_empty() {
        match failures.first() {
            Some(failure) => bail!("could not read the package's color themes ({failure})"),
            None => bail!("this extension does not contribute any color themes"),
        }
    }
    for failure in failures {
        eprintln!("skipped a VS Code theme: {failure}");
    }
    Ok(themes)
}

/// A bare theme file; `include`s resolve next to it.
fn theme_from_json_file(path: &Path) -> Result<VsCodeTheme> {
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .context("not a file")?;
    let read = |relative: &str| {
        let full = dir.join(relative);
        let metadata = std::fs::metadata(&full).ok()?;
        (metadata.len() <= MAX_THEME_FILE_BYTES)
            .then(|| std::fs::read(full).ok())
            .flatten()
    };
    let resolved = resolve_theme(&read, &file_name, 0)?;
    if resolved.colors.is_empty() {
        bail!("{file_name} has no workbench \"colors\"; it is not a VS Code color theme");
    }
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "theme".into());
    let label = resolved.name.clone().unwrap_or_else(|| stem.clone());
    Ok(VsCodeTheme {
        id: sanitize_id(&format!("file.{stem}")),
        label,
        extension: None,
        kind: resolved
            .kind
            .unwrap_or_else(|| infer_kind(&resolved.colors)),
        source: ThemeSource::File { name: file_name },
        colors: resolved.colors,
    })
}

struct ResolvedTheme {
    name: Option<String>,
    kind: Option<ThemeKind>,
    colors: BTreeMap<String, String>,
}

/// Merge a theme file over its `include` chain.
fn resolve_theme(
    read: &dyn Fn(&str) -> Option<Vec<u8>>,
    path: &str,
    depth: usize,
) -> Result<ResolvedTheme> {
    if depth > MAX_INCLUDE_DEPTH {
        bail!("theme includes nest too deeply");
    }
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".tmtheme") {
        bail!("TextMate (.tmTheme) themes have no workbench colors");
    }
    let data = read(path).with_context(|| format!("missing theme file {path}"))?;
    let value = parse_jsonc(&data).with_context(|| format!("could not parse {path}"))?;
    let mut resolved = match value.get("include").and_then(Value::as_str) {
        Some(include) => {
            let base = parent_dir(path);
            resolve_theme(
                read,
                &normalize_path(&format!("{base}{include}")),
                depth + 1,
            )?
        }
        None => ResolvedTheme {
            name: None,
            kind: None,
            colors: BTreeMap::new(),
        },
    };
    if let Some(name) = value.get("name").and_then(Value::as_str) {
        resolved.name = Some(name.to_owned());
    }
    if let Some(kind) = value
        .get("type")
        .and_then(Value::as_str)
        .and_then(ThemeKind::from_type)
    {
        resolved.kind = Some(kind);
    }
    if let Some(colors) = value.get("colors").and_then(Value::as_object) {
        for (key, color) in colors {
            match color.as_str() {
                Some(color) if parse_hex_color(color).is_some() => {
                    resolved.colors.insert(key.clone(), color.trim().to_owned());
                }
                // `null` (or junk) clears an inherited value.
                _ => {
                    resolved.colors.remove(key);
                }
            }
        }
    }
    Ok(resolved)
}

/// Dark when the editor background is dark; light otherwise.
fn infer_kind(colors: &BTreeMap<String, String>) -> ThemeKind {
    let background = ["editor.background", "sideBar.background"]
        .into_iter()
        .find_map(|key| parse_hex_color(colors.get(key)?));
    match background {
        Some([r, g, b, _]) => {
            let luma = 0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b);
            if luma < 128.0 {
                ThemeKind::Dark
            } else {
                ThemeKind::Light
            }
        }
        None => ThemeKind::Dark,
    }
}

/// `%key%` placeholders resolve through `package.nls.json`.
fn localize(value: &str, nls: Option<&Value>) -> String {
    value
        .strip_prefix('%')
        .and_then(|key| key.strip_suffix('%'))
        .and_then(|key| nls?.get(key))
        .and_then(|entry| match entry {
            Value::String(text) => Some(text.clone()),
            // `{ "message": "...", "comment": [...] }`
            other => other.get("message")?.as_str().map(str::to_owned),
        })
        .unwrap_or_else(|| value.to_owned())
}

fn parent_dir(path: &str) -> &str {
    path.rfind('/').map_or("", |index| &path[..=index])
}

/// Forward slashes with `.` and `..` folded; never escapes the root.
fn normalize_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    parts.join("/")
}

/// Lowercase ASCII letters, digits, `.`, `-` and `_`; safe as a file name.
fn sanitize_id(value: &str) -> String {
    let mut id = String::with_capacity(value.len());
    for ch in value.chars().flat_map(char::to_lowercase) {
        let ch = if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_') {
            ch
        } else {
            '-'
        };
        if ch == '-' && id.ends_with('-') {
            continue;
        }
        id.push(ch);
    }
    let id = id.trim_matches(['-', '.']).to_owned();
    if id.is_empty() { "theme".into() } else { id }
}

/// JSON with comments and trailing commas, as VS Code accepts it.
fn parse_jsonc(data: &[u8]) -> Result<Value> {
    let text = String::from_utf8_lossy(data);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    Ok(serde_json::from_str(&strip_jsonc(text))?)
}

fn strip_jsonc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                out.push(ch);
                while let Some(ch) = chars.next() {
                    out.push(ch);
                    match ch {
                        '\\' => {
                            if let Some(escaped) = chars.next() {
                                out.push(escaped);
                            }
                        }
                        '"' => break,
                        _ => {}
                    }
                }
            }
            '/' if chars.peek() == Some(&'/') => {
                for ch in chars.by_ref() {
                    if ch == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut previous = '\0';
                for ch in chars.by_ref() {
                    if previous == '*' && ch == '/' {
                        break;
                    }
                    previous = ch;
                }
                out.push(' ');
            }
            _ => out.push(ch),
        }
    }
    // Drop commas that only precede a closing bracket.
    let mut cleaned = String::with_capacity(out.len());
    let mut in_string = false;
    let mut escaped = false;
    let bytes = out.char_indices().collect::<Vec<_>>();
    for (index, &(_, ch)) in bytes.iter().enumerate() {
        if in_string {
            cleaned.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
        } else if ch == ',' {
            let next = bytes[index + 1..]
                .iter()
                .map(|&(_, ch)| ch)
                .find(|ch| !ch.is_whitespace());
            if matches!(next, Some('}' | ']')) {
                continue;
            }
        }
        cleaned.push(ch);
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_colors() {
        assert_eq!(parse_hex_color("#fff"), Some([255, 255, 255, 255]));
        assert_eq!(parse_hex_color("#1e1e1e"), Some([0x1e, 0x1e, 0x1e, 255]));
        assert_eq!(parse_hex_color("#00000080"), Some([0, 0, 0, 0x80]));
        assert_eq!(parse_hex_color("#abcd"), Some([0xaa, 0xbb, 0xcc, 0xdd]));
        assert_eq!(parse_hex_color("red"), None);
        assert_eq!(parse_hex_color("#12345"), None);
    }

    #[test]
    fn strips_comments_and_trailing_commas() {
        let text = r#"{
            // line comment
            "a": "http://x", /* block */
            "b": [1, 2,],
            "c": "trailing, }",
        }"#;
        let value: Value = serde_json::from_str(&strip_jsonc(text)).unwrap();
        assert_eq!(value["a"], "http://x");
        assert_eq!(value["b"], serde_json::json!([1, 2]));
        assert_eq!(value["c"], "trailing, }");
    }

    #[test]
    fn merges_includes_and_null_clears() {
        let files: HashMap<&str, &str> = HashMap::from([
            (
                "themes/base.json",
                r##"{ "type": "dark", "colors": { "editor.background": "#111111", "focusBorder": "#ff0000" } }"##,
            ),
            (
                "themes/child.json",
                r##"{ "name": "Child", "include": "./base.json", "colors": { "focusBorder": null, "foreground": "#eeeeee" } }"##,
            ),
        ]);
        let read = |path: &str| files.get(path).map(|text| text.as_bytes().to_vec());
        let resolved = resolve_theme(&read, "themes/child.json", 0).unwrap();
        assert_eq!(resolved.name.as_deref(), Some("Child"));
        assert_eq!(resolved.kind, Some(ThemeKind::Dark));
        assert_eq!(resolved.colors.get("editor.background").unwrap(), "#111111");
        assert_eq!(resolved.colors.get("foreground").unwrap(), "#eeeeee");
        assert!(!resolved.colors.contains_key("focusBorder"));
    }

    #[test]
    fn reads_themes_from_a_vsix() {
        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("extension/package.json", options).unwrap();
            std::io::Write::write_all(
                &mut zip,
                br#"{ "name": "night", "publisher": "Acme", "displayName": "%display%",
                     "contributes": { "themes": [
                        { "label": "Night", "uiTheme": "vs-dark", "path": "./themes/night.json" },
                        { "label": "Day", "uiTheme": "vs", "path": "./themes/day.json" } ] } }"#,
            )
            .unwrap();
            zip.start_file("extension/package.nls.json", options)
                .unwrap();
            std::io::Write::write_all(&mut zip, br#"{ "display": "Acme Themes" }"#).unwrap();
            zip.start_file("extension/themes/night.json", options)
                .unwrap();
            std::io::Write::write_all(
                &mut zip,
                br##"{ "colors": { "editor.background": "#000" } }"##,
            )
            .unwrap();
            zip.start_file("extension/themes/day.json", options)
                .unwrap();
            std::io::Write::write_all(
                &mut zip,
                br##"{ "colors": { "editor.background": "#fff" } }"##,
            )
            .unwrap();
            zip.finish().unwrap();
        }
        let themes = themes_from_vsix(
            buffer.get_ref(),
            ThemeSource::File {
                name: "acme.vsix".into(),
            },
        )
        .unwrap();
        assert_eq!(themes.len(), 2);
        assert_eq!(themes[0].id, "acme.night.night");
        assert_eq!(themes[0].kind, ThemeKind::Dark);
        assert_eq!(themes[0].extension.as_deref(), Some("Acme Themes"));
        assert_eq!(themes[1].kind, ThemeKind::Light);
        assert_eq!(
            themes[1].color("editor.background"),
            Some([255, 255, 255, 255])
        );
    }

    #[test]
    fn sanitizes_ids_and_paths() {
        assert_eq!(
            sanitize_id("Dracula Theme/Soft  Pink"),
            "dracula-theme-soft-pink"
        );
        assert_eq!(normalize_path("./themes/../themes/a.json"), "themes/a.json");
        assert_eq!(normalize_path("../../etc/x.json"), "etc/x.json");
    }
}
