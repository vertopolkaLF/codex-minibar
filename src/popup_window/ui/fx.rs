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

use gpui::SharedString;

/// WinUI `ControlFasterAnimationDuration` — pointer-over / micro-interactions.
pub(crate) const FASTER: Duration = Duration::from_millis(83);
/// WinUI `ControlFastAnimationDuration`.
pub(crate) const FAST: Duration = Duration::from_millis(167);
/// Text crossfades in the period selector and Usage Stats title.
pub(crate) const TEXT_FADE: Duration = Duration::from_millis(200);
/// WinUI `ControlNormalAnimationDuration`.
pub(crate) const NORMAL: Duration = Duration::from_millis(250);
/// Data changes: donut sweeps, rolling digits and reordered rows.
pub(crate) const SETTLE: Duration = Duration::from_millis(420);
/// Rolling digits: long enough to read the spin through intermediate digits.
pub(crate) const ROLL: Duration = Duration::from_millis(640);

#[derive(Clone, Copy, Debug)]
enum Easing {
    EaseOut,
    Smooth,
}

#[derive(Clone, Copy, Debug)]
struct Tween {
    from: f32,
    to: f32,
    started: Instant,
    duration: Duration,
    easing: Easing,
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
        let progress = match self.easing {
            Easing::EaseOut => ease_out_cubic(t),
            Easing::Smooth => cubic_bezier(0.9, 0.0, 0.1, 1.0, f64::from(t)) as f32,
        };
        self.from + (self.to - self.from) * progress
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

/// A label rolling from its previous text to the current one.
#[derive(Clone)]
struct Roll {
    from: Option<SharedString>,
    to: SharedString,
    value: u64,
    rising: bool,
    started: Instant,
    duration: Duration,
    last_frame: u64,
}

/// One frame of a [`Fx::roll`]: `from` is `None` once the label has settled.
pub(crate) struct RollFrame {
    pub(crate) from: Option<SharedString>,
    pub(crate) to: SharedString,
    /// Increasing values roll upward, like an odometer.
    pub(crate) rising: bool,
    pub(crate) progress: f32,
}

#[derive(Default)]
pub(crate) struct Fx {
    tweens: HashMap<u64, Tween>,
    stretches: HashMap<u64, IndicatorStretch>,
    rolls: HashMap<u64, Roll>,
    frame: u64,
    now: Option<Instant>,
    enabled: bool,
    animating: bool,
}

#[derive(Clone, Copy)]
struct IndicatorStretch {
    from: f32,
    to: f32,
    started: Instant,
    duration: Duration,
    last_frame: u64,
}

impl IndicatorStretch {
    fn width(&self, now: Instant) -> f32 {
        if self.duration.is_zero() {
            return self.to;
        }
        let t = (now.saturating_duration_since(self.started).as_secs_f32()
            / self.duration.as_secs_f32())
        .clamp(0.0, 1.0);
        let progress = cubic_bezier(0.9, 0.0, 0.1, 1.0, f64::from(t)) as f32;
        let base = self.from + (self.to - self.from) * progress;
        // Grow smoothly in the middle, capped at 30% above resting width.
        // Retargeting starts from the current width, not a fresh pulse at zero.
        let pulse = (std::f32::consts::PI * t).sin().powi(2);
        base + (self.to * 1.3 - base).max(0.0) * pulse
    }
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
            self.stretches
                .retain(|_, stretch| frame.wrapping_sub(stretch.last_frame) < 240);
            self.rolls
                .retain(|_, roll| frame.wrapping_sub(roll.last_frame) < 240);
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
        self.value_eased(id, target, duration, Easing::EaseOut)
    }

    /// A slow launch, fast middle and soft landing for the moving tab marker.
    pub(crate) fn value_smooth(&mut self, id: u64, target: f32, duration: Duration) -> f32 {
        self.value_eased(id, target, duration, Easing::Smooth)
    }

