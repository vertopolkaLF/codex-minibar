use super::*;

pub(super) fn format_activation_at(at: DateTime<Utc>) -> String {
    let local = at.with_timezone(&Local);
    format!(
        "{} {}",
        TimeFormat::current().format_hms(local),
        local.format("%d.%m.%Y")
    )
}

pub(super) fn format_expired_at(at: DateTime<Utc>) -> String {
    let local = at.with_timezone(&Local);
    let time = TimeFormat::current().format_hm(local);
    if local.date_naive() == Local::now().date_naive() {
        crate::i18n::format("expired-at-time", &[("time", time.to_string())])
    } else {
        crate::i18n::format(
            "expired-at-time-6c39ff",
            &[
                ("time", time.to_string()),
                ("v0", (local.format("%d.%m")).to_string()),
            ],
        )
    }
}

/// Start of the current 5h window: resets_at minus duration.
pub(super) fn window_started_at(window: &LimitWindow) -> Option<DateTime<Utc>> {
    match (window.resets_at, window.duration_minutes) {
        (Some(reset), Some(minutes)) => Some(reset - ChronoDuration::minutes(i64::from(minutes))),
        _ => None,
    }
}

pub(super) fn format_last_activation(
    limits: &RateLimits,
    fallback_attempt: Option<DateTime<Utc>>,
) -> String {
    window_started_at(&limits.primary)
        .or(fallback_attempt)
        .map(format_activation_at)
        .unwrap_or_else(|| crate::i18n::tr("never").into())
}

pub(super) fn format_token_count(tokens: u64) -> String {
    match tokens {
        0..=999 => tokens.to_string(),
        1_000..=999_999 => format!("{:.1}K", tokens as f64 / 1_000.0),
        1_000_000..=999_999_999 => format!("{:.1}M", tokens as f64 / 1_000_000.0),
        _ => format!("{:.1}B", tokens as f64 / 1_000_000_000.0),
    }
}

pub(super) fn format_usd(value: f64) -> String {
    let amount = format!("{value:.2}");
    let Some((dollars, cents)) = amount.split_once('.') else {
        return format!("${amount}");
    };
    let (sign, digits) = dollars
        .strip_prefix('-')
        .map_or(("", dollars), |digits| ("-", digits));
    let mut grouped = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("${sign}{grouped}.{cents}")
}

pub(super) fn credits_display_value(limits: &RateLimits) -> Option<String> {
    if limits.credits.unlimited {
        return Some(crate::i18n::tr("unlimited").into());
    }
    if !limits.credits.has_credits {
        return None;
    }

    let balance = limits.credits.balance.as_deref()?.trim();
    if balance.is_empty()
        || matches!(
            balance.to_ascii_lowercase().as_str(),
            "none" | "undefined" | "null" | "n/a" | "unavailable"
        )
    {
        None
    } else if limits.credits.has_credits {
        Some(balance.into())
    } else {
        None
    }
}

pub(super) fn capitalize_plan_name(plan: &str) -> String {
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

pub(super) fn format_reset_in(reset: Option<DateTime<Utc>>) -> String {
    let Some(reset) = reset else {
        return crate::i18n::tr("unavailable").into();
    };

    let remaining_minutes = (reset - Utc::now()).num_minutes().max(0);
    let days = remaining_minutes / 1_440;
    let hours = (remaining_minutes % 1_440) / 60;
    let minutes = remaining_minutes % 60;

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

pub(super) fn format_last_updated(sampled_at: DateTime<Utc>, _clock_tick: u64) -> String {
    if sampled_at.timestamp() == 0 {
        return crate::i18n::tr("waiting-for-first-update").into();
    }
    let seconds = (Utc::now() - sampled_at).num_seconds().max(0);
    let elapsed = match seconds {
        0..=4 => crate::i18n::tr("just-now").into(),
        5..=59 => crate::i18n::format("seconds-seconds-ago", &[("seconds", seconds.to_string())]),
        _ => crate::i18n::format("minutes-ago", &[("v0", (seconds / 60).to_string())]),
    };
    crate::i18n::format("updated-elapsed", &[("elapsed", elapsed.to_string())])
}

#[cfg(test)]
mod money_tests {
    use super::format_usd;

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
}
