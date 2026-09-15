use super::persistence::{
    persist_bool, persist_update, try_persist_update, try_persist_update_fallible,
};
use super::platform::{choose_provider_folder, copy_text_to_clipboard, reveal_in_explorer};
use super::*;
use crate::limits::{OpenRouterAccountSnapshot, OpenRouterApiKeySnapshot, SpendingSummary};

static CODEX_PATH_SAVE_GEN: AtomicU64 = AtomicU64::new(0);
static CLAUDE_PATH_SAVE_GEN: AtomicU64 = AtomicU64::new(0);
static CURSOR_PATH_SAVE_GEN: AtomicU64 = AtomicU64::new(0);
static ANTIGRAVITY_PATH_SAVE_GEN: AtomicU64 = AtomicU64::new(0);
static GROK_PATH_SAVE_GEN: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, PartialEq)]
pub(super) struct ProviderInstallStatus {
    app: Option<String>,
    cli: Option<String>,
    used: Option<ProviderInstallSource>,
    app_applicable: bool,
    cli_applicable: bool,
    checking: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum ProviderInstallSource {
    App,
    Cli,
}

impl ProviderInstallStatus {
    pub(super) fn checking() -> Self {
        Self {
            app: None,
            cli: None,
            used: None,
            app_applicable: true,
            cli_applicable: true,
            checking: true,
        }
    }

    pub(super) fn checking_app() -> Self {
        Self {
            app: None,
            cli: None,
            used: None,
            app_applicable: true,
            cli_applicable: false,
            checking: true,
        }
    }

    pub(super) fn checking_cli() -> Self {
        Self {
            app: None,
            cli: None,
            used: None,
            app_applicable: false,
            cli_applicable: true,
            checking: true,
        }
    }
}

pub(super) fn provider_install_status(
    provider: ProviderKind,
    configured_folder: &str,
) -> ProviderInstallStatus {
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
        ProviderKind::OpenRouter => {
            let detected = crate::openrouter::is_installed();
            let detail = detected.then(|| "OpenRouter account credentials are configured".into());
            (detail, None, detected.then_some(ProviderInstallSource::App))
        }
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
    };
    ProviderInstallStatus {
        app,
        cli,
        used,
        app_applicable: provider != ProviderKind::Grok,
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
    AddOpenRouterAccount,
    /// `key_id: None` adds a new key slot; `Some` replaces that slot's secret.
    OpenRouterApiKey {
        account_id: String,
        key_id: Option<String>,
    },
    OpenRouterManagementKey {
        account_id: String,
        replace: bool,
    },
    RenameOpenRouterAccount {
        account_id: String,
    },
    RemoveOpenRouterApiKey {
        account_id: String,
        key_id: String,
    },
    RemoveOpenRouterManagementKey {
        account_id: String,
    },
    RemoveOpenRouterAccount {
        account_id: String,
    },
    OpenCodeKey {
        provider: ProviderKind,
        replace: bool,
    },
    RemoveOpenCodeKey {
        provider: ProviderKind,
    },
}

#[derive(Clone, Default)]
struct DialogInputs {
    name: String,
    key: String,
    second_key: String,
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
}

impl PartialEq for ProviderDialog {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.initial_name == other.initial_name
            && Arc::ptr_eq(&self.inputs, &other.inputs)
            && self.error == other.error
            && self.checking == other.checking
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
        }
    }

    fn with_error(&self, error: impl Into<String>) -> Self {
        Self {
            error: Some(error.into()),
            checking: false,
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

fn account_display_name(account: &OpenRouterAccount) -> String {
    let name = account.name.trim();
    if name.is_empty() {
        "Unnamed account".into()
    } else {
        name.to_owned()
    }
}

fn find_account<'a>(
    accounts: &'a [OpenRouterAccount],
    account_id: &str,
) -> Option<&'a OpenRouterAccount> {
    accounts.iter().find(|account| account.id == account_id)
}

