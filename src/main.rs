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

struct BlinkApp {
    scheduler: reminder::Scheduler,
    tray: Option<tray::Tray>,
    paused: bool,
    /// Whether the overlay has been stretched to cover the monitor yet.
    sized: bool,
}

impl BlinkApp {
    fn new(cc: &eframe::CreationContext<'_>, cfg: config::Config) -> Self {
        let tray = tray::Tray::new(cc.egui_ctx.clone());
        if tray.is_none() {
            eprintln!("blink: tray icon unavailable; running without tray controls");
        }
        Self {
            scheduler: reminder::Scheduler::new(cfg),
            tray,
            paused: false,
            sized: false,
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

        // Paused: draw nothing; the next tray click wakes us via the handler.
        if self.paused {
            return;
        }

        let now = Instant::now();
        let wake = self.scheduler.update(now, ui.max_rect());

        if self.scheduler.current.is_some() {
            self.draw_active(ui, now);
            ctx.request_repaint(); // animate the fade
        } else if let Some(dur) = wake {
            ctx.request_repaint_after(dur);
        }
    }
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
