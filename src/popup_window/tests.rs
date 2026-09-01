use std::collections::HashSet;

use chrono::TimeZone;

use super::*;
use crate::settings::{PopupSurface, PopupVisibility};

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
        "Waiting for first update..."
    );
    assert_eq!(format_reset_in(None), "Unavailable");
}

#[test]
fn popup_refresh_is_sent_to_every_provider_worker() {
    let (codex_tx, codex_rx) = std::sync::mpsc::channel();
    let (claude_tx, claude_rx) = std::sync::mpsc::channel();
    let (cursor_tx, cursor_rx) = std::sync::mpsc::channel();
    let commands = vec![
        (ProviderKind::Codex, codex_tx),
        (ProviderKind::Claude, claude_tx),
        (ProviderKind::Cursor, cursor_tx),
    ];

    assert!(refresh_all_workers(&commands));
    assert_eq!(codex_rx.try_recv(), Ok(WorkerCommand::Refresh));
    assert_eq!(claude_rx.try_recv(), Ok(WorkerCommand::Refresh));
    assert_eq!(cursor_rx.try_recv(), Ok(WorkerCommand::Refresh));
}

#[test]
fn activity_chart_groups_long_histories_without_losing_tokens() {
    assert_eq!(compact_activity_bars(&[2, 3, 5], 60), vec![2, 3, 5]);
    assert_eq!(compact_activity_bars(&[2, 3, 5, 7, 11], 2), vec![10, 18]);
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
        (today - ChronoDuration::days(1), today - ChronoDuration::days(1))
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
    assert_eq!(format_spend(1_250_000), "$1.25");
}

#[test]
fn spend_donut_uses_native_arc_geometry() {
    let xaml = combined_usage_donut_xaml(
        &[
            (ProviderKind::Cursor, 2_000_000),
            (ProviderKind::Claude, 1_000_000),
            (ProviderKind::Codex, 500_000),
        ],
        3_500_000,
        ColorScheme::Dark,
    );

    assert!(xaml.starts_with("<Grid"));
    assert_eq!(xaml.matches("<Path ").count(), 3);
    assert!(xaml.contains(" A 53.00 53.00 "));
    assert!(!xaml.contains("Rectangle"));
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

    assert!(popup_sections(&limits, false).contains(&PopupSection::UsageStatistics));
    let mut hidden_usage = all_visible();
    hidden_usage.set_brick("opencode.usage", false, false);
    let cards = provider_cards(
        ProviderKind::OpenCodeZen,
        true,
        &limits,
        false,
        true,
        &hidden_usage,
        PopupSurface::ProviderTab,
        true,
        false,
        ColorScheme::Dark,
        None,
        None,
        None,
    );
    assert_eq!(cards.len(), 1);
}

#[test]
fn swap_chain_strip_keys_include_identity_inputs_without_hover_state() {
    let providers = vec![ProviderKind::Codex, ProviderKind::Claude];
    let same = provider_tabs_key(&providers, true, false, ColorScheme::Dark);
    assert_eq!(
        same,
        provider_tabs_key(&providers, true, false, ColorScheme::Dark)
    );
    assert_ne!(
        same,
        provider_tabs_key(&[ProviderKind::Codex], true, false, ColorScheme::Dark)
    );
    assert_ne!(
        same,
        provider_tabs_key(
            &[ProviderKind::Claude, ProviderKind::Codex],
            true,
            false,
            ColorScheme::Dark,
        )
    );
    assert_ne!(
        same,
        provider_tabs_key(&providers, true, true, ColorScheme::Dark)
    );
    assert_ne!(
        same,
        provider_tabs_key(&providers, true, false, ColorScheme::Light)
    );

    assert_ne!(
        footer_actions_key(false, ColorScheme::Dark),
        footer_actions_key(true, ColorScheme::Dark)
    );
    assert_ne!(
        footer_actions_key(false, ColorScheme::Dark),
        footer_actions_key(false, ColorScheme::Light)
    );
}

