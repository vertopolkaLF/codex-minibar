//! Direct2D blur and luminosity descriptions for Windows Composition, without Win2D.
//! Implements the same effect/property contract as Microsoft's Win32 sample:
//! https://github.com/microsoft/Windows.UI.Composition-Win32-Samples/blob/master/cpp/HelloComposition/HelloComposition/microsoft.ui.composition.effects_impl.h

use std::sync::Mutex;

use windows::{
    Foundation::{IPropertyValue, PropertyValue},
    Graphics::Effects::{
        IGraphicsEffect, IGraphicsEffect_Impl, IGraphicsEffectSource, IGraphicsEffectSource_Impl,
    },
    Win32::{
        Foundation::{E_INVALIDARG, E_POINTER},
        Graphics::Direct2D::{
            CLSID_D2D1Blend, CLSID_D2D1GaussianBlur,
            Common::{D2D1_BLEND_MODE_COLOR, D2D1_BORDER_MODE_HARD},
            D2D1_GAUSSIANBLUR_OPTIMIZATION_BALANCED,
        },
        System::WinRT::Graphics::Direct2D::{
            GRAPHICS_EFFECT_PROPERTY_MAPPING, GRAPHICS_EFFECT_PROPERTY_MAPPING_DIRECT,
            IGraphicsEffectD2D1Interop, IGraphicsEffectD2D1Interop_Impl,
        },
    },
};
use windows_core::{GUID, HSTRING, Interface, PCWSTR, Result, implement};

#[implement(IGraphicsEffect, IGraphicsEffectSource, IGraphicsEffectD2D1Interop)]
struct Effect {
    name: Mutex<HSTRING>,
    sources: Vec<IGraphicsEffectSource>,
    kind: EffectKind,
}

enum EffectKind {
    Blur(f32),
    Luminosity,
}

pub(super) fn gaussian_blur(source: IGraphicsEffectSource, sigma: f32) -> IGraphicsEffect {
    Effect {
        name: Mutex::new(HSTRING::from("CapsuleBlur")),
        sources: vec![source],
        kind: EffectKind::Blur(sigma),
    }
    .into()
}

pub(super) fn luminosity_blend(
    backdrop: IGraphicsEffectSource,
    luminosity: IGraphicsEffectSource,
) -> IGraphicsEffect {
    Effect {
        name: Mutex::new(HSTRING::from("CapsuleLuminosity")),
        sources: vec![backdrop, luminosity],
        kind: EffectKind::Luminosity,
    }
    .into()
}

impl IGraphicsEffectSource_Impl for Effect_Impl {}

impl IGraphicsEffect_Impl for Effect_Impl {
    fn Name(&self) -> Result<HSTRING> {
        Ok(self.name.lock().unwrap_or_else(|e| e.into_inner()).clone())
    }

    fn SetName(&self, name: &HSTRING) -> Result<()> {
        *self.name.lock().unwrap_or_else(|e| e.into_inner()) = name.clone();
        Ok(())
    }
}

impl IGraphicsEffectD2D1Interop_Impl for Effect_Impl {
    fn GetEffectId(&self) -> Result<GUID> {
        Ok(match self.kind {
            EffectKind::Blur(_) => CLSID_D2D1GaussianBlur,
            EffectKind::Luminosity => CLSID_D2D1Blend,
        })
    }

    fn GetNamedPropertyMapping(
        &self,
        name: &PCWSTR,
        index: *mut u32,
        mapping: *mut GRAPHICS_EFFECT_PROPERTY_MAPPING,
    ) -> Result<()> {
        if name.is_null() || index.is_null() || mapping.is_null() {
            return Err(E_POINTER.into());
        }
        // The Composition ABI supplies a null-terminated property name and
        // writable output pointers; reject all unknown names explicitly.
        let property_name =
            unsafe { name.to_string() }.map_err(|_| windows_core::Error::from(E_INVALIDARG))?;
        let property = match (&self.kind, property_name.as_str()) {
            (EffectKind::Blur(_), "BlurAmount") | (EffectKind::Luminosity, "Mode") => 0,
            (EffectKind::Blur(_), "Optimization") => 1,
            (EffectKind::Blur(_), "BorderMode") => 2,
            _ => return Err(E_INVALIDARG.into()),
        };
        unsafe {
            index.write(property);
            mapping.write(GRAPHICS_EFFECT_PROPERTY_MAPPING_DIRECT);
        }
        Ok(())
    }

    fn GetPropertyCount(&self) -> Result<u32> {
        Ok(match self.kind {
            EffectKind::Blur(_) => 3,
            EffectKind::Luminosity => 1,
        })
    }

    fn GetProperty(&self, index: u32) -> Result<IPropertyValue> {
        let sigma = match self.kind {
            EffectKind::Blur(sigma) => sigma,
            EffectKind::Luminosity if index == 0 => {
                // Composition's Color/Luminosity behavior is swapped. Match
                // Microsoft's Acrylic recipe: COLOR uses backdrop chroma and
                // foreground lightness. LUMINOSITY retains white wallpaper
                // brightness and washes out the dark surface.
                // https://github.com/microsoft/microsoft-ui-xaml/blob/5e0e4df6592adbe973b53a6b7323e07148bc2a8e/dev/Materials/Acrylic/AcrylicBrush.cpp#L604-L615
                return PropertyValue::CreateUInt32(D2D1_BLEND_MODE_COLOR.0 as u32)?.cast();
            }
            EffectKind::Luminosity => return Err(E_INVALIDARG.into()),
        };
        match index {
            0 => PropertyValue::CreateSingle(sigma)?.cast(),
            1 => PropertyValue::CreateUInt32(D2D1_GAUSSIANBLUR_OPTIMIZATION_BALANCED.0 as u32)?
                .cast(),
            // Clamp the sample at the edge instead of fading to transparent.
            2 => PropertyValue::CreateUInt32(D2D1_BORDER_MODE_HARD.0 as u32)?.cast(),
            _ => Err(E_INVALIDARG.into()),
        }
    }

    fn GetSource(&self, index: u32) -> Result<IGraphicsEffectSource> {
        self.sources
            .get(index as usize)
            .cloned()
            .ok_or_else(|| E_INVALIDARG.into())
    }

    fn GetSourceCount(&self) -> Result<u32> {
        Ok(self.sources.len() as u32)
    }
}
