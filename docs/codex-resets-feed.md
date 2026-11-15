# Codex reset feed

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
      "timing": "before",
      "source_url": "https://x.com/tibo/status/1234567890",
      "label": "Day 18 reset (lands before 10am PT)",
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
- `type` is either `forced` (everyone's limits are reset) or `banked` (everyone
  receives a banked reset credit to spend later). Both are shown in the app and
  labelled by kind.
- `timing` is optional. `exact` (the default) means the reset lands at
  `reset_at`. `before` means it lands at some point before `reset_at`, which is
  then only a deadline; use it for announcements such as "lands before 10am PT".
- `not_before` is optional and only meaningful with `timing: "before"`: the
  earliest ISO-8601 UTC time the reset may land. Without it the app assumes a
  12-hour landing window ending at `reset_at`.
- `source_url` is optional. When present, it must be the HTTPS post or page
  that confirms the reset; the corresponding reset row becomes clickable in
  the app. When absent, the reset must still be emitted normally.
- `id` is stable across corrections. If the time is corrected, update the
  existing entry instead of creating a second id.
- Keep future announcements in the file. Expired entries may be removed; the
  app keeps its own copy of past announcements for a week. Removing a future
  entry retracts it.
- Remove expired entries and add new ones in the **same** commit. GitHub's raw
  CDN caches the file for about five minutes, so a separate "clear" commit
  followed quickly by an "add" commit can leave clients with the empty
  intermediate version until the next refresh.
- `label` is optional and is only short context for the user.

The app checks immediately on startup and then at the configured interval
(one hour by default). It stores the last valid feed and sent-notification ids
in the user's app-data directory, so a temporary GitHub failure does not erase
an already confirmed reset and reopening the app does not send the same toast
again.

## Reception detection

The feed only says when a reset should land. The app decides per Codex instance
whether it actually arrived by comparing consecutive quota samples
(`src/quota/reset_detection.rs`). The last sample and a week of detected events are
stored with the feed cache, and the first live sample after launch is compared
with the persisted sample from the previous session, so detection also covers
resets that landed while the app was closed.

- A **forced** reset is received when a quota window restarted or emptied while
  its previous deadline was still in the future, inside the reset's landing
  window, and no banked reset credit was spent in the same interval. An idle
  account counts as received when it is at 100/100 after the deadline and no
  banked credit was spent since the landing window opened.
- A **banked** reset is received when a reset credit granted inside the landing
  window appears.
- When the interval between two samples also covers an earlier announcement's
  landing window, the change is attributed to the earlier announcement only.

Rows show "Received" for six hours after detection. A reset that has not
arrived stays visible for 48 hours after its deadline, showing how long ago it
should have landed, unless a later forced reset reached the account.

The feed notification means only that new possible-reset information arrived;
it is not confirmation that the reset happened. The `Codex limits reset`
notification remains driven exclusively by the provider API changing its
reported limits snapshot, and is never scheduled from `reset_at` in this feed.
