//! Small Windows-user-scoped secret store for values entered in Settings.
//!
//! Secrets are kept outside the TOML settings file and are protected with
//! DPAPI. The plaintext only exists for the duration of a provider request or
//! an explicit save operation.

use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

const FILE_NAME: &str = "provider-secrets.json";
// Serialize read/modify/replace across provider UI writes and token refreshes.
static OPERATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());
// Ciphertext detects replacements; generations also detect repeated removals
// (where both the old and newer write leave the slot absent).
static GENERATIONS: std::sync::Mutex<BTreeMap<(PathBuf, String), u64>> =
    std::sync::Mutex::new(BTreeMap::new());

fn generation(path: &Path, name: &str) -> u64 {
    *GENERATIONS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(&(path.to_owned(), name.to_owned()))
        .unwrap_or(&0)
}

fn advance_generations(path: &Path, names: impl IntoIterator<Item = impl AsRef<str>>) -> Vec<u64> {
    let mut generations = GENERATIONS
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    names
        .into_iter()
        .map(|name| {
            let revision = generations
                .entry((path.to_owned(), name.as_ref().to_owned()))
                .or_default();
            *revision = revision.wrapping_add(1);
            *revision
        })
        .collect()
}

fn operation_guard() -> Result<std::sync::MutexGuard<'static, ()>> {
    OPERATIONS
        .lock()
        .map_err(|_| anyhow::anyhow!("Provider secret storage is unavailable."))
}

#[derive(Default, Serialize, Deserialize)]
struct SecretFile {
    #[serde(default)]
    values: BTreeMap<String, String>,
}

pub fn load(name: &str) -> Result<Option<String>> {
    let _guard = operation_guard()?;
    let path = path()?;
    load_from(&path, name)
}

fn load_from(path: &Path, name: &str) -> Result<Option<String>> {
    let Some(file) = read_secret_file(path)? else {
        return Ok(None);
    };
    let Some(encoded) = file.values.get(name) else {
        return Ok(None);
    };
    let protected = STANDARD
        .decode(encoded)
        .context("decode protected provider secret")?;
    let plaintext = unprotect(&protected)?;
    let value = String::from_utf8(plaintext).context("provider secret is not UTF-8")?;
    (!value.trim().is_empty())
        .then_some(value)
        .ok_or_else(|| anyhow::anyhow!("provider secret is empty"))
        .map(Some)
}

/// Previous ciphertext for a set of secret names. Restores the exact stored
/// blobs without decrypting, so a corrupt slot can still be replaced or removed.
pub(crate) struct EncodedRollback {
    entries: Vec<(String, Option<String>)>,
    applied: Vec<(String, Option<String>)>,
    generations: Vec<u64>,
}

impl EncodedRollback {
    pub(crate) fn restore(self) -> Result<()> {
        let _guard = operation_guard()?;
        let path = path()?;
        restore_encoded_to(&path, &self)
    }
}

/// Capture the prior ciphertext and publish the batch under one lock. Later
/// rollback compares exact ciphertext so a newer write to a slot always wins.
pub(crate) fn apply_with_rollback(changes: &[(String, Option<String>)]) -> Result<EncodedRollback> {
    let _guard = operation_guard()?;
    apply_with_rollback_to(&path()?, changes)
}

fn apply_with_rollback_to(
    path: &Path,
    changes: &[(String, Option<String>)],
) -> Result<EncodedRollback> {
    let mut file = read_secret_file(path)?.unwrap_or_default();
    let names = changes
        .iter()
        .map(|(name, _)| name)
        .collect::<std::collections::BTreeSet<_>>();
    let entries = names
        .iter()
        .map(|name| ((*name).clone(), file.values.get(*name).cloned()))
        .collect();
    apply_changes(&mut file, changes)?;
    let applied = names
        .iter()
        .map(|name| ((*name).clone(), file.values.get(*name).cloned()))
        .collect();
    commit_secret_file(path, &file)?;
    let generations = advance_generations(path, names);
    Ok(EncodedRollback {
        entries,
        applied,
        generations,
    })
}

pub fn save(name: &str, value: Option<&str>) -> Result<()> {
    save_many(&[(name.to_owned(), value.map(str::to_owned))])
}

/// Applies several secret changes through one protected-file replacement.
/// Validation and DPAPI encryption happen before the existing file changes,
/// so account-level edits cannot leave only part of their keys updated.
pub fn save_many(changes: &[(String, Option<String>)]) -> Result<()> {
    let _guard = operation_guard()?;
    let path = path()?;
    save_many_to(&path, changes)
}

