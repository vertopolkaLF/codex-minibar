//! Limit activation: automatic 5-hour sessions, quiet periods and schedules.

use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px,
};

use super::kit::{self, Button, Kit, Row, eid};
use super::window::SettingsWindow;
use crate::settings::{
    AutoActivationPause, ProviderId, ProviderInstance, ScheduledActivation, TimeFormat,
};

const WEEKDAY_LABELS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// Enabled instances that can start sessions with their own login.
fn activation_providers(instances: &[ProviderInstance]) -> Vec<ProviderId> {
    instances
        .iter()
        .filter(|instance| {
            instance.enabled && crate::instances::Capabilities::of(instance).auto_activation
        })
        .map(ProviderInstance::provider_id)
        .collect()
}

fn provider_choices(
    instances: &[ProviderInstance],
    current: Option<ProviderId>,
) -> Vec<ProviderId> {
    instances
        .iter()
        .filter(|instance| {
            crate::instances::Capabilities::of(instance).auto_activation
                && (instance.enabled || current == Some(instance.provider_id()))
        })
        .map(ProviderInstance::provider_id)
        .collect()
}

pub(super) fn weekdays_summary(weekdays: &[u8]) -> String {
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
            .map(|day| WEEKDAY_LABELS[*day as usize])
            .collect::<Vec<_>>()
            .join(", "),
    }
}

pub(super) fn time_label(time_format: TimeFormat, minutes: u16) -> String {
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

/// Toggle one weekday, never leaving a rule without days.
pub(super) fn set_weekday(weekdays: &mut Vec<u8>, day: u8, checked: bool) -> bool {
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

/// Which list a rule belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RuleList {
    Schedule,
    Pause,
}

impl RuleList {
    fn prefix(self) -> &'static str {
        match self {
            Self::Schedule => "schedule",
            Self::Pause => "pause",
        }
    }
}

/// Fields a rule editor needs, independent of its list.
#[derive(Clone)]
struct RuleView {
    id: String,
    enabled: bool,
    provider: Option<ProviderId>,
    weekdays: Vec<u8>,
    times: Vec<(&'static str, u16)>,
    summary: String,
}

impl SettingsWindow {
    pub(super) fn activation_page(
        &mut self,
        k: &mut Kit,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let instances = self.settings.instances.clone();
        let time_format = self.settings.time_format;
        let mut rows = Vec::new();

        let candidates = instances
            .iter()
            .filter(|instance| {
                crate::provider_registry::descriptor(instance.driver).supports_activation
            })
            .collect::<Vec<_>>();
        rows.push(kit::section_header(
            k,
            "Start 5-hour sessions automatically",
            Some(
                "Starts a new session as soon as a window is available, instead of waiting for your first request. Each account uses its own login."
                    .into(),
            ),
            None,
        ));
        if candidates.is_empty() {
            rows.push(kit::caption(k, "Add Codex or Claude in Providers first."));
        } else {
            let mut automatic = Vec::new();
            for instance in candidates {
                use crate::instances::{Capabilities, Capability};
                let provider = instance.provider_id();
                let reason = Capabilities::reason(instance, Capability::AutoActivation);
                let description =
                    (!instance.enabled).then(|| SharedString::from("Off in Providers"));
                automatic.push(kit::toggle_row_with(
                    k,
                    format!("activation-auto-{}", instance.id),
                    provider.qualified_name(),
                    description,
                    instance.auto_activation,
                    reason.map(SharedString::from),
                    Self::h(cx, move |this, value: bool, _, cx| {
                        this.edit_instance(cx, provider, move |instance| {
                            instance.auto_activation = value
                        })
                    }),
                ));
            }
            rows.push(kit::card(k, automatic));
        }

        let default_provider = activation_providers(&instances).into_iter().next();
        let add_pause = Self::h(cx, move |this, (), _, cx| {
            let Some(provider) = default_provider else {
                return;
            };
            let pause = AutoActivationPause::new(provider);
            this.set_expanded(format!("pause-{}", pause.id), true);
            this.edit(cx, move |settings| {
                settings.auto_activation_pauses.push(pause.clone())
            });
        });
        rows.push(kit::section_header(
            k,
            "Quiet periods",
            Some("Don't auto-start sessions during these times.".into()),
            Some(
                Button::new("activation-add-pause", "Add")
                    .with_icon("plus-bold")
                    .disabled(default_provider.is_none())
                    .on_click(add_pause)
                    .render(k),
            ),
        ));
        let pauses = self
            .settings
            .auto_activation_pauses
            .iter()
            .map(|pause| {
                let all_day =
                    pause.start_time_minutes == 0 && pause.end_time_minutes == LAST_MINUTE;
                let times = if all_day {
                    "All day".to_owned()
                } else {
                    format!(
                        "{}–{}",
                        time_label(time_format, pause.start_time_minutes),
                        time_label(time_format, pause.end_time_minutes)
                    )
                };
                RuleView {
                    id: pause.id.clone(),
                    enabled: pause.enabled,
                    provider: pause.provider(),
                    weekdays: pause.weekdays.clone(),
                    times: vec![
                        ("From", pause.start_time_minutes),
                        ("Until", pause.end_time_minutes),
                    ],
                    summary: format!("{} · {times}", weekdays_summary(&pause.weekdays)),
                }
            })
            .collect::<Vec<_>>();
        rows.extend(self.rule_cards(k, cx, RuleList::Pause, pauses, &instances, time_format));

        let add_schedule = Self::h(cx, move |this, (), _, cx| {
            let Some(provider) = default_provider else {
                return;
            };
            let schedule = ScheduledActivation::new(provider);
            this.set_expanded(format!("schedule-{}", schedule.id), true);
            this.edit(cx, move |settings| {
                settings.scheduled_activations.push(schedule.clone())
            });
        });
        rows.push(kit::section_header(
            k,
            "Scheduled activations",
            Some("Start a 5-hour session at a set time.".into()),
            Some(
                Button::new("activation-add-schedule", "Add")
                    .with_icon("plus-bold")
                    .disabled(default_provider.is_none())
                    .on_click(add_schedule)
                    .render(k),
            ),
        ));
        let schedules = self
            .settings
            .scheduled_activations
            .iter()
            .map(|schedule| RuleView {
                id: schedule.id.clone(),
                enabled: schedule.enabled,
                provider: schedule.provider(),
                weekdays: schedule.weekdays.clone(),
                times: vec![("Time", schedule.time_minutes)],
                summary: format!(
                    "{} · {}",
                    weekdays_summary(&schedule.weekdays),
                    time_label(time_format, schedule.time_minutes)
                ),
            })
            .collect::<Vec<_>>();
        rows.extend(self.rule_cards(
            k,
            cx,
            RuleList::Schedule,
            schedules,
            &instances,
            time_format,
        ));
        rows
    }

