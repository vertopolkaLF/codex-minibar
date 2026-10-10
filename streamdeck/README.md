# Codex Minibar Stream Deck companion

This is a thin Stream Deck plugin. It stores each key's configuration in the
Stream Deck Property Inspector and reads sanitized quota snapshots from the
running Codex Minibar process over loopback.

Each key selects one provider instance. The Property Inspector lists the
enabled instances reported by Minibar (plus any instance a key already
references, marked disabled or unavailable); while Minibar is not running it
falls back to the built-in providers. A removed or disabled instance stays
unavailable instead of showing another instance's quota. When a driver has
several enabled instances, logo watermarks carry the instance badge.

Keys saved before provider instances migrate automatically: a Claude/Codex
key on the Default account selects the primary instance (the driver id) and
any other account selects the instance that kept its profile id.

The bridge reports instance ids in `id`, the driver in `kind`, and the
instance `badge`. Snapshots contain quota data and display names, never
authentication credentials.

## Development

From this directory:

```powershell
npm install
npm run build
npm test
npx streamdeck dev
```

To build, link the plugin into Stream Deck, and restart it in one command:

```powershell
npm run install:reload
```

The current plugin uses the official Elgato SDK and renders dynamic SVG key
images. Each key offers provider-specific widgets: a single 5-hour/weekly
limit with configurable reset display, or a combined 5-hour + weekly view.
The style selector includes progress rings, a solid status-color background,
horizontal and vertical status-colored progress bars, a left rail with
left-aligned info, stacked numbers/bars, and reset-only views. The solid and
bar styles keep the selected reset time or countdown at the bottom when it is
enabled. Tap a key to run its configured popup action; while Minibar is running,
hold it for 0.7 seconds to request a background refresh of all provider data
without opening the popup.
The Minibar process writes `streamdeck-bridge.json` beside its settings file
while running. Tapping a key while Minibar is stopped launches the normal
per-user installation and the plugin reconnects on its next refresh.
GitHub Actions includes the packaged `.streamDeckPlugin` companion alongside
the Windows builds in each draft release.
