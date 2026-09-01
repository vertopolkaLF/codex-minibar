use super::persistence::{persist_bool, persist_update};
use super::shared::{enabled_providers, settings_section_heading};
use super::*;

fn persist_popup_brick(
    current: &PopupVisibility,
    set_popup_visibility: SetState<PopupVisibility>,
    settings_tx: Sender<Settings>,
    brick_id: String,
    all_tab: bool,
    provider_tab: bool,
) {
    let mut next = current.clone();
    next.set_brick(brick_id.clone(), all_tab, provider_tab);
    set_popup_visibility.call(next);
    persist_update(settings_tx, move |settings| {
        settings
            .popup_visibility
            .set_brick(brick_id, all_tab, provider_tab);
    });
}

fn persist_popup_provider_all(
    current: &PopupVisibility,
    set_popup_visibility: SetState<PopupVisibility>,
    settings_tx: Sender<Settings>,
    provider: ProviderKind,
    show_on_all: bool,
) {
    let mut next = current.clone();
    next.set_provider_all_tab(provider, show_on_all);
    set_popup_visibility.call(next);
    persist_update(settings_tx, move |settings| {
        settings
            .popup_visibility
            .set_provider_all_tab(provider, show_on_all);
    });
}

pub(super) fn render(ctx: &SettingsPageContext<'_>) -> (&'static str, Vec<Element>) {
    let codex_enabled = ctx.codex_enabled;
    let claude_enabled = ctx.claude_enabled;
    let cursor_enabled = ctx.cursor_enabled;
    let opencode_zen_enabled = ctx.opencode_zen_enabled;
    let opencode_go_enabled = ctx.opencode_go_enabled;
    let openrouter_enabled = ctx.openrouter_enabled;
    let popup_order = ctx.popup_order;
    let use_colored_provider_icons = ctx.use_colored_provider_icons;
    let replace_chatgpt_logo_with_codex = ctx.replace_chatgpt_logo_with_codex;
    let show_used_percentage = ctx.show_used_percentage;
    let show_usage_pace = ctx.show_usage_pace;
    let show_account_name = ctx.show_account_name;
    let popup_visibility = ctx.popup_visibility;
    let discovered_popup_bricks = ctx.discovered_popup_bricks;
    let show_total_spend_on_all_tab = ctx.show_total_spend_on_all_tab;
    let total_spend_presentation = ctx.total_spend_presentation;
    let expanded_popup_provider = ctx.expanded_popup_provider;
    let set_use_colored_provider_icons = ctx.set_use_colored_provider_icons.clone();
    let set_replace_chatgpt_logo_with_codex = ctx.set_replace_chatgpt_logo_with_codex.clone();
    let set_show_used_percentage = ctx.set_show_used_percentage.clone();
    let set_show_usage_pace = ctx.set_show_usage_pace.clone();
    let set_show_account_name = ctx.set_show_account_name.clone();
    let set_popup_visibility = ctx.set_popup_visibility.clone();
    let set_show_total_spend_on_all_tab = ctx.set_show_total_spend_on_all_tab.clone();
    let set_total_spend_presentation = ctx.set_total_spend_presentation.clone();
    let set_expanded_popup_provider = ctx.set_expanded_popup_provider.clone();
    let hovered_card_id = ctx.hovered_card_id;
    let set_hovered_card_id = ctx.set_hovered_card_id.clone();
    let settings_tx = ctx.settings_tx.clone();
    let apply_use_colored_provider_icons = settings_tx.clone();
    let apply_replace_chatgpt_logo_with_codex = settings_tx.clone();
    let apply_show_used_percentage = settings_tx.clone();
    let apply_show_usage_pace = settings_tx.clone();
    let apply_show_account_name = settings_tx.clone();
    let providers: Vec<ProviderKind> = popup_order
        .iter()
        .filter_map(|widget| widget.as_provider())
        .collect();
    let enabled = enabled_providers(
        &providers,
        codex_enabled,
        claude_enabled,
        cursor_enabled,
        opencode_zen_enabled,
        opencode_go_enabled,
        openrouter_enabled,
    );
    let mut rows = vec![
        settings_section_heading("Tabs").with_key("customize-tabs-heading"),
        settings_toggle_card(
            "Use colored provider icons",
            use_colored_provider_icons,
            {
                let set_use_colored_provider_icons = set_use_colored_provider_icons.clone();
                let apply_use_colored_provider_icons = apply_use_colored_provider_icons.clone();
                move |value| {
                    persist_bool(
                        set_use_colored_provider_icons.clone(),
                        apply_use_colored_provider_icons.clone(),
                        value,
                        |settings, value| {
                            settings.use_colored_provider_icons = value;
                        },
                    );
                }
            },
            "customize-colored-icons",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("customize-colored-icons"),
    ];
    if codex_enabled {
        rows.push(
            settings_toggle_card(
                "Replace ChatGPT logo with Codex",
                replace_chatgpt_logo_with_codex,
                {
                    let set_replace_chatgpt_logo_with_codex =
                        set_replace_chatgpt_logo_with_codex.clone();
                    let apply_replace_chatgpt_logo_with_codex =
                        apply_replace_chatgpt_logo_with_codex.clone();
                    move |value| {
                        persist_bool(
                            set_replace_chatgpt_logo_with_codex.clone(),
                            apply_replace_chatgpt_logo_with_codex.clone(),
                            value,
                            |settings, value| {
                                settings.replace_chatgpt_logo_with_codex = value;
                            },
                        );
                    }
                },
                "customize-codex-logo",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("customize-codex-logo"),
        );
    }
    rows.extend([
        settings_section_heading("Cards").with_key("customize-cards-heading"),
        settings_toggle_card(
            "Show used instead of remaining",
            show_used_percentage,
            {
                let set_show_used_percentage = set_show_used_percentage.clone();
                let apply_show_used_percentage = apply_show_used_percentage.clone();
                move |value| {
                    persist_bool(
                        set_show_used_percentage.clone(),
                        apply_show_used_percentage.clone(),
                        value,
                        |settings, value| {
                            settings.show_used_percentage = value;
                        },
                    );
                }
            },
            "customize-show-used",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("customize-show-used"),
        settings_toggle_card_with_description(
            "Show usage pace",
            Some("Marks whether you're burning quota faster or slower than an even pace."),
            show_usage_pace,
            {
                let set_show_usage_pace = set_show_usage_pace.clone();
                let apply_show_usage_pace = apply_show_usage_pace.clone();
                move |value| {
                    persist_bool(
                        set_show_usage_pace.clone(),
                        apply_show_usage_pace.clone(),
                        value,
                        |settings, value| {
                            settings.show_usage_pace = value;
                        },
                    );
                }
            },
            "customize-show-usage-pace",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("customize-show-usage-pace"),
        settings_toggle_card(
            "Show account name",
            show_account_name,
            {
                let set_show_account_name = set_show_account_name.clone();
                let apply_show_account_name = apply_show_account_name.clone();
                move |value| {
                    persist_bool(
                        set_show_account_name.clone(),
                        apply_show_account_name.clone(),
                        value,
                        |settings, value| {
                            settings.show_account_name = value;
                        },
                    );
                }
            },
            "customize-show-account-name",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("customize-show-account-name"),
    ]);
    rows.extend(popup_settings_cards(
        popup_visibility,
        discovered_popup_bricks,
        popup_order,
        &enabled,
        show_total_spend_on_all_tab,
        total_spend_presentation,
        expanded_popup_provider,
        set_expanded_popup_provider,
        set_popup_visibility,
        set_show_total_spend_on_all_tab,
        set_total_spend_presentation,
        hovered_card_id,
        set_hovered_card_id.clone(),
        settings_tx.clone(),
    ));
    ("Customize", rows)
}

pub(super) fn popup_settings_cards(
    popup_visibility: &PopupVisibility,
    discovered_popup_bricks: &BTreeMap<String, String>,
    popup_order: &[PopupWidgetKind],
    enabled_providers: &[ProviderKind],
    show_total_spend_on_all_tab: bool,
    total_spend_presentation: TotalSpendPresentation,
    expanded_popup_provider: &Option<String>,
    set_expanded_popup_provider: SetState<Option<String>>,
    set_popup_visibility: SetState<PopupVisibility>,
    set_show_total_spend_on_all_tab: SetState<bool>,
    set_total_spend_presentation: SetState<TotalSpendPresentation>,
    hovered_card_id: &Option<String>,
    set_hovered_card_id: SetState<Option<String>>,
    settings_tx: Sender<Settings>,
) -> Vec<Element> {
    let apply_show_total_spend = settings_tx.clone();
    let apply_total_spend_presentation = settings_tx.clone();
    let mut rows = vec![
        settings_section_heading("Home tab").with_key("popup-home-tab-heading"),
        settings_toggle_card(
            "Usage Stats",
            show_total_spend_on_all_tab,
            move |value| {
                persist_bool(
                    set_show_total_spend_on_all_tab.clone(),
                    apply_show_total_spend.clone(),
                    value,
                    |settings, value| {
                        settings.show_total_spend_on_all_tab = value;
                    },
                );
            },
            "popup-show-total-spend",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("popup-show-total-spend"),
        settings_control_card(
            "Layout",
            None,
            ComboBox::new(["Donut", "Cards"])
                .selected_index(total_spend_presentation.index())
                .on_selection_changed({
                    let apply_total_spend_presentation = apply_total_spend_presentation.clone();
                    move |choice| {
                        let value = TotalSpendPresentation::from_index(choice);
                        set_total_spend_presentation.call(value);
                        persist_update(apply_total_spend_presentation.clone(), move |settings| {
                            settings.total_spend_presentation = value;
                        });
                    }
                }),
            "popup-total-spend-layout",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("popup-total-spend-layout"),
        settings_section_heading("Provider cards").with_key("popup-provider-cards-heading"),
    ];

    let ordered_enabled: Vec<ProviderKind> = popup_order
        .iter()
        .filter_map(|widget| widget.as_provider())
        .filter(|provider| enabled_providers.contains(provider))
        .collect();

    if ordered_enabled.is_empty() {
        rows.push(
            settings_info_card("Popup cards", "Turn on a provider to set up its cards.")
                .with_key("popup-empty"),
        );
    }

    for provider in ordered_enabled {
        let descriptor = crate::provider_registry::descriptor(provider);
        let provider_id = provider.id().to_string();
        let is_expanded = expanded_popup_provider.as_deref() == Some(provider_id.as_str());
        let expand_id = provider_id.clone();
        let expand_setter = set_expanded_popup_provider.clone();
        let section_all = popup_visibility.provider_shown_on_all(provider);
        let mut brick_rows = vec![settings_brick_table_header(provider.id())];

        let extra_ids = popup_visibility
            .bricks
            .keys()
            .chain(discovered_popup_bricks.keys())
            .cloned()
            .collect::<Vec<_>>();
        for brick_id in crate::provider_registry::settings_brick_ids(provider, &extra_ids) {
            let snapshot_all = popup_visibility.clone();
            let snapshot_tab = popup_visibility.clone();
            let visibility = snapshot_all.visibility_for(&brick_id);
            let label = crate::provider_registry::settings_brick_label(
                provider,
                &brick_id,
                discovered_popup_bricks,
            );
            let brick_id_for_all = brick_id.clone();
            let brick_id_for_tab = brick_id.clone();
            let set_visibility_all = set_popup_visibility.clone();
            let set_visibility_tab = set_popup_visibility.clone();
            let settings_tx_all = settings_tx.clone();
            let settings_tx_tab = settings_tx.clone();
            brick_rows.push(settings_brick_row(
                label,
                visibility.all_tab,
                visibility.provider_tab,
                section_all,
                move |all_tab| {
                    let provider_tab = snapshot_all.visibility_for(&brick_id_for_all).provider_tab;
                    persist_popup_brick(
                        &snapshot_all,
                        set_visibility_all.clone(),
                        settings_tx_all.clone(),
                        brick_id_for_all.clone(),
                        all_tab,
                        provider_tab,
                    );
                },
                move |provider_tab| {
                    let all_tab = snapshot_tab.visibility_for(&brick_id_for_tab).all_tab;
                    persist_popup_brick(
                        &snapshot_tab,
                        set_visibility_tab.clone(),
                        settings_tx_tab.clone(),
                        brick_id_for_tab.clone(),
                        all_tab,
                        provider_tab,
                    );
                },
                &format!("{}-{}", provider.id(), brick_id),
            ));
        }

        let section_snapshot = popup_visibility.clone();
        let set_section = set_popup_visibility.clone();
        let section_tx = settings_tx.clone();
        let expanded_body_height = Some(settings_brick_body_height(brick_rows.len()));
        rows.push(
            settings_checkbox_expander(
                descriptor.display_name,
                section_all,
                move |show_on_all| {
                    persist_popup_provider_all(
                        &section_snapshot,
                        set_section.clone(),
                        section_tx.clone(),
                        provider,
                        show_on_all,
                    );
                },
                is_expanded,
                move |expanded| {
                    if expanded {
                        expand_setter.call(Some(expand_id.clone()));
                    } else {
                        expand_setter.call(None);
                    }
                },
                expanded_body_height,
                format!("popup-provider-{}", provider.id()),
                hovered_card_id,
                set_hovered_card_id.clone(),
                vstack(brick_rows).spacing(0.0),
            )
            .with_key(format!("popup-provider-{}", provider.id())),
        );
    }

    rows
}
