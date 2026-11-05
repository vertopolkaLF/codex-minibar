# Linux panel widgets

Thin clients of the running Codex Minibar app. They hold no credentials and show only
what the app reports.

- `gnome-shell/codex-minibar@vertopolkalf.github.io`: GNOME Shell 45+ top-bar indicator.
  Talks to the app's session D-Bus service (`src/linux/dbus.rs`) and updates on its
  `Changed` signal.
- `kde-plasma/io.github.vertopolkalf.codexminibar`: Plasma 6 widget. Reads the
  D-Bus status on a timer, falling back to `codex-minibar status --json`
  (`src/status.rs`), and calls the D-Bus service with `dbus-send` for its actions.

`build-linux.sh` installs both into the `.deb`, `.rpm` and tarball. Installation and the
D-Bus interface are described in the main README under
[Panel widgets](../README.md#panel-widgets).
