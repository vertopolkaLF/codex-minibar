//! Provider dialogs: add/delete instances, sign-in, credentials and keys.
//!
//! Typed values live in the dialog's text inputs and are read only on
//! submit. Network checks and CLI logins run on the background executor;
//! the dialog shows "Checking…" until they finish, then closes with a toast
//! or returns with the error.

use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px, relative};

use super::super::kit::{self, Button, Kit};
use super::super::window::{SettingsWindow, open_url};
use super::*;
use crate::claude::ProfileCredentialMethod;
use crate::settings::BadgeColor;

const DIALOG_WIDTH: f32 = 460.0;
const FIELD_NAME: &str = "dlg-name";
const FIELD_KEY: &str = "dlg-key";
const FIELD_SECOND: &str = "dlg-second";
const FIELD_BADGE: &str = "dlg-badge";

#[derive(Clone, PartialEq)]
pub(crate) enum ProviderDialogKind {
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

/// The steps of the "Add provider" dialog.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AddStep {
    Provider,
    Details,
}

impl AddStep {
    const ALL: [Self; 2] = [Self::Provider, Self::Details];

    fn label(self) -> &'static str {
        match self {
            Self::Provider => crate::i18n::tr("provider"),
            Self::Details => crate::i18n::tr("details"),
        }
    }
}

#[derive(Clone)]
pub(crate) struct ProviderDialog {
    kind: ProviderDialogKind,
    add_step: AddStep,
    initial_name: String,
    driver: Option<ProviderKind>,
    badge_color: BadgeColor,
    error: Option<String>,
    checking: bool,
    claude_method: ProfileCredentialMethod,
    login_control: crate::claude::profile_oauth::LoginControl,
    focused: bool,
}

impl ProviderDialog {
    pub(crate) fn new(kind: ProviderDialogKind) -> Self {
        Self::with_name(kind, String::new())
    }

    pub(crate) fn with_name(kind: ProviderDialogKind, name: String) -> Self {
        Self {
            kind,
            add_step: AddStep::Provider,
            initial_name: name,
            driver: None,
            badge_color: BadgeColor::default(),
            error: None,
            checking: false,
            claude_method: ProfileCredentialMethod::default(),
            login_control: Default::default(),
            focused: false,
        }
    }

    /// The "Add provider" dialog with no driver chosen yet.
    pub(crate) fn add_instance() -> Self {
        Self::new(ProviderDialogKind::AddInstance)
    }

    pub(crate) fn login_control(&self) -> crate::claude::profile_oauth::LoginControl {
        self.login_control.clone()
    }

    fn is_sign_in(&self) -> bool {
        matches!(self.kind, ProviderDialogKind::SignIn { .. })
    }
}

fn instruction(k: &Kit, text: &str) -> AnyElement {
    kit::text(text.to_owned(), 14.0, k.theme.text).into_any_element()
}

fn link(k: &Kit, id: &'static str, label: &'static str, url: &'static str) -> AnyElement {
    div()
        .flex()
        .child(
            Button::new(id, label)
                .link()
                .with_icon("arrow-square-out")
                .on_click(kit::handler(move |(), _, _| open_url(url)))
                .render(k),
        )
        .into_any_element()
}

/// Numbered step strip; finished steps show a check and can be revisited.
fn add_stepper(k: &Kit, current: AddStep, on_select: kit::Handler<AddStep>) -> AnyElement {
    let labels = AddStep::ALL.map(AddStep::label);
    let current = AddStep::ALL
        .iter()
        .position(|step| *step == current)
        .unwrap_or(0);
    kit::stepper(
        k,
        "dlg",
        &labels,
        current,
        false,
        kit::handler(move |index: usize, window, cx| {
            if let Some(step) = AddStep::ALL.get(index) {
                on_select(*step, window, cx);
            }
        }),
    )
}

/// One selectable provider card in the "Add provider" grid.
fn driver_card(
    k: &Kit,
    driver: ProviderKind,
    selected: bool,
    blocked: Option<&str>,
    on_click: Option<kit::Handler<bool>>,
) -> AnyElement {
    let theme = &k.theme;
    let mut card = kit::ChoiceCard::new(
        format!("dlg-driver-{}", driver.display_name()),
        driver.display_name(),
    )
    .leading(
        kit::icon(
            crate::provider_registry::icon(driver),
            20.0,
            theme.brand(driver),
        )
        .into_any_element(),
    )
    .selected(selected);
    if blocked.is_some() {
        card = card.trailing(kit::chip(k, crate::i18n::tr("added")));
    } else if let Some(on_click) = on_click {
        card = card.on_click(on_click);
    }
    let card = card.render(k).when(blocked.is_some(), |el| el.opacity(0.5));
    let card = match blocked {
        Some(reason) => kit::with_tooltip(k, card, reason.to_owned()),
        None => card,
    };
    card.into_any_element()
}

