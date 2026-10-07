//! Provider instances: independent copies of one provider driver.
//!
//! A [`ProviderKind`] is a *driver* (how limits are read). A
//! [`ProviderInstance`] is one configured copy of it with its own name, paths
//! and credentials, and [`ProviderId`] is the cheap key that identifies it in
//! workers, caches, popup tabs, tray indicators and Stream Deck actions.
//!
//! The first instance of a driver keeps the driver's legacy id (`claude`,
//! `codex`, ...), so persisted references written before instances existed
//! keep pointing at it.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex, RwLock},
};

use serde::{Deserialize, Serialize};

use crate::settings::{OpenRouterAccount, ProviderKind};

/// Identity of one provider instance. `Copy` so it can replace the former
/// provider enum everywhere a provider was used as a key.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProviderId {
    kind: ProviderKind,
    id: &'static str,
}

impl std::fmt::Debug for ProviderId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.id)
    }
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.id)
    }
}

impl Default for ProviderId {
    fn default() -> Self {
        Self::primary(ProviderKind::default())
    }
}

impl From<ProviderKind> for ProviderId {
    fn from(kind: ProviderKind) -> Self {
        Self::primary(kind)
    }
}

/// Instance ids are few and live for the whole process, so each distinct id
/// is leaked once to keep [`ProviderId`] `Copy`.
fn intern(id: &str) -> &'static str {
    static INTERNED: LazyLock<Mutex<BTreeSet<&'static str>>> =
        LazyLock::new(|| Mutex::new(BTreeSet::new()));
    if let Some(kind) = ProviderKind::from_id(id) {
        return kind.id();
    }
    let mut interned = INTERNED.lock().unwrap_or_else(|error| error.into_inner());
    if let Some(existing) = interned.get(id) {
        return existing;
    }
    let leaked: &'static str = Box::leak(id.to_owned().into_boxed_str());
    interned.insert(leaked);
    leaked
}

impl ProviderId {
    pub fn new(kind: ProviderKind, id: &str) -> Self {
        Self {
            kind,
            id: intern(id),
        }
    }

    /// The instance that inherits the driver's legacy id.
    pub const fn primary(kind: ProviderKind) -> Self {
        Self {
            kind,
            id: kind.id(),
        }
    }

    pub const fn kind(self) -> ProviderKind {
        self.kind
    }

