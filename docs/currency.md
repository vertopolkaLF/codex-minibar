# Usage currency

Settings > General > Usage currency controls displayed usage costs. USD is the
default. Existing settings files require no migration. Provider balances, quotas,
credits, stored micro-USD amounts, pricing and usage aggregation keep their
original units.

`src/format/currency.rs` owns currency selection, rate validation, disk caching
and formatting, next to the micro-USD helpers in `src/format/money.rs`. Usage
surfaces call `currency::format` instead of `format_micro_usd`; balances and
spending limits keep calling `format_micro_usd`.

## Rates and offline behavior

The application requests `https://open.er-api.com/v6/latest/USD` only when a
non-USD currency is selected. No API key or usage data is sent. The response's
next-update timestamp schedules refreshes; failed requests retry after one hour.
A single background worker performs requests with a 20-second timeout and a
bounded response size. Switching currency wakes it without waiting for a scan.

The validated response is atomically saved beside the settings file as
`exchange-rates.json` and loaded before first paint. Expired cached rates remain
usable offline and are identified as cached in the UI. If the selected currency
has no cached rate, amounts explicitly remain in USD until a rate arrives.

All historical amounts use the latest available rate, not the rate on the day of
usage. This is a display estimate, not historical foreign-exchange accounting.
The settings description explains this behavior. Relevant popup surfaces and
Settings link to Exchange Rate API and show the rate's UTC date.

Currency codes disambiguate non-USD amounts. Formatting uses each currency's
minor-unit precision (including JPY, KWD and CLF); USD retains integer rounding
through the maximum stored `u64` value. Chart tooltip totals sum the displayed
minor units so rounding agrees with their rows.

## Live updates

`settings::runtime::apply_presentation` selects the currency whenever
`usage_currency` changes, so a Settings edit shows before it commits and edits
from any other surface apply the same way. Selection and new rates bump a
generation counter; the bridge checks it each tick and marks limits and usage
changed, which repaints the tray, taskbar, Settings and popup through the usual
fan-out. Aggregation snapshots remain in USD and need no rescan.

## Symbol

Settings > General > Currency symbol replaces the default mark (`$` for USD, the
code otherwise) and Symbol position puts it before or after the amount. A custom
symbol is written verbatim, spaces included, so `₽` after the amount gives
`3,223.03₽` and ` ₽` gives `3,223.03 ₽`; a default code keeps its space. Symbols
are capped at eight characters and reset when the currency changes. While a
missing rate keeps amounts in USD, the custom symbol is ignored so dollars never
wear another currency's mark.
