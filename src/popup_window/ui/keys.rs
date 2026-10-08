//! OpenRouter key administration on the OpenRouter tab.
//!
//! Accounts with a saved management key get a Keys section under their
//! cards: every key with its spend, an editor that expands in place, inline
//! delete confirmation, and New key / one-time reveal pages that replace the
//! account's section while open. Network calls run on the background
//! executor; the plaintext of a created key lives only in [`Reveal`] and is
//! wiped when that page closes or the popup hides.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Local, Utc};
use gpui::{
    AnyElement, AppContext as _, ClickEvent, ClipboardItem, Context, Entity, Focusable as _, Hsla,
    InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, Subscription, Transformation, Window, div, prelude::FluentBuilder as _, px, radians,
};
use zeroize::Zeroizing;

use super::{
    components::{self, Severity, caption, icon, nowrap},
    fx,
    root::{PopupRoot, eid},
    theme::HslaExt,
};
use crate::{
    openrouter::admin::{self, KeyPatch, LimitReset, ManagedKey, NewKey},
    popup_window::*,
    settings::OpenRouterAccount,
    settings_window::{
        input::{InputColors, InputEvent, TextInput},
        kit,
    },
};

/// Keys listed before the "Show all" row.
const COLLAPSED_ROWS: usize = 6;
/// Upper bound of an open editor; only limits the reveal while it animates.
const EDITOR_CAP: f32 = 460.0;
/// Upper bound of one collapsed row while it grows in or collapses away.
const ROW_CAP: f32 = 96.0;
/// Horizontal travel of a page sliding in over the account's cards.
const PAGE_SLIDE: f32 = 28.0;
/// Fluent's critical button fill; readable with white text in both themes.
const CRITICAL_FILL: (u8, u8, u8) = (196, 43, 28);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct RowId {
    account: String,
    hash: String,
}

#[derive(Default)]
struct AccountKeys {
    keys: Vec<ManagedKey>,
    loaded: bool,
    loading: bool,
    /// Refetch on the next render (popup reopened, refresh, or a change).
    stale: bool,
    /// The stored list from an earlier run was already looked up.
    cache_checked: bool,
    error: Option<String>,
    generation: u64,
}

struct Editor {
    row: RowId,
    reset: LimitReset,
    byok: bool,
    saving: bool,
    toggling: bool,
    deleting: bool,
    error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Expiry {
    Hour,
    Day,
    Week,
    Month,
    Quarter,
    HalfYear,
    Year,
    Never,
}

impl Expiry {
    /// Same choices, in the same order, as OpenRouter's own key form.
    const ALL: [Self; 8] = [
        Self::Hour,
        Self::Day,
        Self::Week,
        Self::Month,
        Self::Quarter,
        Self::HalfYear,
        Self::Year,
        Self::Never,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Hour => crate::i18n::tr("openrouter-keys-expires-1-hour"),
            Self::Day => crate::i18n::tr("openrouter-keys-expires-1-day"),
            Self::Week => crate::i18n::tr("openrouter-keys-expires-7-days"),
            Self::Month => crate::i18n::tr("openrouter-keys-expires-30-days"),
            Self::Quarter => crate::i18n::tr("openrouter-keys-expires-90-days"),
            Self::HalfYear => crate::i18n::tr("openrouter-keys-expires-180-days"),
            Self::Year => crate::i18n::tr("openrouter-keys-expires-1-year"),
            Self::Never => crate::i18n::tr("openrouter-keys-expires-never"),
        }
    }

    fn at(self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let lifetime = match self {
            Self::Hour => chrono::Duration::hours(1),
            Self::Day => chrono::Duration::days(1),
            Self::Week => chrono::Duration::days(7),
            Self::Month => chrono::Duration::days(30),
            Self::Quarter => chrono::Duration::days(90),
            Self::HalfYear => chrono::Duration::days(180),
            Self::Year => chrono::Duration::days(365),
            Self::Never => return None,
        };
        Some(now + lifetime)
    }
}

struct CreateForm {
    provider: ProviderId,
    account: String,
    reset: LimitReset,
    expiry: Expiry,
    byok: bool,
    track: bool,
    saving: bool,
    error: Option<String>,
}

enum TrackState {
    Saving,
    Saved,
    Failed(String),
}

struct Reveal {
    provider: ProviderId,
    account: String,
    name: String,
    summary: String,
    secret: Zeroizing<String>,
    copied: bool,
    tracking: Option<TrackState>,
}

enum KeyPage {
    Create(CreateForm),
    Reveal(Reveal),
}

impl KeyPage {
    fn provider(&self) -> ProviderId {
        match self {
            Self::Create(form) => form.provider,
            Self::Reveal(reveal) => reveal.provider,
        }
    }

    fn tag(&self) -> &'static str {
        match self {
            Self::Create(_) => "create",
            Self::Reveal(_) => "reveal",
        }
    }
}

struct Inputs {
    limit: Entity<TextInput>,
    name: Entity<TextInput>,
    new_limit: Entity<TextInput>,
    _subscriptions: Vec<Subscription>,
}

/// Key administration state for every OpenRouter account in the popup.
#[derive(Default)]
pub(crate) struct KeyAdmin {
    accounts: HashMap<String, AccountKeys>,
    open_row: Option<RowId>,
    editor: Option<Editor>,
    confirm_delete: Option<RowId>,
    page: Option<KeyPage>,
    show_all: HashSet<String>,
    /// Deleted rows collapse before they leave the list.
    removing: HashSet<RowId>,
    inputs: Option<Inputs>,
}

impl KeyAdmin {
    /// Every list is refetched the next time it is shown.
    pub(super) fn mark_stale(&mut self) {
        for account in self.accounts.values_mut() {
            account.stale = true;
        }
    }

    /// The popup hid: forms are abandoned and a revealed key is wiped.
    pub(super) fn on_hidden(&mut self) {
        self.page = None;
        self.open_row = None;
        self.editor = None;
        self.confirm_delete = None;
        crate::popup::set_pinned(false);
    }
}

/// Dollar amount typed by the user, in micro-dollars. Empty means no limit.
pub(crate) fn parse_limit(text: &str) -> Result<Option<u64>, ()> {
    let cleaned = text
        .trim()
        .trim_start_matches('$')
        .replace([' ', '\u{a0}'], "")
        .replace(',', ".");
    if cleaned.is_empty() {
        return Ok(None);
    }
    let value = cleaned.parse::<f64>().map_err(|_| ())?;
    if !value.is_finite() || !(0.0..=1_000_000_000.0).contains(&value) {
        return Err(());
    }
    Ok(Some((value * 1_000_000.0).round() as u64))
}

/// Whether `text` can be a dollar amount being typed: digits with at most one
/// decimal separator and two decimals. Anything else never reaches the field.
fn is_amount_draft(text: &str) -> bool {
    let mut parts = text.splitn(2, ['.', ',']);
    let whole = parts.next().unwrap_or_default();
    let cents = parts.next().unwrap_or_default();
    whole.len() <= 10
        && cents.len() <= 2
        && whole
            .chars()
            .chain(cents.chars())
            .all(|c| c.is_ascii_digit())
}

/// Where a row sits in the key list. Clipping in GPUI is rectangular, so tints
/// on the outer rows must round their own corners to stay inside the card.
#[derive(Clone, Copy)]
struct RowEdge {
    first: bool,
    last: bool,
}

/// Full-size tint behind a row's content, rounded to match the card's edge.
fn edge_layer(edge: RowEdge, color: Hsla, card_radius: f32) -> Option<AnyElement> {
    (color.a > 0.001).then(|| {
        let radius = px(card_radius - 1.0);
        div()
            .absolute()
            .inset_0()
            .when(edge.first, |el| el.rounded_tl(radius).rounded_tr(radius))
            .when(edge.last, |el| el.rounded_bl(radius).rounded_br(radius))
            .bg(color)
            .into_any_element()
    })
}

/// Editable text for a stored limit: `25`, `12.5`, or empty for none.
fn limit_text(limit: Option<u64>) -> String {
    limit.map_or_else(String::new, |limit| {
        format!("{:.2}", limit as f64 / 1_000_000.0)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned()
    })
}

