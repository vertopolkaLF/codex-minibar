use super::*;
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Default, Debug, PartialEq)]
pub struct StackPanel {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    pub mounted: Option<Callback<Option<windows_core::IInspectable>>>,
    pub unmounted: Option<Callback<Option<windows_core::IInspectable>>>,
    pub orientation: Orientation,
    pub spacing: f64,
    pub children: Vec<Element>,
}

impl StackPanel {
    /// Invoke `f` whenever WinUI lays out this panel, with its size in DIPs.
    pub fn on_resize(mut self, f: impl Fn(f64, f64) + 'static) -> Self {
        let f = Rc::new(f);
        let previous = self.mounted.take();
        self.mounted = Some(Callback::new(move |native: Option<windows_core::IInspectable>| {
            if let Some(ref callback) = previous {
                callback.invoke(native.clone());
            }
            let Some(native) = native else {
                return;
            };
            let Ok(element) = native.cast::<bindings::IFrameworkElement>() else {
                return;
            };
            let callback = f.clone();
            let measurement_element = native.clone();
            let revoker: Rc<RefCell<Option<windows_core::EventRevoker>>> =
                Rc::new(RefCell::new(None));
            if let Ok(revoker_value) = element.SizeChanged(move |_sender, args| {
                if let Some(args) = args.as_ref()
                    && let Ok(size) = args.NewSize()
                {
                    let desired = measurement_element
                        .cast::<bindings::IUIElement>()
                        .and_then(|element| element.DesiredSize())
                        .unwrap_or(size);
                    callback(desired.width as f64, desired.height as f64);
                }
            }) {
                *revoker.borrow_mut() = Some(revoker_value);
                // Keep the subscription alive for the native element's lifetime.
                std::mem::forget(revoker);
            }
        }));
        self
    }
}
impl StackPanel {
    pub fn vertical() -> Self {
        Self {
            orientation: Orientation::Vertical,
            ..Self::default()
        }
    }
    pub fn horizontal() -> Self {
        Self {
            orientation: Orientation::Horizontal,
            ..Self::default()
        }
    }
}

impl Widget for StackPanel {
    widget_header!(ControlKind::StackPanel);
    fn bindings(&self) -> PropBindings {
        generated::stack_panel_bindings(self)
    }
    fn children(&self) -> Children<'_> {
        Children::Keyed(&self.children)
    }
    fn on_mounted_callback(&self) -> Option<&Callback<Option<windows_core::IInspectable>>> {
        self.mounted.as_ref()
    }
    fn on_unmounted_callback(&self) -> Option<&Callback<Option<windows_core::IInspectable>>> {
        self.unmounted.as_ref()
    }
}

impl StackPanel {
    pub fn spacing(mut self, v: f64) -> Self {
        self.spacing = v;
        self
    }
}

pub fn vstack(children: impl IntoElements) -> StackPanel {
    let mut s = StackPanel::vertical();
    s.children = children.into_elements();
    s
}

pub fn hstack(children: impl IntoElements) -> StackPanel {
    let mut s = StackPanel::horizontal();
    s.children = children.into_elements();
    s
}
