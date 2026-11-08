//! Own a provider CLI and its process group until it exits or is cancelled.

use std::{
    io,
    os::unix::process::CommandExt,
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus},
};

pub(crate) struct LoginChild {
    child: Child,
    running: bool,
    /// Whether helpers that outlive a normal exit are terminated with the CLI.
    kill_helpers_on_exit: bool,
    pub(crate) stdin: Option<ChildStdin>,
    pub(crate) stdout: Option<ChildStdout>,
    pub(crate) stderr: Option<ChildStderr>,
}

impl LoginChild {
    pub(crate) fn spawn(command: Command) -> io::Result<Self> {
        Self::spawn_with(command, true)
    }

    /// For browser sign-ins: a browser the CLI cold-starts stays in this group
    /// and must survive a finished sign-in. The login's output goes nowhere,
    /// so no helper can hold a pipe open. Cancelling still ends the group.
    pub(crate) fn spawn_login(command: Command) -> io::Result<Self> {
        Self::spawn_with(command, false)
    }

    fn spawn_with(mut command: Command, kill_helpers_on_exit: bool) -> io::Result<Self> {
        super::provider_env::prepare(&mut command);
        let mut child = command.process_group(0).spawn()?;
        Ok(Self {
            stdin: child.stdin.take(),
            stdout: child.stdout.take(),
            stderr: child.stderr.take(),
            child,
            running: true,
            kill_helpers_on_exit,
        })
    }

    pub(crate) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        if self.running {
            if !self.exited(true)? {
                return Ok(None);
            }
            self.finish_group();
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

    fn finish_group(&self) {
        if self.kill_helpers_on_exit {
            self.kill_group();
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
            self.finish_group();
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
    fn a_finished_login_leaves_the_browser_it_started_running() {
        for poll in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let pid_file = root.path().join("browser.pid");
            let mut command = Command::new("/bin/sh");
            command
                .arg("-c")
                .arg("sleep 30 & echo $! > \"$1\"")
                .arg("login-fixture")
                .arg(&pid_file)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            let mut child = LoginChild::spawn_login(command).unwrap();
            let status = if poll {
                loop {
                    if let Some(status) = child.try_wait().unwrap() {
                        break status;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            } else {
                child.wait().unwrap()
            };
            assert!(status.success());
            drop(child);
            let browser = std::fs::read_to_string(&pid_file)
                .unwrap()
                .trim()
                .parse::<libc::pid_t>()
                .unwrap();
            std::thread::sleep(Duration::from_millis(50));
            // SAFETY: signal 0 only checks that this fixture process exists,
            // and the SIGKILL below targets that same just-checked fixture.
            unsafe {
                assert_eq!(libc::kill(browser, 0), 0, "browser was killed with login");
                libc::kill(browser, libc::SIGKILL);
            }
        }
    }

    #[test]
    fn cancelling_a_login_still_ends_its_process_group() {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "sh -c 'echo ready; exec sleep 30' & wait"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = LoginChild::spawn_login(command).unwrap();
        let mut reader = BufReader::new(child.stdout.take().unwrap());
        let mut ready = String::new();
        reader.read_line(&mut ready).unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
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
