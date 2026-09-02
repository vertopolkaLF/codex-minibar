use super::persistence::persist_update;
use super::shared::{enabled_providers, settings_section_heading};
use super::*;

static INDICATOR_MODAL_ANIM_GEN: AtomicU64 = AtomicU64::new(0);
const INDICATOR_MODAL_SCRIM: Color = Color {
    a: 0xaa,
    r: 0,
    g: 0,
    b: 0,
};
const INDICATOR_MODAL_WIDTH: f64 = 520.0;
const INDICATOR_MODAL_RADIUS: f64 = 12.0;

#[derive(Clone)]
struct TrayPreviewCacheEntry {
    widget: TrayWidget,
    accent: [u8; 3],
    uses_light_theme: bool,
    time_format: TimeFormat,
    minute_bucket: u64,
    pixels: Arc<Vec<u8>>,
}

thread_local! {
    static TRAY_PREVIEW_MOUNTS: RefCell<HashMap<String, windows_core::IInspectable>> =
        RefCell::new(HashMap::new());
    static TRAY_PREVIEW_CACHE: RefCell<HashMap<String, TrayPreviewCacheEntry>> =
        RefCell::new(HashMap::new());
}

pub(super) fn render(ctx: &SettingsPageContext<'_>) -> (&'static str, Vec<Element>) {
    let popup_order = ctx.popup_order;
    let codex_enabled = ctx.codex_enabled;
    let claude_enabled = ctx.claude_enabled;
    let cursor_enabled = ctx.cursor_enabled;
    let opencode_zen_enabled = ctx.opencode_zen_enabled;
    let opencode_go_enabled = ctx.opencode_go_enabled;
    let openrouter_enabled = ctx.openrouter_enabled;
    let tray_widgets = ctx.tray_widgets;
    let expanded_tray_widget = ctx.expanded_tray_widget;
    let editing_tray_indicator = ctx.editing_tray_indicator;
    let removed_tray_widget = ctx.removed_tray_widget;
    let set_tray_widgets = ctx.set_tray_widgets.clone();
    let set_expanded_tray_widget = ctx.set_expanded_tray_widget.clone();
    let set_editing_tray_indicator = ctx.set_editing_tray_indicator.clone();
    let set_indicator_modal_visible = ctx.set_indicator_modal_visible.clone();
    let set_removed_tray_widget = ctx.set_removed_tray_widget.clone();
    let hovered_card_id = ctx.hovered_card_id;
    let set_hovered_card_id = ctx.set_hovered_card_id.clone();
    let settings_tx = ctx.settings_tx.clone();
    let providers: Vec<ProviderKind> = popup_order
        .iter()
        .filter_map(|widget| widget.as_provider())
        .collect();
    let enabled_providers = enabled_providers(
        &providers,
        codex_enabled,
        claude_enabled,
        cursor_enabled,
        opencode_zen_enabled,
        opencode_go_enabled,
        openrouter_enabled,
    );
    (
        "Tray",
        tray_settings_cards(
            tray_widgets,
            &enabled_providers,
            expanded_tray_widget,
            editing_tray_indicator,
            removed_tray_widget,
            set_tray_widgets,
            set_expanded_tray_widget,
            set_editing_tray_indicator,
            set_indicator_modal_visible,
            set_removed_tray_widget,
            hovered_card_id,
            set_hovered_card_id.clone(),
            settings_tx.clone(),
        ),
    )
}

fn tray_color_mode_label(mode: TrayColorMode) -> &'static str {
    match mode {
        TrayColorMode::Status => "Status color",
        TrayColorMode::Fixed => "Fixed color",
        TrayColorMode::Provider => "Provider color",
        TrayColorMode::Accent => "App accent",
        TrayColorMode::Monochrome => "Monochrome",
    }
}

fn tray_indicator_summary(indicator: &TrayIndicator) -> String {
    let Some(provider) = indicator.provider() else {
        return format!("Unsupported {}", indicator.provider_id);
    };
    let metric = crate::provider_registry::metric(provider, &indicator.metric_id)
        .map(|metric| metric.label.to_owned())
        .unwrap_or_else(|| indicator.metric_id.clone());
    let value = match indicator.limit_value {
        LimitValue::Used => "Used",
        LimitValue::Remaining => "Remaining",
    };
    format!(
        "{} · {metric} · {value} · {}",
        provider.display_name(),
        tray_color_mode_label(indicator.color_mode)
    )
}

fn tray_widget_summary(widget: &TrayWidget) -> String {
    if widget.kind == TrayWidgetKind::AppIcon {
        return "App icon".into();
    }
    let labels = widget
        .indicators
        .iter()
        .map(tray_indicator_summary)
        .collect::<Vec<_>>();
    if labels.is_empty() {
        "Empty widget".into()
    } else {
        labels.join(" · ")
    }
}

fn tray_preview_limits() -> &'static crate::limits::ProviderLimits {
    static LIMITS: std::sync::OnceLock<crate::limits::ProviderLimits> = std::sync::OnceLock::new();
    LIMITS.get_or_init(|| {
        let window = |used_percent| crate::limits::LimitWindow {
            used_percent: Some(used_percent),
            resets_at: None,
            duration_minutes: Some(300),
        };
        crate::limits::ProviderLimits::from_entries([
            (
                ProviderKind::Codex,
                crate::limits::RateLimits {
                    primary: window(38),
                    secondary: window(70),
                    ..Default::default()
                },
            ),
            (
                ProviderKind::Claude,
                crate::limits::RateLimits {
                    primary: window(55),
                    secondary: window(12),
                    ..Default::default()
                },
            ),
            (
                ProviderKind::Cursor,
                crate::limits::RateLimits {
                    secondary: window(18),
                    additional_limits: vec![
                        crate::limits::AdditionalLimit {
                            id: "cursor-api".into(),
                            title: "Other Models".into(),
                            window: window(47),
                        },
                        crate::limits::AdditionalLimit {
                            id: "cursor-grok-bot".into(),
                            title: "Grok Bot".into(),
                            window: window(1),
                        },
                    ],
                    ..Default::default()
                },
            ),
        ])
    })
}

