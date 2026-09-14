//! Bounded, hidden CLI handoff for providers that own their refresh credentials.

use std::{
    io::ErrorKind,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Result, bail};

pub(crate) fn refresh_session(
    provider: &str,
    candidates: &[PathBuf],
    args: &[&str],
    timeout: Duration,
) -> Result<()> {
    for program in candidates {
        let mut command = Command::new(program);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(_) => bail!("{provider} session refresh could not start"),
        };
        let deadline = Instant::now() + timeout;
        loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => return Ok(()),
                Ok(Some(_)) => bail!("{provider} session refresh did not complete"),
                Ok(None) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(50));
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    bail!("{provider} session refresh timed out")
                }
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    bail!("{provider} session refresh could not be checked")
                }
            }
        }
    }
    bail!("{provider} CLI is not installed")
}
