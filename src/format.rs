//! Display strings shared by the popup, tray, taskbar, Command Palette and
//! notifications.

use chrono::{DateTime, Utc};

/// `1234567` → `1,234,567`; keeps a leading minus sign.
pub fn group_digits(integer: &str) -> String {
    let (sign, digits) = integer
        .strip_prefix('-')
        .map_or(("", integer), |digits| ("-", digits));
    let mut grouped = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{sign}{grouped}")
}

pub fn format_usd(value: f64) -> String {
    let amount = format!("{value:.2}");
    let Some((dollars, cents)) = amount.split_once('.') else {
        return format!("${amount}");
    };
    format!("${}.{cents}", group_digits(dollars))
}

pub fn format_cents(cents: u64) -> String {
    format!(
        "${}.{:02}",
        group_digits(&(cents / 100).to_string()),
        cents % 100
    )
}

pub fn format_micro_usd(micros: u64) -> String {
    format_cents(crate::money::to_cents(micros))
}

pub fn format_token_count(tokens: u64) -> String {
    match tokens {
        0..=999 => tokens.to_string(),
        1_000..=999_999 => format!("{:.1}K", tokens as f64 / 1_000.0),
        1_000_000..=999_999_999 => format!("{:.1}M", tokens as f64 / 1_000_000.0),
        _ => format!("{:.1}B", tokens as f64 / 1_000_000_000.0),
    }
}

pub fn capitalize_plan_name(plan: &str) -> String {
    let plan = plan.trim();
    let mut characters = plan.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };
    format!(
        "{}{}",
        first.to_uppercase(),
        characters.as_str().to_lowercase()
    )
}

/// Compact countdown to `until`: `2h 13m`, `3d 4h`, `45m`.
pub fn countdown(until: DateTime<Utc>, now: DateTime<Utc>) -> String {
    format_minutes((until - now).num_minutes().max(0))
}

/// Local wall-clock time of a reset, with the weekday once it is not today.
pub fn reset_clock(reset: DateTime<Utc>) -> String {
    use chrono::Local;
    let local = reset.with_timezone(&Local);
    let time = crate::settings::TimeFormat::current().format_hm(local);
    if local.date_naive() == Local::now().date_naive() {
        time
    } else {
        format!("{} {time}", local.format("%a"))
    }
}

pub fn format_reset_in(reset: Option<DateTime<Utc>>) -> String {
    match reset {
        Some(reset) => countdown(reset, Utc::now()),
        None => crate::i18n::tr("unavailable").into(),
    }
}

/// "2 h 5 m ago" for a moment in the past.
pub fn format_time_ago(at: DateTime<Utc>) -> String {
    crate::i18n::format("time-ago", &[("time", countdown(Utc::now(), at))])
}

/// "2 h 5 m late" for a deadline in the past.
pub fn format_time_late(deadline: DateTime<Utc>) -> String {
    crate::i18n::format("time-late", &[("time", countdown(Utc::now(), deadline))])
}

fn format_minutes(total_minutes: i64) -> String {
    let days = total_minutes / 1_440;
    let hours = (total_minutes % 1_440) / 60;
    let minutes = total_minutes % 60;

    if days > 0 {
        if hours > 0 {
            crate::i18n::format(
                "days-d-hours-h",
                &[("days", days.to_string()), ("hours", hours.to_string())],
            )
        } else {
            crate::i18n::format("days-d", &[("days", days.to_string())])
        }
    } else if hours > 0 {
        if minutes > 0 {
            crate::i18n::format(
                "hours-h-minutes-m",
                &[
                    ("hours", hours.to_string()),
                    ("minutes", minutes.to_string()),
                ],
            )
        } else {
            crate::i18n::format("hours-h", &[("hours", hours.to_string())])
        }
    } else {
        crate::i18n::format("minutes-m", &[("minutes", minutes.to_string())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_dollar_amounts_remain_distinguishable_at_compact_boundaries() {
        for (value, expected) in [
            (0.0, "$0.00"),
            (1.0, "$1.00"),
            (999.99, "$999.99"),
            (1000.01, "$1,000.01"),
            (1284.0, "$1,284.00"),
            (1299.0, "$1,299.00"),
            (12345678.9, "$12,345,678.90"),
            (-1284.5, "$-1,284.50"),
        ] {
            assert_eq!(format_usd(value), expected);
        }
    }

    #[test]
    fn microdollars_keep_cents_through_rounding_and_u64_boundaries() {
        for (value, expected) in [
            (0, "$0.00"),
            (4_999, "$0.00"),
            (5_000, "$0.01"),
            (995_000, "$1.00"),
            (999_995_000, "$1,000.00"),
            (1_284_000_000, "$1,284.00"),
            (1_299_000_000, "$1,299.00"),
            (u64::MAX, "$18,446,744,073,709.55"),
        ] {
            assert_eq!(format_micro_usd(value), expected);
        }
    }

    #[test]
    fn countdown_is_compact() {
        let now = Utc::now();
        assert_eq!(countdown(now + chrono::Duration::minutes(45), now), "45m");
        assert_eq!(
            countdown(now + chrono::Duration::minutes(133), now),
            "2h 13m"
        );
        assert_eq!(
            countdown(now + chrono::Duration::minutes(3 * 1440 + 240), now),
            "3d 4h"
        );
    }
}
