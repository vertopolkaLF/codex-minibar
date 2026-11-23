//! Display strings shared by the popup, tray, taskbar, Command Palette and
//! notifications.

pub mod currency;
pub mod money;

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
    format_cents(crate::format::money::to_cents(micros))
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
    if plan.is_empty() {
        return String::new();
    }
    if let Some(formatted) = format_google_plan_name(plan) {
        return formatted;
    }
    if has_mixed_case(plan) {
        return plan.to_owned();
    }
    sentence_case(plan)
}

fn format_google_plan_name(plan: &str) -> Option<String> {
    let has_whitespace = plan.chars().any(char::is_whitespace);
    let words: Vec<&str> = plan
        .split(|ch: char| ch.is_whitespace() || ch == '_' || (!has_whitespace && ch == '-'))
        .filter(|part| !part.is_empty())
        .collect();
    let first = *words.first()?;
    if !first.eq_ignore_ascii_case("google") && !first.eq_ignore_ascii_case("gemini") {
        return None;
    }
    Some(
        words
            .into_iter()
            .enumerate()
            .map(|(index, word)| format_google_plan_word(index, word))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

fn format_google_plan_word(index: usize, word: &str) -> String {
    let Some((start, _)) = word.char_indices().find(|(_, ch)| ch.is_alphanumeric()) else {
        return word.to_owned();
    };
    let Some((last_idx, last_ch)) = word.char_indices().rfind(|(_, ch)| ch.is_alphanumeric())
    else {
        return word.to_owned();
    };
    let end = last_idx + last_ch.len_utf8();
    let prefix = &word[..start];
    let core = &word[start..end];
    let suffix = &word[end..];

    if core.eq_ignore_ascii_case("ai") {
        return format!("{prefix}AI{suffix}");
    }
    if index > 0
        && prefix.is_empty()
        && suffix.is_empty()
        && matches!(
            word.to_ascii_lowercase().as_str(),
            "for" | "of" | "and" | "to" | "in" | "on" | "with" | "the"
        )
    {
        return word.to_ascii_lowercase();
    }
    if has_mixed_case(word) {
        return word.to_owned();
    }
    let mut chars = core.chars();
    let Some(first) = chars.next().filter(|ch| ch.is_alphabetic()) else {
        return word.to_owned();
    };
    format!(
        "{prefix}{}{}{suffix}",
        first.to_uppercase(),
        chars.as_str().to_lowercase()
    )
}

fn has_mixed_case(text: &str) -> bool {
    text.chars().any(char::is_uppercase) && text.chars().any(char::is_lowercase)
}

fn sentence_case(text: &str) -> String {
    let mut characters = text.chars();
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

    #[test]
    fn google_and_general_plan_names_format_cleanly() {
        for (input, expected) in [
            ("Google AI Pro", "Google AI Pro"),
            ("Google ai pro", "Google AI Pro"),
            ("Google Ai Pro", "Google AI Pro"),
            ("google ai pro", "Google AI Pro"),
            ("GOOGLE AI PRO", "Google AI Pro"),
            ("GOOGLE_AI_PRO", "Google AI Pro"),
            ("google-ai-pro", "Google AI Pro"),
            ("  Google AI Pro  ", "Google AI Pro"),
            ("Google AI Ultra", "Google AI Ultra"),
            ("google ai ultra", "Google AI Ultra"),
            ("GOOGLE_AI_ULTRA", "Google AI Ultra"),
            ("Google AI Plus", "Google AI Plus"),
            ("GOOGLE_AI_PLUS", "Google AI Plus"),
            ("Google AI Free", "Google AI Free"),
            ("Google AI Pro for Business", "Google AI Pro for Business"),
            ("GOOGLE_AI_PRO_FOR_BUSINESS", "Google AI Pro for Business"),
            ("Google One AI Premium", "Google One AI Premium"),
            ("google one ai premium", "Google One AI Premium"),
            ("Gemini Code Assist Standard", "Gemini Code Assist Standard"),
            (
                "GEMINI_CODE_ASSIST_FOR_INDIVIDUALS",
                "Gemini Code Assist for Individuals",
            ),
            ("PLUS", "Plus"),
            ("  pro  ", "Pro"),
            ("max 5x", "Max 5x"),
            ("SUPERGROK", "Supergrok"),
            ("Zen · 2 models", "Zen · 2 models"),
            ("Go · 2 keys", "Go · 2 keys"),
            ("Pro Max", "Pro Max"),
            ("", ""),
            ("   ", ""),
        ] {
            assert_eq!(capitalize_plan_name(input), expected, "input: {input:?}");
        }
    }
}
