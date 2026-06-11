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

/// Unit used to display/edit a reminder interval in the settings window.
#[derive(Clone, Copy, PartialEq)]
enum TimeUnit {
    Seconds,
    Minutes,
    Hours,
}

impl TimeUnit {
    /// Units offered for a reminder's interval.
    const ALL: [TimeUnit; 3] = [TimeUnit::Seconds, TimeUnit::Minutes, TimeUnit::Hours];
    /// Units offered for how long a reminder stays on screen.
    const DURATION: [TimeUnit; 2] = [TimeUnit::Seconds, TimeUnit::Minutes];

    fn secs(self) -> u64 {
        match self {
            TimeUnit::Seconds => 1,
            TimeUnit::Minutes => 60,
            TimeUnit::Hours => 3600,
        }
    }

    fn label(self) -> &'static str {
        match self {
            TimeUnit::Seconds => "seconds",
            TimeUnit::Minutes => "minutes",
            TimeUnit::Hours => "hours",
        }
    }
}

/// Split a raw interval into the largest whole unit that divides it evenly, so
/// e.g. 1800s shows as "30 minutes" rather than "1800 seconds".
fn split_interval(secs: u64) -> (u64, TimeUnit) {
    if secs >= 3600 && secs.is_multiple_of(3600) {
        (secs / 3600, TimeUnit::Hours)
    } else if secs >= 60 && secs.is_multiple_of(60) {
        (secs / 60, TimeUnit::Minutes)
    } else {
        (secs.max(1), TimeUnit::Seconds)
    }
}

/// Show a duration in whole minutes when it divides evenly, else in seconds.
fn split_duration(secs: f32) -> (f32, TimeUnit) {
    if secs >= 60.0 && (secs % 60.0) == 0.0 {
        (secs / 60.0, TimeUnit::Minutes)
    } else {
        (secs, TimeUnit::Seconds)
    }
}

/// One editable reminder row in the settings window.
struct ReminderDraft {
    message: String,
    amount: u64,
    unit: TimeUnit,
    duration_amount: f32,
    duration_unit: TimeUnit,
}

/// The whole settings window's working state (decoupled from the on-disk config
/// so the interval can be edited as a value + unit).
struct SettingsDraft {
    reminders: Vec<ReminderDraft>,
    appearance: config::Appearance,
}

impl SettingsDraft {
    fn from_config(cfg: &config::Config) -> Self {
        let reminders = cfg
            .reminders
            .iter()
            .map(|r| {
                let (amount, unit) = split_interval(r.interval_secs);
                let (duration_amount, duration_unit) = split_duration(r.duration_secs);
                ReminderDraft {
                    message: r.message.clone(),
                    amount,
                    unit,
                    duration_amount,
                    duration_unit,
                }
            })
            .collect();
        Self {
            reminders,
            appearance: cfg.appearance.clone(),
        }
    }

    fn to_config(&self) -> config::Config {
        let reminders = self
            .reminders
            .iter()
            .map(|r| config::ReminderConfig {
                message: r.message.clone(),
                interval_secs: r.amount.max(1) * r.unit.secs(),
                duration_secs: r.duration_amount.max(0.1) * r.duration_unit.secs() as f32,
            })
            .collect();
        config::Config {
            appearance: self.appearance.clone(),
            reminders,
        }
    }
}

struct BlinkApp {
    /// Source-of-truth config; the scheduler is derived from it.
    config: config::Config,
    scheduler: reminder::Scheduler,
    tray: Option<tray::Tray>,
    paused: bool,
    /// Whether the overlay has been stretched to cover the monitor yet.
    sized: bool,
    /// Current OS click-through state of the overlay; toggled off only while a
    /// click-to-dismiss reminder is on screen.
    passthrough: bool,
    /// Working copy shown in the settings window; `Some` while it's open.
    settings: Option<SettingsDraft>,
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
            passthrough: true,
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

