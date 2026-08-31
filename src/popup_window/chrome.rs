use super::*;

pub(super) const ICON_BUTTON_SIZE: f64 = 36.0;
pub(super) const REORDER_BUTTON_SIZE: f64 = 28.0;
pub(super) const ALL_TAB_WIDTH: f64 = 44.0;
pub(super) const TAB_STRIP_SPACING: f64 = 2.0;
pub(super) const FOOTER_TAB_PADDING_LEFT: f64 = 14.0;
pub(super) const FOOTER_PADDING_RIGHT: f64 = 18.0;
pub(super) const FOOTER_COLUMN_SPACING: f64 = 8.0;
pub(super) const FOOTER_ACTION_SPACING: f64 = 4.0;
pub(super) const FOOTER_ACTION_COUNT: f64 = 2.0;
const PROVIDER_ERROR_COLOR: Color = Color::rgb(247, 117, 117);

pub(super) fn provider_tab_strip_content_width(provider_count: usize) -> f64 {
    // Home + Usage + enabled provider tabs.
    ICON_BUTTON_SIZE
        + ICON_BUTTON_SIZE
        + TAB_STRIP_SPACING
        + provider_count as f64 * (ICON_BUTTON_SIZE + TAB_STRIP_SPACING)
}

pub(super) fn provider_tab_strip_viewport_width() -> f64 {
    f64::from(popup::POPUP_WIDTH)
        - FOOTER_TAB_PADDING_LEFT
        - FOOTER_PADDING_RIGHT
        - FOOTER_COLUMN_SPACING
        - (ICON_BUTTON_SIZE * FOOTER_ACTION_COUNT
            + FOOTER_ACTION_SPACING * (FOOTER_ACTION_COUNT - 1.0))
}

pub(super) fn provider_tabs_key(
    providers: &[ProviderKind],
    show_provider_icon_tabs: bool,
    use_colored_provider_icons: bool,
    color_scheme: ColorScheme,
) -> String {
    format!(
        "provider-tabs-home-usage-{}-{}-{}-{}",
        provider_order_key(providers),
        show_provider_icon_tabs,
        use_colored_provider_icons,
        color_scheme as i32,
    )
}

pub(super) fn footer_actions_key(update_available: bool, color_scheme: ColorScheme) -> String {
    format!(
        "footer-actions-{}-{}",
        update_available, color_scheme as i32
    )
}

