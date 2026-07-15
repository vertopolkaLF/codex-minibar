//! Rounded Acrylic via WinUI `SystemBackdropElement` (WASDK 2.x).
//!
//! Window-level `SystemBackdrop` ignores `SetWindowRgn` and paints a square
//! frame + DWM shadow. Element-level acrylic respects `CornerRadius` and stays
//! inside the popup chrome.
//!
//! Installed as a child of a reactor `SwapChainPanel` (a `Panel` whose children
//! the reconciler does not manage), so UI re-renders do not rip it out.

#![allow(non_snake_case)]

use windows_core::{self, Interface, Result, RuntimeName, RuntimeType, Type, imp::FactoryCache};

/// XAML host: acrylic clipped to the same radius as the popup chrome.
fn acrylic_xaml() -> String {
    format!(
        r#"
<SystemBackdropElement
    xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
    CornerRadius="{}"
    HorizontalAlignment="Stretch"
    VerticalAlignment="Stretch">
    <SystemBackdropElement.SystemBackdrop>
        <DesktopAcrylicBackdrop />
    </SystemBackdropElement.SystemBackdrop>
</SystemBackdropElement>
"#,
        crate::popup::WINDOW_CORNER_RADIUS_DIP
    )
}

/// Full-window Mica hosted inside the XAML visual tree. Unlike
/// `Window.SystemBackdrop`, this is composed with the rest of the UI instead
/// of being presented as a separate window backdrop surface. Keeping the
/// radius on this element is important: the popup HWND is region-clipped and
/// must not reveal square Mica corners while it slides in.
fn mica_xaml() -> String {
    format!(
        r#"
<SystemBackdropElement
    xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
    CornerRadius="{}"
    HorizontalAlignment="Stretch"
    VerticalAlignment="Stretch">
    <SystemBackdropElement.SystemBackdrop>
        <MicaBackdrop />
    </SystemBackdropElement.SystemBackdrop>
</SystemBackdropElement>
"#,
        crate::popup::WINDOW_CORNER_RADIUS_DIP
    )
}

/// GitHub mark rendered as a XAML path rather than an `Image`-hosted SVG, so
/// its fill can bind to WinUI's live system accent brush.
fn accent_github_xaml() -> &'static str {
    r#"
<Viewbox
    xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
    Stretch="Uniform">
    <Path
        Fill="{ThemeResource AccentFillColorDefaultBrush}"
        Data="M12 .297c-6.63 0-12 5.373-12 12c0 5.303 3.438 9.8 8.205 11.385c.6.113.82-.258.82-.577c0-.285-.01-1.04-.015-2.04c-3.338.724-4.042-1.61-4.042-1.61C4.422 18.07 3.633 17.7 3.633 17.7c-1.087-.744.084-.729.084-.729c1.205.084 1.838 1.236 1.838 1.236c1.07 1.835 2.809 1.305 3.495.998c.108-.776.417-1.305.76-1.605c-2.665-.3-5.466-1.332-5.466-5.93c0-1.31.465-2.38 1.235-3.22c-.135-.303-.54-1.523.105-3.176c0 0 1.005-.322 3.3 1.23c.96-.267 1.98-.399 3-.405c1.02.006 2.04.138 3 .405c2.28-1.552 3.285-1.23 3.285-1.23c.645 1.653.24 2.873.12 3.176c.765.84 1.23 1.91 1.23 3.22c0 4.61-2.805 5.625-5.475 5.92c.42.36.81 1.096.81 2.22c0 1.606-.015 2.896-.015 3.286c0 .315.21.69.825.57C20.565 22.092 24 17.592 24 12.297c0-6.627-5.373-12-12-12" />
</Viewbox>
"#
}

/// Host a rounded `SystemBackdropElement` inside `mount` (a `Panel`).
pub fn install_into(mount: windows_core::IInspectable) {
    let _ = install_into_inner(mount, &acrylic_xaml());
}

/// Host Mica inside `mount` as part of the XAML composition tree.
pub fn install_mica_into(mount: windows_core::IInspectable) -> Result<()> {
    install_into_inner(mount, &mica_xaml())
}

/// Host a Phosphor path with a caller-supplied color. The geometry and tint
/// deliberately stay independent so controls can react to hover/theme state.
pub fn install_colored_icon_into(
    mount: windows_core::IInspectable,
    path: &str,
    color: (u8, u8, u8),
) -> Result<()> {
    let (r, g, b) = color;
    let xaml = format!(
        r##"<Viewbox xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Stretch="Uniform"><Path Fill="#{r:02X}{g:02X}{b:02X}" Data="{path}" /></Viewbox>"##
    );
    install_into_inner(mount, &xaml)
}

/// Host a data-driven XAML chart inside a reactor swap-chain panel. The
/// caller provides only locally generated XAML geometry (never user input).
pub fn install_spend_donut_into(mount: windows_core::IInspectable, xaml: &str) -> Result<()> {
    install_into_inner(mount, xaml)
}

/// Host a GitHub SVG path whose color follows the current Windows accent.
pub fn install_accent_github_icon_into(mount: windows_core::IInspectable) -> Result<()> {
    install_into_inner(mount, accent_github_xaml())
}

fn install_into_inner(mount: windows_core::IInspectable, xaml: &str) -> Result<()> {
    let panel: IPanel = mount.cast()?;
    let children = panel.Children()?;
    if children.Size()? > 0 {
        return Ok(());
    }
    let backdrop = XamlReader::Load(xaml)?;
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

windows_core::imp::define_interface!(IPanel, IPanel_Vtbl, 0x27a1b418_56f3_525e_b883_cefed905eed3);
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
        windows_core::imp::ConstBuffer::for_class::<Self, windows_collections::IVector<UIElement>>(
        );
}

unsafe impl Interface for UIElementCollection {
    type Vtable = <windows_collections::IVector<UIElement> as Interface>::Vtable;
    const IID: windows_core::GUID = <windows_collections::IVector<UIElement> as Interface>::IID;
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
