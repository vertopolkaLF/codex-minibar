//! Customize: popup layout, tabs, card presentation and the Usage widget,
//! plus the per-instance popup card table shown on provider pages.

use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled, div, px,
};

use super::kit::{self, Kit, Row};
use super::window::SettingsWindow;
use crate::settings::{PopupTabMode, ProviderInstance, Settings, TotalSpendPresentation};

fn bool_row(
    this: &SettingsWindow,
    k: &mut Kit,
    cx: &mut Context<SettingsWindow>,
    id: &'static str,
    title: &'static str,
    description: Option<&'static str>,
    read: fn(&Settings) -> bool,
    write: fn(&mut Settings, bool),
) -> AnyElement {
    kit::toggle_row(
        k,
        id,
        title,
        description.map(SharedString::from),
        read(&this.settings),
        SettingsWindow::h(cx, move |this, value: bool, _, cx| {
            this.edit(cx, move |settings| write(settings, value))
        }),
    )
}

impl SettingsWindow {
    pub(super) fn customize_page(
        &mut self,
        k: &mut Kit,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let layout = kit::card_of(k, |k| {
            vec![bool_row(
                self,
                k,
                cx,
                "customize-two-columns",
                crate::i18n::tr("use-two-columns"),
                Some(crate::i18n::tr(
                    "widen-home-and-usage-drag-home-blocks-between-columns-provider-ta",
                )),
                |s| s.popup_two_columns,
                |s, v| s.popup_two_columns = v,
            )]
        });
        let tab_mode = self.settings.popup_tab_mode;
        let tabs = kit::card_of(k, |k| {
            vec![
                kit::dropdown_row(
                    k,
                    "customize-tab-mode",
                    crate::i18n::tr("several-accounts-of-one-provider"),
                    Some(crate::i18n::tr(
                        "separate-tabs-gives-every-instance-its-own-tab-grouped-shows-one",
                    )),
                    PopupTabMode::ALL
                        .iter()
                        .map(|mode| SharedString::from(mode.label()))
                        .collect(),
                    tab_mode.index(),
                    false,
                    Self::h(cx, |this, index: usize, _, cx| {
                        let mode = PopupTabMode::from_index(index as i32);
                        this.edit(cx, move |settings| settings.popup_tab_mode = mode)
                    }),
                ),
                bool_row(
                    self,
                    k,
                    cx,
                    "customize-mono-icons",
                    crate::i18n::tr("use-monochrome-icons"),
                    Some(crate::i18n::tr(
                        "draw-provider-marks-in-the-popup-without-brand-colors",
                    )),
                    |s| !s.use_colored_provider_icons,
                    |s, v| s.use_colored_provider_icons = !v,
                ),
            ]
        });
        let cards = kit::card_of(k, |k| {
            vec![
                bool_row(
                    self,
                    k,
                    cx,
                    "customize-show-used",
                    crate::i18n::tr("show-used-instead-of-remaining"),
                    None,
                    |s| s.show_used_percentage,
                    |s, v| s.show_used_percentage = v,
                ),
                bool_row(
                    self,
                    k,
                    cx,
                    "customize-usage-values",
                    crate::i18n::tr("show-usage-in-values-when-possible"),
                    Some(crate::i18n::tr(
                        "adds-exact-used-limit-amounts-next-to-percentages-when-a-provider",
                    )),
                    |s| s.show_usage_values,
                    |s, v| s.show_usage_values = v,
                ),
                bool_row(
                    self,
                    k,
                    cx,
                    "customize-usage-pace",
                    crate::i18n::tr("show-usage-pace"),
                    Some(crate::i18n::tr(
                        "marks-whether-you-re-burning-quota-faster-or-slower-than-an-even",
                    )),
                    |s| s.show_usage_pace,
                    |s, v| s.show_usage_pace = v,
                ),
                bool_row(
                    self,
                    k,
                    cx,
                    "customize-legacy-cards",
                    crate::i18n::tr("use-legacy-usage-cards"),
                    Some(crate::i18n::tr(
                        "show-the-older-layout-with-a-header-a-thin-bar-and-a-footer",
                    )),
                    |s| !s.compact_usage_cards,
                    |s, v| s.compact_usage_cards = !v,
                ),
                bool_row(
                    self,
                    k,
                    cx,
                    "customize-account-name",
                    crate::i18n::tr("show-account-name"),
                    None,
                    |s| s.show_account_name,
                    |s, v| s.show_account_name = v,
                ),
            ]
        });
        let presentation = self.settings.total_spend_presentation;
        let widget = kit::card_of(k, |k| {
            vec![
                bool_row(
                    self,
                    k,
                    cx,
                    "customize-spend-home",
                    crate::i18n::tr("show-on-home-tab"),
                    None,
                    |s| s.show_total_spend_on_all_tab,
                    |s, v| s.show_total_spend_on_all_tab = v,
                ),
                Row::new("customize-spend-layout", crate::i18n::tr("layout"))
                    .trailing(kit::segmented(
                        k,
                        "customize-spend-layout",
                        &[crate::i18n::tr("donut"), crate::i18n::tr("cards")],
                        presentation.index().max(0) as usize,
                        !self.settings.show_total_spend_on_all_tab,
                        Self::h(cx, |this, index: usize, _, cx| {
                            let value = TotalSpendPresentation::from_index(index as i32);
                            this.edit(cx, move |settings| {
                                settings.total_spend_presentation = value
                            })
                        }),
                    ))
                    .render(k),
            ]
        });
        vec![
            kit::section_heading(k, crate::i18n::tr("layout")),
            layout,
            kit::section_heading(k, crate::i18n::tr("tabs")),
            tabs,
            kit::section_heading(k, crate::i18n::tr("cards")),
            cards,
            kit::section_heading(k, crate::i18n::tr("usage-widget")),
            widget,
        ]
    }