        // When click-to-dismiss is on, show a pointer cursor over the pill so
        // it's obvious you can click it away.
        if app.click_to_dismiss {
            let resp = ui.interact(rect, egui::Id::new("blink-dismiss"), egui::Sense::click());
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        }
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
                            self.settings = Some(SettingsDraft::from_config(&self.config));
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
        let mut showing = false;
        if !self.paused && self.settings.is_none() {
            let now = Instant::now();
            let wake = self.scheduler.update(now, ui.max_rect());

            if self.scheduler.current.is_some() {
                self.draw_active(ui, now);
                showing = true;

                // Click-to-dismiss: a click anywhere clears the reminder (the
                // overlay is interactive in this mode — see passthrough below).
                if self.config.appearance.click_to_dismiss
                    && ui.input(|i| i.pointer.primary_clicked())
                {
                    self.scheduler.current = None;
                    showing = false;
                }
            }

            if self.scheduler.current.is_some() {
                ctx.request_repaint(); // animate the fade
            } else if let Some(dur) = wake {
                ctx.request_repaint_after(dur);
            }
        }

        // The overlay is click-through except while a click-to-dismiss reminder
        // is on screen, so by default blink never intercepts your clicks.
        let want_passthrough = !(self.config.appearance.click_to_dismiss && showing);
        if want_passthrough != self.passthrough {
            ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(want_passthrough));
            self.passthrough = want_passthrough;
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
                    .with_inner_size([840.0, 480.0])
                    .with_min_inner_size([640.0, 340.0])
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
                self.config = draft.to_config();
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
fn settings_ui(ui: &mut egui::Ui, draft: &mut SettingsDraft, outcome: &mut SettingsOutcome) {
    ui.add_space(4.0);
    ui.label("Reminders fire on their own schedule and fade in at a random spot.");
    ui.add_space(6.0);

    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .max_height(300.0)
        .show(ui, |ui| {
            let mut remove = None;
            for (i, r) in draft.reminders.iter_mut().enumerate() {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    // Fill the window width so the message field is roomy.
                    let w = ui.available_width();
                    ui.set_width(w);

                    ui.add(
                        egui::TextEdit::singleline(&mut r.message)
                            .hint_text("Reminder text")
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label("Every");
                        ui.add(egui::DragValue::new(&mut r.amount).range(1..=9999));
                        egui::ComboBox::from_id_salt(("unit", i))
                            .selected_text(r.unit.label())
                            .width(96.0)
                            .show_ui(ui, |ui| {
                                for unit in TimeUnit::ALL {
                                    ui.selectable_value(&mut r.unit, unit, unit.label());
                                }
                            });
                        ui.add_space(16.0);
                        ui.label("Show for");
                        ui.add(
                            egui::DragValue::new(&mut r.duration_amount)
                                .range(0.1..=600.0)
                                .speed(0.1),
                        );
                        egui::ComboBox::from_id_salt(("dur_unit", i))
                            .selected_text(r.duration_unit.label())
                            .width(96.0)
                            .show_ui(ui, |ui| {
                                for unit in TimeUnit::DURATION {
                                    ui.selectable_value(&mut r.duration_unit, unit, unit.label());
                                }
                            });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("🗑 Remove").clicked() {
                                remove = Some(i);
                            }
                        });
                    });
                });
                ui.add_space(8.0);
            }
            if let Some(i) = remove {
                draft.reminders.remove(i);
            }
        });

    ui.add_space(4.0);
    if ui.button("➕ Add reminder").clicked() {
        draft.reminders.push(ReminderDraft {
            message: "New reminder".into(),
            amount: 1,
            unit: TimeUnit::Minutes,
            duration_amount: 4.0,
            duration_unit: TimeUnit::Seconds,
        });
    }

    ui.separator();
    ui.strong("Appearance");
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("Font");
        ui.add(egui::DragValue::new(&mut draft.appearance.font_size).range(8.0..=200.0));
        ui.add_space(12.0);
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

    ui.add_space(6.0);
    ui.checkbox(
        &mut draft.appearance.click_to_dismiss,
        "Click a reminder to dismiss it",
    )
    .on_hover_text(
        "When off, blink is completely click-through and never intercepts clicks.\n\
         When on, the overlay becomes clickable while a reminder is shown so you \
         can click to dismiss it.",
    );

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
