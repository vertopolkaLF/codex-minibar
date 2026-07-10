//! Settings-window entry point.
//!
//! The host is exposed here so callers do not depend on popup implementation
//! details; both surfaces share tokens from [`crate::theme`].

use crate::settings::Settings;
use crate::theme::SETTINGS_CONTENT_FILL;
use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};
use windows_reactor::*;

const WINDOW_WIDTH: f64 = 760.0;
const WINDOW_HEIGHT: f64 = 520.0;

thread_local! {
    static HOST: RefCell<Option<Rc<ReactorHost>>> = const { RefCell::new(None) };
}

pub fn open(settings: Arc<Settings>) -> windows_core::Result<()> {
    HOST.with(|slot| {
        if let Some(host) = slot.borrow().as_ref() {
            return host.activate();
        }
        let view_settings = Arc::clone(&settings);
        let host = Rc::new(ReactorHost::new_with_window_options(
            "Codex Minibar Settings",
            Some(WindowSize {
                width: WINDOW_WIDTH,
                height: WINDOW_HEIGHT,
            }),
            InnerConstraints {
                min_width: Some(560.0),
                min_height: Some(400.0),
                max_width: None,
                max_height: None,
            },
            Box::new(move |_: &(), cx: &mut RenderCx| render(cx, Arc::clone(&view_settings))),
            |_| {},
        )?);
        host.set_backdrop(Backdrop::Mica);
        host.activate()?;
        *slot.borrow_mut() = Some(host);
        Ok(())
    })
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Tab {
    #[default]
    General,
    Tray,
    Notifications,
    Advanced,
}

impl Tab {
    fn tag(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Tray => "tray",
            Self::Notifications => "notifications",
            Self::Advanced => "advanced",
        }
    }

    fn from_tag(tag: &str) -> Self {
        match tag {
            "tray" => Self::Tray,
            "notifications" => Self::Notifications,
            "advanced" => Self::Advanced,
            _ => Self::General,
        }
    }
}

/// Root content for the independent WinUI settings window.
pub fn render(cx: &mut RenderCx, settings: Arc<Settings>) -> Element {
    let (selected, set_selected) = cx.use_state(Tab::default());
    let (rendered_tab, set_rendered_tab) = cx.use_async_state(Tab::default());
    let (page_visible, set_page_visible) = cx.use_async_state(true);

    let navigation = NavigationView::new(
        [
            NavViewItem::new("General")
                .tag("general")
                .icon(Symbol::Home),
            NavViewItem::new("Tray").tag("tray").icon(Symbol::More),
            NavViewItem::new("Notifications")
                .tag("notifications")
                .icon(Symbol::Flag),
            NavViewItem::new("Advanced")
                .tag("advanced")
                .icon(Symbol::Edit),
        ],
        Element::Empty,
    )
    .selected_tag(selected.tag())
    .on_selection_changed({
        let set_rendered_tab = set_rendered_tab.clone();
        let set_page_visible = set_page_visible.clone();
        move |tag: String| {
            let next = Tab::from_tag(&tag);
            if next != selected {
                set_page_visible.call(false);
                set_selected.call(next);
                let set_rendered_tab = set_rendered_tab.clone();
                let set_page_visible = set_page_visible.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(180));
                    set_rendered_tab.call(next);
                    set_page_visible.call(true);
                });
            }
        }
    })
    .pane_display_mode(NavigationViewPaneDisplayMode::Left)
    .pane_open(true)
    .open_pane_length(220.0)
    .settings_visible(false)
    .back_button_visible(false)
    .pane_toggle_button_visible(false)
    .background(Color::transparent())
    .width(220.0)
    .horizontal_alignment(HorizontalAlignment::Left)
    .vertical_alignment(VerticalAlignment::Stretch);

    let page_content = border(
        border(tab_content(&settings, rendered_tab))
            .with_key(format!("settings-page-{}", rendered_tab.tag()))
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Stretch),
    )
    .opacity(if page_visible { 1.0 } else { 0.0 })
    .with_opacity_transition(Duration::from_millis(180))
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Stretch);

    let page = border(page_content)
        .padding(Thickness {
            left: 32.0,
            top: 24.0,
            right: 32.0,
            bottom: 32.0,
        })
        .background(SETTINGS_CONTENT_FILL)
        .corner_radii(CornerRadii {
            top_left: 12.0,
            ..Default::default()
        })
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Stretch);

    let title_bar = TitleBar::new("Codex Minibar Settings")
        .back_button_visible(false)
        .pane_toggle_button_visible(false);
    let shell = grid((navigation.grid_column(0), page.grid_column(1)))
        .columns([GridLength::Pixel(220.0), GridLength::Star(1.0)])
        .rows([GridLength::Star(1.0)])
        .background(Color::transparent());
    grid((title_bar.grid_row(0), shell.grid_row(1)))
        .rows([GridLength::Auto, GridLength::Star(1.0)])
        .columns([GridLength::Star(1.0)])
        .background(Color::transparent())
        .into()
}

fn tab_content(settings: &Settings, tab: Tab) -> Element {
    let (title, subtitle, rows) = match tab {
        Tab::General => (
            "General",
            "Core behavior for Codex Minibar.",
            vec![
                row(
                    "Automatic activation",
                    on_off(settings.automatic_activation),
                ),
                row("Start at sign-in", on_off(settings.start_at_login)),
                row("Check for updates", on_off(settings.check_for_updates)),
            ],
        ),
        Tab::Tray => (
            "Tray",
            "Choose what Codex Minibar shows in the notification area.",
            vec![row(
                "Active tray widgets",
                format!("{} configured", settings.tray_widgets.len()),
            )],
        ),
        Tab::Notifications => (
            "Notifications",
            "Decide which important events deserve your attention.",
            vec![
                row(
                    "Activation failures",
                    on_off(settings.notifications.activation_failure),
                ),
                row(
                    "Codex unavailable",
                    on_off(settings.notifications.codex_unavailable),
                ),
                row(
                    "Activation successes",
                    on_off(settings.notifications.activation_success),
                ),
            ],
        ),
        Tab::Advanced => (
            "Advanced",
            "Storage and integration settings that should stay out of the way.",
            vec![
                row(
                    "History retention",
                    format!("{} days", settings.history_retention_days),
                ),
                row(
                    "Codex executable",
                    settings
                        .codex_path
                        .as_ref()
                        .map_or("Automatic".into(), |path| path.display().to_string()),
                ),
            ],
        ),
    };
    vstack((
        text_block(title).font_size(28.0).bold(),
        text_block(subtitle).foreground(ThemeRef::SecondaryText),
        vstack(rows).spacing(8.0),
    ))
    .spacing(10.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .vertical_alignment(VerticalAlignment::Top)
    .into()
}

fn on_off(value: bool) -> &'static str {
    if value { "On" } else { "Off" }
}

fn row(label: impl Into<String>, value: impl Into<String>) -> Element {
    border(
        grid((
            text_block(label)
                .grid_column(0)
                .vertical_alignment(VerticalAlignment::Center),
            text_block(value)
                .foreground(ThemeRef::SecondaryText)
                .grid_column(1)
                .horizontal_alignment(HorizontalAlignment::Right)
                .vertical_alignment(VerticalAlignment::Center),
        ))
        .columns([GridLength::Star(1.0), GridLength::Auto])
        .rows([GridLength::Auto])
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(Thickness {
        left: 12.0,
        top: 10.0,
        right: 12.0,
        bottom: 10.0,
    })
    .background(ThemeRef::CardBackground)
    .corner_radius(6.0)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}
