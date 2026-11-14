//! Interruptible UI transitions driven by the reader's explicit frame callback.
use std::time::{Duration, Instant};
#[derive(Debug)]
pub struct Transition {
    from: f32,
    target: f32,
    started: Instant,
    duration: Duration,
}
impl Default for Transition {
    fn default() -> Self {
        Self {
            from: 0.,
            target: 0.,
            started: Instant::now(),
            duration: Duration::from_millis(180),
        }
    }
}
impl Transition {
    pub fn settled(value: f32) -> Self {
        Self {
            from: value,
            target: value,
            ..Self::default()
        }
    }

    pub fn value(&self, now: Instant) -> f32 {
        let progress =
            (now.duration_since(self.started).as_secs_f32() / self.duration.as_secs_f32()).min(1.);
        let eased = 1. - (1. - progress).powi(3);
        self.from + (self.target - self.from) * eased
    }
    pub fn update(&mut self, target: f32, now: Instant, enabled: bool) -> f32 {
        if target != self.target {
            self.from = self.value(now);
            self.target = target;
            self.started = now;
        }
        if !enabled {
            self.from = target;
        }
        self.value(now)
    }
    pub fn active(&self, now: Instant) -> bool {
        self.from != self.target && now.duration_since(self.started) < self.duration
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reversals_are_continuous_and_disabling_settles_immediately() {
        let mut transition = Transition::default();
        let now = Instant::now();
        assert_eq!(transition.update(380., now, true), 0.);
        let midway = now + Duration::from_millis(70);
        let before = transition.value(midway);
        assert!(before > 0. && before < 380.);
        assert_eq!(transition.update(0., midway, true), before);
        assert!(transition.value(midway + Duration::from_millis(20)) < before);
        assert_eq!(transition.update(380., midway, false), 380.);
        assert!(!transition.active(midway));
    }
    #[test]
    fn zoom_starts_at_the_displayed_size_and_retargets_without_a_jump() {
        let mut zoom = Transition::settled(800.);
        let now = Instant::now();
        assert_eq!(zoom.update(1200., now, true), 800.);
        let midway = now + Duration::from_millis(60);
        let before = zoom.value(midway);
        assert!(before > 800. && before < 1200.);
        assert_eq!(zoom.update(600., midway, true), before);
        assert_eq!(zoom.value(midway + Duration::from_millis(200)), 600.);
        assert_eq!(
            zoom.update(900., midway + Duration::from_millis(210), false),
            900.
        );
    }
}
