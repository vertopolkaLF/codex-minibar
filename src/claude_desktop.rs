//! Reads the Claude desktop app's local OAuth session.
//!
//! The Claude Code CLI writes a plain `~/.claude/.credentials.json`, but the
//! desktop app keeps its tokens in `%APPDATA%\Claude\config.json` under
//! Chromium's OSCrypt "v10" envelope: an AES-256-GCM blob whose key lives in
//! `Local State` wrapped by DPAPI for the current Windows user. A desktop-only
//! install therefore has no credentials file for the CLI path to read.
//!
//! Nothing here leaves the machine. The decrypted token is used for the same
//! `api.anthropic.com` OAuth requests the CLI session already performs, and
//! DPAPI keeps the envelope readable only under the signed-in Windows account.

use std::{fs, path::PathBuf};

use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{DateTime, Utc};
use directories::BaseDirs;
use serde::Deserialize;
use serde_json::Value;

/// OSCrypt tags its AES-256-GCM envelopes with this prefix.
const OSCRYPT_V10_PREFIX: &[u8] = b"v10";
/// DPAPI-wrapped master keys in `Local State` carry this prefix.
const DPAPI_PREFIX: &[u8] = b"DPAPI";
const GCM_NONCE_LEN: usize = 12;
const GCM_TAG_LEN: usize = 16;
/// Only the Claude Code-scoped session can read the OAuth usage endpoint, so
/// prefer it over the plain desktop-chat token when both are cached.
const CLAUDE_CODE_SCOPE: &str = "claude_code";

/// A decrypted desktop OAuth session. `subscription_type` and `rate_limit_tier`
/// ride along in the token cache, so the desktop path can label the plan
/// without the extra account-settings request the CLI path needs.
pub struct DesktopSession {
    pub access_token: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub subscription_type: Option<String>,
    pub rate_limit_tier: Option<String>,
}

/// `%APPDATA%\Claude` — the desktop app's Electron user-data directory.
/// Every known Claude Desktop user-data directory. The direct installer uses
/// `%APPDATA%\\Claude`, while the Microsoft Store/MSIX build is sandboxed under
/// `%LOCALAPPDATA%\\Packages\\Claude_pzs8sxrjxfjjc\\LocalCache\\Roaming\\Claude`.
/// The latter is discovered from the package identity rather than an installed
/// version, because WindowsApps versions change on every Store update.
fn config_roots() -> Vec<PathBuf> {
    let mut roots = BaseDirs::new()
        .map(|directories| vec![directories.config_dir().join("Claude")])
        .unwrap_or_default();

    #[cfg(windows)]
    if let Some(packages) = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|local| local.join("Packages"))
    {
        if let Ok(entries) = fs::read_dir(packages) {
            for entry in entries.flatten() {
                let package_name = entry.file_name();
                let Some(package_name) = package_name.to_str() else {
                    continue;
                };
                if is_store_package_name(package_name) {
                    roots.push(
                        entry
                            .path()
                            .join("LocalCache")
                            .join("Roaming")
                            .join("Claude"),
                    );
                }
            }
        }
    }

    roots.sort();
    roots.dedup();
    roots
}

fn is_store_package_name(name: &str) -> bool {
    name.starts_with("Claude_") && name.ends_with("pzs8sxrjxfjjc")
}

/// The `claude.exe` the desktop app unpacks for its embedded Claude Code. This
/// is the only launcher available when the CLI was never installed on PATH.
pub fn bundled_cli() -> Option<PathBuf> {
    let mut best: Option<(semver::Version, PathBuf)> = None;
    for root in config_roots() {
        let Ok(entries) = fs::read_dir(root.join("claude-code")) else {
            continue;
        };
        for entry in entries.flatten() {
            let executable = entry.path().join("claude.exe");
            if !executable.is_file() {
                continue;
            }
            let Some(version) = entry
                .file_name()
                .to_str()
                .and_then(|name| semver::Version::parse(name).ok())
            else {
                continue;
            };
            if best
                .as_ref()
                .is_none_or(|(best_version, _)| version > *best_version)
            {
                best = Some((version, executable));
            }
        }
    }
    best.map(|(_, executable)| executable)
}

