//! Quality levels and the load monitor.
//!
//! Auto measures the full level on the first live frames (self-test) and keeps it only if it
//! fits the frame budget; during the call it steps down to Light after 3 s over budget or on
//! battery, and back up after 30 s under budget on mains power. The step back up is only taken
//! when the self-test showed that the full level fits this PC; otherwise Auto would bounce
//! between the levels every 30 s.

use std::time::{Duration, Instant};

/// Share of the frame interval left for the effects; the rest is headroom for capture and
/// encoding. At 30 fps that is 20 ms of the 33 ms interval.
const BUDGET_SHARE: f64 = 0.6;
/// Frames measured for the median; at 30 fps about half a second.
const WINDOW: usize = 15;
/// Self-test: frames skipped while caches warm up, then frames measured.
const SELFTEST_WARMUP: usize = 5;
const SELFTEST_FRAMES: usize = WINDOW;
const STEP_DOWN_AFTER: Duration = Duration::from_secs(3);
const STEP_UP_AFTER: Duration = Duration::from_secs(30);
/// Below 15 fps the machine counts as too busy for the effect.
const MIN_FPS: f64 = 15.0;

/// The quality setting chosen by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Quality {
    /// Measures the full level on the first frames and keeps it only if it fits the frame
    /// budget; steps down to [`Level::Light`] after 3 s over budget or on battery, and back up
    /// after 30 s under budget on mains power (only if the full level was measured to fit).
    #[default]
    Auto,
    /// Always [`Level::Full`].
    Best,
    /// Always [`Level::Light`].
    Light,
}

/// The two quality levels the pipeline runs at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Level {
    /// Square model on every frame, guided-filter edges, light wrap for images, finer blur.
    Full,
    /// Landscape model at 15 fps, simpler edges and a coarser blur: about half the work.
    Light,
}

/// How much of the frame budget the effect uses, for example to show in a preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Load {
    /// Under half the frame budget.
    #[default]
    Low,
    /// Half to all of the frame budget.
    Medium,
    /// Over the frame budget (Auto is stepping down).
    High,
}

/// Picks the [`Level`] from the [`Quality`] setting and the measured time per frame.
#[derive(Debug, Clone)]
pub struct LevelController {
    quality: Quality,
    budget: Duration,
    level: Level,
    /// Auto only: frames seen by the self-test so far, `None` once it is done.
    selftest: Option<usize>,
    /// Auto only: whether the self-test showed that the full level fits.
    full_fits: bool,
    times: [Duration; WINDOW],
    count: usize,
    over_since: Option<Instant>,
    under_since: Option<Instant>,
}

impl LevelController {
    /// A controller for frames arriving at `fps`; the budget per frame is 60 % of the frame
    /// interval. `on_battery` makes Auto start at the light level.
    pub fn new(quality: Quality, fps: u32, on_battery: bool) -> Self {
        let interval = Duration::from_secs(1) / fps.max(1);
        let (level, selftest) = match quality {
            Quality::Best => (Level::Full, None),
            Quality::Light => (Level::Light, None),
            // On battery Auto starts light; the full level is measured once mains returns.
            Quality::Auto if on_battery => (Level::Light, None),
            Quality::Auto => (Level::Full, Some(0)),
        };
        Self {
            quality,
            budget: interval.mul_f64(BUDGET_SHARE),
            level,
            selftest,
            full_fits: quality == Quality::Auto && on_battery,
            times: [Duration::ZERO; WINDOW],
            count: 0,
            over_since: None,
            under_since: None,
        }
    }

    /// The level to run at now.
    pub fn level(&self) -> Level {
        self.level
    }

    /// Time per frame the effect may take.
    pub fn budget(&self) -> Duration {
        self.budget
    }

    /// Median effect time over the last frames.
    pub fn median(&self) -> Duration {
        let n = self.count.min(WINDOW);
        if n == 0 {
            return Duration::ZERO;
        }
        let mut sorted = self.times;
        let sorted = &mut sorted[..n];
        sorted.sort_unstable();
        sorted[n / 2]
    }

    /// The load class of the median time.
    pub fn load(&self) -> Load {
        let median = self.median();
        if median > self.budget {
            Load::High
        } else if median * 2 >= self.budget {
            Load::Medium
        } else {
            Load::Low
        }
    }

    /// Whether the effect can't even deliver 15 fps.
    pub fn too_busy(&self) -> bool {
        self.count >= WINDOW && self.median().as_secs_f64() > 1.0 / MIN_FPS
    }

    /// Records one frame's effect time, measured at `now`; returns the new level when it
    /// changed. `on_battery` is the current power state.
    pub fn record(
        &mut self,
        now: Instant,
        effect_time: Duration,
        on_battery: bool,
    ) -> Option<Level> {
        if let Some(seen) = self.selftest {
            return self.selftest_frame(seen, effect_time);
        }
        self.push(effect_time);
        if self.quality != Quality::Auto {
            return None;
        }
        let over = self.count >= WINDOW && self.median() > self.budget;
        match self.level {
            Level::Full => {
                self.under_since = None;
                let since = if over {
                    *self.over_since.get_or_insert(now)
                } else {
                    self.over_since = None;
                    now
                };
                if on_battery || now.duration_since(since) >= STEP_DOWN_AFTER {
                    return self.switch(Level::Light);
                }
            }
            Level::Light => {
                self.over_since = None;
                if on_battery || over || !self.full_fits {
                    self.under_since = None;
                    return None;
                }
                let since = *self.under_since.get_or_insert(now);
                if now.duration_since(since) >= STEP_UP_AFTER {
                    return self.switch(Level::Full);
                }
            }
        }
        None
    }

