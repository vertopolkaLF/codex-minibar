//! Usage costs in the currency the user picked. Display only: stored usage
//! and every sum stay in micro-USD, and account balances and spending limits
//! keep the provider's own dollars.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{
        Condvar, LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

pub const CODES: &[&str] = &[
    "USD", "AED", "AFN", "ALL", "AMD", "ANG", "AOA", "ARS", "AUD", "AWG", "AZN", "BAM", "BBD",
    "BDT", "BGN", "BHD", "BIF", "BMD", "BND", "BOB", "BRL", "BSD", "BTN", "BWP", "BYN", "BZD",
    "CAD", "CDF", "CHF", "CLF", "CLP", "CNH", "CNY", "COP", "CRC", "CUP", "CVE", "CZK", "DJF",
    "DKK", "DOP", "DZD", "EGP", "ERN", "ETB", "EUR", "FJD", "FKP", "FOK", "GBP", "GEL", "GGP",
    "GHS", "GIP", "GMD", "GNF", "GTQ", "GYD", "HKD", "HNL", "HRK", "HTG", "HUF", "IDR", "ILS",
    "IMP", "INR", "IQD", "IRR", "ISK", "JEP", "JMD", "JOD", "JPY", "KES", "KGS", "KHR", "KID",
    "KMF", "KRW", "KWD", "KYD", "KZT", "LAK", "LBP", "LKR", "LRD", "LSL", "LYD", "MAD", "MDL",
    "MGA", "MKD", "MMK", "MNT", "MOP", "MRU", "MUR", "MVR", "MWK", "MXN", "MYR", "MZN", "NAD",
    "NGN", "NIO", "NOK", "NPR", "NZD", "OMR", "PAB", "PEN", "PGK", "PHP", "PKR", "PLN", "PYG",
    "QAR", "RON", "RSD", "RUB", "RWF", "SAR", "SBD", "SCR", "SDG", "SEK", "SGD", "SHP", "SLE",
    "SLL", "SOS", "SRD", "SSP", "STN", "SYP", "SZL", "THB", "TJS", "TMT", "TND", "TOP", "TRY",
    "TTD", "TVD", "TWD", "TZS", "UAH", "UGX", "UYU", "UZS", "VES", "VND", "VUV", "WST", "XAF",
    "XCD", "XCG", "XDR", "XOF", "XPF", "YER", "ZAR", "ZMW", "ZWG", "ZWL",
];
pub const SOURCE_URL: &str = "https://www.exchangerate-api.com";
const ENDPOINT: &str = "https://open.er-api.com/v6/latest/USD";

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Rates {
    result: String,
    base_code: String,
    time_last_update_unix: i64,
    time_next_update_unix: i64,
    rates: BTreeMap<String, f64>,
}
impl Rates {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.result == "success" && self.base_code == "USD",
            "invalid exchange rate response"
        );
        ensure!(
            self.time_last_update_unix > 0
                && self.time_next_update_unix > self.time_last_update_unix,
            "invalid exchange rate timestamps"
        );
        ensure!(self.rates.get("USD") == Some(&1.0), "missing USD base rate");
        ensure!(
            !self.rates.is_empty()
                && self
                    .rates
                    .values()
                    .all(|r| r.is_finite() && *r > 0.0 && *r < 1e12),
            "invalid exchange rate"
        );
        Ok(())
    }
}
/// Longest custom symbol kept, in characters.
pub const SYMBOL_MAX_CHARS: usize = 8;

#[derive(Default)]
struct State {
    selected: String,
    /// User symbol for `selected`, written verbatim; empty uses the default.
    symbol: String,
    /// The symbol follows the amount.
    after: bool,
    rates: Option<Rates>,
}
static STATE: LazyLock<(Mutex<State>, Condvar)> =
    LazyLock::new(|| (Mutex::new(State::default()), Condvar::new()));
