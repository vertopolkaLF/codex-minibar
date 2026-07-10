use std::time::Duration;

use chrono::{DateTime, Local, Utc};
use eframe::egui::{self, Color32, RichText, Stroke, ViewportCommand};

use crate::{
    limits::{LimitWindow, RateLimits},
    popup::TrayPopup,
    settings::Settings,
    tray::TrayManager,
    worker::{WorkerEvent, WorkerHandle},
};

pub struct MinibarApp {
    settings: Settings,
    tray: TrayManager,
    worker: Option<WorkerHandle>,
    limits: RateLimits,
    last_activation: String,
    error: Option<String>,
    visible: bool,
    exiting: bool,
    #[cfg(windows)]
    popup: Option<TrayPopup>,
}

#[derive(Clone, Copy)]
struct WinUiPalette {
    surface: Color32,
    surface_stroke: Color32,
    card_fill: Color32,
    card_stroke: Color32,
    text_primary: Color32,
    text_secondary: Color32,
    text_tertiary: Color32,
    progress_track: Color32,
    accent_fill: Color32,
    success_fill: Color32,
    warning_fill: Color32,
    danger_fill: Color32,
    button_fill: Color32,
    button_fill_hovered: Color32,
    button_fill_pressed: Color32,
}

impl WinUiPalette {
    fn dark() -> Self {
        Self {
            surface: Color32::from_rgb(32, 32, 32),
            surface_stroke: Color32::from_rgb(61, 61, 61),
            card_fill: Color32::from_rgb(44, 44, 44),
            card_stroke: Color32::from_rgb(61, 61, 61),
            text_primary: Color32::from_rgb(243, 243, 243),
            text_secondary: Color32::from_rgb(198, 198, 198),
            text_tertiary: Color32::from_rgb(153, 153, 153),
            progress_track: Color32::from_rgb(56, 56, 56),
            accent_fill: Color32::from_rgb(96, 205, 255),
            success_fill: Color32::from_rgb(108, 203, 95),
            warning_fill: Color32::from_rgb(255, 185, 0),
            danger_fill: Color32::from_rgb(255, 153, 164),
            button_fill: Color32::from_rgb(55, 55, 55),
            button_fill_hovered: Color32::from_rgb(64, 64, 64),
            button_fill_pressed: Color32::from_rgb(49, 49, 49),
        }
    }
}

impl MinibarApp {
    pub fn new(
        creation_context: &eframe::CreationContext<'_>,
        settings: Settings,
        worker: Option<WorkerHandle>,
        error: Option<String>,
    ) -> Self {
        let limits = RateLimits::default();
        let mut tray = TrayManager::new();
        if let Err(tray_error) = tray.sync(&settings.tray_widgets, &limits) {
            return Self {
                settings,
                tray,
                worker,
                limits,
                last_activation: "No activation attempt in this session".into(),
                error: Some(tray_error.to_string()),
                visible: false,
                exiting: false,
                #[cfg(windows)]
                popup: TrayPopup::configure(creation_context),
            };
        }
        Self {
            settings,
            tray,
            worker,
            limits,
            last_activation: "No activation attempt in this session".into(),
            error,
            visible: false,
            exiting: false,
            #[cfg(windows)]
            popup: TrayPopup::configure(creation_context),
        }
    }

    fn drain_worker_events(&mut self) {
        let Some(worker) = &self.worker else {
            return;
        };
        while let Ok(event) = worker.events.try_recv() {
            match event {
                WorkerEvent::LimitsUpdated(limits) => {
                    self.error = self
                        .tray
                        .sync(&self.settings.tray_widgets, &limits)
                        .err()
                        .map(|error| error.to_string());
                    self.limits = limits;
                }
                WorkerEvent::ActivationSucceeded => {
                    self.last_activation =
                        format!("Succeeded at {}", Local::now().format("%H:%M:%S %d.%m.%Y"));
                }
                WorkerEvent::ActivationFailed(error) => {
                    self.last_activation = format!("Failed: {error}");
                }
                WorkerEvent::PollFailed(error) => self.error = Some(error),
                WorkerEvent::Stopped => {}
            }
        }
    }

    #[cfg(windows)]
    fn drain_tray_events(&mut self, ctx: &egui::Context) {
        use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};

        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click {
                id,
                position,
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
                && self.tray.contains(&id)
            {
                let scale = f64::from(ctx.pixels_per_point());
                let x = (position.x / scale - 180.0).max(0.0) as f32;
                let y = (position.y / scale - 440.0).max(0.0) as f32;
                ctx.send_viewport_cmd(ViewportCommand::OuterPosition(egui::pos2(x, y)));
                ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                self.visible = true;
            }
        }
    }

    #[cfg(not(windows))]
    fn drain_tray_events(&mut self, _ctx: &egui::Context) {}

    fn handle_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|input| input.viewport().close_requested()) && !self.exiting {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            self.visible = false;
        }
    }

    #[cfg(windows)]
    fn dismiss_on_outside_click(&mut self, ctx: &egui::Context) {
        if self.visible && self.popup.as_mut().is_some_and(TrayPopup::clicked_outside) {
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            self.visible = false;
        }
    }

    #[cfg(not(windows))]
    fn dismiss_on_outside_click(&mut self, _ctx: &egui::Context) {}
}