fn claude_instructions(k: &Kit, method: ProfileCredentialMethod) -> Vec<AnyElement> {
    match method {
        ProfileCredentialMethod::BrowserSession => vec![
            instruction(
                k,
                crate::i18n::tr(
                    "reads-the-session-and-weekly-limits-of-a-claude-subscription-with",
                ),
            ),
            instruction(
                k,
                crate::i18n::tr(
                    "msg-1-open-claude-in-a-separate-browser-profile-or-private-window-sig",
                ),
            ),
            link(
                k,
                "claude-open",
                crate::i18n::tr("open-claude-ai"),
                "https://claude.ai",
            ),
            instruction(
                k,
                crate::i18n::tr(
                    "msg-2-in-chrome-or-edge-press-f12-open-application-storage-cookies-an",
                ),
            ),
            instruction(
                k,
                crate::i18n::tr(
                    "msg-3-find-sessionkey-copy-its-value-not-its-name-or-the-whole-cookie",
                ),
            ),
            link(
                k,
                "claude-cookies",
                crate::i18n::tr("how-to-view-cookies-in-chrome"),
                "https://developer.chrome.com/docs/devtools/application/cookies",
            ),
            instruction(
                k,
                crate::i18n::tr(
                    "a-cookie-header-containing-sessionkey-also-works-when-the-session",
                ),
            ),
        ],
        ProfileCredentialMethod::OAuthToken => vec![
            instruction(
                k,
                crate::i18n::tr(
                    "use-the-access-token-from-a-claude-code-subscription-login-miniba",
                ),
            ),
            instruction(
                k,
                crate::i18n::tr(
                    "copy-only-claudeaioauth-accesstoken-starts-with-sk-ant-oat-from-t",
                ),
            ),
            link(
                k,
                "claude-multi",
                crate::i18n::tr("claude-code-log-in-with-multiple-accounts"),
                "https://code.claude.com/docs/en/authentication#log-in-with-multiple-accounts",
            ),
            instruction(
                k,
                crate::i18n::tr(
                    "requires-a-claude-subscription-login-with-usage-access-api-keys-a",
                ),
            ),
        ],
    }
}

struct DialogOutcome {
    notice: String,
}

impl SettingsWindow {
    pub(in super::super) fn open_provider_dialog(
        &mut self,
        dialog: ProviderDialog,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.kit.menus.close(window);
        if let Some(previous) = self.provider_dialog.take() {
            previous.login_control.cancel();
        }
        self.reset_inputs("dlg-");
        self.provider_dialog = Some(dialog);
        cx.notify();
    }