static GENERATION: AtomicU64 = AtomicU64::new(0);
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}
/// Shows usage costs in `code`, marked with `symbol` (empty: `$` or the
/// code) before the amount, or after it when `after` is set.
pub fn select(code: &str, symbol: &str, after: bool) {
    let code = if CODES.contains(&code) { code } else { "USD" };
    let symbol = clean_symbol(symbol);
    let mut state = STATE.0.lock().unwrap_or_else(|e| e.into_inner());
    if state.selected != code || state.symbol != symbol || state.after != after {
        state.selected = code.into();
        state.symbol = symbol;
        state.after = after;
        GENERATION.fetch_add(1, Ordering::Release);
        STATE.1.notify_one();
    }
}
/// A symbol as typed, capped at [`SYMBOL_MAX_CHARS`]; blank means none.
pub fn clean_symbol(symbol: &str) -> String {
    if symbol.trim().is_empty() {
        return String::new();
    }
    symbol.chars().take(SYMBOL_MAX_CHARS).collect()
}
/// The symbol shown for `code` when the user set none.
pub fn default_symbol(code: &str) -> &str {
    if code == "USD" { "$" } else { code }
}
/// Hydrate cached rates before first paint and start the refresh worker; the
/// worker never blocks the UI on HTTP. Settings select the currency.
pub fn initialize(path: PathBuf) {
    if let Ok(bytes) = fs::read(&path)
        && let Ok(rates) = serde_json::from_slice::<Rates>(&bytes)
        && rates.validate().is_ok()
    {
        STATE.0.lock().unwrap_or_else(|e| e.into_inner()).rates = Some(rates);
    }
    std::thread::spawn(move || {
        let mut retry_at = 0;
        loop {
            let state = STATE.0.lock().unwrap_or_else(|e| e.into_inner());
            let now = chrono::Utc::now().timestamp();
            let due = state.rates.as_ref().map_or(0, |r| r.time_next_update_unix);
            if state.selected == "USD" || now < due.max(retry_at) {
                let _ = STATE.1.wait_timeout(state, Duration::from_secs(60));
                continue;
            }
            drop(state);
            match fetch() {
                Ok(rates) => {
                    // Atomic replacement avoids leaving a truncated cache after a crash.
                    if let Err(error) = save(&path, &rates) {
                        crate::platform::logger::info(format!("Exchange rate cache: {error:#}"));
                    }
                    STATE.0.lock().unwrap_or_else(|e| e.into_inner()).rates = Some(rates);
                    GENERATION.fetch_add(1, Ordering::Release);
                    retry_at = 0;
                }
                Err(error) => {
                    crate::platform::logger::info(format!("Exchange rate refresh: {error:#}"));
                    GENERATION.fetch_add(1, Ordering::Release);
                    retry_at = chrono::Utc::now().timestamp() + 3600;
                }
            }
        }
    });
}
fn fetch() -> Result<Rates> {
    let response = crate::platform::net::native_tls_agent()
        .timeout(Duration::from_secs(20))
        .build()
        .get(ENDPOINT)
        .call()
        .context("request exchange rates")?;
    let bytes = crate::platform::net::read_limited(response, 256 * 1024)?
        .context("exchange rate response is too large")?;
    let rates: Rates = serde_json::from_slice(&bytes)?;
    rates.validate()?;
    // A stale server response must not cause a tight retry loop.
    ensure!(
        rates.time_next_update_unix > chrono::Utc::now().timestamp(),
        "exchange rates already expired"
    );
    Ok(rates)
}
fn save(path: &std::path::Path, rates: &Rates) -> Result<()> {
    use std::io::Write;
    let mut file =
        tempfile::NamedTempFile::new_in(path.parent().unwrap_or(std::path::Path::new(".")))?;
    file.write_all(&serde_json::to_vec(rates)?)?;
    file.persist(path)?;
    Ok(())
}
#[derive(Clone)]
struct DisplayCurrency {
    code: String,
    rate: f64,
    /// Empty uses [`default_symbol`].
    symbol: String,
    after: bool,
}
impl DisplayCurrency {
    fn resolve(state: &State) -> Self {
        let code = state.selected.as_str();
        let rate = if code == "USD" {
            Some(1.0)
        } else {
            state
                .rates
                .as_ref()
                .and_then(|r| r.rates.get(code))
                .copied()
        };
        match rate {
            Some(rate) => Self {
                code: code.into(),
                rate,
                symbol: state.symbol.clone(),
                after: state.after,
            },
            // Dollars must not wear the chosen currency's symbol.
            None => Self {
                code: "USD".into(),
                rate: 1.0,
                symbol: String::new(),
                after: false,
            },
        }
    }
    fn symbol(&self) -> &str {
        if self.symbol.is_empty() {
            default_symbol(&self.code)
        } else {
            &self.symbol
        }
    }
    fn decimals(&self) -> usize {
        match self.code.as_str() {
            "BIF" | "CLP" | "DJF" | "GNF" | "ISK" | "JPY" | "KMF" | "KRW" | "PYG" | "RWF"
            | "UGX" | "VND" | "VUV" | "XAF" | "XOF" | "XPF" => 0,
            "BHD" | "IQD" | "JOD" | "KWD" | "LYD" | "OMR" | "TND" => 3,
            "CLF" => 4,
            _ => 2,
        }
    }
    fn minor(&self, microusd: u64) -> u128 {
        if self.code == "USD" {
            return u128::from(super::money::to_cents(microusd));
        }
        (microusd as f64 / 1e6 * self.rate * 10_u64.pow(self.decimals() as u32) as f64).round()
            as u128
    }
    fn format_minor(&self, minor: u128) -> String {
        let scale = 10_u128.pow(self.decimals() as u32);
        let integer = super::group_digits(&(minor / scale).to_string());
        let amount = if self.decimals() == 0 {
            integer
        } else {
            format!(
                "{integer}.{:0width$}",
                minor % scale,
                width = self.decimals()
            )
        };
        // Custom symbols are verbatim; a default code is a word and needs a gap.
        let gap = if self.symbol.is_empty() && self.code != "USD" {
            " "
        } else {
            ""
        };
        let symbol = self.symbol();
        if self.after {
            format!("{amount}{gap}{symbol}")
        } else {
            format!("{symbol}{gap}{amount}")
        }
    }
}
fn display() -> DisplayCurrency {
    DisplayCurrency::resolve(&STATE.0.lock().unwrap_or_else(|e| e.into_inner()))
}
pub fn format(microusd: u64) -> String {
    let d = display();
    d.format_minor(d.minor(microusd))
}
/// [`format`] for an amount in dollars, such as an estimated API value.
pub fn format_dollars(usd: f64) -> String {
    format(super::money::from_usd(usd))
}
pub fn display_minor(microusd: u64) -> u128 {
    display().minor(microusd)
}
pub fn format_minor(minor: u128) -> String {
    display().format_minor(minor)
}
/// Stable-height UI note with a source link; USD has no exchange rate dependency.
pub fn note() -> Option<String> {
    let state = STATE.0.lock().unwrap_or_else(|e| e.into_inner());
    if state.selected.is_empty() || state.selected == "USD" {
        return None;
    }
    let Some(rates) = state
        .rates
        .as_ref()
        .filter(|r| r.rates.contains_key(&state.selected))
    else {
        return Some(crate::i18n::tr("currency-unavailable").into());
    };
    let date = chrono::DateTime::from_timestamp(rates.time_last_update_unix, 0)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    let status = if chrono::Utc::now().timestamp() >= rates.time_next_update_unix {
        crate::i18n::tr("currency-cached")
    } else {
        crate::i18n::tr("currency-latest")
    };
    Some(format!("{status}: {date}"))
}

