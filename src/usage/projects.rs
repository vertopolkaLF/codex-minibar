//! Groups transcript working directories into projects for the Usage tab.
//!
//! Codex and Claude Code record the session's working directory in their
//! transcripts. A session started in a subfolder or a git worktree still
//! belongs to the repository it was launched from, so directories are folded
//! into their repository root (the main checkout for linked worktrees).

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

/// Repository a working directory belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Project {
    /// Stable grouping key. Case-folded on Windows, whose paths are not
    /// case-sensitive.
    pub key: String,
    /// Folder name shown in the breakdown.
    pub name: String,
    /// Full root path, used to tell apart projects with the same name.
    pub root: String,
}

/// Canonical text form of a transcript `cwd`. Codex has written both plain
/// paths and `file://` URLs; trailing separators are dropped so the same
/// folder always yields the same string.
pub(crate) fn normalize_cwd(raw: &str) -> String {
    let raw = raw.trim();
    let path = match raw.strip_prefix("file://") {
        Some(rest) => {
            let rest = percent_decode(rest);
            // `file:///C:/x` keeps a leading slash before the drive letter.
            let rest = match rest.strip_prefix('/') {
                Some(stripped) if has_drive_prefix(stripped) => stripped.to_owned(),
                _ => rest,
            };
            if cfg!(windows) {
                rest.replace('/', "\\")
            } else {
                rest
            }
        }
        None => raw.to_owned(),
    };
    let trimmed = path.trim_end_matches(['/', '\\']);
    // Keep drive and filesystem roots intact (`C:\`, `/`).
    if trimmed.is_empty() || (has_drive_prefix(trimmed) && trimmed.len() == 2) {
        return path;
    }
    trimmed.to_owned()
}

/// Projects of transcript logs that sit in per-project folders. A log that
/// names no `cwd` (deleted, or no header yet) takes the first `cwd` a sibling
/// log in the same folder names, else the folder name itself.
pub(crate) struct LogProjects<'a> {
    folder_cwd: HashMap<&'a str, &'a str>,
}

impl<'a> LogProjects<'a> {
    /// `logs` yields `(folder, cwd)` per log, in a stable order.
    pub(crate) fn new(logs: impl IntoIterator<Item = (Option<&'a str>, Option<&'a str>)>) -> Self {
        let mut folder_cwd = HashMap::new();
        for (folder, cwd) in logs {
            if let (Some(folder), Some(cwd)) = (folder, cwd.filter(|cwd| !cwd.is_empty())) {
                folder_cwd.entry(folder).or_insert(cwd);
            }
        }
        Self { folder_cwd }
    }

    pub(crate) fn project(&self, folder: Option<&'a str>, cwd: Option<&'a str>) -> &'a str {
        cwd.filter(|cwd| !cwd.is_empty())
            .or_else(|| folder.and_then(|folder| self.folder_cwd.get(folder).copied()))
            .or(folder)
            .unwrap_or("")
    }
}

fn has_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let Some(value) = bytes
                .get(index + 1..index + 3)
                .and_then(|hex| std::str::from_utf8(hex).ok())
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            decoded.push(value);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// Resolves a normalized `cwd` to its project. `None` for an empty `cwd`.
/// Results are memoized: the lookup touches the filesystem and runs for every
/// overview rebuild.
pub(crate) fn resolve(cwd: &str) -> Option<Project> {
    if cwd.is_empty() {
        return None;
    }
    static MEMO: Mutex<Option<HashMap<String, Project>>> = Mutex::new(None);
    if let Ok(memo) = MEMO.lock()
        && let Some(project) = memo.as_ref().and_then(|memo| memo.get(cwd))
    {
        return Some(project.clone());
    }
    let project = resolve_uncached(cwd);
    if let Ok(mut memo) = MEMO.lock() {
        memo.get_or_insert_with(HashMap::new)
            .insert(cwd.to_owned(), project.clone());
    }
    Some(project)
}

