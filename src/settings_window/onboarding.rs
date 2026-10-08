//! First-launch flow: choose providers, a few general settings, then
//! notifications.
//! Choices stay local until Done, so a dismissed window never
//! half-configures provider workers.

use std::sync::Arc;

use gpui::{
    AnyElement, App, Context, FocusHandle, Focusable, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Window,
    WindowControlArea, div, px,
};

use super::general::usage_refresh_labels;
use super::kit::{self, Button, Handler, Kit};
use super::persistence::{load_settings_for_window, replace_settings};
use super::theme::{Fonts, Theme};
use crate::popup_window::AppState;
use crate::popup_window::ui::fx;
use crate::settings::{
    ProviderInstance, ProviderKind, Settings, TrayWidget,
    UsageRefreshInterval,
};

const DRIVERS: usize = ProviderKind::ALL.len();

/// Whether each driver (in [`ProviderKind::ALL`] order) is installed, using
/// the paths of its first instance.
fn detected_providers(settings: &Settings) -> [bool; DRIVERS] {
    let first = |driver: ProviderKind| {
        settings
            .instances
            .iter()
            .find(|instance| instance.driver == driver)
            .cloned()
            .unwrap_or_else(|| ProviderInstance::primary(driver))
    };
    ProviderKind::ALL.map(|driver| {
        let instance = first(driver);
        let path = instance.binary_path.as_deref();
        match driver {
            ProviderKind::Codex => crate::codex::is_installed(path),
            ProviderKind::Claude => crate::claude::is_installed(path),
            ProviderKind::Cursor => crate::cursor::is_installed(path),
            ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo => {
                crate::opencode::is_installed(driver)
            }
            ProviderKind::OpenRouter => {
                crate::openrouter::is_installed_for_accounts(instance.openrouter.as_slice())
            }
            ProviderKind::Antigravity => crate::antigravity::is_installed(path),
            ProviderKind::Grok => crate::grok::is_installed(path),
            ProviderKind::Kiro => crate::kiro::source_is_ready(
                path,
                instance.kiro_crew_path.as_deref(),
                instance.kiro_cli_path.as_deref(),
            ),
        }
    })
}

/// What the provider step says about each driver.
fn detection_note(driver: ProviderKind, found: bool) -> &'static str {
    match (driver, found) {
        (ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo, true) => {
            crate::i18n::tr("found-in-opencode-auth-or-local-history")
        }
        (ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo, false) => {
            crate::i18n::tr("not-found-turn-it-on-if-it-s-set-up-elsewhere")
        }
        (ProviderKind::OpenRouter, true) => crate::i18n::tr("account-credentials-are-already-set"),
        (ProviderKind::OpenRouter, false) => {
            crate::i18n::tr("optional-add-accounts-later-in-providers")
        }
        (ProviderKind::Antigravity, true) => {
            crate::i18n::tr("found-an-official-agy-sign-in-on-this-pc")
        }
        (ProviderKind::Antigravity, false) => {
            crate::i18n::tr("not-found-sign-in-with-agy-before-enabling-it")
        }
        (ProviderKind::Grok, true) => {
            crate::i18n::tr("found-an-official-grok-cli-sign-in-on-this-pc")
        }
        (ProviderKind::Grok, false) => {
            crate::i18n::tr("not-found-run-grok-login-before-enabling-it")
        }
        (ProviderKind::Kiro, true) => {
            crate::i18n::tr("found-kiro-ide-kiro-crew-or-a-signed-in-kiro-cli")
        }
        (ProviderKind::Kiro, false) => {
            crate::i18n::tr("not-found-install-kiro-ide-or-kiro-crew-or-sign-in-to-kiro-cli")
        }
        (_, true) => crate::i18n::tr("found-on-this-pc"),
        (_, false) => crate::i18n::tr("not-found-turn-it-on-if-it-s-installed-somewhere-else"),
    }
}

