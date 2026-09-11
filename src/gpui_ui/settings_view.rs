use super::*;
use gpui::{prelude::*, Focusable, *};
use crate::settings::*;
use gpui_component::{
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::{Input, InputEvent, InputState},
    switch::Switch,
    ActiveTheme,
};
use serde_json::Value;
use std::collections::HashMap;

use super::popup_view::card;

const PAGES: &[(&str, &str, &str)] = &[
    ("general", "General", "nav-general"),
    ("appearance", "Appearance", "nav-appearance"),
    ("providers", "Providers", "nav-providers"),
    ("popup", "Popup", "nav-popup"),
    ("schedule", "Limit activation", "nav-schedule"),
    ("tray", "Tray", "nav-tray"),
    ("notifications", "Notifications", "nav-notifications"),
    ("advanced", "Advanced", "nav-advanced"),
    ("log", "Log", "nav-log"),
    ("about", "About", "nav-about"),
];

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

pub(super) struct SettingsView {
    model: Entity<Model>,
    page: &'static str,
    inputs: HashMap<String, Entity<InputState>>,
    subscriptions: Vec<Subscription>,
    error: Option<String>,
    expanded: Option<String>,
    confirm: Option<&'static str>,
}

impl SettingsView {
    pub fn new(model: Entity<Model>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe_in(&model, window, |this, _, window, cx| {
            let document = serde_json::to_value(&this.model.read(cx).snapshot.settings).unwrap();
            for (path, input) in &this.inputs {
                if !path.starts_with('/') || input.read(cx).focus_handle(cx).is_focused(window) {
                    continue;
                }
                if let Some(value) = document.pointer(path) {
                    let text = match value {
                        Value::String(value) => value.clone(),
                        Value::Null => String::new(),
                        _ => value.to_string(),
                    };
                    if input.read(cx).value().as_ref() != text {
                        input.update(cx, |input, cx| input.set_value(text, window, cx));
                    }
                }
            }
            cx.notify();
        });
        let onboarding = !model.read(cx).snapshot.settings.onboarding_completed;
        Self {
            model,
            page: if onboarding { "providers" } else { "general" },
            inputs: HashMap::new(),
            subscriptions: vec![subscription],
            error: None,
            expanded: None,
            confirm: None,
        }
    }

    fn patch(&mut self, path: &str, value: Value, cx: &mut Context<Self>) {
        let mut document =
            serde_json::to_value(&self.model.read(cx).snapshot.settings).expect("serialize settings");
        if let Some(slot) = document.pointer_mut(path) {
            *slot = value;
        } else {
            self.error = Some("This setting is no longer available.".into());
            cx.notify();
            return;
        }
        match serde_json::from_value::<Settings>(document) {
            Ok(settings) => {
                self.error = None;
                self.model
                    .update(cx, |model, cx| model.edit(|current| *current = settings, cx));
            }
            Err(error) => {
                self.error = Some(format!("Invalid value: {error}"));
                cx.notify();
            }
        }
    }