fn account_card_id(account_id: &str) -> String {
    format!("openrouter-account-{account_id}")
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
        .menu_flyout(items.iter().map(|item| menu_item(*item)).collect())
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

/// Apply an OpenRouter account-list change against on-disk settings, never a
/// stale UI snapshot. Accounts are addressed by stable id so list shifts cannot
/// move keys between accounts. The open window picks the result up through the
/// live settings sync. Popup headings overlay these names onto the live quota
/// snapshot, so a rename must not bump the credentials revision.
fn persist_openrouter_accounts(
    settings_tx: Sender<Settings>,
    bump_credentials: bool,
    mutate: impl FnOnce(&mut Vec<OpenRouterAccount>) -> anyhow::Result<()> + 'static,
) -> anyhow::Result<()> {
    try_persist_update_fallible(settings_tx, move |settings| {
        // Include the synthetic legacy account when present so edits land on
        // the same identities the Settings UI is showing.
        let mut accounts = crate::openrouter::accounts_for_settings(settings);
        mutate(&mut accounts)?;
        settings.openrouter_accounts = accounts;
        if bump_credentials {
            settings.openrouter_credentials_revision =
                settings.openrouter_credentials_revision.wrapping_add(1);
        }
        Ok(())
    })
}

/// Commits protected OpenRouter secrets as one file update, then commits the
/// matching account metadata. If the settings write fails, protected storage
/// is restored to its exact previous values so neither side can be orphaned.
fn persist_openrouter_credentials(
    settings_tx: Sender<Settings>,
    changes: Vec<crate::openrouter::AccountSecretChange>,
    mutate: impl FnOnce(&mut Vec<OpenRouterAccount>) -> anyhow::Result<()> + 'static,
) -> anyhow::Result<()> {
    let rollback = crate::openrouter::apply_account_secret_changes(&changes)?;
    if let Err(error) = persist_openrouter_accounts(settings_tx, true, mutate) {
        return match rollback.restore() {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(anyhow::anyhow!(
                "could not save account metadata ({error:#}); restoring protected credentials also failed ({rollback_error:#})"
            )),
        };
    }
    Ok(())
}

fn bump_opencode_credentials(
    settings_tx: Sender<Settings>,
    provider: ProviderKind,
) -> anyhow::Result<()> {
    try_persist_update(settings_tx, move |settings| match provider {
        ProviderKind::OpenCodeZen => {
            settings.opencode_zen_credentials_revision =
                settings.opencode_zen_credentials_revision.wrapping_add(1);
        }
        ProviderKind::OpenCodeGo => {
            settings.opencode_go_credentials_revision =
                settings.opencode_go_credentials_revision.wrapping_add(1);
        }
        _ => {}
    })
}

fn persist_opencode_manual_key(
    settings_tx: Sender<Settings>,
    provider: ProviderKind,
    value: Option<String>,
) -> anyhow::Result<()> {
    let rollback = crate::opencode::snapshot_manual_key(provider)?;
    crate::opencode::save_manual_key(provider, value.as_deref())?;
    if let Err(error) = bump_opencode_credentials(settings_tx, provider) {
        return match crate::opencode::restore_manual_key(rollback) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(anyhow::anyhow!(
                "could not save credential revision ({error:#}); restoring the protected key also failed ({rollback_error:#})"
            )),
        };
    }
    Ok(())
}

fn persist_provider_folder(provider: ProviderKind, value: String, settings_tx: Sender<Settings>) {
    let Some(generation) = path_save_generation(provider) else {
        return;
    };
    let revision = generation.fetch_add(1, Ordering::Relaxed) + 1;
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        let Some(generation) = path_save_generation(provider) else {
            return;
        };
        if generation.load(Ordering::Relaxed) != revision {
            return;
        }
        let folder = (!value.trim().is_empty()).then(|| PathBuf::from(value.trim()));
        persist_update(settings_tx, move |settings| {
            assign_provider_folder(settings, provider, folder);
        });
    });
}

fn path_save_generation(provider: ProviderKind) -> Option<&'static AtomicU64> {
    Some(match provider {
        ProviderKind::Codex => &CODEX_PATH_SAVE_GEN,
        ProviderKind::Claude => &CLAUDE_PATH_SAVE_GEN,
        ProviderKind::Cursor => &CURSOR_PATH_SAVE_GEN,
        ProviderKind::Antigravity => &ANTIGRAVITY_PATH_SAVE_GEN,
        ProviderKind::Grok => &GROK_PATH_SAVE_GEN,
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo | ProviderKind::OpenRouter => {
            return None;
        }
    })
}

fn assign_provider_folder(
    settings: &mut Settings,
    provider: ProviderKind,
    folder: Option<PathBuf>,
) {
    match provider {
        ProviderKind::Codex => settings.codex_path = folder,
        ProviderKind::Claude => settings.claude_path = folder,
        ProviderKind::Cursor => settings.cursor_path = folder,
        ProviderKind::Antigravity => settings.antigravity_path = folder,
        ProviderKind::Grok => settings.grok_path = folder,
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo | ProviderKind::OpenRouter => {}
    }
}

fn pick_provider_folder(
    provider: ProviderKind,
    setter: SetState<String>,
    settings_tx: Sender<Settings>,
) {
    match choose_provider_folder() {
        Ok(Some(folder)) => {
            let value = folder.display().to_string();
            setter.call(value.clone());
            persist_provider_folder(provider, value, settings_tx);
        }
        Ok(None) => {}
        Err(error) => eprintln!(
            "failed to choose {} folder: {error:#}",
            provider.display_name()
        ),
    }
}

