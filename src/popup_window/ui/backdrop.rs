//! Capsule-only desktop blur underneath GPUI's transparent swap chain.
//!
//! GPUI 0.2.2 owns the topmost DirectComposition target. Windows permits a
//! second, non-topmost target on the same HWND, which hosts only our rounded
//! SpriteVisual. Unlike ACCENT_ENABLE_ACRYLICBLURBEHIND, the host backdrop
//! brush samples desktop content only where this clipped visual is painted.

use anyhow::{Context, Result};
use windows::{
    System::{DispatcherQueue, DispatcherQueueController},
    UI::{
        Color,
        Composition::{
            CompositionColorBrush, CompositionEffectSourceParameter,
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
        dark: bool,
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
        let luminosity = compositor.CreateColorBrushWithColor(luminosity_color(dark))?;
        brush.SetSourceParameter(&luminosity_name, &luminosity)?;
        let geometry = compositor.CreateRoundedRectangleGeometry()?;
        let clip = compositor.CreateGeometricClipWithGeometry(&geometry)?;
        let visual = compositor.CreateSpriteVisual()?;
        visual.SetBrush(&brush)?;
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
            _queue: queue,
            enabled: material == PopupBackgroundMaterial::Acrylic,
            visible: false,
            failed: false,
            last_geometry: None,
        })
    }

    pub(super) fn set_appearance(&mut self, material: PopupBackgroundMaterial, dark: bool) {
        if let Err(error) = self.luminosity.SetColor(luminosity_color(dark)) {
            self.failed = true;
            eprintln!("could not change capsule backdrop luminosity: {error}");
        }
        self.enabled = material == PopupBackgroundMaterial::Acrylic;
        self.apply_opacity();
    }

    pub(super) fn available(&self) -> bool {
        !self.failed
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

fn luminosity_color(dark: bool) -> Color {
    let level = if dark { 48 } else { 245 };
    Color {
        A: 255,
        R: level,
        G: level,
        B: level,
    }
}

impl Drop for Backdrop {
    fn drop(&mut self) {
        let _ = self.target.Close();
        let _ = self.compositor.Close();
    }
}