/// Turns each driver's first instance on or off, adding it when missing.
fn apply_onboarding_choices(settings: &mut Settings, enabled: [bool; DRIVERS], automatic: bool) {
    for (driver, enabled) in ProviderKind::ALL.into_iter().zip(enabled) {
        let provider = match settings
            .instances
            .iter()
            .find(|instance| instance.driver == driver)
        {
            Some(instance) => instance.provider_id(),
            None if enabled => {
                let instance = settings.new_instance(driver, driver.display_name());
                settings.add_instance(instance)
            }
            None => continue,
        };
        if let Some(instance) = settings.instance_mut(provider) {
            instance.enabled = enabled;
            if crate::instances::Capabilities::of(instance).auto_activation {
                instance.auto_activation = automatic;
            }
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
enum Step {
    #[default]
    Providers,
    General,
    Notifications,
}

impl Step {
    const fn index(self) -> usize {
        match self {
            Self::Providers => 0,
            Self::General => 1,
            Self::Notifications => 2,
        }
    }
}

pub(crate) struct OnboardingWindow {
    state: Arc<AppState>,
    backdrop: super::backdrop::Backdrop,
    settings: Settings,
    detected: [bool; DRIVERS],
    enabled: [bool; DRIVERS],
    automatic: bool,
    step: Step,
    kit: Kit,
    fonts: Fonts,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl Focusable for OnboardingWindow {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl OnboardingWindow {
    pub(crate) fn new(state: Arc<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = load_settings_for_window();
        let detected = detected_providers(&settings);
        let automatic = settings
            .instances
            .iter()
            .any(|instance| instance.auto_activation);
        let mut subscriptions = vec![cx.observe_window_appearance(window, |_, _, cx| cx.notify())];
        subscriptions.push(cx.on_release(|_, _| super::window_closed(true)));
        window.on_window_should_close(cx, |_, _| {
            super::window_closed(true);
            true
        });
        let focus = cx.focus_handle();
        window.focus(&focus);
        Self {
            state,
            backdrop: super::backdrop::Backdrop::install(window),
            settings,
            detected,
            enabled: detected,
            automatic,
            step: Step::Providers,
            kit: Kit::default(),
            fonts: Fonts::resolve(cx),
            focus,
            _subscriptions: subscriptions,
        }
    }

    fn h<T: 'static>(
        cx: &Context<Self>,
        f: impl Fn(&mut Self, T, &mut Window, &mut Context<Self>) + 'static,
    ) -> Handler<T> {
        let this = cx.weak_entity();
        std::rc::Rc::new(move |value, window, cx| {
            let _ = this.update(cx, |this, cx| f(this, value, window, cx));
        })
    }

    fn finish(&mut self, window: &mut Window) {
        let mut completed = self.settings.clone();
        completed.onboarding_completed = true;
        apply_onboarding_choices(&mut completed, self.enabled, self.automatic);
        completed.tray_widgets = completed
            .enabled_providers()
            .into_iter()
            .filter(|provider| {
                !crate::provider_registry::descriptor(provider.kind())
                    .default_tray_metrics
                    .is_empty()
            })
            .map(TrayWidget::for_provider)
            .collect();
        if let Err(error) = replace_settings(self.state.settings_tx.clone(), completed) {
            eprintln!("failed to complete onboarding: {error:#}");
            crate::notifications::show(
                crate::i18n::tr("setup-could-not-be-saved"),
                &format!("{error:#}"),
            );
            return;
        }
        // Show the popup before dismissing onboarding so Done always lands on it.
        crate::popup::show_near_cursor();
        super::window_closed(true);
        window.remove_window();
    }

    fn providers_step(&mut self, k: &mut Kit, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        // Detected providers first; detection is fixed when the window opens,
        // so the order never shifts while the user toggles.
        let mut order: Vec<(usize, ProviderKind)> =
            ProviderKind::ALL.into_iter().enumerate().collect();
        order.sort_by_key(|(index, _)| !self.detected[*index]);
        for (index, driver) in order {
            let theme = &k.theme;
            let on = self.enabled[index];
            let mark = div()
                .size(px(32.0))
                .flex_none()
                .rounded(px(8.0))
                .bg(theme.card)
                .flex()
                .items_center()
                .justify_center()
                .child(kit::icon(
                    crate::provider_registry::icon(driver),
                    16.0,
                    if on {
                        theme.brand(driver)
                    } else {
                        theme.glyph()
                    },
                ))
                .into_any_element();
            let found = self.detected[index];
            let mut row = kit::Row::new(
                format!("onboarding-{}", driver.display_name()),
                driver.display_name(),
            )
            .icon(mark)
            .description(k, detection_note(driver, found));
            if found {
                row = row.trailing(kit::chip(k, crate::i18n::tr("detected")));
            }
            let switch = kit::toggle(
                k,
                format!("onboarding-toggle-{index}"),
                on,
                false,
                Self::h(cx, move |this, value: bool, _, cx| {
                    this.enabled[index] = value;
                    cx.notify();
                }),
            );
            let toggle_row = Self::h(cx, move |this, (), _, cx| {
                this.enabled[index] = !this.enabled[index];
                cx.notify();
            });
            rows.push(row.trailing(switch).on_click(toggle_row).render(k));
        }
        vec![kit::card(k, rows)]
    }

    fn general_step(&mut self, k: &mut Kit, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let s = &self.settings;
        let startup = kit::card_of(k, |k| {
            vec![
                kit::dropdown_row(
                    k,
                    "onboarding-language",
                    crate::i18n::tr("language"),
                    None,
                    kit::options(&crate::i18n::Language::labels()),
                    s.language.index() as i32,
                    false,
                    Self::h(cx, |this, index: usize, _, cx| {
                        this.settings.language = crate::i18n::Language::from_index(index);
                        this.settings.language.apply();
                        cx.refresh_windows();
                        cx.notify();
                    }),
                ),
                kit::toggle_row(
                    k,
                    "onboarding-start",
                    crate::i18n::tr("start-with-windows"),
                    None,
                    s.start_at_login,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.settings.start_at_login = value;
                        cx.notify();
                    }),
                ),
            ]
        });
        let features = kit::card_of(k, |k| {
            vec![
                kit::toggle_row(
                    k,
                    "onboarding-automatic",
                    crate::i18n::tr("start-5-hour-sessions-automatically"),
                    Some(
                        crate::i18n::tr(
                            "starts-a-new-codex-or-claude-session-as-soon-as-a-window-is-avail",
                        )
                        .into(),
                    ),
                    self.automatic,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.automatic = value;
                        cx.notify();
                    }),
                ),
                kit::dropdown_row(
                    k,
                    "onboarding-usage-refresh",
                    crate::i18n::tr("collect-usage-data"),
                    Some(crate::i18n::tr(
                        "scans-local-provider-history-for-usage-stats",
                    )),
                    kit::options(&usage_refresh_labels()),
                    s.usage_refresh_interval.index(),
                    false,
                    Self::h(cx, |this, index: usize, _, cx| {
                        this.settings.usage_refresh_interval =
                            UsageRefreshInterval::from_index(index as i32);
                        cx.notify();
                    }),
                ),
            ]
        });
        let customize = kit::card_of(k, |k| {
            vec![
                kit::toggle_row(
                    k,
                    "onboarding-show-used",
                    crate::i18n::tr("show-used-instead-of-remaining"),
                    None,
                    s.show_used_percentage,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.settings.show_used_percentage = value;
                        cx.notify();
                    }),
                ),
                kit::toggle_row(
                    k,
                    "onboarding-pace",
                    crate::i18n::tr("show-usage-pace"),
                    Some(
                        crate::i18n::tr(
                            "marks-whether-you-re-burning-quota-faster-or-slower-than-an-even",
                        )
                        .into(),
                    ),
                    s.show_usage_pace,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.settings.show_usage_pace = value;
                        cx.notify();
                    }),
                ),
                kit::toggle_row(
                    k,
                    "onboarding-account",
                    crate::i18n::tr("show-account-name"),
                    None,
                    s.show_account_name,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.settings.show_account_name = value;
                        cx.notify();
                    }),
                ),
            ]
        });
        vec![
            kit::section_heading(k, crate::i18n::tr("startup")),
            startup,
            kit::section_heading(k, crate::i18n::tr("features")),
            features,
            kit::section_heading(k, crate::i18n::tr("customization")),
            customize,
        ]
    }

    fn notifications_step(&mut self, k: &mut Kit, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let n = &self.settings.notifications;
        let activity = kit::card_of(k, |k| {
            vec![
                kit::toggle_row(
                    k,
                    "onboarding-notif-activation-success",
                    crate::i18n::tr("successful-activations"),
                    None,
                    n.activation_success,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.settings.notifications.activation_success = value;
                        cx.notify();
                    }),
                ),
                kit::toggle_row(
                    k,
                    "onboarding-notif-activation-failure",
                    crate::i18n::tr("failed-activations"),
                    None,
                    n.activation_failure,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.settings.notifications.activation_failure = value;
                        cx.notify();
                    }),
                ),
                kit::toggle_row(
                    k,
                    "onboarding-notif-limits-reset",
                    crate::i18n::tr("when-limits-reset"),
                    None,
                    n.limits_changed,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.settings.notifications.limits_changed = value;
                        cx.notify();
                    }),
                ),
            ]
        });
        let low = kit::card_of(k, |k| {
            vec![
                kit::toggle_row(
                    k,
                    "onboarding-notif-low-usage",
                    crate::i18n::format(
                        "when-5-hour-remaining-hits",
                        &[("v0", n.low_usage_threshold_percent.to_string())],
                    ),
                    None,
                    n.low_usage_enabled,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.settings.notifications.low_usage_enabled = value;
                        cx.notify();
                    }),
                ),
                kit::toggle_row(
                    k,
                    "onboarding-notif-weekly-low-usage",
                    crate::i18n::format(
                        "when-weekly-remaining-hits",
                        &[("v0", n.weekly_low_usage_threshold_percent.to_string())],
                    ),
                    None,
                    n.weekly_low_usage_enabled,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.settings.notifications.weekly_low_usage_enabled = value;
                        cx.notify();
                    }),
                ),
            ]
        });
        let updates = kit::card_of(k, |k| {
            vec![kit::toggle_row(
                k,
                "onboarding-notif-update",
                crate::i18n::tr("when-a-new-version-is-found"),
                None,
                n.update_available,
                Self::h(cx, |this, value: bool, _, cx| {
                    this.settings.notifications.update_available = value;
                    cx.notify();
                }),
            )]
        });
        vec![
            kit::section_heading(k, crate::i18n::tr("activity")),
            activity,
            kit::section_heading(k, crate::i18n::tr("low-usage")),
            low,
            kit::section_heading(k, crate::i18n::tr("updates")),
            updates,
        ]
    }

    fn step_dots(&self, k: &mut Kit) -> AnyElement {
        let index = self.step.index() as f32;
        let t = k.fx.value(fx::key("onboarding-step"), index, fx::NORMAL);
        let theme = &k.theme;
        let dot = |active: f32| {
            div()
                .h(px(6.0))
                .w(px(6.0 + 14.0 * active))
                .rounded_full()
                .bg(theme
                    .control_strong
                    .opacity(0.5)
                    .blend(theme.accent.opacity(active)))
        };
        div()
            .flex()
            .gap(px(6.0))
            .children((0..3).map(|i| dot((1.0 - (t - i as f32).abs()).max(0.0))))
            .into_any_element()
    }
}

