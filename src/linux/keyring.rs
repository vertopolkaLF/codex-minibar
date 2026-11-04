//! Encryption for provider secrets at rest.
//!
//! Windows protects each secret with DPAPI, keyed to the user account. Linux
//! has no per-user system key, so a random 256-bit master key is kept in the
//! user's login keyring (Secret Service, through oo7) and secrets are sealed
//! with AES-256-GCM. Without a Secret Service the key lives in a file only
//! the user can read, next to the secrets file.

use std::{path::PathBuf, sync::OnceLock};

use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use anyhow::{Context, Result, anyhow, bail, ensure};
use zeroize::Zeroizing;

/// Identifies sealed values; bumping it changes the format.
const MAGIC: &[u8] = b"CMBL1";
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
const KEY_FILE: &str = "secrets.key";

fn attributes() -> Vec<(&'static str, &'static str)> {
    vec![
        ("application", "codex-minibar"),
        ("purpose", "provider-secrets-key"),
    ]
}

/// The master key, loaded or created once per process.
fn master_key(allow_create: bool) -> Result<&'static Zeroizing<[u8; KEY_LEN]>> {
    static KEY: OnceLock<Zeroizing<[u8; KEY_LEN]>> = OnceLock::new();
    if let Some(key) = KEY.get() {
        return Ok(key);
    }
    let key = load_or_create_key(allow_create)?;
    Ok(KEY.get_or_init(|| key))
}

fn load_or_create_key(allow_create: bool) -> Result<Zeroizing<[u8; KEY_LEN]>> {
    with_key_file(&key_file()?, || load_keyring(allow_create))
}

fn with_key_file(
    path: &std::path::Path,
    fallback: impl FnOnce() -> Result<Zeroizing<[u8; KEY_LEN]>>,
) -> Result<Zeroizing<[u8; KEY_LEN]>> {
    // This key sealed the secrets while Secret Service was unavailable.
    // A stale or newly available keyring item must never override it.
    if let Some(key) = read_key_file(path)? {
        return Ok(key);
    }
    fallback()
}

fn load_keyring(allow_create: bool) -> Result<Zeroizing<[u8; KEY_LEN]>> {
    let keyring = futures::executor::block_on(oo7::Keyring::new());
    if let Ok(keyring) = &keyring
        && let Some(key) = futures::executor::block_on(read_keyring(keyring))?
    {
        return Ok(key);
    }
    ensure_key_creation_allowed(allow_create, crate::secrets::has_stored_secrets)?;
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    super::random_bytes(key.as_mut()).context("generate secrets key")?;
    match &keyring {
        Ok(keyring) => match futures::executor::block_on(write_keyring(keyring, &key)) {
            Ok(()) => return Ok(key),
            Err(error) => crate::logger::info(format!(
                "login keyring unavailable, keeping the secrets key in a file: {error:#}"
            )),
        },
        Err(error) => crate::logger::info(format!(
            "login keyring unavailable, keeping the secrets key in a file: {error}"
        )),
    }
    write_key_file(&key)?;
    Ok(key)
}

fn ensure_key_creation_allowed(
    allow_create: bool,
    has_secrets: impl FnOnce() -> Result<bool>,
) -> Result<()> {
    ensure!(
        allow_create,
        "provider secrets key is unavailable; unlock the login keyring and retry"
    );
    ensure!(
        !has_secrets()?,
        "saved provider secrets already exist but their key is unavailable; unlock the login keyring and retry"
    );
    Ok(())
}

async fn read_keyring(keyring: &oo7::Keyring) -> Result<Option<Zeroizing<[u8; KEY_LEN]>>> {
    keyring
        .unlock()
        .await
        .context("unlock provider secrets keyring")?;
    let items = keyring
        .search_items(&attributes())
        .await
        .context("search provider secrets keyring")?;
    let mut failure = None;
    for item in items {
        let result: Result<Zeroizing<[u8; KEY_LEN]>> = async {
            item.unlock().await.context("unlock provider secrets key")?;
            let secret = item.secret().await.context("read provider secrets key")?;
            ensure!(
                secret.len() == KEY_LEN,
                "login keyring contains an invalid provider secrets key"
            );
            let mut key = Zeroizing::new([0u8; KEY_LEN]);
            key.copy_from_slice(&secret);
            Ok(key)
        }
        .await;
        match result {
            Ok(key) => return Ok(Some(key)),
            Err(error) => failure = Some(error),
        }
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(None),
    }
}

async fn write_keyring(keyring: &oo7::Keyring, key: &[u8; KEY_LEN]) -> Result<()> {
    keyring.unlock().await?;
    keyring
        .create_item(
            "Codex Minibar provider secrets key",
            &attributes(),
            key.to_vec(),
            true,
        )
        .await?;
    Ok(())
}

fn key_file() -> Result<PathBuf> {
    directories::ProjectDirs::from("dev", "Codex Minibar", "Codex Minibar")
        .map(|dirs| dirs.config_dir().join(KEY_FILE))
        .context("could not resolve the provider secrets directory")
}