fn tray_widget_preview(widget: &TrayWidget) -> Element {
    let preview_id = widget.id.clone();
    let accent = crate::theme::current_accent_rgb();
    let uses_light_theme = crate::tray::system_uses_light_theme();
    let time_format = TimeFormat::current();
    let minute_bucket = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() / 60);
    let (pixels, pixels_changed) = TRAY_PREVIEW_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(entry) = cache.get(&preview_id)
            && entry.widget == *widget
            && entry.accent == accent
            && entry.uses_light_theme == uses_light_theme
            && entry.time_format == time_format
            && entry.minute_bucket == minute_bucket
        {
            return (Arc::clone(&entry.pixels), false);
        }

        let pixels = Arc::new(crate::tray::render_widget_with_accent(
            widget,
            tray_preview_limits(),
            accent,
        ));
        cache.insert(
            preview_id.clone(),
            TrayPreviewCacheEntry {
                widget: widget.clone(),
                accent,
                uses_light_theme,
                time_format,
                minute_bucket,
                pixels: Arc::clone(&pixels),
            },
        );
        (pixels, true)
    });

    // Swap-chain preview painters normally run only on mount. Settings need
    // true live feedback. Repaint the retained native panel only when preview
    // inputs changed; root settings rerenders must not reinstall identical pixels.
    if pixels_changed {
        TRAY_PREVIEW_MOUNTS.with(|mounts| {
            if let Some(native) = mounts.borrow().get(&preview_id).cloned()
                && let Err(error) =
                    crate::acrylic::install_tray_pixels_into(native, pixels.as_slice())
            {
                eprintln!("Could not update tray preview: {error:?}");
            }
        });
    }

    let pixels_for_mount = Arc::clone(&pixels);
    let id_for_mount = preview_id.clone();
    let id_for_unmount = preview_id.clone();
    let mut host = swap_chain_panel().width(32.0).height(32.0);
    host.mounted = Some(Callback::new(
        move |native: Option<windows_core::IInspectable>| {
            if let Some(native) = native {
                if let Err(error) = crate::acrylic::install_tray_pixels_into(
                    native.clone(),
                    pixels_for_mount.as_slice(),
                ) {
                    eprintln!("Could not install tray preview: {error:?}");
                }
                TRAY_PREVIEW_MOUNTS.with(|mounts| {
                    mounts.borrow_mut().insert(id_for_mount.clone(), native);
                });
            }
        },
    ));
    host.unmounted = Some(Callback::new(
        move |_: Option<windows_core::IInspectable>| {
            TRAY_PREVIEW_MOUNTS.with(|mounts| {
                mounts.borrow_mut().remove(&id_for_unmount);
            });
            TRAY_PREVIEW_CACHE.with(|cache| {
                cache.borrow_mut().remove(&id_for_unmount);
            });
        },
    ));
    let preview: Element = host.into();
    preview.with_key(format!("tray-preview-{preview_id}"))
}

fn tray_presentation_index(presentation: TrayPresentation) -> i32 {
    match presentation.canonical_percentage() {
        TrayPresentation::StackedBars => 1,
        TrayPresentation::NestedRings => 2,
        TrayPresentation::ResetTime => 3,
        TrayPresentation::ResetCountdown => 4,
        _ => 0,
    }
}

fn tray_presentation_from_index(index: i32) -> TrayPresentation {
    match index {
        1 => TrayPresentation::StackedBars,
        2 => TrayPresentation::NestedRings,
        3 => TrayPresentation::ResetTime,
        4 => TrayPresentation::ResetCountdown,
        _ => TrayPresentation::StackedNumbers,
    }
}

fn tray_color_mode_index(mode: TrayColorMode) -> i32 {
    match mode {
        TrayColorMode::Status => 0,
        TrayColorMode::Fixed => 1,
        TrayColorMode::Provider => 2,
        TrayColorMode::Accent => 3,
        TrayColorMode::Monochrome => 4,
    }
}

fn tray_color_mode_from_index(index: i32) -> TrayColorMode {
    match index {
        1 => TrayColorMode::Fixed,
        2 => TrayColorMode::Provider,
        3 => TrayColorMode::Accent,
        4 => TrayColorMode::Monochrome,
        _ => TrayColorMode::Status,
    }
}

// Segoe Fluent chevron glyphs — same family/size as the settings card chevron.
const CHEVRON_UP_GLYPH: &str = "\u{E70E}";
const CHEVRON_DOWN_GLYPH: &str = "\u{E70D}";
const TRAY_REORDER_ICON_FONT: &str = "Segoe Fluent Icons";
/// Match the settings card chevron glyph size.
const TRAY_REORDER_ICON_SIZE: f64 = 12.0;
const TRAY_REORDER_BUTTON_SIZE: f64 = 18.0;

fn open_indicator_edit_modal(
    widget_id: String,
    indicator_index: usize,
    set_editing: AsyncSetState<Option<(String, usize)>>,
    set_visible: AsyncSetState<bool>,
) {
    let anim_id = INDICATOR_MODAL_ANIM_GEN.fetch_add(1, Ordering::Relaxed) + 1;
    set_editing.call(Some((widget_id, indicator_index)));
    set_visible.call(false);
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(16));
        if INDICATOR_MODAL_ANIM_GEN.load(Ordering::Relaxed) != anim_id {
            return;
        }
        set_visible.call(true);
    });
}

fn close_indicator_edit_modal(
    set_editing: AsyncSetState<Option<(String, usize)>>,
    set_visible: AsyncSetState<bool>,
) {
    let anim_id = INDICATOR_MODAL_ANIM_GEN.fetch_add(1, Ordering::Relaxed) + 1;
    set_visible.call(false);
    let wait = duration(CONTROL_NORMAL_ANIMATION);
    if wait.is_zero() {
        set_editing.call(None);
        return;
    }
    thread::spawn(move || {
        thread::sleep(wait);
        if INDICATOR_MODAL_ANIM_GEN.load(Ordering::Relaxed) != anim_id {
            return;
        }
        set_editing.call(None);
    });
}

fn clear_indicator_edit_modal(
    set_editing: AsyncSetState<Option<(String, usize)>>,
    set_visible: AsyncSetState<bool>,
) {
    INDICATOR_MODAL_ANIM_GEN.fetch_add(1, Ordering::Relaxed);
    set_visible.call(false);
    set_editing.call(None);
}

