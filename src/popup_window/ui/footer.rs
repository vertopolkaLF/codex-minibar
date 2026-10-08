//! Footer: page tabs on the left, refresh/settings/update on the right.

use std::rc::Rc;

use gpui::{
    AnyElement, AppContext, ClickEvent, Context, DragMoveEvent, Hsla, InteractiveElement,
    IntoElement, ParentElement, Render, ScrollWheelEvent, SharedString, StatefulInteractiveElement,
    Styled, Transformation, Window, div, px, radians,
};

use super::{
    components::{self, icon},
    fx,
    root::{PopupRoot, eid, view_key},
    theme::{HslaExt, Palette},
};
use crate::popup_window::{model, *};

/// Payload carried while a provider tab is dragged to a new position.
#[derive(Clone)]
pub(super) struct TabDrag {
    view: PopupView,
    icon: &'static str,
    badge: Option<crate::instances::Badge>,
    color: Hsla,
    palette: Palette,
    size: f32,
    glyph: f32,
}

impl Render for TabDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size(px(self.size))
            .rounded(px(6.0))
            .bg(self.palette.solid_background.opacity(0.92))
            .border_1()
            .border_color(self.palette.accent.opacity(0.6))
            .shadow_md()
            .flex()
            .items_center()
            .justify_center()
            .child(components::provider_mark(
                self.icon,
                self.glyph,
                self.color,
                self.badge.as_ref(),
                &self.palette,
            ))
    }
}

struct TabSpec {
    id: String,
    icon: &'static str,
    /// Driver whose brand tints the icon; `None` for Home and Usage.
    driver: Option<ProviderKind>,
    badge: Option<crate::instances::Badge>,
    tip: String,
    view: PopupView,
    has_error: bool,
    selected: bool,
}

impl PopupRoot {
    fn tab_specs(&self) -> Vec<TabSpec> {
        let ui = Rc::clone(&self.ui);
        let current = self.pager.current;
        let mut tabs = vec![TabSpec {
            id: "home".into(),
            icon: "fluent-home",
            driver: None,
            badge: None,
            tip: crate::i18n::tr("home").into(),
            view: PopupView::Home,
            has_error: false,
            selected: current == PopupView::Home,
        }];
        if ui.usage_stats_enabled {
            tabs.push(TabSpec {
                id: "usage".into(),
                icon: "fluent-chart",
                driver: None,
                badge: None,
                tip: crate::i18n::tr("usage-0bb186").into(),
                view: PopupView::Usage,
                has_error: false,
                selected: current == PopupView::Usage,
            });
        }
        if !self.show_provider_tabs() {
            return tabs;
        }
        // Every tab id carries its instance (or driver) identity, so a slot
        // can never keep another provider's icon, badge or label.
        for view in model::provider_tabs(&ui) {
            let spec = match view {
                PopupView::Provider(provider) => TabSpec {
                    id: format!("instance-{}", provider.id()),
                    icon: crate::provider_registry::icon(provider.kind()),
                    driver: Some(provider.kind()),
                    badge: provider.badge(),
                    tip: provider.qualified_name(),
                    view,
                    has_error: ui.has_provider_error(provider),
                    selected: current == view,
                },
                PopupView::Group(driver) => TabSpec {
                    id: format!("group-{}", driver.id()),
                    icon: crate::provider_registry::icon(driver),
                    driver: Some(driver),
                    badge: None,
                    tip: driver.display_name().into(),
                    view,
                    has_error: tab_members(&ui.instances, view)
                        .into_iter()
                        .any(|member| ui.has_provider_error(member)),
                    selected: current == view,
                },
                PopupView::Home | PopupView::Usage => continue,
            };
            tabs.push(spec);
        }
        tabs
    }

