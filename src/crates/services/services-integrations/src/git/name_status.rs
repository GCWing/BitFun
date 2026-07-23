use super::types::{GitChangedFile, GitChangedFileStatus};

fn status_from_raw(raw_status: &str) -> GitChangedFileStatus {
    match raw_status.chars().next().unwrap_or_default() {
        'A' => GitChangedFileStatus::Added,
        'M' => GitChangedFileStatus::Modified,
        'D' => GitChangedFileStatus::Deleted,
        'R' => GitChangedFileStatus::Renamed,
        'C' => GitChangedFileStatus::Copied,
        _ => GitChangedFileStatus::Unknown,
    }
}

/// Parses output from `git diff --name-status`.
pub fn parse_name_status_output(output: &str) -> Vec<GitChangedFile> {
    if output.contains('\0') {
        let mut fields = output.split('\0').filter(|field| !field.is_empty());
        let mut files = Vec::new();
        while let Some(raw_status) = fields.next() {
            let status = status_from_raw(raw_status);
            let Some(first_path) = fields.next() else {
                break;
            };
            if matches!(
                status,
                GitChangedFileStatus::Renamed | GitChangedFileStatus::Copied
            ) {
                let Some(path) = fields.next() else {
                    break;
                };
                files.push(GitChangedFile {
                    path: path.to_string(),
                    old_path: Some(first_path.to_string()),
                    status,
                });
            } else {
                files.push(GitChangedFile {
                    path: first_path.to_string(),
                    old_path: None,
                    status,
                });
            }
        }
        return files;
    }

    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let raw_status = parts.next()?.trim();
            if raw_status.is_empty() {
                return None;
            }

            let status = status_from_raw(raw_status);

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
