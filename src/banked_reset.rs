//! Account-scoped reset redemption. Unanswered requests keep their key and
//! grant so a manual retry cannot accidentally spend a second credit.
use anyhow::{Result, bail};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

/// Match T3's login-directory isolation. Organization IDs alone may be shared
/// by different team members, whose pending requests must stay independent.
pub(crate) fn account_key(driver: &str, folder: &std::path::Path, account: &str) -> String {
    let folder = folder
        .canonicalize()
        .unwrap_or_else(|_| folder.to_path_buf());
    let folder = folder.to_string_lossy();
    let folder = if cfg!(windows) {
        folder.to_lowercase()
    } else {
        folder.into_owned()
    };
    format!("{driver}:{folder}:{account}")
}

pub(crate) fn consume(
    instance: &crate::instances::ProviderInstance,
) -> Result<(Outcome, Result<crate::limits::RateLimits>)> {
    match instance.driver {
        crate::settings::ProviderKind::Codex => {
            let executable = crate::codex::first_available(instance.binary_path.as_deref())?;
            let client = crate::codex::CodexClient::for_instance(executable, instance);
            let outcome = client.consume_reset()?;
            Ok((outcome, client.read_rate_limits()))
        }
        crate::settings::ProviderKind::Claude => {
            let mut client = crate::claude::ClaudeClient::for_instance(instance);
            let outcome = client.consume_reset()?;
            Ok((outcome, client.read_rate_limits()))
        }
        _ => bail!("Provider does not support reset credits"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Reset,
    NothingToReset,
    NoCredit,
    AlreadyRedeemed,
}

impl Outcome {
    pub fn message_key(self) -> &'static str {
        match self {
            Self::Reset => "reset-applied",
            Self::NothingToReset => "reset-nothing-to-reset",
            Self::NoCredit => "reset-no-credit",
            Self::AlreadyRedeemed => "reset-already-redeemed",
        }
    }
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "reset" => Ok(Self::Reset),
            "nothingToReset" | "not_limited" => Ok(Self::NothingToReset),
            "noCredit" | "ineligible" => Ok(Self::NoCredit),
            "alreadyRedeemed" | "already_used" => Ok(Self::AlreadyRedeemed),
            _ => bail!("{}", crate::i18n::tr("reset-unconfirmed")),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Status {
    Outcome(Outcome),
    RefreshFailed,
    Failure(&'static str),
    Error(String),
}
impl Status {
    pub(crate) fn from_error(error: anyhow::Error) -> Self {
        error.downcast_ref::<Settled>().map_or_else(
            || Self::Error(error.to_string()),
            |error| Self::Failure(error.0),
        )
    }
    pub(crate) fn message(&self) -> String {
        match self {
            Self::Outcome(outcome) => crate::i18n::tr(outcome.message_key()).into(),
            Self::RefreshFailed => crate::i18n::tr("reset-applied-refresh-failed").into(),
            Self::Failure(key) => crate::i18n::tr(key).into(),
            Self::Error(message) => message.clone(),
        }
    }
}

#[derive(Default)]
struct Attempt {
    pending: Option<(String, String)>,
}

pub(crate) fn redeem(
    account: String,
    grant: String,
    consume: impl FnOnce(&str, &str) -> Result<Outcome>,
) -> Result<Outcome> {
    static ACCOUNTS: OnceLock<Mutex<HashMap<String, Arc<Mutex<Attempt>>>>> = OnceLock::new();
    let state = ACCOUNTS
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .entry(account)
        .or_default()
        .clone();
    // Reject overlapping clicks from another instance of the same account.
    let mut state = state
        .try_lock()
        .map_err(|_| anyhow::anyhow!(crate::i18n::tr("reset-in-progress")))?;
    redeem_attempt(&mut state, grant, consume)
}

fn redeem_attempt(
    state: &mut Attempt,
    grant: String,
    consume: impl FnOnce(&str, &str) -> Result<Outcome>,
) -> Result<Outcome> {
    let (key, grant) = state.pending.get_or_insert_with(|| {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        (
            format!(
                "minibar-{}-{nanos:x}-{:x}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ),
            grant,
        )
    });
    let result = consume(key, grant);
    if result.is_ok()
        || result
            .as_ref()
            .err()
            .is_some_and(|e| e.downcast_ref::<Settled>().is_some())
    {
        state.pending = None;
    }
    result
}

#[derive(Debug)]
struct Settled(&'static str);
impl std::fmt::Display for Settled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(crate::i18n::tr(self.0))
    }
}
impl std::error::Error for Settled {}
pub(crate) fn settled(key: &'static str) -> anyhow::Error {
    anyhow::Error::new(Settled(key))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reset_account_key_canonicalizes_folder_without_merging_team_logins() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let key = account_key("claude", first.path(), "team");
        assert_eq!(key, account_key("claude", &first.path().join("."), "team"));
        assert_ne!(key, account_key("claude", second.path(), "team"));
        assert_ne!(key, account_key("claude", first.path(), "other-team"));
    }
    #[test]
    fn overlapping_account_claim_is_rejected_without_running_it() {
        redeem("test-overlap".into(), "a".into(), |_, _| {
            let mut ran = false;
            assert!(
                redeem("test-overlap".into(), "b".into(), |_, _| {
                    ran = true;
                    Ok(Outcome::Reset)
                })
                .is_err()
            );
            assert!(!ran);
            Ok(Outcome::Reset)
        })
        .unwrap();
    }
    #[test]
    fn reset_provider_outcomes_are_distinct_and_unknown_is_not_success() {
        for (value, outcome) in [
            ("reset", Outcome::Reset),
            ("not_limited", Outcome::NothingToReset),
            ("nothingToReset", Outcome::NothingToReset),
            ("ineligible", Outcome::NoCredit),
            ("noCredit", Outcome::NoCredit),
            ("already_used", Outcome::AlreadyRedeemed),
            ("alreadyRedeemed", Outcome::AlreadyRedeemed),
        ] {
            assert_eq!(Outcome::parse(value).unwrap(), outcome);
        }
        assert!(Outcome::parse("unavailable").is_err());
        assert!(Outcome::parse("").is_err());
    }
    #[test]
    fn unanswered_retry_keeps_key_and_original_grant() {
        let mut attempt = Attempt::default();
        let mut first = String::new();
        assert!(
            redeem_attempt(&mut attempt, "a".into(), |key, _| {
                first = key.into();
                bail!("timeout")
            })
            .is_err()
        );
        redeem_attempt(&mut attempt, "b".into(), |key, grant| {
            assert_eq!(key, first);
            assert_eq!(grant, "a");
            Ok(Outcome::Reset)
        })
        .unwrap();
        assert!(attempt.pending.is_none());
    }
    #[test]
    fn settled_failure_clears_attempt() {
        let mut attempt = Attempt::default();
        assert!(
            redeem_attempt(&mut attempt, "a".into(), |_, _| Err(settled(
                "reset-cooldown"
            )))
            .is_err()
        );
        assert!(attempt.pending.is_none());
    }
}