    pub const fn id(self) -> &'static str {
        self.id
    }

    pub fn is_primary(self) -> bool {
        self.id == self.kind.id()
    }

    /// The user-visible instance name. Falls back to the driver name before
    /// settings were published (tests, early startup).
    pub fn display_name(self) -> String {
        labels()
            .get(self.id)
            .map(|label| label.name.clone())
            .unwrap_or_else(|| self.kind.display_name().to_owned())
    }

    /// The badge drawn on the driver icon. `None` while this instance is the
    /// only enabled instance of its driver.
    pub fn badge(self) -> Option<Badge> {
        labels().get(self.id).and_then(|label| label.badge.clone())
    }

    /// `Claude · Work` when the driver has several enabled instances,
    /// otherwise just the instance name.
    pub fn qualified_name(self) -> String {
        let name = self.display_name();
        if self.badge().is_some() && name != self.kind.display_name() {
            format!("{} \u{00b7} {name}", self.kind.display_name())
        } else {
            name
        }
    }

    /// Resolves a persisted instance id against the published settings.
    /// Before settings are published, a primary id resolves to its driver and
    /// a generated `<driver>-…` id to that driver.
    pub fn lookup(id: &str) -> Option<Self> {
        labels()
            .get(id)
            .map(|label| Self::new(label.kind, id))
            .or_else(|| ProviderKind::from_id(id).map(Self::primary))
            .or_else(|| {
                ProviderKind::ALL
                    .into_iter()
                    .filter(|kind| {
                        id.strip_prefix(kind.id())
                            .is_some_and(|rest| rest.starts_with('-'))
                    })
                    .max_by_key(|kind| kind.id().len())
                    .map(|kind| Self::new(kind, id))
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Badge {
    pub text: String,
    pub color: BadgeColor,
}

#[derive(Clone, Debug)]
struct InstanceLabel {
    kind: ProviderKind,
    name: String,
    badge: Option<Badge>,
}

static LABELS: LazyLock<RwLock<HashMap<&'static str, InstanceLabel>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Every configured instance in display order with its enabled flag.
static ORDER: LazyLock<RwLock<Vec<(ProviderId, bool)>>> = LazyLock::new(|| RwLock::new(Vec::new()));

/// Configured instances in display order, as last published. Lets surfaces
/// without a settings copy (Stream Deck bridge) enumerate instances.
pub fn published_providers() -> Vec<ProviderId> {
    ORDER
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .iter()
        .map(|(provider, _)| *provider)
        .collect()
}

/// Enabled instances in display order, as last published.
pub fn published_enabled_providers() -> Vec<ProviderId> {
    ORDER
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .iter()
        .filter(|(_, enabled)| *enabled)
        .map(|(provider, _)| *provider)
        .collect()
}

fn labels() -> std::sync::RwLockReadGuard<'static, HashMap<&'static str, InstanceLabel>> {
    LABELS.read().unwrap_or_else(|error| error.into_inner())
}

/// Publishes names and badges for every configured instance. Called whenever
/// settings are loaded or changed so all surfaces label instances alike.
pub fn publish(instances: &[ProviderInstance]) {
    let mut enabled_per_driver = HashMap::<ProviderKind, usize>::new();
    for instance in instances.iter().filter(|instance| instance.enabled) {
        *enabled_per_driver.entry(instance.driver).or_default() += 1;
    }
    let next = instances
        .iter()
        .map(|instance| {
            let shared = enabled_per_driver
                .get(&instance.driver)
                .is_some_and(|count| *count > 1);
            (
                intern(&instance.id),
                InstanceLabel {
                    kind: instance.driver,
                    name: instance.display_name(),
                    badge: shared.then(|| instance.badge()),
                },
            )
        })
        .collect();
    *LABELS.write().unwrap_or_else(|error| error.into_inner()) = next;
    *ORDER.write().unwrap_or_else(|error| error.into_inner()) = instances
        .iter()
        .map(|instance| (instance.provider_id(), instance.enabled))
        .collect();
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BadgeColor {
    /// Neutral plate that follows the theme.
    #[default]
    Auto,
    Red,
    Orange,
    Yellow,
    Green,
    Teal,
    Blue,
    Purple,
    Pink,
    /// User-picked color, packed as `0xRRGGBB`.
    Custom(u32),
}

impl BadgeColor {
    /// The presets offered next to the custom color.
    pub const PRESETS: [Self; 9] = [
        Self::Auto,
        Self::Red,
        Self::Orange,
        Self::Yellow,
        Self::Green,
        Self::Teal,
        Self::Blue,
        Self::Purple,
        Self::Pink,
    ];

    pub const fn custom(rgb: (u8, u8, u8)) -> Self {
        Self::Custom(((rgb.0 as u32) << 16) | ((rgb.1 as u32) << 8) | rgb.2 as u32)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Red => "Red",
            Self::Orange => "Orange",
            Self::Yellow => "Yellow",
            Self::Green => "Green",
            Self::Teal => "Teal",
            Self::Blue => "Blue",
            Self::Purple => "Purple",
            Self::Pink => "Pink",
            Self::Custom(_) => "Custom",
        }
    }

    /// Plate color; `None` uses the theme's neutral plate.
    pub const fn rgb(self) -> Option<(u8, u8, u8)> {
        match self {
            Self::Auto => None,
            Self::Red => Some((0xF0, 0x2D, 0x35)),
            Self::Orange => Some((0xFB, 0x9A, 0x44)),
            Self::Yellow => Some((0xE2, 0xA3, 0x00)),
            Self::Green => Some((0x30, 0xA4, 0x6C)),
            Self::Teal => Some((0x12, 0xA5, 0x94)),
            Self::Blue => Some((0x00, 0x78, 0xD4)),
            Self::Purple => Some((0x8E, 0x4E, 0xC6)),
            Self::Pink => Some((0xD6, 0x40, 0x9F)),
            Self::Custom(packed) => Some(((packed >> 16) as u8, (packed >> 8) as u8, packed as u8)),
        }
    }

    /// Letter color on the plate: white on presets; black on light custom
    /// colors so they stay readable.
    pub fn text_rgb(self) -> Option<(u8, u8, u8)> {
        let (r, g, b) = self.rgb()?;
        let light = matches!(self, Self::Custom(_))
            && 0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b) > 170.0;
        Some(if light {
            (0x1C, 0x1C, 0x1C)
        } else {
            (0xFF, 0xFF, 0xFF)
        })
    }
}

/// Where a Claude/Codex instance reads its login from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstanceSource {
    /// `CLAUDE_CONFIG_DIR` / `CODEX_HOME`. `None` selects the standard folder
    /// for the primary instance and an app-managed folder for the others.
    ConfigFolder {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<PathBuf>,
    },
    /// A pasted Claude credential kept in protected storage (limits only).
    Manual,
}

impl Default for InstanceSource {
    fn default() -> Self {
        Self::ConfigFolder { path: None }
    }
}

/// One configured provider. Field order is the order written to settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderInstance {
    pub id: String,
    pub driver: ProviderKind,
    pub name: String,
    pub enabled: bool,
    /// Empty derives the badge from the name.
    pub badge: String,
    pub badge_color: BadgeColor,
    pub show_on_home: bool,
    pub auto_activation: bool,
    pub usage_stats: bool,
    /// Counts toward the machine-wide Usage tab and Home total spend. Off
    /// keeps collecting for this instance's own page.
    pub in_usage_overview: bool,
    /// Explicit CLI or app location. Empty keeps automatic discovery.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_path: Option<PathBuf>,
    pub source: InstanceSource,
    /// Kiro reads three independent installations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kiro_crew_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kiro_cli_path: Option<PathBuf>,
    /// OpenRouter key identities; the key material lives in protected storage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openrouter: Option<OpenRouterAccount>,
    /// Advanced when this instance's credential changes so queued results from
    /// the replaced worker are rejected.
    pub credentials_revision: u64,
}

