// Generated with windows-bindgen 0.66.0 (07f344c), --flat --minimal, from
// Windows App SDK Runtime 2.4.0 Microsoft.UI.winmd and Windows SDK 19041.
// Filters: Compositor.CreateRectangleClip, Compositor.CreateExpressionAnimationWithExpression,
// RectangleClip.{TopLeftRadius,TopRightRadius,BottomLeftRadius,BottomRightRadius},
// CompositionObject.StartAnimation, CompositionAnimation.SetReferenceParameter.
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionAnimation(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionAnimation,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionAnimation, CompositionObject);
impl windows_core::RuntimeType for CompositionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionAnimation>();
}
unsafe impl windows_core::Interface for CompositionAnimation {
    type Vtable = <ICompositionAnimation as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionAnimation as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionAnimation {
    type Target = ICompositionAnimation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionAnimation {
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionAnimation";
}
unsafe impl Send for CompositionAnimation {}
unsafe impl Sync for CompositionAnimation {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionClip(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionClip,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionClip, CompositionObject);
impl windows_core::RuntimeType for CompositionClip {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionClip>();
}
unsafe impl windows_core::Interface for CompositionClip {
    type Vtable = <ICompositionClip as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionClip as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionClip {
    type Target = ICompositionClip;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionClip {
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionClip";
}
unsafe impl Send for CompositionClip {}
unsafe impl Sync for CompositionClip {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionObject(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionObject,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for CompositionObject {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionObject>();
}
unsafe impl windows_core::Interface for CompositionObject {
    type Vtable = <ICompositionObject as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionObject as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionObject {
    type Target = ICompositionObject;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionObject {
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionObject";
}
unsafe impl Send for CompositionObject {}
unsafe impl Sync for CompositionObject {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Compositor(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    Compositor,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for Compositor {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositor>();
}
unsafe impl windows_core::Interface for Compositor {
    type Vtable = <ICompositor as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositor as windows_core::Interface>::IID;
}
impl core::ops::Deref for Compositor {
    type Target = ICompositor;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Compositor {
    const NAME: &'static str = "Microsoft.UI.Composition.Compositor";
}
unsafe impl Send for Compositor {}
unsafe impl Sync for Compositor {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpressionAnimation(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    ExpressionAnimation,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    ExpressionAnimation,
    CompositionAnimation,
    CompositionObject
);
impl windows_core::RuntimeType for ExpressionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IExpressionAnimation>();
}
unsafe impl windows_core::Interface for ExpressionAnimation {
    type Vtable = <IExpressionAnimation as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IExpressionAnimation as windows_core::Interface>::IID;
}
impl core::ops::Deref for ExpressionAnimation {
    type Target = IExpressionAnimation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ExpressionAnimation {
    const NAME: &'static str = "Microsoft.UI.Composition.ExpressionAnimation";
}
unsafe impl Send for ExpressionAnimation {}
unsafe impl Sync for ExpressionAnimation {}
windows_core::imp::define_interface!(
    ICompositionAnimation,
    ICompositionAnimation_Vtbl,
    0xa829ccc8_6fde_5b90_ad37_efd307e1b631
);
impl windows_core::RuntimeType for ICompositionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionAnimation {
    pub fn SetReferenceParameter<P1>(
        &self,
        key: &str,
        compositionobject: P1,
    ) -> windows_core::Result<()>
    where
        P1: windows_core::Param<CompositionObject>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetReferenceParameter)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(key)),
                compositionobject.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionAnimation_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    ClearAllParameters: usize,
    ClearParameter: usize,
    SetColorParameter: usize,
    SetMatrix3x2Parameter: usize,
    SetMatrix4x4Parameter: usize,
    SetQuaternionParameter: usize,
    pub SetReferenceParameter: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionAnimationFactory,
    ICompositionAnimationFactory_Vtbl,
    0x2ed278ca_4cca_5f7f_8d47_f930552a7769
);
impl windows_core::RuntimeType for ICompositionAnimationFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionAnimationFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionClip,
    ICompositionClip_Vtbl,
    0xb66b55cb_b5a5_5bee_8972_ae78233cb34c
);
impl windows_core::RuntimeType for ICompositionClip {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionClip_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionObject,
    ICompositionObject_Vtbl,
    0x0e583d49_fb5e_5481_a426_d3c41e059a5a
);
impl windows_core::RuntimeType for ICompositionObject {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionObject {
    pub fn StartAnimation<P1>(&self, propertyname: &str, animation: P1) -> windows_core::Result<()>
    where
        P1: windows_core::Param<CompositionAnimation>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).StartAnimation)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(propertyname)),
                animation.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionObject_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Compositor: usize,
    Properties: usize,
    pub StartAnimation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionObjectFactory,
    ICompositionObjectFactory_Vtbl,
    0x6133c5f9_cd3b_56b2_876f_eb849db14911
);
impl windows_core::RuntimeType for ICompositionObjectFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionObjectFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositor,
    ICompositor_Vtbl,
    0x95213c13_c4cb_57de_b267_d21ab901ae38
);
impl windows_core::RuntimeType for ICompositor {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositor {
    pub fn CreateExpressionAnimationWithExpression(
        &self,
        expression: &str,
    ) -> windows_core::Result<ExpressionAnimation> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateExpressionAnimationWithExpression)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(expression)),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ICompositor_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    CreateColorKeyFrameAnimation: usize,
    CreateColorBrush: usize,
    CreateColorBrushWithColor: usize,
    CreateContainerVisual: usize,
    CreateCubicBezierEasingFunction: usize,
    CreateEffectFactory: usize,
    CreateEffectFactoryWithProperties: usize,
    CreateExpressionAnimation: usize,
    pub CreateExpressionAnimationWithExpression: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    )
        -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositor7,
    ICompositor7_Vtbl,
    0x5358b81b_4799_5e7c_906c_e3df3b2e95eb
);
impl windows_core::RuntimeType for ICompositor7 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositor7 {
    pub fn CreateRectangleClip(&self) -> windows_core::Result<RectangleClip> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateRectangleClip)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ICompositor7_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    DispatcherQueue: usize,
    CreateAnimationPropertyInfo: usize,
    pub CreateRectangleClip: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IExpressionAnimation,
    IExpressionAnimation_Vtbl,
    0x7fff5826_1992_56c0_9060_5ade561a4f2d
);
impl windows_core::RuntimeType for IExpressionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IExpressionAnimation_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IRectangleClip,
    IRectangleClip_Vtbl,
    0xbc261502_2259_5c01_8616_556110a09657
);
impl windows_core::RuntimeType for IRectangleClip {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IRectangleClip {
    pub fn BottomLeftRadius(&self) -> windows_core::Result<windows_numerics::Vector2> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).BottomLeftRadius)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn SetBottomLeftRadius(
        &self,
        value: windows_numerics::Vector2,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetBottomLeftRadius)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn BottomRightRadius(&self) -> windows_core::Result<windows_numerics::Vector2> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).BottomRightRadius)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn SetBottomRightRadius(
        &self,
        value: windows_numerics::Vector2,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetBottomRightRadius)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn TopLeftRadius(&self) -> windows_core::Result<windows_numerics::Vector2> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TopLeftRadius)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn SetTopLeftRadius(&self, value: windows_numerics::Vector2) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTopLeftRadius)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn TopRightRadius(&self) -> windows_core::Result<windows_numerics::Vector2> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TopRightRadius)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn SetTopRightRadius(&self, value: windows_numerics::Vector2) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTopRightRadius)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IRectangleClip_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Bottom: usize,
    SetBottom: usize,
    pub BottomLeftRadius: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    pub SetBottomLeftRadius: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    pub BottomRightRadius: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    pub SetBottomRightRadius: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    Left: usize,
    SetLeft: usize,
    Right: usize,
    SetRight: usize,
    Top: usize,
    SetTop: usize,
    pub TopLeftRadius: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    pub SetTopLeftRadius: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    pub TopRightRadius: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    pub SetTopRightRadius: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RectangleClip(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    RectangleClip,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(RectangleClip, CompositionClip, CompositionObject);
impl windows_core::RuntimeType for RectangleClip {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IRectangleClip>();
}
unsafe impl windows_core::Interface for RectangleClip {
    type Vtable = <IRectangleClip as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IRectangleClip as windows_core::Interface>::IID;
}
impl core::ops::Deref for RectangleClip {
    type Target = IRectangleClip;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RectangleClip {
    const NAME: &'static str = "Microsoft.UI.Composition.RectangleClip";
}
unsafe impl Send for RectangleClip {}
unsafe impl Sync for RectangleClip {}
