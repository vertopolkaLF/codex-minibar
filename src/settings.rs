use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

pub const SETTINGS_VERSION: u32 = 21;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppTheme {
    #[default]
    Auto,
    Light,
    Dark,
}

impl AppTheme {
    pub const fn index(self) -> i32 {
        match self {
            Self::Auto => 0,
            Self::Light => 1,
            Self::Dark => 2,
        }
    }

    pub const fn from_index(index: i32) -> Self {
        match index {
            1 => Self::Light,
            2 => Self::Dark,
            _ => Self::Auto,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccentColor {
    #[default]
    Windows,
    Blue,
    Purple,
    Pink,
    Red,
    Orange,
    Green,
    Teal,
}

impl AccentColor {
    pub const fn index(self) -> i32 {
        match self {
            Self::Windows => 0,
            Self::Blue => 1,
            Self::Purple => 2,
            Self::Pink => 3,
            Self::Red => 4,
            Self::Orange => 5,
            Self::Green => 6,
            Self::Teal => 7,
        }
    }

    pub const fn from_index(index: i32) -> Self {
        match index {
            1 => Self::Blue,
            2 => Self::Purple,
            3 => Self::Pink,
            4 => Self::Red,
            5 => Self::Orange,
            6 => Self::Green,
            7 => Self::Teal,
            _ => Self::Windows,
        }
    }

    pub const fn rgb(self) -> Option<(u8, u8, u8)> {
        match self {
            Self::Windows => None,
            Self::Blue => Some((0x00, 0x78, 0xD4)),
            Self::Purple => Some((0x88, 0x17, 0x98)),
            Self::Pink => Some((0xE3, 0x00, 0x8C)),
            Self::Red => Some((0xD1, 0x34, 0x38)),
            Self::Orange => Some((0xCA, 0x50, 0x10)),
            Self::Green => Some((0x10, 0x7C, 0x10)),
            Self::Teal => Some((0x00, 0x83, 0x8C)),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitRefreshInterval {
    Seconds30,
    #[default]
    Minute1,
    Minutes5,
    Minutes10,
    Minutes15,
}

impl LimitRefreshInterval {
    pub const fn seconds(self) -> u64 {
        match self {
            Self::Seconds30 => 30,
            Self::Minute1 => 60,
            Self::Minutes5 => 5 * 60,
            Self::Minutes10 => 10 * 60,
            Self::Minutes15 => 15 * 60,
        }
    }

    pub const fn index(self) -> i32 {
        match self {
            Self::Seconds30 => 0,
            Self::Minute1 => 1,
            Self::Minutes5 => 2,
            Self::Minutes10 => 3,
            Self::Minutes15 => 4,
        }
    }

    pub const fn from_index(index: i32) -> Self {
        match index {
            0 => Self::Seconds30,
            2 => Self::Minutes5,
            3 => Self::Minutes10,
            4 => Self::Minutes15,
            _ => Self::Minute1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    #[default]
    Codex,
    Claude,
    Cursor,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderSettings {
    /// Enabled providers in the user's preferred popup/tab order.
    pub enabled: Vec<String>,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            // A new installation chooses providers during onboarding.
            enabled: Vec::new(),
        }
    }
}

impl ProviderSettings {
    pub fn is_enabled(&self, provider: ProviderKind) -> bool {
        self.enabled.iter().any(|id| id == provider.id())
    }

    pub fn set_enabled(&mut self, provider: ProviderKind, enabled: bool) {
        if enabled {
            if !self.is_enabled(provider) {
                self.enabled.push(provider.id().into());
            }
        } else {
            self.enabled.retain(|id| id != provider.id());
        }
    }

    /// Returns the provider only when there is exactly one usable choice.
    pub fn single_enabled_provider(&self) -> Option<ProviderKind> {
        if self.enabled.len() == 1 {
            self.enabled
                .first()
                .and_then(|id| ProviderKind::from_id(id))
        } else {
            None
        }
    }

    pub fn from_enabled(enabled: impl IntoIterator<Item = ProviderKind>) -> Self {
        let mut settings = Self::default();
        for provider in enabled {
            settings.set_enabled(provider, true);
        }
        settings
    }
}

impl ProviderKind {
    pub const ALL: [Self; 3] = [Self::Codex, Self::Claude, Self::Cursor];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Cursor => "cursor",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "codex" => Some(Self::Codex),
            "claude" => Some(Self::Claude),
            "cursor" => Some(Self::Cursor),
            _ => None,
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude",
            Self::Cursor => "Cursor",
        }
    }

    pub fn default_order() -> Vec<Self> {
        Self::ALL.to_vec()
    }
}

/// Ordered slots on the popup All tab, including Total Spend.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PopupWidgetKind {
    TotalSpend,
    Codex,
    Claude,
    Cursor,
}

impl PopupWidgetKind {
    pub const ALL: [Self; 4] = [Self::TotalSpend, Self::Codex, Self::Claude, Self::Cursor];

    pub fn default_order() -> Vec<Self> {
        Self::ALL.to_vec()
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::TotalSpend => "total_spend",
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Cursor => "cursor",
        }
    }

    pub const fn as_provider(self) -> Option<ProviderKind> {
        match self {
            Self::TotalSpend => None,
            Self::Codex => Some(ProviderKind::Codex),
            Self::Claude => Some(ProviderKind::Claude),
            Self::Cursor => Some(ProviderKind::Cursor),
        }
    }

    pub const fn from_provider(provider: ProviderKind) -> Self {
        match provider {
            ProviderKind::Codex => Self::Codex,
            ProviderKind::Claude => Self::Claude,
            ProviderKind::Cursor => Self::Cursor,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraySource {
    Combined,
    Primary,
    Secondary,
    PrimaryReset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayPresentation {
    StackedNumbers,
    StackedBars,
    NestedRings,
    Number,
    Bar,
    Ring,
    ResetTime,
    ResetCountdown,
}

impl TrayPresentation {
    pub const fn is_percentage(self) -> bool {
        matches!(
            self,
            Self::StackedNumbers
                | Self::StackedBars
                | Self::NestedRings
                | Self::Number
                | Self::Bar
                | Self::Ring
        )
    }

    pub const fn canonical_percentage(self) -> Self {
        match self {
            Self::StackedNumbers | Self::Number => Self::StackedNumbers,
            Self::StackedBars | Self::Bar => Self::StackedBars,
            Self::NestedRings | Self::Ring => Self::NestedRings,
            other => other,
        }
    }

    pub const fn is_reset_clock(self) -> bool {
        matches!(self, Self::ResetTime | Self::ResetCountdown)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TotalSpendPresentation {
    #[default]
    #[serde(alias = "list")]
    Donut,
    #[serde(alias = "grouped")]
    ProgressBar,
}

impl TotalSpendPresentation {
    pub const fn index(self) -> i32 {
        match self {
            Self::Donut => 0,
            Self::ProgressBar => 1,
        }
    }

    pub const fn from_index(index: i32) -> Self {
        match index {
            1 => Self::ProgressBar,
            _ => Self::Donut,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitValue {
    #[default]
    Remaining,
    Used,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayWidgetKind {
    #[default]
    Limits,
    AppIcon,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayColorMode {
    #[default]
    Status,
    Fixed,
    Provider,
    Accent,
    Monochrome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayFixedColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Default for TrayFixedColor {
    fn default() -> Self {
        Self {
            red: 0,
            green: 120,
            blue: 212,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayIndicator {
    #[serde(rename = "provider")]
    pub provider_id: String,
    pub metric_id: String,
    #[serde(default)]
    pub limit_value: LimitValue,
    #[serde(default)]
    pub color_mode: TrayColorMode,
    #[serde(default)]
    pub fixed_color: TrayFixedColor,
}

impl TrayIndicator {
    pub fn new(provider: ProviderKind, metric_id: impl Into<String>) -> Self {
        Self {
            provider_id: provider.id().into(),
            metric_id: metric_id.into(),
            limit_value: LimitValue::Remaining,
            color_mode: TrayColorMode::Status,
            fixed_color: TrayFixedColor::default(),
        }
    }

    pub fn provider(&self) -> Option<ProviderKind> {
        ProviderKind::from_id(&self.provider_id)
    }
}

fn new_tray_widget_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    format!("tray-{timestamp:x}-{sequence:x}")
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayWidget {
    #[serde(default = "new_tray_widget_id")]
    pub id: String,
    #[serde(default)]
    pub kind: TrayWidgetKind,
    #[serde(default)]
    pub indicators: Vec<TrayIndicator>,
    #[serde(default = "default_tray_presentation")]
    pub presentation: TrayPresentation,
}

fn default_tray_presentation() -> TrayPresentation {
    TrayPresentation::StackedNumbers
}

impl TrayWidget {
    pub fn default_user_widget() -> Self {
        Self::for_provider(ProviderKind::Codex)
    }

    pub fn for_provider(provider: ProviderKind) -> Self {
        let descriptor = crate::provider_registry::descriptor(provider);
        Self {
            id: new_tray_widget_id(),
            kind: TrayWidgetKind::Limits,
            indicators: descriptor
                .default_tray_metrics
                .iter()
                .take(3)
                .map(|metric| TrayIndicator::new(provider, *metric))
                .collect(),
            presentation: TrayPresentation::StackedNumbers,
        }
    }

    pub fn custom_for_provider(provider: ProviderKind) -> Self {
        let descriptor = crate::provider_registry::descriptor(provider);
        let metric = descriptor
            .default_tray_metrics
            .first()
            .copied()
            .or_else(|| descriptor.metrics.first().map(|metric| metric.id))
            .unwrap_or("unknown");
        Self {
            id: new_tray_widget_id(),
            kind: TrayWidgetKind::Limits,
            indicators: vec![TrayIndicator::new(provider, metric)],
            presentation: TrayPresentation::StackedNumbers,
        }
    }

    pub fn app_icon() -> Self {
        Self {
            id: new_tray_widget_id(),
            kind: TrayWidgetKind::AppIcon,
            indicators: Vec::new(),
            presentation: TrayPresentation::StackedNumbers,
        }
    }

    pub fn duplicate_with_new_id(&self) -> Self {
        let mut copy = self.clone();
        copy.id = new_tray_widget_id();
        copy
    }

    pub fn is_app_icon(&self) -> bool {
        self.kind == TrayWidgetKind::AppIcon
    }

    pub fn is_visible_for(&self, providers: &ProviderSettings) -> bool {
        self.is_app_icon()
            || self.indicators.iter().any(|indicator| {
                indicator
                    .provider()
                    .is_some_and(|provider| providers.is_enabled(provider))
            })
    }

    pub fn normalize(&mut self) -> bool {
        let mut changed = false;
        if self.id.is_empty() {
            self.id = new_tray_widget_id();
            changed = true;
        }
        if self.kind == TrayWidgetKind::AppIcon {
            if !self.indicators.is_empty() {
                self.indicators.clear();
                changed = true;
            }
            return changed;
        }
        if self.indicators.len() > 3 {
            self.indicators.truncate(3);
            changed = true;
        }
        if self.presentation.is_reset_clock() && self.indicators.len() > 1 {
            self.indicators.truncate(1);
            changed = true;
        }
        changed
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationSettings {
    pub activation_success: bool,
    pub activation_failure: bool,
    pub codex_unavailable: bool,
    pub approaching_reset: bool,
    /// Notify when a rate-limit window resets (`resets_at` changes).
    pub limits_changed: bool,
    /// Notify when remaining session usage drops to [`Self::low_usage_threshold_percent`].
    pub low_usage_enabled: bool,
    /// Remaining session-percent threshold for low-usage notifications (1–99).
    pub low_usage_threshold_percent: u8,
    /// Notify when remaining weekly usage drops to
    /// [`Self::weekly_low_usage_threshold_percent`].
    pub weekly_low_usage_enabled: bool,
    /// Remaining weekly-percent threshold for low-usage notifications (1–99).
    pub weekly_low_usage_threshold_percent: u8,
    /// Toast when a newer application release is discovered.
    pub update_available: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            activation_success: false,
            activation_failure: false,
            codex_unavailable: false,
            approaching_reset: false,
            limits_changed: false,
            low_usage_enabled: false,
            low_usage_threshold_percent: 20,
            weekly_low_usage_enabled: false,
            weekly_low_usage_threshold_percent: 20,
            update_available: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    /// False only while a brand-new installation is still in the first-launch
    /// flow. Keeping this persisted makes onboarding resilient to a close or
    /// reboot between its two pages.
    pub onboarding_completed: bool,
    pub theme: AppTheme,
    pub accent_color: AccentColor,
    /// App-level accessibility override. The Windows animation preference is
    /// still honored when this remains enabled.
    pub animations_enabled: bool,
    pub providers: ProviderSettings,
    /// Display order for All-tab widgets (Total Spend + providers) and footer tabs.
    pub popup_order: Vec<PopupWidgetKind>,
    pub use_colored_provider_icons: bool,
    pub replace_chatgpt_logo_with_codex: bool,
    pub automatic_activation: bool,
    pub limit_refresh_interval: LimitRefreshInterval,
    pub start_at_login: bool,
    pub show_used_percentage: bool,
    pub show_usage_pace: bool,
    pub show_banked_resets: bool,
    pub show_usage_stats: bool,
    /// Shows the compact provider spend breakdown on the popup's All tab.
    pub show_total_spend_on_all_tab: bool,
    /// Chooses between the donut and progress-bar spend layouts.
    pub total_spend_presentation: TotalSpendPresentation,
    pub show_account_name: bool,
    pub codex_path: Option<PathBuf>,
    pub tray_widgets: Vec<TrayWidget>,
    pub notifications: NotificationSettings,
    pub history_retention_days: u16,
    pub check_for_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            onboarding_completed: false,
            theme: AppTheme::Auto,
            accent_color: AccentColor::Windows,
            animations_enabled: true,
            providers: ProviderSettings::default(),
            popup_order: PopupWidgetKind::default_order(),
            use_colored_provider_icons: false,
            replace_chatgpt_logo_with_codex: false,
            automatic_activation: false,
            limit_refresh_interval: LimitRefreshInterval::default(),
            start_at_login: true,
            show_used_percentage: false,
            show_usage_pace: true,
            show_banked_resets: true,
            show_usage_stats: true,
            show_total_spend_on_all_tab: true,
            total_spend_presentation: TotalSpendPresentation::default(),
            show_account_name: false,
            codex_path: None,
            // An empty list intentionally means "show the ordinary app icon".
            tray_widgets: Vec::new(),
            notifications: NotificationSettings::default(),
            history_retention_days: 30,
            check_for_updates: true,
        }
    }
}

impl Settings {
    pub fn default_path() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("dev", "Codex Minibar", "Codex Minibar")
            .context("could not resolve the application config directory")?;
        Ok(dirs.config_dir().join("settings.toml"))
    }

    pub fn load_or_create(path: &Path) -> Result<Self> {
        if !path.exists() {
            let settings = Self::default();
            settings.save(path)?;
            return Ok(settings);
        }
        let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let mut document: toml::Value = toml::from_str(&raw).context("parse settings TOML")?;
        let original_version = document
            .get("version")
            .and_then(toml::Value::as_integer)
            .unwrap_or(0);
        // A settings file must never prevent the application from starting.
        // Serde intentionally ignores unknown fields, so a newer file can still
        // supply every option this build understands. Only migrate older files.
        let original_version = u32::try_from(original_version).unwrap_or(u32::MAX);
        if original_version < SETTINGS_VERSION {
            migrate(&mut document, original_version)?;
        }
        let mut settings: Self = document.try_into().context("decode migrated settings")?;
        settings.validate()?;
        let tray_widgets_normalized = settings.normalize_tray_widgets();
        let popup_order_normalized = settings.normalize_popup_order();
        if original_version < SETTINGS_VERSION || tray_widgets_normalized || popup_order_normalized
        {
            settings.save(path)?;
        }
        Ok(settings)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        anyhow::ensure!(
            self.version >= SETTINGS_VERSION,
            "refusing to save obsolete settings version {}",
            self.version
        );
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        let parent = path
            .parent()
            .context("settings path has no parent directory")?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .with_context(|| format!("create temporary settings in {}", parent.display()))?;
        use std::io::Write;
        temporary
            .write_all(toml::to_string_pretty(self)?.as_bytes())
            .context("write temporary settings")?;
        temporary
            .as_file()
            .sync_all()
            .context("flush temporary settings")?;
        temporary
            .persist(path)
            .with_context(|| format!("commit {}", path.display()))?;
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            (1..=365).contains(&self.history_retention_days),
            "history retention must be between 1 and 365 days"
        );
        anyhow::ensure!(
            (1..=99).contains(&self.notifications.low_usage_threshold_percent),
            "session low usage threshold must be between 1 and 99 percent"
        );
        anyhow::ensure!(
            (1..=99).contains(&self.notifications.weekly_low_usage_threshold_percent),
            "weekly low usage threshold must be between 1 and 99 percent"
        );
        Ok(())
    }

    /// Ensures `popup_order` lists every known All-tab widget exactly once.
    pub fn normalize_popup_order(&mut self) -> bool {
        let mut next = Vec::with_capacity(PopupWidgetKind::ALL.len());
        for widget in &self.popup_order {
            if PopupWidgetKind::ALL.contains(widget) && !next.contains(widget) {
                next.push(*widget);
            }
        }
        for widget in PopupWidgetKind::ALL {
            if !next.contains(&widget) {
                next.push(widget);
            }
        }
        if next == self.popup_order {
            return false;
        }
        self.popup_order = next;
        true
    }

    /// Provider subsequence of [`Self::popup_order`].
    pub fn provider_order(&self) -> Vec<ProviderKind> {
        self.popup_order
            .iter()
            .filter_map(|widget| widget.as_provider())
            .collect()
    }

    /// Enabled providers in the user's preferred display order.
    pub fn ordered_enabled_providers(&self) -> Vec<ProviderKind> {
        self.provider_order()
            .into_iter()
            .filter(|provider| self.providers.is_enabled(*provider))
            .collect()
    }

    /// Visible All-tab widgets for the current enable flags.
    pub fn ordered_visible_popup_widgets(&self, show_total_spend: bool) -> Vec<PopupWidgetKind> {
        self.popup_order
            .iter()
            .copied()
            .filter(|widget| match widget {
                PopupWidgetKind::TotalSpend => show_total_spend,
                other => other
                    .as_provider()
                    .is_some_and(|provider| self.providers.is_enabled(provider)),
            })
            .collect()
    }

    /// Moves a visible All-tab widget onto another visible widget's slot.
    pub fn move_popup_widget(
        &mut self,
        active: PopupWidgetKind,
        target: PopupWidgetKind,
        show_total_spend: bool,
    ) -> bool {
        self.normalize_popup_order();
        if active == target {
            return false;
        }
        let before_visible = self.ordered_visible_popup_widgets(show_total_spend);
        let Some(from) = before_visible.iter().position(|item| *item == active) else {
            return false;
        };
        let Some(to) = before_visible.iter().position(|item| *item == target) else {
            return false;
        };
        let mut after_visible = before_visible.clone();
        let item = after_visible.remove(from);
        after_visible.insert(to, item);
        if after_visible == before_visible {
            return false;
        }

        let visible_set: std::collections::HashSet<_> = before_visible.iter().copied().collect();
        let mut sequence = after_visible.into_iter();
        let mut rebuilt = Vec::with_capacity(self.popup_order.len());
        for widget in &self.popup_order {
            if visible_set.contains(widget) {
                if let Some(next) = sequence.next() {
                    rebuilt.push(next);
                }
            } else {
                rebuilt.push(*widget);
            }
        }
        rebuilt.extend(sequence);
        self.popup_order = rebuilt;
        true
    }

    /// Moves any provider earlier or later among provider slots in `popup_order`.
    pub fn move_provider(&mut self, provider: ProviderKind, earlier: bool) -> bool {
        self.normalize_popup_order();
        let providers = self.provider_order();
        let Some(index) = providers.iter().position(|item| *item == provider) else {
            return false;
        };
        let swap_index = if earlier {
            index.checked_sub(1)
        } else if index + 1 < providers.len() {
            Some(index + 1)
        } else {
            None
        };
        let Some(swap_index) = swap_index else {
            return false;
        };
        let left = PopupWidgetKind::from_provider(providers[index]);
        let right = PopupWidgetKind::from_provider(providers[swap_index]);
        let Some(left_pos) = self.popup_order.iter().position(|item| *item == left) else {
            return false;
        };
        let Some(right_pos) = self.popup_order.iter().position(|item| *item == right) else {
            return false;
        };
        self.popup_order.swap(left_pos, right_pos);
        true
    }

    /// Keeps persisted widget identities and cardinality valid without
    /// rewriting disabled or temporarily unavailable provider references.
    pub fn normalize_tray_widgets(&mut self) -> bool {
        let mut changed = false;
        let mut ids = std::collections::HashSet::new();
        for widget in &mut self.tray_widgets {
            changed |= widget.normalize();
            if !ids.insert(widget.id.clone()) {
                widget.id = new_tray_widget_id();
                ids.insert(widget.id.clone());
                changed = true;
            }
        }
        changed
    }

    /// Applies settings whose effect lives outside the render tree.
    pub fn apply_runtime_effects(&self) -> Result<()> {
        crate::theme::set_animations_enabled(self.animations_enabled);
        apply_startup_registration(self.start_at_login)
    }

    /// If the installer (or another tool) registered us in HKCU Run while
    /// settings still say off, adopt that into settings before we apply them —
    /// otherwise `apply_runtime_effects` would delete the Run value on launch.
    pub fn reconcile_startup_from_registry(&mut self, path: &Path) -> Result<()> {
        if self.start_at_login || !startup_registration_present()? {
            return Ok(());
        }
        self.start_at_login = true;
        self.save(path)
    }
}

#[cfg(windows)]
fn startup_registration_present() -> Result<bool> {
    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_READ, RRF_RT_REG_SZ, RegCloseKey, RegGetValueW,
            RegOpenKeyExW,
        },
    };

    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let value_name: Vec<u16> = "Codex Minibar"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut key: HKEY = std::ptr::null_mut();
    let status =
        unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut key) };
    if status != ERROR_SUCCESS {
        return Ok(false);
    }
    let mut data_size = 0u32;
    let result = unsafe {
        RegGetValueW(
            key,
            std::ptr::null(),
            value_name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut data_size,
        )
    };
    unsafe { RegCloseKey(key) };
    Ok(result == ERROR_SUCCESS)
}

#[cfg(not(windows))]
fn startup_registration_present() -> Result<bool> {
    Ok(false)
}

#[cfg(windows)]
fn apply_startup_registration(enabled: bool) -> Result<()> {
    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
            RegCreateKeyExW, RegDeleteValueW, RegSetValueExW,
        },
    };

    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let value_name: Vec<u16> = "Codex Minibar"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut key: HKEY = std::ptr::null_mut();
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            std::ptr::null_mut(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        )
    };
    anyhow::ensure!(
        status == ERROR_SUCCESS,
        "open Windows startup registry key: {status}"
    );

    let result = if enabled {
        let executable =
            std::env::current_exe().context("resolve current executable for startup")?;
        let command = format!("\"{}\"", executable.display());
        let data: Vec<u16> = command.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            RegSetValueExW(
                key,
                value_name.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * size_of::<u16>()) as u32,
            )
        }
    } else {
        unsafe { RegDeleteValueW(key, value_name.as_ptr()) }
    };
    unsafe { RegCloseKey(key) };
    // Deleting an already-absent Run value is success: the desired end state is
    // "not registered", whether we removed it now or it was never there.
    const ERROR_FILE_NOT_FOUND: u32 = 2;
    anyhow::ensure!(
        result == ERROR_SUCCESS || (!enabled && result == ERROR_FILE_NOT_FOUND),
        "update Windows startup registration: {result}"
    );
    Ok(())
}

#[cfg(not(windows))]
fn apply_startup_registration(_enabled: bool) -> Result<()> {
    Ok(())
}

fn migrate(document: &mut toml::Value, mut version: u32) -> Result<()> {
    while version < SETTINGS_VERSION {
        match version {
            // Version 0 was the pre-versioned format. All its property names remain
            // compatible; serde defaults fill newly introduced notification/update fields.
            0 => {
                document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?
                    .insert("version".into(), toml::Value::Integer(1));
                version = 1;
            }
            1 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                if let Some(toml::Value::Array(widgets)) = root.get_mut("tray_widgets") {
                    for widget in widgets {
                        let Some(widget) = widget.as_table_mut() else {
                            continue;
                        };
                        let metric = widget
                            .remove("metric")
                            .and_then(|value| value.as_str().map(str::to_owned));
                        let (source, presentation) = match metric.as_deref() {
                            Some("primary_remaining") => ("primary", "number"),
                            Some("secondary_remaining") => ("secondary", "number"),
                            Some("primary_reset") => ("primary_reset", "reset_time"),
                            Some("secondary_reset") => ("primary_reset", "reset_time"),
                            Some("combined") | _ => ("combined", "stacked_numbers"),
                        };
                        widget.insert("source".into(), toml::Value::String(source.into()));
                        widget.insert(
                            "presentation".into(),
                            toml::Value::String(presentation.into()),
                        );
                        widget.insert(
                            "limit_value".into(),
                            toml::Value::String("remaining".into()),
                        );
                    }
                }
                root.insert("version".into(), toml::Value::Integer(2));
                version = 2;
            }
            2 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                let notifications = root
                    .entry("notifications")
                    .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
                if let Some(table) = notifications.as_table_mut() {
                    table
                        .entry("update_available")
                        .or_insert(toml::Value::Boolean(true));
                }
                root.insert("version".into(), toml::Value::Integer(3));
                version = 3;
            }
            3 => {
                document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?
                    .entry("show_usage_stats")
                    .or_insert(toml::Value::Boolean(true));
                document
                    .as_table_mut()
                    .expect("settings root was checked above")
                    .insert("version".into(), toml::Value::Integer(4));
                version = 4;
            }
            4 => {
                // Usage activity is intentionally a compact recent view. This
                // setting was previously informational-only, so migrate the
                // former default rather than preserving an inaccessible 90-day
                // value.
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.insert("history_retention_days".into(), toml::Value::Integer(30));
                root.insert("version".into(), toml::Value::Integer(5));
                version = 5;
            }
            5 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("show_banked_resets")
                    .or_insert(toml::Value::Boolean(true));
                root.insert("version".into(), toml::Value::Integer(6));
                version = 6;
            }
            6 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("provider")
                    .or_insert(toml::Value::String("codex".into()));
                root.insert("version".into(), toml::Value::Integer(7));
                version = 7;
            }
            7 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                let selected = root
                    .remove("provider")
                    .and_then(|value| value.as_str().map(str::to_owned));
                let claude_enabled = selected.as_deref() == Some("claude");
                let mut providers = toml::map::Map::new();
                providers.insert(
                    "codex_enabled".into(),
                    toml::Value::Boolean(!claude_enabled),
                );
                providers.insert(
                    "claude_enabled".into(),
                    toml::Value::Boolean(claude_enabled),
                );
                root.insert("providers".into(), toml::Value::Table(providers));
                root.insert("version".into(), toml::Value::Integer(8));
                version = 8;
            }
            8 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("limit_refresh_interval")
                    .or_insert(toml::Value::String("minute1".into()));
                root.insert("version".into(), toml::Value::Integer(9));
                version = 9;
            }
            9 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("show_account_name")
                    .or_insert(toml::Value::Boolean(false));
                root.insert("version".into(), toml::Value::Integer(10));
                version = 10;
            }
            10 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                let providers = root
                    .entry("providers")
                    .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
                if let Some(providers) = providers.as_table_mut() {
                    providers
                        .entry("cursor_enabled")
                        .or_insert(toml::Value::Boolean(false));
                }
                root.insert("version".into(), toml::Value::Integer(11));
                version = 11;
            }
            11 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.remove("hide_plan_credits");
                root.insert("version".into(), toml::Value::Integer(12));
                version = 12;
            }
            12 => {
                // Do not surprise existing users with onboarding after an
                // update. Only settings files created by this version start
                // with onboarding incomplete.
                document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?
                    .insert("onboarding_completed".into(), toml::Value::Boolean(true));
                document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?
                    .insert("version".into(), toml::Value::Integer(13));
                version = 13;
            }
            13 => {
                // The All tab previously had no usage summary at all, so
                // preserve the new feature's default when existing settings
                // files are upgraded.
                document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?
                    .entry("show_combined_usage_on_all_tab")
                    .or_insert(toml::Value::Boolean(true));
                document
                    .as_table_mut()
                    .expect("settings root was checked above")
                    .insert("version".into(), toml::Value::Integer(14));
                version = 14;
            }
            14 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                let previous = root
                    .remove("show_combined_usage_on_all_tab")
                    .unwrap_or(toml::Value::Boolean(true));
                root.entry("show_total_spend_on_all_tab")
                    .or_insert(previous);
                root.insert("version".into(), toml::Value::Integer(15));
                version = 15;
            }
            15 => {
                document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?
                    .entry("total_spend_presentation")
                    .or_insert(toml::Value::String("donut".into()));
                document
                    .as_table_mut()
                    .expect("settings root was checked above")
                    .insert("version".into(), toml::Value::Integer(16));
                version = 16;
            }
            16 => {
                // Preserve the historical Codex → Claude → Cursor presentation
                // order for existing installs when provider reordering lands.
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("provider_order").or_insert_with(|| {
                    toml::Value::Array(
                        ProviderKind::ALL
                            .iter()
                            .map(|provider| toml::Value::String(provider.id().into()))
                            .collect(),
                    )
                });
                root.insert("version".into(), toml::Value::Integer(17));
                version = 17;
            }
            17 => {
                // Promote provider_order into popup_order and pin Total Spend
                // above the historical provider stack.
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                let providers = root
                    .remove("provider_order")
                    .and_then(|value| value.as_array().cloned())
                    .unwrap_or_else(|| {
                        ProviderKind::ALL
                            .iter()
                            .map(|provider| toml::Value::String(provider.id().into()))
                            .collect()
                    });
                let mut popup_order = vec![toml::Value::String("total_spend".into())];
                for provider in providers {
                    if let Some(id) = provider.as_str() {
                        if matches!(id, "codex" | "claude" | "cursor") {
                            popup_order.push(toml::Value::String(id.into()));
                        }
                    }
                }
                for provider in ProviderKind::ALL {
                    let id = provider.id();
                    if !popup_order.iter().any(|value| value.as_str() == Some(id)) {
                        popup_order.push(toml::Value::String(id.into()));
                    }
                }
                root.insert("popup_order".into(), toml::Value::Array(popup_order));
                root.insert("version".into(), toml::Value::Integer(18));
                version = 18;
            }
            18 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                root.entry("theme")
                    .or_insert(toml::Value::String("auto".into()));
                root.entry("accent_color")
                    .or_insert(toml::Value::String("windows".into()));
                root.entry("animations_enabled")
                    .or_insert(toml::Value::Boolean(true));
                root.insert("version".into(), toml::Value::Integer(19));
                version = 19;
            }
            19 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                let provider_flags = root
                    .get("providers")
                    .and_then(toml::Value::as_table)
                    .cloned()
                    .unwrap_or_default();
                let popup_order = root
                    .get("popup_order")
                    .and_then(toml::Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let mut enabled = Vec::new();
                for value in popup_order {
                    let Some(id) = value.as_str() else {
                        continue;
                    };
                    let flag = format!("{id}_enabled");
                    if provider_flags
                        .get(&flag)
                        .and_then(toml::Value::as_bool)
                        .unwrap_or(false)
                    {
                        enabled.push(toml::Value::String(id.into()));
                    }
                }
                for provider in ProviderKind::ALL {
                    let id = provider.id();
                    let flag = format!("{id}_enabled");
                    if provider_flags
                        .get(&flag)
                        .and_then(toml::Value::as_bool)
                        .unwrap_or(false)
                        && !enabled.iter().any(|value| value.as_str() == Some(id))
                    {
                        enabled.push(toml::Value::String(id.into()));
                    }
                }
                let mut providers = toml::map::Map::new();
                providers.insert("enabled".into(), toml::Value::Array(enabled));
                root.insert("providers".into(), toml::Value::Table(providers));
                if let Some(toml::Value::Array(widgets)) = root.get_mut("tray_widgets") {
                    for (index, value) in widgets.iter_mut().enumerate() {
                        let Some(widget) = value.as_table_mut() else {
                            continue;
                        };
                        let provider = widget
                            .remove("provider")
                            .and_then(|value| value.as_str().map(str::to_owned))
                            .unwrap_or_else(|| "codex".into());
                        let source = widget
                            .remove("source")
                            .and_then(|value| value.as_str().map(str::to_owned))
                            .unwrap_or_else(|| "combined".into());
                        let limit_value = widget
                            .remove("limit_value")
                            .and_then(|value| value.as_str().map(str::to_owned))
                            .unwrap_or_else(|| "remaining".into());
                        let primary = match provider.as_str() {
                            "claude" => "claude.session",
                            "cursor" => "cursor.auto",
                            _ => "codex.session",
                        };
                        let secondary = match provider.as_str() {
                            "claude" => "claude.weekly",
                            "cursor" => "cursor.auto",
                            _ => "codex.weekly",
                        };
                        let metric_ids: Vec<&str> = match source.as_str() {
                            "primary" | "primary_reset" => vec![primary],
                            "secondary" => vec![secondary],
                            _ if primary == secondary => vec![primary],
                            _ => vec![primary, secondary],
                        };
                        let indicators = metric_ids
                            .into_iter()
                            .map(|metric_id| {
                                let mut indicator = toml::map::Map::new();
                                indicator.insert(
                                    "provider".into(),
                                    toml::Value::String(provider.clone()),
                                );
                                indicator.insert(
                                    "metric_id".into(),
                                    toml::Value::String(metric_id.into()),
                                );
                                indicator.insert(
                                    "limit_value".into(),
                                    toml::Value::String(limit_value.clone()),
                                );
                                toml::Value::Table(indicator)
                            })
                            .collect();
                        widget.insert(
                            "id".into(),
                            toml::Value::String(format!("legacy-tray-{index}")),
                        );
                        widget.insert("kind".into(), toml::Value::String("limits".into()));
                        widget.insert("indicators".into(), toml::Value::Array(indicators));
                        widget.insert("color_mode".into(), toml::Value::String("status".into()));
                    }
                }
                root.insert("version".into(), toml::Value::Integer(20));
                version = 20;
            }
            20 => {
                let root = document
                    .as_table_mut()
                    .context("settings root must be a TOML table")?;
                if let Some(toml::Value::Array(widgets)) = root.get_mut("tray_widgets") {
                    for value in widgets.iter_mut() {
                        let Some(widget) = value.as_table_mut() else {
                            continue;
                        };
                        let color_mode = widget
                            .remove("color_mode")
                            .unwrap_or_else(|| toml::Value::String("status".into()));
                        let fixed_color = widget.remove("fixed_color");
                        let Some(toml::Value::Array(indicators)) = widget.get_mut("indicators")
                        else {
                            continue;
                        };
                        for indicator_value in indicators.iter_mut() {
                            let Some(indicator) = indicator_value.as_table_mut() else {
                                continue;
                            };
                            indicator
                                .entry("color_mode")
                                .or_insert(color_mode.clone());
                            if let Some(fixed_color) = fixed_color.clone() {
                                indicator.entry("fixed_color").or_insert(fixed_color);
                            }
                        }
                    }
                }
                root.insert("version".into(), toml::Value::Integer(21));
                version = 21;
            }
            unsupported => anyhow::bail!("no migration path from settings version {unsupported}"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_product_decisions() {
        let value = Settings::default();
        assert!(!value.providers.is_enabled(ProviderKind::Codex));
        assert!(!value.providers.is_enabled(ProviderKind::Claude));
        assert_eq!(value.theme, AppTheme::Auto);
        assert_eq!(value.accent_color, AccentColor::Windows);
        assert!(value.animations_enabled);
        assert!(!value.use_colored_provider_icons);
        assert!(!value.replace_chatgpt_logo_with_codex);
        assert!(!value.automatic_activation);
        assert_eq!(value.limit_refresh_interval, LimitRefreshInterval::Minute1);
        assert!(value.start_at_login);
        assert!(!value.show_used_percentage);
        assert!(value.show_usage_pace);
        assert!(value.show_banked_resets);
        assert!(value.show_usage_stats);
        assert!(value.show_total_spend_on_all_tab);
        assert_eq!(
            value.total_spend_presentation,
            TotalSpendPresentation::Donut
        );
        assert_eq!(value.history_retention_days, 30);
        assert!(value.tray_widgets.is_empty());
        assert_eq!(value.popup_order, PopupWidgetKind::default_order());
        assert!(!value.notifications.activation_success);
        assert!(!value.notifications.activation_failure);
        assert!(!value.notifications.codex_unavailable);
        assert!(!value.notifications.approaching_reset);
        assert!(!value.notifications.limits_changed);
        assert!(!value.notifications.low_usage_enabled);
        assert_eq!(value.notifications.low_usage_threshold_percent, 20);
        assert!(!value.notifications.weekly_low_usage_enabled);
        assert_eq!(value.notifications.weekly_low_usage_threshold_percent, 20);
        assert!(value.notifications.update_available);
    }

    #[test]
    fn round_trips_through_disk() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        let expected = Settings::default();
        expected.save(&path).unwrap();
        assert_eq!(Settings::load_or_create(&path).unwrap(), expected);
    }

    #[test]
    fn migrates_pre_versioned_settings_and_rewrites_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        fs::write(
            &path,
            r#"
automatic_activation = false
start_at_login = true
history_retention_days = 30
check_for_updates = false
tray_widgets = []
"#,
        )
        .unwrap();

        let migrated = Settings::load_or_create(&path).unwrap();
        assert_eq!(migrated.version, SETTINGS_VERSION);
        assert!(!migrated.automatic_activation);
        assert!(migrated.start_at_login);
        assert!(migrated.show_usage_pace);
        assert!(migrated.show_banked_resets);
        assert!(migrated.show_usage_stats);
        assert!(migrated.show_total_spend_on_all_tab);
        assert_eq!(
            migrated.total_spend_presentation,
            TotalSpendPresentation::Donut
        );
        assert_eq!(migrated.history_retention_days, 30);
        assert!(
            fs::read_to_string(path)
                .unwrap()
                .contains(&format!("version = {SETTINGS_VERSION}"))
        );
    }

    #[test]
    fn migrates_the_previous_ninety_day_default_to_thirty_days() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        fs::write(&path, "version = 4\nhistory_retention_days = 90\n").unwrap();

        let migrated = Settings::load_or_create(&path).unwrap();
        assert_eq!(migrated.version, SETTINGS_VERSION);
        assert_eq!(migrated.history_retention_days, 30);
    }

    #[test]
    fn total_spend_presentation_uses_stable_dropdown_indices() {
        assert_eq!(TotalSpendPresentation::Donut.index(), 0);
        assert_eq!(TotalSpendPresentation::ProgressBar.index(), 1);
        assert_eq!(
            TotalSpendPresentation::from_index(1),
            TotalSpendPresentation::ProgressBar
        );
        assert_eq!(
            TotalSpendPresentation::from_index(99),
            TotalSpendPresentation::Donut
        );
    }

    #[test]
    fn migrates_combined_usage_toggle_to_total_spend_toggle() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        fs::write(
            &path,
            "version = 14\nshow_combined_usage_on_all_tab = false\n",
        )
        .unwrap();

        let migrated = Settings::load_or_create(&path).unwrap();
        assert!(!migrated.show_total_spend_on_all_tab);
        let rewritten = fs::read_to_string(path).unwrap();
        assert!(rewritten.contains("show_total_spend_on_all_tab = false"));
        assert!(!rewritten.contains("show_combined_usage_on_all_tab"));
    }

    #[test]
    fn migrates_single_claude_selection_to_the_provider_toggle_list() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        fs::write(&path, "version = 7\nprovider = 'claude'\n").unwrap();

        let migrated = Settings::load_or_create(&path).unwrap();
        assert!(!migrated.providers.is_enabled(ProviderKind::Codex));
        assert!(migrated.providers.is_enabled(ProviderKind::Claude));
        assert!(
            fs::read_to_string(path)
                .unwrap()
                .contains(&format!("version = {SETTINGS_VERSION}"))
        );
    }

    #[test]
    fn accepts_newer_settings_versions_and_ignores_unknown_options() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        fs::write(
            &path,
            "version = 999\nfuture_option = true\nhistory_retention_days = 30\n",
        )
        .unwrap();
        let settings = Settings::load_or_create(&path).unwrap();
        assert_eq!(settings.version, 999);
        assert_eq!(settings.history_retention_days, 30);
    }

    #[test]
    fn validates_retention_range() {
        let settings = Settings {
            history_retention_days: 0,
            ..Settings::default()
        };
        assert!(settings.validate().is_err());
    }

    #[test]
    fn normalizes_and_reorders_popup_order() {
        let mut settings = Settings {
            popup_order: vec![
                PopupWidgetKind::Cursor,
                PopupWidgetKind::Cursor,
                PopupWidgetKind::Codex,
            ],
            providers: ProviderSettings::from_enabled([ProviderKind::Codex, ProviderKind::Cursor]),
            show_total_spend_on_all_tab: true,
            ..Settings::default()
        };
        assert!(settings.normalize_popup_order());
        assert_eq!(
            settings.popup_order,
            vec![
                PopupWidgetKind::Cursor,
                PopupWidgetKind::Codex,
                PopupWidgetKind::TotalSpend,
                PopupWidgetKind::Claude,
            ]
        );
        assert!(settings.move_popup_widget(
            PopupWidgetKind::Codex,
            PopupWidgetKind::TotalSpend,
            true
        ));
        assert_eq!(
            settings.ordered_visible_popup_widgets(true),
            vec![
                PopupWidgetKind::Cursor,
                PopupWidgetKind::TotalSpend,
                PopupWidgetKind::Codex,
            ]
        );
        assert!(settings.move_provider(ProviderKind::Codex, true));
        assert_eq!(
            settings.ordered_enabled_providers(),
            vec![ProviderKind::Codex, ProviderKind::Cursor]
        );
    }

    #[test]
    fn disabled_provider_references_are_preserved() {
        let mut settings = Settings {
            providers: ProviderSettings::from_enabled([ProviderKind::Claude]),
            tray_widgets: vec![TrayWidget::default_user_widget()],
            ..Settings::default()
        };

        assert!(!settings.normalize_tray_widgets());
        assert_eq!(
            settings.tray_widgets[0].indicators[0].provider(),
            Some(ProviderKind::Codex)
        );
    }

    #[test]
    fn loading_preserves_a_widget_for_a_disabled_provider() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        let stale = Settings {
            providers: ProviderSettings::from_enabled([ProviderKind::Claude]),
            tray_widgets: vec![TrayWidget::default_user_widget()],
            ..Settings::default()
        };
        stale.save(&path).unwrap();

        let loaded = Settings::load_or_create(&path).unwrap();

        assert_eq!(
            loaded.tray_widgets[0].indicators[0].provider(),
            Some(ProviderKind::Codex)
        );
        assert!(
            fs::read_to_string(path)
                .unwrap()
                .contains("provider = \"codex\"")
        );
    }

    #[test]
    fn unknown_provider_and_metric_ids_round_trip_without_data_loss() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        let mut settings = Settings::default();
        settings.version = SETTINGS_VERSION;
        settings.providers.enabled.push("future-provider".into());
        let mut widget = TrayWidget::custom_for_provider(ProviderKind::Codex);
        widget.indicators[0].provider_id = "future-provider".into();
        widget.indicators[0].metric_id = "future-provider.daily".into();
        settings.tray_widgets.push(widget);
        settings.save(&path).unwrap();

        let loaded = Settings::load_or_create(&path).unwrap();

        assert!(loaded.providers.enabled.contains(&"future-provider".into()));
        assert_eq!(
            loaded.tray_widgets[0].indicators[0].provider_id,
            "future-provider"
        );
        assert_eq!(
            loaded.tray_widgets[0].indicators[0].metric_id,
            "future-provider.daily"
        );
    }

    #[test]
    fn migrates_v19_tray_widgets_to_ordered_indicators() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        fs::write(
            &path,
            r#"version = 19
popup_order = ["total_spend", "claude", "codex", "cursor"]
tray_widgets = [{ provider = "claude", source = "combined", presentation = "stacked_numbers", limit_value = "used" }]

[providers]
codex_enabled = false
claude_enabled = true
cursor_enabled = false
"#,
        )
        .unwrap();

        let loaded = Settings::load_or_create(&path).unwrap();

        assert_eq!(loaded.providers.enabled, vec!["claude"]);
        assert_eq!(loaded.tray_widgets.len(), 1);
        assert_eq!(loaded.tray_widgets[0].id, "legacy-tray-0");
        assert_eq!(loaded.tray_widgets[0].indicators.len(), 2);
        assert_eq!(
            loaded.tray_widgets[0].indicators[0].metric_id,
            "claude.session"
        );
        assert_eq!(
            loaded.tray_widgets[0].indicators[1].metric_id,
            "claude.weekly"
        );
        assert_eq!(
            loaded.tray_widgets[0].indicators[0].limit_value,
            LimitValue::Used
        );
    }

    #[test]
    fn migrates_v20_widget_color_onto_each_indicator() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        fs::write(
            &path,
            r#"version = 20
tray_widgets = [
  { id = "tray-1", kind = "limits", presentation = "stacked_numbers", color_mode = "provider",
    indicators = [
      { provider = "codex", metric_id = "codex.session", limit_value = "remaining" },
      { provider = "claude", metric_id = "claude.session", limit_value = "used" }
    ] }
]
"#,
        )
        .unwrap();

        let loaded = Settings::load_or_create(&path).unwrap();

        assert_eq!(loaded.version, SETTINGS_VERSION);
        assert_eq!(loaded.tray_widgets.len(), 1);
        assert_eq!(
            loaded.tray_widgets[0].indicators[0].color_mode,
            TrayColorMode::Provider
        );
        assert_eq!(
            loaded.tray_widgets[0].indicators[1].color_mode,
            TrayColorMode::Provider
        );
    }
}
