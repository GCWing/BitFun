use crate::util::string::shell_single_quote;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteListCommandPlan {
    pub scan_command: String,
    pub listing_command: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteListEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

pub fn build_remote_list_commands(resolved_path: &str, limit: usize) -> RemoteListCommandPlan {
    let quoted_path = shell_single_quote(resolved_path);
    RemoteListCommandPlan {
        scan_command: format!(
            "find {path} -maxdepth 1 -not -name '.*' -not -path {path} | head -n {head_limit} | sort",
            path = quoted_path,
            head_limit = limit + 1,
        ),
        listing_command: format!(
            "ls -la --time-style=long-iso {path} 2>/dev/null || ls -la {path}",
            path = quoted_path,
        ),
    }
}

pub fn parse_remote_list_entries(stdout: &str) -> Vec<RemoteListEntry> {
    stdout
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let name = Path::new(line)
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| line.to_string());
            RemoteListEntry {
                name,
                path: line.to_string(),
                is_dir: line.ends_with('/'),
            }
        })
        .collect()
}
