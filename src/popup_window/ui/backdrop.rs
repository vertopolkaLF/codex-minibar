//! Capsule-only desktop blur underneath GPUI's transparent swap chain.
//!
//! GPUI 0.2.2 owns the topmost DirectComposition target. Windows permits a
//! second, non-topmost target on the same HWND, which hosts only our rounded
//! SpriteVisual. Unlike ACCENT_ENABLE_ACRYLICBLURBEHIND, the host backdrop
//! brush samples desktop content only where this clipped visual is painted.
//!
//! Mica swaps the visual's brush for the compositor's blurred-wallpaper brush
//! (the same source DWM's Mica uses), so it tints from the desktop wallpaper
//! rather than the windows behind the popup.

use anyhow::{Context, Result};
use windows::{
    System::{DispatcherQueue, DispatcherQueueController},
    UI::{
        Color,
        Composition::{
            CompositionBrush, CompositionColorBrush, CompositionEffectSourceParameter,
            CompositionRoundedRectangleGeometry, Compositor, Desktop::DesktopWindowTarget,
            SpriteVisual,
        },
    },
    Win32::{
        Foundation::HWND,
        System::WinRT::{
            Composition::ICompositorDesktopInterop, CreateDispatcherQueueController,
            DQTAT_COM_NONE, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions,
        },
    },
};
use windows_core::Interface;
use windows_numerics::{Vector2, Vector3};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_USE_HOSTBACKDROPBRUSH, DwmSetWindowAttribute};

use crate::settings::PopupBackgroundMaterial;

/// Shut down only a dispatcher queue we created ourselves. GPUI initializes
/// OLE on this thread; the queue must not change its COM apartment.
struct QueueOwner(Option<DispatcherQueueController>);

impl QueueOwner {
    fn new() -> Result<Self> {
        if DispatcherQueue::GetForCurrentThread().is_ok() {
            return Ok(Self(None));
        }
        let options = DispatcherQueueOptions {
            dwSize: size_of::<DispatcherQueueOptions>() as u32,
            threadType: DQTYPE_THREAD_CURRENT,
            apartmentType: DQTAT_COM_NONE,
        };
        Ok(Self(Some(unsafe {
            CreateDispatcherQueueController(options)?
        })))
    }
}

impl Drop for QueueOwner {
    fn drop(&mut self) {
        if let Some(controller) = &self.0 {
            let _ = controller.ShutdownQueueAsync();
        }
    }
}

pub(super) struct Backdrop {
    // Keep the lower target and its compositor alive as long as the popup.
    target: DesktopWindowTarget,
    compositor: Compositor,
    visual: SpriteVisual,
    geometry: CompositionRoundedRectangleGeometry,
    luminosity: CompositionColorBrush,
    acrylic: CompositionBrush,
    /// None where the system has no blurred-wallpaper brush (pre-Win11).
    mica: Option<CompositionBrush>,
    material: PopupBackgroundMaterial,
    _queue: QueueOwner,
    enabled: bool,
    visible: bool,
    failed: bool,
    last_geometry: Option<[f32; 5]>,
}

