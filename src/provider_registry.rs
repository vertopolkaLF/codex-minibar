use crate::limits::{AdditionalLimit, LimitWindow, RateLimits};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

/// When false (the default), Codex uses the ChatGPT mark. Settings flips this
/// so every surface that calls [`icon`] updates together.
static REPLACE_CHATGPT_LOGO_WITH_CODEX: AtomicBool = AtomicBool::new(false);

#[derive(
    Clone, Copy, Debug, Default, Hash, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    #[default]
    Codex,
    Claude,
    Cursor,
    OpenCodeZen,
    OpenCodeGo,
    #[serde(rename = "openrouter")]
    OpenRouter,
    Antigravity,
    Grok,
    Kiro,
    CommandCode,
}

impl ProviderKind {
    pub const ALL: [Self; 10] = [
        Self::Codex,
        Self::Claude,
        Self::Cursor,
        Self::OpenCodeZen,
        Self::OpenCodeGo,
        Self::OpenRouter,
        Self::Antigravity,
        Self::Grok,
        Self::Kiro,
        Self::CommandCode,
    ];

    pub const fn id(self) -> &'static str {
        descriptor(self).id
    }

    pub fn from_id(id: &str) -> Option<Self> {
        PROVIDERS
            .iter()
            .find(|descriptor| descriptor.id == id)
            .map(|descriptor| descriptor.kind)
    }

    pub const fn display_name(self) -> &'static str {
        descriptor(self).display_name
    }
}

// Additional-limit ids reported by provider APIs. Provider clients emit them
// and the metric tables below map them onto stable widget metrics.
pub const CURSOR_API_LIMIT_ID: &str = "cursor-api";
pub const CURSOR_ALL_MODELS_LIMIT_ID: &str = "cursor-all-models";
pub const CURSOR_GROK_BOT_LIMIT_ID: &str = "cursor-grok-bot";
pub const OPENCODE_GO_MONTHLY_LIMIT_ID: &str = "monthly";
pub const COMMAND_CODE_MONTHLY_LIMIT_ID: &str = "monthly";
/// Claude's additional window that carries cloud-session credit dollars.
pub const CLAUDE_CLOUD_SESSION_CREDITS_LIMIT_ID: &str = "iguana_necktie";

/// Provider-independent identity for a quota window exposed to popup, tray,
/// settings, and future provider surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricSource {
    Primary,
    Secondary,
    Additional(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MetricDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub label_key: &'static str,
    pub source: MetricSource,
    /// Whether a fresh install shows the metric's popup brick.
    pub visible_by_default: bool,
}

impl MetricDescriptor {
    const fn new(
        id: &'static str,
        label: &'static str,
        label_key: &'static str,
        source: MetricSource,
    ) -> Self {
        Self {
            id,
            label,
            label_key,
            source,
            visible_by_default: true,
        }
    }

    pub fn localized_label(&self) -> &'static str {
        crate::i18n::tr(self.label_key)
    }

    /// A few characters for tight spaces: `5h` and `7d` for the standard
    /// session and weekly windows, the full label otherwise.
    pub fn localized_short_label(&self) -> &'static str {
        match self.label_key {
            "msg-5h-session" => crate::i18n::tr("short-5h"),
            "weekly" => crate::i18n::tr("short-7d"),
            _ => self.localized_label(),
        }
    }
}

pub type Rgb = (u8, u8, u8);

/// A color for light and for dark surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemedRgb {
    pub light: Rgb,
    pub dark: Rgb,
}

impl ThemedRgb {
    const fn same(rgb: Rgb) -> Self {
        Self {
            light: rgb,
            dark: rgb,
        }
    }

    pub const fn pick(self, dark: bool) -> Rgb {
        if dark { self.dark } else { self.light }
    }
}

/// How the popup's banked-reset card can redeem a credit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BankedResets {
    Unsupported,
    /// The provider picks the credit; a positive count is enough.
    ByCount,
    /// Redemption needs at least one credit listed as available.
    ByListedCredit,
}

/// What the provider's credits balance counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Credits {
    Unsupported,
    /// A plain number, shown rounded with grouped digits.
    Count,
    /// Shown exactly as the provider reports it (it may carry a currency).
    AsReported,
}

/// Where Usage Stats come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageStats {
    Unsupported,
    /// Scanned from the provider's local session history.
    Local,
    /// Downloaded from an export that may lag behind; the card stays visible
    /// while the export is still empty.
    RemoteExport,
    /// Read from the provider's API with a management key.
    ManagementKey,
}

impl UsageStats {
    pub const fn is_supported(self) -> bool {
        !matches!(self, Self::Unsupported)
    }
}

/// How the usage card values a period.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageValue {
    /// Tokens, priced from the public catalog as an approximation.
    EstimatedTokens,
    /// Tokens with the cost the provider billed.
    BilledTokens,
    /// Billed per request: spend and request count lead.
    Spend,
}

/// i18n keys of the popup's monthly, 5-hour and secondary quota card titles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimitTitles {
    pub monthly: &'static str,
    pub five_hour: &'static str,
    pub secondary: &'static str,
}

impl LimitTitles {
    const STANDARD: Self = Self::all("monthly", "msg-5h-session", "weekly");

    const fn all(monthly: &'static str, five_hour: &'static str, secondary: &'static str) -> Self {
        Self {
            monthly,
            five_hour,
            secondary,
        }
    }

    const fn single(key: &'static str) -> Self {
        Self::all(key, key, key)
    }
}