    pub(super) fn render_footer(
        &mut self,
        capsule_width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let size = crate::popup::bottom_bar_size();
        let button = size.icon_button_size() as f32;
        let spacing = size.tab_spacing() as f32;
        let palette = self.palette.clone();
        let radius = crate::popup::corner_radius_dip() as f32;
        let tabs = self.tab_specs();
        let step = button + spacing;
        let content_width = tabs.len() as f32 * step - spacing;
        let action_count = 3.0 + f32::from(u8::from(self.ui.update_version.is_some()));
        let viewport_width = capsule_width
            - 2.0
            - size.tab_padding_left() as f32
            - size.padding_right() as f32
            - size.column_spacing() as f32
            - (button * action_count + size.action_spacing() as f32 * (action_count - 1.0));
        let max_scroll = (content_width - viewport_width).max(0.0);
        self.tab_scroll = self.tab_scroll.clamp(0.0, max_scroll);
        let scroll = self
            .fx
            .value(fx::key("tab-scroll"), self.tab_scroll, fx::FAST);

        let mut strip = div()
            .absolute()
            .top_0()
            .left(px(-scroll))
            .h(px(button))
            .w(px(content_width.max(0.0)));
        let selected_index = tabs.iter().position(|tab| tab.selected);
        // One sliding selection indicator instead of per-tab fades.
        if let Some(index) = selected_index {
            let resting_width = button - 18.0;
            let (x, indicator_width) = self.fx.indicator(
                fx::key("tab-indicator"),
                index as f32 * step,
                resting_width,
                std::time::Duration::from_millis(300),
            );
            strip = strip.child(
                div()
                    .absolute()
                    .bottom_0()
                    .left(px(x + 9.0 - (indicator_width - resting_width) * 0.5))
                    .w(px(indicator_width))
                    .h(px(3.0))
                    .rounded(px(1.5))
                    .bg(palette.accent),
            );
        }
        let reorderable: Vec<PopupView> = model::provider_tabs(&self.ui);
        for (index, tab) in tabs.into_iter().enumerate() {
            let element = self.render_tab(tab, index, step, button, size, &reorderable, window, cx);
            strip = strip.child(element);
        }

        let tabs_viewport = div()
            .id("footer-tabs")
            .relative()
            .flex_1()
            .min_w_0()
            .h(px(button))
            .overflow_hidden()
            .on_scroll_wheel(cx.listener(move |this, event: &ScrollWheelEvent, _, cx| {
                let delta = event.delta.pixel_delta(px(48.0));
                let dx = f32::from(delta.x) - f32::from(delta.y);
                let next = (this.tab_scroll + dx).clamp(0.0, max_scroll);
                if (next - this.tab_scroll).abs() > 0.5 {
                    this.tab_scroll = next;
                    cx.notify();
                }
            }))
            .child(strip);

        let editing = self.home_editing;
        let mut actions = div()
            .flex()
            .flex_row()
            .items_center()
            .flex_none()
            .gap(px(size.action_spacing() as f32))
            .child(self.render_refresh_button(button, size.icon_glyph_size() as f32, cx))
            .child(self.render_action_button(
                "edit-home",
                if editing {
                    "fluent-checkmark"
                } else {
                    "fluent-edit"
                },
                crate::i18n::tr(if editing { "done" } else { "edit-home" }).into(),
                button,
                size.icon_glyph_size() as f32,
                editing,
                cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_home_editing(cx)),
                cx,
            ))
            .child(self.render_action_button(
                "settings",
                "fluent-settings",
                crate::i18n::tr("settings").into(),
                button,
                size.icon_glyph_size() as f32,
                false,
                cx.listener(|this, _: &ClickEvent, _, _| this.open_settings()),
                cx,
            ));
        if self.ui.update_version.is_some() {
            actions = actions.child(self.render_action_button(
                "update",
                "fluent-arrow-download",
                crate::i18n::tr("install-update").into(),
                button,
                size.icon_glyph_size() as f32,
                true,
                cx.listener(|this, _: &ClickEvent, _, _| this.install_update()),
                cx,
            ));
        }

