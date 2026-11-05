//! In-place updates for the AppImage and tarball installs.
//!
//! A running executable may be renamed over, so the new version is fully
//! prepared beside the old one and then swapped in atomically. Anything this
//! process does not own, such as a distribution package under `/usr`, is left
//! to its package manager: [`InstallKind::Unmanaged`] has no update asset, so
//! the updater falls back to the release page.

use std::{
    ffi::OsStr,
    fs,
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::OnceLock,
};

use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;
use tar::{Archive, EntryType};

use crate::updater::{self, AvailableUpdate};

const BINARY: &str = "codex-minibar";
/// Installed only by the release tarball's layout (`install_tree` in
/// `build-linux.sh`), so it tells a tarball prefix from any other `bin/`.
const TARBALL_MARKER: &str = "share/doc/codex-minibar/LICENSE";
/// Staging directories beside the installed program. They vanish on their own
/// unless the process is killed mid-update.
const STAGING_PREFIX: &str = ".codex-minibar-update-";
/// Waits for this process to exit, because a new instance started earlier
/// would hand over to it and quit. Bounded: a parent that never reaps this
/// process leaves a zombie that `kill -0` still finds.
const RELAUNCH_SCRIPT: &str = r#"i=0
while kill -0 "$1" 2>/dev/null && [ "$i" -lt 100 ]; do
    sleep 0.2
    i=$((i + 1))
done
shift
exec "$0" "$@""#;
/// Set by the AppImage runtime; the relaunched program must not inherit them.
const APPIMAGE_ENV: [&str; 4] = ["APPIMAGE", "APPDIR", "ARGV0", "OWD"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallKind {
    AppImage(PathBuf),
    Tarball { prefix: PathBuf, binary: PathBuf },
    Unmanaged,
}

/// Resolved once: after a swap, the running executable's path no longer
/// describes the install, and a failed relaunch must stay retryable.
pub fn install_kind() -> &'static InstallKind {
    static KIND: OnceLock<InstallKind> = OnceLock::new();
    KIND.get_or_init(|| match std::env::current_exe() {
        Ok(executable) => detect(
            std::env::var_os("APPIMAGE").as_deref(),
            std::env::var_os("APPDIR").as_deref(),
            &executable,
        ),
        Err(error) => {
            crate::logger::info(format!("update install detection: {error}"));
            InstallKind::Unmanaged
        }
    })
}

fn detect(appimage: Option<&OsStr>, appdir: Option<&OsStr>, executable: &Path) -> InstallKind {
    if let Some(appdir) = appdir.and_then(|path| fs::canonicalize(path).ok())
        // The variables are inherited by everything an AppImage starts, such
        // as a terminal: only trust them when they describe this program.
        && executable.starts_with(&appdir)
    {
        return match appimage.and_then(|path| fs::canonicalize(path).ok()) {
            Some(path) if path.is_file() && parent_writable(&path) => InstallKind::AppImage(path),
            _ => InstallKind::Unmanaged,
        };
    }
    detect_tarball(executable).unwrap_or(InstallKind::Unmanaged)
}

fn detect_tarball(executable: &Path) -> Option<InstallKind> {
    if executable.file_name()? != BINARY {
        return None;
    }
    let bin = executable.parent()?;
    if bin.file_name()? != "bin" {
        return None;
    }
    let prefix = bin.parent()?;
    // Never replace files the system package manager owns, even as root.
    if prefix == Path::new("/usr")
        || !prefix.join(TARBALL_MARKER).is_file()
        || !parent_writable(executable)
        || !dir_writable(&prefix.join("share"))
    {
        return None;
    }
    Some(InstallKind::Tarball {
        prefix: prefix.to_path_buf(),
        binary: executable.to_path_buf(),
    })
}

fn parent_writable(path: &Path) -> bool {
    path.parent().is_some_and(dir_writable)
}

fn dir_writable(dir: &Path) -> bool {
    tempfile::NamedTempFile::new_in(dir).is_ok()
}

