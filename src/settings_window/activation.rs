use super::persistence::{persist_bool, persist_update};
use super::*;

pub(super) fn render(ctx: &SettingsPageContext<'_>) -> (&'static str, Vec<Element>) {
    let codex_enabled = ctx.codex_enabled;
    let claude_enabled = ctx.claude_enabled;
    let cursor_enabled = ctx.cursor_enabled;
    let opencode_zen_enabled = ctx.opencode_zen_enabled;
    let opencode_go_enabled = ctx.opencode_go_enabled;
    let openrouter_enabled = ctx.openrouter_enabled;
    let automatic_activation = ctx.automatic_activation;
    let scheduled_activations = ctx.scheduled_activations;
    let auto_activation_pauses = ctx.auto_activation_pauses;
    let expanded_scheduled_activation = ctx.expanded_scheduled_activation;
    let expanded_auto_activation_pause = ctx.expanded_auto_activation_pause;
    let time_format = ctx.time_format;
    let set_automatic_activation = ctx.set_automatic_activation.clone();
    let set_scheduled_activations = ctx.set_scheduled_activations.clone();
    let set_auto_activation_pauses = ctx.set_auto_activation_pauses.clone();
    let set_expanded_scheduled_activation = ctx.set_expanded_scheduled_activation.clone();
    let set_expanded_auto_activation_pause = ctx.set_expanded_auto_activation_pause.clone();
    let hovered_card_id = ctx.hovered_card_id;
    let set_hovered_card_id = ctx.set_hovered_card_id.clone();
    let settings_tx = ctx.settings_tx.clone();
    let apply_automatic_activation = settings_tx.clone();
    let provider_enabled = [
        codex_enabled,
        claude_enabled,
        cursor_enabled,
        opencode_zen_enabled,
        opencode_go_enabled,
        openrouter_enabled,
    ];
    let default_provider = activation_providers(&provider_enabled).into_iter().next();
    let mut rows = vec![settings_toggle_card_with_description(
                "Start 5-hour sessions automatically",
                Some("Starts a new Codex or Claude session as soon as a window is available, instead of waiting for your first request."),
                automatic_activation,
                move |value| {
                    persist_bool(
                        set_automatic_activation.clone(),
                        apply_automatic_activation.clone(),
                        value,
                        |settings, value| {
                            settings.automatic_activation = value;
                        },
                    );
                },
                "activation-automatic",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("activation-automatic")];

    let existing_pauses = auto_activation_pauses.to_vec();
    let pause_setter = set_auto_activation_pauses.clone();
    let pause_tx = settings_tx.clone();
    let expand_added_pause = set_expanded_auto_activation_pause.clone();
    rows.push(activation_section_header(
        "Quiet periods",
        "Don't auto-start sessions during these times.",
        default_provider.is_some(),
        move || {
            let Some(provider) = default_provider else {
                return;
            };
            let mut next = existing_pauses.clone();
            let pause = AutoActivationPause::new(provider);
            let id = pause.id.clone();
            next.push(pause);
            persist_auto_activation_pauses(pause_setter.clone(), pause_tx.clone(), next);
            expand_added_pause.call(Some(id));
        },
        "activation-pauses-heading-row",
    ));
    rows.extend(auto_activation_pause_cards(
        auto_activation_pauses,
        &provider_enabled,
        time_format,
        expanded_auto_activation_pause,
        set_expanded_auto_activation_pause.clone(),
        set_auto_activation_pauses,
        hovered_card_id,
        set_hovered_card_id.clone(),
        settings_tx.clone(),
    ));

    let existing = scheduled_activations.to_vec();
    let schedule_setter = set_scheduled_activations.clone();
    let schedule_tx = settings_tx.clone();
    let expand_added_schedule = set_expanded_scheduled_activation.clone();
    rows.push(activation_section_header(
        "Scheduled activations",
        "Start a 5-hour session at a set time.",
        default_provider.is_some(),
        move || {
            let Some(provider) = default_provider else {
                return;
            };
            let mut next = existing.clone();
            let schedule = ScheduledActivation::new(provider);
            let id = schedule.id.clone();
            next.push(schedule);
            persist_schedules(schedule_setter.clone(), schedule_tx.clone(), next);
            expand_added_schedule.call(Some(id));
        },
        "activation-scheduled-heading-row",
    ));
    rows.extend(scheduled_activation_cards(
        scheduled_activations,
        &provider_enabled,
        time_format,
        expanded_scheduled_activation,
        set_expanded_scheduled_activation,
        set_scheduled_activations.clone(),
        hovered_card_id,
        set_hovered_card_id.clone(),
        settings_tx.clone(),
    ));
    ("Limit activation", rows)
}

const ACTIVATION_WEEKDAY_LABELS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const ACTIVATION_TIME_PICKER_HEIGHT: f64 = 36.0;

fn activation_time_segment(
    label: String,
    choices: Vec<String>,
    on_selected: impl Fn(String) + Clone + 'static,
) -> Element {
    Button::new(label)
        .subtle()
        .min_width(0.0)
        .height(ACTIVATION_TIME_PICKER_HEIGHT)
        .padding(Thickness::uniform(0.0))
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .menu_flyout(choices.into_iter().map(menu_item).collect())
        .on_item_clicked(on_selected)
        .into()
}

fn activation_time_field(
    label: &'static str,
    minutes: u16,
    time_format: TimeFormat,
    on_changed: impl Fn(u16) + Clone + 'static,
) -> Element {
    let minutes = minutes.min(23 * 60 + 59);
    let hour = minutes / 60;
    let minute = minutes % 60;
    let mut segments = Vec::<Element>::new();

    let hour_label = match time_format {
        TimeFormat::Hour24 => format!("{hour:02}"),
        TimeFormat::Hour12 => format!(
            "{}",
            match hour % 12 {
                0 => 12,
                value => value,
            }
        ),
    };
    let hour_values = match time_format {
        TimeFormat::Hour24 => (0..24).map(|value| format!("{value:02}")).collect(),
        TimeFormat::Hour12 => (1..=12).map(|value| value.to_string()).collect(),
    };
    let hour_changed = on_changed.clone();
    segments.push(
        activation_time_segment(hour_label, hour_values, move |value| {
            let Ok(value) = value.parse::<u16>() else {
                return;
            };
            let hour = match time_format {
                TimeFormat::Hour24 => value.min(23),
                TimeFormat::Hour12 => {
                    let hour12 = value % 12;
                    if hour >= 12 { hour12 + 12 } else { hour12 }
                }
            };
            hour_changed(hour * 60 + minute);
        })
        .grid_column(0),
    );

    segments.push(
        border(Element::Empty)
            .width(1.0)
            .background(ThemeRef::ControlStroke)
            .grid_column(1)
            .into(),
    );

    let minute_changed = on_changed.clone();
    segments.push(
        activation_time_segment(
            format!("{minute:02}"),
            (0..12).map(|value| format!("{:02}", value * 5)).collect(),
            move |value| {
                let Ok(value) = value.parse::<u16>() else {
                    return;
                };
                minute_changed(hour * 60 + value.min(59));
            },
        )
        .grid_column(2),
    );

    let columns = if time_format == TimeFormat::Hour12 {
        segments.push(
            border(Element::Empty)
                .width(1.0)
                .background(ThemeRef::ControlStroke)
                .grid_column(3)
                .into(),
        );
        let period_changed = on_changed;
        segments.push(
            activation_time_segment(
                if hour >= 12 { "PM".into() } else { "AM".into() },
                vec!["AM".into(), "PM".into()],
                move |value| {
                    let hour12 = match hour % 12 {
                        0 => 12,
                        value => value,
                    };
                    let hour = if value == "PM" {
                        hour12 % 12 + 12
                    } else {
                        hour12 % 12
                    };
                    period_changed(hour * 60 + minute);
                },
            )
            .grid_column(4),
        );
        vec![
            GridLength::Star(1.0),
            GridLength::Pixel(1.0),
            GridLength::Star(1.0),
            GridLength::Pixel(1.0),
            GridLength::Star(1.0),
        ]
    } else {
        vec![
            GridLength::Star(1.0),
            GridLength::Pixel(1.0),
            GridLength::Star(1.0),
        ]
    };

    vstack((
        text_block(label)
            .font_size(12.0)
            .foreground(ThemeRef::SecondaryText),
        border(
            grid(segments)
                .columns(columns)
                .rows([GridLength::Pixel(ACTIVATION_TIME_PICKER_HEIGHT)])
                .horizontal_alignment(HorizontalAlignment::Stretch),
        )
        .background(ThemeRef::ControlFill)
        .border_thickness(Thickness::uniform(1.0))
        .border_brush(ThemeRef::ControlStroke)
        .corner_radius(4.0)
        .height(ACTIVATION_TIME_PICKER_HEIGHT)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    ))
    .spacing(4.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn activation_section_header(
    title: &'static str,
    description: &'static str,
    enabled: bool,
    on_click: impl IntoUnitCallback,
    key: &'static str,
) -> Element {
    grid((
        vstack((
            text_block(title).font_size(16.0).semibold(),
            text_block(description)
                .font_size(12.0)
                .foreground(ThemeRef::SecondaryText)
                .wrap(),
        ))
        .spacing(2.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Center)
        .grid_column(0),
        Button::new("Add")
            .icon(Symbol::Add)
            .enabled(enabled)
            .on_click(on_click)
            .grid_column(1)
            .vertical_alignment(VerticalAlignment::Center),
    ))
    .columns([GridLength::Star(1.0), GridLength::Auto])
    .rows([GridLength::Auto])
    .margin(Thickness {
        left: 0.0,
        top: 16.0,
        right: 0.0,
        bottom: 8.0,
    })
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .with_key(key)
    .into()
}

fn activation_providers(provider_enabled: &[bool; 6]) -> Vec<ProviderKind> {
    ProviderKind::ALL
        .into_iter()
        .enumerate()
        .filter(|(index, provider)| {
            provider_enabled[*index]
                && crate::provider_registry::descriptor(*provider).supports_activation
        })
        .map(|(_, provider)| provider)
        .collect()
}

fn activation_provider_choices(
    provider_enabled: &[bool; 6],
    current: Option<ProviderKind>,
) -> Vec<ProviderKind> {
    ProviderKind::ALL
        .into_iter()
        .enumerate()
        .filter(|(index, provider)| {
            crate::provider_registry::descriptor(*provider).supports_activation
                && (provider_enabled[*index] || current == Some(*provider))
        })
        .map(|(_, provider)| provider)
        .collect()
}

fn activation_weekdays_summary(weekdays: &[u8]) -> String {
    let mut weekdays = weekdays
        .iter()
        .copied()
        .filter(|weekday| *weekday <= 6)
        .collect::<Vec<_>>();
    weekdays.sort_unstable();
    weekdays.dedup();
    match weekdays.as_slice() {
        [0, 1, 2, 3, 4, 5, 6] => "Every day".into(),
        [0, 1, 2, 3, 4] => "Weekdays".into(),
        [5, 6] => "Weekends".into(),
        [] => "No days".into(),
        days => days
            .iter()
            .map(|day| ACTIVATION_WEEKDAY_LABELS[*day as usize])
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn activation_time_label(time_format: TimeFormat, minutes: u16) -> String {
    let minutes = minutes.min(23 * 60 + 59);
    let hour = minutes / 60;
    let minute = minutes % 60;
    match time_format {
        TimeFormat::Hour24 => format!("{hour:02}:{minute:02}"),
        TimeFormat::Hour12 => {
            let suffix = if hour < 12 { "AM" } else { "PM" };
            let hour = match hour % 12 {
                0 => 12,
                value => value,
            };
            format!("{hour}:{minute:02} {suffix}")
        }
    }
}

fn activation_rule_header(provider: Option<ProviderKind>, summary: String) -> Element {
    vstack((
        text_block(
            provider
                .map(ProviderKind::display_name)
                .unwrap_or("Unknown provider"),
        )
        .font_size(14.0),
        text_block(summary)
            .font_size(12.0)
            .foreground(ThemeRef::SecondaryText)
            .wrap(),
    ))
    .spacing(2.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn activation_rule_toggle(enabled: bool, on_toggled: impl IntoCallback<bool>) -> Element {
    ToggleSwitch::new(enabled)
        .on_content("")
        .off_content("")
        .on_toggled(on_toggled)
        .min_width(0.0)
        .max_width(50.0)
        .width(50.0)
        .into()
}

fn activation_weekday_selector(
    selected: &[u8],
    key_prefix: &str,
    on_checked: impl Fn(u8, bool) + Clone + 'static,
) -> Element {
    let buttons = ACTIVATION_WEEKDAY_LABELS
        .iter()
        .enumerate()
        .map(|(weekday, label)| {
            let on_checked = on_checked.clone();
            ToggleButton::new(*label, selected.contains(&(weekday as u8)))
                .on_checked(move |checked| on_checked(weekday as u8, checked))
                .grid_column(weekday as i32)
                .min_width(0.0)
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .with_key(format!("{key_prefix}-{weekday}"))
                .into()
        })
        .collect::<Vec<Element>>();
    grid(buttons)
        .columns(vec![GridLength::Star(1.0); 7])
        .rows([GridLength::Auto])
        .column_spacing(4.0)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .into()
}

fn activation_days_field(selector: Element) -> Element {
    vstack((
        text_block("Days")
            .font_size(12.0)
            .foreground(ThemeRef::SecondaryText),
        selector,
    ))
    .spacing(4.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

fn set_activation_weekday(weekdays: &mut Vec<u8>, day: u8, checked: bool) -> bool {
    if checked {
        if weekdays.contains(&day) {
            return false;
        }
        weekdays.push(day);
        weekdays.sort_unstable();
        true
    } else {
        if weekdays.len() == 1 && weekdays[0] == day {
            return false;
        }
        let previous_len = weekdays.len();
        weekdays.retain(|candidate| *candidate != day);
        weekdays.len() != previous_len
    }
}

fn scheduled_activation_cards(
    schedules: &[ScheduledActivation],
    provider_enabled: &[bool; 6],
    time_format: TimeFormat,
    expanded_schedule: &Option<String>,
    set_expanded_schedule: SetState<Option<String>>,
    set_schedules: SetState<Vec<ScheduledActivation>>,
    hovered_card_id: &Option<String>,
    set_hovered_card_id: SetState<Option<String>>,
    settings_tx: Sender<Settings>,
) -> Vec<Element> {
    if schedules.is_empty() {
        return vec![
            text_block(if activation_providers(provider_enabled).is_empty() {
                "Turn on Codex or Claude in Providers first."
            } else {
                "No scheduled activations."
            })
            .font_size(12.0)
            .foreground(ThemeRef::SecondaryText)
            .wrap()
            .into(),
        ];
    }

    let mut rows = Vec::with_capacity(schedules.len());
    for schedule in schedules {
        let schedule_id = schedule.id.clone();
        let choices = activation_provider_choices(provider_enabled, schedule.provider());
        let provider_labels = choices
            .iter()
            .map(|provider| provider.display_name().to_string())
            .collect::<Vec<_>>();
        let selected_provider = schedule
            .provider()
            .and_then(|provider| choices.iter().position(|candidate| *candidate == provider))
            .unwrap_or(0) as i32;

        let schedules_for_toggle = schedules.to_vec();
        let toggle_setter = set_schedules.clone();
        let toggle_tx = settings_tx.clone();
        let toggle_id = schedule_id.clone();
        let trailing = activation_rule_toggle(schedule.enabled, move |enabled| {
            let mut next = schedules_for_toggle.clone();
            let Some(rule) = next.iter_mut().find(|rule| rule.id == toggle_id) else {
                return;
            };
            if rule.enabled == enabled {
                return;
            }
            rule.enabled = enabled;
            persist_schedules(toggle_setter.clone(), toggle_tx.clone(), next);
        });

        let mut fields = Vec::<Element>::new();
        if choices.is_empty() {
            fields.push(
                text_block("Turn on Codex or Claude in Providers first.")
                    .font_size(12.0)
                    .foreground(ThemeRef::SecondaryText)
                    .wrap()
                    .into(),
            );
        } else if choices.len() > 1 || schedule.provider().is_none() {
            let schedules_for_provider = schedules.to_vec();
            let provider_setter = set_schedules.clone();
            let provider_tx = settings_tx.clone();
            let provider_id = schedule_id.clone();
            fields.push(
                ComboBox::new(provider_labels)
                    .header("Provider")
                    .selected_index(selected_provider)
                    .horizontal_alignment(HorizontalAlignment::Stretch)
                    .on_selection_changed(move |choice: i32| {
                        let Some(provider) = choices.get(choice.max(0) as usize).copied() else {
                            return;
                        };
                        let mut next = schedules_for_provider.clone();
                        let Some(rule) = next.iter_mut().find(|rule| rule.id == provider_id) else {
                            return;
                        };
                        if rule.provider_id == provider.id() {
                            return;
                        }
                        rule.provider_id = provider.id().into();
                        persist_schedules(provider_setter.clone(), provider_tx.clone(), next);
                    })
                    .into(),
            );
        }

        let schedules_for_time = schedules.to_vec();
        let time_setter = set_schedules.clone();
        let time_tx = settings_tx.clone();
        let time_id = schedule_id.clone();
        fields.push(activation_time_field(
            "Time",
            schedule.time_minutes,
            time_format,
            move |time_minutes| {
                let mut next = schedules_for_time.clone();
                let Some(rule) = next.iter_mut().find(|rule| rule.id == time_id) else {
                    return;
                };
                if rule.time_minutes == time_minutes {
                    return;
                }
                rule.time_minutes = time_minutes;
                persist_schedules(time_setter.clone(), time_tx.clone(), next);
            },
        ));

        let schedules_for_days = schedules.to_vec();
        let days_setter = set_schedules.clone();
        let days_tx = settings_tx.clone();
        let days_id = schedule_id.clone();
        let weekday_selector = activation_weekday_selector(
            &schedule.weekdays,
            &format!("schedule-{schedule_id}-weekday"),
            move |day, checked| {
                let mut next = schedules_for_days.clone();
                let Some(rule) = next.iter_mut().find(|rule| rule.id == days_id) else {
                    return;
                };
                if !set_activation_weekday(&mut rule.weekdays, day, checked) {
                    return;
                }
                rule.weekday = *rule.weekdays.first().unwrap_or(&0);
                persist_schedules(days_setter.clone(), days_tx.clone(), next);
            },
        );
        fields.push(activation_days_field(weekday_selector));

        let schedules_for_remove = schedules.to_vec();
        let remove_setter = set_schedules.clone();
        let remove_tx = settings_tx.clone();
        let remove_id = schedule_id.clone();
        let clear_expanded = set_expanded_schedule.clone();
        fields.push(
            Button::new("Remove activation")
                .on_click(move || {
                    let next = schedules_for_remove
                        .iter()
                        .filter(|rule| rule.id != remove_id)
                        .cloned()
                        .collect();
                    clear_expanded.call(None);
                    persist_schedules(remove_setter.clone(), remove_tx.clone(), next);
                })
                .horizontal_alignment(HorizontalAlignment::Left)
                .into(),
        );

        let header = activation_rule_header(
            schedule.provider(),
            format!(
                "{} · {}",
                activation_weekdays_summary(&schedule.weekdays),
                activation_time_label(time_format, schedule.time_minutes),
            ),
        );
        let is_expanded = expanded_schedule.as_deref() == Some(schedule.id.as_str());
        let expand_setter = set_expanded_schedule.clone();
        let expand_id = schedule_id.clone();
        let content = vstack(fields)
            .spacing(10.0)
            .horizontal_alignment(HorizontalAlignment::Stretch);
        rows.push(
            settings_content_expander_with_trailing(
                header,
                Some(trailing),
                is_expanded,
                move |expanded: bool| {
                    expand_setter.call(expanded.then(|| expand_id.clone()));
                },
                format!("schedule-rule-{schedule_id}"),
                hovered_card_id,
                set_hovered_card_id.clone(),
                content,
            )
            .with_key(format!("schedule-rule-{schedule_id}"))
            .with_translation_transition(duration(CONTROL_NORMAL_ANIMATION))
            .with_opacity_transition(duration(CONTROL_NORMAL_ANIMATION)),
        );
    }
    rows
}

fn auto_activation_pause_cards(
    pauses: &[AutoActivationPause],
    provider_enabled: &[bool; 6],
    time_format: TimeFormat,
    expanded_pause: &Option<String>,
    set_expanded_pause: SetState<Option<String>>,
    set_pauses: SetState<Vec<AutoActivationPause>>,
    hovered_card_id: &Option<String>,
    set_hovered_card_id: SetState<Option<String>>,
    settings_tx: Sender<Settings>,
) -> Vec<Element> {
    if pauses.is_empty() {
        return vec![
            text_block(if activation_providers(provider_enabled).is_empty() {
                "Turn on Codex or Claude in Providers first."
            } else {
                "No quiet periods."
            })
            .font_size(12.0)
            .foreground(ThemeRef::SecondaryText)
            .wrap()
            .into(),
        ];
    }

    let mut rows = Vec::with_capacity(pauses.len());
    for pause in pauses {
        let pause_id = pause.id.clone();
        let choices = activation_provider_choices(provider_enabled, pause.provider());
        let provider_labels = choices
            .iter()
            .map(|provider| provider.display_name().to_string())
            .collect::<Vec<_>>();
        let selected_provider = pause
            .provider()
            .and_then(|provider| choices.iter().position(|candidate| *candidate == provider))
            .unwrap_or(0) as i32;

        let pauses_for_toggle = pauses.to_vec();
        let toggle_setter = set_pauses.clone();
        let toggle_tx = settings_tx.clone();
        let toggle_id = pause_id.clone();
        let trailing = activation_rule_toggle(pause.enabled, move |enabled| {
            let mut next = pauses_for_toggle.clone();
            let Some(rule) = next.iter_mut().find(|rule| rule.id == toggle_id) else {
                return;
            };
            if rule.enabled == enabled {
                return;
            }
            rule.enabled = enabled;
            persist_auto_activation_pauses(toggle_setter.clone(), toggle_tx.clone(), next);
        });

        let mut fields = Vec::<Element>::new();
        if choices.is_empty() {
            fields.push(
                text_block("Turn on Codex or Claude in Providers first.")
                    .font_size(12.0)
                    .foreground(ThemeRef::SecondaryText)
                    .wrap()
                    .into(),
            );
        } else if choices.len() > 1 || pause.provider().is_none() {
            let pauses_for_provider = pauses.to_vec();
            let provider_setter = set_pauses.clone();
            let provider_tx = settings_tx.clone();
            let provider_id = pause_id.clone();
            fields.push(
                ComboBox::new(provider_labels)
                    .header("Provider")
                    .selected_index(selected_provider)
                    .horizontal_alignment(HorizontalAlignment::Stretch)
                    .on_selection_changed(move |choice: i32| {
                        let Some(provider) = choices.get(choice.max(0) as usize).copied() else {
                            return;
                        };
                        let mut next = pauses_for_provider.clone();
                        let Some(rule) = next.iter_mut().find(|rule| rule.id == provider_id) else {
                            return;
                        };
                        if rule.provider_id == provider.id() {
                            return;
                        }
                        rule.provider_id = provider.id().into();
                        persist_auto_activation_pauses(
                            provider_setter.clone(),
                            provider_tx.clone(),
                            next,
                        );
                    })
                    .into(),
            );
        }

        let pauses_for_start = pauses.to_vec();
        let start_setter = set_pauses.clone();
        let start_tx = settings_tx.clone();
        let start_id = pause_id.clone();
        let pauses_for_end = pauses.to_vec();
        let end_setter = set_pauses.clone();
        let end_tx = settings_tx.clone();
        let end_id = pause_id.clone();
        fields.push(
            grid((
                activation_time_field(
                    "From",
                    pause.start_time_minutes,
                    time_format,
                    move |time_minutes| {
                        let mut next = pauses_for_start.clone();
                        let Some(rule) = next.iter_mut().find(|rule| rule.id == start_id) else {
                            return;
                        };
                        if rule.start_time_minutes == time_minutes {
                            return;
                        }
                        rule.start_time_minutes = time_minutes;
                        persist_auto_activation_pauses(
                            start_setter.clone(),
                            start_tx.clone(),
                            next,
                        );
                    },
                )
                .grid_column(0),
                activation_time_field(
                    "Until",
                    pause.end_time_minutes,
                    time_format,
                    move |time_minutes| {
                        let mut next = pauses_for_end.clone();
                        let Some(rule) = next.iter_mut().find(|rule| rule.id == end_id) else {
                            return;
                        };
                        if rule.end_time_minutes == time_minutes {
                            return;
                        }
                        rule.end_time_minutes = time_minutes;
                        persist_auto_activation_pauses(end_setter.clone(), end_tx.clone(), next);
                    },
                )
                .grid_column(1),
            ))
            .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
            .rows([GridLength::Auto])
            .column_spacing(8.0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .into(),
        );

        let pauses_for_days = pauses.to_vec();
        let days_setter = set_pauses.clone();
        let days_tx = settings_tx.clone();
        let days_id = pause_id.clone();
        let weekday_selector = activation_weekday_selector(
            &pause.weekdays,
            &format!("auto-pause-{pause_id}-weekday"),
            move |day, checked| {
                let mut next = pauses_for_days.clone();
                let Some(rule) = next.iter_mut().find(|rule| rule.id == days_id) else {
                    return;
                };
                if !set_activation_weekday(&mut rule.weekdays, day, checked) {
                    return;
                }
                persist_auto_activation_pauses(days_setter.clone(), days_tx.clone(), next);
            },
        );
        fields.push(activation_days_field(weekday_selector));

        let pauses_for_remove = pauses.to_vec();
        let remove_setter = set_pauses.clone();
        let remove_tx = settings_tx.clone();
        let remove_id = pause_id.clone();
        let clear_expanded = set_expanded_pause.clone();
        fields.push(
            Button::new("Remove quiet period")
                .on_click(move || {
                    let next = pauses_for_remove
                        .iter()
                        .filter(|rule| rule.id != remove_id)
                        .cloned()
                        .collect();
                    clear_expanded.call(None);
                    persist_auto_activation_pauses(remove_setter.clone(), remove_tx.clone(), next);
                })
                .horizontal_alignment(HorizontalAlignment::Left)
                .into(),
        );

        let time_summary =
            if pause.start_time_minutes == 0 && pause.end_time_minutes == 23 * 60 + 59 {
                "All day".into()
            } else {
                format!(
                    "{}–{}",
                    activation_time_label(time_format, pause.start_time_minutes),
                    activation_time_label(time_format, pause.end_time_minutes),
                )
            };
        let header = activation_rule_header(
            pause.provider(),
            format!(
                "{} · {time_summary}",
                activation_weekdays_summary(&pause.weekdays),
            ),
        );
        let is_expanded = expanded_pause.as_deref() == Some(pause.id.as_str());
        let expand_setter = set_expanded_pause.clone();
        let expand_id = pause_id.clone();
        let content = vstack(fields)
            .spacing(10.0)
            .horizontal_alignment(HorizontalAlignment::Stretch);
        rows.push(
            settings_content_expander_with_trailing(
                header,
                Some(trailing),
                is_expanded,
                move |expanded: bool| {
                    expand_setter.call(expanded.then(|| expand_id.clone()));
                },
                format!("auto-activation-pause-{pause_id}"),
                hovered_card_id,
                set_hovered_card_id.clone(),
                content,
            )
            .with_key(format!("auto-activation-pause-{pause_id}"))
            .with_translation_transition(duration(CONTROL_NORMAL_ANIMATION))
            .with_opacity_transition(duration(CONTROL_NORMAL_ANIMATION)),
        );
    }
    rows
}

fn persist_schedules(
    setter: SetState<Vec<ScheduledActivation>>,
    settings_tx: Sender<Settings>,
    schedules: Vec<ScheduledActivation>,
) {
    setter.call(schedules.clone());
    persist_update(settings_tx, move |settings| {
        settings.scheduled_activations = schedules
    });
}

fn persist_auto_activation_pauses(
    setter: SetState<Vec<AutoActivationPause>>,
    settings_tx: Sender<Settings>,
    pauses: Vec<AutoActivationPause>,
) {
    setter.call(pauses.clone());
    persist_update(settings_tx, move |settings| {
        settings.auto_activation_pauses = pauses
    });
}