/// A provider's public status page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusPage {
    pub page_url: &'static str,
    pub api: StatusApi,
    pub relevance: StatusRelevance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusApi {
    /// Atlassian Statuspage `api/v2/summary.json`.
    Statuspage(&'static str),
    /// incident.io's public page proxy. Its Statuspage-compatible endpoint
    /// omits component groups, which is where OpenAI keeps Codex.
    IncidentIo(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusRelevance {
    /// Component (or its group) names containing one of these, ignoring case.
    Matching(&'static [&'static str]),
    /// Every component except these exact names.
    Except(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderDescriptor {
    pub kind: ProviderKind,
    pub id: &'static str,
    pub display_name: &'static str,
    pub icon: &'static str,
    /// Source brand color for charts, the tray and external widgets.
    pub brand_rgb: Rgb,
    /// Small provider marks on popup cards and Settings rows, tuned to stay
    /// visible on each surface (monochrome marks turn white on dark).
    pub icon_rgb: ThemedRgb,
    /// Share-bar / donut / legend color on combined spend surfaces.
    pub spend_rgb: ThemedRgb,
    /// Area-chart series color on the Usage tab.
    pub series_rgb: ThemedRgb,
    /// Whether the provider exposes a real operation that starts a fresh
    /// session limit window.
    pub supports_activation: bool,
    /// Whether the provider contributes date-scoped token history to Usage Stats.
    pub include_in_total_spend: bool,
    /// Whether more than one instance can be configured. Only drivers with an
    /// isolating parameter (config folder or API key) qualify.
    pub supports_multiple_instances: bool,
    /// Whether instances read a CLI login folder (sign-in, folder settings,
    /// folder conflicts).
    pub config_folder: bool,
    pub banked_resets: BankedResets,
    pub credits: Credits,
    pub usage_stats: UsageStats,
    pub usage_value: UsageValue,
    /// Whether the popup shows per-key spending strips.
    pub spending_strips: bool,
    /// Whether glanceable surfaces show the account balance instead of quota.
    pub balance_only: bool,
    /// Whether the quota is a single monthly window on every plan.
    pub monthly_only: bool,
    pub limit_titles: LimitTitles,
    pub default_refresh_seconds: u64,
    /// File name of the primary instance's activation state.
    pub activation_state_file: &'static str,
    /// Catalog namespaces tried, in order, to price a bare model name.
    pub pricing_prefixes: &'static [&'static str],
    pub status_page: Option<StatusPage>,
    /// Stable metrics shown before runtime-discovered provider-specific lanes.
    pub metrics: &'static [MetricDescriptor],
    /// Ordered metrics used by onboarding and the provider preset.
    pub default_tray_metrics: &'static [&'static str],
}

const CODEX_METRICS: &[MetricDescriptor] = &[
    MetricDescriptor::new(
        "codex.session",
        "5h session",
        "msg-5h-session",
        MetricSource::Primary,
    ),
    MetricDescriptor::new("codex.weekly", "Weekly", "weekly", MetricSource::Secondary),
    MetricDescriptor::new(
        "codex.lunaReserve",
        "Luna Reserve",
        "luna-reserve",
        MetricSource::Additional(crate::limits::GPT_RESERVE_LIMIT_ID),
    ),
];

const CLAUDE_METRICS: &[MetricDescriptor] = &[
    MetricDescriptor::new(
        "claude.session",
        "5h session",
        "msg-5h-session",
        MetricSource::Primary,
    ),
    MetricDescriptor::new("claude.weekly", "Weekly", "weekly", MetricSource::Secondary),
];

const CURSOR_METRICS: &[MetricDescriptor] = &[
    MetricDescriptor::new(
        "cursor.auto",
        "Cursor Models",
        "cursor-models",
        MetricSource::Secondary,
    ),
    MetricDescriptor::new(
        "cursor.api",
        "Other Models",
        "other-models",
        MetricSource::Additional(CURSOR_API_LIMIT_ID),
    ),
    MetricDescriptor {
        visible_by_default: false,
        ..MetricDescriptor::new(
            "cursor.allModels",
            "All Models",
            "all-models",
            MetricSource::Additional(CURSOR_ALL_MODELS_LIMIT_ID),
        )
    },
    MetricDescriptor::new(
        "cursor.grokBot",
        "Grok Bot",
        "grok-bot",
        MetricSource::Additional(CURSOR_GROK_BOT_LIMIT_ID),
    ),
];

// Zen has no authoritative percentage/reset windows exposed by its API.
// Local spend is rendered in the popup activity card, not as a fake tray
// quota.
const OPENCODE_ZEN_METRICS: &[MetricDescriptor] = &[];

const OPENCODE_GO_METRICS: &[MetricDescriptor] = &[
    MetricDescriptor::new(
        "opencode-go.session",
        "5h session",
        "msg-5h-session",
        MetricSource::Primary,
    ),
    MetricDescriptor::new(
        "opencode-go.weekly",
        "Weekly",
        "weekly",
        MetricSource::Secondary,
    ),
    MetricDescriptor::new(
        "opencode-go.monthly",
        "Monthly",
        "monthly",
        MetricSource::Additional(OPENCODE_GO_MONTHLY_LIMIT_ID),
    ),
];

const OPENROUTER_METRICS: &[MetricDescriptor] = &[MetricDescriptor::new(
    "openrouter.limit",
    "Spending limit",
    "spending-limit",
    MetricSource::Primary,
)];

const ANTIGRAVITY_METRICS: &[MetricDescriptor] = &[
    MetricDescriptor::new(
        "antigravity.gemini",
        "Gemini",
        "gemini",
        MetricSource::Primary,
    ),
    MetricDescriptor::new(
        "antigravity.thirdParty",
        "Claude + GPT",
        "claude-gpt",
        MetricSource::Secondary,
    ),
];

const GROK_METRICS: &[MetricDescriptor] = &[MetricDescriptor::new(
    "grok.credits",
    "Credits",
    "credits",
    MetricSource::Primary,
)];

// Keep the original Kiro metric id so existing tray and popup visibility
// settings survive the switch from an extra lane to the primary monthly quota.
const KIRO_METRICS: &[MetricDescriptor] = &[MetricDescriptor::new(
    "kiro.additional.credits",
    "Monthly credits",
    "monthly-credits",
    MetricSource::Secondary,
)];

const COMMAND_CODE_METRICS: &[MetricDescriptor] = &[
    MetricDescriptor::new(
        "command-code.session",
        "5h session",
        "msg-5h-session",
        MetricSource::Primary,
    ),
    MetricDescriptor::new(
        "command-code.weekly",
        "Weekly",
        "weekly",
        MetricSource::Secondary,
    ),
    MetricDescriptor::new(
        "command-code.monthly",
        "Monthly",
        "monthly",
        MetricSource::Additional(COMMAND_CODE_MONTHLY_LIMIT_ID),
    ),
];

const WHITE: Rgb = (255, 255, 255);

/// Indexed by `ProviderKind as usize`.
pub const PROVIDERS: &[ProviderDescriptor] = &[
    ProviderDescriptor {
        kind: ProviderKind::Codex,
        id: "codex",
        display_name: "Codex",
        icon: "codex",
        brand_rgb: (128, 159, 255),
        icon_rgb: ThemedRgb {
            light: (107, 132, 244),
            dark: (128, 159, 255),
        },
        spend_rgb: ThemedRgb::same((128, 159, 255)),
        series_rgb: ThemedRgb::same((128, 159, 255)),
        supports_activation: true,
        include_in_total_spend: true,
        supports_multiple_instances: true,
        config_folder: true,
        banked_resets: BankedResets::ByCount,
        credits: Credits::Count,
        usage_stats: UsageStats::Local,
        usage_value: UsageValue::EstimatedTokens,
        spending_strips: false,
        balance_only: false,
        monthly_only: false,
        limit_titles: LimitTitles::STANDARD,
        default_refresh_seconds: 60,
        // The file Codex used before other providers existed, so existing
        // installs keep their activation baseline.
        activation_state_file: "activation.toml",
        pricing_prefixes: &["openai"],
        status_page: Some(StatusPage {
            page_url: "https://status.openai.com",
            api: StatusApi::IncidentIo("https://status.openai.com/proxy/status.openai.com"),
            relevance: StatusRelevance::Matching(&["codex"]),
        }),
        metrics: CODEX_METRICS,
        default_tray_metrics: &["codex.session", "codex.weekly"],
    },
    ProviderDescriptor {
        kind: ProviderKind::Claude,
        id: "claude",
        display_name: "Claude",
        icon: "claude",
        brand_rgb: (217, 119, 87),
        icon_rgb: ThemedRgb::same((217, 119, 87)),
        spend_rgb: ThemedRgb::same((217, 119, 87)),
        series_rgb: ThemedRgb::same((217, 119, 87)),
        supports_activation: true,
        include_in_total_spend: true,
        supports_multiple_instances: true,
        config_folder: true,
        banked_resets: BankedResets::ByListedCredit,
        credits: Credits::AsReported,
        usage_stats: UsageStats::Local,
        usage_value: UsageValue::EstimatedTokens,
        spending_strips: false,
        balance_only: false,
        monthly_only: false,
        limit_titles: LimitTitles::STANDARD,
        // Claude answers frequent quota reads with HTTP 429.
        default_refresh_seconds: 2 * 60,
        activation_state_file: "activation-claude.toml",
        pricing_prefixes: &["anthropic"],
        status_page: Some(StatusPage {
            page_url: "https://status.claude.com",
            api: StatusApi::Statuspage("https://status.claude.com/api/v2/summary.json"),
            relevance: StatusRelevance::Matching(&["claude code", "claude api", "claude.ai"]),
        }),
        metrics: CLAUDE_METRICS,
        default_tray_metrics: &["claude.session", "claude.weekly"],
    },
    ProviderDescriptor {
        kind: ProviderKind::Cursor,
        id: "cursor",
        display_name: "Cursor",
        icon: "cursor",
        brand_rgb: (145, 151, 164),
        icon_rgb: ThemedRgb {
            light: (51, 51, 51),
            dark: WHITE,
        },
        spend_rgb: ThemedRgb {
            light: (18, 18, 18),
            dark: WHITE,
        },
        series_rgb: ThemedRgb {
            light: (28, 28, 28),
            dark: WHITE,
        },
        supports_activation: false,
        include_in_total_spend: true,
        supports_multiple_instances: false,
        config_folder: false,
        banked_resets: BankedResets::Unsupported,
        credits: Credits::AsReported,
        usage_stats: UsageStats::RemoteExport,
        usage_value: UsageValue::EstimatedTokens,
        spending_strips: false,
        balance_only: false,
        monthly_only: false,
        limit_titles: LimitTitles::single("cursor-models"),
        default_refresh_seconds: 60,
        activation_state_file: "activation-cursor.toml",
        pricing_prefixes: &["cursor", "openai", "anthropic", "google", "xai", "x-ai"],
        status_page: Some(StatusPage {
            page_url: "https://status.cursor.com",
            api: StatusApi::Statuspage("https://status.cursor.com/api/v2/summary.json"),
            relevance: StatusRelevance::Except(&["Grok Bot"]),
        }),
        metrics: CURSOR_METRICS,
        default_tray_metrics: &["cursor.auto", "cursor.api"],
    },
    ProviderDescriptor {
        kind: ProviderKind::OpenCodeZen,
        id: "opencode",
        display_name: "OpenCode Zen",
        icon: "opencode",
        brand_rgb: (128, 128, 128),
        icon_rgb: ThemedRgb {
            light: (77, 77, 77),
            dark: (128, 128, 128),
        },
        spend_rgb: ThemedRgb {
            light: (75, 75, 75),
            dark: (205, 205, 205),
        },
        series_rgb: ThemedRgb {
            light: (72, 72, 72),
            dark: (210, 210, 210),
        },
        supports_activation: false,
        include_in_total_spend: true,
        supports_multiple_instances: true,
        config_folder: false,
        banked_resets: BankedResets::Unsupported,
        credits: Credits::AsReported,
        usage_stats: UsageStats::Local,
        usage_value: UsageValue::Spend,
        spending_strips: false,
        balance_only: false,
        monthly_only: false,
        limit_titles: LimitTitles::STANDARD,
        default_refresh_seconds: 60,
        activation_state_file: "activation-opencode-zen.toml",
        pricing_prefixes: &[],
        status_page: None,
        metrics: OPENCODE_ZEN_METRICS,
        default_tray_metrics: &[],
    },
    ProviderDescriptor {
        kind: ProviderKind::OpenCodeGo,
        id: "opencode-go",
        display_name: "OpenCode Go",
        icon: "opencode",
        brand_rgb: (128, 128, 128),
        icon_rgb: ThemedRgb {
            light: (77, 77, 77),
            dark: (128, 128, 128),
        },
        spend_rgb: ThemedRgb {
            light: (75, 75, 75),
            dark: (205, 205, 205),
        },
        series_rgb: ThemedRgb {
            light: (72, 72, 72),
            dark: (210, 210, 210),
        },
        supports_activation: false,
        include_in_total_spend: true,
        supports_multiple_instances: true,
        config_folder: false,
        banked_resets: BankedResets::Unsupported,
        credits: Credits::AsReported,
        usage_stats: UsageStats::Local,
        usage_value: UsageValue::Spend,
        spending_strips: false,
        balance_only: false,
        monthly_only: false,
        limit_titles: LimitTitles::STANDARD,
        default_refresh_seconds: 60,
        activation_state_file: "activation-opencode-go.toml",
        pricing_prefixes: &[],
        status_page: None,
        metrics: OPENCODE_GO_METRICS,
        default_tray_metrics: &[
            "opencode-go.session",
            "opencode-go.weekly",
            "opencode-go.monthly",
        ],
    },
    ProviderDescriptor {
        kind: ProviderKind::OpenRouter,
        id: "openrouter",
        display_name: "OpenRouter",
        icon: "openrouter",
        brand_rgb: (200, 255, 0),
        icon_rgb: ThemedRgb {
            light: (118, 36, 244),
            dark: (200, 255, 0),
        },
        spend_rgb: ThemedRgb::same((200, 255, 0)),
        series_rgb: ThemedRgb::same((200, 255, 0)),
        supports_activation: false,
        include_in_total_spend: true,
        supports_multiple_instances: true,
        config_folder: false,
        banked_resets: BankedResets::Unsupported,
        credits: Credits::Unsupported,
        usage_stats: UsageStats::ManagementKey,
        usage_value: UsageValue::BilledTokens,
        spending_strips: true,
        balance_only: true,
        monthly_only: false,
        limit_titles: LimitTitles::single("spending"),
        // Credits rarely move.
        default_refresh_seconds: 10 * 60,
        activation_state_file: "activation-openrouter.toml",
        pricing_prefixes: &[],
        status_page: None,
        metrics: OPENROUTER_METRICS,
        default_tray_metrics: &["openrouter.limit"],
    },
    ProviderDescriptor {
        kind: ProviderKind::Antigravity,
        id: "antigravity",
        display_name: "Antigravity",
        icon: "antigravity",
        brand_rgb: (66, 133, 244),
        icon_rgb: ThemedRgb {
            light: (32, 96, 205),
            dark: (66, 133, 244),
        },
        spend_rgb: ThemedRgb::same((66, 133, 244)),
        series_rgb: ThemedRgb::same((66, 133, 244)),
        supports_activation: false,
        include_in_total_spend: false,
        supports_multiple_instances: false,
        config_folder: false,
        banked_resets: BankedResets::Unsupported,
        credits: Credits::Unsupported,
        usage_stats: UsageStats::Unsupported,
        usage_value: UsageValue::EstimatedTokens,
        spending_strips: false,
        balance_only: false,
        monthly_only: false,
        limit_titles: LimitTitles::all("gemini", "gemini", "claude-gpt"),
        default_refresh_seconds: 60,
        activation_state_file: "activation-antigravity.toml",
        pricing_prefixes: &[],
        status_page: None,
        metrics: ANTIGRAVITY_METRICS,
        default_tray_metrics: &["antigravity.gemini", "antigravity.thirdParty"],
    },
    ProviderDescriptor {
        kind: ProviderKind::Grok,
        id: "grok",
        display_name: "Grok",
        icon: "grok",
        brand_rgb: (92, 92, 92),
        icon_rgb: ThemedRgb {
            light: (51, 51, 51),
            dark: WHITE,
        },
        spend_rgb: ThemedRgb {
            light: (51, 51, 51),
            dark: WHITE,
        },
        series_rgb: ThemedRgb::same((92, 92, 92)),
        supports_activation: false,
        include_in_total_spend: false,
        supports_multiple_instances: false,
        config_folder: false,
        banked_resets: BankedResets::Unsupported,
        credits: Credits::Unsupported,
        usage_stats: UsageStats::Unsupported,
        usage_value: UsageValue::EstimatedTokens,
        spending_strips: false,
        balance_only: false,
        monthly_only: false,
        limit_titles: LimitTitles::single("credits"),
        default_refresh_seconds: 60,
        activation_state_file: "activation-grok.toml",
        pricing_prefixes: &[],
        status_page: None,
        metrics: GROK_METRICS,
        default_tray_metrics: &["grok.credits"],
    },
    ProviderDescriptor {
        kind: ProviderKind::Kiro,
        id: "kiro",
        display_name: "Kiro",
        icon: "kiro",
        brand_rgb: (151, 125, 255),
        icon_rgb: ThemedRgb {
            light: (105, 78, 198),
            dark: (151, 125, 255),
        },
        spend_rgb: ThemedRgb::same((151, 125, 255)),
        series_rgb: ThemedRgb::same((151, 125, 255)),
        supports_activation: false,
        include_in_total_spend: false,
        supports_multiple_instances: false,
        config_folder: false,
        banked_resets: BankedResets::Unsupported,
        credits: Credits::Unsupported,
        usage_stats: UsageStats::Unsupported,
        usage_value: UsageValue::EstimatedTokens,
        spending_strips: false,
        balance_only: false,
        monthly_only: true,
        limit_titles: LimitTitles::all("monthly-credits", "credits", "credits"),
        default_refresh_seconds: 60,
        activation_state_file: "activation-kiro.toml",
        pricing_prefixes: &[],
        status_page: None,
        metrics: KIRO_METRICS,
        default_tray_metrics: &["kiro.additional.credits"],
    },
    ProviderDescriptor {
        kind: ProviderKind::CommandCode,
        id: "command-code",
        display_name: "Command Code",
        icon: "commandcode",
        brand_rgb: (92, 92, 92),
        icon_rgb: ThemedRgb {
            light: (51, 51, 51),
            dark: WHITE,
        },
        spend_rgb: ThemedRgb {
            light: (51, 51, 51),
            dark: WHITE,
        },
        series_rgb: ThemedRgb::same((92, 92, 92)),
        supports_activation: false,
        include_in_total_spend: true,
        supports_multiple_instances: false,
        config_folder: false,
        banked_resets: BankedResets::Unsupported,
        credits: Credits::AsReported,
        usage_stats: UsageStats::Local,
        usage_value: UsageValue::Spend,
        spending_strips: false,
        balance_only: false,
        monthly_only: false,
        limit_titles: LimitTitles::STANDARD,
        default_refresh_seconds: 60,
        activation_state_file: "activation-command-code.toml",
        pricing_prefixes: &[],
        status_page: None,
        metrics: COMMAND_CODE_METRICS,
        default_tray_metrics: &["command-code.session", "command-code.weekly"],
    },
];

pub const fn descriptor(provider: ProviderKind) -> &'static ProviderDescriptor {
    &PROVIDERS[provider as usize]
}

/// Install live logo overrides from settings. Call this from
/// [`crate::settings_runtime::apply`] so UI, tray, and
/// plugins all see the same marks without threading a bool through every page.
pub fn apply_logo_settings(replace_chatgpt_logo_with_codex: bool) {
    REPLACE_CHATGPT_LOGO_WITH_CODEX.store(replace_chatgpt_logo_with_codex, Ordering::Relaxed);
}

/// Live provider mark for UI. Canonical asset ids stay on
/// [`ProviderDescriptor::icon`]; this is what every surface should paint.
pub fn icon(provider: ProviderKind) -> &'static str {
    match provider {
        ProviderKind::Codex if !REPLACE_CHATGPT_LOGO_WITH_CODEX.load(Ordering::Relaxed) => {
            "chatgpt"
        }
        _ => descriptor(provider).icon,
    }
}