impl Default for ProviderInstance {
    fn default() -> Self {
        Self::primary(ProviderKind::default())
    }
}

impl ProviderInstance {
    pub fn primary(driver: ProviderKind) -> Self {
        Self {
            id: driver.id().into(),
            driver,
            name: driver.display_name().into(),
            enabled: false,
            badge: String::new(),
            badge_color: BadgeColor::Auto,
            show_on_home: true,
            auto_activation: false,
            usage_stats: true,
            in_usage_overview: true,
            binary_path: None,
            source: InstanceSource::default(),
            kiro_crew_path: None,
            kiro_cli_path: None,
            openrouter: (driver == ProviderKind::OpenRouter).then(OpenRouterAccount::legacy),
            credentials_revision: 0,
        }
    }

    /// A new, enabled instance with a fresh id.
    pub fn new(driver: ProviderKind, name: impl Into<String>) -> Self {
        let id = new_instance_id(driver);
        Self {
            openrouter: (driver == ProviderKind::OpenRouter)
                .then(|| OpenRouterAccount::with_id(id.clone(), "OpenRouter account")),
            id,
            name: name.into(),
            enabled: true,
            ..Self::primary(driver)
        }
    }

    pub fn provider_id(&self) -> ProviderId {
        ProviderId::new(self.driver, &self.id)
    }

    pub fn is_primary(&self) -> bool {
        self.id == self.driver.id()
    }

    pub fn display_name(&self) -> String {
        let name = self.name.trim();
        if name.is_empty() {
            self.driver.display_name().into()
        } else {
            name.into()
        }
    }

    pub fn badge(&self) -> Badge {
        Badge {
            text: badge_text(&self.badge, &self.display_name()),
            color: self.badge_color,
        }
    }

