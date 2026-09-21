//! Shared troubleshooting launcher for the CLI alias and the Settings Log page.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};

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

fn launch_terminal(selected: Option<&AvailableAiTool>) -> Result<()> {
    let script = script_path()?;
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
        powershell_args.push(tool.executable.to_string_lossy().into_owned());
    }

    #[cfg(windows)]
    {
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

    #[cfg(not(windows))]
    {
        let _ = powershell_args;
        bail!("troubleshooting terminals are only supported on Windows")
    }
}

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

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| fs::canonicalize(candidate).is_ok_and(|path| path.is_file()))
}
