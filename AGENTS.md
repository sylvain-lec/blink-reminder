# AGENTS.md

Guidance for AI coding agents (and humans) working in this repository. This is
the canonical agent guide; `CLAUDE.md` points here.

## What this project is

**Blink Reminder** is a small cross-platform desktop app (Rust) that shows gentle,
semi-transparent "blink" reminders at random spots on screen. It renders a
single full-screen, transparent, click-through, always-on-top overlay and paints
the message into it; a system-tray menu controls it; a TOML config (and an in-app
Settings window) drives the reminders.

## Layout

```
src/
  main.rs       eframe bootstrap, overlay window flags, the app loop, the
                Settings window (a second egui viewport), and packaging glue.
  config.rs     Config / Appearance / ReminderConfig structs; load-or-create
                + save the TOML config.
  reminder.rs   Scheduler (when each reminder fires) and the fade animation
                state/curve. Has unit tests.
  tray.rs       System-tray icon + menu and its event handling.
build.rs        Embeds assets/icon.ico into the Windows .exe (Windows only).
tools/gen_icon.py   Generates assets/icon.{png,ico} with no third-party deps.
package-macos.sh    Builds dist/Blink Reminder.app (+ optional .dmg / universal).
.github/workflows/build.yml   Native CI builds for macOS / Linux / Windows.
```

## Common commands

```sh
cargo run                 # run locally (debug)
cargo build --release     # optimized binary
cargo test                # scheduler + fade-curve unit tests
cargo clippy              # lint (keep it clean)
cargo fmt                 # format
./package-macos.sh --dmg  # build the macOS .app and a shareable .dmg
python3 tools/gen_icon.py # regenerate the icon assets
```

Always run `cargo fmt` and `cargo clippy` before committing; keep both clean.

## Architecture notes & non-obvious gotchas

- **eframe 0.34 `App` trait**: the required method is `fn ui(&mut self, ui:
  &mut egui::Ui, _)`, not `update`. The provided `ui` is already a margin-less
  central area — paint directly into it. `update` is deprecated.
- **The overlay** is one borderless, `transparent`, `always_on_top`,
  `mouse_passthrough` window sized to the monitor. `clear_color` returns a fully
  transparent color. We never move the window — reminders are painted at random
  positions inside it, and fades are an alpha multiply.
- **Click-through is toggled at runtime.** It's normally on (so blink never
  intercepts clicks). When a reminder marked `click_to_dismiss` is visible, the
  overlay sends `ViewportCommand::MousePassthrough(false)` so a click can dismiss
  it, then turns it back on. `click_to_dismiss` is **per-reminder** (on
  `ReminderConfig`), not global.
- **tray-icon / muda event delivery (important!)**: `muda` routes menu events to
  *either* a registered handler *or* its global channel, never both. We install a
  handler (to wake egui instantly) and therefore must resolve the click → action
  *inside that handler* and push it onto a shared queue that `Tray::poll` drains.
  Reading `MenuEvent::receiver()` would silently get nothing. See `src/tray.rs`.
- **macOS accessory app**: `main.rs` sets the winit activation policy to
  `Accessory` (no Dock icon, tray only) via `NativeOptions.event_loop_builder`.
  `winit` is therefore a **macOS-only** dependency.
- **Settings window** is a second egui *immediate viewport* opened from the tray.
  It edits a `SettingsDraft` (interval/duration as value + unit); Save converts
  back to `Config`, writes it, and applies live via `Scheduler::apply_config`.
- The app stays low-CPU: when idle it only requests a repaint when the next
  reminder is due; tray clicks wake it via the handler.

## Conventions

- Match the surrounding style; doc-comment modules and non-trivial functions.
- Keep the scheduler/fade logic covered by the `#[cfg(test)]` tests in
  `reminder.rs`; add tests when changing timing or fade math.
- Config changes: add `#[serde(default)]` to new fields so existing config files
  keep loading.
- The app must remain **non-disturbing**: don't make the overlay clickable or
  focus-stealing by default.

## Verifying GUI changes

This is a GUI app, so tests don't cover rendering. To sanity-check a UI change
without a display interaction, temporarily set `settings: Some(...)` (or a short
reminder interval) in `BlinkApp::new`, run for a few seconds, confirm no panic in
stderr, then revert. The tray menu and live click-to-dismiss must be verified by
a human (they can't be driven programmatically here).