    fn edit_rule(
        &mut self,
        cx: &mut Context<Self>,
        list: RuleList,
        id: String,
        edit: impl Fn(&mut Option<ProviderId>, &mut bool, &mut Vec<u8>, &mut [u16]) -> bool
        + Send
        + 'static,
    ) {
        // Validate against the local snapshot first so no-op edits are skipped.
        let changed = {
            let mut probe = self.settings.clone();
            apply_rule(&mut probe, list, &id, &edit)
        };
        if changed {
            self.edit(cx, move |settings| {
                apply_rule(settings, list, &id, &edit);
            });
        }
    }

    fn rule_cards(
        &mut self,
        k: &mut Kit,
        cx: &mut Context<Self>,
        list: RuleList,
        rules: Vec<RuleView>,
        instances: &[ProviderInstance],
        time_format: TimeFormat,
    ) -> Vec<AnyElement> {
        if rules.is_empty() {
            let message = if activation_providers(instances).is_empty() {
                "Turn on Codex or Claude in Providers first, with a config folder login."
            } else if list == RuleList::Pause {
                "No quiet periods yet."
            } else {
                "No scheduled activations yet."
            };
            return vec![
                div()
                    .px(px(16.0))
                    .py(px(18.0))
                    .rounded(px(kit::CARD_RADIUS))
                    .bg(k.theme.subtle_hover)
                    .flex()
                    .justify_center()
                    .child(kit::caption(k, message))
                    .into_any_element(),
            ];
        }
        let mut out = Vec::new();
        for rule in rules {
            let card_id = format!("{}-{}", list.prefix(), rule.id);
            let expanded = self.is_expanded(&card_id);
            let remove = {
                let rule_id = rule.id.clone();
                let card_id = card_id.clone();
                Self::h(cx, move |this, (), _, cx| {
                    this.set_expanded(card_id.clone(), false);
                    let id = rule_id.clone();
                    this.edit(cx, move |settings| match list {
                        RuleList::Schedule => {
                            settings.scheduled_activations.retain(|rule| rule.id != id)
                        }
                        RuleList::Pause => {
                            settings.auto_activation_pauses.retain(|rule| rule.id != id)
                        }
                    });
                })
            };
            let rule_id = rule.id.clone();
            let toggle = kit::toggle(
                k,
                format!("{card_id}-enabled"),
                rule.enabled,
                false,
                Self::h(cx, move |this, value: bool, _, cx| {
                    this.edit_rule(cx, list, rule_id.clone(), move |_, enabled, _, _| {
                        let changed = *enabled != value;
                        *enabled = value;
                        changed
                    })
                }),
            );
            let header = Row::new(
                format!("{card_id}-header"),
                rule.provider
                    .map(ProviderId::qualified_name)
                    .unwrap_or_else(|| "Unknown provider".into()),
            )
            .icon(kit::row_icon(
                k,
                if list == RuleList::Pause {
                    "clock-fill"
                } else {
                    "arrow-clockwise-bold"
                },
            ))
            .description(k, rule.summary.clone())
            .trailing(toggle)
            .trailing(
                Button::icon_only(format!("{card_id}-remove"), "trash-fill")
                    .danger()
                    .ghost()
                    .tooltip(if list == RuleList::Pause {
                        "Remove quiet period"
                    } else {
                        "Remove activation"
                    })
                    .on_click(remove)
                    .render(k),
            );
            let body = self.rule_body(k, cx, list, &rule, instances, time_format);
            let on_toggle = Self::expand_handler(cx, card_id.clone());
            let card = kit::expander(k, card_id.clone(), header, expanded, on_toggle, move |_| {
                body
            });
            out.push(kit::appear(k, format!("{card_id}-appear"), card));
        }
        out
    }