        div()
            .flex()
            .flex_row()
            .items_center()
            .flex_none()
            .w_full()
            .h(px(size.footer_height_dip() as f32))
            .pl(px(size.tab_padding_left() as f32))
            .pr(px(size.padding_right() as f32))
            .gap(px(size.column_spacing() as f32))
            .border_t_1()
            .border_color(palette.card_stroke)
            .bg(palette.footer_background)
            .rounded_b(px((radius - 1.0).max(0.0)))
            .child(tabs_viewport)
            .child(actions)
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn render_tab(
        &mut self,
        tab: TabSpec,
        index: usize,
        step: f32,
        button: f32,
        size: crate::settings::BottomBarSize,
        reorderable: &[PopupView],
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let glyph = size.icon_glyph_size() as f32;
        let hover_id = fx::key(("tab", tab.id.as_str()));
        let hovered = self.hovered(hover_id);
        let hover = self
            .fx
            .toggle(fx::key(("tab-hover", hover_id)), hovered, fx::FASTER);
        // Slots glide to their new place after a reorder or membership change.
        let x = self.fx.value(
            fx::key(("tab-x", tab.id.as_str())),
            index as f32 * step,
            fx::NORMAL,
        );
        let colored = self.ui.use_colored_provider_icons;
        let idle = palette.tab_icon(tab.driver, colored);
        let icon_color = if colored && tab.driver.is_some() {
            idle
        } else {
            idle.mix(palette.chrome_icon_hover, hover)
        };
        let dragging_this = self.tab_drag == Some(tab.view);
        let drop_target =
            self.tab_drag.is_some() && self.tab_drop == Some(tab.view) && !dragging_this;

        let view = tab.view;
        let mut element = div()
            .id(eid(format!("tab-{}", tab.id)))
            .absolute()
            .top_0()
            .left(px(x))
            .size(px(button))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(palette.control_radius))
            .opacity(if dragging_this { 0.45 } else { 1.0 })
            .on_hover(self.hover_listener(hover_id, Some(tab.tip.clone().into()), cx))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.navigate(view, cx);
            }));
        if let Some(layer) = components::hover_layer(&palette, hover, 4.0) {
            element = element.child(layer);
        }
        if drop_target {
            element = element.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(palette.control_radius))
                    .border_1()
                    .border_color(palette.accent),
            );
        }
        element = element.child(components::provider_mark(
            tab.icon,
            glyph,
            icon_color,
            tab.badge.as_ref(),
            &palette,
        ));
        if tab.has_error {
            element = element.child(
                div()
                    .absolute()
                    .top(px(2.0))
                    .right(px(2.0))
                    .child(components::error_badge(13.0, &palette)),
            );
        }
        if reorderable.contains(&tab.view) {
            let target = tab.view;
            let drag = TabDrag {
                view: target,
                icon: tab.icon,
                badge: tab.badge.clone(),
                color: icon_color,
                palette: palette.clone(),
                size: button,
                glyph,
            };
            let root = cx.entity();
            element = element
                .on_drag(drag, move |drag, _, _, cx| {
                    let dragged = drag.view;
                    root.update(cx, |root, cx| {
                        root.tab_drag = Some(dragged);
                        root.tip = None;
                        cx.notify();
                    });
                    cx.new(|_| drag.clone())
                })
                .on_drag_move(
                    cx.listener(move |this, event: &DragMoveEvent<TabDrag>, _, cx| {
                        if event.bounds.contains(&event.event.position)
                            && this.tab_drop != Some(target)
                        {
                            this.tab_drop = Some(target);
                            cx.notify();
                        }
                    }),
                )
                .on_drop(cx.listener(move |this, drag: &TabDrag, _, cx| {
                    this.reorder_provider_tab(drag.view, target, cx);
                }));
        }
        let _ = view_key;
        element.into_any_element()
    }

    /// Edit mode belongs to Home: entering it from another page opens Home.
    fn toggle_home_editing(&mut self, cx: &mut Context<Self>) {
        if self.home_editing {
            self.home_editing = false;
            cx.notify();
            return;
        }
        self.navigate(PopupView::Home, cx);
        self.home_editing = true;
    }

    fn reorder_provider_tab(&mut self, from: PopupView, to: PopupView, cx: &mut Context<Self>) {
        self.tab_drag = None;
        self.tab_drop = None;
        let tabs = model::provider_tabs(&self.ui);
        let Some(order) = reordered_instance_ids(&self.ui.instances, &tabs, from, to) else {
            cx.notify();
            return;
        };
        let local_order = order.clone();
        self.persist(
            cx,
            move |ui| {
                let mut remaining = std::mem::take(&mut ui.instances);
                for id in &local_order {
                    if let Some(index) = remaining.iter().position(|instance| &instance.id == id) {
                        ui.instances.push(remaining.remove(index));
                    }
                }
                ui.instances.append(&mut remaining);
            },
            move |settings| {
                settings.apply_instance_order(&order);
            },
        );
    }

    fn render_refresh_button(
        &mut self,
        button: f32,
        glyph: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let refreshing = self.ui.refreshing;
        let hover_id = fx::key("action-refresh");
        let hovered = self.hovered(hover_id);
        let hover = self
            .fx
            .toggle(fx::key(("action-hover", hover_id)), hovered, fx::FASTER);
        let emphasis = self
            .fx
            .toggle(fx::key("refresh-emphasis"), hovered || refreshing, fx::FAST);
        let rotation = match (refreshing, self.refresh_started) {
            (true, Some(started)) if self.fx.enabled() => {
                refresh_rotation_at(started.elapsed()) as f32
            }
            _ => 0.0,
        };
        let tooltip = if refreshing {
            crate::i18n::tr("refreshing-limits-and-usage").to_owned()
        } else {
            let updated = format_last_updated(model::latest_sampled_at(&self.limits), 0);
            let relative = updated
                .strip_prefix(crate::i18n::tr("updated"))
                .unwrap_or(&updated);
            crate::i18n::format(
                "refresh-last-updated-relative",
                &[("relative", relative.to_string())],
            )
        };
        let mut element = div()
            .id("action-refresh")
            .relative()
            .size(px(button))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.0))
            .on_hover(self.hover_listener(hover_id, Some(tooltip.into()), cx))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.refresh(cx)));
        if let Some(layer) = components::hover_layer(&palette, hover, 6.0) {
            element = element.child(layer);
        }
        element
            .child(
                icon(
                    "fluent-refresh",
                    glyph,
                    palette.chrome_icon.mix(palette.accent, emphasis),
                )
                .with_transformation(Transformation::rotate(radians(rotation.to_radians()))),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn render_action_button(
        &mut self,
        id: &'static str,
        glyph_name: &'static str,
        tooltip: SharedString,
        button: f32,
        glyph: f32,
        always_accent: bool,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette.clone();
        let hover_id = fx::key(("action", id));
        let hovered = self.hovered(hover_id);
        let hover = self
            .fx
            .toggle(fx::key(("action-hover", hover_id)), hovered, fx::FASTER);
        let emphasis = if always_accent {
            1.0
        } else {
            self.fx
                .toggle(fx::key(("action-emphasis", id)), hovered, fx::FAST)
        };
        let mut element = div()
            .id(eid(format!("action-{id}")))
            .relative()
            .size(px(button))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.0))
            .on_hover(self.hover_listener(hover_id, Some(tooltip), cx))
            .on_click(on_click);
        if let Some(layer) = components::hover_layer(&palette, hover, 6.0) {
            element = element.child(layer);
        }
        element
            .child(icon(
                glyph_name,
                glyph,
                palette.chrome_icon.mix(palette.accent, emphasis),
            ))
            .into_any_element()
    }
}
