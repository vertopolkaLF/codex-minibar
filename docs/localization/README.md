# Localization

Codex Minibar uses embedded Mozilla Fluent catalogs. English is the source
language. Complete translations are available in Russian, Brazilian Portuguese,
Spanish, and Simplified Chinese. Each catalog contains 755 messages.

- `locales/en/app.ftl`: existing English copy and full-message templates.
- `locales/ru/app.ftl`: Russian translations, including plural rules and dates.
- `locales/pt-BR/app.ftl`: Brazilian Portuguese translations.
- `locales/es/app.ftl`: Spanish translations.
- `locales/zh-CN/app.ftl`: Simplified Chinese translations.
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
Auto (Windows), English, Русский, Português (Brasil), Español, and 简体中文.
Language names stay in their native spelling so they are recognizable in any
selected language. Onboarding also offers this setting on its
General page. Old settings files default to Auto. Auto uses the Windows display
language, independently of the keyboard layout or regional clock format, and
uses English for unsupported languages. Portuguese display languages use the
Brazilian catalog; Spanish regional variants use the Spanish catalog; Chinese
display languages use the Simplified Chinese catalog. No Traditional Chinese
catalog is included.

Textual dates use day–month order in Russian, Portuguese, and Spanish, and
year–month–day order in Chinese. Numeric clock/date preferences stay independent.

Changes immediately refresh app windows. Settings edits use the existing serial
writer and live synchronization. Tray menus/tooltips, provider error summaries,
and cached Usage/Home snapshots follow the current language. A setting change
does not require reopening a window or restarting the application.

Segmented controls measure each label with the active GPUI font and size; fixed
segments and the animated selection thumb follow each label's width.

Popup cards never clip translated copy. Each card measures its text with
`components::TextMetrics` and the active font, then picks a layout:

- `components::adaptive_split` keeps leading and trailing groups on one row
  when both fit; otherwise the trailing group moves below, right-aligned.
- `components::fit_text` keeps a label on one line when it fits and lets it
  wrap once it has its own row.
- Compact quota, spending and cloud credit cards keep their background fill in
  the stacked layout. Spending cards stack the reset and expiry countdowns when
  they do not fit on one row. Banked and Tibo reset rows stack their dates and
  countdowns the same way.

New card rows must follow the same pattern: measure both groups and use
`adaptive_split` instead of a bare `split_row` around translated text.

### Pseudo-locale for layout QA

Start the app with `CODEX_MINIBAR_PSEUDO_LOCALE=1` and select English. English
copy becomes accented and about 40% longer (`Resets in` → `Réséts íñẋẋẋẋ`),
which approximates long translations such as Portuguese or German. Numbers,
names and other Fluent parameters stay unchanged. Check every popup surface
in one- and two-column layouts. It is a development aid only, not a language
option.

## Translate or add a message

Use a stable message ID for application copy. Keep GPUI element IDs, provider IDs,
storage keys and API field names separate from translated text. Static labels use
`crate::i18n::tr("message-id")`. Full messages use
`crate::i18n::format("message-id", &[("name", value.to_string())])`.

Translate full sentences with named parameters; do not build sentences by
concatenating fragments. Fluent selectors handle Russian one/few/many forms for
requests, sessions, API keys, banked resets, and login-expiry days. Portuguese
and Spanish use one/other forms, while Chinese uses invariant count phrases.
Numeric selectors
must receive numbers: the resolver recognizes these messages' count parameters.

Add the English source first, update all four translation catalogs, then regenerate the
translator template:

```powershell
python tools/localization_template.py
```

Missing, empty, or unresolvable messages in any translation fall back individually to the
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

Localization tests validate Fluent syntax, unique IDs, exact message and parameter
parity across all five catalogs, template coverage, literal translation call sites,
plural forms, per-message fallback in every translated language, settings
round-trips, language-selection indices, display-language detection, textual dates,
and preservation of parameter values. Every catalog is resolved directly for
counts 0, 1, 2, 5, 21, and 1,000,000 so English fallback cannot hide broken translations.

For manual QA, select each translated language and inspect every Settings tab, onboarding,
provider pages and dialogs, popup Home/Usage/provider tabs, chart tooltips, tray
menus and notifications. Switch back to English while the popup is open, check
theme flips and provider combinations, and verify longer Portuguese/Spanish copy and Chinese glyphs fit.
Also switch directly between Portuguese, Spanish, and Chinese while Home/Usage
charts and tray menus are visible to check their language-dependent caches. The
implementation was checked without launching the application; visual QA is pending.
