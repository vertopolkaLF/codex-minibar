use super::*;
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Debug, PartialEq)]
pub struct ScrollViewer {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    pub child: Box<Element>,
    pub horizontal_scroll_bar_visibility: ScrollBarVisibility,
    pub vertical_scroll_bar_visibility: ScrollBarVisibility,
}
impl Default for ScrollViewer {
    fn default() -> Self {
        Self {
            key: None,
            modifiers: Modifiers::default(),
            child: Box::new(Element::Empty),
            horizontal_scroll_bar_visibility: ScrollBarVisibility::Disabled,
            vertical_scroll_bar_visibility: ScrollBarVisibility::Auto,
        }
    }
}
impl ScrollViewer {
    pub fn new(child: impl Into<Element>) -> Self {
        Self {
            child: Box::new(child.into()),
            ..Default::default()
        }
    }
}

impl Widget for ScrollViewer {
    widget_header!(ControlKind::ScrollViewer);
    fn bindings(&self) -> PropBindings {
        generated::scroll_viewer_bindings(self)
    }
    fn children(&self) -> Children<'_> {
        // A popup can replace its measured body when providers or an error
        // change. Preserve this native ScrollViewer, but honor the body's key
        // so the old WinUI child is unmounted before the new one is inserted.
        Children::Keyed(std::slice::from_ref(&*self.child))
    }
}

impl ScrollViewer {
    pub fn horizontal_scroll_bar_visibility(mut self, v: ScrollBarVisibility) -> Self {
        self.horizontal_scroll_bar_visibility = v;
        self
    }

    pub fn vertical_scroll_bar_visibility(mut self, v: ScrollBarVisibility) -> Self {
        self.vertical_scroll_bar_visibility = v;
        self
    }
}

pub fn scroll_viewer(child: impl Into<Element>) -> ScrollViewer {
    ScrollViewer::new(child)
}

/// Horizontal tab strip that clips to its grid slot.
///
/// WinUI `Grid` does not clip overflow, so a star-column child that measures
/// to content width paints under the Auto actions column. This locks the
/// scroller to the arranged slot width. Wheel panning is applied by the
/// caller via `margin` + `on_pointer_wheel` — `ScrollViewer` does not map a
/// vertical wheel onto a horizontal offset.
pub fn horizontal_wheel_strip(
    strip: StackPanel,
    height: f64,
    key: impl Into<String>,
    on_wheel: impl IntoCallback<PointerEventInfo>,
) -> Element {
    let key = key.into();
    let on_wheel = on_wheel.into_callback();
    let strip = strip.on_pointer_wheel(on_wheel.clone());
    let scroller: Element = scroll_viewer(strip)
        .horizontal_scroll_bar_visibility(ScrollBarVisibility::Hidden)
        .vertical_scroll_bar_visibility(ScrollBarVisibility::Disabled)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .vertical_alignment(VerticalAlignment::Center)
        .height(height)
        .with_key(format!("{key}-scroller"))
        .into();
    clip_to_arranged_width(
        vstack((scroller,))
            .spacing(0.0)
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .vertical_alignment(VerticalAlignment::Center)
            .height(height),
    )
    .on_pointer_wheel(on_wheel)
    .with_key(format!("{key}-slot"))
    .into()
}

fn clip_to_arranged_width(mut slot: StackPanel) -> StackPanel {
    let previous_mounted = slot.mounted.take();
    let previous_unmounted = slot.unmounted.take();
    let revoker_slot: Rc<RefCell<Option<windows_core::EventRevoker>>> =
        Rc::new(RefCell::new(None));
    let revoker_for_mount = Rc::clone(&revoker_slot);
    slot.mounted = Some(Callback::new(move |native: Option<windows_core::IInspectable>| {
        if let Some(ref callback) = previous_mounted {
            callback.invoke(native.clone());
        }
        let Some(native) = native else {
            return;
        };
        let Ok(element) = native.cast::<bindings::IFrameworkElement>() else {
            return;
        };
        if let Ok(revoker) = element.SizeChanged(move |sender, args| {
            let Some(args) = args.as_ref() else {
                return;
            };
            let Ok(size) = args.NewSize() else {
                return;
            };
            if !(size.width.is_finite() && size.width > 0.0) {
                return;
            }
            let width = f64::from(size.width);
            let Some(sender) = sender.as_ref() else {
                return;
            };
            let Ok(panel) = sender.cast::<bindings::IPanel>() else {
                return;
            };
            let Ok(children) = panel.Children() else {
                return;
            };
            let Ok(child) = children.GetAt(0) else {
                return;
            };
            if let Ok(fe) = child.cast::<bindings::IFrameworkElement>() {
                let _ = fe.SetMaxWidth(width);
                let _ = fe.SetWidth(width);
            }
        }) {
            *revoker_for_mount.borrow_mut() = Some(revoker);
        }
    }));
    let revoker_for_unmount = Rc::clone(&revoker_slot);
    slot.unmounted = Some(Callback::new(move |native: Option<windows_core::IInspectable>| {
        *revoker_for_unmount.borrow_mut() = None;
        if let Some(ref callback) = previous_unmounted {
            callback.invoke(native);
        }
    }));
    slot
}
