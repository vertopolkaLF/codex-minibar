use super::*;

pub(super) fn log_view_card(log_content: &str) -> Element {
    border(
        scroll_viewer(
            border(
                text_block(if log_content.is_empty() {
                    "No log events yet."
                } else {
                    log_content
                })
                .font_size(12.0)
                .wrap(),
            )
            .padding(settings_card_padding()),
        )
        .horizontal_scroll_bar_visibility(ScrollBarVisibility::Disabled)
        .vertical_scroll_bar_visibility(ScrollBarVisibility::Auto)
        .height(340.0),
    )
    .background(ThemeRef::CardBackground)
    .corner_radius(8.0)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

pub(super) fn settings_section_heading(title: impl Into<String>) -> Element {
    text_block(title)
        .font_size(16.0)
        .semibold()
        .margin(Thickness {
            left: 0.0,
            top: 16.0,
            right: 0.0,
            bottom: 4.0,
        })
        .into()
}

pub(super) fn enabled_providers(
    order: &[ProviderKind],
    codex_enabled: bool,
    claude_enabled: bool,
    cursor_enabled: bool,
    opencode_zen_enabled: bool,
    opencode_go_enabled: bool,
    openrouter_enabled: bool,
) -> Vec<ProviderKind> {
    order
        .iter()
        .copied()
        .filter(|provider| match provider {
            ProviderKind::Codex => codex_enabled,
            ProviderKind::Claude => claude_enabled,
            ProviderKind::Cursor => cursor_enabled,
            ProviderKind::OpenCodeZen => opencode_zen_enabled,
            ProviderKind::OpenCodeGo => opencode_go_enabled,
            ProviderKind::OpenRouter => openrouter_enabled,
        })
        .filter(|provider| {
            !crate::provider_registry::descriptor(*provider)
                .default_tray_metrics
                .is_empty()
        })
        .collect()
}
