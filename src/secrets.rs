//! Small Windows-user-scoped secret store for values entered in Settings.
//!
//! Secrets are kept outside the TOML settings file and are protected with
//! DPAPI. The plaintext only exists for the duration of a provider request or
//! an explicit save operation.

use std::{collections::BTreeMap, fs, path::PathBuf};

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
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("read protected provider secrets from {}", path.display()))?;
    let file: SecretFile =
        serde_json::from_str(&raw).context("parse protected provider secrets")?;
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

pub fn save(name: &str, value: Option<&str>) -> Result<()> {
    let path = path()?;
    let mut file = if path.is_file() {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("read protected provider secrets from {}", path.display()))?;
        serde_json::from_str::<SecretFile>(&raw).context("parse protected provider secrets")?
    } else {
        SecretFile::default()
    };

    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => {
            let protected = protect(value.as_bytes())?;
            file.values
                .insert(name.to_owned(), STANDARD.encode(protected));
        }
        None => {
            file.values.remove(name);
        }
    }

    if file.values.is_empty() {
        if path.is_file() {
            fs::remove_file(&path).with_context(|| {
                format!("remove empty provider secrets file {}", path.display())
            })?;
        }
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create provider secrets directory {}", parent.display()))?;
    }
    let encoded = serde_json::to_vec_pretty(&file).context("serialize provider secrets")?;
    fs::write(&path, encoded)
        .with_context(|| format!("write protected provider secrets to {}", path.display()))?;
    Ok(())
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
