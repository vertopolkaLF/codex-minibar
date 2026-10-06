use super::persistence::{persist_bool, try_persist_update_fallible};
use super::platform::{choose_provider_folder, copy_text_to_clipboard, reveal_in_explorer};
use super::*;
use crate::claude::ProfileCredentialMethod;
use crate::limits::{OpenRouterAccountSnapshot, OpenRouterApiKeySnapshot, SpendingSummary};
use std::sync::LazyLock;

#[derive(Clone, PartialEq)]
pub(super) struct ProviderInstallStatus {
    app: Option<String>,
    crew: Option<String>,
    cli: Option<String>,
    used: Option<ProviderInstallSource>,
    app_applicable: bool,
    crew_applicable: bool,
    cli_applicable: bool,
    checking: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum ProviderInstallSource {
    App,
    Crew,
    Cli,
}

impl ProviderInstallStatus {
    /// The "Checking…" placeholder with the sources a driver can report.
    pub(super) fn checking_for(provider: ProviderKind) -> Self {
        match provider {
            ProviderKind::Kiro => Self::checking_kiro(),
            ProviderKind::Grok => Self::checking_cli(),
            ProviderKind::Codex | ProviderKind::Claude | ProviderKind::Antigravity => {
                Self::checking()
            }
            ProviderKind::Cursor
            | ProviderKind::OpenCodeZen
            | ProviderKind::OpenCodeGo
            | ProviderKind::OpenRouter => Self::checking_app(),
        }
    }

    pub(super) fn checking() -> Self {
        Self {
            app: None,
            crew: None,
            cli: None,
            used: None,
            app_applicable: true,
            crew_applicable: false,
            cli_applicable: true,
            checking: true,
        }
    }

    pub(super) fn checking_app() -> Self {
        Self {
            app: None,
            crew: None,
            cli: None,
            used: None,
            app_applicable: true,
            crew_applicable: false,
            cli_applicable: false,
            checking: true,
        }
    }

    pub(super) fn checking_cli() -> Self {
        Self {
            app: None,
            crew: None,
            cli: None,
            used: None,
            app_applicable: false,
            crew_applicable: false,
            cli_applicable: true,
            checking: true,
        }
    }

    pub(super) fn checking_kiro() -> Self {
        Self {
            app: None,
            crew: None,
            cli: None,
            used: None,
            app_applicable: true,
            crew_applicable: true,
            cli_applicable: true,
            checking: true,
        }
    }
}

/// Detects what one instance reads from: its binary/app (explicit path or
/// discovery) and, for key-based drivers, its own saved credentials.
pub(super) fn instance_install_status(instance: &ProviderInstance) -> ProviderInstallStatus {
    let path = |path: &Option<PathBuf>| {
        path.as_ref()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let provider = instance.provider_id();
    match instance.driver {
        ProviderKind::Kiro => provider_install_status_kiro(
            &path(&instance.binary_path),
            &path(&instance.kiro_crew_path),
            &path(&instance.kiro_cli_path),
        ),
        ProviderKind::OpenRouter => {
            let detected = instance
                .openrouter
                .as_ref()
                .is_some_and(|account| crate::openrouter::is_installed_for_accounts(std::slice::from_ref(account)));
            ProviderInstallStatus {
                app: detected.then(|| "OpenRouter account credentials are configured".into()),
                used: detected.then_some(ProviderInstallSource::App),
                ..provider_install_status(ProviderKind::OpenRouter, "")
            }
        }
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo if !provider.is_primary() => {
            // Secondary OpenCode instances read only their own API key.
            let detected = crate::opencode::key_is_configured(provider);
            ProviderInstallStatus {
                app: detected.then(|| "Saved API key".into()),
                used: detected.then_some(ProviderInstallSource::App),
                ..provider_install_status(instance.driver, "")
            }
        }
        ProviderKind::Claude if instance.uses_manual_credential() => {
            let detected = crate::claude::load_manual_credential(&instance.id)
                .ok()
                .flatten()
                .is_some_and(|value| !value.trim().is_empty());
            ProviderInstallStatus {
                app: detected.then(|| "Saved credential".into()),
                cli: None,
                used: detected.then_some(ProviderInstallSource::App),
                cli_applicable: false,
                ..provider_install_status(ProviderKind::Claude, "")
            }
        }
        driver => provider_install_status(driver, &path(&instance.binary_path)),
    }
}

pub(super) fn provider_install_status(
    provider: ProviderKind,
    configured_folder: &str,
) -> ProviderInstallStatus {
    if provider == ProviderKind::Kiro {
        return provider_install_status_kiro(configured_folder, "", "");
    }
    let configured_folder = (!configured_folder.trim().is_empty())
        .then(|| std::path::Path::new(configured_folder.trim()));
    let (app, cli, used) = match provider {
        ProviderKind::Codex => {
            let candidates = crate::discovery::discover(configured_folder);
            let app = candidates
                .iter()
                .find(|candidate| candidate.source == crate::discovery::CandidateSource::DesktopApp)
                .map(|candidate| candidate.path.as_path());
            let cli = candidates
                .iter()
                .find(|candidate| candidate.source != crate::discovery::CandidateSource::DesktopApp)
                .map(|candidate| candidate.path.as_path());
            let used = candidates.first().map(|candidate| match candidate.source {
                crate::discovery::CandidateSource::DesktopApp => ProviderInstallSource::App,
                _ => ProviderInstallSource::Cli,
            });
            (app.map(display_fs_path), cli.map(display_fs_path), used)
        }
        ProviderKind::Claude => {
            let app = crate::claude_desktop::bundled_cli();
            let cli = crate::claude::cli_available(configured_folder);
            let used = if app.is_some() {
                Some(ProviderInstallSource::App)
            } else {
                cli.as_ref().map(|_| ProviderInstallSource::Cli)
            };
            (
                app.as_deref().map(display_fs_path),
                cli.as_deref().map(display_fs_path),
                used,
            )
        }
        ProviderKind::Cursor => {
            let app = crate::cursor::installation_path(configured_folder);
            let used = app.as_ref().map(|_| ProviderInstallSource::App);
            (app.as_deref().map(display_fs_path), None, used)
        }
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => {
            let detected = crate::opencode::is_installed(provider);
            let detail = detected.then(|| "OpenCode auth.json or local database".into());
            (detail, None, detected.then_some(ProviderInstallSource::App))
        }
        ProviderKind::OpenRouter => (None, None, None),
        ProviderKind::Antigravity => {
            let app = crate::antigravity::desktop_app(configured_folder);
            let cli = crate::antigravity::cli_available(configured_folder);
            let used = if cli.is_some() {
                Some(ProviderInstallSource::Cli)
            } else {
                app.as_ref().map(|_| ProviderInstallSource::App)
            };
            (
                app.as_deref().map(display_fs_path),
                cli.as_deref().map(display_fs_path),
                used,
            )
        }
        ProviderKind::Grok => {
            let cli = crate::grok::cli_available(configured_folder);
            (
                None,
                cli.as_deref().map(display_fs_path),
                cli.as_ref().map(|_| ProviderInstallSource::Cli),
            )
        }
        ProviderKind::Kiro => {
            unreachable!("Kiro status is resolved before the provider match")
        }
    };
    ProviderInstallStatus {
        app,
        crew: None,
        cli,
        used,
        app_applicable: provider != ProviderKind::Grok,
        crew_applicable: false,
        cli_applicable: matches!(
            provider,
            ProviderKind::Codex
                | ProviderKind::Claude
                | ProviderKind::Antigravity
                | ProviderKind::Grok
        ),
        checking: false,
    }
}

pub(super) fn provider_install_status_kiro(
    configured_app_folder: &str,
    configured_crew_folder: &str,
    configured_cli_folder: &str,
) -> ProviderInstallStatus {
    let configured_app_folder = (!configured_app_folder.trim().is_empty())
        .then(|| std::path::Path::new(configured_app_folder.trim()));
    let configured_crew_folder = (!configured_crew_folder.trim().is_empty())
        .then(|| std::path::Path::new(configured_crew_folder.trim()));
    let configured_cli_folder = (!configured_cli_folder.trim().is_empty())
        .then(|| std::path::Path::new(configured_cli_folder.trim()));
    let app_path = crate::kiro::ide_source_path(configured_app_folder);
    let crew_path = crate::kiro::crew_installation_path(configured_crew_folder);
    let cli_path = crate::kiro::cli_path(configured_cli_folder);
    let selected = crate::kiro::selected_source(
        configured_app_folder,
        configured_crew_folder,
        configured_cli_folder,
    );
    ProviderInstallStatus {
        app: app_path.as_deref().map(display_fs_path),
        crew: crew_path.as_deref().map(display_fs_path),
        cli: cli_path.as_deref().map(display_fs_path),
        used: match selected {
            Some(crate::kiro::KiroSource::Ide) => Some(ProviderInstallSource::App),
            Some(crate::kiro::KiroSource::Crew) => Some(ProviderInstallSource::Crew),
            Some(crate::kiro::KiroSource::Cli) => Some(ProviderInstallSource::Cli),
            None => None,
        },
        app_applicable: true,
        crew_applicable: true,
        cli_applicable: true,
        checking: false,
    }
}

/// `fs::canonicalize` on Windows prefixes `\\?\`. Keep the Settings UI on the
/// ordinary drive-letter form the rest of the app already shows.
fn display_fs_path(path: &std::path::Path) -> String {
    let raw = path.display().to_string();
    if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = raw.strip_prefix(r"\\?\") {
        rest.to_owned()
    } else {
        raw
    }
}

static PROVIDER_NOTICE_GEN: AtomicU64 = AtomicU64::new(0);

const NOT_OPENROUTER_KEY: &str =
    "That doesn't look like an OpenRouter key. Keys start with sk-or-.";
const DIALOG_WIDTH: f64 = 440.0;
const DIALOG_SCRIM: Color = Color {
    a: 102,
    r: 0,
    g: 0,
    b: 0,
};

/// OpenRouter usage the worker last published. Settings only reads labels,
/// masked keys, spend and balances from it; it never holds secrets.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct OpenRouterSettingsSnapshot {
    pub(crate) accounts: Vec<OpenRouterAccountSnapshot>,
    pub(crate) sampled_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Signed-in account name of every instance that reported one, keyed by
    /// instance id. Shown in each instance's Account section.
    pub(crate) identities: std::collections::BTreeMap<String, String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ProviderReadiness {
    Checking,
    Ready,
    NeedsSetup,
}

pub(super) fn provider_readiness(status: &ProviderInstallStatus) -> ProviderReadiness {
    if status.checking {
        ProviderReadiness::Checking
    } else if status.used.is_some() {
        ProviderReadiness::Ready
    } else {
        ProviderReadiness::NeedsSetup
    }
}

#[derive(Clone, PartialEq)]
pub(super) enum ProviderDialogKind {
    /// "+ Add provider": driver, name, badge and color.
    AddInstance,
    /// Delete an instance and everything Minibar stored for it.
    DeleteInstance {
        provider: ProviderId,
    },
    /// Run the CLI's own login with the instance's config folder.
    SignIn {
        provider: ProviderId,
    },
    /// Paste a Claude credential for a manual-source instance.
    ManualCredential {
        provider: ProviderId,
    },
    /// `key_id: None` adds a new key slot; `Some` replaces that slot's secret.
    OpenRouterApiKey {
        account_id: String,
        key_id: Option<String>,
    },
    OpenRouterManagementKey {
        account_id: String,
        replace: bool,
    },
    RenameOpenRouterApiKey {
        account_id: String,
        key_id: String,
    },
    RemoveOpenRouterApiKey {
        account_id: String,
        key_id: String,
    },
    RemoveOpenRouterManagementKey {
        account_id: String,
    },
    OpenCodeKey {
        provider: ProviderId,
        replace: bool,
    },
    RemoveOpenCodeKey {
        provider: ProviderId,
    },
}

#[derive(Clone, Default)]
struct DialogInputs {
    name: String,
    key: String,
    second_key: String,
    /// Add provider: the chosen driver.
    driver: Option<ProviderKind>,
    /// Add provider: badge override and color.
    badge: String,
    badge_color: BadgeColor,
}

/// Modal state for provider credential dialogs. Typed values live behind a
/// shared cell instead of reactive state so keystrokes never re-render (and
/// never race) the native inputs; only errors and the checking flag do.
#[derive(Clone)]
pub(super) struct ProviderDialog {
    kind: ProviderDialogKind,
    initial_name: String,
    inputs: Arc<Mutex<DialogInputs>>,
    error: Option<String>,
    checking: bool,
    claude_method: ProfileCredentialMethod,
    login_control: crate::claude::profile_oauth::LoginControl,
}

impl PartialEq for ProviderDialog {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.initial_name == other.initial_name
            && Arc::ptr_eq(&self.inputs, &other.inputs)
            && self.error == other.error
            && self.checking == other.checking
            && self.claude_method == other.claude_method
    }
}

impl ProviderDialog {
    pub(super) fn new(kind: ProviderDialogKind) -> Self {
        Self::with_name(kind, String::new())
    }

    fn with_name(kind: ProviderDialogKind, name: String) -> Self {
        Self {
            kind,
            inputs: Arc::new(Mutex::new(DialogInputs {
                name: name.clone(),
                ..Default::default()
            })),
            initial_name: name,
            error: None,
            checking: false,
            claude_method: ProfileCredentialMethod::default(),
            login_control: Default::default(),
        }
    }

    fn with_error(&self, error: impl Into<String>) -> Self {
        Self {
            error: Some(error.into()),
            checking: false,
            login_control: Default::default(),
            ..self.clone()
        }
    }

    fn with_checking(&self) -> Self {
        Self {
            error: None,
            checking: true,
            ..self.clone()
        }
    }

    fn inputs(&self) -> DialogInputs {
        self.inputs
            .lock()
            .map(|inputs| inputs.clone())
            .unwrap_or_default()
    }

    pub(super) fn is_checking(&self) -> bool {
        self.checking
    }

    fn is_sign_in(&self) -> bool {
        matches!(self.kind, ProviderDialogKind::SignIn { .. })
    }

    /// The "Add provider" dialog with no driver chosen yet.
    pub(super) fn add_instance() -> Self {
        Self::new(ProviderDialogKind::AddInstance)
    }

