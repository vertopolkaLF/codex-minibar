//! First-launch flow: choose providers, a theme, a few general settings,
//! then notifications.
//! Every choice is committed as it is made so the popup, shown beside the
//! window, previews it live. Dismissing the window restores what was there
//! before, so it never leaves provider workers half-configured.

use std::sync::Arc;

use gpui::{
    AnyElement, App, Context, FocusHandle, Focusable, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Window,
    WindowControlArea, div, px,
};

use super::appearance::ThemePicker;
use super::general::usage_refresh_labels;
use super::input::{HasInputs, Inputs};
use super::kit::{self, Button, Handler, Kit, Row};
use super::persistence::load_settings_for_window;
use super::theme::{Fonts, Theme};
use super::vscode_themes::{ThemeBrowser, ThemeBrowserUi, ThemeHost, end_theme_preview};
use crate::popup_window::AppState;
use crate::popup_window::ui::fx;
use crate::settings::{ProviderInstance, ProviderKind, Settings, TrayWidget, UsageRefreshInterval};

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

/// Whether each driver's first instance is enabled.
fn enabled_providers(settings: &Settings) -> [bool; DRIVERS] {
    ProviderKind::ALL.map(|driver| {
        settings
            .instances
            .iter()
            .find(|instance| instance.driver == driver)
            .is_some_and(|instance| instance.enabled)
    })
}

/// Turns each driver's first instance on or off, adding it when missing.
/// `automatic` sets auto-activation too; only Done passes it, so previewing
/// choices never starts a session.
fn apply_onboarding_choices(
    settings: &mut Settings,
    enabled: [bool; DRIVERS],
    automatic: Option<bool>,
) {
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
            if let Some(automatic) = automatic
                && crate::instances::Capabilities::of(instance).auto_activation
            {
                instance.auto_activation = automatic;
            }
        }
    }
}

/// Copies the settings onboarding edits from `from` onto `to`; providers
/// and tray widgets are handled by the callers.
fn copy_choices(from: &Settings, to: &mut Settings) {
    to.language = from.language;
    to.start_at_login = from.start_at_login;
    to.usage_refresh_interval = from.usage_refresh_interval;
    to.show_used_percentage = from.show_used_percentage;
    to.show_usage_pace = from.show_usage_pace;
    to.show_account_name = from.show_account_name;
    to.notifications = from.notifications.clone();
    to.theme = from.theme;
    to.accent_color = from.accent_color;
    to.popup_theme = from.popup_theme;
    to.popup_vscode_theme = from.popup_vscode_theme.clone();
}

/// A tray widget for every enabled provider that has default tray metrics.
fn default_tray_widgets(settings: &Settings) -> Vec<TrayWidget> {
    settings
        .enabled_providers()
        .into_iter()
        .filter(|provider| {
            !crate::provider_registry::descriptor(provider.kind())
                .default_tray_metrics
                .is_empty()
        })
        .map(TrayWidget::for_provider)
        .collect()
}

