//! Pure conversion helpers between reactor's backend-facing types and
//! the underlying `Microsoft.UI.Xaml` types. These are free functions
//! that touch neither `WinUIBackend` state nor the `Handle` enum — see
//! `winui/backend/mod.rs` for the dispatch tables and `Handle`-aware
//! helpers.

use super::*;

windows_core::imp::define_interface!(
    IXamlReaderStatics,
    IXamlReaderStatics_Vtbl,
    0x82a4cd9e_435e_5aeb_8c4f_300cece45cae
);
impl windows_core::RuntimeType for IXamlReaderStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}

#[repr(C)]
pub struct IXamlReaderStatics_Vtbl {
    base__: windows_core::IInspectable_Vtbl,
    load: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}

struct XamlReader;
impl windows_core::RuntimeName for XamlReader {
    const NAME: &'static str = "Microsoft.UI.Xaml.Markup.XamlReader";
}

fn svg_icon(path: &str, color: &str) -> Result<bindings::IconElement> {
    let xaml = format!(
        r#"<PathIcon xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Data="{path}" Foreground="{color}" />"#
    );
    let factory: IXamlReaderStatics = {
        static SHARED: windows_core::imp::FactoryCache<XamlReader, IXamlReaderStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(|factory| Ok(factory.clone()))?
    };
    unsafe {
        let mut result = core::ptr::null_mut();
        (windows_core::Interface::vtable(&factory).load)(
            windows_core::Interface::as_raw(&factory),
            core::mem::transmute_copy(&windows_core::HSTRING::from(xaml)),
            &mut result,
        )
        .and_then(|| windows_core::Type::from_abi(result))
    }
}

pub(super) fn to_xaml_gridlength(v: GridLength) -> Result<bindings::GridLength> {
    use bindings::GridUnitType;
    match v {
        GridLength::Auto => Ok(bindings::GridLength {
            value: 0.0,
            grid_unit_type: GridUnitType::Auto,
        }),
        GridLength::Pixel(v) => Ok(bindings::GridLength {
            value: v,
            grid_unit_type: GridUnitType::Pixel,
        }),
        GridLength::Star(v) => Ok(bindings::GridLength {
            value: v,
            grid_unit_type: GridUnitType::Star,
        }),
    }
}

pub(super) fn solid_brush(c: Color) -> Result<bindings::SolidColorBrush> {
    let brush = bindings::SolidColorBrush::new()?;
    brush.SetColor(c)?;
    Ok(brush)
}

pub(super) fn string_as_textblock(s: &str) -> Result<bindings::TextBlock> {
    let tb = bindings::TextBlock::new()?;
    tb.SetText(s)?;
    Ok(tb)
}

pub(super) fn build_nav_view_item(item: &NavViewItem) -> Result<windows_core::IInspectable> {
    if item.is_header {
        let h = bindings::NavigationViewItemHeader::new()?;
        let tb = string_as_textblock(&item.content)?;
        h.cast::<bindings::IContentControl>()?.SetContent(&tb)?;
        return h.cast();
    }
    let nv_item = bindings::NavigationViewItem::new()?;
    let tb = string_as_textblock(&item.content)?;
    nv_item
        .cast::<bindings::IContentControl>()?
        .SetContent(&tb)?;
    let tag = item.tag.clone().unwrap_or_else(|| item.content.clone());
    let tag_inspectable = windows_reference::IReference::from(tag.as_str());
    nv_item
        .cast::<bindings::IFrameworkElement>()?
        .SetTag(&tag_inspectable)?;
    if let Some((path, color)) = &item.icon_path {
        let icon_elem = svg_icon(path, color)?;
        nv_item.SetIcon(&icon_elem)?;
    }
    if !item.children.is_empty() {
        let menu = nv_item
            .cast::<bindings::INavigationViewItem2>()?
            .MenuItems()?;
        for child in &item.children {
            let child_obj = build_nav_view_item(child)?;
            menu.Append(&child_obj)?;
        }
    }
    nv_item.cast()
}