    pub(super) fn login_control(&self) -> crate::claude::profile_oauth::LoginControl {
        self.login_control.clone()
    }
}

/// Setters a dialog needs after it closes. Everything here is `Send` so key
/// checks can finish on a worker thread.
#[derive(Clone)]
pub(super) struct ProviderDialogActions {
    pub(super) set_dialog: AsyncSetState<Option<ProviderDialog>>,
    pub(super) expanded_cards: Vec<String>,
    pub(super) set_expanded_cards: AsyncSetState<Vec<String>>,
    pub(super) set_notice: AsyncSetState<Option<String>>,
    pub(super) status_revision: u64,
    pub(super) set_status_revision: AsyncSetState<u64>,
    pub(super) settings_tx: Sender<Settings>,
    /// Opens an instance's page (after Add provider or Delete).
    pub(super) select_provider: Option<Arc<dyn Fn(ProviderId) + Send + Sync>>,
}

struct DialogOutcome {
    notice: String,
    expand_card: Option<String>,
}

fn plural(count: usize, word: &str) -> String {
    format!("{count} {word}{}", if count == 1 { "" } else { "s" })
}

fn money(microusd: u64) -> String {
    format!("${:.2}", microusd as f64 / 1_000_000.0)
}

/// The OpenRouter instance that owns `account_id`.
fn find_account_instance<'a>(
    instances: &'a [ProviderInstance],
    account_id: &str,
) -> Option<&'a ProviderInstance> {
    instances.iter().find(|instance| {
        instance
            .openrouter
            .as_ref()
            .is_some_and(|account| account.id == account_id)
    })
}

fn secondary_text(text: impl Into<String>) -> TextBlock {
    text_block(text)
        .font_size(12.0)
        .foreground(ThemeRef::SecondaryText)
        .wrap()
}

fn status_dot(brush: ThemeRef) -> Element {
    border(Element::Empty)
        .width(8.0)
        .height(8.0)
        .corner_radius(4.0)
        .background(brush)
        .vertical_alignment(VerticalAlignment::Center)
        // Segoe's line box is taller than the glyphs, so a centered 8px
        // circle sits high next to 12–13px labels. Equal +top/−bottom
        // keeps the layout size and shifts the fill onto the x-height.
        .margin(Thickness {
            left: 0.0,
            top: 1.0,
            right: 0.0,
            bottom: -1.0,
        })
        .into()
}

/// Full-width rule inside a card. Explicit colors: the Fluent card stroke is
/// nearly invisible on a dark card.
fn divider(color_scheme: ColorScheme) -> Border {
    let color = match color_scheme {
        ColorScheme::Dark => Color {
            a: 46,
            r: 255,
            g: 255,
            b: 255,
        },
        ColorScheme::Light => Color {
            a: 36,
            r: 0,
            g: 0,
            b: 0,
        },
    };
    border(Element::Empty)
        .height(1.0)
        .background(color)
        .horizontal_alignment(HorizontalAlignment::Stretch)
}

fn glyph_color(color_scheme: ColorScheme) -> Color {
    match color_scheme {
        ColorScheme::Dark => Color::rgb(200, 200, 200),
        ColorScheme::Light => Color::rgb(90, 90, 90),
    }
}

/// Leading glyph for a card row (key, app, terminal).
fn row_icon(name: &'static str, color_scheme: ColorScheme) -> Element {
    border(
        crate::icons::element(name, 18.0, glyph_color(color_scheme))
            .horizontal_alignment(HorizontalAlignment::Center)
            .vertical_alignment(VerticalAlignment::Center),
    )
    .width(20.0)
    .vertical_alignment(VerticalAlignment::Center)
    .into()
}

fn provider_card(content: impl Into<Element>) -> Element {
    border(content)
        .padding(settings_card_padding())
        .background(ThemeRef::CardBackground)
        .corner_radius(8.0)
        .border_thickness(Thickness::uniform(1.0))
        .border_brush(ThemeRef::CardStroke)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into()
}

/// Optional glyph, then title and detail lines, then trailing status or actions.
fn provider_row(
    icon: Option<Element>,
    title: impl Into<String>,
    detail: Vec<Element>,
    trailing: Vec<Element>,
) -> Element {
    let mut text: Vec<Element> = vec![text_block(title).font_size(14.0).wrap().into()];
    text.extend(detail);
    let mut children: Vec<Element> = Vec::new();
    if let Some(icon) = icon {
        children.push(icon.grid_column(0));
    }
    children.push(
        vstack(text)
            .spacing(2.0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(1)
            .into(),
    );
    children.push(
        hstack(trailing)
            .spacing(8.0)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(2)
            .into(),
    );
    grid(children)
        .columns([GridLength::Auto, GridLength::Star(1.0), GridLength::Auto])
        .column_spacing(16.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into()
}

fn section_header(title: &str, caption: Option<&str>, action: Option<Element>) -> Element {
    let mut text: Vec<Element> = vec![text_block(title).font_size(14.0).semibold().into()];
    if let Some(caption) = caption {
        text.push(secondary_text(caption).into());
    }
    let mut children: Vec<Element> = vec![
        vstack(text)
            .spacing(2.0)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(0)
            .into(),
    ];
    if let Some(action) = action {
        children.push(
            action
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(1),
        );
    }
    grid(children)
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .column_spacing(16.0)
        .margin(Thickness {
            left: 0.0,
            top: 16.0,
            right: 0.0,
            bottom: 4.0,
        })
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into()
}

fn masked_key_text(hint: impl Into<String>) -> Element {
    text_block(hint)
        .font_size(12.0)
        .font_family("Cascadia Mono, Consolas")
        .foreground(ThemeRef::SecondaryText)
        .vertical_alignment(VerticalAlignment::Center)
        .into()
}

/// Compact "⋯" button. Explicit 32px box and zero padding: the default button
/// inset clips the glyph inside narrow table columns.
fn more_menu(
    items: &[&'static str],
    _color_scheme: ColorScheme,
    on_choice: impl Fn(String) + 'static,
) -> Element {
    Button::new("")
        // The built-in SymbolIcon is sized by the Button template. PathIcon
        // keeps the source's 256px geometry here and is clipped in compact
        // buttons, which made the menu appear empty.
        .icon(Symbol::More)
        .subtle()
        .menu_flyout(
            items
                .iter()
                .map(|item| {
                    let icon = match *item {
                        "Rename key" => Some("pencil-simple"),
                        "Remove key" => Some("trash"),
                        "Add key" => Some("plus"),
                        _ => None,
                    };
                    let definition = menu_item(*item);
                    match icon {
                        Some(name) => {
                            let geometry = crate::icons::geom(name);
                            definition.path_icon(geometry.path)
                        }
                        None => definition,
                    }
                })
                .collect(),
        )
        .on_item_clicked(on_choice)
        .tooltip("More options")
        .width(32.0)
        .height(32.0)
        .min_width(0.0)
        .padding(Thickness::uniform(0.0))
        .vertical_alignment(VerticalAlignment::Center)
        .into()
}

fn open_dialog(set_dialog: &AsyncSetState<Option<ProviderDialog>>, kind: ProviderDialogKind) {
    set_dialog.call(Some(ProviderDialog::new(kind)));
}

fn toggle_expanded_card(card_id: String, ctx: &SettingsPageContext<'_>) -> impl Fn(bool) + 'static {
    let current = ctx.expanded_provider_cards.to_vec();
    let setter = ctx.set_expanded_provider_cards.clone();
    move |expanded: bool| {
        let mut next = current.clone();
        next.retain(|id| id != &card_id);
        if expanded {
            next.push(card_id.clone());
        }
        setter.call(next);
    }
}

fn show_provider_notice(set_notice: AsyncSetState<Option<String>>, message: String) {
    let generation = PROVIDER_NOTICE_GEN.fetch_add(1, Ordering::Relaxed) + 1;
    set_notice.call(Some(message));
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(3200));
        if PROVIDER_NOTICE_GEN.load(Ordering::Relaxed) == generation {
            set_notice.call(None);
        }
    });
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

/// Applies a change to one instance against on-disk settings, never a stale
/// UI snapshot. The open window picks the result up through the live sync.
fn persist_instance(
    settings_tx: Sender<Settings>,
    provider: ProviderId,
    mutate: impl FnOnce(&mut ProviderInstance) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    try_persist_update_fallible(settings_tx, move |settings| {
        let instance = settings
            .instance_mut(provider)
            .ok_or_else(|| anyhow::anyhow!("This provider no longer exists."))?;
        mutate(instance)?;
        instance.normalize();
        Ok(())
    })
}

/// Fire-and-forget variant for toggles and text fields; failures are shown
/// as a notification instead of a dialog error.
fn update_instance(
    settings_tx: Sender<Settings>,
    provider: ProviderId,
    mutate: impl FnOnce(&mut ProviderInstance),
) {
    if let Err(error) = persist_instance(settings_tx, provider, |instance| {
        mutate(instance);
        Ok(())
    }) {
        crate::notifications::show("Could not save provider", &format!("{error:#}"));
    }
}

/// Apply a change to one OpenRouter account. Accounts are addressed by stable
/// id so list shifts cannot move keys between instances. Renaming key labels
/// must not bump the credentials revision; popup headings overlay names.
fn persist_openrouter_account(
    settings_tx: Sender<Settings>,
    account_id: String,
    bump_credentials: bool,
    previous_availability: Option<bool>,
    mutate: impl FnOnce(&mut OpenRouterAccount) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    try_persist_update_fallible(settings_tx, move |settings| {
        let provider = find_account_instance(&settings.instances, &account_id)
            .map(ProviderInstance::provider_id)
            .ok_or_else(|| anyhow::anyhow!("OpenRouter account no longer exists"))?;
        let instance = settings.instance_mut(provider).expect("instance just found");
        let account = instance
            .openrouter
            .as_mut()
            .expect("OpenRouter instance has an account");
        mutate(account)?;
        let after = crate::openrouter::has_management_key(std::slice::from_ref(account));
        if bump_credentials {
            instance.credentials_revision = instance.credentials_revision.wrapping_add(1);
        }
        if let Some(before) = previous_availability {
            settings.sync_openrouter_usage_availability(provider, before, after);
        }
        Ok(())
    })
}

/// Commits protected OpenRouter secrets as one file update, then commits the
/// matching account metadata. If the settings write fails, protected storage
/// is restored to its exact previous values so neither side can be orphaned.
fn persist_openrouter_credentials(
    settings_tx: Sender<Settings>,
    account_id: String,
    changes: Vec<crate::openrouter::AccountSecretChange>,
    mutate: impl FnOnce(&mut OpenRouterAccount) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let current = Settings::load_or_create(&Settings::default_path()?)?;
    let before = find_account_instance(&current.instances, &account_id)
        .and_then(|instance| instance.openrouter.as_ref())
        .is_some_and(|account| {
            crate::openrouter::has_management_key(std::slice::from_ref(account))
        });
    let rollback = crate::openrouter::apply_account_secret_changes(&changes)?;
    if let Err(error) =
        persist_openrouter_account(settings_tx, account_id, true, Some(before), mutate)
    {
        return match rollback.restore() {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(anyhow::anyhow!(
                "could not save account metadata ({error:#}); restoring protected credentials also failed ({rollback_error:#})"
            )),
        };
    }
    Ok(())
}

fn bump_credentials(settings_tx: Sender<Settings>, provider: ProviderId) -> anyhow::Result<()> {
    persist_instance(settings_tx, provider, |instance| {
        instance.credentials_revision = instance.credentials_revision.wrapping_add(1);
        Ok(())
    })
}

fn persist_opencode_manual_key(
    settings_tx: Sender<Settings>,
    provider: ProviderId,
    value: Option<String>,
) -> anyhow::Result<()> {
    let rollback = crate::opencode::apply_manual_key(provider, value.as_deref())?;
    if let Err(error) = bump_credentials(settings_tx, provider) {
        return match crate::opencode::restore_manual_key(rollback) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(anyhow::anyhow!(
                "could not save credential revision ({error:#}); restoring the protected key also failed ({rollback_error:#})"
            )),
        };
    }
    Ok(())
}

/// Keep the existing credential when validation or the settings commit fails.
fn persist_claude_manual_credential(
    settings_tx: Sender<Settings>,
    provider: ProviderId,
    credential: &str,
) -> anyhow::Result<()> {
    let previous = crate::claude::load_manual_credential(provider.id())?;
    crate::claude::save_manual_credential(provider.id(), Some(credential))?;
    if let Err(error) = bump_credentials(settings_tx, provider) {
        return match crate::claude::save_manual_credential(provider.id(), previous.as_deref()) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(anyhow::anyhow!(
                "Could not save provider settings ({error:#}); restoring its previous credential also failed ({rollback_error:#})."
            )),
        };
    }
    Ok(())
}