fn save_many_to(path: &Path, changes: &[(String, Option<String>)]) -> Result<()> {
    let mut file = read_secret_file(path)?.unwrap_or_default();
    apply_changes(&mut file, changes)?;
    commit_secret_file(path, &file)?;
    advance_generations(path, changes.iter().map(|(name, _)| name));
    Ok(())
}

fn apply_changes(file: &mut SecretFile, changes: &[(String, Option<String>)]) -> Result<()> {
    for (name, value) in changes {
        match value
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(value) => {
                let protected = protect(value.as_bytes())?;
                file.values
                    .insert(name.to_owned(), STANDARD.encode(protected));
            }
            None => {
                file.values.remove(name);
            }
        }
    }

    Ok(())
}

fn read_secret_file(path: &Path) -> Result<Option<SecretFile>> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| {
                format!("read protected provider secrets from {}", path.display())
            });
        }
    };
    serde_json::from_str(&raw)
        .context("parse protected provider secrets")
        .map(Some)
}

fn restore_encoded_to(path: &Path, rollback: &EncodedRollback) -> Result<()> {
    let mut file = read_secret_file(path)?.unwrap_or_default();
    let mut conflict = false;
    let mut changed = false;
    let mut restored = Vec::new();
    for (((name, encoded), (_, applied)), expected_generation) in rollback
        .entries
        .iter()
        .zip(&rollback.applied)
        .zip(&rollback.generations)
    {
        if file.values.get(name) != applied.as_ref()
            || generation(path, name) != *expected_generation
        {
            conflict = true;
            continue;
        }
        changed = true;
        restored.push(name);
        match encoded {
            Some(encoded) => {
                file.values.insert(name.clone(), encoded.clone());
            }
            None => {
                file.values.remove(name);
            }
        }
    }
    if changed {
        commit_secret_file(path, &file)?;
        advance_generations(path, restored);
    }
    anyhow::ensure!(
        !conflict,
        "A newer provider secret update was preserved; its rollback was skipped."
    );
    Ok(())
}

fn commit_secret_file(path: &Path, file: &SecretFile) -> Result<()> {
    if file.values.is_empty() {
        if path.is_file() {
            fs::remove_file(path).with_context(|| {
                format!("remove empty provider secrets file {}", path.display())
            })?;
        }
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create provider secrets directory {}", parent.display()))?;
    }
    let parent = path
        .parent()
        .context("provider secrets path has no parent directory")?;
    let encoded = serde_json::to_vec_pretty(file).context("serialize provider secrets")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).with_context(|| {
        format!(
            "create temporary provider secrets file in {}",
            parent.display()
        )
    })?;
    temporary
        .write_all(&encoded)
        .context("write temporary provider secrets")?;
    temporary
        .as_file()
        .sync_all()
        .context("flush temporary provider secrets")?;
    temporary
        .persist(path)
        .with_context(|| format!("commit protected provider secrets to {}", path.display()))?;
    Ok(())
}

/// Short display form of a secret: its known prefix plus the last four
/// characters, e.g. `sk-or-v1-…7c1e`. Never reveals the rest of the value.
pub fn masked_hint(value: &str) -> String {
    let value = value.trim();
    let tail: String = {
        let chars: Vec<char> = value.chars().collect();
        chars[chars.len().saturating_sub(4)..].iter().collect()
    };
    let prefix = ["sk-or-v1-", "sk-or-", "sk-"]
        .into_iter()
        .find(|prefix| value.starts_with(prefix) && value.len() > prefix.len() + 4)
        .unwrap_or("");
    format!("{prefix}…{tail}")
}

/// Checks whether a protected secret with the given logical-name prefix is
/// present without decrypting every value in the file.
pub fn contains_prefix(prefix: &str) -> bool {
    let Ok(path) = path() else {
        return false;
    };
    let Ok(raw) = fs::read_to_string(path) else {
        return false;
    };
    serde_json::from_str::<SecretFile>(&raw)
        .ok()
        .is_some_and(|file| file.values.keys().any(|name| name.starts_with(prefix)))
}

fn path() -> Result<PathBuf> {
    ProjectDirs::from("dev", "Codex Minibar", "Codex Minibar")
        .map(|dirs| dirs.config_dir().join(FILE_NAME))
        .context("could not resolve the provider secrets directory")
}

#[cfg(windows)]
fn protect(data: &[u8]) -> Result<Vec<u8>> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{CRYPT_INTEGER_BLOB, CryptProtectData},
    };

    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(data.len()).context("provider secret is too large")?,
        pbData: data.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let succeeded = unsafe {
        CryptProtectData(
            &input,
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            &mut output,
        )
    } != 0;
    if !succeeded {
        bail!(
            "CryptProtectData failed: {}",
            std::io::Error::last_os_error()
        );
    }
    let protected =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe { LocalFree(output.pbData.cast()) };
    Ok(protected)
}