#[test]
fn popup_visibility_hides_codex_resets_on_all_but_shows_on_provider_tab() {
    let mut limits = plan_limits("plus");
    limits.reset_credits = Some(crate::limits::RateLimitResetCreditsSummary {
        available_count: 1,
        ..Default::default()
    });
    let visibility = visibility_with("codex.resets", false, true);
    let all_cards = provider_cards(
        ProviderKind::Codex,
        true,
        &limits,
        false,
        true,
        &visibility,
        PopupSurface::HomeTab,
        true,
        false,
        ColorScheme::Dark,
        None,
        None,
        None,
    );
    let tab_cards = provider_cards(
        ProviderKind::Codex,
        true,
        &limits,
        false,
        true,
        &visibility,
        PopupSurface::ProviderTab,
        true,
        false,
        ColorScheme::Dark,
        None,
        None,
        None,
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
fn popup_section_all_off_drops_provider_from_home_tab() {
    let mut visibility = all_visible();
    visibility.set_provider_all_tab(ProviderKind::Codex, false);
    let widgets = visible_popup_widgets(
        &PopupWidgetKind::default_order(),
        false,
        &visibility,
        true,
        false,
        false,
        false,
        false,
        false,
    );
    assert!(!widgets.contains(&PopupWidgetKind::Codex));
    let limits = plan_limits("plus");
    let tab_cards = provider_cards(
        ProviderKind::Codex,
        true,
        &limits,
        false,
        true,
        &visibility,
        PopupSurface::ProviderTab,
        true,
        false,
        ColorScheme::Dark,
        None,
        None,
        None,
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
    let free = popup_sections(&plan_limits("free"), false);
    assert_eq!(free, vec![PopupSection::Monthly]);
    assert_unique_section_keys(&free);

    let plus = popup_sections(&plan_limits("plus"), false);
    assert_eq!(plus, vec![PopupSection::FiveHour, PopupSection::Weekly,]);
    assert_unique_section_keys(&plus);
}

#[test]
fn disabled_five_hour_session_is_omitted_from_popup() {
    let mut limits = plan_limits("plus");
    limits.primary = LimitWindow::default();

    let sections = popup_sections(&limits, false);
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
        popup_sections(&limits, false),
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
    assert!(!popup_sections(&limits, false).contains(&PopupSection::Credits));

    limits.credits.balance = Some("$12.50".into());
    assert_eq!(credits_display_value(&limits).as_deref(), Some("$12.50"));
    assert!(popup_sections(&limits, false).contains(&PopupSection::Credits));

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

    let cards = provider_cards(
        ProviderKind::Claude,
        true,
        &limits,
        false,
        true,
        &all_visible(),
        PopupSurface::ProviderTab,
        true,
        false,
        ColorScheme::Dark,
        None,
        None,
        None,
    );
    // Heading + 5h + weekly + Fable (no separate plan metadata row).
    assert_eq!(cards.len(), 4);
}

#[test]
fn sections_keep_banked_resets_singleton() {
    let mut limits = plan_limits("plus");
    limits.reset_credits = Some(crate::limits::RateLimitResetCreditsSummary {
        available_count: 1,
        ..Default::default()
    });

    let sections = popup_sections(&limits, true);
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

    assert!(popup_sections(&limits, false).contains(&PopupSection::BankedResets));
}

#[test]
fn every_limits_sample_forces_a_reactive_state_change() {
    let mut ui = UiState::default();
    let initial = ui.clone();

    ui.observe_limits_update();
    assert_ne!(ui, initial);
    assert_eq!(ui.limits_revision, 1);

    // A Plus sample can have the same footer metadata as the preceding
    // Free sample; the revision still guarantees a rerender of the shared
    // snapshot.
    ui.observe_limits_update();
    assert_eq!(ui.limits_revision, 2);
    assert_eq!(ui.last_activation, initial.last_activation);
    assert_eq!(ui.error, initial.error);
}

#[test]
fn provider_error_survives_until_that_provider_succeeds() {
    let mut ui = UiState::default();

    ui.set_provider_error(ProviderKind::Claude, "first failure");
    assert_eq!(
        ui.provider_error(ProviderKind::Claude),
        Some("first failure")
    );
    assert!(!ui.has_provider_error(ProviderKind::Codex));

    ui.set_provider_error(ProviderKind::Claude, "updated failure");
    assert_eq!(
        ui.provider_error(ProviderKind::Claude),
        Some("updated failure")
    );

    ui.clear_provider_error(ProviderKind::Claude);
    assert_eq!(ui.provider_error(ProviderKind::Claude), None);
}

#[test]
fn refresh_indicator_waits_for_both_limit_and_usage_requests() {
    let mut ui = UiState::default();

    ui.request_started(ProviderKind::Codex, RequestKind::Limits);
    ui.request_started(ProviderKind::Codex, RequestKind::Usage);
    assert!(ui.refreshing);

    ui.request_finished(ProviderKind::Codex, RequestKind::Limits);
    assert!(ui.refreshing);

    ui.request_finished(ProviderKind::Codex, RequestKind::Usage);
    assert!(!ui.refreshing);
}

#[test]
fn pager_queues_only_the_latest_destination() {
    let state = reduce_pager(PagerState::default(), PagerAction::Select(PopupView::Codex));
    assert_eq!(state.outgoing, Some(PopupView::Home));
    assert_eq!(state.current, PopupView::Codex);
    assert_eq!(state.direction, PagerDirection::Forward);

    let animation_id = state.animation_id;
    let state = reduce_pager(state, PagerAction::Select(PopupView::Claude));
    let state = reduce_pager(state, PagerAction::Select(PopupView::Cursor));
    assert_eq!(state.pending, Some(PopupView::Cursor));

    let state = reduce_pager(state, PagerAction::AnimationFinished(animation_id));
    assert_eq!(state.outgoing, Some(PopupView::Codex));
    assert_eq!(state.current, PopupView::Cursor);
    assert_eq!(state.pending, None);
    assert_eq!(state.direction, PagerDirection::Forward);
}

#[test]
fn pager_uses_reverse_motion_for_an_earlier_tab() {
    let state = PagerState {
        current: PopupView::Cursor,
        ..PagerState::default()
    };
    let state = reduce_pager(state, PagerAction::Select(PopupView::Home));
    assert_eq!(state.outgoing, Some(PopupView::Cursor));
    assert_eq!(state.current, PopupView::Home);
    assert_eq!(state.direction, PagerDirection::Backward);
    assert!(state.direction.outgoing_offset() > 0.0);
    assert!(state.direction.incoming_offset() < 0.0);
}

#[test]
fn every_provider_membership_has_the_expected_tab_order() {
    let default_order = PopupWidgetKind::default_order();
    for mask in 0_u8..64 {
        let codex = mask & 0b001 != 0;
        let claude = mask & 0b010 != 0;
        let cursor = mask & 0b100 != 0;
        let opencode_zen = mask & 0b01000 != 0;
        let opencode_go = mask & 0b10000 != 0;
        let openrouter = mask & 0b100000 != 0;
        let views = enabled_popup_views(
            &default_order,
            codex,
            claude,
            cursor,
            opencode_zen,
            opencode_go,
            openrouter,
        );
        let providers = provider_order_from_popup(&default_order);

        assert_eq!(views.first(), Some(&PopupView::Home));
        assert_eq!(views.get(1), Some(&PopupView::Usage));
        assert_eq!(views.contains(&PopupView::Codex), codex);
        assert_eq!(views.contains(&PopupView::Claude), claude);
        assert_eq!(views.contains(&PopupView::Cursor), cursor);
        assert_eq!(views.contains(&PopupView::OpenCodeZen), opencode_zen);
        assert_eq!(views.contains(&PopupView::OpenCodeGo), opencode_go);
        assert_eq!(views.contains(&PopupView::OpenRouter), openrouter);
        assert!(
            views
                .windows(2)
                .all(|pair| pair[0].order(&providers) < pair[1].order(&providers))
        );
        assert_eq!(
            views.len(),
            2 + usize::from(codex)
                + usize::from(claude)
                + usize::from(cursor)
                + usize::from(opencode_zen)
                + usize::from(opencode_go)
                + usize::from(openrouter)
        );
    }

    let reversed = vec![
        PopupWidgetKind::TotalSpend,
        PopupWidgetKind::Cursor,
        PopupWidgetKind::Claude,
        PopupWidgetKind::Codex,
        PopupWidgetKind::OpenCodeZen,
        PopupWidgetKind::OpenCodeGo,
        PopupWidgetKind::OpenRouter,
    ];
    let views = enabled_popup_views(&reversed, true, true, true, true, true, true);
    assert_eq!(
        views,
        vec![
            PopupView::Home,
            PopupView::Usage,
            PopupView::Cursor,
            PopupView::Claude,
            PopupView::Codex,
            PopupView::OpenCodeZen,
            PopupView::OpenCodeGo,
            PopupView::OpenRouter,
        ]
    );
}

#[test]
fn stale_pager_completion_cannot_end_a_newer_transition() {
    let state = reduce_pager(PagerState::default(), PagerAction::Select(PopupView::Codex));
    let unchanged = reduce_pager(
        state.clone(),
        PagerAction::AnimationFinished(state.animation_id.wrapping_sub(1)),
    );
    assert_eq!(unchanged, state);
}
