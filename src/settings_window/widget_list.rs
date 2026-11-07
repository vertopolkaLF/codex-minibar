//! The widget lists Settings edits (tray icons, taskbar widgets and Command
//! Palette Dock bands) and the editor they share: list rows with a move,
//! duplicate and remove menu, undo after a removal, the editor dialog's
//! frame, indicator cards, style tiles and sample quotas for previews.

use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};

use super::Host;
use super::window::SettingsWindow;
use crate::gui::color::rgb8;
use crate::gui::kit::{self, Button, Kit, Row, eid};
use crate::limits::{AdditionalLimit, LimitWindow, ProviderLimits, RateLimits, SpendingSummary};
use crate::provider_registry::{MetricSource, ProviderKind};
use crate::settings::{
    Choice, LimitValue, ProviderId, ProviderInstance, Settings, TaskbarWidget, TaskbarWidgetStyle,
    TrayColorMode, TrayFixedColor, TrayIndicator, TrayWidget, TrayWidgetKind,
};

/// One widget list in the settings file.
pub(super) trait WidgetList: 'static {
    type Widget: Clone + 'static;
    /// Prefix of the list's element ids.
    const TAG: &'static str;
    /// Whether indicators pick a color; the Dock draws band text in its own.
    const COLORED: bool = true;
    fn widgets(settings: &Settings) -> &Vec<Self::Widget>;
    fn widgets_mut(settings: &mut Settings) -> &mut Vec<Self::Widget>;
    fn editor(window: &mut SettingsWindow) -> &mut ListEditor<Self::Widget>;
    fn id(widget: &Self::Widget) -> &str;
    fn indicators_mut(widget: &mut Self::Widget) -> &mut Vec<TrayIndicator>;
    fn duplicate(widget: &Self::Widget) -> Self::Widget;
    fn normalize(widget: &mut Self::Widget);
    /// The undo banner's message.
    fn removed_message() -> &'static str;
    /// The editor's remove button.
    fn remove_label() -> &'static str;
}

pub(super) struct Tray;
pub(super) struct Taskbar;
/// Command Palette Dock bands: Glance taskbar widgets in their own list.
pub(super) struct Dock;

impl WidgetList for Tray {
    type Widget = TrayWidget;
    const TAG: &'static str = "tray";

    fn widgets(settings: &Settings) -> &Vec<TrayWidget> {
        &settings.tray_widgets
    }

    fn widgets_mut(settings: &mut Settings) -> &mut Vec<TrayWidget> {
        &mut settings.tray_widgets
    }

    fn editor(window: &mut SettingsWindow) -> &mut ListEditor<TrayWidget> {
        &mut window.tray_list
    }

    fn id(widget: &TrayWidget) -> &str {
        &widget.id
    }

    fn indicators_mut(widget: &mut TrayWidget) -> &mut Vec<TrayIndicator> {
        &mut widget.indicators
    }

    fn duplicate(widget: &TrayWidget) -> TrayWidget {
        if widget.kind == TrayWidgetKind::AppIcon {
            TrayWidget::app_icon()
        } else {
            widget.duplicate_with_new_id()
        }
    }

    fn normalize(widget: &mut TrayWidget) {
        widget.normalize();
    }

    fn removed_message() -> &'static str {
        crate::i18n::tr("widget-removed")
    }

    fn remove_label() -> &'static str {
        crate::i18n::tr("remove-widget")
    }
}

impl WidgetList for Taskbar {
    type Widget = TaskbarWidget;
    const TAG: &'static str = "taskbar";

    fn widgets(settings: &Settings) -> &Vec<TaskbarWidget> {
        &settings.taskbar.widgets
    }

    fn widgets_mut(settings: &mut Settings) -> &mut Vec<TaskbarWidget> {
        &mut settings.taskbar.widgets
    }

    fn editor(window: &mut SettingsWindow) -> &mut ListEditor<TaskbarWidget> {
        &mut window.taskbar_list
    }

    fn id(widget: &TaskbarWidget) -> &str {
        &widget.id
    }

    fn indicators_mut(widget: &mut TaskbarWidget) -> &mut Vec<TrayIndicator> {
        &mut widget.indicators
    }

    fn duplicate(widget: &TaskbarWidget) -> TaskbarWidget {
        widget.duplicate_with_new_id()
    }

    fn normalize(widget: &mut TaskbarWidget) {
        widget.normalize();
    }

    fn removed_message() -> &'static str {
        crate::i18n::tr("widget-removed")
    }

    fn remove_label() -> &'static str {
        crate::i18n::tr("remove-widget")
    }
}

impl WidgetList for Dock {
    type Widget = TaskbarWidget;
    const TAG: &'static str = "dock";
    const COLORED: bool = false;

