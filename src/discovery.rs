use std::{
    collections::HashSet,
    env,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexCandidate {
    pub path: PathBuf,
    pub source: CandidateSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateSource {
    Explicit,
    Path,
    CommonLocation,
}

pub fn discover(explicit: Option<&Path>) -> Vec<CodexCandidate> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    if let Some(path) = explicit {
        push_candidate(&mut candidates, &mut seen, path, CandidateSource::Explicit);
    }

    if let Some(path) = env::var_os("PATH") {
        for directory in env::split_paths(&path) {
            for name in executable_names() {
                push_candidate(
                    &mut candidates,
                    &mut seen,
                    &directory.join(name),
                    CandidateSource::Path,
                );
            }
        }
    }

    for path in common_locations() {
        push_candidate(
            &mut candidates,
            &mut seen,
            &path,
            CandidateSource::CommonLocation,
        );
    }
    candidates
}

fn push_candidate(
    candidates: &mut Vec<CodexCandidate>,
    seen: &mut HashSet<String>,
    path: &Path,
    source: CandidateSource,
) {
    if !path.is_file() {
        return;
    }
    let normalized = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_lowercase();
    if seen.insert(normalized) {
        candidates.push(CodexCandidate {
            path: path.to_path_buf(),
            source,
        });
    }
}

#[cfg(windows)]
fn executable_names() -> &'static [&'static str] {
    &["codex.exe", "codex.cmd", "codex.ps1", "codex.bat"]
}

#[cfg(not(windows))]
fn executable_names() -> &'static [&'static str] {
    &["codex"]
}

fn common_locations() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    #[cfg(windows)]
    {
        if let Some(local) = env::var_os("LOCALAPPDATA") {
            let local = PathBuf::from(local);
            paths.extend([
                local.join("pnpm/codex.cmd"),
                local.join("pnpm/codex.ps1"),
                local.join("npm/codex.cmd"),
                local.join("npm/codex.ps1"),
            ]);
        }
        if let Some(roaming) = env::var_os("APPDATA") {
            let roaming = PathBuf::from(roaming);
            paths.extend([roaming.join("npm/codex.cmd"), roaming.join("npm/codex.ps1")]);
        }
    }
    #[cfg(not(windows))]
    if let Some(home) = env::var_os("HOME") {
        let home = PathBuf::from(home);
        paths.extend([
            home.join(".local/bin/codex"),
            home.join(".npm-global/bin/codex"),
        ]);
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_missing_candidate_is_ignored() {
        assert!(
            discover(Some(Path::new("definitely-not-a-real-codex-binary")))
                .iter()
                .all(|candidate| candidate.source != CandidateSource::Explicit)
        );
    }
}
