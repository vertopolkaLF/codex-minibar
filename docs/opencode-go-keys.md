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
the other keys still report their quotas. When every key fails, a fresh
snapshot still publishes every key's error and clears the old quota windows;
the instance also reports the first error. A single failing key keeps its
error row too. HTTP errors expose only the status, never the response body.
When any key fails, notifications retain the last complete quota observation.
Successful keys still update the displayed quotas, but notification tracking
waits until all keys report successfully. This preserves reset detection and
prevents repeated low-usage warnings when a key temporarily disappears from
the aggregate and then recovers.

Requests run sequentially, with a 15-second timeout per request, so a slow
key delays the following keys. HTTP 429 from any key pauses automatic polling
for the whole instance, including when other keys return valid quotas. The
existing manual-refresh and rate-limit cooldown rules still apply.

## Storage

Keys never enter `settings.toml`. `ProviderInstance.opencode` stores only the
stable identities and optional display names:

```toml
[[instances]]
id = "opencode-go"
driver = "opencode_go"
opencode = { key_ids = ["primary", "opencode-key-1a2b"], key_names = { "opencode-key-1a2b" = "Work" } }
```

The key material lives in the protected secret store (Windows user-scoped
DPAPI, or Secret Service-backed encryption on Linux):

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
limit) show the **most consumed key** for each window, so instance-wide tray
indicators and quota notifications retain their meaning. Each window is
chosen independently and can come from a different key. Failed keys contribute
no quota windows. The plan line reads `Go · N keys` when more than one key
is tracked.

Every key's report is kept in `RateLimits::opencode_go_keys`. With two or more
keys, both Home and the OpenCode Go tab replace the aggregate quota cards with
the key list: name, key fingerprint, and bars for the quota columns enabled
for that surface. One healthy key uses the ordinary quota cards instead.

The **API keys** card can be switched off independently for Home and the
provider tab in **Settings → Appearance → Popup cards**. With several keys,
turning it off shows ordinary quota cards under each key's heading, rather
than returning to aggregate cards. Five-hour, weekly and monthly switches
control the corresponding columns or cards. Failed keys keep their reasons
instead of quota bars.

Tray, taskbar and Command Palette widget indicators can select an individual
key when several keys are tracked. Their values and hover-card sections then
belong to that key. If it is removed, the indicator falls back to the
instance's aggregate quotas and heading.

## Settings

**Settings → Providers → OpenCode Go → Account** lists one row per key. Each row
offers:

- **Rename key** — a local display name only; the key itself is unchanged.
  Renaming or clearing the name takes effect immediately and survives later
  polls without restarting the worker.
- **Replace key** — validates nothing remotely and saves immediately; the row
  keeps its position and name.
- **Remove key** — available even for the last key. Removing the last key
  clears its saved secret and name but keeps its slot, so the instance always
  has one row to fill. The primary instance's legacy slot can still resolve
  a key from environment variables or `auth.json` after its saved value is
  removed; those external credentials are not deleted.

**Add API key** appends a slot. Adding, replacing or removing a key advances the
instance's `credentials_revision`, so the running worker is replaced and the
popup drops samples from the previous keys.

Adding, replacing and removing secrets preserve their previous ciphertext
until the settings write succeeds. If that write fails, the previous secret
is restored without overwriting a newer secret update; a rollback conflict is
reported as an error.

## Limitations

- The endpoint is keyed by the API key. If a workspace returns the same
  workspace-wide numbers for every member's key, every row shows the same
  values; Minibar does not deduplicate them.
- Service accounts are not covered by Go Teams, so their keys generally fail
  here with the server's error, which the row displays.
- The key list is read-only with respect to OpenCode: keys are created and
  revoked in the OpenCode Console.
