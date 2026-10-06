use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Border {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    pub mounted: Option<Callback<Option<windows_core::IInspectable>>>,
    pub unmounted: Option<Callback<Option<windows_core::IInspectable>>>,
    pub corner_radius: Option<f64>,
    pub corner_radii: Option<CornerRadii>,
    pub border_brush: Option<BrushBinding>,
    pub border_thickness: Option<Thickness>,
    pub child: Box<Element>,
}
impl Border {
    pub fn new(child: impl Into<Element>) -> Self {
        Self {
            key: None,
            modifiers: Modifiers::default(),
            mounted: None,
            unmounted: None,
            corner_radius: None,
            corner_radii: None,
            border_brush: None,
            border_thickness: None,
            child: Box::new(child.into()),
        }
    }

    /// Uniform corner radius in DIPs. Maps to WinUI `Border.CornerRadius`
    /// (all four corners). Useful for card / pill / chip layouts.
    pub fn corner_radius(mut self, v: f64) -> Self {
        self.corner_radius = Some(v);
        self.corner_radii = None;
        self
    }

    /// Crop all descendants to this border's uniform rounded contour.
    /// Call after `corner_radius`; the clip follows the arranged size.
    pub fn clip_contents(mut self) -> Self {
        let radius = self.corner_radius.unwrap_or(0.0);
        self.mounted = Some(Callback::new(
            move |native: Option<windows_core::IInspectable>| {
                if let Some(native) = native {
                    let _ = install_rounded_clip(native, radius);
                }
            },
        ));
        self.unmounted = Some(Callback::new(
            |native: Option<windows_core::IInspectable>| {
                if let Some(native) = native {
                    let _ = clear_rounded_clip(native);
                }
            },
        ));
        self
    }

    /// Per-corner radii in DIPs. Maps to WinUI `Border.CornerRadius`.
    pub fn corner_radii(mut self, v: CornerRadii) -> Self {
        self.corner_radius = None;
        self.corner_radii = Some(v);
        self
    }

    /// Brush used to paint the border stroke. Maps to WinUI
    /// `Border.BorderBrush`. Pair with [`Border::border_thickness`] for
    /// the stroke to actually render. Accepts a direct [`Color`] or a
    /// [`ThemeRef`] for theme-aware strokes.
    pub fn border_brush(mut self, v: impl Into<BrushBinding>) -> Self {
        apply_widget_brush_binding(
            &mut self.border_brush,
            &mut self.modifiers,
            Prop::BorderBrush,
            v.into(),
        );
        self
    }

    /// Stroke thickness, in DIPs, on each side. Maps to WinUI
    /// `Border.BorderThickness`.
    pub fn border_thickness(mut self, v: Thickness) -> Self {
        self.border_thickness = Some(v);
        self
    }
}
impl Default for Border {
    fn default() -> Self {
        Self {
            key: None,
            modifiers: Modifiers::default(),
            mounted: None,
            unmounted: None,
            corner_radius: None,
            corner_radii: None,
            border_brush: None,
            border_thickness: None,
            child: Box::new(Element::Empty),
        }
    }
}

impl Widget for Border {
    widget_header!(ControlKind::Border);
    fn bindings(&self) -> PropBindings {
        let mut out = generated::border_bindings(self);
        if let Some(BrushBinding::Direct(br)) = &self.border_brush {
            out.push(Binding::Prop(
                Prop::BorderBrush,
                PropValue::Color(*br),
            ));
        }
        if let Some(v) = self.border_thickness {
            out.push(Binding::Prop(
                Prop::BorderThickness,
                PropValue::Thickness(v),
            ));
        }
        if let Some(v) = self.corner_radii {
            out.push(Binding::Prop(Prop::CornerRadius, PropValue::CornerRadii(v)));
        } else if let Some(v) = self.corner_radius {
            out.push(Binding::Prop(Prop::CornerRadius, PropValue::F64(v)));
        }
        out
    }
    fn children(&self) -> Children<'_> {
        Children::PositionalSingle(&self.child)
    }
    fn on_mounted_callback(&self) -> Option<&Callback<Option<windows_core::IInspectable>>> {
        self.mounted.as_ref()
    }
    fn on_unmounted_callback(&self) -> Option<&Callback<Option<windows_core::IInspectable>>> {
        self.unmounted.as_ref()
    }
}

pub fn border(child: impl Into<Element>) -> Border {
    Border::new(child)
}
