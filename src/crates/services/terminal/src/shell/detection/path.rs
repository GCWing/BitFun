use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{ShellCandidate, ShellDetector};

impl ShellDetector {
    pub(super) fn find_all_in_path(executable: &str) -> Vec<PathBuf> {
        let Ok(path_var) = std::env::var("PATH") else {
            return Vec::new();
        };
        let mut paths = Vec::new();
        let mut seen = HashSet::new();
        for entry in split_path_entries(&path_var) {
            for name in executable_names(executable) {
                let candidate = entry.join(name);
                if seen.insert(normalized_path_identity(&candidate)) {
                    paths.push(candidate);
                }
            }
        }
        paths
    }
}

pub(super) fn split_path_entries(path_var: &str) -> Vec<PathBuf> {
    let separator = if cfg!(windows) { ';' } else { ':' };
    path_var
        .split(separator)
        .map(|entry| entry.trim().trim_matches('"'))
        .filter(|entry| !entry.is_empty())
        .map(PathBuf::from)
        .collect()
}

pub(super) fn executable_names(executable: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        let extensions =
            std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
        let names = extensions
            .split(';')
            .map(str::trim)
            .filter(|extension| !extension.is_empty())
            .map(|extension| {
                if extension.starts_with('.') {
                    format!("{executable}{extension}")
                } else {
                    format!("{executable}.{extension}")
                }
            })
            .collect::<Vec<_>>();
        if names.is_empty() {
            vec![format!("{executable}.exe")]
        } else {
            names
        }
    }
    #[cfg(not(windows))]
    {
        vec![executable.to_string()]
    }
}

pub(super) fn is_regular_file(path: &Path) -> bool {
    path.metadata().is_ok_and(|metadata| metadata.is_file())
}

pub(super) fn candidate_identity(candidate: &ShellCandidate) -> (super::ShellType, String) {
    (
        candidate.shell_type.clone(),
        normalized_path_identity(&candidate.path),
    )
}

pub(super) fn normalized_path_identity(path: &Path) -> String {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let value = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        value.to_ascii_lowercase()
    } else {
        value
    }
}
