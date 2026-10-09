//! VS Code themes on the Appearance page: file import, the Open VSX browser
//! and install / remove of library entries.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use gpui::{
    Animation, AnimationExt, AnyElement, Context, FontWeight, InteractiveElement, IntoElement,
    ParentElement, PathPromptOptions, SharedString, StatefulInteractiveElement, Styled, Task,
    Transformation, Window, div, img, prelude::FluentBuilder, px, radians,
};

use super::appearance::popup_mock;
use super::kit::{self, Button, ButtonSize, Kit, Row, eid};
use super::window::SettingsWindow;
use crate::popup_window::ui::fx;
use crate::settings::PopupTheme;
use crate::vscode_themes::{self, ThemeSource, VsCodeTheme, open_vsx};

const SEARCH_INPUT: &str = "appearance-open-vsx-search";
const BROWSER_ID: &str = "appearance-open-vsx";
/// Parallel icon downloads after a search.
const ICON_WORKERS: usize = 4;
const ICON_SIZE: f32 = 40.0;
/// Logo pixels kept: `ICON_SIZE` at 200% scale.
const ICON_PIXELS: u32 = 80;
/// Preview tiles per row, matching the popup theme grid.
const PREVIEW_COLUMNS: u16 = 3;
const DIALOG_WIDTH: f32 = 640.0;
const DIALOG_HEIGHT: f32 = 690.0;

enum Icon {
    Loading,
    /// Already decoded and shrunk: GPUI caches an encoded `gpui::Image`
    /// app-wide at full size (store logos run to 1024 px), for good.
    Ready(Arc<gpui::RenderImage>),
    Missing,
}

/// A package downloaded for preview; installing reuses the parsed themes.
enum Preview {
    Loading,
    Ready(Vec<Arc<VsCodeTheme>>),
    Failed(SharedString),
}

/// Open VSX search state; lives as long as the Settings window.
#[derive(Default)]
pub(super) struct ThemeBrowser {
    /// The browser dialog is showing.
    open: bool,
    scroll: gpui::ScrollHandle,
    query: String,
    results: Option<Vec<open_vsx::Extension>>,
    /// Bumped per finished search so the list replays its entrance.
    revision: u64,
    searching: bool,
    error: Option<SharedString>,
    /// `Extension::key`s being downloaded.
    installing: HashSet<String>,
    importing: bool,
    search_task: Option<Task<()>>,
    /// Keyed by icon URL; kept across searches.
    icons: HashMap<String, Icon>,
    /// Keyed by `Extension::key`; kept across searches.
    previews: HashMap<String, Preview>,
    /// The result whose preview is open.
    previewing: Option<String>,
}

impl SettingsWindow {
    /// "VS Code themes" row with the file import button.
    pub(super) fn vscode_import_row(&mut self, k: &mut Kit, cx: &mut Context<Self>) -> AnyElement {
        let importing = self.theme_browser.importing;
        Row::new("appearance-vscode-themes", crate::i18n::tr("vscode-themes"))
            .description(k, crate::i18n::tr("vscode-themes-description"))
            .trailing(
                Button::new(
                    "appearance-vscode-import",
                    if importing {
                        crate::i18n::tr("importing-theme")
                    } else {
                        crate::i18n::tr("import-theme")
                    },
                )
                .with_icon("download-simple-fill")
                .disabled(importing)
                .on_click(Self::h(cx, |this, (), _, cx| this.import_vscode_theme(cx)))
                .render(k),
            )
            .render(k)
    }

    /// "Browse Open VSX" row; the browser itself is a dialog.
    pub(super) fn open_vsx_entry_row(&mut self, k: &mut Kit, cx: &mut Context<Self>) -> AnyElement {
        Row::new(BROWSER_ID, crate::i18n::tr("browse-open-vsx"))
            .description(k, crate::i18n::tr("browse-open-vsx-description"))
            .trailing(
                Button::new(
                    "appearance-open-vsx-open",
                    crate::i18n::tr("open-vsx-browse"),
                )
                .with_icon("magnifying-glass-bold")
                .on_click(Self::h(cx, |this, (), _, cx| this.open_open_vsx(cx)))
                .render(k),
            )
            .render(k)
    }

