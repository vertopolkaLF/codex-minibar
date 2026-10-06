//! The Settings window: shell (title bar, sidebar, content plane), live
//! state and the helpers every page uses to read and edit settings.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use gpui::{
    AnyElement, App, AppContext, Context, Entity, FocusHandle, Focusable, FontWeight,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, ScrollHandle,
    SharedString, StatefulInteractiveElement, Styled, Subscription, Task, Window,
    WindowControlArea, div, prelude::FluentBuilder, px,
};

use super::{
    input::{InputEvent, TextInput},
    kit::{self, Button, Handler, Kit, eid, icon},
    nav::{NavMode, Page, Tab, first_provider_page, provider_nav_order, shows_badge},
    persistence,
    providers::{
        OpenRouterSettingsSnapshot, ProviderDialog, ProviderInstallStatus, ProviderReadiness,
        instance_install_status, provider_readiness,
    },
    theme::{Fonts, Theme},
};
use crate::popup_window::AppState;
use crate::popup_window::ui::fx;
use crate::settings::{ProviderId, ProviderInstance, ProviderKind, Settings, TrayWidget};
use crate::updater::UpdatePhase;

pub(crate) const SIDEBAR_WIDTH: f32 = 264.0;
const TITLEBAR_HEIGHT: f32 = 44.0;
const NAV_ITEM_HEIGHT: f32 = 36.0;
const NAV_ITEM_GAP: f32 = 2.0;
const CONTENT_MAX_WIDTH: f32 = 1000.0;

type CommitHandler =
    Rc<dyn Fn(&mut SettingsWindow, String, InputEvent, &mut Window, &mut Context<SettingsWindow>)>;

/// Instance fields that drive install detection; names, badges and toggles
/// never re-run it.
type DetectionInput = (
    String,
    ProviderKind,
    Option<std::path::PathBuf>,
    Option<std::path::PathBuf>,
    Option<std::path::PathBuf>,
);