    fn widgets(settings: &Settings) -> &Vec<TaskbarWidget> {
        &settings.command_palette.widgets
    }

    fn widgets_mut(settings: &mut Settings) -> &mut Vec<TaskbarWidget> {
        &mut settings.command_palette.widgets
    }

    fn editor(window: &mut SettingsWindow) -> &mut ListEditor<TaskbarWidget> {
        &mut window.dock_list
    }

    fn id(widget: &TaskbarWidget) -> &str {
        &widget.id
    }

    fn indicators_mut(widget: &mut TaskbarWidget) -> &mut Vec<TrayIndicator> {
        &mut widget.indicators
    }

    fn duplicate(widget: &TaskbarWidget) -> TaskbarWidget {
        widget.dock_duplicate_with_new_id()
    }

    fn normalize(widget: &mut TaskbarWidget) {
        widget.style = TaskbarWidgetStyle::Glance;
        widget.normalize();
    }

    fn removed_message() -> &'static str {
        crate::i18n::tr("cmdpal-band-removed")
    }

    fn remove_label() -> &'static str {
        crate::i18n::tr("cmdpal-remove-band")
    }
}

/// The open widget editor: which widget and which indicator is expanded.
#[derive(Clone)]
pub(crate) struct WidgetDialog {
    pub(super) widget_id: String,
    pub(super) expanded: Option<usize>,
}

impl WidgetDialog {
    fn new(widget_id: String) -> Self {
        Self {
            widget_id,
            expanded: Some(0),
        }
    }
}

/// Editor state of one widget list.
pub(crate) struct ListEditor<W> {
    pub(super) dialog: Option<WidgetDialog>,
    /// The last removed widget and its position, for Undo.
    removed: Option<(usize, W)>,
    /// The dialog's last shown state, kept while it fades out.
    overlay: kit::Presence<(WidgetDialog, usize, W)>,
}

impl<W> Default for ListEditor<W> {
    fn default() -> Self {
        Self {
            dialog: None,
            removed: None,
            overlay: kit::Presence::default(),
        }
    }
}

/// Instances a widget indicator can read, in display order: those whose
/// driver has tray metrics. `enabled_only` drops the ones turned off.
pub(super) fn widget_providers(
    instances: &[ProviderInstance],
    enabled_only: bool,
) -> Vec<ProviderId> {
    instances
        .iter()
        .filter(|instance| instance.enabled || !enabled_only)
        .filter(|instance| {
            !crate::provider_registry::descriptor(instance.driver)
                .default_tray_metrics
                .is_empty()
        })
        .map(ProviderInstance::provider_id)
        .collect()
}

pub(super) fn default_indicator(provider: ProviderId) -> TrayIndicator {
    let metric = crate::provider_registry::descriptor(provider.kind())
        .default_tray_metrics
        .first()
        .copied()
        .unwrap_or("unknown");
    TrayIndicator::new(provider, metric)
}

/// Used percentages handed out to sample windows, in metric order.
const SAMPLE_USED: [u8; 8] = [38, 70, 55, 12, 18, 47, 64, 1];

/// Sample quotas for one driver: every declared metric filled in, or a
/// balance for drivers that show one instead of quota.
fn sample_limits(kind: ProviderKind) -> RateLimits {
    let descriptor = crate::provider_registry::descriptor(kind);
    let mut limits = RateLimits::default();
    if descriptor.balance_only {
        limits.spending = Some(SpendingSummary {
            balance_microusd: Some(24_600_000),
            ..Default::default()
        });
        return limits;
    }
    // A reset a little over two hours out, so reset clocks show a time.
    let resets_at = Some(chrono::Utc::now() + chrono::Duration::minutes(137));
    for (index, metric) in descriptor.metrics.iter().enumerate() {
        let window = LimitWindow {
            used_percent: Some(SAMPLE_USED[(kind as usize * 2 + index) % SAMPLE_USED.len()]),
            resets_at,
            duration_minutes: Some(300),
        };
        match metric.source {
            MetricSource::Primary => limits.primary = window,
            MetricSource::Secondary => limits.secondary = window,
            MetricSource::Additional(id) => limits.additional_limits.push(AdditionalLimit {
                id: id.into(),
                title: metric.localized_label().into(),
                window,
            }),
        }
    }
    limits
}

/// Sample quotas for previews, for every configured instance and every
/// driver's primary instance, so indicators of any provider render.
pub(super) fn preview_limits(instances: &[ProviderInstance]) -> ProviderLimits {
    ProviderLimits::from_entries(
        instances
            .iter()
            .map(ProviderInstance::provider_id)
            .chain(ProviderKind::ALL.into_iter().map(ProviderId::primary))
            .map(|provider| (provider, sample_limits(provider.kind()))),
    )
}

