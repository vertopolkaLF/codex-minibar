use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LimitWindow {
    pub used_percent: Option<u8>,
    pub resets_at: Option<DateTime<Utc>>,
}

impl LimitWindow {
    pub fn remaining_percent(&self) -> Option<u8> {
        self.used_percent
            .map(|used| 100u8.saturating_sub(used.min(100)))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RateLimits {
    pub primary: LimitWindow,
    pub secondary: LimitWindow,
    pub sampled_at: DateTime<Utc>,
}
