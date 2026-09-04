use super::{RemoteDirEntry, RemoteFileService, RemoteWorkspaceEntry};
use openbitfun_services_core::filesystem::{
    FileSearchOutcome, FileSearchProgressSink, FileSearchResult, FileSearchResultGroup,
    SearchMatchType,
};
use regex::Regex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// One remote filename search routed to an already-resolved workspace.
///
/// Target resolution remains with the caller because a POSIX path alone does
/// not identify a remote workspace when multiple SSH connections expose the
/// same root. This service owns only the reusable remote filesystem traversal.
pub struct RemoteFileNameSearch {
    pub remote_fs: RemoteFileService,
    pub workspace: RemoteWorkspaceEntry,
    pub root_path: String,
    pub pattern: String,
    pub case_sensitive: bool,
    pub use_regex: bool,
    pub whole_word: bool,
    pub include_directories: bool,
    pub limit: usize,
    pub cancel_flag: Option<Arc<AtomicBool>>,
    pub progress_sink: Option<Arc<dyn FileSearchProgressSink>>,
}

fn compile_file_name_search_regex(
    pattern: &str,
    case_sensitive: bool,
    use_regex: bool,
    whole_word: bool,
) -> Result<Regex, String> {
    let mut pattern = if use_regex {
        pattern.to_string()
    } else {
        regex::escape(pattern)
    };

    if whole_word {
        pattern = format!(r"\b(?:{})\b", pattern);
    }

    if !case_sensitive {
        pattern = format!("(?i){}", pattern);
    }

    Regex::new(&pattern).map_err(|error| format!("Invalid search pattern: {}", error))
}

fn should_skip_directory(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".svn"
            | ".hg"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".next"
            | ".nuxt"
            | ".cache"
            | ".turbo"
    )
}

fn should_skip_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.rsplit_once('.').map(|(_, ext)| ext),
        Some(
            "png"
                | "jpg"
                | "jpeg"
                | "gif"
                | "webp"
                | "ico"
                | "pdf"
                | "zip"
                | "tar"
                | "gz"
                | "rar"
                | "7z"
                | "exe"
                | "dll"
                | "so"
                | "dylib"
        )
    )
}

fn search_result(entry: &RemoteDirEntry) -> FileSearchResult {
    FileSearchResult {
        path: entry.path.clone(),
        name: entry.name.clone(),
        is_directory: entry.is_dir,
        match_type: SearchMatchType::FileName,
        line_number: None,
        matched_content: None,
        preview_before: None,
        preview_inside: None,
        preview_after: None,
    }
}

/// Searches a remote workspace through its file provider and reports matches
/// as soon as they are discovered. The progress contract is important for
/// high-latency workspaces: callers must not wait for the entire recursive walk
/// before showing an early match.
pub async fn search_remote_file_names(
    search: RemoteFileNameSearch,
) -> Result<FileSearchOutcome, String> {
    let RemoteFileNameSearch {
        remote_fs,
        workspace,
        root_path,
        pattern,
        case_sensitive,
        use_regex,
        whole_word,
        include_directories,
        limit,
        cancel_flag,
        progress_sink,
    } = search;
    let matcher = compile_file_name_search_regex(&pattern, case_sensitive, use_regex, whole_word)?;
    let mut stack = vec![root_path];
    let mut results = Vec::new();
    let mut truncated = false;

    while let Some(directory) = stack.pop() {
        if cancel_flag
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
        {
            break;
        }

        let mut entries = match remote_fs
            .read_dir(&workspace.connection_id, &directory)
            .await
        {
            Ok(entries) => entries,
            Err(error) => {
                if let Some(sink) = progress_sink.as_ref() {
                    sink.flush();
                }
                return Err(format!("Failed to read remote directory: {}", error));
            }
        };
        entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        });

        for child in entries {
            if cancel_flag
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::Relaxed))
            {
                break;
            }

            if child.is_dir {
                if should_skip_directory(&child.name) {
                    continue;
                }

                if include_directories && matcher.is_match(&child.name) {
                    let result = search_result(&child);
                    if let Some(sink) = progress_sink.as_ref() {
                        sink.report(FileSearchResultGroup {
                            path: result.path.clone(),
                            name: result.name.clone(),
                            is_directory: result.is_directory,
                            file_name_match: Some(result.clone()),
                            content_matches: Vec::new(),
                        });
                    }
                    results.push(result);
                    if results.len() >= limit {
                        truncated = true;
                        break;
                    }
                }

                stack.push(child.path);
                continue;
            }

            if !child.is_file || should_skip_file(&child.name) {
                continue;
            }

            if matcher.is_match(&child.name) {
                let result = search_result(&child);
                if let Some(sink) = progress_sink.as_ref() {
                    sink.report(FileSearchResultGroup {
                        path: result.path.clone(),
                        name: result.name.clone(),
                        is_directory: result.is_directory,
                        file_name_match: Some(result.clone()),
                        content_matches: Vec::new(),
                    });
                }
                results.push(result);
                if results.len() >= limit {
                    truncated = true;
                    break;
                }
            }
        }

        if truncated {
            break;
        }
    }

    if let Some(sink) = progress_sink.as_ref() {
        sink.flush();
    }

    Ok(FileSearchOutcome { results, truncated })
}

#[cfg(test)]
mod tests {
    use super::{compile_file_name_search_regex, should_skip_directory, should_skip_file};

    #[test]
    fn literal_search_matches_unicode_directory_names() {
        let matcher = compile_file_name_search_regex("手写", false, false, false)
            .expect("compile literal matcher");
        assert!(matcher.is_match("手写笔画标注项目"));
        assert!(!matcher.is_match("框球项目"));
    }

    #[test]
    fn traversal_filters_keep_existing_search_contract() {
        assert!(should_skip_directory("node_modules"));
        assert!(should_skip_directory(".git"));
        assert!(!should_skip_directory("src"));
        assert!(should_skip_file("archive.zip"));
        assert!(!should_skip_file("README.md"));
    }
}