    fn field(
        &mut self,
        path: String,
        label: String,
        value: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Value::Bool(value) = value {
            let path2 = path.clone();
            return card(cx)
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .gap_3()
                        .child(div().flex_1().child(label))
                        .child(switch(path).checked(*value).on_click(cx.listener(
                            move |this, value, _, cx| this.patch(&path2, Value::Bool(*value), cx),
                        ))),
                )
                .into_any_element();
        }
        if let Value::Object(fields) = value {
            let mut group = div()
                .flex()
                .flex_col()
                .gap_2()
                .child(div().mt_2().font_weight(FontWeight::SEMIBOLD).child(label));
            for (key, value) in fields {
                if key == "id" || key == "api_key_ids" {
                    continue;
                }
                group = group.child(self.field(
                    format!("{path}/{}", escape(key)),
                    human(key),
                    value,
                    window,
                    cx,
                ));
            }
            return group.into_any_element();
        }
        let key = path.rsplit('/').next().unwrap_or("");
        let options = options(key);
        if !options.is_empty() {
            let value_label = value.as_str().map(human).unwrap_or_default();
            let expand_path = path.clone();
            let mut control = card(cx).child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(label)
                    .child(button(path.clone()).label(value_label).on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.expanded = if this.expanded.as_ref() == Some(&expand_path) {
                                None
                            } else {
                                Some(expand_path.clone())
                            };
                            cx.notify();
                        }),
                    )),
            );
            if self.expanded.as_ref() == Some(&path) {
                let mut choices = div().flex().flex_wrap().gap_2();
                for option in options {
                    let path = path.clone();
                    choices = choices.child(button(format!("{path}-{option}")).label(human(option)).on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.expanded = None;
                            this.patch(&path, Value::String(option.into()), cx);
                            cx.notify();
                        }),
                    ));
                }
                control = control.child(choices);
            }
            return control.into_any_element();
        }
        let input_key = path.clone();
        if !self.inputs.contains_key(&path) {
            let text = match value {
                Value::String(value) => value.clone(),
                Value::Null => String::new(),
                _ => value.to_string(),
            };
            let number = value.is_number();
            let nullable = value.is_null() || key.ends_with("_path");
            let input = cx.new(|cx| InputState::new(window, cx).default_value(text));
            let subscription = cx.subscribe_in(&input, window, move |this, input, event, _, cx| {
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    let text = input.read(cx).value().to_string();
                    let value = if number {
                        match text.parse::<i64>() {
                            Ok(value) => Value::from(value),
                            Err(_) => {
                                this.error = Some("Enter a whole number.".into());
                                cx.notify();
                                return;
                            }
                        }
                    } else if nullable && text.trim().is_empty() {
                        Value::Null
                    } else {
                        Value::String(text)
                    };
                    this.patch(&input_key, value, cx);
                }
            });
            self.subscriptions.push(subscription);
            self.inputs.insert(path.clone(), input);
        }
        card(cx)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(div().flex_1().child(label))
                    .child(div().w(px(180.)).child(Input::new(self.inputs.get(&path).unwrap()))),
            )
            .into_any_element()
    }

    fn secret(
        &mut self,
        id: String,
        label: String,
        provider: ProviderKind,
        account: Option<(String, Option<String>)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !self.inputs.contains_key(&id) {
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .masked(true)
                    .placeholder("Paste key to replace")
            });
            self.inputs.insert(id.clone(), input);
        }
        let input = self.inputs[&id].clone();
        let save_input = input.clone();
        let clear_account = account.clone();
        card(cx)
            .child(label)
            .child(Input::new(&input))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(button(format!("save-{id}")).primary().label("Save key").on_click(
                        cx.listener(move |this, _, window, cx| {
                            let value = save_input.read(cx).value().to_string();
                            if value.trim().is_empty() {
                                return;
                            }
                            this.save_key(provider, account.clone(), Some(value.trim()), cx);
                            save_input.update(cx, |input, cx| input.set_value("", window, cx));
                        }),
                    ))
                    .child(button(format!("clear-{id}")).label("Remove key").on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.save_key(provider, clear_account.clone(), None, cx)
                        }),
                    )),
            )
            .into_any_element()
    }

    fn save_key(
        &mut self,
        provider: ProviderKind,
        account: Option<(String, Option<String>)>,
        value: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let result = if let Some((account, key)) = account {
            if let Some(key) = key {
                crate::openrouter::save_account_api_key(&account, &key, value)
            } else {
                crate::openrouter::save_management_key(&account, value)
            }
        } else {
            crate::opencode::save_manual_key(provider, value)
        };
        match result {
            Ok(()) => self.model.update(cx, |model, cx| {
                model.edit(
                    |settings| match provider {
                        ProviderKind::OpenCodeZen => settings.opencode_zen_credentials_revision += 1,
                        ProviderKind::OpenCodeGo => settings.opencode_go_credentials_revision += 1,
                        _ => settings.openrouter_credentials_revision += 1,
                    },
                    cx,
                )
            }),
            Err(error) => self.error = Some(error.to_string()),
        }
        cx.notify();
    }

    fn providers(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let settings = self.model.read(cx).snapshot.settings.clone();
        let mut body = div().flex().flex_col().gap_3();
        if !settings.onboarding_completed {
            body = body.child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child("Choose the tools you want in the tray and popup. You can change this later."),
            );
        }
        for provider in ProviderKind::ALL {
            let model = self.model.clone();
            body = body.child(card(cx).child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(provider.display_name())
                    .child(
                        switch(format!("provider-{}", provider.id()))
                            .checked(settings.providers.is_enabled(provider))
                            .on_click(move |value, _, cx| {
                                model.update(cx, |model, cx| {
                                    model.edit(
                                        |settings| settings.providers.set_enabled(provider, *value),
                                        cx,
                                    )
                                })
                            }),
                    ),
            ));
            if !settings.providers.is_enabled(provider) {
                continue;
            }
            if matches!(
                provider,
                ProviderKind::Codex | ProviderKind::Claude | ProviderKind::Cursor
            ) {
                let path = format!("/{}_path", provider.id());
                let document = serde_json::to_value(&settings).unwrap();
                body = body.child(self.field(
                    path.clone(),
                    "Application path (automatic when empty)".into(),
                    document.pointer(&path).unwrap_or(&Value::Null),
                    window,
                    cx,
                ));
            }
            if matches!(provider, ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo) {
                body = body.child(self.secret(
                    format!("key-{}", provider.id()),
                    "API key".into(),
                    provider,
                    None,
                    window,
                    cx,
                ));
            }
            if provider == ProviderKind::OpenRouter {
                for (index, account) in settings.openrouter_accounts.iter().enumerate() {
                    body = body.child(self.field(
                        format!("/openrouter_accounts/{index}/name"),
                        "Account name".into(),
                        &Value::String(account.name.clone()),
                        window,
                        cx,
                    ));
                    body = body.child(self.secret(
                        format!("management-{}", account.id),
                        "Management key".into(),
                        provider,
                        Some((account.id.clone(), None)),
                        window,
                        cx,
                    ));
                    for key in &account.api_key_ids {
                        body = body.child(self.secret(
                            format!("api-{}-{key}", account.id),
                            "API key".into(),
                            provider,
                            Some((account.id.clone(), Some(key.clone()))),
                            window,
                            cx,
                        ));
                    }
                    let model = self.model.clone();
                    let id = account.id.clone();
                    body = body.child(
                        button(format!("add-key-{}", account.id))
                            .label("Add API key")
                            .on_click(move |_, _, cx| {
                                model.update(cx, |model, cx| {
                                    model.edit(
                                        |settings| {
                                            if let Some(account) = settings
                                                .openrouter_accounts
                                                .iter_mut()
                                                .find(|account| account.id == id)
                                            {
                                                account
                                                    .api_key_ids
                                                    .push(OpenRouterAccount::new_api_key_id());
                                            }
                                        },
                                        cx,
                                    )
                                })
                            }),
                    );
                }
                let model = self.model.clone();
                body = body.child(button("add-account").label("Add account").on_click(
                    move |_, _, cx| {
                        model.update(cx, |model, cx| {
                            model.edit(
                                |settings| settings.openrouter_accounts.push(OpenRouterAccount::default()),
                                cx,
                            )
                        })
                    },
                ));
            }
        }
        body.into_any_element()
    }

    fn popup_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let settings = self.model.read(cx).snapshot.settings.clone();
        let mut body = div().flex().flex_col().gap_3();
        for (path, label) in [
            ("show_used_percentage", "Show used percentage"),
            ("show_usage_pace", "Show usage pace"),
            ("compact_usage_cards", "Compact usage cards"),
            ("show_account_name", "Show account name"),
            ("show_total_spend_on_all_tab", "Show Usage Stats on Home"),
        ] {
            let model = self.model.clone();
            let checked = match path {
                "show_used_percentage" => settings.show_used_percentage,
                "show_usage_pace" => settings.show_usage_pace,
                "compact_usage_cards" => settings.compact_usage_cards,
                "show_account_name" => settings.show_account_name,
                _ => settings.show_total_spend_on_all_tab,
            };
            body = body.child(toggle_card(cx, path, label, checked, move |value, cx| {
                model.update(cx, |model, cx| {
                    model.edit(
                        |settings| match path {
                            "show_used_percentage" => settings.show_used_percentage = value,
                            "show_usage_pace" => settings.show_usage_pace = value,
                            "compact_usage_cards" => settings.compact_usage_cards = value,
                            "show_account_name" => settings.show_account_name = value,
                            _ => settings.show_total_spend_on_all_tab = value,
                        },
                        cx,
                    )
                })
            }));
        }
        body = body.child(enum_row(
            self,
            cx,
            "total-spend-presentation",
            "Usage Stats layout",
            settings.total_spend_presentation.id(),
            &[("donut", "Donut"), ("progress_bar", "Cards")],
            |settings, value| {
                settings.total_spend_presentation = if value == "progress_bar" {
                    TotalSpendPresentation::ProgressBar
                } else {
                    TotalSpendPresentation::Donut
                };
            },
        ));
        body = body.child(div().font_weight(FontWeight::SEMIBOLD).child("Home tab order"));
        for (index, widget) in settings.popup_order.iter().enumerate() {
            let label = widget
                .as_provider()
                .map(ProviderKind::display_name)
                .unwrap_or("Usage Stats");
            let model_up = self.model.clone();
            let model_down = self.model.clone();
            body = body.child(card(cx).child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().child(label))
                    .child(button(format!("popup-up-{index}")).label("Up").on_click(
                        move |_, _, cx| {
                            if index == 0 {
                                return;
                            }
                            model_up.update(cx, |model, cx| {
                                model.edit(
                                    |settings| settings.popup_order.swap(index, index - 1),
                                    cx,
                                )
                            });
                        },
                    ))
                    .child(button(format!("popup-down-{index}")).label("Down").on_click(
                        move |_, _, cx| {
                            model_down.update(cx, |model, cx| {
                                model.edit(
                                    |settings| {
                                        if index + 1 < settings.popup_order.len() {
                                            settings.popup_order.swap(index, index + 1);
                                        }
                                    },
                                    cx,
                                )
                            });
                        },
                    )),
            ));
        }
        body = body.child(div().font_weight(FontWeight::SEMIBOLD).child("Provider cards"));
        for provider in settings.ordered_enabled_providers() {
            let model = self.model.clone();
            let shown = settings.popup_visibility.provider_shown_on_all(provider);
            body = body.child(toggle_card(
                cx,
                &format!("all-{}", provider.id()),
                &format!("Show {} on Home", provider.display_name()),
                shown,
                move |value, cx| {
                    model.update(cx, |model, cx| {
                        model.edit(
                            |settings| settings.popup_visibility.set_provider_all_tab(provider, value),
                            cx,
                        )
                    })
                },
            ));
            for brick_id in crate::provider_registry::catalog_brick_ids(provider) {
                let visibility = settings.popup_visibility.visibility_for(&brick_id);
                let label = crate::provider_registry::brick_label(provider, &brick_id);
                let model_home = self.model.clone();
                let model_tab = self.model.clone();
                let brick_home = brick_id.clone();
                let brick_tab = brick_id.clone();
                body = body.child(card(cx).child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(label)
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .child(
                                    Checkbox::new(SharedString::from(format!("home-{brick_id}")))
                                        .label("Home")
                                        .checked(visibility.all_tab)
                                        .on_click(move |value, _, cx| {
                                            let brick = brick_home.clone();
                                            model_home.update(cx, |model, cx| {
                                                model.edit(
                                                    |settings| {
                                                        let current = settings
                                                            .popup_visibility
                                                            .visibility_for(&brick);
                                                        settings.popup_visibility.set_brick(
                                                            brick,
                                                            *value,
                                                            current.provider_tab,
                                                        );
                                                    },
                                                    cx,
                                                )
                                            });
                                        }),
                                )
                                .child(
                                    Checkbox::new(SharedString::from(format!("tab-{brick_id}")))
                                        .label("Provider tab")
                                        .checked(visibility.provider_tab)
                                        .on_click(move |value, _, cx| {
                                            let brick = brick_tab.clone();
                                            model_tab.update(cx, |model, cx| {
                                                model.edit(
                                                    |settings| {
                                                        let current = settings
                                                            .popup_visibility
                                                            .visibility_for(&brick);
                                                        settings.popup_visibility.set_brick(
                                                            brick,
                                                            current.all_tab,
                                                            *value,
                                                        );
                                                    },
                                                    cx,
                                                )
                                            });
                                        }),
                                ),
                        ),
                ));
            }
        }
        body.into_any_element()
    }

    fn schedule_page(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let settings = self.model.read(cx).snapshot.settings.clone();
        let model = self.model.clone();
        let mut body = div().flex().flex_col().gap_3().child(toggle_card(
            cx,
            "automatic_activation",
            "Automatic activation",
            settings.automatic_activation,
            move |value, cx| {
                model.update(cx, |model, cx| {
                    model.edit(|settings| settings.automatic_activation = value, cx)
                })
            },
        ));
        body = body.child(div().font_weight(FontWeight::SEMIBOLD).child("Scheduled activations"));
        for (index, rule) in settings.scheduled_activations.iter().enumerate() {
            body = body.child(self.schedule_rule(index, rule, false, window, cx));
        }
        let model = self.model.clone();
        body = body.child(button("add-schedule").label("Add schedule").on_click(move |_, _, cx| {
            model.update(cx, |model, cx| {
                model.edit(
                    |settings| settings.scheduled_activations.push(ScheduledActivation::default()),
                    cx,
                )
            })
        }));
        body = body.child(div().font_weight(FontWeight::SEMIBOLD).child("Quiet periods"));
        for (index, rule) in settings.auto_activation_pauses.iter().enumerate() {
            body = body.child(self.schedule_rule(index, &pause_as_schedule(rule), true, window, cx));
        }
        let model = self.model.clone();
        body = body.child(button("add-pause").label("Add quiet period").on_click(move |_, _, cx| {
            model.update(cx, |model, cx| {
                model.edit(
                    |settings| settings.auto_activation_pauses.push(AutoActivationPause::default()),
                    cx,
                )
            })
        }));
        body.into_any_element()
    }

    fn schedule_rule(
        &self,
        index: usize,
        rule: &ScheduledActivation,
        pause: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut row = card(cx).child(
            div()
                .flex()
                .justify_between()
                .child(if pause { "Quiet period" } else { "Schedule" })
                .child({
                    let model = self.model.clone();
                    switch(format!("{}-enabled-{index}", if pause { "pause" } else { "sched" }))
                        .checked(rule.enabled)
                        .on_click(move |value, _, cx| {
                            model.update(cx, |model, cx| {
                                model.edit(
                                    |settings| {
                                        if pause {
                                            if let Some(rule) =
                                                settings.auto_activation_pauses.get_mut(index)
                                            {
                                                rule.enabled = *value;
                                            }
                                        } else if let Some(rule) =
                                            settings.scheduled_activations.get_mut(index)
                                        {
                                            rule.enabled = *value;
                                        }
                                    },
                                    cx,
                                )
                            })
                        })
                }),
        );
        let mut providers = div().flex().flex_wrap().gap_2();
        for provider in ProviderKind::ALL {
            let model = self.model.clone();
            let mut choice = button(format!(
                "{}-prov-{index}-{}",
                if pause { "pause" } else { "sched" },
                provider.id()
            ))
            .label(provider.display_name());
            if rule.provider() == Some(provider) {
                choice = choice.primary();
            }
            providers = providers.child(choice.on_click(move |_, _, cx| {
                model.update(cx, |model, cx| {
                    model.edit(
                        |settings| {
                            if pause {
                                if let Some(rule) = settings.auto_activation_pauses.get_mut(index) {
                                    rule.provider_id = provider.id().into();
                                }
                            } else if let Some(rule) = settings.scheduled_activations.get_mut(index)
                            {
                                rule.provider_id = provider.id().into();
                            }
                        },
                        cx,
                    )
                })
            }));
        }
        row = row.child(providers);
        let mut days = div().flex().flex_wrap().gap_1();
        for (day, label) in WEEKDAYS.iter().enumerate() {
            let day = day as u8;
            let selected = rule.weekdays.contains(&day);
            let model = self.model.clone();
            days = days.child(
                Checkbox::new(SharedString::from(format!(
                    "{}-day-{index}-{day}",
                    if pause { "pause" } else { "sched" }
                )))
                .label(*label)
                .checked(selected)
                .on_click(move |value, _, cx| {
                    model.update(cx, |model, cx| {
                        model.edit(
                            |settings| {
                                let weekdays = if pause {
                                    settings
                                        .auto_activation_pauses
                                        .get_mut(index)
                                        .map(|rule| &mut rule.weekdays)
                                } else {
                                    settings
                                        .scheduled_activations
                                        .get_mut(index)
                                        .map(|rule| &mut rule.weekdays)
                                };
                                if let Some(weekdays) = weekdays {
                                    if *value {
                                        if !weekdays.contains(&day) {
                                            weekdays.push(day);
                                        }
                                    } else {
                                        weekdays.retain(|item| *item != day);
                                    }
                                }
                            },
                            cx,
                        )
                    })
                }),
            );
        }
        row = row.child(days);
        let time_path = format!("/{}s/{index}/time_minutes", if pause { "pause" } else { "scheduled_activation" });
        let _ = (window, time_path);
        let clock = format_clock(rule.time_minutes);
        row = row.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child("Time")
                .child(button(format!("{}-time-{index}", if pause { "pause" } else { "sched" })).label(clock).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.expanded = Some(format!("clock-{}-{index}", if pause { "p" } else { "s" }));
                        cx.notify();
                    }),
                )),
        );
        if self.expanded.as_deref() == Some(&format!("clock-{}-{index}", if pause { "p" } else { "s" })) {
            let mut hours = div().flex().flex_wrap().gap_1();
            for hour in 0..24u16 {
                let model = self.model.clone();
                hours = hours.child(button(format!("h-{pause}-{index}-{hour}")).label(format!("{hour:02}:00")).on_click(
                    move |_, _, cx| {
                        model.update(cx, |model, cx| {
                            model.edit(
                                |settings| {
                                    if pause {
                                        if let Some(rule) = settings.auto_activation_pauses.get_mut(index)
                                        {
                                            rule.start_time_minutes = hour * 60;
                                        }
                                    } else if let Some(rule) =
                                        settings.scheduled_activations.get_mut(index)
                                    {
                                        rule.time_minutes = hour * 60;
                                    }
                                },
                                cx,
                            )
                        });
                    },
                ));
            }
            row = row.child(hours);
        }
        let model = self.model.clone();
        row = row.child(button(format!("{}-remove-{index}", if pause { "pause" } else { "sched" })).label("Remove").on_click(
            move |_, _, cx| {
                model.update(cx, |model, cx| {
                    model.edit(
                        |settings| {
                            if pause {
                                if index < settings.auto_activation_pauses.len() {
                                    settings.auto_activation_pauses.remove(index);
                                }
                            } else if index < settings.scheduled_activations.len() {
                                settings.scheduled_activations.remove(index);
                            }
                        },
                        cx,
                    )
                })
            },
        ));
        row.into_any_element()
    }

    fn tray_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let settings = self.model.read(cx).snapshot.settings.clone();
        let mut body = div().flex().flex_col().gap_3();
        if settings.tray_widgets.is_empty() {
            body = body.child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child("No widgets yet. The app icon stays in the tray until you add one."),
            );
        }
        for (index, widget) in settings.tray_widgets.iter().enumerate() {
            let title = match widget.kind {
                TrayWidgetKind::AppIcon => "App icon".to_string(),
                TrayWidgetKind::Limits => widget
                    .indicators
                    .first()
                    .and_then(|indicator| indicator.provider())
                    .map(ProviderKind::display_name)
                    .unwrap_or("Limits")
                    .to_string(),
            };
            let mut card_ui = card(cx).child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child(title))
                    .child({
                        let model = self.model.clone();
                        button(format!("tray-up-{index}")).label("Up").on_click(move |_, _, cx| {
                            if index == 0 {
                                return;
                            }
                            model.update(cx, |model, cx| {
                                model.edit(|settings| settings.tray_widgets.swap(index, index - 1), cx)
                            });
                        })
                    })
                    .child({
                        let model = self.model.clone();
                        button(format!("tray-down-{index}")).label("Down").on_click(move |_, _, cx| {
                            model.update(cx, |model, cx| {
                                model.edit(
                                    |settings| {
                                        if index + 1 < settings.tray_widgets.len() {
                                            settings.tray_widgets.swap(index, index + 1);
                                        }
                                    },
                                    cx,
                                )
                            });
                        })
                    })
                    .child({
                        let model = self.model.clone();
                        button(format!("tray-remove-{index}")).label("Remove").on_click(
                            move |_, _, cx| {
                                model.update(cx, |model, cx| {
                                    model.edit(
                                        |settings| {
                                            if index < settings.tray_widgets.len() {
                                                settings.tray_widgets.remove(index);
                                            }
                                        },
                                        cx,
                                    )
                                })
                            },
                        )
                    }),
            );
            if widget.kind == TrayWidgetKind::Limits {
                card_ui = card_ui.child(enum_row(
                    self,
                    cx,
                    &format!("tray-pres-{index}"),
                    "Presentation",
                    widget.presentation_id(),
                    &[
                        ("stacked_numbers", "Stacked numbers"),
                        ("stacked_bars", "Stacked bars"),
                        ("nested_rings", "Nested rings"),
                        ("reset_time", "Reset time"),
                        ("reset_countdown", "Reset countdown"),
                    ],
                    move |settings, value| {
                        if let Some(widget) = settings.tray_widgets.get_mut(index) {
                            widget.presentation = presentation_from_id(value);
                        }
                    },
                ));
                for (indicator_index, indicator) in widget.indicators.iter().enumerate() {
                    let provider = indicator.provider().unwrap_or(ProviderKind::Codex);
                    let mut providers = div().flex().flex_wrap().gap_1();
                    for candidate in ProviderKind::ALL {
                        let model = self.model.clone();
                        let mut choice = button(format!(
                            "ind-p-{index}-{indicator_index}-{}",
                            candidate.id()
                        ))
                        .label(candidate.display_name());
                        if candidate == provider {
                            choice = choice.primary();
                        }
                        providers = providers.child(choice.on_click(move |_, _, cx| {
                            model.update(cx, |model, cx| {
                                model.edit(
                                    |settings| {
                                        if let Some(indicator) = settings
                                            .tray_widgets
                                            .get_mut(index)
                                            .and_then(|widget| {
                                                widget.indicators.get_mut(indicator_index)
                                            })
                                        {
                                            indicator.provider_id = candidate.id().into();
                                            if let Some(metric) =
                                                crate::provider_registry::descriptor(candidate)
                                                    .default_tray_metrics
                                                    .first()
                                            {
                                                indicator.metric_id = (*metric).into();
                                            }
                                        }
                                    },
                                    cx,
                                )
                            })
                        }));
                    }
                    card_ui = card_ui.child(providers);
                    let mut metrics = div().flex().flex_wrap().gap_1();
                    for metric in crate::provider_registry::descriptor(provider).metrics {
                        let model = self.model.clone();
                        let mut choice = button(format!(
                            "ind-m-{index}-{indicator_index}-{}",
                            metric.id
                        ))
                        .label(metric.label);
                        if indicator.metric_id == metric.id {
                            choice = choice.primary();
                        }
                        metrics = metrics.child(choice.on_click(move |_, _, cx| {
                            model.update(cx, |model, cx| {
                                model.edit(
                                    |settings| {
                                        if let Some(indicator) = settings
                                            .tray_widgets
                                            .get_mut(index)
                                            .and_then(|widget| {
                                                widget.indicators.get_mut(indicator_index)
                                            })
                                        {
                                            indicator.metric_id = metric.id.into();
                                        }
                                    },
                                    cx,
                                )
                            })
                        }));
                    }
                    card_ui = card_ui.child(metrics);
                }
                let model = self.model.clone();
                card_ui = card_ui.child(
                    button(format!("add-ind-{index}"))
                        .label("Add indicator")
                        .on_click(move |_, _, cx| {
                            model.update(cx, |model, cx| {
                                model.edit(
                                    |settings| {
                                        if let Some(widget) = settings.tray_widgets.get_mut(index) {
                                            widget.indicators.push(TrayIndicator::new(
                                                ProviderKind::Codex,
                                                "codex.session",
                                            ));
                                        }
                                    },
                                    cx,
                                )
                            })
                        }),
                );
            }
            body = body.child(card_ui);
        }
        let model_limits = self.model.clone();
        let model_icon = self.model.clone();
        body = body.child(
            div()
                .flex()
                .gap_2()
                .child(button("add-tray-limits").label("Add limits widget").on_click(
                    move |_, _, cx| {
                        model_limits.update(cx, |model, cx| {
                            model.edit(
                                |settings| {
                                    settings
                                        .tray_widgets
                                        .push(TrayWidget::default_user_widget())
                                },
                                cx,
                            )
                        })
                    },
                ))
                .child(button("add-tray-icon").label("Add app icon").on_click(
                    move |_, _, cx| {
                        model_icon.update(cx, |model, cx| {
                            model.edit(
                                |settings| settings.tray_widgets.push(TrayWidget::app_icon()),
                                cx,
                            )
                        })
                    },
                )),
        );
        body.into_any_element()
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings = self.model.read(cx).snapshot.settings.clone();
        let mut sidebar = div()
            .w(px(196.))
            .flex_none()
            .p_2()
            .flex()
            .flex_col()
            .gap_1()
            .border_r_1()
            .border_color(cx.theme().border);
        for &(id, label, icon) in PAGES {
            let selected = self.page == id;
            sidebar = sidebar.child(
                div()
                    .id(id)
                    .p_2()
                    .rounded_md()
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .gap_2()
                    .bg(if selected {
                        cx.theme().secondary
                    } else {
                        cx.theme().background
                    })
                    .child(
                        svg()
                            .path(icon)
                            .size(px(16.))
                            .text_color(if selected {
                                cx.theme().primary
                            } else {
                                cx.theme().muted_foreground
                            }),
                    )
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.page = id;
                        this.expanded = None;
                        this.inputs.clear();
                        this.subscriptions.truncate(1);
                        cx.notify();
                    })),
            );
        }
        let title = if !settings.onboarding_completed {
            "Welcome to Codex Minibar"
        } else {
            PAGES.iter().find(|page| page.0 == self.page).unwrap().1
        };
        let mut content = div()
            .flex()
            .flex_col()
            .gap_3()
            .p_5()
            .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child(title));
        if let Some(error) = self.error.as_ref().or(self.model.read(cx).snapshot.error.as_ref()) {
            content = content.child(div().text_color(rgb(0xe16d73)).child(error.clone()));
        }
        match self.page {
            "providers" => content = content.child(self.providers(window, cx)),
            "popup" => content = content.child(self.popup_page(cx)),
            "schedule" => content = content.child(self.schedule_page(window, cx)),
            "tray" => content = content.child(self.tray_page(cx)),
            "log" => {
                content = content
                    .child(button("open-log").label("Open log").on_click(|_, _, _| {
                        let _ = crate::logger::open();
                    }))
                    .child(
                        div()
                            .text_size(px(11.))
                            .child(crate::logger::tail_lines(150).unwrap_or_default()),
                    );
            }
            "about" => {
                let runtime = self.model.read(cx).runtime.clone();
                content = content
                    .child(
                        card(cx)
                            .child(format!("Codex Minibar {}", env!("CARGO_PKG_VERSION")))
                            .child("AI usage in your system tray.")
                            .child("Built with GPUI"),
                    )
                    .child(button("check-update").label("Check for updates").on_click(
                        move |_, _, _| {
                            runtime.state.updates.check_async(false, false);
                        },
                    ))
                    .child(button("github").label("GitHub").on_click(|_, _, cx| {
                        cx.open_url(crate::updater::REPO_URL);
                    }));
                let model = self.model.clone();
                content = content.child(toggle_card(
                    cx,
                    "check_for_updates",
                    "Check for updates automatically",
                    settings.check_for_updates,
                    move |value, cx| {
                        model.update(cx, |model, cx| {
                            model.edit(|settings| settings.check_for_updates = value, cx)
                        })
                    },
                ));
            }
            "advanced" => {
                let document = serde_json::to_value(&settings).unwrap();
                if let Some(value) = document.get("history_retention_days") {
                    content = content.child(self.field(
                        "/history_retention_days".into(),
                        "History retention (days)".into(),
                        value,
                        window,
                        cx,
                    ));
                }
                content = content
                    .child(button("export-settings").label("Copy settings").on_click({
                        let model = self.model.clone();
                        move |_, _, cx| {
                            if let Ok(text) = toml::to_string_pretty(&model.read(cx).snapshot.settings)
                            {
                                cx.write_to_clipboard(ClipboardItem::new_string(text));
                            }
                        }
                    }))
                    .child(button("import-settings").label("Import settings from clipboard").on_click(
                        cx.listener(|this, _, _, cx| {
                            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                                match toml::from_str::<Settings>(&text) {
                                    Ok(next) => this.model.update(cx, |model, cx| {
                                        model.edit(|settings| *settings = next, cx)
                                    }),
                                    Err(error) => {
                                        this.error = Some(error.to_string());
                                        cx.notify();
                                    }
                                }
                            }
                        }),
                    ))
                    .child(button("clear-usage").label("Clear usage data").on_click(
                        cx.listener(|this, _, _, cx| {
                            this.confirm = Some("clear");
                            cx.notify();
                        }),
                    ))
                    .child(button("reset-settings").label("Reset settings").on_click(
                        cx.listener(|this, _, _, cx| {
                            this.confirm = Some("reset");
                            cx.notify();
                        }),
                    ));
                if let Some(action) = self.confirm {
                    content = content.child(
                        card(cx)
                            .child("This action cannot be undone.")
                            .child(button("confirm-action").primary().label("Confirm").on_click(
                                cx.listener(move |this, _, _, cx| {
                                    if action == "clear" {
                                        this.model.read(cx).runtime.send(Command::ClearUsage);
                                    } else {
                                        this.model.update(cx, |model, cx| {
                                            model.edit(
                                                |settings| {
                                                    *settings = Settings {
                                                        onboarding_completed: true,
                                                        ..Default::default()
                                                    }
                                                },
                                                cx,
                                            )
                                        });
                                    }
                                    this.confirm = None;
                                    cx.notify();
                                }),
                            ))
                            .child(button("cancel-action").label("Cancel").on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.confirm = None;
                                    cx.notify();
                                },
                            ))),
                    );
                }
            }
            page => {
                let fields: &[&str] = match page {
                    "general" => &[
                        "start_at_login",
                        "usage_stats_enabled",
                        "limit_refresh_interval",
                        "usage_refresh_interval",
                    ],
                    "appearance" => &[
                        "theme",
                        "accent_color",
                        "popup_background_material",
                        "popup_corner_radius",
                        "bottom_bar_size",
                        "animations_enabled",
                        "use_colored_provider_icons",
                        "use_colored_sidebar_icons",
                        "replace_chatgpt_logo_with_codex",
                        "time_format",
                    ],
                    "notifications" => &["notifications"],
                    _ => &[],
                };
                let document = serde_json::to_value(&settings).unwrap();
                for &key in fields {
                    if let Some(value) = document.get(key) {
                        content = content.child(self.field(
                            format!("/{key}"),
                            human(key),
                            value,
                            window,
                            cx,
                        ));
                    }
                }
            }
        }
        if !settings.onboarding_completed {
            let model = self.model.clone();
            content = content.child(button("done").primary().label("Done").on_click(
                move |_, window, cx| {
                    model.update(cx, |model, cx| {
                        model.edit(|settings| settings.onboarding_completed = true, cx)
                    });
                    window.remove_window();
                },
            ));
        }
        div()
            .size_full()
            .flex()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_size(px(13.))
            .child(sidebar)
            .child(
                div()
                    .id(SharedString::from(format!("settings-scroll-{}", self.page)))
                    .flex_1()
                    .min_w_0()
                    .overflow_y_scroll()
                    .child(content),
            )
    }
}

