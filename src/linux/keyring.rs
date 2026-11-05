//! Encryption for provider secrets at rest.
//!
//! Windows protects each secret with DPAPI, keyed to the user account. Linux
//! has no per-user system key, so a random 256-bit master key is kept in the
//! user's login keyring (Secret Service, through oo7) and secrets are sealed
//! with AES-256-GCM. Without a Secret Service the key lives in a file only
//! the user can read, next to the secrets file.
//!
//! An unlock prompt can stay open for as long as the user ignores it, so the
//! key is only ever loaded on its own thread. Callers wait a bounded time and
//! otherwise get an error they can retry; the UI thread never waits on a prompt.

use std::{
    path::PathBuf,
    sync::{Condvar, Mutex, OnceLock},
    time::{Duration, Instant},
};

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

/// Long enough for a keyring that is already unlocked, short enough that a
/// pending prompt is at most a single hitch in the UI.
const UI_THREAD_WAIT: Duration = Duration::from_millis(250);
/// Background readers retry on their next refresh.
const READ_WAIT: Duration = Duration::from_secs(5);
/// A save is something the user just asked for, so give them time to unlock.
const WRITE_WAIT: Duration = Duration::from_secs(120);
/// How often background reads check, without prompting, for an unlock that
/// happened elsewhere (such as the desktop's wallet manager).
const READ_RETRY: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Intent {
    Read,
    Write,
}

/// What one load attempt may do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LoadMode {
    allow_prompt: bool,
    allow_create: bool,
}

struct Attempt {
    started: Instant,
    allow_create: bool,
}

struct Failure {
    at: Instant,
    message: String,
}

struct LoaderState {
    running: Option<Attempt>,
    failure: Option<Failure>,
}

#[derive(Clone, Copy)]
struct Waits {
    ui_thread: bool,
    read: Duration,
    write: Duration,
    read_retry: Duration,
}

/// Loads a value at most once at a time, off the calling thread. Only the
/// first read may prompt: after a refused unlock, background reads stay quiet
/// and only a write, which the user asked for, may prompt again.
struct KeyLoader<K: 'static> {
    key: OnceLock<K>,
    state: Mutex<LoaderState>,
    finished: Condvar,
}

impl<K: Send + Sync + 'static> KeyLoader<K> {
    const fn new() -> Self {
        Self {
            key: OnceLock::new(),
            state: Mutex::new(LoaderState {
                running: None,
                failure: None,
            }),
            finished: Condvar::new(),
        }
    }

    fn get(
        &'static self,
        intent: Intent,
        waits: Waits,
        load: impl FnOnce(LoadMode) -> Result<K> + Send + 'static,
    ) -> Result<&'static K> {
        if let Some(key) = self.key.get() {
            return Ok(key);
        }
        let entered = Instant::now();
        let mut load = Some(load);
        let mut joined_write = false;
        let mut state = self.state.lock().map_err(|_| loader_poisoned())?;
        loop {
            if let Some(key) = self.key.get() {
                return Ok(key);
            }
            if state.running.is_none() {
                if let Some(failure) = &state.failure
                    && (joined_write
                        || load.is_none()
                        || (intent == Intent::Read && failure.at.elapsed() < waits.read_retry))
                {
                    return Err(anyhow!("{}", failure.message));
                }
                let Some(load) = load.take() else {
                    bail!(
                        "provider secrets key is unavailable; unlock the login keyring and retry"
                    );
                };
                let mode = LoadMode {
                    allow_prompt: intent == Intent::Write || state.failure.is_none(),
                    allow_create: intent == Intent::Write,
                };
                self.start(&mut state, mode, load)?;
            }
            let attempt = state.running.as_ref().context("provider key load state")?;
            joined_write |= intent == Intent::Write && attempt.allow_create;
            let deadline = match intent {
                _ if waits.ui_thread => attempt.started + UI_THREAD_WAIT,
                Intent::Read => attempt.started + waits.read,
                Intent::Write => entered + waits.write,
            };
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                bail!("waiting for the login keyring; unlock it and retry");
            };
            state = self
                .finished
                .wait_timeout(state, remaining)
                .map_err(|_| loader_poisoned())?
                .0;
        }
    }

    fn start(
        &'static self,
        state: &mut LoaderState,
        mode: LoadMode,
        load: impl FnOnce(LoadMode) -> Result<K> + Send + 'static,
    ) -> Result<()> {
        std::thread::Builder::new()
            .name("provider-secrets-key".into())
            .spawn(move || {
                let result = load(mode);
                let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
                state.running = None;
                match result {
                    Ok(key) => {
                        let _ = self.key.set(key);
                        state.failure = None;
                    }
                    Err(error) => {
                        crate::logger::info(format!(
                            "provider secrets key could not be loaded: {error:#}"
                        ));
                        state.failure = Some(Failure {
                            at: Instant::now(),
                            message: format!("{error:#}"),
                        });
                    }
                }
                self.finished.notify_all();
            })
            .context("start provider secrets key loader")?;
        state.running = Some(Attempt {
            started: Instant::now(),
            allow_create: mode.allow_create,
        });
        Ok(())
    }
}

