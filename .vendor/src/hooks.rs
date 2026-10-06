use std::time::Duration;

use super::*;
use bindings::*;

/// Clip a mounted element and its descendants to its live rounded bounds.
pub fn install_rounded_clip(native: windows_core::IInspectable, radius: f64) -> Result<()> {
    use crate::clip_bindings as clip;
    let ui = native.cast::<UIElement>()?;
    let visual = ElementCompositionPreview::GetElementVisual(&ui)?;
    let compositor = visual.cast::<ICompositionObject>()?.Compositor()?;
    let rectangle = compositor
        .cast::<clip::ICompositor7>()?
        .CreateRectangleClip()?;
    let corners = windows_numerics::Vector2 {
        x: radius as f32,
        y: radius as f32,
    };
    rectangle.SetTopLeftRadius(corners)?;
    rectangle.SetTopRightRadius(corners)?;
    rectangle.SetBottomLeftRadius(corners)?;
    rectangle.SetBottomRightRadius(corners)?;
    // Composition expressions track resizing and layout animations without
    // a SizeChanged subscription or a one-frame stale clip.
    for (edge, expression) in [("Right", "card.Size.X"), ("Bottom", "card.Size.Y")] {
        let animation = compositor
            .cast::<clip::ICompositor>()?
            .CreateExpressionAnimationWithExpression(expression)?;
        animation
            .cast::<clip::ICompositionAnimation>()?
            .SetReferenceParameter("card", &visual.cast::<clip::CompositionObject>()?)?;
        rectangle
            .cast::<clip::ICompositionObject>()?
            .StartAnimation(edge, &animation.cast::<clip::CompositionAnimation>()?)?;
    }
    visual.cast::<IVisual>()?.SetClip(Some(&rectangle.cast()?))
}

/// Clear clips before the reconciler can reuse a native element.
pub fn clear_rounded_clip(native: windows_core::IInspectable) -> Result<()> {
    let ui = native.cast::<UIElement>()?;
    ElementCompositionPreview::GetElementVisual(&ui)?
        .cast::<IVisual>()?
        .SetClip(None)
}

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

/// Snap `UIElement.Translation` in both axes. Hit-testing follows the visual,
/// unlike compositor `Offset`.
pub fn set_translation_xy(native: windows_core::IInspectable, x: f32, y: f32) -> Result<()> {
    let ui = native.cast::<UIElement>()?;
    ui.SetTranslation(windows_numerics::Vector3 { x, y, z: 0.0 })
}

/// Pointer events pass through when `visible` is false.
pub fn set_hit_test_visible(native: windows_core::IInspectable, visible: bool) -> Result<()> {
    let ui = native.cast::<UIElement>()?;
    ui.SetIsHitTestVisible(visible)
}

/// Snap compositor `Offset.X` without touching XAML layout.
pub fn set_offset_x(native: windows_core::IInspectable, x: f32) -> Result<()> {
    let ui = native.cast::<UIElement>()?;
    let visual = ElementCompositionPreview::GetElementVisual(&ui)?;
    let iv = visual.cast::<IVisual>()?;
    let current = iv.Offset()?;
    iv.SetOffset(windows_numerics::Vector3 {
        x,
        y: current.y,
        z: current.z,
    })
}

/// Slide `Offset.X` from `from_x` to `to_x`. Same channel as popup page
/// transitions — do not use Margin or `UIElement.Translation` here.
pub fn animate_offset_x(
    native: windows_core::IInspectable,
    from_x: f32,
    to_x: f32,
    duration: Duration,
    easing: Easing,
) -> Result<()> {
    set_offset_x(native.clone(), from_x)?;
    if duration.is_zero() || (from_x - to_x).abs() < 0.5 {
        return set_offset_x(native, to_x);
    }
    animate_translation_x(native, 0.0, to_x - from_x, duration, easing)
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

/// Layout position in the XAML root, used to animate a retained block after reparenting.
/// Walk XAML parents because a visual's Offset is local to its layout parent.
pub fn layout_position(native: windows_core::IInspectable) -> Result<(f32, f32)> {
    let mut current = native;
    let (mut x, mut y) = (0.0, 0.0);
    for _ in 0..64 {
        let ui = current.cast::<UIElement>()?;
        let offset = ElementCompositionPreview::GetElementVisual(&ui)?
            .cast::<IVisual>()?
            .Offset()?;
        x += offset.x;
        y += offset.y;
        let Ok(parent) = current.cast::<IFrameworkElement>()?.Parent() else {
            break;
        };
        if parent.cast::<UIElement>().is_err() {
            break;
        }
        current = parent.cast()?;
    }
    Ok((x, y))
}

/// Animate displacement from the previous layout position into the new one.
/// The caller measures both positions after layout; neither animation changes DesiredSize.
pub fn animate_layout_displacement(
    native: windows_core::IInspectable,
    x: f32,
    y: f32,
    duration: Duration,
) -> Result<()> {
    let ui = native.cast::<UIElement>()?;
    let visual = ElementCompositionPreview::GetElementVisual(&ui)?;
    let offset = visual.cast::<IVisual>()?.Offset()?;
    let compositor = visual
        .cast::<ICompositionObject>()?
        .Compositor()?
        .cast::<ICompositor>()?;
    let easing = composition_easing(&compositor, Easing::Fluent)?;
    for (property, base, delta) in [("Offset.X", offset.x, x), ("Offset.Y", offset.y, y)] {
        let animation = compositor.CreateScalarKeyFrameAnimation()?;
        animation
            .cast::<IKeyFrameAnimation>()?
            .SetDuration(duration_to_timespan(duration))?;
        let keyframes = animation.cast::<IScalarKeyFrameAnimation>()?;
        keyframes.InsertKeyFrameWithEasingFunction(0.0, base + delta, &easing)?;
        keyframes.InsertKeyFrameWithEasingFunction(1.0, base, &easing)?;
        visual
            .cast::<ICompositionObject>()?
            .StartAnimation(property, &animation.cast::<CompositionAnimation>()?)?;
    }
    Ok(())
}

/// Queue a callback after XAML has arranged the newly reconciled tree.
pub fn after_layout(f: impl Fn() + 'static) -> Result<()> {
    DispatcherQueue::GetForCurrentThread()?.TryEnqueueWithPriority(
        DispatcherQueuePriority::Low,
        &DispatcherQueueHandler::new(f),
    )?;
    Ok(())
}

/// Update a retained layout host without reconciling its children. XAML layout
/// and pointer hit testing follow the width on the next compositor frame.
pub fn set_layout_width(native: windows_core::IInspectable, width: f64) -> Result<()> {
    native.cast::<IFrameworkElement>()?.SetWidth(width.max(1.0))
}
