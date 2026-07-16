use std::time::Duration;

use super::*;
use bindings::*;

/// RAII timer wrapper; stops and unhooks on drop.
pub struct DispatcherTimer {
    timer: DispatcherQueueTimer,
    _tick_revoker: windows_core::EventRevoker,
}

impl DispatcherTimer {
    pub fn new<F>(interval: Duration, f: F) -> Result<Self>
    where
        F: Fn() + 'static,
    {
        Self::build(interval, true, f)
    }

    pub fn new_one_shot<F>(after: Duration, f: F) -> Result<Self>
    where
        F: Fn() + 'static,
    {
        Self::build(after, false, f)
    }

    fn build<F>(interval: Duration, repeating: bool, f: F) -> Result<Self>
    where
        F: Fn() + 'static,
    {
        let queue = DispatcherQueue::GetForCurrentThread()?;
        let timer = queue.CreateTimer()?;
        timer.SetInterval(duration_to_timespan(interval))?;
        timer.SetIsRepeating(repeating)?;

        let tick_revoker = timer.Tick(move |_, _| {
            fault::catch("timer", &f);
        })?;
        timer.Start()?;
        Ok(Self {
            timer,
            _tick_revoker: tick_revoker,
        })
    }

    pub fn stop(&self) -> Result<()> {
        self.timer.Stop()
    }

    pub fn start(&self) -> Result<()> {
        self.timer.Start()
    }
}

impl Drop for DispatcherTimer {
    fn drop(&mut self) {
        let _ = self.timer.Stop();
    }
}

/// RAII handle for a `CompositionTarget::Rendering` subscription; detaches on drop.
pub struct Rendering {
    _revoker: windows_core::EventRevoker,
}

/// Subscribe `f` to `CompositionTarget::Rendering` for the current thread.
pub fn on_rendering<F>(f: F) -> Result<Rendering>
where
    F: Fn() + 'static,
{
    let revoker = CompositionTarget::Rendering(move |_, _| {
        fault::catch("rendering", &f);
    })?;
    Ok(Rendering { _revoker: revoker })
}

/// Animate a mounted XAML element horizontally on its compositor visual.
///
/// The offset is relative to the element's layout-managed X position, so XAML
/// remains free to arrange the element while the animation owns only `Offset.X`.
/// This is intended for retained page transitions where both pages are already
/// present in the visual tree.
pub fn animate_translation_x(
    native: windows_core::IInspectable,
    from_delta: f32,
    to_delta: f32,
    duration: Duration,
    easing: Easing,
) -> Result<()> {
    let ui = native.cast::<UIElement>()?;
    let visual = ElementCompositionPreview::GetElementVisual(&ui)?;
    let base_x = visual.cast::<IVisual>()?.Offset()?.x;
    let compositor = visual.cast::<ICompositionObject>()?.Compositor()?;
    let compositor = compositor.cast::<ICompositor>()?;
    let animation = compositor.CreateScalarKeyFrameAnimation()?;
    let keyframes = animation.cast::<IScalarKeyFrameAnimation>()?;
    animation
        .cast::<IKeyFrameAnimation>()?
        .SetDuration(duration_to_timespan(duration))?;
    let easing = composition_easing(&compositor, easing)?;
    keyframes.InsertKeyFrameWithEasingFunction(0.0, base_x + from_delta, &easing)?;
    keyframes.InsertKeyFrameWithEasingFunction(1.0, base_x + to_delta, &easing)?;
    visual
        .cast::<ICompositionObject>()?
        .StartAnimation("Offset.X", &animation.cast::<CompositionAnimation>()?)
}

fn composition_easing(
    compositor: &ICompositor,
    easing: Easing,
) -> Result<CompositionEasingFunction> {
    let (first, second) = match easing {
        Easing::Linear => {
            return compositor
                .CreateLinearEasingFunction()?
                .cast::<CompositionEasingFunction>();
        }
        Easing::EaseOut => ((0.0, 0.0), (0.58, 1.0)),
        Easing::EaseIn => ((0.42, 0.0), (1.0, 1.0)),
        Easing::EaseInOut => ((0.42, 0.0), (0.58, 1.0)),
        Easing::Fluent => ((0.55, 0.55), (0.0, 1.0)),
    };
    compositor
        .CreateCubicBezierEasingFunction(
            windows_numerics::Vector2 {
                x: first.0,
                y: first.1,
            },
            windows_numerics::Vector2 {
                x: second.0,
                y: second.1,
            },
        )?
        .cast::<CompositionEasingFunction>()
}

fn duration_to_timespan(d: Duration) -> TimeSpan {
    TimeSpan::try_from(d).unwrap_or(TimeSpan::MAX)
}
