Always run `cargo check` after changes related to the app to ensure the code is correct. No need to run it after changes related to the website.

Never launch the app itself.

All settings must take effect immediately in the running application. The user must never need to relaunch the app for a setting change to be applied. Keep every open UI surface and affected background component synchronized with the updated settings.

## Provider UI guardrails

When adding or changing a provider, update every provider enumeration, settings migration, worker lifecycle, popup state, tray source and provider-tab path as one atomic change. Native popup provider tabs use swap-chain icon hosts: key the complete tab selector by the enabled-provider set so reconciliation cannot reuse an old provider's text or icon in another provider's slot. Do not treat `Element::Empty` as sufficient remounting.

Before finishing provider UI work, verify all enabled-provider combinations (each provider alone, every pair, all providers, and none) in code review. Confirm that every visible tab has the right label/icon and points to the matching provider snapshot. Always run `cargo check` after the change; never launch the app to perform that verification.

https://github.com/steipete/CodexBar - similar app for macos. You can use it as a reference for the features and implementations.

When you need icons, download them from iocnify. Icon pack - Phosphor Icons for settings window, Fluent UI Icons for the popup icons
