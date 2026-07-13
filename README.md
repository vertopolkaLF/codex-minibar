<p align="center">
  <img src="assets/app-icon.png" alt="Codex Minibar logo" width="256" height="256">
</p>

<h1 align="center">Codex Minibar</h1>

<p align="center">
  <b>Free, open-source Windows tray companion for Codex rate limits with configurable tray widgets, a compact usage popup, notifications, auto-start, in-place updates, and local history.</b>
</p>

<p align="center">
  <a href="https://github.com/vertopolkaLF/codex-minibar/releases">Download</a>
  |
  <a href="https://vertopolkalf.github.io/codex-minibar/">Website</a>
  |
  <a href="https://github.com/vertopolkaLF/codex-minibar/issues">Issues</a>
</p>

<p align="center">
  <img src="https://img.shields.io/github/downloads/vertopolkalf/codex-minibar/total?style=flat-square" alt="Downloads">
  <img src="https://img.shields.io/badge/platform-Windows%2010%20%2F%2011-blue?style=flat-square" alt="Platform">
  <img src="https://img.shields.io/badge/Rust-2024-orange?style=flat-square" alt="Rust edition">
  <img src="https://img.shields.io/badge/UI-WinUI%203-green?style=flat-square" alt="UI framework">
</p>

---

## Overview

Codex Minibar reads the usage data exposed by a locally installed, authenticated Codex CLI/Desktop installation and keeps your five-hour and weekly limits visible in the notification area. It is a native WinUI 3 application written in Rust.

> Codex Minibar is an independent project. It is not affiliated with, endorsed by, or sponsored by OpenAI.

## Features

- Show five-hour and weekly usage in one or more configurable tray icons.
- Choose numbers, bars, rings, reset times, or reset countdowns; show remaining or used
  percentage as appropriate.
- Open a compact native popup for the current plan, credits, limit windows, and usage
  history.
- Receive Windows notifications when a limit resets, usage becomes low, Codex cannot be
  reached, or an update is available.
- Optionally start Codex automatically to activate a fresh five-hour window.
- Start with Windows, update in place from GitHub Releases, and retain history locally.
- Detect Codex installations automatically, with an override for a custom executable path.

## Requirements

- Windows 10 or Windows 11 (64-bit ARM or x64).
- A locally installed and authenticated Codex CLI or Codex desktop application.

The app does not ask for, store, or transmit your Codex credentials. It talks to the local
Codex app server and stores its own settings and usage history in your Windows user profile.

## Install

1. Open the [latest release](https://github.com/vertopolkaLF/codex-minibar/releases/latest).
2. Download the installer matching your Windows architecture (`x64` or `arm64`) and run it.
   The installer is per-user and does not require administrator rights.
3. Alternatively, download the matching `portable.zip`, extract it, and run
   `codex-minibar.exe`.
4. Find the icon in the notification area. If it is hidden, Windows may have tucked it under
   the `^` overflow menu, because apparently that is where delightful UX goes to die.

On first run, Codex Minibar discovers Codex automatically. Open **Settings** from the tray
menu if you need to choose another executable or adjust the tray widgets and notifications.

## Updating

By default, Codex Minibar checks GitHub Releases for updates and can install a matching
portable package in place. You can disable update checks in Settings at any time.

## Build from source

Install the Rust toolchain pinned in [`rust-toolchain.toml`](rust-toolchain.toml), then run:

```powershell
cargo check --locked
cargo test --all-targets --all-features --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
```

To build distributable Windows packages, run:

```powershell
.\build.ps1
```

This produces architecture-specific portable ZIP files and NSIS installers under `dist/`.

## Development

The UI uses [windows-reactor](https://github.com/microsoft/windows-rs/pull/4479) with WinUI 3;
the Windows App SDK runtime is bundled through `windows-reactor-setup` self-contained deployment.
CI checks formatting, lints, tests, and a release build on Windows.

Bug reports and focused pull requests are welcome. Please include your Windows version,
Codex installation type, and clear reproduction steps when reporting a problem.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
