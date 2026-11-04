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
    let mut program = vec!["bash".to_owned(), script.to_string_lossy().into_owned()];
    if let Some(tool) = selected {
        program.push(tool.tool.id().to_owned());
        let executable = prepared
            .as_ref()
            .map(|cli| cli.path.as_path())
            .unwrap_or(&tool.executable);
        program.push(executable.to_string_lossy().into_owned());
    }
    let mut child = linux_terminal(&program)?
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
    fs::create_dir_all(&directory).with_context(|| format!("create {}", directory.display()))?;
    let path = directory.join("troubleshoot.sh");
    fs::write(&path, include_str!("../assets/troubleshoot.sh"))
        .with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

/// A terminal emulator command running `program`: `$TERMINAL`, the Debian
/// `x-terminal-emulator` alternative, then common emulators. Each family
/// takes the command differently.
#[cfg(not(windows))]
fn linux_terminal(program: &[String]) -> Result<Command> {
    let mut candidates: Vec<PathBuf> = env::var_os("TERMINAL")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .into_iter()
        .collect();
    candidates.extend(
        [
            "x-terminal-emulator",
            "ptyxis",
            "gnome-terminal",
            "kgx",
            "konsole",
            "xfce4-terminal",
            "alacritty",
            "kitty",
            "foot",
            "wezterm",
            "xterm",
        ]
        .map(PathBuf::from),
    );
    linux_terminal_from(program, candidates)
}

#[cfg(not(windows))]
fn linux_terminal_from(
    program: &[String],
    candidates: impl IntoIterator<Item = PathBuf>,
) -> Result<Command> {
    let terminal = candidates
        .into_iter()
        .find_map(|candidate| {
            if candidate.is_absolute() {
                crate::linux::apps::is_executable(&candidate).then_some(candidate)
            } else {
                find_on_path(&candidate.to_string_lossy())
            }
        })
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
        "xfce4-terminal" => &["-x"],
        "kitty" | "foot" => &[],
        _ => &["-e"],
    }
}

#[cfg(not(windows))]
fn find_on_path(name: &str) -> Option<PathBuf> {
    crate::linux::apps::on_path(name)
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
    fn terminal_selection_skips_unusable_files_and_preserves_command_arguments() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let bad = root.path().join("unusable-terminal");
        let good = root.path().join("konsole");
        fs::write(&bad, b"not executable").unwrap();
        fs::write(&good, b"fixture, never launched").unwrap();
        fs::set_permissions(&good, fs::Permissions::from_mode(0o755)).unwrap();
        let program = [
            "bash".to_owned(),
            "/tmp/path with spaces/script.sh".to_owned(),
        ];
        let command = linux_terminal_from(&program, [bad.clone(), good.clone()]).unwrap();
        assert_eq!(command.get_program(), good.as_os_str());
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["-e", "bash", "/tmp/path with spaces/script.sh"]
        );
        assert!(linux_terminal_from(&program, [bad]).is_err());
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
        let command = linux_terminal_from(&["bash".to_owned()], [alias]).unwrap();
        assert_eq!(command.get_args().collect::<Vec<_>>(), ["bash"]);
    }

    #[test]
    fn terminals_receive_the_command_in_their_own_syntax() {
        assert_eq!(terminal_prefix("ptyxis"), ["--"]);
        assert_eq!(terminal_prefix("gnome-terminal.wrapper"), ["--"]);
        assert_eq!(terminal_prefix("wezterm"), ["start", "--"]);
        assert_eq!(terminal_prefix("kitty"), [] as [&str; 0]);
        assert_eq!(terminal_prefix("konsole"), ["-e"]);
        assert_eq!(terminal_prefix("xterm"), ["-e"]);
    }
}
