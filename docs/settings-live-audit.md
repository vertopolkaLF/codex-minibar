# Settings live propagation audit

Branch: `t3/settings-live-propagation`.

Static consumer trace of every field in `Settings` (`src/settings.rs`). “Yes” means a committed in-app edit reaches the running consumer; it does not mean an event-driven setting produces an event immediately. Settings edits remain optimistic while the serial writer commits field edits against the latest file. All editable snapshots shared the final-publication race described below; the per-field table describes their consumer paths in addition to that shared issue.

| Setting | Consumers | Live before | Live after |
| --- | --- | --- | --- |
| `version` | Loader/migrations; save validation | Metadata | Metadata |
| `onboarding_completed` | Initial-window selection; wizard completion | Lifecycle marker | Lifecycle marker |
| `theme` | Popup palette/native backdrop, Settings, onboarding, tray accent | Yes | Yes |
| `language` | All windows/titles, tray menus, taskbar/Command Palette, future notifications | Last commit could miss language apply | Yes |
| `accent_color` | Popup palette/native backdrop, Settings, onboarding, tray accent | Yes | Yes |
| `font_family` | Popup, Settings, onboarding | Popup only | Yes: all three windows |
| `recent_font_families` | Settings font picker | Yes | Yes |
| `animations_enabled` | Global animation preference; popup and Settings/onboarding motion | Yes | Yes |
| `bottom_bar_size` | Popup chrome atomics and AppearanceChanged; Linux forces Solid material | Yes | Yes |
| `popup_corner_radius` | Popup chrome atomics and AppearanceChanged; Linux forces Solid material | Yes | Yes |
| `popup_background_material` | Popup chrome atomics and AppearanceChanged; Linux forces Solid material | Yes | Yes |
| `popup_theme` | Popup palette/chrome; Settings/onboarding selection/preview | Yes | Yes |
| `popup_vscode_theme` | Popup palette/chrome; Settings/onboarding selection/preview | Yes | Yes |
| `popup_vscode_contrast` | Popup palette/chrome; Settings/onboarding selection/preview | Yes | Yes |
| `popup_vscode_tint` | Popup palette/chrome; Settings/onboarding selection/preview | Yes | Yes |
| `popup_borders` | Popup palette/chrome; Settings/onboarding selection/preview | Yes | Yes |
| `popup_window_border` | Popup palette/chrome; Settings/onboarding selection/preview | Yes | Yes |
| `time_format` | Popup clocks, tray/taskbar/Command Palette reset labels, future notifications | Yes | Yes |
| `instances` | Popup, Settings/onboarding, provider workers, status watcher, tray/taskbar/Command Palette; instance registry | Yes | Yes |
| `popup_tab_mode` | Popup pager/Home/layout; Settings customization | Yes | Yes |
| `grouped_tab_selection` | Popup pager/Home/layout; Settings customization | Yes | Yes |
| `popup_two_columns` | Popup pager/Home/layout; Settings customization | Yes | Yes |
| `popup_home_order` | Popup pager/Home/layout; Settings customization | Yes | Yes |
| `popup_home_right_column` | Popup pager/Home/layout; Settings customization | Yes | Yes |
| `popup_home_card_layouts` | Popup pager/Home/layout; Settings customization | Yes | Yes |
| `use_colored_provider_icons` | Popup, taskbar, Command Palette; Settings control | Yes | Yes |
| `replace_chatgpt_logo_with_codex` | Provider registry glyph selection: popup/tray/taskbar/Command Palette/Settings | Yes | Yes |
| `scheduled_activations` | Worker commands; Settings scheduling editors | Yes | Yes |
| `auto_activation_pauses` | Worker commands; Settings scheduling editors | Yes | Yes |
| `auto_reset_lead` | AutoReset bridge poll reads live_settings; Settings editor | Yes | Yes |
| `usage_stats_enabled` | Popup Usage/Home; worker collection command; Settings | Yes | Yes |
| `usage_refresh_interval` | Every worker via SetUsageRefreshInterval; Settings/onboarding | Yes | Yes |
| `reset_announcement_refresh_interval` | Reset-feed worker command; Settings | Yes | Yes |
| `start_at_login` | Runtime startup registration: Windows Run / Linux XDG autostart; Settings/onboarding | Yes | Yes |
| `show_used_percentage` | Popup renderer/layout; Settings controls (onboarding exposes a subset) | Yes | Yes |
| `show_usage_values` | Popup renderer/layout; Settings controls (onboarding exposes a subset) | Yes | Yes |
| `show_usage_pace` | Popup renderer/layout; Settings controls (onboarding exposes a subset) | Yes | Yes |
| `popup_visibility` | Popup renderer/layout; Settings controls (onboarding exposes a subset) | Yes | Yes |
| `show_total_spend_on_all_tab` | Popup renderer/layout; Settings controls (onboarding exposes a subset) | Yes | Yes |
| `total_spend_presentation` | Popup renderer/layout; Settings controls (onboarding exposes a subset) | Yes | Yes |
| `total_spend_period` | Popup renderer/layout; Settings controls (onboarding exposes a subset) | Yes | Yes |
| `show_account_name` | Popup renderer/layout; Settings controls (onboarding exposes a subset) | Yes | Yes |
| `show_edit_home_button` | Popup renderer/layout; Settings controls (onboarding exposes a subset) | Yes | Yes |
| `show_popup_action_bar` | Popup renderer/layout; Settings controls (onboarding exposes a subset) | Yes | Yes |
| `popup_pinned` | Popup lifecycle/topmost; Linux open-window topmost; Settings | Yes | Yes |
| `popup_floating` | Popup host placement/layout; Windows native positioning; Linux X11 placement (Wayland compositor controls position) | Yes | Yes |
| `popup_height_dip` | Popup host placement/layout; Windows native positioning; Linux X11 placement (Wayland compositor controls position) | Yes | Yes |
| `popup_position` | Windows floating panel saved position; Linux uses compositor dragging rather than persisting coordinates | Yes (Windows) | Yes (Windows) |
| `popup_placement` | Popup host placement/layout; Windows native positioning; Linux X11 placement (Wayland compositor controls position) | Yes | Yes |
| `popup_hotkey` | Windows bridge Registration::sync; Settings recorder. Linux uses desktop shortcut integration instead | Yes | Yes |
| `tray_widgets` | TrayManager::sync on each commit; Settings previews/editor | Yes | Yes |
| `taskbar` | Windows taskbar publish_settings; Settings editor. Hidden on Linux | Yes | Yes |
| `command_palette` | Published Dock bands/indicators; bridge requests read latest. Windows integration; hidden on Linux | Yes | Yes |
| `notifications` | Bridge trackers/feed/status workers; notification globals, updater, auto reset, game mode; Settings/onboarding | Partial: failure toggle unused; updater captured preference | Yes for active options; see nested fields |
| `history_retention_days` | Every worker via SetHistoryRetentionDays: reload cached range/refresh; Settings | Yes | Yes |
| `check_for_updates` | Startup automatic check; enabling starts check; disabling silences pending automatic update toast | Pending check retained toast permission | Yes (in-flight request may finish) |
| `software_rendering` | Vendored Windows GPUI device policy; open devices rebuild next vsync; Settings. Hidden on Linux | Yes | Yes |
| `game_mode` | Game detector/runtime atomics; worker poll cadence, auto reset, notifications; Settings | Yes | Yes |
| `game_mode_games` | Game detector/runtime atomics; worker poll cadence, auto reset, notifications; Settings | Yes | Yes |
| `game_mode_ignored` | Game detector/runtime atomics; worker poll cadence, auto reset, notifications; Settings | Yes | Yes |
| `unknown_entries` | Loader/save forward-compatibility preservation | Metadata | Metadata |

