<div align="center">

<img src="assets/icon.png" alt="Blink Reminder" width="120" height="120" />

# Blink Reminder

**A tiny cross-platform desktop app that gently reminds you to blink** — and
anything else you like (stand up, drink water…).

[![Build](https://github.com/sylvain-lec/blink-reminder/actions/workflows/build.yml/badge.svg)](https://github.com/sylvain-lec/blink-reminder/actions/workflows/build.yml)
&nbsp;[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
&nbsp;![Rust](https://img.shields.io/badge/Rust-1.95%2B-orange?logo=rust)
&nbsp;![Platforms](https://img.shields.io/badge/platforms-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey)

</div>

Built with Rust + [egui]/[eframe]. Every reminder appears as a **semi-transparent
message at a random spot** on a **full-screen, click-through, always-on-top
overlay**, fades in and out, and never steals focus or blocks clicks. It lives in
the **system tray** with Pause/Resume, Quit, and a Settings window.

## Features

- ⏰ **Multiple reminders**, each on its own schedule (seconds / minutes / hours).
- 🎯 Appears at a **random spot**, **fades** in and out, **semi-transparent**.
- 🖱️ **Click-through** by default so it never interrupts you — with an optional
  per-reminder *click to dismiss*.
- ⚙️ In-app **Settings window** (from the tray) to edit messages, intervals,
  durations and appearance.
- 🩺 **System-tray** menu: Settings, Pause/Resume, Quit. No Dock icon on macOS.
- 📦 **TOML config**, auto-created on first run.
- 🖥️ Runs on **macOS, Linux and Windows**.

## Run

```sh
cargo run            # debug
cargo run --release  # smoother
```

A tray icon (a little eye 👁️) appears; on macOS there is no Dock icon. The
default blink reminder fires every 20 seconds.

## Build a macOS app (.app)

To get a double-clickable app instead of running from the terminal:

```sh
./package-macos.sh              # builds dist/Blink Reminder.app
./package-macos.sh --universal  # arm64 + x86_64 (for sharing with Intel Macs)
./package-macos.sh --dmg        # also build a shareable dist/Blink Reminder.dmg
open "dist/Blink Reminder.app"  # launch it
```

The bundle is marked `LSUIElement`, so it runs as a menu-bar (tray) app with no
Dock icon — quit it from the eye icon's menu. It's ad-hoc code-signed for local
use; to keep it in your Applications folder just drag `Blink Reminder.app`
there (the `.dmg` opens with a drag-to-Applications shortcut). To launch at
login: System Settings → General → Login Items → add it.

## Windows & Linux executables

Cross-compiling this GUI app from one OS to another isn't reliable, so native
binaries are built by CI (`.github/workflows/build.yml`) on macOS, Linux and
Windows runners. Push the repo to GitHub and the workflow produces downloadable
artifacts on every push/tag:

- `blink-rust-windows-x86_64.exe` — no console window; double-click to run.
- `blink-rust-linux-x86_64` — needs GTK/X11 at runtime (`libgtk-3`, `libxdo`,
  `libayatana-appindicator3`).
- `blink-rust-macos-arm64`.

To build locally on Windows or Linux instead, just run `cargo build --release`
on that machine (install the Linux dev packages listed in the workflow first).
The Windows `.exe` gets the app icon embedded automatically (via `build.rs`).

## Icon

The eye icon lives in `assets/` (`icon.png` for macOS, `icon.ico` for Windows).
Both are generated with no third-party tools by `python3 tools/gen_icon.py`;
edit that script and rerun it to change the icon, then rebuild.

## Editing reminders

Right-click the tray icon → **Settings…** to open an editor where you can:

- change each reminder's **message**, **interval** (in seconds, minutes or
  hours), and **on-screen duration** (in seconds or minutes),
- **add** or **remove** reminders,
- tweak **font size**, **opacity**, and **fade** time,
- mark individual reminders **clickable** so a click dismisses them (see below).

Click **Save** to apply immediately and write the changes back to the config
file (below); **Cancel** or closing the window discards them. You can also edit
the file directly — see the next section.

## Configuration

On first launch a default config is written to your OS config directory and
printed to the console:

| OS      | Path                                                        |
| ------- | ----------------------------------------------------------- |
| macOS   | `~/Library/Application Support/blink-rust/config.toml`      |
| Linux   | `~/.config/blink-rust/config.toml`                          |
| Windows | `%APPDATA%\blink-rust\config.toml`                          |

```toml
[appearance]
font_size   = 28.0   # text size in points
max_opacity = 0.85   # peak text alpha (0.0–1.0); keep it semi-transparent
fade_secs   = 0.6    # fade-in (== fade-out) duration in seconds

# Add as many [[reminders]] blocks as you like — each has its own schedule.
[[reminders]]
message          = "Time to blink 👁️"
interval_secs    = 20
duration_secs    = 4      # total time on screen, including fade in/out
click_to_dismiss = false  # if true, click this reminder to dismiss it

[[reminders]]
message       = "Stand up and stretch 🧍"
interval_secs = 1800
duration_secs = 5

[[reminders]]
message       = "Sip some water 💧"
interval_secs = 3600
duration_secs = 5
```

Edits made in the **Settings…** window are saved here automatically. If you
edit the file by hand, **relaunch** to apply the changes. An invalid or missing
file falls back to the built-in defaults (a message is printed to stderr).

## Click to dismiss

By default blink is **completely click-through** — clicks always fall through to
whatever is underneath, so a reminder can never interrupt you. Each reminder has
its own **click_to_dismiss** flag (set per row in Settings, or per `[[reminders]]`
block in the config). When it's on, clicking that reminder makes it disappear
early. The trade-off: while such a reminder is on screen the overlay becomes
clickable (a click *anywhere* dismisses it), so for those few seconds clicks land
on the overlay instead of the app behind it. Reminders left click-through (the
default) never intercept clicks, and the idle overlay never does either.

## How it works

- A single borderless, transparent, always-on-top window covers the primary
  monitor. `clear_color` is fully transparent and mouse pass-through is enabled,
  so the overlay is invisible and clicks fall through to whatever is underneath.
- A scheduler tracks each reminder's next firing time; when one is due it picks a
  random on-screen position and animates a fade-in → hold → fade-out. Between
  reminders the app only wakes when the next one is due (low CPU); tray clicks
  wake it instantly.

## Platform notes

- **macOS / Windows**: work out of the box.
- **Linux**: the tray icon (`tray-icon` crate) requires a GTK environment and the
  `libxdo`/`gtk` system libraries. The overlay itself works without the tray; if
  the tray can't be created the app still runs (a message is printed) and you can
  quit it from the terminal.

## Test

```sh
cargo test     # scheduler + fade-curve logic
cargo clippy
```

[egui]: https://github.com/emilk/egui
[eframe]: https://github.com/emilk/egui/tree/master/crates/eframe
