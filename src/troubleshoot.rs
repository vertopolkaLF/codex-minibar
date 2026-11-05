//! Shared troubleshooting launcher for the CLI alias and the Settings Log page.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

#[cfg(windows)]
use anyhow::bail;
use anyhow::{Context, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiTool {
    Codex,
    Claude,
}

impl AiTool {
    pub fn id(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvailableAiTool {
    pub tool: AiTool,
    pub executable: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolPickerState {
    pub tools: Vec<AvailableAiTool>,
    pub selected_index: i32,
}

impl ToolPickerState {
    pub fn new(tools: Vec<AvailableAiTool>) -> Self {
        Self {
            tools,
            selected_index: 0,
        }
    }
}

pub fn is_cli_request() -> bool {
    env::args_os()
        .nth(1)
        .is_some_and(|argument| argument.eq_ignore_ascii_case("trouble"))
}

/// Finds the supported AI CLIs without starting them.
pub fn available_tools(
    codex_path: Option<&Path>,
    claude_path: Option<&Path>,
) -> Vec<AvailableAiTool> {
    let mut tools = Vec::new();
    if let Ok(executable) = crate::codex::first_available(codex_path) {
        tools.push(AvailableAiTool {
            tool: AiTool::Codex,
            executable,
        });
    }
    if let Some(executable) = crate::claude::first_available(claude_path) {
        tools.push(AvailableAiTool {
            tool: AiTool::Claude,
            executable,
        });
    }
    tools
}

pub fn launch_cli_picker() -> Result<()> {
    launch_terminal(None)
}

pub fn launch_selected(tool: &AvailableAiTool) -> Result<()> {
    launch_terminal(Some(tool))
}

#[cfg(windows)]
fn launch_terminal(selected: Option<&AvailableAiTool>) -> Result<()> {
    let script = script_path()?;
    let prepared = selected
        .filter(|tool| tool.tool == AiTool::Codex)
        .map(|tool| crate::discovery::prepare(&tool.executable))
        .transpose()?;
    let mut powershell_args = vec![
        "-NoLogo".to_owned(),
        "-NoProfile".to_owned(),
        "-ExecutionPolicy".to_owned(),
        "Bypass".to_owned(),
        "-NoExit".to_owned(),
        "-File".to_owned(),
        script.to_string_lossy().into_owned(),
    ];
    if let Some(tool) = selected {
        powershell_args.push("-Tool".to_owned());
        powershell_args.push(tool.tool.id().to_owned());
        powershell_args.push("-Executable".to_owned());
        let executable = prepared
            .as_ref()
            .map(|cli| cli.path.as_path())
            .unwrap_or(&tool.executable);
        powershell_args.push(executable.to_string_lossy().into_owned());
    }

    {
        // A terminal launcher such as wt.exe exits before its shell starts.
        // For cached CLIs own the shell child instead, retaining the lease
        // across that gap and until the troubleshooting console closes.
        if let Some(prepared) = prepared.filter(|cli| cli.is_cached()) {
            use std::os::windows::process::CommandExt;
            let mut child = Command::new("powershell.exe")
                .args(&powershell_args)
                .creation_flags(0x0000_0010) // CREATE_NEW_CONSOLE
                .spawn()
                .context("open Codex troubleshooting terminal")?;
            std::thread::spawn(move || {
                let _lease = prepared;
                let _ = child.wait();
            });
            return Ok(());
        }
        if let Some(windows_terminal) = find_on_path("wt.exe") {
            let mut command = Command::new(windows_terminal);
            command.args(["new-tab", "--", "powershell.exe"]);
            command.args(&powershell_args);
            command.spawn().context("open Windows Terminal")?;
            return Ok(());
        }

        use std::os::windows::process::CommandExt;

        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        Command::new("powershell.exe")
            .args(&powershell_args)
            .creation_flags(CREATE_NEW_CONSOLE)
            .spawn()
            .context("open PowerShell terminal")?;
        Ok(())
    }
}

#[cfg(windows)]
fn script_path() -> Result<PathBuf> {
    let packaged = env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .map(|parent| parent.join("assets/troubleshoot.ps1"))
        })
        .filter(|path| path.is_file());
    if let Some(path) = packaged {
        return Ok(path);
    }

    let development = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/troubleshoot.ps1");
    if development.is_file() {
        return Ok(development);
    }

    bail!("troubleshooting script was not found")
}

/// Linux: runs the embedded bash script in the user's terminal emulator.
#[cfg(not(windows))]
fn launch_terminal(selected: Option<&AvailableAiTool>) -> Result<()> {
    let prepared = selected
        .filter(|tool| tool.tool == AiTool::Codex)
        .map(|tool| crate::discovery::prepare(&tool.executable))
        .transpose()?;
    let script = linux_script()?;
    let mut program = vec![std::ffi::OsString::from("bash"), script.into_os_string()];
    let executable = selected.map(|tool| {
        prepared
            .as_ref()
            .map(|cli| cli.path.as_path())
            .unwrap_or(&tool.executable)
    });
    if let Some(tool) = selected {
        program.push(tool.tool.id().into());
    }
    if let Some(executable) = executable {
        program.push(executable.as_os_str().to_owned());
    }
    let mut terminal = linux_terminal(&program)?;
    if let Some(executable) = executable {
        crate::linux::provider_env::prepare_for_program(&mut terminal, executable);
    }
    let mut child = terminal
        .spawn()
        .context("open a terminal for troubleshooting")?;
    // Hold any prepared CLI lease until the terminal closes.
    std::thread::spawn(move || {
        let _lease = prepared;
        let _ = child.wait();
    });
    Ok(())
}

/// Writes the bundled script where the terminal can run it.
#[cfg(not(windows))]
fn linux_script() -> Result<PathBuf> {
    let base = directories::BaseDirs::new().context("resolve the cache directory")?;
    let directory = base.cache_dir().join("codex-minibar");
    linux_script_in(&directory)
}

#[cfg(not(windows))]
fn linux_script_in(directory: &Path) -> Result<PathBuf> {
    use std::io::Write;
    fs::create_dir_all(directory).with_context(|| format!("create {}", directory.display()))?;
    let path = directory.join("troubleshoot.sh");
    // Bash may still be reading a previous invocation's script. Publish a
    // complete replacement rather than truncating that reader's inode.
    let mut staged =
        tempfile::NamedTempFile::new_in(directory).context("stage troubleshooting script")?;
    staged
        .write_all(include_bytes!("../assets/troubleshoot.sh"))
        .context("write troubleshooting script")?;
    staged
        .persist(&path)
        .with_context(|| format!("replace {}", path.display()))?;
    Ok(path)
}

/// A terminal emulator command running `program`: `$TERMINAL`, the desktop's
/// xdg-terminal-exec selection, then distribution alternatives and emulators.
#[cfg(not(windows))]
fn linux_terminal(program: &[std::ffi::OsString]) -> Result<Command> {
    if let Some(terminal) = env::var_os("TERMINAL").filter(|value| !value.is_empty())
        && let Ok(command) = linux_terminal_from(program, [PathBuf::from(terminal)])
    {
        return Ok(command);
    }
    if let Some(launcher) = crate::linux::apps::on_path("xdg-terminal-exec")
        && let Ok(command) = xdg_terminal_command(&launcher, program)
    {
        return Ok(command);
    }
    linux_terminal_from(
        program,
        [
            "x-terminal-emulator",
            "ptyxis",
            "gnome-terminal",
            "kgx",
            "konsole",
            "xfce4-terminal",
            "mate-terminal",
            "qterminal",
            "lxterminal",
            "tilix",
            "alacritty",
            "kitty",
            "foot",
            "wezterm",
            "ghostty",
            "xterm",
        ]
        .map(PathBuf::from),
    )
}

#[cfg(not(windows))]
fn xdg_terminal_command(launcher: &Path, program: &[std::ffi::OsString]) -> Result<Command> {
    use std::{
        ffi::OsString,
        os::unix::ffi::OsStringExt,
        time::{Duration, Instant},
    };
    let started = Instant::now();
    let budget = Duration::from_secs(2);
    // Older selectors can silently ignore unknown options and open a window.
    // Confirm print mode before sending it a command to resolve.
    let mut help = Command::new(launcher);
    help.arg("--help");
    let (status, stdout, _) = crate::linux::command::output_bytes(help, budget)?;
    anyhow::ensure!(
        status.success()
            && stdout
                .windows(b"--print-cmd".len())
                .any(|part| part == b"--print-cmd"),
        "desktop terminal selector has no print mode"
    );
    // Print mode resolves the configured terminal and its execution syntax
    // without opening a window. Never interpret the result as shell code.
    let mut probe = Command::new(launcher);
    // Keep the plain print flag too: a selector that understands print mode
    // but not custom delimiters must still never interpret this as execution.
    probe
        .args(["--print-cmd", "--print-cmd=\\0", "--"])
        .args(program);
    let (status, stdout, _) =
        crate::linux::command::output_bytes(probe, budget.saturating_sub(started.elapsed()))?;
    anyhow::ensure!(status.success(), "desktop terminal selection failed");
    let mut arguments = stdout
        .split(|byte| *byte == 0)
        .map(|bytes| OsString::from_vec(bytes.to_vec()));
    let executable = arguments
        .next()
        .filter(|name| !name.is_empty())
        .context("desktop terminal selection returned no command")?;
    let executable = crate::linux::apps::on_path_os(&executable)
        .context("selected desktop terminal is not executable")?;
    let mut command = Command::new(executable);
    command.args(arguments);
    Ok(command)
}

#[cfg(not(windows))]
fn linux_terminal_from(
    program: &[std::ffi::OsString],
    candidates: impl IntoIterator<Item = PathBuf>,
) -> Result<Command> {
    let terminal = candidates
        .into_iter()
        .find_map(|candidate| crate::linux::apps::on_path_os(candidate.as_os_str()))
        .context("no terminal emulator was found; set $TERMINAL")?;
    // `x-terminal-emulator` is a symlink; its target decides the syntax.
    let resolved = fs::canonicalize(&terminal).unwrap_or_else(|_| terminal.clone());
    let name = resolved
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut command = Command::new(&terminal);
    command.args(terminal_prefix(&name)).args(program);
    Ok(command)
}

/// Arguments that precede the command for a terminal binary named `name`.
#[cfg(not(windows))]
fn terminal_prefix(name: &str) -> &'static [&'static str] {
    match name {
        "ptyxis"
        | "gnome-terminal"
        | "gnome-terminal.real"
        | "gnome-terminal.wrapper"
        | "kgx"
        | "tilix" => &["--"],
        "wezterm" | "wezterm-gui" => &["start", "--"],
        "xfce4-terminal" | "mate-terminal" => &["-x"],
        "kitty" | "foot" => &[],
        _ => &["-e"],
    }
}

#[cfg(windows)]
fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| fs::canonicalize(candidate).is_ok_and(|path| path.is_file()))
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn script_publication_does_not_truncate_an_active_readers_file() {
        use std::{io::Read, os::unix::fs::symlink};
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("troubleshoot.sh");
        fs::write(&path, "previous script").unwrap();
        let mut reader = fs::File::open(&path).unwrap();
        assert_eq!(linux_script_in(root.path()).unwrap(), path);
        let mut previous = String::new();
        reader.read_to_string(&mut previous).unwrap();
        assert_eq!(previous, "previous script");
        assert_eq!(
            fs::read(&path).unwrap(),
            include_bytes!("../assets/troubleshoot.sh")
        );
        fs::remove_file(&path).unwrap();
        let outside = root.path().join("unrelated");
        fs::write(&outside, "unchanged").unwrap();
        symlink(&outside, &path).unwrap();
        linux_script_in(root.path()).unwrap();
        assert_eq!(fs::read_to_string(&outside).unwrap(), "unchanged");
    }

    #[test]
    fn explicit_diagnostic_cli_wins_over_path_and_preserves_unix_path_bytes() {
        use std::os::unix::{ffi::OsStringExt, fs::PermissionsExt};
        let root = tempfile::tempdir().unwrap();
        let path_cli = root.path().join("codex");
        let selected_cli = root
            .path()
            .join(std::ffi::OsString::from_vec(b"selected-\xff".to_vec()));
        let marker = root.path().join("selected.txt");
        for (path, label) in [(&path_cli, "PATH"), (&selected_cli, "selected")] {
            fs::write(
                path,
                format!("#!/bin/sh\nprintf {label} > \"$MINIBAR_TEST_MARKER\"\n"),
            )
            .unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let script = root.path().join("troubleshoot.sh");
        fs::write(&script, include_str!("../assets/troubleshoot.sh")).unwrap();
        let run = || {
            Command::new("/bin/bash")
                .arg(&script)
                .arg("codex")
                .arg(&selected_cli)
                .env("PATH", root.path())
                .env("MINIBAR_TEST_MARKER", &marker)
                .stdin(std::process::Stdio::null())
                .output()
                .unwrap()
        };
        assert!(run().status.success());
        assert_eq!(fs::read_to_string(&marker).unwrap(), "selected");
        fs::remove_file(&marker).unwrap();
        fs::remove_file(&selected_cli).unwrap();
        assert!(!run().status.success());
        assert!(
            !marker.exists(),
            "missing explicit CLI must not run PATH CLI"
        );

        // Closing stdin in the standalone picker must exit, rather than
        // repeatedly printing the prompt after read reports EOF.
        let mut picker = Command::new("/bin/bash")
            .arg(&script)
            .env("PATH", root.path())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if let Some(status) = picker.try_wait().unwrap() {
                assert!(!status.success());
                break;
            }
            if std::time::Instant::now() >= deadline {
                picker.kill().unwrap();
                picker.wait().unwrap();
                panic!("diagnostic picker did not exit at EOF");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn terminal_selection_skips_unusable_files_and_preserves_command_arguments() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let bad = root.path().join("unusable-terminal");
        let good = root.path().join("konsole");
        fs::write(&bad, b"not executable").unwrap();
        fs::write(&good, b"fixture, never launched").unwrap();
        fs::set_permissions(&good, fs::Permissions::from_mode(0o755)).unwrap();
        let program = ["bash".into(), "/tmp/path with spaces/script.sh".into()];
        let command = linux_terminal_from(&program, [bad.clone(), good.clone()]).unwrap();
        assert_eq!(command.get_program(), good.as_os_str());
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["-e", "bash", "/tmp/path with spaces/script.sh"]
        );
        assert!(linux_terminal_from(&program, [bad]).is_err());
    }

    #[test]
    fn terminal_arguments_preserve_non_utf8_paths() {
        use std::os::unix::{ffi::OsStringExt, fs::PermissionsExt};
        let root = tempfile::tempdir().unwrap();
        let terminal = root.path().join("foot");
        fs::write(&terminal, b"fixture, never launched").unwrap();
        fs::set_permissions(&terminal, fs::Permissions::from_mode(0o755)).unwrap();
        let program = [
            "bash".into(),
            std::ffi::OsString::from_vec(b"/tmp/script-\xff".to_vec()),
        ];
        let command = linux_terminal_from(&program, [terminal]).unwrap();
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            program
                .iter()
                .map(|arg| arg.as_os_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn relative_terminal_paths_keep_non_utf8_directory_names() {
        use std::os::unix::{ffi::OsStringExt, fs::PermissionsExt};
        let root = tempfile::tempdir_in(".").unwrap();
        let directory = root
            .path()
            .join(std::ffi::OsString::from_vec(b"terminal-\xff".to_vec()));
        fs::create_dir(&directory).unwrap();
        let terminal = directory.join("foot");
        let terminal = terminal
            .strip_prefix(std::env::current_dir().unwrap())
            .unwrap()
            .to_owned();
        assert!(!terminal.is_absolute());
        fs::write(&terminal, "#!/bin/sh\nexec \"$@\"\n").unwrap();
        fs::set_permissions(&terminal, fs::Permissions::from_mode(0o755)).unwrap();
        let program = ["/bin/sh".into(), "-c".into(), "printf relative-ok".into()];
        let output = linux_terminal_from(&program, [terminal])
            .unwrap()
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"relative-ok");
    }

    #[test]
    fn desktop_terminal_selection_keeps_binary_argv_and_empty_arguments() {
        use std::os::unix::{ffi::OsStringExt, fs::PermissionsExt};
        let root = tempfile::tempdir().unwrap();
        let launcher = root.path().join("xdg-terminal-exec");
        fs::write(&launcher, "#!/bin/sh\nif [ \"$1\" = --help ]; then printf '%s\\n' --print-cmd; exit 0; fi\n[ \"$1\" = --print-cmd ] && [ \"$2\" = '--print-cmd=\\0' ] && [ \"$3\" = -- ] || exit 64\nshift 3\nprintf '%s' \"${0%/*}/terminal\"\nfor arg in \"$@\"; do printf '\\000%s' \"$arg\"; done\n").unwrap();
        let terminal = root.path().join("terminal");
        fs::write(&terminal, "#!/bin/sh\nexec \"$@\"\n").unwrap();
        for file in [&launcher, &terminal] {
            fs::set_permissions(file, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let script = root.path().join(std::ffi::OsString::from_vec(
            b"script space-\xff.sh".to_vec(),
        ));
        fs::write(&script, "printf '%s\\0' \"$@\"\n").unwrap();
        let payload = b"argument\nwith-\xfe $`&";
        let program = [
            "/bin/sh".into(),
            script.into_os_string(),
            std::ffi::OsString::from_vec(payload.to_vec()),
            "".into(),
        ];
        let output = xdg_terminal_command(&launcher, &program)
            .unwrap()
            .output()
            .unwrap();
        assert!(output.status.success());
        let mut expected = payload.to_vec();
        expected.extend([0, 0]);
        assert_eq!(output.stdout, expected);
    }

    #[test]
    fn failed_or_empty_desktop_terminal_selection_can_fall_back() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let launcher = root.path().join("xdg-terminal-exec");
        for contents in [
            "exit 1\n",
            "exit 0\n",
            "printf '/missing/minibar-fixture-terminal'\n",
        ] {
            fs::write(&launcher, format!("#!/bin/sh\nif [ \"$1\" = --help ]; then printf '%s\\n' --print-cmd; exit 0; fi\n{contents}")).unwrap();
            fs::set_permissions(&launcher, fs::Permissions::from_mode(0o755)).unwrap();
            assert!(xdg_terminal_command(&launcher, &["bash".into()]).is_err());
        }
    }

    #[test]
    fn legacy_desktop_selectors_are_not_asked_to_execute_a_print_request() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let launcher = root.path().join("xdg-terminal-exec");
        for help in ["legacy selector help", "--print-cmd"] {
            fs::write(&launcher, format!("#!/bin/sh\nif [ \"$1\" = --help ]; then printf '%s\\n' '{help}'; exit 0; fi\nif [ \"$1\" = --print-cmd ]; then printf '/missing/terminal\\n-e\\nbash\\n'; exit 0; fi\nprintf launched > \"${{0%/*}}/unexpected-launch\"\n")).unwrap();
            fs::set_permissions(&launcher, fs::Permissions::from_mode(0o755)).unwrap();
            assert!(xdg_terminal_command(&launcher, &["bash".into()]).is_err());
            assert!(!root.path().join("unexpected-launch").exists());
        }
    }

    #[test]
    fn terminal_alternatives_use_the_resolved_emulators_argument_syntax() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let root = tempfile::tempdir().unwrap();
        let terminal = root.path().join("foot");
        fs::write(&terminal, b"fixture, never launched").unwrap();
        fs::set_permissions(&terminal, fs::Permissions::from_mode(0o755)).unwrap();
        let alias = root.path().join("x-terminal-emulator");
        symlink(&terminal, &alias).unwrap();
        let command = linux_terminal_from(&["bash".into()], [alias]).unwrap();
        assert_eq!(command.get_args().collect::<Vec<_>>(), ["bash"]);
    }

    #[test]
    fn mate_terminal_handoff_preserves_script_and_provider_arguments() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let terminal = root.path().join("mate-terminal");
        // MATE's -e parses one argument; -x passes the remaining argv intact.
        // This fixture rejects -e and executes the diagnostic fixture only.
        fs::write(
            &terminal,
            "#!/bin/sh\n[ \"$1\" = -x ] || exit 64\nshift\nexec \"$@\"\n",
        )
        .unwrap();
        fs::set_permissions(&terminal, fs::Permissions::from_mode(0o755)).unwrap();
        let script = root.path().join("script with spaces.sh");
        fs::write(&script, "printf '%s\\0' \"$@\"\n").unwrap();
        let provider = "/opt/CLI $apps/it's `codex`/Юникод";
        let program = [
            "/bin/sh".into(),
            script.into_os_string(),
            "codex".into(),
            provider.into(),
        ];
        let output = linux_terminal_from(&program, [terminal])
            .unwrap()
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, format!("codex\0{provider}\0").into_bytes());
    }

    #[test]
    fn terminals_receive_the_command_in_their_own_syntax() {
        assert_eq!(terminal_prefix("ptyxis"), ["--"]);
        assert_eq!(terminal_prefix("gnome-terminal.wrapper"), ["--"]);
        assert_eq!(terminal_prefix("wezterm"), ["start", "--"]);
        assert_eq!(terminal_prefix("kitty"), [] as [&str; 0]);
        assert_eq!(terminal_prefix("mate-terminal"), ["-x"]);
        assert_eq!(terminal_prefix("konsole"), ["-e"]);
        assert_eq!(terminal_prefix("xterm"), ["-e"]);
    }
}
