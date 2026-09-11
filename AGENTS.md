Always check GPUI and gpui-component docs and examples before doing UI work.

Always run `cargo check` after changes related to the app to ensure the code is correct. No need to run it after changes related to the website.

Never launch the app itself.

All settings must take effect immediately in the running application. The user must never need to relaunch the app for a setting change to be applied. Keep every open UI surface and affected background component synchronized with the updated settings.

## Appearance initialization guardrails

Theme and accent settings must be applied during application startup, before any window is shown. Do not rely on opening the Settings window, mounting an Appearance page, or a later rerender to initialize global appearance resources.

Apply `Theme::change` and the accent color on the GPUI app before `open_window`. Repeat the mapping when the user changes theme/accent and when `window_appearance` changes for Auto theme. Popup mica/acrylic and corner radius are native window attributes; apply them from the popup render path, but skip redundant DWM calls when the fingerprint is unchanged.

Before finishing appearance work, review both cold-start paths (popup first and Settings first) for Auto, Light, and Dark themes. Opening or closing another window must never be required to correct colors. Always run `cargo check`; never launch the app for this verification.

## Provider UI guardrails

When adding or changing a provider, update every provider enumeration, settings migration, worker lifecycle, popup state, tray source and provider-tab path as one atomic change. Build the popup tab strip from only live enabled providers so an empty slot cannot steal a neighbor's icon or label.

Before finishing provider UI work, verify all enabled-provider combinations (each provider alone, every pair, all providers, and none) in code review. Confirm that every visible tab has the right label/icon and points to the matching provider snapshot. Always run `cargo check` after the change; never launch the app to perform that verification.

https://github.com/steipete/CodexBar - similar app for macos. You can use it as a reference for the features and implementations.

https://github.com/pingdotgg/t3code - T3 Code. Use it as a reference for the Usage tab, session-log scanning, and hourly/daily aggregation. Their scanner lives in `apps/server/src/usage/` (`UsageService`, `usageTranscripts`, `usageTranscriptReader`) and reads provider CLI transcripts (not T3's own orchestration), same approach as `ccusage`.

https://github.com/janekbaraniewski/openusage - OpenUsage. Local-first usage/quota dashboard; use it for Claude/Codex transcript conventions (dedupe, session logs, provider detection).

When you need icons, download them from iocnify. Icon pack - Phosphor Icons for settings window, Fluent UI Icons for the popup icons

All layout shifts have to be animated, respect the user's preference for animations.
