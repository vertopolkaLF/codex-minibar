# Antigravity quotas

Authentication reuses the official `agy` session from Windows Credential Manager
(`gemini:antigravity`). Expired sessions are refreshed through `agy models`.

Quota requests use the Hub compatibility User-Agent
`antigravity/hub/2.9.1 windows/amd64`:

1. `POST https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist` resolves
   the account project and plan, preferring the paid tier name.
2. `POST https://daily-cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary`
   reads the subscription quota groups for that project.

The existing Gemini and Claude + GPT metrics each show the most consumed known
allowance within their family, considering both five-hour and weekly buckets.
The percentage, reset timestamp, and duration always come from the same bucket.
Missing families remain unavailable; they do not inherit another family's quota.

Do not fall back to `fetchAvailableModels` or `retrieveUserQuota` to calculate
Antigravity subscription usage. Both endpoints can return 100% remaining for all
models even when the subscription's weekly quota has been consumed. A generic
`antigravity` User-Agent can also omit the Gemini group from the summary.
This was reproduced against the same signed-in account on 2026-10-20: the
model quota endpoint returned all-full fractions, while the Hub summary reported
`0.97032213` remaining for Gemini's weekly allowance (3% used after rounding).

Unknown or malformed quota fractions and failed summary requests remain errors
or unavailable metrics rather than becoming a synthetic 0% usage reading.
Genuine 100%-remaining summary buckets are valid and display 0% used.

Protocol reference:
[CodexBar's Antigravity remote fetcher](https://github.com/steipete/CodexBar/blob/main/Sources/CodexBarCore/Providers/Antigravity/AntigravityRemoteUsageFetcher.swift).

## Local usage history

Usage reads Antigravity conversation databases from `~/.gemini/antigravity*`,
`~/.config/antigravity`, and local T3 Code ACP profiles. Set
`ANTIGRAVITY_DATA_DIR` to a comma-separated list of data directories to override
the native Antigravity paths. `T3CODE_HOME` overrides the T3 Code base directory
(`~/.t3` by default).

The scanner opens the databases read-only and uses recorded token counts.
Usage includes hourly and daily totals, a model breakdown, and API-equivalent
costs for models with known rates.
