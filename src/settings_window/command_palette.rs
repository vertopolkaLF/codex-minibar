//! Integrations: the PowerToys Command Palette extension and its Dock bands.
//!
//! Bands are Glance taskbar widgets in their own list. The Dock draws each as
//! the provider mark over a title and a subtitle, so the editor offers only
//! what reaches that text.

use crate::choice::Choice;
use futures::StreamExt;
use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, Window, div, px};

use super::kit::{self, Button, Kit, Row};
use super::tray::{IndicatorOwner, TrayDialog, dialog_section, preview_limits};
use super::window::SettingsWindow;
use crate::cmdpal::InstallPhase;
use crate::popup_window::ui::theme::rgb8;
use crate::settings::{ProviderId, TaskbarResetDisplay, TaskbarWidget, TaskbarWidgetStyle};

/// Width of the band editor.
const DIALOG_WIDTH: f32 = 560.0;
/// Height of a band in the Dock's default size.
const BAND_HEIGHT: f32 = 40.0;

/// A content toggle in the band editor: id, title and description keys, the
/// current value and how to store a new one.
type ContentToggle = (
    &'static str,
    &'static str,
    &'static str,
    bool,
    fn(&mut TaskbarWidget, bool),
);

fn band_title(index: usize) -> String {
    crate::i18n::format("cmdpal-band", &[("v0", (index + 1).to_string())])
}

fn phase_description(phase: &InstallPhase, installed: bool) -> &'static str {
    crate::i18n::tr(match phase {
        InstallPhase::Downloading => "cmdpal-downloading",
        InstallPhase::Installing => "cmdpal-installing",
        InstallPhase::Removing => "cmdpal-removing",
        InstallPhase::Failed(_) => "cmdpal-failed",
        InstallPhase::Installed => "cmdpal-installed-hint",
        InstallPhase::Idle | InstallPhase::Removed if installed => "cmdpal-installed-hint",
        InstallPhase::Idle | InstallPhase::Removed => "cmdpal-description",
    })
}

impl SettingsWindow {
    /// The Command Palette section of the Integrations page.
    pub(super) fn command_palette_section(
        &mut self,
        k: &mut Kit,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut rows = vec![kit::section_heading(k, crate::i18n::tr("command-palette"))];
        rows.push(self.cmdpal_install_card(k, cx));

        rows.push(kit::section_heading(
            k,
            crate::i18n::tr("cmdpal-dock-bands"),
        ));
        rows.push(kit::caption(
            k,
            crate::i18n::tr("cmdpal-dock-bands-caption"),
        ));
        if let Some((index, removed)) = self.removed_dock_widget.clone() {
            let undo = Self::h(cx, move |this, (), _, cx| {
                let removed = removed.clone();
                this.update_dock_widgets(cx, |widgets| {
                    widgets.insert(index.min(widgets.len()), removed)
                });
                this.removed_dock_widget = None;
            });
            rows.push(kit::appear(
                k,
                "dock-undo",
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .px(px(16.0))
                    .py(px(8.0))
                    .rounded(px(kit::CARD_RADIUS))
                    .bg(k.theme.accent_soft)
                    .child(
                        kit::text(crate::i18n::tr("cmdpal-band-removed"), 13.0, k.theme.text)
                            .flex_1(),
                    )
                    .child(
                        Button::new("dock-undo", crate::i18n::tr("undo"))
                            .on_click(undo)
                            .render(k),
                    )
                    .into_any_element(),
            ));
        }
        let widgets = self.settings.command_palette.widgets.clone();
        if widgets.is_empty() {
            rows.push(kit::row_card(
                k,
                Row::new("dock-empty", crate::i18n::tr("cmdpal-no-bands"))
                    .description(k, crate::i18n::tr("cmdpal-no-bands-description")),
            ));
        } else {
            let count = widgets.len();
            let items = widgets
                .iter()
                .enumerate()
                .map(|(index, widget)| {
                    let row = self.dock_widget_row(k, index, count, widget, cx);
                    kit::appear(k, format!("dock-widget-{}", widget.id), row)
                })
                .collect();
            rows.push(kit::card(k, items));
        }
        let providers = self.enabled_dock_providers();
        let first = providers.first().copied();
        let add = Self::h(cx, move |this, (), _, cx| {
            let Some(provider) = first else {
                return;
            };
            let widget = TaskbarWidget::dock_for_provider(provider);
            this.dock_dialog = Some(TrayDialog::new(widget.id.clone()));
            this.update_dock_widgets(cx, |widgets| widgets.push(widget));
        });
        rows.push(
            div()
                .flex()
                .gap(px(8.0))
                .pt(px(8.0))
                .child(
                    Button::new("dock-add-widget", crate::i18n::tr("cmdpal-add-band"))
                        .accent()
                        .with_icon("plus-bold")
                        .disabled(providers.is_empty())
                        .on_click(add)
                        .render(k),
                )
                .into_any_element(),
        );
        rows
    }

