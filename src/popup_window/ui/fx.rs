//! Retargetable value transitions for the GPUI popup.
//!
//! GPUI is immediate-mode: every frame rebuilds the element tree from state.
//! [`Fx`] keeps the in-flight value of every animated property between frames
//! so hover crossfades, selection pills, chevrons and reveals glide toward
//! their latest target instead of snapping. Retargeting mid-flight continues
//! from the current value, so rapid hover changes never jump.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

/// WinUI `ControlFasterAnimationDuration` — pointer-over / micro-interactions.
pub(crate) const FASTER: Duration = Duration::from_millis(83);
/// WinUI `ControlFastAnimationDuration`.
pub(crate) const FAST: Duration = Duration::from_millis(167);
/// Text crossfades in the period selector and Usage Stats title.
pub(crate) const TEXT_FADE: Duration = Duration::from_millis(200);
/// WinUI `ControlNormalAnimationDuration`.
pub(crate) const NORMAL: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug)]
struct Tween {
    from: f32,
    to: f32,
    started: Instant,
    duration: Duration,
    last_frame: u64,
}

impl Tween {
    fn value(&self, now: Instant) -> f32 {
        if self.duration.is_zero() {
            return self.to;
        }
        let t =
            now.saturating_duration_since(self.started).as_secs_f32() / self.duration.as_secs_f32();
        if t >= 1.0 {
            return self.to;
        }
        self.from + (self.to - self.from) * ease_out_cubic(t)
    }

    fn done(&self, now: Instant) -> bool {
        self.duration.is_zero() || now.saturating_duration_since(self.started) >= self.duration
    }
}

pub(crate) fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// CSS-style cubic bezier easing, solved for x with Newton + bisection.
pub(crate) fn cubic_bezier(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let bezier = |t: f64, p1: f64, p2: f64| {
        let u = 1.0 - t;
        3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t
    };
    let derivative = |t: f64, p1: f64, p2: f64| {
        let u = 1.0 - t;
        3.0 * u * u * p1 + 6.0 * u * t * (p2 - p1) + 3.0 * t * t * (1.0 - p2)
    };
    let mut t = x;
    for _ in 0..8 {
        let error = bezier(t, x1, x2) - x;
        if error.abs() < 1e-6 {
            return bezier(t, y1, y2);
        }
        let slope = derivative(t, x1, x2);
        if slope.abs() < 1e-6 {
            break;
        }
        t -= error / slope;
    }
    let (mut low, mut high) = (0.0, 1.0);
    t = x;
    for _ in 0..32 {
        let value = bezier(t, x1, x2);
        if (value - x).abs() < 1e-6 {
            break;
        }
        if value < x {
            low = t;
        } else {
            high = t;
        }
        t = (low + high) / 2.0;
    }
    bezier(t, y1, y2)
}

/// WinUI's Fluent "decelerate" curve used by page transitions.
pub(crate) fn fluent(x: f64) -> f64 {
    cubic_bezier(0.55, 0.55, 0.0, 1.0, x)
}

#[derive(Default)]
pub(crate) struct Fx {
    tweens: HashMap<u64, Tween>,
    frame: u64,
    now: Option<Instant>,
    enabled: bool,
    animating: bool,
}

/// Stable identity for an animated property (e.g. `("tab-hover", id)`).
pub(crate) fn key(parts: impl std::hash::Hash) -> u64 {
    use std::hash::{DefaultHasher, Hasher};
    let mut hasher = DefaultHasher::new();
    parts.hash(&mut hasher);
    hasher.finish()
}

impl Fx {
    pub(crate) fn begin_frame(&mut self, enabled: bool) {
        self.frame = self.frame.wrapping_add(1);
        self.now = Some(Instant::now());
        self.enabled = enabled;
        self.animating = false;
        if self.frame.is_multiple_of(240) {
            let frame = self.frame;
            self.tweens
                .retain(|_, tween| frame.wrapping_sub(tween.last_frame) < 240);
        }
    }

    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }

    fn now(&self) -> Instant {
        self.now.unwrap_or_else(Instant::now)
    }

    /// Current value of `id`, gliding toward `target` over `duration`.
    pub(crate) fn value(&mut self, id: u64, target: f32, duration: Duration) -> f32 {
        let now = self.now();
        let frame = self.frame;
        let enabled = self.enabled;
        let tween = self.tweens.entry(id).or_insert(Tween {
            from: target,
            to: target,
            started: now,
            duration: Duration::ZERO,
            last_frame: frame,
        });
        tween.last_frame = frame;
        if !enabled {
            tween.from = target;
            tween.to = target;
            tween.duration = Duration::ZERO;
            return target;
        }
        if (tween.to - target).abs() > f32::EPSILON {
            let current = tween.value(now);
            tween.from = current;
            tween.to = target;
            tween.started = now;
            tween.duration = duration;
        }
        let value = tween.value(now);
        if !tween.done(now) {
            self.animating = true;
        }
        value
    }

    /// Boolean convenience: 0 → 1 while `on`.
    pub(crate) fn toggle(&mut self, id: u64, on: bool, duration: Duration) -> f32 {
        self.value(id, if on { 1.0 } else { 0.0 }, duration)
    }

    /// Snap a value to `target` without animating (used on first mount).
    pub(crate) fn snap(&mut self, id: u64, target: f32) {
        let now = self.now();
        let frame = self.frame;
        self.tweens.insert(
            id,
            Tween {
                from: target,
                to: target,
                started: now,
                duration: Duration::ZERO,
                last_frame: frame,
            },
        );
    }

    pub(crate) fn mark_animating(&mut self) {
        self.animating = true;
    }

    pub(crate) fn is_animating(&self) -> bool {
        self.animating
    }
}

