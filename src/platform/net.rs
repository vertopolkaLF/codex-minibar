//! HTTP plumbing shared by every request the app makes.

use std::{
    fs,
    io::{Read, Write},
    path::Path,
    sync::{Arc, OnceLock},
};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

/// Ceiling for downloads that are not the update package itself.
const MAX_DOWNLOAD_BYTES: u64 = 64 * 1024 * 1024;

/// An agent builder on the OS TLS stack (Schannel). It honors the Windows
/// certificate store, which VPNs and proxies that inspect TLS rely on.
/// ureq does not pick its `native-tls` backend on its own, so every agent
/// that needs it must be configured here.
pub fn native_tls_agent() -> ureq::AgentBuilder {
    let builder = ureq::AgentBuilder::new();
    match ureq::native_tls::TlsConnector::new() {
        Ok(tls) => builder.tls_connector(Arc::new(tls)),
        Err(error) => {
            crate::platform::logger::info(format!("falling back to bundled TLS: {error}"));
            builder
        }
    }
}

/// Native-TLS agent for one-off GitHub requests and downloads.
pub(crate) fn shared_agent() -> ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT
        .get_or_init(|| {
            native_tls_agent()
                .user_agent("codex-minibar-updater")
                .build()
        })
        .clone()
}

pub(crate) fn download(url: &str, destination: &Path) -> Result<()> {
    download_limited(url, destination, MAX_DOWNLOAD_BYTES, &mut |_, _| {}).map(|_| ())
}

/// Downloads `url` to `destination`, failing if more than `max_bytes` arrive.
/// Reports `(downloaded, Content-Length)` as bytes arrive and returns the
/// lowercase hex SHA-256 of the written bytes.
pub(crate) fn download_limited(
    url: &str,
    destination: &Path,
    max_bytes: u64,
    on_progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<String> {
    let response = shared_agent()
        .get(url)
        .call()
        .with_context(|| format!("download {url}"))?;
    let status = response.status();
    if status / 100 != 2 {
        bail!("download failed with status {status}");
    }
    let length = response
        .header("Content-Length")
        .and_then(|value| value.trim().parse::<u64>().ok());
    let reader = response.into_reader();
    let mut file = fs::File::create(destination)
        .with_context(|| format!("create {}", destination.display()))?;
    let result = copy_limited(reader, &mut file, max_bytes, &mut |downloaded| {
        on_progress(downloaded, length)
    });
    drop(file);
    if result.is_err() {
        let _ = fs::remove_file(destination);
    }
    result
}

/// Copies at most `max_bytes` from `reader`, erroring if more are available.
/// Reports the running total after every chunk and returns the hex SHA-256
/// of the copied bytes.
fn copy_limited(
    reader: impl Read,
    writer: &mut impl Write,
    max_bytes: u64,
    on_progress: &mut dyn FnMut(u64),
) -> Result<String> {
    let mut limited = reader.take(max_bytes.saturating_add(1));
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let read = limited.read(&mut buf).context("read download")?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > max_bytes {
            bail!("download exceeds the {max_bytes} byte limit");
        }
        hasher.update(&buf[..read]);
        writer.write_all(&buf[..read]).context("write download")?;
        on_progress(total);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// The body of `response`, or `None` when it is larger than `limit` bytes.
pub fn read_limited(response: ureq::Response, limit: u64) -> std::io::Result<Option<Vec<u8>>> {
    let mut body = Vec::new();
    response
        .into_reader()
        .take(limit.saturating_add(1))
        .read_to_end(&mut body)?;
    Ok((body.len() as u64 <= limit).then_some(body))
}

#[derive(Debug)]
pub(crate) struct ProviderRateLimitResponse;

impl std::fmt::Display for ProviderRateLimitResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("provider returned HTTP 429")
    }
}

impl std::error::Error for ProviderRateLimitResponse {}

pub(crate) fn rate_limit_error(message: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(ProviderRateLimitResponse).context(message.into())
}

pub(crate) fn is_rate_limited_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause.downcast_ref::<ProviderRateLimitResponse>().is_some()
            || matches!(
                cause.downcast_ref::<ureq::Error>(),
                Some(ureq::Error::Status(429, _))
            )
    })
}

/// Maps the failures a provider's API request shares: 401/403 means the
/// saved sign-in was rejected (`unauthorized` tells the user what to do),
/// 429 pauses the worker, and anything else names `subject`.
pub(crate) fn provider_response(
    result: Result<ureq::Response, ureq::Error>,
    subject: &str,
    unauthorized: &str,
) -> Result<ureq::Response> {
    match result {
        Ok(response) => Ok(response),
        Err(ureq::Error::Status(401 | 403, _)) => bail!("{unauthorized}"),
        Err(ureq::Error::Status(429, _)) => Err(rate_limit_error(format!(
            "{subject} request was rate limited (HTTP 429)."
        ))),
        Err(ureq::Error::Status(status, _)) => {
            bail!("{subject} request failed with HTTP {status}")
        }
        Err(error) => Err(error).with_context(|| format!("request {subject}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_limited_enforces_cap_and_hashes() {
        let mut out = Vec::new();
        let digest = copy_limited(&b"abc"[..], &mut out, 3, &mut |_| {}).unwrap();
        assert_eq!(out, b"abc");
        assert_eq!(
            digest,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let mut out = Vec::new();
        assert!(copy_limited(&b"abcd"[..], &mut out, 3, &mut |_| {}).is_err());
    }
}