/// Forgets every secret Minibar stored for a deleted instance. Config folders
/// belong to the CLI and are left on disk.
fn forget_instance_secrets(instance: &ProviderInstance) {
    let provider = instance.provider_id();
    match instance.driver {
        ProviderKind::Claude => {
            if let Err(error) = crate::claude::save_manual_credential(&instance.id, None) {
                eprintln!("failed to delete the Claude credential: {error:#}");
            }
        }
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => {
            if let Err(error) = crate::opencode::save_manual_key(provider, None) {
                eprintln!("failed to delete the OpenCode key: {error:#}");
            }
        }
        ProviderKind::OpenRouter => {
            if let Some(account) = &instance.openrouter {
                let mut changes = account
                    .api_key_ids
                    .iter()
                    .map(|key_id| {
                        crate::openrouter::AccountSecretChange::api_key(
                            account.id.clone(),
                            key_id.clone(),
                            None,
                        )
                    })
                    .collect::<Vec<_>>();
                changes.push(crate::openrouter::AccountSecretChange::management(
                    account.id.clone(),
                    None,
                ));
                if let Err(error) = crate::openrouter::apply_account_secret_changes(&changes) {
                    eprintln!("failed to delete the OpenRouter keys: {error:#}");
                }
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

/// Which path field of an instance a folder picker edits.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum PathField {
    /// CLI/app location (Kiro: IDE).
    Binary,
    KiroCrew,
    KiroCli,
    /// `CLAUDE_CONFIG_DIR` / `CODEX_HOME`.
    ConfigFolder,
}

impl PathField {
    fn key(self) -> &'static str {
        match self {
            Self::Binary => "binary",
            Self::KiroCrew => "kiro-crew",
            Self::KiroCli => "kiro-cli",
            Self::ConfigFolder => "config-folder",
        }
    }

    fn read(self, instance: &ProviderInstance) -> String {
        let path = match self {
            Self::Binary => instance.binary_path.as_ref(),
            Self::KiroCrew => instance.kiro_crew_path.as_ref(),
            Self::KiroCli => instance.kiro_cli_path.as_ref(),
            Self::ConfigFolder => match &instance.source {
                InstanceSource::ConfigFolder { path } => path.as_ref(),
                InstanceSource::Manual => None,
            },
        };
        path.map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    fn write(self, instance: &mut ProviderInstance, folder: Option<PathBuf>) {
        match self {
            Self::Binary => instance.binary_path = folder,
            Self::KiroCrew => instance.kiro_crew_path = folder,
            Self::KiroCli => instance.kiro_cli_path = folder,
            Self::ConfigFolder => {
                instance.source = InstanceSource::ConfigFolder { path: folder };
            }
        }
    }
}

static PATH_SAVE_GEN: LazyLock<Mutex<HashMap<String, u64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Debounced path save, so typing a folder does not restart the worker on
/// every keystroke.
fn persist_path(
    provider: ProviderId,
    field: PathField,
    value: String,
    settings_tx: Sender<Settings>,
) {
    let key = format!("{}:{}", provider.id(), field.key());
    let revision = {
        let mut generations = PATH_SAVE_GEN.lock().unwrap_or_else(|e| e.into_inner());
        let entry = generations.entry(key.clone()).or_default();
        *entry += 1;
        *entry
    };
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        let current = PATH_SAVE_GEN
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .copied();
        if current != Some(revision) {
            return;
        }
        let folder = (!value.trim().is_empty()).then(|| PathBuf::from(value.trim()));
        update_instance(settings_tx, provider, move |instance| {
            field.write(instance, folder);
        });
    });
}

fn pick_folder(provider: ProviderId, field: PathField, settings_tx: Sender<Settings>) {
    match choose_provider_folder() {
        Ok(Some(folder)) => {
            persist_path(provider, field, folder.display().to_string(), settings_tx);
        }
        Ok(None) => {}
        Err(error) => eprintln!("failed to choose a folder: {error:#}"),
    }
}

fn folder_picker(
    provider: ProviderId,
    field: PathField,
    path: String,
    placeholder: &str,
    settings_tx: Sender<Settings>,
) -> Element {
    let picker_tx = settings_tx.clone();
    grid((
        text_box(path)
            .placeholder_text(placeholder)
            .on_commit(move |value: String| {
                persist_path(provider, field, value, settings_tx.clone());
            })
            .height(32.0)
            .grid_column(0),
        Button::new("")
            .icon(Symbol::Folder)
            .width(44.0)
            .height(32.0)
            .tooltip("Choose folder")
            .on_click(move || pick_folder(provider, field, picker_tx.clone()))
            .grid_column(1),
    ))
    .columns([GridLength::Star(1.0), GridLength::Auto])
    .column_spacing(8.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    // The text box keeps its typed text; remount when the stored path
    // changes elsewhere so it never shows a stale value.
    .with_key(format!("{}-{}", provider.id(), field.key()))
    .into()
}

struct FolderConfig {
    field: PathField,
    label: &'static str,
    description: &'static str,
    placeholder: &'static str,
}

fn folder_configs(driver: ProviderKind) -> Vec<FolderConfig> {
    let binary = |label, description, placeholder| FolderConfig {
        field: PathField::Binary,
        label,
        description,
        placeholder,
    };
    match driver {
        ProviderKind::Codex => vec![binary(
            "Codex CLI folder",
            "Folder with codex.exe, codex.cmd, or codex.ps1. Leave empty to find it automatically.",
            r"C:\Users\you\AppData\Roaming\npm",
        )],
        ProviderKind::Claude => vec![binary(
            "Claude Code CLI folder",
            "Folder with claude.exe, claude.cmd, or claude.ps1. Leave empty to find it automatically.",
            r"C:\Users\you\AppData\Roaming\npm",
        )],
        ProviderKind::Cursor => vec![binary(
            "Cursor app folder",
            "Folder with Cursor.exe. Leave empty to find it automatically. Usage still comes from the signed-in profile.",
            r"C:\Users\you\AppData\Local\Programs\Cursor",
        )],
        ProviderKind::Antigravity => vec![binary(
            "agy CLI folder",
            "Folder with agy.exe, agy.cmd, or agy.ps1. Leave empty to find it automatically.",
            r"C:\Users\you\AppData\Local\agy\bin",
        )],
        ProviderKind::Grok => vec![binary(
            "Grok CLI folder",
            "Folder with grok.exe, grok.cmd, or grok.ps1. Leave empty to find it automatically.",
            r"C:\Users\you\.grok\bin",
        )],
        ProviderKind::Kiro => vec![
            binary(
                "Kiro IDE folder",
                "Folder containing Kiro.exe, or the executable itself. Leave empty to find it automatically.",
                r"C:\Users\you\AppData\Local\Programs\Kiro",
            ),
            FolderConfig {
                field: PathField::KiroCrew,
                label: "Kiro Crew app path",
                description: "Folder containing KiroCrew.exe, or the executable itself. Leave empty to detect per-user and all-users installs automatically.",
                placeholder: r"C:\Users\you\AppData\Local\Programs\KiroCrew",
            },
            FolderConfig {
                field: PathField::KiroCli,
                label: "Kiro CLI folder",
                description: "Folder containing kiro-cli.exe, or the executable itself. Leave empty to find it automatically.",
                placeholder: r"C:\Users\you\AppData\Local\kiro-cli",
            },
        ],
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo | ProviderKind::OpenRouter => {
            Vec::new()
        }
    }
}

// ---------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------

/// One-line status shared by the page header and the sidebar dot.
fn provider_status_line(
    instance: &ProviderInstance,
    status: &ProviderInstallStatus,
) -> (String, Option<ThemeRef>) {
    let provider = instance.driver;
    if !instance.enabled {
        return ("Off".into(), None);
    }
    let readiness = provider_readiness(status);
    let dot = match readiness {
        ProviderReadiness::Checking => None,
        ProviderReadiness::Ready => Some(ThemeRef::SystemSuccess),
        ProviderReadiness::NeedsSetup => Some(ThemeRef::SystemCaution),
    };
    if provider == ProviderKind::OpenRouter {
        let keys = instance
            .openrouter
            .as_ref()
            .map_or(0, |account| account.api_key_ids.len());
        let management = instance.openrouter.as_ref().is_some_and(|account| {
            crate::openrouter::has_management_key(std::slice::from_ref(account))
        });
        if keys == 0 && !management {
            return ("No keys yet".into(), Some(ThemeRef::SystemCaution));
        }
        let mut text = plural(keys, "API key");
        if management {
            text.push_str(" · management key");
        }
        return (text, dot);
    }
    if instance.uses_manual_credential() {
        return match readiness {
            ProviderReadiness::Ready => ("Using a saved credential".into(), dot),
            ProviderReadiness::Checking => ("Checking…".into(), dot),
            ProviderReadiness::NeedsSetup => ("Paste a credential under Account".into(), dot),
        };
    }
    let text = match readiness {
        ProviderReadiness::Checking => "Checking…".to_owned(),
        ProviderReadiness::NeedsSetup => match provider {
            ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => {
                if instance.is_primary() {
                    "Needs an API key or OpenCode sign-in".to_owned()
                } else {
                    "Needs an API key".to_owned()
                }
            }
            _ => "Not found. Set its folder under Runtime.".to_owned(),
        },
        ProviderReadiness::Ready => match provider {
            ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => {
                if crate::opencode::key_is_configured(instance.provider_id()) {
                    "Using a saved API key".to_owned()
                } else {
                    "Using OpenCode sign-in or local history".to_owned()
                }
            }
            _ => {
                let (app, crew, cli) = source_labels(provider);
                match status.used {
                    Some(ProviderInstallSource::Cli) => format!("Reading {cli}"),
                    Some(ProviderInstallSource::Crew) => format!("Reading {crew}"),
                    _ => format!("Reading {app}"),
                }
            }
        },
    };
    (text, dot)
}

/// A small rounded chip next to the status line (e.g. `Limits only`).
fn status_chip(label: &str) -> Element {
    border(
        text_block(label)
            .font_size(11.0)
            .semibold()
            .foreground(ThemeRef::SecondaryText),
    )
    .padding(Thickness {
        left: 6.0,
        top: 1.0,
        right: 6.0,
        bottom: 2.0,
    })
    .corner_radius(4.0)
    .background(ThemeRef::ControlFillSecondary)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .vertical_alignment(VerticalAlignment::Center)
    .into()
}

/// A badge plate as drawn in the sidebar and popup.
fn badge_plate(badge: &crate::instances::Badge, color_scheme: ColorScheme) -> Element {
    let (background, foreground) = match badge.color.rgb() {
        Some((r, g, b)) => (Color::rgb(r, g, b), Color::rgb(255, 255, 255)),
        None => match color_scheme {
            ColorScheme::Dark => (Color::rgb(200, 200, 200), Color::rgb(28, 28, 28)),
            ColorScheme::Light => (Color::rgb(90, 90, 90), Color::rgb(255, 255, 255)),
        },
    };
    border(
        text_block(badge.text.clone())
            .font_size(11.0)
            .bold()
            .foreground(foreground)
            .horizontal_alignment(HorizontalAlignment::Center)
            .vertical_alignment(VerticalAlignment::Center),
    )
    .padding(Thickness {
        left: 4.0,
        top: 0.0,
        right: 4.0,
        bottom: 1.0,
    })
    .min_width(20.0)
    .height(18.0)
    .corner_radius(4.0)
    .background(background)
    .vertical_alignment(VerticalAlignment::Center)
    .into()
}

fn provider_header(
    instance: &ProviderInstance,
    status_text: String,
    dot: Option<ThemeRef>,
    show_badge: bool,
    ctx: &SettingsPageContext<'_>,
) -> Element {
    let provider = instance.provider_id();
    let driver = instance.driver;
    let enabled = instance.enabled;
    let color_scheme = ctx.color_scheme;
    let icon_name = crate::provider_registry::icon(driver);
    let scheme_tag = match color_scheme {
        ColorScheme::Dark => "dark",
        ColorScheme::Light => "light",
    };
    let icon_color = if enabled {
        crate::icons::provider_brand_color(driver, color_scheme)
    } else {
        match color_scheme {
            ColorScheme::Dark => Color::rgb(230, 230, 230),
            ColorScheme::Light => Color::rgb(58, 58, 58),
        }
    };
    let mut status: Vec<Element> = Vec::new();
    if let Some(dot) = dot {
        status.push(status_dot(dot));
    }
    status.push(
        text_block(status_text)
            .font_size(13.0)
            .foreground(ThemeRef::SecondaryText)
            .vertical_alignment(VerticalAlignment::Center)
            .into(),
    );
    if crate::instances::Capabilities::of(instance).limits_only() {
        status.push(status_chip("Limits only"));
    }
    let mut title: Vec<Element> = Vec::new();
    if show_badge {
        title.push(badge_plate(&instance.badge(), color_scheme));
    }
    title.push(
        text_block(instance.display_name())
            .font_size(28.0)
            .bold()
            .vertical_alignment(VerticalAlignment::Center)
            .into(),
    );
    let settings_tx = ctx.settings_tx.clone();
    let widgets = ctx.tray_widgets.to_vec();
    let widgets_setter = ctx.set_tray_widgets.clone();
    let delete_dialog = ctx.set_provider_dialog.clone();
    grid(vec![
        border(
            crate::icons::element(icon_name, 24.0, icon_color)
                .horizontal_alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Center),
        )
        .width(48.0)
        .height(48.0)
        .corner_radius(8.0)
        .background(ThemeRef::CardBackground)
        .border_thickness(Thickness::uniform(1.0))
        .border_brush(ThemeRef::CardStroke)
        .vertical_alignment(VerticalAlignment::Center)
        .grid_column(0)
        .with_key(format!(
            "provider-icon-{}-{scheme_tag}-{icon_name}-{:02X}{:02X}{:02X}",
            provider.id(),
            icon_color.r,
            icon_color.g,
            icon_color.b
        ))
        .into(),
        vstack((
            hstack(title)
                .spacing(10.0)
                // Title line-box is ~36px for a 28px Segoe face. Trim the extra
                // leading so the name+status group optically matches the 48px icon.
                .margin(Thickness {
                    left: 0.0,
                    top: -6.0,
                    right: 0.0,
                    bottom: -2.0,
                }),
            hstack(status)
                .spacing(8.0)
                .vertical_alignment(VerticalAlignment::Center),
        ))
        .spacing(0.0)
        .vertical_alignment(VerticalAlignment::Center)
        .grid_column(1)
        .into(),
        hstack((
            text_block(if enabled { "On" } else { "Off" })
                .vertical_alignment(VerticalAlignment::Center),
            ToggleSwitch::new(enabled)
                .on_content("")
                .off_content("")
                .on_toggled(move |value: bool| {
                    widgets_setter.call(widgets.clone());
                    update_instance(settings_tx.clone(), provider, move |instance| {
                        instance.enabled = value;
                    });
                })
                .min_width(0.0)
                .max_width(50.0)
                .width(50.0)
                .vertical_alignment(VerticalAlignment::Center),
            Button::new("")
                .icon(Symbol::Delete)
                .subtle()
                .tooltip("Delete provider")
                .on_click(move || {
                    open_dialog(
                        &delete_dialog,
                        ProviderDialogKind::DeleteInstance { provider },
                    )
                })
                .width(36.0)
                .height(32.0)
                .min_width(0.0)
                .padding(Thickness::uniform(0.0))
                .vertical_alignment(VerticalAlignment::Center),
        ))
        .spacing(12.0)
        .vertical_alignment(VerticalAlignment::Center)
        .grid_column(2)
        .into(),
    ])
    .columns([GridLength::Auto, GridLength::Star(1.0), GridLength::Auto])
    .column_spacing(16.0)
    .margin(Thickness {
        left: 0.0,
        top: 0.0,
        right: 0.0,
        bottom: 8.0,
    })
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

/// Shown in the Providers pane when no instance is configured.
pub(super) fn no_providers_page(ctx: &SettingsPageContext<'_>) -> Element {
    let set_dialog = ctx.set_provider_dialog.clone();
    let mut rows: Vec<Element> = vec![
        text_block("Providers")
            .font_size(28.0)
            .bold()
            .with_key("no-providers-title")
            .into(),
    ];
    if let Some(notice) = ctx.provider_notice {
        rows.push(
            InfoBar::new(notice.clone())
                .success()
                .is_closable(false)
                .with_key("provider-notice")
                .into(),
        );
    }
    rows.push(
        provider_card(
            vstack((
                secondary_text("No providers yet. Add one to start reading limits.")
                    .horizontal_alignment(HorizontalAlignment::Center),
                Button::new("Add provider")
                    .icon(Symbol::Add)
                    .accent()
                    .on_click(move || set_dialog.call(Some(ProviderDialog::add_instance())))
                    .horizontal_alignment(HorizontalAlignment::Center),
            ))
            .spacing(10.0)
            .horizontal_alignment(HorizontalAlignment::Stretch),
        )
        .with_key("no-providers-card"),
    );
    vstack(rows)
        .spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Top)
        .into()
}

pub(super) fn provider_page_content(provider: ProviderId, ctx: &SettingsPageContext<'_>) -> Element {
    let Some(instance) = ctx.instance(provider).cloned() else {
        return no_providers_page(ctx);
    };
    let status = ctx.install_status(provider);
    let (status_text, dot) = provider_status_line(&instance, &status);
    let show_badge = ctx
        .instances
        .iter()
        .filter(|other| other.driver == instance.driver)
        .count()
        > 1;

    let mut rows: Vec<Element> = vec![
        provider_header(&instance, status_text, dot, show_badge, ctx).with_key("provider-header"),
    ];
    if let Some(notice) = ctx.provider_notice {
        rows.push(
            InfoBar::new(notice.clone())
                .success()
                .is_closable(false)
                .with_key("provider-notice")
                .into(),
        );
    }
    if !instance.enabled {
        rows.push(
            provider_card(secondary_text(format!(
                "{} is off, so it doesn't appear in the minibar or tray. Turn it on to start reading usage.",
                instance.display_name()
            )))
            .with_key("provider-off-note"),
        );
    }
    let mut sections = general_sections(&instance, ctx);
    sections.extend(account_sections(&instance, ctx));
    sections.extend(runtime_sections(&instance, &status, ctx));
    sections.extend(feature_sections(&instance, ctx));
    sections.push(section_header("Appearance", None, None).with_key("appearance-header"));
    if instance.driver == ProviderKind::Codex {
        sections.push(codex_logo_toggle(ctx).with_key("codex-replace-logo"));
    }
    sections.extend(super::customize::provider_settings_cards(&instance, ctx));
    rows.push(
        vstack(sections)
            .spacing(4.0)
            .opacity(if instance.enabled { 1.0 } else { 0.5 })
            .with_opacity_transition(duration(CONTROL_FAST_ANIMATION))
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .with_key(format!("provider-{}-sections", provider.id()))
            .into(),
    );

    vstack(rows)
        .spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Top)
        .with_layout_animation(
            LayoutAnimationConfig::linear(duration(CONTROL_NORMAL_ANIMATION)).animate_size(true),
        )
        .into()
}

