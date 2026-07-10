use std::time::Duration;

use chrono::{DateTime, Local, Utc};
use eframe::egui::{self, Color32, RichText, ViewportCommand};

use crate::{
    limits::{LimitWindow, RateLimits},
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
}

impl MinibarApp {
    pub fn new(settings: Settings, worker: Option<WorkerHandle>, error: Option<String>) -> Self {
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
                ctx.send_viewport_cmd(ViewportCommand::Focus);
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
}

impl eframe::App for MinibarApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_worker_events();
        self.drain_tray_events(ctx);
        self.handle_close(ctx);
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        egui::Frame::new()
            .fill(Color32::from_rgb(20, 22, 24))
            .inner_margin(18)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(
                        RichText::new("Codex Minibar")
                            .size(21.0)
                            .color(Color32::WHITE),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Quit").clicked() {
                            self.exiting = true;
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                        if ui.small_button("Refresh").clicked()
                            && let Some(worker) = &self.worker
                        {
                            worker.refresh();
                        }
                    });
                });
                ui.add_space(14.0);

                limit_card(ui, "5 HOUR WINDOW", &self.limits.primary);
                ui.add_space(10.0);
                limit_card(ui, "7 DAY WINDOW", &self.limits.secondary);
                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("PLAN")
                            .size(10.0)
                            .color(Color32::from_gray(145)),
                    );
                    ui.label(
                        self.limits
                            .plan_type
                            .as_deref()
                            .unwrap_or("Unavailable")
                            .to_uppercase(),
                    );
                    ui.separator();
                    ui.label(
                        RichText::new("CREDITS")
                            .size(10.0)
                            .color(Color32::from_gray(145)),
                    );
                    ui.label(credits_label(&self.limits));
                });
                ui.add_space(10.0);

                egui::Frame::new()
                    .fill(Color32::from_rgb(29, 32, 35))
                    .corner_radius(8)
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("LATEST SAMPLE")
                                .size(10.0)
                                .color(Color32::from_rgb(140, 147, 153)),
                        );
                        ui.label(sample_freshness(self.limits.sampled_at));
                        ui.add_space(7.0);
                        ui.label(
                            RichText::new("LAST ACTIVATION")
                                .size(10.0)
                                .color(Color32::from_rgb(140, 147, 153)),
                        );
                        ui.label(&self.last_activation);
                    });

                if let Some(error) = &self.error {
                    ui.add_space(10.0);
                    ui.colored_label(Color32::from_rgb(230, 100, 96), error);
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

fn limit_card(ui: &mut egui::Ui, title: &str, window: &LimitWindow) {
    let remaining = window.remaining_percent();
    let color = remaining_color(remaining);
    egui::Frame::new()
        .fill(Color32::from_rgb(29, 32, 35))
        .corner_radius(8)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(title)
                        .size(11.0)
                        .color(Color32::from_rgb(156, 163, 169)),
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
            ui.add(
                egui::ProgressBar::new(f32::from(remaining.unwrap_or_default()) / 100.0)
                    .fill(color)
                    .animate(false),
            );
            ui.add_space(7.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Used").color(Color32::from_gray(145)));
                ui.label(
                    window
                        .used_percent
                        .map(|value| format!("{value}%"))
                        .unwrap_or_else(|| "?".into()),
                );
                ui.separator();
                ui.label(RichText::new("Resets").color(Color32::from_gray(145)));
                ui.label(format_reset(window.resets_at));
                if let Some(minutes) = window.duration_minutes {
                    ui.separator();
                    ui.label(format_duration(minutes));
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

fn remaining_color(remaining: Option<u8>) -> Color32 {
    match remaining {
        Some(0..=15) => Color32::from_rgb(230, 74, 72),
        Some(16..=50) => Color32::from_rgb(245, 158, 11),
        Some(_) => Color32::from_rgb(49, 196, 141),
        None => Color32::from_rgb(140, 147, 153),
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
        assert_eq!(remaining_color(Some(10)), Color32::from_rgb(230, 74, 72));
        assert_eq!(remaining_color(Some(30)), Color32::from_rgb(245, 158, 11));
        assert_eq!(remaining_color(Some(80)), Color32::from_rgb(49, 196, 141));
    }
}
