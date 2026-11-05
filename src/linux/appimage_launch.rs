//! Stable launch wrappers for independent AppImage extraction lifetimes.

use std::{
    fs,
    io::Write,
    os::unix::{ffi::OsStrExt, fs::PermissionsExt},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

pub(super) fn for_image(image: &Path) -> Result<PathBuf> {
    let base = directories::BaseDirs::new().context("resolve AppImage launcher directory")?;
    launcher_in(&base.data_dir().join("codex-minibar/launchers"), image)
}

fn launcher_in(directory: &Path, image: &Path) -> Result<PathBuf> {
    let digest = Sha256::digest(image.as_os_str().as_bytes());
    let path = directory.join(format!("appimage-{digest:x}.sh"));
    let template = include_bytes!("../../assets/appimage-launch.sh");
    let marker = b"@APPIMAGE@";
    let position = template
        .windows(marker.len())
        .position(|part| part == marker)
        .context("AppImage launcher template has no target marker")?;
    let mut contents = template[..position].to_vec();
    contents.push(b'\'');
    for byte in image.as_os_str().as_bytes() {
        if *byte == b'\'' {
            contents.extend_from_slice(b"'\\''");
        } else {
            contents.push(*byte);
        }
    }
    contents.push(b'\'');
    contents.extend_from_slice(&template[position + marker.len()..]);
    if fs::symlink_metadata(&path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o7777 == 0o700)
        && fs::read(&path).is_ok_and(|previous| previous == contents)
    {
        return Ok(path);
    }
    fs::create_dir_all(directory).context("create AppImage launcher directory")?;
    let mut staged =
        tempfile::NamedTempFile::new_in(directory).context("stage AppImage launcher")?;
    staged
        .write_all(&contents)
        .context("write AppImage launcher")?;
    staged
        .as_file()
        .set_permissions(fs::Permissions::from_mode(0o700))
        .context("make AppImage launcher private and executable")?;
    staged
        .as_file()
        .sync_all()
        .context("flush AppImage launcher")?;
    staged
        .persist(&path)
        .map_err(|error| error.error)
        .context("publish AppImage launcher")?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Read,
        os::unix::{ffi::OsStringExt, fs::symlink},
        process::{Command, Stdio},
        time::{Duration, Instant},
    };

    #[test]
    fn overlapping_launches_cannot_remove_another_instances_extracted_files() {
        let root = tempfile::tempdir().unwrap();
        let image = root.path().join(std::ffi::OsString::from_vec(
            b"Image quote' $` space-\xff".to_vec(),
        ));
        fs::write(&image, "#!/bin/sh\n[ \"$1\" = --appimage-extract-and-run ] || exit 64\nshift\nmkdir \"$TMPDIR/appimage_extracted_fixture\"\nprintf '%s' \"$TMPDIR\" > \"$1\"\nif [ \"$2\" = hold ]; then while [ ! -f \"$3\" ]; do sleep 0.01; done; fi\nrm -rf \"$TMPDIR/appimage_extracted_fixture\"\n").unwrap();
        fs::set_permissions(&image, fs::Permissions::from_mode(0o755)).unwrap();
        let launcher = launcher_in(&root.path().join("launchers"), &image).unwrap();
        let first_info = root.path().join("first");
        let second_info = root.path().join("second");
        let release = root.path().join("release");
        let mut command = Command::new(&launcher);
        command
            .arg(super::super::autostart::EXTRACT_AND_RUN_FLAG)
            .arg(&first_info)
            .arg("hold")
            .arg(&release)
            .env("TMPDIR", root.path())
            .env_remove("CODEX_MINIBAR_APPIMAGE_TMPDIR")
            .env_remove("NO_CLEANUP")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut first = super::super::login_child::LoginChild::spawn(command).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !first_info.exists() {
            assert!(first.try_wait().unwrap().is_none());
            assert!(Instant::now() < deadline, "first fixture did not start");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            Command::new(&launcher)
                .arg(super::super::autostart::EXTRACT_AND_RUN_FLAG)
                .arg(&second_info)
                .arg("quick")
                .arg(&release)
                .env("TMPDIR", root.path())
                .env_remove("CODEX_MINIBAR_APPIMAGE_TMPDIR")
                .env_remove("NO_CLEANUP")
                .status()
                .unwrap()
                .success()
        );
        let read_path =
            |file: &Path| PathBuf::from(std::ffi::OsString::from_vec(fs::read(file).unwrap()));
        let first_root = read_path(&first_info);
        let second_root = read_path(&second_info);
        assert_ne!(first_root, second_root);
        assert!(first_root.join("appimage_extracted_fixture").is_dir());
        assert!(!second_root.exists());
        fs::write(release, "done").unwrap();
        assert!(first.wait().unwrap().success());
        assert!(!first_root.exists());
    }

    #[test]
    fn delayed_descendant_launch_survives_parent_temporary_directory_cleanup() {
        let root = tempfile::tempdir().unwrap();
        let image = root.path().join("fixture.AppImage");
        fs::write(
            &image,
            "#!/bin/sh\nprintf '%s\\n%s\\n' \"$TMPDIR\" \"$CODEX_MINIBAR_APPIMAGE_TMPDIR\"\n",
        )
        .unwrap();
        fs::set_permissions(&image, fs::Permissions::from_mode(0o755)).unwrap();
        let launcher = launcher_in(&root.path().join("launchers"), &image).unwrap();
        let first = Command::new(&launcher)
            .env("TMPDIR", root.path())
            .env_remove("CODEX_MINIBAR_APPIMAGE_TMPDIR")
            .env_remove("NO_CLEANUP")
            .output()
            .unwrap();
        assert!(first.status.success());
        let output = String::from_utf8(first.stdout).unwrap();
        let mut lines = output.lines();
        let private = Path::new(lines.next().unwrap());
        let base = Path::new(lines.next().unwrap());
        assert!(!private.exists());
        assert_eq!(base, root.path());
        let second = Command::new(&launcher)
            .env("TMPDIR", private)
            .env("CODEX_MINIBAR_APPIMAGE_TMPDIR", base)
            .env_remove("NO_CLEANUP")
            .output()
            .unwrap();
        assert!(second.status.success(), "{:?}", second.stderr);
        let output = String::from_utf8(second.stdout).unwrap();
        let next = Path::new(output.lines().next().unwrap());
        assert_eq!(next.parent(), Some(root.path()));
        assert_ne!(next, private);
        assert!(!next.exists());
    }

    #[test]
    fn launcher_publication_is_private_atomic_and_does_not_follow_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let image = root.path().join("Image.AppImage");
        let path = launcher_in(root.path(), &image).unwrap();
        fs::write(&path, "old launcher").unwrap();
        let mut reader = fs::File::open(&path).unwrap();
        assert_eq!(launcher_in(root.path(), &image).unwrap(), path);
        let mut old = String::new();
        reader.read_to_string(&mut old).unwrap();
        assert_eq!(old, "old launcher");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o7777,
            0o700
        );
        let outside = root.path().join("outside");
        fs::write(&outside, "untouched").unwrap();
        fs::remove_file(&path).unwrap();
        symlink(&outside, &path).unwrap();
        launcher_in(root.path(), &image).unwrap();
        assert_eq!(fs::read_to_string(outside).unwrap(), "untouched");
        assert!(!fs::symlink_metadata(path).unwrap().file_type().is_symlink());
    }
}
