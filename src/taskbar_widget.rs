//! Compact limit surface embedded into the Windows taskbar.
//!
//! The host follows the same shell-integration model as FluentFlyout: a normal
//! app window is converted to `WS_CHILD`, parented to Explorer's taskbar HWND,
//! expanded to the taskbar client, and clipped to the widget's rounded hit
//! region. The XAML content remains owned by windows-reactor.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicIsize, Ordering},
    },
    time::Duration,
};

use chrono::{DateTime, Utc};
use windows_reactor::*;

use crate::{
    limits::{LimitWindow, ProviderLimits, RateLimits},
    popup_window::AppState,
    settings::{
        ProviderKind, Settings, TaskbarSectionPresentation, TaskbarWidgetSection,
        TaskbarWidgetSectionKind,
    },
};

pub const WINDOW_TITLE: &str = "Codex Minibar Taskbar Widget";

const WIDGET_HEIGHT_DIP: f64 = 40.0;
const CONTENT_PADDING_DIP: f64 = 6.0;
const CONTENT_GAP_DIP: f64 = 8.0;
const ICON_SIZE_DIP: f64 = 14.0;
const BAR_WIDTH_DIP: f64 = 44.0;
const VERTICAL_BAR_WIDTH_DIP: f64 = 4.0;
const VERTICAL_BAR_HEIGHT_DIP: f64 = 22.0;
const RING_SIZE_DIP: f64 = 26.0;
const LAYOUT_REFRESH: Duration = Duration::from_millis(1500);
const DATA_REFRESH: Duration = Duration::from_millis(250);