impl Backdrop {
    pub(super) fn new(
        hwnd: windows_sys::Win32::Foundation::HWND,
        material: PopupBackgroundMaterial,
        luminosity: (u8, u8, u8),
    ) -> Result<Self> {
        let queue = QueueOwner::new().context("creating backdrop dispatcher queue")?;
        let compositor = Compositor::new().context("creating backdrop compositor")?;
        let interop: ICompositorDesktopInterop = compositor.cast()?;
        // false uses the lower slot; true would conflict with GPUI's target.
        let target = unsafe { interop.CreateDesktopWindowTarget(HWND(hwnd), false) }
            .context("attaching backdrop below GPUI")?;

        // This enables brush access to the desktop, NOT a window-wide material.
        // It is supported on Win11; older systems retain plain transparency.
        let enabled = 1i32;
        let status = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_HOSTBACKDROPBRUSH as u32,
                &enabled as *const i32 as *const _,
                size_of::<i32>() as u32,
            )
        };
        windows_core::HRESULT(status)
            .ok()
            .context("enabling desktop access for the backdrop brush")?;

        let desktop = compositor.CreateHostBackdropBrush()?;
        let source_name = windows_core::HSTRING::from("desktop");
        let source = CompositionEffectSourceParameter::Create(&source_name)?;
        let blurred = super::blur_effect::gaussian_blur(source.cast()?, 24.0);
        let luminosity_name = windows_core::HSTRING::from("luminosity");
        let luminosity_source = CompositionEffectSourceParameter::Create(&luminosity_name)?;
        let effect =
            super::blur_effect::luminosity_blend(blurred.cast()?, luminosity_source.cast()?);
        let factory = compositor
            .CreateEffectFactory(&effect)
            .context("creating capsule blur and luminosity effect")?;
        let brush = factory.CreateBrush()?;
        brush.SetSourceParameter(&source_name, &desktop)?;
        let luminosity = compositor.CreateColorBrushWithColor(luminosity_color(luminosity))?;
        brush.SetSourceParameter(&luminosity_name, &luminosity)?;
        let acrylic: CompositionBrush = brush.cast()?;
        let mica = match mica_brush(&compositor, &luminosity) {
            Ok(mica) => Some(mica),
            Err(error) => {
                eprintln!("Mica wallpaper brush unavailable; using a solid popup: {error:#}");
                None
            }
        };
        let geometry = compositor.CreateRoundedRectangleGeometry()?;
        let clip = compositor.CreateGeometricClipWithGeometry(&geometry)?;
        let visual = compositor.CreateSpriteVisual()?;
        let initial = match (material, &mica) {
            (PopupBackgroundMaterial::Mica, Some(mica)) => mica,
            _ => &acrylic,
        };
        visual.SetBrush(initial)?;
        visual.SetClip(&clip)?;
        visual.SetOpacity(0.0)?;
        target.SetRoot(&visual)?;
        eprintln!("capsule backdrop ready: clipped Gaussian blur + luminosity, sigma=24px");

        Ok(Self {
            target,
            compositor,
            visual,
            geometry,
            luminosity,
            acrylic,
            mica,
            material,
            _queue: queue,
            enabled: false,
            visible: false,
            failed: false,
            last_geometry: None,
        }
        .with_enabled())
    }

    fn with_enabled(mut self) -> Self {
        self.enabled = self.supports(self.material);
        self
    }

    /// Whether this backdrop can paint `material` (Solid never paints).
    pub(super) fn supports(&self, material: PopupBackgroundMaterial) -> bool {
        !self.failed
            && match material {
                PopupBackgroundMaterial::Acrylic => true,
                PopupBackgroundMaterial::Mica => self.mica.is_some(),
                PopupBackgroundMaterial::Solid => false,
            }
    }

    /// `luminosity` is the color whose lightness the blurred backdrop takes;
    /// its hue and saturation still come from the desktop.
    pub(super) fn set_appearance(
        &mut self,
        material: PopupBackgroundMaterial,
        luminosity: (u8, u8, u8),
    ) {
        if let Err(error) = self.luminosity.SetColor(luminosity_color(luminosity)) {
            self.failed = true;
            eprintln!("could not change capsule backdrop luminosity: {error}");
        }
        if material != self.material {
            let brush = match (material, &self.mica) {
                (PopupBackgroundMaterial::Mica, Some(mica)) => Some(mica),
                (PopupBackgroundMaterial::Acrylic, _) => Some(&self.acrylic),
                _ => None,
            };
            if let Some(brush) = brush
                && let Err(error) = self.visual.SetBrush(brush)
            {
                self.failed = true;
                eprintln!("could not switch popup backdrop material: {error}");
            }
            self.material = material;
        }
        self.enabled = self.supports(material);
        self.apply_opacity();
    }

    pub(super) fn hide(&mut self) {
        self.visible = false;
        self.apply_opacity();
    }

    /// Physical pixels, including the untrimmed capsule bounds during its
    /// slide. Viewport clipping must not move the capsule's rounded corners.
    pub(super) fn update(&mut self, x: f32, y: f32, width: f32, height: f32, radius: f32) {
        if self.failed {
            return;
        }
        let bounds = [x, y, width, height, radius];
        if self.last_geometry != Some(bounds) {
            let size = Vector2 {
                x: width,
                y: height,
            };
            let radius = radius.max(0.0).min(width * 0.5).min(height * 0.5);
            let update = || -> windows_core::Result<()> {
                self.geometry.SetSize(size)?;
                self.geometry.SetCornerRadius(Vector2 {
                    x: radius,
                    y: radius,
                })?;
                self.visual.SetSize(size)?;
                self.visual.SetOffset(Vector3 { x, y, z: 0.0 })
            };
            if let Err(error) = update() {
                // A partial update must not leave a stale glass rectangle.
                self.failed = true;
                let _ = self.visual.SetOpacity(0.0);
                eprintln!("could not update capsule backdrop: {error}");
                return;
            }
            self.last_geometry = Some(bounds);
        }
        if !self.visible {
            self.visible = true;
            self.apply_opacity();
        }
    }

    fn apply_opacity(&mut self) {
        let opacity = if self.enabled && self.visible && !self.failed {
            1.0
        } else {
            0.0
        };
        if let Err(error) = self.visual.SetOpacity(opacity) {
            self.failed = true;
            // Detach on failure rather than leaving an old visible backdrop.
            let _ = self
                .target
                .SetRoot(None::<&windows::UI::Composition::Visual>);
            eprintln!("could not change capsule backdrop visibility: {error}");
        }
    }
}

/// Mica: the system's blurred wallpaper, recolored to the theme's lightness.
/// The GPUI capsule tint supplies Mica's tint opacity on top.
fn mica_brush(
    compositor: &Compositor,
    luminosity: &CompositionColorBrush,
) -> Result<CompositionBrush> {
    let wallpaper = compositor
        .TryCreateBlurredWallpaperBackdropBrush()
        .context("creating blurred wallpaper brush")?;
    let source_name = windows_core::HSTRING::from("wallpaper");
    let source = CompositionEffectSourceParameter::Create(&source_name)?;
    let luminosity_name = windows_core::HSTRING::from("luminosity");
    let luminosity_source = CompositionEffectSourceParameter::Create(&luminosity_name)?;
    let effect = super::blur_effect::luminosity_blend(source.cast()?, luminosity_source.cast()?);
    let factory = compositor
        .CreateEffectFactory(&effect)
        .context("creating Mica luminosity effect")?;
    let brush = factory.CreateBrush()?;
    brush.SetSourceParameter(&source_name, &wallpaper)?;
    brush.SetSourceParameter(&luminosity_name, luminosity)?;
    Ok(brush.cast()?)
}

fn luminosity_color((r, g, b): (u8, u8, u8)) -> Color {
    Color {
        A: 255,
        R: r,
        G: g,
        B: b,
    }
}

impl Drop for Backdrop {
    fn drop(&mut self) {
        let _ = self.target.Close();
        let _ = self.compositor.Close();
    }
}