    /// The folder handed to the CLI as `CLAUDE_CONFIG_DIR` / `CODEX_HOME`.
    /// `None` means the CLI's own default location.
    pub fn config_folder(&self) -> Option<PathBuf> {
        match &self.source {
            InstanceSource::ConfigFolder { path: Some(path) } => Some(path.clone()),
            InstanceSource::ConfigFolder { path: None } if !self.is_primary() => {
                managed_folder(&self.id).ok()
            }
            _ => None,
        }
    }

    pub fn uses_manual_credential(&self) -> bool {
        self.source == InstanceSource::Manual
    }

    /// Fields that require replacing the running worker when they change.
    /// Names, badges and Home visibility are presentation-only.
    pub fn runtime_key(&self) -> impl PartialEq + use<> {
        (
            self.enabled,
            self.binary_path.clone(),
            self.source.clone(),
            self.config_folder(),
            self.kiro_crew_path.clone(),
            self.kiro_cli_path.clone(),
            self.openrouter
                .as_ref()
                .map(|account| account.api_key_ids.clone()),
            self.credentials_revision,
        )
    }

    pub fn normalize(&mut self) -> bool {
        let mut changed = false;
        if self.id.trim().is_empty() {
            self.id = new_instance_id(self.driver);
            changed = true;
        }
        let name = self.name.trim().to_owned();
        if name.is_empty() {
            self.name = self.driver.display_name().into();
            changed = true;
        } else if name != self.name {
            self.name = name;
            changed = true;
        }
        let badge = sanitize_badge(&self.badge);
        if badge != self.badge {
            self.badge = badge;
            changed = true;
        }
        if self.driver == ProviderKind::OpenRouter {
            let account = self
                .openrouter
                .get_or_insert_with(|| OpenRouterAccount::with_id(self.id.clone(), "OpenRouter"));
            changed |= account.normalize();
            if account.name != self.name {
                account.name = self.name.clone();
                changed = true;
            }
        } else if self.openrouter.take().is_some() {
            changed = true;
        }
        if self.driver != ProviderKind::Claude && self.source == InstanceSource::Manual {
            self.source = InstanceSource::default();
            changed = true;
        }
        if !crate::provider_registry::descriptor(self.driver).supports_activation
            && self.auto_activation
        {
            self.auto_activation = false;
            changed = true;
        }
        changed
    }
}

fn new_instance_id(driver: ProviderKind) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    format!("{}-{timestamp:x}-{sequence:x}", driver.id())
}

/// Up to three visible characters; whitespace is never part of a badge.
pub fn sanitize_badge(raw: &str) -> String {
    raw.chars()
        .filter(|ch| !ch.is_whitespace() && !ch.is_control())
        .take(3)
        .collect::<String>()
        .to_uppercase()
}

/// `Work` → `WO`, `Big Corp` → `BC`; an explicit badge always wins.
pub fn badge_text(explicit: &str, name: &str) -> String {
    let explicit = sanitize_badge(explicit);
    if !explicit.is_empty() {
        return explicit;
    }
    let words = name
        .split(|ch: char| ch.is_whitespace() || ch == '-' || ch == '_')
        .filter(|word| word.chars().any(char::is_alphanumeric))
        .collect::<Vec<_>>();
    let text = if words.len() >= 2 {
        words
            .iter()
            .take(2)
            .filter_map(|word| word.chars().find(|ch| ch.is_alphanumeric()))
            .collect::<String>()
    } else {
        name.chars()
            .filter(|ch| ch.is_alphanumeric())
            .take(2)
            .collect()
    };
    text.to_uppercase()
}

