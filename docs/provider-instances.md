# Provider instances

Replaces "accounts inside a provider" with independent provider instances,
modelled on T3 Code. A provider kind becomes a *driver*; every instance of a
driver is a first-class provider with its own name, parameters, paths, worker,
popup tab, Home card, activation state and usage statistics.

Minibar only reads limits — it is not a harness. An instance is a source of
limits, not an execution environment, so it does not have to be a folder.

## Scope

| Driver | Multiple instances | Isolating parameter |
|---|---|---|
| Claude | yes | `CLAUDE_CONFIG_DIR` or manual credential |
| Codex | yes | `CODEX_HOME` |
| OpenCode Zen | yes | API key |
| OpenCode Go | yes | API key |
| OpenRouter | yes | API keys (former OpenRouter accounts) |
| Cursor, Grok, Kiro, Antigravity | no (one instance) | none added |

`provider_registry` gains `supports_multiple_instances`. Single-instance
drivers use the same instance model; they simply cannot be added twice.

## Data model

```toml
[[instances]]
id = "claude"              # first migrated instance keeps the legacy provider id
driver = "claude"
name = "Claude"
enabled = true
badge = ""                 # empty = derived from name
badge_color = "auto"
show_on_home = true
auto_activation = true
usage_stats = true
binary_path = ""           # empty = auto-discovery
source = { kind = "config_folder", path = "" }   # or { kind = "manual" }
```

- `InstanceId` replaces `ProviderKind` as the key for workers, limit cache,
  popup tabs, Home widgets, tray indicators, Stream Deck actions,
  notifications, activation schedules/pauses and `activation-*.toml` files.
- IDs: first instance of a driver = legacy provider id (`claude`, `codex`,
  `openrouter`, …), so every existing string reference keeps pointing at it.
  Migrated profiles keep their profile id; migrated OpenRouter accounts keep
  their account id; new instances get `<driver>-<random>`.
- Instance order is the shared order for the Settings sidebar and popup tabs.

## Credentials (Claude / Codex)

`source`:

