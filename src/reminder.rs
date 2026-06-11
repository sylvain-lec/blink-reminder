//! Reminder scheduling and fade state. The scheduler tracks when each reminder
//! is next due and, when one fires, picks a random on-screen position and an
//! animation start time. It owns no rendering — `main.rs` reads `current` to
//! paint the active reminder.

use std::time::{Duration, Instant};

use eframe::egui::{Pos2, Rect, pos2};
use rand::RngExt;

use crate::config::{Appearance, Config, ReminderConfig};

/// A configured reminder plus its next scheduled firing time.
struct Scheduled {
    cfg: ReminderConfig,
    next_fire: Instant,
}

/// A reminder currently being shown (mid fade-in / hold / fade-out).
pub struct Active {
    pub message: String,
    /// Center point of the message on screen.
    pub pos: Pos2,
    pub started: Instant,
    pub duration: f32,
}

impl Active {
    /// Alpha for the current moment following a fade-in → hold → fade-out curve,
    /// scaled by `max` (the configured peak opacity).
    pub fn alpha(&self, now: Instant, fade: f32, max: f32) -> f32 {
        let t = now.duration_since(self.started).as_secs_f32();
        let d = self.duration;
        let fade = fade.max(0.001);
        let a = if t < fade {
            t / fade
        } else if t > d - fade {
            (d - t) / fade
        } else {
            1.0
        };
        a.clamp(0.0, 1.0) * max
    }
}

pub struct Scheduler {
    reminders: Vec<Scheduled>,
    pub appearance: Appearance,
    pub current: Option<Active>,
}

impl Scheduler {
    pub fn new(cfg: Config) -> Self {
        let Config {
            appearance,
            reminders,
        } = cfg;
        let now = Instant::now();
        let reminders = reminders
            .into_iter()
            .map(|c| {
                let next_fire = now + interval_of(&c);
                Scheduled { cfg: c, next_fire }
            })
            .collect();
        Self {
            reminders,
            appearance,
            current: None,
        }
    }

    /// Reschedule every reminder relative to `now` (used when resuming from pause
    /// so a long pause doesn't trigger a burst of overdue reminders at once).
    pub fn reset_timers(&mut self, now: Instant) {
        for s in &mut self.reminders {
            s.next_fire = now + interval_of(&s.cfg);
        }
    }

    /// Advance the state machine. Expires a finished reminder, starts a due one
    /// (picking a random position within `screen`), and returns how long until
    /// the next reminder is due — used to schedule an idle repaint. Returns
    /// `None` while a reminder is active (caller should repaint every frame).
    pub fn update(&mut self, now: Instant, screen: Rect) -> Option<Duration> {
        if let Some(active) = &self.current
            && now.duration_since(active.started).as_secs_f32() >= active.duration
        {
            self.current = None;
        }

        if self.current.is_none() {
            // Among reminders that are due, fire the most overdue one.
            let chosen = self
                .reminders
                .iter()
                .enumerate()
                .filter(|(_, s)| s.next_fire <= now)
                .min_by_key(|(_, s)| s.next_fire)
                .map(|(i, _)| i);

            if let Some(i) = chosen {
                let s = &mut self.reminders[i];
                let message = s.cfg.message.clone();
                let duration = s.cfg.duration_secs.max(0.1);
                let pos = random_pos(&message, self.appearance.font_size, screen);
                s.next_fire = now + interval_of(&s.cfg);
                self.current = Some(Active {
                    message,
                    pos,
                    started: now,
                    duration,
                });
            }
        }

        if self.current.is_some() {
            None
        } else {
            self.reminders
                .iter()
                .map(|s| s.next_fire.saturating_duration_since(now))
                .min()
        }
    }
}

fn interval_of(c: &ReminderConfig) -> Duration {
    Duration::from_secs(c.interval_secs.max(1))
}

/// Pick a random center point such that the (estimated) message box stays fully
/// on screen, inset by a small margin.
fn random_pos(message: &str, font_size: f32, screen: Rect) -> Pos2 {
    let est_w = font_size * 0.6 * message.chars().count().max(1) as f32 + 40.0;
    let est_h = font_size * 1.4 + 28.0;
    let margin = 24.0;

    let min_x = screen.left() + margin + est_w / 2.0;
    let max_x = screen.right() - margin - est_w / 2.0;
    let min_y = screen.top() + margin + est_h / 2.0;
    let max_y = screen.bottom() - margin - est_h / 2.0;

    let mut rng = rand::rng();
    let x = if max_x > min_x {
        rng.random_range(min_x..max_x)
    } else {
        screen.center().x
    };
    let y = if max_y > min_y {
        rng.random_range(min_y..max_y)
    } else {
        screen.center().y
    };
    pos2(x, y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{pos2, vec2};

    fn cfg(interval_secs: u64, duration_secs: f32) -> Config {
        Config {
            appearance: Appearance::default(),
            reminders: vec![ReminderConfig {
                message: "blink".into(),
                interval_secs,
                duration_secs,
            }],
        }
    }

    fn screen() -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(1920.0, 1080.0))
    }

    #[test]
    fn fade_curve_in_hold_out() {
        let a = Active {
            message: "x".into(),
            pos: pos2(0.0, 0.0),
            started: Instant::now(),
            duration: 4.0,
        };
        let max = 0.8;
        let fade = 0.5;
        // start = transparent, midpoint = full max, just-before-end = fading out.
        assert!(a.alpha(a.started, fade, max) < 0.01);
        assert!((a.alpha(a.started + Duration::from_secs_f32(2.0), fade, max) - max).abs() < 1e-4);
        let near_end = a.alpha(a.started + Duration::from_secs_f32(3.75), fade, max);
        assert!(near_end > 0.0 && near_end < max);
    }

    #[test]
    fn reminder_fires_when_due_and_reschedules() {
        let mut s = Scheduler::new(cfg(5, 4.0));
        let start = Instant::now();

        // Not yet due.
        assert!(s.update(start, screen()).is_some());
        assert!(s.current.is_none());

        // Due after the interval: a reminder becomes active, on screen.
        let due = start + Duration::from_secs(6);
        let wake = s.update(due, screen());
        assert!(wake.is_none(), "active reminder => repaint every frame");
        let active = s.current.as_ref().expect("reminder should be active");
        assert!(screen().contains(active.pos));

        // After its duration it expires and the next fire is ~one interval out.
        let after = due + Duration::from_secs_f32(4.1);
        let wake = s.update(after, screen()).expect("idle => timed wake");
        assert!(s.current.is_none());
        assert!(wake <= Duration::from_secs(5));
    }

    #[test]
    fn reset_timers_pushes_everything_into_the_future() {
        let mut s = Scheduler::new(cfg(5, 4.0));
        let now = Instant::now() + Duration::from_secs(100);
        s.reset_timers(now);
        // Nothing should be due right at `now`.
        assert!(s.update(now, screen()).is_some());
        assert!(s.current.is_none());
    }
}