    fn rule_body(
        &mut self,
        k: &mut Kit,
        cx: &mut Context<Self>,
        list: RuleList,
        rule: &RuleView,
        instances: &[ProviderInstance],
        time_format: TimeFormat,
    ) -> AnyElement {
        let mut fields = div().flex().flex_col().gap(px(14.0));
        let choices = provider_choices(instances, rule.provider);
        if choices.is_empty() {
            fields = fields.child(kit::caption(
                k,
                "Turn on Codex or Claude in Providers first, with a config folder login.",
            ));
        } else if choices.len() > 1 || rule.provider.is_none() {
            let labels = choices
                .iter()
                .map(|provider| SharedString::from(provider.qualified_name()))
                .collect();
            let selected = rule
                .provider
                .and_then(|provider| choices.iter().position(|candidate| *candidate == provider));
            let rule_id = rule.id.clone();
            fields = fields.child(kit::field(
                k,
                "Provider",
                kit::dropdown(
                    k,
                    format!("{}-{}-provider", list.prefix(), rule.id),
                    labels,
                    selected,
                    false,
                    280.0,
                    Self::h(cx, move |this, index: usize, _, cx| {
                        let Some(provider) = choices.get(index).copied() else {
                            return;
                        };
                        this.edit_rule(cx, list, rule_id.clone(), move |current, _, _, _| {
                            let changed = *current != Some(provider);
                            *current = Some(provider);
                            changed
                        })
                    }),
                ),
            ));
        }
        let pause = list == RuleList::Pause;
        let mut time_row = div().flex().gap(px(16.0)).flex_wrap();
        if pause {
            time_row = time_row.items_end();
        }
        let mut pickers = Vec::new();
        for (slot, (label, minutes)) in rule.times.iter().enumerate() {
            let rule_id = rule.id.clone();
            let on_change = Self::h(cx, move |this, value: u16, _, cx| {
                this.edit_rule(cx, list, rule_id.clone(), move |_, _, _, times| {
                    let changed = times.get(slot).is_some_and(|current| *current != value);
                    if let Some(time) = times.get_mut(slot) {
                        *time = value;
                    }
                    changed
                })
            });
            time_row = time_row.child(kit::field(
                k,
                *label,
                kit::time_picker(
                    k,
                    format!("{}-{}-time-{slot}", list.prefix(), rule.id),
                    *minutes,
                    time_format,
                    on_change,
                ),
            ));
        }
        let time_row = time_row.into_any_element();
        if list == RuleList::Pause {
            // A whole-day pause is 00:00–23:59 inclusive; the checkbox makes
            // that explicit instead of relying on picking the last minute.
            let all_day = is_all_day(&rule.times);
            let rule_id = rule.id.clone();
            fields = fields.child(kit::checkbox(
                k,
                format!("{}-{}-all-day", list.prefix(), rule.id),
                all_day,
                false,
                Some("All day".into()),
                Self::h(cx, move |this, value: bool, _, cx| {
                    this.edit_rule(cx, list, rule_id.clone(), move |_, _, _, times| {
                        let next = if value {
                            [0, LAST_MINUTE]
                        } else {
                            [9 * 60, 17 * 60]
                        };
                        let changed = times[..] != next[..];
                        times.copy_from_slice(&next);
                        changed
                    })
                }),
            ));
            let key = crate::popup_window::ui::fx::key(("pause-times", rule.id.as_str()));
            if let Some(times) = kit::collapsible(k, key, !all_day, move |_| time_row) {
                fields = fields.child(times);
            }
        } else {
            fields = fields.child(time_row);
        }

        let mut days = div().flex().gap(px(6.0));
        for (day, label) in WEEKDAY_LABELS.iter().enumerate() {
            let selected = rule.weekdays.contains(&(day as u8));
            let rule_id = rule.id.clone();
            let theme = &k.theme;
            let hover = if selected {
                theme.accent_hover
            } else {
                theme.control_hover
            };
            let toggle = Self::h(cx, move |this, (), _, cx| {
                this.edit_rule(cx, list, rule_id.clone(), move |_, _, weekdays, _| {
                    set_weekday(weekdays, day as u8, !selected)
                })
            });
            let day_id = format!("{}-{}-day-{day}", list.prefix(), rule.id);
            let rest = if selected {
                theme.accent
            } else {
                theme.control
            };
            days = days.child(
                kit::hover_bg(
                    k,
                    div().id(eid(day_id.clone())),
                    kit::hover_key(&day_id),
                    rest,
                    hover,
                )
                .w(px(48.0))
                .h(px(kit::CONTROL_HEIGHT))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(kit::CONTROL_RADIUS))
                .text_size(px(13.0))
                .text_color(if selected {
                    theme.on_accent
                } else {
                    theme.text
                })
                .cursor_pointer()
                .on_click(move |_, window, cx| toggle((), window, cx))
                .child(*label),
            );
        }
        fields = fields.child(kit::field(k, "Days", days.into_any_element()));