    fn selftest_frame(&mut self, seen: usize, effect_time: Duration) -> Option<Level> {
        let seen = seen + 1;
        if seen <= SELFTEST_WARMUP {
            self.selftest = Some(seen);
            return None;
        }
        self.push(effect_time);
        if seen < SELFTEST_WARMUP + SELFTEST_FRAMES {
            self.selftest = Some(seen);
            return None;
        }
        self.selftest = None;
        self.full_fits = self.median() <= self.budget;
        log::info!(
            "background effect self-test: median {} µs, budget {} µs, full level fits: {}",
            self.median().as_micros(),
            self.budget.as_micros(),
            self.full_fits
        );
        if self.full_fits {
            None
        } else {
            self.switch(Level::Light)
        }
    }

    fn push(&mut self, effect_time: Duration) {
        self.times[self.count % WINDOW] = effect_time;
        self.count += 1;
    }

    fn switch(&mut self, level: Level) -> Option<Level> {
        self.level = level;
        // Times measured at the old level say nothing about the new one.
        self.count = 0;
        self.over_since = None;
        self.under_since = None;
        Some(level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);
    const FRAME: Duration = Duration::from_millis(33);

    /// Feeds `frames` frames of `time` each, 33 ms apart; returns the last level change.
    fn feed(
        c: &mut LevelController,
        now: &mut Instant,
        frames: usize,
        time: Duration,
        battery: bool,
    ) -> Option<Level> {
        let mut last = None;
        for _ in 0..frames {
            *now += FRAME;
            if let Some(level) = c.record(*now, time, battery) {
                last = Some(level);
            }
        }
        last
    }

    #[test]
    fn auto_keeps_best_when_the_selftest_fits() {
        let mut c = LevelController::new(Quality::Auto, 30, false);
        let mut now = Instant::now();
        assert_eq!(feed(&mut c, &mut now, 40, 12 * MS, false), None);
        assert_eq!(c.level(), Level::Full);
        assert_eq!(c.load(), Load::Medium);
    }

    #[test]
    fn auto_picks_light_when_the_selftest_is_too_slow() {
        let mut c = LevelController::new(Quality::Auto, 30, false);
        let mut now = Instant::now();
        assert_eq!(
            feed(&mut c, &mut now, 20, 25 * MS, false),
            Some(Level::Light)
        );
        // Light under budget does not step up: the full level was measured too slow.
        assert_eq!(feed(&mut c, &mut now, 31 * 30, 5 * MS, false), None);
        assert_eq!(c.level(), Level::Light);
    }

    #[test]
    fn auto_steps_down_after_three_seconds_over_budget_and_back_up_after_thirty() {
        let mut c = LevelController::new(Quality::Auto, 30, false);
        let mut now = Instant::now();
        feed(&mut c, &mut now, 20, 10 * MS, false);
        // Two seconds over budget: still full.
        assert_eq!(feed(&mut c, &mut now, 2 * 30, 25 * MS, false), None);
        assert_eq!(c.load(), Load::High);
        assert_eq!(
            feed(&mut c, &mut now, 2 * 30, 25 * MS, false),
            Some(Level::Light)
        );
        // 29 s under budget on mains: still light; after 30 s: full again.
        assert_eq!(feed(&mut c, &mut now, 29 * 30, 6 * MS, false), None);
        assert_eq!(
            feed(&mut c, &mut now, 2 * 30, 6 * MS, false),
            Some(Level::Full)
        );
    }

    #[test]
    fn auto_steps_down_on_battery_and_stays_light_there() {
        let mut c = LevelController::new(Quality::Auto, 30, false);
        let mut now = Instant::now();
        feed(&mut c, &mut now, 20, 10 * MS, false);
        assert_eq!(feed(&mut c, &mut now, 1, 10 * MS, true), Some(Level::Light));
        assert_eq!(feed(&mut c, &mut now, 40 * 30, 5 * MS, true), None);
    }

    #[test]
    fn auto_on_battery_starts_light_and_may_step_up_on_mains() {
        let mut c = LevelController::new(Quality::Auto, 30, true);
        assert_eq!(c.level(), Level::Light);
        let mut now = Instant::now();
        assert_eq!(
            feed(&mut c, &mut now, 31 * 30, 5 * MS, false),
            Some(Level::Full)
        );
    }

    #[test]
    fn best_and_light_never_switch() {
        for (quality, level) in [(Quality::Best, Level::Full), (Quality::Light, Level::Light)] {
            let mut c = LevelController::new(quality, 30, false);
            let mut now = Instant::now();
            assert_eq!(feed(&mut c, &mut now, 200, 40 * MS, true), None);
            assert_eq!(feed(&mut c, &mut now, 40 * 30, 2 * MS, false), None);
            assert_eq!(c.level(), level);
        }
    }

    #[test]
    fn reports_too_busy_below_fifteen_fps() {
        let mut c = LevelController::new(Quality::Light, 30, false);
        let mut now = Instant::now();
        feed(&mut c, &mut now, 20, 50 * MS, false);
        assert!(!c.too_busy());
        feed(&mut c, &mut now, 20, 70 * MS, false);
        assert!(c.too_busy());
        assert_eq!(c.load(), Load::High);
    }
}