thread_local! {
    static HOST: RefCell<Option<Rc<ReactorHost>>> = const { RefCell::new(None) };
    static HOST_ACTIVATED: Cell<bool> = const { Cell::new(false) };
}
static HWND_BITS: AtomicIsize = AtomicIsize::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskbarSectionStyle {
    Chip,
    Bar,
    VerticalBar,
    Ring,
    Clock,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskbarSectionView {
    pub id: String,
    pub icon: &'static str,
    pub brand_rgb: (u8, u8, u8),
    pub title: String,
    pub value: String,
    pub progress: Option<f64>,
    pub style: TaskbarSectionStyle,
    pub show_ring_text: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskbarWidgetSnapshot {
    pub enabled: bool,
    pub sections: Vec<TaskbarWidgetSection>,
    pub show_used_percentage: bool,
    pub use_colored_provider_icons: bool,
    pub limits: ProviderLimits,
    pub system_uses_light_theme: bool,
    pub clock_minute: i64,
}

impl TaskbarWidgetSnapshot {
    pub fn from_settings(settings: &Settings, limits: ProviderLimits) -> Self {
        Self {
            enabled: settings.taskbar_widget_enabled,
            sections: settings.resolved_taskbar_sections(),
            show_used_percentage: settings.show_used_percentage,
            use_colored_provider_icons: settings.use_colored_provider_icons,
            limits,
            system_uses_light_theme: crate::tray::system_uses_light_theme(),
            clock_minute: Utc::now().timestamp() / 60,
        }
    }

    pub fn replace_limits(&mut self, limits: ProviderLimits) {
        self.limits = limits;
    }

    pub fn refresh_system_theme(&mut self) -> bool {
        let next = crate::tray::system_uses_light_theme();
        if next == self.system_uses_light_theme {
            return false;
        }
        self.system_uses_light_theme = next;
        true
    }

    pub fn rendered_sections(&self, now: DateTime<Utc>) -> Vec<TaskbarSectionView> {
        render_sections(&self.sections, &self.limits, self.show_used_percentage, now)
    }
}

pub fn render_sections(
    sections: &[TaskbarWidgetSection],
    limits: &ProviderLimits,
    show_used: bool,
    now: DateTime<Utc>,
) -> Vec<TaskbarSectionView> {
    sections
        .iter()
        .filter_map(|section| render_section(section, limits, show_used, now))
        .collect()
}

fn render_section(
    section: &TaskbarWidgetSection,
    limits: &ProviderLimits,
    show_used: bool,
    now: DateTime<Utc>,
) -> Option<TaskbarSectionView> {
    let icon_provider = section.provider().unwrap_or(ProviderKind::Codex);
    let descriptor = crate::provider_registry::descriptor(icon_provider);
    let style = section_style(section.presentation);
    let (title, mut value, progress) = match section.kind {
        TaskbarWidgetSectionKind::Session => {
            quota_view(limits.get(icon_provider), true, show_used)?
        }
        TaskbarWidgetSectionKind::Weekly => {
            quota_view(limits.get(icon_provider), false, show_used)?
        }
        TaskbarWidgetSectionKind::Reset => {
            let window = &limits.get(icon_provider).primary;
            (
                "Reset".into(),
                format_reset_in(window.resets_at, now),
                reset_progress(window, now),
            )
        }
        TaskbarWidgetSectionKind::Credits => (
            "Credits".into(),
            credits_value(limits.get(icon_provider)).unwrap_or_else(|| "—".into()),
            None,
        ),
        TaskbarWidgetSectionKind::TodaySpend => {
            let spend = if section.provider().is_some() {
                limits.get(icon_provider).usage.today.estimated_cost_microusd
            } else {
                ProviderKind::ALL
                    .into_iter()
                    .map(|provider| limits.get(provider).usage.today.estimated_cost_microusd)
                    .sum()
            };
            (
                "Spend".into(),
                format_spend(spend),
                spend_progress(limits, section.provider(), spend),
            )
        }
        TaskbarWidgetSectionKind::TodayTokens => {
            let tokens = if section.provider().is_some() {
                limits.get(icon_provider).usage.today.total_tokens()
            } else {
                ProviderKind::ALL
                    .into_iter()
                    .map(|provider| limits.get(provider).usage.today.total_tokens())
                    .sum()
            };
            ("Tokens".into(), format_tokens(tokens), None)
        }
    };
    if section.presentation == TaskbarSectionPresentation::Clock {
        let window = match section.kind {
            TaskbarWidgetSectionKind::Session | TaskbarWidgetSectionKind::Reset => {
                Some(&limits.get(icon_provider).primary)
            }
            TaskbarWidgetSectionKind::Weekly => Some(&limits.get(icon_provider).secondary),
            _ => None,
        };
        if let Some(window) = window {
            value = format_reset_in(window.resets_at, now);
        }
    }

    Some(TaskbarSectionView {
        id: section.id.clone(),
        icon: descriptor.icon,
        brand_rgb: descriptor.brand_rgb,
        title,
        value,
        progress,
        style,
        show_ring_text: section.presentation.is_ring() && section.show_ring_text,
    })
}

fn quota_view(
    limits: &RateLimits,
    session: bool,
    show_used: bool,
) -> Option<(String, String, Option<f64>)> {
    let window = if session {
        &limits.primary
    } else {
        &limits.secondary
    };
    let title = if session { "5h" } else { "Wk" };
    let percent = if show_used {
        window.used_percent
    } else {
        window.remaining_percent()
    };
    let value = percent
        .map(|value| format!("{value}%"))
        .unwrap_or_else(|| "—".into());
    Some((title.into(), value, percent.map(f64::from)))
}

fn section_style(presentation: TaskbarSectionPresentation) -> TaskbarSectionStyle {
    match presentation {
        TaskbarSectionPresentation::Number => TaskbarSectionStyle::Chip,
        TaskbarSectionPresentation::ProgressBar => TaskbarSectionStyle::Bar,
        TaskbarSectionPresentation::VerticalBar => TaskbarSectionStyle::VerticalBar,
        TaskbarSectionPresentation::Ring => TaskbarSectionStyle::Ring,
        TaskbarSectionPresentation::Clock => TaskbarSectionStyle::Clock,
    }
}

fn style_tag(style: TaskbarSectionStyle) -> &'static str {
    match style {
        TaskbarSectionStyle::Chip => "chip",
        TaskbarSectionStyle::Bar => "bar",
        TaskbarSectionStyle::VerticalBar => "vbar",
        TaskbarSectionStyle::Ring => "ring",
        TaskbarSectionStyle::Clock => "clock",
    }
}

fn reset_progress(window: &LimitWindow, now: DateTime<Utc>) -> Option<f64> {
    let reset = window.resets_at?;
    let duration_minutes = window.duration_minutes.filter(|minutes| *minutes > 0)?;
    let remaining = (reset - now).num_seconds().max(0) as f64;
    let total = f64::from(duration_minutes) * 60.0;
    Some(((remaining / total) * 100.0).clamp(0.0, 100.0))
}

fn spend_progress(
    limits: &ProviderLimits,
    provider: Option<ProviderKind>,
    used_microusd: u64,
) -> Option<f64> {
    let limit = match provider {
        Some(provider) => limits.get(provider).spending.as_ref()?.limit_microusd?,
        None => ProviderKind::ALL
            .into_iter()
            .filter_map(|provider| limits.get(provider).spending.as_ref()?.limit_microusd)
            .sum::<u64>(),
    };
    if limit == 0 {
        return None;
    }
    Some(((used_microusd as f64 / limit as f64) * 100.0).clamp(0.0, 100.0))
}

fn format_reset_in(reset: Option<DateTime<Utc>>, now: DateTime<Utc>) -> String {
    let Some(reset) = reset else {
        return "—".into();
    };
    let remaining_minutes = (reset - now).num_minutes().max(0);
    let days = remaining_minutes / 1_440;
    let hours = (remaining_minutes % 1_440) / 60;
    let minutes = remaining_minutes % 60;
    if days > 0 {
        if hours > 0 {
            format!("{days}d {hours}h")
        } else {
            format!("{days}d")
        }
    } else if hours > 0 {
        if minutes > 0 {
            format!("{hours}h {minutes}m")
        } else {
            format!("{hours}h")
        }
    } else {
        format!("{minutes}m")
    }
}

fn format_spend(microusd: u64) -> String {
    let value = microusd as f64 / 1_000_000.0;
    if value >= 1_000.0 {
        format!("${:.1}K", value / 1_000.0)
    } else if microusd == 0 {
        "$0".into()
    } else {
        format!("${value:.2}")
    }
}

fn format_tokens(tokens: u64) -> String {
    match tokens {
        0..=999 => tokens.to_string(),
        1_000..=999_999 => format!("{:.1}K", tokens as f64 / 1_000.0),
        _ => format!("{:.1}M", tokens as f64 / 1_000_000.0),
    }
}

fn credits_value(limits: &RateLimits) -> Option<String> {
    if limits.credits.unlimited {
        return Some("Unlimited".into());
    }
    if !limits.credits.has_credits {
        return None;
    }
    let balance = limits.credits.balance.as_deref()?.trim();
    if balance.is_empty() {
        None
    } else {
        Some(balance.into())
    }
}

fn section_width(section: &TaskbarSectionView) -> f64 {
    let value_width = (section.value.len() as f64 * 7.2).clamp(22.0, 72.0);
    match section.style {
        TaskbarSectionStyle::Chip => ICON_SIZE_DIP + 6.0 + value_width,
        TaskbarSectionStyle::Bar => ICON_SIZE_DIP + 8.0 + BAR_WIDTH_DIP + 6.0 + value_width,
        TaskbarSectionStyle::VerticalBar => ICON_SIZE_DIP + 6.0 + VERTICAL_BAR_WIDTH_DIP,
        TaskbarSectionStyle::Ring if section.show_ring_text => RING_SIZE_DIP,
        TaskbarSectionStyle::Ring => ICON_SIZE_DIP + 6.0 + RING_SIZE_DIP,
        TaskbarSectionStyle::Clock => ICON_SIZE_DIP + 6.0 + value_width + 4.0,
    }
}

pub fn content_width_for(sections: &[TaskbarSectionView]) -> f64 {
    if sections.is_empty() {
        return 84.0;
    }
    let body: f64 = sections.iter().map(section_width).sum();
    let gaps = CONTENT_GAP_DIP * (sections.len().saturating_sub(1) as f64);
    CONTENT_PADDING_DIP * 2.0 + body + gaps
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Placement {
    root_width_dip: f64,
    root_height_dip: f64,
    left_dip: f64,
    top_dip: f64,
    vertical: bool,
}

impl Placement {
    fn bootstrap(widget_width_dip: f64) -> Self {
        Self {
            root_width_dip: widget_width_dip,
            root_height_dip: WIDGET_HEIGHT_DIP,
            left_dip: 0.0,
            top_dip: 0.0,
            vertical: false,
        }
    }
}

pub fn register_host(host: Rc<ReactorHost>) {
    HWND_BITS.store(0, Ordering::Release);
    platform::reset_layout_cache();
    HOST.with(|slot| *slot.borrow_mut() = Some(host));
}

/// Create a fresh hidden reactor host when Explorer has destroyed the previous
/// child window. Safe to call repeatedly from the WinUI dispatcher thread.
pub fn ensure_host(state: Arc<AppState>) -> windows_core::Result<()> {
    if platform::window_exists() {
        return Ok(());
    }
    HOST_ACTIVATED.with(|activated| activated.set(false));
    let render_state = Arc::clone(&state);
    let host = Rc::new(ReactorHost::new_with_window_options(
        WINDOW_TITLE,
        Some(WindowSize {
            width: WIDGET_HEIGHT_DIP,
            height: WIDGET_HEIGHT_DIP,
        }),
        InnerConstraints {
            min_width: Some(1.0),
            min_height: Some(1.0),
            max_width: None,
            max_height: None,
        },
        Box::new(move |_: &(), cx: &mut RenderCx| view(cx, Arc::clone(&render_state))),
        |_| {},
    )?);
    register_host(host);
    Ok(())
}

pub fn view(cx: &mut RenderCx, state: Arc<AppState>) -> Element {
    let initial = live_snapshot(&state);
    let (snapshot, set_snapshot) = cx.use_async_state(initial);
    let initial_width = content_width_for(&snapshot.rendered_sections(Utc::now()));
    let (placement, set_placement) = cx.use_async_state(Placement::bootstrap(initial_width));
    cx.use_effect_with_cleanup((), {
        let state = Arc::clone(&state);
        let set_snapshot = set_snapshot.clone();
        move || {
            let seen = Rc::new(Cell::new(
                state.taskbar_widget_revision.load(Ordering::Acquire),
            ));
            let timer = DispatcherTimer::new(DATA_REFRESH, move || {
                let revision = state.taskbar_widget_revision.load(Ordering::Acquire);
                seen.set(revision);
                set_snapshot.call(live_snapshot(&state));
            })
            .ok();
            Some(move || drop(timer))
        }
    });

    let mut visible_sections = snapshot.rendered_sections(Utc::now());
    if placement.vertical {
        visible_sections.truncate(1);
    }
    let widget_width = content_width_for(&visible_sections);

    cx.use_effect_with_cleanup((snapshot.enabled, widget_width.to_bits()), {
        let set_placement = set_placement.clone();
        move || {
            let refresh = {
                let set_placement = set_placement.clone();
                move || {
                    if let Some(next) = platform::sync(snapshot.enabled, widget_width) {
                        set_placement.call(next);
                    }
                }
            };
            refresh();
            let timer = DispatcherTimer::new(LAYOUT_REFRESH, refresh).ok();
            Some(move || drop(timer))
        }
    });

    let strip = widget_strip(
        &visible_sections,
        snapshot.use_colored_provider_icons,
        snapshot.system_uses_light_theme,
        widget_width,
        WIDGET_HEIGHT_DIP,
    );
    let hit_target = border(Element::Empty)
        .background(Color::transparent())
        .width(widget_width)
        .height(WIDGET_HEIGHT_DIP)
        .tooltip("Open Codex Minibar")
        .on_tapped(crate::popup::show_near_cursor);

    let surface_layers: Vec<Element> = vec![strip, hit_target.into()];
    let strip_identity = visible_sections
        .iter()
        .map(|section| {
            format!(
                "{}:{}:{}:{}",
                section.id,
                style_tag(section.style),
                section.icon,
                u8::from(section.show_ring_text)
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    let surface: Element = relative_panel(surface_layers)
        .width(widget_width)
        .height(WIDGET_HEIGHT_DIP)
        .canvas_left(placement.left_dip)
        .canvas_top(placement.top_dip)
        .with_key(format!(
            "taskbar-strip-{strip_identity}-c{}-t{}",
            snapshot.use_colored_provider_icons, snapshot.system_uses_light_theme
        ))
        .into();

    Canvas::new([surface])
        .width(placement.root_width_dip.max(1.0))
        .height(placement.root_height_dip.max(1.0))
        .background(Color::transparent())
        .into()
}

fn live_snapshot(state: &AppState) -> TaskbarWidgetSnapshot {
    let mut snapshot = state.current_taskbar_widget_snapshot();
    snapshot.replace_limits(state.current_limits());
    snapshot.refresh_system_theme();
    snapshot.clock_minute = Utc::now().timestamp() / 60;
    snapshot
}

pub fn widget_strip(
    sections: &[TaskbarSectionView],
    use_colored_icons: bool,
    light_theme: bool,
    width: f64,
    height: f64,
) -> Element {
    let children = if sections.is_empty() {
        vec![text_block("Minibar")
            .font_size(12.0)
            .semibold()
            .vertical_alignment(VerticalAlignment::Center)
            .into()]
    } else {
        sections
            .iter()
            .map(|section| {
                section_element(section, use_colored_icons, light_theme).with_key(format!(
                    "taskbar-section-{}-{}-{}-{}",
                    section.id,
                    style_tag(section.style),
                    section.icon,
                    u8::from(section.show_ring_text)
                ))
            })
            .collect()
    };
    hstack(children)
        .spacing(CONTENT_GAP_DIP)
        .padding(Thickness::uniform(CONTENT_PADDING_DIP))
        .width(width)
        .height(height)
        .horizontal_alignment(HorizontalAlignment::Left)
        .vertical_alignment(VerticalAlignment::Center)
        .into()
}

pub fn preview_section(
    section: &TaskbarWidgetSection,
    limits: &ProviderLimits,
    show_used: bool,
    use_colored_icons: bool,
    light_theme: bool,
) -> Element {
    let now = Utc::now();
    let view = render_section(section, limits, show_used, now).unwrap_or_else(|| {
        TaskbarSectionView {
            id: section.id.clone(),
            icon: "codex",
            brand_rgb: (128, 159, 255),
            title: section.kind.label().into(),
            value: "—".into(),
            progress: None,
            style: section_style(section.presentation),
            show_ring_text: section.presentation.is_ring() && section.show_ring_text,
        }
    });
    section_element(&view, use_colored_icons, light_theme)
        .with_key(format!("widget-preview-{}", section.id))
}

fn section_element(
    section: &TaskbarSectionView,
    use_colored_icons: bool,
    light_theme: bool,
) -> Element {
    let icon_color = if use_colored_icons {
        Color::rgb(section.brand_rgb.0, section.brand_rgb.1, section.brand_rgb.2)
    } else if light_theme {
        Color::rgb(16, 16, 16)
    } else {
        Color::rgb(245, 245, 245)
    };
    let icon = crate::icons::element_in_slot(section.icon, ICON_SIZE_DIP, icon_color, &section.id)
        .vertical_alignment(VerticalAlignment::Center);
    let value = text_block(section.value.clone())
        .font_size(12.0)
        .semibold()
        .vertical_alignment(VerticalAlignment::Center);
    match section.style {
        TaskbarSectionStyle::Chip => hstack((icon, value))
            .spacing(6.0)
            .vertical_alignment(VerticalAlignment::Center)
            .into(),
        TaskbarSectionStyle::Bar => {
            let bar = compact_bar(section.progress.unwrap_or(0.0));
            hstack((icon, bar, value))
                .spacing(6.0)
                .vertical_alignment(VerticalAlignment::Center)
                .into()
        }
        TaskbarSectionStyle::VerticalBar => {
            let bar = compact_vertical_bar(section.progress.unwrap_or(0.0));
            hstack((icon, bar))
                .spacing(6.0)
                .vertical_alignment(VerticalAlignment::Center)
                .into()
        }
        TaskbarSectionStyle::Ring => {
            let ring = compact_ring(section);
            if section.show_ring_text {
                ring
            } else {
                hstack((icon, ring))
                    .spacing(6.0)
                    .vertical_alignment(VerticalAlignment::Center)
                    .into()
            }
        }
        TaskbarSectionStyle::Clock => {
            let caption = text_block(section.title.clone())
                .font_size(9.0)
                .opacity(0.68)
                .vertical_alignment(VerticalAlignment::Center);
            hstack((icon, vstack((caption, value)).spacing(0.0)))
                .spacing(6.0)
                .vertical_alignment(VerticalAlignment::Center)
                .into()
        }
    }
}

fn compact_bar(progress: f64) -> Element {
    let fill = (BAR_WIDTH_DIP * (progress / 100.0).clamp(0.0, 1.0)).max(if progress > 0.0 {
        3.0
    } else {
        0.0
    });
    let track: Element = border(Element::Empty)
        .background(ThemeRef::ControlFillSecondary)
        .corner_radius(2.0)
        .width(BAR_WIDTH_DIP)
        .height(4.0)
        .relative_align_left()
        .relative_align_v_center()
        .into();
    let fill_bar: Element = border(Element::Empty)
        .background(ThemeRef::Accent)
        .corner_radius(2.0)
        .width(fill)
        .height(4.0)
        .relative_align_left()
        .relative_align_v_center()
        .into();
    relative_panel(vec![track, fill_bar])
        .width(BAR_WIDTH_DIP)
        .height(6.0)
        .vertical_alignment(VerticalAlignment::Center)
        .into()
}

fn compact_vertical_bar(progress: f64) -> Element {
    let fill = (VERTICAL_BAR_HEIGHT_DIP * (progress / 100.0).clamp(0.0, 1.0)).max(
        if progress > 0.0 { 3.0 } else { 0.0 },
    );
    let track: Element = border(Element::Empty)
        .background(ThemeRef::ControlFillSecondary)
        .corner_radius(2.0)
        .width(VERTICAL_BAR_WIDTH_DIP)
        .height(VERTICAL_BAR_HEIGHT_DIP)
        .relative_align_left()
        .relative_align_bottom()
        .into();
    let fill_bar: Element = border(Element::Empty)
        .background(ThemeRef::Accent)
        .corner_radius(2.0)
        .width(VERTICAL_BAR_WIDTH_DIP)
        .height(fill)
        .relative_align_left()
        .relative_align_bottom()
        .into();
    relative_panel(vec![track, fill_bar])
        .width(VERTICAL_BAR_WIDTH_DIP)
        .height(VERTICAL_BAR_HEIGHT_DIP)
        .vertical_alignment(VerticalAlignment::Center)
        .into()
}

fn compact_ring(section: &TaskbarSectionView) -> Element {
    thread_local! {
        static RING_MOUNTS: RefCell<HashMap<String, windows_core::IInspectable>> =
            RefCell::new(HashMap::new());
        static RING_PROGRESS: RefCell<HashMap<String, u16>> = RefCell::new(HashMap::new());
    }

    let progress = section.progress.unwrap_or(0.0).clamp(0.0, 100.0);
    let fill = section.brand_rgb;
    let track = (70_u8, 70_u8, 70_u8);
    let xaml = ring_xaml(progress, fill, track);
    let host_key = format!(
        "taskbar-ring-{}-{:02X}{:02X}{:02X}",
        section.id, fill.0, fill.1, fill.2
    );
    let progress_bucket = (progress * 10.0).round() as u16;
    let changed = RING_PROGRESS.with(|cache| {
        let mut cache = cache.borrow_mut();
        match cache.get(&host_key) {
            Some(previous) if *previous == progress_bucket => false,
            _ => {
                cache.insert(host_key.clone(), progress_bucket);
                true
            }
        }
    });
    if changed {
        RING_MOUNTS.with(|mounts| {
            if let Some(native) = mounts.borrow().get(&host_key).cloned()
                && let Err(error) = crate::acrylic::install_spend_donut_into(native, &xaml)
            {
                eprintln!("Could not update taskbar ring: {error:?}");
            }
        });
    }

    let xaml_for_mount = xaml;
    let key_for_mount = host_key.clone();
    let key_for_unmount = host_key.clone();
    let mut host = swap_chain_panel()
        .width(RING_SIZE_DIP)
        .height(RING_SIZE_DIP);
    host.mounted = Some(Callback::new(
        move |native: Option<windows_core::IInspectable>| {
            if let Some(native) = native {
                if let Err(error) =
                    crate::acrylic::install_spend_donut_into(native.clone(), &xaml_for_mount)
                {
                    eprintln!("Could not install taskbar ring: {error:?}");
                }
                RING_MOUNTS.with(|mounts| {
                    mounts.borrow_mut().insert(key_for_mount.clone(), native);
                });
            }
        },
    ));
    host.unmounted = Some(Callback::new(
        move |native: Option<windows_core::IInspectable>| {
            if let Some(native) = native {
                let _ = crate::acrylic::clear_children(native);
            }
            RING_MOUNTS.with(|mounts| {
                mounts.borrow_mut().remove(&key_for_unmount);
            });
            RING_PROGRESS.with(|cache| {
                cache.borrow_mut().remove(&key_for_unmount);
            });
        },
    ));
    let ring: Element = host.with_key(host_key).into();
    if !section.show_ring_text {
        return ring;
    }
    grid((
        ring,
        text_block(section.value.clone())
            .font_size(8.0)
            .semibold()
            .horizontal_alignment(HorizontalAlignment::Center)
            .vertical_alignment(VerticalAlignment::Center),
    ))
    .columns([GridLength::Pixel(RING_SIZE_DIP)])
    .rows([GridLength::Pixel(RING_SIZE_DIP)])
    .width(RING_SIZE_DIP)
    .height(RING_SIZE_DIP)
    .vertical_alignment(VerticalAlignment::Center)
    .into()
}

fn ring_xaml(progress: f64, fill: (u8, u8, u8), track: (u8, u8, u8)) -> String {
    const CENTER: f64 = RING_SIZE_DIP / 2.0;
    const OUTER: f64 = CENTER - 1.5;
    const INNER: f64 = OUTER - 3.0;
    let track_path = ring_path(track, -90.0, 270.0, CENTER, OUTER, INNER);
    let sweep = (progress / 100.0) * 360.0;
    let fill_path = if sweep <= 0.5 {
        String::new()
    } else {
        ring_path(fill, -90.0, -90.0 + sweep, CENTER, OUTER, INNER)
    };
    format!(
        r#"<Canvas xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Width="{RING_SIZE_DIP:.0}" Height="{RING_SIZE_DIP:.0}">{track_path}{fill_path}</Canvas>"#
    )
}

fn ring_path(
    color: (u8, u8, u8),
    start: f64,
    end: f64,
    center: f64,
    outer: f64,
    inner: f64,
) -> String {
    let sweep = (end - start).max(0.0);
    if sweep <= 0.0 {
        return String::new();
    }
    let fill = format!("#{:02X}{:02X}{:02X}", color.0, color.1, color.2);
    if sweep >= 359.0 {
        return format!(
            r#"<Path Fill="{fill}" Data="M {center:.2} {outer_top:.2} A {outer:.2} {outer:.2} 0 1 1 {center:.2} {outer_bottom:.2} A {outer:.2} {outer:.2} 0 1 1 {center:.2} {outer_top:.2} M {center:.2} {inner_top:.2} A {inner:.2} {inner:.2} 0 1 0 {center:.2} {inner_bottom:.2} A {inner:.2} {inner:.2} 0 1 0 {center:.2} {inner_top:.2} Z" />"#,
            outer_top = center - outer,
            outer_bottom = center + outer,
            inner_top = center - inner,
            inner_bottom = center + inner,
        );
    }
    let (outer_start_x, outer_start_y) = ring_point(center, outer, start);
    let (outer_end_x, outer_end_y) = ring_point(center, outer, end);
    let (inner_start_x, inner_start_y) = ring_point(center, inner, start);
    let (inner_end_x, inner_end_y) = ring_point(center, inner, end);
    let large_arc = u8::from(sweep > 180.0);
    format!(
        r#"<Path Fill="{fill}" Data="M {outer_start_x:.2} {outer_start_y:.2} A {outer:.2} {outer:.2} 0 {large_arc} 1 {outer_end_x:.2} {outer_end_y:.2} L {inner_end_x:.2} {inner_end_y:.2} A {inner:.2} {inner:.2} 0 {large_arc} 0 {inner_start_x:.2} {inner_start_y:.2} Z" />"#
    )
}

fn ring_point(center: f64, radius: f64, degrees: f64) -> (f64, f64) {
    let radians = degrees.to_radians();
    (
        center + radius * radians.cos(),
        center + radius * radians.sin(),
    )
}

fn primary_axis_origin(container: i32, anchor: Option<i32>, extent: i32) -> i32 {
    let max = (container - extent - 4).max(4);
    anchor
        .map(|start| start - extent - 6)
        .unwrap_or(max)
        .clamp(4, max)
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::{cell::RefCell, ffi::c_void, ptr};
    use windows_sys::Win32::{
        Foundation::{HWND, POINT, RECT},
        Graphics::{
            Dwm::DwmExtendFrameIntoClientArea,
            Gdi::{CreateRoundRectRgn, DeleteObject, ScreenToClient, SetWindowRgn},
        },
        UI::{
            Controls::MARGINS,
            HiDpi::GetDpiForWindow,
            WindowsAndMessaging::{
                FindWindowExW, FindWindowW, GWL_EXSTYLE, GWL_STYLE, GetParent, GetWindowLongW,
                GetWindowRect, IsWindow, SW_HIDE, SW_SHOWNOACTIVATE, SWP_ASYNCWINDOWPOS,
                SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
                SetParent, SetWindowLongW, SetWindowPos, ShowWindow, WS_CAPTION, WS_CHILD,
                WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW,
                WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
            },
        },
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct NativeLayout {
        hwnd: isize,
        taskbar: isize,
        container_x: i32,
        container_y: i32,
        container_width: i32,
        container_height: i32,
        region_left: i32,
        region_top: i32,
        region_width: i32,
        region_height: i32,
        dpi: u32,
    }

    thread_local! {
        static LAST_LAYOUT: RefCell<Option<NativeLayout>> = const { RefCell::new(None) };
    }

    pub(super) fn sync(enabled: bool, widget_width_dip: f64) -> Option<Placement> {
        let hwnd = current_hwnd()?;
        if !enabled {
            hide(hwnd);
            return None;
        }
        let taskbar = find_window_by_class("Shell_TrayWnd")?;
        if taskbar.is_null() {
            hide(hwnd);
            return None;
        }

        let needs_parent = unsafe { GetParent(hwnd) } != taskbar;
        let needs_activation = HOST_ACTIVATED.with(|activated| !activated.get());
        if needs_parent || needs_activation {
            configure(hwnd, taskbar);
        }
        if activate_host_once(hwnd, taskbar) {
            configure(hwnd, taskbar);
        }

        let mut taskbar_rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if unsafe { GetWindowRect(taskbar, &mut taskbar_rect) } == 0 {
            hide(hwnd);
            return None;
        }
        let width_px = taskbar_rect.right - taskbar_rect.left;
        let height_px = taskbar_rect.bottom - taskbar_rect.top;
        if width_px <= 0 || height_px <= 0 {
            hide(hwnd);
            return None;
        }
        let dpi = unsafe { GetDpiForWindow(taskbar) }.max(96);
        let dip_to_px = |dip: f64| (dip * f64::from(dpi) / 96.0).round() as i32;
        let widget_width_px = dip_to_px(widget_width_dip).max(1);
        let widget_height_px = dip_to_px(WIDGET_HEIGHT_DIP).max(1);
        let vertical = height_px > width_px;

        let tray = unsafe {
            FindWindowExW(
                taskbar,
                ptr::null_mut(),
                wide("TrayNotifyWnd").as_ptr(),
                ptr::null(),
            )
        };
        let tray_rect = (!tray.is_null()).then(|| {
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            (unsafe { GetWindowRect(tray, &mut rect) } != 0).then_some(rect)
        });
        let tray_rect = tray_rect.flatten();

        let (left_px, top_px, region_width_px, region_height_px) = if vertical {
            let anchor = tray_rect.map(|rect| rect.top - taskbar_rect.top);
            (
                ((width_px - widget_width_px) / 2).max(0),
                primary_axis_origin(height_px, anchor, widget_height_px),
                widget_width_px,
                widget_height_px,
            )
        } else {
            let anchor = tray_rect.map(|rect| rect.left - taskbar_rect.left);
            (
                primary_axis_origin(width_px, anchor, widget_width_px),
                ((height_px - widget_height_px) / 2).max(0),
                widget_width_px,
                widget_height_px,
            )
        };

        let mut container_origin = POINT {
            x: taskbar_rect.left,
            y: taskbar_rect.top,
        };
        unsafe {
            ScreenToClient(taskbar, &mut container_origin);
        }
        let layout = NativeLayout {
            hwnd: hwnd as isize,
            taskbar: taskbar as isize,
            container_x: container_origin.x,
            container_y: container_origin.y,
            container_width: width_px,
            container_height: height_px,
            region_left: left_px,
            region_top: top_px,
            region_width: region_width_px,
            region_height: region_height_px,
            dpi,
        };
        let unchanged = LAST_LAYOUT.with(|cached| cached.borrow().as_ref() == Some(&layout));
        if unchanged {
            return Some(placement_from_layout(layout, vertical));
        }

        unsafe {
            SetWindowPos(
                hwnd,
                ptr::null_mut(),
                container_origin.x,
                container_origin.y,
                width_px,
                height_px,
                SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS | SWP_SHOWWINDOW,
            );
        }
        HOST.with(|slot| {
            if let Some(host) = slot.borrow().as_ref() {
                host.sync_render_size(
                    f64::from(width_px) * 96.0 / f64::from(dpi),
                    f64::from(height_px) * 96.0 / f64::from(dpi),
                );
            }
        });

        let radius = dip_to_px(8.0).max(1) * 2;
        let region = unsafe {
            CreateRoundRectRgn(
                left_px,
                top_px,
                left_px + region_width_px + 1,
                top_px + region_height_px + 1,
                radius,
                radius,
            )
        };
        if region.is_null() {
            hide(hwnd);
            return None;
        }
        if unsafe { SetWindowRgn(hwnd, region, 1) } == 0 {
            unsafe {
                DeleteObject(region);
            }
            hide(hwnd);
            return None;
        }
        unsafe {
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        LAST_LAYOUT.with(|cached| *cached.borrow_mut() = Some(layout));

        Some(placement_from_layout(layout, vertical))
    }

    fn placement_from_layout(layout: NativeLayout, vertical: bool) -> Placement {
        Placement {
            root_width_dip: f64::from(layout.container_width) * 96.0 / f64::from(layout.dpi),
            root_height_dip: f64::from(layout.container_height) * 96.0 / f64::from(layout.dpi),
            left_dip: f64::from(layout.region_left) * 96.0 / f64::from(layout.dpi),
            top_dip: f64::from(layout.region_top) * 96.0 / f64::from(layout.dpi),
            vertical,
        }
    }

    pub(super) fn window_exists() -> bool {
        current_hwnd().is_some()
    }

    fn activate_host_once(hwnd: HWND, taskbar: HWND) -> bool {
        HOST_ACTIVATED.with(|activated| {
            if activated.get() {
                return false;
            }
            hide_pixels(hwnd);
            configure(hwnd, taskbar);
            let activated_now = HOST.with(|slot| {
                slot.borrow().as_ref().is_some_and(|host| {
                    let _ = host.set_shown_in_switchers(false);
                    host.activate_now().is_ok()
                })
            });
            activated.set(activated_now);
            activated_now
        })
    }

    fn configure(hwnd: HWND, taskbar: HWND) {
        unsafe {
            let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
            let style = (style
                & !(WS_POPUP
                    | WS_CAPTION
                    | WS_THICKFRAME
                    | WS_MINIMIZEBOX
                    | WS_MAXIMIZEBOX
                    | WS_SYSMENU))
                | WS_CHILD;
            SetWindowLongW(hwnd, GWL_STYLE, style as i32);

            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            let ex_style = (ex_style
                | WS_EX_TOOLWINDOW
                | WS_EX_NOACTIVATE
                | WS_EX_NOREDIRECTIONBITMAP)
                & !WS_EX_APPWINDOW;
            SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style as i32);
            if GetParent(hwnd) != taskbar {
                SetParent(hwnd, taskbar);
            }
            let glass = MARGINS {
                cxLeftWidth: -1,
                cxRightWidth: -1,
                cyTopHeight: -1,
                cyBottomHeight: -1,
            };
            let _ = DwmExtendFrameIntoClientArea(hwnd, &glass);
            SetWindowPos(
                hwnd,
                ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE,
            );
        }
    }

    fn hide(hwnd: HWND) {
        reset_layout_cache();
        hide_pixels(hwnd);
        unsafe {
            ShowWindow(hwnd, SW_HIDE);
        }
    }

    fn hide_pixels(hwnd: HWND) {
        let empty = unsafe { CreateRoundRectRgn(0, 0, 0, 0, 0, 0) };
        if !empty.is_null() {
            if unsafe { SetWindowRgn(hwnd, empty, 0) } == 0 {
                unsafe {
                    DeleteObject(empty);
                }
            }
        }
    }

    fn current_hwnd() -> Option<HWND> {
        let cached = HWND_BITS.load(Ordering::Acquire) as HWND;
        if !cached.is_null() && unsafe { IsWindow(cached) } != 0 {
            return Some(cached);
        }
        HWND_BITS.store(0, Ordering::Release);
        let hwnd = find_window(WINDOW_TITLE)?;
        HWND_BITS.store(hwnd as isize, Ordering::Release);
        Some(hwnd)
    }

    fn find_window(title: &str) -> Option<HWND> {
        let hwnd = unsafe { FindWindowW(ptr::null(), wide(title).as_ptr()) };
        (!hwnd.is_null()).then_some(hwnd)
    }

    fn find_window_by_class(class_name: &str) -> Option<HWND> {
        let hwnd = unsafe { FindWindowW(wide(class_name).as_ptr(), ptr::null()) };
        (!hwnd.is_null()).then_some(hwnd)
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub(super) fn reset_layout_cache() {
        LAST_LAYOUT.with(|cached| *cached.borrow_mut() = None);
    }

    #[allow(dead_code)]
    fn _assert_hwnd_pointer_shape(_: *mut c_void) {}
}

#[cfg(not(windows))]
mod platform {
    use super::*;

    pub(super) fn sync(_enabled: bool, _widget_width_dip: f64) -> Option<Placement> {
        None
    }

    pub(super) fn window_exists() -> bool {
        false
    }

    pub(super) fn reset_layout_cache() {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{ProviderKind, ProviderSettings};

    #[test]
    fn compact_template_uses_session_chips_for_enabled_providers() {
        let settings = Settings {
            providers: ProviderSettings::from_enabled([ProviderKind::Claude]),
            ..Settings::default()
        };
        let snapshot = TaskbarWidgetSnapshot::from_settings(&settings, ProviderLimits::default());
        assert_eq!(snapshot.sections.len(), 1);
        assert_eq!(snapshot.sections[0].kind, TaskbarWidgetSectionKind::Session);
        assert_eq!(snapshot.sections[0].provider(), Some(ProviderKind::Claude));
    }

    #[test]
    fn rendered_session_reads_live_limits() {
        let settings = Settings {
            providers: ProviderSettings::from_enabled([ProviderKind::Codex]),
            ..Settings::default()
        };
        let mut limits = ProviderLimits::default();
        limits.get_mut(ProviderKind::Codex).primary.used_percent = Some(37);
        let snapshot = TaskbarWidgetSnapshot::from_settings(&settings, limits);
        let rendered = snapshot.rendered_sections(Utc::now());
        assert_eq!(rendered[0].value, "63%");
    }

    #[test]
    fn clock_presentation_uses_the_window_reset_time() {
        let mut section = TaskbarWidgetSection::for_provider(
            TaskbarWidgetSectionKind::Weekly,
            ProviderKind::Codex,
        );
        section.presentation = TaskbarSectionPresentation::Clock;
        let now = Utc::now();
        let mut limits = ProviderLimits::default();
        limits.get_mut(ProviderKind::Codex).secondary = LimitWindow {
            used_percent: Some(16),
            resets_at: Some(now + chrono::Duration::hours(5)),
            duration_minutes: Some(10_080),
        };
        let rendered = render_section(&section, &limits, false, now).unwrap();
        assert_eq!(rendered.style, TaskbarSectionStyle::Clock);
        assert_eq!(rendered.title, "Wk");
        assert_eq!(rendered.value, "5h");
    }

    #[test]
    fn ring_presentation_keeps_text_inside_when_requested() {
        let mut section = TaskbarWidgetSection::for_provider(
            TaskbarWidgetSectionKind::Session,
            ProviderKind::Codex,
        );
        section.presentation = TaskbarSectionPresentation::Ring;
        section.show_ring_text = true;
        let mut limits = ProviderLimits::default();
        limits.get_mut(ProviderKind::Codex).primary.used_percent = Some(19);
        let rendered = render_section(&section, &limits, false, Utc::now()).unwrap();
        assert_eq!(rendered.style, TaskbarSectionStyle::Ring);
        assert!(rendered.show_ring_text);
        assert_eq!(rendered.value, "81%");
        assert!(section_width(&rendered) <= RING_SIZE_DIP + 1.0);
    }

    fn vertical_bar_is_narrower_than_a_horizontal_bar() {
        let mut horizontal = TaskbarWidgetSection::for_provider(
            TaskbarWidgetSectionKind::Session,
            ProviderKind::Codex,
        );
        horizontal.presentation = TaskbarSectionPresentation::ProgressBar;
        let mut vertical = horizontal.clone();
        vertical.id = "taskbar-vertical".into();
        vertical.presentation = TaskbarSectionPresentation::VerticalBar;
        let mut limits = ProviderLimits::default();
        limits.get_mut(ProviderKind::Codex).primary.used_percent = Some(40);
        let now = Utc::now();
        let horizontal = render_section(&horizontal, &limits, false, now).unwrap();
        let vertical = render_section(&vertical, &limits, false, now).unwrap();
        assert_eq!(vertical.style, TaskbarSectionStyle::VerticalBar);
        assert!(section_width(&vertical) < section_width(&horizontal));
    }

    fn progress_bar_presentation_is_available_without_a_template() {
        let mut section = TaskbarWidgetSection::for_provider(
            TaskbarWidgetSectionKind::Session,
            ProviderKind::Codex,
        );
        section.presentation = TaskbarSectionPresentation::ProgressBar;
        let mut limits = ProviderLimits::default();
        limits.get_mut(ProviderKind::Codex).primary.used_percent = Some(40);
        let rendered = render_section(&section, &limits, false, Utc::now()).unwrap();
        assert_eq!(rendered.style, TaskbarSectionStyle::Bar);
        assert_eq!(rendered.progress, Some(60.0));
    }

    #[test]
    fn origin_prefers_the_space_immediately_before_the_tray() {
        assert_eq!(primary_axis_origin(1920, Some(1700), 120), 1574);
        assert_eq!(primary_axis_origin(100, Some(20), 80), 4);
        assert_eq!(primary_axis_origin(1920, None, 120), 1796);
    }
}