pub fn metric(provider: ProviderKind, id: &str) -> Option<&'static MetricDescriptor> {
    descriptor(provider)
        .metrics
        .iter()
        .find(|metric| metric.id == id)
}

pub fn provider_for_metric(id: &str) -> Option<ProviderKind> {
    PROVIDERS
        .iter()
        .find(|provider| provider.metrics.iter().any(|metric| metric.id == id))
        .map(|provider| provider.kind)
}

/// Maps any popup brick id — catalog, extras, or discovered — onto its provider.
pub fn provider_for_brick_id(brick_id: &str) -> Option<ProviderKind> {
    ProviderKind::ALL
        .into_iter()
        .find(|provider| brick_id.starts_with(&format!("{}.", descriptor(*provider).id)))
}

pub fn dynamic_metric_id(provider: ProviderKind, source_id: &str) -> String {
    format!("{}.additional.{source_id}", descriptor(provider).id)
}

pub fn resets_brick_id(provider: ProviderKind) -> String {
    format!("{}.resets", descriptor(provider).id)
}

pub fn credits_brick_id(provider: ProviderKind) -> String {
    format!("{}.credits", descriptor(provider).id)
}

pub fn usage_brick_id(provider: ProviderKind) -> String {
    format!("{}.usage", descriptor(provider).id)
}

