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

pub(super) fn credits_display_value(provider: ProviderKind, limits: &RateLimits) -> Option<String> {
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
    } else {
        // Codex credits are counts, while other providers may report currency.
        // Format only at the display boundary to retain the original precision.
        if provider == ProviderKind::Codex
            && let Ok(value) = balance.parse::<f64>()
            && value.is_finite()
        {
            return Some(crate::format::group_digits(&format!(
                "{:.0}",
                value.round()
            )));
        }
        Some(balance.into())
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
