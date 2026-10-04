use std::collections::HashSet;

use chrono::TimeZone;

use super::*;
use crate::claude::set_home_profile_visibility as set_claude_home_visibility;
use crate::settings::{PopupSurface, PopupVisibility};

#[test]
fn home_account_visibility_filters_only_the_rendered_copy_and_can_restore_all() {
    use crate::settings::ClaudeProfile;
    let profiles = vec![
        ClaudeProfile {
            id: ClaudeProfile::DEFAULT_ID.into(),
            name: "Default".into(),
            enabled: true,
        },
        ClaudeProfile {
            id: "work".into(),
            name: "Work".into(),
            enabled: true,
        },
    ];
    let limits = RateLimits {
        claude_profiles: profiles
            .iter()
            .enumerate()
            .map(|(index, profile)| crate::limits::ClaudeProfileSnapshot {
                id: profile.id.clone(),
                name: profile.name.clone(),
                limits: RateLimits {
                    primary: LimitWindow {
                        used_percent: Some(10 + index as u8),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                error: None,
            })
            .collect(),
        ..Default::default()
    };
    let mut excluded = Vec::new();
    set_claude_home_visibility(&mut excluded, ClaudeProfile::DEFAULT_ID, false);
    set_claude_home_visibility(&mut excluded, ClaudeProfile::DEFAULT_ID, false);
    assert_eq!(excluded.len(), 1);
    let home = claude_limits_for_home(&limits, &profiles, &excluded).unwrap();
    assert_eq!(home.claude_profiles.len(), 1);
    assert_eq!(home.claude_profiles[0].id, "work");
    assert_eq!(
        home.claude_profiles[0].limits.primary.used_percent,
        Some(11)
    );
    assert_eq!(limits.claude_profiles.len(), 2);
    assert_eq!(claude_account_tabs(&profiles).len(), 2);
    set_claude_home_visibility(&mut excluded, "work", false);
    assert!(claude_limits_for_home(&limits, &profiles, &excluded).is_none());
    set_claude_home_visibility(&mut excluded, "work", true);
    assert_eq!(
        claude_limits_for_home(&limits, &profiles, &excluded)
            .unwrap()
            .claude_profiles[0]
            .id,
        "work"
    );
    set_claude_home_visibility(&mut excluded, ClaudeProfile::DEFAULT_ID, true);
    assert_eq!(
        claude_limits_for_home(&limits, &profiles, &excluded)
            .unwrap()
            .claude_profiles
            .len(),
        2
    );
}

#[test]
fn home_visibility_also_hides_the_legacy_single_default_snapshot() {
    let limits = plan_limits("pro");
    assert!(claude_limits_for_home(&limits, &[], &[]).is_some());
    assert!(
        claude_limits_for_home(
            &limits,
            &[],
            &[crate::settings::ClaudeProfile::DEFAULT_ID.into()]
        )
        .is_none()
    );
}

#[test]
fn claude_account_tabs_follow_enabled_profiles_and_fall_back_after_removal() {
    use crate::settings::ClaudeProfile;
    let implicit = claude_account_tabs(&[]);
    assert_eq!(implicit.len(), 1);
    assert_eq!(implicit[0].id, ClaudeProfile::DEFAULT_ID);
    let saved = vec![
        ClaudeProfile {
            id: ClaudeProfile::DEFAULT_ID.into(),
            name: "Default".into(),
            enabled: false,
        },
        ClaudeProfile {
            id: "work".into(),
            name: "Work".into(),
            enabled: true,
        },
        ClaudeProfile {
            id: "personal".into(),
            name: "Personal".into(),
            enabled: true,
        },
    ];
    let tabs = claude_account_tabs(&saved);
    assert_eq!(
        tabs.iter()
            .map(|p| (p.id.as_str(), p.name.as_str()))
            .collect::<Vec<_>>(),
        vec![("work", "Work"), ("personal", "Personal")]
    );
    assert_eq!(
        selected_claude_account_tab(&tabs, Some("personal")),
        Some("personal")
    );
    assert_eq!(
        selected_claude_account_tab(&tabs, Some("removed")),
        Some("work")
    );
    let mut reversed = tabs.clone();
    reversed.reverse();
    assert_ne!(
        claude_account_tabs_key(&tabs),
        claude_account_tabs_key(&reversed)
    );
    assert_ne!(
        claude_account_tabs_key(&tabs),
        claude_account_tabs_key(&tabs[..1])
    );
    assert_eq!(selected_claude_account_tab(&[], Some("work")), None);
    let disabled = saved
        .into_iter()
        .map(|mut p| {
            p.enabled = false;
            p
        })
        .collect::<Vec<_>>();
    assert!(claude_account_tabs(&disabled).is_empty());
}

#[test]
fn account_tabs_replace_only_claude_for_every_enabled_provider_combination() {
    let accounts = claude_account_tabs(&[
        crate::settings::ClaudeProfile {
            id: "work".into(),
            name: "Work".into(),
            enabled: true,
        },
        crate::settings::ClaudeProfile {
            id: "personal".into(),
            name: "Personal".into(),
            enabled: true,
        },
    ]);
    for mask in 0..(1usize << ProviderKind::ALL.len()) {
        let providers = ProviderKind::ALL
            .iter()
            .enumerate()
            .filter_map(|(index, provider)| ((mask & (1 << index)) != 0).then_some(*provider))
            .collect::<Vec<_>>();
        let enabled_accounts = if providers.contains(&ProviderKind::Claude) {
            accounts.as_slice()
        } else {
            &[]
        };
        let expected = if providers.contains(&ProviderKind::Claude) {
            providers.len() + 2
        } else if providers.len() > 1 {
            providers.len()
        } else {
            0
        };
        assert_eq!(
            provider_icon_tab_count(&providers, enabled_accounts),
            expected,
            "mask={mask}"
        );
        assert_eq!(
            provider_icon_tab_count(&providers, &[]),
            if providers.len() > 1 {
                providers.len()
            } else {
                0
            }
        );
    }
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

    assert!(
        popup_sections(ProviderKind::OpenCodeZen, &limits, false)
            .contains(&PopupSection::UsageStatistics)
    );
    let excluded_from_home = all_visible();
    let home_cards = provider_cards(
        ProviderKind::OpenCodeZen,
        true,
        &limits,
        &[],
        false,
        true,
        false,
        true,
        &excluded_from_home,
        PopupSurface::HomeTab,
        true,
        false,
        false,
        ColorScheme::Dark,
        None,
        None,
        None,
        false,
        None,
        None,
        None,
        None,
        None,
    );
    assert_eq!(home_cards.len(), 1);

    let provider_page_cards = provider_cards(
        ProviderKind::OpenCodeZen,
        true,
        &limits,
        &[],
        false,
        true,
        false,
        true,
        &excluded_from_home,
        PopupSurface::ProviderTab,
        true,
        true,
        false,
        ColorScheme::Dark,
        None,
        None,
        None,
        false,
        None,
        None,
        None,
        None,
        None,
    );
    assert_eq!(provider_page_cards.len(), 2);

    let mut hidden_usage = all_visible();
    hidden_usage.set_brick("opencode.usage", false, false);
    let cards = provider_cards(
        ProviderKind::OpenCodeZen,
        true,
        &limits,
        &[],
        false,
        true,
        false,
        true,
        &hidden_usage,
        PopupSurface::ProviderTab,
        true,
        true,
        false,
        ColorScheme::Dark,
        None,
        None,
        None,
        false,
        None,
        None,
        None,
        None,
        None,
    );
    assert_eq!(cards.len(), 1);
}

#[test]
fn total_spend_provider_count_respects_usage_provider_selection() {
    assert_eq!(
        total_spend_provider_count(true, true, false, false, false, false, &[]),
        2
    );
    assert_eq!(
        total_spend_provider_count(
            true,
            true,
            false,
            false,
            false,
            false,
            &[ProviderKind::Codex.id().into()],
        ),
        1
    );
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
fn popup_body_key_changes_when_pace_label_appears_or_hides() {
    let now = Utc::now();
    let visible = RateLimits {
        primary: LimitWindow {
            used_percent: Some(20),
            resets_at: Some(now + ChronoDuration::hours(4)),
            duration_minutes: Some(300),
        },
        ..Default::default()
    };
    let hidden = RateLimits {
        primary: LimitWindow {
            used_percent: Some(20),
            resets_at: Some(now + ChronoDuration::hours(4) + ChronoDuration::minutes(55)),
            duration_minutes: Some(300),
        },
        ..Default::default()
    };
    let visible_key = popup_body_height_key(
        &ProviderLimits::from_entries([(ProviderKind::Codex, visible)]),
        PopupView::Codex,
        false,
        true,
        0,
    );
    let hidden_key = popup_body_height_key(
        &ProviderLimits::from_entries([(ProviderKind::Codex, hidden)]),
        PopupView::Codex,
        false,
        true,
        0,
    );

    assert_ne!(visible_key, hidden_key);

    let no_tibo_key =
        popup_body_height_key(&ProviderLimits::default(), PopupView::Codex, false, true, 0);
    let two_tibo_key =
        popup_body_height_key(&ProviderLimits::default(), PopupView::Codex, false, true, 2);
    assert_ne!(no_tibo_key, two_tibo_key);
}

#[test]
fn swap_chain_strip_keys_include_identity_inputs_without_hover_state() {
    let providers = vec![ProviderKind::Codex, ProviderKind::Claude];
    let same = provider_tabs_key(&providers, true, true, false, ColorScheme::Dark, &[]);
    assert_eq!(
        same,
        provider_tabs_key(&providers, true, true, false, ColorScheme::Dark, &[])
    );
    assert_ne!(
        same,
        provider_tabs_key(
            &[ProviderKind::Codex],
            true,
            true,
            false,
            ColorScheme::Dark,
            &[],
        )
    );
    assert_ne!(
        same,
        provider_tabs_key(
            &[ProviderKind::Claude, ProviderKind::Codex],
            true,
            true,
            false,
            ColorScheme::Dark,
            &[],
        )
    );
    assert_ne!(
        same,
        provider_tabs_key(&providers, true, true, true, ColorScheme::Dark, &[])
    );
    assert_ne!(
        same,
        provider_tabs_key(&providers, true, true, false, ColorScheme::Light, &[])
    );
    assert_ne!(
        same,
        provider_tabs_key(
            &providers,
            true,
            true,
            false,
            ColorScheme::Dark,
            &[ProviderKind::Claude],
        )
    );

    assert_ne!(
        footer_actions_key(false, ColorScheme::Dark),
        footer_actions_key(true, ColorScheme::Dark)
    );
    assert_ne!(
        footer_actions_key(false, ColorScheme::Dark),
        footer_actions_key(false, ColorScheme::Light)
    );

    let without_update = provider_tab_strip_viewport_width(false);
    let with_update = provider_tab_strip_viewport_width(true);
    let size = popup::bottom_bar_size();
    assert_eq!(
        without_update - with_update,
        size.icon_button_size() + size.action_spacing()
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
        &[],
        false,
        true,
        false,
        true,
        &visibility,
        PopupSurface::HomeTab,
        true,
        true,
        false,
        ColorScheme::Dark,
        None,
        None,
        None,
        false,
        None,
        None,
        None,
        None,
        None,
    );
    let tab_cards = provider_cards(
        ProviderKind::Codex,
        true,
        &limits,
        &[],
        false,
        true,
        false,
        true,
        &visibility,
        PopupSurface::ProviderTab,
        true,
        true,
        false,
        ColorScheme::Dark,
        None,
        None,
        None,
        false,
        None,
        None,
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
        &[],
        false,
        true,
        false,
        true,
        &visibility,
        PopupSurface::ProviderTab,
        true,
        true,
        false,
        ColorScheme::Dark,
        None,
        None,
        None,
        false,
        None,
        None,
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

    let cards = provider_cards(
        ProviderKind::Claude,
        true,
        &limits,
        &[],
        false,
        true,
        false,
        true,
        &all_visible(),
        PopupSurface::ProviderTab,
        true,
        true,
        false,
        ColorScheme::Dark,
        None,
        None,
        None,
        false,
        None,
        None,
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
    let ui = UiState {
        openrouter_credentials_revision: 7,
        claude_credentials_revision: 3,
        ..UiState::default()
    };

    assert!(bridge::provider_worker_event_is_current(
        &ui,
        ProviderKind::OpenRouter,
        7
    ));
    assert!(!bridge::provider_worker_event_is_current(
        &ui,
        ProviderKind::OpenRouter,
        6
    ));
    assert!(bridge::provider_worker_event_is_current(
        &ui,
        ProviderKind::Claude,
        3
    ));
    assert!(!bridge::provider_worker_event_is_current(
        &ui,
        ProviderKind::Claude,
        2
    ));
    assert!(!bridge::provider_worker_event_is_current(
        &ui,
        ProviderKind::Claude,
        0
    ));
    assert!(bridge::provider_worker_event_is_current(
        &ui,
        ProviderKind::OpenCodeZen,
        0
    ));
    assert!(bridge::provider_worker_event_is_current(
        &ui,
        ProviderKind::Codex,
        0
    ));
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
fn forbidden_provider_error_is_shortened_for_ui() {
    let mut ui = UiState::default();

    ui.set_provider_error(
        ProviderKind::Codex,
        "codex stderr: unexpected status 403 Forbidden: <html>the full response</html>",
    );

    assert_eq!(
        ui.provider_error(ProviderKind::Codex),
        Some("Access denied by the provider (HTTP 403).")
    );
}

#[test]
fn repeated_network_failures_become_one_readable_provider_error() {
    let raw = "OpenRouter quota refresh failed: TEST: request https://openrouter.ai/api/v1/key: Connection Failed: Connect error: A connection attempt failed because the connected party did not properly respond (os error 10060); ".repeat(3);
    let mut ui = UiState::default();
    ui.set_provider_error(ProviderKind::OpenRouter, raw.clone());
    assert_eq!(
        ui.provider_error(ProviderKind::OpenRouter),
        Some("The request timed out. Try refreshing again.")
    );
    ui.clear_provider_error(ProviderKind::OpenRouter);
    ui.set_usage_error(ProviderKind::OpenRouter, raw);
    assert_eq!(
        ui.provider_error(ProviderKind::OpenRouter),
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
        ProviderKind::OpenRouter,
        "OpenRouter analytics request failed: TLS certificate error",
    );
    assert_eq!(
        ui.provider_error(ProviderKind::OpenRouter),
        Some("The secure connection could not be verified. See Log for details.")
    );
    assert!(ui.has_provider_error(ProviderKind::OpenRouter));

    ui.set_provider_error(ProviderKind::OpenRouter, "quota request failed");
    assert_eq!(
        ui.provider_error(ProviderKind::OpenRouter),
        Some("quota request failed")
    );

    ui.clear_provider_error(ProviderKind::OpenRouter);
    assert_eq!(
        ui.provider_error(ProviderKind::OpenRouter),
        Some("The secure connection could not be verified. See Log for details.")
    );

    ui.clear_usage_error(ProviderKind::OpenRouter);
    assert_eq!(ui.provider_error(ProviderKind::OpenRouter), None);
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

    ui.set_provider_error(ProviderKind::Codex, "Codex app-server response timed out");

    assert_eq!(
        ui.provider_error(ProviderKind::Codex),
        Some("The request timed out. Try refreshing again.")
    );
}

#[test]
fn unrelated_403_text_is_not_treated_as_an_http_status() {
    let mut ui = UiState::default();

    ui.set_provider_error(ProviderKind::Codex, "model 4030 is unavailable");

    assert_eq!(
        ui.provider_error(ProviderKind::Codex),
        Some("model 4030 is unavailable")
    );
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
    assert!(state.direction.outgoing_offset(popup::POPUP_WIDTH) > 0.0);
    assert!(state.direction.incoming_offset(popup::POPUP_WIDTH) < 0.0);
}

#[test]
fn every_provider_membership_has_the_expected_tab_order() {
    let default_order = PopupWidgetKind::default_order();
    for mask in 0_u16..512 {
        let codex = mask & 0b001 != 0;
        let claude = mask & 0b010 != 0;
        let cursor = mask & 0b100 != 0;
        let opencode_zen = mask & 0b01000 != 0;
        let opencode_go = mask & 0b10000 != 0;
        let openrouter = mask & 0b100000 != 0;
        let antigravity = mask & 0b1000000 != 0;
        let grok = mask & 0b10000000 != 0;
        let kiro = mask & 0b100000000 != 0;
        let views = enabled_popup_views(
            &default_order,
            true,
            codex,
            claude,
            cursor,
            opencode_zen,
            opencode_go,
            openrouter,
            antigravity,
            grok,
            kiro,
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
        assert_eq!(views.contains(&PopupView::Antigravity), antigravity);
        assert_eq!(views.contains(&PopupView::Grok), grok);
        assert_eq!(views.contains(&PopupView::Kiro), kiro);
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
                + usize::from(antigravity)
                + usize::from(grok)
                + usize::from(kiro)
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
        PopupWidgetKind::Antigravity,
        PopupWidgetKind::Grok,
        PopupWidgetKind::Kiro,
    ];
    let views = enabled_popup_views(
        &reversed, true, true, true, true, true, true, true, true, true, true,
    );
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
            PopupView::Antigravity,
            PopupView::Grok,
            PopupView::Kiro,
        ]
    );
}

#[test]
fn usage_stats_toggle_removes_only_the_usage_view() {
    let views = enabled_popup_views(
        &PopupWidgetKind::default_order(),
        false,
        true,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
    );
    assert_eq!(views, vec![PopupView::Home, PopupView::Codex]);
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

#[test]
fn openrouter_places_each_chart_inside_its_own_account_on_both_surfaces() {
    fn chart_keys(element: &Element, keys: &mut Vec<String>) {
        if let Some(key) = element
            .key()
            .filter(|key| key.starts_with("activity-chart-"))
        {
            keys.push(key.into());
        }
        match element {
            Element::StackPanel(panel) => {
                for child in &panel.children {
                    chart_keys(child, keys);
                }
            }
            Element::Grid(grid) => {
                for child in &grid.children {
                    chart_keys(child, keys);
                }
            }
            Element::Border(border) => chart_keys(&border.child, keys),
            _ => {}
        }
    }
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
    for scheme in [ColorScheme::Dark, ColorScheme::Light] {
        for surface in [PopupSurface::HomeTab, PopupSurface::ProviderTab] {
            for spending in [true, false] {
                let mut visibility = all_visible();
                visibility.set_brick("openrouter.spending", spending, spending);
                visibility.set_brick("openrouter.usage", true, true);
                let cards = provider_cards(
                    ProviderKind::OpenRouter,
                    true,
                    &limits,
                    &[],
                    false,
                    true,
                    false,
                    true,
                    &visibility,
                    surface,
                    true,
                    true,
                    false,
                    scheme,
                    None,
                    None,
                    None,
                    false,
                    None,
                    None,
                    None,
                    None,
                    None,
                );
                assert_eq!(cards.len(), 3); // provider heading and two account strips; no combined card.
                for (index, id) in ["first", "second"].iter().enumerate() {
                    let mut keys = Vec::new();
                    chart_keys(&cards[index + 1], &mut keys);
                    assert_eq!(keys, vec![format!("activity-chart-openrouter-{id}")]);
                }
            }
        }
    }
    let before = openrouter_accounts_strip_key(&limits);
    limits
        .usage
        .accounts
        .get_mut("first")
        .unwrap()
        .daily
        .clear();
    assert_ne!(before, openrouter_accounts_strip_key(&limits));
}

#[test]
fn two_columns_only_widen_home_and_usage_for_every_provider() {
    for enabled in [false, true] {
        assert_eq!(PopupView::Home.uses_two_columns(enabled), enabled);
        assert_eq!(PopupView::Usage.uses_two_columns(enabled), enabled);
        for provider in ProviderKind::ALL {
            assert!(!PopupView::from_provider(provider).uses_two_columns(enabled));
        }
    }
}

#[test]
fn home_auto_columns_balance_every_visible_provider_combination() {
    for mask in 0..512 {
        let mut ui = UiState {
            codex_enabled: mask & 1 != 0,
            claude_enabled: mask & 2 != 0,
            cursor_enabled: mask & 4 != 0,
            opencode_zen_enabled: mask & 8 != 0,
            opencode_go_enabled: mask & 16 != 0,
            openrouter_enabled: mask & 32 != 0,
            antigravity_enabled: mask & 64 != 0,
            grok_enabled: mask & 128 != 0,
            kiro_enabled: mask & 256 != 0,
            ..UiState::default()
        };
        for show_spend in [false, true] {
            let widgets = visible_popup_widgets(
                &ui.popup_order,
                show_spend,
                &ui.popup_visibility,
                ui.codex_enabled,
                ui.claude_enabled,
                ui.cursor_enabled,
                ui.opencode_zen_enabled,
                ui.opencode_go_enabled,
                ui.openrouter_enabled,
                ui.antigravity_enabled,
                ui.grok_enabled,
                ui.kiro_enabled,
            );
            let right = home_right_column(&ui, show_spend);
            assert_eq!(right.len(), widgets.len() / 2);
            if let Some(first) = widgets.first() {
                assert!(!right.contains(first));
            }
            assert!(right.iter().all(|widget| widgets.contains(widget)));
        }
        ui.popup_right_column = Some(vec![PopupWidgetKind::Codex, PopupWidgetKind::Kiro]);
        assert_eq!(
            home_right_column(&ui, false),
            vec![PopupWidgetKind::Codex, PopupWidgetKind::Kiro]
        );
    }
}

#[test]
fn pager_slides_clear_both_pages_for_every_compact_and_wide_view_pair() {
    let views = std::iter::once(PopupView::Home)
        .chain(std::iter::once(PopupView::Usage))
        .chain(ProviderKind::ALL.into_iter().map(PopupView::from_provider))
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

#[test]
fn popup_startup_snapshots_restore_enabled_layout_and_saved_columns() {
    for enabled in [false, true] {
        for right in [
            None,
            Some(Vec::new()),
            Some(vec![PopupWidgetKind::Codex, PopupWidgetKind::Kiro]),
        ] {
            let settings = Settings {
                popup_two_columns: enabled,
                popup_right_column: right.clone(),
                ..Settings::default()
            };
            let saved = toml::to_string(&settings).unwrap();
            let restored: Settings = toml::from_str(&saved).unwrap();
            // Both startup snapshots use this shared seed, before any live settings push.
            let ui = UiState::popup_layout_from_settings(&restored);
            assert_eq!(ui.popup_two_columns, enabled);
            assert_eq!(ui.popup_right_column, right);
        }
    }
}

#[test]
fn popup_first_frame_excludes_openrouter_usage_without_a_management_key() {
    let mut settings = Settings::default();
    settings.set_usage_stats_provider_enabled(ProviderKind::Claude, false);
    assert!(settings.openrouter_accounts.is_empty());
    // Raw preferences may still include OpenRouter in an existing settings
    // file. The first render must use the same effective filter as live updates.
    assert!(settings.usage_stats_provider_enabled(ProviderKind::OpenRouter));
    let initial = UiState::popup_layout_from_settings(&settings);
    assert!(!initial.usage_stats_provider_enabled(ProviderKind::OpenRouter));
    assert!(!initial.usage_stats_provider_enabled(ProviderKind::Claude));
    assert!(initial.usage_stats_provider_enabled(ProviderKind::Codex));
    assert_eq!(
        initial.usage_stats_excluded_providers,
        settings.effective_usage_stats_excluded_providers()
    );
}

#[test]
fn pager_native_host_identity_survives_outgoing_page_removal() {
    let current: Element = scroll_viewer(caption("provider"))
        .with_key(popup_page_host_key("current"))
        .into();
    let outgoing: Element = scroll_viewer(caption("Home"))
        .with_key(popup_page_host_key("outgoing"))
        .into();
    let during = grid(vec![outgoing, current.clone()]);
    let after = grid(vec![current]);
    assert_eq!(during.children[1].key(), after.children[0].key());
    assert_ne!(during.children[0].key(), during.children[1].key());
    assert!(during.children.iter().all(|page| page.key().is_some()));
}

#[test]
fn two_column_native_host_stays_wide_on_every_compact_provider_page() {
    for enabled in [false, true] {
        let host_width = popup::native_host_width_for_layout(enabled);
        assert_eq!(
            host_width,
            if enabled {
                popup::POPUP_WIDE_WIDTH
            } else {
                popup::POPUP_WIDTH
            }
        );
        for provider in ProviderKind::ALL {
            let view = PopupView::from_provider(provider);
            assert!(!view.uses_two_columns(enabled));
            assert!(host_width >= popup::POPUP_WIDTH);
            if enabled {
                assert!(host_width > popup::POPUP_WIDTH);
            }
        }
    }
}

#[test]
fn home_shows_every_claude_profile_as_its_own_strip() {
    let mut limits = plan_limits("max");
    for id in ["default", "work"] {
        limits
            .claude_profiles
            .push(crate::limits::ClaudeProfileSnapshot {
                id: id.into(),
                name: id.into(),
                limits: plan_limits("max"),
                error: None,
            });
    }
    let cards = provider_cards(
        ProviderKind::Claude,
        true,
        &limits,
        &[],
        false,
        true,
        false,
        true,
        &all_visible(),
        PopupSurface::HomeTab,
        true,
        true,
        false,
        ColorScheme::Dark,
        None,
        None,
        None,
        false,
        None,
        None,
        None,
        None,
        None,
    );
    // One keyed strip per profile instead of one flat list of limit cards.
    assert_eq!(cards.len(), 2);
}
