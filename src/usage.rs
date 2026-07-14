use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{BufRead, BufReader},
    path::Path,
};

use anyhow::Result;
use chrono::{DateTime, Duration, Local, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Locally recorded Codex token usage. This is deliberately derived only from
/// session logs: it never reads credentials or contacts OpenAI directly.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub requests: u64,
}

impl TokenUsage {
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens
            .saturating_add(self.cached_input_tokens)
            .saturating_add(self.output_tokens)
    }

    fn add(&mut self, other: &Self) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.cached_input_tokens = self
            .cached_input_tokens
            .saturating_add(other.cached_input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.requests = self.requests.saturating_add(other.requests);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageStatistics {
    pub today: TokenUsage,
    pub history: TokenUsage,
    pub history_days: u16,
    /// One aggregate per local calendar day, ordered from oldest to newest.
    pub daily: Vec<DailyTokenUsage>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyTokenUsage {
    pub date: NaiveDate,
    pub usage: TokenUsage,
}

impl UsageStatistics {
    pub fn has_data(&self) -> bool {
        self.history.requests > 0
    }

    pub fn tokens_on(&self, date: NaiveDate) -> u64 {
        self.daily
            .iter()
            .find(|entry| entry.date == date)
            .map(|entry| entry.usage.total_tokens())
            .unwrap_or(0)
    }
}

/// Read the token-count events emitted by the local Codex CLI/Desktop session
/// logs. `last_token_usage` is per request, unlike `total_token_usage`, which
/// is cumulative and would otherwise double-count every turn.
pub fn read_usage_statistics(history_days: u16) -> Result<UsageStatistics> {
    let history_days = history_days.clamp(1, 365);
    let now = Local::now();
    let today = now.date_naive();
    let first_day = today - Duration::days(i64::from(history_days.saturating_sub(1)));
    let sessions_root = codex_home().join("sessions");
    let mut stats = UsageStatistics {
        history_days,
        ..Default::default()
    };
    let mut daily = BTreeMap::<NaiveDate, TokenUsage>::new();

    scan_sessions(&sessions_root, &mut |line| {
        let Some((timestamp, usage)) = token_usage_from_line(line) else {
            return;
        };
        let day = timestamp.with_timezone(&Local).date_naive();
        if day < first_day || day > today {
            return;
        }
        stats.history.add(&usage);
        daily.entry(day).or_default().add(&usage);
        if day == today {
            stats.today.add(&usage);
        }
    })?;
    stats.daily = daily
        .into_iter()
        .map(|(date, usage)| DailyTokenUsage { date, usage })
        .collect();

    Ok(stats)
}

fn codex_home() -> std::path::PathBuf {
    std::env::var_os("CODEX_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".codex")))
        .unwrap_or_else(|| std::path::PathBuf::from(".codex"))
}

fn scan_sessions(root: &Path, on_line: &mut impl FnMut(&str)) -> Result<()> {
    let Ok(entries) = fs::read_dir(root) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            scan_sessions(&path, on_line)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "jsonl")
        {
            // Session logs are append-only and line-oriented; invalid/incomplete
            // final lines are simply ignored while Codex is still writing them.
            let Ok(file) = File::open(path) else {
                continue;
            };
            for line in BufReader::new(file).lines() {
                if let Ok(line) = line {
                    on_line(&line);
                }
            }
        }
    }
    Ok(())
}

fn token_usage_from_line(line: &str) -> Option<(DateTime<Utc>, TokenUsage)> {
    let event: Value = serde_json::from_str(line).ok()?;
    if event.get("type")?.as_str()? != "event_msg"
        || event.pointer("/payload/type")?.as_str()? != "token_count"
    {
        return None;
    }
    let timestamp = DateTime::parse_from_rfc3339(event.get("timestamp")?.as_str()?)
        .ok()?
        .with_timezone(&Utc);
    let usage = event.pointer("/payload/info/last_token_usage")?;
    let token = |name: &str| usage.get(name).and_then(Value::as_u64).unwrap_or(0);
    Some((
        timestamp,
        TokenUsage {
            input_tokens: token("input_tokens"),
            cached_input_tokens: token("cached_input_tokens").max(token("cache_read_input_tokens")),
            output_tokens: token("output_tokens"),
            requests: 1,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_per_request_token_usage() {
        let line = r#"{"timestamp":"2026-07-14T10:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":12,"cached_input_tokens":7,"output_tokens":4}}}}"#;
        let (_, usage) = token_usage_from_line(line).unwrap();
        assert_eq!(usage.total_tokens(), 23);
        assert_eq!(usage.requests, 1);
    }

    #[test]
    fn ignores_non_usage_events() {
        assert!(
            token_usage_from_line(r#"{"type":"event_msg","payload":{"type":"task_started"}}"#)
                .is_none()
        );
    }

    #[test]
    fn token_usage_saturates() {
        let usage = TokenUsage {
            input_tokens: u64::MAX,
            cached_input_tokens: 1,
            output_tokens: 1,
            requests: 0,
        };
        assert_eq!(usage.total_tokens(), u64::MAX);
    }

    #[test]
    fn looks_up_daily_token_totals() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 14).unwrap();
        let stats = UsageStatistics {
            daily: vec![DailyTokenUsage {
                date,
                usage: TokenUsage {
                    input_tokens: 10,
                    output_tokens: 5,
                    ..Default::default()
                },
            }],
            ..Default::default()
        };
        assert_eq!(stats.tokens_on(date), 15);
    }
}
