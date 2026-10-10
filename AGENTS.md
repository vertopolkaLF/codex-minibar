Always check docs and examples before doing UI work. LLM doesn't have great knowledge of GPUI; read the vendored `gpui` crate sources and its `examples/` in the cargo registry.

Always run `cargo check` after changes related to the app to ensure the code is correct. No need to run it after changes related to the website.

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

The Settings and onboarding windows are GPUI windows in the same application as the popup (`src/settings_window`). Build UI from `kit.rs` components; every edit goes through `SettingsWindow::edit`, which updates the local snapshot immediately and queues the write on the serial settings writer. Committed changes from any surface flow back through `sync_open_window`.

## Provider UI guardrails

When adding or changing a provider, update every provider enumeration, settings migration, worker lifecycle, popup state, tray source and provider-tab path as one atomic change. Native popup provider tabs use swap-chain icon hosts: key the complete tab selector by the enabled-provider set so reconciliation cannot reuse an old provider's text or icon in another provider's slot. Do not treat `Element::Empty` as sufficient remounting.

Before finishing provider UI work, verify all enabled-provider combinations (each provider alone, every pair, all providers, and none) in code review. Confirm that every visible tab has the right label/icon and points to the matching provider snapshot. Always run `cargo check` after the change; never launch the app to perform that verification.

## Swap-chain icon / popup chrome identity

Popup tabs and icon buttons paint through `SwapChainPanel` hosts. The painter runs **only on mount** (`acrylic::install_*_into`). In-place updates do **not** repaint the glyph. Recycled native panels can also retain a previous XAML child — installers must **clear and replace** panel children, never early-return because the panel is already non-empty.

Prevent “button/tab shows a copy of its neighbor” bugs:

1. **Stable keys for ephemeral UI state.** Do not put hover/selection into a swap-chain host’s `with_key`. Remounting on hover recycles native panels and can leave another control’s icon in the slot. Prefer dual hosts with opacity crossfade (idle + accent/emphasized).
2. **Identity keys for real content changes.** Key by glyph name, tint/theme, and provider/action id so a real icon change remounts the host.
3. **No `Element::Empty` placeholders** in rows that contain swap-chain children. Build a `Vec` of only live tabs/actions. Empty siblings collapse during reconcile and shift slots.
4. **Key the whole strip** when membership or control kind changes (provider tab selector by enabled set + tint mode + theme; footer actions by quit-vs-update + theme).
5. **Never assume `Element::Empty` or a parent re-render remounts** an existing swap-chain host. If the painted content must change, the host key (or its parent strip key) must change — or clear/replace inside the installer.

When touching popup footer tabs/buttons or `icons::element` / `acrylic::install_*`, re-check that refresh/settings/quit stay distinct and that provider tabs keep the correct marks after enabling/disabling providers and after theme flips.

https://github.com/steipete/CodexBar - similar app for macos. You can use it as a reference for the features and implementations.

https://github.com/pingdotgg/t3code - T3 Code. Use it as a reference for the Usage tab, session-log scanning, and hourly/daily aggregation. Their scanner lives in `apps/server/src/usage/` (`UsageService`, `usageTranscripts`, `usageTranscriptReader`) and reads provider CLI transcripts (not T3's own orchestration), same approach as `ccusage`.

https://github.com/janekbaraniewski/openusage - OpenUsage. Local-first usage/quota dashboard; use it for Claude/Codex transcript conventions (dedupe, session logs, provider detection).

When you need icons, download them from iocnify. Icon pack - Phosphor Icons for settings window, Fluent UI Icons for the popup icons

All layout shifts have to be animated, respect the user's preference for animations.