/// A labeled control row inside a provider card.
fn control_row(title: &str, description: &str, control: Element) -> Element {
    provider_card(provider_row(
        None,
        title,
        vec![secondary_text(description).into()],
        vec![control],
    ))
}

/// A toggle row that stays visible but disabled, with the reason, when the
/// instance lacks the capability. Losing a capability never resets the
/// stored value.
fn capability_toggle(
    title: &str,
    description: &str,
    value: bool,
    reason: Option<&'static str>,
    on_toggled: impl Fn(bool) + 'static,
) -> Element {
    let available = reason.is_none();
    let mut detail: Vec<Element> = vec![secondary_text(description).into()];
    if let Some(reason) = reason {
        detail.push(
            text_block(reason)
                .font_size(12.0)
                .foreground(ThemeRef::SystemCaution)
                .wrap()
                .into(),
        );
    }
    provider_card(provider_row(
        None,
        title,
        detail,
        vec![
            ToggleSwitch::new(value && available)
                .on_content("")
                .off_content("")
                .enabled(available)
                .on_toggled(on_toggled)
                .min_width(0.0)
                .width(50.0)
                .vertical_alignment(VerticalAlignment::Center)
                .into(),
        ],
    ))
}

fn general_sections(instance: &ProviderInstance, ctx: &SettingsPageContext<'_>) -> Vec<Element> {
    let provider = instance.provider_id();
    let mut out = vec![section_header("General", None, None).with_key("general-header")];
    let name_tx = ctx.settings_tx.clone();
    out.push(
        control_row(
            "Display name",
            "Shown on popup tabs, Home cards, the tray and notifications.",
            text_box(instance.name.clone())
                .placeholder_text(instance.driver.display_name())
                .on_commit(move |value: String| {
                    update_instance(name_tx.clone(), provider, move |instance| {
                        instance.name = value;
                    });
                })
                .width(220.0)
                .height(32.0)
                .vertical_alignment(VerticalAlignment::Center)
                .into(),
        )
        .with_key(format!("instance-name-{}", instance.name)),
    );
    let badge_tx = ctx.settings_tx.clone();
    out.push(
        control_row(
            "Badge",
            "Up to three letters. Leave empty to use the name's initials. Shown while a provider has more than one instance turned on.",
            hstack((
                badge_plate(&instance.badge(), ctx.color_scheme),
                text_box(instance.badge.clone())
                    .placeholder_text(instance.badge().text)
                    .on_commit(move |value: String| {
                        update_instance(badge_tx.clone(), provider, move |instance| {
                            instance.badge = crate::instances::sanitize_badge(&value);
                        });
                    })
                    .width(96.0)
                    .height(32.0),
            ))
            .spacing(8.0)
            .vertical_alignment(VerticalAlignment::Center)
            .into(),
        )
        .with_key(format!("instance-badge-{}", instance.badge)),
    );
    let color_tx = ctx.settings_tx.clone();
    out.push(
        control_row(
            "Badge color",
            "Auto uses a neutral plate that follows the theme.",
            ComboBox::new(BadgeColor::ALL.map(BadgeColor::label))
                .selected_index(instance.badge_color.index())
                .on_selection_changed(move |index: i32| {
                    let color = BadgeColor::from_index(index);
                    update_instance(color_tx.clone(), provider, move |instance| {
                        instance.badge_color = color;
                    });
                })
                .width(160.0)
                .vertical_alignment(VerticalAlignment::Center)
                .into(),
        )
        .with_key("instance-badge-color"),
    );
    let home_tx = ctx.settings_tx.clone();
    out.push(
        control_row(
            "Show on Home",
            "Its provider tab stays available when hidden from Home.",
            ToggleSwitch::new(instance.show_on_home)
                .on_content("")
                .off_content("")
                .on_toggled(move |value: bool| {
                    update_instance(home_tx.clone(), provider, move |instance| {
                        instance.show_on_home = value;
                    });
                })
                .min_width(0.0)
                .width(50.0)
                .vertical_alignment(VerticalAlignment::Center)
                .into(),
        )
        .with_key("instance-show-on-home"),
    );
    out
}

fn account_sections(instance: &ProviderInstance, ctx: &SettingsPageContext<'_>) -> Vec<Element> {
    let provider = instance.provider_id();
    match instance.driver {
        ProviderKind::Claude | ProviderKind::Codex => {}
        ProviderKind::OpenRouter => return openrouter_sections(instance, ctx),
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => {
            return opencode_key_section(instance, ctx);
        }
        _ => return Vec::new(),
    }
    let mut out = vec![section_header("Account", None, None).with_key("account-header")];
    let identity = ctx.openrouter_snapshot.identities.get(&instance.id).cloned();
    if instance.uses_manual_credential() {
        let saved = crate::claude::load_manual_credential(&instance.id)
            .ok()
            .flatten()
            .filter(|value| !value.trim().is_empty());
        let set_dialog = ctx.set_provider_dialog.clone();
        let mut detail: Vec<Element> = vec![
            secondary_text(match (&saved, &identity) {
                (Some(_), Some(identity)) => format!("Signed in as {identity}"),
                (Some(_), None) => "Saved in Windows user storage".into(),
                (None, _) => "Paste a sessionKey or an OAuth access token.".into(),
            })
            .into(),
        ];
        if saved.is_some() {
            detail.push(
                secondary_text("Reads limits only. Minibar cannot refresh a pasted credential.")
                    .into(),
            );
        }
        let mut trailing: Vec<Element> = Vec::new();
        if let Some(saved) = &saved {
            trailing.push(masked_key_text(crate::secrets::masked_hint(saved)));
        }
        trailing.push(
            Button::new(if saved.is_some() {
                "Replace credential"
            } else {
                "Add credential"
            })
            .on_click(move || {
                open_dialog(&set_dialog, ProviderDialogKind::ManualCredential { provider })
            })
            .into(),
        );
        out.push(
            provider_card(provider_row(
                Some(row_icon("key", ctx.color_scheme)),
                "Credential",
                detail,
                trailing,
            ))
            .with_key("account-manual"),
        );
        return out;
    }
    let folder = instance
        .config_folder()
        .or_else(|| crate::instances::default_folder(instance.driver));
    let set_dialog = ctx.set_provider_dialog.clone();
    let sign_in_reason = crate::instances::Capabilities::reason(
        instance,
        crate::instances::Capability::SignIn,
    );
    let detail = match &identity {
        Some(identity) => format!("Signed in as {identity}"),
        None => "Not signed in yet, or no limits read so far.".into(),
    };
    let mut detail: Vec<Element> = vec![secondary_text(detail).into()];
    if let Some(folder) = &folder {
        detail.push(
            text_block(display_fs_path(folder))
                .font_size(12.0)
                .foreground(ThemeRef::TertiaryText)
                .max_lines(1)
                .tooltip(display_fs_path(folder))
                .into(),
        );
    }
    out.push(
        provider_card(provider_row(
            Some(row_icon("user", ctx.color_scheme)),
            "Signed-in account",
            detail,
            vec![
                Button::new(if identity.is_some() {
                    "Sign in again"
                } else {
                    "Sign in"
                })
                .enabled(sign_in_reason.is_none())
                .on_click(move || open_dialog(&set_dialog, ProviderDialogKind::SignIn { provider }))
                .into(),
            ],
        ))
        .with_key("account-sign-in"),
    );
    out
}

fn runtime_sections(
    instance: &ProviderInstance,
    status: &ProviderInstallStatus,
    ctx: &SettingsPageContext<'_>,
) -> Vec<Element> {
    let provider = instance.provider_id();
    let driver = instance.driver;
    let mut out = vec![
        section_header(
            "Runtime",
            Some(&format!(
                "{} Minibar finds these automatically.",
                provider_description(driver)
            )),
            None,
        )
        .with_key("runtime-header"),
    ];
    if matches!(driver, ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo) {
        if instance.is_primary() {
            out.extend(opencode_source_row(status, ctx));
        }
        return out;
    }
    if driver == ProviderKind::OpenRouter {
        out.pop();
        return out;
    }
    if !instance.uses_manual_credential() {
        if status.checking {
            out.push(checking_card(status).with_key("sources-checking"));
        } else {
            let (app_label, crew_label, cli_label) = source_labels(driver);
            if status.app_applicable {
                out.push(
                    source_row(
                        provider,
                        "desktop",
                        app_label,
                        status.app.as_deref(),
                        status.used == Some(ProviderInstallSource::App),
                        driver == ProviderKind::Cursor,
                        ctx,
                    )
                    .with_key("source-app"),
                );
            }
            if status.crew_applicable {
                out.push(
                    source_row(
                        provider,
                        "desktop",
                        crew_label,
                        status.crew.as_deref(),
                        status.used == Some(ProviderInstallSource::Crew),
                        false,
                        ctx,
                    )
                    .with_key("source-crew"),
                );
            }
            if status.cli_applicable {
                out.push(
                    source_row(
                        provider,
                        "terminal-window",
                        cli_label,
                        status.cli.as_deref(),
                        status.used == Some(ProviderInstallSource::Cli),
                        driver != ProviderKind::Kiro,
                        ctx,
                    )
                    .with_key("source-cli"),
                );
            }
        }
    }
    if matches!(driver, ProviderKind::Claude | ProviderKind::Codex) {
        out.extend(source_settings(instance, ctx));
    }
    for config in folder_configs(driver) {
        let key = config.field.key();
        out.push(advanced_folder_expander(instance, config, ctx).with_key(key));
    }
    out
}

/// Source selector (Claude) and config folder for Claude/Codex instances.
fn source_settings(instance: &ProviderInstance, ctx: &SettingsPageContext<'_>) -> Vec<Element> {
    let provider = instance.provider_id();
    let mut out = Vec::new();
    if instance.driver == ProviderKind::Claude {
        let source_tx = ctx.settings_tx.clone();
        let manual = instance.uses_manual_credential();
        out.push(
            control_row(
                "Source",
                "Config folder reads the Claude Code login in CLAUDE_CONFIG_DIR. Manual credential reads limits only from a pasted credential.",
                ComboBox::new(["Config folder", "Manual credential"])
                    .selected_index(i32::from(manual))
                    .on_selection_changed(move |index: i32| {
                        let manual = index == 1;
                        update_instance(source_tx.clone(), provider, move |instance| {
                            if manual == instance.uses_manual_credential() {
                                return;
                            }
                            instance.source = if manual {
                                InstanceSource::Manual
                            } else {
                                InstanceSource::default()
                            };
                            instance.credentials_revision =
                                instance.credentials_revision.wrapping_add(1);
                        });
                    })
                    .width(180.0)
                    .vertical_alignment(VerticalAlignment::Center)
                    .into(),
            )
            .with_key("instance-source"),
        );
        if manual {
            return out;
        }
    }
    let env = if instance.driver == ProviderKind::Claude {
        "CLAUDE_CONFIG_DIR"
    } else {
        "CODEX_HOME"
    };
    let default = if instance.is_primary() {
        crate::instances::default_folder(instance.driver)
    } else {
        crate::instances::managed_folder(&instance.id).ok()
    };
    let placeholder = default
        .as_deref()
        .map(display_fs_path)
        .unwrap_or_default();
    let conflicts = crate::instances::folder_conflicts(ctx.instances);
    let mut body: Vec<Element> = vec![
        secondary_text(format!(
            "Passed to the CLI as {env}. Leave empty to use {}.",
            if instance.is_primary() {
                "the standard folder"
            } else {
                "a folder Minibar creates for this instance"
            }
        ))
        .into(),
        folder_picker(
            provider,
            PathField::ConfigFolder,
            PathField::ConfigFolder.read(instance),
            &placeholder,
            ctx.settings_tx.clone(),
        ),
    ];
    if let Some(other) = conflicts.get(&instance.id) {
        body.push(
            text_block(format!(
                "{other} already reads this folder. Two instances must not share a login, or usage is counted twice."
            ))
            .font_size(12.0)
            .foreground(ThemeRef::SystemCritical)
            .wrap()
            .with_key("folder-conflict")
            .into(),
        );
    }
    out.push(
        provider_card(
            vstack((
                text_block("Config folder").font_size(14.0).wrap(),
                vstack(body)
                    .spacing(8.0)
                    .horizontal_alignment(HorizontalAlignment::Stretch),
            ))
            .spacing(6.0)
            .horizontal_alignment(HorizontalAlignment::Stretch),
        )
        .with_key("instance-config-folder"),
    );
    out
}

fn feature_sections(instance: &ProviderInstance, ctx: &SettingsPageContext<'_>) -> Vec<Element> {
    use crate::instances::{Capabilities, Capability};
    let provider = instance.provider_id();
    let descriptor = crate::provider_registry::descriptor(instance.driver);
    let mut out = vec![section_header("Features", None, None).with_key("features-header")];
    if descriptor.supports_activation {
        let tx = ctx.settings_tx.clone();
        out.push(
            capability_toggle(
                "Automatic activation",
                "Starts this account's 5-hour window when it resets, using its own login. Schedules and pauses are under Limit activation.",
                instance.auto_activation,
                Capabilities::reason(instance, Capability::AutoActivation),
                move |value| {
                    update_instance(tx.clone(), provider, move |instance| {
                        instance.auto_activation = value;
                    })
                },
            )
            .with_key("instance-auto-activation"),
        );
    }
    let tx = ctx.settings_tx.clone();
    out.push(
        capability_toggle(
            "Usage statistics",
            "Scans this instance's local history for the Usage tab and cost totals.",
            instance.usage_stats,
            Capabilities::reason(instance, Capability::UsageStats),
            move |value| {
                update_instance(tx.clone(), provider, move |instance| {
                    instance.usage_stats = value;
                })
            },
        )
        .with_key("instance-usage-stats"),
    );
    out
}