/// A critically damped spring that keeps its velocity when retargeted, so a
/// content height that changes mid-glide never makes the edge restart/bob.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Spring {
    pub(crate) position: f64,
    pub(crate) velocity: f64,
    pub(crate) target: f64,
    last_sample: Instant,
    pub(crate) settled: bool,
}

impl Spring {
    /// Critical-damping frequency for the popup shell height spring.
    pub(crate) const OMEGA: f64 = 24.0;

    pub(crate) fn at(value: f64) -> Self {
        Self {
            position: value,
            velocity: 0.0,
            target: value,
            last_sample: Instant::now(),
            settled: true,
        }
    }

    pub(crate) fn snap(&mut self, value: f64) {
        *self = Self::at(value);
    }

    pub(crate) fn retarget(&mut self, target: f64) {
        if (self.target - target).abs() <= 0.25 {
            return;
        }
        if self.settled {
            self.last_sample = Instant::now();
        }
        self.target = target;
        self.settled = false;
    }

    /// Advance with the closed-form critically damped solution. Stable across
    /// uneven frame intervals, unlike a hand-written Euler integrator.
    pub(crate) fn step(&mut self, now: Instant) -> f64 {
        if self.settled {
            self.position = self.target;
            self.last_sample = now;
            return self.position;
        }
        let dt = now
            .saturating_duration_since(self.last_sample)
            .as_secs_f64()
            .clamp(0.0, 0.05);
        self.last_sample = now;
        let offset = self.position - self.target;
        let omega = Self::OMEGA;
        let decay = (-omega * dt).exp();
        let next_offset = (offset + (self.velocity + omega * offset) * dt) * decay;
        let next_velocity = (self.velocity - omega * (self.velocity + omega * offset) * dt) * decay;
        self.position = self.target + next_offset;
        self.velocity = next_velocity;
        if next_offset.abs() < 0.25 && next_velocity.abs() < 1.0 {
            self.position = self.target;
            self.velocity = 0.0;
            self.settled = true;
        }
        self.position
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_converges_without_overshooting() {
        let started = Instant::now();
        let mut spring = Spring::at(300.0);
        spring.retarget(700.0);
        spring.last_sample = started;
        let mut previous = 300.0;
        for frame in 1..=120 {
            let value = spring.step(started + Duration::from_millis(frame * 16));
            assert!(value >= previous - 1e-9 && value <= 700.0 + 1e-9);
            previous = value;
        }
        assert!(spring.settled);
        assert_eq!(spring.position, 700.0);
    }

    #[test]
    fn spring_retarget_keeps_position_and_velocity() {
        let started = Instant::now();
        let mut spring = Spring::at(380.0);
        spring.retarget(760.0);
        spring.last_sample = started;
        spring.step(started + Duration::from_millis(48));
        let before = (spring.position, spring.velocity);
        spring.retarget(380.0);
        assert_eq!((spring.position, spring.velocity), before);
        for frame in 4..=160 {
            spring.step(started + Duration::from_millis(frame * 16));
        }
        assert_eq!(spring.position, 380.0);
    }

    #[test]
    fn fluent_curve_is_monotonic_with_exact_endpoints() {
        assert_eq!(fluent(0.0), 0.0);
        assert_eq!(fluent(1.0), 1.0);
        let samples = (0..=100)
            .map(|step| fluent(f64::from(step) / 100.0))
            .collect::<Vec<_>>();
        assert!(samples.windows(2).all(|pair| pair[0] <= pair[1] + 1e-9));
        // Decelerating: most of the travel happens early.
        assert!(fluent(0.3) > 0.5);
    }

    #[test]
    fn retargeted_tween_continues_from_its_current_value() {
        let mut fx = Fx::default();
        fx.begin_frame(true);
        assert_eq!(fx.value(1, 0.0, FAST), 0.0);
        fx.begin_frame(true);
        let start = fx.value(1, 1.0, FAST);
        assert!(start < 0.05);
        assert!(fx.is_animating());
        fx.begin_frame(false);
        assert_eq!(fx.value(1, 0.5, FAST), 0.5);
        assert!(!fx.is_animating());
    }
}
