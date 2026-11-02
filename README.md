<p align="center">
  <img src="assets/app-icon.png" alt="Codex Minibar logo" width="256" height="256">
</p>

<h1 align="center">Codex Minibar</h1>

<p align="center">
  <b>Free, open-source Windows and Linux tray companion for AI subscription usage limits with configurable tray widgets, a compact usage popup, notifications, auto-start, in-place updates, and local history.</b>
</p>

<p align="center">
  <a href="https://github.com/vertopolkaLF/codex-minibar/releases">Download</a>
  |
  <a href="https://vertopolkalf.ru/codex-minibar/">Website</a>
  |
  <a href="https://github.com/vertopolkaLF/codex-minibar/issues">Issues</a>
</p>

<p align="center">
  <img src="https://img.shields.io/github/downloads/vertopolkalf/codex-minibar/total?style=flat-square" alt="Downloads">
  <img src="https://img.shields.io/badge/platform-Windows%2010%20%2F%2011%20%7C%20Linux-blue?style=flat-square" alt="Platform">
  <img src="https://img.shields.io/badge/Rust-2024-orange?style=flat-square" alt="Rust edition">
  <img src="https://img.shields.io/badge/UI-GPUI-green?style=flat-square" alt="UI framework">
</p>

---

## Overview

Codex Minibar reads quota data from locally authenticated provider sessions and keeps subscription limits and reset times visible in the notification area. It is a native Windows application written in Rust, with the tray popup and Settings both rendered by GPUI.

> Codex Minibar is an independent project. It is not affiliated with, endorsed by, or sponsored by OpenAI.

<p align="center">
  <img src="docs/media/1.0.0/home.png" alt="Codex Minibar popup with Home, Usage and a provider tab">
</p>

## Features

### Every subscription in one popup

Click the tray icon for a native popup with the current plan, five-hour and weekly windows,
credits, reset countdowns and usage pace of every enabled provider: Codex, Claude, Cursor,
OpenCode Zen, OpenCode Go, OpenRouter, Antigravity, Grok, Kiro, and Command Code. Providers are
enabled independently in **Settings → Providers** and refresh independently. Antigravity, Grok,
Kiro and Command Code show subscription quota rather than unrelated API billing.

### Several accounts and your own Home

Add several accounts of any provider, each with its own name, badge color, refresh interval
and tab. Edit Home to choose Cards, Lines, Rings or Tiles per provider, reorder blocks and pick
what shows on Home.

<p align="center">
  <img src="docs/media/1.0.0/accounts.webp" alt="Several accounts and Home layouts">
</p>

### Usage and cost analytics

The Usage tab scans local provider history for animated cost and token charts, per-model
breakdowns, provider rankings and a spend donut on Home. Everything stays on your PC.

<p align="center">
  <img src="docs/media/1.0.0/usage.webp" alt="Usage tab with cost and token charts">
</p>

### Notifications that show the quota

Animated in-app notifications with sounds arrive when a limit resets, usage runs low, an
activation finishes or an update is ready. Limit alerts show the quota they are about. Any
notification can be turned off in **Settings → Notifications**.

<p align="center">
  <img src="docs/media/1.0.0/notifications.webp" alt="Limit and update notifications">
</p>

### Themes

Fluent, Vercel, Apple, or any installed VS Code color theme with contrast and tint controls.
Pick an accent color, a font and an independent popup border. Auto follows the Windows theme.

<p align="center">
  <img src="docs/media/1.0.0/themes.webp" alt="Popup themes">
</p>

### Nine languages

English, Русский, Deutsch, Français, Español, Português (Brasil), 日本語, 简体中文, 繁體中文 and 한국어.
Auto follows the Windows display language, including Traditional Chinese in Taiwan,
Hong Kong and Macao.

<p align="center">
  <img src="docs/media/1.0.0/languages.webp" alt="Popup in different languages">
</p>

### OpenRouter keys

See spending per key and manage OpenRouter API keys from the popup's OpenRouter tab.