fn source_row(
    provider: ProviderId,
    icon: &'static str,
    label: &str,
    path: Option<&str>,
    in_use: bool,
    can_choose_folder: bool,
    ctx: &SettingsPageContext<'_>,
) -> Element {
    let (detail, trailing): (Vec<Element>, Vec<Element>) = match path {
        Some(path) => {
            let path_for_menu = path.to_owned();
            let set_notice = ctx.set_provider_notice.clone();
            (
                vec![
                    text_block(path)
                        .font_size(12.0)
                        .foreground(ThemeRef::SecondaryText)
                        .max_lines(2)
                        .horizontal_alignment(HorizontalAlignment::Stretch)
                        .tooltip(path)
                        .into(),
                ],
                vec![
                    status_dot(ThemeRef::SystemSuccess),
                    text_block(if in_use { "In use" } else { "Found" })
                        .font_size(12.0)
                        .foreground(ThemeRef::SecondaryText)
                        .vertical_alignment(VerticalAlignment::Center)
                        .into(),
                    more_menu(
                        &["Copy path", "Open folder"],
                        ctx.color_scheme,
                        move |choice: String| {
                            let result = if choice == "Open folder" {
                                reveal_in_explorer(&path_for_menu)
                            } else {
                                copy_text_to_clipboard(&path_for_menu).inspect(|()| {
                                    show_provider_notice(set_notice.clone(), "Path copied.".into());
                                })
                            };
                            if let Err(error) = result {
                                crate::notifications::show(
                                    "That didn't work",
                                    &format!("{error:#}"),
                                );
                            }
                        },
                    ),
                ],
            )
        }
        None => {
            let mut trailing: Vec<Element> = vec![
                text_block("Not found")
                    .font_size(12.0)
                    .foreground(ThemeRef::SecondaryText)
                    .vertical_alignment(VerticalAlignment::Center)
                    .into(),
            ];
            if can_choose_folder {
                let settings_tx = ctx.settings_tx.clone();
                trailing.push(
                    Button::new("Choose folder…")
                        .on_click(move || {
                            pick_folder(provider, PathField::Binary, settings_tx.clone())
                        })
                        .vertical_alignment(VerticalAlignment::Center)
                        .into(),
                );
            }
            (
                vec![
                    secondary_text("Not installed, or installed somewhere Minibar doesn't look.")
                        .into(),
                ],
                trailing,
            )
        }
    };
    provider_card(provider_row(
        Some(row_icon(icon, ctx.color_scheme)),
        label,
        detail,
        trailing,
    ))
}

fn advanced_folder_expander(
    instance: &ProviderInstance,
    config: FolderConfig,
    ctx: &SettingsPageContext<'_>,
) -> Element {
    let provider = instance.provider_id();
    let card_id = format!("provider-{}-advanced-{}", provider.id(), config.field.key());
    let expanded = ctx.expanded_provider_cards.contains(&card_id);
    let toggle_header = toggle_expanded_card(card_id.clone(), ctx);
    settings_content_expander(
        vstack((
            text_block(config.label).font_size(14.0).wrap(),
            secondary_text("Only needed if automatic detection misses your install."),
        ))
        .spacing(2.0)
        .on_tapped(move || toggle_header(!expanded)),
        expanded,
        toggle_expanded_card(card_id.clone(), ctx),
        card_id,
        ctx.hovered_card_id,
        ctx.set_hovered_card_id.clone(),
        vstack((
            secondary_text(config.description),
            folder_picker(
                provider,
                config.field,
                config.field.read(instance),
                config.placeholder,
                ctx.settings_tx.clone(),
            ),
        ))
        .spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
}

fn opencode_source_row(status: &ProviderInstallStatus, ctx: &SettingsPageContext<'_>) -> Vec<Element> {
    if status.checking {
        return vec![checking_card(status).with_key("sources-checking")];
    }
    let found = status.used.is_some();
    let trailing: Vec<Element> = if found {
        vec![
            status_dot(ThemeRef::SystemSuccess),
            text_block("Found")
                .font_size(12.0)
                .foreground(ThemeRef::SecondaryText)
                .vertical_alignment(VerticalAlignment::Center)
                .into(),
        ]
    } else {
        vec![
            text_block("Not found")
                .font_size(12.0)
                .foreground(ThemeRef::SecondaryText)
                .vertical_alignment(VerticalAlignment::Center)
                .into(),
        ]
    };
    vec![
        provider_card(provider_row(
            Some(row_icon("terminal-window", ctx.color_scheme)),
            "OpenCode sign-in or local history",
            vec![
                secondary_text(if found {
                    "Found in OpenCode auth, environment, a saved key, or local history."
                } else {
                    "Nothing found in OpenCode auth, environment, or local history."
                })
                .into(),
            ],
            trailing,
        ))
        .with_key("source-opencode"),
    ]
}

fn opencode_key_section(instance: &ProviderInstance, ctx: &SettingsPageContext<'_>) -> Vec<Element> {
    let provider = instance.provider_id();
    let mut out = vec![section_header("Account", None, None).with_key("account-header")];
    let saved_key = crate::opencode::manual_key(provider)
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty());
    let set_dialog = ctx.set_provider_dialog.clone();
    let key_row = match saved_key {
        Some(key) => provider_row(
            Some(row_icon("key", ctx.color_scheme)),
            "API key",
            vec![secondary_text("Saved in Windows user storage").into()],
            vec![
                masked_key_text(crate::secrets::masked_hint(&key)),
                more_menu(
                    &["Replace key", "Remove key"],
                    ctx.color_scheme,
                    move |choice: String| {
                        let kind = if choice == "Remove key" {
                            ProviderDialogKind::RemoveOpenCodeKey { provider }
                        } else {
                            ProviderDialogKind::OpenCodeKey {
                                provider,
                                replace: true,
                            }
                        };
                        open_dialog(&set_dialog, kind);
                    },
                ),
            ],
        ),
        None => provider_row(
            Some(row_icon("key", ctx.color_scheme)),
            "API key",
            vec![
                secondary_text(if instance.is_primary() {
                    "Optional. Only needed without OpenCode sign-in on this PC."
                } else {
                    "Add the key of the account this instance tracks."
                })
                .into(),
            ],
            vec![
                Button::new("Add API key")
                    .icon(Symbol::Add)
                    .on_click(move || {
                        open_dialog(
                            &set_dialog,
                            ProviderDialogKind::OpenCodeKey {
                                provider,
                                replace: false,
                            },
                        )
                    })
                    .into(),
            ],
        ),
    };
    out.push(provider_card(key_row).with_key("opencode-api-key"));
    out
}

// ---------------------------------------------------------------------------
// OpenRouter
// ---------------------------------------------------------------------------

fn openrouter_sections(instance: &ProviderInstance, ctx: &SettingsPageContext<'_>) -> Vec<Element> {
    let Some(account) = instance.openrouter.clone() else {
        return Vec::new();
    };
    let mut out = vec![
        section_header(
            "Keys",
            Some("A management key shows credit balance and usage history. API keys show spend per key."),
            None,
        )
        .with_key("openrouter-keys-header"),
    ];
    if let Some(at) = ctx.openrouter_snapshot.sampled_at {
        out.push(
            secondary_text(format!(
                "Last updated {}, {}",
                at.with_timezone(&chrono::Local).format("%b %-d"),
                TimeFormat::current().format_hm(at.with_timezone(&chrono::Local))
            ))
            .with_key("openrouter-updated-at")
            .into(),
        );
    }
    let snapshot = ctx
        .openrouter_snapshot
        .accounts
        .iter()
        .find(|snapshot| snapshot.id == account.id);
    let management_hint = crate::openrouter::management_key_hint(&account.id);
    out.push(
        provider_card(openrouter_account_body(&account, snapshot, management_hint, ctx))
            .with_key(format!("openrouter-account-{}", account.id)),
    );
    out
}