    /// Popup card visibility for one instance's page. Card visibility is
    /// per instance; the Home column follows the General "Show on Home".
    pub(super) fn popup_cards_section(
        &mut self,
        instance: &ProviderInstance,
        k: &mut Kit,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let provider = instance.driver;
        let instance_id = instance.provider_id();
        let show_on_home = instance.show_on_home;
        let visibility = self.settings.popup_visibility.clone();
        let extra_ids = visibility
            .bricks
            .keys()
            .chain(self.discovered_bricks.keys())
            .cloned()
            .collect::<Vec<_>>();
        let theme = k.theme.clone();
        let header_cell = |label: &'static str| {
            div()
                .w(px(56.0))
                .flex()
                .justify_center()
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_secondary)
                .child(label)
        };
        let mut table = div().flex().flex_col().w_full().child(
            div()
                .flex()
                .items_center()
                .h(px(28.0))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_secondary)
                        .child(crate::i18n::tr("card")),
                )
                .child(header_cell(crate::i18n::tr("home")))
                .child(header_cell(crate::i18n::tr("tab"))),
        );
        for brick_id in crate::provider_registry::settings_brick_ids(provider, &extra_ids) {
            let current = visibility.instance_visibility_for(instance_id.id(), &brick_id);
            let label = crate::provider_registry::settings_brick_label(
                provider,
                &brick_id,
                &self.discovered_bricks,
            );
            let make = |all_tab: Option<bool>| {
                let brick_id = brick_id.clone();
                Self::h(cx, move |this, checked: bool, _, cx| {
                    let id = instance_id.id().to_owned();
                    let brick = brick_id.clone();
                    let now = this
                        .settings
                        .popup_visibility
                        .instance_visibility_for(&id, &brick);
                    let (all, tab) = match all_tab {
                        Some(_) => (checked, now.provider_tab),
                        None => (now.all_tab, checked),
                    };
                    this.edit(cx, move |settings| {
                        settings.popup_visibility.set_instance_brick(
                            id.clone(),
                            brick.clone(),
                            all,
                            tab,
                        )
                    })
                })
            };
            let home = kit::checkbox(
                k,
                format!("brick-home-{}-{brick_id}", instance_id.id()),
                current.all_tab,
                !show_on_home,
                None,
                make(Some(true)),
            );
            let tab = kit::checkbox(
                k,
                format!("brick-tab-{}-{brick_id}", instance_id.id()),
                current.provider_tab,
                false,
                None,
                make(None),
            );
            table = table.child(
                div()
                    .flex()
                    .items_center()
                    .h(px(34.0))
                    .border_t_1()
                    .border_color(theme.divider)
                    .child(div().flex_1().text_size(px(14.0)).child(label))
                    .child(div().w(px(56.0)).flex().justify_center().child(home))
                    .child(div().w(px(56.0)).flex().justify_center().child(tab)),
            );
        }
        let card_id = format!("popup-cards-{}", instance_id.id());
        let expanded = !self.is_expanded(&format!("{card_id}-collapsed"));
        let collapse_id = format!("{card_id}-collapsed");
        let on_toggle = Self::h(cx, move |this, open: bool, _, cx| {
            this.set_expanded(collapse_id.clone(), !open);
            cx.notify();
        });
        let header = Row::new(card_id.clone(), crate::i18n::tr("popup-cards")).description(
            k,
            if show_on_home {
                crate::i18n::tr("choose-which-cards-this-account-shows-on-home-and-on-its-own-tab")
            } else {
                crate::i18n::tr("choose-which-cards-this-account-shows-on-its-own-tab-turn-on-show")
            },
        );
        let table = table.into_any_element();
        kit::expander(k, card_id, header, expanded, on_toggle, move |_| table)
    }
}
