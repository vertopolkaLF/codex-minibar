# Codex forced-reset feed

The app reads [`data/codex-resets.json`](../data/codex-resets.json) from the
repository's `main` branch through GitHub raw content. A bot should replace the
`resets` array in one commit whenever Tibo confirms a reset in X.

```json
{
  "schema_version": 1,
  "resets": [
    {
      "id": "forced-2026-09-15T18:00:00Z",
      "type": "forced",
      "label": "Codex weekly quota",
      "reset_at": "2026-09-15T18:00:00Z"
    },
    {
      "id": "banked-2026-09-16T18:00:00Z",
      "type": "banked",
      "reset_at": "2026-09-16T18:00:00Z"
    }
  ]
}
```

Rules for the bot:

- `reset_at` is always an ISO-8601 UTC timestamp.
- `id` is stable across corrections. If the time is corrected, update the
  existing entry instead of creating a second id.
- `type` is either `forced` or `banked`. The app intentionally discards
  `banked` entries at ingestion, so they can share the same feed without ever
  appearing in the forced-reset card or toast.
- Keep future announcements in the file and remove expired entries. The app
  sorts by `reset_at`, ignores duplicates, and ignores past entries.
- `label` is optional and is only short context for the user.

The app checks immediately on startup and then at the configured interval
(one hour by default). It stores the last valid feed and sent-notification ids
in the user's app-data directory, so a temporary GitHub failure does not erase
an already confirmed reset and reopening the app does not send the same toast
again.