    fn cmdpal_install_card(&mut self, k: &mut Kit, cx: &mut Context<Self>) -> AnyElement {
        let phase = self.cmdpal_phase.clone();
        let installed = self.cmdpal_installed;
        let busy = matches!(
            phase,
            InstallPhase::Downloading | InstallPhase::Installing | InstallPhase::Removing
        );
        let track = |this: &mut Self, removing: bool, cx: &mut Context<Self>| {
            let (tx, mut rx) = futures::channel::mpsc::unbounded();
            let report = move |phase| {
                let _ = tx.unbounded_send(phase);
            };
            if removing {
                crate::cmdpal::uninstall_async(report);
            } else {
                // Bands start from the enabled providers, so a fresh
                // install has something to pin.
                let seeds = this.enabled_dock_providers();
                if this.settings.command_palette.widgets.is_empty() && !seeds.is_empty() {
                    this.update_dock_widgets(cx, |widgets| {
                        widgets.extend(seeds.into_iter().map(TaskbarWidget::dock_for_provider))
                    });
                }
                crate::cmdpal::install_async(report);
            }
            cx.spawn(async move |this, cx| {
                while let Some(phase) = rx.next().await {
                    if this
                        .update(cx, |this, cx| {
                            this.cmdpal_installed = crate::cmdpal::is_installed();
                            this.cmdpal_phase = phase;
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .detach();
        };
        let install = Self::h(cx, move |this, (), _, cx| track(this, false, cx));
        let remove = Self::h(cx, move |this, (), _, cx| track(this, true, cx));
        let mut row = Row::new("integrations-cmdpal", crate::i18n::tr("cmdpal-extension"))
            .icon(kit::row_icon(k, "package-fill"))
            .description(k, phase_description(&phase, installed));
        if let InstallPhase::Failed(error) = &phase {
            row = row.detail(kit::text(error.clone(), 12.0, k.theme.caution).into_any_element());
        }
        if installed {
            row = row.trailing(
                Button::new("integrations-cmdpal-remove", crate::i18n::tr("remove"))
                    .disabled(busy)
                    .on_click(remove)
                    .render(k),
            );
        }
        let label = match phase {
            InstallPhase::Downloading => crate::i18n::tr("downloading"),
            InstallPhase::Installing => crate::i18n::tr("cmdpal-installing-short"),
            _ if installed => crate::i18n::tr("cmdpal-reinstall"),
            _ => crate::i18n::tr("install"),
        };
        let row = row
            .trailing(
                Button::new("integrations-cmdpal-install", label)
                    .accent()
                    .disabled(busy)
                    .on_click(install)
                    .render(k),
            )
            .render(k);
        kit::card(k, vec![row])
    }

    fn enabled_dock_providers(&self) -> Vec<ProviderId> {
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

    pub(super) fn update_dock_widgets(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut Vec<TaskbarWidget>),
    ) {
        let mut widgets = self.settings.command_palette.widgets.clone();
        f(&mut widgets);
        for widget in &mut widgets {
            widget.style = TaskbarWidgetStyle::Glance;
            widget.normalize();
        }
        self.edit(cx, move |settings| {
            settings.command_palette.widgets = widgets.clone()
        });
    }

    /// The band as the Dock lays it out: the mark beside a title over a
    /// subtitle, with sample quotas.
    fn dock_band(&self, k: &Kit, widget: &TaskbarWidget) -> AnyElement {
        let light = !k.theme.dark;
        let limits = preview_limits();
        let view = crate::taskbar::model::resolve(
            widget,
            &limits,
            crate::taskbar::model::Appearance {
                light,
                colored_icons: self.settings.use_colored_provider_icons,
                accent: crate::theme::current_accent_rgb(),
            },
        );
        let (title, subtitle) = crate::cmdpal::band_lines(widget, &view, &limits);
        let mark = view.icon.as_ref().map(|icon| {
            let [red, green, blue] = icon.tint;
            kit::provider_mark(
                k,
                icon.name,
                16.0,
                rgb8((red, green, blue)),
                view.provider.and_then(ProviderId::badge).as_ref(),
            )
        });
        let text = div()
            .flex()
            .flex_col()
            .min_w_0()
            .child(
                kit::text(title, 12.0, k.theme.text)
                    .line_height(px(16.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .whitespace_nowrap(),
            )
            .children((!subtitle.is_empty()).then(|| {
                kit::text(subtitle, 11.0, k.theme.text_secondary)
                    .line_height(px(14.0))
                    .whitespace_nowrap()
            }));
        div()
            .h(px(BAND_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(10.0))
            .rounded(px(6.0))
            .bg(k.theme.control)
            .border_1()
            .border_color(k.theme.divider)
            .children(mark)
            .child(text)
            .into_any_element()
    }

    fn dock_widget_row(
        &mut self,
        k: &Kit,
        index: usize,
        count: usize,
        widget: &TaskbarWidget,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = widget.id.clone();
        let open_id = id.clone();
        let open = Self::h(cx, move |this, (), _, cx| {
            this.dock_dialog = Some(TrayDialog::new(open_id.clone()));
            cx.notify();
        });
        let menu_id = id.clone();
        let mut up = kit::MenuItem::new(crate::i18n::tr("move-up")).icon("arrow-up-bold");
        up.disabled = index == 0;
        let mut down = kit::MenuItem::new(crate::i18n::tr("move-down")).icon("arrow-down-bold");
        down.disabled = index + 1 >= count;
        let menu = kit::more_menu(
            k,
            format!("dock-{id}-menu"),
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
                    this.dock_dialog = Some(TrayDialog::new(menu_id.clone()));
                    cx.notify();
                }
                1 => this.move_dock_widget(index, -1, cx),
                2 => this.move_dock_widget(index, 1, cx),
                3 => this.update_dock_widgets(cx, |widgets| {
                    if let Some(widget) = widgets.get(index) {
                        widgets.insert(index + 1, widget.dock_duplicate_with_new_id());
                    }
                }),
                _ => this.remove_dock_widget(index, cx),
            }),
        );
        Row::new(format!("dock-row-{id}"), band_title(index))
            .icon(self.dock_band(k, widget))
            .trailing(menu)
            .trailing(kit::icon("caret-right", 12.0, k.theme.text_secondary).into_any_element())
            .on_click(open)
            .render(k)
    }

    fn move_dock_widget(&mut self, index: usize, delta: isize, cx: &mut Context<Self>) {
        self.update_dock_widgets(cx, |widgets| {
            let target = index as isize + delta;
            if index < widgets.len() && target >= 0 && (target as usize) < widgets.len() {
                widgets.swap(index, target as usize);
            }
        });
    }

    fn remove_dock_widget(&mut self, index: usize, cx: &mut Context<Self>) {
        let widgets = &self.settings.command_palette.widgets;
        let Some(removed) = widgets.get(index).cloned() else {
            return;
        };
        if self
            .dock_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.widget_id == removed.id)
        {
            self.dock_dialog = None;
        }
        self.removed_dock_widget = Some((index, removed));
        self.update_dock_widgets(cx, |widgets| {
            if index < widgets.len() {
                widgets.remove(index);
            }
        });
    }

    /// The band editor: what the title and subtitle show, then which quotas.
    /// Every choice reaches the Dock immediately.
    pub(super) fn dock_dialog_overlay(
        &mut self,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let current = self.dock_dialog.clone().and_then(|dialog| {
            self.settings
                .command_palette
                .widgets
                .iter()
                .cloned()
                .enumerate()
                .find(|(_, widget)| widget.id == dialog.widget_id)
                .map(|(index, widget)| (dialog, index, widget))
        });
        if current.is_none() {
            self.dock_dialog = None;
        }
        let ((dialog, index, widget), phase) = self.overlays.dock.track(k, current)?;
        let close = Self::h(cx, |this, (), _, cx| {
            this.dock_dialog = None;
            cx.notify();
        });
        let header = div()
            .flex()
            .items_center()
            .gap(px(14.0))
            .child(self.dock_band(k, &widget))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.0))
                    .child(kit::dialog_title(k, band_title(index)))
                    .child(kit::caption(k, crate::cmdpal::band_name(&widget))),
            )
            .into_any_element();
        let mut body = vec![header];
        body.push(dialog_section(
            k,
            crate::i18n::tr("taskbar-content"),
            crate::i18n::tr("cmdpal-content-hint"),
        ));
        let widget_id = widget.id.clone();
        let toggles: [ContentToggle; 3] = [
            (
                "icon",
                "taskbar-show-provider-icon",
                "taskbar-show-provider-icon-description",
                widget.show_icon,
                |widget, value| widget.show_icon = value,
            ),
            (
                "values",
                "taskbar-show-values",
                "taskbar-show-values-description",
                widget.show_values,
                |widget, value| widget.show_values = value,
            ),
            (
                "labels",
                "taskbar-show-labels",
                "taskbar-show-labels-description",
                widget.show_labels,
                |widget, value| widget.show_labels = value,
            ),
        ];
        let reset = widget.reset;
        body.push(kit::card_of(k, |k| {
            let mut rows = toggles
                .into_iter()
                .map(|(id, title, description, value, set)| {
                    kit::toggle_row(
                        k,
                        format!("dock-{widget_id}-{id}"),
                        crate::i18n::tr(title),
                        Some(crate::i18n::tr(description).into()),
                        value,
                        Self::h(cx, move |this, value: bool, _, cx| {
                            this.update_dock_widgets(cx, |widgets| {
                                if let Some(widget) = widgets.get_mut(index) {
                                    set(widget, value);
                                }
                            })
                        }),
                    )
                })
                .collect::<Vec<_>>();
            rows.push(kit::dropdown_row(
                k,
                &format!("dock-{widget_id}-reset"),
                crate::i18n::tr("taskbar-reset-time"),
                Some(crate::i18n::tr("taskbar-reset-time-description")),
                kit::options(&[
                    crate::i18n::tr("taskbar-reset-off"),
                    crate::i18n::tr("taskbar-reset-countdown"),
                    crate::i18n::tr("taskbar-reset-clock"),
                ]),
                reset.index(),
                false,
                Self::h(cx, move |this, choice: usize, _, cx| {
                    let reset = TaskbarResetDisplay::ALL
                        .get(choice)
                        .copied()
                        .unwrap_or_default();
                    this.update_dock_widgets(cx, |widgets| {
                        if let Some(widget) = widgets.get_mut(index) {
                            widget.reset = reset;
                        }
                    })
                }),
            ));
            rows
        }));
        let max = TaskbarWidgetStyle::Glance.max_indicators();
        body.push(dialog_section(
            k,
            crate::i18n::tr("indicators"),
            crate::i18n::tr("cmdpal-indicators-hint"),
        ));
        let indicators = self.tray_indicators(
            k,
            IndicatorOwner::Dock,
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
            format!("dock-dlg-{}", widget.id),
            indicators,
        ));
        let remove = Button::new("dock-dlg-remove", crate::i18n::tr("cmdpal-remove-band"))
            .danger()
            .with_icon("trash-fill")
            .full_width()
            .on_click(Self::h(cx, move |this, (), _, cx| {
                this.remove_dock_widget(index, cx)
            }));
        let done = Button::new("dock-dlg-done", crate::i18n::tr("done"))
            .accent()
            .full_width()
            .on_click(close.clone());
        let body = body
            .into_iter()
            .map(|section| div().flex_none().child(section).into_any_element())
            .collect();
        Some(kit::dialog(
            k,
            format!("dock-dialog-{}", widget.id),
            phase,
            DIALOG_WIDTH,
            body,
            vec![remove.render(k), done.render(k)],
            Some(close),
        ))
    }
}
