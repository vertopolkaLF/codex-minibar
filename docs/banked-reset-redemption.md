# Banked reset redemption

Researched against T3 Code commit
`0e7abea7c98060db537fe81f31ce5ba7c6d376d0` (2026-10-10).

- [CodexDriver](https://github.com/pingdotgg/t3code/blob/0e7abea7c98060db537fe81f31ce5ba7c6d376d0/apps/server/src/provider/Drivers/CodexDriver.ts):
  launches an account-scoped Codex app-server and sends
  `account/rateLimitResetCredit/consume` with `idempotencyKey`.
- [claudeResetCredits](https://github.com/pingdotgg/t3code/blob/0e7abea7c98060db537fe81f31ce5ba7c6d376d0/apps/server/src/provider/claudeResetCredits.ts):
  reads OAuth CLI credentials and `oauthAccount.organizationUuid`, then posts
  to `https://api.anthropic.com/api/organizations/{uuid}/reset_rate_limits`.
  The JSON body contains `program: "cedar_ember"`, `grant_id`, and `request_id`.
  Headers match the usage request: Bearer token, `oauth-2025-04-20` beta,
  and CLI User-Agent. Only the server-selected `next_grant_id` is claimable;
  paused/unusable/expired grants are excluded from redemption.
- [resetCreditCoordinator](https://github.com/pingdotgg/t3code/blob/0e7abea7c98060db537fe81f31ce5ba7c6d376d0/apps/server/src/provider/resetCreditCoordinator.ts):
  serializes account requests and retains an idempotency key after an
  unanswered request. Confirmed outcomes and final failures clear the key.
- [ResetCredits UI](https://github.com/pingdotgg/t3code/blob/0e7abea7c98060db537fe81f31ce5ba7c6d376d0/apps/mobile/src/features/usage/UsageLimitsSection.tsx):
  confirms spending a credit, disables the action while busy, and reports
  the provider outcome plus a warning if the follow-up quota refresh fails.

Minibar follows these protocols. Redemption runs on the background executor,
scoped to the selected instance's login. Locks use the canonical login
directory and account/organization ID, avoiding conflation of different team
members in the same organization. Account locks reject overlapping
redemptions, and unanswered retries retain both the request ID and Claude
grant. Pending IDs last for the application session. No reset is automatically
spent, and no model inference request is sent.

Claude redemption requires CLI credentials and the CLI account configuration.
Manual credentials remain quota-only. The ambient Desktop login is checked
against the CLI token before any claim, preventing redemption on a different
CLI account. Explicit Claude folders read `.claude.json` beside credentials;
the ambient CLI uses the home directory's `.claude.json`.

Confirmed quota snapshots use the existing revision-guarded event bridge,
updating Home, provider tabs, tray, and settings metadata. A subsequent worker
refresh updates sibling instances. Results survive exhaustion of the last
credit; the zero-count card remains available to show the outcome. Changes
to instance credentials/path clear old status and confirmation state.

Verification must use compilation and isolated tests. Do not launch the
application or spend a real account's credit during development.