<p align="center">
  <img src="docs/media/1.0.0/openrouter.webp" alt="OpenRouter key management">
</p>

### More features

- Configurable tray icons: numbers, bars, rings, reset times or reset countdowns, showing
  remaining or used percentage. Use several icons for several providers.
- Taskbar widgets drawn inside the Windows taskbar beside the clock, in the taskbar's own style:
  a weather-button glance, progress rings, nested rings, dials, bars or level
  meters, with a pace mark on the graphics. Hover one for a flyout with every quota and its reset
  time; click it to open the popup on Home or that provider's tab, at that end of the taskbar.
- Ongoing Codex, Claude and Cursor incidents from their public status pages, with an optional
  notification when a new incident starts. Only components that affect each provider's coding
  tools count.
- Redeem banked Codex and Claude resets from the popup.
- Announced Codex forced-reset countdowns from the public reset feed (Tibo Resets). The feed
  contract and bot update rules live in [`docs/codex-resets-feed.md`](docs/codex-resets-feed.md).
- Optionally start Codex automatically to activate a fresh five-hour window, plan activations
  ahead, and set provider-specific quiet periods, such as keeping a work Claude session inactive
  on weekends.
- Usage pace indicators show whether you are burning a window faster than it resets.
- Kiro's monthly included credits and reset date, and Claude cloud-session credit balances.
- Command Code's five-hour, weekly and monthly credit windows, plus any purchased top-up balance.
- Warnings before a Claude login stops renewing.
- An optional Stream Deck companion with quota keys that open the popup or a provider tab and
  launch Minibar when it is not running.
