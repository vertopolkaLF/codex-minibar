//! Rates for displaying USD usage estimates in New Taiwan dollars.

use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use chrono::Utc;
use serde::{Deserialize, Serialize};

pub(crate) const SOURCE_URL: &str = "https://www.exchangerate-api.com";
const ENDPOINT: &str = "https://open.er-api.com/v6/latest/USD";
const RETRY_INTERVAL: Duration = Duration::from_secs(3600);

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct TwdRate {
    millionths: u64,
    updated_at: i64,
    next_update: i64,
}

impl TwdRate {
    fn valid(self, now: i64) -> bool {
        (1..1_000_000_000).contains(&self.millionths)
            && (1..=now).contains(&self.updated_at)
            && self.next_update > self.updated_at
    }

    pub(crate) fn per_usd(self) -> f64 {
        self.millionths as f64 / 1_000_000.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum CostDisplay {
    #[default]
    Usd,
    AwaitingTwd,
    Twd(TwdRate),
}

impl CostDisplay {
    pub(crate) fn symbol(self) -> &'static str {
        match self {
            Self::Twd(_) => "NT$",
            _ => "$",
        }
    }

    pub(crate) fn cents(self, microusd: u64) -> u128 {
        let rate = match self {
            Self::Twd(rate) => rate.millionths,
            _ => 1_000_000,
        };
        (u128::from(microusd) * u128::from(rate) + 5_000_000_000) / 10_000_000_000
    }

    pub(crate) fn format_cents(self, cents: u128) -> String {
        let dollars = crate::popup_window::group_integer_digits(&(cents / 100).to_string());
        format!("{}{dollars}.{:02}", self.symbol(), cents % 100)
    }

    pub(crate) fn format(self, microusd: u64) -> String {
        self.format_cents(self.cents(microusd))
    }

    pub(crate) fn format_activity_cost(self, microusd: u64) -> String {
        if matches!(self, Self::Twd(_)) {
            self.format(microusd)
        } else {
            crate::popup_window::format_usd(microusd as f64 / 1_000_000.0)
        }
    }

    pub(crate) fn format_dollars(self, dollars: f64) -> String {
        let value = match self {
            Self::Twd(rate) => dollars * rate.per_usd(),
            _ => dollars,
        };
        crate::popup_window::format_money(self.symbol(), value)
    }
}

fn parse_rate(body: &str, now: i64) -> Result<TwdRate> {
    let json: serde_json::Value = serde_json::from_str(body)?;
    ensure!(
        json["result"] == "success" && json["base_code"] == "USD",
        "invalid USD rate response"
    );
    let rate = TwdRate {
        millionths: (json["rates"]["TWD"].as_f64().context("missing TWD rate")? * 1_000_000.0)
            .round() as u64,
        updated_at: json["time_last_update_unix"]
            .as_i64()
            .context("missing rate date")?,
        next_update: json["time_next_update_unix"]
            .as_i64()
            .context("missing next update")?,
    };
    ensure!(
        rate.valid(now) && rate.next_update > now,
        "invalid or expired TWD rate"
    );
    Ok(rate)
}

fn fetch_rate(path: &Path) -> Result<TwdRate> {
    let mut body = String::new();
    crate::http::public_agent_builder()
        .timeout(Duration::from_secs(20))
        .build()
        .get(ENDPOINT)
        .call()?
        .into_reader()
        .take(65_536)
        .read_to_string(&mut body)?;
    let rate = parse_rate(&body, Utc::now().timestamp())?;
    if let Err(error) = save_rate(path, rate) {
        crate::logger::info(format!("Could not cache TWD exchange rate: {error:#}"));
    }
    Ok(rate)
}

fn save_rate(path: &Path, rate: TwdRate) -> Result<()> {
    let mut file =
        tempfile::NamedTempFile::new_in(path.parent().context("missing cache directory")?)?;
    file.write_all(&serde_json::to_vec(&rate)?)?;
    file.persist(path)?;
    Ok(())
}

/// The popup bridge owns scheduling and publishes a single rate with each UI snapshot.
pub(crate) struct RateRefresh {
    path: PathBuf,
    rate: Option<TwdRate>,
    pending: Option<JoinHandle<Result<TwdRate>>>,
    next_attempt: Instant,
}

impl RateRefresh {
    pub(crate) fn new(path: PathBuf) -> Self {
        let rate = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<TwdRate>(&bytes).ok())
            .filter(|rate| rate.valid(Utc::now().timestamp()));
        Self {
            path,
            rate,
            pending: None,
            next_attempt: Instant::now(),
        }
    }

    pub(crate) fn tick(&mut self, enabled: bool) -> CostDisplay {
        if self.pending.as_ref().is_some_and(|task| task.is_finished()) {
            match self.pending.take().expect("finished rate request").join() {
                Ok(Ok(rate)) => self.rate = Some(rate),
                Ok(Err(error)) => {
                    crate::logger::info(format!("TWD exchange rate refresh failed: {error:#}"))
                }
                Err(_) => crate::logger::info("TWD exchange rate worker stopped unexpectedly"),
            }
        }
        if !enabled {
            return CostDisplay::Usd;
        }
        if self.pending.is_none()
            && Instant::now() >= self.next_attempt
            && self
                .rate
                .is_none_or(|rate| Utc::now().timestamp() >= rate.next_update)
        {
            self.next_attempt = Instant::now() + RETRY_INTERVAL;
            let path = self.path.clone();
            self.pending = Some(thread::spawn(move || fetch_rate(&path)));
        }
        self.rate.map_or(CostDisplay::AwaitingTwd, CostDisplay::Twd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_700_000_100;
    const RESPONSE: &str = r#"{"result":"success","base_code":"USD","rates":{"TWD":32.5},"time_last_update_unix":1700000000,"time_next_update_unix":1700086400}"#;

    #[test]
    fn converts_before_rounding_and_keeps_usd_when_no_rate_is_available() {
        let twd = CostDisplay::Twd(parse_rate(RESPONSE, NOW).unwrap());
        assert_eq!(twd.format(5_000), "NT$0.16");
        assert_eq!(twd.format(100_000_000), "NT$3,250.00");
        assert_eq!(twd.format(u64::MAX), "NT$599,519,182,395,560.43");
        assert_eq!(CostDisplay::AwaitingTwd.format(5_000), "$0.01");
    }

    #[test]
    fn rejects_invalid_and_expired_responses() {
        for invalid in [
            RESPONSE.replace("32.5", "0"),
            RESPONSE.replace("32.5", "-1"),
            RESPONSE.replace("USD", "EUR"),
            RESPONSE.replace("TWD", "JPY"),
        ] {
            assert!(parse_rate(&invalid, NOW).is_err());
        }
        assert!(parse_rate(RESPONSE, 1_700_086_400).is_err());
    }

    #[test]
    fn cached_rate_is_only_used_when_twd_is_selected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("twd-exchange-rate.json");
        let mut rate = parse_rate(RESPONSE, NOW).unwrap();
        rate.next_update = Utc::now().timestamp() + 3600;
        save_rate(&path, rate).unwrap();

        let mut refresh = RateRefresh::new(path);
        assert_eq!(refresh.tick(false), CostDisplay::Usd);
        assert_eq!(refresh.tick(true), CostDisplay::Twd(rate));
        assert_eq!(refresh.tick(false), CostDisplay::Usd);
        assert!(refresh.pending.is_none());
    }
}
