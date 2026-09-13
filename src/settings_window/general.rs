use super::persistence::{persist_bool, persist_update};
use super::shared::settings_section_heading;
use super::*;

pub(super) fn render(ctx: &SettingsPageContext<'_>) -> (&'static str, Vec<Element>) {
    let start_at_login = ctx.start_at_login;
    let limit_refresh_interval = ctx.limit_refresh_interval;
    let usage_stats_enabled = ctx.usage_stats_enabled;
    let usage_stats_excluded_providers = ctx.usage_stats_excluded_providers;
    let available_usage_providers = crate::provider_registry::PROVIDERS
        .iter()
        .filter(|descriptor| match descriptor.kind {
            ProviderKind::Codex => ctx.codex_enabled,
            ProviderKind::Claude => ctx.claude_enabled,
            ProviderKind::Cursor => ctx.cursor_enabled,
            ProviderKind::OpenCodeZen => ctx.opencode_zen_enabled,
            ProviderKind::OpenCodeGo => ctx.opencode_go_enabled,
            ProviderKind::OpenRouter => ctx.openrouter_enabled,
        })
        .map(|descriptor| descriptor.kind)
        .collect::<Vec<_>>();
    let usage_refresh_interval = ctx.usage_refresh_interval;
    let set_start_at_login = ctx.set_start_at_login.clone();
    let set_usage_stats_enabled = ctx.set_usage_stats_enabled.clone();
    let set_usage_stats_excluded_providers = ctx.set_usage_stats_excluded_providers.clone();
    let set_limit_refresh_interval = ctx.set_limit_refresh_interval.clone();
    let set_usage_refresh_interval = ctx.set_usage_refresh_interval.clone();
    let hovered_card_id = ctx.hovered_card_id;
    let set_hovered_card_id = ctx.set_hovered_card_id.clone();
    let settings_tx = ctx.settings_tx.clone();
    let apply_start_at_login = settings_tx.clone();
    let apply_usage_stats_enabled = settings_tx.clone();
    let apply_limit_refresh_interval = settings_tx.clone();
    let apply_usage_refresh_interval = settings_tx.clone();
    (
        "General",
        vec![
            settings_toggle_card(
                "Start with Windows",
                start_at_login,
                move |value| {
                    persist_bool(
                        set_start_at_login.clone(),
                        apply_start_at_login.clone(),
                        value,
                        |settings, value| {
                            settings.start_at_login = value;
                        },
                    );
                },
                "general-startup",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("general-startup"),
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
                .on_selection_changed(move |choice: i32| {
                    let value = LimitRefreshInterval::from_index(choice);
                    set_limit_refresh_interval.call(value);
                    persist_update(apply_limit_refresh_interval.clone(), move |settings| {
                        settings.limit_refresh_interval = value;
                    });
                }),
                "general-limit-refresh-interval",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("general-limit-refresh-interval"),
            settings_section_heading("Usage Stats").with_key("general-usage-stats-heading"),
            settings_toggle_card(
                "Enable Usage Stats",
                usage_stats_enabled,
                move |value| {
                    persist_bool(
                        set_usage_stats_enabled.clone(),
                        apply_usage_stats_enabled.clone(),
                        value,
                        |settings, value| {
                            settings.usage_stats_enabled = value;
                        },
                    );
                },
                "general-usage-stats-enabled",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("general-usage-stats-enabled"),
            usage_stats_provider_selection_card(
                &available_usage_providers,
                usage_stats_excluded_providers,
                usage_stats_enabled,
                set_usage_stats_excluded_providers,
                settings_tx.clone(),
            )
            .with_key("general-usage-stats-providers"),
            settings_control_card(
                "Collection period",
                Some("How often local provider history is scanned."),
                ComboBox::new([
                    "1 minute",
                    "5 minutes",
                    "10 minutes",
                    "15 minutes",
                    "30 minutes",
                    "45 minutes",
                    "60 minutes",
                ])
                .selected_index(usage_refresh_interval.index())
                .enabled(usage_stats_enabled)
                .on_selection_changed(move |choice: i32| {
                    let value = UsageRefreshInterval::from_index(choice);
                    set_usage_refresh_interval.call(value);
                    persist_update(apply_usage_refresh_interval.clone(), move |settings| {
                        settings.usage_refresh_interval = value;
                    });
                }),
                "general-usage-refresh-interval",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("general-usage-refresh-interval"),
        ],
    )
}

fn usage_stats_provider_selection_card(
    available_providers: &[ProviderKind],
    excluded_providers: &[String],
    usage_stats_enabled: bool,
    set_excluded_providers: SetState<Vec<String>>,
    settings_tx: Sender<Settings>,
) -> Element {
    const PROVIDER_COLUMNS: usize = 3;
    let mut provider_checks = Vec::with_capacity(available_providers.len());
    for (index, provider) in available_providers.iter().copied().enumerate() {
        let row = (index / PROVIDER_COLUMNS) as i32;
        let column = (index % PROVIDER_COLUMNS) as i32;
        let descriptor = crate::provider_registry::descriptor(provider);
        let checked = !excluded_providers.iter().any(|id| id == provider.id());
        let current = excluded_providers.to_vec();
        let set_excluded_providers = set_excluded_providers.clone();
        let settings_tx = settings_tx.clone();
        let checkbox: Element = CheckBox::new(checked)
            .content(descriptor.display_name)
            .enabled(usage_stats_enabled)
            .on_checked(move |checked| {
                let mut optimistic = current.clone();
                if checked {
                    optimistic.retain(|id| id != provider.id());
                } else if !optimistic.iter().any(|id| id == provider.id()) {
                    optimistic.push(provider.id().into());
                }
                set_excluded_providers.call(optimistic);
                persist_update(settings_tx.clone(), move |settings| {
                    settings.set_usage_stats_provider_enabled(provider, checked);
                });
            })
            .min_width(0.0)
            .padding(Thickness::uniform(0.0))
            .horizontal_alignment(HorizontalAlignment::Left)
            .vertical_alignment(VerticalAlignment::Center)
            .into();
        provider_checks.push(
            checkbox
                .with_key(format!("general-usage-provider-{}", provider.id()))
                .grid_row(row)
                .grid_column(column),
        );
    }

    let provider_grid: Element = if provider_checks.is_empty() {
        text_block("Enable a provider in the Providers tab to include it here.")
            .font_size(12.0)
            .opacity(0.72)
            .wrap()
            .into()
    } else {
        grid(provider_checks)
            .columns(vec![GridLength::Star(1.0); PROVIDER_COLUMNS])
            .rows(vec![
                GridLength::Auto;
                available_providers.len().div_ceil(PROVIDER_COLUMNS)
            ])
            .column_spacing(12.0)
            .row_spacing(4.0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Center)
            .into()
    };

    border(
        vstack((
            text_block("Included providers").font_size(14.0).wrap(),
            text_block(
                "Choose which providers contribute to the Usage tab and Home card. Provider pages keep their usage card, and existing data is never deleted.",
            )
            .font_size(12.0)
            .opacity(0.72)
            .wrap(),
            provider_grid,
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
    .into()
}
