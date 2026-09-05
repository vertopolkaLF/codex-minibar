use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct WidgetDragState {
    pub(super) active: PopupWidgetKind,
    pub(super) over: PopupWidgetKind,
}

pub(super) fn persist_popup_order(
    settings_tx: Sender<Settings>,
    set_ui: AsyncSetState<UiState>,
    mut ui: UiState,
    next_order: Vec<PopupWidgetKind>,
) {
    ui.popup_order = next_order.clone();
    set_ui.call(ui);
    crate::settings_window::persist_update(settings_tx, move |settings| {
        settings.popup_order = next_order;
        settings.normalize_popup_order();
    });
}

pub(super) fn persist_total_spend_period(
    settings_tx: Sender<Settings>,
    set_ui: AsyncSetState<UiState>,
    mut ui: UiState,
    period: TotalSpendPeriod,
) {
    if ui.total_spend_period == period {
        return;
    }
    ui.total_spend_period = period;
    set_ui.call(ui);
    crate::settings_window::persist_update(settings_tx, move |settings| {
        settings.total_spend_period = period;
    });
}

pub(super) fn commit_widget_drag(
    settings_tx: Sender<Settings>,
    set_ui: AsyncSetState<UiState>,
    ui: UiState,
    drag: WidgetDragState,
    set_drag: SetState<Option<WidgetDragState>>,
) {
    // PointerReleased can hit both the section catcher and the page body in one
    // gesture; only the first commit may mutate order.
    thread_local! {
        static COMMITTING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    if COMMITTING.with(|flag| flag.replace(true)) {
        return;
    }

    set_drag.call(None);
    if drag.active == drag.over {
        COMMITTING.with(|flag| flag.set(false));
        return;
    }
    let show_total_spend = ui.show_total_spend_on_all_tab
        && total_spend_provider_count(
            ui.codex_enabled,
            ui.claude_enabled,
            ui.cursor_enabled,
            ui.opencode_zen_enabled,
            ui.opencode_go_enabled,
        ) > 1;
    let mut scratch = Settings {
        popup_order: ui.popup_order.clone(),
        providers: crate::settings::ProviderSettings::from_enabled(
            crate::provider_registry::PROVIDERS
                .iter()
                .filter(|descriptor| match descriptor.kind {
                    ProviderKind::Codex => ui.codex_enabled,
                    ProviderKind::Claude => ui.claude_enabled,
                    ProviderKind::Cursor => ui.cursor_enabled,
                    ProviderKind::OpenCodeZen => ui.opencode_zen_enabled,
                    ProviderKind::OpenCodeGo => ui.opencode_go_enabled,
                    ProviderKind::OpenRouter => ui.openrouter_enabled,
                })
                .map(|descriptor| descriptor.kind),
        ),
        show_total_spend_on_all_tab: ui.show_total_spend_on_all_tab,
        ..Settings::default()
    };
    if !scratch.move_popup_widget(drag.active, drag.over, show_total_spend) {
        COMMITTING.with(|flag| flag.set(false));
        return;
    }
    persist_popup_order(settings_tx, set_ui, ui, scratch.popup_order);
    COMMITTING.with(|flag| flag.set(false));
}

pub(super) fn drag_handle(
    widget: PopupWidgetKind,
    color_scheme: ColorScheme,
    drag: &Option<WidgetDragState>,
    set_drag: SetState<Option<WidgetDragState>>,
) -> Element {
    let idle = popup_chrome_icon_color(color_scheme, false);
    let active = drag.as_ref().is_some_and(|state| state.active == widget);
    let set_on_press = set_drag.clone();
    relative_panel::<Vec<Element>>(vec![
        border(Element::Empty)
            .background(ThemeRef::SubtleFill)
            .opacity(if active { 1.0 } else { 0.0 })
            .corner_radius(4.0)
            .relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .relative_align_bottom()
            .into(),
        crate::icons::element("fluent-drag", 14.0, idle)
            .relative_align_h_center()
            .relative_align_v_center()
            .into(),
    ])
    .tooltip("Drag to reorder")
    .width(REORDER_BUTTON_SIZE)
    .height(REORDER_BUTTON_SIZE)
    .min_width(REORDER_BUTTON_SIZE)
    .min_height(REORDER_BUTTON_SIZE)
    .max_width(REORDER_BUTTON_SIZE)
    .max_height(REORDER_BUTTON_SIZE)
    .background(Color::transparent())
    .on_pointer_pressed(move |_: PointerEventInfo| {
        set_on_press.call(Some(WidgetDragState {
            active: widget,
            over: widget,
        }));
    })
    .with_key(format!("drag-handle-{}", widget.id()))
    .into()
}

pub(super) fn with_widget_drop_target(
    widget: PopupWidgetKind,
    content: Element,
    drag: &Option<WidgetDragState>,
    set_drag: SetState<Option<WidgetDragState>>,
    settings_tx: Sender<Settings>,
    set_ui: AsyncSetState<UiState>,
    ui: UiState,
) -> Element {
    let is_active = drag.as_ref().is_some_and(|state| state.active == widget);
    let is_over = drag.as_ref().is_some_and(|state| state.over == widget);
    let show_outline = is_over && !is_active;
    let dragging = drag.is_some();
    let set_on_enter = set_drag.clone();
    let set_on_release = set_drag.clone();
    let drag_for_enter = drag.clone();
    let drag_for_release = drag.clone();

    // Visual ring only — null fill so it does not steal hits on its own.
    let outline: Element = border(Element::Empty)
        .border_thickness(Thickness::uniform(1.0))
        .border_brush(ThemeRef::Accent)
        .corner_radius(6.0)
        .opacity(if show_outline { 1.0 } else { 0.0 })
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Stretch)
        .grid_column(0)
        .grid_row(0)
        .into();

    let mut layers: Vec<Element> = vec![
        content
            .opacity(if is_active { 0.55 } else { 1.0 })
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .grid_column(0)
            .grid_row(0),
        outline,
    ];

    // While dragging, a transparent full-size catcher matches the highlight
    // zone (header + cards) so release on the title row commits the drop.
    // WinUI hit-tests Transparent backgrounds; null backgrounds do not.
    if dragging {
        layers.push(
            border(Element::Empty)
                .background(Color::transparent())
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .vertical_alignment(VerticalAlignment::Stretch)
                .grid_column(0)
                .grid_row(0)
                .on_pointer_entered(move |_: PointerEventInfo| {
                    let Some(current) = drag_for_enter.clone() else {
                        return;
                    };
                    if current.over == widget {
                        return;
                    }
                    set_on_enter.call(Some(WidgetDragState {
                        active: current.active,
                        over: widget,
                    }));
                })
                .on_pointer_released(move |_: PointerEventInfo| {
                    let Some(current) = drag_for_release.clone() else {
                        return;
                    };
                    // This catcher covers the whole section (header + body), so
                    // the drop target is always `widget` — do not trust a possibly
                    // stale `over` captured before the last pointer-enter update.
                    commit_widget_drag(
                        settings_tx.clone(),
                        set_ui.clone(),
                        ui.clone(),
                        WidgetDragState {
                            active: current.active,
                            over: widget,
                        },
                        set_on_release.clone(),
                    );
                })
                .with_key(format!("drop-catcher-{}", widget.id()))
                .into(),
        );
    }

    grid(layers)
        .columns([GridLength::Star(1.0)])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch)
        // Keep the host identity stable across highlight toggles so a remount
        // cannot swallow the in-flight pointer release.
        .with_key(format!("drop-target-{}", widget.id()))
        .into()
}