fn toggle_card(
    cx: &App,
    id: &str,
    label: &str,
    checked: bool,
    on_change: impl Fn(bool, &mut App) + 'static,
) -> Div {
    card(cx).child(
        div()
            .flex()
            .justify_between()
            .items_center()
            .gap_3()
            .child(div().flex_1().child(label.to_string()))
            .child(
                switch(id.to_string())
                    .checked(checked)
                    .on_click(move |value, _, cx| on_change(*value, cx)),
            ),
    )
}

fn enum_row(
    view: &SettingsView,
    cx: &mut Context<SettingsView>,
    id: &str,
    label: &str,
    current: &str,
    options: &'static [(&'static str, &'static str)],
    apply: impl Fn(&mut Settings, &str) + Clone + 'static,
) -> Div {
    let mut row = card(cx).child(
        div()
            .flex()
            .justify_between()
            .items_center()
            .child(label.to_string())
            .child({
                let current_label = options
                    .iter()
                    .find(|(value, _)| *value == current)
                    .map(|(_, label)| *label)
                    .unwrap_or(current);
                let expand = id.to_string();
                button(id.to_string()).label(current_label.to_string()).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.expanded = if this.expanded.as_ref() == Some(&expand) {
                            None
                        } else {
                            Some(expand.clone())
                        };
                        cx.notify();
                    },
                ))
            }),
    );
    if view.expanded.as_deref() == Some(id) {
        let mut choices = div().flex().flex_wrap().gap_2();
        for (value, label) in options {
            let apply = apply.clone();
            let model = view.model.clone();
            let value = *value;
            choices = choices.child(button(format!("{id}-{value}")).label(*label).on_click(
                move |_, _, cx| {
                    model.update(cx, |model, cx| {
                        model.edit(|settings| apply(settings, value), cx)
                    });
                },
            ));
        }
        row = row.child(choices);
    }
    row
}