/// Puts back what onboarding changed: its choices, the providers it added
/// or toggled, and the tray widgets.
fn restore(original: &Settings, settings: &mut Settings) {
    copy_choices(original, settings);
    let added = settings
        .instances
        .iter()
        .filter(|instance| original.instance_by_id(&instance.id).is_none())
        .map(ProviderInstance::provider_id)
        .collect::<Vec<_>>();
    for provider in added {
        settings.remove_instance(provider);
    }
    for instance in &mut settings.instances {
        if let Some(before) = original.instance_by_id(&instance.id) {
            instance.enabled = before.enabled;
        }
    }
    settings.tray_widgets = original.tray_widgets.clone();
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
enum Step {
    #[default]
    Providers,
    Theme,
    General,
    Notifications,
}

impl Step {
    const COUNT: usize = 4;

    const fn index(self) -> usize {
        match self {
            Self::Providers => 0,
            Self::Theme => 1,
            Self::General => 2,
            Self::Notifications => 3,
        }
    }

    const fn previous(self) -> Self {
        match self {
            Self::Providers | Self::Theme => Self::Providers,
            Self::General => Self::Theme,
            Self::Notifications => Self::General,
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::Providers => Self::Theme,
            Self::Theme => Self::General,
            Self::General | Self::Notifications => Self::Notifications,
        }
    }
}

pub(crate) struct OnboardingWindow {
    state: Arc<AppState>,
    backdrop: super::backdrop::Backdrop,
    settings: Settings,
    /// Settings as they were when the window opened; dismissing restores them.
    original: Settings,
    /// Done was saved or the window was dismissed; nothing is published after.
    settled: bool,
    /// Done is being saved.
    finishing: bool,
    theme_browser: ThemeBrowser,
    inputs: Inputs<Self>,
    detected: [bool; DRIVERS],
    enabled: [bool; DRIVERS],
    automatic: bool,
    step: Step,
    kit: Kit,
    fonts: Fonts,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl HasInputs for OnboardingWindow {
    fn inputs(&self) -> &Inputs<Self> {
        &self.inputs
    }

    fn inputs_mut(&mut self) -> &mut Inputs<Self> {
        &mut self.inputs
    }
}

impl ThemeHost for OnboardingWindow {
    fn settings(&self) -> &Settings {
        &self.settings
    }

    fn fonts(&self) -> &Fonts {
        &self.fonts
    }

    fn theme_browser(&self) -> &ThemeBrowser {
        &self.theme_browser
    }

    fn theme_browser_mut(&mut self) -> &mut ThemeBrowser {
        &mut self.theme_browser
    }

    fn edit_settings(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl Fn(&mut Settings) + Send + 'static,
    ) {
        self.edit(cx, edit);
    }

    /// The installed theme is selected in the grid; no notice is needed.
    fn theme_installed(&mut self, cx: &mut Context<Self>) {
        cx.notify();
    }
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
        subscriptions.push(cx.on_release(|this, cx| {
            this.dismiss(cx);
            super::window_closed(true);
        }));
        let this = cx.weak_entity();
        window.on_window_should_close(cx, move |_, cx| {
            let _ = this.update(cx, |this, cx| this.dismiss(cx));
            super::window_closed(true);
            true
        });
        let focus = cx.focus_handle();
        window.focus(&focus);
        let onboarding = Self {
            state,
            backdrop: super::backdrop::Backdrop::install(window),
            original: settings.clone(),
            settings,
            settled: false,
            finishing: false,
            theme_browser: ThemeBrowser::default(),
            inputs: Inputs::default(),
            detected,
            enabled: detected,
            automatic,
            step: Step::Providers,
            kit: Kit::default(),
            fonts: Fonts::resolve(cx),
            focus,
            _subscriptions: subscriptions,
        };
        // Detected providers start on; show them in the popup right away.
        if onboarding.enabled != enabled_providers(&onboarding.settings) {
            onboarding.publish();
        }
        onboarding
    }

    /// Apply an edit locally and commit every choice so the popup follows.
    fn edit(&mut self, cx: &mut Context<Self>, edit: impl Fn(&mut Settings)) {
        edit(&mut self.settings);
        self.settings.language.apply();
        cx.refresh_windows();
        self.publish();
        cx.notify();
    }

    /// Commit the current choices (without auto-activation) for the popup.
    fn publish(&self) {
        if self.settled {
            return;
        }
        let choices = self.settings.clone();
        let enabled = self.enabled;
        super::persistence::queue(self.state.settings_tx.clone(), move |settings| {
            copy_choices(&choices, settings);
            apply_onboarding_choices(settings, enabled, None);
            settings.tray_widgets = default_tray_widgets(settings);
        });
    }

    fn set_enabled(&mut self, index: usize, value: bool, cx: &mut Context<Self>) {
        if self.enabled[index] != value {
            self.enabled[index] = value;
            self.publish();
            // Keep the popup on Home, where the change shows.
            crate::popup_window::request_home_view();
        }
        cx.notify();
    }

    /// The window closed without Done: restore the settings it found.
    fn dismiss(&mut self, cx: &mut gpui::App) {
        end_theme_preview(cx);
        // Closing while Done is saving keeps the completed setup.
        if std::mem::replace(&mut self.settled, true) || self.finishing {
            return;
        }
        let original = self.original.clone();
        super::persistence::queue(self.state.settings_tx.clone(), move |settings| {
            restore(&original, settings)
        });
    }

    fn go(&mut self, step: Step, cx: &mut Context<Self>) {
        if self.step == step {
            return;
        }
        // A theme preview belongs to the step it was started on.
        if self.step == Step::Theme {
            end_theme_preview(cx);
        }
        self.step = step;
        if step == Step::Theme {
            self.ensure_open_vsx_results(cx);
        }
        cx.notify();
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

    fn finish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settled || self.finishing {
            return;
        }
        self.finishing = true;
        end_theme_preview(cx);
        cx.notify();
        let choices = self.settings.clone();
        let (enabled, automatic) = (self.enabled, self.automatic);
        let outcome =
            super::persistence::queue_fallible(self.state.settings_tx.clone(), move |settings| {
                copy_choices(&choices, settings);
                apply_onboarding_choices(settings, enabled, Some(automatic));
                settings.tray_widgets = default_tray_widgets(settings);
                settings.onboarding_completed = true;
                Ok(())
            });
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { super::persistence::wait(outcome) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.finishing = false;
                cx.notify();
                match result {
                    Ok(()) => {
                        this.settled = true;
                        // The popup has been previewing beside onboarding;
                        // keep it up so Done always lands on it.
                        if crate::popup::is_visible() && !crate::popup::is_closing() {
                            crate::popup::arm_outside_click_grace();
                        } else {
                            crate::popup::show_near_cursor();
                        }
                        super::window_closed(true);
                        window.remove_window();
                    }
                    Err(error) => {
                        eprintln!("failed to complete onboarding: {error:#}");
                        crate::notifications::show_error(
                            crate::i18n::tr("setup-could-not-be-saved"),
                            &format!("{error:#}"),
                        );
                    }
                }
            });
        })
        .detach();
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
                    this.set_enabled(index, value, cx)
                }),
            );
            let toggle_row = Self::h(cx, move |this, (), _, cx| {
                this.set_enabled(index, !this.enabled[index], cx)
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
                        let language = crate::i18n::Language::from_index(index);
                        this.edit(cx, move |settings| settings.language = language);
                    }),
                ),
                kit::toggle_row(
                    k,
                    "onboarding-start",
                    crate::i18n::tr("start-with-windows"),
                    None,
                    s.start_at_login,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |settings| settings.start_at_login = value);
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
                        this.edit(cx, move |settings| {
                            settings.usage_refresh_interval =
                                UsageRefreshInterval::from_index(index as i32)
                        });
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
                        this.edit(cx, move |settings| settings.show_used_percentage = value);
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
                        this.edit(cx, move |settings| settings.show_usage_pace = value);
                    }),
                ),
                kit::toggle_row(
                    k,
                    "onboarding-account",
                    crate::i18n::tr("show-account-name"),
                    None,
                    s.show_account_name,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |settings| settings.show_account_name = value);
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
                        this.edit(cx, move |settings| {
                            settings.notifications.activation_success = value
                        });
                    }),
                ),
                kit::toggle_row(
                    k,
                    "onboarding-notif-activation-failure",
                    crate::i18n::tr("failed-activations"),
                    None,
                    n.activation_failure,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |settings| {
                            settings.notifications.activation_failure = value
                        });
                    }),
                ),
                kit::toggle_row(
                    k,
                    "onboarding-notif-limits-reset",
                    crate::i18n::tr("when-limits-reset"),
                    None,
                    n.limits_changed,
                    Self::h(cx, |this, value: bool, _, cx| {
                        this.edit(cx, move |settings| {
                            settings.notifications.limits_changed = value
                        });
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
                        this.edit(cx, move |settings| {
                            settings.notifications.low_usage_enabled = value
                        });
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
                        this.edit(cx, move |settings| {
                            settings.notifications.weekly_low_usage_enabled = value
                        });
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
                    this.edit(cx, move |settings| {
                        settings.notifications.update_available = value
                    });
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

    /// Color theme, accent and popup theme, with Open VSX search on the page.
    fn theme_step(
        &mut self,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let theme_cards = self.theme_cards(k, cx);
        let accent = self.settings.accent_color;
        let colors = self.accent_swatches(k, accent, cx);
        let look = kit::card_of(k, |k| {
            vec![
                Row::new("onboarding-theme", crate::i18n::tr("color-theme"))
                    .description(
                        k,
                        crate::i18n::tr("applies-to-settings-the-popup-and-its-tray-menu"),
                    )
                    .detail(div().pt(px(12.0)).child(theme_cards).into_any_element())
                    .render(k),
                Row::new("onboarding-accent", crate::i18n::tr("accent-color"))
                    .description(k, crate::i18n::tr("windows-follows-your-system-accent"))
                    .detail(div().pt(px(12.0)).child(colors).into_any_element())
                    .render(k),
            ]
        });
        let grid = self.popup_theme_grid(k, cx);
        let popup = kit::row_card(
            k,
            Row::new("onboarding-popup-theme", crate::i18n::tr("popup-theme"))
                .description(k, crate::i18n::tr("popup-theme-description"))
                .detail(div().pt(px(12.0)).child(grid).into_any_element()),
        );
        let browser = self.open_vsx_inline(k, window, cx);
        let more = kit::row_card(
            k,
            Row::new("onboarding-open-vsx", crate::i18n::tr("browse-open-vsx"))
                .description(k, crate::i18n::tr("browse-open-vsx-description"))
                .detail(div().pt(px(12.0)).child(browser).into_any_element()),
        );
        vec![
            look,
            kit::section_heading(k, crate::i18n::tr("popup")),
            popup,
            more,
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
            .children((0..Step::COUNT).map(|i| dot((1.0 - (t - i as f32).abs()).max(0.0))))
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
            Step::Theme => (
                crate::i18n::tr("choose-a-theme"),
                crate::i18n::tr("theme-step-description"),
                self.theme_step(&mut k, window, cx),
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
            Step::Theme | Step::General | Step::Notifications => {
                Button::new("onboarding-back", crate::i18n::tr("back"))
                    .on_click(Self::h(cx, |this, (), _, cx| {
                        this.go(this.step.previous(), cx)
                    }))
                    .render(&k)
            }
        };
        let action = match self.step {
            Step::Providers | Step::Theme | Step::General => {
                Button::new("onboarding-continue", crate::i18n::tr("continue"))
                    .accent()
                    .on_click(Self::h(cx, |this, (), _, cx| this.go(this.step.next(), cx)))
            }
            Step::Notifications => Button::new("onboarding-done", crate::i18n::tr("done"))
                .accent()
                .disabled(self.finishing)
                .on_click(Self::h(cx, |this, (), window, cx| this.finish(window, cx))),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::AppTheme;

    #[test]
    fn restore_undoes_previewed_choices_and_added_providers() {
        let mut original = Settings::default();
        for instance in &mut original.instances {
            instance.enabled = false;
        }
        let mut settings = original.clone();
        let mut choices = original.clone();
        choices.theme = AppTheme::Dark;
        choices.show_usage_pace = !original.show_usage_pace;
        copy_choices(&choices, &mut settings);
        apply_onboarding_choices(&mut settings, [true; DRIVERS], None);
        settings.tray_widgets = default_tray_widgets(&settings);
        assert!(ProviderKind::ALL.iter().all(|driver| {
            settings
                .instances
                .iter()
                .any(|instance| instance.driver == *driver && instance.enabled)
        }));

        restore(&original, &mut settings);

        assert_eq!(settings.theme, original.theme);
        assert_eq!(settings.show_usage_pace, original.show_usage_pace);
        assert_eq!(settings.tray_widgets, original.tray_widgets);
        let ids = |settings: &Settings| {
            settings
                .instances
                .iter()
                .map(|instance| (instance.id.clone(), instance.enabled))
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&settings), ids(&original));
    }

    #[test]
    fn previews_leave_auto_activation_alone() {
        let mut settings = Settings::default();
        let before = settings
            .instances
            .iter()
            .map(|instance| instance.auto_activation)
            .collect::<Vec<_>>();
        apply_onboarding_choices(&mut settings, [true; DRIVERS], None);
        let after = settings
            .instances
            .iter()
            .take(before.len())
            .map(|instance| instance.auto_activation)
            .collect::<Vec<_>>();
        assert_eq!(before, after);
    }
}
