use super::*;

pub(super) fn render(tab: Tab, ctx: &SettingsPageContext<'_>) -> Element {
    let (title, rows) = match tab {
        Tab::General => super::general::render(ctx),
        Tab::Appearance => super::appearance::render(ctx),
        Tab::Popup => super::customize::render(ctx),
        Tab::Providers => unreachable!("Providers drill-in uses provider_page_content"),
        Tab::Schedule => super::activation::render(ctx),
        Tab::Tray => super::tray::render(ctx),
        Tab::Notifications => super::notifications::render(ctx),
        Tab::Advanced => super::advanced::render(ctx),
        Tab::Log => super::log::render(ctx),
        Tab::About => super::about::render(ctx),
    };

    let row_count = rows.len();
    let cards = vstack(rows)
        .spacing(if tab == Tab::Tray { 12.0 } else { 4.0 })
        .grid_row(1)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .with_key(format!("{}-cards-{row_count}", tab.tag()));

    let heading: Element = if tab == Tab::About {
        Element::Empty
    } else {
        text_block(title).font_size(28.0).bold().grid_row(0).into()
    };

    grid((heading, cards))
        .columns([GridLength::Star(1.0)])
        .rows([GridLength::Auto, GridLength::Auto])
        .row_spacing(if tab == Tab::About { 0.0 } else { 10.0 })
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Top)
        .into()
}
