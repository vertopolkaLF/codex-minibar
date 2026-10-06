use super::*;

pub(super) fn provider_icon_tab_count(
    providers: &[ProviderKind],
    claude_accounts: &[crate::settings::ClaudeProfile],
    codex_accounts: &[crate::settings::CodexProfile],
) -> usize {
    let separate_claude = providers.contains(&ProviderKind::Claude) && !claude_accounts.is_empty();
    let separate_codex = providers.contains(&ProviderKind::Codex) && !codex_accounts.is_empty();
    if providers.len() <= 1 && !separate_claude && !separate_codex {
        return 0;
    }
    providers
        .iter()
        .map(|provider| {
            if *provider == ProviderKind::Claude {
                claude_accounts.len().max(1)
            } else if *provider == ProviderKind::Codex {
                codex_accounts.len().max(1)
            } else {
                1
            }
        })
        .sum()
}

pub(super) fn claude_account_tabs(
    saved: &[crate::settings::ClaudeProfile],
) -> Vec<crate::settings::ClaudeProfile> {
    crate::claude::profiles_with_default(saved)
        .into_iter()
        .filter(|profile| profile.enabled)
        .collect()
}

pub(super) fn selected_claude_account_tab<'a>(
    profiles: &'a [crate::settings::ClaudeProfile],
    selected: Option<&str>,
) -> Option<&'a str> {
    profiles
        .iter()
        .find(|profile| Some(profile.id.as_str()) == selected)
        .or_else(|| profiles.first())
        .map(|profile| profile.id.as_str())
}

pub(super) fn codex_account_tabs(
    saved: &[crate::settings::CodexProfile],
) -> Vec<crate::settings::CodexProfile> {
    crate::codex::profiles_with_default(saved)
        .into_iter()
        .filter(|profile| profile.enabled)
        .collect()
}

pub(super) fn selected_codex_account_tab<'a>(
    profiles: &'a [crate::settings::CodexProfile],
    selected: Option<&str>,
) -> Option<&'a str> {
    profiles
        .iter()
        .find(|profile| Some(profile.id.as_str()) == selected)
        .or_else(|| profiles.first())
        .map(|profile| profile.id.as_str())
}
