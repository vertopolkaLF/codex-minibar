# Codex forced-reset feed

The app reads [`data/codex-resets.json`](../data/codex-resets.json) from the
repository's `main` branch through GitHub raw content. A bot should replace the
`resets` array in one commit whenever Tibo confirms a reset in X.

The current contract is schema version `2`. The app temporarily accepts the
older version `1` so an existing feed keeps working during migration. A reset
without `source_url` is still accepted, cached, shown, and eligible for a
notification; it simply has no clickable verification link. New bot writes
should use version `2` and include the direct source URL whenever one is known.

```json
{
  "schema_version": 2,
  "resets": [
    {
      "id": "forced-2026-09-15T18:00:00Z",
      "type": "forced",
      "source_url": "https://x.com/tibo/status/1234567890",
      "label": "Codex weekly quota",
      "reset_at": "2026-09-15T18:00:00Z"
    },
    {
      "id": "banked-2026-09-16T18:00:00Z",
      "type": "banked",
      "source_url": "https://x.com/tibo/status/1234567891",
      "reset_at": "2026-09-16T18:00:00Z"
    }
  ]
}
```

Rules for the bot:

- `reset_at` is always an ISO-8601 UTC timestamp.
- `source_url` is optional. When present, it must be the HTTPS post or page
  that confirms the reset; the corresponding reset row becomes clickable in
  the app. When absent, the reset must still be emitted normally.
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