fn read_key_file(path: &std::path::Path) -> Result<Option<Zeroizing<[u8; KEY_LEN]>>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => Zeroizing::new(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
    };
    ensure!(
        bytes.len() == KEY_LEN,
        "{} is not a secrets key",
        path.display()
    );
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    key.copy_from_slice(&bytes);
    Ok(Some(key))
}

fn write_key_file(key: &[u8; KEY_LEN]) -> Result<()> {
    write_key_file_at(&key_file()?, key)
}

fn write_key_file_at(path: &std::path::Path, key: &[u8; KEY_LEN]) -> Result<()> {
    use std::io::Write;
    let parent = path.parent().context("resolve secrets key directory")?;
    std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    // NamedTempFile creates a private (0600) file. Publish only a complete,
    // synced key; a failed write cannot leave a truncated authoritative key.
    let mut file = tempfile::NamedTempFile::new_in(parent).context("stage provider secrets key")?;
    file.write_all(key).context("write provider secrets key")?;
    file.as_file()
        .sync_all()
        .context("sync provider secrets key")?;
    file.persist_noclobber(path)
        .map_err(|error| error.error)
        .with_context(|| {
            format!(
                "create {} without replacing an existing key",
                path.display()
            )
        })?;
    Ok(())
}

/// Seals `data` with the master key.
pub fn protect(data: &[u8]) -> Result<Vec<u8>> {
    seal(master_key(true)?, data)
}

/// Opens a value sealed by [`protect`].
pub fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    open(master_key(false)?, data)
}

fn seal(key: &[u8; KEY_LEN], data: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| anyhow!("invalid secrets key"))?;
    let mut nonce = [0u8; NONCE_LEN];
    super::random_bytes(&mut nonce).context("generate nonce")?;
    let sealed = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: data,
                aad: MAGIC,
            },
        )
        .map_err(|_| anyhow!("encrypt provider secret"))?;
    let mut output = Vec::with_capacity(MAGIC.len() + NONCE_LEN + sealed.len());
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&sealed);
    Ok(output)
}

fn open(key: &[u8; KEY_LEN], data: &[u8]) -> Result<Vec<u8>> {
    let Some(rest) = data.strip_prefix(MAGIC) else {
        bail!("provider secret was not sealed on Linux");
    };
    ensure!(rest.len() > NONCE_LEN, "provider secret is truncated");
    let (nonce, sealed) = rest.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| anyhow!("invalid secrets key"))?;
    cipher
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: sealed,
                aad: MAGIC,
            },
        )
        .map_err(|_| anyhow!("provider secret could not be decrypted with this keyring"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_key_is_published_privately_without_replacing_existing_keys() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("config/key");
        let original = [11u8; KEY_LEN];
        write_key_file_at(&path, &original).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(write_key_file_at(&path, &[22u8; KEY_LEN]).is_err());
        let loaded = read_key_file(&path).unwrap().unwrap();
        assert_eq!(*loaded, original);
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
        let ciphertext = seal(&original, b"persisted credential").unwrap();
        assert_eq!(open(&loaded, &ciphertext).unwrap(), b"persisted credential");
    }

    #[test]
    fn new_keys_are_only_allowed_for_writes_without_existing_credentials() {
        assert!(ensure_key_creation_allowed(true, || Ok(false)).is_ok());
        assert!(ensure_key_creation_allowed(true, || Ok(true)).is_err());
        assert!(
            ensure_key_creation_allowed(true, || Err(anyhow!("cannot read secret store"))).is_err()
        );
        assert!(
            ensure_key_creation_allowed(false, || panic!("reads must not create keys")).is_err()
        );
    }

    #[test]
    fn fallback_key_remains_authoritative_when_keyring_availability_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("key");
        let key = [7u8; KEY_LEN];
        std::fs::write(&path, key).unwrap();
        let ciphertext = seal(&key, b"saved token").unwrap();
        let loaded =
            with_key_file(&path, || panic!("must not consult a different keyring key")).unwrap();
        assert_eq!(open(&loaded, &ciphertext).unwrap(), b"saved token");
    }

    #[test]
    fn unreadable_or_invalid_fallback_keys_do_not_create_replacements() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("key");
        std::fs::write(&path, b"truncated").unwrap();
        assert!(with_key_file(&path, || panic!("must not create replacement key")).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"truncated");
    }

    #[test]
    fn sealed_values_round_trip_and_reject_other_keys() {
        let key = [7u8; KEY_LEN];
        let sealed = seal(&key, b"sk-test").expect("seal");
        assert_ne!(&sealed[MAGIC.len() + NONCE_LEN..], b"sk-test");
        assert_eq!(open(&key, &sealed).expect("open"), b"sk-test");
        assert!(open(&[8u8; KEY_LEN], &sealed).is_err());
        assert!(open(&key, b"garbage").is_err());
    }
}