#[cfg(not(windows))]
fn protect(_data: &[u8]) -> Result<Vec<u8>> {
    bail!("manual provider secrets are only supported on Windows")
}

#[cfg(windows)]
fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{CRYPT_INTEGER_BLOB, CryptUnprotectData},
    };

    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(data.len()).context("protected provider secret is too large")?,
        pbData: data.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let succeeded = unsafe {
        CryptUnprotectData(
            &input,
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
    let plaintext =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe { LocalFree(output.pbData.cast()) };
    Ok(plaintext)
}

#[cfg(not(windows))]
fn unprotect(_data: &[u8]) -> Result<Vec<u8>> {
    bail!("manual provider secrets are only supported on Windows")
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn rollback_does_not_overwrite_a_newer_secret_update() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("provider-secrets.json");
        save_many_to(&path, &[("api".into(), Some("original".into()))])?;
        let rollback =
            apply_with_rollback_to(&path, &[("api".into(), Some("first-update".into()))])?;
        save_many_to(&path, &[("api".into(), Some("newer-update".into()))])?;
        assert!(restore_encoded_to(&path, &rollback).is_err());
        assert_eq!(load_from(&path, "api")?.as_deref(), Some("newer-update"));
        Ok(())
    }

    #[test]
    fn rollback_preserves_a_newer_removal_of_an_already_absent_slot() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("provider-secrets.json");
        save_many_to(&path, &[("api".into(), Some("original".into()))])?;
        let rollback = apply_with_rollback_to(&path, &[("api".into(), None)])?;
        save_many_to(&path, &[("api".into(), None)])?;
        assert!(restore_encoded_to(&path, &rollback).is_err());
        assert_eq!(load_from(&path, "api")?, None);
        Ok(())
    }

    #[test]
    fn rollback_restores_its_slots_without_losing_another_provider_update() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("provider-secrets.json");
        save_many_to(&path, &[("api".into(), Some("original".into()))])?;
        let rollback = apply_with_rollback_to(&path, &[("api".into(), Some("temporary".into()))])?;
        save_many_to(&path, &[("other-provider".into(), Some("newer".into()))])?;
        restore_encoded_to(&path, &rollback)?;
        assert_eq!(load_from(&path, "api")?.as_deref(), Some("original"));
        assert_eq!(
            load_from(&path, "other-provider")?.as_deref(),
            Some("newer")
        );
        Ok(())
    }

    #[test]
    fn batch_updates_replace_existing_values_and_remove_them_together() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("provider-secrets.json");
        save_many_to(
            &path,
            &[
                ("management".into(), Some("management-value".into())),
                ("api".into(), Some("api-value".into())),
            ],
        )?;
        assert_eq!(
            load_from(&path, "management")?.as_deref(),
            Some("management-value")
        );
        assert_eq!(load_from(&path, "api")?.as_deref(), Some("api-value"));

        // `persist` requests replacement semantics; verify the Windows path
        // updates an existing destination instead of only creating new files.
        save_many_to(
            &path,
            &[
                ("management".into(), Some("replacement-value".into())),
                ("api".into(), None),
            ],
        )?;
        assert_eq!(
            load_from(&path, "management")?.as_deref(),
            Some("replacement-value")
        );
        assert_eq!(load_from(&path, "api")?, None);

        save_many_to(&path, &[("management".into(), None)])?;
        assert!(!path.exists());
        Ok(())
    }

    #[test]
    fn encoded_rollback_can_replace_and_restore_undecryptable_blobs() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("provider-secrets.json");
        fs::write(
            &path,
            r#"{
  "values": {
    "api": "not-valid-ciphertext"
  }
}"#,
        )?;
        assert!(load_from(&path, "api").is_err());

        let rollback =
            apply_with_rollback_to(&path, &[("api".into(), Some("replacement".into()))])?;
        assert_eq!(load_from(&path, "api")?.as_deref(), Some("replacement"));

        restore_encoded_to(&path, &rollback)?;
        assert!(load_from(&path, "api").is_err());
        assert!(fs::read_to_string(&path)?.contains("not-valid-ciphertext"));
        Ok(())
    }

    #[test]
    fn encoded_rollback_treats_missing_names_as_absent() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("provider-secrets.json");
        let rollback = apply_with_rollback_to(&path, &[("missing".into(), Some("new".into()))])?;
        assert_eq!(rollback.entries, vec![("missing".into(), None)]);

        restore_encoded_to(&path, &rollback)?;
        assert!(!path.exists());
        Ok(())
    }
}
