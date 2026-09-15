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

#[derive(Default, Serialize, Deserialize)]
struct SecretFile {
    #[serde(default)]
    values: BTreeMap<String, String>,
}

pub fn load(name: &str) -> Result<Option<String>> {
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
}

impl EncodedRollback {
    pub(crate) fn capture(names: impl IntoIterator<Item = impl AsRef<str>>) -> Result<Self> {
        let path = path()?;
        let names = names
            .into_iter()
            .map(|name| name.as_ref().to_owned())
            .collect::<Vec<_>>();
        capture_encoded_from(&path, &names)
    }

    pub(crate) fn restore(self) -> Result<()> {
        let path = path()?;
        restore_encoded_to(&path, &self.entries)
    }
}

pub fn save(name: &str, value: Option<&str>) -> Result<()> {
    save_many(&[(name.to_owned(), value.map(str::to_owned))])
}

/// Applies several secret changes through one protected-file replacement.
/// Validation and DPAPI encryption happen before the existing file changes,
/// so account-level edits cannot leave only part of their keys updated.
pub fn save_many(changes: &[(String, Option<String>)]) -> Result<()> {
    let path = path()?;
    save_many_to(&path, changes)
}

fn save_many_to(path: &Path, changes: &[(String, Option<String>)]) -> Result<()> {
    let mut file = read_secret_file(path)?.unwrap_or_default();

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

    commit_secret_file(path, &file)
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

fn capture_encoded_from(path: &Path, names: &[String]) -> Result<EncodedRollback> {
    let file = read_secret_file(path)?;
    Ok(EncodedRollback {
        entries: names
            .iter()
            .map(|name| {
                let encoded = file
                    .as_ref()
                    .and_then(|file| file.values.get(name).cloned());
                (name.clone(), encoded)
            })
            .collect(),
    })
}

fn restore_encoded_to(path: &Path, entries: &[(String, Option<String>)]) -> Result<()> {
    let mut file = read_secret_file(path)?.unwrap_or_default();
    for (name, encoded) in entries {
        match encoded {
            Some(encoded) => {
                file.values.insert(name.clone(), encoded.clone());
            }
            None => {
                file.values.remove(name);
            }
        }
    }
    commit_secret_file(path, &file)
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

        let names = vec!["api".to_string()];
        let rollback = capture_encoded_from(&path, &names)?;
        save_many_to(&path, &[("api".into(), Some("replacement".into()))])?;
        assert_eq!(load_from(&path, "api")?.as_deref(), Some("replacement"));

        restore_encoded_to(&path, &rollback.entries)?;
        assert!(load_from(&path, "api").is_err());
        assert!(fs::read_to_string(&path)?.contains("not-valid-ciphertext"));
        Ok(())
    }

    #[test]
    fn encoded_rollback_treats_missing_names_as_absent() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("provider-secrets.json");
        let names = vec!["missing".to_string()];
        let rollback = capture_encoded_from(&path, &names)?;
        assert_eq!(rollback.entries, vec![("missing".into(), None)]);

        save_many_to(&path, &[("missing".into(), Some("new".into()))])?;
        restore_encoded_to(&path, &rollback.entries)?;
        assert!(!path.exists());
        Ok(())
    }
}
