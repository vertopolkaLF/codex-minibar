use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Windows longer than this are treated as weekly (or similar), not the 5h session.
const SHORT_WINDOW_MAX_MINUTES: u32 = 12 * 60;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LimitWindow {
    pub used_percent: Option<u8>,
    pub resets_at: Option<DateTime<Utc>>,
    pub duration_minutes: Option<u32>,
}

impl LimitWindow {
    pub fn remaining_percent(&self) -> Option<u8> {
        self.used_percent
            .map(|used| 100u8.saturating_sub(used.min(100)))
    }

    pub fn is_empty(&self) -> bool {
        self.used_percent.is_none() && self.resets_at.is_none()
    }

    /// True when this window is clearly longer than a 5-hour session.
    pub fn looks_like_weekly(&self, now: DateTime<Utc>) -> bool {
        if let Some(minutes) = self.duration_minutes {
            return minutes > SHORT_WINDOW_MAX_MINUTES;
        }
        self.resets_at
            .map(|reset| (reset - now).num_minutes() > i64::from(SHORT_WINDOW_MAX_MINUTES))
            .unwrap_or(false)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RateLimits {
    pub primary: LimitWindow,
    pub secondary: LimitWindow,
    pub sampled_at: DateTime<Utc>,
    pub plan_type: Option<String>,
    pub limit_name: Option<String>,
    pub credits: Credits,
}

impl RateLimits {
    /// OpenAI sometimes drops the 5h window and leaves weekly data in `primary`.
    /// Remap that so the UI/tray keep treating primary as the short session.
    pub fn normalized(mut self, now: DateTime<Utc>) -> Self {
        if self.secondary.is_empty()
            && !self.primary.is_empty()
            && self.primary.looks_like_weekly(now)
        {
            self.secondary = std::mem::take(&mut self.primary);
        }
        self
    }

    pub fn five_hour_disabled(&self) -> bool {
        self.primary.is_empty()
    }

    /// Window used by tray widgets that target the 5h/primary source.
    pub fn effective_primary(&self) -> &LimitWindow {
        if self.five_hour_disabled() {
            &self.secondary
        } else {
            &self.primary
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Credits {
    pub has_credits: bool,
    pub unlimited: bool,
    pub balance: Option<String>,
}