    /// Shows the account's page and starts its sign-in, the same dialog its
    /// "Sign in again" button opens. A sign-in already running is kept.
    pub(crate) fn begin_sign_in(
        &mut self,
        provider: ProviderId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.instance(provider).is_none() {
            return;
        }
        self.select_provider(provider, cx);
        if self
            .provider_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.checking)
        {
            return;
        }
        self.open_provider_dialog(
            ProviderDialog::new(ProviderDialogKind::SignIn { provider }),
            window,
            cx,
        );
    }

    pub(in super::super) fn dismiss_provider_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = &self.provider_dialog else {
            return;
        };
        if dialog.checking {
            // A pending login can be cancelled; a key check cannot.
            if dialog.is_sign_in() {
                dialog.login_control.cancel();
            }
            return;
        }
        self.provider_dialog = None;
        cx.notify();
    }

    fn dialog_input(
        &mut self,
        k: &Kit,
        id: &'static str,
        label: &'static str,
        placeholder: &str,
        initial: &str,
        password: bool,
        help: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let checking = self
            .provider_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.checking);
        let input = self.input(
            id,
            initial,
            placeholder.to_owned(),
            password,
            checking,
            window,
            cx,
            Self::dialog_submit_handler(),
        );
        let field = kit::text_field(k, &input, None, window, cx);
        let mut column = div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(kit::text(label, 12.0, k.theme.text_secondary))
            .child(field);
        if let Some(help) = help {
            column = column.child(kit::caption(k, help.to_owned()));
        }
        column.into_any_element()
    }

    pub(in super::super) fn provider_dialog_overlay(
        &mut self,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (dialog, phase) = self
            .overlays
            .provider
            .track(k, self.provider_dialog.clone())?;
        let instances = self.settings.instances.clone();
        let instance_name = |provider: &ProviderId| {
            instances
                .iter()
                .find(|instance| instance.id == provider.id())
                .map(|instance| provider_label(instance, &instances))
                .unwrap_or_else(|| crate::i18n::tr("this-provider").into())
        };
        let account_name = |account_id: &str| {
            find_account_instance(&instances, account_id)
                .map(|instance| provider_label(instance, &instances))
                .unwrap_or_else(|| "this".into())
        };
        let mut fields: Vec<AnyElement> = Vec::new();
        let mut first_input: Option<&'static str> = None;
        let mut wide = false;
        let (title, primary) = match &dialog.kind {
            ProviderDialogKind::AddInstance => {
                wide = true;
                fields.push(kit::caption(
                    k,
                    crate::i18n::tr(
                        "track-another-account-or-a-provider-minibar-has-not-shown-yet",
                    ),
                ));
                let step = dialog.add_step;
                fields.push(add_stepper(
                    k,
                    step,
                    Self::h(cx, |this, step: AddStep, _, cx| this.set_add_step(step, cx)),
                ));
                let content = match step {
                    AddStep::Provider => {
                        let cards = add_instance_choices(&instances)
                            .into_iter()
                            .map(|(driver, blocked)| {
                                let on_click = (!dialog.checking).then(|| {
                                    Self::h(cx, move |this, advance: bool, _, cx| {
                                        if let Some(dialog) = this.provider_dialog.as_mut() {
                                            dialog.driver = Some(driver);
                                            dialog.error = None;
                                        }
                                        if advance {
                                            this.set_add_step(AddStep::Details, cx);
                                        }
                                        cx.notify();
                                    })
                                });
                                div().w(relative(0.5)).p(px(4.0)).child(driver_card(
                                    k,
                                    driver,
                                    dialog.driver == Some(driver),
                                    blocked.as_deref(),
                                    on_click,
                                ))
                            })
                            .collect::<Vec<_>>();
                        div()
                            .flex()
                            .flex_wrap()
                            .mx(px(-4.0))
                            .children(cards)
                            .into_any_element()
                    }
                    AddStep::Details => {
                        let placeholder = dialog
                            .driver
                            .map_or(crate::i18n::tr("e-g-work"), |driver| driver.display_name());
                        let name = self.dialog_input(
                            k,
                            FIELD_NAME,
                            crate::i18n::tr("name"),
                            placeholder,
                            &dialog.initial_name,
                            false,
                            Some(crate::i18n::tr(
                                "shown-on-its-tab-home-card-tray-and-notifications",
                            )),
                            window,
                            cx,
                        );
                        first_input = Some(FIELD_NAME);
                        let badge = self.dialog_input(
                            k,
                            FIELD_BADGE,
                            crate::i18n::tr("badge"),
                            crate::i18n::tr("auto"),
                            "",
                            false,
                            None,
                            window,
                            cx,
                        );
                        let color = kit::field(
                            k,
                            crate::i18n::tr("badge-color"),
                            badge_color_swatches(
                                k,
                                "dlg-badge-color",
                                dialog.badge_color,
                                Self::h(cx, |this, color: BadgeColor, _, cx| {
                                    if let Some(dialog) = this
                                        .provider_dialog
                                        .as_mut()
                                        .filter(|dialog| !dialog.checking)
                                    {
                                        dialog.badge_color = color;
                                    }
                                    cx.notify();
                                }),
                            ),
                        );
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(14.0))
                            .children(dialog.driver.map(|driver| {
                                driver_card(k, driver, true, None, None)
                            }))
                            .child(name)
                            .child(badge)
                            .child(color)
                            .child(kit::caption(
                                k,
                                crate::i18n::tr("up-to-three-letters-empty-uses-the-name-s-initials-badges-show-wh"),
                            ))
                            .into_any_element()
                    }
                };
                let key = match step {
                    AddStep::Provider => "provider",
                    AddStep::Details => "details",
                };
                fields.push(kit::appear(k, format!("dlg-add-{key}"), content));
                (
                    crate::i18n::tr("add-provider").to_owned(),
                    match step {
                        AddStep::Provider => crate::i18n::tr("next"),
                        AddStep::Details => crate::i18n::tr("add"),
                    },
                )
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
                fields.push(kit::caption(
                    k,
                    crate::i18n::format(
                        "minibar-stops-reading-name-and-forgets-its-saved-keys-schedules-t",
                        &[
                            ("name", name.to_string()),
                            (
                                "v0",
                                (if folder {
                                    crate::i18n::tr("its-config-folder-stays-on-disk")
                                } else {
                                    ""
                                })
                                .to_string(),
                            ),
                        ],
                    ),
                ));
                (
                    crate::i18n::format("delete-name", &[("name", name.to_string())]),
                    crate::i18n::tr("delete"),
                )
            }
            ProviderDialogKind::SignIn { provider } => {
                let name = instance_name(provider);
                fields.push(kit::caption(
                    k,
                    if dialog.checking {
                        crate::i18n::tr("finish-signing-in-in-your-browser-cancel-stops-this-login")
                            .to_owned()
                    } else if provider.kind() == ProviderKind::Claude {
                        crate::i18n::format(
                            "runs-claude-code-s-own-login-for-name-with-its-config-folder-as-c",
                            &[("name", name.to_string())],
                        )
                    } else {
                        crate::i18n::format(
                            "runs-codex-s-own-login-for-name-with-its-config-folder-as-codex-h",
                            &[("name", name.to_string())],
                        )
                    },
                ));
                (
                    crate::i18n::format("sign-in-to-name", &[("name", name.to_string())]),
                    crate::i18n::tr("sign-in"),
                )
            }
            ProviderDialogKind::ManualCredential { provider } => {
                wide = true;
                fields.push(kit::caption(
                    k,
                    crate::i18n::format("for", &[("v0", (instance_name(provider)).to_string())]),
                ));
                let method = dialog.claude_method;
                fields.push(kit::segmented(
                    k,
                    "dlg-claude-method",
                    &[
                        crate::i18n::tr("browser-session"),
                        crate::i18n::tr("oauth-token"),
                    ],
                    usize::from(method == ProfileCredentialMethod::OAuthToken),
                    dialog.checking,
                    Self::h(cx, |this, index: usize, _, cx| {
                        if let Some(dialog) = this.provider_dialog.as_mut() {
                            dialog.claude_method = if index == 1 {
                                ProfileCredentialMethod::OAuthToken
                            } else {
                                ProfileCredentialMethod::BrowserSession
                            };
                            dialog.error = None;
                            dialog.focused = false;
                        }
                        cx.notify();
                    }),
                ));
                let (id, label, placeholder) = match method {
                    ProfileCredentialMethod::BrowserSession => (
                        FIELD_KEY,
                        crate::i18n::tr("session-key"),
                        crate::i18n::tr("paste-the-sessionkey-value"),
                    ),
                    ProfileCredentialMethod::OAuthToken => (
                        FIELD_SECOND,
                        crate::i18n::tr("oauth-access-token"),
                        "sk-ant-oat…",
                    ),
                };
                fields.push(self.dialog_input(
                    k,
                    id,
                    label,
                    placeholder,
                    "",
                    true,
                    None,
                    window,
                    cx,
                ));
                first_input = Some(id);
                if let Some(error) = &dialog.error {
                    fields.push(kit::info_bar(k, kit::Severity::Critical, error.clone()));
                }
                fields.push(instruction(
                    k,
                    crate::i18n::tr(
                        "the-saved-credential-is-replaced-only-after-the-new-one-passes-th",
                    ),
                ));
                let steps = claude_instructions(k, method);
                fields.push(kit::appear(
                    k,
                    format!("claude-steps-{method:?}"),
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.0))
                        .children(steps)
                        .into_any_element(),
                ));
                (
                    crate::i18n::tr("claude-credential").to_owned(),
                    crate::i18n::tr("check-and-save"),
                )
            }
            ProviderDialogKind::OpenRouterApiKey { account_id, key_id } => {
                let replacing = key_id.as_ref().is_some_and(|key_id| {
                    crate::openrouter::api_key_is_configured(account_id, key_id)
                });
                fields.push(kit::caption(
                    k,
                    crate::i18n::format("for", &[("v0", (account_name(account_id)).to_string())]),
                ));
                if !replacing {
                    fields.push(self.dialog_input(
                        k,
                        FIELD_NAME,
                        crate::i18n::tr("key-name-optional"),
                        crate::i18n::tr("e-g-personal"),
                        &dialog.initial_name,
                        false,
                        Some(crate::i18n::tr(
                            "leave-blank-to-use-the-name-from-openrouter",
                        )),
                        window,
                        cx,
                    ));
                    first_input = Some(FIELD_NAME);
                }
                fields.push(self.dialog_input(
                    k,
                    FIELD_KEY,
                    crate::i18n::tr("api-key"),
                    "sk-or-v1-…",
                    "",
                    true,
                    Some(crate::i18n::tr(
                        "minibar-checks-the-key-with-openrouter-before-saving-it",
                    )),
                    window,
                    cx,
                ));
                first_input.get_or_insert(FIELD_KEY);
                (
                    if replacing {
                        crate::i18n::tr("replace-api-key")
                    } else {
                        crate::i18n::tr("add-api-key")
                    }
                    .to_owned(),
                    crate::i18n::tr("check-and-save"),
                )
            }
            ProviderDialogKind::OpenRouterManagementKey {
                account_id,
                replace,
            } => {
                fields.push(kit::caption(
                    k,
                    crate::i18n::format("for", &[("v0", (account_name(account_id)).to_string())]),
                ));
                fields.push(self.dialog_input(
                    k,
                    FIELD_KEY,
                    crate::i18n::tr("management-key"),
                    "sk-or-v1-…",
                    "",
                    true,
                    Some(crate::i18n::tr(
                        "create-one-under-settings-management-keys-on-openrouter-ai",
                    )),
                    window,
                    cx,
                ));
                first_input = Some(FIELD_KEY);
                (
                    if *replace {
                        crate::i18n::tr("replace-management-key")
                    } else {
                        crate::i18n::tr("add-management-key")
                    }
                    .to_owned(),
                    crate::i18n::tr("check-and-save"),
                )
            }
            ProviderDialogKind::RenameOpenRouterApiKey { .. } => {
                fields.push(self.dialog_input(
                    k,
                    FIELD_NAME,
                    crate::i18n::tr("key-name"),
                    crate::i18n::tr("e-g-personal"),
                    &dialog.initial_name,
                    false,
                    Some(crate::i18n::tr(
                        "leave-blank-to-use-the-name-from-openrouter",
                    )),
                    window,
                    cx,
                ));
                first_input = Some(FIELD_NAME);
                (
                    crate::i18n::tr("rename-key").to_owned(),
                    crate::i18n::tr("save"),
                )
            }
            ProviderDialogKind::RemoveOpenRouterApiKey { account_id, key_id } => {
                let hint = crate::openrouter::api_key_hint(account_id, key_id)
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| crate::i18n::tr("this-key").into());
                fields.push(kit::caption(
                    k,
                    crate::i18n::format(
                        "minibar-stops-tracking-hint-the-key-keeps-working-on-openrouter",
                        &[("hint", hint.to_string())],
                    ),
                ));
                (
                    crate::i18n::tr("remove-api-key").to_owned(),
                    crate::i18n::tr("remove"),
                )
            }
            ProviderDialogKind::RemoveOpenRouterManagementKey { account_id } => {
                fields.push(kit::caption(
                    k,
                    crate::i18n::format(
                        "minibar-stops-showing-credit-balance-and-usage-history-for-the-ke",
                        &[("v0", (account_name(account_id)).to_string())],
                    ),
                ));
                (
                    crate::i18n::tr("remove-management-key").to_owned(),
                    crate::i18n::tr("remove"),
                )
            }
            ProviderDialogKind::OpenCodeKey { provider, replace } => {
                fields.push(kit::caption(
                    k,
                    crate::i18n::format("for", &[("v0", (instance_name(provider)).to_string())]),
                ));
                fields.push(self.dialog_input(
                    k,
                    FIELD_KEY,
                    crate::i18n::tr("api-key"),
                    "sk-…",
                    "",
                    true,
                    Some(crate::i18n::tr(
                        "saved-in-windows-user-storage-never-in-the-settings-file",
                    )),
                    window,
                    cx,
                ));
                first_input = Some(FIELD_KEY);
                (
                    if *replace {
                        crate::i18n::tr("replace-api-key")
                    } else {
                        crate::i18n::tr("add-api-key")
                    }
                    .to_owned(),
                    crate::i18n::tr("save-key"),
                )
            }
            ProviderDialogKind::RemoveOpenCodeKey { provider } => {
                fields.push(kit::caption(
                    k,
                    crate::i18n::format(
                        "minibar-forgets-the-saved-key-of-it-keeps-working-with-opencode",
                        &[("v0", (instance_name(provider)).to_string())],
                    ),
                ));
                (
                    crate::i18n::tr("remove-api-key").to_owned(),
                    crate::i18n::tr("remove"),
                )
            }
        };
        if let Some(error) = &dialog.error
            && !matches!(dialog.kind, ProviderDialogKind::ManualCredential { .. })
        {
            fields.push(kit::info_bar(k, kit::Severity::Critical, error.clone()));
        }
        if !dialog.focused && !phase.closing() {
            if let Some(id) = first_input {
                self.focus_input(id, window, cx);
            }
            if let Some(dialog) = self.provider_dialog.as_mut() {
                dialog.focused = true;
            }
        }

        let mut body = vec![kit::dialog_title(k, title)];
        body.extend(fields);
        let destructive = matches!(
            dialog.kind,
            ProviderDialogKind::DeleteInstance { .. }
                | ProviderDialogKind::RemoveOpenRouterApiKey { .. }
                | ProviderDialogKind::RemoveOpenRouterManagementKey { .. }
                | ProviderDialogKind::RemoveOpenCodeKey { .. }
        );
        let primary_label = if dialog.checking {
            if dialog.is_sign_in() {
                crate::i18n::tr("signing-in")
            } else {
                crate::i18n::tr("checking")
            }
        } else {
            primary
        };
        let picking =
            dialog.kind == ProviderDialogKind::AddInstance && dialog.add_step == AddStep::Provider;
        let mut primary_button = Button::new("dlg-primary", primary_label)
            .full_width()
            .disabled(dialog.checking || (picking && dialog.driver.is_none()))
            .on_click(Self::h(cx, |this, (), window, cx| {
                this.submit_provider_dialog(window, cx)
            }));
        primary_button = if destructive {
            primary_button.danger()
        } else {
            primary_button.accent()
        };
        let login_pending = dialog.checking && dialog.is_sign_in();
        let detailing =
            dialog.kind == ProviderDialogKind::AddInstance && dialog.add_step == AddStep::Details;
        let cancel = Button::new(
            "dlg-cancel",
            if detailing {
                crate::i18n::tr("back")
            } else {
                crate::i18n::tr("cancel")
            },
        )
        .full_width()
        .disabled(dialog.checking && !login_pending)
        .on_click(Self::h(cx, move |this, (), _, cx| {
            if detailing {
                this.set_add_step(AddStep::Provider, cx);
            } else {
                this.dismiss_provider_dialog(cx);
            }
        }));
        let dismiss = (!dialog.checking)
            .then(|| Self::h(cx, |this, (), _, cx| this.dismiss_provider_dialog(cx)));
        Some(kit::dialog(
            k,
            "provider",
            phase,
            if wide { 560.0 } else { DIALOG_WIDTH },
            body,
            vec![cancel.render(k), primary_button.render(k)],
            dismiss,
        ))
    }

    fn set_add_step(&mut self, step: AddStep, cx: &mut Context<Self>) {
        let Some(dialog) = self.provider_dialog.as_mut() else {
            return;
        };
        if dialog.checking || (step == AddStep::Details && dialog.driver.is_none()) {
            return;
        }
        if dialog.add_step != step {
            dialog.add_step = step;
            dialog.error = None;
            // Focus the name field again when the details step mounts.
            dialog.focused = false;
        }
        cx.notify();
    }

    fn fail_dialog(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        if let Some(dialog) = self.provider_dialog.as_mut() {
            dialog.error = Some(message.into());
            dialog.checking = false;
            dialog.login_control = Default::default();
        }
        cx.notify();
    }

    fn finish_dialog(&mut self, outcome: DialogOutcome, cx: &mut Context<Self>) {
        self.provider_dialog = None;
        self.status_revision = self.status_revision.wrapping_add(1);
        self.show_notice(outcome.notice, cx);
    }

    /// Show "Checking…", run `work` off the UI thread, then close with a
    /// notice or return to the dialog with the error.
    fn run_dialog_work(
        &mut self,
        cx: &mut Context<Self>,
        work: impl FnOnce() -> anyhow::Result<DialogOutcome> + Send + 'static,
    ) {
        let Some(dialog) = self.provider_dialog.as_mut() else {
            return;
        };
        dialog.error = None;
        dialog.checking = true;
        let control = dialog.login_control.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { work() }).await;
            let _ = this.update(cx, |this, cx| {
                if control.cancelled() {
                    this.provider_dialog = None;
                    cx.notify();
                    return;
                }
                match result {
                    Ok(outcome) => this.finish_dialog(outcome, cx),
                    Err(error) => {
                        eprintln!("provider credential dialog failed: {error:#}");
                        this.fail_dialog(format!("{error:#}"), cx);
                    }
                }
            });
        })
        .detach();
    }

    pub(in super::super) fn submit_provider_dialog(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self.provider_dialog.clone() else {
            return;
        };
        if dialog.checking {
            return;
        }
        let instances = self.settings.instances.clone();
        let settings_tx = self.settings_tx();
        let name = self.input_text(FIELD_NAME, cx).trim().to_owned();
        let key = self.input_text(FIELD_KEY, cx).trim().to_owned();
        let second = self.input_text(FIELD_SECOND, cx).trim().to_owned();
        let badge = crate::instances::sanitize_badge(&self.input_text(FIELD_BADGE, cx));
        let account_exists =
            |account_id: &str| find_account_instance(&instances, account_id).is_some();

        match dialog.kind.clone() {
            ProviderDialogKind::AddInstance => {
                let Some(driver) = dialog.driver else {
                    return self.fail_dialog(crate::i18n::tr("choose-a-provider"), cx);
                };
                if dialog.add_step == AddStep::Provider {
                    return self.set_add_step(AddStep::Details, cx);
                }
                if let Some((_, Some(reason))) = add_instance_choices(&instances)
                    .into_iter()
                    .find(|(choice, _)| *choice == driver)
                {
                    return self.fail_dialog(
                        crate::i18n::format(
                            "reason-it-is-already-in-the-list",
                            &[("reason", reason.to_string())],
                        ),
                        cx,
                    );
                }
                let name = if name.is_empty() {
                    driver.display_name().to_owned()
                } else {
                    name
                };
                let badge_color = dialog.badge_color;
                let mut added = None;
                let result = try_persist_update_fallible(settings_tx, |settings| {
                    let mut instance = settings.new_instance(driver, name.clone());
                    instance.badge = badge.clone();
                    instance.badge_color = badge_color;
                    instance.enabled = true;
                    // A second instance of a CLI driver gets its own managed
                    // folder; reusing another login would double-count it.
                    instance.normalize();
                    added = Some(settings.add_instance(instance));
                    Ok(())
                });
                match result {
                    Ok(()) => {
                        self.reload_settings();
                        self.finish_dialog(
                            DialogOutcome {
                                notice: crate::i18n::format(
                                    "added-name",
                                    &[("name", name.to_string())],
                                ),
                            },
                            cx,
                        );
                        if let Some(provider) = added {
                            self.select_provider(provider, cx);
                        }
                    }
                    Err(error) => self.fail_dialog(
                        crate::i18n::format(
                            "could-not-add-the-provider-error",
                            &[("error", format!("{:#}", error))],
                        ),
                        cx,
                    ),
                }
            }
            ProviderDialogKind::SignIn { provider } => {
                let control = dialog.login_control.clone();
                self.run_dialog_work(cx, move || sign_in(provider, control, settings_tx));
            }
            ProviderDialogKind::DeleteInstance { provider } => {
                let mut removed = None;
                let result = try_persist_update_fallible(settings_tx, |settings| {
                    removed = settings.remove_instance(provider);
                    anyhow::ensure!(
                        removed.is_some(),
                        crate::i18n::tr("this-provider-no-longer-exists")
                    );
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
                        self.reload_settings();
                        self.finish_dialog(
                            DialogOutcome {
                                notice: crate::i18n::format(
                                    "name-deleted",
                                    &[("name", name.to_string())],
                                ),
                            },
                            cx,
                        );
                        let page = super::super::nav::first_provider_page(&self.settings.instances);
                        self.navigate(page, cx);
                    }
                    Err(error) => self.fail_dialog(
                        crate::i18n::format(
                            "could-not-delete-the-provider-error",
                            &[("error", format!("{:#}", error))],
                        ),
                        cx,
                    ),
                }
            }
            ProviderDialogKind::ManualCredential { provider } => {
                let credential = match dialog.claude_method {
                    ProfileCredentialMethod::BrowserSession => key,
                    ProfileCredentialMethod::OAuthToken => second,
                };
                if credential.is_empty() {
                    return self.fail_dialog(crate::i18n::tr("paste-a-credential-first"), cx);
                }
                if let Err(error) = dialog.claude_method.validate(&credential) {
                    return self.fail_dialog(error.to_string(), cx);
                }
                self.run_dialog_work(cx, move || {
                    crate::claude::verify_credential(&credential)?;
                    persist_claude_manual_credential(settings_tx, provider, &credential)?;
                    Ok(DialogOutcome {
                        notice: crate::i18n::tr("credential-saved-in-windows-user-storage").into(),
                    })
                });
            }
            ProviderDialogKind::OpenRouterApiKey { account_id, key_id } => {
                if key.is_empty() {
                    return self.fail_dialog(crate::i18n::tr("paste-a-key-first"), cx);
                }
                if !looks_like_openrouter_key(&key) {
                    return self.fail_dialog(not_openrouter_key(), cx);
                }
                if !account_exists(&account_id) {
                    return self.fail_dialog(crate::i18n::tr("this-account-no-longer-exists"), cx);
                }
                let local_name = name;
                self.run_dialog_work(cx, move || {
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
                        notice: crate::i18n::tr("api-key-saved-in-windows-user-storage").into(),
                    })
                });
            }
            ProviderDialogKind::OpenRouterManagementKey {
                account_id,
                replace,
            } => {
                if key.is_empty() {
                    return self.fail_dialog(crate::i18n::tr("paste-a-key-first"), cx);
                }
                if !looks_like_openrouter_key(&key) {
                    return self.fail_dialog(not_openrouter_key(), cx);
                }
                if !account_exists(&account_id) {
                    return self.fail_dialog(crate::i18n::tr("this-account-no-longer-exists"), cx);
                }
                self.run_dialog_work(cx, move || {
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
                            crate::i18n::tr("management-key-replaced").to_owned()
                        } else {
                            crate::i18n::tr("management-key-added").to_owned()
                        },
                    })
                });
            }
            ProviderDialogKind::RenameOpenRouterApiKey { account_id, key_id } => {
                if let Err(error) = persist_openrouter_account(
                    settings_tx,
                    account_id,
                    false,
                    None,
                    move |account| {
                        anyhow::ensure!(
                            account.api_key_ids.contains(&key_id),
                            crate::i18n::tr("openrouter-api-key-no-longer-exists")
                        );
                        if name.is_empty() {
                            account.api_key_names.remove(&key_id);
                        } else {
                            account.api_key_names.insert(key_id, name);
                        }
                        Ok(())
                    },
                ) {
                    return self.fail_dialog(
                        crate::i18n::format(
                            "could-not-rename-the-key-error",
                            &[("error", format!("{:#}", error))],
                        ),
                        cx,
                    );
                }
                self.reload_settings();
                self.finish_dialog(
                    DialogOutcome {
                        notice: crate::i18n::tr("api-key-renamed").into(),
                    },
                    cx,
                );
            }
            ProviderDialogKind::RemoveOpenRouterApiKey { account_id, key_id } => {
                let change = crate::openrouter::AccountSecretChange::api_key(
                    account_id.clone(),
                    key_id.clone(),
                    None,
                );
                if let Err(error) = persist_openrouter_credentials(
                    settings_tx,
                    account_id,
                    vec![change],
                    move |account| {
                        let before = account.api_key_ids.len();
                        account.api_key_ids.retain(|id| id != &key_id);
                        account.api_key_names.remove(&key_id);
                        anyhow::ensure!(
                            account.api_key_ids.len() != before,
                            crate::i18n::tr("openrouter-api-key-no-longer-exists")
                        );
                        Ok(())
                    },
                ) {
                    return self.fail_dialog(
                        crate::i18n::format(
                            "could-not-remove-the-key-error",
                            &[("error", format!("{:#}", error))],
                        ),
                        cx,
                    );
                }
                self.reload_settings();
                self.finish_dialog(
                    DialogOutcome {
                        notice: crate::i18n::tr("api-key-removed").into(),
                    },
                    cx,
                );
            }
            ProviderDialogKind::RemoveOpenRouterManagementKey { account_id } => {
                if let Err(error) = persist_openrouter_credentials(
                    settings_tx,
                    account_id.clone(),
                    vec![crate::openrouter::AccountSecretChange::management(
                        account_id, None,
                    )],
                    |_| Ok(()),
                ) {
                    return self.fail_dialog(
                        crate::i18n::format(
                            "could-not-remove-the-key-error",
                            &[("error", format!("{:#}", error))],
                        ),
                        cx,
                    );
                }
                self.reload_settings();
                self.finish_dialog(
                    DialogOutcome {
                        notice: crate::i18n::tr("management-key-removed").into(),
                    },
                    cx,
                );
            }
            ProviderDialogKind::OpenCodeKey { provider, .. } => {
                if key.is_empty() {
                    return self.fail_dialog(crate::i18n::tr("paste-a-key-first"), cx);
                }
                if let Err(error) = persist_opencode_manual_key(settings_tx, provider, Some(key)) {
                    return self.fail_dialog(
                        crate::i18n::format(
                            "could-not-save-the-key-error",
                            &[("error", format!("{:#}", error))],
                        ),
                        cx,
                    );
                }
                self.reload_settings();
                self.finish_dialog(
                    DialogOutcome {
                        notice: crate::i18n::tr("api-key-saved").into(),
                    },
                    cx,
                );
            }
            ProviderDialogKind::RemoveOpenCodeKey { provider } => {
                if let Err(error) = persist_opencode_manual_key(settings_tx, provider, None) {
                    return self.fail_dialog(
                        crate::i18n::format(
                            "could-not-remove-the-key-error",
                            &[("error", format!("{:#}", error))],
                        ),
                        cx,
                    );
                }
                self.reload_settings();
                self.finish_dialog(
                    DialogOutcome {
                        notice: crate::i18n::tr("api-key-removed").into(),
                    },
                    cx,
                );
            }
        }
    }

    /// Adopt the settings file after a synchronous commit, before the live
    /// sync arrives, so the page never shows the previous state.
    fn reload_settings(&mut self) {
        self.settings = super::super::persistence::load_settings_for_window();
    }
}