#[derive(Clone)]
pub(super) struct OpenRouterPopupActions {
    pub(super) settings_tx: Sender<Settings>,
    pub(super) hovered_action: Option<String>,
    pub(super) set_hovered_action: SetState<Option<String>>,
    pub(super) now: DateTime<Utc>,
}

pub(super) fn remove_openrouter_api_key(
    account_id: String,
    key_id: String,
    settings_tx: Sender<Settings>,
) {
    if let Err(error) = crate::openrouter::save_account_api_key(&account_id, &key_id, None) {
        notifications::show("OpenRouter key not removed", &format!("{error:#}"));
        return;
    }
    crate::settings_window::persist_update(settings_tx, move |settings| {
        let mut accounts = crate::openrouter::accounts_for_settings(settings);
        let Some(account) = accounts.iter_mut().find(|account| account.id == account_id) else {
            return;
        };
        let before = account.api_key_ids.len();
        account.api_key_ids.retain(|id| id != &key_id);
        if account.api_key_ids.len() == before {
            return;
        }
        settings.openrouter_accounts = accounts;
        settings.openrouter_credentials_revision =
            settings.openrouter_credentials_revision.wrapping_add(1);
    });
}

pub(super) fn openrouter_delete_button(
    id: String,
    color_scheme: ColorScheme,
    hovered_action: &Option<String>,
    set_hovered_action: SetState<Option<String>>,
    on_click: impl IntoUnitCallback,
) -> Element {
    let hovered = hovered_action.as_deref() == Some(id.as_str());
    let set_on_enter = set_hovered_action.clone();
    let set_on_exit = set_hovered_action;
    let button_id = id.clone();
    let idle_color = popup_chrome_icon_color(color_scheme, false);
    let hover_background: Element = border(Element::Empty)
        .background(ThemeRef::SubtleFill)
        .opacity(if hovered { 1.0 } else { 0.0 })
        .corner_radius(4.0)
        .relative_align_left()
        .relative_align_right()
        .relative_align_top()
        .relative_align_bottom()
        .into();
    // Same hit target and glyph size as popup footer chrome. The old 24/14
    // slot crushed the Fluent delete path into unreadable slivers.
    let idle_icon: Element = crate::icons::element("fluent-delete", 18.0, idle_color)
        .opacity(if hovered { 0.0 } else { 1.0 })
        .relative_align_h_center()
        .relative_align_v_center()
        .into();
    let accent_icon: Element = crate::icons::accent_element("fluent-delete", 18.0)
        .opacity(if hovered { 1.0 } else { 0.0 })
        .relative_align_h_center()
        .relative_align_v_center()
        .into();
    relative_panel(vec![hover_background, idle_icon, accent_icon])
        .tooltip("Remove key")
        .width(POPUP_ACTION_SIZE)
        .height(POPUP_ACTION_SIZE)
        .min_width(POPUP_ACTION_SIZE)
        .min_height(POPUP_ACTION_SIZE)
        .max_width(POPUP_ACTION_SIZE)
        .max_height(POPUP_ACTION_SIZE)
        .background(Color::transparent())
        .on_pointer_entered(move |_: PointerEventInfo| {
            set_on_enter.call(Some(button_id.clone()));
        })
        .on_pointer_exited(move || set_on_exit.call(None))
        .on_tapped(on_click)
        .with_key(format!(
            "{id}-delete-{}-18-{:02X}{:02X}{:02X}",
            POPUP_ACTION_SIZE, idle_color.r, idle_color.g, idle_color.b
        ))
        .into()
}
