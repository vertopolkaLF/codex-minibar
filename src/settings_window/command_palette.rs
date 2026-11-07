//! Integrations: the PowerToys Command Palette extension and its Dock bands.
//!
//! Bands are Glance taskbar widgets in their own list. The Dock draws each as
//! the provider mark over a title and a subtitle, so the editor offers only
//! what reaches that text.

use futures::StreamExt;
use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, Window, div, px};

use super::Host;
use super::widget_list::{Dock, dialog_section, preview_limits, widget_providers};
use super::window::SettingsWindow;
use crate::choice::Choice;
use crate::cmdpal::InstallPhase;
use crate::gui::color::rgb8;
use crate::gui::kit::{self, Button, Kit, Row};
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
        let empty = Row::new("dock-empty", crate::i18n::tr("cmdpal-no-bands"))
            .description(k, crate::i18n::tr("cmdpal-no-bands-description"));
        rows.extend(
            self.widget_list::<Dock>(k, cx, empty, |this, k, index, widget| {
                Row::new(format!("dock-row-{}", widget.id), band_title(index))
                    .icon(this.dock_band(k, widget))
            }),
        );
        let providers = widget_providers(&self.settings.instances, true);
        let first = providers.first().copied();
        let add = Self::h(cx, move |this, (), _, cx| {
            if let Some(provider) = first {
                this.add_widget::<Dock>(TaskbarWidget::dock_for_provider(provider), cx);
            }
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
                let seeds = widget_providers(&this.settings.instances, true);
                if this.settings.command_palette.widgets.is_empty() && !seeds.is_empty() {
                    this.update_widgets::<Dock>(cx, |widgets| {
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

    /// The band as the Dock lays it out: the mark beside a title over a
    /// subtitle, with sample quotas.
    fn dock_band(&self, k: &Kit, widget: &TaskbarWidget) -> AnyElement {
        let light = !k.theme.dark;
        let limits = preview_limits(&self.settings.instances);
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
            crate::gui::mark::provider_mark(
                icon.name,
                16.0,
                rgb8((red, green, blue)),
                view.provider.and_then(ProviderId::badge).as_ref(),
                k.theme.plate(),
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

    /// The band editor: what the title and subtitle show, then which quotas.
    /// Every choice reaches the Dock immediately.
    pub(super) fn dock_dialog_overlay(
        &mut self,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (dialog, index, widget, phase) = self.widget_dialog::<Dock>(k)?;
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
                            this.update_widgets::<Dock>(cx, |widgets| {
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
                    this.update_widgets::<Dock>(cx, |widgets| {
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
        let indicators = self.indicator_editor::<Dock>(
            k,
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
        Some(self.widget_dialog_frame::<Dock>(k, cx, &widget.id, index, phase, DIALOG_WIDTH, body))
    }
}