fn tray_settings_cards(
    widgets: &[TrayWidget],
    enabled_providers: &[ProviderKind],
    expanded_widget: &Option<String>,
    editing_indicator: &Option<(String, usize)>,
    removed_widget: &Option<(usize, TrayWidget)>,
    set_widgets: SetState<Vec<TrayWidget>>,
    set_expanded_widget: SetState<Option<String>>,
    set_editing_indicator: AsyncSetState<Option<(String, usize)>>,
    set_indicator_modal_visible: AsyncSetState<bool>,
    set_removed_widget: SetState<Option<(usize, TrayWidget)>>,
    hovered_card_id: &Option<String>,
    set_hovered_card_id: SetState<Option<String>>,
    settings_tx: Sender<Settings>,
) -> Vec<Element> {
    let _ = editing_indicator;
    let mut rows = Vec::new();
    if let Some((removed_index, removed)) = removed_widget.clone() {
        let widgets_for_undo = widgets.to_vec();
        let undo_setter = set_widgets.clone();
        let clear_removed = set_removed_widget.clone();
        let undo_tx = settings_tx.clone();
        let providers_for_undo = enabled_providers.to_vec();
        rows.push(
            border(
                hstack((
                    text_block("Widget removed")
                        .font_size(13.0)
                        .vertical_alignment(VerticalAlignment::Center),
                    Button::new("Undo").on_click(move || {
                        let mut next = widgets_for_undo.clone();
                        next.insert(removed_index.min(next.len()), removed.clone());
                        persist_tray_widgets(
                            undo_setter.clone(),
                            undo_tx.clone(),
                            next,
                            &providers_for_undo,
                        );
                        clear_removed.call(None);
                    }),
                ))
                .spacing(10.0),
            )
            .padding(settings_card_padding())
            .background(ThemeRef::LayerFill)
            .corner_radius(6.0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .with_opacity_transition(duration(CONTROL_FAST_ANIMATION))
            .with_key("tray-widget-undo")
            .into(),
        );
    }
    if widgets.is_empty() {
        rows.push(settings_info_card("Tray icon", "App icon").with_key("tray-empty"));
    }

    for (index, widget) in widgets.iter().cloned().enumerate() {
        let widget_id = widget.id.clone();
        let is_expanded = expanded_widget.as_deref() == Some(widget_id.as_str());
        let expand_id = widget_id.clone();
        let expand_setter = set_expanded_widget.clone();
        let header_id = widget_id.clone();
        let widgets_for_up = widgets.to_vec();
        let up_setter = set_widgets.clone();
        let up_tx = settings_tx.clone();
        let providers_for_up = enabled_providers.to_vec();
        let widgets_for_down = widgets.to_vec();
        let down_setter = set_widgets.clone();
        let down_tx = settings_tx.clone();
        let providers_for_down = enabled_providers.to_vec();

        let reorder_buttons = vstack((
            Button::new(CHEVRON_UP_GLYPH)
                .subtle()
                .font_family(TRAY_REORDER_ICON_FONT)
                .font_size(TRAY_REORDER_ICON_SIZE)
                .width(TRAY_REORDER_BUTTON_SIZE)
                .height(TRAY_REORDER_BUTTON_SIZE)
                .min_width(TRAY_REORDER_BUTTON_SIZE)
                .min_height(TRAY_REORDER_BUTTON_SIZE)
                .max_width(TRAY_REORDER_BUTTON_SIZE)
                .max_height(TRAY_REORDER_BUTTON_SIZE)
                .padding(Thickness::uniform(0.0))
                .enabled(index > 0)
                .tooltip("Move widget up")
                .on_click(move || {
                    if index == 0 {
                        return;
                    }
                    let mut next = widgets_for_up.clone();
                    next.swap(index, index - 1);
                    persist_tray_widgets(up_setter.clone(), up_tx.clone(), next, &providers_for_up);
                }),
            Button::new(CHEVRON_DOWN_GLYPH)
                .subtle()
                .font_family(TRAY_REORDER_ICON_FONT)
                .font_size(TRAY_REORDER_ICON_SIZE)
                .width(TRAY_REORDER_BUTTON_SIZE)
                .height(TRAY_REORDER_BUTTON_SIZE)
                .min_width(TRAY_REORDER_BUTTON_SIZE)
                .min_height(TRAY_REORDER_BUTTON_SIZE)
                .max_width(TRAY_REORDER_BUTTON_SIZE)
                .max_height(TRAY_REORDER_BUTTON_SIZE)
                .padding(Thickness::uniform(0.0))
                .enabled(index + 1 < widgets.len())
                .tooltip("Move widget down")
                .on_click(move || {
                    if index + 1 >= widgets_for_down.len() {
                        return;
                    }
                    let mut next = widgets_for_down.clone();
                    next.swap(index, index + 1);
                    persist_tray_widgets(
                        down_setter.clone(),
                        down_tx.clone(),
                        next,
                        &providers_for_down,
                    );
                }),
        ))
        .spacing(0.0)
        .horizontal_alignment(HorizontalAlignment::Center)
        .vertical_alignment(VerticalAlignment::Center);
        let header = grid((
            reorder_buttons
                .grid_column(0)
                .vertical_alignment(VerticalAlignment::Center),
            tray_widget_preview(&widget)
                .grid_column(1)
                .vertical_alignment(VerticalAlignment::Center),
            vstack((
                text_block(format!("Widget {}", index + 1))
                    .font_size(14.0)
                    .semibold(),
                text_block(tray_widget_summary(&widget))
                    .font_size(12.0)
                    .foreground(ThemeRef::SecondaryText),
            ))
            .spacing(2.0)
            .vertical_alignment(VerticalAlignment::Center)
            .on_tapped({
                let expand_setter = set_expanded_widget.clone();
                let expand_id = widget_id.clone();
                move || {
                    expand_setter.call(if is_expanded {
                        None
                    } else {
                        Some(expand_id.clone())
                    });
                }
            })
            .grid_column(2),
        ))
        .columns([
            GridLength::Pixel(TRAY_REORDER_BUTTON_SIZE),
            GridLength::Pixel(32.0),
            GridLength::Star(1.0),
        ])
        .column_spacing(8.0)
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .with_key(format!("tray-header-{header_id}"));

        // Collapsed cards need headers only. Building their declarative editor trees
        // eagerly allocates many callbacks and clones the complete widget list per action.
        let content: Element = if !is_expanded {
            Element::Empty
        } else if widget.kind == TrayWidgetKind::AppIcon {
            let widgets_for_duplicate = widgets.to_vec();
            let duplicate_setter = set_widgets.clone();
            let duplicate_tx = settings_tx.clone();
            let providers_for_duplicate = enabled_providers.to_vec();
            let widgets_for_remove = widgets.to_vec();
            let remove_setter = set_widgets.clone();
            let removed_setter = set_removed_widget.clone();
            let remove_tx = settings_tx.clone();
            let providers_for_remove = enabled_providers.to_vec();
            hstack((
                Button::new("Duplicate").on_click(move || {
                    let mut next = widgets_for_duplicate.clone();
                    next.insert(index + 1, TrayWidget::app_icon());
                    persist_tray_widgets(
                        duplicate_setter.clone(),
                        duplicate_tx.clone(),
                        next,
                        &providers_for_duplicate,
                    );
                }),
                Button::new("Remove").on_click(move || {
                    let mut next = widgets_for_remove.clone();
                    let removed = next.remove(index);
                    removed_setter.call(Some((index, removed)));
                    persist_tray_widgets(
                        remove_setter.clone(),
                        remove_tx.clone(),
                        next,
                        &providers_for_remove,
                    );
                }),
            ))
            .spacing(8.0)
            .into()
        } else {
            let mut fields = Vec::<Element>::new();

            let widgets_for_presentation = widgets.to_vec();
            let presentation_setter = set_widgets.clone();
            let presentation_tx = settings_tx.clone();
            let providers_for_presentation = enabled_providers.to_vec();
            fields.push(
                ComboBox::new([
                    "Numbers",
                    "Progress bars",
                    "Rings",
                    "Reset time",
                    "Countdown",
                ])
                .header("Appearance")
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .selected_index(tray_presentation_index(widget.presentation))
                .on_selection_changed({
                    let providers_for_empty = enabled_providers.to_vec();
                    move |choice| {
                        let mut next = widgets_for_presentation.clone();
                        next[index].presentation = tray_presentation_from_index(choice);
                        if next[index].presentation.is_reset_clock() {
                            next[index].indicators.truncate(1);
                            if next[index].indicators.is_empty() {
                                let provider = providers_for_empty
                                    .first()
                                    .copied()
                                    .unwrap_or(ProviderKind::Codex);
                                let descriptor = crate::provider_registry::descriptor(provider);
                                let metric = descriptor
                                    .default_tray_metrics
                                    .first()
                                    .copied()
                                    .unwrap_or("unknown");
                                next[index]
                                    .indicators
                                    .push(TrayIndicator::new(provider, metric));
                            }
                        }
                        persist_tray_widgets(
                            presentation_setter.clone(),
                            presentation_tx.clone(),
                            next,
                            &providers_for_presentation,
                        );
                    }
                })
                .into(),
            );

            if widget.presentation.is_reset_clock() {
                fields.push(tray_time_parameter_fields(
                    index,
                    &widget,
                    widgets,
                    enabled_providers,
                    set_widgets.clone(),
                    settings_tx.clone(),
                ));
            } else {
                fields.push(settings_section_heading("Indicators"));
                for (indicator_index, indicator) in widget.indicators.iter().cloned().enumerate() {
                    fields.push(tray_indicator_summary_card(
                        index,
                        indicator_index,
                        &indicator,
                        &widget,
                        widgets,
                        enabled_providers,
                        set_widgets.clone(),
                        set_editing_indicator.clone(),
                        set_indicator_modal_visible.clone(),
                        set_removed_widget.clone(),
                        settings_tx.clone(),
                    ));
                }
            }

            let mut widget_actions = Vec::<Element>::new();
            if !widget.presentation.is_reset_clock() && widget.indicators.len() < 3 {
                let widgets_for_add = widgets.to_vec();
                let add_setter = set_widgets.clone();
                let add_tx = settings_tx.clone();
                let enabled_for_add = enabled_providers.to_vec();
                let edit_added = set_editing_indicator.clone();
                let show_added = set_indicator_modal_visible.clone();
                let widget_id_for_add = widget.id.clone();
                let fallback_provider = widget
                    .indicators
                    .last()
                    .and_then(TrayIndicator::provider)
                    .or_else(|| enabled_providers.first().copied())
                    .unwrap_or(ProviderKind::Codex);
                widget_actions.push(
                    Button::new("Add indicator")
                        .on_click(move || {
                            let descriptor =
                                crate::provider_registry::descriptor(fallback_provider);
                            let metric = descriptor
                                .default_tray_metrics
                                .first()
                                .copied()
                                .unwrap_or("unknown");
                            let mut next = widgets_for_add.clone();
                            let new_index = next[index].indicators.len();
                            next[index]
                                .indicators
                                .push(TrayIndicator::new(fallback_provider, metric));
                            persist_tray_widgets(
                                add_setter.clone(),
                                add_tx.clone(),
                                next,
                                &enabled_for_add,
                            );
                            open_indicator_edit_modal(
                                widget_id_for_add.clone(),
                                new_index,
                                edit_added.clone(),
                                show_added.clone(),
                            );
                        })
                        .into(),
                );
            }

            let widgets_for_duplicate = widgets.to_vec();
            let duplicate_setter = set_widgets.clone();
            let duplicate_tx = settings_tx.clone();
            let enabled_for_duplicate = enabled_providers.to_vec();
            let widgets_for_remove = widgets.to_vec();
            let remove_setter = set_widgets.clone();
            let removed_setter = set_removed_widget.clone();
            let remove_tx = settings_tx.clone();
            let enabled_for_remove = enabled_providers.to_vec();
            let clear_editing = set_editing_indicator.clone();
            let clear_visible = set_indicator_modal_visible.clone();
            let removed_widget_id = widget.id.clone();
            widget_actions.push(
                Button::new("Duplicate")
                    .on_click(move || {
                        let mut next = widgets_for_duplicate.clone();
                        let copy = next[index].duplicate_with_new_id();
                        next.insert(index + 1, copy);
                        persist_tray_widgets(
                            duplicate_setter.clone(),
                            duplicate_tx.clone(),
                            next,
                            &enabled_for_duplicate,
                        );
                    })
                    .into(),
            );
            widget_actions.push(
                Button::new("Remove")
                    .on_click(move || {
                        let mut next = widgets_for_remove.clone();
                        let removed = next.remove(index);
                        clear_indicator_edit_modal(clear_editing.clone(), clear_visible.clone());
                        let _ = removed_widget_id;
                        removed_setter.call(Some((index, removed)));
                        persist_tray_widgets(
                            remove_setter.clone(),
                            remove_tx.clone(),
                            next,
                            &enabled_for_remove,
                        );
                    })
                    .into(),
            );
            fields.push(
                hstack(widget_actions)
                    .spacing(8.0)
                    .horizontal_alignment(HorizontalAlignment::Left)
                    .into(),
            );
            vstack(fields).spacing(10.0).into()
        };

        let row: Element = settings_content_expander(
            header,
            is_expanded,
            move |expanded: bool| {
                expand_setter.call(expanded.then(|| expand_id.clone()));
            },
            format!("tray-widget-{widget_id}"),
            hovered_card_id,
            set_hovered_card_id.clone(),
            content,
        )
        .with_key(format!("tray-widget-{widget_id}"));
        rows.push(row);
    }

    let first_enabled = enabled_providers
        .first()
        .copied()
        .unwrap_or(ProviderKind::Codex);
    let mut add_actions = Vec::<Element>::new();
    let widgets_for_custom = widgets.to_vec();
    let custom_setter = set_widgets.clone();
    let custom_tx = settings_tx.clone();
    let providers_for_custom = enabled_providers.to_vec();
    let expanded_for_custom = set_expanded_widget.clone();
    let widgets_for_app = widgets.to_vec();
    let app_setter = set_widgets;
    let providers_for_app = enabled_providers.to_vec();
    add_actions.push(
        Button::new("Add widget")
            .accent()
            .enabled(!enabled_providers.is_empty())
            .on_click(move || {
                let mut next = widgets_for_custom.clone();
                let widget = TrayWidget::custom_for_provider(first_enabled);
                let id = widget.id.clone();
                next.push(widget);
                persist_tray_widgets(
                    custom_setter.clone(),
                    custom_tx.clone(),
                    next,
                    &providers_for_custom,
                );
                expanded_for_custom.call(Some(id));
            })
            .into(),
    );
    add_actions.push(
        Button::new("Add app icon")
            .on_click(move || {
                let mut next = widgets_for_app.clone();
                next.push(TrayWidget::app_icon());
                persist_tray_widgets(
                    app_setter.clone(),
                    settings_tx.clone(),
                    next,
                    &providers_for_app,
                );
            })
            .into(),
    );
    rows.push(
        hstack(add_actions)
            .spacing(8.0)
            .horizontal_alignment(HorizontalAlignment::Left)
            .with_key("tray-add-actions")
            .into(),
    );
    rows
}

fn tray_time_parameter_fields(
    widget_index: usize,
    widget: &TrayWidget,
    widgets: &[TrayWidget],
    enabled_providers: &[ProviderKind],
    set_widgets: SetState<Vec<TrayWidget>>,
    settings_tx: Sender<Settings>,
) -> Element {
    let indicator = widget.indicators.first().cloned().unwrap_or_else(|| {
        let provider = enabled_providers
            .first()
            .copied()
            .unwrap_or(ProviderKind::Codex);
        let descriptor = crate::provider_registry::descriptor(provider);
        let metric = descriptor
            .default_tray_metrics
            .first()
            .copied()
            .unwrap_or("unknown");
        TrayIndicator::new(provider, metric)
    });
    let indicator_index = 0usize;

    let provider_options: Vec<_> = crate::provider_registry::PROVIDERS
        .iter()
        .filter(|provider| !provider.default_tray_metrics.is_empty())
        .collect();
    let known_provider = indicator.provider();
    let mut provider_labels = provider_options
        .iter()
        .map(|provider| provider.display_name.to_owned())
        .collect::<Vec<_>>();
    let provider_index = known_provider
        .and_then(|provider| {
            provider_options
                .iter()
                .position(|descriptor| descriptor.kind == provider)
        })
        .unwrap_or_else(|| {
            provider_labels.push(format!("Unsupported ({})", indicator.provider_id));
            provider_labels.len() - 1
        }) as i32;
    let metric_provider = known_provider.unwrap_or(ProviderKind::Codex);
    let metrics = crate::provider_registry::descriptor(metric_provider).metrics;
    let mut metric_labels = metrics
        .iter()
        .map(|metric| metric.label.to_owned())
        .collect::<Vec<_>>();
    let metric_index = metrics
        .iter()
        .position(|metric| metric.id == indicator.metric_id)
        .unwrap_or_else(|| {
            metric_labels.push(format!("Unavailable ({})", indicator.metric_id));
            metric_labels.len() - 1
        }) as i32;

    let widgets_for_provider = widgets.to_vec();
    let provider_setter = set_widgets.clone();
    let provider_tx = settings_tx.clone();
    let enabled_for_provider = enabled_providers.to_vec();
    let provider_box = ComboBox::new(provider_labels)
        .header("Provider")
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .selected_index(provider_index)
        .on_selection_changed(move |choice: i32| {
            let Some(descriptor) = crate::provider_registry::PROVIDERS.get(choice.max(0) as usize)
            else {
                return;
            };
            let mut next = widgets_for_provider.clone();
            if next[widget_index].indicators.is_empty() {
                next[widget_index]
                    .indicators
                    .push(TrayIndicator::new(descriptor.kind, "unknown"));
            }
            next[widget_index].indicators[indicator_index].provider_id = descriptor.id.into();
            next[widget_index].indicators[indicator_index].metric_id = descriptor
                .default_tray_metrics
                .first()
                .copied()
                .unwrap_or("unknown")
                .into();
            persist_tray_widgets(
                provider_setter.clone(),
                provider_tx.clone(),
                next,
                &enabled_for_provider,
            );
        });

    let widgets_for_metric = widgets.to_vec();
    let metric_setter = set_widgets.clone();
    let metric_tx = settings_tx.clone();
    let enabled_for_metric = enabled_providers.to_vec();
    let metric_box = ComboBox::new(metric_labels)
        .header("Metric")
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .selected_index(metric_index)
        .with_key(format!(
            "tray-time-metric-{}-{}",
            widget.id, indicator.provider_id
        ))
        .on_selection_changed(move |choice: i32| {
            let Some(metric) = metrics.get(choice.max(0) as usize) else {
                return;
            };
            let mut next = widgets_for_metric.clone();
            if next[widget_index].indicators.is_empty() {
                return;
            }
            next[widget_index].indicators[indicator_index].metric_id = metric.id.into();
            persist_tray_widgets(
                metric_setter.clone(),
                metric_tx.clone(),
                next,
                &enabled_for_metric,
            );
        });

    let widgets_for_color = widgets.to_vec();
    let color_setter = set_widgets.clone();
    let color_tx = settings_tx.clone();
    let providers_for_color = enabled_providers.to_vec();
    let color_box = ComboBox::new(["Status", "Fixed", "Provider", "App accent", "Monochrome"])
        .header("Color")
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .selected_index(tray_color_mode_index(indicator.color_mode))
        .on_selection_changed(move |choice| {
            let mut next = widgets_for_color.clone();
            if next[widget_index].indicators.is_empty() {
                return;
            }
            next[widget_index].indicators[indicator_index].color_mode =
                tray_color_mode_from_index(choice);
            persist_tray_widgets(
                color_setter.clone(),
                color_tx.clone(),
                next,
                &providers_for_color,
            );
        });

    let mut fields = Vec::<Element>::new();
    fields.push(
        grid((
            provider_box.grid_column(0).grid_row(0),
            metric_box.grid_column(1).grid_row(0),
            color_box.grid_column(0).grid_row(1).grid_column_span(2),
        ))
        .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
        .rows([GridLength::Auto, GridLength::Auto])
        .column_spacing(12.0)
        .row_spacing(12.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into(),
    );

    if indicator.color_mode == TrayColorMode::Fixed {
        let widgets_for_picker = widgets.to_vec();
        let picker_setter = set_widgets;
        let picker_tx = settings_tx;
        let providers_for_picker = enabled_providers.to_vec();
        fields.push(
            border(
                ColorPicker::new(ColorArgb::new(
                    indicator.fixed_color.red,
                    indicator.fixed_color.green,
                    indicator.fixed_color.blue,
                ))
                .alpha_enabled(false)
                .hex_input_visible(true)
                .color_slider_visible(true)
                .color_channel_text_input_visible(false)
                .on_color_changed(move |(_, red, green, blue)| {
                    let mut next = widgets_for_picker.clone();
                    if next[widget_index].indicators.is_empty() {
                        return;
                    }
                    next[widget_index].indicators[indicator_index].fixed_color =
                        TrayFixedColor { red, green, blue };
                    persist_tray_widgets(
                        picker_setter.clone(),
                        picker_tx.clone(),
                        next,
                        &providers_for_picker,
                    );
                }),
            )
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .min_height(180.0)
            .into(),
        );
    }

    vstack(fields)
        .spacing(12.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .with_key(format!("tray-time-fields-{}", widget.id))
        .into()
}

fn tray_indicator_summary_card(
    widget_index: usize,
    indicator_index: usize,
    indicator: &TrayIndicator,
    widget: &TrayWidget,
    widgets: &[TrayWidget],
    enabled_providers: &[ProviderKind],
    set_widgets: SetState<Vec<TrayWidget>>,
    set_editing_indicator: AsyncSetState<Option<(String, usize)>>,
    set_indicator_modal_visible: AsyncSetState<bool>,
    set_removed_widget: SetState<Option<(usize, TrayWidget)>>,
    settings_tx: Sender<Settings>,
) -> Element {
    let widget_id = widget.id.clone();
    let widgets_for_up = widgets.to_vec();
    let up_setter = set_widgets.clone();
    let up_tx = settings_tx.clone();
    let enabled_for_up = enabled_providers.to_vec();
    let widgets_for_down = widgets.to_vec();
    let down_setter = set_widgets.clone();
    let down_tx = settings_tx.clone();
    let enabled_for_down = enabled_providers.to_vec();
    let widgets_for_remove = widgets.to_vec();
    let remove_setter = set_widgets;
    let removed_setter = set_removed_widget;
    let remove_tx = settings_tx;
    let enabled_for_remove = enabled_providers.to_vec();
    let clear_editing = set_editing_indicator.clone();
    let clear_visible = set_indicator_modal_visible.clone();
    let open_editor = set_editing_indicator;
    let show_editor = set_indicator_modal_visible;
    let edit_widget_id = widget_id.clone();

    let reorder = vstack((
        Button::new(CHEVRON_UP_GLYPH)
            .subtle()
            .font_family(TRAY_REORDER_ICON_FONT)
            .font_size(TRAY_REORDER_ICON_SIZE)
            .width(TRAY_REORDER_BUTTON_SIZE)
            .height(TRAY_REORDER_BUTTON_SIZE)
            .min_width(TRAY_REORDER_BUTTON_SIZE)
            .min_height(TRAY_REORDER_BUTTON_SIZE)
            .max_width(TRAY_REORDER_BUTTON_SIZE)
            .max_height(TRAY_REORDER_BUTTON_SIZE)
            .padding(Thickness::uniform(0.0))
            .enabled(indicator_index > 0)
            .tooltip("Move indicator up")
            .on_click(move || {
                if indicator_index == 0 {
                    return;
                }
                let mut next = widgets_for_up.clone();
                next[widget_index]
                    .indicators
                    .swap(indicator_index, indicator_index - 1);
                persist_tray_widgets(up_setter.clone(), up_tx.clone(), next, &enabled_for_up);
            }),
        Button::new(CHEVRON_DOWN_GLYPH)
            .subtle()
            .font_family(TRAY_REORDER_ICON_FONT)
            .font_size(TRAY_REORDER_ICON_SIZE)
            .width(TRAY_REORDER_BUTTON_SIZE)
            .height(TRAY_REORDER_BUTTON_SIZE)
            .min_width(TRAY_REORDER_BUTTON_SIZE)
            .min_height(TRAY_REORDER_BUTTON_SIZE)
            .max_width(TRAY_REORDER_BUTTON_SIZE)
            .max_height(TRAY_REORDER_BUTTON_SIZE)
            .padding(Thickness::uniform(0.0))
            .enabled(indicator_index + 1 < widget.indicators.len())
            .tooltip("Move indicator down")
            .on_click(move || {
                let mut next = widgets_for_down.clone();
                if indicator_index + 1 >= next[widget_index].indicators.len() {
                    return;
                }
                next[widget_index]
                    .indicators
                    .swap(indicator_index, indicator_index + 1);
                persist_tray_widgets(
                    down_setter.clone(),
                    down_tx.clone(),
                    next,
                    &enabled_for_down,
                );
            }),
    ))
    .spacing(0.0)
    .vertical_alignment(VerticalAlignment::Center);

    border(
        grid((
            reorder
                .grid_column(0)
                .vertical_alignment(VerticalAlignment::Center),
            vstack((
                text_block(tray_indicator_summary(indicator))
                    .font_size(14.0)
                    .semibold()
                    .wrap(),
                text_block(format!("Indicator {}", indicator_index + 1))
                    .font_size(12.0)
                    .foreground(ThemeRef::SecondaryText),
            ))
            .spacing(2.0)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(1),
            hstack((
                Button::new("Edit").on_click(move || {
                    open_indicator_edit_modal(
                        edit_widget_id.clone(),
                        indicator_index,
                        open_editor.clone(),
                        show_editor.clone(),
                    );
                }),
                Button::new("Remove").on_click(move || {
                    let mut next = widgets_for_remove.clone();
                    next[widget_index].indicators.remove(indicator_index);
                    if next[widget_index].indicators.is_empty() {
                        let removed = next.remove(widget_index);
                        clear_indicator_edit_modal(clear_editing.clone(), clear_visible.clone());
                        removed_setter.call(Some((widget_index, removed)));
                    } else {
                        clear_indicator_edit_modal(clear_editing.clone(), clear_visible.clone());
                    }
                    persist_tray_widgets(
                        remove_setter.clone(),
                        remove_tx.clone(),
                        next,
                        &enabled_for_remove,
                    );
                }),
            ))
            .spacing(8.0)
            .vertical_alignment(VerticalAlignment::Center)
            .horizontal_alignment(HorizontalAlignment::Right)
            .grid_column(2),
        ))
        .columns([
            GridLength::Pixel(TRAY_REORDER_BUTTON_SIZE),
            GridLength::Star(1.0),
            GridLength::Auto,
        ])
        .column_spacing(12.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(settings_card_padding())
    .background(ThemeRef::CardBackground)
    .corner_radius(8.0)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .min_height(60.0)
    .with_key(format!("tray-indicator-{}-{indicator_index}", widget.id))
    .with_translation_transition(duration(CONTROL_FAST_ANIMATION))
    .into()
}

pub(super) fn tray_indicator_edit_overlay(
    widgets: &[TrayWidget],
    enabled_providers: &[ProviderKind],
    editing: &(String, usize),
    visible: bool,
    set_widgets: SetState<Vec<TrayWidget>>,
    set_editing_indicator: AsyncSetState<Option<(String, usize)>>,
    set_indicator_modal_visible: AsyncSetState<bool>,
    settings_tx: Sender<Settings>,
) -> Option<Element> {
    let (widget_id, indicator_index) = editing;
    let (widget_index, widget) = widgets
        .iter()
        .cloned()
        .enumerate()
        .find(|(_, widget)| widget.id == *widget_id)?;
    let indicator = widget.indicators.get(*indicator_index)?.clone();

    let dismiss_editing = set_editing_indicator.clone();
    let dismiss_visible = set_indicator_modal_visible.clone();
    let form = tray_indicator_edit_form(
        widget_index,
        *indicator_index,
        &indicator,
        &widget,
        widgets,
        enabled_providers,
        set_widgets,
        set_editing_indicator,
        set_indicator_modal_visible,
        settings_tx,
    );

    let anim = duration(CONTROL_NORMAL_ANIMATION);
    let card = border(
        scroll_viewer(border(form).padding(Thickness {
            left: 24.0,
            top: 24.0,
            right: 24.0,
            bottom: 24.0,
        }))
        .horizontal_scroll_bar_visibility(ScrollBarVisibility::Disabled)
        .vertical_scroll_bar_visibility(ScrollBarVisibility::Auto)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Stretch),
    )
    .background(ThemeRef::SolidBackground)
    .corner_radius(INDICATOR_MODAL_RADIUS)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .width(INDICATOR_MODAL_WIDTH)
    .horizontal_alignment(HorizontalAlignment::Center)
    .vertical_alignment(VerticalAlignment::Stretch)
    .on_tapped(|| {})
    .with_key(format!(
        "tray-indicator-modal-{}-{indicator_index}",
        widget.id
    ));

    // 1+18+1 star rows → middle band is 90% of the window height.
    Some(
        relative_panel::<Vec<Element>>(vec![
            border(Element::Empty)
                .background(INDICATOR_MODAL_SCRIM)
                .opacity(if visible { 1.0 } else { 0.0 })
                .relative_align_left()
                .relative_align_right()
                .relative_align_top()
                .relative_align_bottom()
                .on_tapped(move || {
                    close_indicator_edit_modal(dismiss_editing.clone(), dismiss_visible.clone());
                })
                .with_opacity_transition(anim)
                .into(),
            grid((
                border(Element::Empty).grid_row(0),
                card.opacity(if visible { 1.0 } else { 0.0 })
                    .with_opacity_transition(anim)
                    .grid_row(1),
                border(Element::Empty).grid_row(2),
            ))
            .columns([GridLength::Star(1.0)])
            .rows([
                GridLength::Star(1.0),
                GridLength::Star(18.0),
                GridLength::Star(1.0),
            ])
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch)
            .relative_align_left()
            .relative_align_right()
            .relative_align_top()
            .relative_align_bottom()
            .into(),
        ])
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Stretch)
        .with_key("tray-indicator-edit-overlay")
        .into(),
    )
}

fn tray_indicator_edit_form(
    widget_index: usize,
    indicator_index: usize,
    indicator: &TrayIndicator,
    widget: &TrayWidget,
    widgets: &[TrayWidget],
    enabled_providers: &[ProviderKind],
    set_widgets: SetState<Vec<TrayWidget>>,
    set_editing_indicator: AsyncSetState<Option<(String, usize)>>,
    set_indicator_modal_visible: AsyncSetState<bool>,
    settings_tx: Sender<Settings>,
) -> Element {
    let provider_options: Vec<_> = crate::provider_registry::PROVIDERS
        .iter()
        .filter(|provider| !provider.default_tray_metrics.is_empty())
        .collect();
    let known_provider = indicator.provider();
    let mut provider_labels = provider_options
        .iter()
        .map(|provider| provider.display_name.to_owned())
        .collect::<Vec<_>>();
    let provider_index = known_provider
        .and_then(|provider| {
            provider_options
                .iter()
                .position(|descriptor| descriptor.kind == provider)
        })
        .unwrap_or_else(|| {
            provider_labels.push(format!("Unsupported ({})", indicator.provider_id));
            provider_labels.len() - 1
        }) as i32;
    let metric_provider = known_provider.unwrap_or(ProviderKind::Codex);
    let metrics = crate::provider_registry::descriptor(metric_provider).metrics;
    let mut metric_labels = metrics
        .iter()
        .map(|metric| metric.label.to_owned())
        .collect::<Vec<_>>();
    let metric_index = metrics
        .iter()
        .position(|metric| metric.id == indicator.metric_id)
        .unwrap_or_else(|| {
            metric_labels.push(format!("Unavailable ({})", indicator.metric_id));
            metric_labels.len() - 1
        }) as i32;

    let mut fields = Vec::<Element>::new();
    let close_editing = set_editing_indicator;
    let close_visible = set_indicator_modal_visible;
    fields.push(
        grid((
            hstack((
                tray_widget_preview(widget).vertical_alignment(VerticalAlignment::Center),
                text_block(format!("Edit indicator {}", indicator_index + 1))
                    .font_size(24.0)
                    .bold()
                    .vertical_alignment(VerticalAlignment::Center),
            ))
            .spacing(12.0)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(0),
            Button::new("Done")
                .accent()
                .on_click(move || {
                    close_indicator_edit_modal(close_editing.clone(), close_visible.clone());
                })
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .column_spacing(12.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into(),
    );

    fields.push(
        text_block(tray_indicator_summary(indicator))
            .font_size(14.0)
            .foreground(ThemeRef::SecondaryText)
            .into(),
    );

    let widgets_for_provider = widgets.to_vec();
    let provider_setter = set_widgets.clone();
    let provider_tx = settings_tx.clone();
    let enabled_for_provider = enabled_providers.to_vec();
    let provider_box = ComboBox::new(provider_labels)
        .header("Provider")
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .selected_index(provider_index)
        .on_selection_changed(move |choice: i32| {
            let Some(descriptor) = crate::provider_registry::PROVIDERS.get(choice.max(0) as usize)
            else {
                return;
            };
            let mut next = widgets_for_provider.clone();
            next[widget_index].indicators[indicator_index].provider_id = descriptor.id.into();
            next[widget_index].indicators[indicator_index].metric_id = descriptor
                .default_tray_metrics
                .first()
                .copied()
                .unwrap_or("unknown")
                .into();
            persist_tray_widgets(
                provider_setter.clone(),
                provider_tx.clone(),
                next,
                &enabled_for_provider,
            );
        });

    let widgets_for_metric = widgets.to_vec();
    let metric_setter = set_widgets.clone();
    let metric_tx = settings_tx.clone();
    let enabled_for_metric = enabled_providers.to_vec();
    let metric_box = ComboBox::new(metric_labels)
        .header("Metric")
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .selected_index(metric_index)
        .with_key(format!(
            "tray-metric-modal-{}-{indicator_index}-{}",
            widget.id, indicator.provider_id
        ))
        .on_selection_changed(move |choice: i32| {
            let Some(metric) = metrics.get(choice.max(0) as usize) else {
                return;
            };
            let mut next = widgets_for_metric.clone();
            next[widget_index].indicators[indicator_index].metric_id = metric.id.into();
            persist_tray_widgets(
                metric_setter.clone(),
                metric_tx.clone(),
                next,
                &enabled_for_metric,
            );
        });

    let widgets_for_value = widgets.to_vec();
    let value_setter = set_widgets.clone();
    let value_tx = settings_tx.clone();
    let enabled_for_value = enabled_providers.to_vec();
    let value_box = ComboBox::new(["Remaining", "Used"])
        .header("Value")
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .selected_index(if indicator.limit_value == LimitValue::Remaining {
            0
        } else {
            1
        })
        .on_selection_changed(move |choice| {
            let mut next = widgets_for_value.clone();
            next[widget_index].indicators[indicator_index].limit_value = if choice == 1 {
                LimitValue::Used
            } else {
                LimitValue::Remaining
            };
            persist_tray_widgets(
                value_setter.clone(),
                value_tx.clone(),
                next,
                &enabled_for_value,
            );
        });

    // Color is per-indicator so stacked tray glyphs can use different palettes.
    let widgets_for_color = widgets.to_vec();
    let color_setter = set_widgets.clone();
    let color_tx = settings_tx.clone();
    let providers_for_color = enabled_providers.to_vec();
    let color_box = ComboBox::new(["Status", "Fixed", "Provider", "App accent", "Monochrome"])
        .header("Color")
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .selected_index(tray_color_mode_index(indicator.color_mode))
        .on_selection_changed(move |choice| {
            let mut next = widgets_for_color.clone();
            next[widget_index].indicators[indicator_index].color_mode =
                tray_color_mode_from_index(choice);
            persist_tray_widgets(
                color_setter.clone(),
                color_tx.clone(),
                next,
                &providers_for_color,
            );
        });

    fields.push(
        grid((
            provider_box.grid_column(0).grid_row(0),
            metric_box.grid_column(1).grid_row(0),
            value_box.grid_column(0).grid_row(1),
            color_box.grid_column(1).grid_row(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
        .rows([GridLength::Auto, GridLength::Auto])
        .column_spacing(12.0)
        .row_spacing(12.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into(),
    );

    if indicator.color_mode == TrayColorMode::Fixed {
        let widgets_for_picker = widgets.to_vec();
        let picker_setter = set_widgets;
        let picker_tx = settings_tx;
        let providers_for_picker = enabled_providers.to_vec();
        fields.push(
            border(
                ColorPicker::new(ColorArgb::new(
                    indicator.fixed_color.red,
                    indicator.fixed_color.green,
                    indicator.fixed_color.blue,
                ))
                .alpha_enabled(false)
                .hex_input_visible(true)
                .color_slider_visible(true)
                .color_channel_text_input_visible(false)
                .on_color_changed(move |(_, red, green, blue)| {
                    let mut next = widgets_for_picker.clone();
                    next[widget_index].indicators[indicator_index].fixed_color =
                        TrayFixedColor { red, green, blue };
                    persist_tray_widgets(
                        picker_setter.clone(),
                        picker_tx.clone(),
                        next,
                        &providers_for_picker,
                    );
                }),
            )
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .min_height(220.0)
            .into(),
        );
    }

    vstack(fields)
        .spacing(16.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .with_key(format!(
            "tray-indicator-form-{}-{indicator_index}",
            widget.id
        ))
        .into()
}

fn persist_tray_widgets(
    setter: SetState<Vec<TrayWidget>>,
    settings_tx: Sender<Settings>,
    widgets: Vec<TrayWidget>,
    _enabled_providers: &[ProviderKind],
) {
    let mut widgets = widgets;
    for widget in &mut widgets {
        widget.normalize();
    }
    setter.call(widgets.clone());
    persist_update(settings_tx, move |settings| settings.tray_widgets = widgets);
}