fn resolve_uncached(cwd: &str) -> Project {
    let path = Path::new(cwd);
    // Claude folder names stand in for a missing `cwd`; they are not paths
    // and must never be resolved against Minibar's own working directory.
    let root = if path.is_absolute() {
        repository_root(path).unwrap_or_else(|| path.to_path_buf())
    } else {
        path.to_path_buf()
    };
    let root = root.to_string_lossy().into_owned();
    // Git writes worktree `gitdir:` targets with forward slashes on Windows.
    let root = if cfg!(windows) {
        root.replace('/', "\\")
    } else {
        root
    };
    let name = Path::new(&root)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| root.clone());
    Project {
        key: if cfg!(windows) {
            root.to_lowercase()
        } else {
            root.clone()
        },
        name,
        root,
    }
}

/// Nearest ancestor holding `.git`. A linked worktree's `.git` file points
/// into `<main>/.git/worktrees/<name>`, which is folded into `<main>`.
fn repository_root(path: &Path) -> Option<PathBuf> {
    for dir in path.ancestors() {
        let dot_git = dir.join(".git");
        let Ok(meta) = fs::metadata(&dot_git) else {
            continue;
        };
        if meta.is_dir() {
            return Some(dir.to_path_buf());
        }
        return Some(
            fs::read_to_string(&dot_git)
                .ok()
                .and_then(|text| worktree_main_checkout(dir, &text))
                .unwrap_or_else(|| dir.to_path_buf()),
        );
    }
    None
}

fn worktree_main_checkout(dir: &Path, dot_git: &str) -> Option<PathBuf> {
    let target = dot_git
        .lines()
        .find_map(|line| line.trim().strip_prefix("gitdir:"))?
        .trim();
    let target = dir.join(target);
    // `<main>/.git/worktrees/<name>`: the main checkout is two levels above
    // `worktrees`. Submodules (`.git/modules/...`) stay their own project.
    let worktrees = target.parent()?;
    if worktrees.file_name()? != "worktrees" {
        return None;
    }
    let git_dir = worktrees.parent()?;
    if git_dir.file_name()? != ".git" {
        return None;
    }
    git_dir.parent().map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_plain_paths_and_file_urls() {
        assert_eq!(normalize_cwd("  /home/me/repo/ "), "/home/me/repo");
        assert_eq!(normalize_cwd("/"), "/");
        if cfg!(windows) {
            assert_eq!(
                normalize_cwd("file:///C:/Users/me/My%20Repo"),
                r"C:\Users\me\My Repo"
            );
            assert_eq!(normalize_cwd(r"C:\Dev\repo\"), r"C:\Dev\repo");
            assert_eq!(normalize_cwd(r"C:\"), r"C:\");
        } else {
            assert_eq!(
                normalize_cwd("file:///home/me/My%20Repo"),
                "/home/me/My Repo"
            );
        }
    }

    #[test]
    fn folds_subfolders_and_worktrees_into_the_main_checkout() {
        let temp = tempfile::tempdir().unwrap();
        let main = temp.path().join("app");
        fs::create_dir_all(main.join(".git").join("worktrees").join("feature")).unwrap();
        fs::create_dir_all(main.join("src").join("ui")).unwrap();
        let worktree = temp.path().join("worktrees").join("app-feature");
        fs::create_dir_all(worktree.join("crates")).unwrap();
        fs::write(
            worktree.join(".git"),
            format!(
                "gitdir: {}\n",
                main.join(".git")
                    .join("worktrees")
                    .join("feature")
                    .display()
            ),
        )
        .unwrap();
        let plain = temp.path().join("scratch");
        fs::create_dir_all(&plain).unwrap();

        let root = |path: &Path| resolve_uncached(&path.to_string_lossy()).root;
        let main_root = main.to_string_lossy().into_owned();
        assert_eq!(root(&main.join("src").join("ui")), main_root);
        assert_eq!(root(&worktree.join("crates")), main_root);
        assert_eq!(resolve_uncached(&main_root).name, "app");
        assert_eq!(root(&plain), plain.to_string_lossy());
    }

    #[test]
    fn relative_names_are_kept_verbatim() {
        let project = resolve_uncached("C--Dev-app");
        assert_eq!(project.name, "C--Dev-app");
        assert_eq!(project.root, "C--Dev-app");
    }

    #[test]
    fn empty_cwd_has_no_project() {
        assert_eq!(resolve(""), None);
    }
}
