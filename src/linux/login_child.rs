//! Own a provider CLI and its process group until it exits or is cancelled.

use std::{
    io,
    os::unix::process::CommandExt,
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus},
};

pub(crate) struct LoginChild {
    child: Child,
    running: bool,
    pub(crate) stdin: Option<ChildStdin>,
    pub(crate) stdout: Option<ChildStdout>,
    pub(crate) stderr: Option<ChildStderr>,
}

impl LoginChild {
    pub(crate) fn spawn(mut command: Command) -> io::Result<Self> {
        super::provider_env::prepare(&mut command);
        let mut child = command.process_group(0).spawn()?;
        Ok(Self {
            stdin: child.stdin.take(),
            stdout: child.stdout.take(),
            stderr: child.stderr.take(),
            child,
            running: true,
        })
    }

    pub(crate) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        let status = self.child.try_wait()?;
        if status.is_some() {
            // Once reaped, the PID may be reused. Never signal it again.
            self.running = false;
        }
        Ok(status)
    }

    pub(crate) fn kill(&mut self) -> io::Result<()> {
        if !self.running {
            return Ok(());
        }
        // SAFETY: the unreaped child leads a private process group created by
        // process_group(0). A negative PID addresses that group only.
        let group_killed =
            unsafe { libc::kill(-(self.child.id() as libc::pid_t), libc::SIGKILL) } == 0;
        // Also address the root directly in case the CLI moved to a different
        // group/session while helpers remained in the original group.
        let root = self.child.kill();
        if group_killed { Ok(()) } else { root }
    }

    pub(crate) fn wait(&mut self) -> io::Result<ExitStatus> {
        // Match std::process::Child::wait: EOF must reach a CLI waiting on
        // input before we wait for it. The wrapper owns this pipe separately.
        drop(self.stdin.take());
        let status = self.child.wait()?;
        self.running = false;
        Ok(status)
    }
}

impl Drop for LoginChild {
    fn drop(&mut self) {
        if self.running {
            let _ = self.kill();
            let _ = self.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Read},
        process::Stdio,
        sync::mpsc,
        time::Duration,
    };

    fn process_tree() -> (LoginChild, BufReader<std::process::ChildStdout>) {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "sh -c 'echo ready; exec sleep 30' & wait"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = LoginChild::spawn(command).unwrap();
        let mut reader = BufReader::new(child.stdout.take().unwrap());
        let mut ready = String::new();
        reader.read_line(&mut ready).unwrap();
        assert_eq!(ready.trim(), "ready");
        (child, reader)
    }

    fn assert_descendant_pipe_closed(mut reader: BufReader<std::process::ChildStdout>) {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut rest = Vec::new();
            tx.send(reader.read_to_end(&mut rest)).unwrap();
        });
        rx.recv_timeout(Duration::from_secs(2))
            .expect("login descendant remained alive")
            .unwrap();
    }

    #[test]
    fn cancellation_kills_the_login_and_its_descendant() {
        let (mut child, reader) = process_tree();
        child.kill().unwrap();
        assert!(!child.wait().unwrap().success());
        assert_descendant_pipe_closed(reader);
    }

    #[test]
    fn dropping_an_unfinished_login_cleans_up_its_process_group() {
        let (child, reader) = process_tree();
        drop(child);
        assert_descendant_pipe_closed(reader);
    }

    #[test]
    fn waiting_closes_the_owned_stdin_before_waiting_for_exit() {
        let mut command = Command::new("/bin/bash");
        command
            .args(["-c", "read -r -t 5 ignored"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = LoginChild::spawn(command).unwrap();
        let started = std::time::Instant::now();
        assert!(!child.wait().unwrap().success());
        assert!(child.stdin.is_none());
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "child waited for input instead of receiving EOF"
        );
    }
}
