//! Runtime lookup for npm provider CLIs launched outside an interactive shell.

use std::{
    ffi::OsStr,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

pub(super) fn prepare(command: &mut Command) {
    let program = PathBuf::from(command.get_program());
    prepare_for_program(command, &program);
}

/// Terminal handoffs execute a CLI indirectly, but need its runtime too.
pub(crate) fn prepare_for_program(command: &mut Command, program: &Path) {
    if !program.is_absolute() || !uses_env_node(program) {
        return;
    }
    let path = match command.get_envs().find(|(key, _)| *key == "PATH") {
        Some((_, value)) => value.map(OsStr::to_owned),
        None => std::env::var_os("PATH"),
    }
    .unwrap_or_else(|| "/bin:/usr/bin".into());
    // Preserve the runtime the user's environment already selects. Recover
    // the npm installation's runtime only when desktop PATH cannot find one.
    if std::env::split_paths(&path).any(|directory| {
        let directory = if directory.is_absolute() {
            directory
        } else {
            command
                .get_current_dir()
                .unwrap_or(Path::new("."))
                .join(directory)
        };
        super::apps::is_executable(&directory.join("node"))
    }) {
        return;
    }
    let Some(directory) = node_directory(program) else {
        return;
    };
    let directories = std::iter::once(directory.clone())
        .chain(std::env::split_paths(&path).filter(|path| path != &directory));
    if let Ok(path) = std::env::join_paths(directories) {
        command.env("PATH", path);
    }
}

fn uses_env_node(program: &Path) -> bool {
    if !fs::metadata(program).is_ok_and(|metadata| metadata.is_file()) {
        return false;
    }
    let Ok(mut file) = fs::File::open(program) else {
        return false;
    };
    let mut header = [0; 256];
    let Ok(length) = file.read(&mut header) else {
        return false;
    };
    let line = header[..length]
        .split(|byte| *byte == b'\n')
        .next()
        .unwrap_or_default();
    let Some(description) = std::str::from_utf8(line)
        .ok()
        .and_then(|line| line.strip_prefix("#!"))
    else {
        return false;
    };
    let mut words = description.split_ascii_whitespace();
    if words.next().and_then(|path| Path::new(path).file_name()) != Some(OsStr::new("env")) {
        return false;
    }
    match words.next() {
        Some("node") => true,
        Some("-S") => words.next() == Some("node"),
        _ => false,
    }
}

fn node_directory(program: &Path) -> Option<PathBuf> {
    let sibling = program.parent()?;
    if super::apps::is_executable(&sibling.join("node")) {
        return Some(sibling.to_owned());
    }
    // Canonical discovery can resolve the npm bin symlink. Recover its
    // install prefix from <prefix>/lib/node_modules/<package>/... instead.
    // This also handles NVM installations without sourcing shell startup files.
    let canonical = program.canonicalize().ok()?;
    for modules in canonical
        .ancestors()
        .filter(|path| path.file_name() == Some(OsStr::new("node_modules")))
    {
        let library = modules.parent()?;
        if library.file_name() != Some(OsStr::new("lib")) {
            continue;
        }
        let directory = library.parent()?.join("bin");
        if super::apps::is_executable(&directory.join("node")) {
            return Some(directory);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Read,
        os::unix::fs::{PermissionsExt, symlink},
        process::Stdio,
    };

    fn executable(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn npm_cli_uses_its_own_node_without_an_interactive_shell_path() {
        let root = tempfile::tempdir().unwrap();
        let prefix = root.path().join("node version with spaces");
        executable(
            &prefix.join("bin/node"),
            "#!/bin/sh\nprintf 'selected runtime: %s\\n' \"$2\"\n",
        );
        let script = prefix.join("lib/node_modules/@openai/codex/bin/codex.js");
        executable(
            &script,
            "#!/usr/bin/env node\nfixture, never interpreted as JavaScript\n",
        );
        let launcher = prefix.join("bin/codex");
        symlink(&script, &launcher).unwrap();
        for program in [&launcher, &script] {
            let mut command = Command::new(program);
            command
                .arg("probe argument")
                .env("PATH", root.path().join("empty"))
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            let mut child = super::super::login_child::LoginChild::spawn(command).unwrap();
            let mut output = String::new();
            child
                .stdout
                .take()
                .unwrap()
                .read_to_string(&mut output)
                .unwrap();
            assert!(child.wait().unwrap().success());
            assert_eq!(output, "selected runtime: probe argument\n");
        }
        // A terminal executes the CLI indirectly; the selected installation's
        // runtime must reach that handoff without depending on shell rc files.
        let mut terminal = Command::new("/bin/sh");
        terminal
            .args(["-c", "exec \"$@\"", "fixture-terminal"])
            .arg(&script)
            .arg("terminal argument")
            .env("PATH", root.path().join("empty"));
        prepare_for_program(&mut terminal, &script);
        let output = terminal.output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"selected runtime: terminal argument\n");

        let active_bin = root.path().join("active/bin");
        executable(
            &active_bin.join("node"),
            "#!/bin/sh\nprintf 'active runtime\\n'\n",
        );
        let mut command = Command::new(&launcher);
        command.env("PATH", &active_bin).stdout(Stdio::piped());
        let mut child = super::super::login_child::LoginChild::spawn(command).unwrap();
        let mut output = String::new();
        child
            .stdout
            .take()
            .unwrap()
            .read_to_string(&mut output)
            .unwrap();
        assert!(child.wait().unwrap().success());
        assert_eq!(output, "active runtime\n");
    }

    #[test]
    fn other_scripts_keep_their_runtime_and_path() {
        let root = tempfile::tempdir().unwrap();
        let script = root.path().join("bin/provider");
        executable(&script, "#!/bin/sh\nexit 0\n");
        executable(&root.path().join("bin/node"), "#!/bin/sh\nexit 0\n");
        let mut command = Command::new(script);
        command.env("PATH", "/chosen/search/path");
        prepare(&mut command);
        assert_eq!(
            command
                .get_envs()
                .find(|(key, _)| *key == "PATH")
                .unwrap()
                .1,
            Some(OsStr::new("/chosen/search/path"))
        );
        let unrelated = root.path().join("scripts/provider.js");
        executable(&unrelated, "#!/usr/bin/env node\n");
        assert!(node_directory(&unrelated).is_none());
    }
}