- **Config folder** (default, full feature set). Empty path = standard folder
  (`~/.claude` with Claude Desktop session fallback, `~/.codex`). Non-first
  instances without a path get `%LOCALAPPDATA%\Codex Minibar\instances\<id>\`.
  Credentials are read from the folder and never written back by Minibar.
  **Sign in** runs `claude /login` / `codex login` with the instance folder as
  `CLAUDE_CONFIG_DIR` / `CODEX_HOME`.
- **Manual credential** (Claude only, limits only). Stored in the protected
  secret store; kind detected by prefix: `sessionKey`/cookie → web limits,
  `sk-ant-oat…` → OAuth limits, `sk-ant-admin…` → Admin API spending.

Minibar no longer refreshes OAuth sessions itself. On an expired token it runs
a model-free status command once in the background (`claude auth status`,
`codex login status` or whatever actually refreshes — verify against the
installed CLI versions first), re-reads the file, and otherwise shows an error
with a **Sign in again** button. Nothing that sends a model request may be
used for refresh: it would start a 5-hour window.

Two instances of one driver may not resolve to the same canonical folder
(empty = default); Settings shows a validation error.

## Capabilities

Each instance exposes computed capabilities: `limits`, `usage_stats`,
`auto_activation`, `sign_in`, `custom_home`. Static part comes from the
registry, dynamic part from the environment (binary found, source kind).
Settings never hide unavailable features: rows are disabled with a reason
("Not available for manual credentials — switch Source to Config folder").
The instance header shows a `Limits only` chip when applicable. The popup
shows nothing extra. Losing a capability at runtime never resets a toggle.

## Per-instance features

- **Auto-activation**: toggle per instance; the CLI runs with the instance's
  config folder. Schedules and pauses bind to an instance id. One
  `activation-<instance-id>.toml` per instance; legacy files move to the first
  instance.
- **Usage statistics**: each instance scans transcripts in its own folder; the
  Usage tab filters/legends by instance and sums totals. Per-instance toggle
  replaces `usage_stats_excluded_providers`.
- **Notifications**: global thresholds, evaluated per instance, text names the
  instance ("Claude · Work: 15% left").
- **Tray / Stream Deck**: indicators and actions select an instance.
- Limit refresh interval stays global.

## Badge

- Text: first two letters of the name uppercased, or initials for multi-word
  names (`Big Corp` → `BC`); overridable with 1–3 characters.
- Color: fixed palette or `auto` (neutral plate).
- Shown only when a driver has more than one enabled instance.
- Rendered on popup tabs, Home card headers, Settings sidebar and Stream Deck.
  Not in the tray icon (too small).
- Badge text and color are part of the swap-chain icon host key so a rename or
  recolor remounts the host (see AGENTS.md).

## Popup

One setting with three values:

- **Separate tabs** (default): one tab per instance.
- **Grouped, switcher**: one tab per driver, segmented account switcher with
  badges and an animated indicator; selection persisted per driver.
- **Grouped, all accounts**: one tab per driver, a section per instance
  (badge + name header) with that instance's quota cards.

Grouped tabs use the driver icon without a badge. A driver with one enabled
instance is unaffected by grouping. Home always shows one card per instance
with independent order and column, regardless of the tab mode. The tab strip
key is built from the enabled instance set (plus tint mode and theme).

## Settings window

- Sidebar "Providers": one entry per instance (icon + badge + name), instances
  of one driver adjacent; enabled bright, disabled dimmed.
- "+ Add provider" opens a dialog: driver, name, badge, color. Single-instance
  drivers that already exist are disabled in the list.
- Any instance can be deleted (with confirmation), including the last one of a
  driver; the driver then disappears until re-added.
- Instance page: header (icon, name, status, `Limits only` chip, On toggle,
  delete) → General (display name, badge, badge color, show on Home) →
  Account (signed-in identity, Sign in / Sign in again) → Runtime (binary
  path, source + config folder; API keys for OpenCode/OpenRouter) → existing
  provider sections scoped to the instance.

## Migration

- Bump `SETTINGS_VERSION`; write `settings.toml.bak-v<N>` before migrating.
- Each driver gets its first instance (disabled drivers stay disabled).
- Claude/Codex profiles → instances. Pasted credentials → `manual` source
  (secret re-keyed). Minibar sign-in sessions → `config_folder` with an
  auto-created folder, credentials exported in the CLI's own format.
- OpenRouter accounts → instances, each keeping its API keys.
- Global `codex_path` / `claude_path` / other paths → first instance.
- Global `automatic_activation = true` enables auto-activation only on
  instances that previously qualified (Default Claude/Codex profiles).
- `HomeWidgetId { profile }`, `TrayIndicator.profile_id`, Home order/columns,
  cached per-profile limit snapshots → mapped onto instance ids.
- Removed after migration: `claude_profiles`, `codex_profiles`, `*_path`,
  `openrouter_accounts`, `*_home_excluded_profiles`, `show_accounts_as_tabs`,
  global `automatic_activation`, `usage_stats_excluded_providers`, the
  multi-profile merge layer and Minibar-managed OAuth refresh.
- Downgrade is not supported.

## Delivery

Branch `t3code/provider-instances` from `nightly` (which contains the GPUI
popup). One PR into `nightly`, as a series of commits that each pass
`cargo check`, clippy and tests:

1. Instance model, capabilities, migration with backup, migration tests.
2. Workers, limit cache, activation files, usage stats keyed by instance;
   profile layer removed.
3. Credential sources: config folder + in-folder Sign in, safe background
   refresh, manual credential.
4. Settings window: sidebar, instance page, Add provider, path validation.
5. Popup: per-instance tabs, badges, grouping modes, Home.
6. Tray, Stream Deck (Rust + TS), notifications, schedules.

Before finishing, review every enabled-instance combination (each alone,
pairs, all, none) for correct tab labels, icons and snapshots.
