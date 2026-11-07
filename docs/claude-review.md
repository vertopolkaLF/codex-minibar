# Claude provider review — 2026-11-06

Scope: production code in `src/claude.rs`, with worker error handling checked for context. Static review only; no live API requests or credential changes. Both findings below are now fixed and covered by regression tests.

## Fixed — P2: Profile cache survives a change of account

`ClaudeClient::read_oauth` caches account name and plan using only elapsed time and the quota reset schedule (`ClaudeAccountCache::needs_refresh`, around lines 420–442 and 676–711). The cache is not keyed by account or credential. If the CLI/Desktop login changes while the worker survives, and the two accounts have the same reset schedule (including absent reset timestamps), the next quota response belongs to the new account but receives the previous account name and fallback plan for up to 30 minutes. A transient profile failure can also postpone discovering the new identity.

Suggested fix: invalidate profile metadata when the credential identity changes, without logging or persisting token material. Verify by reading account A followed by B with identical reset timestamps.

## Fixed — P2: Renewal HTTP 429 loses the typed rate-limit signal

`refresh_cli_login` (around lines 825–827) converts HTTP 429 into a plain `anyhow` text error. Unlike `send`, it neither retains the `ureq::Error::Status(429, ...)` nor uses `worker::rate_limit_error`. `worker::is_rate_limited_error` recognizes only those typed causes. Consequently renewal throttling does not activate the shared provider pause; the independent five-minute refresh retry guard still operates, but other provider requests are not paused as intended.

Suggested fix: return `worker::rate_limit_error` for renewal 429. Verify that the renewal failure is recognized by `is_rate_limited_error` and the worker enters the shared pause.

## Limits of this review

The cache now invalidates on an in-memory SHA-256 credential digest change, before requesting usage. Renewal response mapping now preserves a typed rate-limit error for HTTP 429. Regression tests cover unchanged and changed credentials with identical reset schedules, and recognition of renewal throttling through an added error context. Token refresh file replacement and reset redemption were inspected, but external-process races and actual upstream endpoint behavior were not exercised. The nested `profile_oauth` and Desktop implementation require separate review to cover the entire authentication flow.
