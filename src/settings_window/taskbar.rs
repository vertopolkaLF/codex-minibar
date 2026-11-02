//! Taskbar: widgets drawn inside the Windows taskbar, with live previews
//! rendered by the same Direct2D code the taskbar uses.

use std::{collections::HashMap, sync::Arc};

use gpui::{
    AnyElement, Context, FontWeight, ImageFormat, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, img, prelude::FluentBuilder, px,
};

use super::kit::{self, Button, Kit, Row, eid};
use super::tray::{IndicatorOwner, TrayDialog, dialog_section, preview_limits};
use super::window::SettingsWindow;
use crate::popup_window::ui::theme::rgb8;
use crate::settings::{
    ProviderId, TaskbarAlignment, TaskbarClick, TaskbarResetDisplay, TaskbarWidget,
    TaskbarWidgetStyle, TrayIndicator,
};

/// Height of the previewed taskbar, in DIPs: the Windows 11 default.
const TASKBAR_HEIGHT: f32 = 48.0;

/// Width of the widget editor.
const DIALOG_WIDTH: f32 = 640.0;
/// Style tiles per picker row.
const STYLE_COLUMNS: usize = 3;
const STYLE_GAP: f32 = 8.0;
/// Each tile's share of the dialog body (24 DIP padding on both sides).
const STYLE_TILE: f32 =
    (DIALOG_WIDTH - 48.0 - STYLE_GAP * (STYLE_COLUMNS as f32 - 1.0)) / STYLE_COLUMNS as f32;
const STYLE_PAD: f32 = 6.0;
const PLATE_PAD: f32 = 4.0;

fn style_label(style: TaskbarWidgetStyle) -> &'static str {
    crate::i18n::tr(match style {
        TaskbarWidgetStyle::Glance => "taskbar-style-glance",
        TaskbarWidgetStyle::Rings => "rings",
        TaskbarWidgetStyle::Nested => "taskbar-style-nested",
        TaskbarWidgetStyle::Dials => "taskbar-style-dials",
        TaskbarWidgetStyle::Bars => "progress-bars",
        TaskbarWidgetStyle::Meters => "taskbar-style-meters",
    })
}

fn style_hint(style: TaskbarWidgetStyle) -> &'static str {
    crate::i18n::tr(match style {
        TaskbarWidgetStyle::Glance => "taskbar-style-glance-hint",
        TaskbarWidgetStyle::Rings => "taskbar-style-rings-hint",
        TaskbarWidgetStyle::Nested => "taskbar-style-nested-hint",
        TaskbarWidgetStyle::Dials => "taskbar-style-dials-hint",
        TaskbarWidgetStyle::Bars => "taskbar-style-bars-hint",
        TaskbarWidgetStyle::Meters => "taskbar-style-meters-hint",
    })
}

/// A per-widget switch in the editor.
struct WidgetToggle {
    id: &'static str,
    title: &'static str,
    description: &'static str,
    value: bool,
    set: fn(&mut TaskbarWidget, bool),
}

/// One line of the editor's Content card.
enum WidgetOption {
    Toggle(WidgetToggle),
    Reset(TaskbarResetDisplay),
}

/// What this style can show, in the order the card lists it.
fn widget_options(widget: &TaskbarWidget) -> Vec<WidgetOption> {
    let style = widget.style;
    let mut options = vec![WidgetOption::Toggle(WidgetToggle {
        id: "icon",
        title: "taskbar-show-provider-icon",
        description: "taskbar-show-provider-icon-description",
        value: widget.show_icon,
        set: |widget, value| widget.show_icon = value,
    })];
    options.push(WidgetOption::Toggle(WidgetToggle {
        id: "values",
        title: "taskbar-show-values",
        description: "taskbar-show-values-description",
        value: widget.show_values,
        set: |widget, value| widget.show_values = value,
    }));
    if style.has_labels() {
        options.push(WidgetOption::Toggle(WidgetToggle {
            id: "labels",
            title: "taskbar-show-labels",
            description: "taskbar-show-labels-description",
            value: widget.show_labels,
            set: |widget, value| widget.show_labels = value,
        }));
    }
    if style.has_reset() {
        options.push(WidgetOption::Reset(widget.reset));
    }
    if style.has_graphic() {
        options.push(WidgetOption::Toggle(WidgetToggle {
            id: "pace",
            title: "taskbar-show-pace",
            description: "taskbar-show-pace-description",
            value: widget.show_pace,
            set: |widget, value| widget.show_pace = value,
        }));
    }
    options
}

