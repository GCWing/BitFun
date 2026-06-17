use super::types::{GitChangedFile, GitChangedFileStatus};

/// Parses output from `git diff --name-status`.
pub fn parse_name_status_output(output: &str) -> Vec<GitChangedFile> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let raw_status = parts.next()?.trim();
            if raw_status.is_empty() {
                return None;
            }

            let status = match raw_status.chars().next().unwrap_or_default() {
                'A' => GitChangedFileStatus::Added,
                'M' => GitChangedFileStatus::Modified,
                'D' => GitChangedFileStatus::Deleted,
                'R' => GitChangedFileStatus::Renamed,
                'C' => GitChangedFileStatus::Copied,
                _ => GitChangedFileStatus::Unknown,
            };

            match status {
                GitChangedFileStatus::Renamed | GitChangedFileStatus::Copied => {
                    let old_path = parts.next()?.to_string();
                    let path = parts.next()?.to_string();
                    Some(GitChangedFile {
                        path,
                        old_path: Some(old_path),
                        status,
                    })
                }
                _ => {
                    let path = parts.next()?.to_string();
                    Some(GitChangedFile {
                        path,
                        old_path: None,
                        status,
                    })
                }
            }
        })
        .collect()
}