fn usd(microusd: u64) -> String {
    format_usd(microusd as f64 / 1_000_000.0)
}

fn reset_suffix(reset: LimitReset) -> Option<&'static str> {
    match reset {
        LimitReset::Never => None,
        LimitReset::Daily => Some(crate::i18n::tr("openrouter-keys-resets-daily")),
        LimitReset::Weekly => Some(crate::i18n::tr("openrouter-keys-resets-weekly")),
        LimitReset::Monthly => Some(crate::i18n::tr("openrouter-keys-resets-monthly")),
    }
}

/// "$3.40 of $10 · resets monthly", "No limit · expires Oct 31".
fn key_summary(key: &ManagedKey, now: DateTime<Utc>) -> String {
    let mut parts = Vec::new();
    match key.limit_microusd {
        Some(limit) => parts.push(crate::i18n::format(
            "openrouter-keys-of-limit",
            &[
                ("amount", usd(key.period_usage_microusd())),
                ("limit", usd(limit)),
            ],
        )),
        None => parts.push(crate::i18n::tr("openrouter-keys-no-limit").to_owned()),
    }
    if key.limit_microusd.is_some()
        && let Some(reset) = reset_suffix(key.limit_reset)
    {
        parts.push(reset.to_owned());
    }
    if let Some(expires) = key.expires_at {
        let date = crate::i18n::month_day(expires.with_timezone(&Local));
        parts.push(if expires <= now {
            crate::i18n::format("openrouter-keys-expired-on", &[("date", date)])
        } else {
            crate::i18n::format("openrouter-keys-expires-on", &[("date", date)])
        });
    }
    parts.join(" · ")
}

