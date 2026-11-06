//! Linux popup shortcut: a dialog records a key combination and Save hands it
//! to the desktop's own keyboard settings (`linux::keybinding`).

use gpui::{AnyElement, Context, IntoElement, KeyDownEvent, ParentElement, Styled, div, px};

use super::kit::{self, Button, Kit, Row};
use super::window::SettingsWindow;
use crate::linux::keybinding::{self, Accelerator};

/// What the shortcut dialog shows and has recorded so far.
#[derive(Clone, Default)]
pub(super) struct ShortcutDialog {
    pending: Option<Accelerator>,
    /// The last combination pressed had no Ctrl, Alt or Super.
    rejected: bool,
    /// The desktop refused the write.
    failed: bool,
    saving: bool,
}

/// The shortcut as the desktop currently has it.
pub(super) struct DesktopShortcut {
    /// Whether this desktop keeps shortcuts where Minibar can write them.
    pub(super) supported: bool,
    pub(super) current: Option<Accelerator>,
}

impl DesktopShortcut {
    pub(super) fn load() -> Self {
        let supported = keybinding::supported();
        Self {
            supported,
            current: supported.then(keybinding::current).flatten(),
        }
    }
}

impl SettingsWindow {
    pub(super) fn popup_hotkey_row(&self, k: &mut Kit, cx: &mut Context<Self>) -> AnyElement {
        let command = crate::linux::instance::toggle_command();
        let mut row = Row::new(
            "general-popup-hotkey-row",
            crate::i18n::tr("popup-shortcut"),
        );
        if !self.desktop_shortcut.supported {
            // Other desktops keep shortcuts where we cannot write them: show
            // the command to bind by hand.
            let copied = command.clone();
            return row
                .description(k, crate::i18n::tr("linux-popup-shortcut-description"))
                .detail(kit::text(command, 12.0, k.theme.text_secondary).into_any_element())
                .trailing(
                    Button::icon_only("general-popup-hotkey-copy", "copy")
                        .ghost()
                        .tooltip(crate::i18n::tr("copy-command"))
                        .on_click(Self::h(cx, move |this, (), _, cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(copied.clone()));
                            this.show_notice(crate::i18n::tr("command-copied"), cx);
                        }))
                        .render(k),
                )
                .render(k);
        }
        let label: gpui::SharedString = match &self.desktop_shortcut.current {
            Some(accelerator) => accelerator.label().into(),
            None => crate::i18n::tr("linux-popup-shortcut-choose").into(),
        };
        row = row.description(
            k,
            crate::i18n::tr("linux-popup-shortcut-native-description"),
        );
        row.trailing(
            Button::new("general-popup-hotkey", label)
                .on_click(Self::h(cx, |this, (), window, cx| {
                    // The shortcut may have been edited in the desktop's own
                    // settings since this window opened.
                    this.desktop_shortcut.current = keybinding::current();
                    this.shortcut_dialog = Some(ShortcutDialog {
                        pending: this.desktop_shortcut.current.clone(),
                        ..ShortcutDialog::default()
                    });
                    // Text fields would otherwise consume the keystrokes.
                    window.focus(&this.focus);
                    cx.notify();
                }))
                .render(k),
        )
        .render(k)
    }

    /// Consumes a key press while the shortcut dialog is open.
    pub(super) fn record_shortcut(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let Some(dialog) = self.shortcut_dialog.as_mut() else {
            return false;
        };
        if dialog.saving {
            return true;
        }
        let modifiers = &event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();
        let bare = !(modifiers.control || modifiers.alt || modifiers.shift || modifiers.platform);
        match key {
            "escape" if bare => self.shortcut_dialog = None,
            "enter" if bare => {
                if dialog.pending.is_some() && dialog.pending != self.desktop_shortcut.current {
                    self.save_shortcut(cx);
                }
            }
            _ => {
                if let Some(accelerator) = Accelerator::from_press(
                    key,
                    modifiers.control,
                    modifiers.alt,
                    modifiers.shift,
                    modifiers.platform,
                ) {
                    dialog.failed = false;
                    if accelerator.is_valid() {
                        dialog.rejected = false;
                        dialog.pending = Some(accelerator);
                    } else {
                        dialog.rejected = true;
                    }
                }
            }
        }
        cx.notify();
        true
    }

    /// Writes the recorded combination (or, with none, removes the shortcut)
    /// to the desktop without blocking the window.
    fn save_shortcut(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.shortcut_dialog.as_mut() else {
            return;
        };
        if dialog.saving || dialog.pending == self.desktop_shortcut.current {
            return;
        }
        let value = dialog.pending.clone();
        dialog.saving = true;
        dialog.failed = false;
        let task = cx.background_executor().spawn({
            let value = value.clone();
            async move { keybinding::save(value.as_ref()) }
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        crate::logger::info(format!(
                            "Popup shortcut saved: {}",
                            value
                                .as_ref()
                                .map_or("none".to_owned(), Accelerator::to_gtk)
                        ));
                        this.desktop_shortcut.current = value;
                        this.shortcut_dialog = None;
                    }
                    Err(error) => {
                        crate::logger::info(format!("Popup shortcut not saved: {error:#}"));
                        if let Some(dialog) = this.shortcut_dialog.as_mut() {
                            dialog.saving = false;
                            dialog.failed = true;
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn shortcut_overlay(
        &mut self,
        k: &mut Kit,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (dialog, phase) = self
            .overlays
            .shortcut
            .track(k, self.shortcut_dialog.clone())?;
        let theme = k.theme.clone();
        let cancel = Self::h(cx, |this, (), _, cx| {
            if this
                .shortcut_dialog
                .as_ref()
                .is_some_and(|dialog| dialog.saving)
            {
                return;
            }
            this.shortcut_dialog = None;
            cx.notify();
        });
        let save = Self::h(cx, |this, (), _, cx| this.save_shortcut(cx));
        let remove = Self::h(cx, |this, (), _, cx| {
            if let Some(dialog) = this.shortcut_dialog.as_mut() {
                dialog.pending = None;
            }
            this.save_shortcut(cx);
        });

        let label: gpui::SharedString = match &dialog.pending {
            Some(accelerator) => accelerator.label().into(),
            None => crate::i18n::tr("press-a-shortcut").into(),
        };
        let keys = div()
            .flex()
            .items_center()
            .justify_center()
            .w_full()
            .h(px(72.0))
            .rounded(px(8.0))
            .bg(theme.subtle_hover)
            .border_1()
            .border_color(theme.divider)
            .child(kit::text(
                label,
                22.0,
                if dialog.pending.is_some() {
                    theme.text
                } else {
                    theme.text_secondary
                },
            ))
            .into_any_element();
        let mut body = vec![
            kit::dialog_title(k, crate::i18n::tr("popup-shortcut")),
            kit::caption(k, crate::i18n::tr("linux-popup-shortcut-dialog-hint")),
            keys,
        ];
        if dialog.rejected {
            body.push(
                kit::text(
                    crate::i18n::tr("linux-popup-shortcut-needs-modifier"),
                    12.0,
                    theme.caution,
                )
                .into_any_element(),
            );
        }
        if dialog.failed {
            body.push(
                kit::text(
                    crate::i18n::tr("linux-popup-shortcut-save-failed"),
                    12.0,
                    theme.caution,
                )
                .into_any_element(),
            );
        }
        let changed = dialog.pending.is_some() && dialog.pending != self.desktop_shortcut.current;
        let mut buttons = Vec::with_capacity(3);
        if self.desktop_shortcut.current.is_some() {
            buttons.push(
                Button::new("shortcut-remove", crate::i18n::tr("clear-shortcut"))
                    .disabled(dialog.saving)
                    .full_width()
                    .on_click(remove)
                    .render(k),
            );
        }
        buttons.push(
            Button::new("shortcut-cancel", crate::i18n::tr("cancel"))
                .disabled(dialog.saving)
                .full_width()
                .on_click(cancel.clone())
                .render(k),
        );
        buttons.push(
            Button::new("shortcut-save", crate::i18n::tr("save"))
                .accent()
                .disabled(dialog.saving || !changed)
                .full_width()
                .on_click(save)
                .render(k),
        );
        Some(kit::dialog(
            k,
            "shortcut",
            phase,
            420.0,
            body,
            buttons,
            Some(cancel),
        ))
    }
}
