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

/// Enabled instances that can feed a tray indicator, in display order.
pub(super) fn enabled_providers(instances: &[ProviderInstance]) -> Vec<ProviderId> {
    instances
        .iter()
        .filter(|instance| instance.enabled)
        .filter(|instance| {
            !crate::provider_registry::descriptor(instance.driver)
                .default_tray_metrics
                .is_empty()
        })
        .map(ProviderInstance::provider_id)
        .collect()
}