/// The end of the release asset's name for this install.
pub fn asset_suffix(arch: &str) -> Option<String> {
    suffix_for(install_kind(), arch)
}

fn suffix_for(kind: &InstallKind, arch: &str) -> Option<String> {
    match kind {
        InstallKind::AppImage(_) => Some(format!("-linux-{arch}.AppImage")),
        InstallKind::Tarball { .. } => Some(format!("-linux-{arch}.tar.gz")),
        InstallKind::Unmanaged => None,
    }
}

/// Downloads and swaps in the update, and schedules the relaunch. The caller
/// exits the process afterwards.
pub fn install(update: &AvailableUpdate) -> Result<()> {
    updater::report_download_started(&update.version);
    if update.sha256.is_none() {
        bail!("update package has no published checksum");
    }
    let program = match install_kind() {
        InstallKind::AppImage(target) => {
            install_appimage(update, target)?;
            target
        }
        InstallKind::Tarball { prefix, binary } => {
            install_tarball(update, prefix, binary)?;
            binary
        }
        InstallKind::Unmanaged => bail!("this installation cannot be updated in place"),
    };
    // The update is already in place, so a lost notice is not worth failing it.
    if let Err(error) = write_success_marker(&update.version) {
        crate::logger::info(format!("post-update marker: {error:#}"));
    }
    spawn_relaunch(program, std::process::id()).context("schedule relaunch")?;
    updater::report_restart();
    Ok(())
}

fn install_appimage(update: &AvailableUpdate, target: &Path) -> Result<()> {
    let staging = staging_dir(target)?;
    let package = staging.path().join("package.AppImage");
    updater::download_package(update, &package)?;
    updater::report_unpack(1, 1);
    swap_in(&package, target)
}

fn install_tarball(update: &AvailableUpdate, prefix: &Path, binary: &Path) -> Result<()> {
    let staging = staging_dir(binary)?;
    let package = staging.path().join("package.tar.gz");
    updater::download_package(update, &package)?;
    updater::report_unpack(0, 1);
    let extracted = staging.path().join("extracted");
    let root = unpack_tarball(&package, &extracted)?;
    updater::report_unpack(1, 1);
    apply_tarball(&root, prefix, binary)
}

/// In the target's directory, because a rename cannot cross filesystems.
fn staging_dir(target: &Path) -> Result<tempfile::TempDir> {
    let dir = target.parent().context("install path has no parent")?;
    tempfile::Builder::new()
        .prefix(STAGING_PREFIX)
        .tempdir_in(dir)
        .with_context(|| format!("create update staging in {}", dir.display()))
}

/// Makes `source` executable and durable, then atomically replaces `target`.
fn swap_in(source: &Path, target: &Path) -> Result<()> {
    let file = fs::File::open(source).with_context(|| format!("open {}", source.display()))?;
    file.set_permissions(fs::Permissions::from_mode(0o755))
        .context("mark the update executable")?;
    file.sync_all().context("flush the update to disk")?;
    fs::rename(source, target).with_context(|| format!("replace {}", target.display()))
}

/// Extracts the tarball into `destination` and returns its single root
/// directory, which must hold the program.
fn unpack_tarball(archive: &Path, destination: &Path) -> Result<PathBuf> {
    fs::create_dir_all(destination).context("create extraction directory")?;
    let file = fs::File::open(archive).with_context(|| format!("open {}", archive.display()))?;
    let mut archive = Archive::new(GzDecoder::new(file));
    for entry in archive.entries().context("read update tarball")? {
        let mut entry = entry.context("read update tarball entry")?;
        let path = entry
            .path()
            .context("read tarball entry path")?
            .into_owned();
        // The package has no links; refusing them keeps every later path
        // inside the staging directory.
        if !matches!(
            entry.header().entry_type(),
            EntryType::Regular | EntryType::Directory
        ) {
            bail!(
                "unexpected entry type in update tarball: {}",
                path.display()
            );
        }
        if !entry
            .unpack_in(destination)
            .with_context(|| format!("extract {}", path.display()))?
        {
            bail!("unsafe path in update tarball: {}", path.display());
        }
    }
    let mut roots = fs::read_dir(destination)
        .context("read extracted update")?
        .collect::<std::io::Result<Vec<_>>>()
        .context("read extracted update")?;
    let root = match (roots.pop(), roots.is_empty()) {
        (Some(root), true) => root.path(),
        _ => bail!("update tarball must have a single root directory"),
    };
    if !root.join("bin").join(BINARY).is_file() {
        bail!("update tarball has no {BINARY} program");
    }
    Ok(root)
}