- An optional global keyboard shortcut that shows or hides the popup from anywhere.
- `codex-minibar status` prints the running app's quotas as text, JSON, a Waybar module or a
  Polybar/i3blocks line, for status bars and scripts. See [Status bars and scripts](#status-bars-and-scripts).
- Start with Windows, update in place from GitHub Releases, and keep usage history locally.
- Automatic Codex discovery with an override for a custom executable path.
- Settings import and export, and a live software rendering option for systems where GPU
  rendering misbehaves.
- An AI troubleshooting handoff: `minibar trouble`, or **Settings → Log → Run Troubleshoot
  with AI**.

## Requirements

- Windows 10 or Windows 11 (64-bit ARM or x64), or a 64-bit Linux desktop (x64) with a
  system tray (see [Linux](#linux)).
- A locally installed and authenticated supported provider, or an OpenRouter API key.

The app does not copy provider credentials into its ordinary settings file. It talks to the local
Codex app server, reads Claude Code's existing local OAuth session, reads OpenCode's local
configuration/history, reuses the official Antigravity or Grok CLI sign-in, requests live Kiro
monthly credits with the shared Kiro sign-in and detects the Kiro IDE, Kiro Crew, and Kiro CLI as
available sources, or requests OpenRouter key usage. The Crew app is detected for per-user and
all-users installs and shares the same Kiro credit provider rather than adding a duplicate balance
card. The shared Kiro access token is read-only and never refreshed or written. The live endpoint
also supplies plan and account labels. If it is unavailable, Minibar falls back to the IDE's local
usage cache when available and keeps its own last stored Kiro snapshot. The endpoint is currently
undocumented by Kiro and may change. Kiro subscription credits stay separate from API-equivalent
Total Spend.
Optional OpenCode and OpenRouter manual API keys and saved Codex sessions and Claude profile credentials are protected with Windows user-scoped DPAPI
storage. The app stores its own settings and usage history in your Windows user profile.

Kiro's plan appears on its provider card. The account label uses Kiro's display name when available,
then its email address, and follows the **Show account name** preference. Redacted values are hidden.

For Antigravity, run `agy` and complete its normal sign-in once; Minibar reads that existing
session from Windows Credential Manager, or from the login keyring (Secret Service) on Linux,
and never stores it in app settings. For Grok, run `grok login`;
Minibar reads the official CLI auth cache. If either session expires, refresh it in the official CLI.
For Command Code, run `cmdc login`; Minibar reads the API key the CLI stores in
`~/.commandcode/auth.json` and only calls the CLI's own read-only billing endpoints. The monthly
row is sized from the reported grant or the plan's published allowance, so an unrecognized plan
shows only the rolling windows. Usage statistics come from the CLI's local session logs in
`~/.commandcode/projects`, using the credit cost the CLI recorded for each response; transcripts
written by CLI versions that did not record token usage contribute nothing.
Browser-session access is not used.

### Add another Codex account

Open **Settings → Providers → Codex → Add account**, optionally name it, and click
**Sign in**. Finish authorization with the desired ChatGPT account in your browser
within five minutes. Requires native Codex CLI or Codex desktop; npm installations
are supported when their package contains the native Windows executable.

**Default** follows your existing Codex login (including `CODEX_HOME`). Additional
accounts use isolated logins, stored with Windows user-scoped DPAPI, and refresh
automatically. Adding or selecting an account does not switch your CLI or desktop
login. Each enabled account has independent limits and errors. Rename, disable,
remove, or sign in again from its account card; **Show on Home** affects only Home.
The Codex tab has an account switcher, or enable **Show accounts as separate tabs**
to give each Codex and Claude account its own numbered icon.

Tray quotas follow the provider-level account sample. Automatic and scheduled
activation use only the existing Default login; saved accounts are never activated.
Usage statistics still come from local CLI logs and retain their existing account
attribution; adding OAuth accounts does not create historical usage for them.

### Add another Claude account

Open **Settings → Providers → Claude → Add account**, optionally give the account a name,
and choose a connection method. The dialog includes instructions and clickable help links.
The built-in **Default** profile follows this PC's Claude Code or desktop login;
you can turn it off without removing your other profiles.
An empty name becomes the first available **Account 1**, **Account 2**, and so on;
you can rename it later.

- **Sign in**: requires native Windows Claude Code. Click **Sign in**, choose the
  subscription account in your browser, and complete authorization within five minutes.
  Minibar uses an isolated temporary configuration directory, keeps the complete session
  in Windows user-scoped encrypted storage, and refreshes its tokens automatically.
  Your current CLI and Desktop logins stay in place. **Cancel** stops the pending login.
  If the browser returns a code that needs a terminal, use the manual OAuth token method.
- **Browser session**: open [claude.ai](https://claude.ai) in a separate
  browser profile or private window and sign in to the second account. In Chrome or Edge,
  press **F12 → Application → Storage → Cookies → https://claude.ai**. Find `sessionKey`,
  copy its **Value**, and paste it into Minibar. See [Chrome's cookie guide](https://developer.chrome.com/docs/devtools/application/cookies).
- **OAuth token**: sign in to Claude Code with a separate configuration directory.
  See [Claude Code's multiple-account instructions](https://code.claude.com/docs/en/authentication#log-in-with-multiple-accounts).
  In a separate PowerShell window, run:

  ```powershell
  $env:CLAUDE_CONFIG_DIR = Join-Path $env:USERPROFILE '.claude-minibar-work'
  claude
  ```

  After signing in to the second subscription account, open
  `%USERPROFILE%\.claude-minibar-work\.credentials.json` and copy only
  `claudeAiOauth.accessToken`, without quotes. It starts with `sk-ant-oat`.
  If that file is unavailable, use Browser session. `claude setup-token` may lack usage
  access and is not a substitute for this login token.

For pasted credentials, click **Check and save**. Home shows the enabled profiles; the Claude tab lets you
switch between them. Expand the account card to change its name, toggle **Show on Home**,
or use **Update credential** when a cookie or token expires:
Minibar keeps the profile's name and enabled state. Pasted OAuth tokens are not
automatically refreshed. API keys and Admin API keys are not offered for subscription
account setup; organization API spending is a different metric.

Adding an account keeps the last successful limits for unchanged profiles while
the reader refreshes. If a refresh fails, those limits remain visible with their
original update time and an error. Removing, disabling, or replacing a profile's
credential clears only its sample. A partial HTTP 429 still pauses Claude polling.

## Install

1. Open the [latest release](https://github.com/vertopolkaLF/codex-minibar/releases/latest).
2. Download the installer matching your Windows architecture (`x64` or `arm64`) and run it.
   The installer is per-user and does not require administrator rights.
3. Alternatively, download the matching `portable.zip`, extract it, and run
   `codex-minibar.exe`.
   The NSIS installer also registers `minibar` as a launch alias for the same app.
4. Find the icon in the notification area. If it is hidden, Windows may have tucked it under
   the `^` overflow menu, because apparently that is where delightful UX goes to die.

On first run, Codex Minibar discovers Codex automatically. Open **Settings** from the tray
menu if you need to choose another executable or adjust the tray widgets and notifications.

To start the troubleshooting handoff, run `codex-minibar trouble` or `minibar trouble`.
The command scans for Codex and Claude Code, asks which one should investigate, then opens
a terminal with the shared Codex Minibar troubleshooting prompt. The same handoff is
available from **Settings → Log → Run Troubleshoot with AI**.

## Linux

Codex Minibar also runs on Linux desktops under Wayland and X11. Download from the
[latest release](https://github.com/vertopolkaLF/codex-minibar/releases/latest):

- **Debian/Ubuntu:** `sudo apt install ./codex-minibar_<version>_amd64.deb`
- **Any distribution, no install:** make `codex-minibar-<version>-linux-x64.AppImage` executable
  and run it.
- **Fedora/openSUSE:** `sudo dnf install ./codex-minibar-<version>-1.x86_64.rpm`
  (or `sudo zypper install ...`).
- **Arch Linux:** build the `PKGBUILD` attached to each release with `makepkg -si`
  (it packages the release tarball as `codex-minibar-bin`).
- **Other distributions:** extract `codex-minibar-<version>-linux-x64.tar.gz` and run
  `./install.sh` to install into `~/.local`, or run `bin/codex-minibar` in place.

The tray icon uses the StatusNotifierItem protocol. KDE Plasma, Xfce, Cinnamon, Budgie and
most panels show it out of the box. On GNOME, enable the **AppIndicator and KStatusNotifierItem
Support** extension (Ubuntu ships it enabled). Without a tray, launch `codex-minibar` again
to open the popup.

Click the tray icon to toggle the popup. On panels that open a menu instead (such as
GNOME AppIndicator), choose **Show** to open the popup or **Settings** to open Settings.
Closing the popup or Settings leaves the app running in the tray.

Differences from Windows:

- **Popup placement.** The popup is a floating panel by default. On X11, **Settings → Customize →
  Popup position** picks one of six spots on the screen. Wayland does not let apps place their
  windows, so the desktop positions it there and that option is disabled. The popup closes when it loses focus or
  you press Esc, unless it is pinned.
- **Global shortcut.** On GNOME-based desktops (GNOME, Ubuntu, Pop!_OS, Budgie), **Settings →
  General → Popup shortcut** opens a dialog: press the combination you want and choose
  **Save**. Minibar writes it into the desktop's own keyboard settings as a custom shortcut
  running `codex-minibar --toggle`, so it appears in the desktop's shortcut list and the
  desktop checks it for conflicts. Removing it deletes that entry. No shortcut is installed
  until you set one. On other desktops, add a custom shortcut running `codex-minibar --toggle`
  yourself; the same settings row shows the exact command and a copy button.
- **Secrets.** API keys and saved sessions are encrypted with a key kept in your login keyring
  (Secret Service). Without a keyring service the key is kept in a file only you can read.
- **Notifications** go to your desktop's notification server and follow its Do Not Disturb.
- **Game mode** detects games launched by Steam and games installed in Steam or Heroic
  libraries.
- **Updates** install in place for the AppImage and the tarball, when their location is
  writable. The `.deb` links to the release page; install it with your package manager.
- **Floating panel.** Enable it in Settings → Customize to keep the popup open when
  switching apps and drag its header to move it. Wayland lets the compositor place
  new windows, so saved panel positions are only available on Windows. On X11,
  the pin also requests Always on Top from the window manager. On Wayland, it is
  labeled Keep open: stacking is controlled by the desktop. Right-click the panel
  header to open the native window menu and select Always on Top where available
  (including GNOME). This desktop-owned state is separate from the Keep open setting.
- **Panel widgets instead of taskbar widgets.** The packages install a GNOME Shell extension
  and a Plasma 6 widget that show your quotas in the panel. See
  [Panel widgets](#panel-widgets).
- **Windows-only features:** taskbar widgets, the Command Palette and Stream Deck integrations,
  Acrylic/Mica backgrounds and reading the Claude desktop app's token cache.

### Panel widgets

Both widgets read the running app, show each enabled provider's default quotas in the app's
language and remaining/used preference, and color a quota that is nearly used up. Click a
provider in their menus to open the popup on its tab.

- **GNOME Shell (45 and later):** the `.deb` and `.rpm` install the extension system-wide and
  `install.sh` installs it into `~/.local`. Log out and back in once, then run
  `gnome-extensions enable codex-minibar@vertopolkalf.github.io` or turn on **Codex Minibar**
  in the Extensions app. From a source checkout, copy
  `integrations/gnome-shell/codex-minibar@vertopolkalf.github.io` into
  `~/.local/share/gnome-shell/extensions/`. It updates the moment a quota changes.
- **KDE Plasma 6:** right-click the panel, choose **Add or Manage Widgets** and add
  **Codex Minibar**. From a source checkout, run
  `kpackagetool6 --type Plasma/Applet --install integrations/kde-plasma/io.github.vertopolkalf.codexminibar`.
  The widget runs `codex-minibar status --json` every 30 seconds; for an AppImage, set its
  path under **Configure Codex Minibar**. Middle-click the widget to toggle the popup.

Other clients can use the app's session D-Bus service `io.github.vertopolkalf.CodexMinibar`
(object `/io/github/vertopolkalf/CodexMinibar`, interface
`io.github.vertopolkalf.CodexMinibar1`): `Status()` returns the same JSON as
`codex-minibar status --json`, the `Changed(s)` signal carries it whenever it changes, and
`Toggle(s activation_token)`, `ShowProvider(s id)` and `Refresh()` control the app.

```bash
gdbus call --session --dest io.github.vertopolkalf.CodexMinibar --object-path /io/github/vertopolkalf/CodexMinibar --method io.github.vertopolkalf.CodexMinibar1.Status
```

### Status bars and scripts

`codex-minibar status` asks the running app for its quotas and prints them. It works on
Windows too, but is most useful with Linux status bars such as Waybar, Polybar and i3blocks.

```text
codex-minibar status [--format text|json|waybar|polybar] [--provider ID]... [--watch [SECONDS]]
                     [--warning PERCENT] [--critical PERCENT]
```

- `--format text` (default) prints a block per provider with every quota and its reset.
- `--json` prints the full report, including raw percentages and reset times.
- `--waybar` prints a [Waybar custom module](https://github.com/Alexays/Waybar/wiki/Module:-Custom)
  object: one-line text, a tooltip with every quota, a `percentage` of the most used default
  quota and a `normal`, `warning`, `critical` or `offline` class.
- `--polybar` prints one plain line, for Polybar, i3blocks or a tmux status line.
- `--provider` limits the output to providers or accounts by id (`codex`, `claude`, `claude-2`).
- `--watch` keeps printing a new line every 30 seconds, or the given interval, so a bar
  needs no polling of its own.

Waybar:

```json
"custom/minibar": {
    "exec": "codex-minibar status --waybar --watch",
    "return-type": "json"
}
```

```css
#custom-minibar.warning { color: #f6d32d; }
#custom-minibar.critical { color: #ff7b63; }
```

Polybar:

```ini
[module/minibar]
type = custom/script
exec = codex-minibar status --polybar --watch
tail = true
```

Without a running app, text and JSON exit with status 1, and the Waybar and Polybar formats
print an empty module so the bar hides it.

## Updating

By default, Codex Minibar checks GitHub Releases for updates and can install a matching
portable package in place. You can disable update checks in Settings at any time.

## Build from source

Install the Rust toolchain pinned in [`rust-toolchain.toml`](rust-toolchain.toml), then run:

```powershell
.\cargo-worktree.ps1 check --locked
.\cargo-worktree.ps1 test --all-targets --all-features --locked
.\cargo-worktree.ps1 clippy --all-targets --all-features --locked '--' -D warnings
```

The wrapper runs Cargo in the current Git worktree and shares the main checkout's
existing `target` directory. Dependencies can be reused across worktrees with
matching build settings; simultaneous builds wait for Cargo's lock. The shared
binary belongs to the last build. Set `CARGO_TARGET_DIR` explicitly when you need
isolated build output. From an older worktree, invoke the main checkout's
`cargo-worktree.ps1` by absolute path. Plain Cargo commands remain suitable for CI.
Quote the `'--'` separator when passing compiler, Clippy or rustfmt flags through
the PowerShell wrapper.

To build distributable Windows packages, run:

```powershell
.\build.ps1
```

This produces architecture-specific portable ZIP files and NSIS installers under `dist/`.

On Linux, install the build dependencies (Debian/Ubuntu shown) and use plain Cargo:

```sh
sudo apt install build-essential pkg-config libxkbcommon-dev libxkbcommon-x11-dev \
  libwayland-dev libx11-dev libx11-xcb-dev libxcb1-dev libxcb-xkb-dev libvulkan-dev \
  libfontconfig-dev libfreetype-dev libdbus-1-dev libssl-dev libzstd-dev patchelf
cargo test --all-targets --all-features --locked
./build-linux.sh   # dist/<version>/: tarball, .deb and AppImage
```

## Development

Every window is rendered with [GPUI](https://gpui.rs) on one dedicated UI thread: the tray
popup, the Settings window and the first-launch onboarding. Settings controls (toggles,
dropdowns, sliders, text inputs, dialogs) live in `src/settings_window/kit.rs` and
`src/settings_window/input.rs`. No UI framework runtime is redistributed; the release
package is the executable plus its `assets` folder.
CI checks formatting, lints, and tests on Windows and Linux. Linux desktop integration
(tray, notifications, portal settings, keyring, autostart, single instance, the D-Bus status
service) lives in `src/linux/`. `codex-minibar status` is `src/status.rs`; it and the D-Bus
service share one report, served to the CLI over the same loopback bridge as Stream Deck
(`src/streamdeck.rs`). The GNOME Shell extension and Plasma widget live in `integrations/`.

### Stream Deck companion

The optional companion lives in [`streamdeck/`](streamdeck/). It stores each key's
configuration in Stream Deck and reads sanitized quota snapshots from the running
Minibar process over loopback. With Node.js 24+ installed, build and package it with:

Each key can be configured as a single-limit ring for 5h or Weekly, with a
custom reset display, or as a combined 5h + Weekly widget.

```powershell
cd streamdeck
npm install
npm run build
npx @elgato/cli@latest validate .\com.vertopolkalf.codex-minibar.sdPlugin
npx @elgato/cli@latest pack .\com.vertopolkalf.codex-minibar.sdPlugin
```

The resulting `.streamDeckPlugin` file can be opened with Stream Deck Desktop.

Bug reports and focused pull requests are welcome. Please include your Windows version,
Codex installation type, and clear reproduction steps when reporting a problem.

## Contributing

We especially need help supporting the many different providers people use. Provider APIs,
local data formats, authentication flows, and quota semantics all vary, so testing integrations
with real provider accounts and keeping them working over time is particularly valuable.

To contribute:

1. Fork the repository and create a focused branch.
2. Make the smallest change that solves the problem.
3. Run the relevant checks from [Build from source](#build-from-source).
4. Open a pull request with a clear description, reproduction steps, and provider-specific
   setup details when applicable.

For provider changes, include sanitized sample data or fixtures when possible. Never commit
credentials, tokens, or personal usage history.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