fn provider_folder_picker(
    provider: ProviderKind,
    path: &str,
    placeholder: &str,
    setter: SetState<String>,
    settings_tx: Sender<Settings>,
) -> Element {
    let picker_setter = setter.clone();
    let picker_tx = settings_tx.clone();
    grid((
        text_box(path)
            .placeholder_text(placeholder)
            .on_commit(move |value: String| {
                setter.call(value.clone());
                persist_provider_folder(provider, value, settings_tx.clone());
            })
            .height(32.0)
            .grid_column(0),
        Button::new("")
            .icon(Symbol::Folder)
            .width(44.0)
            .height(32.0)
            .tooltip("Choose folder")
            .on_click(move || {
                pick_provider_folder(provider, picker_setter.clone(), picker_tx.clone())
            })
            .grid_column(1),
    ))
    .columns([GridLength::Star(1.0), GridLength::Auto])
    .column_spacing(8.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn persist_provider_enabled(
    setter: SetState<bool>,
    widgets_setter: SetState<Vec<TrayWidget>>,
    settings_tx: Sender<Settings>,
    provider: ProviderKind,
    enabled: bool,
    widgets: Vec<TrayWidget>,
) {
    setter.call(enabled);
    widgets_setter.call(widgets);
    persist_update(settings_tx, move |settings| {
        settings.providers.set_enabled(provider, enabled);
    });
}

// ---------------------------------------------------------------------------
// Per-provider lookups
// ---------------------------------------------------------------------------

fn provider_enabled_state(
    provider: ProviderKind,
    ctx: &SettingsPageContext<'_>,
) -> (bool, SetState<bool>) {
    match provider {
        ProviderKind::Codex => (ctx.codex_enabled, ctx.set_codex_enabled.clone()),
        ProviderKind::Claude => (ctx.claude_enabled, ctx.set_claude_enabled.clone()),
        ProviderKind::Cursor => (ctx.cursor_enabled, ctx.set_cursor_enabled.clone()),
        ProviderKind::OpenCodeZen => (
            ctx.opencode_zen_enabled,
            ctx.set_opencode_zen_enabled.clone(),
        ),
        ProviderKind::OpenCodeGo => (ctx.opencode_go_enabled, ctx.set_opencode_go_enabled.clone()),
        ProviderKind::OpenRouter => (ctx.openrouter_enabled, ctx.set_openrouter_enabled.clone()),
        ProviderKind::Antigravity => (ctx.antigravity_enabled, ctx.set_antigravity_enabled.clone()),
        ProviderKind::Grok => (ctx.grok_enabled, ctx.set_grok_enabled.clone()),
    }
}

fn provider_install_status_for<'a>(
    provider: ProviderKind,
    ctx: &SettingsPageContext<'a>,
) -> &'a ProviderInstallStatus {
    match provider {
        ProviderKind::Codex => ctx.codex_install_status,
        ProviderKind::Claude => ctx.claude_install_status,
        ProviderKind::Cursor => ctx.cursor_install_status,
        ProviderKind::OpenCodeZen => ctx.opencode_zen_install_status,
        ProviderKind::OpenCodeGo => ctx.opencode_go_install_status,
        ProviderKind::OpenRouter => ctx.openrouter_install_status,
        ProviderKind::Antigravity => ctx.antigravity_install_status,
        ProviderKind::Grok => ctx.grok_install_status,
    }
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
    }
}

/// Display names for the app and CLI sources a provider can be read from.
fn source_labels(provider: ProviderKind) -> (&'static str, &'static str) {
    match provider {
        ProviderKind::Codex => ("Codex desktop app", "Codex CLI"),
        ProviderKind::Claude => ("Claude desktop app", "Claude Code CLI"),
        ProviderKind::Cursor => ("Cursor app", ""),
        ProviderKind::Antigravity => ("Antigravity app", "agy CLI"),
        ProviderKind::Grok => ("", "Grok CLI"),
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo | ProviderKind::OpenRouter => ("", ""),
    }
}

struct FolderConfig<'a> {
    path: &'a str,
    setter: SetState<String>,
    label: &'static str,
    description: &'static str,
    placeholder: &'static str,
}

fn folder_config<'a>(
    provider: ProviderKind,
    ctx: &SettingsPageContext<'a>,
) -> Option<FolderConfig<'a>> {
    Some(match provider {
        ProviderKind::Codex => FolderConfig {
            path: ctx.codex_path,
            setter: ctx.set_codex_path.clone(),
            label: "Custom Codex CLI folder",
            description: "Folder with codex.exe, codex.cmd, or codex.ps1. Leave empty to find it automatically.",
            placeholder: r"C:\Users\you\AppData\Roaming\npm",
        },
        ProviderKind::Claude => FolderConfig {
            path: ctx.claude_path,
            setter: ctx.set_claude_path.clone(),
            label: "Custom Claude Code CLI folder",
            description: "Folder with claude.exe, claude.cmd, or claude.ps1. Leave empty to find it automatically.",
            placeholder: r"C:\Users\you\AppData\Roaming\npm",
        },
        ProviderKind::Cursor => FolderConfig {
            path: ctx.cursor_path,
            setter: ctx.set_cursor_path.clone(),
            label: "Custom Cursor app folder",
            description: "Folder with Cursor.exe. Leave empty to find it automatically. Usage still comes from the signed-in profile.",
            placeholder: r"C:\Users\you\AppData\Local\Programs\Cursor",
        },
        ProviderKind::Antigravity => FolderConfig {
            path: ctx.antigravity_path,
            setter: ctx.set_antigravity_path.clone(),
            label: "Custom agy CLI folder",
            description: "Folder with agy.exe, agy.cmd, or agy.ps1. Leave empty to find it automatically.",
            placeholder: r"C:\Users\you\AppData\Local\agy\bin",
        },
        ProviderKind::Grok => FolderConfig {
            path: ctx.grok_path,
            setter: ctx.set_grok_path.clone(),
            label: "Custom Grok CLI folder",
            description: "Folder with grok.exe, grok.cmd, or grok.ps1. Leave empty to find it automatically.",
            placeholder: r"C:\Users\you\.grok\bin",
        },
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo | ProviderKind::OpenRouter => {
            return None;
        }
    })
}

