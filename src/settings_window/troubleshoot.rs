//! "Run Troubleshoot with AI" tool picker.

use gpui::{AnyElement, Context, SharedString};

use super::kit::{self, Button, Kit};
use super::window::SettingsWindow;

impl SettingsWindow {
    pub(super) fn troubleshoot_overlay(
        &mut self,
        k: &mut Kit,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (picker, phase) = self.overlays.troubleshoot.track(k, self.troubleshoot.clone())?;
        let labels = picker
            .tools
            .iter()
            .map(|tool| SharedString::from(tool.tool.label()))
            .collect::<Vec<_>>();
        let selected = usize::try_from(picker.selected_index).ok();
        let tool = selected.and_then(|index| picker.tools.get(index)).cloned();
        let dismiss = Self::h(cx, |this, (), _, cx| {
            this.troubleshoot = None;
            cx.notify();
        });
        let run = Self::h(cx, move |this, (), _, cx| {
            if let Some(tool) = &tool
                && let Err(error) = crate::troubleshoot::launch_selected(tool)
            {
                eprintln!("failed to start troubleshooting terminal: {error:#}");
                crate::notifications::show("Troubleshooting could not start", &error.to_string());
            }
            this.troubleshoot = None;
            cx.notify();
        });
        Some(kit::dialog(
            k,
            "troubleshoot",
            phase,
            440.0,
            vec![
                kit::dialog_title(k, "Run Troubleshoot with AI"),
                kit::caption(
                    k,
                    "Choose which installed AI tool should investigate the problem.",
                ),
                kit::field(
                    k,
                    "AI tool",
                    kit::dropdown(
                        k,
                        "troubleshoot-tool",
                        labels,
                        selected,
                        false,
                        0.0,
                        Self::h(cx, |this, index: usize, _, cx| {
                            if let Some(picker) = this.troubleshoot.as_mut() {
                                picker.selected_index = index as i32;
                            }
                            cx.notify();
                        }),
                    ),
                ),
            ],
            vec![
                Button::new("troubleshoot-cancel", "Cancel")
                    .full_width()
                    .on_click(dismiss.clone())
                    .render(k),
                Button::new("troubleshoot-run", "Open terminal")
                    .accent()
                    .full_width()
                    .disabled(selected.is_none())
                    .on_click(run)
                    .render(k),
            ],
            Some(dismiss),
        ))
    }
}
