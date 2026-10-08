//! One provider instance's page.

use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px, relative,
};

use super::super::input::InputEvent;
use super::super::kit::{self, Button, Kit, MenuItem, Row};
use super::super::window::SettingsWindow;
use super::*;
use crate::popup_window::ui::fx;
use crate::settings::{BadgeColor, LimitRefreshInterval, TimeFormat};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dot {
    Success,
    Caution,
}

/// One-line status shared by the page header and the sidebar dot.
fn status_line(
    instance: &ProviderInstance,
    status: &ProviderInstallStatus,
) -> (String, Option<Dot>) {
    let provider = instance.driver;
    if !instance.enabled {
        return (crate::i18n::tr("off").into(), None);
    }
    let readiness = provider_readiness(status);
    let dot = match readiness {
        ProviderReadiness::Checking => None,
        ProviderReadiness::Ready => Some(Dot::Success),
        ProviderReadiness::NeedsSetup => Some(Dot::Caution),
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
            return (crate::i18n::tr("no-keys-yet").into(), Some(Dot::Caution));
        }
        let mut text = crate::i18n::format("api-key-count", &[("v0", keys.to_string())]);
        if management {
            text.push_str(crate::i18n::tr("management-key-5c8cf2"));
        }
        return (text, dot);
    }
    if instance.uses_manual_credential() {
        return match readiness {
            ProviderReadiness::Ready => (crate::i18n::tr("using-a-saved-credential").into(), dot),
            ProviderReadiness::Checking => (crate::i18n::tr("checking").into(), dot),
            ProviderReadiness::NeedsSetup => (
                crate::i18n::tr("paste-a-credential-under-account").into(),
                dot,
            ),
        };
    }
    let text = match readiness {
        ProviderReadiness::Checking => crate::i18n::tr("checking").to_owned(),
        ProviderReadiness::NeedsSetup => match provider {
            ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => {
                if instance.is_primary() {
                    crate::i18n::tr("needs-an-api-key-or-opencode-sign-in").to_owned()
                } else {
                    crate::i18n::tr("needs-an-api-key").to_owned()
                }
            }
            _ => crate::i18n::tr("not-found-set-its-folder-under-runtime").to_owned(),
        },
        ProviderReadiness::Ready => match provider {
            ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => {
                if crate::opencode::key_is_configured(instance.provider_id()) {
                    crate::i18n::tr("using-a-saved-api-key").to_owned()
                } else {
                    crate::i18n::tr("using-opencode-sign-in-or-local-history").to_owned()
                }
            }
            _ => {
                let (app, crew, cli) = source_labels(provider);
                match status.used {
                    Some(ProviderInstallSource::Cli) => {
                        crate::i18n::format("reading-cli", &[("cli", cli.to_string())])
                    }
                    Some(ProviderInstallSource::Crew) => {
                        crate::i18n::format("reading-crew", &[("crew", crew.to_string())])
                    }
                    _ => crate::i18n::format("reading-app", &[("app", app.to_string())]),
                }
            }
        },
    };
    (text, dot)
}

fn checking_message(status: &ProviderInstallStatus) -> &'static str {
    if status.crew_applicable {
        crate::i18n::tr("checking-kiro-ide-kiro-crew-and-cli")
    } else if status.app_applicable && status.cli_applicable {
        crate::i18n::tr("checking-installed-app-and-cli")
    } else if status.cli_applicable {
        crate::i18n::tr("checking-cli")
    } else {
        crate::i18n::tr("checking-installed-app")
    }
}

fn masked(k: &Kit, hint: impl Into<SharedString>) -> AnyElement {
    div()
        .font_family(k.theme.mono_font.clone())
        .text_size(px(12.0))
        .text_color(k.theme.text_secondary)
        .child(hint.into())
        .into_any_element()
}