/// One-line status shared by the page header and the sidebar dot.
fn provider_status_line(
    provider: ProviderKind,
    enabled: bool,
    status: &ProviderInstallStatus,
    ctx: &SettingsPageContext<'_>,
) -> (String, Option<ThemeRef>) {
    if !enabled {
        return ("Off".into(), None);
    }
    let readiness = provider_readiness(status);
    let dot = match readiness {
        ProviderReadiness::Checking => None,
        ProviderReadiness::Ready => Some(ThemeRef::SystemSuccess),
        ProviderReadiness::NeedsSetup => Some(ThemeRef::SystemCaution),
    };
    if provider == ProviderKind::OpenRouter {
        let accounts = ctx.openrouter_accounts;
        if accounts.is_empty() {
            return ("No accounts yet".into(), Some(ThemeRef::SystemCaution));
        }
        let keys = accounts
            .iter()
            .map(|account| account.api_key_ids.len())
            .sum::<usize>();
        return (
            format!(
                "{} · {}",
                plural(accounts.len(), "account"),
                plural(keys, "API key")
            ),
            dot,
        );
    }
    let text = match readiness {
        ProviderReadiness::Checking => "Checking…".to_owned(),
        ProviderReadiness::NeedsSetup => match provider {
            ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => {
                "Needs an API key or OpenCode sign-in".to_owned()
            }
            _ => "Not found. Set its folder under Advanced.".to_owned(),
        },
        ProviderReadiness::Ready => match provider {
            ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => {
                if crate::opencode::key_is_configured(provider) {
                    "Using a saved API key".to_owned()
                } else {
                    "Using OpenCode sign-in or local history".to_owned()
                }
            }
            _ => {
                let (app, cli) = source_labels(provider);
                match status.used {
                    Some(ProviderInstallSource::Cli) => format!("Reading {cli}"),
                    _ => format!("Reading {app}"),
                }
            }
        },
    };
    (text, dot)
}

// ---------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------

