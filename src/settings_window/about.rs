use super::persistence::persist_bool;
use super::*;

/// About mirrors the README hero with the high-resolution app icon including
/// its rounded background.
fn settings_about_icon_uri() -> String {
    let packaged = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .map(|parent| parent.join("assets/app-icon.png"))
        })
        .filter(|path| path.exists());
    let path = packaged.unwrap_or_else(|| {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/app-icon.png")
    });
    format!("file:///{}", path.to_string_lossy().replace('\\', "/"))
}

fn update_status_label(phase: &UpdatePhase) -> String {
    match phase {
        UpdatePhase::Idle => "Check GitHub for a new version".into(),
        UpdatePhase::Checking => "Checking for updates".into(),
        UpdatePhase::UpToDate => "You're up to date".into(),
        UpdatePhase::Available(update) => format!("Update {} available", update.version),
        UpdatePhase::Applying => "Installing update...".into(),
        // Never surface raw transport errors (e.g. "GET https://...").
        UpdatePhase::Failed(_) => "Couldn't check for updates".into(),
    }
}

fn about_settings_cards(
    check_for_updates: bool,
    notify_on_update: bool,
    update_phase: &UpdatePhase,
    set_check_for_updates: SetState<bool>,
    set_notify_on_update: SetState<bool>,
    apply_check_for_updates: Sender<Settings>,
    apply_notify_on_update: Sender<Settings>,
    hovered_card_id: &Option<String>,
    set_hovered_card_id: SetState<Option<String>>,
    settings_tx: Sender<Settings>,
    updates: Arc<UpdateController>,
) -> Vec<Element> {
    let version = current_version().to_string();
    let updates_for_check = updates.clone();
    let notify_for_check = notify_on_update;

    let hero = border(
        vstack((
            Image::new_with_uri(settings_about_icon_uri())
                .width(112.0)
                .height(112.0)
                .horizontal_alignment(HorizontalAlignment::Center)
                .margin(Thickness {
                    left: 0.0,
                    top: 0.0,
                    right: 0.0,
                    bottom: 10.0,
                }),
            vstack((
                text_block("Codex Minibar")
                    .font_size(26.0)
                    .bold()
                    .horizontal_alignment(HorizontalAlignment::Center),
                text_block(format!("Version {version}"))
                    .font_size(13.0)
                    .foreground(ThemeRef::SecondaryText)
                    .horizontal_alignment(HorizontalAlignment::Center),
            ))
            .spacing(2.0)
            .horizontal_alignment(HorizontalAlignment::Center),
            text_block("Usage limits in the Windows tray.")
                .font_size(15.0)
                .wrap()
                .foreground(ThemeRef::SecondaryText)
                .horizontal_alignment(HorizontalAlignment::Center)
                .margin(Thickness {
                    left: 0.0,
                    top: 10.0,
                    right: 0.0,
                    bottom: 0.0,
                }),
        ))
        .spacing(0.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(Thickness {
        left: 0.0,
        top: 8.0,
        right: 0.0,
        bottom: 22.0,
    })
    .background(Color::transparent())
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .with_key("about-hero")
    .into();

    let update_options = vstack((
        settings_toggle_card(
            "Check for updates on startup",
            check_for_updates,
            move |value| {
                persist_bool(
                    set_check_for_updates.clone(),
                    apply_check_for_updates.clone(),
                    value,
                    |settings, value| {
                        settings.check_for_updates = value;
                    },
                );
            },
            "about-check-updates",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("about-check-updates"),
        settings_toggle_card(
            "Notify when a new version is found",
            notify_on_update,
            move |value| {
                persist_bool(
                    set_notify_on_update.clone(),
                    apply_notify_on_update.clone(),
                    value,
                    |settings, value| {
                        settings.notifications.update_available = value;
                    },
                );
            },
            "about-notify-updates",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("about-notify-updates"),
    ))
    .spacing(4.0);

    let update_settings_separator = border(Element::Empty)
        .height(1.0)
        .background(ThemeRef::DividerStroke)
        .margin(Thickness {
            left: 0.0,
            top: 4.0,
            right: 0.0,
            bottom: 4.0,
        })
        .horizontal_alignment(HorizontalAlignment::Stretch);

    let update_actions: Element = if matches!(update_phase, UpdatePhase::Available(_)) {
        vstack((
            settings_action_card(
                "Install the latest version",
                "Update",
                || {
                    if let Err(error) = crate::updater::apply_pending_update() {
                        eprintln!("failed to apply update: {error:#}");
                        crate::notifications::show("Update failed", &format!("{error:#}"));
                    }
                },
                "about-update-apply",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("about-update-apply"),
            settings_action_card(
                "GitHub release notes",
                "What's New",
                || {
                    if let Err(error) = crate::updater::open_release_notes() {
                        eprintln!("failed to open release notes: {error:#}");
                    }
                },
                "about-whats-new",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .with_key("about-whats-new"),
        ))
        .spacing(4.0)
        .into()
    } else {
        settings_action_card(
            update_status_label(update_phase),
            "Check for updates",
            move || {
                updates_for_check.check_async(false, notify_for_check);
            },
            "about-check-now",
            hovered_card_id,
            set_hovered_card_id.clone(),
        )
        .with_key("about-check-now")
    };

    let updates_card = about_section(
        "Updates",
        vstack((update_actions, update_settings_separator, update_options)).spacing(4.0),
    )
    .with_key("about-updates");

    let resources = about_section(
        "Resources",
        grid((
            about_action_card(
                "GitHub",
                "Source code",
                AboutCardIcon::Phosphor("github-logo"),
                || {
                    let _ = crate::updater::open_url(REPO_URL);
                },
                "about-github",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .grid_row(0)
            .grid_column(0),
            about_action_card(
                "Releases",
                "See what's new",
                AboutCardIcon::Phosphor("download-simple"),
                || {
                    let _ = crate::updater::open_url(RELEASES_URL);
                },
                "about-releases",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .grid_row(0)
            .grid_column(1),
            about_action_card(
                "Report an issue",
                "Found a bug?",
                AboutCardIcon::Phosphor("flag"),
                || {
                    let _ = crate::updater::open_url(ISSUES_URL);
                },
                "about-issues",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .grid_row(1)
            .grid_column(0),
            about_action_card(
                "Author",
                "@vertopolkaLF",
                AboutCardIcon::Phosphor("at"),
                || {
                    let _ = crate::updater::open_url("https://github.com/vertopolkaLF");
                },
                "about-author",
                hovered_card_id,
                set_hovered_card_id.clone(),
            )
            .grid_row(1)
            .grid_column(1),
        ))
        .columns([GridLength::Star(1.0), GridLength::Star(1.0)])
        .rows([GridLength::Auto, GridLength::Auto])
        .column_spacing(12.0)
        .row_spacing(12.0)
        .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .with_key("about-resources");

    let cards = vec![hero, updates_card.into(), resources.into()];

    let _ = settings_tx;
    cards
}

fn about_section(title: impl Into<String>, content: impl Into<Element>) -> Element {
    about_section_with_header(text_block(title).font_size(18.0).bold(), content)
}

fn about_section_with_header(header: impl Into<Element>, content: impl Into<Element>) -> Element {
    border(
        vstack((header.into(), content.into()))
            .spacing(14.0)
            .horizontal_alignment(HorizontalAlignment::Stretch),
    )
    .padding(settings_card_padding())
    .background(ThemeRef::CardBackground)
    .corner_radius(14.0)
    .border_thickness(Thickness::uniform(1.0))
    .border_brush(ThemeRef::CardStroke)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .into()
}

/// Full-surface action card used by the About page.  The panel, not a nested
/// button, owns the click target so it feels like one intentional control.
#[derive(Clone, Copy)]
enum AboutCardIcon {
    Phosphor(&'static str),
}

fn about_action_card(
    title: impl Into<String>,
    description: impl Into<String>,
    icon: AboutCardIcon,
    on_click: impl IntoUnitCallback,
    card_id: &'static str,
    hovered_id: &Option<String>,
    set_hovered_id: SetState<Option<String>>,
) -> Element {
    let hovered = hovered_id.as_deref() == Some(card_id);
    let on_click = on_click.into_unit_callback();
    let on_enter = {
        let set_hovered_id = set_hovered_id.clone();
        move |_: PointerEventInfo| set_hovered_id.call(Some(card_id.to_string()))
    };
    let on_exit = move || set_hovered_id.call(None);

    let base: Element = border(Element::Empty)
        .background(ThemeRef::AccentTertiary)
        // Accent resources can be fully opaque on some Windows palettes.
        // Keep only a gentle tint, comparable to the previous card fill.
        .opacity(0.18)
        .corner_radius(10.0)
        .relative_align_left()
        .relative_align_right()
        .relative_align_top()
        .relative_align_bottom()
        .into();
    let hover: Element = border(Element::Empty)
        .background(ThemeRef::AccentSecondary)
        .opacity(if hovered { 0.28 } else { 0.0 })
        .with_opacity_transition(duration(CONTROL_FAST_ANIMATION))
        .corner_radius(10.0)
        .relative_align_left()
        .relative_align_right()
        .relative_align_top()
        .relative_align_bottom()
        .into();
    let AboutCardIcon::Phosphor(name) = icon;
    let icon: Element = crate::icons::element(name, 16.0, Color::rgb(226, 151, 78))
        .vertical_alignment(VerticalAlignment::Center)
        .into();
    let heading = grid((
        icon.grid_column(0),
        text_block(title)
            .font_size(15.0)
            .semibold()
            .grid_column(1)
            .vertical_alignment(VerticalAlignment::Center),
    ))
    .columns([GridLength::Pixel(16.0), GridLength::Star(1.0)])
    .column_spacing(8.0)
    .rows([GridLength::Auto]);

    relative_panel(vec![
        base,
        hover,
        vstack((
            heading,
            text_block(description)
                .font_size(13.0)
                .foreground(ThemeRef::SecondaryText),
        ))
        .spacing(5.0)
        .margin(Thickness {
            left: SETTINGS_CARD_PADDING,
            top: SETTINGS_CARD_PADDING,
            right: SETTINGS_CARD_PADDING,
            bottom: SETTINGS_CARD_PADDING,
        })
        .relative_align_left()
        .relative_align_top()
        .into(),
    ])
    .min_height(82.0)
    .horizontal_alignment(HorizontalAlignment::Stretch)
    .background(Color::transparent())
    .on_pointer_entered(on_enter)
    .on_pointer_exited(on_exit)
    .on_tapped(move || on_click.invoke(()))
    .with_key(card_id)
    .into()
}

pub(super) fn render(ctx: &SettingsPageContext<'_>) -> (&'static str, Vec<Element>) {
    let check_for_updates = ctx.check_for_updates;
    let notify_on_update = ctx.notify_on_update;
    let update_phase = ctx.update_phase;
    let set_check_for_updates = ctx.set_check_for_updates.clone();
    let set_notify_on_update = ctx.set_notify_on_update.clone();
    let hovered_card_id = ctx.hovered_card_id;
    let set_hovered_card_id = ctx.set_hovered_card_id.clone();
    let settings_tx = ctx.settings_tx.clone();
    let updates = ctx.updates.clone();
    let apply_check_for_updates = settings_tx.clone();
    let apply_notify_on_update = settings_tx.clone();
    (
        "About & Updates",
        about_settings_cards(
            check_for_updates,
            notify_on_update,
            update_phase,
            set_check_for_updates,
            set_notify_on_update,
            apply_check_for_updates,
            apply_notify_on_update,
            hovered_card_id,
            set_hovered_card_id.clone(),
            settings_tx.clone(),
            updates,
        ),
    )
}