/// Compact footer selector item for choosing the Home, Usage, or provider view.
pub(super) fn popup_tab_button(
    id: &'static str,
    icon_name: Option<&'static str>,
    label: Option<&'static str>,
    tip: &'static str,
    selected: bool,
    has_error: bool,
    use_colored_provider_icons: bool,
    color_scheme: ColorScheme,
    hovered_action: &Option<String>,
    set_hovered_action: SetState<Option<String>>,
    on_wheel: impl IntoCallback<PointerEventInfo>,
    on_click: impl IntoUnitCallback,
) -> Element {
    let on_click = on_click.into_unit_callback();
    let hovered = hovered_action.as_deref() == Some(id);
    let set_on_enter = set_hovered_action.clone();
    let set_on_exit = set_hovered_action;
    let idle_icon_color = popup_chrome_icon_color(color_scheme, false);
    let hover_icon_color = popup_chrome_icon_color(color_scheme, true);
    let brand_icon_color = match icon_name {
        Some("codex") | Some("chatgpt") => Color::rgb(128, 159, 255),
        Some("claude") => Color::rgb(217, 119, 87),
        // Match Total Spend: Cursor mark flips with the Windows text theme.
        Some("cursor") => combined_usage_color(ProviderKind::Cursor, color_scheme),
        Some("opencode") => combined_usage_color(ProviderKind::OpenCodeZen, color_scheme),
        Some("openrouter") => combined_usage_color(ProviderKind::OpenRouter, color_scheme),
        Some("fluent-chart") | Some("fluent-home") => popup_chrome_icon_color(color_scheme, false),
        _ => idle_icon_color,
    };
    let tab_width = if label.is_some() {
        ALL_TAB_WIDTH
    } else {
        ICON_BUTTON_SIZE
    };
    let hover_background: Element = border(Element::Empty)
        .background(ThemeRef::SubtleFill)
        .opacity(if hovered { 1.0 } else { 0.0 })
        .corner_radius(4.0)
        .relative_align_left()
        .relative_align_right()
        .relative_align_top()
        .relative_align_bottom()
        .into();
    let selection_marker: Element = border(Element::Empty)
        .height(2.0)
        .background(ThemeRef::Accent)
        .opacity(if selected { 1.0 } else { 0.0 })
        .corner_radius(1.0)
        .margin(Thickness {
            left: 9.0,
            top: 0.0,
            right: 9.0,
            bottom: 0.0,
        })
        .relative_align_left()
        .relative_align_right()
        .relative_align_bottom()
        .into();
    let mut layers: Vec<Element> = vec![hover_background];
    if let Some(label) = label {
        layers.push(
            body_strong(label)
                .foreground(if selected {
                    ThemeRef::Accent
                } else if hovered {
                    ThemeRef::PrimaryText
                } else {
                    ThemeRef::SecondaryText
                })
                .relative_align_h_center()
                .relative_align_v_center()
                .into(),
        );
    } else {
        let icon_name = icon_name.expect("provider tab icon");
        if use_colored_provider_icons {
            layers.push(
                crate::icons::element(icon_name, 18.0, brand_icon_color)
                    .relative_align_h_center()
                    .relative_align_v_center()
                    .into(),
            );
        } else {
            // Crossfade idle/emphasized hosts instead of remounting on hover.
            layers.push(
                crate::icons::element(icon_name, 18.0, idle_icon_color)
                    .opacity(if hovered { 0.0 } else { 1.0 })
                    .relative_align_h_center()
                    .relative_align_v_center()
                    .into(),
            );
            layers.push(
                crate::icons::element(icon_name, 18.0, hover_icon_color)
                    .opacity(if hovered { 1.0 } else { 0.0 })
                    .relative_align_h_center()
                    .relative_align_v_center()
                    .into(),
            );
        }
    }
    layers.push(selection_marker);
    if has_error {
        layers.push(
            provider_error_badge(13.0, on_click.clone())
                .relative_align_right()
                .relative_align_top()
                .margin(Thickness {
                    left: 0.0,
                    top: 2.0,
                    right: 2.0,
                    bottom: 0.0,
                }),
        );
    }
    // Cover swap-chain icons so wheel hits a normal XAML element and
    // bubbles here. SwapChainPanel often swallows wheel input.
    layers.push(
        border(Element::Empty)
            .background(Color::transparent())
            .relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .relative_align_bottom()
            .into(),
    );

    // `SwapChainPanel` paints only on mount. Keep the tab key stable across
    // hover so reconciliation cannot recycle another tab's native icon host.
    relative_panel(layers)
        .tooltip(tip)
        .width(tab_width)
        .height(ICON_BUTTON_SIZE)
        .min_width(tab_width)
        .min_height(ICON_BUTTON_SIZE)
        .max_width(tab_width)
        .max_height(ICON_BUTTON_SIZE)
        .background(Color::transparent())
        .on_pointer_entered(move |_: PointerEventInfo| {
            set_on_enter.call(Some(id.to_string()));
        })
        .on_pointer_exited(move || set_on_exit.call(None))
        .on_pointer_wheel(on_wheel)
        .on_tapped(on_click)
        .with_key(format!(
            "{id}-{}-{}-{}",
            icon_name.unwrap_or("label"),
            use_colored_provider_icons,
            color_scheme as i32
        ))
        .into()
}

/// Compact Fluent filled error-circle marker used in provider headings and
/// footer tabs. Its identity key includes the requested tint, so the
/// mount-only icon painter receives the exact `#F77575` color.
pub(super) fn provider_error_badge(size: f64, on_click: Callback<()>) -> Element {
    crate::icons::element("fluent-error-circle", size, PROVIDER_ERROR_COLOR)
        .tooltip("Provider error")
        .on_tapped(on_click)
        .with_key(format!(
            "provider-error-badge-{size}-{:02X}{:02X}{:02X}",
            PROVIDER_ERROR_COLOR.r, PROVIDER_ERROR_COLOR.g, PROVIDER_ERROR_COLOR.b
        ))
}

