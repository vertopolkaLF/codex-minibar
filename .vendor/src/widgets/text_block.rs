use super::*;

#[derive(Clone, Default, Debug, PartialEq)]
pub struct TextBlock {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    pub text: String,
    pub font_size: Option<f64>,
    pub font_weight: Option<u16>,
    pub text_wrapping: TextWrapping,
    pub text_trimming: TextTrimming,
    pub is_text_selection_enabled: bool,
}
impl TextBlock {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            text: content.into(),
            ..Self::default()
        }
    }
}

impl Widget for TextBlock {
    widget_header!(ControlKind::TextBlock);
    fn bindings(&self) -> PropBindings {
        let mut out = generated::text_block_bindings(self);
        if let Some(v) = self.font_size {
            out.push(Binding::Prop(Prop::FontSize, PropValue::F64(v)));
        }
        if let Some(v) = self.font_weight {
            out.push(Binding::Prop(Prop::FontWeight, PropValue::U16(v)));
        }
        if self.text_trimming != TextTrimming::None {
            out.push(Binding::Prop(
                Prop::TextTrimming,
                PropValue::I32(self.text_trimming.0),
            ));
        }
        out
    }
}

impl TextBlock {
    pub fn bold(mut self) -> Self {
        self.font_weight = Some(700);
        self
    }

    pub fn semibold(mut self) -> Self {
        self.font_weight = Some(600);
        self
    }

    pub fn font_weight(mut self, weight: u16) -> Self {
        self.font_weight = Some(weight);
        self
    }

    pub fn font_size(mut self, v: f64) -> Self {
        self.font_size = Some(v);
        self
    }

    pub fn wrap(mut self) -> Self {
        self.text_wrapping = TextWrapping::Wrap;
        self
    }

    pub fn text_trimming(mut self, trimming: TextTrimming) -> Self {
        self.text_trimming = trimming;
        self
    }

    /// Wrap to at most `lines`, then crop with a trailing ellipsis.
    pub fn max_lines(self, lines: i32) -> Self {
        let line_height = self.font_size.unwrap_or(14.0) * 1.4;
        self.wrap()
            .text_trimming(TextTrimming::CharacterEllipsis)
            .max_height((line_height * f64::from(lines.max(1))).ceil())
    }

    pub fn selectable(mut self) -> Self {
        self.is_text_selection_enabled = true;
        self
    }
}

pub fn text_block(content: impl Into<String>) -> TextBlock {
    TextBlock::new(content)
}

// ── Type-ramp factories (WinUI 3 spec) ──────────────────────────────────────

/// 28 px Semibold — page/section titles.
pub fn title(content: impl Into<String>) -> TextBlock {
    TextBlock::new(content).font_size(28.0).semibold()
}

/// 20 px Semibold — secondary headings.
pub fn subtitle(content: impl Into<String>) -> TextBlock {
    TextBlock::new(content).font_size(20.0).semibold()
}

/// 18 px Normal — larger body text.
pub fn body_large(content: impl Into<String>) -> TextBlock {
    TextBlock::new(content).font_size(18.0)
}

/// 14 px Semibold — emphasized body text.
pub fn body_strong(content: impl Into<String>) -> TextBlock {
    TextBlock::new(content).font_size(14.0).semibold()
}

/// 14 px Normal — standard body text.
pub fn body(content: impl Into<String>) -> TextBlock {
    TextBlock::new(content).font_size(14.0)
}

/// 12 px Normal — captions and secondary labels.
pub fn caption(content: impl Into<String>) -> TextBlock {
    TextBlock::new(content).font_size(12.0)
}