/// Runs the CLI's own login with the instance's config folder, then advances
/// its credential revision so the worker re-reads the folder.
fn sign_in(
    provider: ProviderId,
    control: crate::claude::profile_oauth::LoginControl,
    settings_tx: Sender<Settings>,
) -> anyhow::Result<DialogOutcome> {
    let settings = Settings::load_or_create(&Settings::default_path()?)?;
    let instance = settings
        .instance(provider)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!(crate::i18n::tr("this-provider-no-longer-exists")))?;
    anyhow::ensure!(
        !instance.uses_manual_credential(),
        crate::i18n::tr("switch-source-to-config-folder-to-sign-in")
    );
    let folder = instance
        .config_folder()
        .or_else(|| crate::instances::default_folder(instance.driver))
        .ok_or_else(|| anyhow::anyhow!(crate::i18n::tr("this-provider-has-no-config-folder")))?;
    anyhow::ensure!(
        !crate::instances::folder_conflicts(&settings.instances).contains_key(&instance.id),
        crate::i18n::tr("another-instance-already-reads-this-config-folder-choose-a-differ")
    );
    match instance.driver {
        ProviderKind::Claude => {
            crate::claude::profile_oauth::login(instance.binary_path.as_deref(), &folder, &control)?
        }
        ProviderKind::Codex => {
            crate::codex::profile_oauth::login(instance.binary_path.as_deref(), &folder, &control)?
        }
        _ => anyhow::bail!(crate::i18n::tr("this-provider-has-no-sign-in")),
    }
    control.begin_save()?;
    if !instance.is_primary()
        && matches!(instance.source, InstanceSource::ConfigFolder { path: None })
    {
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
        notice: crate::i18n::format(
            "signed-in-its-cli-keeps-the-login-fresh-in-its-config-folder",
            &[("v0", instance.display_name().to_string())],
        ),
    })
}