## Nested settings

| Fields | Consumers | Live before → after |
| --- | --- | --- |
| `instances.{id,driver}` | Instance identity/dispatch; worker lifecycle | Identity metadata; instance creation/removal live |
| `instances.{name,badge,badge_color}` | Published registry, popup, Settings, tray/taskbar/Command Palette, notification names | Yes → Yes |
| `instances.{enabled,show_on_home,in_usage_overview}` | Worker membership, popup tabs/Home/Usage, status watcher, tray visibility | Yes → Yes |
| `instances.{auto_activation,auto_reset,usage_stats,refresh_interval}` | Worker activation/collection/interval commands; AutoReset live snapshot | Yes → Yes |
| `instances.{binary_path,source,kiro_crew_path,kiro_cli_path,credentials_revision}` | Runtime-key comparison replaces worker; queued old-revision events rejected | Yes → Yes |
| `instances.openrouter.{id,name,api_key_ids,api_key_names}` | Account identity/name overlay, key workers and key UI; API-key-id changes replace worker | Yes → Yes |
| `scheduled_activations.{id,provider_id,weekday,weekdays,time_minutes,enabled}` | Per-instance worker schedules; legacy weekday compatibility | Yes → Yes |
| `auto_activation_pauses.{id,provider_id,weekdays,start_time_minutes,end_time_minutes,enabled}` | Per-instance worker pause windows | Yes → Yes |
| `popup_visibility.{bricks,instances}` and `{all_tab,provider_tab}` | Popup render/filter; Settings customization | Yes → Yes |
| `popup_home_card_layouts` entries | Popup Home quota layouts | Yes → Yes |
| `popup_position.{monitor,x_dip,y_dip,screen_x,screen_y}` | Windows popup placement; Linux compositor limits as above | Yes → Yes |
| `tray_widgets.{id,kind,indicators,presentation}` | Tray reconciliation; Settings preview | Yes → Yes |
| `TrayIndicator.{provider_id,metric_id,limit_value,color_mode,fixed_color}`; fixed `{red,green,blue}` | Tray/taskbar/Command Palette glyph and value rendering | Yes → Yes |
| `taskbar.{enabled,alignment,cycle,click,widgets}` | Windows widget host and action dispatch | Yes → Yes |
| `command_palette.widgets` | Latest published Dock response | Yes → Yes |
| `TaskbarWidget.{id,style,indicators,show_icon,show_labels,show_values,reset,show_pace}` | Taskbar/Dock composition and previews | Yes → Yes |
| `notifications.activation_success` | Bridge activation result | Yes → Yes |
| `notifications.activation_failure` | Bridge activation failure | No consumer → Error notification on failure |
| `notifications.{codex_unavailable,approaching_reset}` | No runtime consumer or current UI control; retained legacy serialized flags | Dormant → Dormant (no new behavior introduced) |
| `notifications.{limits_changed,low_usage_enabled,low_usage_threshold_percent,weekly_low_usage_enabled,weekly_low_usage_threshold_percent}` | Bridge LimitNotificationTracker, current settings on each observation | Yes → Yes |
| `notifications.update_available` | UpdateController result notification | Captured at request start → Live atomic permission at completion |
| `notifications.{forced_reset_feed_enabled,forced_reset_notifications}` | Feed worker enable/interval, popup feed data, bridge notifications | Yes → Yes |
| `notifications.{provider_status_enabled,provider_status_notifications}` | Status worker watched kinds, popup statuses, bridge notifications | Yes → Yes |
| `notifications.{sound,follow_do_not_disturb}` | Notification runtime atomics; Linux notification silent hint | Yes → Yes |
| `notifications.game_mode_changes` | Game detector notification atomic | Yes → Yes |
| `notifications.auto_reset_upcoming` | AutoReset polls current settings | Yes → Yes |

