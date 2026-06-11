//! System tray icon with a Pause/Resume and Quit menu.
//!
//! The icon is created on the main thread (from `BlinkApp::new`), which is
//! required on macOS. A menu-event handler wakes the egui loop the instant the
//! user clicks an item, so `poll()` (called from `update`) sees it immediately
//! without busy-polling.

use eframe::egui;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

/// What the user asked for via the tray menu.
pub enum TrayAction {
    TogglePause,
    Quit,
}

pub struct Tray {
    // Kept alive for the lifetime of the app; dropping it removes the icon.
    _tray: TrayIcon,
    pause_item: MenuItem,
    pause_id: MenuId,
    quit_id: MenuId,
}

impl Tray {
    /// Build the tray icon and menu. Returns `None` if the platform refuses to
    /// create it (the app then runs without tray controls). `egui_ctx` is used
    /// to wake the render loop on menu clicks.
    pub fn new(egui_ctx: egui::Context) -> Option<Self> {
        let menu = Menu::new();
        let pause_item = MenuItem::new("Pause", true, None);
        let quit_item = MenuItem::new("Quit", true, None);
        menu.append(&pause_item).ok()?;
        menu.append(&quit_item).ok()?;

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Blink Reminder")
            .with_icon(make_icon())
            .build()
            .ok()?;

        // Wake egui immediately whenever a menu item is clicked.
        MenuEvent::set_event_handler(Some(move |_event| egui_ctx.request_repaint()));

        Some(Self {
            _tray: tray,
            pause_id: pause_item.id().clone(),
            quit_id: quit_item.id().clone(),
            pause_item,
        })
    }

    /// Drain any pending menu events into a `TrayAction`.
    pub fn poll(&self) -> Option<TrayAction> {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.pause_id {
                return Some(TrayAction::TogglePause);
            }
            if event.id == self.quit_id {
                return Some(TrayAction::Quit);
            }
        }
        None
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
