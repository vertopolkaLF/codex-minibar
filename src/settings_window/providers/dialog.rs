//! Provider dialogs: add/delete instances, sign-in, credentials and keys.
//!
//! Typed values live in the dialog's text inputs and are read only on
//! submit. Network checks and CLI logins run on the background executor;
//! the dialog shows "Checking…" until they finish, then closes with a toast
//! or returns with the error.

use gpui::{
    AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};

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

#[derive(Clone)]
pub(crate) struct ProviderDialog {
    kind: ProviderDialogKind,
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

fn claude_instructions(k: &Kit, method: ProfileCredentialMethod) -> Vec<AnyElement> {
    match method {
        ProfileCredentialMethod::BrowserSession => vec![
            instruction(
                k,
                "Reads the session and weekly limits of a Claude subscription without a Claude Code login.",
            ),
            instruction(
                k,
                "1. Open Claude in a separate browser profile or private window. Sign in to the account you want to track and confirm its email in Claude's settings.",
            ),
            link(k, "claude-open", "Open claude.ai", "https://claude.ai"),
            instruction(
                k,
                "2. In Chrome or Edge, press F12. Open Application > Storage > Cookies and select https://claude.ai.",
            ),
            instruction(
                k,
                "3. Find sessionKey. Copy its Value, not its name or the whole cookie table, and paste it into the field above.",
            ),
            link(
                k,
                "claude-cookies",
                "How to view cookies in Chrome",
                "https://developer.chrome.com/docs/devtools/application/cookies",
            ),
            instruction(
                k,
                "A Cookie header containing sessionKey also works. When the session expires, paste a fresh one here.",
            ),
        ],
        ProfileCredentialMethod::OAuthToken => vec![
            instruction(
                k,
                "Use the access token from a Claude Code subscription login. Minibar cannot refresh a pasted token; prefer Source: Config folder when you can.",
            ),
            instruction(
                k,
                "Copy only claudeAiOauth.accessToken (starts with sk-ant-oat) from that login's .credentials.json, without quotes. Do not use claude setup-token: it may lack usage access.",
            ),
            link(
                k,
                "claude-multi",
                "Claude Code: log in with multiple accounts",
                "https://code.claude.com/docs/en/authentication#log-in-with-multiple-accounts",
            ),
            instruction(
                k,
                "Requires a Claude subscription login with usage access. API keys and Admin API keys do not show subscription limits.",
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
        let dialog = self.provider_dialog.clone()?;
        let instances = self.settings.instances.clone();
        let instance_name = |provider: &ProviderId| {
            instances
                .iter()
                .find(|instance| instance.id == provider.id())
                .map(|instance| provider_label(instance, &instances))
                .unwrap_or_else(|| "this provider".into())
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
                let choices = add_instance_choices(&instances);
                let labels = choices
                    .iter()
                    .map(|(driver, blocked)| {
                        SharedString::from(match blocked {
                            Some(_) => format!("{} (already added)", driver.display_name()),
                            None => driver.display_name().to_owned(),
                        })
                    })
                    .collect::<Vec<_>>();
                let selected = dialog
                    .driver
                    .and_then(|driver| choices.iter().position(|(choice, _)| *choice == driver));
                fields.push(kit::field(
                    k,
                    "Provider",
                    kit::dropdown_with_placeholder(
                        k,
                        "dlg-driver",
                        labels,
                        selected,
                        dialog.checking,
                        0.0,
                        Some("Choose a provider".into()),
                        Self::h(cx, move |this, index: usize, _, cx| {
                            if let (Some(dialog), Some((driver, _))) =
                                (this.provider_dialog.as_mut(), choices.get(index))
                            {
                                dialog.driver = Some(*driver);
                                dialog.error = None;
                            }
                            cx.notify();
                        }),
                    ),
                ));
                let placeholder = dialog
                    .driver
                    .map_or("e.g. Work", |driver| driver.display_name());
                fields.push(self.dialog_input(
                    k,
                    FIELD_NAME,
                    "Name",
                    placeholder,
                    &dialog.initial_name,
                    false,
                    Some("Shown on its tab, Home card, tray and notifications."),
                    window,
                    cx,
                ));
                first_input = Some(FIELD_NAME);
                let badge =
                    self.dialog_input(k, FIELD_BADGE, "Badge", "Auto", "", false, None, window, cx);
                let color = kit::field(
                    k,
                    "Badge color",
                    kit::dropdown(
                        k,
                        "dlg-badge-color",
                        BadgeColor::ALL
                            .iter()
                            .map(|color| SharedString::from(color.label()))
                            .collect(),
                        usize::try_from(dialog.badge_color.index()).ok(),
                        dialog.checking,
                        0.0,
                        Self::h(cx, |this, index: usize, _, cx| {
                            if let Some(dialog) = this.provider_dialog.as_mut() {
                                dialog.badge_color = BadgeColor::from_index(index as i32);
                            }
                            cx.notify();
                        }),
                    ),
                );
                fields.push(
                    div()
                        .flex()
                        .gap(px(12.0))
                        .child(div().flex_1().child(badge))
                        .child(color)
                        .into_any_element(),
                );
                fields.push(kit::caption(
                    k,
                    "Up to three letters; empty uses the name's initials. Badges show while a provider has more than one instance turned on.",
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
                fields.push(kit::caption(
                    k,
                    format!(
                        "Minibar stops reading {name} and forgets its saved keys, schedules, tray indicators and Home position.{}",
                        if folder {
                            " Its config folder stays on disk."
                        } else {
                            ""
                        }
                    ),
                ));
                (format!("Delete {name}?"), "Delete")
            }
            ProviderDialogKind::SignIn { provider } => {
                let name = instance_name(provider);
                fields.push(kit::caption(
                    k,
                    if dialog.checking {
                        "Finish signing in in your browser. Cancel stops this login.".to_owned()
                    } else if provider.kind() == ProviderKind::Claude {
                        format!("Runs Claude Code's own login for {name} with its config folder as CLAUDE_CONFIG_DIR. The login stays in that folder, where Claude Code keeps it fresh. Requires native Windows Claude Code.")
                    } else {
                        format!("Runs Codex's own login for {name} with its config folder as CODEX_HOME. The login stays in that folder, where Codex keeps it fresh. Requires the native Codex CLI or desktop app.")
                    },
                ));
                (format!("Sign in to {name}"), "Sign in")
            }
            ProviderDialogKind::ManualCredential { provider } => {
                wide = true;
                fields.push(kit::caption(k, format!("For {}.", instance_name(provider))));
                let method = dialog.claude_method;
                fields.push(kit::segmented(
                    k,
                    "dlg-claude-method",
                    &["Browser session", "OAuth token"],
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
                    ProfileCredentialMethod::BrowserSession => {
                        (FIELD_KEY, "Session key", "Paste the sessionKey value")
                    }
                    ProfileCredentialMethod::OAuthToken => {
                        (FIELD_SECOND, "OAuth access token", "sk-ant-oat…")
                    }
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
                    "The saved credential is replaced only after the new one passes the check.",
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
                ("Claude credential".to_owned(), "Check and save")
            }
            ProviderDialogKind::OpenRouterApiKey { account_id, key_id } => {
                let replacing = key_id.as_ref().is_some_and(|key_id| {
                    crate::openrouter::api_key_is_configured(account_id, key_id)
                });
                fields.push(kit::caption(
                    k,
                    format!("For {}.", account_name(account_id)),
                ));
                if !replacing {
                    fields.push(self.dialog_input(
                        k,
                        FIELD_NAME,
                        "Key name (optional)",
                        "e.g. Personal",
                        &dialog.initial_name,
                        false,
                        Some("Leave blank to use the name from OpenRouter."),
                        window,
                        cx,
                    ));
                    first_input = Some(FIELD_NAME);
                }
                fields.push(self.dialog_input(
                    k,
                    FIELD_KEY,
                    "API key",
                    "sk-or-v1-…",
                    "",
                    true,
                    Some("Minibar checks the key with OpenRouter before saving it."),
                    window,
                    cx,
                ));
                first_input.get_or_insert(FIELD_KEY);
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
                fields.push(kit::caption(
                    k,
                    format!("For {}.", account_name(account_id)),
                ));
                fields.push(self.dialog_input(
                    k,
                    FIELD_KEY,
                    "Management key",
                    "sk-or-v1-…",
                    "",
                    true,
                    Some("Create one under Settings → Management keys on openrouter.ai."),
                    window,
                    cx,
                ));
                first_input = Some(FIELD_KEY);
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
                fields.push(self.dialog_input(
                    k,
                    FIELD_NAME,
                    "Key name",
                    "e.g. Personal",
                    &dialog.initial_name,
                    false,
                    Some("Leave blank to use the name from OpenRouter."),
                    window,
                    cx,
                ));
                first_input = Some(FIELD_NAME);
                ("Rename key".to_owned(), "Save")
            }
            ProviderDialogKind::RemoveOpenRouterApiKey { account_id, key_id } => {
                let hint = crate::openrouter::api_key_hint(account_id, key_id)
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "this key".into());
                fields.push(kit::caption(
                    k,
                    format!("Minibar stops tracking {hint}. The key keeps working on OpenRouter."),
                ));
                ("Remove API key?".to_owned(), "Remove")
            }
            ProviderDialogKind::RemoveOpenRouterManagementKey { account_id } => {
                fields.push(kit::caption(
                    k,
                    format!(
                        "Minibar stops showing credit balance and usage history for {}. The key keeps working on OpenRouter.",
                        account_name(account_id)
                    ),
                ));
                ("Remove management key?".to_owned(), "Remove")
            }
            ProviderDialogKind::OpenCodeKey { provider, replace } => {
                fields.push(kit::caption(k, format!("For {}.", instance_name(provider))));
                fields.push(self.dialog_input(
                    k,
                    FIELD_KEY,
                    "API key",
                    "sk-…",
                    "",
                    true,
                    Some("Saved in Windows user storage, never in the settings file."),
                    window,
                    cx,
                ));
                first_input = Some(FIELD_KEY);
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
                fields.push(kit::caption(
                    k,
                    format!(
                        "Minibar forgets the saved key of {}. It keeps working with OpenCode.",
                        instance_name(provider)
                    ),
                ));
                ("Remove API key?".to_owned(), "Remove")
            }
        };
        if let Some(error) = &dialog.error
            && !matches!(dialog.kind, ProviderDialogKind::ManualCredential { .. })
        {
            fields.push(kit::info_bar(k, kit::Severity::Critical, error.clone()));
        }
        if !dialog.focused {
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
                "Signing in…"
            } else {
                "Checking…"
            }
        } else {
            primary
        };
        let mut primary_button = Button::new("dlg-primary", primary_label)
            .full_width()
            .disabled(dialog.checking)
            .on_click(Self::h(cx, |this, (), window, cx| {
                this.submit_provider_dialog(window, cx)
            }));
        primary_button = if destructive {
            primary_button.danger()
        } else {
            primary_button.accent()
        };
        let login_pending = dialog.checking && dialog.is_sign_in();
        let cancel = Button::new("dlg-cancel", "Cancel")
            .full_width()
            .disabled(dialog.checking && !login_pending)
            .on_click(Self::h(cx, |this, (), _, cx| {
                this.dismiss_provider_dialog(cx)
            }));
        let dismiss = (!dialog.checking)
            .then(|| Self::h(cx, |this, (), _, cx| this.dismiss_provider_dialog(cx)));
        Some(kit::dialog(
            k,
            "provider",
            if wide { 560.0 } else { DIALOG_WIDTH },
            body,
            vec![primary_button.render(k), cancel.render(k)],
            dismiss,
        ))
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
                    return self.fail_dialog("Choose a provider.", cx);
                };
                if let Some((_, Some(reason))) = add_instance_choices(&instances)
                    .into_iter()
                    .find(|(choice, _)| *choice == driver)
                {
                    return self.fail_dialog(format!("{reason}. It is already in the list."), cx);
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
                                notice: format!("Added {name}."),
                            },
                            cx,
                        );
                        if let Some(provider) = added {
                            self.select_provider(provider, cx);
                        }
                    }
                    Err(error) => {
                        self.fail_dialog(format!("Could not add the provider: {error:#}"), cx)
                    }
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
                        self.reload_settings();
                        self.finish_dialog(
                            DialogOutcome {
                                notice: format!("{name} deleted."),
                            },
                            cx,
                        );
                        let page = super::super::nav::first_provider_page(&self.settings.instances);
                        self.navigate(page, cx);
                    }
                    Err(error) => {
                        self.fail_dialog(format!("Could not delete the provider: {error:#}"), cx)
                    }
                }
            }
            ProviderDialogKind::ManualCredential { provider } => {
                let credential = match dialog.claude_method {
                    ProfileCredentialMethod::BrowserSession => key,
                    ProfileCredentialMethod::OAuthToken => second,
                };
                if credential.is_empty() {
                    return self.fail_dialog("Paste a credential first.", cx);
                }
                if let Err(error) = dialog.claude_method.validate(&credential) {
                    return self.fail_dialog(error.to_string(), cx);
                }
                self.run_dialog_work(cx, move || {
                    crate::claude::verify_credential(&credential)?;
                    persist_claude_manual_credential(settings_tx, provider, &credential)?;
                    Ok(DialogOutcome {
                        notice: "Credential saved in Windows user storage.".into(),
                    })
                });
            }
            ProviderDialogKind::OpenRouterApiKey { account_id, key_id } => {
                if key.is_empty() {
                    return self.fail_dialog("Paste a key first.", cx);
                }
                if !looks_like_openrouter_key(&key) {
                    return self.fail_dialog(NOT_OPENROUTER_KEY, cx);
                }
                if !account_exists(&account_id) {
                    return self.fail_dialog("This account no longer exists.", cx);
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
                        notice: "API key saved in Windows user storage.".into(),
                    })
                });
            }
            ProviderDialogKind::OpenRouterManagementKey {
                account_id,
                replace,
            } => {
                if key.is_empty() {
                    return self.fail_dialog("Paste a key first.", cx);
                }
                if !looks_like_openrouter_key(&key) {
                    return self.fail_dialog(NOT_OPENROUTER_KEY, cx);
                }
                if !account_exists(&account_id) {
                    return self.fail_dialog("This account no longer exists.", cx);
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
                            "Management key replaced.".to_owned()
                        } else {
                            "Management key added.".to_owned()
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
                    return self.fail_dialog(format!("Could not rename the key: {error:#}"), cx);
                }
                self.reload_settings();
                self.finish_dialog(
                    DialogOutcome {
                        notice: "API key renamed.".into(),
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
                            "OpenRouter API key no longer exists"
                        );
                        Ok(())
                    },
                ) {
                    return self.fail_dialog(format!("Could not remove the key: {error:#}"), cx);
                }
                self.reload_settings();
                self.finish_dialog(
                    DialogOutcome {
                        notice: "API key removed.".into(),
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
                    return self.fail_dialog(format!("Could not remove the key: {error:#}"), cx);
                }
                self.reload_settings();
                self.finish_dialog(
                    DialogOutcome {
                        notice: "Management key removed.".into(),
                    },
                    cx,
                );
            }
            ProviderDialogKind::OpenCodeKey { provider, .. } => {
                if key.is_empty() {
                    return self.fail_dialog("Paste a key first.", cx);
                }
                if let Err(error) = persist_opencode_manual_key(settings_tx, provider, Some(key)) {
                    return self.fail_dialog(format!("Could not save the key: {error:#}"), cx);
                }
                self.reload_settings();
                self.finish_dialog(
                    DialogOutcome {
                        notice: "API key saved.".into(),
                    },
                    cx,
                );
            }
            ProviderDialogKind::RemoveOpenCodeKey { provider } => {
                if let Err(error) = persist_opencode_manual_key(settings_tx, provider, None) {
                    return self.fail_dialog(format!("Could not remove the key: {error:#}"), cx);
                }
                self.reload_settings();
                self.finish_dialog(
                    DialogOutcome {
                        notice: "API key removed.".into(),
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
        ProviderKind::Claude => {
            crate::claude::profile_oauth::login(instance.binary_path.as_deref(), &folder, &control)?
        }
        ProviderKind::Codex => {
            crate::codex::profile_oauth::login(instance.binary_path.as_deref(), &folder, &control)?
        }
        _ => anyhow::bail!("This provider has no sign-in."),
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
        notice: format!(
            "{} signed in. Its CLI keeps the login fresh in its config folder.",
            instance.display_name()
        ),
    })
}
