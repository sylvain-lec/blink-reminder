//! System tray icon with a Settings, Pause/Resume and Quit menu.
//!
//! The icon is created on the main thread (from `BlinkApp::new`), which is
//! required on macOS. We install a `muda` menu-event handler that maps the
//! clicked item to a [`TrayAction`], pushes it onto a shared queue, and wakes
//! the egui loop — so `poll()` (called from `ui`) sees it immediately without
//! busy-polling.
//!
//! Note: `muda` routes events *either* to a handler *or* to its global channel,
//! never both. Once `set_event_handler` is installed, `MenuEvent::receiver()`
//! goes silent, so we must turn events into actions inside the handler itself.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use eframe::egui;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

/// What the user asked for via the tray menu.
pub enum TrayAction {
    OpenSettings,
    TogglePause,
    Quit,
}

pub struct Tray {
    // Kept alive for the lifetime of the app; dropping it removes the icon.
    _tray: TrayIcon,
    pause_item: MenuItem,
    /// Actions produced by the menu-event handler, drained by `poll`.
    actions: Arc<Mutex<VecDeque<TrayAction>>>,
}

impl Tray {
    /// Build the tray icon and menu. Returns `None` if the platform refuses to
    /// create it (the app then runs without tray controls). `egui_ctx` is used
    /// to wake the render loop on menu clicks.
    pub fn new(egui_ctx: egui::Context) -> Option<Self> {
        let menu = Menu::new();
        let settings_item = MenuItem::new("Settings…", true, None);
        let pause_item = MenuItem::new("Pause", true, None);
        let quit_item = MenuItem::new("Quit", true, None);
        menu.append(&settings_item).ok()?;
        menu.append(&PredefinedMenuItem::separator()).ok()?;
        menu.append(&pause_item).ok()?;
        menu.append(&quit_item).ok()?;

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Blink Reminder")
            .with_icon(make_icon())
            .build()
            .ok()?;

        let actions: Arc<Mutex<VecDeque<TrayAction>>> = Arc::new(Mutex::new(VecDeque::new()));

        // Map each clicked item to an action, queue it, and wake egui. We match
        // by id because the handler can't borrow `self`.
        let settings_id = settings_item.id().clone();
        let pause_id = pause_item.id().clone();
        let quit_id = quit_item.id().clone();
        let queue = actions.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let action = if event.id == settings_id {
                TrayAction::OpenSettings
            } else if event.id == pause_id {
                TrayAction::TogglePause
            } else if event.id == quit_id {
                TrayAction::Quit
            } else {
                return;
            };
            if let Ok(mut q) = queue.lock() {
                q.push_back(action);
            }
            egui_ctx.request_repaint();
        }));

        Some(Self {
            _tray: tray,
            pause_item,
            actions,
        })
    }

    /// Pop the next queued menu action, if any.
    pub fn poll(&self) -> Option<TrayAction> {
        self.actions.lock().ok()?.pop_front()
    }

    /// Reflect the paused state in the menu label.
    pub fn set_paused(&self, paused: bool) {
        self.pause_item
            .set_text(if paused { "Resume" } else { "Pause" });
    }
}

/// Generate a small eye-shaped RGBA icon so the project ships no asset files.
fn make_icon() -> Icon {
    const SIZE: u32 = 32;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    let c = SIZE as f32 / 2.0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 + 0.5 - c;
            let dy = y as f32 + 0.5 - c;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = ((y * SIZE + x) * 4) as usize;
            if dist <= c - 1.0 {
                let (r, g, b) = if dist <= 5.0 {
                    (40, 80, 160) // pupil
                } else {
                    (230, 240, 255) // sclera
                };
                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = 255;
            }
        }
    }
    Icon::from_rgba(rgba, SIZE, SIZE).expect("32x32 RGBA buffer is a valid icon")
}