        fields.into_any_element()
    }
}

fn apply_rule(
    settings: &mut crate::settings::Settings,
    list: RuleList,
    id: &str,
    edit: &impl Fn(&mut Option<ProviderId>, &mut bool, &mut Vec<u8>, &mut [u16]) -> bool,
) -> bool {
    match list {
        RuleList::Schedule => {
            let Some(rule) = settings
                .scheduled_activations
                .iter_mut()
                .find(|rule| rule.id == id)
            else {
                return false;
            };
            let mut provider = rule.provider();
            let mut times = [rule.time_minutes];
            let changed = edit(
                &mut provider,
                &mut rule.enabled,
                &mut rule.weekdays,
                &mut times,
            );
            if let Some(provider) = provider {
                rule.provider_id = provider.id().into();
            }
            rule.time_minutes = times[0];
            rule.weekday = *rule.weekdays.first().unwrap_or(&0);
            changed
        }
        RuleList::Pause => {
            let Some(rule) = settings
                .auto_activation_pauses
                .iter_mut()
                .find(|rule| rule.id == id)
            else {
                return false;
            };
            let mut provider = rule.provider();
            let mut times = [rule.start_time_minutes, rule.end_time_minutes];
            let changed = edit(
                &mut provider,
                &mut rule.enabled,
                &mut rule.weekdays,
                &mut times,
            );
            if let Some(provider) = provider {
                rule.provider_id = provider.id().into();
            }
            rule.start_time_minutes = times[0];
            rule.end_time_minutes = times[1];
            changed
        }
    }
}

const LAST_MINUTE: u16 = 23 * 60 + 59;

/// Pause bounds that cover every minute of the day.
fn is_all_day(times: &[(&'static str, u16)]) -> bool {
    matches!(times, [(_, 0), (_, LAST_MINUTE)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekday_summary_names_common_sets() {
        assert_eq!(weekdays_summary(&[0, 1, 2, 3, 4, 5, 6]), "Every day");
        assert_eq!(weekdays_summary(&[4, 3, 2, 1, 0]), "Weekdays");
        assert_eq!(weekdays_summary(&[5, 6]), "Weekends");
        assert_eq!(weekdays_summary(&[0, 2]), "Mon, Wed");
    }

    #[test]
    fn last_weekday_cannot_be_removed() {
        let mut days = vec![3];
        assert!(!set_weekday(&mut days, 3, false));
        assert!(set_weekday(&mut days, 1, true));
        assert_eq!(days, vec![1, 3]);
    }

    #[test]
    fn twelve_hour_labels() {
        assert_eq!(time_label(TimeFormat::Hour12, 0), "12:00 AM");
        assert_eq!(time_label(TimeFormat::Hour12, 13 * 60 + 5), "1:05 PM");
        assert_eq!(time_label(TimeFormat::Hour24, 13 * 60 + 5), "13:05");
    }
}