/// True when the desktop app is present, whether or not it can be decrypted.
/// Used only as an installation signal, so a cached session is enough.
pub fn is_installed() -> bool {
    if bundled_cli().is_some() {
        return true;
    }
    config_roots()
        .into_iter()
        .any(|root| read_config(&root).is_ok_and(|config| config.token_caches().next().is_some()))
}

pub fn load_session() -> Result<DesktopSession> {
    let mut best: Option<(SessionRank, DesktopSession)> = None;
    let mut errors = Vec::new();
    for root in config_roots() {
        let config = match read_config(&root) {
            Ok(config) => config,
            Err(error) => {
                errors.push(format!("{}: {error:#}", root.display()));
                continue;
            }
        };
        let master_key = match master_key(&root) {
            Ok(master_key) => master_key,
            Err(error) => {
                errors.push(format!("{}: {error:#}", root.display()));
                continue;
            }
        };
        for envelope in config.token_caches() {
            // A cache that fails to decrypt or parse is not fatal: the other cache
            // key, or the CLI credentials file, may still hold a usable session.
            let Ok(entries) = decrypt_token_cache(envelope, &master_key) else {
                continue;
            };
            for (scopes, entry) in entries {
                let Some(session) = entry.into_session() else {
                    continue;
                };
                let rank = SessionRank::of(&scopes, &session);
                if best.as_ref().is_none_or(|(best_rank, _)| rank > *best_rank) {
                    best = Some((rank, session));
                }
            }
        }
    }
    best.map(|(_, session)| session).with_context(|| {
        if errors.is_empty() {
            "the Claude desktop app has no usable OAuth session; open Claude and sign in first".to_owned()
        } else {
            format!(
                "the Claude desktop app has no usable OAuth session; open Claude and sign in first ({})",
                errors.join("; ")
            )
        }
    })
}

/// Ordering used to pick between cached sessions: Claude Code scope first, then
/// a live token over an expired one, then the latest expiry.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct SessionRank {
    claude_code_scope: bool,
    unexpired: bool,
    expires_at: Option<DateTime<Utc>>,
}

impl SessionRank {
    fn of(scopes: &str, session: &DesktopSession) -> Self {
        Self {
            claude_code_scope: scopes.contains(CLAUDE_CODE_SCOPE),
            unexpired: session
                .expires_at
                .is_none_or(|expires_at| expires_at > Utc::now()),
            expires_at: session.expires_at,
        }
    }
}

#[derive(Deserialize)]
struct DesktopConfig {
    #[serde(rename = "oauth:tokenCacheV2")]
    token_cache_v2: Option<String>,
    #[serde(rename = "oauth:tokenCache")]
    token_cache: Option<String>,
}

impl DesktopConfig {
    fn token_caches(&self) -> impl Iterator<Item = &str> {
        [self.token_cache_v2.as_deref(), self.token_cache.as_deref()]
            .into_iter()
            .flatten()
            .filter(|envelope| !envelope.trim().is_empty())
    }
}

#[derive(Deserialize)]
struct CachedToken {
    token: Option<String>,
    #[serde(rename = "expiresAt")]
    expires_at_millis: Option<i64>,
    #[serde(rename = "subscriptionType")]
    subscription_type: Option<String>,
    #[serde(rename = "rateLimitTier")]
    rate_limit_tier: Option<String>,
}

impl CachedToken {
    fn into_session(self) -> Option<DesktopSession> {
        let access_token = self.token?.trim().to_owned();
        if access_token.is_empty() {
            return None;
        }
        Some(DesktopSession {
            access_token,
            expires_at: self
                .expires_at_millis
                .and_then(DateTime::from_timestamp_millis),
            subscription_type: self.subscription_type,
            rate_limit_tier: self.rate_limit_tier,
        })
    }
}

fn read_config(root: &PathBuf) -> Result<DesktopConfig> {
    let path = root.join("config.json");
    let contents = fs::read(&path).with_context(|| {
        format!(
            "read {} (install the Claude desktop app first)",
            path.display()
        )
    })?;
    serde_json::from_slice(&contents).with_context(|| format!("parse {}", path.display()))
}

#[derive(Deserialize)]
struct LocalState {
    os_crypt: Option<OsCrypt>,
}

#[derive(Deserialize)]
struct OsCrypt {
    encrypted_key: Option<String>,
}

