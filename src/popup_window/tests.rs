use std::collections::HashSet;

use chrono::TimeZone;

use super::model::*;
use super::*;
use crate::limits::OpenRouterAccountSnapshot;
use crate::settings::{PopupSurface, PopupVisibility};

fn id(kind: ProviderKind) -> ProviderId {
    ProviderId::primary(kind)
}

/// The popup state the bridge would publish for `settings`.
fn ui_from(settings: &Settings) -> UiState {
    let mut ui = UiState::popup_layout_from_settings(settings);
    ui.apply_settings(settings);
    ui
}

fn plan_limits(plan_type: &str) -> RateLimits {
    RateLimits {
        plan_type: Some(plan_type.into()),
        primary: LimitWindow {
            used_percent: Some(20),
            ..Default::default()
        },
        secondary: LimitWindow {
            used_percent: Some(40),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[allow(clippy::too_many_arguments)]
fn test_cards<'a>(
    provider: ProviderKind,
    is_first: bool,
    limits: &'a RateLimits,
    visibility: &PopupVisibility,
    surface: PopupSurface,
    show_provider_tabs: bool,
    include_usage_stats: bool,
    show_account_name: bool,
) -> Vec<Card<'a>> {
    provider_cards(
        provider.into(),
        is_first,
        false,
        limits,
        &[],
        &CardOptions {
            popup_visibility: visibility,
            surface,
            show_provider_tabs,
            include_usage_stats,
            show_account_name,
            drag_handle: false,
            openrouter_actions: false,
            provider_error: None,
            now: Utc::now(),
        },
    )
}

fn all_visible() -> PopupVisibility {
    PopupVisibility::build_defaults()
}

fn visibility_with(brick_id: &str, all_tab: bool, provider_tab: bool) -> PopupVisibility {
    let mut visibility = PopupVisibility::build_defaults();
    visibility.set_brick(brick_id, all_tab, provider_tab);
    visibility
}

fn assert_unique_section_keys(sections: &[PopupSection]) {
    let keys: HashSet<_> = sections.iter().map(|section| section.key()).collect();
    assert_eq!(
        keys.len(),
        sections.len(),
        "popup sections must not duplicate"
    );
}

#[test]
fn last_activation_uses_window_start() {
    let primary = LimitWindow {
        used_percent: Some(1),
        resets_at: Some(chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 7, 10, 16, 8, 0).unwrap()),
        duration_minutes: Some(300),
    };
    assert_eq!(
        window_started_at(&primary),
        Some(chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 7, 10, 11, 8, 0).unwrap())
    );
    assert_eq!(
        format_last_activation(&RateLimits::default(), None),
        "Never"
    );
}

#[test]
fn expired_at_includes_date_only_when_not_today() {
    let today = Local::now().date_naive();
    let today_at = Local
        .from_local_datetime(&today.and_hms_opt(7, 8, 0).unwrap())
        .single()
        .unwrap();
    let yesterday_at = today_at - ChronoDuration::days(1);
    let time_format = TimeFormat::current();

    assert_eq!(
        format_expired_at(today_at.with_timezone(&Utc)),
        format!("expired at {}", time_format.format_hm(today_at))
    );
    assert_eq!(
        format_expired_at(yesterday_at.with_timezone(&Utc)),
        format!(
            "expired at {} {}",
            time_format.format_hm(yesterday_at),
            yesterday_at.format("%d.%m")
        )
    );
}

#[test]
fn unavailable_sample_has_clear_copy() {
    assert_eq!(
        format_last_updated(DateTime::default(), 0),
        "Waiting for first update"
    );
    assert_eq!(format_reset_in(None), "Unavailable");
}

#[test]
fn popup_refresh_is_sent_to_every_provider_worker() {
    let (codex_tx, codex_rx) = std::sync::mpsc::channel();
    let (claude_tx, claude_rx) = std::sync::mpsc::channel();
    let (cursor_tx, cursor_rx) = std::sync::mpsc::channel();
    let commands = vec![
        (id(ProviderKind::Codex), codex_tx),
        (id(ProviderKind::Claude), claude_tx),
        (id(ProviderKind::Cursor), cursor_tx),
    ];

    assert!(refresh_all_workers(&commands));
    assert_eq!(codex_rx.try_recv(), Ok(WorkerCommand::Refresh));
    assert_eq!(claude_rx.try_recv(), Ok(WorkerCommand::Refresh));
    assert_eq!(cursor_rx.try_recv(), Ok(WorkerCommand::Refresh));
}

#[test]
fn combined_spend_uses_usage_tab_windows() {
    let today = Local::now().date_naive();
    assert_eq!(
        crate::usage_overview::dates_for_total_spend(TotalSpendPeriod::Today),
        (today, today)
    );
    assert_eq!(
        crate::usage_overview::dates_for_total_spend(TotalSpendPeriod::Yesterday),
        (
            today - ChronoDuration::days(1),
            today - ChronoDuration::days(1)
        )
    );
    assert_eq!(
        crate::usage_overview::dates_for_total_spend(TotalSpendPeriod::ThirtyDays),
        (
            today
                - ChronoDuration::days(i64::from(
                    crate::usage_overview::OverviewRange::ThirtyDays
                        .days()
                        .saturating_sub(1)
                )),
            today
        )
    );
    assert_eq!(format_usd(1.25), "$1.25");
}