fn pause_as_schedule(pause: &AutoActivationPause) -> ScheduledActivation {
    ScheduledActivation {
        id: pause.id.clone(),
        provider_id: pause.provider_id.clone(),
        weekday: pause.weekdays.first().copied().unwrap_or(0),
        weekdays: pause.weekdays.clone(),
        time_minutes: pause.start_time_minutes,
        enabled: pause.enabled,
    }
}

fn format_clock(minutes: u16) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

fn presentation_from_id(value: &str) -> TrayPresentation {
    match value {
        "stacked_bars" => TrayPresentation::StackedBars,
        "nested_rings" => TrayPresentation::NestedRings,
        "reset_time" => TrayPresentation::ResetTime,
        "reset_countdown" => TrayPresentation::ResetCountdown,
        _ => TrayPresentation::StackedNumbers,
    }
}

trait PresentationId {
    fn presentation_id(&self) -> &'static str;
    fn id(&self) -> &'static str;
}

impl PresentationId for TrayWidget {
    fn presentation_id(&self) -> &'static str {
        match self.presentation {
            TrayPresentation::StackedNumbers | TrayPresentation::Number => "stacked_numbers",
            TrayPresentation::StackedBars | TrayPresentation::Bar => "stacked_bars",
            TrayPresentation::NestedRings | TrayPresentation::Ring => "nested_rings",
            TrayPresentation::ResetTime => "reset_time",
            TrayPresentation::ResetCountdown => "reset_countdown",
        }
    }

    fn id(&self) -> &'static str {
        self.presentation_id()
    }
}