/// The program goes last: until then the old version stays complete enough to
/// run and retry, and only gains newer icons and a newer desktop entry.
fn apply_tarball(root: &Path, prefix: &Path, binary: &Path) -> Result<()> {
    // Keep a per-user launcher independent of the graphical session's PATH,
    // including after an update replaces the packaged desktop entry.
    let desktop = root.join("share/applications/io.github.vertopolkalf.CodexMinibar.desktop");
    if desktop.is_file() {
        let template = fs::read_to_string(&desktop).context("read updated desktop entry")?;
        fs::write(&desktop, super::identity::desktop_entry(&template, binary))
            .context("set updated desktop entry executable")?;
    }
    copy_tree(&root.join("share"), &prefix.join("share"))?;
    if root.join("share/icons/hicolor").is_dir()
        && let Err(error) = super::identity::invalidate_icon_cache(&prefix.join("share"))
    {
        crate::logger::info(format!("updated desktop icon cache: {error:#}"));
    }
    swap_in(&root.join("bin").join(BINARY), binary)
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination).with_context(|| format!("create {}", destination.display()))?;
    for entry in fs::read_dir(source).with_context(|| format!("read {}", source.display()))? {
        let entry = entry.with_context(|| format!("read {}", source.display()))?;
        let target = destination.join(entry.file_name());
        if entry.file_type().context("read file type")?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            copy_resource(&entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Desktop shells may read launchers and widgets while they are updated.
/// Publish complete files by rename, replacing a file symlink itself rather
/// than truncating its target outside the installation.
fn copy_resource(source: &Path, target: &Path) -> Result<()> {
    let parent = target
        .parent()
        .context("resolve update resource directory")?;
    let staged = tempfile::NamedTempFile::new_in(parent).context("stage update resource")?;
    fs::copy(source, staged.path()).with_context(|| format!("stage {}", target.display()))?;
    staged
        .as_file()
        .sync_all()
        .context("flush update resource")?;
    staged
        .persist(target)
        .map_err(|error| error.error)
        .with_context(|| format!("replace {}", target.display()))?;
    Ok(())
}

fn write_success_marker(version: &str) -> Result<()> {
    let path = updater::update_success_marker()?;
    fs::write(&path, version).with_context(|| format!("write {}", path.display()))
}

/// Starts a detached shell that opens `program` once `pid` has exited. Spawning
/// it now, rather than at exit, keeps a failure reportable while the app runs.
fn spawn_relaunch(program: &Path, pid: u32) -> Result<Child> {
    let extract = super::autostart::requires_extraction();
    let launcher = if extract {
        super::appimage_launch::for_image(program)?
    } else {
        program.to_owned()
    };
    let mut command = relaunch_command(&launcher, pid, extract);
    if let Some(directory) = program.parent() {
        command.current_dir(directory);
    }
    command
        .spawn()
        .with_context(|| format!("start {}", program.display()))
}

fn relaunch_command(program: &Path, pid: u32, extract: bool) -> Command {
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", RELAUNCH_SCRIPT])
        .arg(program)
        .arg(pid.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // Outlives a terminal the app was started from.
        .process_group(0);
    if let Some(dir) = program.parent() {
        command.current_dir(dir);
    }
    for variable in APPIMAGE_ENV {
        command.env_remove(variable);
    }
    if extract {
        command.arg(super::autostart::EXTRACT_AND_RUN_FLAG);
    }
    command
}

/// Removes what a killed update left beside the installed program.
pub fn remove_stale_staging() {
    let dir = match install_kind() {
        InstallKind::AppImage(path) => path.parent(),
        InstallKind::Tarball { binary, .. } => binary.parent(),
        InstallKind::Unmanaged => None,
    };
    if let Some(dir) = dir {
        remove_stale_staging_in(dir);
    }
}

fn remove_stale_staging_in(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with(STAGING_PREFIX)
        {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::GzEncoder};
    use std::os::unix::fs::MetadataExt;

    fn touch(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn tarball_prefix(root: &Path) -> (PathBuf, PathBuf) {
        let prefix = root.canonicalize().unwrap();
        let binary = prefix.join("bin").join(BINARY);
        touch(&binary, "old program");
        touch(&prefix.join(TARBALL_MARKER), "license");
        (prefix, binary)
    }

    fn append(builder: &mut tar::Builder<GzEncoder<fs::File>>, path: &str, contents: &str) {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, path, contents.as_bytes())
            .unwrap();
    }

    fn write_tarball(path: &Path, build: impl FnOnce(&mut tar::Builder<GzEncoder<fs::File>>)) {
        let encoder = GzEncoder::new(fs::File::create(path).unwrap(), Compression::fast());
        let mut builder = tar::Builder::new(encoder);
        build(&mut builder);
        builder.into_inner().unwrap().finish().unwrap();
    }

    #[test]
    fn resource_updates_publish_complete_files_and_preserve_permissions() {
        use std::io::Read;
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        touch(&source, "new launcher");
        fs::set_permissions(&source, fs::Permissions::from_mode(0o644)).unwrap();
        touch(&target, "old launcher");
        let mut reader = fs::File::open(&target).unwrap();
        copy_resource(&source, &target).unwrap();
        let mut old = String::new();
        reader.read_to_string(&mut old).unwrap();
        assert_eq!(old, "old launcher");
        assert_eq!(fs::read_to_string(&target).unwrap(), "new launcher");
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o644
        );
    }

    #[test]
    fn resource_updates_replace_file_symlinks_without_writing_their_targets() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let external = dir.path().join("external");
        let target = dir.path().join("target");
        touch(&source, "new resource");
        touch(&external, "unrelated file");
        std::os::unix::fs::symlink(&external, &target).unwrap();
        copy_resource(&source, &target).unwrap();
        assert_eq!(fs::read_to_string(&external).unwrap(), "unrelated file");
        assert_eq!(fs::read_to_string(&target).unwrap(), "new resource");
        assert!(
            !fs::symlink_metadata(&target)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn an_appimage_is_found_only_when_this_program_runs_from_it() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let appimage = root.join("Codex.AppImage");
        touch(&appimage, "image");
        let mount = root.join("mount");
        let executable = mount.join("usr/bin").join(BINARY);
        touch(&executable, "program");

        assert_eq!(
            detect(
                Some(appimage.as_os_str()),
                Some(mount.as_os_str()),
                &executable
            ),
            InstallKind::AppImage(appimage.clone())
        );
        // Inherited from another AppImage: its file must never be replaced.
        let other_mount = root.join("other-mount");
        fs::create_dir(&other_mount).unwrap();
        assert_eq!(
            detect(
                Some(appimage.as_os_str()),
                Some(other_mount.as_os_str()),
                &executable
            ),
            InstallKind::Unmanaged
        );
        // The variable without a mount that proves it.
        assert_eq!(
            detect(Some(appimage.as_os_str()), None, &executable),
            InstallKind::Unmanaged
        );
    }

    #[test]
    fn a_tarball_install_is_recognized_by_its_layout() {
        let dir = tempfile::tempdir().unwrap();
        let (prefix, binary) = tarball_prefix(dir.path());
        assert_eq!(
            detect(None, None, &binary),
            InstallKind::Tarball {
                prefix: prefix.clone(),
                binary: binary.clone(),
            }
        );
        fs::remove_file(prefix.join(TARBALL_MARKER)).unwrap();
        assert_eq!(detect(None, None, &binary), InstallKind::Unmanaged);
    }

    #[test]
    fn other_installs_are_left_to_their_owners() {
        let dir = tempfile::tempdir().unwrap();
        let (prefix, _) = tarball_prefix(dir.path());
        // A build tree: writable, but not a `bin` directory.
        let build = prefix.join("target/release").join(BINARY);
        touch(&build, "program");
        assert_eq!(detect(None, None, &build), InstallKind::Unmanaged);
        assert_eq!(
            detect(None, None, Path::new("/usr/bin/codex-minibar")),
            InstallKind::Unmanaged
        );
    }

    #[test]
    fn each_install_kind_asks_for_its_own_asset() {
        let tarball = InstallKind::Tarball {
            prefix: PathBuf::new(),
            binary: PathBuf::new(),
        };
        assert_eq!(
            suffix_for(&InstallKind::AppImage(PathBuf::new()), "x64").as_deref(),
            Some("-linux-x64.AppImage")
        );
        assert_eq!(
            suffix_for(&tarball, "arm64").as_deref(),
            Some("-linux-arm64.tar.gz")
        );
        assert_eq!(suffix_for(&InstallKind::Unmanaged, "x64"), None);
    }

    #[test]
    fn swapping_replaces_a_program_while_it_is_open() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("app");
        let source = dir.path().join("new");
        touch(&target, "old");
        touch(&source, "new");
        let running = fs::File::open(&target).unwrap();
        let old_inode = running.metadata().unwrap().ino();

        swap_in(&source, &target).unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert_ne!(fs::metadata(&target).unwrap().ino(), old_inode);
        assert!(!source.exists());
    }

    #[test]
    fn a_tarball_updates_the_program_and_its_desktop_files() {
        let dir = tempfile::tempdir().unwrap();
        let install = dir.path().join("prefix");
        fs::create_dir(&install).unwrap();
        let (prefix, binary) = tarball_prefix(&install);
        let icon = "share/icons/hicolor/48x48/apps/codex-minibar.png";
        touch(&prefix.join(icon), "old icon");
        for relative in ["share/icons", "share/icons/hicolor"] {
            fs::File::open(prefix.join(relative))
                .unwrap()
                .set_modified(std::time::UNIX_EPOCH)
                .unwrap();
        }
        let archive = dir.path().join("package.tar.gz");
        write_tarball(&archive, |builder| {
            append(
                builder,
                "codex-minibar-2.0.0/bin/codex-minibar",
                "new program",
            );
            append(
                builder,
                "codex-minibar-2.0.0/share/applications/a.desktop",
                "entry",
            );
            append(
                builder,
                "codex-minibar-2.0.0/share/applications/io.github.vertopolkalf.CodexMinibar.desktop",
                "[Desktop Entry]\nExec=codex-minibar\n[Desktop Action Toggle]\nExec=codex-minibar --toggle\n",
            );
            append(builder, "codex-minibar-2.0.0/install.sh", "#!/bin/sh");
            append(builder, &format!("codex-minibar-2.0.0/{icon}"), "new icon");
        });

        let root = unpack_tarball(&archive, &dir.path().join("extracted")).unwrap();
        apply_tarball(&root, &prefix, &binary).unwrap();

        assert_eq!(fs::read_to_string(&binary).unwrap(), "new program");
        assert_eq!(
            fs::read_to_string(prefix.join("share/applications/a.desktop")).unwrap(),
            "entry"
        );
        let quoted = super::super::autostart::quote_exec(&binary);
        assert_eq!(
            fs::read_to_string(
                prefix.join("share/applications/io.github.vertopolkalf.CodexMinibar.desktop")
            )
            .unwrap(),
            format!(
                "[Desktop Entry]\nExec={quoted}\n[Desktop Action Toggle]\nExec={quoted} --toggle\n"
            )
        );
        assert!(!prefix.join("install.sh").exists());
        assert_eq!(fs::read_to_string(prefix.join(icon)).unwrap(), "new icon");
        for relative in ["share/icons", "share/icons/hicolor"] {
            assert!(
                fs::metadata(prefix.join(relative))
                    .unwrap()
                    .modified()
                    .unwrap()
                    > std::time::UNIX_EPOCH
            );
        }
        assert_eq!(
            fs::metadata(&binary).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }

    #[test]
    fn a_tarball_without_the_program_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("package.tar.gz");
        write_tarball(&archive, |builder| append(builder, "root/notes.txt", "x"));
        let error = unpack_tarball(&archive, &dir.path().join("extracted")).unwrap_err();
        assert!(error.to_string().contains("no codex-minibar program"));
    }

    #[test]
    fn a_tarball_with_several_roots_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("package.tar.gz");
        write_tarball(&archive, |builder| {
            append(builder, "one/bin/codex-minibar", "x");
            append(builder, "two/bin/codex-minibar", "x");
        });
        let error = unpack_tarball(&archive, &dir.path().join("extracted")).unwrap_err();
        assert!(error.to_string().contains("single root"));
    }

    #[test]
    fn tarball_links_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("package.tar.gz");
        write_tarball(&archive, |builder| {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(EntryType::Symlink);
            header.set_size(0);
            builder
                .append_link(&mut header, "root/link", "/etc/passwd")
                .unwrap();
        });
        let error = unpack_tarball(&archive, &dir.path().join("extracted")).unwrap_err();
        assert!(error.to_string().contains("unexpected entry type"));
    }

    #[test]
    fn tarball_paths_cannot_leave_the_destination() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("package.tar.gz");
        write_tarball(&archive, |builder| {
            let mut header = tar::Header::new_gnu();
            let name = b"../escaped";
            header.as_old_mut().name[..name.len()].copy_from_slice(name);
            header.set_size(1);
            header.set_cksum();
            builder.append(&header, &b"x"[..]).unwrap();
        });
        let extracted = dir.path().join("extracted");
        assert!(unpack_tarball(&archive, &extracted).is_err());
        assert!(!dir.path().join("escaped").exists());
    }

    #[test]
    fn relaunch_runs_the_program_once_the_process_is_gone() {
        let mut finished = Command::new("/bin/sh")
            .args(["-c", "exit 0"])
            .spawn()
            .unwrap();
        let pid = finished.id();
        finished.wait().unwrap();

        // `false` is the only way the script can exit with a failure here, so
        // this proves it reached the program.
        let status = spawn_relaunch(Path::new("/bin/false"), pid)
            .unwrap()
            .wait()
            .unwrap();
        assert_eq!(status.code(), Some(1));
    }

    #[test]
    fn relaunch_keeps_extraction_flags_without_passing_the_old_pid_to_the_app() {
        let root = tempfile::tempdir().unwrap();
        let program = root.path().join("My AppImage");
        touch(
            &program,
            "#!/bin/sh\nprintf '%s\\n' \"$#\" > \"$0.args\"\nfor arg in \"$@\"; do printf '%s\\n' \"$arg\" >> \"$0.args\"; done\n",
        );
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
        let mut finished = Command::new("/bin/sh")
            .args(["-c", "exit 0"])
            .spawn()
            .unwrap();
        let pid = finished.id();
        finished.wait().unwrap();
        for (extract, expected) in [(false, "0\n"), (true, "1\n--appimage-extract-and-run\n")] {
            let mut command = relaunch_command(&program, pid, extract);
            for name in APPIMAGE_ENV {
                assert_eq!(
                    command.get_envs().find(|(key, _)| *key == name).unwrap().1,
                    None
                );
            }
            assert!(command.spawn().unwrap().wait().unwrap().success());
            assert_eq!(
                fs::read_to_string(root.path().join("My AppImage.args")).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn stale_staging_is_removed_and_the_install_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let stale = dir.path().join(format!("{STAGING_PREFIX}abc/package"));
        let program = dir.path().join("Codex.AppImage");
        touch(&stale, "partial");
        touch(&program, "image");

        remove_stale_staging_in(dir.path());

        assert!(!stale.parent().unwrap().exists());
        assert!(program.exists());
    }
}