#[test]
fn spend_donut_segments_cover_the_ring_with_gaps() {
    let segments = ui::donut_segments(
        &[
            (id(ProviderKind::Cursor), 2_000_000),
            (id(ProviderKind::Claude), 1_000_000),
            (id(ProviderKind::Codex), 500_000),
        ],
        3_500_000,
    );
    assert_eq!(segments.len(), 3);
    assert_eq!(segments[0].0, Some(id(ProviderKind::Cursor)));
    // Each slice keeps half of a 2° gap on both ends.
    assert!((segments[0].1 - -89.0).abs() < 1e-3);
    let sweep: f32 = segments.iter().map(|(_, start, end)| end - start).sum();
    assert!((sweep - (360.0 - 6.0)).abs() < 1e-2);
    assert_eq!(ui::donut_segments(&[], 0), vec![(None, -90.0, 270.0)]);
    assert_eq!(
        ui::donut_segments(&[(id(ProviderKind::Codex), 5)], 5),
        vec![(Some(id(ProviderKind::Codex)), -90.0, 270.0)]
    );
}

#[test]
fn usage_statistics_section_respects_its_live_toggle() {
    let limits = RateLimits {
        usage: crate::usage::UsageStatistics {
            history: crate::usage::TokenUsage {
                requests: 1,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };

    assert!(
        popup_sections(ProviderKind::OpenCodeZen, &limits, false)
            .contains(&PopupSection::UsageStatistics)
    );
    let excluded_from_home = all_visible();
    let home_cards = test_cards(
        ProviderKind::OpenCodeZen,
        true,
        &limits,
        &excluded_from_home,
        PopupSurface::HomeTab,
        true,
        false,
        false,
    );
    assert_eq!(home_cards.len(), 1);

    let provider_page_cards = test_cards(
        ProviderKind::OpenCodeZen,
        true,
        &limits,
        &excluded_from_home,
        PopupSurface::ProviderTab,
        true,
        true,
        false,
    );
    assert_eq!(provider_page_cards.len(), 2);

    let mut hidden_usage = all_visible();
    hidden_usage.set_brick("opencode.usage", false, false);
    let cards = test_cards(
        ProviderKind::OpenCodeZen,
        true,
        &limits,
        &hidden_usage,
        PopupSurface::ProviderTab,
        true,
        true,
        false,
    );
    assert_eq!(cards.len(), 1);
}

#[test]
fn total_spend_counts_enabled_instances_with_usage_statistics() {
    let mut settings = Settings::default();
    settings.set_enabled(id(ProviderKind::Codex), true);
    settings.set_enabled(id(ProviderKind::Claude), true);
    let ui = ui_from(&settings);
    assert_eq!(spend_providers(&ui).len(), 2);
    assert!(show_total_spend(&ui));
    settings.set_usage_stats_provider_enabled(id(ProviderKind::Codex), false);
    let ui = ui_from(&settings);
    assert_eq!(spend_providers(&ui), vec![id(ProviderKind::Claude)]);
    assert!(!show_total_spend(&ui));
    // A second Claude instance counts on its own.
    let work = settings.add_instance(ProviderInstance::new(ProviderKind::Claude, "Work"));
    let ui = ui_from(&settings);
    assert_eq!(spend_providers(&ui), vec![id(ProviderKind::Claude), work]);
}

#[test]
fn exhausted_limit_is_compact_in_both_percentage_modes() {
    let exhausted = LimitWindow {
        used_percent: Some(100),
        ..Default::default()
    };

    let (used_label, used_progress, _, used_compact) =
        limit_card_presentation(&exhausted, true, false);
    assert_eq!(used_label, "100% used");
    assert_eq!(used_progress, 100.0);
    assert!(used_compact);

    let (left_label, left_progress, _, left_compact) =
        limit_card_presentation(&exhausted, false, false);
    assert_eq!(left_label, "0% left");
    assert_eq!(left_progress, 0.0);
    assert!(left_compact);

    let fresh = LimitWindow {
        used_percent: Some(0),
        ..Default::default()
    };
    let (_, _, _, fresh_compact) = limit_card_presentation(&fresh, false, false);
    assert!(!fresh_compact);
}

#[test]
fn popup_visibility_hides_codex_resets_on_all_but_shows_on_provider_tab() {
    let mut limits = plan_limits("plus");
    limits.reset_credits = Some(crate::limits::RateLimitResetCreditsSummary {
        available_count: 1,
        ..Default::default()
    });
    let visibility = visibility_with("codex.resets", false, true);
    let all_cards = test_cards(
        ProviderKind::Codex,
        true,
        &limits,
        &visibility,
        PopupSurface::HomeTab,
        true,
        true,
        false,
    );
    let tab_cards = test_cards(
        ProviderKind::Codex,
        true,
        &limits,
        &visibility,
        PopupSurface::ProviderTab,
        true,
        true,
        false,
    );
    assert_eq!(all_cards.len(), 3);
    assert_eq!(tab_cards.len(), 4);
}

#[test]
fn popup_visibility_union_applies_when_provider_tabs_are_hidden() {
    let visibility = visibility_with("codex.usage", false, true);
    assert!(visibility.is_visible("codex.usage", PopupSurface::HomeTab, false));
}

#[test]
fn show_on_home_off_drops_only_that_instance_from_home() {
    let mut settings = Settings::default();
    settings.set_enabled(id(ProviderKind::Codex), true);
    settings.set_enabled(id(ProviderKind::Cursor), true);
    settings
        .instance_mut(id(ProviderKind::Codex))
        .unwrap()
        .show_on_home = false;
    let ui = ui_from(&settings);
    let widgets = visible_home_widgets(&ui, false);
    assert!(!widgets.contains(&HomeWidgetId::provider(id(ProviderKind::Codex))));
    assert!(widgets.contains(&HomeWidgetId::provider(id(ProviderKind::Cursor))));
    // Its tab stays.
    assert!(provider_tabs(&ui).contains(&PopupView::Provider(id(ProviderKind::Codex))));
    let limits = plan_limits("plus");
    let tab_cards = test_cards(
        ProviderKind::Codex,
        true,
        &limits,
        &ui.popup_visibility,
        PopupSurface::ProviderTab,
        true,
        true,
        false,
    );
    assert!(!tab_cards.is_empty());
}

#[test]
fn format_reset_in_future_duration() {
    assert_eq!(
        format_reset_in(Some(
            Utc::now() + ChronoDuration::days(2) + ChronoDuration::minutes(1),
        )),
        "2d"
    );
}

#[test]
fn free_to_plus_replaces_monthly_with_session_and_weekly_sections() {
    let free = popup_sections(ProviderKind::Codex, &plan_limits("free"), false);
    assert_eq!(free, vec![PopupSection::Monthly]);
    assert_unique_section_keys(&free);

    let plus = popup_sections(ProviderKind::Codex, &plan_limits("plus"), false);
    assert_eq!(plus, vec![PopupSection::FiveHour, PopupSection::Weekly,]);
    assert_unique_section_keys(&plus);
}

#[test]
fn disabled_five_hour_session_is_omitted_from_popup() {
    let mut limits = plan_limits("plus");
    limits.primary = LimitWindow::default();

    let sections = popup_sections(ProviderKind::Codex, &limits, false);
    assert_eq!(sections, vec![PopupSection::Weekly]);
    assert_unique_section_keys(&sections);
}

#[test]
fn zen_without_quota_windows_does_not_render_placeholder_limit_cards() {
    let limits = RateLimits {
        plan_type: Some("Zen · 2 models".into()),
        usage: crate::usage::UsageStatistics {
            history: crate::usage::TokenUsage {
                requests: 1,
                estimated_cost_microusd: 1_250_000,
                priced_requests: 1,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        popup_sections(ProviderKind::OpenCodeZen, &limits, false),
        vec![PopupSection::UsageStatistics]
    );
}

#[test]
fn plan_names_use_sentence_case() {
    assert_eq!(capitalize_plan_name("PLUS"), "Plus");
    assert_eq!(capitalize_plan_name("  pro  "), "Pro");
}

#[test]
fn credits_only_render_for_a_real_balance_or_unlimited_access() {
    let mut limits = plan_limits("plus");
    limits.credits.has_credits = true;
    limits.credits.balance = Some("undefined".into());
    assert_eq!(credits_display_value(&limits), None);
    assert!(!popup_sections(ProviderKind::Codex, &limits, false).contains(&PopupSection::Credits));

    limits.credits.balance = Some("$12.50".into());
    assert_eq!(credits_display_value(&limits).as_deref(), Some("$12.50"));
    assert!(popup_sections(ProviderKind::Codex, &limits, false).contains(&PopupSection::Credits));

    limits.credits = Default::default();
    limits.credits.unlimited = true;
    assert_eq!(credits_display_value(&limits).as_deref(), Some("Unlimited"));
}

#[test]
fn provider_cards_include_each_additional_limit() {
    let mut limits = plan_limits("plus");
    limits
        .additional_limits
        .push(crate::limits::AdditionalLimit {
            id: "seven_day_fable".into(),
            title: "Fable".into(),
            window: LimitWindow {
                used_percent: Some(42),
                ..Default::default()
            },
        });

    let cards = test_cards(
        ProviderKind::Claude,
        true,
        &limits,
        &all_visible(),
        PopupSurface::ProviderTab,
        true,
        true,
        false,
    );
    // Heading + 5h + weekly + Fable (no separate plan metadata row).
    assert_eq!(cards.len(), 4);
    assert_eq!(
        cards
            .iter()
            .filter_map(Card::limit_title)
            .collect::<Vec<_>>(),
        vec!["5H SESSION", "WEEKLY", "FABLE"]
    );
    assert!(!cards.iter().any(Card::is_usage_statistics));
}

#[test]
fn sections_keep_banked_resets_singleton() {
    let mut limits = plan_limits("plus");
    limits.reset_credits = Some(crate::limits::RateLimitResetCreditsSummary {
        available_count: 1,
        ..Default::default()
    });

    let sections = popup_sections(ProviderKind::Codex, &limits, true);
    assert_eq!(
        sections,
        vec![
            PopupSection::Error,
            PopupSection::FiveHour,
            PopupSection::Weekly,
            PopupSection::BankedResets,
        ]
    );
    assert_unique_section_keys(&sections);
}

#[test]
fn banked_resets_section_is_available_when_data_exists() {
    let mut limits = plan_limits("plus");
    limits.reset_credits = Some(crate::limits::RateLimitResetCreditsSummary {
        available_count: 1,
        ..Default::default()
    });

    assert!(
        popup_sections(ProviderKind::Codex, &limits, false).contains(&PopupSection::BankedResets)
    );
}

#[test]
fn every_limits_sample_forces_a_reactive_state_change() {
    let mut ui = UiState::default();
    let initial = ui.clone();

    ui.observe_limits_update();
    assert_ne!(ui, initial);
    assert_eq!(ui.limits_revision, 1);
    assert_eq!(ui.usage_revision, 0);

    // A Plus sample can have the same footer metadata as the preceding
    // Free sample; the revision still guarantees a rerender of the shared
    // snapshot.
    ui.observe_limits_update();
    assert_eq!(ui.limits_revision, 2);
    assert_eq!(ui.usage_revision, 0);
    assert_eq!(ui.last_activation, initial.last_activation);
    assert_eq!(ui.error, initial.error);

    ui.observe_usage_update();
    assert_eq!(ui.usage_revision, 1);
}

#[test]
fn credential_worker_events_are_accepted_only_for_the_current_revision() {
    let mut settings = Settings::default();
    for instance in &mut settings.instances {
        instance.enabled = true;
    }
    settings
        .instance_mut(id(ProviderKind::OpenRouter))
        .unwrap()
        .credentials_revision = 7;
    settings
        .instance_mut(id(ProviderKind::Claude))
        .unwrap()
        .credentials_revision = 3;
    let work = settings.add_instance(ProviderInstance::new(ProviderKind::Claude, "Work"));
    let ui = ui_from(&settings);

    let current =
        |provider, revision| bridge::provider_worker_event_is_current(&ui, provider, revision);
    assert!(current(id(ProviderKind::OpenRouter), 7));
    assert!(!current(id(ProviderKind::OpenRouter), 6));
    assert!(current(id(ProviderKind::Claude), 3));
    assert!(!current(id(ProviderKind::Claude), 2));
    // Another instance of the same driver has its own revision.
    assert!(current(work, 0));
    assert!(!current(work, 3));
    assert!(current(id(ProviderKind::OpenCodeZen), 0));
    // Disabled and removed instances drop queued output.
    let mut disabled = settings.clone();
    disabled.set_enabled(id(ProviderKind::Codex), false);
    let ui = ui_from(&disabled);
    assert!(!bridge::provider_worker_event_is_current(
        &ui,
        id(ProviderKind::Codex),
        0
    ));
    assert!(!bridge::provider_worker_event_is_current(
        &ui,
        ProviderId::new(ProviderKind::Claude, "claude-removed"),
        0
    ));
}

#[test]
fn provider_error_survives_until_that_provider_succeeds() {
    let mut ui = UiState::default();

    ui.set_provider_error(id(ProviderKind::Claude), "first failure");
    assert_eq!(
        ui.provider_error(id(ProviderKind::Claude)),
        Some("first failure")
    );
    assert!(!ui.has_provider_error(id(ProviderKind::Codex)));

    ui.set_provider_error(id(ProviderKind::Claude), "updated failure");
    assert_eq!(
        ui.provider_error(id(ProviderKind::Claude)),
        Some("updated failure")
    );

    ui.clear_provider_error(id(ProviderKind::Claude));
    assert_eq!(ui.provider_error(id(ProviderKind::Claude)), None);
}

#[test]
fn forbidden_provider_error_is_shortened_for_ui() {
    let mut ui = UiState::default();

    ui.set_provider_error(
        id(ProviderKind::Codex),
        "codex stderr: unexpected status 403 Forbidden: <html>the full response</html>",
    );

    assert_eq!(
        ui.provider_error(id(ProviderKind::Codex)),
        Some("Access denied by the provider (HTTP 403).")
    );
}

#[test]
fn repeated_network_failures_become_one_readable_provider_error() {
    let raw = "OpenRouter quota refresh failed: TEST: request https://openrouter.ai/api/v1/key: Connection Failed: Connect error: A connection attempt failed because the connected party did not properly respond (os error 10060); ".repeat(3);
    let mut ui = UiState::default();
    ui.set_provider_error(id(ProviderKind::OpenRouter), raw.clone());
    assert_eq!(
        ui.provider_error(id(ProviderKind::OpenRouter)),
        Some("The request timed out. Try refreshing again.")
    );
    ui.clear_provider_error(id(ProviderKind::OpenRouter));
    ui.set_usage_error(id(ProviderKind::OpenRouter), raw);
    assert_eq!(
        ui.provider_error(id(ProviderKind::OpenRouter)),
        Some("The request timed out. Try refreshing again.")
    );
}

#[test]
fn provider_errors_explain_http_failures_and_deduplicate_server_errors() {
    for (raw, expected) in [
        (
            "request failed: status code 401",
            "Authentication failed. Sign in again or update the provider key.",
        ),
        (
            "HTTP 429; HTTP 429; rate limited",
            "Too many requests. Wait a few minutes before refreshing again.",
        ),
        (
            "directory: HTTP 500; credits: HTTP 503; HTTP 500",
            "The provider is temporarily unavailable. Try again later.",
        ),
        (
            "Network Error: connection forcibly closed by remote host (os error 10054)",
            "The provider closed the connection. Try refreshing again.",
        ),
        (
            "parse OpenRouter analytics: invalid JSON response",
            "The provider returned an unexpected response. Try refreshing again.",
        ),
    ] {
        assert_eq!(UiState::error_for_ui(raw), expected);
    }
    assert_eq!(
        UiState::error_for_ui("API key: HTTP 401; credits: timed out; directory: timed out"),
        "The request timed out. Try refreshing again.\nAuthentication failed. Sign in again or update the provider key."
    );
}

#[test]
fn unknown_technical_dumps_stay_in_log_but_short_domain_errors_remain_visible() {
    assert_eq!(
        UiState::error_for_ui(&"unrecognized diagnostic context; ".repeat(20)),
        "The request failed. See Log for details."
    );
    assert_eq!(
        UiState::error_for_ui("Could not load https://provider.example/internal-request"),
        "The request failed. See Log for details."
    );
    assert_eq!(
        UiState::error_for_ui("This account no longer exists."),
        "This account no longer exists."
    );
}

#[test]
fn usage_error_uses_provider_error_presentation_without_overwriting_quota_error() {
    let mut ui = UiState::default();

    ui.set_usage_error(
        id(ProviderKind::OpenRouter),
        "OpenRouter analytics request failed: TLS certificate error",
    );
    assert_eq!(
        ui.provider_error(id(ProviderKind::OpenRouter)),
        Some("The secure connection could not be verified. See Log for details.")
    );
    assert!(ui.has_provider_error(id(ProviderKind::OpenRouter)));

    ui.set_provider_error(id(ProviderKind::OpenRouter), "quota request failed");
    assert_eq!(
        ui.provider_error(id(ProviderKind::OpenRouter)),
        Some("quota request failed")
    );

    ui.clear_provider_error(id(ProviderKind::OpenRouter));
    assert_eq!(
        ui.provider_error(id(ProviderKind::OpenRouter)),
        Some("The secure connection could not be verified. See Log for details.")
    );

    ui.clear_usage_error(id(ProviderKind::OpenRouter));
    assert_eq!(ui.provider_error(id(ProviderKind::OpenRouter)), None);
}

#[test]
fn nested_usage_errors_are_promoted_to_provider_error_state() {
    let mut statistics = crate::usage::UsageStatistics::default();
    statistics.accounts.insert(
        "account".into(),
        crate::usage::UsageStatistics {
            error: Some("analytics request failed".into()),
            ..Default::default()
        },
    );

    assert_eq!(
        usage_error_message(&statistics).as_deref(),
        Some("analytics request failed")
    );
}

#[test]
fn provider_timeout_has_a_readable_message() {
    let mut ui = UiState::default();

    ui.set_provider_error(
        id(ProviderKind::Codex),
        "Codex app-server response timed out",
    );

    assert_eq!(
        ui.provider_error(id(ProviderKind::Codex)),
        Some("The request timed out. Try refreshing again.")
    );
}

#[test]
fn unrelated_403_text_is_not_treated_as_an_http_status() {
    let mut ui = UiState::default();

    ui.set_provider_error(id(ProviderKind::Codex), "model 4030 is unavailable");

    assert_eq!(
        ui.provider_error(id(ProviderKind::Codex)),
        Some("model 4030 is unavailable")
    );
}

#[test]
fn refresh_indicator_waits_for_both_limit_and_usage_requests() {
    let mut ui = UiState::default();

    ui.request_started(id(ProviderKind::Codex), RequestKind::Limits);
    ui.request_started(id(ProviderKind::Codex), RequestKind::Usage);
    assert!(ui.refreshing);

    ui.request_finished(id(ProviderKind::Codex), RequestKind::Limits);
    assert!(ui.refreshing);

    ui.request_finished(id(ProviderKind::Codex), RequestKind::Usage);
    assert!(!ui.refreshing);
}

#[test]
fn pager_queues_only_the_latest_destination() {
    let state = reduce_pager(
        PagerState::default(),
        PagerAction::Select(PopupView::Provider(id(ProviderKind::Codex))),
    );
    assert_eq!(state.outgoing, Some(PopupView::Home));
    assert_eq!(state.current, PopupView::Provider(id(ProviderKind::Codex)));
    assert_eq!(state.direction, PagerDirection::Forward);

    let animation_id = state.animation_id;
    let state = reduce_pager(
        state,
        PagerAction::Select(PopupView::Provider(id(ProviderKind::Claude))),
    );
    let state = reduce_pager(
        state,
        PagerAction::Select(PopupView::Provider(id(ProviderKind::Cursor))),
    );
    assert_eq!(
        state.pending,
        Some(PopupView::Provider(id(ProviderKind::Cursor)))
    );

    let state = reduce_pager(state, PagerAction::AnimationFinished(animation_id));
    assert_eq!(
        state.outgoing,
        Some(PopupView::Provider(id(ProviderKind::Codex)))
    );
    assert_eq!(state.current, PopupView::Provider(id(ProviderKind::Cursor)));
    assert_eq!(state.pending, None);
    assert_eq!(state.direction, PagerDirection::Forward);
}

#[test]
fn pager_uses_reverse_motion_for_an_earlier_tab() {
    let state = PagerState {
        current: PopupView::Provider(id(ProviderKind::Cursor)),
        ..PagerState::default()
    };
    let state = reduce_pager(state, PagerAction::Select(PopupView::Home));
    assert_eq!(
        state.outgoing,
        Some(PopupView::Provider(id(ProviderKind::Cursor)))
    );
    assert_eq!(state.current, PopupView::Home);
    assert_eq!(state.direction, PagerDirection::Backward);
    assert!(state.direction.outgoing_offset(popup::POPUP_WIDTH) > 0.0);
    assert!(state.direction.incoming_offset(popup::POPUP_WIDTH) < 0.0);
}

#[test]
fn stale_pager_completion_cannot_end_a_newer_transition() {
    let state = reduce_pager(
        PagerState::default(),
        PagerAction::Select(PopupView::Provider(id(ProviderKind::Codex))),
    );
    let unchanged = reduce_pager(
        state.clone(),
        PagerAction::AnimationFinished(state.animation_id.wrapping_sub(1)),
    );
    assert_eq!(unchanged, state);
}

#[test]
fn openrouter_places_each_chart_inside_its_own_account_on_both_surfaces() {
    let date = Local::now().date_naive();
    let mut limits = RateLimits::default();
    for (id, requests) in [("first", 2), ("second", 7)] {
        limits.openrouter_accounts.push(OpenRouterAccountSnapshot {
            id: id.into(),
            name: id.into(),
            ..Default::default()
        });
        let mut stats = crate::usage::statistics_from_daily(
            &[crate::usage::DailyTokenUsage {
                date,
                usage: crate::usage::TokenUsage {
                    requests,
                    ..Default::default()
                },
            }],
            30,
        );
        stats.account_id = Some(id.into());
        limits.usage.accounts.insert(id.into(), stats);
    }
    {
        for surface in [PopupSurface::HomeTab, PopupSurface::ProviderTab] {
            for spending in [true, false] {
                let mut visibility = all_visible();
                visibility.set_brick("openrouter.spending", spending, spending);
                visibility.set_brick("openrouter.usage", true, true);
                let cards = test_cards(
                    ProviderKind::OpenRouter,
                    true,
                    &limits,
                    &visibility,
                    surface,
                    true,
                    true,
                    false,
                );
                assert_eq!(cards.len(), 3); // provider heading and two account strips; no combined card.
                for (index, id) in ["first", "second"].iter().enumerate() {
                    let charts = cards[index + 1]
                        .nested()
                        .iter()
                        .filter_map(|card| match card {
                            Card::UsageStatistics { statistics, .. } => {
                                statistics.account_id.as_deref()
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(charts, vec![*id]);
                }
            }
        }
    }
}

#[test]
fn two_columns_only_widen_home_and_usage_for_every_provider() {
    for enabled in [false, true] {
        assert_eq!(PopupView::Home.uses_two_columns(enabled), enabled);
        assert_eq!(PopupView::Usage.uses_two_columns(enabled), enabled);
        for provider in ProviderKind::ALL {
            assert!(!PopupView::from_provider(provider.into()).uses_two_columns(enabled));
            assert!(!PopupView::Group(provider).uses_two_columns(enabled));
        }
    }
}

#[test]
fn pager_slides_clear_both_pages_for_every_compact_and_wide_view_pair() {
    let views = std::iter::once(PopupView::Home)
        .chain(std::iter::once(PopupView::Usage))
        .chain(
            ProviderKind::ALL
                .into_iter()
                .map(|provider| PopupView::from_provider(provider.into())),
        )
        .chain(ProviderKind::ALL.into_iter().map(PopupView::Group))
        .collect::<Vec<_>>();
    for two_columns in [false, true] {
        for &from in &views {
            for &to in &views {
                let width_for = |view: PopupView| {
                    if view.uses_two_columns(two_columns) {
                        popup::POPUP_WIDE_WIDTH
                    } else {
                        popup::POPUP_WIDTH
                    }
                };
                let width = pager_slide_width(from, to, two_columns);
                assert_eq!(width, width_for(from).max(width_for(to)));
                for direction in [PagerDirection::Forward, PagerDirection::Backward] {
                    let incoming = direction.incoming_offset(width);
                    let outgoing = direction.outgoing_offset(width);
                    assert_eq!(incoming, -outgoing);
                    assert!(incoming.abs() >= width_for(to) as f32);
                    assert!(outgoing.abs() >= width_for(from) as f32);
                    assert_eq!(
                        outgoing.is_sign_negative(),
                        direction == PagerDirection::Forward
                    );
                }
            }
        }
    }
}

/// Every primary plus a second Claude and a second Codex instance, placed
/// right after their drivers' primaries.
fn instance_fixture() -> Settings {
    let mut settings = Settings::default();
    let mut claude = ProviderInstance::new(ProviderKind::Claude, "Work");
    claude.id = "claude-work".into();
    settings.add_instance(claude);
    let mut codex = ProviderInstance::new(ProviderKind::Codex, "Personal");
    codex.id = "codex-personal".into();
    settings.add_instance(codex);
    settings
}

#[test]
fn every_enabled_instance_combination_has_matching_tabs() {
    let base = instance_fixture();
    let count = base.instances.len();
    for mask in 0..(1_u32 << count) {
        let mut settings = base.clone();
        for (index, instance) in settings.instances.iter_mut().enumerate() {
            instance.enabled = mask & (1 << index) != 0;
        }
        let enabled = settings.enabled_providers();
        for mode in PopupTabMode::ALL {
            settings.popup_tab_mode = mode;
            let ui = ui_from(&settings);
            let tabs = provider_tabs(&ui);
            // Tab identities never repeat, so no slot can reuse another
            // tab's icon, badge or label.
            let keys = tabs.iter().map(|tab| tab.key()).collect::<HashSet<_>>();
            assert_eq!(keys.len(), tabs.len(), "mask={mask} {mode:?}");
            // Every enabled instance is shown by exactly one tab, and the
            // tab points at that instance's own snapshot.
            for provider in &enabled {
                let showing = tabs.iter().filter(|tab| tab.shows(*provider)).count();
                assert_eq!(showing, 1, "mask={mask} {mode:?} {provider:?}");
                let tab = tab_for_provider(&ui, *provider).unwrap();
                assert!(tab_members(&ui.instances, tab).contains(provider));
            }
            let members = tabs
                .iter()
                .flat_map(|tab| tab_members(&ui.instances, *tab))
                .collect::<Vec<_>>();
            assert_eq!(members.len(), enabled.len());
            for tab in &tabs {
                match tab {
                    PopupView::Provider(provider) => {
                        assert!(enabled.contains(provider));
                        if mode.is_grouped() {
                            assert_eq!(ui.enabled_instances_of(provider.kind()).len(), 1);
                        }
                    }
                    PopupView::Group(driver) => {
                        assert!(mode.is_grouped());
                        assert!(ui.enabled_instances_of(*driver).len() > 1);
                        assert_eq!(
                            tab_members(&ui.instances, *tab),
                            ui.enabled_instances_of(*driver)
                        );
                    }
                    PopupView::Home | PopupView::Usage => panic!("not a provider tab"),
                }
            }
            if mode == PopupTabMode::Separate {
                assert_eq!(
                    tabs,
                    enabled
                        .iter()
                        .map(|provider| PopupView::Provider(*provider))
                        .collect::<Vec<_>>()
                );
            }
            assert_eq!(show_provider_tabs(&ui), tabs.len() > 1);
            // Tabs keep the shared instance order.
            assert!(
                tabs.windows(2)
                    .all(|pair| pair[0].order(&tabs) < pair[1].order(&tabs))
            );
        }
    }
}

#[test]
fn grouped_tab_selection_falls_back_to_the_first_enabled_member() {
    let mut settings = instance_fixture();
    for instance in &mut settings.instances {
        instance.enabled = instance.driver == ProviderKind::Claude;
    }
    settings.popup_tab_mode = PopupTabMode::GroupedSwitcher;
    let work = ProviderId::new(ProviderKind::Claude, "claude-work");
    let ui = ui_from(&settings);
    assert_eq!(
        provider_tabs(&ui),
        vec![PopupView::Group(ProviderKind::Claude)]
    );
    assert_eq!(
        selected_group_member(&ui, ProviderKind::Claude),
        Some(id(ProviderKind::Claude))
    );
    assert_eq!(
        tab_for_provider(&ui, work),
        Some(PopupView::Group(ProviderKind::Claude))
    );
    settings
        .grouped_tab_selection
        .insert("claude".into(), "claude-work".into());
    assert_eq!(
        selected_group_member(&ui_from(&settings), ProviderKind::Claude),
        Some(work)
    );
    settings.set_enabled(work, false);
    let ui = ui_from(&settings);
    assert_eq!(
        selected_group_member(&ui, ProviderKind::Claude),
        Some(id(ProviderKind::Claude))
    );
    // One enabled instance is unaffected by grouping.
    assert_eq!(
        provider_tabs(&ui),
        vec![PopupView::Provider(id(ProviderKind::Claude))]
    );
}

#[test]
fn grouped_tabs_reorder_as_blocks_and_keep_disabled_slots() {
    let mut settings = instance_fixture();
    for instance in &mut settings.instances {
        instance.enabled = matches!(instance.driver, ProviderKind::Claude | ProviderKind::Cursor);
    }
    settings.popup_tab_mode = PopupTabMode::GroupedStacked;
    let ui = ui_from(&settings);
    let tabs = provider_tabs(&ui);
    let cursor = PopupView::Provider(id(ProviderKind::Cursor));
    let claude = PopupView::Group(ProviderKind::Claude);
    assert_eq!(tabs, vec![claude, cursor]);
    let order = reordered_instance_ids(&ui.instances, &tabs, cursor, claude).unwrap();
    let mut reordered = settings.clone();
    reordered.apply_instance_order(&order);
    let ids = reordered
        .instances
        .iter()
        .map(|instance| instance.id.as_str())
        .collect::<Vec<_>>();
    let position = |id: &str| ids.iter().position(|item| *item == id).unwrap();
    assert!(position("cursor") < position("claude"));
    assert_eq!(position("claude-work"), position("claude") + 1);
    // Disabled instances keep their slots.
    for (index, instance) in settings.instances.iter().enumerate() {
        if !instance.enabled {
            assert_eq!(ids[index], instance.id);
        }
    }
    assert_eq!(provider_tabs(&ui_from(&reordered)), vec![cursor, claude]);
    assert!(reordered_instance_ids(&ui.instances, &tabs, claude, claude).is_none());
}

#[test]
fn home_instances_move_independently_and_restore_hidden_positions() {
    let mut settings = instance_fixture();
    for instance in &mut settings.instances {
        instance.enabled = matches!(instance.driver, ProviderKind::Claude | ProviderKind::Cursor);
    }
    let primary = HomeWidgetId::provider(id(ProviderKind::Claude));
    let work = HomeWidgetId("claude-work".into());
    let cursor = HomeWidgetId::provider(id(ProviderKind::Cursor));
    let mut ui = ui_from(&settings);
    assert_ne!(primary, work);
    let initial = home_widget_order(&ui);
    let primary_was_right = home_widget_right_column(&ui, false).contains(&primary);
    let (order, right) = home_widget_drop_layout(&ui, &work, &cursor, Some(1), false).unwrap();
    assert_eq!(
        order.iter().position(|w| w == &primary),
        initial.iter().position(|w| w == &primary)
    );
    assert!(right.contains(&work));
    assert_eq!(right.contains(&primary), primary_was_right);
    settings.popup_home_order = order;
    settings.popup_home_right_column = Some(right);
    let restored: Settings = toml::from_str(&toml::to_string(&settings).unwrap()).unwrap();
    assert_eq!(restored.popup_home_order, settings.popup_home_order);
    ui = ui_from(&restored);
    // Hiding an instance from Home keeps its slot.
    settings
        .instance_mut(ProviderId::new(ProviderKind::Claude, "claude-work"))
        .unwrap()
        .show_on_home = false;
    ui.apply_settings(&settings);
    assert!(!visible_home_widgets(&ui, false).contains(&work));
    let hidden_slot = home_widget_order(&ui)
        .iter()
        .position(|w| w == &work)
        .unwrap();
    let (order, _) = home_widget_drop_layout(&ui, &primary, &cursor, None, false).unwrap();
    assert_eq!(order.iter().position(|w| w == &work), Some(hidden_slot));
    settings.popup_home_order = order;
    settings
        .instance_mut(ProviderId::new(ProviderKind::Claude, "claude-work"))
        .unwrap()
        .show_on_home = true;
    ui.apply_settings(&settings);
    assert!(visible_home_widgets(&ui, false).contains(&work));
    assert!(home_widget_right_column(&ui, false).contains(&work));
}

#[test]
fn new_instances_join_home_after_their_driver() {
    let mut settings = Settings::default();
    settings.set_enabled(id(ProviderKind::Claude), true);
    settings.set_enabled(id(ProviderKind::Cursor), true);
    let ui = ui_from(&settings);
    settings.popup_home_order = home_widget_order(&ui);
    let mut work = ProviderInstance::new(ProviderKind::Claude, "Work");
    work.id = "claude-work".into();
    settings.add_instance(work);
    let ui = ui_from(&settings);
    let order = home_widget_order(&ui);
    let claude = order
        .iter()
        .position(|w| w == &HomeWidgetId::provider(id(ProviderKind::Claude)))
        .unwrap();
    assert_eq!(order[claude + 1], HomeWidgetId("claude-work".into()));
    assert_eq!(visible_home_widgets(&ui, false).len(), 3);
    assert_eq!(
        home_widget_right_column(&ui, false),
        visible_home_widgets(&ui, false)
            .into_iter()
            .skip(1)
            .step_by(2)
            .collect::<Vec<_>>()
    );
    let legacy: Settings = toml::from_str("version = 39\n").unwrap();
    assert!(legacy.popup_home_order.is_empty());
    assert!(legacy.popup_home_right_column.is_none());
}

#[test]
fn popup_startup_snapshots_restore_layout_instances_and_saved_columns() {
    for enabled in [false, true] {
        for right in [
            None,
            Some(Vec::new()),
            Some(vec![
                HomeWidgetId::provider(id(ProviderKind::Codex)),
                HomeWidgetId::provider(id(ProviderKind::Kiro)),
            ]),
        ] {
            for mode in PopupTabMode::ALL {
                let settings = Settings {
                    popup_two_columns: enabled,
                    popup_home_right_column: right.clone(),
                    popup_tab_mode: mode,
                    ..instance_fixture()
                };
                let saved = toml::to_string(&settings).unwrap();
                let restored: Settings = toml::from_str(&saved).unwrap();
                // Both startup snapshots use this shared seed, before any live settings push.
                let ui = UiState::popup_layout_from_settings(&restored);
                assert_eq!(ui.popup_two_columns, enabled);
                assert_eq!(ui.popup_home_right_column, right);
                assert_eq!(ui.popup_tab_mode, mode);
                assert_eq!(ui.instances, settings.instances);
            }
        }
    }
}

#[test]
fn popup_first_frame_excludes_openrouter_usage_without_a_management_key() {
    let mut settings = Settings::default();
    settings.set_usage_stats_provider_enabled(id(ProviderKind::Claude), false);
    // The raw preference may still include OpenRouter. The first render must
    // use the same effective filter as live updates.
    assert!(settings.usage_stats_provider_enabled(id(ProviderKind::OpenRouter)));
    let initial = UiState::popup_layout_from_settings(&settings);
    assert!(!initial.usage_stats_provider_enabled(id(ProviderKind::OpenRouter)));
    assert!(!initial.usage_stats_provider_enabled(id(ProviderKind::Claude)));
    assert!(initial.usage_stats_provider_enabled(id(ProviderKind::Codex)));
    assert_eq!(
        initial.usage_stats_providers,
        settings.usage_stats_providers()
    );
}

#[test]
fn only_read_relevant_changes_restart_an_instance_worker() {
    let before = instance_fixture().instances;
    let mut after = before.clone();
    for instance in after.iter_mut().chain(&mut before.clone()) {
        instance.enabled = true;
    }
    let before = after.clone();
    let work = after
        .iter_mut()
        .find(|instance| instance.id == "claude-work")
        .unwrap();
    work.name = "Renamed".into();
    work.badge = "RN".into();
    work.show_on_home = false;
    assert!(bridge::instances_needing_restart(&before, &after).is_empty());
    let work = after
        .iter_mut()
        .find(|instance| instance.id == "claude-work")
        .unwrap();
    work.binary_path = Some("C:/tools".into());
    assert_eq!(
        bridge::instances_needing_restart(&before, &after),
        vec![ProviderId::new(ProviderKind::Claude, "claude-work")]
    );
    let codex = after
        .iter_mut()
        .find(|instance| instance.id == "codex")
        .unwrap();
    codex.credentials_revision += 1;
    assert_eq!(bridge::instances_needing_restart(&before, &after).len(), 2);
}
