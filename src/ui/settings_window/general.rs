//! General: startup, Usage Stats, the reset feed and provider status pages.

use gpui::{
    AnyElement, AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, SharedString, Styled, Subscription, Window, div, px, relative,
};

use super::Host;
use super::window::SettingsWindow;
use crate::format::currency;
use crate::settings::choice::Choice;
use crate::settings::{ResetAnnouncementRefreshInterval, UsageRefreshInterval};
use crate::ui::gui::input::{InputEvent, TextInput};
#[cfg(windows)]
use crate::ui::gui::kit::Button;
use crate::ui::gui::kit::{self, Kit, Row};

const CURRENCY_PICKER: &str = "general-usage-currency";
const CURRENCY_PICKER_WIDTH: f32 = 200.0;

/// Search state of the usage currency ComboBox.
#[derive(Default)]
pub(super) struct CurrencyPicker {
    input: Option<Entity<TextInput>>,
    _subscription: Option<Subscription>,
    /// The list was open last frame.
    open: bool,
    /// Codes as last rendered, for keyboard navigation.
    choices: Vec<&'static str>,
    selected: Option<usize>,
    highlighted: Option<usize>,
    /// Entry to scroll into view on the next frame.
    reveal: Option<usize>,
    /// The list was opened since the last frame.
    fresh: bool,
}

pub(super) fn usage_refresh_labels() -> [&'static str; 7] {
    [
        crate::i18n::tr("msg-1-minute"),
        crate::i18n::tr("msg-5-minutes"),
        crate::i18n::tr("msg-10-minutes"),
        crate::i18n::tr("msg-15-minutes"),
        crate::i18n::tr("msg-30-minutes"),
        crate::i18n::tr("msg-45-minutes"),
        crate::i18n::tr("msg-60-minutes"),
    ]
}