#[derive(Clone, PartialEq)]
pub(crate) struct PreviewKey {
    widgets: Vec<TaskbarWidget>,
    badges: Vec<Option<crate::instances::Badge>>,
    light: bool,
    colored: bool,
    accent: [u8; 3],
    scale: u32,
    minute: u64,
}

pub(crate) type PreviewCache = HashMap<String, (PreviewKey, Arc<gpui::Image>, (f32, f32))>;

fn widget_title(index: usize) -> String {
    crate::i18n::format("widget", &[("v0", (index + 1).to_string())])
}

fn widget_summary(widget: &TaskbarWidget) -> String {
    let style = style_label(widget.style);
    let mut providers: Vec<String> = Vec::new();
    for provider in widget.indicators.iter().filter_map(TrayIndicator::provider) {
        let name = provider.qualified_name();
        if !providers.contains(&name) {
            providers.push(name);
        }
    }
    if providers.is_empty() {
        crate::i18n::format("style-no-indicators", &[("style", style.to_string())])
    } else {
        format!("{style} · {}", providers.join(", "))
    }
}

/// The Windows 11 taskbar material behind the previews.
fn taskbar_plate(light: bool) -> gpui::Hsla {
    if light {
        rgb8((0xEE, 0xEE, 0xEE))
    } else {
        rgb8((0x1C, 0x1C, 0x1C))
    }
}

impl SettingsWindow {
    /// The widgets rendered exactly as the taskbar draws them, with sample
    /// quotas, at their natural size.
    fn taskbar_preview(
        &mut self,
        key: String,
        widgets: &[TaskbarWidget],
        window: &Window,
    ) -> Option<AnyElement> {
        let (image, size) = self.taskbar_image(key, widgets, window)?;
        Some(
            img(image)
                .w(px(size.0))
                .h(px(size.1))
                .flex_none()
                .into_any_element(),
        )
    }

