use super::persistence::persist_update;
use super::platform::choose_provider_folder;
use super::*;

static CODEX_PATH_SAVE_GEN: AtomicU64 = AtomicU64::new(0);
static CLAUDE_PATH_SAVE_GEN: AtomicU64 = AtomicU64::new(0);
static CURSOR_PATH_SAVE_GEN: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, PartialEq)]
pub(super) struct ProviderInstallStatus {
    app: Option<String>,
    cli: Option<String>,
    used: Option<ProviderInstallSource>,
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
            (
                app.map(|path| path.display().to_string()),
                cli.map(|path| path.display().to_string()),
                used,
            )
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
                app.map(|path| path.display().to_string()),
                cli.map(|path| path.display().to_string()),
                used,
            )
        }
        ProviderKind::Cursor => {
            let app = crate::cursor::installation_path(configured_folder);
            let used = app.as_ref().map(|_| ProviderInstallSource::App);
            (app.map(|path| path.display().to_string()), None, used)
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
    };
    ProviderInstallStatus {
        app,
        cli,
        used,
        cli_applicable: matches!(provider, ProviderKind::Codex | ProviderKind::Claude),
        checking: false,
    }
}

fn provider_install_status_card(status: &ProviderInstallStatus) -> Element {
    if status.checking {
        return border(
            text_block("Checking installed app and CLI…")
                .font_size(12.0)
                .opacity(0.72),
        )
        .padding(settings_card_padding())
        .background(ThemeRef::SubtleFill)
        .corner_radius(6.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into();
    }
    let status_line =
        |label: &str, path: Option<&String>, used: bool, unavailable: bool| -> Element {
            let mut title = Vec::<Element>::new();
            if used {
                title.push(
                    crate::icons::element("check-circle-fill", 15.0, Color::rgb(65, 184, 131))
                        .vertical_alignment(VerticalAlignment::Center),
                );
            }
            title.push(
                text_block(format!("{label}:"))
                    .font_size(12.0)
                    .bold()
                    .into(),
            );
            let detail = if unavailable {
                "Not applicable".into()
            } else {
                path.cloned().unwrap_or_else(|| "Not found".into())
            };
            vstack((
                hstack(title).spacing(5.0),
                text_block(detail).font_size(12.0).opacity(0.72).wrap(),
            ))
            .spacing(2.0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .into()
        };
    border(
        vstack((
            status_line(
                "Desktop App",
                status.app.as_ref(),
                status.used == Some(ProviderInstallSource::App),
                false,
            ),
            status_line(
                "CLI",
                status.cli.as_ref(),
                status.used == Some(ProviderInstallSource::Cli),
                !status.cli_applicable,
            ),
        ))
        .spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(settings_card_padding())
    .background(ThemeRef::SubtleFill)
    .corner_radius(6.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn opencode_credentials_card(
    provider: ProviderKind,
    key_input: &str,
    set_key_input: SetState<String>,
    settings_tx: Sender<Settings>,
) -> Element {
    let provider_name = provider.display_name();
    let manual_key_saved = crate::opencode::key_is_configured(provider);
    let detected = crate::opencode::is_installed(provider);
    let source = if manual_key_saved {
        "Saved in Windows user storage."
    } else if detected {
        "Using OpenCode auth or local history."
    } else {
        "No key or local history yet."
    };
    let save_input = key_input.to_owned();
    let save_setter = set_key_input.clone();
    let save_tx = settings_tx.clone();
    let clear_setter = set_key_input.clone();
    let clear_tx = settings_tx;
    border(
        vstack((
            text_block(format!("{provider_name} API key"))
                .font_size(12.0)
                .bold(),
            text_block(source).font_size(11.0).opacity(0.72).wrap(),
            PasswordBox::new()
                .placeholder_text("Paste an API key (optional)")
                .on_password_changed(set_key_input)
                .height(32.0),
            hstack((
                Button::new("Save key").on_click(move || {
                    let value = save_input.trim().to_owned();
                    if value.is_empty() {
                        crate::notifications::show(
                            "OpenCode key not saved",
                            "Paste an API key before saving it.",
                        );
                        return;
                    }
                    persist_opencode_manual_key(
                        provider,
                        Some(value),
                        save_setter.clone(),
                        save_tx.clone(),
                    );
                }),
                Button::new("Clear key").on_click(move || {
                    persist_opencode_manual_key(
                        provider,
                        None,
                        clear_setter.clone(),
                        clear_tx.clone(),
                    );
                }),
            ))
            .spacing(8.0),
        ))
        .spacing(6.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(settings_card_padding())
    .background(ThemeRef::SubtleFill)
    .corner_radius(6.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn opencode_detection_card(provider: ProviderKind) -> Element {
    let detected = crate::opencode::is_installed(provider);
    let status = if detected {
        "Found in OpenCode auth, environment, a saved key, or local history."
    } else {
        "No credential or local history yet."
    };
    border(
        vstack((
            text_block("OpenCode local source").font_size(12.0).bold(),
            text_block(status).font_size(11.0).opacity(0.72).wrap(),
        ))
        .spacing(2.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(settings_card_padding())
    .background(ThemeRef::SubtleFill)
    .corner_radius(6.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn openrouter_accounts_card(
    accounts: &[OpenRouterAccount],
    set_accounts: SetState<Vec<OpenRouterAccount>>,
    key_inputs: &HashMap<String, String>,
    set_key_inputs: SetState<HashMap<String, String>>,
    management_inputs: &HashMap<String, String>,
    set_management_inputs: SetState<HashMap<String, String>>,
    settings_tx: Sender<Settings>,
) -> Element {
    let mut account_cards: Vec<Element> = Vec::new();
    for account in accounts {
        let account_id = account.id.clone();
        let account_name = account.name.clone();
        let mut api_key_cards: Vec<Element> = Vec::new();
        for (key_index, key_id) in account.api_key_ids.iter().enumerate() {
            let input_id = format!("{}:{key_id}", account.id);
            let input_value = key_inputs.get(&input_id).cloned().unwrap_or_default();
            let key_saved = crate::openrouter::api_key_is_configured(&account.id, key_id);
            let save_account_id = account.id.clone();
            let save_key_id = key_id.clone();
            let save_input = input_value.clone();
            let save_input_id = input_id.clone();
            let save_inputs = key_inputs.clone();
            let save_input_setter = set_key_inputs.clone();
            let save_tx = settings_tx.clone();
            let clear_account_id = account.id.clone();
            let clear_key_id = key_id.clone();
            let clear_input_id = input_id.clone();
            let clear_inputs = key_inputs.clone();
            let clear_input_setter = set_key_inputs.clone();
            let clear_tx = settings_tx.clone();
            let remove_account_id = account.id.clone();
            let remove_key_id = key_id.clone();
            let remove_setter = set_accounts.clone();
            let remove_tx = settings_tx.clone();
            api_key_cards.push(
                border(
                    vstack((
                        text_block(format!("API key {}", key_index + 1))
                            .font_size(12.0)
                            .bold(),
                        text_block(if key_saved {
                            "Saved in Windows user storage."
                        } else {
                            "No API key yet."
                        })
                        .font_size(11.0)
                        .opacity(0.72)
                        .wrap(),
                        PasswordBox::new()
                            .value(input_value)
                            .placeholder_text(if key_saved {
                                "Enter a replacement API key"
                            } else {
                                "Paste an OpenRouter API key"
                            })
                            .on_password_changed({
                                let input_id = input_id.clone();
                                let inputs = key_inputs.clone();
                                let setter = set_key_inputs.clone();
                                move |value: String| {
                                    let mut next = inputs.clone();
                                    next.insert(input_id.clone(), value);
                                    setter.call(next);
                                }
                            })
                            .height(32.0),
                        hstack((
                            Button::new("Save key").on_click(move || {
                                let value = save_input.trim().to_owned();
                                if value.is_empty() {
                                    crate::notifications::show(
                                        "OpenRouter key not saved",
                                        "Paste an API key before saving it.",
                                    );
                                    return;
                                }
                                persist_openrouter_api_key(
                                    save_account_id.clone(),
                                    save_key_id.clone(),
                                    Some(value),
                                    save_input_id.clone(),
                                    save_inputs.clone(),
                                    save_input_setter.clone(),
                                    save_tx.clone(),
                                );
                            }),
                            Button::new("Clear").on_click(move || {
                                persist_openrouter_api_key(
                                    clear_account_id.clone(),
                                    clear_key_id.clone(),
                                    None,
                                    clear_input_id.clone(),
                                    clear_inputs.clone(),
                                    clear_input_setter.clone(),
                                    clear_tx.clone(),
                                );
                            }),
                            Button::new("Remove").on_click(move || {
                                if let Err(error) = crate::openrouter::save_account_api_key(
                                    &remove_account_id,
                                    &remove_key_id,
                                    None,
                                ) {
                                    crate::notifications::show(
                                        "OpenRouter key not removed",
                                        &format!("{error:#}"),
                                    );
                                    return;
                                }
                                // Mutate the persisted account by stable id so a
                                // stale UI snapshot cannot reassign the key row
                                // to a neighboring account after list shifts.
                                let account_id = remove_account_id.clone();
                                let key_id = remove_key_id.clone();
                                mutate_openrouter_accounts(
                                    remove_setter.clone(),
                                    remove_tx.clone(),
                                    move |accounts| {
                                        let Some(account) = accounts
                                            .iter_mut()
                                            .find(|account| account.id == account_id)
                                        else {
                                            return false;
                                        };
                                        let before = account.api_key_ids.len();
                                        account.api_key_ids.retain(|id| id != &key_id);
                                        account.api_key_ids.len() != before
                                    },
                                );
                            }),
                        ))
                        .spacing(8.0),
                    ))
                    .spacing(6.0)
                    .horizontal_alignment(HorizontalAlignment::Stretch),
                )
                .padding(settings_card_padding())
                .background(ThemeRef::SubtleFill)
                .corner_radius(6.0)
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .with_key(format!("openrouter-api-card-{}-{key_id}", account.id))
                .into(),
            );
        }

        let management_input = management_inputs
            .get(&account.id)
            .cloned()
            .unwrap_or_default();
        let management_saved = crate::openrouter::management_key_is_configured(&account.id);
        let save_management_account_id = account.id.clone();
        let save_management_input = management_input.clone();
        let save_management_inputs = management_inputs.clone();
        let save_management_input_setter = set_management_inputs.clone();
        let save_management_tx = settings_tx.clone();
        let clear_management_account_id = account.id.clone();
        let clear_management_inputs = management_inputs.clone();
        let clear_management_input_setter = set_management_inputs.clone();
        let clear_management_tx = settings_tx.clone();
        let add_key_account_id = account.id.clone();
        let add_key_setter = set_accounts.clone();
        let add_key_tx = settings_tx.clone();
        let remove_account = account.clone();
        let remove_account_setter = set_accounts.clone();
        let remove_account_tx = settings_tx.clone();
        let rename_account_id = account.id.clone();
        let rename_setter = set_accounts.clone();
        let rename_tx = settings_tx.clone();
        account_cards.push(
            border(
                vstack((
                    hstack((
                        text_block("Account name").font_size(12.0).bold(),
                        Button::new("Remove account").on_click(move || {
                            for key_id in &remove_account.api_key_ids {
                                if let Err(error) = crate::openrouter::save_account_api_key(
                                    &remove_account.id,
                                    key_id,
                                    None,
                                ) {
                                    crate::notifications::show(
                                        "OpenRouter account not removed",
                                        &format!("{error:#}"),
                                    );
                                    return;
                                }
                            }
                            if let Err(error) =
                                crate::openrouter::save_management_key(&remove_account.id, None)
                            {
                                crate::notifications::show(
                                    "OpenRouter account not removed",
                                    &format!("{error:#}"),
                                );
                                return;
                            }
                            let removed_id = remove_account.id.clone();
                            mutate_openrouter_accounts(
                                remove_account_setter.clone(),
                                remove_account_tx.clone(),
                                move |accounts| {
                                    let before = accounts.len();
                                    accounts.retain(|account| account.id != removed_id);
                                    accounts.len() != before
                                },
                            );
                        }),
                    ))
                    .spacing(8.0)
                    .horizontal_alignment(HorizontalAlignment::Stretch),
                    text_box(account_name)
                        .placeholder_text("OpenRouter account name")
                        .on_commit(move |value: String| {
                            let account_id = rename_account_id.clone();
                            mutate_openrouter_accounts(
                                rename_setter.clone(),
                                rename_tx.clone(),
                                move |accounts| {
                                    let Some(account) = accounts
                                        .iter_mut()
                                        .find(|account| account.id == account_id)
                                    else {
                                        return false;
                                    };
                                    if account.name == value {
                                        return false;
                                    }
                                    account.name = value;
                                    true
                                },
                            );
                        })
                        .height(32.0),
                    text_block(if management_saved {
                        "Used for the account credit balance."
                    } else {
                        "Needed to show the credit balance."
                    })
                    .font_size(11.0)
                    .opacity(0.72)
                    .wrap(),
                    PasswordBox::new()
                        .value(management_input)
                        .placeholder_text(if management_saved {
                            "Enter a replacement management key"
                        } else {
                            "Paste a management key (optional)"
                        })
                        .on_password_changed({
                            let account_id = account.id.clone();
                            let inputs = management_inputs.clone();
                            let setter = set_management_inputs.clone();
                            move |value: String| {
                                let mut next = inputs.clone();
                                next.insert(account_id.clone(), value);
                                setter.call(next);
                            }
                        })
                        .height(32.0),
                    hstack((
                        Button::new("Save management key").on_click(move || {
                            let value = save_management_input.trim().to_owned();
                            if value.is_empty() {
                                crate::notifications::show(
                                    "OpenRouter management key not saved",
                                    "Paste a management key before saving it.",
                                );
                                return;
                            }
                            persist_openrouter_management_key(
                                save_management_account_id.clone(),
                                Some(value),
                                save_management_inputs.clone(),
                                save_management_input_setter.clone(),
                                save_management_tx.clone(),
                            );
                        }),
                        Button::new("Clear management key").on_click(move || {
                            persist_openrouter_management_key(
                                clear_management_account_id.clone(),
                                None,
                                clear_management_inputs.clone(),
                                clear_management_input_setter.clone(),
                                clear_management_tx.clone(),
                            );
                        }),
                    ))
                    .spacing(8.0),
                    vstack(api_key_cards).spacing(8.0).with_layout_animation(
                        LayoutAnimationConfig::linear(duration(CONTROL_NORMAL_ANIMATION))
                            .animate_size(true),
                    ),
                    Button::new("Add API key").on_click(move || {
                        let account_id = add_key_account_id.clone();
                        mutate_openrouter_accounts(
                            add_key_setter.clone(),
                            add_key_tx.clone(),
                            move |accounts| {
                                let Some(account) =
                                    accounts.iter_mut().find(|account| account.id == account_id)
                                else {
                                    return false;
                                };
                                account
                                    .api_key_ids
                                    .push(OpenRouterAccount::new_api_key_id());
                                true
                            },
                        );
                    }),
                ))
                .spacing(8.0)
                .horizontal_alignment(HorizontalAlignment::Stretch),
            )
            .padding(settings_card_padding())
            .background(ThemeRef::CardBackground)
            .corner_radius(8.0)
            .border_thickness(Thickness::uniform(1.0))
            .border_brush(ThemeRef::CardStroke)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .with_key(format!("openrouter-account-card-{account_id}"))
            .into(),
        );
    }

    let add_account_setter = set_accounts;
    let add_account_tx = settings_tx;
    border(
        vstack((
            text_block("OpenRouter accounts").font_size(12.0).bold(),
            text_block(
                "API keys show per-key usage. A management key shows the shared credit balance.",
            )
            .font_size(11.0)
            .opacity(0.72)
            .wrap(),
            vstack(account_cards).spacing(10.0).with_layout_animation(
                LayoutAnimationConfig::linear(duration(CONTROL_NORMAL_ANIMATION))
                    .animate_size(true),
            ),
            Button::new("Add account").on_click(move || {
                mutate_openrouter_accounts(
                    add_account_setter.clone(),
                    add_account_tx.clone(),
                    move |accounts| {
                        let next_index = accounts.len() + 1;
                        accounts.push(OpenRouterAccount::new(format!("Account {next_index}")));
                        true
                    },
                );
            }),
        ))
        .spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(settings_card_padding())
    .background(ThemeRef::SubtleFill)
    .corner_radius(6.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .with_layout_animation(
        LayoutAnimationConfig::linear(duration(CONTROL_NORMAL_ANIMATION)).animate_size(true),
    )
    .into()
}

/// Apply an OpenRouter account-list mutation against the on-disk settings, never
/// against a stale UI snapshot. Account membership is always addressed by
/// stable account id so list shifts cannot move API keys between accounts.
///
/// The mutator returns `true` when it changed anything; no-ops skip the
/// credentials revision bump so workers are not restarted for free.
pub(super) fn mutate_openrouter_accounts(
    setter: SetState<Vec<OpenRouterAccount>>,
    settings_tx: Sender<Settings>,
    mutate: impl FnOnce(&mut Vec<OpenRouterAccount>) -> bool + 'static,
) {
    persist_update(settings_tx, move |settings| {
        // Include the synthetic legacy account when present so edits land on the
        // same identities the Settings UI is showing.
        let mut accounts = crate::openrouter::accounts_for_settings(settings);
        if !mutate(&mut accounts) {
            return;
        }
        settings.openrouter_accounts = accounts;
        settings.openrouter_credentials_revision =
            settings.openrouter_credentials_revision.wrapping_add(1);
        setter.call(crate::openrouter::accounts_for_settings(settings));
    });
}

fn persist_openrouter_api_key(
    account_id: String,
    key_id: String,
    value: Option<String>,
    input_id: String,
    mut inputs: HashMap<String, String>,
    input_setter: SetState<HashMap<String, String>>,
    settings_tx: Sender<Settings>,
) {
    if let Err(error) =
        crate::openrouter::save_account_api_key(&account_id, &key_id, value.as_deref())
    {
        eprintln!("failed to save OpenRouter API key: {error:#}");
        crate::notifications::show("OpenRouter key not saved", &format!("{error:#}"));
        return;
    }
    inputs.remove(&input_id);
    input_setter.call(inputs);
    persist_update(settings_tx, |settings| {
        settings.openrouter_credentials_revision =
            settings.openrouter_credentials_revision.wrapping_add(1);
    });
}

fn persist_openrouter_management_key(
    account_id: String,
    value: Option<String>,
    mut inputs: HashMap<String, String>,
    input_setter: SetState<HashMap<String, String>>,
    settings_tx: Sender<Settings>,
) {
    if let Err(error) = crate::openrouter::save_management_key(&account_id, value.as_deref()) {
        eprintln!("failed to save OpenRouter management key: {error:#}");
        crate::notifications::show("OpenRouter management key not saved", &format!("{error:#}"));
        return;
    }
    inputs.remove(&account_id);
    input_setter.call(inputs);
    persist_update(settings_tx, |settings| {
        settings.openrouter_credentials_revision =
            settings.openrouter_credentials_revision.wrapping_add(1);
    });
}

fn persist_opencode_manual_key(
    provider: ProviderKind,
    value: Option<String>,
    input_setter: SetState<String>,
    settings_tx: Sender<Settings>,
) {
    let result = crate::opencode::save_manual_key(provider, value.as_deref());
    if let Err(error) = result {
        eprintln!(
            "failed to save {} manual key: {error:#}",
            provider.display_name()
        );
        crate::notifications::show("OpenCode key not saved", &format!("{error:#}"));
        return;
    }
    input_setter.call(String::new());
    persist_update(settings_tx, move |settings| match provider {
        ProviderKind::OpenCodeZen => {
            settings.opencode_zen_credentials_revision =
                settings.opencode_zen_credentials_revision.wrapping_add(1);
        }
        ProviderKind::OpenCodeGo => {
            settings.opencode_go_credentials_revision =
                settings.opencode_go_credentials_revision.wrapping_add(1);
        }
        _ => {}
    });
}

fn persist_provider_folder(provider: ProviderKind, value: String, settings_tx: Sender<Settings>) {
    let generation = match provider {
        ProviderKind::Codex => &CODEX_PATH_SAVE_GEN,
        ProviderKind::Claude => &CLAUDE_PATH_SAVE_GEN,
        ProviderKind::Cursor => &CURSOR_PATH_SAVE_GEN,
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo | ProviderKind::OpenRouter => return,
    };
    let revision = generation.fetch_add(1, Ordering::Relaxed) + 1;
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        let generation = match provider {
            ProviderKind::Codex => &CODEX_PATH_SAVE_GEN,
            ProviderKind::Claude => &CLAUDE_PATH_SAVE_GEN,
            ProviderKind::Cursor => &CURSOR_PATH_SAVE_GEN,
            ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo | ProviderKind::OpenRouter => {
                return;
            }
        };
        if generation.load(Ordering::Relaxed) != revision {
            return;
        }
        let folder = (!value.trim().is_empty()).then(|| PathBuf::from(value.trim()));
        persist_update(settings_tx, move |settings| match provider {
            ProviderKind::Codex => settings.codex_path = folder,
            ProviderKind::Claude => settings.claude_path = folder,
            ProviderKind::Cursor => settings.cursor_path = folder,
            ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo | ProviderKind::OpenRouter => {}
        });
    });
}

fn persist_provider_enabled(
    setter: SetState<bool>,
    widgets_setter: SetState<Vec<TrayWidget>>,
    settings_tx: Sender<Settings>,
    provider: ProviderKind,
    enabled: bool,
    other_provider_enabled: bool,
    cursor_enabled: bool,
    widgets: Vec<TrayWidget>,
) {
    setter.call(enabled);
    let _ = (other_provider_enabled, cursor_enabled);
    widgets_setter.call(widgets);
    persist_update(settings_tx, move |settings| {
        settings.providers.set_enabled(provider, enabled);
    });
}

fn persist_cursor_enabled(
    setter: SetState<bool>,
    widgets_setter: SetState<Vec<TrayWidget>>,
    settings_tx: Sender<Settings>,
    enabled: bool,
    codex_enabled: bool,
    claude_enabled: bool,
    widgets: Vec<TrayWidget>,
) {
    setter.call(enabled);
    let _ = (codex_enabled, claude_enabled);
    widgets_setter.call(widgets);
    persist_update(settings_tx, move |settings| {
        settings
            .providers
            .set_enabled(ProviderKind::Cursor, enabled);
    });
}

pub(super) fn provider_page_content(
    provider: ProviderKind,
    ctx: &SettingsPageContext<'_>,
) -> Element {
    let codex_enabled = ctx.codex_enabled;
    let claude_enabled = ctx.claude_enabled;
    let cursor_enabled = ctx.cursor_enabled;
    let opencode_zen_enabled = ctx.opencode_zen_enabled;
    let opencode_go_enabled = ctx.opencode_go_enabled;
    let openrouter_enabled = ctx.openrouter_enabled;
    let codex_path = ctx.codex_path;
    let claude_path = ctx.claude_path;
    let cursor_path = ctx.cursor_path;
    let codex_install_status = ctx.codex_install_status;
    let claude_install_status = ctx.claude_install_status;
    let cursor_install_status = ctx.cursor_install_status;
    let opencode_zen_install_status = ctx.opencode_zen_install_status;
    let opencode_go_install_status = ctx.opencode_go_install_status;
    let openrouter_install_status = ctx.openrouter_install_status;
    let opencode_zen_key_input = ctx.opencode_zen_key_input;
    let opencode_go_key_input = ctx.opencode_go_key_input;
    let openrouter_accounts = ctx.openrouter_accounts;
    let openrouter_key_inputs = ctx.openrouter_key_inputs;
    let openrouter_management_inputs = ctx.openrouter_management_inputs;
    let tray_widgets = ctx.tray_widgets;
    let hovered_card_id = ctx.hovered_card_id;
    let set_codex_enabled = ctx.set_codex_enabled.clone();
    let set_claude_enabled = ctx.set_claude_enabled.clone();
    let set_cursor_enabled = ctx.set_cursor_enabled.clone();
    let set_opencode_zen_enabled = ctx.set_opencode_zen_enabled.clone();
    let set_opencode_go_enabled = ctx.set_opencode_go_enabled.clone();
    let set_openrouter_enabled = ctx.set_openrouter_enabled.clone();
    let set_opencode_zen_key_input = ctx.set_opencode_zen_key_input.clone();
    let set_opencode_go_key_input = ctx.set_opencode_go_key_input.clone();
    let set_openrouter_accounts = ctx.set_openrouter_accounts.clone();
    let set_openrouter_key_inputs = ctx.set_openrouter_key_inputs.clone();
    let set_openrouter_management_inputs = ctx.set_openrouter_management_inputs.clone();
    let set_codex_path = ctx.set_codex_path.clone();
    let set_claude_path = ctx.set_claude_path.clone();
    let set_cursor_path = ctx.set_cursor_path.clone();
    let set_tray_widgets = ctx.set_tray_widgets.clone();
    let set_hovered_card_id = ctx.set_hovered_card_id.clone();
    let settings_tx = ctx.settings_tx.clone();
    let apply_codex_enabled = settings_tx.clone();
    let apply_claude_enabled = settings_tx.clone();
    let apply_cursor_enabled = settings_tx.clone();
    let apply_codex_path = settings_tx.clone();
    let apply_claude_path = settings_tx.clone();
    let apply_cursor_path = settings_tx.clone();
    let tray_widgets_for_codex_toggle = tray_widgets.to_vec();
    let tray_widgets_for_claude_toggle = tray_widgets.to_vec();
    let tray_widgets_for_cursor_toggle = tray_widgets.to_vec();
    let tray_widgets_for_opencode_toggle = tray_widgets.to_vec();
    let tray_widget_setter_for_codex_toggle = set_tray_widgets.clone();
    let tray_widget_setter_for_claude_toggle = set_tray_widgets.clone();
    let tray_widget_setter_for_cursor_toggle = set_tray_widgets.clone();
    let tray_widget_setter_for_opencode_toggle = set_tray_widgets.clone();
    let apply_opencode_zen_enabled = settings_tx.clone();
    let apply_opencode_go_enabled = settings_tx.clone();
    let apply_openrouter_enabled = settings_tx.clone();
    let settings_tx_for_details = settings_tx.clone();

    let enable_card = match provider {
        ProviderKind::Codex => settings_toggle_card_with_description(
            "Enabled",
            Some("Reads the signed-in Codex CLI or desktop app."),
            codex_enabled,
            move |value| {
                persist_provider_enabled(
                    set_codex_enabled.clone(),
                    tray_widget_setter_for_codex_toggle.clone(),
                    apply_codex_enabled.clone(),
                    ProviderKind::Codex,
                    value,
                    claude_enabled,
                    cursor_enabled,
                    tray_widgets_for_codex_toggle.clone(),
                )
            },
            "provider-codex-enabled",
            hovered_card_id,
            set_hovered_card_id.clone(),
        ),
        ProviderKind::Claude => settings_toggle_card_with_description(
            "Enabled",
            Some("Reads your existing Claude Code login."),
            claude_enabled,
            move |value| {
                persist_provider_enabled(
                    set_claude_enabled.clone(),
                    tray_widget_setter_for_claude_toggle.clone(),
                    apply_claude_enabled.clone(),
                    ProviderKind::Claude,
                    value,
                    codex_enabled,
                    cursor_enabled,
                    tray_widgets_for_claude_toggle.clone(),
                )
            },
            "provider-claude-enabled",
            hovered_card_id,
            set_hovered_card_id.clone(),
        ),
        ProviderKind::Cursor => settings_toggle_card_with_description(
            "Enabled",
            Some("Reads the signed-in Cursor app for this billing cycle."),
            cursor_enabled,
            move |value| {
                persist_cursor_enabled(
                    set_cursor_enabled.clone(),
                    tray_widget_setter_for_cursor_toggle.clone(),
                    apply_cursor_enabled.clone(),
                    value,
                    codex_enabled,
                    claude_enabled,
                    tray_widgets_for_cursor_toggle.clone(),
                )
            },
            "provider-cursor-enabled",
            hovered_card_id,
            set_hovered_card_id.clone(),
        ),
        ProviderKind::OpenCodeZen => settings_toggle_card_with_description(
            "Enabled",
            Some("Reads Zen auth and local OpenCode history."),
            opencode_zen_enabled,
            move |value| {
                persist_provider_enabled(
                    set_opencode_zen_enabled.clone(),
                    tray_widget_setter_for_opencode_toggle.clone(),
                    apply_opencode_zen_enabled.clone(),
                    ProviderKind::OpenCodeZen,
                    value,
                    opencode_go_enabled,
                    false,
                    tray_widgets_for_opencode_toggle.clone(),
                )
            },
            "provider-opencode-zen-enabled",
            hovered_card_id,
            set_hovered_card_id.clone(),
        ),
        ProviderKind::OpenCodeGo => settings_toggle_card_with_description(
            "Enabled",
            Some("Reads Go quota windows and local OpenCode history."),
            opencode_go_enabled,
            move |value| {
                persist_provider_enabled(
                    set_opencode_go_enabled.clone(),
                    tray_widget_setter_for_opencode_toggle.clone(),
                    apply_opencode_go_enabled.clone(),
                    ProviderKind::OpenCodeGo,
                    value,
                    opencode_zen_enabled,
                    false,
                    tray_widgets_for_opencode_toggle.clone(),
                )
            },
            "provider-opencode-go-enabled",
            hovered_card_id,
            set_hovered_card_id.clone(),
        ),
        ProviderKind::OpenRouter => settings_toggle_card_with_description(
            "Enabled",
            Some(
                "Reads API-key usage and spend limits. A management key also shows credit balance.",
            ),
            openrouter_enabled,
            move |value| {
                persist_provider_enabled(
                    set_openrouter_enabled.clone(),
                    tray_widget_setter_for_opencode_toggle.clone(),
                    apply_openrouter_enabled.clone(),
                    ProviderKind::OpenRouter,
                    value,
                    opencode_zen_enabled,
                    opencode_go_enabled,
                    tray_widgets_for_opencode_toggle.clone(),
                )
            },
            "provider-openrouter-enabled",
            hovered_card_id,
            set_hovered_card_id.clone(),
        ),
    };

    let install_status = match provider {
        ProviderKind::Codex => codex_install_status,
        ProviderKind::Claude => claude_install_status,
        ProviderKind::Cursor => cursor_install_status,
        ProviderKind::OpenCodeZen => opencode_zen_install_status,
        ProviderKind::OpenCodeGo => opencode_go_install_status,
        ProviderKind::OpenRouter => openrouter_install_status,
    };

    let (path, path_label, path_description, placeholder) = match provider {
        ProviderKind::Codex => (
            codex_path,
            "Codex CLI folder (optional)",
            "Folder with codex.exe, codex.cmd, or codex.ps1. Leave empty to find it automatically.",
            r"C:\\Users\\you\\AppData\\Roaming\\npm",
        ),
        ProviderKind::Claude => (
            claude_path,
            "Claude Code CLI folder (optional)",
            "Folder with claude.exe, claude.cmd, or claude.ps1. Leave empty to find it automatically.",
            r"C:\\Users\\you\\AppData\\Roaming\\npm",
        ),
        ProviderKind::Cursor => (
            cursor_path,
            "Cursor app folder (optional)",
            "Folder with Cursor.exe. Leave empty to find it automatically. Usage still comes from the signed-in profile.",
            r"C:\\Users\\you\\AppData\\Local\\Programs\\Cursor",
        ),
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo | ProviderKind::OpenRouter => {
            ("", "", "", "")
        }
    };

    let codex_path_setter = set_codex_path.clone();
    let claude_path_setter = set_claude_path.clone();
    let cursor_path_setter = set_cursor_path.clone();
    let codex_path_tx = apply_codex_path.clone();
    let claude_path_tx = apply_claude_path.clone();
    let cursor_path_tx = apply_cursor_path.clone();

    let path_input: Element = match provider {
        ProviderKind::Codex => {
            let picker_setter = set_codex_path.clone();
            let picker_tx = apply_codex_path.clone();
            grid((
                text_box(path)
                    .placeholder_text(placeholder)
                    .on_commit(move |value: String| {
                        codex_path_setter.call(value.clone());
                        persist_provider_folder(ProviderKind::Codex, value, codex_path_tx.clone());
                    })
                    .height(32.0)
                    .grid_column(0),
                Button::new("")
                    .icon_path(crate::icons::data("fluent-folder"), "#E6E6E6")
                    .width(44.0)
                    .height(32.0)
                    .on_click(move || match choose_provider_folder() {
                        Ok(Some(folder)) => {
                            let value = folder.display().to_string();
                            picker_setter.call(value.clone());
                            persist_provider_folder(ProviderKind::Codex, value, picker_tx.clone());
                        }
                        Ok(None) => {}
                        Err(error) => eprintln!("failed to choose Codex folder: {error:#}"),
                    })
                    .grid_column(1),
            ))
            .columns([GridLength::Star(1.0), GridLength::Auto])
            .column_spacing(8.0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .into()
        }
        ProviderKind::Claude => {
            let picker_setter = set_claude_path.clone();
            let picker_tx = apply_claude_path.clone();
            grid((
                text_box(path)
                    .placeholder_text(placeholder)
                    .on_commit(move |value: String| {
                        claude_path_setter.call(value.clone());
                        persist_provider_folder(
                            ProviderKind::Claude,
                            value,
                            claude_path_tx.clone(),
                        );
                    })
                    .height(32.0)
                    .grid_column(0),
                Button::new("")
                    .icon_path(crate::icons::data("fluent-folder"), "#E6E6E6")
                    .width(44.0)
                    .height(32.0)
                    .on_click(move || match choose_provider_folder() {
                        Ok(Some(folder)) => {
                            let value = folder.display().to_string();
                            picker_setter.call(value.clone());
                            persist_provider_folder(ProviderKind::Claude, value, picker_tx.clone());
                        }
                        Ok(None) => {}
                        Err(error) => eprintln!("failed to choose Claude folder: {error:#}"),
                    })
                    .grid_column(1),
            ))
            .columns([GridLength::Star(1.0), GridLength::Auto])
            .column_spacing(8.0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .into()
        }
        ProviderKind::Cursor => {
            let picker_setter = set_cursor_path.clone();
            let picker_tx = apply_cursor_path.clone();
            grid((
                text_box(path)
                    .placeholder_text(placeholder)
                    .on_commit(move |value: String| {
                        cursor_path_setter.call(value.clone());
                        persist_provider_folder(
                            ProviderKind::Cursor,
                            value,
                            cursor_path_tx.clone(),
                        );
                    })
                    .height(32.0)
                    .grid_column(0),
                Button::new("")
                    .icon_path(crate::icons::data("fluent-folder"), "#E6E6E6")
                    .width(44.0)
                    .height(32.0)
                    .on_click(move || match choose_provider_folder() {
                        Ok(Some(folder)) => {
                            let value = folder.display().to_string();
                            picker_setter.call(value.clone());
                            persist_provider_folder(ProviderKind::Cursor, value, picker_tx.clone());
                        }
                        Ok(None) => {}
                        Err(error) => eprintln!("failed to choose Cursor folder: {error:#}"),
                    })
                    .grid_column(1),
            ))
            .columns([GridLength::Star(1.0), GridLength::Auto])
            .column_spacing(8.0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .into()
        }
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo | ProviderKind::OpenRouter => {
            Element::Empty
        }
    };

    let details: Element = if matches!(
        provider,
        ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo
    ) {
        let (key_input, set_key_input) = match provider {
            ProviderKind::OpenCodeZen => {
                (opencode_zen_key_input, set_opencode_zen_key_input.clone())
            }
            ProviderKind::OpenCodeGo => (opencode_go_key_input, set_opencode_go_key_input.clone()),
            _ => unreachable!("OpenCode credentials branch"),
        };
        vstack((
            opencode_detection_card(provider),
            opencode_credentials_card(
                provider,
                key_input,
                set_key_input,
                settings_tx_for_details.clone(),
            ),
        ))
        .spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Top)
        .into()
    } else if provider == ProviderKind::OpenRouter {
        vstack((
            settings_info_card(
                "OpenRouter source",
                if crate::openrouter::is_installed_for_accounts(openrouter_accounts) {
                    "Credentials saved"
                } else {
                    "No credentials yet"
                },
            ),
            openrouter_accounts_card(
                openrouter_accounts,
                set_openrouter_accounts.clone(),
                openrouter_key_inputs,
                set_openrouter_key_inputs.clone(),
                openrouter_management_inputs,
                set_openrouter_management_inputs.clone(),
                settings_tx_for_details.clone(),
            ),
        ))
        .spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Top)
        .into()
    } else {
        vstack((
            provider_install_status_card(install_status),
            vstack((
                text_block(path_label).font_size(12.0),
                text_block(path_description)
                    .font_size(11.0)
                    .opacity(0.72)
                    .wrap(),
                path_input,
            ))
            .spacing(3.0)
            .horizontal_alignment(HorizontalAlignment::Stretch),
        ))
        .spacing(8.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Top)
        .into()
    };

    let mut rows = vec![enable_card.with_key(format!("provider-{}-enabled", provider.id()))];
    rows.push(details.with_key(format!("provider-{}-details", provider.id())));

    let row_count = rows.len();
    let cards = vstack(rows)
        .spacing(4.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .with_key(format!("provider-{}-cards-{row_count}", provider.id()));
    grid((
        text_block(provider.display_name())
            .font_size(28.0)
            .bold()
            .grid_row(0),
        cards.grid_row(1),
    ))
    .columns([GridLength::Star(1.0)])
    .rows([GridLength::Auto, GridLength::Auto])
    .row_spacing(10.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Top)
    .into()
}