/// Used share of the limit in percent, if the key has one.
fn used_percent(key: &ManagedKey) -> Option<f32> {
    let limit = key.limit_microusd.filter(|limit| *limit > 0)?;
    Some((key.period_usage_microusd() as f64 / limit as f64 * 100.0).clamp(0.0, 100.0) as f32)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tone {
    Accent,
    Standard,
    Subtle,
    Danger,
    Critical,
}

impl PopupRoot {
    // ----- data ---------------------------------------------------------------

    fn openrouter_account(&self, provider: ProviderId) -> Option<OpenRouterAccount> {
        self.ui
            .instances
            .iter()
            .find(|instance| instance.provider_id() == provider)
            .and_then(|instance| instance.openrouter.clone())
    }

    /// Masked fingerprints of the keys this app tracks for the account.
    fn tracked_masks(&self, provider: ProviderId, account: &str) -> Vec<String> {
        self.limits
            .get(provider)
            .openrouter_accounts
            .iter()
            .filter(|snapshot| snapshot.id == account)
            .flat_map(|snapshot| snapshot.api_keys.iter())
            .filter_map(|key| key.masked_key.clone())
            .collect()
    }

    fn ensure_keys_loaded(&mut self, account: &str, cx: &mut Context<Self>) {
        let entry = self.keys.accounts.entry(account.to_owned()).or_default();
        let current = entry.loaded || entry.error.is_some();
        if entry.loading || (current && !entry.stale) {
            return;
        }
        entry.loading = true;
        entry.stale = false;
        entry.generation = entry.generation.wrapping_add(1);
        let generation = entry.generation;
        let check_cache = !entry.loaded && !std::mem::replace(&mut entry.cache_checked, true);
        let account = account.to_owned();
        cx.spawn(async move |this, cx| {
            // Paint the list stored by an earlier run while the fresh one loads.
            if check_cache {
                let lookup = account.clone();
                let cached = cx
                    .background_executor()
                    .spawn(async move { admin::load_cached_keys(&lookup) })
                    .await;
                match cached {
                    Ok(Some(keys)) => {
                        let _ = this.update(cx, |this, cx| {
                            if let Some(entry) = this.keys.accounts.get_mut(&account)
                                && entry.generation == generation
                                && !entry.loaded
                            {
                                entry.keys = keys;
                                entry.loaded = true;
                                cx.notify();
                            }
                        });
                    }
                    Ok(None) => {}
                    Err(error) => crate::logger::info(format!(
                        "OpenRouter cached key list unavailable: {error:#}"
                    )),
                }
            }
            let lookup = account.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let management_key = admin::load_management_key(&lookup)?;
                    let keys = admin::list_keys(&management_key)?;
                    if let Err(error) = admin::save_cached_keys(&lookup, &management_key, &keys) {
                        crate::logger::info(format!(
                            "OpenRouter key list was not cached: {error:#}"
                        ));
                    }
                    Ok::<_, anyhow::Error>(keys)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let Some(entry) = this.keys.accounts.get_mut(&account) else {
                    return;
                };
                if entry.generation != generation {
                    return;
                }
                entry.loading = false;
                match result {
                    Ok(keys) => {
                        entry.keys = keys;
                        entry.loaded = true;
                        entry.error = None;
                    }
                    Err(error) => entry.error = Some(format!("{error:#}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Runs one management call off the UI thread, then `done` on it.
    fn run_key_admin<T: Send + 'static>(
        &mut self,
        account: String,
        work: impl FnOnce(&str) -> anyhow::Result<T> + Send + 'static,
        done: impl FnOnce(&mut Self, anyhow::Result<T>, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let management_key = admin::load_management_key(&account)?;
                    work(&management_key)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                done(this, result, cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// A key changed: refetch the list and the account's quota cards.
    fn after_key_change(&mut self, provider: ProviderId, account: &str) {
        if let Some(entry) = self.keys.accounts.get_mut(account) {
            entry.stale = true;
        }
        for (worker, commands) in self.state.worker_commands() {
            if worker == provider {
                let _ = commands.send(WorkerCommand::Refresh);
            }
        }
    }

    fn replace_key(&mut self, account: &str, key: ManagedKey) {
        if let Some(entry) = self.keys.accounts.get_mut(account)
            && let Some(slot) = entry.keys.iter_mut().find(|slot| slot.hash == key.hash)
        {
            *slot = key;
        }
    }

    fn find_key(&self, row: &RowId) -> Option<ManagedKey> {
        self.keys
            .accounts
            .get(&row.account)?
            .keys
            .iter()
            .find(|key| key.hash == row.hash)
            .cloned()
    }

    fn ensure_key_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.keys.inputs.is_some() {
            return;
        }
        let amount_input = |window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| {
                let mut input = TextInput::new(window, cx);
                input.accept = Some(is_amount_draft);
                input
            })
        };
        let limit = amount_input(window, cx);
        let name = cx.new(|cx| TextInput::new(window, cx));
        let new_limit = amount_input(window, cx);
        let subscriptions = vec![
            cx.subscribe(&limit, |this, _, event: &InputEvent, cx| match event {
                InputEvent::Submit => this.save_editor(cx),
                _ => cx.notify(),
            }),
            cx.subscribe(&name, |this, _, event: &InputEvent, cx| match event {
                InputEvent::Submit => this.submit_create(cx),
                _ => cx.notify(),
            }),
            cx.subscribe(&new_limit, |this, _, event: &InputEvent, cx| match event {
                InputEvent::Submit => this.submit_create(cx),
                _ => cx.notify(),
            }),
        ];
        self.keys.inputs = Some(Inputs {
            limit,
            name,
            new_limit,
            _subscriptions: subscriptions,
        });
    }

    fn input_text(
        &self,
        pick: impl Fn(&Inputs) -> &Entity<TextInput>,
        cx: &Context<Self>,
    ) -> String {
        self.keys
            .inputs
            .as_ref()
            .map(|inputs| pick(inputs).read(cx).text())
            .unwrap_or_default()
    }

    // ----- actions ------------------------------------------------------------

    fn toggle_key_row(&mut self, row: RowId, window: &mut Window, cx: &mut Context<Self>) {
        if self.keys.open_row.as_ref() == Some(&row) {
            self.keys.open_row = None;
            self.keys.editor = None;
            self.keys.confirm_delete = None;
            cx.notify();
            return;
        }
        let Some(key) = self.find_key(&row) else {
            return;
        };
        self.keys.confirm_delete = None;
        self.keys.editor = Some(Editor {
            row: row.clone(),
            reset: key.limit_reset,
            byok: key.include_byok_in_limit,
            saving: false,
            toggling: false,
            deleting: false,
            error: None,
        });
        self.keys.open_row = Some(row);
        if let Some(inputs) = &self.keys.inputs {
            let limit = inputs.limit.clone();
            limit.update(cx, |input, cx| {
                input.set_text(limit_text(key.limit_microusd), cx)
            });
            window.focus(&limit.read(cx).focus_handle(cx));
        }
        cx.notify();
    }

    fn save_editor(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.keys.editor.as_ref() else {
            return;
        };
        if editor.saving || self.keys.confirm_delete.is_some() {
            return;
        }
        let row = editor.row.clone();
        let Some(key) = self.find_key(&row) else {
            return;
        };
        let Ok(limit) = parse_limit(&self.input_text(|inputs| &inputs.limit, cx)) else {
            if let Some(editor) = self.keys.editor.as_mut() {
                editor.error = Some(crate::i18n::tr("openrouter-keys-invalid-amount").into());
            }
            cx.notify();
            return;
        };
        let patch = editor_patch(&key, limit, editor.reset, editor.byok);
        if patch.is_empty() {
            self.keys.open_row = None;
            self.keys.editor = None;
            cx.notify();
            return;
        }
        let Some(provider) = self.key_provider(&row.account) else {
            return;
        };
        if let Some(editor) = self.keys.editor.as_mut() {
            editor.saving = true;
            editor.error = None;
        }
        let hash = row.hash.clone();
        self.run_key_admin(
            row.account.clone(),
            move |management_key| admin::update_key(management_key, &hash, &patch),
            move |this, result, _| match result {
                Ok(updated) => {
                    this.replace_key(&row.account, updated);
                    if this.keys.editor.as_ref().is_some_and(|e| e.row == row) {
                        this.keys.open_row = None;
                        this.keys.editor = None;
                    }
                    this.after_key_change(provider, &row.account);
                }
                Err(error) => {
                    if let Some(editor) = this.keys.editor.as_mut().filter(|e| e.row == row) {
                        editor.saving = false;
                        editor.error = Some(format!("{error:#}"));
                    }
                }
            },
            cx,
        );
        cx.notify();
    }

    /// Enabling or disabling is reversible, so it applies at once.
    fn toggle_key_enabled(&mut self, row: RowId, cx: &mut Context<Self>) {
        let Some(key) = self.find_key(&row) else {
            return;
        };
        let Some(provider) = self.key_provider(&row.account) else {
            return;
        };
        match self.keys.editor.as_mut() {
            Some(editor) if editor.row == row && !editor.toggling => {
                editor.toggling = true;
                editor.error = None;
            }
            _ => return,
        }
        let disabled = !key.disabled;
        let hash = row.hash.clone();
        self.run_key_admin(
            row.account.clone(),
            move |management_key| {
                admin::update_key(
                    management_key,
                    &hash,
                    &KeyPatch {
                        disabled: Some(disabled),
                        ..KeyPatch::default()
                    },
                )
            },
            move |this, result, _| {
                let editor = this.keys.editor.as_mut().filter(|e| e.row == row);
                match result {
                    Ok(updated) => {
                        if let Some(editor) = editor {
                            editor.toggling = false;
                        }
                        this.replace_key(&row.account, updated);
                        this.after_key_change(provider, &row.account);
                    }
                    Err(error) => {
                        if let Some(editor) = editor {
                            editor.toggling = false;
                            editor.error = Some(format!("{error:#}"));
                        }
                    }
                }
            },
            cx,
        );
        cx.notify();
    }

    fn delete_key(&mut self, row: RowId, cx: &mut Context<Self>) {
        let Some(provider) = self.key_provider(&row.account) else {
            return;
        };
        match self.keys.editor.as_mut() {
            Some(editor) if editor.row == row && !editor.deleting => {
                editor.deleting = true;
                editor.error = None;
            }
            _ => return,
        }
        let hash = row.hash.clone();
        self.run_key_admin(
            row.account.clone(),
            move |management_key| admin::delete_key(management_key, &hash),
            move |this, result, _| match result {
                Ok(()) => {
                    if this.keys.open_row.as_ref() == Some(&row) {
                        this.keys.open_row = None;
                        this.keys.editor = None;
                        this.keys.confirm_delete = None;
                    }
                    this.keys.removing.insert(row.clone());
                    this.after_key_change(provider, &row.account);
                }
                Err(error) => {
                    if let Some(editor) = this.keys.editor.as_mut().filter(|e| e.row == row) {
                        editor.deleting = false;
                        editor.error = Some(format!("{error:#}"));
                    }
                }
            },
            cx,
        );
        cx.notify();
    }

    /// The instance whose account owns `account`.
    fn key_provider(&self, account: &str) -> Option<ProviderId> {
        self.ui
            .instances
            .iter()
            .find(|instance| {
                instance
                    .openrouter
                    .as_ref()
                    .is_some_and(|candidate| candidate.id == account)
            })
            .map(ProviderInstance::provider_id)
    }

    fn open_create_page(
        &mut self,
        provider: ProviderId,
        account: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.keys.open_row = None;
        self.keys.editor = None;
        self.keys.confirm_delete = None;
        self.fx
            .snap(fx::key(("or-key-page", "create", account.as_str())), 0.0);
        self.keys.page = Some(KeyPage::Create(CreateForm {
            provider,
            account,
            reset: LimitReset::Monthly,
            expiry: Expiry::Never,
            byok: false,
            track: true,
            saving: false,
            error: None,
        }));
        if let Some(inputs) = &self.keys.inputs {
            let (name, limit) = (inputs.name.clone(), inputs.new_limit.clone());
            name.update(cx, |input, cx| input.set_text("", cx));
            limit.update(cx, |input, cx| input.set_text("", cx));
            window.focus(&name.read(cx).focus_handle(cx));
        }
        cx.notify();
    }

    fn close_key_page(&mut self, cx: &mut Context<Self>) {
        // Dropping the reveal wipes the secret.
        self.keys.page = None;
        crate::popup::set_pinned(false);
        cx.notify();
    }

    fn submit_create(&mut self, cx: &mut Context<Self>) {
        let name = self.input_text(|inputs| &inputs.name, cx).trim().to_owned();
        let limit = parse_limit(&self.input_text(|inputs| &inputs.new_limit, cx));
        let Some(KeyPage::Create(form)) = self.keys.page.as_mut() else {
            return;
        };
        if form.saving {
            return;
        }
        if name.is_empty() {
            form.error = Some(crate::i18n::tr("openrouter-keys-name-required").into());
            cx.notify();
            return;
        }
        let Ok(limit) = limit else {
            form.error = Some(crate::i18n::tr("openrouter-keys-invalid-amount").into());
            cx.notify();
            return;
        };
        form.saving = true;
        form.error = None;
        let new = NewKey {
            name: name.clone(),
            limit_microusd: limit,
            limit_reset: if limit.is_some() {
                form.reset
            } else {
                LimitReset::Never
            },
            include_byok_in_limit: form.byok,
            expires_at: form.expiry.at(Utc::now()),
        };
        let (provider, account, track) = (form.provider, form.account.clone(), form.track);
        self.run_key_admin(
            account.clone(),
            move |management_key| admin::create_key(management_key, &new),
            move |this, result, cx| {
                let still_open = matches!(
                    &this.keys.page,
                    Some(KeyPage::Create(form)) if form.account == account && form.saving
                );
                match result {
                    Ok(created) => {
                        let summary = key_summary(&created.key, Utc::now());
                        if let Some(entry) = this.keys.accounts.get_mut(&account)
                            && !entry.keys.iter().any(|key| key.hash == created.key.hash)
                        {
                            entry.keys.insert(0, created.key.clone());
                        }
                        // The new row grows into the list behind the reveal.
                        this.fx.snap(
                            fx::key((
                                "or-key-presence",
                                account.as_str(),
                                created.key.hash.as_str(),
                            )),
                            0.0,
                        );
                        this.after_key_change(provider, &account);
                        if !still_open {
                            return;
                        }
                        this.fx
                            .snap(fx::key(("or-key-page", "reveal", account.as_str())), 0.0);
                        let tracking = track.then_some(TrackState::Saving);
                        if track {
                            this.track_created_key(
                                account.clone(),
                                name.clone(),
                                created.secret.clone(),
                                cx,
                            );
                        }
                        this.keys.page = Some(KeyPage::Reveal(Reveal {
                            provider,
                            account: account.clone(),
                            name: created.key.name.clone(),
                            summary,
                            secret: created.secret,
                            copied: false,
                            tracking,
                        }));
                        crate::popup::set_pinned(true);
                    }
                    Err(error) => {
                        if let Some(KeyPage::Create(form)) = this.keys.page.as_mut()
                            && form.account == account
                        {
                            form.saving = false;
                            form.error = Some(format!("{error:#}"));
                        }
                    }
                }
            },
            cx,
        );
        cx.notify();
    }

    /// Saves a created key as one of the account's tracked API keys.
    fn track_created_key(
        &mut self,
        account: String,
        name: String,
        secret: Zeroizing<String>,
        cx: &mut Context<Self>,
    ) {
        let settings_tx = self.state.settings_tx.clone();
        let account_for_task = account.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let key_id = OpenRouterAccount::new_api_key_id();
                    let saved = key_id.clone();
                    crate::settings_window::persist_openrouter_credentials(
                        settings_tx,
                        account_for_task.clone(),
                        vec![crate::openrouter::AccountSecretChange::api_key(
                            account_for_task,
                            key_id,
                            Some(secret.to_string()),
                        )],
                        move |account| {
                            if !account.api_key_ids.contains(&saved) {
                                account.api_key_ids.push(saved.clone());
                            }
                            if !name.is_empty() {
                                account.api_key_names.insert(saved, name);
                            }
                            Ok(())
                        },
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(KeyPage::Reveal(reveal)) = this.keys.page.as_mut()
                    && reveal.account == account
                {
                    reveal.tracking = Some(match result {
                        Ok(()) => TrackState::Saved,
                        Err(error) => TrackState::Failed(format!("{error:#}")),
                    });
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn copy_revealed_key(&mut self, cx: &mut Context<Self>) {
        if let Some(KeyPage::Reveal(reveal)) = self.keys.page.as_mut() {
            cx.write_to_clipboard(ClipboardItem::new_string(reveal.secret.to_string()));
            reveal.copied = true;
            cx.notify();
        }
    }

    /// Keeps the outside-click pin in step with the reveal page.
    pub(super) fn sync_key_pin(&self) {
        let revealing = matches!(self.keys.page, Some(KeyPage::Reveal(_)));
        if crate::popup::is_pinned() != revealing {
            crate::popup::set_pinned(revealing);
        }
    }

    // ----- small controls -----------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn key_button(
        &mut self,
        id: String,
        label: SharedString,
        glyph: Option<&'static str>,
        tone: Tone,
        disabled: bool,
        tooltip: Option<SharedString>,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let hover_id = fx::key(("or-key-button", id.as_str()));
        let hover = self.fx.toggle(
            fx::key(("or-key-button-fx", hover_id)),
            self.hovered(hover_id) && !disabled,
            fx::FASTER,
        );
        let critical_fill = super::theme::rgb8(CRITICAL_FILL);
        let (background, foreground) = match tone {
            Tone::Accent => (
                Some(palette.accent.opacity(1.0 - 0.1 * hover)),
                palette.text_on_accent,
            ),
            Tone::Critical => (
                Some(critical_fill.opacity(1.0 - 0.1 * hover)),
                gpui::white(),
            ),
            Tone::Standard => (
                Some(palette.control_fill.mix(palette.subtle_fill, hover)),
                palette.text_primary,
            ),
            Tone::Subtle => (
                (hover > 0.001).then(|| palette.subtle_fill.opacity(hover)),
                palette.text_secondary,
            ),
            Tone::Danger => (
                (hover > 0.001).then(|| palette.subtle_fill.opacity(hover)),
                palette.critical,
            ),
        };
        let mut button = div()
            .id(eid(format!("or-key-button-{id}")))
            .h(px(28.0))
            .px(px(10.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(6.0))
            .flex_none()
            .rounded(px(palette.control_radius))
            .when_some(background, |el, background| el.bg(background))
            .when(tone == Tone::Standard, |el| {
                el.border_1().border_color(palette.card_stroke)
            })
            .when(disabled, |el| el.opacity(0.45))
            .on_hover(self.hover_listener(hover_id, tooltip, cx));
        if !disabled {
            button = button.on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                on_click(this, window, cx);
            }));
        }
        if let Some(glyph) = glyph {
            button = button.child(icon(glyph, 14.0, foreground));
        }
        let text = components::body(label, foreground).whitespace_nowrap();
        button
            .child(if matches!(tone, Tone::Accent | Tone::Critical) {
                text.font_weight(gpui::FontWeight::SEMIBOLD)
            } else {
                text
            })
            .into_any_element()
    }

    /// Fluent toggle switch row; the whole row toggles.
    fn key_switch_row(
        &mut self,
        id: String,
        label: &'static str,
        on: bool,
        disabled: bool,
        on_toggle: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let knob = self
            .fx
            .toggle(fx::key(("or-key-switch", id.as_str())), on, fx::FAST);
        let hover_id = fx::key(("or-key-switch-row", id.as_str()));
        let hovered = self.hovered(hover_id) && !disabled;
        let knob_size = if hovered { 14.0 } else { 12.0 };
        let track = div()
            .relative()
            .w(px(40.0))
            .h(px(20.0))
            .flex_none()
            .rounded(px(10.0))
            .border_1()
            .border_color(palette.text_secondary.mix(palette.accent, knob))
            .bg(palette.accent.opacity(knob))
            .child(
                div()
                    .absolute()
                    .top(px((18.0 - knob_size) / 2.0))
                    .left(px(3.0 + (18.0 - knob_size) / 2.0 - 2.0 + 20.0 * knob))
                    .size(px(knob_size))
                    .rounded(px(knob_size / 2.0))
                    .bg(palette.text_secondary.mix(palette.text_on_accent, knob)),
            );
        let mut row = div()
            .id(eid(format!("or-key-switch-{id}")))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.0))
            .min_h(px(28.0))
            .when(disabled, |el| el.opacity(0.45))
            .on_hover(self.hover_listener(hover_id, None, cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(components::body(label, palette.text_secondary)),
            )
            .child(track);
        if !disabled {
            row = row.on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                on_toggle(this, window, cx);
                cx.notify();
            }));
        }
        row.into_any_element()
    }

    fn key_field(
        &self,
        label: &'static str,
        input: &Entity<TextInput>,
        prefix: Option<&'static str>,
        hint: Option<&'static str>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let colors = InputColors {
            text: palette.text_primary,
            placeholder: palette.text_tertiary,
            caret: palette.text_primary,
            selection: palette.accent.opacity(0.35),
        };
        // Re-read every frame so a language switch applies at once.
        let placeholder: SharedString = if prefix.is_some() {
            crate::i18n::tr("openrouter-keys-no-limit").into()
        } else {
            crate::i18n::tr("openrouter-keys-name-placeholder").into()
        };
        let focused = input.update(cx, |input, _| {
            input.colors = colors;
            input.placeholder = placeholder;
            input.is_focused(window)
        });
        let field = div()
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .h(px(32.0))
            .px(px(10.0))
            .rounded(px(palette.control_radius))
            .overflow_hidden()
            .bg(if focused {
                palette.subtle_fill
            } else {
                palette.control_fill
            })
            .border_1()
            .border_color(palette.card_stroke)
            .text_size(px(14.0))
            .line_height(px(20.0))
            .when_some(prefix, |el, prefix| {
                el.child(
                    div()
                        .mr(px(4.0))
                        .child(components::body(prefix, palette.text_tertiary)),
                )
            })
            .child(div().flex_1().min_w_0().child(input.clone()))
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(px(if focused { 2.0 } else { 1.0 }))
                    .bg(if focused {
                        palette.accent
                    } else {
                        palette.text_tertiary.opacity(0.6)
                    }),
            );
        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(caption(label, palette.text_secondary))
            .child(field)
            .when_some(hint, |el, hint| {
                el.child(caption(hint, palette.text_tertiary))
            })
            .into_any_element()
    }

    fn key_badge(
        &self,
        label: impl Into<SharedString>,
        fill: gpui::Hsla,
        text: gpui::Hsla,
    ) -> AnyElement {
        div()
            .flex_none()
            .px(px(6.0))
            .rounded(px(8.0))
            .bg(fill)
            .child(nowrap(caption(label, text)))
            .into_any_element()
    }

    fn reset_labels() -> Vec<SharedString> {
        LimitReset::ALL
            .iter()
            .map(|reset| SharedString::from(crate::i18n::tr(reset.label_id())))
            .collect()
    }

    // ----- Keys section ---------------------------------------------------------

    /// The Keys section for an OpenRouter instance with a management key.
    pub(super) fn render_keys_section(
        &mut self,
        provider: ProviderId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let account = self.openrouter_account(provider)?;
        let has_management_key = crate::openrouter::management_key_hint(&account.id)
            .ok()
            .flatten()
            .is_some();
        if !has_management_key {
            return None;
        }
        self.ensure_keys_loaded(&account.id, cx);
        self.ensure_key_inputs(window, cx);
        let palette = self.palette.clone();
        let (keys, loaded, loading, error) = {
            let entry = self.keys.accounts.get(&account.id)?;
            (
                entry.keys.clone(),
                entry.loaded,
                entry.loading,
                entry.error.clone(),
            )
        };
        let account_id = account.id.clone();

        // Rows whose collapse finished leave the list for good.
        let finished = self
            .keys
            .removing
            .iter()
            .filter(|row| row.account == account_id)
            .filter(|row| {
                self.fx.is_at_target(
                    fx::key(("or-key-presence", row.account.as_str(), row.hash.as_str())),
                    0.0,
                ) && !self.fx.is_at_target(
                    fx::key(("or-key-presence", row.account.as_str(), row.hash.as_str())),
                    1.0,
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        for row in finished {
            self.keys.removing.remove(&row);
            if let Some(entry) = self.keys.accounts.get_mut(&row.account) {
                entry.keys.retain(|key| key.hash != row.hash);
            }
        }

        let visible_count = keys
            .iter()
            .filter(|key| {
                !self.keys.removing.contains(&RowId {
                    account: account_id.clone(),
                    hash: key.hash.clone(),
                })
            })
            .count();
        let mut title = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .min_w_0()
            .child(nowrap(components::body_strong(
                crate::i18n::tr("openrouter-keys-title"),
                palette.text_secondary,
            )));
        if loaded {
            title = title.child(caption(format!("· {visible_count}"), palette.text_tertiary));
        }
        if loading {
            title = title.child(nowrap(caption(
                crate::i18n::tr("openrouter-keys-updating"),
                palette.text_tertiary,
            )));
        }
        let new_key = {
            let account_id = account_id.clone();
            let page = cx.entity().downgrade();
            kit::Button::new(
                format!("or-key-new-{account_id}"),
                crate::i18n::tr("openrouter-keys-new"),
            )
            .with_icon("fluent-add")
            .size(kit::ButtonSize::Small)
            .on_click(kit::handler(move |(), window, cx| {
                let _ = page.update(cx, |this, cx| {
                    this.open_create_page(provider, account_id.clone(), window, cx)
                });
            }))
            .render(&self.kit)
        };
        let header = components::split_row(title, new_key)
            .px(px(4.0))
            .mt(px(8.0));

        let mut section = div().flex().flex_col().gap(px(6.0)).child(header);
        if let Some(error) = error.as_ref() {
            let retry = {
                let account_id = account_id.clone();
                self.key_button(
                    format!("retry-{account_id}"),
                    crate::i18n::tr("openrouter-keys-retry").into(),
                    None,
                    Tone::Standard,
                    loading,
                    None,
                    move |this, _, cx| {
                        if let Some(entry) = this.keys.accounts.get_mut(&account_id) {
                            entry.stale = true;
                        }
                        this.ensure_keys_loaded(&account_id, cx);
                    },
                    cx,
                )
            };
            section = section.child(components::info_bar_with_action(
                crate::i18n::tr("openrouter-keys-load-failed"),
                crate::i18n::localize_error(error),
                Severity::Error,
                &palette,
                Some(retry),
            ));
        }
        if !loaded {
            if error.is_none() {
                section = section.child(
                    caption(
                        crate::i18n::tr("openrouter-keys-loading"),
                        palette.text_tertiary,
                    )
                    .mx(px(4.0)),
                );
            }
            return Some(section.into_any_element());
        }
        if keys.is_empty() {
            section = section.child(
                caption(
                    crate::i18n::tr("openrouter-keys-empty"),
                    palette.text_tertiary,
                )
                .mx(px(4.0)),
            );
            return Some(section.into_any_element());
        }

        let masks = self.tracked_masks(provider, &account_id);
        let show_all = self.keys.show_all.contains(&account_id);
        // An open row always stays listed, even past the collapsed count.
        let open_hash = self
            .keys
            .open_row
            .as_ref()
            .filter(|row| row.account == account_id)
            .map(|row| row.hash.clone());
        let now = Utc::now();
        let mut list = components::card(&palette)
            .flex()
            .flex_col()
            .overflow_hidden();
        let has_footer = keys.len() > COLLAPSED_ROWS;
        let listed: Vec<&ManagedKey> = keys
            .iter()
            .enumerate()
            .filter(|(index, key)| {
                show_all
                    || *index < COLLAPSED_ROWS
                    || open_hash.as_deref() == Some(key.hash.as_str())
            })
            .map(|(_, key)| key)
            .collect();
        for (shown, key) in listed.iter().enumerate() {
            let tracked = masks.iter().any(|mask| key.matches_mask(mask));
            let edge = RowEdge {
                first: shown == 0,
                last: shown + 1 == listed.len() && !has_footer,
            };
            list = list.child(self.render_key_row(
                provider,
                &account_id,
                key,
                tracked,
                edge,
                now,
                window,
                cx,
            ));
        }
        if has_footer {
            let label = if show_all {
                crate::i18n::tr("openrouter-keys-show-fewer").to_owned()
            } else {
                crate::i18n::format(
                    "openrouter-keys-show-all",
                    &[("count", keys.len().to_string())],
                )
            };
            let hover_id = fx::key(("or-key-show-all", account_id.as_str()));
            let hover = self.fx.toggle(
                fx::key(("or-key-show-all-fx", hover_id)),
                self.hovered(hover_id),
                fx::FASTER,
            );
            let account_for_click = account_id.clone();
            list = list.child(
                div()
                    .id(eid(format!("or-key-show-all-{account_id}")))
                    .relative()
                    .border_t_1()
                    .border_color(palette.divider)
                    .px(px(12.0))
                    .py(px(8.0))
                    .on_hover(self.hover_listener(hover_id, None, cx))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if !this.keys.show_all.remove(&account_for_click) {
                            this.keys.show_all.insert(account_for_click.clone());
                        }
                        cx.notify();
                    }))
                    .children(edge_layer(
                        RowEdge {
                            first: false,
                            last: true,
                        },
                        palette.subtle_fill.opacity(hover),
                        palette.card_radius,
                    ))
                    .child(div().relative().child(caption(label, palette.accent))),
            );
        }
        Some(section.child(list).into_any_element())
    }

    #[allow(clippy::too_many_arguments)]
    fn render_key_row(
        &mut self,
        provider: ProviderId,
        account_id: &str,
        key: &ManagedKey,
        tracked: bool,
        edge: RowEdge,
        now: DateTime<Utc>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let row = RowId {
            account: account_id.to_owned(),
            hash: key.hash.clone(),
        };
        let removing = self.keys.removing.contains(&row);
        let presence = self.fx.toggle(
            fx::key(("or-key-presence", account_id, key.hash.as_str())),
            !removing,
            fx::NORMAL,
        );
        let open = self.keys.open_row.as_ref() == Some(&row) && !removing;
        let reveal = self.fx.toggle(
            fx::key(("or-key-open", account_id, key.hash.as_str())),
            open,
            fx::NORMAL,
        );
        let hover_id = fx::key(("or-key-row", account_id, key.hash.as_str()));
        let hover = self.fx.toggle(
            fx::key(("or-key-row-fx", hover_id)),
            self.hovered(hover_id),
            fx::FASTER,
        );
        let expired = key.is_expired(now);
        let percent = used_percent(key);
        let warn = percent.is_some_and(|percent| percent >= 80.0) && !key.disabled;
        let full = percent.is_some_and(|percent| percent >= 100.0) && !key.disabled;

        let mut top = div().flex().flex_row().items_center().gap(px(8.0)).child(
            div()
                .flex_1()
                .min_w_0()
                .child(nowrap(components::body_strong(
                    key.name.clone(),
                    palette.text_primary,
                ))),
        );
        if tracked {
            top = top.child(self.key_badge(
                crate::i18n::tr("openrouter-keys-this-app"),
                palette.accent.opacity(0.16),
                palette.accent,
            ));
        }
        if expired {
            top = top.child(self.key_badge(
                crate::i18n::tr("expired"),
                palette.critical_background,
                palette.critical,
            ));
        } else if key.disabled {
            top = top.child(self.key_badge(
                crate::i18n::tr("disabled"),
                palette.subtle_fill,
                palette.text_secondary,
            ));
        } else if let Some(percent) = percent.filter(|_| warn) {
            let (fill, text) = if full {
                (palette.critical_background, palette.critical)
            } else {
                (palette.caution_background, palette.caution)
            };
            top = top.child(self.key_badge(format!("{}%", percent.round() as u32), fill, text));
        }
        top = top
            .child(nowrap(components::body_strong(
                usd(key.period_usage_microusd()),
                if key.disabled || expired {
                    palette.text_tertiary
                } else {
                    palette.accent
                },
            )))
            .child(
                icon("fluent-chevron-down", 14.0, palette.text_tertiary).with_transformation(
                    Transformation::rotate(radians(std::f32::consts::PI * reveal)),
                ),
            );

        let mut head = div()
            .id(eid(format!("or-key-{account_id}-{}", key.hash)))
            .relative()
            .px(px(12.0))
            .py(px(10.0))
            .on_hover(self.hover_listener(hover_id, None, cx));
        if !removing {
            let row = row.clone();
            head = head.on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.toggle_key_row(row.clone(), window, cx);
            }));
        }
        let mut content = div().relative().flex().flex_col().gap(px(4.0)).child(top);
        if let Some(percent) = percent.filter(|_| !key.disabled && !expired) {
            let shown = if self.ui.show_used_percentage {
                percent
            } else {
                100.0 - percent
            };
            let shown = self.fx.value(
                fx::key(("or-key-progress", account_id, key.hash.as_str())),
                shown,
                fx::NORMAL,
            );
            let fill = if full {
                palette.critical
            } else if warn {
                palette.caution
            } else {
                palette.accent
            };
            content = content.child(components::progress_track(shown, None, 0, fill, &palette));
        }
        let today = (!key.disabled && !expired).then(|| {
            crate::i18n::format(
                "openrouter-keys-today",
                &[("amount", usd(key.usage_daily_microusd))],
            )
        });
        content = content.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(div().flex_1().min_w_0().child(nowrap(caption(
                    key_summary(key, now),
                    palette.text_tertiary,
                ))))
                .when_some(today, |el, today| {
                    el.child(nowrap(caption(today, palette.text_tertiary)).flex_none())
                }),
        );
        head = head
            .children(edge_layer(
                edge,
                palette.subtle_fill.opacity(hover),
                palette.card_radius,
            ))
            .child(content);

        let mut element = div()
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .when(!edge.first, |el| {
                el.border_t_1().border_color(palette.divider)
            })
            .when(open, |el| {
                el.children(edge_layer(
                    edge,
                    palette.subtle_fill.opacity(0.5),
                    palette.card_radius,
                ))
            })
            .child(head);
        if reveal > 0.001 {
            let body = if self.keys.confirm_delete.as_ref() == Some(&row) {
                self.render_delete_confirm(row.clone(), key, cx)
            } else {
                self.render_key_editor(provider, row.clone(), key, tracked, window, cx)
            };
            element = element.child(
                div()
                    .overflow_hidden()
                    .when(reveal < 0.999, |el| el.max_h(px(EDITOR_CAP * reveal)))
                    .opacity(reveal)
                    .child(body),
            );
        }
        if presence < 0.999 {
            element = element
                .max_h(px(
                    (ROW_CAP + if open { EDITOR_CAP } else { 0.0 }) * presence
                ))
                .opacity(presence);
        }
        element.into_any_element()
    }

    fn render_key_editor(
        &mut self,
        provider: ProviderId,
        row: RowId,
        key: &ManagedKey,
        tracked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let _ = provider;
        let palette = self.palette.clone();
        let Some(editor) = self.keys.editor.as_ref().filter(|editor| editor.row == row) else {
            return div().into_any_element();
        };
        let (reset, byok, saving, toggling, error) = (
            editor.reset,
            editor.byok,
            editor.saving,
            editor.toggling,
            editor.error.clone(),
        );
        let Some(limit_input) = self.keys.inputs.as_ref().map(|inputs| inputs.limit.clone()) else {
            return div().into_any_element();
        };
        let typed = parse_limit(&limit_input.read(cx).text());
        let dirty = match typed {
            Ok(limit) => !editor_patch(key, limit, reset, byok).is_empty(),
            Err(()) => true,
        };
        let id = format!("{}-{}", row.account, row.hash);

        let breakdown = crate::i18n::format(
            "openrouter-keys-usage-breakdown",
            &[
                ("today", usd(key.usage_daily_microusd)),
                ("week", usd(key.usage_weekly_microusd)),
                ("month", usd(key.usage_monthly_microusd)),
                ("total", usd(key.usage_microusd)),
            ],
        );
        let limit_field = self.key_field(
            crate::i18n::tr("openrouter-keys-spending-limit"),
            &limit_input,
            Some("$"),
            None,
            window,
            cx,
        );
        let reset_control = {
            let row = row.clone();
            self.segmented_control_quiet_wide(
                fx::key(("or-key-reset", id.as_str())),
                Self::reset_labels(),
                LimitReset::ALL
                    .iter()
                    .position(|item| *item == reset)
                    .unwrap_or(0),
                move |this, index, _| {
                    if let Some(editor) = this.keys.editor.as_mut().filter(|e| e.row == row) {
                        editor.reset = LimitReset::ALL[index];
                        editor.error = None;
                    }
                },
                window,
                cx,
            )
        };
        let byok_row = {
            let row = row.clone();
            self.key_switch_row(
                format!("byok-{id}"),
                crate::i18n::tr("openrouter-keys-byok"),
                byok,
                saving,
                move |this, _, _| {
                    if let Some(editor) = this.keys.editor.as_mut().filter(|e| e.row == row) {
                        editor.byok = !editor.byok;
                    }
                },
                cx,
            )
        };
        let enabled_row = {
            let row = row.clone();
            self.key_switch_row(
                format!("enabled-{id}"),
                crate::i18n::tr("openrouter-keys-enabled"),
                !key.disabled,
                toggling,
                move |this, _, cx| this.toggle_key_enabled(row.clone(), cx),
                cx,
            )
        };
        let delete = {
            let row = row.clone();
            self.key_button(
                format!("delete-{id}"),
                crate::i18n::tr("openrouter-keys-delete").into(),
                Some("fluent-delete"),
                Tone::Danger,
                tracked || saving,
                tracked.then(|| crate::i18n::tr("openrouter-keys-delete-tracked").into()),
                move |this, _, cx| {
                    this.keys.confirm_delete = Some(row.clone());
                    if let Some(editor) = this.keys.editor.as_mut() {
                        editor.error = None;
                    }
                    cx.notify();
                },
                cx,
            )
        };
        let cancel = {
            let row = row.clone();
            self.key_button(
                format!("cancel-{id}"),
                crate::i18n::tr("cancel").into(),
                None,
                Tone::Subtle,
                saving,
                None,
                move |this, window, cx| this.toggle_key_row(row.clone(), window, cx),
                cx,
            )
        };
        let save = self.key_button(
            format!("save-{id}"),
            if saving {
                crate::i18n::tr("openrouter-keys-saving").into()
            } else {
                crate::i18n::tr("save").into()
            },
            None,
            Tone::Accent,
            !dirty || saving,
            None,
            |this, _, cx| this.save_editor(cx),
            cx,
        );
        div()
            .px(px(12.0))
            .pb(px(12.0))
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(components::rule(&palette))
            .child(caption(breakdown, palette.text_tertiary))
            .child(limit_field)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(caption(
                        crate::i18n::tr("openrouter-keys-resets"),
                        palette.text_secondary,
                    ))
                    .child(reset_control),
            )
            .child(byok_row)
            .child(enabled_row)
            .when_some(error, |el, error| {
                el.child(caption(
                    crate::i18n::localize_error(&error),
                    palette.critical,
                ))
            })
            .child(components::split_row(
                delete,
                div()
                    .flex()
                    .flex_row()
                    .gap(px(6.0))
                    .child(cancel)
                    .child(save),
            ))
            .into_any_element()
    }

    fn render_delete_confirm(
        &mut self,
        row: RowId,
        key: &ManagedKey,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let (deleting, error) = self
            .keys
            .editor
            .as_ref()
            .filter(|editor| editor.row == row)
            .map_or((false, None), |editor| {
                (editor.deleting, editor.error.clone())
            });
        let id = format!("{}-{}", row.account, row.hash);
        let keep = self.key_button(
            format!("keep-{id}"),
            crate::i18n::tr("openrouter-keys-keep").into(),
            None,
            Tone::Subtle,
            deleting,
            None,
            |this, _, cx| {
                this.keys.confirm_delete = None;
                cx.notify();
            },
            cx,
        );
        let confirm = {
            let row = row.clone();
            self.key_button(
                format!("confirm-delete-{id}"),
                if deleting {
                    crate::i18n::tr("openrouter-keys-deleting").into()
                } else {
                    crate::i18n::tr("openrouter-keys-delete-confirm").into()
                },
                None,
                Tone::Critical,
                deleting,
                None,
                move |this, _, cx| this.delete_key(row.clone(), cx),
                cx,
            )
        };
        div()
            .px(px(12.0))
            .pb(px(12.0))
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(components::rule(&palette))
            .child(components::info_bar(
                crate::i18n::format(
                    "openrouter-keys-delete-title",
                    &[("name", key.name.clone())],
                ),
                crate::i18n::tr("openrouter-keys-delete-message"),
                Severity::Error,
                &palette,
            ))
            .when_some(error, |el, error| {
                el.child(caption(
                    crate::i18n::localize_error(&error),
                    palette.critical,
                ))
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap(px(6.0))
                    .child(keep)
                    .child(confirm),
            )
            .into_any_element()
    }

    // ----- pages ----------------------------------------------------------------

    /// The New key or reveal page, when one is open for `provider`.
    pub(super) fn render_key_page(
        &mut self,
        provider: ProviderId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let page = self.keys.page.as_ref()?;
        if page.provider() != provider {
            return None;
        }
        let tag = page.tag();
        let creating = matches!(page, KeyPage::Create(_));
        let account = match page {
            KeyPage::Create(form) => form.account.clone(),
            KeyPage::Reveal(reveal) => reveal.account.clone(),
        };
        if self.openrouter_account(provider).map(|account| account.id) != Some(account.clone()) {
            return None;
        }
        self.ensure_key_inputs(window, cx);
        let entrance = self.fx.value(
            fx::key(("or-key-page", tag, account.as_str())),
            1.0,
            fx::NORMAL,
        );
        let content = if creating {
            self.render_create_page(window, cx)
        } else {
            self.render_reveal_page(cx)
        };
        Some(
            div()
                .relative()
                .left(px(PAGE_SLIDE * (1.0 - entrance)))
                .opacity(entrance)
                .child(content)
                .into_any_element(),
        )
    }

    fn page_header(
        &mut self,
        title: &'static str,
        back: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let mut header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .px(px(4.0));
        if back {
            let hover_id = fx::key("or-key-page-back");
            let hover = self.fx.toggle(
                fx::key(("or-key-page-back-fx", hover_id)),
                self.hovered(hover_id),
                fx::FASTER,
            );
            header = header.child(
                div()
                    .id("or-key-page-back")
                    .relative()
                    .size(px(28.0))
                    .ml(px(-4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(palette.control_radius))
                    .on_hover(self.hover_listener(
                        hover_id,
                        Some(crate::i18n::tr("openrouter-keys-back").into()),
                        cx,
                    ))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.close_key_page(cx);
                    }))
                    .children(components::hover_layer(&palette, hover, 4.0))
                    .child(icon(
                        "fluent-arrow-left",
                        16.0,
                        palette.chrome_icon.mix(palette.accent, hover),
                    )),
            );
        }
        header
            .child(nowrap(components::body_strong(title, palette.text_primary)))
            .into_any_element()
    }

    fn render_create_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette.clone();
        let Some(KeyPage::Create(form)) = self.keys.page.as_ref() else {
            return div().into_any_element();
        };
        let (reset, expiry, byok, track, saving, error) = (
            form.reset,
            form.expiry,
            form.byok,
            form.track,
            form.saving,
            form.error.clone(),
        );
        let Some((name_input, limit_input)) = self
            .keys
            .inputs
            .as_ref()
            .map(|inputs| (inputs.name.clone(), inputs.new_limit.clone()))
        else {
            return div().into_any_element();
        };
        let has_limit = matches!(parse_limit(&limit_input.read(cx).text()), Ok(Some(_)));
        let can_create = !name_input.read(cx).text().trim().is_empty() && !saving;

        let header = self.page_header(crate::i18n::tr("openrouter-keys-new-title"), true, cx);
        let name_field = self.key_field(
            crate::i18n::tr("openrouter-keys-name"),
            &name_input,
            None,
            None,
            window,
            cx,
        );
        let limit_field = self.key_field(
            crate::i18n::tr("openrouter-keys-spending-limit"),
            &limit_input,
            Some("$"),
            Some(crate::i18n::tr("openrouter-keys-limit-hint")),
            window,
            cx,
        );
        let reset_control = self.segmented_control_quiet_wide(
            fx::key("or-key-create-reset"),
            Self::reset_labels(),
            LimitReset::ALL
                .iter()
                .position(|item| *item == reset)
                .unwrap_or(0),
            |this, index, _| {
                if let Some(KeyPage::Create(form)) = this.keys.page.as_mut() {
                    form.reset = LimitReset::ALL[index];
                }
            },
            window,
            cx,
        );
        let page = cx.entity().downgrade();
        let expiry_control = kit::dropdown(
            &self.kit,
            "or-key-create-expiry",
            Expiry::ALL
                .iter()
                .map(|expiry| SharedString::from(expiry.label()))
                .collect(),
            Expiry::ALL.iter().position(|item| *item == expiry),
            saving,
            0.0,
            kit::handler(move |index, _, cx| {
                let _ = page.update(cx, |this, cx| {
                    if let Some(KeyPage::Create(form)) = this.keys.page.as_mut() {
                        form.expiry = Expiry::ALL[index];
                    }
                    cx.notify();
                });
            }),
        );
        let byok_row = self.key_switch_row(
            "create-byok".into(),
            crate::i18n::tr("openrouter-keys-byok"),
            byok,
            saving,
            |this, _, _| {
                if let Some(KeyPage::Create(form)) = this.keys.page.as_mut() {
                    form.byok = !form.byok;
                }
            },
            cx,
        );
        let track_row = self.key_switch_row(
            "create-track".into(),
            crate::i18n::tr("openrouter-keys-track"),
            track,
            saving,
            |this, _, _| {
                if let Some(KeyPage::Create(form)) = this.keys.page.as_mut() {
                    form.track = !form.track;
                }
            },
            cx,
        );
        let cancel = self.key_button(
            "create-cancel".into(),
            crate::i18n::tr("cancel").into(),
            None,
            Tone::Subtle,
            saving,
            None,
            |this, _, cx| this.close_key_page(cx),
            cx,
        );
        let create = self.key_button(
            "create-submit".into(),
            if saving {
                crate::i18n::tr("openrouter-keys-creating").into()
            } else {
                crate::i18n::tr("openrouter-keys-create").into()
            },
            None,
            Tone::Accent,
            !can_create,
            None,
            |this, _, cx| this.submit_create(cx),
            cx,
        );
        let label = |text: &'static str| caption(text, palette.text_secondary);
        div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(header)
            .child(
                components::card(&palette)
                    .p(px(12.0))
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(name_field)
                    .child(limit_field)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .when(!has_limit, |el| el.opacity(0.55))
                            .child(label(crate::i18n::tr("openrouter-keys-resets")))
                            .child(reset_control),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(label(crate::i18n::tr("openrouter-keys-expires")))
                            .child(expiry_control),
                    )
                    .child(byok_row)
                    .child(track_row),
            )
            .when_some(error, |el, error| {
                el.child(caption(crate::i18n::localize_error(&error), palette.critical).px(px(4.0)))
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap(px(6.0))
                    .child(cancel)
                    .child(create),
            )
            .into_any_element()
    }

    fn render_reveal_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette.clone();
        let Some(KeyPage::Reveal(reveal)) = self.keys.page.as_ref() else {
            return div().into_any_element();
        };
        let name = reveal.name.clone();
        let summary = reveal.summary.clone();
        let secret = SharedString::from(reveal.secret.to_string());
        let copied = reveal.copied;
        let tracking = match &reveal.tracking {
            None => None,
            Some(TrackState::Saving) => Some((
                crate::i18n::tr("openrouter-keys-tracking").to_owned(),
                palette.text_tertiary,
            )),
            Some(TrackState::Saved) => Some((
                crate::i18n::tr("openrouter-keys-tracked").to_owned(),
                palette.text_tertiary,
            )),
            Some(TrackState::Failed(error)) => Some((
                crate::i18n::format(
                    "openrouter-keys-track-failed",
                    &[("error", crate::i18n::localize_error(error))],
                ),
                palette.critical,
            )),
        };
        let header = self.page_header(crate::i18n::tr("openrouter-keys-created"), false, cx);
        let copy = self.key_button(
            "reveal-copy".into(),
            if copied {
                crate::i18n::tr("openrouter-keys-copied").into()
            } else {
                crate::i18n::tr("openrouter-keys-copy").into()
            },
            Some(if copied {
                "fluent-checkmark-circle"
            } else {
                "fluent-copy"
            }),
            Tone::Accent,
            false,
            None,
            |this, _, cx| this.copy_revealed_key(cx),
            cx,
        );
        let done = self.key_button(
            "reveal-done".into(),
            crate::i18n::tr("openrouter-keys-done").into(),
            None,
            if copied { Tone::Accent } else { Tone::Standard },
            false,
            None,
            |this, _, cx| this.close_key_page(cx),
            cx,
        );
        div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(header)
            .child(
                components::card(&palette)
                    .p(px(12.0))
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(components::split_row(
                        nowrap(components::body_strong(name, palette.text_primary)),
                        nowrap(caption(summary, palette.text_tertiary)),
                    ))
                    .child(
                        div()
                            .p(px(10.0))
                            .rounded(px(6.0))
                            .bg(if palette.dark {
                                gpui::black().opacity(0.25)
                            } else {
                                palette.subtle_fill
                            })
                            .border_1()
                            .border_color(palette.card_stroke)
                            .font_family("Consolas")
                            .text_size(px(12.5))
                            .line_height(px(18.0))
                            .text_color(palette.text_primary)
                            .child(secret),
                    )
                    .child(div().flex().flex_col().child(copy)),
            )
            .child(components::info_bar(
                crate::i18n::tr("openrouter-keys-once-title"),
                crate::i18n::tr("openrouter-keys-once-message"),
                Severity::Caution,
                &palette,
            ))
            .when_some(tracking, |el, (text, color)| {
                el.child(caption(text, color).px(px(4.0)))
            })
            .child(
                components::split_row(
                    caption(
                        crate::i18n::tr("openrouter-keys-pinned"),
                        palette.text_tertiary,
                    ),
                    done,
                )
                .px(px(4.0)),
            )
            .into_any_element()
    }
}