fn loader_poisoned() -> anyhow::Error {
    anyhow!("provider secrets key loader failed")
}

fn on_ui_thread() -> bool {
    // GPUI's foreground executor runs on the process's main thread on Linux.
    // SAFETY: neither call has preconditions or can fail.
    unsafe { libc::gettid() == libc::getpid() }
}

/// The master key, loaded or created once per process.
fn master_key(intent: Intent) -> Result<&'static Zeroizing<[u8; KEY_LEN]>> {
    static KEY: KeyLoader<Zeroizing<[u8; KEY_LEN]>> = KeyLoader::new();
    KEY.get(
        intent,
        Waits {
            ui_thread: on_ui_thread(),
            read: READ_WAIT,
            write: WRITE_WAIT,
            read_retry: READ_RETRY,
        },
        load_or_create_key,
    )
}

/// Load the key ahead of a write so nothing waits for an unlock while holding
/// the secret store's lock, which the UI thread also takes.
pub(crate) fn prepare_write() -> Result<()> {
    master_key(Intent::Write).map(drop)
}

/// Give the one-shot settings migration the full answer from the keyring.
pub(crate) fn settle_before_migration() {
    if !crate::secrets::has_stored_secrets().unwrap_or(false) {
        return;
    }
    // Run on another thread so the UI thread's shorter wait does not apply.
    let _ = std::thread::spawn(|| master_key(Intent::Read).map(drop)).join();
}