fn master_key(root: &PathBuf) -> Result<Vec<u8>> {
    let path = root.join("Local State");
    let contents = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    let state: LocalState =
        serde_json::from_slice(&contents).with_context(|| format!("parse {}", path.display()))?;
    let encoded = state
        .os_crypt
        .and_then(|os_crypt| os_crypt.encrypted_key)
        .context("Claude desktop Local State has no os_crypt key")?;
    let wrapped = STANDARD
        .decode(encoded.trim())
        .context("decode the Claude desktop os_crypt key")?;
    let wrapped = wrapped
        .strip_prefix(DPAPI_PREFIX)
        .context("Claude desktop os_crypt key is not DPAPI-wrapped")?;
    let key = unprotect(wrapped).context("unwrap the Claude desktop os_crypt key with DPAPI")?;
    anyhow::ensure!(
        key.len() == 32,
        "Claude desktop os_crypt key is {} bytes, expected 32",
        key.len()
    );
    Ok(key)
}

fn decrypt_token_cache(envelope: &str, master_key: &[u8]) -> Result<Vec<(String, CachedToken)>> {
    let plaintext = decrypt_oscrypt_v10(envelope, master_key)?;
    // Keys are `<clientId>:<accountUuid>:<baseUrl>:<scopes>`; only the scope
    // tail matters here, and it is the part that survives account changes.
    let entries: Value =
        serde_json::from_slice(&plaintext).context("parse the Claude desktop token cache")?;
    let Value::Object(entries) = entries else {
        bail!("the Claude desktop token cache is not a JSON object");
    };
    Ok(entries
        .into_iter()
        .filter_map(|(scopes, value)| {
            let token = serde_json::from_value::<CachedToken>(value).ok()?;
            Some((scopes, token))
        })
        .collect())
}

fn decrypt_oscrypt_v10(envelope: &str, master_key: &[u8]) -> Result<Vec<u8>> {
    let blob = STANDARD
        .decode(envelope.trim())
        .context("decode a Claude desktop token cache envelope")?;
    let body = blob
        .strip_prefix(OSCRYPT_V10_PREFIX)
        .context("Claude desktop token cache is not an OSCrypt v10 envelope")?;
    anyhow::ensure!(
        body.len() > GCM_NONCE_LEN + GCM_TAG_LEN,
        "Claude desktop token cache envelope is truncated"
    );
    let (nonce, sealed) = body.split_at(GCM_NONCE_LEN);
    // `aes-gcm` expects the tag appended to the ciphertext, which is exactly
    // the OSCrypt layout, so `sealed` is passed through untouched.
    Aes256Gcm::new_from_slice(master_key)
        .map_err(|_| anyhow!("Claude desktop os_crypt key is not a valid AES-256 key"))?
        .decrypt(Nonce::from_slice(nonce), sealed)
        .map_err(|_| anyhow!("could not decrypt the Claude desktop token cache"))
}

/// `CryptUnprotectData` for the current user. The DPAPI blob is bound to the
/// Windows account, so this only ever succeeds for the signed-in user.
#[cfg(windows)]
fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB},
    };

    let mut input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(data.len()).context("DPAPI blob is too large")?,
        pbData: data.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // SAFETY: both blobs are initialized and live across the call, and the
    // out-blob is only read after a success return.
    let succeeded = unsafe {
        CryptUnprotectData(
            &mut input,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            &mut output,
        )
    } != 0;
    if !succeeded {
        bail!(
            "CryptUnprotectData failed: {}",
            std::io::Error::last_os_error()
        );
    }
    // SAFETY: CryptUnprotectData reported a buffer of `cbData` readable bytes.
    let key = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    // SAFETY: the out-blob buffer is DPAPI-allocated with LocalAlloc.
    unsafe { LocalFree(output.pbData.cast()) };
    Ok(key)
}

#[cfg(not(windows))]
fn unprotect(_data: &[u8]) -> Result<Vec<u8>> {
    bail!("the Claude desktop token cache is only readable on Windows")
}

#[cfg(test)]
mod tests {
    use aes_gcm::aead::Payload;

    use super::*;