impl SettingsWindow {
    pub(super) fn general_page(
        &mut self,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let currency_row = self.currency_row(k, window, cx);
        let [symbol_row, position_row] = self.currency_symbol_rows(k, window, cx);
        let s = &self.settings;
        let start_at_login = s.start_at_login;
        let (usage_enabled, usage_interval) = (s.usage_stats_enabled, s.usage_refresh_interval);
        let feed = s.notifications.forced_reset_feed_enabled;
        let feed_toasts = s.notifications.forced_reset_notifications;
        let feed_interval = s.reset_announcement_refresh_interval;
        let status = s.notifications.provider_status_enabled;
        let status_toasts = s.notifications.provider_status_notifications;

        let language = kit::card_of(k, |k| {
            vec![kit::dropdown_row(
                k,
                "general-language",
                crate::i18n::tr("language"),
                Some(crate::i18n::tr(if cfg!(windows) {
                    "applies-immediately-to-every-app-window-and-notification-auto-fol"
                } else {
                    "linux-applies-immediately-to-every-app-window-and-notification-auto-fol"
                })),
                kit::options(&crate::i18n::Language::labels()),
                s.language.index(),
                false,
                Self::h(cx, |this, index: usize, _, cx| {
                    let language = crate::i18n::Language::from_index(index);
                    this.edit(cx, move |settings| settings.language = language);
                }),
            )]
        });

        let startup = kit::card_of(k, |k| {
            vec![kit::toggle_row(
                k,
                "general-startup",
                if cfg!(windows) {
                    crate::i18n::tr("start-with-windows")
                } else {
                    crate::i18n::tr("start-at-login")
                },
                Some(crate::i18n::tr("open-codex-minibar-in-the-tray-when-you-sign-in").into()),
                start_at_login,
                Self::h(cx, |this, value: bool, _, cx| {
                    this.edit(cx, move |settings| settings.start_at_login = value)
                }),
            )]
        });

        let shortcut = kit::card_of(k, |k| vec![self.popup_hotkey_row(k, cx)]);

        let usage = kit::card_of(k, |k| {
            vec![
                kit::toggle_row(
                    k,
                    "general-usage-stats",
                    crate::i18n::tr("enable-usage-stats"),
                    Some(
                        crate::i18n::tr(
                            "scan-local-provider-history-for-the-usage-tab-and-cost-totals",
                        )
                        .into(),
                    ),
                    usage_enabled,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |settings| settings.usage_stats_enabled = value)
                    }),
                ),
                self.usage_providers(k, usage_enabled, cx),
                currency_row,
                symbol_row,
                position_row,
                Row::new("general-currency-source", "Rates by Exchange Rate API")
                    .description(
                        k,
                        currency::note().unwrap_or_else(|| crate::i18n::tr("currency-usd").into()),
                    )
                    .on_click(kit::handler(|(), _, _| {
                        super::window::open_url(currency::SOURCE_URL)
                    }))
                    .render(k),
                kit::dropdown_row(
                    k,
                    "general-usage-refresh",
                    crate::i18n::tr("collection-period"),
                    Some(crate::i18n::tr(
                        "how-often-local-provider-history-is-scanned",
                    )),
                    kit::options(&usage_refresh_labels()),
                    usage_interval.index(),
                    !usage_enabled,
                    Self::h(cx, |this, index: usize, _, cx| {
                        let value = UsageRefreshInterval::from_index(index);
                        this.edit(cx, move |settings| settings.usage_refresh_interval = value)
                    }),
                ),
            ]
        });

        let resets = kit::card_of(k, |k| {
            vec![
                kit::toggle_row(
                    k,
                    "general-tibo-feed",
                    crate::i18n::tr("check-for-confirmed-tibo-resets"),
                    Some(
                        crate::i18n::tr(
                            "reads-the-app-s-public-github-feed-and-keeps-the-latest-announcem",
                        )
                        .into(),
                    ),
                    feed,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |settings| {
                            settings.notifications.forced_reset_feed_enabled = value
                        })
                    }),
                ),
                kit::toggle_row(
                    k,
                    "general-tibo-toast",
                    crate::i18n::tr("notify-when-new-reset-info-arrives"),
                    Some(
                        crate::i18n::tr(
                            "shows-a-notification-when-the-feed-reports-a-possible-reset-never",
                        )
                        .into(),
                    ),
                    feed_toasts,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |settings| {
                            settings.notifications.forced_reset_notifications = value
                        })
                    }),
                ),
                kit::dropdown_row(
                    k,
                    "general-tibo-interval",
                    crate::i18n::tr("check-every"),
                    Some(crate::i18n::tr(
                        "the-feed-is-also-checked-immediately-when-the-app-starts-or-this",
                    )),
                    kit::options(&[
                        crate::i18n::tr("msg-15-minutes"),
                        crate::i18n::tr("msg-30-minutes"),
                        crate::i18n::tr("msg-1-hour"),
                        crate::i18n::tr("msg-3-hours"),
                        crate::i18n::tr("msg-6-hours"),
                        crate::i18n::tr("msg-12-hours"),
                        crate::i18n::tr("msg-24-hours"),
                    ]),
                    feed_interval.index(),
                    !feed,
                    Self::h(cx, |this, index: usize, _, cx| {
                        let value = ResetAnnouncementRefreshInterval::from_index(index);
                        this.edit(cx, move |settings| {
                            settings.reset_announcement_refresh_interval = value
                        })
                    }),
                ),
            ]
        });

        let provider_status = kit::card_of(k, |k| {
            vec![
                kit::toggle_row(
                    k,
                    "general-provider-status",
                    crate::i18n::tr("show-provider-incidents"),
                    Some(crate::i18n::tr("show-provider-incidents-description").into()),
                    status,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |settings| {
                            settings.notifications.provider_status_enabled = value
                        })
                    }),
                ),
                kit::toggle_row(
                    k,
                    "general-provider-status-toast",
                    crate::i18n::tr("notify-about-new-incidents"),
                    Some(crate::i18n::tr("notify-about-new-incidents-description").into()),
                    status_toasts,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |settings| {
                            settings.notifications.provider_status_notifications = value
                        })
                    }),
                ),
            ]
        });

        vec![
            language,
            startup,
            shortcut,
            kit::section_heading(k, crate::i18n::tr("usage-stats")),
            usage,
            kit::section_heading(k, crate::i18n::tr("tibo-resets")),
            resets,
            kit::section_heading(k, crate::i18n::tr("provider-status")),
            provider_status,
        ]
    }

    /// Records the popup shortcut: the button waits for the next combination,
    /// Esc cancels and Backspace or Delete clears it.
    #[cfg(windows)]
    fn popup_hotkey_row(&self, k: &mut Kit, cx: &mut Context<Self>) -> AnyElement {
        let recording = self.hotkey_recording;
        let current = self.settings.popup_hotkey;
        let label: gpui::SharedString = if recording {
            crate::i18n::tr("press-a-shortcut").into()
        } else if let Some(hotkey) = current {
            hotkey.label().into()
        } else {
            crate::i18n::tr("shortcut-not-set").into()
        };
        let mut button = Button::new("general-popup-hotkey", label).on_click(Self::h(
            cx,
            |this, (), window, cx| {
                if this.hotkey_recording {
                    this.stop_hotkey_recording();
                } else {
                    this.hotkey_recording = true;
                    this.hotkey_rejected = false;
                    crate::app::hotkey::set_suspended(true);
                    // Text fields would otherwise consume the keystrokes.
                    window.focus(&this.focus);
                }
                cx.notify();
            },
        ));
        if recording {
            button = button.accent();
        }

        let mut row = Row::new(
            "general-popup-hotkey-row",
            crate::i18n::tr("popup-shortcut"),
        )
        .description(k, crate::i18n::tr("popup-shortcut-description"));
        let detail = if recording && self.hotkey_rejected {
            Some((
                crate::i18n::tr("popup-shortcut-needs-modifier"),
                k.theme.caution,
            ))
        } else if recording {
            Some((
                crate::i18n::tr("popup-shortcut-recording-hint"),
                k.theme.text_secondary,
            ))
        } else if current.is_some()
            && crate::app::hotkey::status() == crate::app::hotkey::Status::Unavailable
        {
            Some((crate::i18n::tr("popup-shortcut-in-use"), k.theme.caution))
        } else {
            None
        };
        if let Some((text, color)) = detail {
            row = row.detail(kit::text(text, 12.0, color).into_any_element());
        }
        if current.is_some() && !recording {
            row = row.trailing(
                Button::icon_only("general-popup-hotkey-clear", "x-circle-fill")
                    .ghost()
                    .tooltip(crate::i18n::tr("clear-shortcut"))
                    .on_click(Self::h(cx, |this, (), _, cx| {
                        this.edit(cx, |settings| settings.popup_hotkey = None)
                    }))
                    .render(k),
            );
        }
        row.trailing(button.render(k)).render(k)
    }

    pub(super) fn stop_hotkey_recording(&mut self) {
        self.hotkey_recording = false;
        self.hotkey_rejected = false;
        crate::app::hotkey::set_suspended(false);
    }

    /// Consumes a key press while the shortcut button is recording.
    pub(super) fn record_hotkey(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        if !self.hotkey_recording {
            return false;
        }
        let modifiers = &event.keystroke.modifiers;
        let bare = !(modifiers.control || modifiers.alt || modifiers.shift || modifiers.platform);
        match event.keystroke.key.as_str() {
            "escape" if bare => self.stop_hotkey_recording(),
            "backspace" | "delete" if bare => {
                self.stop_hotkey_recording();
                self.edit(cx, |settings| settings.popup_hotkey = None);
            }
            _ => match crate::app::hotkey::pressed_now()
                .filter(crate::app::hotkey::Hotkey::is_valid)
            {
                Some(hotkey) => {
                    self.stop_hotkey_recording();
                    self.edit(cx, move |settings| settings.popup_hotkey = Some(hotkey));
                }
                None => self.hotkey_rejected = true,
            },
        }
        cx.notify();
        true
    }

    /// Per-instance Usage Stats switches for enabled instances whose driver
    /// has local history. Unavailable instances stay listed, disabled, with why.
    fn usage_providers(
        &self,
        k: &mut Kit,
        usage_enabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let available = self
            .settings
            .instances
            .iter()
            .filter(|instance| {
                instance.enabled
                    && crate::providers::registry::descriptor(instance.driver)
                        .usage_stats
                        .is_supported()
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut grid = div().flex().flex_wrap().w_full().gap_y(px(10.0));
        if available.is_empty() {
            grid = grid.child(kit::caption(
                k,
                crate::i18n::tr("enable-a-provider-in-the-providers-tab-to-include-it-here"),
            ));
        }
        for instance in &available {
            let provider = instance.provider_id();
            let reason = crate::providers::instances::Capabilities::reason(
                instance,
                crate::providers::instances::Capability::UsageStats,
            )
            .or_else(|| {
                (!instance.has_usage_stats_credential())
                    .then_some(crate::i18n::tr("add-a-management-key"))
            })
            .or_else(|| {
                (!instance.usage_stats).then_some(crate::i18n::tr(
                    "usage-statistics-are-off-on-this-provider-s-page",
                ))
            });
            let checked = reason.is_none() && instance.in_usage_overview;
            let check = kit::checkbox(
                k,
                format!("usage-provider-{}", provider.id()),
                checked,
                !usage_enabled || reason.is_some(),
                Some(provider.qualified_name().into()),
                Self::h(cx, move |this, checked: bool, _, cx| {
                    this.edit(cx, move |settings| {
                        settings.set_usage_overview_included(provider, checked)
                    })
                }),
            );
            let check = match reason {
                Some(reason) => kit::tooltip_host(
                    k,
                    format!("usage-tip-{}", provider.id()),
                    format!("{}\n{reason}", provider.qualified_name()),
                    check,
                ),
                None => kit::tooltip_host(
                    k,
                    format!("usage-tip-{}", provider.id()),
                    provider.qualified_name(),
                    check,
                ),
            };
            grid = grid.child(div().w(relative(1.0 / 3.0)).pr(px(12.0)).child(check));
        }
        Row::new(
            "general-usage-providers",
            crate::i18n::tr("included-providers"),
        )
        .description(
            k,
            crate::i18n::tr("choose-which-accounts-count-toward-this-machine-s-usage-tab-and-h"),
        )
        .detail(div().pt(px(10.0)).child(grid).into_any_element())
        .disabled(!usage_enabled)
        .render(k)
    }
}

impl SettingsWindow {
    /// Searchable currency ComboBox; a plain disabled dropdown while Usage
    /// Stats is off.
    fn currency_row(
        &mut self,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let title = crate::i18n::tr("usage-currency");
        let description = crate::i18n::tr("usage-currency-description");
        let current = self.settings.usage_currency.clone();
        if !self.settings.usage_stats_enabled {
            return kit::dropdown_row(
                k,
                CURRENCY_PICKER,
                title,
                Some(description),
                vec![SharedString::from(current)],
                0,
                true,
                kit::handler(|_: usize, _, _| {}),
            );
        }
        let input = self.currency_search_input(window, cx);
        let open = k.menus.is_open(CURRENCY_PICKER);
        if self.currency_picker.open && !open {
            self.currency_picker.open = false;
            if input.read(cx).is_focused(window) {
                window.focus(&self.focus_handle(cx));
            }
        }
        let query = input.read(cx).text().trim().to_uppercase();
        let choices: Vec<&'static str> = currency::CODES
            .iter()
            .copied()
            .filter(|code| code.contains(query.as_str()))
            .collect();
        let selected = choices.iter().position(|code| *code == current);
        let picker = &mut self.currency_picker;
        picker.highlighted = picker
            .highlighted
            .filter(|index| *index < choices.len())
            .or_else(|| (!query.is_empty() && !choices.is_empty()).then_some(0));
        picker.selected = selected;
        picker.choices = choices.clone();
        let highlighted = picker.highlighted;
        let reveal = picker.reveal.take();
        let fresh = std::mem::take(&mut picker.fresh);
        let items = if choices.is_empty() {
            vec![kit::MenuItem::new(crate::i18n::tr("no-matching-currencies")).disabled()]
        } else {
            choices
                .iter()
                .map(|code| kit::MenuItem::new(*code))
                .collect()
        };
        let on_select = Self::h(cx, move |this, index: usize, _, cx| {
            if let Some(code) = choices.get(index).copied() {
                this.choose_currency(code, cx);
            }
        });
        let on_open = {
            let input = input.clone();
            Self::h(cx, move |this, (), window, cx| {
                let picker = &mut this.currency_picker;
                picker.open = true;
                picker.fresh = true;
                picker.highlighted = None;
                picker.reveal = None;
                input.update(cx, |input, cx| input.set_text("", cx));
                window.focus(&input.focus_handle(cx));
                cx.notify();
            })
        };
        let combo = kit::SearchDropdown {
            id: CURRENCY_PICKER.into(),
            label: current.into(),
            input,
            items,
            selected,
            highlighted,
            reveal,
            fresh,
            width: CURRENCY_PICKER_WIDTH,
            on_open,
            on_select,
            on_hover: kit::handler(|_: (usize, bool), _, _| {}),
        }
        .render(k, window, cx);
        let combo = div()
            .on_key_down(cx.listener(Self::currency_picker_key))
            .child(combo)
            .into_any_element();
        Row::new(format!("row-{CURRENCY_PICKER}"), title)
            .description(k, description)
            .trailing(combo)
            .render(k)
    }

    /// The picker's search field, created on first use.
    fn currency_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        if let Some(input) = &self.currency_picker.input {
            return input.clone();
        }
        let input = cx.new(|cx| TextInput::new(window, cx));
        let subscription = cx.subscribe_in(
            &input,
            window,
            |this: &mut Self, _, event: &InputEvent, window, cx| match event {
                InputEvent::Changed => {
                    this.currency_picker.highlighted = None;
                    this.currency_picker.reveal = Some(0);
                    cx.notify();
                }
                InputEvent::Submit => {
                    let picker = &this.currency_picker;
                    let code = picker
                        .highlighted
                        .and_then(|index| picker.choices.get(index).copied());
                    this.kit.menus.close(window);
                    if let Some(code) = code {
                        this.choose_currency(code, cx);
                    }
                    cx.notify();
                }
                InputEvent::Blur => {}
            },
        );
        self.currency_picker.input = Some(input.clone());
        self.currency_picker._subscription = Some(subscription);
        input
    }

    /// Up and Down walk the list while its field has focus.
    fn currency_picker_key(
        &mut self,
        event: &KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let picker = &mut self.currency_picker;
        if !picker.open {
            return;
        }
        let Some(last) = picker.choices.len().checked_sub(1) else {
            return;
        };
        let from = picker.highlighted.or(picker.selected);
        let next = match event.keystroke.key.as_str() {
            "down" => from.map_or(0, |index| (index + 1).min(last)),
            "up" => from.map_or(0, |index| index.saturating_sub(1)),
            "pagedown" => from.map_or(0, |index| (index + 8).min(last)),
            "pageup" => from.map_or(0, |index| index.saturating_sub(8)),
            _ => return,
        };
        picker.highlighted = Some(next);
        picker.reveal = Some(next);
        cx.stop_propagation();
        cx.notify();
    }

    fn choose_currency(&mut self, code: &'static str, cx: &mut Context<Self>) {
        if self.settings.usage_currency != code {
            // A symbol belongs to the currency it was typed for.
            self.edit(cx, move |settings| {
                settings.usage_currency = code.into();
                settings.usage_currency_symbol.clear();
                settings.usage_currency_symbol_after = false;
            });
        }
    }

    /// The custom symbol field and which side of the amount it sits on.
    fn currency_symbol_rows(
        &mut self,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> [AnyElement; 2] {
        let disabled = !self.settings.usage_stats_enabled;
        let code = self.settings.usage_currency.clone();
        let symbol = self.settings.usage_currency_symbol.clone();
        let after = self.settings.usage_currency_symbol_after;
        let input = self.input(
            "general-usage-currency-symbol",
            &symbol,
            currency::default_symbol(&code).to_owned(),
            false,
            disabled,
            window,
            cx,
            |this, value, _, _, cx| {
                let value = currency::clean_symbol(&value);
                if this.settings.usage_currency_symbol != value {
                    this.edit(cx, move |settings| {
                        settings.usage_currency_symbol = value.clone()
                    });
                }
            },
        );
        let field = kit::text_field(k, &input, Some(120.0), window, cx);
        let symbol_row = Row::new(
            "row-general-usage-currency-symbol",
            crate::i18n::tr("currency-symbol"),
        )
        .description(k, crate::i18n::tr("currency-symbol-description"))
        .trailing(field)
        .disabled(disabled)
        .render(k);
        let position = kit::segmented(
            k,
            "general-usage-currency-symbol-position",
            &[
                crate::i18n::tr("currency-symbol-before"),
                crate::i18n::tr("currency-symbol-after"),
            ],
            usize::from(after),
            disabled,
            Self::h(cx, |this, index: usize, _, cx| {
                let after = index == 1;
                this.edit(cx, move |settings| {
                    settings.usage_currency_symbol_after = after
                });
            }),
        );
        let position_row = Row::new(
            "row-general-usage-currency-symbol-position",
            crate::i18n::tr("currency-symbol-position"),
        )
        .trailing(position)
        .disabled(disabled)
        .render(k);
        [symbol_row, position_row]
    }
}
