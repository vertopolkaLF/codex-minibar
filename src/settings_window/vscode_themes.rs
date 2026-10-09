//! VS Code themes on the Appearance page: file import, the Open VSX browser
//! and install / remove of library entries.

use std::collections::HashSet;

use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, PathPromptOptions, SharedString,
    Styled, Task, Window, div, prelude::FluentBuilder, px,
};

use super::kit::{self, Button, ButtonSize, Kit, Row};
use super::window::SettingsWindow;
use crate::settings::PopupTheme;
use crate::vscode_themes::{self, ThemeSource, open_vsx};

const SEARCH_INPUT: &str = "appearance-open-vsx-search";
const BROWSER_ID: &str = "appearance-open-vsx";

/// Open VSX search state; lives as long as the Settings window.
#[derive(Default)]
pub(super) struct ThemeBrowser {
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

    /// Expandable Open VSX search card.
    pub(super) fn open_vsx_browser(
        &mut self,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let expanded = self.is_expanded(BROWSER_ID);
        let on_toggle = Self::h(cx, |this, open: bool, _, cx| {
            this.set_expanded(BROWSER_ID, open);
            // The first open lists the most popular themes.
            if open && this.theme_browser.results.is_none() && !this.theme_browser.searching {
                let query = this.theme_browser.query.clone();
                this.search_open_vsx(query, cx);
            }
            cx.notify();
        });
        let header = Row::new(BROWSER_ID, crate::i18n::tr("browse-open-vsx"))
            .description(k, crate::i18n::tr("browse-open-vsx-description"));
        // Built while collapsing too: the closing animation measures it.
        let body = self.open_vsx_body(k, window, cx);
        kit::expander(k, BROWSER_ID, header, expanded, on_toggle, move |_| body)
    }

    fn open_vsx_body(
        &mut self,
        k: &mut Kit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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
            Some(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(kit::icon("warning-fill", 14.0, k.theme.caution))
                    .child(kit::text(error.clone(), 13.0, k.theme.text_secondary))
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
        let results = browser
            .results
            .as_ref()
            .filter(|results| !results.is_empty())
            .map(|results| {
                let rows = results
                    .iter()
                    .enumerate()
                    .map(|(index, extension)| {
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
                        let installing = browser.installing.contains(&extension.key());
                        self.open_vsx_row(k, index, extension, installed_version, installing, cx)
                    })
                    .collect::<Vec<_>>();
                let list = div()
                    .flex()
                    .flex_col()
                    .when(browser.searching, |el| el.opacity(0.5))
                    .children(rows)
                    .into_any_element();
                kit::appear(k, format!("open-vsx-results-{}", browser.revision), list)
            });

        div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(search_row)
            .children(status)
            .children(results)
            .into_any_element()
    }

    fn open_vsx_row(
        &self,
        k: &Kit,
        index: usize,
        extension: &open_vsx::Extension,
        installed_version: Option<String>,
        installing: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = &k.theme;
        let id = format!("open-vsx-{}", extension.key());
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
        div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .py(px(10.0))
            .when(index > 0, |el| el.border_t_1().border_color(theme.divider))
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
            .into_any_element()
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
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { open_vsx::install(&extension) })
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
                if let Some(id) = ids.into_iter().next() {
                    self.select_vscode_theme(id, cx);
                }
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

/// `1.2M`, `34K`, `512` downloads.
fn compact_count(count: u64) -> String {
    let arrow = "\u{2193}";
    match count {
        0..=999 => format!("{arrow} {count}"),
        1_000..=999_999 => format!("{arrow} {:.0}K", count as f64 / 1_000.0),
        _ => format!("{arrow} {:.1}M", count as f64 / 1_000_000.0),
    }
}