impl eframe::App for MinibarApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        Color32::TRANSPARENT.to_normalized_gamma_f32()
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_worker_events();
        self.drain_tray_events(ctx);
        self.handle_close(ctx);
        self.dismiss_on_outside_click(ctx);
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let palette = WinUiPalette::dark();
        apply_winui_visuals(&ctx, palette);
        egui::Frame::new()
            .fill(palette.surface)
            .stroke(Stroke::new(1.0, palette.surface_stroke))
            .corner_radius(12)
            .outer_margin(egui::Margin::same(1))
            .inner_margin(18)
            .show(ui, |ui| {
                // The app surface owns the entire native popup; only the
                // intentional inner padding separates controls from its edge.
                ui.set_min_size(ui.available_size());
                ui.horizontal(|ui| {
                    ui.heading(
                        RichText::new("Codex Minibar")
                            .size(21.0)
                            .color(palette.text_primary),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_sized(
                                [90.0, 30.0],
                                egui::Button::new(RichText::new("Quit").color(palette.text_primary))
                                    .corner_radius(7),
                            )
                            .clicked()
                        {
                            self.exiting = true;
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                        if ui
                            .add_sized(
                                [90.0, 30.0],
                                egui::Button::new(
                                    RichText::new("Refresh").color(palette.text_primary),
                                )
                                .corner_radius(7),
                            )
                            .clicked()
                            && let Some(worker) = &self.worker
                        {
                            worker.refresh();
                        }
                    });
                });
                ui.add_space(14.0);

                limit_card(ui, "5 HOUR WINDOW", &self.limits.primary, palette);
                ui.add_space(10.0);
                limit_card(ui, "7 DAY WINDOW", &self.limits.secondary, palette);
                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("PLAN")
                            .size(10.0)
                            .color(palette.text_tertiary),
                    );
                    ui.label(RichText::new(
                        self.limits
                            .plan_type
                            .as_deref()
                            .unwrap_or("Unavailable")
                            .to_uppercase(),
                    )
                    .color(palette.text_primary));
                    ui.separator();
                    ui.label(
                        RichText::new("CREDITS")
                            .size(10.0)
                            .color(palette.text_tertiary),
                    );
                    ui.label(RichText::new(credits_label(&self.limits)).color(palette.text_primary));
                });
                ui.add_space(10.0);

                egui::Frame::new()
                    .fill(palette.card_fill)
                    .stroke(Stroke::new(1.0, palette.card_stroke))
                    .corner_radius(8)
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("LATEST SAMPLE")
                                .size(10.0)
                                .color(palette.text_tertiary),
                        );
                        ui.label(
                            RichText::new(sample_freshness(self.limits.sampled_at))
                                .color(palette.text_primary),
                        );
                        ui.add_space(7.0);
                        ui.label(
                            RichText::new("LAST ACTIVATION")
                                .size(10.0)
                                .color(palette.text_tertiary),
                        );
                        ui.label(RichText::new(&self.last_activation).color(palette.text_primary));
                    });

                if let Some(error) = &self.error {
                    ui.add_space(10.0);
                    ui.colored_label(palette.danger_fill, error);
                }
            });
    }
}

impl Drop for MinibarApp {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            worker.shutdown();
        }
    }
}

fn apply_winui_visuals(ctx: &egui::Context, palette: WinUiPalette) {
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(palette.text_primary);
    visuals.panel_fill = palette.surface;
    visuals.window_fill = palette.surface;
    visuals.window_stroke = Stroke::new(1.0, palette.surface_stroke);
    visuals.faint_bg_color = palette.card_fill;
    visuals.extreme_bg_color = palette.surface;
    visuals.code_bg_color = palette.card_fill;
    visuals.hyperlink_color = palette.accent_fill;
    visuals.selection.bg_fill = palette.accent_fill;
    visuals.selection.stroke = Stroke::new(1.0, palette.accent_fill);
    visuals.widgets.noninteractive.bg_fill = palette.surface;
    visuals.widgets.noninteractive.weak_bg_fill = palette.surface;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.surface_stroke);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.text_secondary);
    visuals.widgets.inactive.bg_fill = palette.button_fill;
    visuals.widgets.inactive.weak_bg_fill = palette.card_fill;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, palette.card_stroke);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, palette.text_primary);
    visuals.widgets.hovered.bg_fill = palette.button_fill_hovered;
    visuals.widgets.hovered.weak_bg_fill = palette.button_fill_hovered;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, palette.card_stroke);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, palette.text_primary);
    visuals.widgets.active.bg_fill = palette.button_fill_pressed;
    visuals.widgets.active.weak_bg_fill = palette.button_fill_pressed;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, palette.card_stroke);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, palette.text_primary);
    ctx.set_visuals(visuals);
}

