use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct TimePicker {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    pub header: Option<String>,
    pub clock_identifier: String,
    pub minute_increment: i32,
    pub time_minutes: u16,
    pub is_enabled: bool,
    pub on_selected_time_changed: Option<Callback<TimeSpan>>,
}

impl Default for TimePicker {
    fn default() -> Self {
        Self {
            key: None,
            modifiers: Modifiers::default(),
            header: None,
            clock_identifier: "12HourClock".to_string(),
            minute_increment: 1,
            time_minutes: 0,
            is_enabled: true,
            on_selected_time_changed: None,
        }
    }
}

impl TimePicker {
    pub fn new() -> Self {
        Self {
            is_enabled: true,
            ..Default::default()
        }
    }

    pub fn header(mut self, s: impl Into<String>) -> Self {
        self.header = Some(s.into());
        self
    }

    /// Clock format: `"12HourClock"` or `"24HourClock"`.
    pub fn clock_identifier(mut self, s: impl Into<String>) -> Self {
        self.clock_identifier = s.into();
        self
    }

    pub fn minute_increment(mut self, v: i32) -> Self {
        self.minute_increment = v;
        self
    }

    /// Minutes after midnight in the local clock used by the picker.
    pub fn time_minutes(mut self, value: u16) -> Self {
        self.time_minutes = value.min(23 * 60 + 59);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.is_enabled = enabled;
        self
    }

    pub fn on_selected_time_changed(mut self, f: impl IntoCallback<TimeSpan>) -> Self {
        self.on_selected_time_changed = Some(f.into_callback());
        self
    }
}

impl Widget for TimePicker {
    widget_header!(ControlKind::TimePicker);
    fn bindings(&self) -> PropBindings {
        generated::time_picker_bindings(self)
    }
}

pub fn time_picker() -> TimePicker {
    TimePicker::new()
}
