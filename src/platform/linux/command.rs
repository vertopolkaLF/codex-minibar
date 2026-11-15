//! Bounded execution of optional desktop utilities.

use anyhow::{Context, Result, bail};
use std::{
    io::{Read, Seek},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// Capture without pipes: large output must not fill a pipe and
/// deadlock the child before it exits. A stalled desktop service is bounded,
/// including helpers that inherit the command's file descriptors.
pub(super) fn output(
    command: Command,
    timeout: Duration,
) -> Result<(std::process::ExitStatus, String, String)> {
    let (status, stdout, stderr) = output_bytes(command, timeout)?;
    Ok((
        status,
        String::from_utf8(stdout).context("desktop command output is not UTF-8")?,
        String::from_utf8(stderr).context("desktop command output is not UTF-8")?,
    ))
}

/// Binary output keeps NUL-delimited argv and non-UTF-8 paths intact.
pub(crate) fn output_bytes(
    mut command: Command,
    timeout: Duration,
) -> Result<(std::process::ExitStatus, Vec<u8>, Vec<u8>)> {
    let mut stdout = tempfile::tempfile().context("capture desktop command output")?;
    let mut stderr = tempfile::tempfile().context("capture desktop command errors")?;
    command
        .stdin(Stdio::null())
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?);
    let mut child =
        super::login_child::LoginChild::spawn(command).context("run desktop command")?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait().context("wait for desktop command")? {
            break status;
        }
        if Instant::now() >= deadline {
            // Dropping the child kills and reaps its private process group.
            bail!("desktop command timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let read = |file: &mut std::fs::File| -> Result<Vec<u8>> {
        file.rewind()?;
        let mut bytes = Vec::new();
        // Fail rather than parse truncated settings data.
        file.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 1024 * 1024 {
            bail!("desktop command output exceeds 1 MiB");
        }
        Ok(bytes)
    };
    Ok((status, read(&mut stdout)?, read(&mut stderr)?))
}
