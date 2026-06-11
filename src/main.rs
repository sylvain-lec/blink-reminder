//! Blink Reminder — a cross-platform desktop nudge to blink (and stretch, drink
//! water, …). It draws gentle, semi-transparent messages at random spots on a
//! full-screen, click-through, always-on-top overlay so it never interrupts you.

// Don't spawn a console window alongside the GUI on Windows release builds.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod config;
mod reminder;
mod tray;

use std::time::Instant;

use eframe::egui;

fn main() -> eframe::Result {
    let cfg = config::load();

    let viewport = egui::ViewportBuilder::default()
        .with_title("Blink Reminder")
        .with_transparent(true)
        .with_decorations(false)
        .with_always_on_top()
        .with_taskbar(false)
        .with_active(false)
        .with_mouse_passthrough(true)
        .with_resizable(false)
        // Placeholder size; resized to cover the monitor on the first frame.
        .with_inner_size([800.0, 600.0]);

    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    // On macOS, run as an "accessory" so there's no Dock icon — tray only.
    #[cfg(target_os = "macos")]
    {
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
            builder.with_activation_policy(ActivationPolicy::Accessory);
        }));
    }

    eframe::run_native(
        "Blink Reminder",
        options,
        Box::new(|cc| Ok(Box::new(BlinkApp::new(cc, cfg)))),
    )
}

/// Outcome of a frame of the settings window.
enum SettingsOutcome {
    None,
    Save,
    Cancel,
}

struct BlinkApp {
    /// Source-of-truth config; the scheduler is derived from it.
    config: config::Config,
    scheduler: reminder::Scheduler,
    tray: Option<tray::Tray>,
    paused: bool,
    /// Whether the overlay has been stretched to cover the monitor yet.
    sized: bool,
    /// Working copy shown in the settings window; `Some` while it's open.
    settings: Option<config::Config>,
}

impl BlinkApp {
    fn new(cc: &eframe::CreationContext<'_>, cfg: config::Config) -> Self {
        let tray = tray::Tray::new(cc.egui_ctx.clone());
        if tray.is_none() {
            eprintln!("blink: tray icon unavailable; running without tray controls");
        }
        Self {
            scheduler: reminder::Scheduler::new(cfg.clone()),
            config: cfg,
            tray,
            paused: false,
            sized: false,
            settings: None,
        }
    }

    /// Paint the active reminder (a faint rounded "pill" plus text) at its
    /// current fade alpha into the full-screen overlay `ui`.
    fn draw_active(&self, ui: &egui::Ui, now: Instant) {
        let Some(active) = &self.scheduler.current else {
            return;
        };
        let app = &self.scheduler.appearance;
        let alpha = active.alpha(now, app.fade_secs, app.max_opacity);

        let painter = ui.painter();
        let font = egui::FontId::proportional(app.font_size);
        let text_color =
            egui::Color32::from_rgba_unmultiplied(245, 245, 255, (alpha * 255.0) as u8);
        let galley = painter.layout_no_wrap(active.message.clone(), font, text_color);

        let pad = egui::vec2(20.0, 12.0);
        let rect = clamp_rect(
            egui::Rect::from_center_size(active.pos, galley.size() + pad * 2.0),
            ui.max_rect(),
        );

        let bg = egui::Color32::from_rgba_unmultiplied(0, 0, 0, (alpha * 0.45 * 255.0) as u8);
        painter.rect_filled(rect, egui::CornerRadius::same(12), bg);
        painter.galley(rect.center() - galley.size() / 2.0, galley, text_color);
    }
}

impl eframe::App for BlinkApp {
    // Fully transparent framebuffer so the desktop shows through the overlay.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Stretch the borderless window to cover the primary monitor, once we
        // know its size, and (re)assert click-through.
        if !self.sized
            && let Some(monitor) = ctx.input(|i| i.viewport().monitor_size)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(0.0, 0.0)));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(monitor));
            ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(true));
            self.sized = true;
        }

        // Handle tray menu clicks.
        if let Some(tray) = &self.tray {
            while let Some(action) = tray.poll() {
                match action {
                    tray::TrayAction::OpenSettings => {
                        // Reopening keeps any in-progress edits.
                        if self.settings.is_none() {
                            self.settings = Some(self.config.clone());
                        }
                    }
                    tray::TrayAction::TogglePause => {
                        self.paused = !self.paused;
                        tray.set_paused(self.paused);
                        if self.paused {
                            self.scheduler.current = None;
                        } else {
                            self.scheduler.reset_timers(Instant::now());
                        }
                    }
                    tray::TrayAction::Quit => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        }

        // Run reminders only in normal overlay mode. While paused or editing
        // settings the overlay stays empty (the next tray/menu event wakes us).
        if !self.paused && self.settings.is_none() {
            let now = Instant::now();
            let wake = self.scheduler.update(now, ui.max_rect());

            if self.scheduler.current.is_some() {
                self.draw_active(ui, now);
                ctx.request_repaint(); // animate the fade
            } else if let Some(dur) = wake {
                ctx.request_repaint_after(dur);
            }
        }

        self.show_settings(ui);
    }
}