impl PresentationId for TotalSpendPresentation {
    fn presentation_id(&self) -> &'static str {
        self.id()
    }

    fn id(&self) -> &'static str {
        match self {
            Self::Donut => "donut",
            Self::ProgressBar => "progress_bar",
        }
    }
}

fn escape(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn human(key: &str) -> String {
    match key {
        "start_at_login" => "Start with Windows".into(),
        "usage_stats_enabled" => "Enable Usage Stats".into(),
        "limit_refresh_interval" => "Refresh limits".into(),
        "usage_refresh_interval" => "Refresh usage".into(),
        "theme" => "Color theme".into(),
        "accent_color" => "Accent color".into(),
        "popup_background_material" => "Popup material".into(),
        "popup_corner_radius" => "Popup corner radius".into(),
        "bottom_bar_size" => "Footer size".into(),
        "animations_enabled" => "Animations".into(),
        "use_colored_provider_icons" => "Colored provider icons".into(),
        "use_colored_sidebar_icons" => "Colored sidebar icons".into(),
        "replace_chatgpt_logo_with_codex" => "Replace ChatGPT logo with Codex".into(),
        "time_format" => "Time format".into(),
        "hour_12" => "12-hour clock".into(),
        "hour_24" => "24-hour clock".into(),
        "minute1" => "1 minute".into(),
        "seconds30" => "30 seconds".into(),
        "minutes5" => "5 minutes".into(),
        "minutes10" => "10 minutes".into(),
        "minutes15" => "15 minutes".into(),
        "minutes30" => "30 minutes".into(),
        "minutes45" => "45 minutes".into(),
        "minutes60" => "60 minutes".into(),
        "auto" => "Windows".into(),
        "windows" => "Windows".into(),
        "acrylic" => "Acrylic".into(),
        "mica" => "Mica".into(),
        "comfortable" => "Comfortable".into(),
        "compact" => "Compact".into(),
        "activation_success" => "Activation succeeded".into(),
        "activation_failure" => "Activation failed".into(),
        "codex_unavailable" => "Provider unavailable".into(),
        "approaching_reset" => "Approaching reset".into(),
        "limits_changed" => "Limit reset".into(),
        "low_usage_enabled" => "Low session usage".into(),
        "low_usage_threshold_percent" => "Session threshold (%)".into(),
        "weekly_low_usage_enabled" => "Low weekly usage".into(),
        "weekly_low_usage_threshold_percent" => "Weekly threshold (%)".into(),
        "update_available" => "Update available".into(),
        "notifications" => "Notifications".into(),
        _ => {
            let text = key.replace('_', " ");
            let mut chars = text.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        }
    }
}

fn options(key: &str) -> Vec<&'static str> {
    match key {
        "theme" => vec!["auto", "light", "dark"],
        "accent_color" => vec![
            "windows", "blue", "purple", "pink", "red", "orange", "green", "teal",
        ],
        "popup_background_material" => vec!["acrylic", "mica"],
        "popup_corner_radius" => {
            vec!["zero", "four", "small", "medium", "large", "extra_large"]
        }
        "bottom_bar_size" => vec!["comfortable", "compact"],
        "time_format" => vec!["hour_12", "hour_24"],
        "limit_refresh_interval" => {
            vec!["seconds30", "minute1", "minutes5", "minutes10", "minutes15"]
        }
        "usage_refresh_interval" => vec![
            "minute1",
            "minutes5",
            "minutes10",
            "minutes15",
            "minutes30",
            "minutes45",
            "minutes60",
        ],
        _ => vec![],
    }
}

fn button(id: impl Into<SharedString>) -> Button {
    Button::new(id.into())
}

fn switch(id: impl Into<SharedString>) -> Switch {
    Switch::new(id.into())
}