fn limit_card(ui: &mut egui::Ui, title: &str, window: &LimitWindow, palette: WinUiPalette) {
    let remaining = window.remaining_percent();
    let color = remaining_color(remaining, palette);
    egui::Frame::new()
        .fill(palette.card_fill)
        .stroke(Stroke::new(1.0, palette.card_stroke))
        .corner_radius(8)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(title)
                        .size(11.0)
                        .color(palette.text_secondary),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(
                            remaining
                                .map(|value| format!("{value}% left"))
                                .unwrap_or_else(|| "Unavailable".into()),
                        )
                        .strong()
                        .color(color),
                    );
                });
            });
            ui.add_space(8.0);
            let desired_size = egui::vec2(ui.available_width(), 12.0);
            let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
            let radius = 6.0;
            ui.painter().rect_filled(rect, radius, palette.progress_track);

            if let Some(value) = remaining {
                let progress = f32::from(value) / 100.0;
                if progress > 0.0 {
                    let fill_rect = egui::Rect::from_min_max(
                        rect.min,
                        egui::pos2(rect.min.x + rect.width() * progress, rect.max.y),
                    );
                    ui.painter().rect_filled(fill_rect, radius, color);
                }
            }

            ui.add_space(7.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Used").color(palette.text_tertiary));
                ui.label(RichText::new(
                    window
                        .used_percent
                        .map(|value| format!("{value}%"))
                        .unwrap_or_else(|| "?".into()),
                )
                .color(palette.text_primary));
                ui.separator();
                ui.label(RichText::new("Resets").color(palette.text_tertiary));
                ui.label(RichText::new(format_reset(window.resets_at)).color(palette.text_primary));
                if let Some(minutes) = window.duration_minutes {
                    ui.separator();
                    ui.label(RichText::new(format_duration(minutes)).color(palette.text_secondary));
                }
            });
        });
}

fn credits_label(limits: &RateLimits) -> String {
    if limits.credits.unlimited {
        "Unlimited".into()
    } else if limits.credits.has_credits {
        limits
            .credits
            .balance
            .clone()
            .unwrap_or_else(|| "Available".into())
    } else {
        "None".into()
    }
}

fn format_duration(minutes: u32) -> String {
    if minutes.is_multiple_of(1_440) {
        format!("{}d window", minutes / 1_440)
    } else if minutes.is_multiple_of(60) {
        format!("{}h window", minutes / 60)
    } else {
        format!("{minutes}m window")
    }
}

fn remaining_color(remaining: Option<u8>, palette: WinUiPalette) -> Color32 {
    match remaining {
        Some(0..=15) => palette.danger_fill,
        Some(16..=50) => palette.warning_fill,
        Some(_) => palette.success_fill,
        None => palette.text_tertiary,
    }
}

fn format_reset(reset: Option<DateTime<Utc>>) -> String {
    reset
        .map(|value| {
            value
                .with_timezone(&Local)
                .format("%H:%M, %d %b")
                .to_string()
        })
        .unwrap_or_else(|| "Unavailable".into())
}

fn sample_freshness(sampled_at: DateTime<Utc>) -> String {
    if sampled_at.timestamp() == 0 {
        return "Waiting for Codex...".into();
    }
    let seconds = (Utc::now() - sampled_at).num_seconds().max(0);
    match seconds {
        0..=4 => "Just now".into(),
        5..=59 => format!("{seconds} seconds ago"),
        _ => format!("{} minutes ago", seconds / 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_sample_has_clear_copy() {
        assert_eq!(
            sample_freshness(DateTime::default()),
            "Waiting for Codex..."
        );
        assert_eq!(format_reset(None), "Unavailable");
    }

    #[test]
    fn colors_follow_remaining_thresholds() {
        let palette = WinUiPalette::dark();
        assert_eq!(
            remaining_color(Some(10), palette),
            Color32::from_rgb(255, 153, 164)
        );
        assert_eq!(
            remaining_color(Some(30), palette),
            Color32::from_rgb(255, 185, 0)
        );
        assert_eq!(
            remaining_color(Some(80), palette),
            Color32::from_rgb(108, 203, 95)
        );
    }
}