impl BlinkApp {
    /// Render the settings window (a second OS window) while `self.settings` is
    /// `Some`, applying or discarding the draft when the user clicks Save/Cancel
    /// or closes the window.
    fn show_settings(&mut self, ui: &egui::Ui) {
        if self.settings.is_none() {
            return;
        }
        let ctx = ui.ctx().clone();
        let mut outcome = SettingsOutcome::None;

        {
            let draft = self.settings.as_mut().expect("settings is Some");
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("blink-settings"),
                egui::ViewportBuilder::default()
                    .with_title("Blink Reminder — Settings")
                    .with_inner_size([580.0, 400.0])
                    .with_min_inner_size([420.0, 260.0])
                    // The overlay is always-on-top; make sure this normal window
                    // comes forward and can take keyboard focus for text editing.
                    .with_active(true),
                |vctx, _class| {
                    if vctx.input(|i| i.viewport().close_requested()) {
                        outcome = SettingsOutcome::Cancel;
                    }
                    // `CentralPanel::show(ctx, …)` is soft-deprecated but is
                    // still the documented way to fill a viewport's root area
                    // (there's no `show_inside` equivalent without a parent Ui).
                    #[allow(deprecated)]
                    egui::CentralPanel::default().show(vctx, |ui| {
                        settings_ui(ui, draft, &mut outcome);
                    });
                },
            );
        }

        match outcome {
            SettingsOutcome::Save => {
                let draft = self.settings.take().expect("settings is Some");
                self.config = draft;
                if let Err(e) = config::save(&self.config) {
                    eprintln!("blink: could not save config: {e}");
                }
                self.scheduler.apply_config(self.config.clone());
            }
            SettingsOutcome::Cancel => self.settings = None,
            SettingsOutcome::None => {}
        }
    }
}

/// The contents of the settings window: a row per reminder plus appearance
/// controls and Save/Cancel.
fn settings_ui(ui: &mut egui::Ui, draft: &mut config::Config, outcome: &mut SettingsOutcome) {
    ui.add_space(4.0);
    ui.label("Reminders fire on their own schedule and fade in at a random spot.");
    ui.separator();

    egui::ScrollArea::vertical()
        .max_height(220.0)
        .show(ui, |ui| {
            let mut remove = None;
            egui::Grid::new("reminders_grid")
                .num_columns(4)
                .spacing([10.0, 8.0])
                .show(ui, |ui| {
                    ui.strong("Message");
                    ui.strong("Every");
                    ui.strong("Show");
                    ui.label("");
                    ui.end_row();

                    for (i, r) in draft.reminders.iter_mut().enumerate() {
                        ui.add(egui::TextEdit::singleline(&mut r.message).desired_width(240.0));
                        ui.add(
                            egui::DragValue::new(&mut r.interval_secs)
                                .range(1..=86_400)
                                .suffix(" s"),
                        );
                        ui.add(
                            egui::DragValue::new(&mut r.duration_secs)
                                .range(0.5..=60.0)
                                .speed(0.1)
                                .suffix(" s"),
                        );
                        if ui.button("🗑").on_hover_text("Remove").clicked() {
                            remove = Some(i);
                        }
                        ui.end_row();
                    }
                });
            if let Some(i) = remove {
                draft.reminders.remove(i);
            }
        });

    if ui.button("➕ Add reminder").clicked() {
        draft.reminders.push(config::ReminderConfig {
            message: "New reminder".into(),
            interval_secs: 60,
            duration_secs: 4.0,
        });
    }

    ui.separator();
    ui.strong("Appearance");
    ui.horizontal(|ui| {
        ui.label("Font");
        ui.add(egui::DragValue::new(&mut draft.appearance.font_size).range(8.0..=200.0));
        ui.add_space(8.0);
        ui.label("Opacity");
        ui.add(
            egui::DragValue::new(&mut draft.appearance.max_opacity)
                .range(0.05..=1.0)
                .speed(0.01),
        );
        ui.add_space(8.0);
        ui.label("Fade");
        ui.add(
            egui::DragValue::new(&mut draft.appearance.fade_secs)
                .range(0.0..=5.0)
                .speed(0.05)
                .suffix(" s"),
        );
    });

    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("Cancel").clicked() {
            *outcome = SettingsOutcome::Cancel;
        }
        if ui.button("Save").clicked() {
            *outcome = SettingsOutcome::Save;
        }
    });
}

/// Slide `rect` so it fits inside `bounds` (without resizing it).
fn clamp_rect(rect: egui::Rect, bounds: egui::Rect) -> egui::Rect {
    let mut r = rect;
    if r.left() < bounds.left() {
        r = r.translate(egui::vec2(bounds.left() - r.left(), 0.0));
    }
    if r.right() > bounds.right() {
        r = r.translate(egui::vec2(bounds.right() - r.right(), 0.0));
    }
    if r.top() < bounds.top() {
        r = r.translate(egui::vec2(0.0, bounds.top() - r.top()));
    }
    if r.bottom() > bounds.bottom() {
        r = r.translate(egui::vec2(0.0, bounds.bottom() - r.bottom()));
    }
    r
}