fn load_or_create_key(mode: LoadMode) -> Result<Zeroizing<[u8; KEY_LEN]>> {
    with_key_file(&key_file()?, || load_keyring(mode))
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

fn load_keyring(mode: LoadMode) -> Result<Zeroizing<[u8; KEY_LEN]>> {
    let keyring = futures::executor::block_on(oo7::Keyring::new());
    if let Ok(keyring) = &keyring
        && let Some(key) = futures::executor::block_on(read_keyring(keyring, mode.allow_prompt))?
    {
        return Ok(key);
    }
    ensure_key_creation_allowed(mode.allow_create, crate::secrets::has_stored_secrets)?;
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

async fn read_keyring(
    keyring: &oo7::Keyring,
    allow_prompt: bool,
) -> Result<Option<Zeroizing<[u8; KEY_LEN]>>> {
    if allow_prompt {
        keyring
            .unlock()
            .await
            .context("unlock provider secrets keyring")?;
    } else if let oo7::Keyring::DBus(collection) = keyring {
        ensure!(
            !collection
                .is_locked()
                .await
                .context("check provider secrets keyring")?,
            "the login keyring is locked; unlock it and retry"
        );
    }
    let items = keyring
        .search_items(&attributes())
        .await
        .context("search provider secrets keyring")?;
    let mut failure = None;
    for item in items {
        let result: Result<Zeroizing<[u8; KEY_LEN]>> = async {
            if allow_prompt {
                item.unlock().await.context("unlock provider secrets key")?;
            } else {
                ensure!(
                    !item
                        .is_locked()
                        .await
                        .context("check provider secrets key")?,
                    "the provider secrets key is locked; unlock it and retry"
                );
            }
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
    seal(master_key(Intent::Write)?, data)
}

/// Opens a value sealed by [`protect`].
pub fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    open(master_key(Intent::Read)?, data)
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

    const TEST_WAITS: Waits = Waits {
        ui_thread: false,
        read: Duration::from_secs(5),
        write: Duration::from_secs(5),
        read_retry: Duration::from_secs(60),
    };

    fn loader() -> &'static KeyLoader<u8> {
        Box::leak(Box::new(KeyLoader::new()))
    }

    #[test]
    fn only_the_first_read_may_prompt_and_failures_are_remembered() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
        let loader = loader();
        let refuse = |mode: LoadMode| -> Result<u8> {
            ATTEMPTS.fetch_add(1, Ordering::SeqCst);
            assert_eq!(
                mode,
                LoadMode {
                    allow_prompt: true,
                    allow_create: false
                }
            );
            bail!("prompt dismissed")
        };
        let error = loader.get(Intent::Read, TEST_WAITS, refuse).err().unwrap();
        assert_eq!(error.to_string(), "prompt dismissed");
        for _ in 0..3 {
            assert!(loader.get(Intent::Read, TEST_WAITS, refuse).is_err());
        }
        assert_eq!(ATTEMPTS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn reads_after_a_refused_unlock_retry_without_prompting_and_recover() {
        let loader = loader();
        let waits = Waits {
            read_retry: Duration::ZERO,
            ..TEST_WAITS
        };
        assert!(
            loader
                .get(Intent::Read, waits, |_| bail!("dismissed"))
                .is_err()
        );
        let key = loader
            .get(Intent::Read, waits, |mode| {
                assert!(!mode.allow_prompt, "a background retry must stay quiet");
                Ok(9)
            })
            .unwrap();
        assert_eq!(*key, 9);
        assert_eq!(
            *loader
                .get(Intent::Read, waits, |_| bail!("cached"))
                .unwrap(),
            9
        );
    }

    #[test]
    fn a_write_may_prompt_again_after_a_refused_read() {
        let loader = loader();
        assert!(
            loader
                .get(Intent::Read, TEST_WAITS, |_| bail!("dismissed"))
                .is_err()
        );
        let key = loader
            .get(Intent::Write, TEST_WAITS, |mode| {
                assert_eq!(
                    mode,
                    LoadMode {
                        allow_prompt: true,
                        allow_create: true
                    }
                );
                Ok(3)
            })
            .unwrap();
        assert_eq!(*key, 3);
    }

    #[test]
    fn the_ui_thread_stops_waiting_on_a_pending_prompt_which_can_still_finish() {
        use std::sync::mpsc;
        let loader = loader();
        let (answer, prompt) = mpsc::channel::<()>();
        let waits = Waits {
            ui_thread: true,
            ..TEST_WAITS
        };
        let started = Instant::now();
        let waiting = loader.get(Intent::Read, waits, move |_| {
            prompt.recv()?;
            Ok(5)
        });
        assert!(waiting.is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
        // Later frames do not wait again, and do not start a second prompt.
        let repeat = Instant::now();
        assert!(
            loader
                .get(Intent::Read, waits, |_| panic!("second prompt"))
                .is_err()
        );
        assert!(repeat.elapsed() < Duration::from_millis(100));
        answer.send(()).unwrap();
        let key = loader
            .get(Intent::Read, TEST_WAITS, |_| panic!("already loading"))
            .unwrap();
        assert_eq!(*key, 5);
    }

    #[test]
    fn concurrent_readers_share_one_attempt() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
        let loader = loader();
        let readers = (0..8)
            .map(|_| {
                std::thread::spawn(move || {
                    *loader
                        .get(Intent::Read, TEST_WAITS, |_| {
                            ATTEMPTS.fetch_add(1, Ordering::SeqCst);
                            std::thread::sleep(Duration::from_millis(50));
                            Ok(1)
                        })
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        for reader in readers {
            assert_eq!(reader.join().unwrap(), 1);
        }
        assert_eq!(ATTEMPTS.load(Ordering::SeqCst), 1);
    }

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
