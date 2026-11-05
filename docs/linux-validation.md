# Linux desktop validation

This is a release test procedure, not a list of verified desktop versions.
Unit tests, compilation and Clippy do not verify compositor placement, focus,
panel rendering or browser activation. Record actual results before describing
a desktop/session combination as tested. The coding workflow must not launch
the application; these interactive checks are for a separate manual test run.

## Session matrix

| Session | Integration to exercise | Placement and focus expectations |
| --- | --- | --- |
| GNOME Wayland, without AppIndicator | Launcher, custom shortcut, notification actions, optional GNOME widget | Reachable without a tray; compositor chooses placement |
| GNOME Wayland, with AppIndicator | Tray menu, Show, Settings, custom shortcut | Show focuses the popup using the desktop activation grant where supplied |
| Plasma 6 Wayland | StatusNotifierItem, Plasma widget, custom desktop shortcut | Keep open preserves the panel; stacking belongs to the compositor |
| Plasma 6 X11 | StatusNotifierItem, Plasma widget | Configured popup placement and Always on Top work |
| Xfce or Cinnamon X11 | Native tray host, notification server, manual shortcut | Popup opens, dismisses and reopens without exiting the app |
| Sway or another wlroots compositor | Configured StatusNotifierItem panel and manual shortcut | No application-controlled placement or global key grab is required |

Record distribution, desktop/compositor version, X11/Wayland backend actually
selected by GPUI, portal backend, package type, display scales and panel location.
An X11 application inside a Wayland session must be recorded as X11/XWayland.

## Required checks in each session

1. With onboarding completed, start through `--toggle` while no instance runs.
   The popup should open on that first request. Repeat while it runs: the same
   instance should toggle rather than start another process. On a fresh profile,
   onboarding takes priority. Launch a fresh profile from the desktop launcher
   too: onboarding must receive the launcher's focus grant under Wayland.
   With onboarding completed, verify the ordinary launcher opens the popup on
   a cold start and brings the app forward when it is already running. Launching
   it while the popup is visible must not close the popup. Autostart without
   `--show` or `--toggle` must remain in the background.
2. Close every window, then reopen from the tray or shortcut. Settings and the
   popup must remain usable after several cycles. Test Escape, outside clicks,
   Keep open and opening Settings while the popup is visible.
3. Test popup-first and Settings-first cold starts with Auto, Light and Dark.
   Change the desktop theme and accent while windows are open. Disable system
   animations and verify reduced motion without restarting the app.
4. Test a second monitor, mixed scales, a fractional scale, a top panel and a
   bottom panel. Check text, icons, pointer targets, scrolling and usable window
   bounds. Under Wayland, judge whether the compositor's placement is usable;
   do not require a saved absolute position.
5. Trigger a notification and its actions. Check plain-text/markup rendering,
   silent notifications, replacement of progress notifications and desktop Do
   Not Disturb. Restart the notification daemon and verify subsequent delivery.
6. Enable and disable Start at login, then test a new login session. For an
   AppImage, the entry must reference the AppImage rather than its temporary mount.
7. Suspend and resume. With logind/elogind, quotas should refresh after resume.
   An existing HTTP 429 cooldown must still apply. Do not induce rate limiting
   against a real provider merely to test this; the worker regression test covers it.

## Installation and optional-service checks

- Test a tarball prefix containing spaces and shell metacharacters. Its launcher
  and Toggle action must work without adding the prefix's `bin` to session PATH,
  including after an in-place update. Validate the installed desktop entry with
  `desktop-file-validate`.
- Test AppImage updates and distribution-managed installs separately. Package
  manager installations must not be replaced by the in-place updater.
- For a direct portable-binary launch without an installed desktop entry,
  test moving the binary and launching it again. Newly generated entries marked
  `X-CodexMinibar-Managed=true` should update both commands. Unmarked entries
  (including legacy generated entries) and symlinks must remain untouched.
  Removing the marker opts a generated entry out of automatic maintenance.
- In a disposable test session, cover absent portals, absent Secret Service and
  a locked keyring. Missing optional services must not prevent reaching Settings.
  Previously saved secrets must not be silently replaced when their key is unavailable.
- Test a CLI login cancellation and timeout with a fixture CLI. Its descendants
  must exit. Use fixtures rather than launching real browser login flows repeatedly.
- With a fixture npm layout under a Node installation and a test desktop `PATH`
  containing no Node runtime, select its CLI explicitly. Test background requests,
  sign-in and the troubleshooting terminal through both the npm launcher symlink
  and its canonical package path. They must use that installation's Node runtime.
- Built-in shortcut installation is gated on a GNOME session identifier and a
  working GNOME media-keys schema. Other sessions must show the manual command;
  merely having GNOME packages installed does not imply shortcut integration.

## Automated checks

Run the Linux commands from `AGENTS.md`: format, `cargo check --locked`, the full
test suite, Linux Clippy and Windows cross-target Clippy. Tests cover command
escaping, process cleanup, portal preference decoding, activation-token handling,
resume-signal decoding, worker cooldown preservation and private bridge-file
publication. They complement the session matrix; they do not replace it.