/// The mark of the displayed currency, such as a metric toggle label.
pub fn symbol() -> String {
    display().symbol().trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn currency(code: &str, rate: f64) -> DisplayCurrency {
        DisplayCurrency {
            code: code.into(),
            rate,
            symbol: String::new(),
            after: false,
        }
    }
    fn rub_state(symbol: &str, after: bool) -> State {
        State {
            selected: "RUB".into(),
            symbol: symbol.into(),
            after,
            rates: Some(Rates {
                result: "success".into(),
                base_code: "USD".into(),
                time_last_update_unix: 1,
                time_next_update_unix: 2,
                rates: BTreeMap::from([("USD".into(), 1.0), ("RUB".into(), 90.0)]),
            }),
        }
    }
    #[test]
    fn custom_symbols_are_verbatim_on_either_side() {
        let amount = |state: &State| {
            let d = DisplayCurrency::resolve(state);
            d.format_minor(d.minor(35_811_445))
        };
        assert_eq!(amount(&rub_state("\u{20bd}", true)), "3,223.03\u{20bd}");
        assert_eq!(amount(&rub_state(" \u{20bd}", true)), "3,223.03 \u{20bd}");
        assert_eq!(amount(&rub_state("\u{20bd}", false)), "\u{20bd}3,223.03");
        assert_eq!(amount(&rub_state("", true)), "3,223.03 RUB");
        assert_eq!(amount(&rub_state("", false)), "RUB 3,223.03");
        let mut usd = rub_state("US$", true);
        usd.selected = "USD".into();
        assert_eq!(amount(&usd), "35.81US$");
        usd.symbol.clear();
        assert_eq!(amount(&usd), "35.81$");
    }
    #[test]
    fn a_usd_fallback_drops_the_custom_symbol() {
        let mut state = rub_state("\u{20bd}", true);
        state.rates = None;
        let d = DisplayCurrency::resolve(&state);
        assert_eq!(d.format_minor(d.minor(1_000_000)), "$1.00");
    }
    #[test]
    fn blank_symbols_reset_and_long_ones_are_capped() {
        assert_eq!(clean_symbol("   "), "");
        assert_eq!(clean_symbol(" \u{20bd}"), " \u{20bd}");
        assert_eq!(
            clean_symbol("abcdefghijk").chars().count(),
            SYMBOL_MAX_CHARS
        );
    }
    #[test]
    fn conversion_and_minor_units() {
        let rub = currency("RUB", 90.0);
        assert_eq!(rub.format_minor(rub.minor(1_234_567)), "RUB 111.11");
        let jpy = currency("JPY", 150.0);
        assert_eq!(jpy.format_minor(jpy.minor(10_000)), "JPY 2");
        let kwd = currency("KWD", 0.3);
        assert_eq!(kwd.format_minor(kwd.minor(1_234_567)), "KWD 0.370");
        let usd = currency("USD", 1.0);
        assert_eq!(
            usd.format_minor(usd.minor(u64::MAX)),
            "$18,446,744,073,709.55"
        );
    }
    #[test]
    fn missing_currency_falls_back_to_usd() {
        let state = State {
            selected: "RUB".into(),
            rates: None,
            ..State::default()
        };
        let d = DisplayCurrency::resolve(&state);
        assert_eq!(d.format_minor(d.minor(1_000_000)), "$1.00");
    }
    #[test]
    fn settings_default_and_roundtrip_keep_display_currency() {
        let legacy: crate::settings::Settings = toml::from_str("").unwrap();
        assert_eq!(legacy.usage_currency, "USD");
        let selected: crate::settings::Settings = toml::from_str("usage_currency = 'JPY'").unwrap();
        let restored: crate::settings::Settings =
            toml::from_str(&toml::to_string(&selected).unwrap()).unwrap();
        assert_eq!(restored.usage_currency, "JPY");
    }
    #[test]
    fn cached_rates_remain_usable_offline_and_missing_codes_fall_back() {
        let mut state = rub_state("", false);
        let d = DisplayCurrency::resolve(&state);
        assert_eq!(d.format_minor(d.minor(1_000_000)), "RUB 90.00");
        state.selected = "JPY".into();
        assert_eq!(DisplayCurrency::resolve(&state).code, "USD");
        state.selected = "USD".into();
        assert_eq!(DisplayCurrency::resolve(&state).rate, 1.0);
    }
    #[test]
    fn rejects_invalid_rates_and_cache_roundtrips() {
        let mut rates = Rates {
            result: "success".into(),
            base_code: "USD".into(),
            time_last_update_unix: 1,
            time_next_update_unix: 2,
            rates: BTreeMap::from([("USD".into(), 1.0), ("RUB".into(), 90.0)]),
        };
        rates.validate().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rates.json");
        save(&path, &rates).unwrap();
        save(&path, &rates).unwrap();
        serde_json::from_slice::<Rates>(&fs::read(path).unwrap())
            .unwrap()
            .validate()
            .unwrap();
        for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            rates.rates.insert("RUB".into(), invalid);
            assert!(rates.validate().is_err());
        }
    }
}
