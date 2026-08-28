//! Windows user-scoped protection for provider credentials.

use anyhow::{Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};

/// Protects a UTF-8 secret with Windows DPAPI for the current user.
///
/// The returned value is safe to serialize as configuration data, but remains
/// tied to the Windows account that created it.
#[cfg(windows)]
pub fn protect(value: &str) -> Result<String> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{CRYPT_INTEGER_BLOB, CryptProtectData},
    };

    let bytes = value.as_bytes();
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(bytes.len()).map_err(|_| anyhow::anyhow!("secret is too large"))?,
        pbData: bytes.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // SAFETY: both blobs are initialized and remain alive for the duration of
    // the call. Windows allocates the output buffer on success.
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
    // SAFETY: CryptProtectData reported a buffer of cbData readable bytes.
    let protected = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) };
    let encoded = STANDARD.encode(protected);
    // SAFETY: the output buffer is allocated by Windows and must be released
    // with LocalFree.
    unsafe { LocalFree(output.pbData.cast()) };
    Ok(encoded)
}

/// Decodes and unprotects a value previously returned by [`protect`].
#[cfg(windows)]
pub fn unprotect(encoded: &str) -> Result<String> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{CRYPT_INTEGER_BLOB, CryptUnprotectData},
    };

    let protected = STANDARD
        .decode(encoded.trim())
        .map_err(|_| anyhow::anyhow!("stored secret is not valid base64"))?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(protected.len())
            .map_err(|_| anyhow::anyhow!("stored secret is too large"))?,
        pbData: protected.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // SAFETY: both blobs are initialized and remain alive for the duration of
    // the call. Windows allocates the output buffer on success.
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
    // SAFETY: CryptUnprotectData reported a buffer of cbData readable bytes.
    let plaintext = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) };
    let secret = String::from_utf8(plaintext.to_vec())
        .map_err(|_| anyhow::anyhow!("stored secret is not valid UTF-8"));
    // SAFETY: the output buffer is allocated by Windows and must be released
    // with LocalFree.
    unsafe { LocalFree(output.pbData.cast()) };
    secret
}

#[cfg(not(windows))]
pub fn protect(_value: &str) -> Result<String> {
    bail!("DPAPI secrets are only supported on Windows")
}

#[cfg(not(windows))]
pub fn unprotect(_encoded: &str) -> Result<String> {
    bail!("DPAPI secrets are only supported on Windows")
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(windows)]
    fn protects_and_unprotects_for_the_current_user() {
        let encoded = super::protect("sk-or-v1-test-secret").unwrap();
        assert_ne!(encoded, "sk-or-v1-test-secret");
        assert_eq!(super::unprotect(&encoded).unwrap(), "sk-or-v1-test-secret");
    }
}