/// App-managed `CLAUDE_CONFIG_DIR` / `CODEX_HOME` for an instance without an
/// explicit folder.
pub fn managed_folder(id: &str) -> anyhow::Result<PathBuf> {
    use anyhow::Context;
    let dirs = directories::ProjectDirs::from("dev", "Codex Minibar", "Codex Minibar")
        .context("could not resolve the application data directory")?;
    Ok(dirs
        .data_local_dir()
        .join("instances")
        .join(sanitize_id(id)))
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

/// Canonical comparison key for config folders, so two instances of one
/// driver never read the same login (which would double-count usage).
pub fn folder_identity(path: Option<&Path>, driver: ProviderKind) -> Option<String> {
    let path = match path {
        Some(path) => path.to_path_buf(),
        None => default_folder(driver)?,
    };
    let resolved = std::fs::canonicalize(&path).unwrap_or(path);
    Some(
        resolved
            .to_string_lossy()
            .trim_end_matches(['\\', '/'])
            .to_lowercase(),
    )
}

/// The CLI's own default config folder.
pub fn default_folder(driver: ProviderKind) -> Option<PathBuf> {
    let home = directories::BaseDirs::new()?.home_dir().to_path_buf();
    match driver {
        ProviderKind::Claude => Some(home.join(".claude")),
        ProviderKind::Codex => Some(home.join(".codex")),
        _ => None,
    }
}

/// Instances of one driver that resolve to the same config folder. Keyed by
/// the later (conflicting) instance id, valued with the earlier one's name.
pub fn folder_conflicts(instances: &[ProviderInstance]) -> BTreeMap<String, String> {
    let mut seen = HashMap::<(ProviderKind, String), String>::new();
    let mut conflicts = BTreeMap::new();
    for instance in instances {
        if !matches!(instance.driver, ProviderKind::Claude | ProviderKind::Codex)
            || instance.uses_manual_credential()
        {
            continue;
        }
        let Some(identity) = folder_identity(instance.config_folder().as_deref(), instance.driver)
        else {
            continue;
        };
        match seen.get(&(instance.driver, identity.clone())) {
            Some(first) => {
                conflicts.insert(instance.id.clone(), first.clone());
            }
            None => {
                seen.insert((instance.driver, identity), instance.display_name());
            }
        }
    }
    conflicts
}

/// Feature availability for one instance. Settings show unavailable features
/// disabled with [`Capabilities::reason`] instead of hiding them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub limits: bool,
    pub usage_stats: bool,
    pub auto_activation: bool,
    pub sign_in: bool,
    pub config_folder: bool,
    pub multiple_instances: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    UsageStats,
    AutoActivation,
    SignIn,
    ConfigFolder,
}

impl Capabilities {
    pub fn of(instance: &ProviderInstance) -> Self {
        let descriptor = crate::provider_registry::descriptor(instance.driver);
        let manual = instance.uses_manual_credential();
        let folder_driver = matches!(instance.driver, ProviderKind::Claude | ProviderKind::Codex);
        // OpenCode's local history is one database for this PC; only the
        // primary instance reads it so it is never counted twice.
        let shared_local_history = matches!(
            instance.driver,
            ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo
        ) && !instance.is_primary();
        Self {
            limits: true,
            usage_stats: crate::provider_registry::supports_usage_stats(instance.driver)
                && !manual
                && !shared_local_history,
            auto_activation: descriptor.supports_activation && !manual,
            sign_in: folder_driver && !manual,
            config_folder: folder_driver && !manual,
            multiple_instances: descriptor.supports_multiple_instances,
        }
    }

    pub fn limits_only(self) -> bool {
        !self.usage_stats && !self.auto_activation && !self.sign_in
    }

    pub fn has(self, capability: Capability) -> bool {
        match capability {
            Capability::UsageStats => self.usage_stats,
            Capability::AutoActivation => self.auto_activation,
            Capability::SignIn => self.sign_in,
            Capability::ConfigFolder => self.config_folder,
        }
    }