pub fn spending_brick_id(provider: ProviderKind) -> String {
    format!("{}.spending", descriptor(provider).id)
}

/// Stable popup brick ids declared for a provider before runtime discovery.
pub fn catalog_brick_ids(provider: ProviderKind) -> Vec<String> {
    let descriptor = descriptor(provider);
    let mut ids = descriptor
        .metrics
        .iter()
        .map(|metric| metric.id.to_string())
        .collect::<Vec<_>>();
    if descriptor.banked_resets != BankedResets::Unsupported {
        ids.push(resets_brick_id(provider));
    }
    if descriptor.credits != Credits::Unsupported {
        ids.push(credits_brick_id(provider));
    }
    if descriptor.usage_stats.is_supported() {
        ids.push(usage_brick_id(provider));
    }
    if descriptor.spending_strips {
        ids.push(spending_brick_id(provider));
    }
    ids
}

/// Human label for a popup brick id shown in Settings.
pub fn brick_label(provider: ProviderKind, brick_id: &str) -> String {
    if let Some(metric) = metric(provider, brick_id) {
        return metric.localized_label().into();
    }
    let provider_id = descriptor(provider).id;
    if brick_id == resets_brick_id(provider) {
        return crate::i18n::tr("banked-resets").into();
    }
    if brick_id == credits_brick_id(provider) {
        return crate::i18n::tr("credits").into();
    }
    if brick_id == usage_brick_id(provider) {
        return crate::i18n::tr("usage-stats-7b9e1a").into();
    }
    if brick_id == spending_brick_id(provider) {
        return crate::i18n::tr("spending").into();
    }
    if let Some(source_id) = brick_id.strip_prefix(&format!("{provider_id}.additional.")) {
        return source_id.replace(['-', '_'], " ");
    }
    brick_id
        .rsplit('.')
        .next()
        .unwrap_or(brick_id)
        .replace('-', " ")
}