fn openrouter_account_body(
    account: &OpenRouterAccount,
    snapshot: Option<&OpenRouterAccountSnapshot>,
    management_hint: anyhow::Result<Option<String>>,
    ctx: &SettingsPageContext<'_>,
) -> Element {
    let set_dialog = ctx.set_provider_dialog.clone();
    let account_id = account.id.clone();
    let mut rows: Vec<Element> = Vec::new();

    let management_row = match management_hint {
        Err(error) => provider_row(
            Some(row_icon("key", ctx.color_scheme)),
            "Management key",
            vec![
                secondary_text("Could not read the saved key. Reopen this page to retry.")
                    .tooltip(format!("{error:#}"))
                    .into(),
            ],
            vec![],
        ),
        Ok(Some(hint)) => {
            let set_dialog = set_dialog.clone();
            let account_id = account_id.clone();
            let mut detail = vec![
                secondary_text("Credit balance and account-wide usage history").into(),
            ];
            if let Some(balance) = snapshot.and_then(|snapshot| snapshot.balance_microusd) {
                detail.push(secondary_text(format!("{} credit", money(balance))).into());
            }
            provider_row(
                Some(row_icon("key", ctx.color_scheme)),
                "Management key",
                detail,
                vec![
                    masked_key_text(hint),
                    more_menu(
                        &["Replace key", "Remove key"],
                        ctx.color_scheme,
                        move |choice: String| {
                            let kind = if choice == "Remove key" {
                                ProviderDialogKind::RemoveOpenRouterManagementKey {
                                    account_id: account_id.clone(),
                                }
                            } else {
                                ProviderDialogKind::OpenRouterManagementKey {
                                    account_id: account_id.clone(),
                                    replace: true,
                                }
                            };
                            open_dialog(&set_dialog, kind);
                        },
                    ),
                ],
            )
        }
        Ok(None) => {
            let set_dialog = set_dialog.clone();
            let account_id = account_id.clone();
            provider_row(
                Some(row_icon("key", ctx.color_scheme)),
                "Management key",
                vec![
                    secondary_text("Not added. Add one to see credit balance and usage history.")
                        .into(),
                ],
                vec![
                    Button::new("Add management key")
                        .on_click(move || {
                            open_dialog(
                                &set_dialog,
                                ProviderDialogKind::OpenRouterManagementKey {
                                    account_id: account_id.clone(),
                                    replace: false,
                                },
                            )
                        })
                        .into(),
                ],
            )
        }
    };
    rows.push(management_row.with_key("management-key"));
    rows.push(
        divider(ctx.color_scheme)
            .margin(Thickness {
                left: 0.0,
                top: 12.0,
                right: 0.0,
                bottom: 8.0,
            })
            .with_key("keys-divider")
            .into(),
    );

    let add_key_dialog = set_dialog;
    let add_key_account = account_id;
    rows.push(
        grid((
            text_block("API keys")
                .font_size(12.0)
                .semibold()
                .foreground(ThemeRef::SecondaryText)
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(0),
            Button::new("Add API key")
                .icon(Symbol::Add)
                .subtle()
                .foreground(ThemeRef::AccentText)
                .on_click(move || {
                    open_dialog(
                        &add_key_dialog,
                        ProviderDialogKind::OpenRouterApiKey {
                            account_id: add_key_account.clone(),
                            key_id: None,
                        },
                    )
                })
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .with_key("keys-header")
        .into(),
    );
    rows.push(openrouter_key_table(account, snapshot, ctx).with_key("keys-table"));

    vstack(rows)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into()
}

fn provider_description(provider: ProviderKind) -> &'static str {
    match provider {
        ProviderKind::Codex => "Reads the signed-in Codex CLI or desktop app.",
        ProviderKind::Claude => "Reads your existing Claude Code login.",
        ProviderKind::Cursor => "Reads the signed-in Cursor app for this billing cycle.",
        ProviderKind::OpenCodeZen => "Reads Zen auth and local OpenCode history.",
        ProviderKind::OpenCodeGo => "Reads Go quota windows and local OpenCode history.",
        ProviderKind::OpenRouter => {
            "Reads API-key usage and spend limits. A management key also enables usage history and credit balance."
        }
        ProviderKind::Antigravity => {
            "Reads subscription quota from your existing official agy Windows sign-in."
        }
        ProviderKind::Grok => {
            "Reads SuperGrok subscription credits from your existing official Grok CLI sign-in."
        }
        ProviderKind::Kiro => {
            "Fetches Kiro's live monthly credits with its shared sign-in; recognizes IDE, Crew, and CLI installs."
        }
    }
}

/// Display names for the app, Crew app, and CLI sources a provider can read from.
fn source_labels(provider: ProviderKind) -> (&'static str, &'static str, &'static str) {
    match provider {
        ProviderKind::Codex => ("Codex desktop app", "", "Codex CLI"),
        ProviderKind::Claude => ("Claude desktop app", "", "Claude Code CLI"),
        ProviderKind::Cursor => ("Cursor app", "", ""),
        ProviderKind::Antigravity => ("Antigravity app", "", "agy CLI"),
        ProviderKind::Grok => ("", "", "Grok CLI"),
        ProviderKind::Kiro => ("Kiro IDE", "Kiro Crew", "Kiro CLI"),
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo | ProviderKind::OpenRouter => {
            ("", "", "")
        }
    }
}

fn checking_card(status: &ProviderInstallStatus) -> Element {
    let message = if status.crew_applicable {
        "Checking Kiro IDE, Kiro Crew, and CLI…"
    } else if status.app_applicable && status.cli_applicable {
        "Checking installed app and CLI…"
    } else if status.cli_applicable {
        "Checking CLI…"
    } else {
        "Checking installed app…"
    };
    provider_card(secondary_text(message))
}

fn codex_logo_toggle(ctx: &SettingsPageContext<'_>) -> Element {
    let set_replace_chatgpt_logo_with_codex = ctx.set_replace_chatgpt_logo_with_codex.clone();
    let settings_tx = ctx.settings_tx.clone();
    settings_toggle_card(
        "Replace ChatGPT logo with Codex",
        ctx.replace_chatgpt_logo_with_codex,
        move |value| {
            crate::provider_registry::apply_logo_settings(value);
            persist_bool(
                set_replace_chatgpt_logo_with_codex.clone(),
                settings_tx.clone(),
                value,
                |settings, value| {
                    settings.replace_chatgpt_logo_with_codex = value;
                },
            );
        },
        "codex-replace-logo",
        ctx.hovered_card_id,
        ctx.set_hovered_card_id.clone(),
    )
}

fn key_table_columns() -> [GridLength; 5] {
    [
        GridLength::Star(1.0),
        GridLength::Pixel(120.0),
        GridLength::Pixel(128.0),
        GridLength::Pixel(64.0),
        GridLength::Pixel(36.0),
    ]
}

/// Only the provider knows the account-wide purchased credit. Tracked keys
/// can be a subset, so their spend plus the balance is not a valid total.
fn account_credit_total(snapshot: Option<&OpenRouterAccountSnapshot>) -> Option<u64> {
    snapshot?.total_credits_microusd.filter(|total| *total > 0)
}

fn spend_bar(percent: f64, tooltip: String) -> Element {
    const WIDTH: f64 = 64.0;
    const HEIGHT: f64 = 6.0;
    let percent = percent.clamp(0.0, 100.0);
    let mut layers: Vec<Element> = vec![
        border(Element::Empty)
            .width(WIDTH)
            .height(HEIGHT)
            .corner_radius(HEIGHT / 2.0)
            .background(ThemeRef::ControlStrokeSecondary)
            .horizontal_alignment(HorizontalAlignment::Left)
            .into(),
    ];
    if percent > 0.0 {
        layers.push(
            border(Element::Empty)
                .width((WIDTH * percent / 100.0).max(2.0))
                .height(HEIGHT)
                .corner_radius(HEIGHT / 2.0)
                .background(ThemeRef::Accent)
                .horizontal_alignment(HorizontalAlignment::Left)
                .into(),
        );
    }
    grid(layers)
        .width(WIDTH)
        .height(HEIGHT)
        .tooltip(tooltip)
        .vertical_alignment(VerticalAlignment::Center)
        .into()
}

fn key_table_header_cell(label: &str, column: i32, right: bool) -> Element {
    text_block(label)
        .font_size(12.0)
        .foreground(ThemeRef::SecondaryText)
        .horizontal_alignment(if right {
            HorizontalAlignment::Right
        } else {
            HorizontalAlignment::Left
        })
        .vertical_alignment(VerticalAlignment::Center)
        .grid_column(column)
        .into()
}

fn available_key_spending(key: &OpenRouterApiKeySnapshot) -> Option<&SpendingSummary> {
    key.has_live_usage.then_some(&key.spending)
}

fn openrouter_key_table(
    account: &OpenRouterAccount,
    snapshot: Option<&OpenRouterAccountSnapshot>,
    ctx: &SettingsPageContext<'_>,
) -> Element {
    if account.api_key_ids.is_empty() {
        return secondary_text("No API keys yet. Add one to track spend per key.")
            .margin(Thickness {
                left: 0.0,
                top: 8.0,
                right: 0.0,
                bottom: 4.0,
            })
            .into();
    }
    let mut rows: Vec<Element> = vec![
        grid(vec![
            key_table_header_cell("Name", 0, false),
            key_table_header_cell("Key", 1, false),
            key_table_header_cell("Spend", 2, true),
            key_table_header_cell("Limit", 3, true),
        ])
        .columns(key_table_columns())
        .rows([GridLength::Auto])
        .column_spacing(8.0)
        .margin(Thickness {
            left: 0.0,
            top: 8.0,
            right: 0.0,
            bottom: 6.0,
        })
        .vertical_alignment(VerticalAlignment::Top)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .with_key("key-table-header")
        .into(),
    ];

    let credit_total = account_credit_total(snapshot);
    for key_id in &account.api_key_ids {
        let key_snapshot =
            snapshot.and_then(|snapshot| snapshot.api_keys.iter().find(|key| &key.id == key_id));
        let hint_result = crate::openrouter::api_key_hint(&account.id, key_id);
        let read_error = hint_result.as_ref().err().map(|error| format!("{error:#}"));
        let hint = hint_result.ok().flatten();
        let saved = hint.is_some();
        let name: Element = if let Some(error) = &read_error {
            text_block("Could not read key")
                .font_size(13.0)
                .wrap()
                .foreground(ThemeRef::SystemCaution)
                .tooltip(format!("{error}. Reopen this page to retry."))
                .into()
        } else {
            match account
                .api_key_names
                .get(key_id)
                .cloned()
                .or_else(|| key_snapshot.and_then(|key| key.label.clone()))
                .filter(|label| !label.trim().is_empty())
            {
                Some(label) => text_block(label).font_size(13.0).wrap().into(),
                None if !saved => text_block("Not saved")
                    .font_size(13.0)
                    .foreground(ThemeRef::SystemCaution)
                    .into(),
                None => text_block("Unnamed")
                    .font_size(13.0)
                    .foreground(ThemeRef::TertiaryText)
                    .into(),
            }
        };
        // The saved key is authoritative immediately after a replacement;
        // the worker may still be showing its previous snapshot.
        let masked = hint.unwrap_or_else(|| "—".into());
        let spending = key_snapshot
            .and_then(available_key_spending)
            .filter(|_| saved);
        let spend_cell: Element = match &spending {
            Some(spending) => {
                let mut cell: Vec<Element> = Vec::new();
                let used = spending.used_microusd;
                if let Some(limit) = spending.limit_microusd.filter(|limit| *limit > 0) {
                    cell.push(spend_bar(
                        used as f64 / limit as f64 * 100.0,
                        format!("{} of the {} limit", money(used), money(limit)),
                    ));
                } else if let Some(total) = credit_total {
                    cell.push(spend_bar(
                        used as f64 / total as f64 * 100.0,
                        format!(
                            "No spend limit. Key spend: {}. Account credits purchased: {}.",
                            money(used),
                            money(total)
                        ),
                    ));
                }
                cell.push(
                    text_block(money(spending.used_microusd))
                        .font_size(13.0)
                        .vertical_alignment(VerticalAlignment::Center)
                        .into(),
                );
                hstack(cell)
                    .spacing(8.0)
                    .horizontal_alignment(HorizontalAlignment::Right)
                    .into()
            }
            None => text_block("—")
                .font_size(13.0)
                .foreground(ThemeRef::TertiaryText)
                .horizontal_alignment(HorizontalAlignment::Right)
                .into(),
        };
        // Limits are stable metadata and remain useful when a usage poll fails.
        let limit_cell: Element = match key_snapshot
            .filter(|_| saved)
            .map(|key| key.spending.limit_microusd)
        {
            Some(Some(limit)) => text_block(money(limit)).font_size(13.0).into(),
            Some(None) => text_block("None")
                .font_size(13.0)
                .foreground(ThemeRef::TertiaryText)
                .into(),
            None => text_block("—")
                .font_size(13.0)
                .foreground(ThemeRef::TertiaryText)
                .into(),
        };
        let set_dialog = ctx.set_provider_dialog.clone();
        let menu_account = account.id.clone();
        let menu_key = key_id.clone();
        let local_name = account
            .api_key_names
            .get(key_id)
            .cloned()
            .unwrap_or_default();
        let menu = more_menu(
            if saved || read_error.is_some() {
                &["Rename key", "Remove key"]
            } else {
                &["Rename key", "Add key", "Remove key"]
            },
            ctx.color_scheme,
            move |choice: String| {
                if choice == "Rename key" {
                    set_dialog.call(Some(ProviderDialog::with_name(
                        ProviderDialogKind::RenameOpenRouterApiKey {
                            account_id: menu_account.clone(),
                            key_id: menu_key.clone(),
                        },
                        local_name.clone(),
                    )));
                    return;
                }
                let kind = if choice == "Remove key" {
                    ProviderDialogKind::RemoveOpenRouterApiKey {
                        account_id: menu_account.clone(),
                        key_id: menu_key.clone(),
                    }
                } else {
                    ProviderDialogKind::OpenRouterApiKey {
                        account_id: menu_account.clone(),
                        key_id: Some(menu_key.clone()),
                    }
                };
                open_dialog(&set_dialog, kind);
            },
        );

        rows.push(
            divider(ctx.color_scheme)
                .with_key(format!("key-divider-{key_id}"))
                .into(),
        );
        rows.push(
            grid(vec![
                name.vertical_alignment(VerticalAlignment::Center)
                    .grid_column(0),
                masked_key_text(masked).grid_column(1),
                spend_cell
                    .vertical_alignment(VerticalAlignment::Center)
                    .grid_column(2),
                limit_cell
                    .horizontal_alignment(HorizontalAlignment::Right)
                    .vertical_alignment(VerticalAlignment::Center)
                    .grid_column(3),
                menu.horizontal_alignment(HorizontalAlignment::Right)
                    .grid_column(4),
            ])
            .columns(key_table_columns())
            .rows([GridLength::Star(1.0)])
            .column_spacing(8.0)
            .height(44.0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .with_key(format!("key-row-{key_id}"))
            .into(),
        );
    }

    vstack(rows)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into()
}

#[derive(Clone, Copy)]
enum DialogField {
    Name,
    Key,
    SecondKey,
}

fn dialog_field_handler(dialog: &ProviderDialog, field: DialogField) -> impl Fn(String) + 'static {
    let inputs = dialog.inputs.clone();
    move |value: String| {
        if let Ok(mut inputs) = inputs.lock() {
            match field {
                DialogField::Name => inputs.name = value,
                DialogField::Key => inputs.key = value,
                DialogField::SecondKey => inputs.second_key = value,
            }
        }
    }
}

fn dialog_submit_enter(on_submit: impl Fn() + Clone + 'static) -> KeyboardAccelerator {
    KeyboardAccelerator::new(VirtualKey::Enter, VirtualKeyModifiers::None, on_submit)
}

fn dialog_password(
    dialog: &ProviderDialog,
    field: DialogField,
    header: &str,
    placeholder: &str,
    help: &str,
    on_submit: impl Fn() + Clone + 'static,
) -> Element {
    vstack((
        PasswordBox::new()
            .header(header)
            .placeholder_text(placeholder)
            .enabled(!dialog.checking)
            .on_password_changed(dialog_field_handler(dialog, field))
            .keyboard_accelerator(dialog_submit_enter(on_submit))
            .horizontal_alignment(HorizontalAlignment::Stretch),
        secondary_text(help),
    ))
    .spacing(4.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn instruction_link(label: &str, url: &str) -> Element {
    HyperlinkButton::new(label)
        .navigate_uri(url)
        .font_size(16.0)
        .padding(Thickness::uniform(0.0))
        .horizontal_alignment(HorizontalAlignment::Left)
        .into()
}

fn instruction_text(text: impl Into<String>) -> TextBlock {
    text_block(text)
        .font_size(16.0)
        .foreground(ThemeRef::PrimaryText)
        .wrap()
}

fn claude_credential_tabs(
    dialog: &ProviderDialog,
    set_dialog: AsyncSetState<Option<ProviderDialog>>,
) -> Element {
    let tabs = [
        ("Browser session", ProfileCredentialMethod::BrowserSession),
        ("OAuth token", ProfileCredentialMethod::OAuthToken),
    ]
    .into_iter()
    .map(|(label, method)| {
        let current = dialog.clone();
        let set_dialog = set_dialog.clone();
        crate::settings_controls::segmented_tab(label, dialog.claude_method == method, move || {
            if !current.checking && method != current.claude_method {
                let mut next = current.clone();
                next.claude_method = method;
                next.error = None;
                set_dialog.call(Some(next));
            }
        })
    })
    .collect();
    crate::settings_controls::segmented_control("claude-credential-methods", tabs, true)
}

fn claude_credential_fields(
    dialog: &ProviderDialog,
    set_dialog: AsyncSetState<Option<ProviderDialog>>,
    on_submit: impl Fn() + Clone + 'static,
) -> Element {
    let inputs = dialog.inputs();
    let (field, header, placeholder, value, key) = match dialog.claude_method {
        ProfileCredentialMethod::BrowserSession => (
            DialogField::Key,
            "Session key",
            "Paste the sessionKey value",
            inputs.key,
            "claude-session-input",
        ),
        ProfileCredentialMethod::OAuthToken => (
            DialogField::SecondKey,
            "OAuth access token",
            "sk-ant-oat…",
            inputs.second_key,
            "claude-oauth-input",
        ),
    };
    let mut input_group: Vec<Element> = vec![
        PasswordBox::new()
            .header(header)
            .placeholder_text(placeholder)
            .value(value)
            .enabled(!dialog.checking)
            .on_password_changed(dialog_field_handler(dialog, field))
            .keyboard_accelerator(dialog_submit_enter(on_submit))
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .with_key(key)
            .into(),
    ];
    if let Some(error) = &dialog.error {
        input_group.push(
            text_block(error.clone())
                .font_size(14.0)
                .foreground(ThemeRef::SystemCritical)
                .wrap()
                .with_key("claude-credential-error")
                .into(),
        );
    }
    let mut instructions: Vec<Element> = Vec::new();
    instructions.push(instruction_text("The saved credential is replaced only after the new one passes the check.").into());
    instructions.push(claude_method_instructions(dialog.claude_method));
    vstack((
        claude_credential_tabs(dialog, set_dialog),
        vstack(input_group)
            .spacing(4.0)
            .horizontal_alignment(HorizontalAlignment::Stretch),
        vstack(instructions)
            .spacing(10.0)
            .horizontal_alignment(HorizontalAlignment::Stretch),
    ))
    .spacing(14.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .with_layout_animation(
        LayoutAnimationConfig::linear(duration(CONTROL_NORMAL_ANIMATION)).animate_size(true),
    )
    .into()
}

fn dialog_key_name_box(dialog: &ProviderDialog, on_submit: impl Fn() + Clone + 'static) -> Element {
    vstack((
        text_box(dialog.initial_name.clone())
            .header("Key name (optional)")
            .placeholder_text("e.g. Personal")
            .enabled(!dialog.checking)
            .on_text_changed(dialog_field_handler(dialog, DialogField::Name))
            .keyboard_accelerator(dialog_submit_enter(on_submit))
            .horizontal_alignment(HorizontalAlignment::Stretch),
        secondary_text("Leave blank to use the name from OpenRouter."),
    ))
    .spacing(4.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn claude_method_instructions(method: ProfileCredentialMethod) -> Element {
    let steps: Vec<Element> = match method {
        ProfileCredentialMethod::BrowserSession => vec![
            instruction_text("Reads the session and weekly limits of a Claude subscription without a Claude Code login.").into(),
            instruction_text("1. Open Claude in a separate browser profile or private window. Sign in to the account you want to track and confirm its email in Claude's settings.").into(),
            instruction_link("Open claude.ai", "https://claude.ai"),
            instruction_text("2. In Chrome or Edge, press F12. Open Application > Storage > Cookies and select https://claude.ai.").into(),
            instruction_text("3. Find sessionKey. Copy its Value, not its name or the whole cookie table, and paste it into the field above.").into(),
            instruction_link("How to view cookies in Chrome", "https://developer.chrome.com/docs/devtools/application/cookies"),
            instruction_text("A Cookie header containing sessionKey also works. When the session expires, paste a fresh one here.").into(),
        ],
        ProfileCredentialMethod::OAuthToken => vec![
            instruction_text("Use the access token from a Claude Code subscription login. Minibar cannot refresh a pasted token; prefer Source: Config folder when you can.").into(),
            instruction_text("Copy only claudeAiOauth.accessToken (starts with sk-ant-oat) from that login's .credentials.json, without quotes. Do not use claude setup-token: it may lack usage access.").into(),
            instruction_link("Claude Code: log in with multiple accounts", "https://code.claude.com/docs/en/authentication#log-in-with-multiple-accounts"),
            instruction_text("Requires a Claude subscription login with usage access. API keys and Admin API keys do not show subscription limits.").into(),
        ],
    };
    // The dialog body owns scrolling, including the instructions and input.
    vstack(steps)
        .spacing(10.0)
        .with_layout_animation(
            LayoutAnimationConfig::linear(duration(CONTROL_NORMAL_ANIMATION)).animate_size(true),
        )
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into()
}

/// Drivers offered by "Add provider", with the reason a driver can't be added.
fn add_instance_choices(instances: &[ProviderInstance]) -> Vec<(ProviderKind, Option<String>)> {
    ProviderKind::ALL
        .into_iter()
        .map(|driver| {
            let exists = instances.iter().any(|instance| instance.driver == driver);
            let single = !crate::provider_registry::descriptor(driver).supports_multiple_instances;
            (
                driver,
                (exists && single)
                    .then(|| format!("{} supports one instance", driver.display_name())),
            )
        })
        .collect()
}

fn add_instance_fields(
    dialog: &ProviderDialog,
    instances: &[ProviderInstance],
    set_dialog: AsyncSetState<Option<ProviderDialog>>,
    on_submit: impl Fn() + Clone + 'static,
) -> Vec<Element> {
    let inputs = dialog.inputs();
    let choices = add_instance_choices(instances);
    let labels = choices
        .iter()
        .map(|(driver, blocked)| match blocked {
            Some(_) => format!("{} (already added)", driver.display_name()),
            None => driver.display_name().to_owned(),
        })
        .collect::<Vec<_>>();
    let selected = inputs
        .driver
        .and_then(|driver| choices.iter().position(|(choice, _)| *choice == driver))
        .map_or(-1, |index| index as i32);
    let driver_dialog = dialog.clone();
    let mut fields: Vec<Element> = vec![
        ComboBox::new(labels)
            .header("Provider")
            .placeholder_text("Choose a provider")
            .selected_index(selected)
            .enabled(!dialog.checking)
            .on_selection_changed(move |index: i32| {
                let Some((driver, _)) = usize::try_from(index)
                    .ok()
                    .and_then(|index| choices.get(index))
                else {
                    return;
                };
                if let Ok(mut inputs) = driver_dialog.inputs.lock() {
                    inputs.driver = Some(*driver);
                }
                // Re-render so the name placeholder follows the driver.
                let mut next = driver_dialog.clone();
                next.error = None;
                set_dialog.call(Some(next));
            })
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .into(),
    ];
    let placeholder = inputs
        .driver
        .map_or("e.g. Work", |driver| driver.display_name());
    fields.push(
        vstack((
            text_box(dialog.initial_name.clone())
                .header("Name")
                .placeholder_text(placeholder)
                .enabled(!dialog.checking)
                .on_text_changed(dialog_field_handler(dialog, DialogField::Name))
                .keyboard_accelerator(dialog_submit_enter(on_submit.clone()))
                .horizontal_alignment(HorizontalAlignment::Stretch),
            secondary_text("Shown on its tab, Home card, tray and notifications."),
        ))
        .spacing(4.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into(),
    );
    let badge_inputs = dialog.inputs.clone();
    let color_inputs = dialog.inputs.clone();
    fields.push(
        grid((
            text_box(String::new())
                .header("Badge")
                .placeholder_text("Auto")
                .enabled(!dialog.checking)
                .on_text_changed(move |value: String| {
                    if let Ok(mut inputs) = badge_inputs.lock() {
                        inputs.badge = crate::instances::sanitize_badge(&value);
                    }
                })
                .keyboard_accelerator(dialog_submit_enter(on_submit))
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .grid_column(0),
            ComboBox::new(BadgeColor::ALL.map(BadgeColor::label))
                .header("Badge color")
                .selected_index(inputs.badge_color.index())
                .enabled(!dialog.checking)
                .on_selection_changed(move |index: i32| {
                    if let Ok(mut inputs) = color_inputs.lock() {
                        inputs.badge_color = BadgeColor::from_index(index);
                    }
                })
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
        .column_spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into(),
    );
    fields.push(
        secondary_text("Up to three letters; empty uses the name's initials. Badges show while a provider has more than one instance turned on.").into(),
    );
    fields
}

pub(super) fn provider_dialog_overlay(
    dialog: &ProviderDialog,
    instances: &[ProviderInstance],
    actions: ProviderDialogActions,
) -> Element {
    let instance_name = |provider: &ProviderId| {
        instances
            .iter()
            .find(|instance| instance.id == provider.id())
            .map(|instance| provider_label(instance, instances))
            .unwrap_or_else(|| "this provider".into())
    };
    let account_name = |account_id: &str| {
        find_account_instance(instances, account_id)
            .map(|instance| provider_label(instance, instances))
            .unwrap_or_else(|| "this".into())
    };

    let on_submit = {
        let dialog = dialog.clone();
        let instances = instances.to_vec();
        let actions = actions.clone();
        move || submit_provider_dialog(dialog.clone(), instances.clone(), actions.clone())
    };

    let mut fields: Vec<Element> = Vec::new();
    let (title, primary) = match &dialog.kind {
        ProviderDialogKind::AddInstance => {
            fields.extend(add_instance_fields(
                dialog,
                instances,
                actions.set_dialog.clone(),
                on_submit.clone(),
            ));
            ("Add provider".to_owned(), "Add")
        }
        ProviderDialogKind::DeleteInstance { provider } => {
            let name = instance_name(provider);
            let folder = instances
                .iter()
                .find(|instance| instance.id == provider.id())
                .is_some_and(|instance| {
                    matches!(instance.driver, ProviderKind::Claude | ProviderKind::Codex)
                        && !instance.uses_manual_credential()
                });
            fields.push(
                secondary_text(format!(
                    "Minibar stops reading {name} and forgets its saved keys, schedules, tray indicators and Home position.{}",
                    if folder {
                        " Its config folder stays on disk."
                    } else {
                        ""
                    }
                ))
                .into(),
            );
            (format!("Delete {name}?"), "Delete")
        }
        ProviderDialogKind::SignIn { provider } => {
            let name = instance_name(provider);
            let driver = provider.kind();
            fields.push(secondary_text(if dialog.checking {
                "Finish signing in in your browser. Cancel stops this login.".to_owned()
            } else if driver == ProviderKind::Claude {
                format!("Runs Claude Code's own login for {name} with its config folder as CLAUDE_CONFIG_DIR. The login stays in that folder, where Claude Code keeps it fresh. Requires native Windows Claude Code.")
            } else {
                format!("Runs Codex's own login for {name} with its config folder as CODEX_HOME. The login stays in that folder, where Codex keeps it fresh. Requires the native Codex CLI or desktop app.")
            }).into());
            (format!("Sign in to {name}"), "Sign in")
        }
        ProviderDialogKind::ManualCredential { provider } => {
            fields.push(
                secondary_text(format!("For {}.", instance_name(provider))).into(),
            );
            fields.push(claude_credential_fields(
                dialog,
                actions.set_dialog.clone(),
                on_submit.clone(),
            ));
            ("Claude credential".to_owned(), "Check and save")
        }
        ProviderDialogKind::OpenRouterApiKey { account_id, key_id } => {
            let replacing = key_id
                .as_ref()
                .is_some_and(|key_id| crate::openrouter::api_key_is_configured(account_id, key_id));
            fields.push(secondary_text(format!("For {}.", account_name(account_id))).into());
            if !replacing {
                fields.push(dialog_key_name_box(dialog, on_submit.clone()));
            }
            fields.push(dialog_password(
                dialog,
                DialogField::Key,
                "API key",
                "sk-or-v1-…",
                "Minibar checks the key with OpenRouter before saving it.",
                on_submit.clone(),
            ));
            (
                if replacing {
                    "Replace API key"
                } else {
                    "Add API key"
                }
                .to_owned(),
                "Check and save",
            )
        }
        ProviderDialogKind::OpenRouterManagementKey {
            account_id,
            replace,
        } => {
            fields.push(secondary_text(format!("For {}.", account_name(account_id))).into());
            fields.push(dialog_password(
                dialog,
                DialogField::Key,
                "Management key",
                "sk-or-v1-…",
                "Create one under Settings → Management keys on openrouter.ai.",
                on_submit.clone(),
            ));
            (
                if *replace {
                    "Replace management key"
                } else {
                    "Add management key"
                }
                .to_owned(),
                "Check and save",
            )
        }
        ProviderDialogKind::RenameOpenRouterApiKey { .. } => {
            fields.push(dialog_key_name_box(dialog, on_submit.clone()));
            ("Rename key".to_owned(), "Save")
        }
        ProviderDialogKind::RemoveOpenRouterApiKey { account_id, key_id } => {
            let hint = crate::openrouter::api_key_hint(account_id, key_id)
                .ok()
                .flatten()
                .unwrap_or_else(|| "this key".into());
            fields.push(
                secondary_text(format!(
                    "Minibar stops tracking {hint}. The key keeps working on OpenRouter."
                ))
                .into(),
            );
            ("Remove API key?".to_owned(), "Remove")
        }
        ProviderDialogKind::RemoveOpenRouterManagementKey { account_id } => {
            fields.push(
                secondary_text(format!(
                    "Minibar stops showing credit balance and usage history for {}. The key keeps working on OpenRouter.",
                    account_name(account_id)
                ))
                .into(),
            );
            ("Remove management key?".to_owned(), "Remove")
        }
        ProviderDialogKind::OpenCodeKey { provider, replace } => {
            fields.push(secondary_text(format!("For {}.", instance_name(provider))).into());
            fields.push(dialog_password(
                dialog,
                DialogField::Key,
                "API key",
                "sk-…",
                "Saved in Windows user storage, never in the settings file.",
                on_submit.clone(),
            ));
            (
                if *replace {
                    "Replace API key"
                } else {
                    "Add API key"
                }
                .to_owned(),
                "Save key",
            )
        }
        ProviderDialogKind::RemoveOpenCodeKey { provider } => {
            fields.push(
                secondary_text(format!(
                    "Minibar forgets the saved key of {}. It keeps working with OpenCode.",
                    instance_name(provider)
                ))
                .into(),
            );
            ("Remove API key?".to_owned(), "Remove")
        }
    };
    if let Some(error) = &dialog.error
        && !matches!(dialog.kind, ProviderDialogKind::ManualCredential { .. })
    {
        fields.push(
            text_block(error.clone())
                .font_size(12.0)
                .foreground(ThemeRef::SystemCritical)
                .wrap()
                .into(),
        );
    }

    let mut body: Vec<Element> = vec![text_block(title).font_size(20.0).semibold().wrap().into()];
    body.extend(fields);

    let primary_button = {
        let dialog = dialog.clone();
        let instances = instances.to_vec();
        let actions = actions.clone();
        let button = Button::new(if dialog.checking {
            if dialog.is_sign_in() {
                "Signing in…"
            } else {
                "Checking…"
            }
        } else {
            primary
        })
        .enabled(!dialog.checking)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .accent();
        button
            .on_click(move || {
                submit_provider_dialog(dialog.clone(), instances.clone(), actions.clone())
            })
            .grid_column(0)
    };
    let cancel_dialog = actions.set_dialog.clone();
    let can_cancel = !dialog.checking;
    let login_pending = dialog.checking && dialog.is_sign_in();
    let login_control = dialog.login_control.clone();
    let cancel_button = Button::new("Cancel")
        .enabled(can_cancel || login_pending)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .on_click(move || {
            if login_pending {
                login_control.cancel();
            } else {
                cancel_dialog.call(None);
            }
        })
        .grid_column(1);

    let tall_dialog = matches!(dialog.kind, ProviderDialogKind::ManualCredential { .. });
    let dialog_body = vstack(body)
        .spacing(14.0)
        .horizontal_alignment(HorizontalAlignment::Stretch);
    let dialog_body: Element = if tall_dialog {
        scroll_viewer(dialog_body.with_layout_animation(
            LayoutAnimationConfig::linear(duration(CONTROL_NORMAL_ANIMATION)).animate_size(true),
        ))
        .into()
    } else {
        dialog_body.into()
    };
    let body_panel = border(dialog_body).padding(Thickness::uniform(24.0));
    let button_panel = border(
        grid((primary_button, cancel_button))
            .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
            .column_spacing(8.0)
            .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(Thickness::uniform(24.0))
    .background(ThemeRef::LayerFill)
    .border_thickness(Thickness {
        left: 0.0,
        top: 1.0,
        right: 0.0,
        bottom: 0.0,
    })
    .border_brush(ThemeRef::CardStroke);
    let card_content: Element = if tall_dialog {
        grid((body_panel.grid_row(0), button_panel.grid_row(1)))
            .rows([GridLength::Star(1.0), GridLength::Auto])
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .into()
    } else {
        vstack((body_panel, button_panel))
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .into()
    };
    let mut card = border(card_content)
        .background(ThemeRef::SolidBackground)
        .corner_radius(8.0)
        .border_thickness(Thickness::uniform(1.0))
        .border_brush(ThemeRef::CardStroke)
        .width(if tall_dialog { 520.0 } else { DIALOG_WIDTH })
        .with_layout_animation(
            LayoutAnimationConfig::linear(duration(CONTROL_NORMAL_ANIMATION)).animate_size(true),
        )
        .horizontal_alignment(HorizontalAlignment::Center)
        .vertical_alignment(VerticalAlignment::Center)
        .on_tapped(|| {});
    if tall_dialog {
        // A star-sized scrollable body and Auto footer keep both buttons
        // reachable when Settings is resized down to its 400-DIP minimum.
        card = card
            .max_height(600.0)
            .margin(Thickness::uniform(16.0))
            .vertical_alignment(VerticalAlignment::Stretch);
    }

    let dismiss = actions.set_dialog;
    let dismiss_escape = dismiss.clone();
    let escape_control = dialog.login_control.clone();
    relative_panel::<Vec<Element>>(vec![
        border(Element::Empty)
            .background(DIALOG_SCRIM)
            .relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .relative_align_bottom()
            .on_tapped(move || {
                if can_cancel {
                    dismiss.call(None);
                }
            })
            .into(),
        card.relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .relative_align_bottom()
            .into(),
    ])
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Stretch)
    .keyboard_accelerator(KeyboardAccelerator::new(
        VirtualKey::Escape,
        VirtualKeyModifiers::None,
        move || {
            if login_pending {
                escape_control.cancel();
            } else if can_cancel {
                dismiss_escape.call(None);
            }
        },
    ))
    .with_key("provider-dialog-overlay")
    .into()
}

/// `Claude · Work` when the driver has several instances, else the name.
fn provider_label(instance: &ProviderInstance, instances: &[ProviderInstance]) -> String {
    let shared = instances
        .iter()
        .filter(|other| other.driver == instance.driver)
        .count()
        > 1;
    let name = instance.display_name();
    if shared && name != instance.driver.display_name() {
        format!("{} \u{00b7} {name}", instance.driver.display_name())
    } else {
        name
    }
}

/// Runs the CLI's own login with the instance's config folder, then advances
/// its credential revision so the worker re-reads the folder.
fn submit_sign_in(dialog: ProviderDialog, provider: ProviderId, actions: ProviderDialogActions) {
    let control = dialog.login_control.clone();
    let settings_tx = actions.settings_tx.clone();
    run_dialog_work(dialog, actions, move || {
        let settings = Settings::load_or_create(&Settings::default_path()?)?;
        let instance = settings
            .instance(provider)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("This provider no longer exists."))?;
        anyhow::ensure!(
            !instance.uses_manual_credential(),
            "Switch Source to Config folder to sign in."
        );
        let folder = instance
            .config_folder()
            .or_else(|| crate::instances::default_folder(instance.driver))
            .ok_or_else(|| anyhow::anyhow!("This provider has no config folder."))?;
        anyhow::ensure!(
            !crate::instances::folder_conflicts(&settings.instances).contains_key(&instance.id),
            "Another instance already reads this config folder. Choose a different folder first."
        );
        match instance.driver {
            ProviderKind::Claude => crate::claude::profile_oauth::login(
                instance.binary_path.as_deref(),
                &folder,
                &control,
            )?,
            ProviderKind::Codex => crate::codex::profile_oauth::login(
                instance.binary_path.as_deref(),
                &folder,
                &control,
            )?,
            _ => anyhow::bail!("This provider has no sign-in."),
        }
        control.begin_save()?;
        if !instance.is_primary() && matches!(instance.source, InstanceSource::ConfigFolder { path: None }) {
            // Pin the managed folder so later changes to the default location
            // never move an existing login.
            let managed = folder.clone();
            persist_instance(settings_tx.clone(), provider, move |instance| {
                instance.source = InstanceSource::ConfigFolder {
                    path: Some(managed),
                };
                Ok(())
            })?;
        }
        bump_credentials(settings_tx, provider)?;
        Ok(DialogOutcome {
            notice: format!(
                "{} signed in. Its CLI keeps the login fresh in its config folder.",
                instance.display_name()
            ),
            expand_card: None,
        })
    });
}

fn submit_add_instance(
    dialog: ProviderDialog,
    instances: Vec<ProviderInstance>,
    actions: ProviderDialogActions,
) {
    let inputs = dialog.inputs();
    let fail = |message: &str| actions.set_dialog.call(Some(dialog.with_error(message)));
    let Some(driver) = inputs.driver else {
        return fail("Choose a provider.");
    };
    if let Some((_, Some(reason))) = add_instance_choices(&instances)
        .into_iter()
        .find(|(choice, _)| *choice == driver)
    {
        return fail(&format!("{reason}. It is already in the list."));
    }
    let name = inputs.name.trim().to_owned();
    let name = if name.is_empty() {
        driver.display_name().to_owned()
    } else {
        name
    };
    let mut added = None;
    let result = try_persist_update_fallible(actions.settings_tx.clone(), |settings| {
        let mut instance = settings.new_instance(driver, name.clone());
        instance.badge = inputs.badge.clone();
        instance.badge_color = inputs.badge_color;
        instance.enabled = true;
        // A second instance of a CLI driver gets its own managed folder;
        // reusing another instance's login would double-count it.
        instance.normalize();
        added = Some(settings.add_instance(instance));
        Ok(())
    });
    match result {
        Ok(()) => {
            finish_dialog(
                &actions,
                DialogOutcome {
                    notice: format!("Added {name}."),
                    expand_card: None,
                },
            );
            if let (Some(provider), Some(select)) = (added, actions.select_provider.as_ref()) {
                select(provider);
            }
        }
        Err(error) => fail(&format!("Could not add the provider: {error:#}")),
    }
}

fn submit_provider_dialog(
    dialog: ProviderDialog,
    instances: Vec<ProviderInstance>,
    actions: ProviderDialogActions,
) {
    if dialog.checking {
        return;
    }
    let inputs = dialog.inputs();
    let fail = |message: &str| actions.set_dialog.call(Some(dialog.with_error(message)));
    let account = |account_id: &str| {
        find_account_instance(&instances, account_id)
            .and_then(|instance| instance.openrouter.clone())
    };

    match dialog.kind.clone() {
        ProviderDialogKind::AddInstance => submit_add_instance(dialog, instances, actions),
        ProviderDialogKind::SignIn { provider } => submit_sign_in(dialog, provider, actions),
        ProviderDialogKind::DeleteInstance { provider } => {
            let mut removed = None;
            let result = try_persist_update_fallible(actions.settings_tx.clone(), |settings| {
                removed = settings.remove_instance(provider);
                anyhow::ensure!(removed.is_some(), "This provider no longer exists.");
                Ok(())
            });
            match result {
                Ok(()) => {
                    // The instance is gone either way; leftover secrets are unused.
                    if let Some(instance) = &removed {
                        forget_instance_secrets(instance);
                    }
                    let name = removed
                        .as_ref()
                        .map(ProviderInstance::display_name)
                        .unwrap_or_default();
                    finish_dialog(
                        &actions,
                        DialogOutcome {
                            notice: format!("{name} deleted."),
                            expand_card: None,
                        },
                    );
                    if let Some(select) = actions.select_provider.as_ref() {
                        let remaining = instances
                            .iter()
                            .filter(|instance| instance.id != provider.id())
                            .cloned()
                            .collect::<Vec<_>>();
                        if let Some(next) = super::navigation::first_provider_in_order(&remaining) {
                            select(next);
                        }
                    }
                }
                Err(error) => fail(&format!("Could not delete the provider: {error:#}")),
            }
        }
        ProviderDialogKind::ManualCredential { provider } => {
            let credential = match dialog.claude_method {
                ProfileCredentialMethod::BrowserSession => inputs.key.trim(),
                ProfileCredentialMethod::OAuthToken => inputs.second_key.trim(),
            }
            .to_owned();
            if credential.is_empty() {
                return fail("Paste a credential first.");
            }
            if let Err(error) = dialog.claude_method.validate(&credential) {
                return fail(&error.to_string());
            }
            let settings_tx = actions.settings_tx.clone();
            run_dialog_work(dialog, actions, move || {
                crate::claude::verify_credential(&credential)?;
                persist_claude_manual_credential(settings_tx, provider, &credential)?;
                Ok(DialogOutcome {
                    notice: "Credential saved in Windows user storage.".into(),
                    expand_card: None,
                })
            });
        }
        ProviderDialogKind::OpenRouterApiKey { account_id, key_id } => {
            let key = inputs.key.trim().to_owned();
            let local_name = inputs.name.trim().to_owned();
            if key.is_empty() {
                return fail("Paste a key first.");
            }
            if !looks_like_openrouter_key(&key) {
                return fail(NOT_OPENROUTER_KEY);
            }
            if account(&account_id).is_none() {
                return fail("This account no longer exists.");
            }
            let settings_tx = actions.settings_tx.clone();
            run_dialog_work(dialog, actions, move || {
                crate::openrouter::verify_api_key(&key)?;
                let key_id = key_id.unwrap_or_else(OpenRouterAccount::new_api_key_id);
                let saved_key = key_id.clone();
                persist_openrouter_credentials(
                    settings_tx,
                    account_id.clone(),
                    vec![crate::openrouter::AccountSecretChange::api_key(
                        account_id,
                        key_id,
                        Some(key),
                    )],
                    move |account| {
                        if !account.api_key_ids.contains(&saved_key) {
                            account.api_key_ids.push(saved_key.clone());
                        }
                        if !local_name.is_empty() {
                            account.api_key_names.insert(saved_key, local_name);
                        }
                        Ok(())
                    },
                )?;
                Ok(DialogOutcome {
                    notice: "API key saved in Windows user storage.".into(),
                    expand_card: None,
                })
            });
        }
        ProviderDialogKind::OpenRouterManagementKey {
            account_id,
            replace,
        } => {
            let key = inputs.key.trim().to_owned();
            if key.is_empty() {
                return fail("Paste a key first.");
            }
            if !looks_like_openrouter_key(&key) {
                return fail(NOT_OPENROUTER_KEY);
            }
            if account(&account_id).is_none() {
                return fail("This account no longer exists.");
            }
            let settings_tx = actions.settings_tx.clone();
            run_dialog_work(dialog, actions, move || {
                crate::openrouter::verify_management_key(&key)?;
                persist_openrouter_credentials(
                    settings_tx,
                    account_id.clone(),
                    vec![crate::openrouter::AccountSecretChange::management(
                        account_id,
                        Some(key),
                    )],
                    |_| Ok(()),
                )?;
                Ok(DialogOutcome {
                    notice: if replace {
                        "Management key replaced.".to_owned()
                    } else {
                        "Management key added.".to_owned()
                    },
                    expand_card: None,
                })
            });
        }
        ProviderDialogKind::RenameOpenRouterApiKey { account_id, key_id } => {
            let name = inputs.name.trim().to_owned();
            if let Err(error) = persist_openrouter_account(
                actions.settings_tx.clone(),
                account_id,
                false,
                None,
                move |account| {
                    anyhow::ensure!(
                        account.api_key_ids.contains(&key_id),
                        "OpenRouter API key no longer exists"
                    );
                    if name.is_empty() {
                        account.api_key_names.remove(&key_id);
                    } else {
                        account.api_key_names.insert(key_id, name);
                    }
                    Ok(())
                },
            ) {
                return fail(&format!("Could not rename the key: {error:#}"));
            }
            finish_dialog(
                &actions,
                DialogOutcome {
                    notice: "API key renamed.".into(),
                    expand_card: None,
                },
            );
        }
        ProviderDialogKind::RemoveOpenRouterApiKey { account_id, key_id } => {
            let secret_change = crate::openrouter::AccountSecretChange::api_key(
                account_id.clone(),
                key_id.clone(),
                None,
            );
            if let Err(error) = persist_openrouter_credentials(
                actions.settings_tx.clone(),
                account_id,
                vec![secret_change],
                move |account| {
                    let before = account.api_key_ids.len();
                    account.api_key_ids.retain(|id| id != &key_id);
                    account.api_key_names.remove(&key_id);
                    anyhow::ensure!(
                        account.api_key_ids.len() != before,
                        "OpenRouter API key no longer exists"
                    );
                    Ok(())
                },
            ) {
                return fail(&format!("Could not remove the key: {error:#}"));
            }
            finish_dialog(
                &actions,
                DialogOutcome {
                    notice: "API key removed.".into(),
                    expand_card: None,
                },
            );
        }
        ProviderDialogKind::RemoveOpenRouterManagementKey { account_id } => {
            if let Err(error) = persist_openrouter_credentials(
                actions.settings_tx.clone(),
                account_id.clone(),
                vec![crate::openrouter::AccountSecretChange::management(
                    account_id, None,
                )],
                |_| Ok(()),
            ) {
                return fail(&format!("Could not remove the key: {error:#}"));
            }
            finish_dialog(
                &actions,
                DialogOutcome {
                    notice: "Management key removed.".into(),
                    expand_card: None,
                },
            );
        }
        ProviderDialogKind::OpenCodeKey { provider, .. } => {
            let key = inputs.key.trim().to_owned();
            if key.is_empty() {
                return fail("Paste a key first.");
            }
            if let Err(error) =
                persist_opencode_manual_key(actions.settings_tx.clone(), provider, Some(key))
            {
                return fail(&format!("Could not save the key: {error:#}"));
            }
            finish_dialog(
                &actions,
                DialogOutcome {
                    notice: "API key saved.".into(),
                    expand_card: None,
                },
            );
        }
        ProviderDialogKind::RemoveOpenCodeKey { provider } => {
            if let Err(error) =
                persist_opencode_manual_key(actions.settings_tx.clone(), provider, None)
            {
                return fail(&format!("Could not remove the key: {error:#}"));
            }
            finish_dialog(
                &actions,
                DialogOutcome {
                    notice: "API key removed.".into(),
                    expand_card: None,
                },
            );
        }
    }
}

fn finish_dialog(actions: &ProviderDialogActions, outcome: DialogOutcome) {
    actions.set_dialog.call(None);
    if let Some(card) = outcome.expand_card {
        let mut next = actions.expanded_cards.clone();
        if !next.contains(&card) {
            next.push(card);
        }
        actions.set_expanded_cards.call(next);
    }
    actions
        .set_status_revision
        .call(actions.status_revision.wrapping_add(1));
    show_provider_notice(actions.set_notice.clone(), outcome.notice);
}

/// Show "Checking…", run `work` off the UI thread, then close with a notice or
/// return to the dialog with the error.
fn run_dialog_work(
    dialog: ProviderDialog,
    actions: ProviderDialogActions,
    work: impl FnOnce() -> anyhow::Result<DialogOutcome> + Send + 'static,
) {
    actions.set_dialog.call(Some(dialog.with_checking()));
    thread::spawn(move || {
        let result = work();
        if dialog.login_control.cancelled() {
            actions.set_dialog.call(None);
            return;
        }
        match result {
            Ok(outcome) => finish_dialog(&actions, outcome),
            Err(error) => {
                eprintln!("provider credential dialog failed: {error:#}");
                actions
                    .set_dialog
                    .call(Some(dialog.with_error(format!("{error:#}"))));
            }
        }
    });
}

fn looks_like_openrouter_key(value: &str) -> bool {
    value.starts_with("sk-or-")
}

#[cfg(test)]
mod providers_snapshot_tests {
    use super::*;

    #[test]
    fn account_credit_bar_uses_reported_total_not_tracked_key_spend() {
        let mut account = OpenRouterAccountSnapshot {
            balance_microusd: Some(25_000_000),
            total_credits_microusd: Some(100_000_000),
            ..Default::default()
        };
        assert_eq!(account_credit_total(Some(&account)), Some(100_000_000));
        account.api_keys.push(OpenRouterApiKeySnapshot {
            spending: SpendingSummary {
                used_microusd: 5_000_000,
                ..Default::default()
            },
            has_live_usage: true,
            ..Default::default()
        });
        assert_eq!(account_credit_total(Some(&account)), Some(100_000_000));
        account.total_credits_microusd = None;
        assert_eq!(account_credit_total(Some(&account)), None);
        account.total_credits_microusd = Some(0);
        assert_eq!(account_credit_total(Some(&account)), None);
    }

    #[test]
    fn cached_accounts_without_purchased_credits_still_deserialize() {
        let account: OpenRouterAccountSnapshot = serde_json::from_str(
            r#"{"id":"account","name":"Example","api_keys":[],"balance_microusd":25000000}"#,
        )
        .unwrap();
        assert_eq!(account.balance_microusd, Some(25_000_000));
        assert_eq!(account.total_credits_microusd, None);
    }

    #[test]
    fn unavailable_usage_is_not_presented_as_zero_spend() {
        let mut key = OpenRouterApiKeySnapshot::default();
        key.spending.limit_microusd = Some(50_000_000);
        assert!(available_key_spending(&key).is_none());
        key.has_live_usage = true;
        assert_eq!(available_key_spending(&key).unwrap().used_microusd, 0);
        key.spending.used_microusd = 12_400_000;
        assert_eq!(
            available_key_spending(&key).unwrap().used_microusd,
            12_400_000
        );
        key.has_live_usage = false;
        assert!(available_key_spending(&key).is_none());
        assert_eq!(key.spending.limit_microusd, Some(50_000_000));
    }
}
