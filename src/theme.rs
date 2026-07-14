//! Shared animation tokens for both WinUI surfaces.

use std::time::Duration;

/// WinUI `ControlFasterAnimationDuration` — pointer-over / micro-interactions.
pub const CONTROL_FASTER_ANIMATION: Duration = Duration::from_millis(83);
/// WinUI `ControlFastAnimationDuration`.
pub const CONTROL_FAST_ANIMATION: Duration = Duration::from_millis(167);
/// WinUI `ControlNormalAnimationDuration`.
pub const CONTROL_NORMAL_ANIMATION: Duration = Duration::from_millis(250);
