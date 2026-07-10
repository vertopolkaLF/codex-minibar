//! Rounded Acrylic via WinUI `SystemBackdropElement` (WASDK 2.x).
//!
//! Window-level `SystemBackdrop` ignores `SetWindowRgn` and paints a square
//! frame + DWM shadow. Element-level acrylic respects `CornerRadius` and stays
//! inside the popup chrome.
//!
//! Installed as a child of a reactor `SwapChainPanel` (a `Panel` whose children
//! the reconciler does not manage), so UI re-renders do not rip it out.

#![allow(non_snake_case)]

use windows_core::{
    self, Interface, Result, RuntimeName, RuntimeType, Type, imp::FactoryCache,
};

/// XAML host: acrylic clipped to the same radius as the popup chrome.
const ACRYLIC_XAML: &str = r#"
<SystemBackdropElement
    xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
    CornerRadius="8"
    HorizontalAlignment="Stretch"
    VerticalAlignment="Stretch">
    <SystemBackdropElement.SystemBackdrop>
        <DesktopAcrylicBackdrop />
    </SystemBackdropElement.SystemBackdrop>
</SystemBackdropElement>
"#;

/// Host a rounded `SystemBackdropElement` inside `mount` (a `Panel`).
pub fn install_into(mount: windows_core::IInspectable) {
    let _ = install_into_inner(mount);
}

fn install_into_inner(mount: windows_core::IInspectable) -> Result<()> {
    let panel: IPanel = mount.cast()?;
    let children = panel.Children()?;
    if children.Size()? > 0 {
        return Ok(());
    }
    let backdrop = XamlReader::Load(ACRYLIC_XAML)?;
    let element: UIElement = backdrop.cast()?;
    children.Append(&element)?;
    Ok(())
}

// --- Minimal WinRT projections (not exported by windows-reactor) -------------

windows_core::imp::define_interface!(
    IXamlReaderStatics,
    IXamlReaderStatics_Vtbl,
    0x82a4cd9e_435e_5aeb_8c4f_300cece45cae
);
impl RuntimeType for IXamlReaderStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}

#[repr(C)]
pub struct IXamlReaderStatics_Vtbl {
    base__: windows_core::IInspectable_Vtbl,
    Load: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}

struct XamlReader;
impl RuntimeName for XamlReader {
    const NAME: &'static str = "Microsoft.UI.Xaml.Markup.XamlReader";
}

impl XamlReader {
    fn Load(xaml: impl AsRef<str>) -> Result<windows_core::IInspectable> {
        let factory: IXamlReaderStatics = {
            static SHARED: FactoryCache<XamlReader, IXamlReaderStatics> = FactoryCache::new();
            SHARED.call(|f| Ok(f.clone()))?
        };
        unsafe {
            let mut result = core::ptr::null_mut();
            (Interface::vtable(&factory).Load)(
                Interface::as_raw(&factory),
                core::mem::transmute_copy(&windows_core::HSTRING::from(xaml.as_ref())),
                &mut result,
            )
            .and_then(|| Type::from_abi(result))
        }
    }
}

windows_core::imp::define_interface!(
    IPanel,
    IPanel_Vtbl,
    0x27a1b418_56f3_525e_b883_cefed905eed3
);
impl RuntimeType for IPanel {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}

#[repr(C)]
pub struct IPanel_Vtbl {
    base__: windows_core::IInspectable_Vtbl,
    Children: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}

impl IPanel {
    fn Children(&self) -> Result<UIElementCollection> {
        unsafe {
            let mut result = core::ptr::null_mut();
            (Interface::vtable(self).Children)(Interface::as_raw(self), &mut result)
                .and_then(|| Type::from_abi(result))
        }
    }
}

windows_core::imp::define_interface!(
    IUIElement,
    IUIElement_Vtbl,
    0xc3c01020_320c_5cf6_9d24_d396bbfa4d8b
);
impl RuntimeType for IUIElement {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}

#[repr(C)]
pub struct IUIElement_Vtbl {
    base__: windows_core::IInspectable_Vtbl,
}

#[repr(transparent)]
#[derive(Clone, PartialEq, Eq)]
struct UIElement(windows_core::IUnknown);

impl RuntimeType for UIElement {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IUIElement>();
}

unsafe impl Interface for UIElement {
    type Vtable = IUIElement_Vtbl;
    const IID: windows_core::GUID = IUIElement::IID;
}

impl RuntimeName for UIElement {
    const NAME: &'static str = "Microsoft.UI.Xaml.UIElement";
}

#[repr(transparent)]
#[derive(Clone, PartialEq, Eq)]
struct UIElementCollection(windows_core::IUnknown);

impl RuntimeType for UIElementCollection {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, windows_collections::IVector<UIElement>>();
}

unsafe impl Interface for UIElementCollection {
    type Vtable = <windows_collections::IVector<UIElement> as Interface>::Vtable;
    const IID: windows_core::GUID =
        <windows_collections::IVector<UIElement> as Interface>::IID;
}

impl RuntimeName for UIElementCollection {
    const NAME: &'static str = "Microsoft.UI.Xaml.Controls.UIElementCollection";
}

impl core::ops::Deref for UIElementCollection {
    type Target = windows_collections::IVector<UIElement>;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