    /// The rendered strip and its size in DIPs, keyed by `key` in the
    /// preview cache.
    fn taskbar_image(
        &mut self,
        key: String,
        widgets: &[TaskbarWidget],
        window: &Window,
    ) -> Option<(Arc<gpui::Image>, (f32, f32))> {
        let light = crate::taskbar::uses_light_theme();
        let scale = window.scale_factor();
        let cache_key = PreviewKey {
            widgets: widgets.to_vec(),
            badges: widgets
                .iter()
                .flat_map(|widget| widget.indicators.iter())
                .filter_map(TrayIndicator::provider)
                .map(|provider| provider.badge())
                .collect(),
            light,
            colored: self.settings.use_colored_provider_icons,
            accent: crate::theme::current_accent_rgb(),
            scale: (scale * 100.0).round() as u32,
            minute: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_secs() / 60),
        };
        let (image, size) = match self.taskbar_previews.get(&key) {
            Some((cached, image, size)) if *cached == cache_key => (Arc::clone(image), *size),
            _ => {
                let appearance = crate::taskbar::model::Appearance {
                    light,
                    colored_icons: cache_key.colored,
                    accent: cache_key.accent,
                };
                let preview = crate::taskbar::preview(
                    widgets,
                    &preview_limits(),
                    appearance,
                    scale,
                    TASKBAR_HEIGHT,
                )?;
                let image = Arc::new(gpui::Image::from_bytes(ImageFormat::Png, preview.png));
                let size = (preview.width, preview.height);
                self.taskbar_previews
                    .insert(key, (cache_key, Arc::clone(&image), size));
                (image, size)
            }
        };
        Some((image, size))
    }

    /// A widget on a slice of taskbar, as tall as the real one.
    fn taskbar_tile(
        &mut self,
        key: String,
        widgets: &[TaskbarWidget],
        window: &Window,
    ) -> AnyElement {
        let light = crate::taskbar::uses_light_theme();
        div()
            .h(px(TASKBAR_HEIGHT))
            .flex_none()
            .px(px(4.0))
            .rounded(px(6.0))
            .bg(taskbar_plate(light))
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .children(self.taskbar_preview(key, widgets, window))
            .into_any_element()
    }

    /// The whole strip beside a mock notification area and clock.
    fn taskbar_mock(&mut self, k: &Kit, widgets: &[TaskbarWidget], window: &Window) -> AnyElement {
        let light = crate::taskbar::uses_light_theme();
        let text = if light {
            rgb8((0x1A, 0x1A, 0x1A))
        } else {
            rgb8((0xFF, 0xFF, 0xFF))
        };
        let now = chrono::Local::now();
        let clock = div()
            .flex()
            .flex_col()
            .items_end()
            .px(px(8.0))
            .text_size(px(12.0))
            .line_height(px(16.0))
            .text_color(text)
            .child(crate::settings::TimeFormat::current().format_hm(now))
            .child(now.format("%d.%m.%Y").to_string());
        let strip = self.taskbar_preview("taskbar-strip".into(), widgets, window);
        let bar = div()
            .h(px(TASKBAR_HEIGHT))
            .w_full()
            .rounded(px(kit::CARD_RADIUS))
            .bg(taskbar_plate(light))
            .border_1()
            .border_color(k.theme.divider)
            .flex()
            .items_center()
            .overflow_hidden();
        let caret = div().px(px(6.0)).child(kit::icon("caret-up", 12.0, text));
        let bar = match self.settings.taskbar.alignment {
            TaskbarAlignment::Start => bar
                .pl(px(6.0))
                .children(strip)
                .child(div().flex_1())
                .child(caret),
            TaskbarAlignment::Tray => bar.child(div().flex_1()).children(strip).child(caret),
            // Inside the notification area, just past the overflow chevron.
            TaskbarAlignment::InTray => bar.child(div().flex_1()).child(caret).children(strip),
        };
        bar.child(clock).into_any_element()
    }

    /// Opens the editor of the widget `id`, if it still exists.
    pub(crate) fn open_taskbar_dialog(&mut self, id: String, cx: &mut Context<Self>) {
        if self
            .settings
            .taskbar
            .widgets
            .iter()
            .any(|widget| widget.id == id)
        {
            self.taskbar_dialog = Some(TrayDialog::new(id));
            cx.notify();
        }
    }

    fn set_taskbar_widgets(&mut self, mut widgets: Vec<TaskbarWidget>, cx: &mut Context<Self>) {
        for widget in &mut widgets {
            widget.normalize();
        }
        self.edit(cx, move |settings| {
            settings.taskbar.widgets = widgets.clone()
        });
    }

    pub(super) fn update_taskbar_widgets(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut Vec<TaskbarWidget>),
    ) {
        let mut next = self.settings.taskbar.widgets.clone();
        f(&mut next);
        self.set_taskbar_widgets(next, cx);
    }

    fn enabled_taskbar_providers(&self) -> Vec<ProviderId> {
        self.settings
            .instances
            .iter()
            .filter(|instance| instance.enabled)
            .filter(|instance| {
                !crate::provider_registry::descriptor(instance.driver)
                    .default_tray_metrics
                    .is_empty()
            })
            .map(|instance| instance.provider_id())
            .collect()
    }

    pub(super) fn taskbar_page(
        &mut self,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let taskbar = self.settings.taskbar.clone();
        let enabled_providers = self.enabled_taskbar_providers();
        let mut rows = vec![kit::caption(k, crate::i18n::tr("taskbar-page-caption"))];

        let alignment_index = match taskbar.alignment {
            TaskbarAlignment::Tray => 0,
            TaskbarAlignment::Start => 1,
            TaskbarAlignment::InTray => 2,
        };
        rows.push(kit::card_of(k, |k| {
            vec![
                kit::toggle_row(
                    k,
                    "taskbar-enabled",
                    crate::i18n::tr("taskbar-show-widgets"),
                    Some(crate::i18n::tr("taskbar-show-widgets-description").into()),
                    taskbar.enabled,
                    Self::h(cx, |this, value: bool, _, cx| {
                        // Turning widgets on for the first time shows one. It
                        // is built once: `edit` runs for the snapshot and the
                        // writer, and both must see the same widget id.
                        let seed = this
                            .enabled_taskbar_providers()
                            .first()
                            .copied()
                            .filter(|_| value && this.settings.taskbar.widgets.is_empty())
                            .map(TaskbarWidget::for_provider);
                        this.edit(cx, move |settings| {
                            settings.taskbar.enabled = value;
                            if let Some(widget) = seed.clone()
                                && settings.taskbar.widgets.is_empty()
                            {
                                settings.taskbar.widgets.push(widget);
                            }
                        })
                    }),
                ),
                kit::dropdown_row(
                    k,
                    "taskbar-alignment",
                    crate::i18n::tr("taskbar-position"),
                    Some(crate::i18n::tr("taskbar-position-description")),
                    kit::options(&[
                        crate::i18n::tr("taskbar-position-tray"),
                        crate::i18n::tr("taskbar-position-start"),
                        crate::i18n::tr("taskbar-position-in-tray"),
                    ]),
                    alignment_index,
                    false,
                    Self::h(cx, |this, index: usize, _, cx| {
                        let alignment = match index {
                            1 => TaskbarAlignment::Start,
                            2 => TaskbarAlignment::InTray,
                            _ => TaskbarAlignment::Tray,
                        };
                        this.edit(cx, move |settings| settings.taskbar.alignment = alignment);
                    }),
                ),
                kit::dropdown_row(
                    k,
                    "taskbar-click",
                    crate::i18n::tr("taskbar-click"),
                    Some(crate::i18n::tr("taskbar-click-description")),
                    kit::options(&[
                        crate::i18n::tr("taskbar-click-nearest"),
                        crate::i18n::tr("taskbar-click-tab"),
                        crate::i18n::tr("taskbar-click-home"),
                    ]),
                    taskbar.click.index() as i32,
                    false,
                    Self::h(cx, |this, index: usize, _, cx| {
                        let click = TaskbarClick::ALL.get(index).copied().unwrap_or_default();
                        this.edit(cx, move |settings| settings.taskbar.click = click);
                    }),
                ),
                kit::toggle_row(
                    k,
                    "taskbar-cycle",
                    crate::i18n::tr("taskbar-cycle"),
                    Some(crate::i18n::tr("taskbar-cycle-description").into()),
                    taskbar.cycle,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |settings| settings.taskbar.cycle = value)
                    }),
                ),
            ]
        }));

        let mut visible = taskbar
            .widgets
            .iter()
            .filter(|widget| widget.is_visible_for(&self.settings.instances))
            .cloned()
            .collect::<Vec<_>>();
        if taskbar.cycle {
            // The taskbar starts the cycle on the first widget.
            visible.truncate(1);
        }
        if !visible.is_empty() {
            rows.push(kit::section_heading(k, crate::i18n::tr("preview")));
            let mock = self.taskbar_mock(k, &visible, window);
            rows.push(mock);
        }

        rows.push(kit::section_heading(k, crate::i18n::tr("taskbar-widgets")));
        if let Some((index, removed)) = self.removed_taskbar_widget.clone() {
            let undo = Self::h(cx, move |this, (), _, cx| {
                let removed = removed.clone();
                this.update_taskbar_widgets(cx, |widgets| {
                    widgets.insert(index.min(widgets.len()), removed)
                });
                this.removed_taskbar_widget = None;
            });
            rows.push(kit::appear(
                k,
                "taskbar-undo",
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .px(px(16.0))
                    .py(px(8.0))
                    .rounded(px(kit::CARD_RADIUS))
                    .bg(k.theme.accent_soft)
                    .child(
                        kit::text(crate::i18n::tr("widget-removed"), 13.0, k.theme.text).flex_1(),
                    )
                    .child(
                        Button::new("taskbar-undo", crate::i18n::tr("undo"))
                            .on_click(undo)
                            .render(k),
                    )
                    .into_any_element(),
            ));
        }
        if taskbar.widgets.is_empty() {
            rows.push(kit::row_card(
                k,
                Row::new("taskbar-empty", crate::i18n::tr("taskbar-no-widgets"))
                    .description(k, crate::i18n::tr("taskbar-no-widgets-description")),
            ));
        } else {
            let count = taskbar.widgets.len();
            let items = taskbar
                .widgets
                .iter()
                .enumerate()
                .map(|(index, widget)| {
                    let row = self.taskbar_widget_row(k, index, count, widget, window, cx);
                    kit::appear(k, format!("taskbar-widget-{}", widget.id), row)
                })
                .collect();
            rows.push(kit::card(k, items));
        }
        let first = enabled_providers.first().copied();
        let add_widget = Self::h(cx, move |this, (), _, cx| {
            let Some(provider) = first else {
                return;
            };
            let widget = TaskbarWidget::for_provider(provider);
            this.taskbar_dialog = Some(TrayDialog::new(widget.id.clone()));
            this.update_taskbar_widgets(cx, |widgets| widgets.push(widget));
        });
        rows.push(
            div()
                .flex()
                .gap(px(8.0))
                .pt(px(8.0))
                .child(
                    Button::new("taskbar-add-widget", crate::i18n::tr("add-widget"))
                        .accent()
                        .with_icon("plus-bold")
                        .disabled(enabled_providers.is_empty())
                        .on_click(add_widget)
                        .render(k),
                )
                .into_any_element(),
        );
        rows
    }

    fn taskbar_widget_row(
        &mut self,
        k: &Kit,
        index: usize,
        count: usize,
        widget: &TaskbarWidget,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = widget.id.clone();
        let open_id = id.clone();
        let open = Self::h(cx, move |this, (), _, cx| {
            this.taskbar_dialog = Some(TrayDialog::new(open_id.clone()));
            cx.notify();
        });
        let menu_id = id.clone();
        let mut up = kit::MenuItem::new(crate::i18n::tr("move-up")).icon("arrow-up-bold");
        up.disabled = index == 0;
        let mut down = kit::MenuItem::new(crate::i18n::tr("move-down")).icon("arrow-down-bold");
        down.disabled = index + 1 >= count;
        let menu = kit::more_menu(
            k,
            format!("taskbar-{id}-menu"),
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
                0 => {
                    this.taskbar_dialog = Some(TrayDialog::new(menu_id.clone()));
                    cx.notify();
                }
                1 => this.move_taskbar_widget(index, -1, cx),
                2 => this.move_taskbar_widget(index, 1, cx),
                3 => this.update_taskbar_widgets(cx, |widgets| {
                    if let Some(widget) = widgets.get(index) {
                        widgets.insert(index + 1, widget.duplicate_with_new_id());
                    }
                }),
                _ => this.remove_taskbar_widget(index, cx),
            }),
        );
        let tile = self.taskbar_tile(format!("row-{id}"), std::slice::from_ref(widget), window);
        Row::new(format!("taskbar-row-{id}"), widget_title(index))
            .icon(tile)
            .description(k, widget_summary(widget))
            .trailing(menu)
            .trailing(kit::icon("caret-right", 12.0, k.theme.text_secondary).into_any_element())
            .on_click(open)
            .render(k)
    }

    fn move_taskbar_widget(&mut self, index: usize, delta: isize, cx: &mut Context<Self>) {
        self.update_taskbar_widgets(cx, |widgets| {
            let target = index as isize + delta;
            if index < widgets.len() && target >= 0 && (target as usize) < widgets.len() {
                widgets.swap(index, target as usize);
            }
        });
    }

    fn remove_taskbar_widget(&mut self, index: usize, cx: &mut Context<Self>) {
        let mut next = self.settings.taskbar.widgets.clone();
        if index >= next.len() {
            return;
        }
        let removed = next.remove(index);
        if self
            .taskbar_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.widget_id == removed.id)
        {
            self.taskbar_dialog = None;
        }
        self.removed_taskbar_widget = Some((index, removed));
        self.set_taskbar_widgets(next, cx);
    }

    /// The widget editor: preview, style, provider mark and indicators.
    /// Every choice applies to the taskbar immediately.
    pub(super) fn taskbar_dialog_overlay(
        &mut self,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let current = self.taskbar_dialog.clone().and_then(|dialog| {
            self.settings
                .taskbar
                .widgets
                .iter()
                .cloned()
                .enumerate()
                .find(|(_, widget)| widget.id == dialog.widget_id)
                .map(|(index, widget)| (dialog, index, widget))
        });
        if current.is_none() {
            self.taskbar_dialog = None;
        }
        let ((dialog, index, widget), phase) = self.overlays.taskbar.track(k, current)?;
        let close = Self::h(cx, |this, (), _, cx| {
            this.taskbar_dialog = None;
            cx.notify();
        });
        let tile = self.taskbar_tile(
            format!("dialog-{}", widget.id),
            std::slice::from_ref(&widget),
            window,
        );
        let header = div()
            .flex()
            .items_center()
            .gap(px(14.0))
            .child(tile)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.0))
                    .child(kit::dialog_title(k, widget_title(index)))
                    .child(kit::caption(k, widget_summary(&widget))),
            )
            .into_any_element();
        let mut body = vec![header];
        body.push(dialog_section(
            k,
            crate::i18n::tr("style"),
            style_hint(widget.style),
        ));
        body.push(self.taskbar_style_picker(k, index, &widget, window, cx));
        body.push(dialog_section(
            k,
            crate::i18n::tr("taskbar-content"),
            crate::i18n::tr("taskbar-content-hint"),
        ));
        let options = widget_options(&widget);
        let widget_id = widget.id.clone();
        body.push(kit::card_of(k, |k| {
            options
                .into_iter()
                .map(|option| match option {
                    WidgetOption::Toggle(toggle) => {
                        let set = toggle.set;
                        kit::toggle_row(
                            k,
                            format!("taskbar-{widget_id}-{}", toggle.id),
                            crate::i18n::tr(toggle.title),
                            Some(crate::i18n::tr(toggle.description).into()),
                            toggle.value,
                            Self::h(cx, move |this, value: bool, _, cx| {
                                this.update_taskbar_widgets(cx, |widgets| {
                                    if let Some(widget) = widgets.get_mut(index) {
                                        set(widget, value);
                                    }
                                })
                            }),
                        )
                    }
                    WidgetOption::Reset(reset) => kit::dropdown_row(
                        k,
                        &format!("taskbar-{widget_id}-reset"),
                        crate::i18n::tr("taskbar-reset-time"),
                        Some(crate::i18n::tr("taskbar-reset-time-description")),
                        kit::options(&[
                            crate::i18n::tr("taskbar-reset-off"),
                            crate::i18n::tr("taskbar-reset-countdown"),
                            crate::i18n::tr("taskbar-reset-clock"),
                        ]),
                        reset.index() as i32,
                        false,
                        Self::h(cx, move |this, choice: usize, _, cx| {
                            let reset = TaskbarResetDisplay::ALL
                                .get(choice)
                                .copied()
                                .unwrap_or_default();
                            this.update_taskbar_widgets(cx, |widgets| {
                                if let Some(widget) = widgets.get_mut(index) {
                                    widget.reset = reset;
                                }
                            })
                        }),
                    ),
                })
                .collect()
        }));
        let max = widget.style.max_indicators();
        body.push(dialog_section(
            k,
            crate::i18n::tr("indicators"),
            crate::i18n::format(
                "up-to-max-indicators-quotas-drawn-in-this-order-expand-one-to-cha",
                &[("max_indicators", max.to_string())],
            ),
        ));
        let indicators = self.tray_indicators(
            k,
            IndicatorOwner::Taskbar,
            index,
            &widget.id,
            &widget.indicators,
            max,
            false,
            dialog.expanded,
            window,
            cx,
        );
        body.push(kit::appear(
            k,
            format!("taskbar-dlg-{}-{max}", widget.id),
            indicators,
        ));
        let remove = Button::new("taskbar-dlg-remove", crate::i18n::tr("remove-widget"))
            .danger()
            .with_icon("trash-fill")
            .full_width()
            .on_click(Self::h(cx, move |this, (), _, cx| {
                this.remove_taskbar_widget(index, cx)
            }));
        let done = Button::new("taskbar-dlg-done", crate::i18n::tr("done"))
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
        Some(kit::dialog(
            k,
            format!("taskbar-dialog-{}", widget.id),
            phase,
            DIALOG_WIDTH,
            body,
            vec![remove.render(k), done.render(k)],
            Some(close),
        ))
    }

    /// One tile per style, each previewing this widget drawn that way, in
    /// rows of equal tiles. Previews wider than a tile scale down to fit.
    fn taskbar_style_picker(
        &mut self,
        k: &Kit,
        index: usize,
        widget: &TaskbarWidget,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let light = crate::taskbar::uses_light_theme();
        let tiles = TaskbarWidgetStyle::ALL
            .into_iter()
            .map(|style| {
                let selected = style == widget.style;
                let mut variant = widget.clone();
                variant.style = style;
                variant.normalize();
                let preview = self
                    .taskbar_image(
                        format!("style-{}-{}", widget.id, style.index()),
                        std::slice::from_ref(&variant),
                        window,
                    )
                    .map(|(image, (width, height))| {
                        // Wide designs shrink to the tile. Sized here: GPUI's
                        // object fit reads device pixels as DIPs.
                        let room = STYLE_TILE - 2.0 - STYLE_PAD * 2.0 - PLATE_PAD * 2.0;
                        let fit = (room / width).min(TASKBAR_HEIGHT / height).min(1.0);
                        img(image)
                            .w(px(width * fit))
                            .h(px(height * fit))
                            .flex_none()
                    });
                let id = format!("taskbar-style-{}", style.index());
                let theme = &k.theme;
                let rest = if selected {
                    theme.accent_soft
                } else {
                    theme.control
                };
                let hover = if selected { rest } else { theme.control_hover };
                let tile = div()
                    .id(eid(id.clone()))
                    .w(px(STYLE_TILE))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .p(px(STYLE_PAD))
                    .pb(px(8.0))
                    .rounded(px(kit::CARD_RADIUS))
                    .border_1()
                    .border_color(if selected {
                        theme.accent
                    } else {
                        gpui::transparent_black()
                    })
                    .child(
                        div()
                            .w_full()
                            .h(px(TASKBAR_HEIGHT))
                            .flex_none()
                            .px(px(PLATE_PAD))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(6.0))
                            .bg(taskbar_plate(light))
                            .overflow_hidden()
                            .children(preview),
                    )
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
                            .child(style_label(style)),
                    );
                kit::hover_bg(k, tile, kit::hover_key(&id), rest, hover)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.update_taskbar_widgets(cx, |widgets| {
                            if let Some(widget) = widgets.get_mut(index) {
                                widget.style = style;
                            }
                        })
                    }))
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let mut rows = Vec::new();
        let mut tiles = tiles.into_iter().peekable();
        while tiles.peek().is_some() {
            rows.push(
                div()
                    .flex()
                    .gap(px(STYLE_GAP))
                    .children(tiles.by_ref().take(STYLE_COLUMNS))
                    .into_any_element(),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(STYLE_GAP))
            .children(rows)
            .into_any_element()
    }
}
