//! Local-time buckets shared by the usage scanners, the store and the charts.

use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone, Timelike};

/// Days of usage history kept on disk, and the longest history a view asks for.
pub const RETENTION_DAYS: u16 = 365;

/// Hourly buckets every scanner keeps current: the 24-hour overview plus the
/// day before it.
pub const HOURLY_WINDOW_HOURS: i64 = 48;

/// The start of the hour containing `at`. Subtracts the elapsed part instead
/// of rebuilding a wall clock, so a repeated local hour at a DST transition
/// keeps its own UTC offset.
pub fn truncate_hour<T: TimeZone>(at: DateTime<T>) -> DateTime<T> {
    let elapsed = Duration::minutes(i64::from(at.minute()))
        + Duration::seconds(i64::from(at.second()))
        + Duration::nanoseconds(i64::from(at.nanosecond()));
    at - elapsed
}

/// The first hourly bucket of the window that ends with the hour of `now`.
pub fn hourly_window_start(now: DateTime<Local>) -> DateTime<Local> {
    truncate_hour(now) - Duration::hours(HOURLY_WINDOW_HOURS - 1)
}

/// Local midnight starting `date`, or now when that midnight does not exist
/// or is ambiguous.
pub fn start_of_local_day(date: NaiveDate) -> DateTime<Local> {
    date.and_hms_opt(0, 0, 0)
        .and_then(|naive| Local.from_local_datetime(&naive).single())
        .unwrap_or_else(Local::now)
}
