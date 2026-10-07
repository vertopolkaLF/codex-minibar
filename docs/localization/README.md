# Localization

Codex Minibar uses embedded Mozilla Fluent catalogs. English is the source
language; Russian is the first translation. Each catalog contains 747 messages.

- `locales/en/app.ftl`: existing English copy and full-message templates.
- `locales/ru/app.ftl`: Russian translations, including plural rules and dates.
- `locales/template/app.ftl`: empty translator template with English examples.
- `src/i18n.rs`: language selection, static labels, parameterized messages and fallback.
- `docs/localization/string-inventory.tsv`: all 4,545 non-test Rust string occurrences
  collected from source revision `ee75ce4`, with their original locations.

The inventory includes protocol fields, paths, SQL, element identities, command
arguments, log messages and AI troubleshooting instructions. These are source
literals, not all translation messages. Application interface copy is in the
catalogs; technical diagnostics, provider/model names, remote API titles, and
user-entered account/key names retain their original content. Windows-owned file
dialogs follow Windows' own language.

## Select a language

Open Settings > General > Language (Настройки > Общие > Язык). The choices are
Auto (Windows), English, and Русский. Onboarding also offers this setting on its
General page. Old settings files default to Auto. Auto uses the Windows display
language, independently of the keyboard layout or regional clock format, and
uses English for unsupported languages.

Changes immediately refresh app windows. Settings edits use the existing serial
writer and live synchronization. Tray menus/tooltips, provider error summaries,
and cached Usage/Home snapshots follow the current language. A setting change
does not require reopening a window or restarting the application.

Segmented controls measure each label with the active GPUI font and size; fixed
segments and the animated selection thumb follow each label's width. Compact
quota and spending cards use the full layout when their complete name cannot
fit alongside the current values. Names then use a separate full-width row.

## Translate or add a message

Use a stable message ID for application copy. Keep GPUI element IDs, provider IDs,
storage keys and API field names separate from translated text. Static labels use
`crate::i18n::tr("message-id")`. Full messages use
`crate::i18n::format("message-id", &[("name", value.to_string())])`.

Translate full sentences with named parameters; do not build sentences by
concatenating fragments. Fluent selectors handle Russian one/few/many forms for
requests, sessions, API keys, banked resets, and login-expiry days. Numeric selectors
must receive numbers: the resolver recognizes these messages' count parameters.

Add the English source first, add the Russian translation, then regenerate the
translator template:

```powershell
python tools/localization_template.py
```

Missing, empty, or unresolvable Russian messages fall back individually to the
English message with the same parameters. The fallback also applies to static
labels. The catalogs are embedded in the executable, so editing a translation
file requires rebuilding; selecting the language in the built app applies live.
Supporting another language also requires registering it in `Language`, the
resolver and the selection controls; copying a catalog alone does not register it.

Refresh the source-literal inventory when needed:

```powershell
python tools/localization_inventory.py
# Or inventory a specific source revision:
python tools/localization_inventory.py --revision ee75ce4
```

## Verification

```powershell
cargo fmt --package codex-minibar --check
cargo check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
```

Localization tests validate Fluent syntax, unique IDs, English/Russian parameter
parity, template coverage, literal translation call sites, Russian plurals,
fallback, old settings migration, and preservation of parameter values. Current
Russian completeness is checked even though the runtime accepts missing messages.

For manual QA, select Russian and inspect every Settings tab, onboarding,
provider pages and dialogs, popup Home/Usage/provider tabs, chart tooltips, tray
menus and notifications. Switch back to English while the popup is open, check
theme flips and provider combinations, and verify longer Russian copy fits. The
implementation was checked without launching the application; visual QA is pending.
