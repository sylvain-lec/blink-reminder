//! Configuration: load (or create) a TOML file describing the reminders and
//! their appearance. The file lives in the user's config dir under `blink-rust/`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Top-level configuration. `#[serde(default)]` means any missing field falls
/// back to the corresponding value from `Config::default()`, so partial or
/// older config files keep working.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub appearance: Appearance,
    pub reminders: Vec<ReminderConfig>,
}

/// Global look-and-feel shared by every reminder.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// Text size in points.
    pub font_size: f32,
    /// Peak alpha of the text, 0.0..=1.0 (keeps it semi-transparent).
    pub max_opacity: f32,
    /// Duration of the fade-in (and, symmetrically, the fade-out) in seconds.
    pub fade_secs: f32,
    /// If true, a click dismisses the visible reminder (and the overlay becomes
    /// clickable while one is shown). If false, the overlay is always
    /// click-through so it never disturbs you. Defaults to false.
    pub click_to_dismiss: bool,
}

/// A single reminder: what to say and how often to say it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReminderConfig {
    pub message: String,
    /// How often this reminder fires, in seconds.
    pub interval_secs: u64,
    /// How long the reminder stays on screen (including fade in/out), in seconds.
    pub duration_secs: f32,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            font_size: 28.0,
            max_opacity: 0.85,
            fade_secs: 0.6,
            click_to_dismiss: false,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            appearance: Appearance::default(),
            reminders: vec![
                ReminderConfig {
                    message: "Time to blink 👁️".into(),
                    interval_secs: 20,
                    duration_secs: 4.0,
                },
                ReminderConfig {
                    message: "Stand up and stretch 🧍".into(),
                    interval_secs: 1800,
                    duration_secs: 5.0,
                },
                ReminderConfig {
                    message: "Sip some water 💧".into(),
                    interval_secs: 3600,
                    duration_secs: 5.0,
                },
            ],
        }
    }
}

/// `<config dir>/blink-rust/config.toml`, e.g.
/// `~/Library/Application Support/blink-rust/config.toml` on macOS.
pub fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("blink-rust").join("config.toml"))
}

/// Write `cfg` back to the config file (used by the in-app settings window).
pub fn save(cfg: &Config) -> std::io::Result<()> {
    let path = config_path()
        .ok_or_else(|| std::io::Error::other("could not determine config directory"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let s = toml::to_string_pretty(cfg).map_err(std::io::Error::other)?;
    std::fs::write(&path, s)
}

/// Load the config, creating a commented default file on first run. Any error
/// (missing dir, unreadable/invalid file) is reported to stderr and the app
/// falls back to built-in defaults so it always starts.
pub fn load() -> Config {
    let Some(path) = config_path() else {
        eprintln!("blink: could not determine config directory; using defaults");
        return Config::default();
    };

    if !path.exists() {
        let cfg = Config::default();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match toml::to_string_pretty(&cfg) {
            Ok(s) => match std::fs::write(&path, s) {
                Ok(()) => eprintln!("blink: wrote default config to {}", path.display()),
                Err(e) => eprintln!("blink: could not write config {}: {e}", path.display()),
            },
            Err(e) => eprintln!("blink: could not serialize default config: {e}"),
        }
        return cfg;
    }

    match std::fs::read_to_string(&path) {
        Ok(s) => match toml::from_str::<Config>(&s) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!(
                    "blink: failed to parse {}: {e}; using defaults",
                    path.display()
                );
                Config::default()
            }
        },
        Err(e) => {
            eprintln!(
                "blink: failed to read {}: {e}; using defaults",
                path.display()
            );
            Config::default()
        }
    }
}
