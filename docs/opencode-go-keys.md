# OpenCode Go API keys

OpenCode Go can cover several people at once: Go Teams bills per member, and
every member gets their own limits. Minibar therefore queries every API key of
an OpenCode Go instance instead of one, and shows each key's own five-hour,
weekly and monthly windows.

## What an instance queries

```
GET https://opencode.ai/zen/go/v1/usage
Authorization: Bearer <key>
```

One request per key, in settings order. A key that fails (expired, revoked,
a service account outside Go, or HTTP 429) keeps its row with the error, and
the other keys still report their quotas. When every key fails the instance
reports the first error exactly as before, so the worker's retry, backoff and
notification behavior does not change.

## Storage

Keys never enter `settings.toml`. `ProviderInstance.opencode` stores only the
stable identities and optional display names:

```toml
[[instances]]
id = "opencode-go"
driver = "opencode_go"
opencode = { key_ids = ["primary", "opencode-key-1a2b"], key_names = { "opencode-key-1a2b" = "Work" } }
```

The key material lives in the protected secret store:

| Slot | Secret name |
|---|---|
| First instance, legacy slot | `opencode-go` |
| Other instance, legacy slot | `opencode-go.<instance-id>` |
| Extra keys | `opencode-go[.<instance-id>].<key-id>` |

The legacy slot keeps the pre-multi-key name, so existing installations need no
migration and keep their environment-variable and `auth.json` fallbacks. Extra
keys use only their own saved value: an environment variable describes this
PC's primary login, not another Go Teams member.

## Aggregation

The instance-level lanes (`primary`, `secondary` and the `monthly` additional
limit) show the **most consumed key** for each window, so the tray icon, Home
cards and low-usage notifications keep the meaning they had with one key. The
plan line reads `Go · N keys` when more than one key is tracked.

Every key's report is kept in `RateLimits::opencode_go_keys` and rendered by
the OpenCode Go tab as a key list: name, key fingerprint, and one bar per
window. Home keeps the aggregate cards, which is why the list is limited to the
provider tab.

## Settings

**Settings → Providers → OpenCode Go → Account** lists one row per key. Each row
offers:

- **Rename key** — a local display name only; the key itself is unchanged.
- **Replace key** — validates nothing remotely and saves immediately; the row
  keeps its position and name.
- **Remove key** — hidden for the last key, which stays as an empty slot so the
  instance always has one row to fill.

**Add API key** appends a slot. Adding or replacing a key advances the
instance's `credentials_revision`, so the running worker is replaced and the
popup drops samples from the previous keys.

## Limitations

- The endpoint is keyed by the API key. If a workspace returns the same
  workspace-wide numbers for every member's key, every row shows the same
  values; Minibar does not deduplicate them.
- Service accounts are not covered by Go Teams, so their keys generally fail
  here with the server's error, which the row displays.
- The key list is read-only with respect to OpenCode: keys are created and
  revoked in the OpenCode Console.
