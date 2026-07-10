//! Shared visual tokens for both WinUI surfaces.

use windows_reactor::Color;

pub const SURFACE_FILL: Color = Color { a: 10, r: 255, g: 255, b: 255 };
pub const DARK_SURFACE_FILL: Color = Color { a: 70, r: 0, g: 0, b: 0 };
pub const WINDOW_FILL: Color = Color { a: 160, r: 32, g: 32, b: 36 };
pub const WINDOW_BORDER: Color = Color { a: 10, r: 255, g: 255, b: 255 };

/// `#FFFFFF05`: the static settings-page wash that composites over Mica.
pub const SETTINGS_CONTENT_FILL: Color = Color { a: 0x05, r: 255, g: 255, b: 255 };