    /// Position and resting-width pulse for the bottom-bar selection marker.
    pub(crate) fn indicator(
        &mut self,
        id: u64,
        target: f32,
        width: f32,
        duration: Duration,
    ) -> (f32, f32) {
        let retargeted = self
            .tweens
            .get(&id)
            .is_some_and(|tween| (tween.to - target).abs() > f32::EPSILON);
        let x = self.value_smooth(id, target, duration);
        let now = self.now();
        let frame = self.frame;
        let stretch = self.stretches.entry(id).or_insert(IndicatorStretch {
            from: width,
            to: width,
            started: now,
            duration: Duration::ZERO,
            last_frame: frame,
        });
        stretch.last_frame = frame;
        if !self.enabled {
            stretch.from = width;
            stretch.to = width;
            stretch.duration = Duration::ZERO;
        } else if retargeted || (stretch.to - width).abs() > f32::EPSILON {
            stretch.from = stretch.width(now);
            stretch.to = width;
            stretch.started = now;
            stretch.duration = duration;
        }
        if now.saturating_duration_since(stretch.started) < stretch.duration {
            self.animating = true;
        }
        (x, stretch.width(now))
    }

    fn value_eased(&mut self, id: u64, target: f32, duration: Duration, easing: Easing) -> f32 {
        let now = self.now();
        let frame = self.frame;
        let enabled = self.enabled;
        let tween = self.tweens.entry(id).or_insert(Tween {
            from: target,
            to: target,
            started: now,
            duration: Duration::ZERO,
            easing,
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
            tween.easing = easing;
        }
        let value = tween.value(now);
        if !tween.done(now) {
            self.animating = true;
        }
        value
    }

    /// Text for a numeric label that rolls its changed characters from the
    /// previous text; `value` orders the two texts to pick the direction.
    pub(crate) fn roll(
        &mut self,
        id: u64,
        text: impl Into<SharedString>,
        value: u64,
        duration: Duration,
    ) -> RollFrame {
        let text = text.into();
        let now = self.now();
        let frame = self.frame;
        let enabled = self.enabled;
        let roll = self.rolls.entry(id).or_insert(Roll {
            from: None,
            to: text.clone(),
            value,
            rising: true,
            started: now,
            duration: Duration::ZERO,
            last_frame: frame,
        });
        roll.last_frame = frame;
        if !enabled {
            roll.from = None;
            roll.duration = Duration::ZERO;
        } else if roll.to != text {
            // A retarget mid-roll starts from the text it was heading to.
            roll.from = Some(roll.to.clone());
            roll.rising = value >= roll.value;
            roll.started = now;
            roll.duration = duration;
        }
        roll.to = text;
        roll.value = value;
        let t = if roll.duration.is_zero() {
            1.0
        } else {
            now.saturating_duration_since(roll.started).as_secs_f32() / roll.duration.as_secs_f32()
        };
        if t >= 1.0 {
            roll.from = None;
        }
        let animating = roll.from.is_some();
        let result = RollFrame {
            from: roll.from.clone(),
            to: roll.to.clone(),
            rising: roll.rising,
            // A quick start and a small overshoot so spinning digits land
            // with a little settle instead of creeping in.
            progress: cubic_bezier(0.34, 1.2, 0.64, 1.0, f64::from(t.min(1.0))) as f32,
        };
        if animating {
            self.animating = true;
        }
        result
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
                easing: Easing::EaseOut,
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

    pub(crate) fn is_at_target(&self, id: u64, target: f32) -> bool {
        self.tweens
            .get(&id)
            .is_none_or(|tween| (tween.value(self.now()) - target).abs() < 0.001)
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
    fn adding_an_instance_keeps_each_quota_animation_at_its_own_percentage() {
        use crate::{
            instances::ProviderId,
            limits::RateLimits,
            popup_window::{PopupSurface, model::*},
            settings::{PopupVisibility, ProviderKind},
        };

        for driver in [ProviderKind::Claude, ProviderKind::Codex] {
            for enabled in [true, false] {
                let visibility = PopupVisibility::build_defaults();
                let options = CardOptions {
                    popup_visibility: &visibility,
                    surface: PopupSurface::HomeTab,
                    show_provider_tabs: true,
                    include_usage_stats: false,
                    show_account_name: true,
                    drag_handle: false,
                    openrouter_actions: false,
                    provider_error: None,
                    now: chrono::Utc::now(),
                };
                let sample = |session_used, weekly_used| {
                    let mut limits = RateLimits::default();
                    limits.primary.used_percent = Some(session_used);
                    limits.secondary.used_percent = Some(weekly_used);
                    limits
                };
                let primary = ProviderId::primary(driver);
                let personal = ProviderId::new(driver, &format!("{}-personal", driver.id()));
                let mut instances = vec![(primary, sample(7, 20))];
                let mut fx = Fx::default();
                let started = Instant::now();
                let mut original_keys = Vec::new();
                for frame in 0..60 {
                    if frame == 1 {
                        instances.push((personal, sample(0, 0)));
                    }
                    if frame == 30 {
                        instances.reverse();
                    }
                    fx.begin_frame(enabled);
                    fx.now = Some(started + Duration::from_millis(frame * 16));
                    let mut frame_keys = std::collections::HashSet::new();
                    for (index, (provider, limits)) in instances.iter().enumerate() {
                        let cards =
                            provider_cards(*provider, index == 0, true, limits, &[], &options);
                        for card in &cards {
                            if let Card::Limit {
                                key: card_key,
                                window,
                                disabled,
                                ..
                            } = card
                            {
                                assert!(frame_keys.insert(card_key.clone()), "duplicate quota key");
                                let (_, target, _, _) =
                                    limit_card_presentation(window, false, *disabled);
                                let actual = fx.value(
                                    key(("limit-progress", card_key.as_str())),
                                    target as f32,
                                    NORMAL,
                                );
                                assert_eq!(actual, target as f32, "{provider:?} frame {frame}");
                                if *provider == primary {
                                    if frame == 0 {
                                        original_keys.push(card_key.clone());
                                    } else {
                                        assert!(original_keys.contains(card_key));
                                    }
                                }
                            }
                        }
                    }
                    assert_eq!(frame_keys.len(), instances.len() * 2);
                    assert!(!fx.is_animating(), "unchanged quotas must settle");
                }
            }
        }
    }

    #[test]
    fn indicator_stretches_then_settles_and_retargets_without_a_size_jump() {
        let started = Instant::now();
        let duration = Duration::from_millis(300);
        let mut fx = Fx::default();
        fx.begin_frame(true);
        fx.now = Some(started);
        assert_eq!(fx.indicator(1, 0.0, 24.0, duration), (0.0, 24.0));
        assert_eq!(fx.indicator(1, 100.0, 24.0, duration), (0.0, 24.0));
        fx.now = Some(started + duration / 2);
        let before = fx.indicator(1, 100.0, 24.0, duration);
        assert!(before.1 > 30.0 && before.1 <= 24.0 * 1.3);
        assert_eq!(fx.indicator(1, -50.0, 24.0, duration), before);
        fx.now = Some(started + duration + duration / 2);
        let settled = fx.indicator(1, -50.0, 24.0, duration);
        assert_eq!(settled.0, -50.0);
        assert!((settled.1 - 24.0).abs() < 0.001);
        fx.begin_frame(false);
        assert_eq!(fx.indicator(1, 100.0, 24.0, duration), (100.0, 24.0));
        assert!(!fx.is_animating());
    }

    #[test]
    fn smooth_indicator_retargets_without_jumping_and_respects_disabled_motion() {
        let started = Instant::now();
        let duration = Duration::from_millis(400);
        let mut fx = Fx::default();
        fx.begin_frame(true);
        fx.now = Some(started);
        assert_eq!(fx.value_smooth(1, 0.0, duration), 0.0);
        assert_eq!(fx.value_smooth(1, 100.0, duration), 0.0);
        fx.now = Some(started + Duration::from_millis(100));
        assert!(fx.value_smooth(1, 100.0, duration) < 15.0);
        fx.now = Some(started + Duration::from_millis(300));
        let before = fx.value_smooth(1, 100.0, duration);
        assert!(before > 85.0 && before < 100.0);
        assert_eq!(fx.value_smooth(1, 200.0, duration), before);
        fx.now = Some(started + Duration::from_millis(700));
        assert_eq!(fx.value_smooth(1, 200.0, duration), 200.0);
        fx.begin_frame(false);
        assert_eq!(fx.value_smooth(1, 0.0, duration), 0.0);
        assert!(!fx.is_animating());
    }

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

    #[test]
    fn rolled_label_keeps_its_previous_text_and_direction_until_it_settles() {
        let mut fx = Fx::default();
        fx.begin_frame(true);
        assert!(fx.roll(1, "$5.00", 500, SETTLE).from.is_none());
        fx.begin_frame(true);
        let frame = fx.roll(1, "$3.00", 300, SETTLE);
        assert_eq!(frame.from.as_ref().map(|text| text.as_ref()), Some("$5.00"));
        assert!(!frame.rising);
        assert!(fx.is_animating());
        fx.begin_frame(false);
        let frame = fx.roll(1, "$9.00", 900, SETTLE);
        assert!(frame.from.is_none());
        assert_eq!(frame.to, "$9.00");
        assert!(!fx.is_animating());
    }
}
