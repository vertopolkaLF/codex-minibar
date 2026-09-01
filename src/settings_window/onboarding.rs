use super::persistence::replace_settings;
use super::platform::{close_open_window, is_open};
use super::shared::settings_section_heading;
use super::*;

pub(super) fn detected_providers(settings: &Settings) -> [bool; 6] {
    [
        crate::codex::is_installed(settings.codex_path.as_deref()),
        crate::claude::is_installed(settings.claude_path.as_deref()),
        crate::cursor::is_installed(settings.cursor_path.as_deref()),
        crate::opencode::is_installed(ProviderKind::OpenCodeZen),
        crate::opencode::is_installed(ProviderKind::OpenCodeGo),
        crate::openrouter::is_installed_for_accounts(&crate::openrouter::accounts_for_settings(
            settings,
        )),
    ]
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum OnboardingStep {
    #[default]
    Providers,
    General,
}

/// Compact first-launch surface. It deliberately reuses the same setting
/// controls as the full editor, but persists exactly once on Done.
pub(super) fn onboarding_render(
    cx: &mut RenderCx,
    settings: Arc<Settings>,
    detected: [bool; 6],
    settings_tx: Sender<Settings>,
) -> Element {
    let color_scheme = cx.use_color_scheme();
    cx.use_effect(color_scheme, move || {
        sync_settings_caption_button_theme(color_scheme);
    });
    let (step, set_step) = cx.use_state(OnboardingStep::default());
    let (codex_enabled, set_codex_enabled) = cx.use_state(detected[0]);
    let (claude_enabled, set_claude_enabled) = cx.use_state(detected[1]);
    let (cursor_enabled, set_cursor_enabled) = cx.use_state(detected[2]);
    let (opencode_zen_enabled, set_opencode_zen_enabled) = cx.use_state(detected[3]);
    let (opencode_go_enabled, set_opencode_go_enabled) = cx.use_state(detected[4]);
    let (openrouter_enabled, set_openrouter_enabled) = cx.use_state(detected[5]);
    let (start_at_login, set_start_at_login) = cx.use_state(settings.start_at_login);
    let (automatic_activation, set_automatic_activation) =
        cx.use_state(settings.automatic_activation);
    let (limit_refresh_interval, set_limit_refresh_interval) =
        cx.use_state(settings.limit_refresh_interval);
    let (show_used_percentage, set_show_used_percentage) =
        cx.use_state(settings.show_used_percentage);
    let (show_usage_pace, set_show_usage_pace) = cx.use_state(settings.show_usage_pace);
    let (show_account_name, set_show_account_name) = cx.use_state(settings.show_account_name);
    let (hovered_card_id, set_hovered_card_id) = cx.use_state(None::<String>);

    let (heading, description, cards): (&str, &str, Vec<Element>) = match step {
        OnboardingStep::Providers => (
            "Choose providers",
            "We turned on the providers found on this PC. You can change this later.",
            vec![
                settings_toggle_card_with_description(
                    "Codex",
                    Some(if detected[0] {
                        "Found on this PC."
                    } else {
                        "Not found. Turn it on if it's installed somewhere else."
                    }),
                    codex_enabled,
                    move |value| set_codex_enabled.call(value),
                    "onboarding-codex",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-codex"),
                settings_toggle_card_with_description(
                    "Claude",
                    Some(if detected[1] {
                        "Found on this PC."
                    } else {
                        "Not found. Turn it on if it's installed somewhere else."
                    }),
                    claude_enabled,
                    move |value| set_claude_enabled.call(value),
                    "onboarding-claude",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-claude"),
                settings_toggle_card_with_description(
                    "Cursor",
                    Some(if detected[2] {
                        "Found on this PC."
                    } else {
                        "Not found. Turn it on if it's installed somewhere else."
                    }),
                    cursor_enabled,
                    move |value| set_cursor_enabled.call(value),
                    "onboarding-cursor",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-cursor"),
                settings_toggle_card_with_description(
                    "OpenCode Zen",
                    Some(if detected[3] {
                        "Found in OpenCode auth or local history."
                    } else {
                        "Not found. Turn it on if it's set up elsewhere."
                    }),
                    opencode_zen_enabled,
                    move |value| set_opencode_zen_enabled.call(value),
                    "onboarding-opencode-zen",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-opencode-zen"),
                settings_toggle_card_with_description(
                    "OpenCode Go",
                    Some(if detected[4] {
                        "Found in OpenCode auth or local history."
                    } else {
                        "Not found. Turn it on if it's set up elsewhere."
                    }),
                    opencode_go_enabled,
                    move |value| set_opencode_go_enabled.call(value),
                    "onboarding-opencode-go",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-opencode-go"),
                settings_toggle_card_with_description(
                    "OpenRouter",
                    Some(if detected[5] {
                        "Account credentials are already set."
                    } else {
                        "Optional. Add accounts later in Providers."
                    }),
                    openrouter_enabled,
                    move |value| set_openrouter_enabled.call(value),
                    "onboarding-openrouter",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-openrouter"),
            ],
        ),
        OnboardingStep::General => (
            "General settings",
            "You can change these later in Settings.",
            vec![
                settings_section_heading("Startup").with_key("onboarding-startup-heading"),
                settings_toggle_card(
                    "Start with Windows",
                    start_at_login,
                    move |value| set_start_at_login.call(value),
                    "onboarding-start-at-login",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-start-at-login"),
                settings_section_heading("Features").with_key("onboarding-features-heading"),
                settings_toggle_card_with_description(
                    "Start 5-hour sessions automatically",
                    Some("Starts a new Codex or Claude session as soon as a window is available, instead of waiting for your first request."),
                    automatic_activation,
                    move |value| set_automatic_activation.call(value),
                    "onboarding-automatic-activation",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-automatic-activation"),
                settings_control_card(
                    "Refresh limits",
                    None,
                    ComboBox::new([
                        "30 seconds",
                        "1 minute",
                        "5 minutes",
                        "10 minutes",
                        "15 minutes",
                    ])
                    .selected_index(limit_refresh_interval.index())
                    .on_selection_changed(move |choice| {
                        set_limit_refresh_interval.call(LimitRefreshInterval::from_index(choice));
                    }),
                    "onboarding-limit-refresh-interval",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-limit-refresh-interval"),
                settings_section_heading("Customization")
                    .with_key("onboarding-customization-heading"),
                settings_toggle_card(
                    "Show used instead of remaining",
                    show_used_percentage,
                    move |value| set_show_used_percentage.call(value),
                    "onboarding-show-used",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-show-used"),
                settings_toggle_card_with_description(
                    "Show usage pace",
                    Some("Marks whether you're burning quota faster or slower than an even pace."),
                    show_usage_pace,
                    move |value| set_show_usage_pace.call(value),
                    "onboarding-show-usage-pace",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-show-usage-pace"),
                settings_toggle_card(
                    "Show account name",
                    show_account_name,
                    move |value| set_show_account_name.call(value),
                    "onboarding-show-account-name",
                    &hovered_card_id,
                    set_hovered_card_id.clone(),
                )
                .with_key("onboarding-show-account-name"),
            ],
        ),
    };

    let back_or_spacer: Element = match step {
        OnboardingStep::Providers => border(Element::Empty).width(72.0).into(),
        OnboardingStep::General => {
            let set_step = set_step.clone();
            Button::new("Back")
                .on_click(move || set_step.call(OnboardingStep::Providers))
                .into()
        }
    };
    let action: Element = match step {
        OnboardingStep::Providers => {
            let set_step = set_step.clone();
            Button::new("Continue")
                .accent()
                .on_click(move || set_step.call(OnboardingStep::General))
                .into()
        }
        OnboardingStep::General => {
            let settings_tx = settings_tx.clone();
            let settings = Arc::clone(&settings);
            Button::new("Done")
                .accent()
                .on_click(move || {
                    let mut completed = (*settings).clone();
                    completed.onboarding_completed = true;
                    completed.providers = crate::settings::ProviderSettings::from_enabled(
                        crate::provider_registry::PROVIDERS
                            .iter()
                            .filter(|provider| match provider.kind {
                                ProviderKind::Codex => codex_enabled,
                                ProviderKind::Claude => claude_enabled,
                                ProviderKind::Cursor => cursor_enabled,
                                ProviderKind::OpenCodeZen => opencode_zen_enabled,
                                ProviderKind::OpenCodeGo => opencode_go_enabled,
                                ProviderKind::OpenRouter => openrouter_enabled,
                            })
                            .map(|provider| provider.kind),
                    );
                    completed.tray_widgets = crate::provider_registry::PROVIDERS
                        .iter()
                        .filter(|provider| completed.providers.is_enabled(provider.kind))
                        .filter(|provider| !provider.default_tray_metrics.is_empty())
                        .map(|provider| TrayWidget::for_provider(provider.kind))
                        .collect();
                    completed.start_at_login = start_at_login;
                    completed.automatic_activation = automatic_activation;
                    completed.limit_refresh_interval = limit_refresh_interval;
                    completed.show_used_percentage = show_used_percentage;
                    completed.show_usage_pace = show_usage_pace;
                    completed.show_account_name = show_account_name;
                    if let Err(error) = replace_settings(settings_tx.clone(), completed) {
                        eprintln!("failed to complete onboarding: {error:#}");
                        return;
                    }
                    // The popup host shares this UI thread. Prepare it before
                    // dismissing onboarding so Done always lands on the popup.
                    if crate::popup::prepare_show_on_ui_thread() {
                        crate::popup::show_near_cursor();
                    }
                    close_open_window();
                })
                .into()
        }
    };

    let content = scroll_viewer(
        vstack((
            text_block(heading).font_size(28.0).font_weight(600),
            text_block(description).font_size(14.0).opacity(0.72).wrap(),
            vstack(cards).spacing(10.0),
        ))
        .spacing(16.0)
        .padding(Thickness {
            left: 32.0,
            top: 28.0,
            right: 32.0,
            bottom: 20.0,
        })
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .horizontal_scroll_bar_visibility(ScrollBarVisibility::Disabled)
    .vertical_scroll_bar_visibility(ScrollBarVisibility::Auto)
    .grid_row(0);
    let footer = border(
        hstack((back_or_spacer, action))
            .spacing(8.0)
            .horizontal_alignment(HorizontalAlignment::Right),
    )
    .padding(Thickness {
        left: 32.0,
        top: 14.0,
        right: 32.0,
        bottom: 18.0,
    })
    .border_thickness(Thickness {
        left: 0.0,
        top: 1.0,
        right: 0.0,
        bottom: 0.0,
    })
    .border_brush(ThemeRef::CardStroke)
    .grid_row(1);
    let title_bar = TitleBar::new(ONBOARDING_WINDOW_TITLE)
        .back_button_visible(false)
        .pane_toggle_button_visible(false)
        .tall(true);
    let body = grid((content, footer))
        .rows([GridLength::Star(1.0), GridLength::Auto])
        .columns([GridLength::Star(1.0)])
        .background(ThemeRef::LayerFill)
        .grid_row(1);
    grid((title_bar.grid_row(0), body))
        .rows([GridLength::Auto, GridLength::Star(1.0)])
        .columns([GridLength::Star(1.0)])
        .background(ThemeRef::LayerFill)
        .into()
}

/// Resetting returns to the same first-launch path as a new install. Wait for
/// the current native host to close before creating the onboarding host so the
/// two settings surfaces can never overlap or fight over the host slot.
pub(super) fn restart_onboarding_after_reset(
    settings_tx: Sender<Settings>,
    ui_dispatcher: UiMarshaller,
) {
    close_open_window();
    thread::spawn(move || {
        for _ in 0..20 {
            if !is_open() {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        ui_dispatcher.dispatch(move || {
            if let Err(error) = open_onboarding(settings_tx) {
                eprintln!("failed to reopen onboarding after settings reset: {error:?}");
            }
        });
    });
}