/// The fields of `key` the editor would change.
fn editor_patch(key: &ManagedKey, limit: Option<u64>, reset: LimitReset, byok: bool) -> KeyPatch {
    KeyPatch {
        limit_microusd: (limit != key.limit_microusd).then_some(limit),
        limit_reset: (reset != key.limit_reset).then_some(reset),
        include_byok_in_limit: (byok != key.include_byok_in_limit).then_some(byok),
        disabled: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typed_dollar_limits() {
        assert_eq!(parse_limit(""), Ok(None));
        assert_eq!(parse_limit("  "), Ok(None));
        assert_eq!(parse_limit("25"), Ok(Some(25_000_000)));
        assert_eq!(parse_limit("$12.50"), Ok(Some(12_500_000)));
        assert_eq!(parse_limit("12,5"), Ok(Some(12_500_000)));
        assert_eq!(parse_limit("1 000"), Ok(Some(1_000_000_000)));
        assert_eq!(parse_limit("0"), Ok(Some(0)));
        assert_eq!(parse_limit("-1"), Err(()));
        assert_eq!(parse_limit("ten"), Err(()));
        assert_eq!(parse_limit("NaN"), Err(()));
    }

    #[test]
    fn amount_fields_only_take_money() {
        for ok in ["", "0", "25", "12.5", "12,50", ".5", "7.", "1000000000"] {
            assert!(is_amount_draft(ok), "{ok:?}");
        }
        for bad in [
            "ee",
            "$5",
            "-1",
            "1 000",
            "1.2.3",
            "1.234",
            "1e5",
            "12345678901",
        ] {
            assert!(!is_amount_draft(bad), "{bad:?}");
        }
    }

    #[test]
    fn formats_limits_back_into_editable_text() {
        assert_eq!(limit_text(None), "");
        assert_eq!(limit_text(Some(25_000_000)), "25");
        assert_eq!(limit_text(Some(12_500_000)), "12.5");
        assert_eq!(limit_text(Some(12_340_000)), "12.34");
        assert_eq!(
            parse_limit(&limit_text(Some(7_250_000))),
            Ok(Some(7_250_000))
        );
    }

    #[test]
    fn editor_patch_only_carries_changes() {
        let key = ManagedKey {
            hash: "h".into(),
            name: "k".into(),
            label: None,
            disabled: false,
            limit_microusd: Some(20_000_000),
            limit_reset: LimitReset::Weekly,
            include_byok_in_limit: false,
            usage_microusd: 0,
            usage_daily_microusd: 0,
            usage_weekly_microusd: 0,
            usage_monthly_microusd: 0,
            created_at: None,
            expires_at: None,
        };
        assert!(editor_patch(&key, Some(20_000_000), LimitReset::Weekly, false).is_empty());
        let patch = editor_patch(&key, None, LimitReset::Weekly, true);
        assert_eq!(patch.limit_microusd, Some(None));
        assert_eq!(patch.limit_reset, None);
        assert_eq!(patch.include_byok_in_limit, Some(true));
    }

    #[test]
    fn expiry_presets_match_openrouter_lifetimes() {
        let now = Utc::now();
        assert_eq!(Expiry::Never.at(now), None);
        assert_eq!(Expiry::Hour.at(now), Some(now + chrono::Duration::hours(1)));
        assert_eq!(
            Expiry::Year.at(now),
            Some(now + chrono::Duration::days(365))
        );
        assert_eq!(Expiry::ALL.last(), Some(&Expiry::Never));
        assert_eq!(Expiry::Week.at(now), Some(now + chrono::Duration::days(7)));
        assert_eq!(
            Expiry::Quarter.at(now),
            Some(now + chrono::Duration::days(90))
        );
    }
}