impl Render for OnboardingWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_window_title(super::onboarding_window_title());
        let mut k = std::mem::take(&mut self.kit);
        let mut theme = Theme::resolve(
            self.settings.theme,
            self.settings.accent_color,
            window.appearance(),
            self.fonts.with_text(self.settings.font_family.as_deref()),
        );
        if self.backdrop.mica() {
            theme = theme.with_mica();
        }
        self.backdrop.sync(theme.dark, window.appearance());
        k.begin_frame(theme, window);
        let (heading, description, rows) = match self.step {
            Step::Providers => (
                crate::i18n::tr("choose-providers"),
                crate::i18n::tr(
                    "we-turned-on-the-providers-found-on-this-pc-you-can-change-this-l",
                ),
                self.providers_step(&mut k, cx),
            ),
            Step::General => (
                crate::i18n::tr("general-settings"),
                crate::i18n::tr("you-can-change-these-later-in-settings"),
                self.general_step(&mut k, cx),
            ),
            Step::Notifications => (
                crate::i18n::tr("notifications"),
                crate::i18n::tr(
                    "turn-off-anything-you-don-t-want-to-hear-about-you-can-change-the",
                ),
                self.notifications_step(&mut k, cx),
            ),
        };
        let theme = k.theme.clone();
        let body = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .text_size(px(28.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(heading),
            )
            .child(div().pb(px(12.0)).child(kit::caption(&k, description)))
            .children(rows)
            .into_any_element();
        let body = kit::appear(&k, format!("onboarding-{:?}", self.step), body);
        let dots = self.step_dots(&mut k);
        let back: AnyElement = match self.step {
            Step::Providers => div().into_any_element(),
            Step::General | Step::Notifications => {
                Button::new("onboarding-back", crate::i18n::tr("back"))
                    .on_click(Self::h(cx, |this, (), _, cx| {
                        this.step = match this.step {
                            Step::Notifications => Step::General,
                            _ => Step::Providers,
                        };
                        cx.notify();
                    }))
                    .render(&k)
            }
        };
        let action = match self.step {
            Step::Providers => Button::new("onboarding-continue", crate::i18n::tr("continue"))
                .accent()
                .on_click(Self::h(cx, |this, (), _, cx| {
                    this.step = Step::General;
                    cx.notify();
                })),
            Step::General => Button::new("onboarding-next", crate::i18n::tr("continue"))
                .accent()
                .on_click(Self::h(cx, |this, (), _, cx| {
                    this.step = Step::Notifications;
                    cx.notify();
                })),
            Step::Notifications => Button::new("onboarding-done", crate::i18n::tr("done"))
                .accent()
                .on_click(Self::h(cx, |this, (), window, _| this.finish(window))),
        }
        .render(&k);
        let minimize = kit::caption_button(
            &k,
            "onboarding-min",
            "\u{E921}",
            WindowControlArea::Min,
            false,
        );
        let close = kit::caption_button(
            &k,
            "onboarding-close",
            "\u{E8BB}",
            WindowControlArea::Close,
            true,
        );
        k.end_frame(window);
        self.kit = k;

        div()
            .id("onboarding-root")
            .size_full()
            .flex()
            .flex_col()
            .font_family(theme.font.clone())
            .text_color(theme.text)
            .bg(theme.window_bg)
            .child(
                div()
                    .id("onboarding-titlebar")
                    .flex()
                    .items_center()
                    .h(px(40.0))
                    .flex_none()
                    .window_control_area(WindowControlArea::Drag)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .pl(px(16.0))
                            .flex_1()
                            .child(kit::image("color/app-icon-32.png", 16.0))
                            .child(kit::text(
                                SharedString::from(super::onboarding_window_title()),
                                12.0,
                                theme.text_secondary,
                            )),
                    )
                    .child(minimize)
                    .child(close),
            )
            .child(
                // Focus is tracked below the title bar so title bar clicks
                // reach Windows (see the Settings window).
                div()
                    .id("onboarding-scroll")
                    .track_focus(&self.focus)
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(36.0))
                    .pt(px(12.0))
                    .pb(px(20.0))
                    .child(body),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(36.0))
                    .py(px(16.0))
                    .border_t_1()
                    .border_color(theme.divider)
                    .bg(theme.layer)
                    .child(div().flex_1().child(dots))
                    .child(back)
                    .child(action),
            )
    }
}