    fn seal(key: &[u8], nonce: &[u8; GCM_NONCE_LEN], plaintext: &str) -> String {
        let sealed = Aes256Gcm::new_from_slice(key)
            .unwrap()
            .encrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: plaintext.as_bytes(),
                    aad: &[],
                },
            )
            .unwrap();
        let mut blob = OSCRYPT_V10_PREFIX.to_vec();
        blob.extend_from_slice(nonce);
        blob.extend_from_slice(&sealed);
        STANDARD.encode(blob)
    }

    #[test]
    fn decrypts_an_oscrypt_v10_envelope() {
        let key = [7u8; 32];
        let envelope = seal(&key, &[3u8; GCM_NONCE_LEN], r#"{"a":1}"#);
        assert_eq!(decrypt_oscrypt_v10(&envelope, &key).unwrap(), br#"{"a":1}"#);
    }

    #[test]
    fn rejects_envelopes_without_the_v10_prefix_or_the_right_key() {
        let key = [7u8; 32];
        let envelope = seal(&key, &[3u8; GCM_NONCE_LEN], "{}");
        assert!(decrypt_oscrypt_v10(&envelope, &[8u8; 32]).is_err());
        assert!(decrypt_oscrypt_v10(&STANDARD.encode(b"v11nope"), &key).is_err());
        assert!(decrypt_oscrypt_v10(&STANDARD.encode(b"v10short"), &key).is_err());
    }

    #[test]
    fn reads_every_cached_entry_and_keeps_its_scopes() {
        let key = [7u8; 32];
        let envelope = seal(
            &key,
            &[3u8; GCM_NONCE_LEN],
            r#"{"cid:acct:https://api.anthropic.com:user:profile:claude_code":{"token":"live","expiresAt":1787746743179,"subscriptionType":"max","rateLimitTier":"default_max"},"cid:acct:https://api.anthropic.com:user:profile":{"token":"chat"}}"#,
        );
        let entries = decrypt_token_cache(&envelope, &key).unwrap();
        assert_eq!(entries.len(), 2);
        let scoped = entries
            .into_iter()
            .find(|(scopes, _)| scopes.contains(CLAUDE_CODE_SCOPE))
            .expect("the Claude Code entry survives parsing");
        let session = scoped.1.into_session().unwrap();
        assert_eq!(session.access_token, "live");
        assert_eq!(session.subscription_type.as_deref(), Some("max"));
        assert_eq!(session.rate_limit_tier.as_deref(), Some("default_max"));
    }

    #[test]
    fn prefers_the_claude_code_scope_then_a_live_token_then_the_latest_expiry() {
        let live = DesktopSession {
            access_token: "live".into(),
            expires_at: Some(Utc::now() + chrono::Duration::hours(1)),
            subscription_type: None,
            rate_limit_tier: None,
        };
        let expired = DesktopSession {
            access_token: "expired".into(),
            expires_at: Some(Utc::now() - chrono::Duration::hours(1)),
            subscription_type: None,
            rate_limit_tier: None,
        };
        let scoped_expired = SessionRank::of("user:profile:claude_code", &expired);
        let unscoped_live = SessionRank::of("user:profile", &live);
        assert!(scoped_expired > unscoped_live);
        assert!(SessionRank::of("claude_code", &live) > scoped_expired);

        let later = DesktopSession {
            access_token: "later".into(),
            expires_at: Some(Utc::now() + chrono::Duration::hours(5)),
            subscription_type: None,
            rate_limit_tier: None,
        };
        assert!(SessionRank::of("claude_code", &later) > SessionRank::of("claude_code", &live));
    }

    #[test]
    fn discards_entries_without_a_token() {
        assert!(CachedToken {
            token: Some("  ".into()),
            expires_at_millis: None,
            subscription_type: None,
            rate_limit_tier: None,
        }
        .into_session()
        .is_none());
    }

    #[test]
    fn recognizes_the_microsoft_store_package_identity() {
        assert!(is_store_package_name("Claude_pzs8sxrjxfjjc"));
        assert!(is_store_package_name(
            "Claude_1.24012.9.0_x64__pzs8sxrjxfjjc"
        ));
        assert!(!is_store_package_name(
            "Claude_1.24012.9.0_x64__otherpublisher"
        ));
        assert!(!is_store_package_name("NotClaude_pzs8sxrjxfjjc"));
    }
}
