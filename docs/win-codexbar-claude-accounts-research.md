# Win-CodexBar: Claude OAuth and multiple accounts

Inspected upstream commit `3c30537e05d69cbcada416083af7d361baf28f83` on 2026-10-04. Source review only; no login, token requests, or application launches were performed.

## Browser login

Win-CodexBar delegates browser authorization to the installed native Claude Code executable. It creates `%APPDATA%/CodexBar/claude-accounts/logins/<UUID>`, sets `CLAUDE_CONFIG_DIR` and the working directory to it, removes inherited API/auth overrides, and invokes `claude auth login --claudeai`. It waits up to 300 seconds, reads the CLI-produced credentials and account identity, then cleans up the temporary directory. Thus the browser authorization protocol and callback handling belong to Claude Code, rather than a custom OAuth authorization client in this feature.

Source: [login runner](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/providers/claude/accounts/login.rs#L71-L96), [isolated login invocation](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/providers/claude/accounts/login.rs#L188-L202).

## Saved logins and switching

Saved logins retain both the `claudeAiOauth` object and `oauthAccount` identity. Validation requires access token, refresh token, email, account UUID, and organization UUID. Identity is `accountUuid:organizationUuid`, so the same email in different organizations remains distinct. Storage is `%APPDATA%/CodexBar/claude-accounts/accounts.json`.

Source: [saved login model and storage](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/providers/claude/accounts.rs#L32-L138), [identity and deduplication](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/providers/claude/accounts.rs#L296-L322).

On Windows the saved store uses DPAPI, first user scope, then machine scope if user protection fails; writes fail if both fail. This is not an unconditional user-scope guarantee. Non-Windows writes use plaintext. Active CLI credentials remain in Claude Code's own file format.

Source: [secure storage implementation](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/secure_file.rs#L107-L149).

Switching saves the outgoing account's latest credentials, replaces only `claudeAiOauth` in the CLI credentials file and `oauthAccount` in its configuration, stages writes, and attempts rollback if the identity update fails. Other settings and MCP secrets are preserved. Running CLI sessions must be closed and reopened; Claude Desktop and browser sessions are not switched. Removing an entry only forgets the saved copy.

Source: [switch implementation](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/providers/claude/accounts.rs#L237-L292), [documented behavior](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/docs/CONFIGURATION.md#L119-L149).

## OAuth usage and refresh

The OAuth fetcher loads the current credential source and calls `GET https://api.anthropic.com/api/oauth/usage` with Bearer authorization and `anthropic-beta: oauth-2025-04-20`. Usage requires `user:profile`; the implementation warns that `claude setup-token` is not a usage-scope recovery. Explicit pasted access tokens have no refresh token or expiry metadata.

Source: [OAuth fetcher](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/providers/claude/oauth/mod.rs#L283-L343), [usage request](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/providers/claude/oauth/mod.rs#L457-L496), [scope recovery](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/providers/claude/oauth/mod.rs#L643-L646).

Tokens within five minutes of expiry are refreshed through `POST https://platform.claude.com/v1/oauth/token`, JSON grant `refresh_token`, client ID `9d1c250a-e61b-44d9-88ed-5944d1962f5e`, and current scopes. Rotated tokens are cached and persisted to the saved account first, then to the active CLI credential file. Account operations and the normal OAuth fetch share a mutex. Refresh `400/401 + invalid_grant` is terminal; other failures use transient cooldown. Usage 429 has a separate shared cooldown with a five-minute floor.

Source: [expiry threshold](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/providers/claude/oauth/mod.rs#L35-L43), [refresh protocol](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/providers/claude/oauth/refresh.rs#L14-L160), [refresh persistence](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/providers/claude/oauth/mod.rs#L427-L454).

## Scope and implication for Minibar

The saved-login feature manages several Claude Code identities but monitors the current provider source, rather than polling every saved login independently. Auto tries Admin API when configured, then Web, OAuth, and CLI. Consequently switching CLI accounts does not ensure that Web-derived usage switches too. Generic session-token accounts are a separate mechanism, whose Claude configuration now prefers sessionKey cookies and describes OAuth tokens as legacy fallback.

Source: [source selection](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/providers/claude/mod.rs#L883-L930), [current OAuth credentials](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/providers/claude/oauth/mod.rs#L295-L319), [session token account definition](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/core/token_accounts.rs#L51-L64).

The desktop selects one active generic token account and performs one fetch per provider; separate managed-account refresh lanes belong to Codex. There is an exception in the CLI serve/dashboard: it fetches all generic Claude token-account rows through cookie overrides using a JoinSet. That is a different path from refreshing all saved Claude Code OAuth logins.

Source: [desktop selection](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/apps/desktop-tauri/src-tauri/src/commands/providers.rs#L65-L79), [desktop refresh lanes](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/apps/desktop-tauri/src-tauri/src/commands/providers.rs#L730-L789), [CLI dashboard multi-account collection](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/cli/serve/dashboard/source.rs#L280-L357).

Current Minibar already reads every enabled Claude profile (`src/claude.rs`, `Client::read`), with protected pasted credentials for named profiles (`src/settings.rs`, `ClaudeProfile`). Its default profile follows Desktop/CLI credentials. The useful upstream idea is isolated browser login that captures a full refreshable credential bundle. Adapting it to Minibar would require per-profile credential bundles, refresh persistence, and refresh synchronization while retaining independent profile monitoring. Replacing the user's ambient CLI login is unnecessary for that goal. This paragraph is an implementation inference from the inspected sources, not a proposed or completed code change.