/// A dialog section title with a one-line hint under it.
pub(super) fn dialog_section(
    k: &Kit,
    title: &'static str,
    hint: impl Into<SharedString>,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .pt(px(6.0))
        .child(
            div()
                .text_size(px(14.0))
                .line_height(px(20.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(k.theme.text)
                .child(title),
        )
        .child(kit::caption(k, hint))
        .into_any_element()
}

/// One choice of a style picker: a preview above its label. The caller sizes
/// and pads the tile and handles clicks.
pub(super) fn style_tile(
    k: &Kit,
    id: String,
    selected: bool,
    preview: AnyElement,
    label: &'static str,
) -> gpui::Stateful<gpui::Div> {
    let theme = &k.theme;
    let rest = if selected {
        theme.accent_soft
    } else {
        theme.control
    };
    let hover = if selected { rest } else { theme.control_hover };
    let tile = div()
        .id(eid(id.clone()))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .rounded(px(kit::CARD_RADIUS))
        .border_1()
        .border_color(if selected {
            theme.accent
        } else {
            gpui::transparent_black()
        })
        .child(preview)
        .child(
            div()
                .w_full()
                .text_center()
                .text_size(px(12.0))
                .line_height(px(16.0))
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .when(selected, |el| el.font_weight(FontWeight::SEMIBOLD))
                .text_color(if selected {
                    theme.accent_text
                } else {
                    theme.text
                })
                .child(label),
        );
    kit::hover_bg(k, tile, kit::hover_key(&id), rest, hover).cursor_pointer()
}

fn color_mode_label(mode: TrayColorMode) -> &'static str {
    match mode {
        TrayColorMode::Status => crate::i18n::tr("status"),
        TrayColorMode::Fixed => crate::i18n::tr("fixed"),
        TrayColorMode::Provider => crate::i18n::tr("provider"),
        TrayColorMode::Accent => crate::i18n::tr("app-accent"),
        TrayColorMode::Monochrome => crate::i18n::tr("monochrome"),
    }
}

/// Applies a dropdown choice to an indicator, given the provider and metric
/// options the dropdown listed.
type IndicatorEdit = fn(&mut TrayIndicator, usize, &[ProviderId], &[(String, String)]);

/// Fixed-color presets offered next to the hex field.
const SWATCHES: [(u8, u8, u8); 12] = [
    (0xFF, 0xFF, 0xFF),
    (0x1F, 0x1F, 0x1F),
    (0x00, 0x78, 0xD4),
    (0x88, 0x6C, 0xE4),
    (0xE3, 0x00, 0x8C),
    (0xE8, 0x11, 0x23),
    (0xF7, 0x63, 0x0C),
    (0xFF, 0xB9, 0x00),
    (0x10, 0x7C, 0x10),
    (0x00, 0xB2, 0x94),
    (0x00, 0x99, 0xBC),
    (0x7A, 0x75, 0x74),
];

impl SettingsWindow {
    /// Edits one list in the settings snapshot.
    pub(super) fn update_widgets<L: WidgetList>(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut Vec<L::Widget>),
    ) {
        self.edit(cx, |settings| {
            let widgets = L::widgets_mut(settings);
            f(widgets);
            widgets.iter_mut().for_each(L::normalize);
        });
    }

    /// Opens the editor of the widget `id`, if it still exists.
    pub(super) fn open_widget<L: WidgetList>(&mut self, id: String, cx: &mut Context<Self>) {
        if L::widgets(&self.settings)
            .iter()
            .any(|widget| L::id(widget) == id)
        {
            L::editor(self).dialog = Some(WidgetDialog::new(id));
            cx.notify();
        }
    }

    /// Appends `widget` and opens its editor.
    pub(super) fn add_widget<L: WidgetList>(&mut self, widget: L::Widget, cx: &mut Context<Self>) {
        L::editor(self).dialog = Some(WidgetDialog::new(L::id(&widget).to_owned()));
        self.update_widgets::<L>(cx, |widgets| widgets.push(widget));
    }

    /// Closes the first open widget editor; whether one was open.
    pub(super) fn close_widget_dialog(&mut self) -> bool {
        self.tray_list.dialog.take().is_some()
            || self.taskbar_list.dialog.take().is_some()
            || self.dock_list.dialog.take().is_some()
    }

    fn move_widget<L: WidgetList>(&mut self, index: usize, delta: isize, cx: &mut Context<Self>) {
        self.update_widgets::<L>(cx, |widgets| {
            let target = index as isize + delta;
            if index < widgets.len() && target >= 0 && (target as usize) < widgets.len() {
                widgets.swap(index, target as usize);
            }
        });
    }

    fn duplicate_widget<L: WidgetList>(&mut self, index: usize, cx: &mut Context<Self>) {
        self.update_widgets::<L>(cx, |widgets| {
            if let Some(widget) = widgets.get(index) {
                let copy = L::duplicate(widget);
                widgets.insert(index + 1, copy);
            }
        });
    }

    fn remove_widget<L: WidgetList>(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(removed) = L::widgets(&self.settings).get(index).cloned() else {
            return;
        };
        let editor = L::editor(self);
        if editor
            .dialog
            .as_ref()
            .is_some_and(|dialog| dialog.widget_id == L::id(&removed))
        {
            editor.dialog = None;
        }
        editor.removed = Some((index, removed));
        self.update_widgets::<L>(cx, |widgets| {
            widgets.remove(index);
        });
    }

    fn undo_remove<L: WidgetList>(&mut self, cx: &mut Context<Self>) {
        if let Some((index, removed)) = L::editor(self).removed.take() {
            self.update_widgets::<L>(cx, |widgets| {
                widgets.insert(index.min(widgets.len()), removed)
            });
        }
    }

    /// The undo banner after a removal, then the list as one card (or
    /// `empty` when it has no widgets). `row` titles and describes a widget;
    /// its menu, chevron and click to edit are added here.
    pub(super) fn widget_list<L: WidgetList>(
        &mut self,
        k: &Kit,
        cx: &mut Context<Self>,
        empty: Row,
        mut row: impl FnMut(&mut Self, &Kit, usize, &L::Widget) -> Row,
    ) -> Vec<AnyElement> {
        let mut out = Vec::new();
        if L::editor(self).removed.is_some() {
            let undo = Self::h(cx, |this, (), _, cx| this.undo_remove::<L>(cx));
            out.push(kit::appear(
                k,
                format!("{}-undo", L::TAG),
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .px(px(16.0))
                    .py(px(8.0))
                    .rounded(px(kit::CARD_RADIUS))
                    .bg(k.theme.accent_soft)
                    .child(kit::text(L::removed_message(), 13.0, k.theme.text).flex_1())
                    .child(
                        Button::new(format!("{}-undo", L::TAG), crate::i18n::tr("undo"))
                            .on_click(undo)
                            .render(k),
                    )
                    .into_any_element(),
            ));
        }
        let widgets = L::widgets(&self.settings).clone();
        if widgets.is_empty() {
            out.push(kit::row_card(k, empty));
            return out;
        }
        let count = widgets.len();
        let items = widgets
            .iter()
            .enumerate()
            .map(|(index, widget)| {
                let id = L::id(widget).to_owned();
                let open_id = id.clone();
                let open = Self::h(cx, move |this, (), _, cx| {
                    this.open_widget::<L>(open_id.clone(), cx)
                });
                let menu = self.widget_menu::<L>(k, index, count, &id, cx);
                let row = row(self, k, index, widget)
                    .trailing(menu)
                    .trailing(
                        crate::gui::mark::icon("caret-right", 12.0, k.theme.text_secondary)
                            .into_any_element(),
                    )
                    .on_click(open)
                    .render(k);
                kit::appear(k, format!("{}-widget-{id}", L::TAG), row)
            })
            .collect();
        out.push(kit::card(k, items));
        out
    }

    fn widget_menu<L: WidgetList>(
        &self,
        k: &Kit,
        index: usize,
        count: usize,
        id: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut up = kit::MenuItem::new(crate::i18n::tr("move-up")).icon("arrow-up-bold");
        up.disabled = index == 0;
        let mut down = kit::MenuItem::new(crate::i18n::tr("move-down")).icon("arrow-down-bold");
        down.disabled = index + 1 >= count;
        let menu_id = id.to_owned();
        kit::more_menu(
            k,
            format!("{}-{id}-menu", L::TAG),
            vec![
                kit::MenuItem::new(crate::i18n::tr("edit")).icon("pencil-simple-fill"),
                up,
                down,
                kit::MenuItem::new(crate::i18n::tr("duplicate")).icon("copy"),
                kit::MenuItem::new(crate::i18n::tr("remove"))
                    .icon("trash-fill")
                    .danger(),
            ],
            Self::h(cx, move |this, choice: usize, _, cx| match choice {
                0 => this.open_widget::<L>(menu_id.clone(), cx),
                1 => this.move_widget::<L>(index, -1, cx),
                2 => this.move_widget::<L>(index, 1, cx),
                3 => this.duplicate_widget::<L>(index, cx),
                _ => this.remove_widget::<L>(index, cx),
            }),
        )
    }

    /// The widget whose editor is open (or still fading out), its position
    /// and the dialog's state.
    pub(super) fn widget_dialog<L: WidgetList>(
        &mut self,
        k: &Kit,
    ) -> Option<(WidgetDialog, usize, L::Widget, kit::OverlayPhase)> {
        let current = L::editor(self).dialog.clone().and_then(|dialog| {
            L::widgets(&self.settings)
                .iter()
                .cloned()
                .enumerate()
                .find(|(_, widget)| L::id(widget) == dialog.widget_id)
                .map(|(index, widget)| (dialog, index, widget))
        });
        let editor = L::editor(self);
        if current.is_none() {
            // The widget is gone; its editor closes with it.
            editor.dialog = None;
        }
        let ((dialog, index, widget), phase) = editor.overlay.track(k, current)?;
        Some((dialog, index, widget, phase))
    }

    /// The editor dialog around `body`, with Remove and Done buttons. Every
    /// choice applies at once; Done only closes it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn widget_dialog_frame<L: WidgetList>(
        &self,
        k: &Kit,
        cx: &mut Context<Self>,
        widget_id: &str,
        index: usize,
        phase: kit::OverlayPhase,
        width: f32,
        body: Vec<AnyElement>,
    ) -> AnyElement {
        let close = Self::h(cx, |this, (), _, cx| {
            L::editor(this).dialog = None;
            cx.notify();
        });
        let remove = Button::new(format!("{}-dlg-remove", L::TAG), L::remove_label())
            .danger()
            .with_icon("trash-fill")
            .full_width()
            .on_click(Self::h(cx, move |this, (), _, cx| {
                this.remove_widget::<L>(index, cx)
            }));
        let done = Button::new(format!("{}-dlg-done", L::TAG), crate::i18n::tr("done"))
            .accent()
            .full_width()
            .on_click(close.clone());
        // The body scrolls once it outgrows the window; without this the
        // flex column squeezes its sections (cards, the picker) to nothing
        // instead.
        let body = body
            .into_iter()
            .map(|section| div().flex_none().child(section).into_any_element())
            .collect();
        kit::dialog(
            k,
            format!("{}-dialog-{widget_id}", L::TAG),
            phase,
            width,
            body,
            vec![remove.render(k), done.render(k)],
            Some(close),
        )
    }

    fn set_widget_dialog<L: WidgetList>(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut WidgetDialog),
    ) {
        if let Some(dialog) = L::editor(self).dialog.as_mut() {
            f(dialog);
        }
        cx.notify();
    }

    /// Edits the indicators of one widget in the list.
    fn edit_indicators<L: WidgetList>(
        &mut self,
        widget_index: usize,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut Vec<TrayIndicator>),
    ) {
        self.update_widgets::<L>(cx, |widgets| {
            if let Some(widget) = widgets.get_mut(widget_index) {
                f(L::indicators_mut(widget));
            }
        });
    }

    /// The widget's indicators as expandable cards, in draw order, with an
    /// "Add" button below. Reset clocks follow a single indicator, so they
    /// show its fields directly.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn indicator_editor<L: WidgetList>(
        &mut self,
        k: &mut Kit,
        index: usize,
        widget_id: &str,
        indicators: &[TrayIndicator],
        max: usize,
        single: bool,
        expanded: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let first_enabled = widget_providers(&self.settings.instances, true)
            .first()
            .copied()
            .unwrap_or_default();
        if single {
            let indicator = indicators
                .first()
                .cloned()
                .unwrap_or_else(|| default_indicator(first_enabled));
            let fields = self.indicator_fields::<L>(k, index, 0, &indicator, false, window, cx);
            return kit::card_surface(
                k,
                [div()
                    .px(px(kit::ROW_PADDING_X))
                    .py(px(14.0))
                    .child(fields)
                    .into_any_element()],
            )
            .into_any_element();
        }
        let count = indicators.len();
        let mut column = div().flex().flex_col().gap(px(8.0));
        for (slot, indicator) in indicators.iter().enumerate() {
            let card = self.indicator_card::<L>(
                k,
                index,
                slot,
                count,
                widget_id,
                indicator,
                expanded == Some(slot),
                window,
                cx,
            );
            column = column.child(kit::appear(
                k,
                format!("{}-{widget_id}-indicator-{slot}-in", L::TAG),
                card,
            ));
        }
        if count < max {
            let fallback = indicators
                .last()
                .and_then(TrayIndicator::provider)
                .unwrap_or(first_enabled);
            column = column.child(
                div().flex().child(
                    Button::new(
                        format!("{}-{widget_id}-add-indicator", L::TAG),
                        crate::i18n::tr("add-indicator"),
                    )
                    .with_icon("plus-bold")
                    .on_click(Self::h(cx, move |this, (), _, cx| {
                        this.edit_indicators::<L>(index, cx, |indicators| {
                            indicators.push(default_indicator(fallback));
                        });
                        this.set_widget_dialog::<L>(cx, |dialog| dialog.expanded = Some(count));
                    }))
                    .render(k),
                ),
            );
        }
        column.into_any_element()
    }

    /// Metric, value and color of an indicator, under its provider name.
    fn indicator_detail(&self, indicator: &TrayIndicator) -> String {
        let metric = indicator.provider().map_or_else(
            || indicator.metric_id.clone(),
            |provider| {
                crate::provider_registry::settings_brick_label(
                    provider.kind(),
                    &indicator.metric_id,
                    &self.live.discovered_bricks,
                )
            },
        );
        let value = match indicator.limit_value {
            LimitValue::Used => crate::i18n::tr("used"),
            LimitValue::Remaining => crate::i18n::tr("remaining"),
        };
        format!(
            "{metric} · {value} · {}",
            color_mode_label(indicator.color_mode)
        )
    }

    /// One indicator: provider mark, provider name and a one-line summary,
    /// with reorder/remove controls. Its header expands the fields in place.
    #[allow(clippy::too_many_arguments)]
    fn indicator_card<L: WidgetList>(
        &mut self,
        k: &mut Kit,
        widget_index: usize,
        slot: usize,
        count: usize,
        widget_id: &str,
        indicator: &TrayIndicator,
        expanded: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = format!("{}-{widget_id}-indicator-{slot}", L::TAG);
        let swap = |delta: isize| {
            Self::h(cx, move |this, (), _, cx| {
                let target = slot as isize + delta;
                if target < 0 {
                    return;
                }
                let target = target as usize;
                this.edit_indicators::<L>(widget_index, cx, |indicators| {
                    if target < indicators.len() {
                        indicators.swap(slot, target);
                    }
                });
                // The expanded card travels with its indicator.
                this.set_widget_dialog::<L>(cx, |dialog| {
                    dialog.expanded = match dialog.expanded {
                        Some(open) if open == slot => Some(target),
                        Some(open) if open == target => Some(slot),
                        other => other,
                    }
                });
            })
        };
        let remove = Self::h(cx, move |this, (), _, cx| {
            this.edit_indicators::<L>(widget_index, cx, |indicators| {
                if indicators.len() > 1 && slot < indicators.len() {
                    indicators.remove(slot);
                }
            });
            this.set_widget_dialog::<L>(cx, |dialog| {
                dialog.expanded = match dialog.expanded {
                    Some(open) if open == slot => None,
                    Some(open) if open > slot => Some(open - 1),
                    other => other,
                }
            });
        });
        let toggle = Self::h(cx, move |this, open: bool, _, cx| {
            this.set_widget_dialog::<L>(cx, |dialog| dialog.expanded = open.then_some(slot))
        });
        let theme = &k.theme;
        let (glyph, tint, title) = match indicator.provider() {
            Some(provider) => (
                crate::provider_registry::icon(provider.kind()),
                crate::gui::color::brand(provider.kind(), theme.dark),
                provider.qualified_name().to_string(),
            ),
            None => (
                "warning-fill",
                theme.caution,
                crate::i18n::format("unsupported", &[("v0", indicator.provider_id.to_string())]),
            ),
        };
        let mark = div()
            .relative()
            .child(kit::glyph_plate(
                36.0,
                theme.subtle_hover,
                crate::gui::mark::icon(glyph, 18.0, tint).into_any_element(),
            ))
            .when(indicator.color_mode == TrayColorMode::Fixed, |el| {
                let color = indicator.fixed_color;
                el.child(
                    div()
                        .absolute()
                        .right(px(-2.0))
                        .bottom(px(-2.0))
                        .size(px(12.0))
                        .rounded_full()
                        .bg(rgb8((color.red, color.green, color.blue))),
                )
            })
            .into_any_element();
        let controls = div()
            .flex()
            .gap(px(2.0))
            .child(
                Button::icon_only(format!("{id}-up"), "arrow-up-bold")
                    .ghost()
                    .tooltip(crate::i18n::tr("move-up"))
                    .disabled(slot == 0)
                    .on_click(swap(-1))
                    .render(k),
            )
            .child(
                Button::icon_only(format!("{id}-down"), "arrow-down-bold")
                    .ghost()
                    .tooltip(crate::i18n::tr("move-down"))
                    .disabled(slot + 1 >= count)
                    .on_click(swap(1))
                    .render(k),
            )
            .child(
                Button::icon_only(format!("{id}-remove"), "trash-fill")
                    .ghost()
                    .tooltip(crate::i18n::tr("remove-indicator"))
                    .disabled(count <= 1)
                    .on_click(remove)
                    .render(k),
            )
            .into_any_element();
        let header = Row::new(format!("{id}-header"), title)
            .icon(mark)
            .description(k, self.indicator_detail(indicator))
            .trailing(controls);
        let fields = self.indicator_fields::<L>(k, widget_index, slot, indicator, true, window, cx);
        kit::expander(k, id, header, expanded, toggle, move |_| fields)
    }

    /// `Claude · Work`, marked as off when the instance is turned off.
    fn indicator_provider_label(&self, provider: ProviderId) -> String {
        if self
            .instance(provider)
            .is_some_and(|instance| instance.enabled)
        {
            provider.qualified_name()
        } else {
            crate::i18n::format(
                "off-45080e",
                &[("v0", provider.qualified_name().to_string())],
            )
        }
    }

    /// Provider, metric, value and color for one indicator.
    #[allow(clippy::too_many_arguments)]
    fn indicator_fields<L: WidgetList>(
        &mut self,
        k: &mut Kit,
        widget_index: usize,
        slot: usize,
        indicator: &TrayIndicator,
        with_value: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Every instance with tray metrics, so a disabled one stays selectable.
        let options = widget_providers(&self.settings.instances, false);
        let known = indicator.provider();
        let mut provider_labels = options
            .iter()
            .map(|provider| SharedString::from(self.indicator_provider_label(*provider)))
            .collect::<Vec<_>>();
        let provider_index = known
            .and_then(|provider| options.iter().position(|option| *option == provider))
            .unwrap_or_else(|| {
                provider_labels.push(
                    crate::i18n::format(
                        "unsupported-bba2a8",
                        &[("v0", indicator.provider_id.to_string())],
                    )
                    .into(),
                );
                provider_labels.len() - 1
            });
        let metrics = crate::provider_registry::tray_metric_options(
            known.unwrap_or_default().kind(),
            &self.live.discovered_bricks,
        );
        let mut metric_labels = metrics
            .iter()
            .map(|(_, label)| SharedString::from(label.clone()))
            .collect::<Vec<_>>();
        let metric_index = metrics
            .iter()
            .position(|(id, _)| id == &indicator.metric_id)
            .unwrap_or_else(|| {
                metric_labels.push(
                    crate::i18n::format(
                        "unavailable-0c5d75",
                        &[("v0", indicator.metric_id.to_string())],
                    )
                    .into(),
                );
                metric_labels.len() - 1
            });
        let key = format!("{}-{widget_index}-{slot}", L::TAG);
        let edit = |cx: &mut Context<Self>,
                    f: IndicatorEdit,
                    options: Vec<ProviderId>,
                    metrics: Vec<(String, String)>| {
            Self::h(cx, move |this, choice: usize, _, cx| {
                let options = options.clone();
                let metrics = metrics.clone();
                this.edit_indicators::<L>(widget_index, cx, move |indicators| {
                    if indicators.is_empty() {
                        indicators.push(default_indicator(
                            options.first().copied().unwrap_or_default(),
                        ));
                    }
                    if let Some(indicator) = indicators.get_mut(slot) {
                        f(indicator, choice, &options, &metrics);
                    }
                })
            })
        };
        let provider_box = kit::dropdown(
            k,
            format!("{key}-provider"),
            provider_labels,
            Some(provider_index),
            false,
            0.0,
            edit(
                cx,
                |indicator, choice, options, _| {
                    if let Some(provider) = options.get(choice).copied() {
                        *indicator = TrayIndicator {
                            provider_id: provider.id().into(),
                            metric_id: default_indicator(provider).metric_id,
                            ..indicator.clone()
                        };
                    }
                },
                options.clone(),
                Vec::new(),
            ),
        );
        let metric_box = kit::dropdown(
            k,
            format!("{key}-metric-{}", indicator.provider_id),
            metric_labels,
            Some(metric_index),
            false,
            0.0,
            edit(
                cx,
                |indicator, choice, _, metrics| {
                    if let Some((id, _)) = metrics.get(choice) {
                        indicator.metric_id = id.clone();
                    }
                },
                Vec::new(),
                metrics.clone(),
            ),
        );
        let color_box = kit::dropdown(
            k,
            format!("{key}-color"),
            kit::options(
                &TrayColorMode::ALL
                    .iter()
                    .map(|mode| color_mode_label(*mode))
                    .collect::<Vec<_>>(),
            ),
            Some(indicator.color_mode.index()),
            false,
            0.0,
            edit(
                cx,
                |indicator, choice, _, _| indicator.color_mode = TrayColorMode::from_index(choice),
                Vec::new(),
                Vec::new(),
            ),
        );
        let mut grid = div().flex().flex_col().gap(px(14.0)).child(
            div()
                .flex()
                .gap(px(12.0))
                .child(kit::field(k, crate::i18n::tr("provider"), provider_box))
                .child(kit::field(k, crate::i18n::tr("metric"), metric_box)),
        );
        let mut second = div().flex().gap(px(12.0));
        if with_value {
            let value_box = kit::dropdown(
                k,
                format!("{key}-value"),
                kit::options(&[crate::i18n::tr("remaining"), crate::i18n::tr("used")]),
                Some(usize::from(indicator.limit_value == LimitValue::Used)),
                false,
                0.0,
                edit(
                    cx,
                    |indicator, choice, _, _| {
                        indicator.limit_value = if choice == 1 {
                            LimitValue::Used
                        } else {
                            LimitValue::Remaining
                        }
                    },
                    Vec::new(),
                    Vec::new(),
                ),
            );
            second = second.child(kit::field(k, crate::i18n::tr("show"), value_box));
        }
        if L::COLORED {
            second = second.child(kit::field(k, crate::i18n::tr("color"), color_box));
        }
        grid = grid.child(second);
        if L::COLORED && indicator.color_mode == TrayColorMode::Fixed {
            grid = grid.child(self.color_picker::<L>(
                k,
                widget_index,
                slot,
                indicator.fixed_color,
                window,
                cx,
            ));
        }
        grid.into_any_element()
    }

    fn color_picker<L: WidgetList>(
        &mut self,
        k: &mut Kit,
        widget_index: usize,
        slot: usize,
        current: TrayFixedColor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let set = move |this: &mut Self, color: TrayFixedColor, cx: &mut Context<Self>| {
            this.edit_indicators::<L>(widget_index, cx, |indicators| {
                if let Some(indicator) = indicators.get_mut(slot) {
                    indicator.fixed_color = color;
                }
            })
        };
        let mut swatches = div().flex().flex_wrap().gap(px(8.0));
        for (index, (red, green, blue)) in SWATCHES.into_iter().enumerate() {
            let color = TrayFixedColor { red, green, blue };
            let selected = color == current;
            swatches = swatches.child(
                div()
                    .id(eid(format!(
                        "{}-swatch-{widget_index}-{slot}-{index}",
                        L::TAG
                    )))
                    .size(px(26.0))
                    .rounded(px(6.0))
                    .bg(rgb8((red, green, blue)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(selected, |el| {
                        let luma = 0.299 * f32::from(red)
                            + 0.587 * f32::from(green)
                            + 0.114 * f32::from(blue);
                        let mark = if luma > 150.0 {
                            rgb8((0, 0, 0))
                        } else {
                            rgb8((255, 255, 255))
                        };
                        el.child(crate::gui::mark::icon("check-bold", 14.0, mark))
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| set(this, color, cx))),
            );
        }
        let hex = format!(
            "#{:02X}{:02X}{:02X}",
            current.red, current.green, current.blue
        );
        let hex_input = self.input(
            format!("{}-hex-{widget_index}-{slot}", L::TAG),
            &hex,
            "#RRGGBB",
            false,
            false,
            window,
            cx,
            move |this, value, _, _, cx| {
                if let Some(color) = parse_hex(&value)
                    && color != current
                {
                    set(this, color, cx);
                }
            },
        );
        let hex_field = kit::text_field(k, &hex_input, Some(120.0), window, cx);
        let preview = div()
            .size(px(kit::CONTROL_HEIGHT))
            .rounded(px(6.0))
            .bg(rgb8((current.red, current.green, current.blue)));
        kit::field(
            k,
            crate::i18n::tr("fixed-color"),
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(swatches)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(preview)
                        .child(hex_field),
                )
                .into_any_element(),
        )
    }
}

fn parse_hex(value: &str) -> Option<TrayFixedColor> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |range: std::ops::Range<usize>| u8::from_str_radix(&hex[range], 16).ok();
    Some(TrayFixedColor {
        red: channel(0..2)?,
        green: channel(2..4)?,
        blue: channel(4..6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_colors_parse_with_or_without_hash() {
        assert_eq!(
            parse_hex("#0078d4"),
            Some(TrayFixedColor {
                red: 0,
                green: 0x78,
                blue: 0xD4
            })
        );
        assert!(parse_hex("12345").is_none());
        assert!(parse_hex("zzzzzz").is_none());
    }

    #[test]
    fn every_provider_previews_its_default_tray_metrics() {
        let limits = preview_limits(&[]);
        for kind in ProviderKind::ALL {
            let descriptor = crate::provider_registry::descriptor(kind);
            let samples = limits.get(ProviderId::primary(kind));
            if descriptor.balance_only {
                assert!(samples.spending.is_some(), "{kind:?}");
                continue;
            }
            for metric in descriptor.default_tray_metrics {
                let resolved = crate::provider_registry::resolve_metric(kind, samples, metric);
                assert_eq!(resolved.map(|(id, _, _)| id).as_deref(), Some(*metric));
            }
        }
    }
}