fn provider_header(
    provider: ProviderKind,
    enabled: bool,
    status_text: String,
    dot: Option<ThemeRef>,
    color_scheme: ColorScheme,
    on_toggled: impl Fn(bool) + 'static,
) -> Element {
    let icon_name = crate::provider_registry::icon(provider);
    let scheme_tag = match color_scheme {
        ColorScheme::Dark => "dark",
        ColorScheme::Light => "light",
    };
    let icon_color = if enabled {
        crate::icons::provider_brand_color(provider, color_scheme)
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
            text_block(provider.display_name())
                .font_size(28.0)
                .bold()
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
                .on_toggled(on_toggled)
                .min_width(0.0)
                .max_width(50.0)
                .width(50.0)
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

pub(super) fn provider_page_content(
    provider: ProviderKind,
    ctx: &SettingsPageContext<'_>,
) -> Element {
    let (enabled, set_enabled) = provider_enabled_state(provider, ctx);
    let status = provider_install_status_for(provider, ctx);
    let (status_text, dot) = provider_status_line(provider, enabled, status, ctx);
    let on_toggled = {
        let widgets_setter = ctx.set_tray_widgets.clone();
        let widgets = ctx.tray_widgets.to_vec();
        let settings_tx = ctx.settings_tx.clone();
        move |value: bool| {
            persist_provider_enabled(
                set_enabled.clone(),
                widgets_setter.clone(),
                settings_tx.clone(),
                provider,
                value,
                widgets.clone(),
            )
        }
    };

    let mut rows: Vec<Element> = vec![
        provider_header(
            provider,
            enabled,
            status_text,
            dot,
            ctx.color_scheme,
            on_toggled,
        )
        .with_key("provider-header"),
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
    if !enabled {
        rows.push(
            provider_card(secondary_text(format!(
                "{} is off, so it doesn't appear in the minibar or tray. Turn it on to start reading usage.",
                provider.display_name()
            )))
            .with_key("provider-off-note"),
        );
    }
    let sections = match provider {
        ProviderKind::OpenRouter => openrouter_sections(ctx),
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => {
            opencode_sections(provider, status, ctx)
        }
        _ => install_sections(provider, status, ctx),
    };
    rows.push(
        vstack(sections)
            .spacing(4.0)
            .opacity(if enabled { 1.0 } else { 0.5 })
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

fn checking_card(status: &ProviderInstallStatus) -> Element {
    let message = if status.app_applicable && status.cli_applicable {
        "Checking installed app and CLI…"
    } else if status.cli_applicable {
        "Checking CLI…"
    } else {
        "Checking installed app…"
    };
    provider_card(secondary_text(message))
}

fn install_sections(
    provider: ProviderKind,
    status: &ProviderInstallStatus,
    ctx: &SettingsPageContext<'_>,
) -> Vec<Element> {
    let caption = format!(
        "{} Minibar finds these automatically.",
        provider_description(provider)
    );
    let mut out = vec![section_header("Sources", Some(&caption), None).with_key("sources-header")];
    if status.checking {
        out.push(checking_card(status).with_key("sources-checking"));
    } else {
        let (app_label, cli_label) = source_labels(provider);
        if status.app_applicable {
            out.push(
                source_row(
                    provider,
                    "desktop",
                    app_label,
                    status.app.as_deref(),
                    status.used == Some(ProviderInstallSource::App),
                    provider == ProviderKind::Cursor,
                    ctx,
                )
                .with_key("source-app"),
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
                    true,
                    ctx,
                )
                .with_key("source-cli"),
            );
        }
    }
    if provider == ProviderKind::Codex {
        out.push(section_header("Appearance", None, None).with_key("appearance-header"));
        out.push(codex_logo_toggle(ctx).with_key("codex-replace-logo"));
    }
    if let Some(config) = folder_config(provider, ctx) {
        out.push(section_header("Advanced", None, None).with_key("advanced-header"));
        out.push(advanced_folder_expander(provider, config, ctx).with_key("advanced-folder"));
    }
    out
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

fn source_row(
    provider: ProviderKind,
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
            if can_choose_folder && let Some(config) = folder_config(provider, ctx) {
                let setter = config.setter;
                let settings_tx = ctx.settings_tx.clone();
                trailing.push(
                    Button::new("Choose folder…")
                        .on_click(move || {
                            pick_provider_folder(provider, setter.clone(), settings_tx.clone())
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
    provider: ProviderKind,
    config: FolderConfig<'_>,
    ctx: &SettingsPageContext<'_>,
) -> Element {
    let card_id = format!("provider-{}-advanced", provider.id());
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
            provider_folder_picker(
                provider,
                config.path,
                config.placeholder,
                config.setter,
                ctx.settings_tx.clone(),
            ),
        ))
        .spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
}

fn opencode_sections(
    provider: ProviderKind,
    status: &ProviderInstallStatus,
    ctx: &SettingsPageContext<'_>,
) -> Vec<Element> {
    let caption = format!(
        "{} Minibar finds these automatically.",
        provider_description(provider)
    );
    let mut out = vec![section_header("Sources", Some(&caption), None).with_key("sources-header")];
    if status.checking {
        out.push(checking_card(status).with_key("sources-checking"));
    } else {
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
        out.push(
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
        );
    }

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
                secondary_text(if status.used.is_some() {
                    "Optional. Only needed without OpenCode sign-in."
                } else {
                    "Add a key if you don't use OpenCode sign-in on this PC."
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

fn openrouter_sections(ctx: &SettingsPageContext<'_>) -> Vec<Element> {
    let set_dialog = ctx.set_provider_dialog.clone();
    let mut out = vec![
        section_header(
            "Accounts",
            Some("A management key shows credit balance and usage history. API keys show spend per key."),
            Some(
                Button::new("Add account")
                    .icon(Symbol::Add)
                    .on_click(move || {
                        open_dialog(&set_dialog, ProviderDialogKind::AddOpenRouterAccount)
                    })
                    .into(),
            ),
        )
        .with_key("openrouter-accounts-header"),
    ];
    if ctx.openrouter_accounts.is_empty() {
        let set_dialog = ctx.set_provider_dialog.clone();
        out.push(
            provider_card(
                vstack((
                    secondary_text("No OpenRouter accounts yet.")
                        .horizontal_alignment(HorizontalAlignment::Center),
                    Button::new("Add account")
                        .accent()
                        .on_click(move || {
                            open_dialog(&set_dialog, ProviderDialogKind::AddOpenRouterAccount)
                        })
                        .horizontal_alignment(HorizontalAlignment::Center),
                ))
                .spacing(10.0)
                .horizontal_alignment(HorizontalAlignment::Stretch),
            )
            .with_key("openrouter-empty"),
        );
    }
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
    for account in ctx.openrouter_accounts {
        out.push(openrouter_account_expander(account, ctx).with_key(account_card_id(&account.id)));
    }
    out
}

fn openrouter_account_expander(
    account: &OpenRouterAccount,
    ctx: &SettingsPageContext<'_>,
) -> Element {
    let snapshot = ctx
        .openrouter_snapshot
        .accounts
        .iter()
        .find(|snapshot| snapshot.id == account.id);
    let management_hint = crate::openrouter::management_key_hint(&account.id);
    let key_count = plural(account.api_key_ids.len(), "API key");
    let summary = match (
        &management_hint,
        snapshot.and_then(|snapshot| snapshot.balance_microusd),
    ) {
        (Ok(Some(_)), Some(balance)) => format!("{} credit · {key_count}", money(balance)),
        (Ok(Some(_)), None) => format!("Management key saved · {key_count}"),
        (Ok(None), _) => format!("No management key · {key_count}"),
        (Err(_), _) => format!("Could not read management key · {key_count}"),
    };
    let name = account_display_name(account);
    let initial = name
        .chars()
        .next()
        .map(|letter| letter.to_uppercase().collect::<String>())
        .unwrap_or_else(|| "?".into());
    let header = hstack((
        border(
            text_block(initial)
                .font_size(13.0)
                .semibold()
                .horizontal_alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Center),
        )
        .width(32.0)
        .height(32.0)
        .corner_radius(16.0)
        .background(ThemeRef::ControlFillSecondary)
        .vertical_alignment(VerticalAlignment::Center),
        vstack((
            text_block(name).font_size(14.0),
            text_block(summary)
                .font_size(12.0)
                .foreground(ThemeRef::SecondaryText),
        ))
        .vertical_alignment(VerticalAlignment::Center),
    ))
    .spacing(12.0);
    let card_id = account_card_id(&account.id);
    let expanded = ctx.expanded_provider_cards.contains(&card_id);
    let toggle_header = toggle_expanded_card(card_id.clone(), ctx);
    settings_content_expander(
        header.on_tapped(move || toggle_header(!expanded)),
        expanded,
        toggle_expanded_card(card_id.clone(), ctx),
        card_id,
        ctx.hovered_card_id,
        ctx.set_hovered_card_id.clone(),
        openrouter_account_body(account, snapshot, management_hint, ctx),
    )
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
            provider_row(
                Some(row_icon("key", ctx.color_scheme)),
                "Management key",
                vec![secondary_text("Credit balance and account-wide usage history").into()],
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

    let add_key_dialog = set_dialog.clone();
    let add_key_account = account_id.clone();
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

    rows.push(
        divider(ctx.color_scheme)
            .with_key("footer-divider")
            .into(),
    );
    let rename_dialog = set_dialog.clone();
    let rename_account = account.clone();
    let remove_dialog = set_dialog;
    let remove_account = account_id;
    rows.push(
        hstack((
            Button::new("Rename").subtle().on_click(move || {
                rename_dialog.call(Some(ProviderDialog::with_name(
                    ProviderDialogKind::RenameOpenRouterAccount {
                        account_id: rename_account.id.clone(),
                    },
                    rename_account.name.clone(),
                )))
            }),
            Button::new("Remove account").danger().on_click(move || {
                open_dialog(
                    &remove_dialog,
                    ProviderDialogKind::RemoveOpenRouterAccount {
                        account_id: remove_account.clone(),
                    },
                )
            }),
        ))
        .spacing(4.0)
        .margin(Thickness {
            left: 0.0,
            top: 12.0,
            right: 0.0,
            bottom: 0.0,
        })
        .horizontal_alignment(HorizontalAlignment::Right)
        .with_key("account-footer")
        .into(),
    );

    vstack(rows)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into()
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
            match key_snapshot
                .and_then(|key| key.label.clone())
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
        let menu = more_menu(
            if saved || read_error.is_some() {
                &["Replace key", "Remove key"]
            } else {
                &["Add key", "Remove key"]
            },
            ctx.color_scheme,
            move |choice: String| {
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

// ---------------------------------------------------------------------------
// Dialogs
// ---------------------------------------------------------------------------

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

fn dialog_name_box(
    dialog: &ProviderDialog,
    help: Option<&str>,
    on_submit: impl Fn() + Clone + 'static,
) -> Element {
    let mut children: Vec<Element> = vec![
        text_box(dialog.initial_name.clone())
            .header("Account name")
            .placeholder_text("e.g. Personal")
            .enabled(!dialog.checking)
            .on_text_changed(dialog_field_handler(dialog, DialogField::Name))
            .keyboard_accelerator(dialog_submit_enter(on_submit))
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .into(),
    ];
    if let Some(help) = help {
        children.push(secondary_text(help).into());
    }
    vstack(children)
        .spacing(4.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into()
}

pub(super) fn provider_dialog_overlay(
    dialog: &ProviderDialog,
    accounts: &[OpenRouterAccount],
    actions: ProviderDialogActions,
) -> Element {
    let account = |account_id: &str| find_account(accounts, account_id);
    let account_name = |account_id: &str| {
        find_account(accounts, account_id)
            .map(account_display_name)
            .unwrap_or_else(|| "this".into())
    };

    let on_submit = {
        let dialog = dialog.clone();
        let accounts = accounts.to_vec();
        let actions = actions.clone();
        move || submit_provider_dialog(dialog.clone(), accounts.clone(), actions.clone())
    };

    let mut fields: Vec<Element> = Vec::new();
    let (title, primary, _destructive) = match &dialog.kind {
        ProviderDialogKind::AddOpenRouterAccount => {
            fields.push(dialog_name_box(
                dialog,
                Some("Only used to tell accounts apart in Minibar."),
                on_submit.clone(),
            ));
            fields.push(dialog_password(
                dialog,
                DialogField::Key,
                "Management key",
                "sk-or-v1-…",
                "Recommended. Shows credit balance and usage history.",
                on_submit.clone(),
            ));
            fields.push(dialog_password(
                dialog,
                DialogField::SecondKey,
                "API key",
                "sk-or-v1-…",
                "Optional. You can add more API keys later.",
                on_submit.clone(),
            ));
            ("Add OpenRouter account".to_owned(), "Add account", false)
        }
        ProviderDialogKind::OpenRouterApiKey { account_id, key_id } => {
            let replacing = key_id
                .as_ref()
                .is_some_and(|key_id| crate::openrouter::api_key_is_configured(account_id, key_id));
            fields.push(
                secondary_text(format!("For the {} account.", account_name(account_id))).into(),
            );
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
                false,
            )
        }
        ProviderDialogKind::OpenRouterManagementKey {
            account_id,
            replace,
        } => {
            fields.push(
                secondary_text(format!("For the {} account.", account_name(account_id))).into(),
            );
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
                false,
            )
        }
        ProviderDialogKind::RenameOpenRouterAccount { .. } => {
            fields.push(dialog_name_box(dialog, None, on_submit.clone()));
            ("Rename account".to_owned(), "Rename", false)
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
            ("Remove API key?".to_owned(), "Remove", true)
        }
        ProviderDialogKind::RemoveOpenRouterManagementKey { account_id } => {
            fields.push(
                secondary_text(format!(
                    "Minibar stops showing credit balance and usage history for {}. The key keeps working on OpenRouter.",
                    account_name(account_id)
                ))
                .into(),
            );
            ("Remove management key?".to_owned(), "Remove", true)
        }
        ProviderDialogKind::RemoveOpenRouterAccount { account_id } => {
            let key_count = account(account_id).map_or(0, |account| account.api_key_ids.len());
            let management = if crate::openrouter::management_key_is_configured(account_id) {
                ", its management key"
            } else {
                ""
            };
            fields.push(
                secondary_text(format!(
                    "Minibar forgets this account{management} and {}. Nothing changes on OpenRouter.",
                    plural(key_count, "API key")
                ))
                .into(),
            );
            (
                format!("Remove {}?", account_name(account_id)),
                "Remove account",
                true,
            )
        }
        ProviderDialogKind::OpenCodeKey { provider, replace } => {
            fields.push(secondary_text(format!("For {}.", provider.display_name())).into());
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
                false,
            )
        }
        ProviderDialogKind::RemoveOpenCodeKey { provider } => {
            fields.push(
                secondary_text(format!(
                    "Minibar forgets the saved {} key. It keeps working with OpenCode.",
                    provider.display_name()
                ))
                .into(),
            );
            ("Remove API key?".to_owned(), "Remove", true)
        }
    };
    if let Some(error) = &dialog.error {
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
        let accounts = accounts.to_vec();
        let actions = actions.clone();
        let button = Button::new(if dialog.checking {
            "Checking…"
        } else {
            primary
        })
        .enabled(!dialog.checking)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .accent();
        button
            .on_click(move || {
                submit_provider_dialog(dialog.clone(), accounts.clone(), actions.clone())
            })
            .grid_column(0)
    };
    let cancel_dialog = actions.set_dialog.clone();
    let can_cancel = !dialog.checking;
    let cancel_button = Button::new("Cancel")
        .enabled(can_cancel)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .on_click(move || cancel_dialog.call(None))
        .grid_column(1);

    let card = border(
        vstack((
            border(
                vstack(body)
                    .spacing(14.0)
                    .horizontal_alignment(HorizontalAlignment::Stretch),
            )
            .padding(Thickness::uniform(24.0)),
            border(
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
            .border_brush(ThemeRef::CardStroke),
        ))
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .background(ThemeRef::SolidBackground)
    .corner_radius(8.0)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .width(DIALOG_WIDTH)
    .horizontal_alignment(HorizontalAlignment::Center)
    .vertical_alignment(VerticalAlignment::Center)
    .on_tapped(|| {});

    let dismiss = actions.set_dialog;
    let dismiss_escape = dismiss.clone();
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
            if can_cancel {
                dismiss_escape.call(None);
            }
        },
    ))
    .with_key("provider-dialog-overlay")
    .into()
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
    thread::spawn(move || match work() {
        Ok(outcome) => finish_dialog(&actions, outcome),
        Err(error) => {
            eprintln!("provider credential dialog failed: {error:#}");
            actions
                .set_dialog
                .call(Some(dialog.with_error(format!("{error:#}"))));
        }
    });
}

fn looks_like_openrouter_key(value: &str) -> bool {
    value.starts_with("sk-or-")
}

fn submit_provider_dialog(
    dialog: ProviderDialog,
    accounts: Vec<OpenRouterAccount>,
    actions: ProviderDialogActions,
) {
    if dialog.checking {
        return;
    }
    let inputs = dialog.inputs();
    let fail = |message: &str| actions.set_dialog.call(Some(dialog.with_error(message)));
    let find_account = |account_id: &str| {
        accounts
            .iter()
            .find(|account| account.id == account_id)
            .cloned()
    };

    match dialog.kind.clone() {
        ProviderDialogKind::AddOpenRouterAccount => {
            let name = inputs.name.trim().to_owned();
            let management_key = inputs.key.trim().to_owned();
            let api_key = inputs.second_key.trim().to_owned();
            if name.is_empty() {
                return fail("Give the account a name.");
            }
            if management_key.is_empty() && api_key.is_empty() {
                return fail("Add a management key, an API key, or both.");
            }
            if [&management_key, &api_key]
                .iter()
                .any(|key| !key.is_empty() && !looks_like_openrouter_key(key))
            {
                return fail(NOT_OPENROUTER_KEY);
            }
            let settings_tx = actions.settings_tx.clone();
            run_dialog_work(dialog, actions, move || {
                if !management_key.is_empty() {
                    crate::openrouter::verify_management_key(&management_key)?;
                }
                if !api_key.is_empty() {
                    crate::openrouter::verify_api_key(&api_key)?;
                }
                let mut account = OpenRouterAccount::new(name.clone());
                account.api_key_ids.clear();
                let mut secret_changes = Vec::new();
                if !management_key.is_empty() {
                    secret_changes.push(crate::openrouter::AccountSecretChange::management(
                        account.id.clone(),
                        Some(management_key),
                    ));
                }
                if !api_key.is_empty() {
                    let key_id = OpenRouterAccount::new_api_key_id();
                    secret_changes.push(crate::openrouter::AccountSecretChange::api_key(
                        account.id.clone(),
                        key_id.clone(),
                        Some(api_key),
                    ));
                    account.api_key_ids.push(key_id);
                }
                let card = account_card_id(&account.id);
                persist_openrouter_credentials(settings_tx, secret_changes, move |accounts| {
                    anyhow::ensure!(
                        !accounts.iter().any(|existing| existing.id == account.id),
                        "OpenRouter account already exists"
                    );
                    accounts.push(account);
                    Ok(())
                })?;
                Ok(DialogOutcome {
                    notice: format!("Added {name}. Keys are saved in Windows user storage."),
                    expand_card: Some(card),
                })
            });
        }
        ProviderDialogKind::OpenRouterApiKey { account_id, key_id } => {
            let key = inputs.key.trim().to_owned();
            if key.is_empty() {
                return fail("Paste a key first.");
            }
            if !looks_like_openrouter_key(&key) {
                return fail(NOT_OPENROUTER_KEY);
            }
            let Some(account) = find_account(&account_id) else {
                return fail("This account no longer exists.");
            };
            let settings_tx = actions.settings_tx.clone();
            run_dialog_work(dialog, actions, move || {
                crate::openrouter::verify_api_key(&key)?;
                let name = account_display_name(&account);
                let card = account_card_id(&account.id);
                match key_id {
                    Some(key_id) => {
                        persist_openrouter_credentials(
                            settings_tx,
                            vec![crate::openrouter::AccountSecretChange::api_key(
                                account.id.clone(),
                                key_id.clone(),
                                Some(key),
                            )],
                            move |accounts| {
                                let saved = accounts
                                    .iter()
                                    .find(|saved| saved.id == account.id)
                                    .ok_or_else(|| {
                                        anyhow::anyhow!("OpenRouter account no longer exists")
                                    })?;
                                anyhow::ensure!(
                                    saved.api_key_ids.contains(&key_id),
                                    "OpenRouter API key no longer exists"
                                );
                                Ok(())
                            },
                        )?;
                        Ok(DialogOutcome {
                            notice: format!("API key saved for {name}."),
                            expand_card: Some(card),
                        })
                    }
                    None => {
                        let key_id = OpenRouterAccount::new_api_key_id();
                        let account_id = account.id.clone();
                        persist_openrouter_credentials(
                            settings_tx,
                            vec![crate::openrouter::AccountSecretChange::api_key(
                                account_id.clone(),
                                key_id.clone(),
                                Some(key),
                            )],
                            move |accounts| {
                                let account = accounts
                                    .iter_mut()
                                    .find(|account| account.id == account_id)
                                    .ok_or_else(|| {
                                        anyhow::anyhow!("OpenRouter account no longer exists")
                                    })?;
                                account.api_key_ids.push(key_id);
                                Ok(())
                            },
                        )?;
                        Ok(DialogOutcome {
                            notice: format!("API key added to {name}."),
                            expand_card: Some(card),
                        })
                    }
                }
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
            let Some(account) = find_account(&account_id) else {
                return fail("This account no longer exists.");
            };
            let settings_tx = actions.settings_tx.clone();
            run_dialog_work(dialog, actions, move || {
                crate::openrouter::verify_management_key(&key)?;
                let saved_account_id = account.id.clone();
                persist_openrouter_credentials(
                    settings_tx,
                    vec![crate::openrouter::AccountSecretChange::management(
                        saved_account_id.clone(),
                        Some(key),
                    )],
                    move |accounts| {
                        anyhow::ensure!(
                            accounts.iter().any(|saved| saved.id == saved_account_id),
                            "OpenRouter account no longer exists"
                        );
                        Ok(())
                    },
                )?;
                Ok(DialogOutcome {
                    notice: if replace {
                        "Management key replaced.".to_owned()
                    } else {
                        format!(
                            "Management key added to {}.",
                            account_display_name(&account)
                        )
                    },
                    expand_card: Some(account_card_id(&account.id)),
                })
            });
        }
        ProviderDialogKind::RenameOpenRouterAccount { account_id } => {
            let name = inputs.name.trim().to_owned();
            if name.is_empty() {
                return fail("Give the account a name.");
            }
            if let Err(error) =
                persist_openrouter_accounts(actions.settings_tx.clone(), false, move |accounts| {
                    let account = accounts
                        .iter_mut()
                        .find(|account| account.id == account_id)
                        .ok_or_else(|| anyhow::anyhow!("OpenRouter account no longer exists"))?;
                    account.name = name;
                    Ok(())
                })
            {
                return fail(&format!("Could not rename the account: {error:#}"));
            }
            finish_dialog(
                &actions,
                DialogOutcome {
                    notice: "Account renamed.".into(),
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
                vec![secret_change],
                move |accounts| {
                    let account = accounts
                        .iter_mut()
                        .find(|account| account.id == account_id)
                        .ok_or_else(|| anyhow::anyhow!("OpenRouter account no longer exists"))?;
                    let before = account.api_key_ids.len();
                    account.api_key_ids.retain(|id| id != &key_id);
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
                vec![crate::openrouter::AccountSecretChange::management(
                    account_id.clone(),
                    None,
                )],
                move |accounts| {
                    anyhow::ensure!(
                        accounts.iter().any(|account| account.id == account_id),
                        "OpenRouter account no longer exists"
                    );
                    Ok(())
                },
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
        ProviderDialogKind::RemoveOpenRouterAccount { account_id } => {
            let Some(account) = find_account(&account_id) else {
                actions.set_dialog.call(None);
                return;
            };
            let mut secret_changes = account
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
            secret_changes.push(crate::openrouter::AccountSecretChange::management(
                account.id.clone(),
                None,
            ));
            let name = account_display_name(&account);
            if let Err(error) = persist_openrouter_credentials(
                actions.settings_tx.clone(),
                secret_changes,
                move |accounts| {
                    let before = accounts.len();
                    accounts.retain(|account| account.id != account_id);
                    anyhow::ensure!(
                        accounts.len() != before,
                        "OpenRouter account no longer exists"
                    );
                    Ok(())
                },
            ) {
                return fail(&format!("Could not remove the account: {error:#}"));
            }
            finish_dialog(
                &actions,
                DialogOutcome {
                    notice: format!("{name} removed."),
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
