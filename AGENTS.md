Always run `cargo check` after changes related to the app to ensure the code is correct. No need to run it after changes related to the website.

Never launch the app itself.

All settings must take effect immediately in the running application. The user must never need to relaunch the app for a setting change to be applied. Keep every open UI surface and affected background component synchronized with the updated settings.

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

When you need icons, download them from iocnify. Icon pack - Phosphor Icons for settings window, Fluent UI Icons for the popup icons