pub(crate) struct SettingsWindow {
    pub(super) state: Arc<AppState>,
    pub(super) settings: Settings,
    pub(super) kit: Kit,
    fonts: Fonts,
    focus: FocusHandle,
    // Navigation.
    pub(super) mode: NavMode,
    pub(super) root_tab: Tab,
    pub(super) return_tab: Tab,
    pub(super) page: Page,
    scroll: ScrollHandle,
    // Text inputs keyed by field id.
    inputs: HashMap<SharedString, Entity<TextInput>>,
    commits: HashMap<SharedString, CommitHandler>,
    input_subscriptions: HashMap<SharedString, Subscription>,
    // Provider pages.
    pub(super) install_statuses: HashMap<String, ProviderInstallStatus>,
    detection_inputs: Option<Vec<DetectionInput>>,
    detection_revision: u64,
    detection_task: Option<Task<()>>,
    pub(super) status_revision: u64,
    pub(super) provider_dialog: Option<ProviderDialog>,
    pub(super) openrouter: OpenRouterSettingsSnapshot,
    pub(super) discovered_bricks: BTreeMap<String, String>,
    // Expanded cards (expanders, rules, tray widgets), by id.
    pub(super) expanded: HashSet<String>,
    // Notices, live data and modal state.
    notice: Option<(SharedString, u64)>,
    notice_task: Option<Task<()>>,
    pub(super) update_phase: UpdatePhase,
    pub(super) log: SharedString,
    pub(super) streamdeck_phase: crate::streamdeck::InstallPhase,
    pub(super) troubleshoot: Option<crate::troubleshoot::ToolPickerState>,
    pub(super) tray_editing: Option<(String, usize)>,
    pub(super) removed_widget: Option<(usize, TrayWidget)>,
    pub(super) confirm_reset: bool,
    pub(super) tray_previews: super::tray::PreviewCache,
    _poll: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl Focusable for SettingsWindow {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl SettingsWindow {
    pub(crate) fn new(state: Arc<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Always reload from disk so tray/popup open paths share the same
        // live values after an earlier toggle.
        let settings = persistence::load_settings_for_window();
        let fonts = Fonts::resolve(cx);
        let page = Page::Root(Tab::General);
        let mut subscriptions = vec![cx.observe_window_appearance(window, |_, _, cx| cx.notify())];
        subscriptions.push(cx.on_release(|_, _| super::window_closed(false)));
        window.on_window_should_close(cx, |_, _| {
            super::window_closed(false);
            true
        });
        let focus = cx.focus_handle();
        window.focus(&focus);
        let updates = Arc::clone(&state.updates);
        // Update state and the log tail are polled while the window lives.
        let poll = cx.spawn(async move |this, cx| {
            loop {
                let phase = updates.snapshot();
                let alive = this
                    .update(cx, |this, cx| {
                        let mut changed = false;
                        if this.update_phase != phase {
                            this.update_phase = phase;
                            changed = true;
                        }
                        if this.page == Page::Root(Tab::Log) {
                            let log = crate::logger::tail_lines(100)
                                .unwrap_or_else(|error| error.to_string());
                            if this.log.as_ref() != log {
                                this.log = log.into();
                                changed = true;
                            }
                        }
                        if changed {
                            cx.notify();
                        }
                    })
                    .is_ok();
                if !alive {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
            }
        });
        Self {
            update_phase: state.updates.snapshot(),
            state,
            settings,
            kit: Kit::default(),
            fonts,
            focus,
            mode: NavMode::Root,
            root_tab: Tab::General,
            return_tab: Tab::General,
            page,
            scroll: ScrollHandle::new(),
            inputs: HashMap::new(),
            commits: HashMap::new(),
            input_subscriptions: HashMap::new(),
            install_statuses: HashMap::new(),
            detection_inputs: None,
            detection_revision: 0,
            detection_task: None,
            status_revision: 0,
            provider_dialog: None,
            openrouter: super::cached_openrouter_snapshot(),
            discovered_bricks: super::cached_discovered_popup_bricks(),
            expanded: HashSet::new(),
            notice: None,
            notice_task: None,
            log: SharedString::default(),
            streamdeck_phase: crate::streamdeck::InstallPhase::Idle,
            troubleshoot: None,
            tray_editing: None,
            removed_widget: None,
            confirm_reset: false,
            tray_previews: Default::default(),
            _poll: poll,
            _subscriptions: subscriptions,
        }
    }

    // ----- settings ----------------------------------------------------------

    /// Adopt a committed snapshot unless more local edits are still queued.
    pub(crate) fn apply_sync(&mut self, settings: Settings, cx: &mut Context<Self>) {
        if persistence::has_pending() {
            return;
        }
        self.settings = settings;
        cx.notify();
    }

    /// Apply an edit locally right away and queue it for the settings file.
    pub(super) fn edit(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl Fn(&mut Settings) + Send + 'static,
    ) {
        edit(&mut self.settings);
        self.settings.normalize_tray_widgets();
        self.settings.normalize_popup_visibility();
        persistence::queue(self.state.settings_tx.clone(), move |settings| {
            edit(settings)
        });
        cx.notify();
    }

    /// Edit one instance by id.
    pub(super) fn edit_instance(
        &mut self,
        cx: &mut Context<Self>,
        provider: ProviderId,
        edit: impl Fn(&mut ProviderInstance) + Send + 'static,
    ) {
        self.edit(cx, move |settings| {
            if let Some(instance) = settings.instance_mut(provider) {
                edit(instance);
                instance.normalize();
            }
        });
    }

    pub(super) fn settings_tx(&self) -> std::sync::mpsc::Sender<Settings> {
        self.state.settings_tx.clone()
    }

    /// Wrap a method into a component handler.
    pub(super) fn h<T: 'static>(
        cx: &Context<Self>,
        f: impl Fn(&mut Self, T, &mut Window, &mut Context<Self>) + 'static,
    ) -> Handler<T> {
        let this = cx.weak_entity();
        Rc::new(move |value, window, cx| {
            let _ = this.update(cx, |this, cx| f(this, value, window, cx));
        })
    }

    pub(super) fn is_expanded(&self, id: &str) -> bool {
        self.expanded.contains(id)
    }

    pub(super) fn set_expanded(&mut self, id: impl Into<String>, expanded: bool) {
        let id = id.into();
        if expanded {
            self.expanded.insert(id);
        } else {
            self.expanded.remove(&id);
        }
    }

    pub(super) fn expand_handler(cx: &Context<Self>, id: impl Into<String>) -> Handler<bool> {
        let id = id.into();
        Self::h(cx, move |this, expanded, _, cx| {
            this.set_expanded(id.clone(), expanded);
            cx.notify();
        })
    }

    // ----- inputs ------------------------------------------------------------

    /// A persistent text input for `id`, synced to `value` when it changes
    /// elsewhere. `on_commit` runs on Enter ([`InputEvent::Submit`]) and when
    /// focus leaves ([`InputEvent::Blur`]).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn input(
        &mut self,
        id: impl Into<SharedString>,
        value: &str,
        placeholder: impl Into<SharedString>,
        password: bool,
        disabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
        on_commit: impl Fn(&mut Self, String, InputEvent, &mut Window, &mut Context<Self>) + 'static,
    ) -> Entity<TextInput> {
        let id: SharedString = id.into();
        let entity = match self.inputs.get(&id) {
            Some(entity) => entity.clone(),
            None => {
                let entity = cx.new(|cx| TextInput::new(window, cx));
                let sub_id = id.clone();
                let subscription = cx.subscribe_in(
                    &entity,
                    window,
                    move |this, input, event: &InputEvent, window, cx| {
                        if matches!(event, InputEvent::Submit | InputEvent::Blur)
                            && let Some(commit) = this.commits.get(&sub_id).cloned()
                        {
                            let text = input.read(cx).text();
                            commit(this, text, *event, window, cx);
                        }
                    },
                );
                self.input_subscriptions.insert(id.clone(), subscription);
                self.inputs.insert(id.clone(), entity.clone());
                entity
            }
        };
        let placeholder = placeholder.into();
        entity.update(cx, |input, cx| {
            input.placeholder = placeholder;
            input.password = password;
            input.disabled = disabled;
            input.sync_external(value, window, cx);
        });
        self.commits.insert(id, Rc::new(on_commit));
        entity
    }

    /// Drop inputs whose id starts with `prefix` (dialog fields), so the next
    /// dialog starts empty.
    pub(super) fn reset_inputs(&mut self, prefix: &str) {
        self.inputs.retain(|id, _| !id.starts_with(prefix));
        self.commits.retain(|id, _| !id.starts_with(prefix));
        self.input_subscriptions
            .retain(|id, _| !id.starts_with(prefix));
    }

    pub(super) fn input_text(&self, id: &str, cx: &App) -> String {
        self.inputs
            .get(id)
            .map(|input| input.read(cx).text())
            .unwrap_or_default()
    }

    pub(super) fn focus_input(&self, id: &str, window: &mut Window, cx: &App) {
        if let Some(input) = self.inputs.get(id) {
            window.focus(&input.focus_handle(cx));
        }
    }

    // ----- notices -----------------------------------------------------------

    /// A transient success toast at the bottom of the content plane.
    pub(super) fn show_notice(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        let generation = self
            .notice
            .as_ref()
            .map_or(0, |(_, generation)| generation + 1);
        self.notice = Some((message.into(), generation));
        self.notice_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(3200))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this
                    .notice
                    .as_ref()
                    .is_some_and(|(_, current)| *current == generation)
                {
                    this.notice = None;
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    // ----- navigation ----------------------------------------------------------

    pub(super) fn navigate(&mut self, page: Page, cx: &mut Context<Self>) {
        if self.page == page {
            return;
        }
        self.page = page;
        self.scroll = ScrollHandle::new();
        self.kit.menus.close_silently();
        cx.notify();
    }

    fn select_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        if tab == Tab::Providers {
            self.return_tab = if self.root_tab == Tab::Providers {
                Tab::General
            } else {
                self.root_tab
            };
            self.mode = NavMode::Providers;
            let first = first_provider_page(&self.settings.instances);
            self.navigate(first, cx);
            return;
        }
        self.root_tab = tab;
        self.navigate(Page::Root(tab), cx);
    }

    pub(super) fn leave_providers(&mut self, cx: &mut Context<Self>) {
        if let Some(dialog) = self.provider_dialog.take() {
            dialog.login_control().cancel();
        }
        self.mode = NavMode::Root;
        self.root_tab = self.return_tab;
        self.navigate(Page::Root(self.return_tab), cx);
    }

    pub(super) fn select_provider(&mut self, provider: ProviderId, cx: &mut Context<Self>) {
        self.mode = NavMode::Providers;
        self.navigate(Page::Provider(provider), cx);
    }

    // ----- provider detection --------------------------------------------------

    /// Re-run install detection when an instance's driver or paths change
    /// (or a dialog asks for a fresh look after saving credentials).
    fn refresh_detection(&mut self, cx: &mut Context<Self>) {
        if self.mode != NavMode::Providers {
            return;
        }
        let inputs = self
            .settings
            .instances
            .iter()
            .map(|instance| {
                (
                    instance.id.clone(),
                    instance.driver,
                    instance.binary_path.clone(),
                    instance.kiro_crew_path.clone(),
                    instance.kiro_cli_path.clone(),
                )
            })
            .collect::<Vec<_>>();
        let revision = self.status_revision;
        if self.detection_inputs.as_ref() == Some(&inputs) && self.detection_revision == revision {
            return;
        }
        // Only a path edit resets rows to "Checking…". A credential change
        // re-detects quietly so saved keys do not flash the page.
        let previous = self.detection_inputs.replace(inputs.clone());
        self.detection_revision = revision;
        self.install_statuses
            .retain(|id, _| inputs.iter().any(|input| &input.0 == id));
        for input in &inputs {
            let unchanged = previous
                .as_ref()
                .is_some_and(|previous| previous.contains(input));
            if !unchanged || !self.install_statuses.contains_key(&input.0) {
                self.install_statuses.insert(
                    input.0.clone(),
                    ProviderInstallStatus::checking_for(input.1),
                );
            }
        }
        let instances = self.settings.instances.clone();
        self.detection_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            let statuses = cx
                .background_executor()
                .spawn(async move {
                    instances
                        .iter()
                        .map(|instance| (instance.id.clone(), instance_install_status(instance)))
                        .collect::<HashMap<_, _>>()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.install_statuses = statuses;
                cx.notify();
            });
        }));
    }

    pub(super) fn readiness(&self, provider: ProviderId) -> ProviderReadiness {
        self.install_statuses
            .get(provider.id())
            .map_or(ProviderReadiness::Checking, provider_readiness)
    }

    pub(super) fn install_status(&self, provider: ProviderId) -> ProviderInstallStatus {
        self.install_statuses
            .get(provider.id())
            .cloned()
            .unwrap_or_else(|| ProviderInstallStatus::checking_for(provider.kind()))
    }

    pub(super) fn instance(&self, provider: ProviderId) -> Option<&ProviderInstance> {
        self.settings
            .instances
            .iter()
            .find(|instance| instance.id == provider.id())
    }

    // ----- keyboard ------------------------------------------------------------

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key != "escape" {
            return;
        }
        if self.kit.menus.any_open() {
            self.kit.menus.close(window);
        } else if self.provider_dialog.is_some() {
            self.dismiss_provider_dialog(cx);
        } else if self.troubleshoot.is_some() {
            self.troubleshoot = None;
        } else if self.tray_editing.is_some() {
            self.tray_editing = None;
        } else if self.confirm_reset {
            self.confirm_reset = false;
        } else if self.mode == NavMode::Providers {
            self.leave_providers(cx);
        } else {
            return;
        }
        cx.stop_propagation();
        cx.notify();
    }

    // ----- shell ------------------------------------------------------------

    fn titlebar(&self, k: &Kit, window: &Window) -> AnyElement {
        let theme = &k.theme;
        let caption =
            |id: &'static str, glyph: &'static str, area: WindowControlArea, close: bool| {
                let hover = if close {
                    crate::popup_window::ui::theme::rgb8((0xC4, 0x2B, 0x1C))
                } else {
                    theme.subtle_hover
                };
                div()
                    .id(id)
                    .w(px(46.0))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(10.0))
                    .font_family(theme.icon_font.clone())
                    .text_color(theme.text)
                    .window_control_area(area)
                    .hover(move |style| {
                        let style = style.bg(hover);
                        if close {
                            style.text_color(gpui::white())
                        } else {
                            style
                        }
                    })
                    .child(glyph)
            };
        let maximize_glyph = if window.is_maximized() {
            "\u{E923}"
        } else {
            "\u{E922}"
        };
        div()
            .id("settings-titlebar")
            .flex()
            .items_center()
            .h(px(TITLEBAR_HEIGHT))
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
                        super::SETTINGS_WINDOW_TITLE,
                        12.0,
                        theme.text_secondary,
                    )),
            )
            .child(caption(
                "caption-min",
                "\u{E921}",
                WindowControlArea::Min,
                false,
            ))
            .child(caption(
                "caption-max",
                maximize_glyph,
                WindowControlArea::Max,
                false,
            ))
            .child(caption(
                "caption-close",
                "\u{E8BB}",
                WindowControlArea::Close,
                true,
            ))
            .into_any_element()
    }

    fn nav_item(
        &self,
        k: &Kit,
        id: SharedString,
        leading: AnyElement,
        label: SharedString,
        selected: bool,
        dimmed: bool,
        trailing: Vec<AnyElement>,
        on_click: Handler<()>,
    ) -> gpui::Stateful<gpui::Div> {
        let theme = &k.theme;
        let hover = theme.subtle_hover;
        div()
            .id(eid(id))
            .flex()
            .items_center()
            .gap(px(12.0))
            .h(px(NAV_ITEM_HEIGHT))
            .px(px(14.0))
            .rounded(px(5.0))
            .cursor_pointer()
            .when(selected, |el| el.bg(hover))
            .hover(move |style| style.bg(hover))
            .on_click(move |_, window, cx| on_click((), window, cx))
            .child(div().when(dimmed, |el| el.opacity(0.55)).child(leading))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(14.0))
                    .text_color(if dimmed {
                        theme.text_secondary
                    } else {
                        theme.text
                    })
                    .child(label),
            )
            .children(trailing)
    }

    /// The moving selection pill, positioned by item index.
    fn nav_indicator(k: &mut Kit, key: &str, index: Option<usize>) -> Option<AnyElement> {
        let index = index?;
        let pitch = NAV_ITEM_HEIGHT + NAV_ITEM_GAP;
        let target = index as f32 * pitch;
        let id = fx::key(("nav-pill", key));
        let (top, height) = k.fx.indicator(id, target, 16.0, fx::NORMAL);
        Some(
            div()
                .absolute()
                .left(px(0.0))
                .top(px(top + (NAV_ITEM_HEIGHT - height) / 2.0))
                .w(px(3.0))
                .h(px(height))
                .rounded_full()
                .bg(k.theme.accent)
                .into_any_element(),
        )
    }

    fn root_sidebar(&mut self, k: &mut Kit, cx: &mut Context<Self>) -> AnyElement {
        let colored = self.settings.use_colored_sidebar_icons;
        let glyph = k.theme.glyph();
        let selected_index = Tab::ALL.iter().position(|tab| *tab == self.root_tab);
        let mut list = div().relative().flex().flex_col().gap(px(NAV_ITEM_GAP));
        for tab in Tab::ALL {
            let leading = if colored {
                kit::image(tab.color_icon(), 18.0).into_any_element()
            } else {
                icon(tab.mono_icon(), 16.0, glyph).into_any_element()
            };
            let trailing = if tab == Tab::Providers {
                vec![icon("caret-right", 12.0, k.theme.text_secondary).into_any_element()]
            } else {
                Vec::new()
            };
            let on_click = Self::h(cx, move |this, (), _, cx| this.select_tab(tab, cx));
            list = list.child(self.nav_item(
                k,
                format!("nav-{}", tab.tag()).into(),
                leading,
                tab.label().into(),
                self.root_tab == tab,
                false,
                trailing,
                on_click,
            ));
        }
        list = list.children(Self::nav_indicator(k, "root", selected_index));
        let footer = match &self.update_phase {
            UpdatePhase::Available(update) => Some(self.update_card(k, update.version.clone())),
            _ => None,
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                div()
                    .id("sidebar-root-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(8.0))
                    .pt(px(4.0))
                    .child(list),
            )
            .children(footer.map(|footer| div().p(px(12.0)).child(footer)))
            .into_any_element()
    }

    fn update_card(&self, k: &Kit, version: String) -> AnyElement {
        let theme = &k.theme;
        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .p(px(14.0))
            .rounded(px(kit::CARD_RADIUS))
            .border_1()
            .border_color(theme.card_stroke)
            .bg(theme.accent_soft)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(icon("download-simple-fill", 14.0, theme.accent_text))
                    .child(kit::text(
                        format!("{version} is available"),
                        13.0,
                        theme.text,
                    )),
            )
            .child(
                Button::new("sidebar-update", "Update now")
                    .accent()
                    .full_width()
                    .on_click(kit::handler(|(), _, _| install_update()))
                    .render(k),
            )
            .into_any_element()
    }

    fn providers_sidebar(&mut self, k: &mut Kit, cx: &mut Context<Self>) -> AnyElement {
        let instances = self.settings.instances.clone();
        let (enabled, disabled) = provider_nav_order(&instances);
        let selected = match self.page {
            Page::Provider(provider) => Some(provider),
            _ => None,
        };
        let theme = k.theme.clone();
        let ordered: Vec<&ProviderInstance> =
            enabled.iter().chain(disabled.iter()).copied().collect();
        let selected_index = selected.and_then(|provider| {
            ordered
                .iter()
                .position(|instance| instance.id == provider.id())
        });
        // The pill tracks items; the divider between groups adds an offset.
        let divider_after = (!enabled.is_empty() && !disabled.is_empty()).then_some(enabled.len());
        let mut list = div().relative().flex().flex_col().gap(px(NAV_ITEM_GAP));
        for (index, instance) in ordered.iter().enumerate() {
            if divider_after == Some(index) {
                list = list.child(
                    div()
                        .h(px(NAV_ITEM_HEIGHT))
                        .flex()
                        .items_center()
                        .px(px(14.0))
                        .child(kit::text("Off", 12.0, theme.text_tertiary)),
                );
            }
            list = list.child(self.provider_nav_item(k, instance, &instances, selected, cx));
        }
        let pill_index = selected_index.map(|index| match divider_after {
            Some(after) if index >= after => index + 1,
            _ => index,
        });
        list = list.children(Self::nav_indicator(k, "providers", pill_index));

        let back = Self::h(cx, |this, (), _, cx| this.leave_providers(cx));
        let back_enabled = self.provider_dialog.is_none();
        let header = div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .px(px(8.0))
            .pb(px(8.0))
            .child(
                Button::icon_only("providers-back", "caret-left")
                    .tooltip("Back")
                    .disabled(!back_enabled)
                    .on_click(back)
                    .render(k),
            )
            .child(
                div()
                    .text_size(px(16.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Providers"),
            );
        let add = Self::h(cx, |this, (), window, cx| {
            this.open_provider_dialog(ProviderDialog::add_instance(), window, cx)
        });
        let footer = div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .p(px(12.0))
            .child(
                Button::new("providers-add", "Add provider")
                    .accent()
                    .with_icon("plus-bold")
                    .full_width()
                    .on_click(add)
                    .render(k),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .p(px(12.0))
                    .rounded(px(kit::CARD_RADIUS))
                    .border_1()
                    .border_color(theme.card_stroke)
                    .bg(theme.card)
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Missing a provider?"),
                    )
                    .child(kit::caption(k, "Request support or send a pull request."))
                    .child(
                        div()
                            .flex()
                            .gap(px(4.0))
                            .child(
                                Button::new("missing-issue", "Issue")
                                    .link()
                                    .on_click(kit::handler(|(), _, _| {
                                        open_url(crate::updater::PROVIDER_REQUEST_ISSUE_URL)
                                    }))
                                    .render(k),
                            )
                            .child(
                                Button::new("missing-pr", "Pull request")
                                    .link()
                                    .on_click(kit::handler(|(), _, _| {
                                        open_url(crate::updater::CONTRIBUTING_URL)
                                    }))
                                    .render(k),
                            ),
                    ),
            );
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(header)
            .child(
                div()
                    .id("sidebar-providers-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(8.0))
                    .child(list),
            )
            .child(footer)
            .into_any_element()
    }

    fn provider_nav_item(
        &self,
        k: &Kit,
        instance: &ProviderInstance,
        instances: &[ProviderInstance],
        selected: Option<ProviderId>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = &k.theme;
        let provider = instance.provider_id();
        let color = if instance.enabled {
            theme.brand(instance.driver)
        } else {
            theme.glyph()
        };
        let mut trailing = Vec::new();
        if shows_badge(instance, instances) {
            trailing.push(kit::badge_plate(k, &instance.badge()));
        }
        if instance.enabled {
            if instance.driver == ProviderKind::OpenRouter {
                let keys = instance
                    .openrouter
                    .as_ref()
                    .map_or(0, |account| account.api_key_ids.len());
                if keys > 0 {
                    trailing.push(
                        div()
                            .px(px(6.0))
                            .rounded_full()
                            .bg(theme.accent)
                            .text_size(px(11.0))
                            .text_color(theme.on_accent)
                            .child(keys.to_string())
                            .into_any_element(),
                    );
                }
            }
            match self.readiness(provider) {
                ProviderReadiness::Ready => trailing.push(kit::status_dot(theme.success)),
                ProviderReadiness::NeedsSetup => trailing.push(kit::status_dot(theme.caution)),
                ProviderReadiness::Checking => {}
            }
        }
        let on_click = Self::h(cx, move |this, (), _, cx| {
            this.select_provider(provider, cx);
        });
        let item = self.nav_item(
            k,
            format!("nav-provider-{}", instance.id).into(),
            icon(crate::provider_registry::icon(instance.driver), 16.0, color).into_any_element(),
            instance.display_name().into(),
            selected == Some(provider),
            !instance.enabled,
            trailing,
            on_click,
        );
        // Drag within the same group to reorder.
        let dragged = DraggedProvider {
            id: instance.id.clone(),
            enabled: instance.enabled,
            label: instance.display_name().into(),
            theme: theme.clone(),
        };
        let target_id = instance.id.clone();
        let target_enabled = instance.enabled;
        let drop_color = theme.accent_soft;
        item.on_drag(dragged, |dragged, _, _, cx| cx.new(|_| dragged.clone()))
            .drag_over::<DraggedProvider>(move |style, dragged, _, _| {
                if dragged.enabled == target_enabled {
                    style.bg(drop_color)
                } else {
                    style
                }
            })
            .on_drop(cx.listener(move |this, dragged: &DraggedProvider, _, cx| {
                this.reorder_provider(&dragged.id, &target_id, cx);
            }))
            .into_any_element()
    }

    fn reorder_provider(&mut self, from: &str, to: &str, cx: &mut Context<Self>) {
        if from == to {
            return;
        }
        let (from, to) = (from.to_owned(), to.to_owned());
        self.edit(cx, move |settings| {
            let (Some(from), Some(to)) = (
                settings.resolve_provider(&from),
                settings.resolve_provider(&to),
            ) else {
                return;
            };
            let enabled = settings.is_enabled(from);
            if settings.is_enabled(to) != enabled {
                return;
            }
            let group: Vec<_> = settings
                .provider_ids()
                .into_iter()
                .filter(|provider| settings.is_enabled(*provider) == enabled)
                .collect();
            settings.reorder_providers(from, to, &group);
        });
    }

    fn page_body(
        &mut self,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        match self.page {
            Page::Root(tab) => {
                let (title, mut rows) = match tab {
                    Tab::General => ("General", self.general_page(k, window, cx)),
                    Tab::Appearance => ("Appearance", self.appearance_page(k, cx)),
                    Tab::Popup => ("Customize", self.customize_page(k, cx)),
                    Tab::Schedule => ("Limit activation", self.activation_page(k, cx)),
                    Tab::Tray => ("Tray", self.tray_page(k, window, cx)),
                    Tab::Notifications => ("Notifications", self.notifications_page(k, cx)),
                    Tab::Advanced => ("Advanced", self.advanced_page(k, cx)),
                    Tab::Log => ("Log", self.log_page(k, cx)),
                    Tab::Integrations => ("Integrations", self.integrations_page(k, cx)),
                    Tab::About => ("", self.about_page(k, cx)),
                    Tab::Providers => ("Providers", Vec::new()),
                };
                if !title.is_empty() {
                    rows.insert(0, kit::page_title(k, title));
                }
                rows
            }
            Page::Provider(provider) if self.instance(provider).is_some() => {
                self.provider_page(provider, k, window, cx)
            }
            Page::Provider(_) | Page::NoProviders => self.no_providers_page(k, cx),
        }
    }

    fn content(&mut self, k: &mut Kit, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let page_key = self.page.key();
        let rows = self.page_body(k, window, cx);
        let body = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .w_full()
            .max_w(px(CONTENT_MAX_WIDTH))
            .children(rows);
        let body = kit::appear(k, format!("{page_key}-appear"), body.into_any_element());
        let scroller = div()
            .id(eid(format!("{page_key}-scroll")))
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(
                div()
                    .flex()
                    .justify_center()
                    .px(px(36.0))
                    .pt(px(28.0))
                    .pb(px(40.0))
                    .child(body),
            );
        let notice = self.notice.as_ref().map(|(message, generation)| {
            kit::appear(
                k,
                format!("notice-{generation}"),
                div()
                    .shadow_lg()
                    .rounded(px(kit::CARD_RADIUS))
                    .child(kit::info_bar(k, kit::Severity::Success, message.clone()))
                    .into_any_element(),
            )
        });
        div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .rounded_tl(px(10.0))
            .border_t_1()
            .border_l_1()
            .border_color(k.theme.layer_stroke)
            .bg(k.theme.layer)
            .overflow_hidden()
            .child(scroller)
            .children(kit::scrollbar(k, &self.scroll))
            .children(notice.map(|notice| {
                div()
                    .absolute()
                    .bottom(px(20.0))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(div().w_full().max_w(px(520.0)).px(px(24.0)).child(notice))
            }))
            .into_any_element()
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut k = std::mem::take(&mut self.kit);
        let theme = Theme::resolve(
            self.settings.theme,
            self.settings.accent_color,
            window.appearance(),
            self.fonts.clone(),
        );
        k.begin_frame(theme);
        self.refresh_detection(cx);
        // A deleted instance must not stay selected.
        if let Page::Provider(provider) = self.page
            && self.instance(provider).is_none()
        {
            self.page = first_provider_page(&self.settings.instances);
        }

        let titlebar = self.titlebar(&k, window);
        let sidebar_mode = self.mode;
        let sidebar = match sidebar_mode {
            NavMode::Root => self.root_sidebar(&mut k, cx),
            NavMode::Providers => self.providers_sidebar(&mut k, cx),
        };
        let sidebar = kit::appear_fill(&k, format!("sidebar-{sidebar_mode:?}"), sidebar);
        let content = self.content(&mut k, window, cx);
        let mut overlays: Vec<AnyElement> = Vec::new();
        if let Some(overlay) = self.provider_dialog_overlay(&mut k, window, cx) {
            overlays.push(overlay);
        }
        if let Some(overlay) = self.troubleshoot_overlay(&k, cx) {
            overlays.push(overlay);
        }
        if let Some(overlay) = self.tray_editor_overlay(&mut k, window, cx) {
            overlays.push(overlay);
        }
        if let Some(overlay) = self.reset_confirm_overlay(&k, cx) {
            overlays.push(overlay);
        }
        k.end_frame(window);
        let theme = k.theme.clone();
        self.kit = k;

        div()
            .id("settings-root")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .font_family(theme.font.clone())
            .text_color(theme.text)
            .bg(theme.window_bg)
            .child(titlebar)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .w(px(SIDEBAR_WIDTH))
                            .flex_none()
                            .h_full()
                            .pb(px(4.0))
                            .child(sidebar),
                    )
                    .child(content),
            )
            .children(overlays)
    }
}

/// Drag payload for reordering providers in the sidebar.
#[derive(Clone)]
struct DraggedProvider {
    id: String,
    enabled: bool,
    label: SharedString,
    theme: Theme,
}

impl Render for DraggedProvider {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .font_family(self.theme.font.clone())
            .px(px(12.0))
            .py(px(6.0))
            .rounded(px(5.0))
            .border_1()
            .border_color(self.theme.popover_stroke)
            .bg(self.theme.popover)
            .text_size(px(13.0))
            .text_color(self.theme.text)
            .shadow_md()
            .child(self.label.clone())
    }
}

pub(super) fn open_url(url: &str) {
    if let Err(error) = crate::updater::open_url(url) {
        eprintln!("failed to open {url}: {error:#}");
    }
}

pub(super) fn install_update() {
    std::thread::spawn(|| {
        if let Err(error) = crate::updater::apply_pending_update() {
            eprintln!("failed to apply update: {error:#}");
            crate::notifications::show("Update failed", &format!("{error:#}"));
        }
    });
}
