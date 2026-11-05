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
        if self.running {
            if !self.exited(true)? {
                return Ok(None);
            }
            self.kill_group();
        }
        let status = self.child.try_wait()?;
        if status.is_some() {
            // Once reaped, the PID may be reused. Never signal it again.
            self.running = false;
        }
        Ok(status)
    }

    /// Observe exit without reaping: the zombie reserves the group leader's
    /// PID while we terminate helpers that could still hold output pipes.
    fn exited(&self, nonblocking: bool) -> io::Result<bool> {
        loop {
            // SAFETY: zero is a valid initial siginfo_t; waitid writes it and
            // WNOWAIT keeps our owned child available for std::Child to reap.
            let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
            let options =
                libc::WEXITED | libc::WNOWAIT | if nonblocking { libc::WNOHANG } else { 0 };
            // SAFETY: info is writable and the ID identifies our unreaped child.
            let result = unsafe { libc::waitid(libc::P_PID, self.child.id(), &mut info, options) };
            if result == 0 {
                // SAFETY: waitid initialized the SIGCHLD fields; zero means
                // no child changed state for a nonblocking observation.
                return Ok(unsafe { info.si_pid() } != 0);
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }

    fn kill_group(&self) -> bool {
        // SAFETY: the unreaped child reserves this private group's ID. Callers
        // never use this after std::Child reaps the leader.
        unsafe { libc::kill(-(self.child.id() as libc::pid_t), libc::SIGKILL) == 0 }
    }

    pub(crate) fn kill(&mut self) -> io::Result<()> {
        if !self.running {
            return Ok(());
        }
        let group_killed = self.kill_group();
        // Also address the root directly in case the CLI moved to a different
        // group/session while helpers remained in the original group.
        let root = self.child.kill();
        if group_killed { Ok(()) } else { root }
    }

    pub(crate) fn wait(&mut self) -> io::Result<ExitStatus> {
        // Match std::process::Child::wait: EOF must reach a CLI waiting on
        // input before we wait for it. The wrapper owns this pipe separately.
        drop(self.stdin.take());
        if self.running {
            self.exited(false)?;
            self.kill_group();
        }
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
    fn normal_exit_closes_descendant_pipes_before_reaping_the_leader() {
        for poll in [false, true] {
            let mut command = Command::new("/bin/sh");
            command
                .args(["-c", "sleep 30 & echo ready; exit 7"])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            let mut child = LoginChild::spawn(command).unwrap();
            let mut reader = BufReader::new(child.stdout.take().unwrap());
            let mut ready = String::new();
            reader.read_line(&mut ready).unwrap();
            assert_eq!(ready.trim(), "ready");
            let status = if poll {
                let deadline = std::time::Instant::now() + Duration::from_secs(2);
                loop {
                    if let Some(status) = child.try_wait().unwrap() {
                        break status;
                    }
                    assert!(std::time::Instant::now() < deadline);
                    std::thread::sleep(Duration::from_millis(10));
                }
            } else {
                child.wait().unwrap()
            };
            assert_eq!(status.code(), Some(7));
            assert_eq!(child.try_wait().unwrap(), Some(status));
            child.kill().unwrap();
            assert_eq!(child.wait().unwrap(), status);
            assert_descendant_pipe_closed(reader);
        }
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
