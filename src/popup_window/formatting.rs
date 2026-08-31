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
        format!("expired at {time}")
    } else {
        format!("expired at {time} {}", local.format("%d.%m"))
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
        .unwrap_or_else(|| "Never".into())
}

pub(super) fn compact_activity_bars(values: &[u64], max_bars: usize) -> Vec<u64> {
    if values.len() <= max_bars || max_bars == 0 {
        return values.to_vec();
    }
    let per_bar = values.len().div_ceil(max_bars);
    values
        .chunks(per_bar)
        .map(|chunk| chunk.iter().copied().sum())
        .collect()
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
    if value >= 1_000_000.0 {
        format!("${:.1}M", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("${:.1}K", value / 1_000.0)
    } else {
        format!("${value:.2}")
    }
}

pub(super) fn credits_display_value(limits: &RateLimits) -> Option<String> {
    if limits.credits.unlimited {
        return Some("Unlimited".into());
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
        return "Unavailable".into();
    };

    let remaining_minutes = (reset - Utc::now()).num_minutes().max(0);
    let days = remaining_minutes / 1_440;
    let hours = (remaining_minutes % 1_440) / 60;
    let minutes = remaining_minutes % 60;

    if days > 0 {
        if hours > 0 {
            format!("{days}d {hours}h")
        } else {
            format!("{days}d")
        }
    } else if hours > 0 {
        if minutes > 0 {
            format!("{hours}h {minutes}m")
        } else {
            format!("{hours}h")
        }
    } else {
        format!("{minutes}m")
    }
}

pub(super) fn format_last_updated(sampled_at: DateTime<Utc>, _clock_tick: u64) -> String {
    if sampled_at.timestamp() == 0 {
        return "Waiting for first update...".into();
    }
    let seconds = (Utc::now() - sampled_at).num_seconds().max(0);
    let elapsed = match seconds {
        0..=4 => "just now".into(),
        5..=59 => format!("{seconds} seconds ago"),
        _ => format!("{} minutes ago", seconds / 60),
    };
    format!("Updated {elapsed}")
}
