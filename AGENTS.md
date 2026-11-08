Always check docs and examples before doing UI work. LLM doesn't have great knowledge of GPUI; read the vendored `gpui` crate sources and its `examples/` in the cargo registry.

Always run `cargo check` after changes related to the app to ensure the code is correct. No need to run it after changes related to the website.

Always run `cargo fmt` after changing Rust code, including tests, and before every commit. CI rejects unformatted code (`cargo fmt --package codex-minibar --check`).

Never launch the app itself.

## Worktree builds

Use `cargo-worktree.ps1` for local Cargo checks and builds so worktrees reuse the
main checkout's warm `target` directory instead of rebuilding all dependencies:

```powershell
.\cargo-worktree.ps1 check --locked
.\cargo-worktree.ps1 build --locked
.\cargo-worktree.ps1 test --all-targets --all-features --locked
.\cargo-worktree.ps1 clippy --all-targets --all-features --locked '--' -D warnings
```

The wrapper resolves the main checkout through `git --git-common-dir`, runs Cargo
in the current worktree, forwards arguments and exit codes, and restores the
caller's environment. It honors an explicit `CARGO_TARGET_DIR` override. In older
worktrees without the script, invoke the main checkout's script by absolute path
while keeping the current directory inside the worktree being checked (on this
machine: `& C:\Dev\codex-minibar\cargo-worktree.ps1 check --locked`).

Quote the `'--'` argument separator when forwarding flags to rustc, Clippy or
rustfmt; PowerShell otherwise consumes an unquoted separator before the script
receives it.

Shared-target Cargo commands wait for Cargo's build lock; do not bypass the lock
or treat the wait as a failed build. Keep toolchain, features, profiles and
RUSTFLAGS consistent for reuse. The app and vendored path dependencies may still
rebuild when switching worktrees. Never run `cargo clean` against the shared cache
as routine cleanup, and never delete it when removing a worktree.

The shared EXE belongs to the last build, not necessarily the main checkout's
branch. Do not infer its source worktree from its path. If a branch-specific
artifact or truly parallel build is required, explicitly use a separate
`CARGO_TARGET_DIR`. Never launch the app for verification. The wrapper does not
support `run` or `clean`. Release packaging continues to use `build.ps1`.

All settings must take effect immediately in the running application. The user must never need to relaunch the app for a setting change to be applied. Keep every open UI surface and affected background component synchronized with the updated settings.

## Appearance initialization guardrails

Theme and accent settings must be applied during application startup, before any window is shown. Every GPUI window (popup, Settings, onboarding) resolves its palette from the current settings and `window.appearance()` on every frame; never cache a palette across appearance changes, and observe `observe_window_appearance` so Auto follows the system theme. `crate::theme::apply_appearance` only records the accent for the tray glyphs.

Before finishing appearance work, review both cold-start paths (popup first and Settings first) for Auto, Light, and Dark themes. Opening or closing another window must never be required to correct colors. Always run `cargo check`; never launch the app for this verification.

## Settings window

The Settings and onboarding windows are GPUI windows in the same application as the popup (`src/ui/settings_window`). Build UI from `src/ui/gui/kit.rs` components; every edit goes through `Host::edit` (the trait in `src/ui/settings_window.rs` that `SettingsWindow` and `OnboardingWindow` implement), which updates the window's snapshot immediately and queues the changed settings on the serial settings writer. Committed changes from any surface flow back through `sync_open_window`.

## Provider UI guardrails

When adding or changing a provider, update every provider enumeration, settings migration, worker lifecycle, popup state, tray source and provider-tab path as one atomic change. Popup provider tabs carry their instance (or driver) identity in their GPUI element id, so a slot can never keep another provider's icon, badge or label; keep it that way when adding tab kinds.

Before finishing provider UI work, verify all enabled-provider combinations (each provider alone, every pair, all providers, and none) in code review. Confirm that every visible tab has the right label/icon and points to the matching provider snapshot. Always run `cargo check` after the change; never launch the app to perform that verification.

https://github.com/steipete/CodexBar - similar app for macos. You can use it as a reference for the features and implementations.

https://github.com/pingdotgg/t3code - T3 Code. Use it as a reference for the Usage tab, session-log scanning, and hourly/daily aggregation. Their scanner lives in `apps/server/src/usage/` (`UsageService`, `usageTranscripts`, `usageTranscriptReader`) and reads provider CLI transcripts (not T3's own orchestration), same approach as `ccusage`.

https://github.com/janekbaraniewski/openusage - OpenUsage. Local-first usage/quota dashboard; use it for Claude/Codex transcript conventions (dedupe, session logs, provider detection).

When you need icons, download them from iocnify. Icon pack - Phosphor Icons for settings window, Fluent UI Icons for the popup icons

All layout shifts have to be animated, respect the user's preference for animations.