/// Catalog bricks plus runtime-discovered additional windows for Settings.
pub fn settings_brick_ids(
    provider: ProviderKind,
    extra_ids: impl IntoIterator<Item = impl AsRef<str>>,
) -> Vec<String> {
    let mut ids = catalog_brick_ids(provider);
    let prefix = format!("{}.additional.", descriptor(provider).id);
    for extra in extra_ids {
        let extra = extra.as_ref();
        if extra.starts_with(&prefix) && !ids.iter().any(|id| id == extra) {
            ids.push(extra.to_string());
        }
    }
    ids
}

/// Tray selectors contain quota metrics only, including API-discovered windows.
pub fn tray_metric_options(
    provider: ProviderKind,
    discovered_labels: &std::collections::BTreeMap<String, String>,
) -> Vec<(String, String)> {
    let mut options = descriptor(provider)
        .metrics
        .iter()
        .map(|metric| (metric.id.to_string(), metric.localized_label().to_string()))
        .collect::<Vec<_>>();
    let prefix = format!("{}.additional.", descriptor(provider).id);
    for (id, label) in discovered_labels {
        if id.starts_with(&prefix) && !options.iter().any(|(known, _)| known == id) {
            options.push((id.clone(), label.clone()));
        }
    }
    options
}

/// Prefer the live provider title for a discovered additional window.
pub fn settings_brick_label(
    provider: ProviderKind,
    brick_id: &str,
    discovered_labels: &std::collections::BTreeMap<String, String>,
) -> String {
    if let Some(label) = discovered_labels.get(brick_id) {
        let label = label.trim();
        if !label.is_empty() {
            return label.to_string();
        }
    }
    brick_label(provider, brick_id)
}