impl SettingsWindow {
    pub(in super::super) fn no_providers_page(
        &mut self,
        k: &mut Kit,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let add = Self::h(cx, |this, (), window, cx| {
            this.open_provider_dialog(ProviderDialog::add_instance(), window, cx)
        });
        vec![
            kit::page_title(k, crate::i18n::tr("providers")),
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(14.0))
                .py(px(40.0))
                .rounded(px(kit::CARD_RADIUS))
                .bg(k.theme.card)
                .child(kit::icon(
                    "plugs-connected-fill",
                    36.0,
                    k.theme.text_tertiary,
                ))
                .child(kit::caption(
                    k,
                    crate::i18n::tr("no-providers-yet-add-one-to-start-reading-limits"),
                ))
                .child(
                    Button::new("no-providers-add", crate::i18n::tr("add-provider"))
                        .accent()
                        .with_icon("plus-bold")
                        .on_click(add)
                        .render(k),
                )
                .into_any_element(),
        ]
    }

    pub(in super::super) fn provider_page(
        &mut self,
        provider: ProviderId,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(instance) = self.instance(provider).cloned() else {
            return self.no_providers_page(k, cx);
        };
        let status = self.install_status(provider);
        let mut rows = vec![self.provider_header(&instance, &status, k, cx)];
        if !instance.enabled {
            rows.push(kit::info_bar(
                k,
                kit::Severity::Info,
                crate::i18n::format(
                    "is-off-so-it-doesn-t-appear-in-the-minibar-or-tray-turn-it-on-to",
                    &[("v0", instance.display_name().to_string())],
                ),
            ));
        }
        let mut sections = Vec::new();
        sections.extend(self.general_section(&instance, k, window, cx));
        sections.extend(self.account_section(&instance, k, cx));
        sections.extend(self.runtime_section(&instance, &status, k, window, cx));
        sections.extend(self.features_section(&instance, k, cx));
        sections.push(kit::section_heading(k, crate::i18n::tr("appearance")));
        if instance.driver == ProviderKind::Codex {
            sections.push(kit::card_of(k, |k| {
                vec![kit::toggle_row(
                    k,
                    "codex-replace-logo",
                    crate::i18n::tr("replace-chatgpt-logo-with-codex"),
                    None,
                    self.settings.replace_chatgpt_logo_with_codex,
                    Self::h(cx, |this, value: bool, _, cx| {
                        crate::provider_registry::apply_logo_settings(value);
                        this.edit(cx, move |s| s.replace_chatgpt_logo_with_codex = value)
                    }),
                )]
            }));
            sections.push(div().h(px(4.0)).into_any_element());
        }
        sections.push(self.popup_cards_section(&instance, k, cx));
        let dim = k.fx.toggle(
            fx::key(("provider-dim", provider.id())),
            !instance.enabled,
            fx::FAST,
        );
        rows.push(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .opacity(1.0 - 0.45 * dim)
                .children(sections)
                .into_any_element(),
        );
        rows
    }

    fn provider_header(
        &mut self,
        instance: &ProviderInstance,
        status: &ProviderInstallStatus,
        k: &mut Kit,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let provider = instance.provider_id();
        let theme = k.theme.clone();
        let (status_text, dot) = status_line(instance, status);
        let mut status_row = div().flex().items_center().gap(px(8.0));
        if let Some(dot) = dot {
            status_row = status_row.child(kit::status_dot(match dot {
                Dot::Success => theme.success,
                Dot::Caution => theme.caution,
            }));
        }
        status_row = status_row.child(kit::text(status_text, 13.0, theme.text_secondary));
        if crate::instances::Capabilities::of(instance).limits_only() {
            status_row = status_row.child(kit::chip(k, crate::i18n::tr("limits-only")));
        }
        let color = if instance.enabled {
            theme.brand(instance.driver)
        } else {
            theme.glyph()
        };
        let enabled = instance.enabled;
        let toggle = kit::toggle(
            k,
            format!("provider-enabled-{}", provider.id()),
            enabled,
            false,
            Self::h(cx, move |this, value: bool, _, cx| {
                this.edit_instance(cx, provider, move |instance| instance.enabled = value)
            }),
        );
        let delete = Self::h(cx, move |this, (), window, cx| {
            this.open_provider_dialog(
                ProviderDialog::new(ProviderDialogKind::DeleteInstance { provider }),
                window,
                cx,
            )
        });
        // The badge sits on the glyph's lower-right corner, as in the popup.
        let badge = super::super::nav::shows_badge(instance, &self.settings.instances)
            .then(|| instance.badge());
        let title = div()
            .text_size(px(26.0))
            .line_height(px(30.0))
            .font_weight(FontWeight::SEMIBOLD)
            .child(instance.display_name());
        div()
            .flex()
            .items_center()
            .gap(px(16.0))
            .pb(px(12.0))
            .child(
                div()
                    .size(px(56.0))
                    .flex_none()
                    .rounded(px(12.0))
                    .bg(theme.card)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(kit::provider_mark(
                        k,
                        crate::provider_registry::icon(instance.driver),
                        28.0,
                        color,
                        badge.as_ref(),
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .flex_1()
                    .min_w_0()
                    .child(title)
                    .child(status_row),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(kit::text(
                        if enabled {
                            crate::i18n::tr("on")
                        } else {
                            crate::i18n::tr("off")
                        },
                        13.0,
                        theme.text_secondary,
                    ))
                    .child(toggle)
                    .child(
                        Button::icon_only(
                            format!("provider-delete-{}", provider.id()),
                            "trash-fill",
                        )
                        .tooltip(crate::i18n::tr("delete-provider"))
                        .on_click(delete)
                        .render(k),
                    ),
            )
            .into_any_element()
    }

    fn general_section(
        &mut self,
        instance: &ProviderInstance,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let provider = instance.provider_id();
        let name = self.input(
            format!("provider-{}-name", provider.id()),
            &instance.name,
            instance.driver.display_name(),
            false,
            false,
            window,
            cx,
            move |this, value, _, _, cx| {
                if this
                    .instance(provider)
                    .is_some_and(|instance| instance.name != value)
                {
                    this.edit_instance(cx, provider, move |instance| instance.name = value.clone());
                }
            },
        );
        let badge = self.input(
            format!("provider-{}-badge", provider.id()),
            &instance.badge,
            instance.badge().text,
            false,
            false,
            window,
            cx,
            move |this, value, _, _, cx| {
                let value = crate::instances::sanitize_badge(&value);
                if this
                    .instance(provider)
                    .is_some_and(|instance| instance.badge != value)
                {
                    this.edit_instance(cx, provider, move |instance| {
                        instance.badge = value.clone()
                    });
                }
            },
        );
        let name_field = kit::text_field(k, &name, Some(240.0), window, cx);
        let badge_field = kit::text_field(k, &badge, Some(88.0), window, cx);
        let badge_color = badge_color_swatches(
            k,
            format!("provider-{}-badge-color", provider.id()),
            instance.badge_color,
            Self::h(cx, move |this, color: BadgeColor, _, cx| {
                if this
                    .instance(provider)
                    .is_some_and(|instance| instance.badge_color != color)
                {
                    this.edit_instance(cx, provider, move |instance| instance.badge_color = color)
                }
            }),
        );
        let rows = vec![
            Row::new("provider-name", crate::i18n::tr("display-name"))
                .description(
                    k,
                    crate::i18n::tr("shown-on-popup-tabs-home-cards-the-tray-and-notifications"),
                )
                .trailing(name_field)
                .render(k),
            Row::new("provider-badge", crate::i18n::tr("badge"))
                .description(
                    k,
                    crate::i18n::tr(
                        "up-to-three-letters-leave-empty-to-use-the-name-s-initials-shown",
                    ),
                )
                .trailing(kit::badge_plate(k, &instance.badge()))
                .trailing(badge_field)
                .render(k),
            Row::new("provider-badge-color", crate::i18n::tr("badge-color"))
                .description(
                    k,
                    crate::i18n::tr("auto-uses-a-neutral-plate-that-follows-the-theme"),
                )
                .detail(div().pt(px(12.0)).child(badge_color).into_any_element())
                .render(k),
            kit::toggle_row(
                k,
                format!("provider-{}-home", provider.id()),
                crate::i18n::tr("show-on-home"),
                Some(
                    crate::i18n::tr("its-provider-tab-stays-available-when-hidden-from-home")
                        .into(),
                ),
                instance.show_on_home,
                Self::h(cx, move |this, value: bool, _, cx| {
                    this.edit_instance(cx, provider, move |instance| instance.show_on_home = value)
                }),
            ),
        ];
        vec![
            kit::section_heading(k, crate::i18n::tr("general")),
            kit::card(k, rows),
        ]
    }

    fn account_section(
        &mut self,
        instance: &ProviderInstance,
        k: &mut Kit,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let provider = instance.provider_id();
        match instance.driver {
            ProviderKind::Claude | ProviderKind::Codex => {}
            ProviderKind::OpenRouter => return self.openrouter_section(instance, k, cx),
            ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => {
                return self.opencode_key_section(instance, k, cx);
            }
            _ => return Vec::new(),
        }
        let identity = self.openrouter.identities.get(&instance.id).cloned();
        let open = |kind: ProviderDialogKind| {
            Self::h(cx, move |this, (), window, cx| {
                this.open_provider_dialog(ProviderDialog::new(kind.clone()), window, cx)
            })
        };
        let row = if instance.uses_manual_credential() {
            let saved = crate::claude::load_manual_credential(&instance.id)
                .ok()
                .flatten()
                .filter(|value| !value.trim().is_empty());
            let mut row = Row::new("account-manual", crate::i18n::tr("credential"))
                .icon(kit::row_icon(k, "key-fill"))
                .description(
                    k,
                    match (&saved, &identity) {
                        (Some(_), Some(identity)) => crate::i18n::format(
                            "signed-in-as-identity",
                            &[("identity", identity.to_string())],
                        ),
                        (Some(_), None) => crate::i18n::tr("saved-in-windows-user-storage").into(),
                        (None, _) => {
                            crate::i18n::tr("paste-a-sessionkey-or-an-oauth-access-token").into()
                        }
                    },
                );
            if saved.is_some() {
                row = row.description(
                    k,
                    crate::i18n::tr("reads-limits-only-minibar-cannot-refresh-a-pasted-credential"),
                );
            }
            if let Some(saved) = &saved {
                row = row.trailing(masked(k, crate::secrets::masked_hint(saved)));
            }
            row.trailing(
                Button::new(
                    "account-manual-button",
                    if saved.is_some() {
                        crate::i18n::tr("replace-credential")
                    } else {
                        crate::i18n::tr("add-credential")
                    },
                )
                .on_click(open(ProviderDialogKind::ManualCredential { provider }))
                .render(k),
            )
        } else {
            let folder = instance
                .config_folder()
                .or_else(|| crate::instances::default_folder(instance.driver));
            let sign_in_reason = crate::instances::Capabilities::reason(
                instance,
                crate::instances::Capability::SignIn,
            );
            let mut row = Row::new("account-sign-in", crate::i18n::tr("signed-in-account"))
                .icon(kit::row_icon(k, "user-fill"))
                .description(
                    k,
                    match &identity {
                        Some(identity) => crate::i18n::format(
                            "signed-in-as-identity",
                            &[("identity", identity.to_string())],
                        ),
                        None => {
                            crate::i18n::tr("not-signed-in-yet-or-no-limits-read-so-far").into()
                        }
                    },
                );
            if let Some(folder) = &folder {
                row = row.detail(
                    div()
                        .truncate()
                        .text_size(px(12.0))
                        .text_color(k.theme.text_tertiary)
                        .child(display_fs_path(folder))
                        .into_any_element(),
                );
            }
            row.trailing(
                Button::new(
                    "account-sign-in-button",
                    if identity.is_some() {
                        crate::i18n::tr("sign-in-again")
                    } else {
                        crate::i18n::tr("sign-in")
                    },
                )
                .with_icon("sign-in-bold")
                .disabled(sign_in_reason.is_some())
                .on_click(open(ProviderDialogKind::SignIn { provider }))
                .render(k),
            )
        };
        vec![
            kit::section_heading(k, crate::i18n::tr("account")),
            kit::row_card(k, row),
        ]
    }

    fn runtime_section(
        &mut self,
        instance: &ProviderInstance,
        status: &ProviderInstallStatus,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let provider = instance.provider_id();
        let driver = instance.driver;
        if driver == ProviderKind::OpenRouter {
            return Vec::new();
        }
        let mut out = vec![kit::section_header(
            k,
            crate::i18n::tr("runtime"),
            Some(
                crate::i18n::format(
                    "minibar-finds-these-automatically",
                    &[("v0", (provider_description(driver)).to_string())],
                )
                .into(),
            ),
            None,
        )];
        if matches!(driver, ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo) {
            if instance.is_primary() {
                out.push(self.opencode_source_card(status, k));
            } else {
                out.pop();
            }
            return out;
        }
        let mut cards = Vec::new();
        if !instance.uses_manual_credential() {
            if status.checking {
                cards.push(
                    Row::new("sources-checking", checking_message(status))
                        .icon(kit::row_icon(k, "arrow-clockwise-bold"))
                        .render(k),
                );
            } else {
                let (app_label, crew_label, cli_label) = source_labels(driver);
                let other_claude_account =
                    driver == ProviderKind::Claude && instance.config_folder().is_some();
                if status.app_applicable {
                    cards.push(self.source_row(
                        k,
                        cx,
                        provider,
                        "desktop-fill",
                        app_label,
                        status.app.as_deref(),
                        status.used == Some(ProviderInstallSource::App),
                        driver == ProviderKind::Cursor,
                        other_claude_account.then_some(crate::claude::OTHER_ACCOUNT_NEEDS_CLI),
                    ));
                }
                if status.crew_applicable {
                    cards.push(self.source_row(
                        k,
                        cx,
                        provider,
                        "desktop-fill",
                        crew_label,
                        status.crew.as_deref(),
                        status.used == Some(ProviderInstallSource::Crew),
                        false,
                        None,
                    ));
                }
                if status.cli_applicable {
                    cards.push(self.source_row(
                        k,
                        cx,
                        provider,
                        "terminal-window-fill",
                        cli_label,
                        status.cli.as_deref(),
                        status.used == Some(ProviderInstallSource::Cli),
                        driver != ProviderKind::Kiro,
                        None,
                    ));
                }
            }
        }
        if !cards.is_empty() {
            out.push(kit::card(k, cards));
        }
        if matches!(driver, ProviderKind::Claude | ProviderKind::Codex) {
            out.extend(self.source_settings(instance, k, window, cx));
        }
        for config in folder_configs(driver) {
            out.push(div().h(px(4.0)).into_any_element());
            out.push(self.folder_expander(instance, config, k, window, cx));
        }
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn source_row(
        &mut self,
        k: &Kit,
        cx: &mut Context<Self>,
        provider: ProviderId,
        icon: &'static str,
        label: &str,
        path: Option<&str>,
        in_use: bool,
        can_choose_folder: bool,
        note: Option<&'static str>,
    ) -> AnyElement {
        let theme = &k.theme;
        let id = format!("source-{}-{label}", provider.id());
        let mut row = Row::new(id.clone(), label.to_owned()).icon(kit::row_icon(k, icon));
        let note = note.map(|note| kit::text(note, 12.0, theme.caution).into_any_element());
        match path {
            Some(path) => {
                let menu_path = path.to_owned();
                row = row.detail(
                    div()
                        .text_size(px(12.0))
                        .text_color(theme.text_secondary)
                        .child(path.to_owned())
                        .into_any_element(),
                );
                if let Some(note) = note {
                    row = row.detail(note);
                }
                row = row
                    .trailing(kit::status_dot(theme.success))
                    .trailing(
                        kit::text(
                            if in_use {
                                crate::i18n::tr("in-use")
                            } else {
                                crate::i18n::tr("found")
                            },
                            12.0,
                            theme.text_secondary,
                        )
                        .into_any_element(),
                    )
                    .trailing(kit::more_menu(
                        k,
                        format!("{id}-menu"),
                        vec![
                            MenuItem::new(crate::i18n::tr("copy-path")).icon("copy"),
                            MenuItem::new(crate::i18n::tr("open-folder")).icon("folder-open-fill"),
                        ],
                        Self::h(cx, move |this, choice: usize, _, cx| {
                            if choice == 1 {
                                cx.reveal_path(std::path::Path::new(&menu_path));
                            } else {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                    menu_path.clone(),
                                ));
                                this.show_notice(crate::i18n::tr("path-copied"), cx);
                            }
                        }),
                    ));
            }
            None => {
                row = row
                    .description(
                        k,
                        crate::i18n::tr(
                            "not-installed-or-installed-somewhere-minibar-doesn-t-look",
                        ),
                    )
                    .trailing(
                        kit::text(crate::i18n::tr("not-found"), 12.0, theme.text_secondary)
                            .into_any_element(),
                    );
                if can_choose_folder {
                    row = row.trailing(
                        Button::new(format!("{id}-choose"), crate::i18n::tr("choose-folder"))
                            .with_icon("folder-fill")
                            .on_click(Self::h(cx, move |this, (), _, cx| {
                                this.pick_folder(provider, PathField::Binary, cx)
                            }))
                            .render(k),
                    );
                }
            }
        }
        row.render(k)
    }

    /// Ask for a folder, then store it in one of the instance's path fields.
    fn pick_folder(&mut self, provider: ProviderId, field: PathField, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(crate::i18n::tr("select-folder").into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = prompt.await else {
                return;
            };
            let Some(folder) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.edit_instance(cx, provider, move |instance| {
                    field.write(instance, Some(folder.clone()))
                });
            });
        })
        .detach();
    }

    /// A text box plus a folder button for one path field.
    fn folder_picker(
        &mut self,
        instance: &ProviderInstance,
        field: PathField,
        placeholder: &str,
        k: &Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let provider = instance.provider_id();
        let input = self.input(
            format!("provider-{}-path-{}", provider.id(), field.key()),
            &field.read(instance),
            placeholder.to_owned(),
            false,
            false,
            window,
            cx,
            move |this, value, _, _, cx| {
                let unchanged = this
                    .instance(provider)
                    .is_some_and(|instance| field.read(instance) == value.trim());
                if unchanged {
                    return;
                }
                let folder = (!value.trim().is_empty()).then(|| PathBuf::from(value.trim()));
                this.edit_instance(cx, provider, move |instance| {
                    field.write(instance, folder.clone())
                });
            },
        );
        div()
            .flex()
            .gap(px(8.0))
            .w_full()
            .child(
                div()
                    .flex_1()
                    .child(kit::text_field(k, &input, None, window, cx)),
            )
            .child(
                Button::icon_only(
                    format!("provider-{}-browse-{}", provider.id(), field.key()),
                    "folder-open-fill",
                )
                .kind(kit::ButtonKind::Standard)
                .tooltip(crate::i18n::tr("choose-folder-1838a4"))
                .on_click(Self::h(cx, move |this, (), _, cx| {
                    this.pick_folder(provider, field, cx)
                }))
                .render(k),
            )
            .into_any_element()
    }

    fn folder_expander(
        &mut self,
        instance: &ProviderInstance,
        config: FolderConfig,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let provider = instance.provider_id();
        let card_id = format!("provider-{}-advanced-{}", provider.id(), config.field.key());
        let expanded = self.is_expanded(&card_id);
        let picker = self.folder_picker(instance, config.field, config.placeholder, k, window, cx);
        let description = kit::caption(k, config.description);
        let header = Row::new(format!("{card_id}-header"), config.label)
            .icon(kit::row_icon(k, "folder-fill"))
            .description(
                k,
                crate::i18n::tr("only-needed-if-automatic-detection-misses-your-install"),
            );
        let on_toggle = Self::expand_handler(cx, card_id.clone());
        kit::expander(k, card_id, header, expanded, on_toggle, move |_| {
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(description)
                .child(picker)
                .into_any_element()
        })
    }

    /// Source selector (Claude) and config folder for Claude/Codex instances.
    fn source_settings(
        &mut self,
        instance: &ProviderInstance,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let provider = instance.provider_id();
        let mut rows = Vec::new();
        let manual = instance.uses_manual_credential();
        if instance.driver == ProviderKind::Claude {
            rows.push(
                Row::new("instance-source", crate::i18n::tr("source"))
                    .description(
                        k,
                        crate::i18n::tr(
                            "config-folder-reads-the-claude-code-login-in-claude-config-dir-ma",
                        ),
                    )
                    .trailing(kit::segmented(
                        k,
                        format!("provider-{}-source", provider.id()),
                        &[crate::i18n::tr("config-folder"), crate::i18n::tr("manual")],
                        usize::from(manual),
                        false,
                        Self::h(cx, move |this, index: usize, _, cx| {
                            let manual = index == 1;
                            this.edit_instance(cx, provider, move |instance| {
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
                            })
                        }),
                    ))
                    .render(k),
            );
        }
        if !manual {
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
            let placeholder = default.as_deref().map(display_fs_path).unwrap_or_default();
            let picker = self.folder_picker(
                instance,
                PathField::ConfigFolder,
                &placeholder,
                k,
                window,
                cx,
            );
            let mut row = Row::new("instance-config-folder", crate::i18n::tr("config-folder"))
                .description(
                    k,
                    crate::i18n::format(
                        "passed-to-the-cli-as-env-leave-empty-to-use",
                        &[
                            ("env", env.to_string()),
                            (
                                "v0",
                                (if instance.is_primary() {
                                    crate::i18n::tr("the-standard-folder")
                                } else {
                                    crate::i18n::tr("a-folder-minibar-creates-for-this-instance")
                                })
                                .to_string(),
                            ),
                        ],
                    ),
                );
            row = row.detail(div().pt(px(8.0)).child(picker).into_any_element());
            if let Some(other) =
                crate::instances::folder_conflicts(&self.settings.instances).get(&instance.id)
            {
                row = row.detail(
                    kit::text(
                        crate::i18n::format(
                            "other-already-reads-this-folder-two-instances-must-not-share-a-lo",
                            &[("other", other.to_string())],
                        ),
                        12.0,
                        k.theme.critical,
                    )
                    .into_any_element(),
                );
            }
            rows.push(row.render(k));
        }
        vec![div().h(px(4.0)).into_any_element(), kit::card(k, rows)]
    }

    fn features_section(
        &mut self,
        instance: &ProviderInstance,
        k: &mut Kit,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        use crate::instances::{Capabilities, Capability};
        let provider = instance.provider_id();
        let descriptor = crate::provider_registry::descriptor(instance.driver);
        let intervals = LimitRefreshInterval::ALL.map(|interval| crate::i18n::tr(interval.label_key()));
        let mut rows = vec![kit::dropdown_row(
            k,
            &format!("provider-{}-refresh", provider.id()),
            crate::i18n::tr("refresh-interval"),
            Some(crate::i18n::tr("how-often-this-instance-s-quotas-are-read")),
            kit::options(&intervals),
            instance.refresh_interval().index(),
            false,
            Self::h(cx, move |this, index: usize, _, cx| {
                let value = LimitRefreshInterval::from_index(index as i32);
                this.edit_instance(cx, provider, move |instance| {
                    instance.refresh_interval = Some(value)
                })
            }),
        )];
        if descriptor.supports_activation {
            rows.push(kit::toggle_row_with(
                k,
                format!("provider-{}-auto", provider.id()),
                crate::i18n::tr("automatic-activation"),
                Some(
                    crate::i18n::tr(
                        "starts-this-account-s-5-hour-window-when-it-resets-using-its-own",
                    )
                    .into(),
                ),
                instance.auto_activation,
                Capabilities::reason(instance, Capability::AutoActivation).map(SharedString::from),
                Self::h(cx, move |this, value: bool, _, cx| {
                    this.edit_instance(cx, provider, move |instance| {
                        instance.auto_activation = value
                    })
                }),
            ));
        }
        rows.push(kit::toggle_row_with(
            k,
            format!("provider-{}-usage", provider.id()),
            crate::i18n::tr("usage-statistics"),
            Some(
                crate::i18n::tr(
                    "scans-this-instance-s-local-history-for-its-usage-card-turn-off-t",
                )
                .into(),
            ),
            instance.usage_stats,
            Capabilities::reason(instance, Capability::UsageStats).map(SharedString::from),
            Self::h(cx, move |this, value: bool, _, cx| {
                this.edit_instance(cx, provider, move |instance| instance.usage_stats = value)
            }),
        ));
        vec![
            kit::section_heading(k, crate::i18n::tr("features")),
            kit::card(k, rows),
        ]
    }

    fn opencode_source_card(&self, status: &ProviderInstallStatus, k: &Kit) -> AnyElement {
        if status.checking {
            return kit::row_card(
                k,
                Row::new("sources-checking", checking_message(status))
                    .icon(kit::row_icon(k, "arrow-clockwise-bold")),
            );
        }
        let found = status.used.is_some();
        let mut row = Row::new(
            "source-opencode",
            crate::i18n::tr("opencode-sign-in-or-local-history"),
        )
        .icon(kit::row_icon(k, "terminal-window-fill"))
        .description(
            k,
            if found {
                crate::i18n::tr("found-in-opencode-auth-environment-a-saved-key-or-local-history")
            } else {
                crate::i18n::tr("nothing-found-in-opencode-auth-environment-or-local-history")
            },
        );
        if found {
            row = row.trailing(kit::status_dot(k.theme.success));
        }
        row = row.trailing(
            kit::text(
                if found {
                    crate::i18n::tr("found")
                } else {
                    crate::i18n::tr("not-found")
                },
                12.0,
                k.theme.text_secondary,
            )
            .into_any_element(),
        );
        kit::row_card(k, row)
    }

    fn opencode_key_section(
        &mut self,
        instance: &ProviderInstance,
        k: &mut Kit,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let provider = instance.provider_id();
        let saved_key = crate::opencode::manual_key(provider)
            .ok()
            .flatten()
            .filter(|value| !value.trim().is_empty());
        let row = Row::new("opencode-api-key", crate::i18n::tr("api-key"))
            .icon(kit::row_icon(k, "key-fill"));
        let row = match saved_key {
            Some(key) => row
                .description(k, crate::i18n::tr("saved-in-windows-user-storage"))
                .trailing(masked(k, crate::secrets::masked_hint(&key)))
                .trailing(kit::more_menu(
                    k,
                    format!("opencode-key-{}", provider.id()),
                    vec![
                        MenuItem::new(crate::i18n::tr("replace-key")).icon("pencil-simple-fill"),
                        MenuItem::new(crate::i18n::tr("remove-key"))
                            .icon("trash-fill")
                            .danger(),
                    ],
                    Self::h(cx, move |this, choice: usize, window, cx| {
                        let kind = if choice == 1 {
                            ProviderDialogKind::RemoveOpenCodeKey { provider }
                        } else {
                            ProviderDialogKind::OpenCodeKey {
                                provider,
                                replace: true,
                            }
                        };
                        this.open_provider_dialog(ProviderDialog::new(kind), window, cx)
                    }),
                )),
            None => row
                .description(
                    k,
                    if instance.is_primary() {
                        crate::i18n::tr("optional-only-needed-without-opencode-sign-in-on-this-pc")
                    } else {
                        crate::i18n::tr("add-the-key-of-the-account-this-instance-tracks")
                    },
                )
                .trailing(
                    Button::new("opencode-add-key", crate::i18n::tr("add-api-key"))
                        .with_icon("plus-bold")
                        .on_click(Self::h(cx, move |this, (), window, cx| {
                            this.open_provider_dialog(
                                ProviderDialog::new(ProviderDialogKind::OpenCodeKey {
                                    provider,
                                    replace: false,
                                }),
                                window,
                                cx,
                            )
                        }))
                        .render(k),
                ),
        };
        vec![
            kit::section_heading(k, crate::i18n::tr("account")),
            kit::row_card(k, row),
        ]
    }

    // ----- OpenRouter ----------------------------------------------------------

    fn openrouter_section(
        &mut self,
        instance: &ProviderInstance,
        k: &mut Kit,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(account) = instance.openrouter.clone() else {
            return Vec::new();
        };
        let mut out = vec![kit::section_header(
            k,
            crate::i18n::tr("keys"),
            Some(
                crate::i18n::tr("a-management-key-shows-credit-balance-and-usage-history-api-keys")
                    .into(),
            ),
            None,
        )];
        let snapshot = self
            .openrouter
            .accounts
            .iter()
            .find(|snapshot| snapshot.id == account.id)
            .cloned();
        let theme = k.theme.clone();
        let account_id = account.id.clone();
        let open = move |cx: &mut Context<Self>, kind: ProviderDialogKind| {
            Self::h(cx, move |this, (), window, cx| {
                this.open_provider_dialog(ProviderDialog::new(kind.clone()), window, cx)
            })
        };

        let management = match crate::openrouter::management_key_hint(&account.id) {
            Err(error) => Row::new("management-key", crate::i18n::tr("management-key"))
                .icon(kit::row_icon(k, "key-fill"))
                .detail(kit::tooltip_host(
                    k,
                    "management-key-error",
                    format!("{error:#}"),
                    kit::text(
                        crate::i18n::tr("could-not-read-the-saved-key-reopen-this-page-to-retry"),
                        12.0,
                        theme.caution,
                    )
                    .into_any_element(),
                )),
            Ok(Some(hint)) => {
                let mut row = Row::new("management-key", crate::i18n::tr("management-key"))
                    .icon(kit::row_icon(k, "key-fill"))
                    .description(
                        k,
                        crate::i18n::tr("credit-balance-and-account-wide-usage-history"),
                    );
                if let Some(balance) = snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.balance_microusd)
                {
                    row = row.detail(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.accent_text)
                            .child(crate::i18n::format(
                                "credit",
                                &[("v0", (money(balance)).to_string())],
                            ))
                            .into_any_element(),
                    );
                }
                let account_id = account_id.clone();
                row.trailing(masked(k, hint)).trailing(kit::more_menu(
                    k,
                    format!("management-{}", account.id),
                    vec![
                        MenuItem::new(crate::i18n::tr("replace-key")).icon("pencil-simple-fill"),
                        MenuItem::new(crate::i18n::tr("remove-key"))
                            .icon("trash-fill")
                            .danger(),
                    ],
                    Self::h(cx, move |this, choice: usize, window, cx| {
                        let kind = if choice == 1 {
                            ProviderDialogKind::RemoveOpenRouterManagementKey {
                                account_id: account_id.clone(),
                            }
                        } else {
                            ProviderDialogKind::OpenRouterManagementKey {
                                account_id: account_id.clone(),
                                replace: true,
                            }
                        };
                        this.open_provider_dialog(ProviderDialog::new(kind), window, cx)
                    }),
                ))
            }
            Ok(None) => Row::new("management-key", crate::i18n::tr("management-key"))
                .icon(kit::row_icon(k, "key-fill"))
                .description(
                    k,
                    crate::i18n::tr("not-added-add-one-to-see-credit-balance-and-usage-history"),
                )
                .trailing(
                    Button::new("management-add", crate::i18n::tr("add-management-key"))
                        .on_click(open(
                            cx,
                            ProviderDialogKind::OpenRouterManagementKey {
                                account_id: account_id.clone(),
                                replace: false,
                            },
                        ))
                        .render(k),
                ),
        };

        let keys_header = div()
            .flex()
            .items_center()
            .px(px(kit::ROW_PADDING_X))
            .pt(px(10.0))
            .child(
                div()
                    .flex_1()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text_secondary)
                    .child(crate::i18n::tr("api-keys")),
            )
            .child(
                Button::new("openrouter-add-key", crate::i18n::tr("add-api-key"))
                    .link()
                    .with_icon("plus-bold")
                    .on_click(open(
                        cx,
                        ProviderDialogKind::OpenRouterApiKey {
                            account_id: account_id.clone(),
                            key_id: None,
                        },
                    ))
                    .render(k),
            )
            .into_any_element();
        let table = self.openrouter_key_table(&account, snapshot.as_ref(), k, cx);
        out.push(kit::card_of(k, |k| {
            vec![
                management.render(k),
                div()
                    .flex()
                    .flex_col()
                    .child(keys_header)
                    .child(table)
                    .into_any_element(),
            ]
        }));
        if let Some(at) = self.openrouter.sampled_at {
            out.push(
                div()
                    .pt(px(6.0))
                    .child(kit::caption(
                        k,
                        crate::i18n::format(
                            "last-updated",
                            &[
                                (
                                    "v0",
                                    (crate::i18n::month_day(at.with_timezone(&chrono::Local)))
                                        .to_string(),
                                ),
                                (
                                    "v1",
                                    (TimeFormat::current()
                                        .format_hm(at.with_timezone(&chrono::Local)))
                                    .to_string(),
                                ),
                            ],
                        ),
                    ))
                    .into_any_element(),
            );
        }
        out
    }

    fn openrouter_key_table(
        &mut self,
        account: &OpenRouterAccount,
        snapshot: Option<&OpenRouterAccountSnapshot>,
        k: &Kit,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = k.theme.clone();
        if account.api_key_ids.is_empty() {
            return div()
                .px(px(kit::ROW_PADDING_X))
                .pt(px(4.0))
                .pb(px(16.0))
                .child(kit::caption(
                    k,
                    crate::i18n::tr("no-api-keys-yet-add-one-to-track-spend-per-key"),
                ))
                .into_any_element();
        }
        let column = |label: &'static str, width: Option<f32>, right: bool| {
            let cell = div()
                .text_size(px(12.0))
                .text_color(theme.text_secondary)
                .when(right, |el| el.text_right())
                .child(label);
            match width {
                Some(width) => cell.w(px(width)).flex_none(),
                None => cell.flex_1().min_w_0(),
            }
        };
        let mut table = div()
            .flex()
            .flex_col()
            .px(px(kit::ROW_PADDING_X))
            .pb(px(8.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .h(px(30.0))
                    .child(column(crate::i18n::tr("name"), None, false))
                    .child(column(crate::i18n::tr("key"), Some(120.0), false))
                    .child(column(crate::i18n::tr("spend"), Some(140.0), true))
                    .child(column(crate::i18n::tr("limit"), Some(64.0), true))
                    .child(div().w(px(kit::CONTROL_HEIGHT))),
            );
        let credit_total = account_credit_total(snapshot);
        for key_id in &account.api_key_ids {
            let key_snapshot = snapshot
                .and_then(|snapshot| snapshot.api_keys.iter().find(|key| &key.id == key_id));
            let hint_result = crate::openrouter::api_key_hint(&account.id, key_id);
            let read_error = hint_result.as_ref().err().map(|error| format!("{error:#}"));
            let hint = hint_result.ok().flatten();
            let saved = hint.is_some();
            let name: AnyElement = if let Some(error) = &read_error {
                kit::tooltip_host(
                    k,
                    format!("key-error-{key_id}"),
                    crate::i18n::format(
                        "error-reopen-this-page-to-retry",
                        &[("error", error.to_string())],
                    ),
                    kit::text(crate::i18n::tr("could-not-read-key"), 13.0, theme.caution)
                        .into_any_element(),
                )
            } else {
                match account
                    .api_key_names
                    .get(key_id)
                    .cloned()
                    .or_else(|| key_snapshot.and_then(|key| key.label.clone()))
                    .filter(|label| !label.trim().is_empty())
                {
                    Some(label) => kit::text(label, 13.0, theme.text)
                        .truncate()
                        .into_any_element(),
                    None if !saved => kit::text(crate::i18n::tr("not-saved"), 13.0, theme.caution)
                        .into_any_element(),
                    None => kit::text(crate::i18n::tr("unnamed"), 13.0, theme.text_tertiary)
                        .into_any_element(),
                }
            };
            // The saved key is authoritative immediately after a replacement;
            // the worker may still be showing its previous snapshot.
            let masked_key = hint.unwrap_or_else(|| "—".into());
            let spending = key_snapshot
                .and_then(available_key_spending)
                .filter(|_| saved);
            let spend: AnyElement = match spending {
                Some(spending) => {
                    let used = spending.used_microusd;
                    let bar =
                        if let Some(limit) = spending.limit_microusd.filter(|limit| *limit > 0) {
                            Some((
                                used as f64 / limit as f64,
                                crate::i18n::format(
                                    "of-the-limit",
                                    &[
                                        ("v0", (money(used)).to_string()),
                                        ("v1", (money(limit)).to_string()),
                                    ],
                                ),
                            ))
                        } else {
                            credit_total.map(|total| {
                                (
                                    used as f64 / total as f64,
                                    crate::i18n::format(
                                        "no-spend-limit-key-spend-account-credits-purchased",
                                        &[
                                            ("v0", (money(used)).to_string()),
                                            ("v1", (money(total)).to_string()),
                                        ],
                                    ),
                                )
                            })
                        };
                    let mut cell = div().flex().items_center().justify_end().gap(px(8.0));
                    if let Some((fraction, tooltip)) = bar {
                        let fraction = fraction.clamp(0.0, 1.0) as f32;
                        let track = div()
                            .w(px(64.0))
                            .h(px(6.0))
                            .rounded_full()
                            .bg(theme.control_strong.opacity(0.35))
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(fraction.max(0.03)))
                                    .rounded_full()
                                    .bg(theme.accent),
                            )
                            .into_any_element();
                        cell = cell.child(kit::tooltip_host(
                            k,
                            format!("key-spend-{key_id}"),
                            tooltip,
                            track,
                        ));
                    }
                    cell.child(kit::text(money(used), 13.0, theme.text))
                        .into_any_element()
                }
                None => kit::text("—", 13.0, theme.text_tertiary)
                    .text_right()
                    .into_any_element(),
            };
            // Limits are stable metadata and remain useful when a usage poll fails.
            let limit: AnyElement = match key_snapshot
                .filter(|_| saved)
                .map(|key| key.spending.limit_microusd)
            {
                Some(Some(limit)) => kit::text(money(limit), 13.0, theme.text).into_any_element(),
                Some(None) => {
                    kit::text(crate::i18n::tr("none"), 13.0, theme.text_tertiary).into_any_element()
                }
                None => kit::text("—", 13.0, theme.text_tertiary).into_any_element(),
            };
            let mut items =
                vec![MenuItem::new(crate::i18n::tr("rename-key")).icon("pencil-simple-fill")];
            if !(saved || read_error.is_some()) {
                items.push(MenuItem::new(crate::i18n::tr("add-key")).icon("plus-bold"));
            }
            items.push(
                MenuItem::new(crate::i18n::tr("remove-key"))
                    .icon("trash-fill")
                    .danger(),
            );
            let actions: Vec<&'static str> = if saved || read_error.is_some() {
                vec!["rename", "remove"]
            } else {
                vec!["rename", "add", "remove"]
            };
            let menu_account = account.id.clone();
            let menu_key = key_id.clone();
            let local_name = account
                .api_key_names
                .get(key_id)
                .cloned()
                .unwrap_or_default();
            let menu = kit::more_menu(
                k,
                format!("key-{}-{key_id}", account.id),
                items,
                Self::h(cx, move |this, choice: usize, window, cx| {
                    let dialog = match actions.get(choice).copied() {
                        Some("rename") => ProviderDialog::with_name(
                            ProviderDialogKind::RenameOpenRouterApiKey {
                                account_id: menu_account.clone(),
                                key_id: menu_key.clone(),
                            },
                            local_name.clone(),
                        ),
                        Some("remove") => {
                            ProviderDialog::new(ProviderDialogKind::RemoveOpenRouterApiKey {
                                account_id: menu_account.clone(),
                                key_id: menu_key.clone(),
                            })
                        }
                        Some("add") => ProviderDialog::new(ProviderDialogKind::OpenRouterApiKey {
                            account_id: menu_account.clone(),
                            key_id: Some(menu_key.clone()),
                        }),
                        _ => return,
                    };
                    this.open_provider_dialog(dialog, window, cx)
                }),
            );
            table = table.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .h(px(44.0))
                    .border_t_1()
                    .border_color(theme.divider)
                    .child(div().flex_1().min_w_0().child(name))
                    .child(div().w(px(120.0)).flex_none().child(masked(k, masked_key)))
                    .child(div().w(px(140.0)).flex_none().child(spend))
                    .child(
                        div()
                            .w(px(64.0))
                            .flex_none()
                            .flex()
                            .justify_end()
                            .child(limit),
                    )
                    .child(menu),
            );
        }
        table.into_any_element()
    }

    /// Commit handler for dialog fields: Enter submits the dialog.
    pub(in super::super) fn dialog_submit_handler()
    -> impl Fn(&mut Self, String, InputEvent, &mut Window, &mut Context<Self>) + 'static {
        |this, _, event, window, cx| {
            if event == InputEvent::Submit {
                this.submit_provider_dialog(window, cx);
            }
        }
    }
}
