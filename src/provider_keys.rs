//! API keys Minibar keeps for providers that read quota with one.
//!
//! OpenCode keeps its historical secret slots (`crate::opencode`); the other
//! keyed providers store theirs as `<driver>.api-key`, plus the instance id
//! for additional instances. Every key is sealed by `crate::secrets` (DPAPI on
//! Windows, the login-keyring key on Linux) and never written to settings.

use anyhow::{Context, Result};

use crate::{instances::ProviderId, secrets, settings::ProviderKind};

/// Whether Settings offers an API key for this driver.
pub fn uses_api_key(driver: ProviderKind) -> bool {
    matches!(
        driver,
        ProviderKind::OpenCodeZen
            | ProviderKind::OpenCodeGo
            | ProviderKind::Copilot
            | ProviderKind::Zai
            | ProviderKind::Kimi
            | ProviderKind::DeepSeek
    )
}

/// Whether the provider cannot read anything without a key: no CLI login or
/// local sign-in to fall back on.
pub fn requires_api_key(driver: ProviderKind) -> bool {
    matches!(
        driver,
        ProviderKind::Zai | ProviderKind::Kimi | ProviderKind::DeepSeek
    )
}

/// Environment variables read when no key is saved in Minibar, in order.
pub fn environment_names(driver: ProviderKind) -> &'static [&'static str] {
    match driver {
        ProviderKind::Zai => &["Z_AI_API_KEY", "ZAI_API_KEY"],
        ProviderKind::Kimi => &["KIMI_CODE_API_KEY", "KIMI_API_KEY"],
        ProviderKind::DeepSeek => &["DEEPSEEK_API_KEY"],
        _ => &[],
    }
}

fn is_opencode(driver: ProviderKind) -> bool {
    matches!(driver, ProviderKind::OpenCodeZen | ProviderKind::OpenCodeGo)
}

fn secret_name(provider: ProviderId) -> Result<String> {
    let driver = provider.kind();
    anyhow::ensure!(
        uses_api_key(driver) && !is_opencode(driver),
        "{} does not use an API key",
        driver.display_name()
    );
    Ok(if provider.is_primary() {
        format!("{}.api-key", driver.id())
    } else {
        format!("{}.api-key.{}", driver.id(), provider.id())
    })
}

/// The key saved in Minibar for this instance.
pub fn manual_key(provider: ProviderId) -> Result<Option<String>> {
    if is_opencode(provider.kind()) {
        return crate::opencode::manual_key(provider);
    }
    if !uses_api_key(provider.kind()) {
        return Ok(None);
    }
    secrets::load(&secret_name(provider)?)
}

pub fn save_manual_key(provider: ProviderId, value: Option<&str>) -> Result<()> {
    if is_opencode(provider.kind()) {
        return crate::opencode::save_manual_key(provider, value);
    }
    secrets::save(&secret_name(provider)?, value)
}

/// Replaces the key and captures the previous ciphertext, so a failed
/// settings commit can put it back.
pub(crate) fn apply_manual_key(
    provider: ProviderId,
    value: Option<&str>,
) -> Result<secrets::EncodedRollback> {
    if is_opencode(provider.kind()) {
        return crate::opencode::apply_manual_key(provider, value);
    }
    secrets::apply_with_rollback(&[(secret_name(provider)?, value.map(str::to_owned))])
}

pub(crate) fn restore_manual_key(rollback: secrets::EncodedRollback) -> Result<()> {
    rollback.restore()
}

/// The saved key, else the primary instance's environment variable. Extra
/// instances never fall back to the environment, which would make them read
/// the primary account a second time.
pub fn resolve(provider: ProviderId) -> Result<Option<String>> {
    if let Some(key) = manual_key(provider)?
        .map(|key| key.trim().to_owned())
        .filter(|key| !key.is_empty())
    {
        return Ok(Some(key));
    }
    if !provider.is_primary() {
        return Ok(None);
    }
    Ok(first_environment_value(environment_names(provider.kind())))
}

/// Like [`resolve`], failing with a message that says where to add the key.
pub fn require(provider: ProviderId) -> Result<String> {
    resolve(provider)?.with_context(|| {
        format!(
            "{} needs an API key; add it in Settings → Providers",
            provider.kind().display_name()
        )
    })
}

fn first_environment_value(names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        std::env::var(name)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyed_drivers_get_their_own_secret_slots() {
        let zai = ProviderId::primary(ProviderKind::Zai);
        assert_eq!(secret_name(zai).unwrap(), "zai.api-key");
        let work = ProviderId::new(ProviderKind::DeepSeek, "deepseek-work");
        assert_eq!(secret_name(work).unwrap(), "deepseek.api-key.deepseek-work");
        assert!(secret_name(ProviderId::primary(ProviderKind::Zed)).is_err());
        assert!(secret_name(ProviderId::primary(ProviderKind::OpenCodeZen)).is_err());
    }

    #[test]
    fn key_requirements_match_the_drivers() {
        for driver in ProviderKind::ALL {
            if requires_api_key(driver) {
                assert!(uses_api_key(driver), "{driver:?}");
                assert!(!environment_names(driver).is_empty(), "{driver:?}");
            }
        }
        assert!(uses_api_key(ProviderKind::Copilot));
        assert!(!requires_api_key(ProviderKind::Copilot));
        assert!(!uses_api_key(ProviderKind::Zed));
    }
}