## Fixes and synchronization boundary

- Publish commits only after the serial writer decrements its pending count. Previously the final snapshot could reach `apply_sync` while pending was still true and be dropped permanently. This also fixes a final language edit missing its global apply. Intermediate optimistic snapshots remain protected.
- Send committed snapshots to onboarding as well as Settings; onboarding adopts provider membership and current settings while idle, retaining its original cancellation snapshot.
- Resolve the chosen font from current settings in both Settings and onboarding renders instead of always using their cached default font. Popup already resolves its configured font live.
- Honor activation-failure notifications and consult current updater notification permission after a request finishes. Turning off automatic checks suppresses an outstanding automatic check's toast; the network request itself is allowed to finish.

Appearance cold-start review: startup applies runtime effects and theme/accent before starting GPUI. Popup seeds its first snapshot from startup settings; Settings and onboarding load settings before their first render. Each resolves Auto/Light/Dark against window appearance and observes appearance changes; no second window is needed to correct colors. Selected font now follows the same per-frame settings path.

Provider review: membership drives tab construction and native strip identity; this patch does not change tab/icon reconciliation. Instance runtime-key changes restart workers, and bridge commands update schedules, pauses, activation, poll intervals, collection and retention. Taskbar and Dock receive each committed snapshot. Stream Deck reads current bridge state, not the startup settings snapshot.

Persistence: edits reload, modify, normalize and atomically replace the file on one serial writer. Rapid field edits do not replace whole snapshots and therefore preserve each other's changes; reset/import are intentional whole-snapshot replacements. Existing FIFO and panic-survival tests plus the publication-order regression test cover the writer. Disk errors remain reported to stderr (fire-and-forget edits do not provide a UI error notice).

External file edits: there is no file watcher or supported automatic external live reload. Opening a settings window and the next in-app edit reload the file, so untouched externally edited fields survive normal field edits. Simultaneous external writers are not serialized with this process; no cross-process merge guarantee is claimed. No new watcher was added.

Windows-only settings remain hidden on Linux; Linux autostart and notification settings apply through their existing runtime paths. Linux desktop shortcut configuration is stored by the desktop integration, separately from `Settings.popup_hotkey`.

## Validation

- `cargo fmt` and `cargo fmt --package codex-minibar --check`: passed.
- `cargo check --locked`: passed.
- `cargo test --all-targets --all-features --locked`: passed: 764 passed, 0 failed, 1 ignored (including the writer publication-order regression).
- `CARGO_TARGET_DIR=target/win-check cargo clippy --target x86_64-pc-windows-gnu --all-targets --all-features -- -D warnings`: passed.
- `cargo clippy --all-targets --all-features --locked -- -D warnings`: passed.
- `git diff --check`: passed. No interactive app verification was performed; this audit uses source tracing and the automated checks above.