/// Labels for additional windows that are not already in the static catalog.
pub fn discovered_additional_brick_labels(
    provider: ProviderKind,
    limits: &RateLimits,
) -> Vec<(String, String)> {
    let catalog = catalog_brick_ids(provider);
    limits
        .additional_limits
        .iter()
        .filter_map(|limit| {
            let id = additional_limit_brick_id(provider, &limit.id);
            if catalog.iter().any(|known| known == &id) {
                return None;
            }
            let title = limit.title.trim();
            if title.is_empty() {
                return None;
            }
            Some((id, title.to_string()))
        })
        .collect()
}

/// Resolve a popup limit card to its configured brick id.
pub fn limit_section_brick_id(provider: ProviderKind, section: LimitSectionKind) -> Option<String> {
    match section {
        LimitSectionKind::FiveHour => descriptor(provider)
            .metrics
            .iter()
            .find(|metric| matches!(metric.source, MetricSource::Primary))
            .map(|metric| metric.id.to_string()),
        LimitSectionKind::Weekly | LimitSectionKind::Monthly => descriptor(provider)
            .metrics
            .iter()
            .find(|metric| matches!(metric.source, MetricSource::Secondary))
            .map(|metric| metric.id.to_string()),
    }
}

/// Resolve an additional limit card to its popup brick id.
pub fn additional_limit_brick_id(provider: ProviderKind, source_id: &str) -> String {
    descriptor(provider)
        .metrics
        .iter()
        .find(|metric| matches!(metric.source, MetricSource::Additional(id) if id == source_id))
        .map(|metric| metric.id.to_string())
        .unwrap_or_else(|| dynamic_metric_id(provider, source_id))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitSectionKind {
    FiveHour,
    Weekly,
    Monthly,
}

pub fn metric_label(provider: ProviderKind, limits: &RateLimits, id: &str) -> String {
    if luna_reserve_override(provider, limits, id).is_some() {
        return crate::i18n::tr("luna-reserve").into();
    }
    if let Some(metric) = metric(provider, id) {
        return metric.localized_label().into();
    }
    limits
        .additional_limits
        .iter()
        .find(|limit| dynamic_metric_id(provider, &limit.id) == id)
        .map(additional_label)
        .unwrap_or_else(|| id.rsplit('.').next().unwrap_or(id).replace('-', " "))
}

/// Localize application-owned lane names by their stable API identity. Model
/// names and unknown provider-supplied titles remain provider data.
pub fn additional_label(limit: &AdditionalLimit) -> String {
    if limit.id == crate::limits::GPT_RESERVE_LIMIT_ID {
        crate::i18n::tr("luna-reserve").into()
    } else {
        limit.title.clone()
    }
}

pub fn metric_window<'a>(
    provider: ProviderKind,
    limits: &'a RateLimits,
    id: &str,
) -> Option<&'a LimitWindow> {
    if let Some(reserve) = luna_reserve_override(provider, limits, id) {
        return Some(&reserve.window);
    }
    if let Some(metric) = metric(provider, id) {
        return match metric.source {
            MetricSource::Primary => Some(&limits.primary),
            MetricSource::Secondary => Some(&limits.secondary),
            MetricSource::Additional(source_id) => limits
                .additional_limits
                .iter()
                .find(|limit| limit.id == source_id)
                .map(|limit| &limit.window),
        };
    }
    limits
        .additional_limits
        .iter()
        .find(|limit| dynamic_metric_id(provider, &limit.id) == id)
        .map(|limit| &limit.window)
}