    /// Why a feature is unavailable, or `None` when it is available.
    pub fn reason(instance: &ProviderInstance, capability: Capability) -> Option<&'static str> {
        if Self::of(instance).has(capability) {
            return None;
        }
        if instance.uses_manual_credential() {
            return Some("Not available for manual credentials. Switch Source to Config folder.");
        }
        Some(match capability {
            Capability::UsageStats
                if matches!(
                    instance.driver,
                    ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo
                ) =>
            {
                "OpenCode's local history is tracked by the first OpenCode instance."
            }
            Capability::UsageStats => "This provider has no local usage history.",
            Capability::AutoActivation => "This provider has no session window to start.",
            Capability::SignIn | Capability::ConfigFolder => {
                "This provider does not use a config folder."
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badges_use_initials_or_first_letters_and_honor_overrides() {
        assert_eq!(badge_text("", "Work"), "WO");
        assert_eq!(badge_text("", "Personal"), "PE");
        assert_eq!(badge_text("", "Big Corp"), "BC");
        assert_eq!(badge_text("", "x"), "X");
        assert_eq!(badge_text(" w 1 ", "Work"), "W1");
        assert_eq!(badge_text("abcd", "Work"), "ABC");
    }

    #[test]
    fn primary_ids_are_the_legacy_provider_ids() {
        for kind in ProviderKind::ALL {
            let id = ProviderId::primary(kind);
            assert_eq!(id.id(), kind.id());
            assert!(id.is_primary());
            assert_eq!(ProviderId::new(kind, kind.id()), id);
        }
        let other = ProviderId::new(ProviderKind::Claude, "claude-work");
        assert!(!other.is_primary());
        assert_eq!(other, ProviderId::new(ProviderKind::Claude, "claude-work"));
        assert_ne!(other, ProviderId::primary(ProviderKind::Claude));
    }

    #[test]
    fn manual_credentials_only_read_limits() {
        let mut instance = ProviderInstance::new(ProviderKind::Claude, "Web");
        assert!(!Capabilities::of(&instance).limits_only());
        instance.source = InstanceSource::Manual;
        let capabilities = Capabilities::of(&instance);
        assert!(capabilities.limits_only());
        assert!(Capabilities::reason(&instance, Capability::AutoActivation).is_some());
        assert_eq!(instance.config_folder(), None);
    }

    #[test]
    fn secondary_instances_get_isolated_managed_folders() {
        let primary = ProviderInstance::primary(ProviderKind::Codex);
        assert_eq!(primary.config_folder(), None);
        let work = ProviderInstance::new(ProviderKind::Codex, "Work");
        let folder = work.config_folder().unwrap();
        assert!(folder.ends_with(sanitize_id(&work.id)));
        let conflicts = folder_conflicts(&[primary.clone(), work.clone()]);
        assert!(conflicts.is_empty());
        let mut twin = ProviderInstance::new(ProviderKind::Codex, "Twin");
        twin.source = InstanceSource::ConfigFolder {
            path: Some(folder.clone()),
        };
        let conflicts = folder_conflicts(&[primary, work, twin.clone()]);
        assert_eq!(conflicts.get(&twin.id).map(String::as_str), Some("Work"));
    }

    #[test]
    fn unpublished_generated_ids_resolve_to_their_driver() {
        let go = ProviderId::lookup("opencode-go-1a2b-3").unwrap();
        assert_eq!(go.kind(), ProviderKind::OpenCodeGo);
        assert_eq!(go.id(), "opencode-go-1a2b-3");
        let zen = ProviderId::lookup("opencode-9f-1").unwrap();
        assert_eq!(zen.kind(), ProviderKind::OpenCodeZen);
        assert_eq!(
            ProviderId::lookup("claude"),
            Some(ProviderId::primary(ProviderKind::Claude))
        );
        assert_eq!(ProviderId::lookup("future-provider"), None);
    }

    #[test]
    fn badges_are_published_only_for_shared_drivers() {
        let mut work = ProviderInstance::new(ProviderKind::Kiro, "Work");
        work.id = "kiro-badge-fixture".into();
        let solo = work.provider_id();
        publish(std::slice::from_ref(&work));
        assert_eq!(solo.display_name(), "Work");
        assert!(solo.badge().is_none());
        let mut primary = ProviderInstance::primary(ProviderKind::Kiro);
        primary.enabled = true;
        publish(&[primary, work]);
        assert_eq!(solo.badge().unwrap().text, "WO");
        assert_eq!(solo.qualified_name(), "Kiro \u{00b7} Work");
    }
}