/// Icon-only chrome action. Refresh uses two rounded circular arrows and
/// rotates the same icon while provider requests are in flight; the remaining
/// actions use neutral swap-chain SVGs that adopt the accent on hover.
pub(super) fn icon_button(
    id: &'static str,
    normal_icon: &'static str,
    hover_icon: &'static str,
    tip: &str,
    is_refreshing: bool,
    rotation: f64,
    color_scheme: ColorScheme,
    hovered_action: &Option<String>,
    set_hovered_action: SetState<Option<String>>,
    on_click: impl IntoUnitCallback,
) -> Element {
    chrome_icon_button(
        id,
        normal_icon,
        hover_icon,
        tip,
        ICON_BUTTON_SIZE,
        18.0,
        is_refreshing,
        rotation,
        color_scheme,
        hovered_action,
        set_hovered_action,
        on_click,
    )
}

pub(super) fn chrome_icon_button(
    id: &'static str,
    normal_icon: &'static str,
    hover_icon: &'static str,
    tip: &str,
    size: f64,
    glyph_size: f64,
    is_refreshing: bool,
    rotation: f64,
    color_scheme: ColorScheme,
    hovered_action: &Option<String>,
    set_hovered_action: SetState<Option<String>>,
    on_click: impl IntoUnitCallback,
) -> Element {
    let hovered = hovered_action.as_deref() == Some(id);
    let set_on_enter = set_hovered_action.clone();
    let set_on_exit = set_hovered_action;
    let idle_color = popup_chrome_icon_color(color_scheme, false);
    let rotation = if is_refreshing { rotation } else { 0.0 };
    let hover_background: Element = border(Element::Empty)
        .background(ThemeRef::SubtleFill)
        .opacity(if hovered { 1.0 } else { 0.0 })
        .corner_radius(6.0)
        .relative_align_left()
        .relative_align_right()
        .relative_align_top()
        .relative_align_bottom()
        .into();
    // Keep both swap-chain hosts mounted and crossfade opacity on hover.
    // Remounting the icon host on every hover recycles native panels and can
    // leave a neighbor's painted glyph in this slot. Rotation is a transform
    // on the existing host, so it does not repaint or recycle the glyph.
    let idle_icon: Element = crate::icons::element(normal_icon, glyph_size, idle_color)
        .rotation(rotation)
        .opacity(if hovered || is_refreshing { 0.0 } else { 1.0 })
        .with_opacity_transition(crate::theme::duration(crate::theme::CONTROL_FAST_ANIMATION))
        .relative_align_h_center()
        .relative_align_v_center()
        .into();
    let accent_icon: Element = crate::icons::accent_element(hover_icon, glyph_size)
        .rotation(rotation)
        .opacity(if hovered || is_refreshing { 1.0 } else { 0.0 })
        .with_opacity_transition(crate::theme::duration(crate::theme::CONTROL_FAST_ANIMATION))
        .relative_align_h_center()
        .relative_align_v_center()
        .into();
    // Stable across hover; remount only when theme tint changes.
    relative_panel(vec![hover_background, idle_icon, accent_icon])
        .tooltip(tip)
        .width(size)
        .height(size)
        .min_width(size)
        .min_height(size)
        .max_width(size)
        .max_height(size)
        .background(Color::transparent())
        .on_pointer_entered(move |_: PointerEventInfo| {
            set_on_enter.call(Some(id.to_string()));
        })
        .on_pointer_exited(move || set_on_exit.call(None))
        .on_tapped(on_click)
        .with_key(format!(
            "{id}-{size}-{glyph_size}-{:02X}{:02X}{:02X}",
            idle_color.r, idle_color.g, idle_color.b
        ))
        .into()
}

/// Approximate WinUI primary/secondary text for swap-chain icons that cannot
/// bind ThemeRef brushes directly.
pub(super) fn popup_chrome_icon_color(color_scheme: ColorScheme, emphasized: bool) -> Color {
    match color_scheme {
        ColorScheme::Light => {
            if emphasized {
                Color::rgb(0, 0, 0)
            } else {
                Color::rgb(96, 96, 96)
            }
        }
        ColorScheme::Dark => {
            if emphasized {
                Color::rgb(230, 230, 230)
            } else {
                Color::rgb(190, 190, 190)
            }
        }
    }
}
