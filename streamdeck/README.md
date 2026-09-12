# Codex Minibar Stream Deck companion

This is a thin Stream Deck plugin. It stores each key's configuration in the
Stream Deck Property Inspector and reads sanitized quota snapshots from the
running Codex Minibar process over loopback.

## Development

From this directory:

```powershell
npm install
npm run build
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
enabled.
The Minibar process writes `streamdeck-bridge.json` beside its settings file
while running. Pressing a key while Minibar is stopped launches the normal
per-user installation and the plugin reconnects on its next refresh.