fn nav_item_tag(item: &bindings::NavigationViewItem) -> Option<String> {
    item.cast::<bindings::IFrameworkElement>()
        .ok()?
        .Tag()
        .ok()?
        .cast::<windows_reference::IReference<windows_core::HSTRING>>()
        .ok()?
        .Value()
        .ok()
        .map(|s| s.to_string_lossy())
}

pub(super) fn select_nav_item_by_tag(nv: &bindings::NavigationView, tag: &str) -> Result<()> {
    let menu = nv.MenuItems()?;

    for obj in &menu {
        let Ok(item) = obj.cast::<bindings::NavigationViewItem>() else {
            continue;
        };
        if nav_item_tag(&item).as_deref() == Some(tag) {
            let inspectable: windows_core::IInspectable = item.cast()?;
            return nv.SetSelectedItem(&inspectable);
        }
        if let Ok(children) = item.cast::<bindings::INavigationViewItem2>()?.MenuItems() {
            for child_obj in &children {
                let Ok(child) = child_obj.cast::<bindings::NavigationViewItem>() else {
                    continue;
                };
                if nav_item_tag(&child).as_deref() == Some(tag) {
                    let inspectable: windows_core::IInspectable = child.cast()?;
                    return nv.SetSelectedItem(&inspectable);
                }
            }
        }
    }
    Ok(())
}

/// Build a `MenuFlyoutItemBase` from a [`MenuItemDef`].
pub(super) fn build_menu_flyout_item_base(
    def: &MenuItemDef,
) -> Result<bindings::MenuFlyoutItemBase> {
    match def {
        MenuItemDef::Item { text } => {
            let item = bindings::MenuFlyoutItem::new()?;
            item.SetText(text)?;
            item.cast()
        }
        MenuItemDef::Separator => {
            let sep = bindings::MenuFlyoutSeparator::new()?;
            sep.cast()
        }
        MenuItemDef::SubItem { text, children } => {
            let sub = bindings::MenuFlyoutSubItem::new()?;
            sub.SetText(text)?;
            let sub_items = sub.Items()?;
            for child in children {
                let child_item = build_menu_flyout_item_base(child)?;
                sub_items.Append(&child_item)?;
            }
            sub.cast()
        }
    }
}

/// Recursively build a `TreeViewNode` from a [`TreeNodeDef`].
pub(super) fn build_tree_view_node(def: &TreeNodeDef) -> Result<bindings::TreeViewNode> {
    let node = bindings::TreeViewNode::new()?;
    let content: windows_core::IInspectable =
        windows_reference::IReference::<windows_core::HSTRING>::from(windows_core::HSTRING::from(
            &def.text,
        ))
        .cast()?;
    node.SetContent(&content)?;
    node.SetIsExpanded(def.is_expanded)?;
    if !def.children.is_empty() {
        let children = node.Children()?;
        for child_def in &def.children {
            let child_node = build_tree_view_node(child_def)?;
            children.Append(&child_node)?;
        }
    }
    Ok(node)
}

/// Builds a WinUI `ICommandBarElement` from a [`CommandBarCommandDef`].
pub(super) fn build_command_bar_element(
    def: &CommandBarCommandDef,
) -> Result<bindings::ICommandBarElement> {
    match def {
        CommandBarCommandDef::Button { label, icon } => {
            let btn = bindings::AppBarButton::new()?;
            btn.SetLabel(label)?;
            if let Some(sym) = icon {
                let icon_elem = bindings::SymbolIcon::CreateInstanceWithSymbol(*sym)?;
                btn.SetIcon(&icon_elem)?;
            }
            btn.cast()
        }
        CommandBarCommandDef::Toggle { label, icon } => {
            let btn = bindings::AppBarToggleButton::new()?;
            btn.SetLabel(label)?;
            if let Some(sym) = icon {
                let icon_elem = bindings::SymbolIcon::CreateInstanceWithSymbol(*sym)?;
                btn.SetIcon(&icon_elem)?;
            }
            btn.cast()
        }
        CommandBarCommandDef::Separator => {
            let sep = bindings::AppBarSeparator::new()?;
            sep.cast()
        }
    }
}