/// The Luna Reserve window that stands in for a Codex session or weekly
/// metric while the weekly quota is exhausted.
pub fn luna_reserve_override<'a>(
    provider: ProviderKind,
    limits: &'a RateLimits,
    id: &str,
) -> Option<&'a AdditionalLimit> {
    if provider != ProviderKind::Codex
        || !matches!(
            metric(provider, id).map(|metric| metric.source),
            Some(MetricSource::Primary | MetricSource::Secondary)
        )
    {
        return None;
    }
    limits.codex_luna_reserve_override()
}

/// Resolves the configured metric, temporarily falling back to the first live
/// compatible metric exposed by the same provider. The configured ID is never
/// mutated, so it automatically returns when the provider reports it again.
pub fn resolve_metric<'a>(
    provider: ProviderKind,
    limits: &'a RateLimits,
    configured_id: &str,
) -> Option<(String, String, &'a LimitWindow)> {
    if let Some(window) = metric_window(provider, limits, configured_id)
        && !window.is_empty()
    {
        return Some((
            configured_id.into(),
            metric_label(provider, limits, configured_id),
            window,
        ));
    }

    for metric in descriptor(provider).metrics {
        if let Some(window) = metric_window(provider, limits, metric.id)
            && !window.is_empty()
        {
            return Some((
                metric.id.into(),
                metric_label(provider, limits, metric.id),
                window,
            ));
        }
    }
    limits.additional_limits.iter().find_map(|limit| {
        (!limit.window.is_empty()).then(|| {
            (
                dynamic_metric_id(provider, &limit.id),
                additional_label(limit),
                &limit.window,
            )
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_icon_follows_codex_logo_setting_for_every_provider() {
        apply_logo_settings(false);
        for provider in ProviderKind::ALL {
            let expected = match provider {
                ProviderKind::Codex => "chatgpt",
                _ => descriptor(provider).icon,
            };
            assert_eq!(icon(provider), expected);
        }
        apply_logo_settings(true);
        for provider in ProviderKind::ALL {
            assert_eq!(icon(provider), descriptor(provider).icon);
        }
        apply_logo_settings(false);
        assert_eq!(icon(ProviderKind::Codex), "chatgpt");
    }

    #[test]
    fn tray_options_include_dynamic_windows_and_resolve_the_selected_quota() {
        let limits = RateLimits {
            additional_limits: vec![crate::limits::AdditionalLimit {
                id: "claude-weekly-scoped-claude-fable-5-promo".into(),
                title: "Fable only".into(),
                window: LimitWindow {
                    used_percent: Some(42),
                    duration_minutes: Some(10080),
                    ..Default::default()
                },
            }],
            ..Default::default()
        };
        let mut labels = discovered_additional_brick_labels(ProviderKind::Claude, &limits)
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>();
        labels.insert("codex.additional.other".into(), "Other provider".into());
        labels.insert("claude.usage".into(), "Usage stats".into());
        let options = tray_metric_options(ProviderKind::Claude, &labels);
        assert_eq!(options.len(), CLAUDE_METRICS.len() + 1);
        let (id, label) = options.last().unwrap();
        assert_eq!(label, "Fable only");
        let (resolved_id, _, window) = resolve_metric(ProviderKind::Claude, &limits, id).unwrap();
        assert_eq!(&resolved_id, id);
        assert_eq!(window.used_percent, Some(42));
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        let mut settings = crate::settings::Settings::default();
        let mut widget = crate::settings::TrayWidget::custom_for_provider(
            crate::instances::ProviderId::from(ProviderKind::Claude),
        );
        widget.indicators[0].metric_id = id.clone();
        settings.tray_widgets = vec![widget];
        settings.save(&path).unwrap();
        let loaded = crate::settings::Settings::load_or_create(&path).unwrap();
        assert_eq!(&loaded.tray_widgets[0].indicators[0].metric_id, id);
        assert_eq!(
            tray_metric_options(ProviderKind::Codex, &labels).len(),
            CODEX_METRICS.len() + 1
        );
        assert_eq!(
            tray_metric_options(ProviderKind::Claude, &Default::default()).len(),
            CLAUDE_METRICS.len()
        );
    }

    #[test]
    fn registry_covers_every_provider_exactly_once_and_ids_round_trip() {
        assert_eq!(PROVIDERS.len(), ProviderKind::ALL.len());
        for (index, provider) in ProviderKind::ALL.into_iter().enumerate() {
            assert_eq!(provider as usize, index);
            assert_eq!(PROVIDERS[index].kind, provider);
            assert_eq!(ProviderKind::from_id(provider.id()), Some(provider));
            let json = serde_json::to_string(&provider).unwrap();
            assert_eq!(
                serde_json::from_str::<ProviderKind>(&json).unwrap(),
                provider
            );
        }
        assert_eq!(ProviderKind::from_id("unknown"), None);
    }

    #[test]
    fn brick_ids_map_to_the_longest_provider_prefix() {
        assert_eq!(
            provider_for_brick_id("opencode.usage"),
            Some(ProviderKind::OpenCodeZen)
        );
        assert_eq!(
            provider_for_brick_id("opencode-go.session"),
            Some(ProviderKind::OpenCodeGo)
        );
        assert_eq!(
            provider_for_brick_id("codex.additional.runtime_lane"),
            Some(ProviderKind::Codex)
        );
    }

    #[test]
    fn cursor_grok_bot_maps_to_the_catalog_brick() {
        assert_eq!(
            additional_limit_brick_id(ProviderKind::Cursor, "cursor-grok-bot"),
            "cursor.grokBot"
        );
        assert!(
            catalog_brick_ids(ProviderKind::Cursor)
                .iter()
                .any(|id| id == "cursor.grokBot")
        );
        assert_eq!(
            discovered_additional_brick_labels(
                ProviderKind::Cursor,
                &RateLimits {
                    additional_limits: vec![crate::limits::AdditionalLimit {
                        id: "cursor-grok-bot".into(),
                        title: "Grok Bot".into(),
                        window: LimitWindow {
                            used_percent: Some(1),
                            ..Default::default()
                        },
                    }],
                    ..Default::default()
                }
            )
            .len(),
            0
        );
    }

    #[test]
    fn cursor_all_models_maps_to_the_catalog_brick() {
        assert_eq!(
            additional_limit_brick_id(ProviderKind::Cursor, "cursor-all-models"),
            "cursor.allModels"
        );
        assert!(
            catalog_brick_ids(ProviderKind::Cursor)
                .iter()
                .any(|id| id == "cursor.allModels")
        );
    }

    #[test]
    fn metric_ids_are_unique_and_namespaced() {
        let mut ids = std::collections::HashSet::new();
        for provider in PROVIDERS {
            for metric in provider.metrics {
                assert!(metric.id.starts_with(provider.id));
                assert!(ids.insert(metric.id));
            }
        }
    }

    #[test]
    fn opencode_registry_keeps_zen_popup_only_and_maps_go_monthly() {
        assert!(descriptor(ProviderKind::OpenCodeZen).metrics.is_empty());
        let limits = RateLimits {
            additional_limits: vec![crate::limits::AdditionalLimit {
                id: "monthly".into(),
                title: "Monthly".into(),
                window: LimitWindow {
                    used_percent: Some(4),
                    ..Default::default()
                },
            }],
            ..Default::default()
        };
        assert_eq!(
            metric_window(ProviderKind::OpenCodeGo, &limits, "opencode-go.monthly")
                .and_then(|window| window.used_percent),
            Some(4)
        );
    }

    #[test]
    fn settings_bricks_include_live_additional_windows_without_catalog_duplicates() {
        let extra_id = additional_limit_brick_id(ProviderKind::Claude, "seven_day_runtime_lane");
        let ids = settings_brick_ids(ProviderKind::Claude, [extra_id.as_str(), "claude.session"]);
        assert!(ids.contains(&"claude.session".into()));
        assert!(ids.contains(&extra_id));
        assert_eq!(ids.iter().filter(|id| *id == &extra_id).count(), 1);

        let mut labels = std::collections::BTreeMap::new();
        labels.insert(extra_id.clone(), "Runtime Lane".into());
        assert_eq!(
            settings_brick_label(ProviderKind::Claude, &extra_id, &labels),
            "Runtime Lane"
        );
        assert_eq!(
            discovered_additional_brick_labels(
                ProviderKind::OpenCodeGo,
                &RateLimits {
                    additional_limits: vec![crate::limits::AdditionalLimit {
                        id: "monthly".into(),
                        title: "Monthly".into(),
                        window: LimitWindow {
                            used_percent: Some(4),
                            ..Default::default()
                        },
                    }],
                    ..Default::default()
                }
            )
            .len(),
            0
        );
    }

    #[test]
    fn codex_session_and_weekly_resolve_to_luna_reserve_when_weekly_is_empty() {
        let limits = RateLimits {
            primary: LimitWindow {
                used_percent: Some(7),
                duration_minutes: Some(300),
                ..Default::default()
            },
            secondary: LimitWindow {
                used_percent: Some(100),
                duration_minutes: Some(10_080),
                ..Default::default()
            },
            additional_limits: vec![crate::limits::AdditionalLimit {
                id: crate::limits::GPT_RESERVE_LIMIT_ID.into(),
                title: crate::limits::LUNA_RESERVE_TITLE.into(),
                window: LimitWindow {
                    used_percent: Some(12),
                    duration_minutes: Some(10_080),
                    ..Default::default()
                },
            }],
            ..Default::default()
        };

        let (session_id, session_label, session_window) =
            resolve_metric(ProviderKind::Codex, &limits, "codex.session").unwrap();
        let (weekly_id, weekly_label, weekly_window) =
            resolve_metric(ProviderKind::Codex, &limits, "codex.weekly").unwrap();

        assert_eq!(session_id, "codex.session");
        assert_eq!(weekly_id, "codex.weekly");
        assert_eq!(session_label, crate::limits::LUNA_RESERVE_TITLE);
        assert_eq!(weekly_label, crate::limits::LUNA_RESERVE_TITLE);
        assert_eq!(session_window.used_percent, Some(12));
        assert_eq!(weekly_window.used_percent, Some(12));
        assert_eq!(
            additional_limit_brick_id(ProviderKind::Codex, "gpt-reserve"),
            "codex.lunaReserve"
        );
    }
}
