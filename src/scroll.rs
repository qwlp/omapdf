/// Keyboard spring and velocity-preserving wheel glides with bounded travel.
#[derive(Debug)]
pub struct ScrollMotion {
    pub target: Option<f32>,
    velocity: f32,
    response: f32,
    wheel: bool,
    wheel_start: f32,
    wheel_elapsed: f32,
    wheel_duration: f32,
    wheel_initial_velocity: f32,
}
impl Default for ScrollMotion {
    fn default() -> Self {
        Self {
            target: None,
            velocity: 0.,
            response: 0.16,
            wheel: false,
            wheel_start: 0.,
            wheel_elapsed: 0.,
            wheel_duration: 0.28,
            wheel_initial_velocity: 0.,
        }
    }
}
impl ScrollMotion {
    pub fn cancel(&mut self) {
        self.target = None;
        self.velocity = 0.;
    }
    pub fn set_response(&mut self, seconds: f32) {
        self.response = seconds.clamp(0.06, 0.4);
    }
    /// Retarget a finite wheel glide without resetting its speed between notches.
    /// Cubic Hermite easing preserves velocity and arrives with zero velocity.
    pub fn queue_wheel(&mut self, current: f32, distance: f32, limit: f32) -> bool {
        let continuing = self.wheel && self.target.is_some();
        let queued = self.queue(current, distance, limit);
        self.wheel = true;
        if queued {
            let travel = self.target.unwrap() - current;
            self.wheel_start = current;
            self.wheel_elapsed = 0.;
            self.wheel_duration = (self.response * 1.75).clamp(0.16, 0.5);
            let speed = if continuing && self.velocity * travel > 0. {
                self.velocity.abs()
            } else {
                2. * travel.abs() / self.wheel_duration
            };
            self.wheel_initial_velocity =
                travel.signum() * speed.min(3. * travel.abs() / self.wheel_duration);
        }
        queued
    }
    pub fn queue(&mut self, current: f32, distance: f32, limit: f32) -> bool {
        self.wheel = false;
        let previous = self.target.unwrap_or(current);
        let reversing =
            (previous - current).abs() > 0.5 && (previous - current).signum() != distance.signum();
        let base = if reversing {
            self.velocity = 0.;
            current
        } else {
            previous
        };
        let backlog = distance.abs().max(240.) * 2.;
        let target = (base + distance)
            .clamp(current - backlog, current + backlog)
            .clamp(-limit.max(0.), 0.);
        if (target - previous).abs() < 0.01 && !reversing {
            return false;
        }
        if (target - current).abs() < 0.01 {
            self.cancel();
            return false;
        }
        self.target = Some(target);
        true
    }
    pub fn step(&mut self, current: f32, seconds: f32, limit: f32) -> f32 {
        let limit = limit.max(0.);
        let bounded = current.clamp(-limit, 0.);
        if bounded != current {
            self.velocity = 0.;
        }
        let Some(target) = self.target else {
            return bounded;
        };
        let target = target.clamp(-limit, 0.);
        self.target = Some(target);
        let dt = seconds.clamp(0., 0.1);
        if self.wheel {
            if (target - bounded).abs() < 0.001 {
                self.cancel();
                return target;
            }
            self.wheel_elapsed = (self.wheel_elapsed + dt).min(self.wheel_duration);
            let t = self.wheel_elapsed / self.wheel_duration;
            let travel = target - self.wheel_start;
            let tangent = self.wheel_initial_velocity * self.wheel_duration;
            let progress =
                (-2. * t * t * t + 3. * t * t) * travel + (t * t * t - 2. * t * t + t) * tangent;
            self.velocity = ((-6. * t * t + 6. * t) * travel
                + (3. * t * t - 4. * t + 1.) * tangent)
                / self.wheel_duration;
            let next =
                (self.wheel_start + progress).clamp(bounded.min(target), bounded.max(target));
            if self.wheel_elapsed >= self.wheel_duration {
                self.cancel();
                return target;
            }
            return next;
        }
        let omega = 4.75 / self.response;
        let error = bounded - target;
        let coefficient = self.velocity + omega * error;
        let decay = (-omega * dt).exp();
        let next = target + (error + coefficient * dt) * decay;
        self.velocity = (self.velocity - omega * coefficient * dt) * decay;
        let next = next.clamp(-limit, 0.);
        if (target - next).abs() < 0.2 && self.velocity.abs() < 5. {
            self.cancel();
            target
        } else {
            next
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn held_keys_settle_at_both_boundaries_without_reversing() {
        let mut motion = ScrollMotion::default();
        let mut y = 0.;
        for _ in 0..400 {
            motion.queue(y, -60., 900.);
            let next = motion.step(y, 1. / 144., 900.);
            assert!(next <= y + 0.0001 && next >= -900.);
            y = next;
        }
        assert_eq!(y, -900.);
        assert!(!motion.queue(y, -60., 900.));
        for _ in 0..400 {
            motion.queue(y, 60., 900.);
            let next = motion.step(y, 1. / 144., 900.);
            assert!(next >= y - 0.0001 && next <= 0.);
            y = next;
        }
        assert_eq!(y, 0.);
        assert!(!motion.queue(y, 60., 900.));
        assert!(motion.target.is_none());
    }
    #[test]
    fn spring_is_independent_of_refresh_rate_and_clamps_after_resize() {
        let simulate = |hz: usize| {
            let mut m = ScrollMotion::default();
            let mut y = 0.;
            m.queue(y, -500., 1000.);
            for _ in 0..hz / 10 {
                y = m.step(y, 1. / hz as f32, 1000.);
            }
            y
        };
        assert!((simulate(60) - simulate(120)).abs() < 0.1);
        let mut m = ScrollMotion::default();
        m.queue(0., -1000., 1000.);
        assert_eq!(m.step(-500., 0.016, 100.), -100.);
    }
    #[test]
    fn direction_changes_cancel_old_travel_instead_of_fighting_input() {
        let mut m = ScrollMotion::default();
        m.queue(0., -300., 1000.);
        let y = m.step(0., 0.04, 1000.);
        m.queue(y, 30., 1000.);
        assert!(m.step(y, 0.016, 1000.) > y);
        for _ in 0..100 {
            m.queue(y, -20., 100000.);
        }
        assert!((m.target.unwrap() - y).abs() <= 480.);
    }
    #[test]
    fn a_single_impulse_eases_in_and_settles_without_overshoot() {
        let mut m = ScrollMotion::default();
        m.queue(0., -100., 1000.);
        let mut y = m.step(0., 1. / 120., 1000.);
        assert!(y < 0. && y > -10.);
        for _ in 0..120 {
            let next = m.step(y, 1. / 120., 1000.);
            assert!(next <= y + 0.0001 && next >= -100.);
            y = next;
        }
        assert_eq!(y, -100.);
        assert!(m.target.is_none());
    }
    #[test]
    fn spaced_wheel_notches_keep_moving_and_then_settle_exactly() {
        let mut motion = ScrollMotion::default();
        let mut y = -100.;
        let mut minimum_step = f32::MAX;
        for frame in 0..240 {
            if frame % 15 == 0 {
                motion.queue_wheel(y, -60., 10000.);
            }
            let next = motion.step(y, 1. / 120., 10000.);
            if frame >= 30 {
                minimum_step = minimum_step.min(y - next);
            }
            assert!(next <= y);
            y = next;
        }
        // A wheel turning at eight notches per second must never pause between notches.
        assert!(minimum_step > 1., "smallest frame movement: {minimum_step}");
        for _ in 0..240 {
            y = motion.step(y, 1. / 120., 10000.);
        }
        assert_eq!(y, -1060.);
        assert!(motion.target.is_none());
        motion.queue_wheel(y, -60., 10000.);
        y = motion.step(y, 0.04, 10000.);
        motion.queue_wheel(y, 60., 10000.);
        assert!(motion.step(y, 1. / 120., 10000.) > y);
    }
    #[test]
    fn wheel_retarget_preserves_speed_and_finishes_in_a_bounded_time() {
        let mut motion = ScrollMotion::default();
        let mut y = -100.;
        motion.queue_wheel(y, -60., 10000.);
        for _ in 0..8 {
            y = motion.step(y, 1. / 120., 10000.);
        }
        let velocity = motion.velocity;
        motion.queue_wheel(y, -60., 10000.);
        assert!((motion.wheel_initial_velocity - velocity).abs() < 0.001);
        assert_eq!(motion.step(y, 0., 10000.), y);
        for _ in 0..60 {
            let next = motion.step(y, 1. / 120., 10000.);
            assert!(next <= y && next >= -220.);
            y = next;
        }
        assert_eq!(y, -220.);
        assert!(motion.target.is_none());
        let simulate = |hz: usize| {
            let mut motion = ScrollMotion::default();
            let mut y = 0.;
            motion.queue_wheel(y, -120., 1000.);
            for _ in 0..hz / 10 {
                y = motion.step(y, 1. / hz as f32, 1000.);
            }
            y
        };
        assert!((simulate(60) - simulate(120)).abs() < 0.01);
    }
}