    fn open_open_vsx(&mut self, cx: &mut Context<Self>) {
        self.theme_browser.open = true;
        // The first open lists the most popular themes.
        if self.theme_browser.results.is_none() && !self.theme_browser.searching {
            let query = self.theme_browser.query.clone();
            self.search_open_vsx(query, cx);
        }
        cx.notify();
    }

    /// Closes the browser dialog and drops its popup preview; false when it
    /// was not open.
    pub(super) fn close_open_vsx(&mut self, cx: &mut Context<Self>) -> bool {
        if !std::mem::take(&mut self.theme_browser.open) {
            return false;
        }
        end_theme_preview(cx);
        cx.notify();
        true
    }

    pub(super) fn open_vsx_overlay(
        &mut self,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let open = self.theme_browser.open.then_some(());
        let ((), phase) = self.overlays.open_vsx.track(k, open)?;
        let close = Self::h(cx, |this, (), _, cx| {
            this.close_open_vsx(cx);
        });
        let (search_row, content) = self.open_vsx_body(k, window, cx);
        // Only the result list scrolls; the dialog keeps a fixed height
        // (capped to the window) so it stays still while results load.
        let list = div()
            .relative()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .id("open-vsx-list")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.theme_browser.scroll)
                    // Room for the scrollbar beside the row buttons.
                    .pr(px(12.0))
                    .child(content),
            )
            .children(kit::scrollbar(k, &self.theme_browser.scroll))
            .into_any_element();
        Some(kit::dialog_sized(
            k,
            "open-vsx",
            phase,
            DIALOG_WIDTH,
            Some(DIALOG_HEIGHT),
            vec![
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(kit::dialog_title(k, crate::i18n::tr("browse-open-vsx")))
                    .child(kit::caption(
                        k,
                        crate::i18n::tr("browse-open-vsx-description"),
                    ))
                    .into_any_element(),
                search_row,
                list,
            ],
            vec![
                Button::new("open-vsx-done", crate::i18n::tr("done"))
                    .full_width()
                    .on_click(close.clone())
                    .render(k),
            ],
            Some(close),
        ))
    }

    /// The search field, and the status or result list below it.
    fn open_vsx_body(
        &mut self,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (AnyElement, AnyElement) {
        let query = self.theme_browser.query.clone();
        let input = self.input(
            SEARCH_INPUT,
            &query,
            crate::i18n::tr("search-color-themes"),
            false,
            false,
            window,
            cx,
            |this, text, event, _, cx| {
                // Blur keeps the text; only Enter runs a search.
                this.theme_browser.query = text.clone();
                if matches!(event, super::input::InputEvent::Submit) {
                    this.search_open_vsx(text, cx);
                }
            },
        );
        let field = kit::text_field(k, &input, None, window, cx);
        let search = Button::icon_only("appearance-open-vsx-go", "magnifying-glass-bold")
            .tooltip(crate::i18n::tr("open-vsx-search"))
            .on_click(Self::h(cx, |this, (), _, cx| {
                let text = this
                    .inputs_text(SEARCH_INPUT, cx)
                    .unwrap_or_else(|| this.theme_browser.query.clone());
                this.theme_browser.query = text.clone();
                this.search_open_vsx(text, cx);
            }))
            .render(k);
        let search_row = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(div().flex_1().min_w_0().child(field))
            .child(search);

        let browser = &self.theme_browser;
        let status = if let Some(error) = &browser.error {
            let retry = Button::new(
                "appearance-open-vsx-retry",
                crate::i18n::tr("open-vsx-retry"),
            )
            .size(ButtonSize::Small)
            .on_click(Self::h(cx, |this, (), _, cx| {
                let query = this.theme_browser.query.clone();
                this.search_open_vsx(query, cx);
            }))
            .render(k);
            Some(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_start()
                            .gap(px(8.0))
                            .child(div().pt(px(2.0)).child(kit::icon(
                                "warning-fill",
                                14.0,
                                k.theme.caution,
                            )))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        kit::text(
                                            crate::i18n::tr("open-vsx-load-failed"),
                                            13.0,
                                            k.theme.text,
                                        )
                                        .line_height(px(18.0)),
                                    )
                                    .child(kit::caption(k, error.clone())),
                            ),
                    )
                    .child(retry)
                    .into_any_element(),
            )
        } else if browser.searching && browser.results.is_none() {
            Some(kit::caption(k, crate::i18n::tr("open-vsx-searching")))
        } else if browser.results.as_ref().is_some_and(Vec::is_empty) {
            Some(kit::caption(k, crate::i18n::tr("no-themes-found")))
        } else {
            None
        };

        let installed = vscode_themes::installed();
        let searching = browser.searching;
        let revision = browser.revision;
        let results = browser
            .results
            .clone()
            .filter(|results| !results.is_empty())
            .map(|results| {
                let mut rows = Vec::with_capacity(results.len() * 2);
                for (index, extension) in results.iter().enumerate() {
                    let installed_version =
                        installed.iter().find_map(|theme| match &theme.source {
                            ThemeSource::OpenVsx {
                                namespace,
                                name,
                                version,
                            } if namespace.eq_ignore_ascii_case(&extension.namespace)
                                && name.eq_ignore_ascii_case(&extension.name) =>
                            {
                                Some(version.clone())
                            }
                            _ => None,
                        });
                    let key = extension.key();
                    let installing = self.theme_browser.installing.contains(&key);
                    rows.push(self.open_vsx_row(
                        k,
                        index,
                        extension,
                        installed_version,
                        installing,
                        cx,
                    ));
                    let open = self.theme_browser.previewing.as_deref() == Some(key.as_str());
                    let panel_key = fx::key(("open-vsx-preview", key.as_str()));
                    rows.extend(kit::collapsible(k, panel_key, open, |k| {
                        self.open_vsx_preview(k, extension, cx)
                    }));
                }
                let list = div()
                    .flex()
                    .flex_col()
                    .when(searching, |el| el.opacity(0.5))
                    .children(rows)
                    .into_any_element();
                kit::appear(k, format!("open-vsx-results-{revision}"), list)
            });

        let content = div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .children(status)
            .children(results)
            .into_any_element();
        (search_row.into_any_element(), content)
    }

    fn open_vsx_row(
        &self,
        k: &mut Kit,
        index: usize,
        extension: &open_vsx::Extension,
        installed_version: Option<String>,
        installing: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = extension.key();
        let open = self.theme_browser.previewing.as_deref() == Some(key.as_str());
        let turn =
            k.fx.toggle(fx::key(("open-vsx-caret", key.as_str())), open, fx::NORMAL);
        let theme = &k.theme;
        let id = format!("open-vsx-{key}");
        let up_to_date = installed_version.as_deref() == Some(extension.version.as_str());
        let (label, accent) = match (&installed_version, installing) {
            (_, true) => (crate::i18n::tr("open-vsx-installing"), false),
            (Some(_), false) if up_to_date => (crate::i18n::tr("open-vsx-installed"), false),
            (Some(_), false) => (crate::i18n::tr("update"), true),
            (None, false) => (crate::i18n::tr("open-vsx-install"), true),
        };
        let mut button = Button::new(format!("{id}-install"), label)
            .size(ButtonSize::Small)
            .disabled(installing || up_to_date);
        if accent {
            button = button.accent();
        }
        if up_to_date {
            button = button.with_icon("check-bold");
        }
        let target = extension.clone();
        let button = button
            .on_click(Self::h(cx, move |this, (), _, cx| {
                this.install_from_open_vsx(target.clone(), cx)
            }))
            .render(k);
        let meta = format!(
            "{} · {} · v{}",
            extension.namespace,
            compact_count(extension.download_count),
            extension.version
        );
        let caret = div()
            .size(px(28.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .child(
                kit::icon("caret-down-bold", 12.0, theme.text_secondary).with_transformation(
                    Transformation::rotate(radians(std::f32::consts::PI * turn)),
                ),
            );
        let target = extension.clone();
        div()
            .id(eid(format!("{id}-row")))
            .flex()
            .items_center()
            .gap(px(12.0))
            .py(px(10.0))
            .cursor_pointer()
            .when(index > 0, |el| el.border_t_1().border_color(theme.divider))
            .on_click(
                cx.listener(move |this, _, _, cx| this.toggle_open_vsx_preview(target.clone(), cx)),
            )
            .child(self.open_vsx_icon(k, extension))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .child(extension.display_name.clone()),
                            )
                            .when(extension.verified, |el| {
                                el.child(kit::icon("seal-check-fill", 14.0, theme.accent_text))
                            }),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(12.0))
                            .text_color(theme.text_tertiary)
                            .child(meta),
                    )
                    .when(!extension.description.is_empty(), |el| {
                        el.child(
                            div()
                                .truncate()
                                .text_size(px(13.0))
                                .text_color(theme.text_secondary)
                                .child(extension.description.clone()),
                        )
                    }),
            )
            .child(button)
            .child(caret)
            .into_any_element()
    }

    /// The extension logo, or its initial on a tile when there is none.
    fn open_vsx_icon(&self, k: &Kit, extension: &open_vsx::Extension) -> AnyElement {
        let theme = &k.theme;
        let icon = extension
            .icon_url
            .as_ref()
            .and_then(|url| self.theme_browser.icons.get(url));
        if let (Some(Icon::Ready(image)), Some(url)) = (icon, &extension.icon_url) {
            let logo = div()
                .size(px(ICON_SIZE))
                .flex_none()
                .child(img(image.clone()).size(px(ICON_SIZE)).rounded(px(8.0)));
            // Fade in place: `kit::appear` wraps in a full-width box, which
            // would take the row's space from the text beside the logo.
            if !k.animate() {
                return logo.into_any_element();
            }
            return logo
                .with_animation(
                    eid(format!("open-vsx-icon-{url}")),
                    Animation::new(Duration::from_millis(220)).with_easing(fx::ease_out_cubic),
                    |el, delta| el.opacity(delta),
                )
                .into_any_element();
        }
        let initial = extension
            .display_name
            .chars()
            .find(|c| c.is_alphanumeric())
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_default();
        div()
            .size(px(ICON_SIZE))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(8.0))
            .bg(theme.card_hover)
            .text_size(px(16.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.text_tertiary)
            // A blank tile while the logo loads avoids flashing the letter.
            .when(!matches!(icon, Some(Icon::Loading)), |el| el.child(initial))
            .into_any_element()
    }

    /// Mini popups for every theme in the package; clicking one previews it
    /// in the popup without installing anything.
    fn open_vsx_preview(
        &self,
        k: &mut Kit,
        extension: &open_vsx::Extension,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let body = match self.theme_browser.previews.get(&extension.key()) {
            None | Some(Preview::Loading) => {
                kit::caption(k, crate::i18n::tr("open-vsx-preview-loading"))
            }
            Some(Preview::Failed(error)) => div()
                .flex()
                .items_start()
                .gap(px(8.0))
                .child(
                    div()
                        .pt(px(1.0))
                        .child(kit::icon("warning-fill", 14.0, k.theme.caution)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(kit::caption(k, error.clone())),
                )
                .into_any_element(),
            Some(Preview::Ready(themes)) => {
                let previewing = vscode_themes::preview();
                let accent = crate::theme::accent_ramp(self.settings.accent_color);
                let tiles = themes
                    .iter()
                    .map(|vscode| {
                        let selected = previewing
                            .as_ref()
                            .is_some_and(|preview| Arc::ptr_eq(preview, vscode));
                        self.open_vsx_preview_tile(k, vscode, selected, accent, cx)
                    })
                    .collect::<Vec<_>>();
                let grid = div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(kit::caption(k, crate::i18n::tr("open-vsx-preview-hint")))
                    .child(
                        div()
                            .grid()
                            .grid_cols(PREVIEW_COLUMNS)
                            .gap(px(12.0))
                            .children(tiles),
                    )
                    .into_any_element();
                kit::appear(k, format!("open-vsx-preview-{}", extension.key()), grid)
            }
        };
        div().pb(px(14.0)).child(body).into_any_element()
    }

    fn open_vsx_preview_tile(
        &self,
        k: &Kit,
        vscode: &Arc<VsCodeTheme>,
        selected: bool,
        accent: crate::theme::AccentRamp,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = &k.theme;
        let palette = self.vscode_palette(vscode.clone(), accent);
        let tile_id = format!("open-vsx-preview-{}", vscode.id);
        let rest = if selected {
            theme.accent_soft
        } else {
            theme.card
        };
        let target = vscode.clone();
        kit::hover_bg(
            k,
            div().id(eid(tile_id.clone())),
            kit::hover_key(&tile_id),
            rest,
            if selected { rest } else { theme.card_hover },
        )
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .p(px(8.0))
        .rounded(px(kit::CARD_RADIUS))
        .shadow(kit::card_shadow(theme))
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _, cx| this.toggle_theme_preview(target.clone(), cx)))
        .child(popup_mock(k, &palette))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.0))
                        .when(selected, |el| el.font_weight(FontWeight::SEMIBOLD))
                        .child(vscode.label.clone()),
                )
                .child(self.install_one_button(k, vscode, cx)),
        )
        .into_any_element()
    }

    /// Installs just this theme rather than the whole package.
    fn install_one_button(
        &self,
        k: &Kit,
        vscode: &Arc<VsCodeTheme>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let installed = vscode_themes::get(&vscode.id).is_some();
        let installing = self
            .theme_browser
            .installing
            .contains(&single_install_key(&vscode.id));
        let (icon, tooltip) = if installed {
            ("check-bold", crate::i18n::tr("open-vsx-installed"))
        } else {
            (
                "download-simple-fill",
                crate::i18n::tr("open-vsx-install-one"),
            )
        };
        let target = vscode.clone();
        Button::icon_only(format!("open-vsx-install-one-{}", vscode.id), icon)
            .ghost()
            .size(ButtonSize::Small)
            .tooltip(tooltip)
            .disabled(installed || installing)
            .on_click(Self::h(cx, move |this, (), _, cx| {
                this.install_one_theme(target.clone(), cx)
            }))
            .render(k)
    }

    fn install_one_theme(&mut self, theme: Arc<VsCodeTheme>, cx: &mut Context<Self>) {
        let key = single_install_key(&theme.id);
        if !self.theme_browser.installing.insert(key.clone()) {
            return;
        }
        self.theme_browser.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { vscode_themes::install(vec![(*theme).clone()]) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.theme_browser.installing.remove(&key);
                this.finish_theme_install(result, cx);
            });
        })
        .detach();
    }

    fn toggle_open_vsx_preview(&mut self, extension: open_vsx::Extension, cx: &mut Context<Self>) {
        let key = extension.key();
        let browser = &mut self.theme_browser;
        if browser.previewing.as_deref() == Some(key.as_str()) {
            browser.previewing = None;
            cx.notify();
            return;
        }
        browser.previewing = Some(key.clone());
        // A ready or in-flight preview is reused; a failed one is retried.
        let fetch = matches!(browser.previews.get(&key), None | Some(Preview::Failed(_)));
        if fetch {
            browser.previews.insert(key.clone(), Preview::Loading);
        }
        cx.notify();
        if !fetch {
            return;
        }
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { open_vsx::download(&extension) })
                .await;
            let _ = this.update(cx, |this, cx| {
                let preview = match result {
                    Ok(themes) if themes.is_empty() => {
                        Preview::Failed(crate::i18n::tr("open-vsx-preview-empty").into())
                    }
                    Ok(themes) => Preview::Ready(themes.into_iter().map(Arc::new).collect()),
                    Err(error) => {
                        eprintln!("Open VSX preview failed: {error:#}");
                        Preview::Failed(format!("{error:#}").into())
                    }
                };
                this.theme_browser.previews.insert(key, preview);
                cx.notify();
            });
        })
        .detach();
    }

    /// Paint the popup with `theme` without installing it; clicking the
    /// previewed theme again ends the preview.
    fn toggle_theme_preview(&mut self, theme: Arc<VsCodeTheme>, cx: &mut Context<Self>) {
        let active = vscode_themes::preview().is_some_and(|current| Arc::ptr_eq(&current, &theme));
        vscode_themes::set_preview((!active).then_some(theme));
        cx.refresh_windows();
        cx.notify();
    }

    /// Fetch logos for search results that have not been requested yet.
    fn load_open_vsx_icons(&mut self, cx: &mut Context<Self>) {
        let browser = &mut self.theme_browser;
        let urls = browser
            .results
            .iter()
            .flatten()
            .filter_map(|extension| extension.icon_url.clone())
            .filter(|url| !browser.icons.contains_key(url))
            .collect::<Vec<_>>();
        let mut queues = vec![Vec::new(); ICON_WORKERS];
        for (index, url) in urls.into_iter().enumerate() {
            browser.icons.insert(url.clone(), Icon::Loading);
            queues[index % ICON_WORKERS].push(url);
        }
        for queue in queues.into_iter().filter(|queue| !queue.is_empty()) {
            cx.spawn(async move |this, cx| {
                for url in queue {
                    let image = cx
                        .background_executor()
                        .spawn({
                            let url = url.clone();
                            async move { decode_icon(&open_vsx::fetch_icon(&url).ok()?) }
                        })
                        .await;
                    let icon = match image {
                        Some(image) => Icon::Ready(Arc::new(image)),
                        None => Icon::Missing,
                    };
                    let updated = this.update(cx, |this, cx| {
                        this.theme_browser.icons.insert(url, icon);
                        cx.notify();
                    });
                    if updated.is_err() {
                        return;
                    }
                }
            })
            .detach();
        }
    }

    fn inputs_text(&self, id: &str, cx: &Context<Self>) -> Option<String> {
        self.input_entity(id).map(|input| input.read(cx).text())
    }

    fn search_open_vsx(&mut self, query: String, cx: &mut Context<Self>) {
        let browser = &mut self.theme_browser;
        browser.searching = true;
        browser.error = None;
        let search = cx.background_executor().spawn({
            let query = query.clone();
            async move { open_vsx::search(&query) }
        });
        // Replacing the task drops (cancels) a search still in flight.
        browser.search_task = Some(cx.spawn(async move |this, cx| {
            let result = search.await;
            let _ = this.update(cx, |this, cx| {
                let browser = &mut this.theme_browser;
                browser.searching = false;
                browser.search_task = None;
                match result {
                    Ok(results) => {
                        browser.results = Some(results);
                        browser.revision += 1;
                        // New results start from the top.
                        browser.scroll.set_offset(gpui::Point::default());
                        this.load_open_vsx_icons(cx);
                    }
                    Err(error) => {
                        eprintln!("Open VSX search failed: {error:#}");
                        browser.error = Some(format!("{error:#}").into());
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn install_from_open_vsx(&mut self, extension: open_vsx::Extension, cx: &mut Context<Self>) {
        let key = extension.key();
        if !self.theme_browser.installing.insert(key.clone()) {
            return;
        }
        self.theme_browser.error = None;
        // A package already downloaded for its preview is not fetched again.
        let downloaded = match self.theme_browser.previews.get(&key) {
            Some(Preview::Ready(themes)) => Some(
                themes
                    .iter()
                    .map(|theme| (**theme).clone())
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        };
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    match downloaded {
                        Some(themes) => vscode_themes::install(themes),
                        None => open_vsx::install(&extension),
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.theme_browser.installing.remove(&key);
                this.finish_theme_install(result, cx);
            });
        })
        .detach();
    }

    fn import_vscode_theme(&mut self, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(crate::i18n::tr("import").into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = prompt.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.theme_browser.importing = true;
                cx.notify();
            });
            let result = cx
                .background_executor()
                .spawn(async move { vscode_themes::import_file(&path) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.theme_browser.importing = false;
                this.finish_theme_install(result, cx);
            });
        })
        .detach();
    }

    /// Select the first installed theme, so installing is also applying.
    fn finish_theme_install(
        &mut self,
        result: anyhow::Result<Vec<String>>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(ids) => {
                // Keep the theme being previewed when it is in the package;
                // the saved choice takes over from the preview either way.
                let previewed = vscode_themes::preview()
                    .map(|theme| theme.id.clone())
                    .filter(|id| ids.contains(id));
                if let Some(id) = previewed.or_else(|| ids.into_iter().next()) {
                    self.select_vscode_theme(id, cx);
                }
                end_theme_preview(cx);
                self.show_notice(crate::i18n::tr("theme-installed"), cx);
            }
            Err(error) => {
                eprintln!("failed to install a VS Code theme: {error:#}");
                crate::notifications::show(
                    crate::i18n::tr("theme-install-failed"),
                    &format!("{error:#}"),
                );
            }
        }
        // Other windows rebuild palettes from the updated library.
        cx.refresh_windows();
        cx.notify();
    }

    pub(super) fn select_vscode_theme(&mut self, id: String, cx: &mut Context<Self>) {
        self.edit(cx, move |settings| {
            settings.popup_theme = PopupTheme::VsCode;
            settings.popup_vscode_theme = Some(id.clone());
        });
    }

    pub(super) fn remove_vscode_theme(&mut self, id: String, cx: &mut Context<Self>) {
        let selected = self.settings.popup_theme == PopupTheme::VsCode
            && self.settings.popup_vscode_theme.as_deref() == Some(id.as_str());
        if let Err(error) = vscode_themes::remove(&id) {
            eprintln!("failed to remove VS Code theme {id}: {error:#}");
            crate::notifications::show(
                crate::i18n::tr("theme-remove-failed"),
                &format!("{error:#}"),
            );
            return;
        }
        if selected {
            self.edit(cx, |settings| {
                settings.popup_theme = PopupTheme::Fluent;
                settings.popup_vscode_theme = None;
            });
        }
        cx.refresh_windows();
        cx.notify();
    }
}

/// `ThemeBrowser::installing` entry of a single theme, apart from the
/// `Extension::key`s of whole packages.
fn single_install_key(theme_id: &str) -> String {
    format!("theme:{theme_id}")
}

/// Drop a theme preview and repaint the popup with the saved design.
pub(super) fn end_theme_preview(cx: &mut gpui::App) {
    if vscode_themes::set_preview(None) {
        cx.refresh_windows();
    }
}

/// Marketplace icons should be PNG, but older packages ship JPEG or SVG;
/// sniff the bytes rather than trust the file name.
/// A logo as BGRA pixels at most `ICON_PIXELS` on a side. Only the first
/// frame of an animated logo is kept.
fn decode_icon(bytes: &[u8]) -> Option<gpui::RenderImage> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]);
    let image = if head.contains("<svg") {
        rasterize_svg(bytes)?
    } else {
        image::load_from_memory(bytes).ok()?
    };
    let image = if image.width() > ICON_PIXELS || image.height() > ICON_PIXELS {
        image.resize(
            ICON_PIXELS,
            ICON_PIXELS,
            image::imageops::FilterType::Triangle,
        )
    } else {
        image
    };
    let mut pixels = image.into_rgba8();
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Some(gpui::RenderImage::new(smallvec::smallvec![
        image::Frame::new(pixels)
    ]))
}

/// An SVG logo drawn straight at `ICON_PIXELS`.
fn rasterize_svg(bytes: &[u8]) -> Option<image::DynamicImage> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_data(bytes, &usvg::Options::default()).ok()?;
    let size = tree.size();
    let scale = ICON_PIXELS as f32 / size.width().max(size.height());
    let mut pixmap = tiny_skia::Pixmap::new(
        ((size.width() * scale).ceil() as u32).max(1),
        ((size.height() * scale).ceil() as u32).max(1),
    )?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let (width, height) = (pixmap.width(), pixmap.height());
    let straight = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    image::RgbaImage::from_raw(width, height, straight).map(image::DynamicImage::ImageRgba8)
}

/// `1.2M`, `34K`, `512` downloads.
fn compact_count(count: u64) -> String {
    let arrow = "\u{2193}";
    match count {
        0..=999 => format!("{arrow} {count}"),
        1_000..=999_999 => format!("{arrow} {:.0}K", count as f64 / 1_000.0),
        _ => format!("{arrow} {:.1}M", count as f64 / 1_000_000.0),
    }
}
