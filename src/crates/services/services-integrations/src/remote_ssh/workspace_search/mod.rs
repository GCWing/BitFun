//! Remote SSH workspace-search strategy helpers.
//!
//! Concrete SSH channel execution remains in product assembly until a reviewed
//! remote-command provider boundary exists. This module owns remote workspace
//! search strategy plus the flashgrep session/context lifecycle that can run
//! behind that provider boundary.

use super::normalize_remote_workspace_path;
use crate::workspace_search::flashgrep::{
    PathScope, SearchBackend, SearchModeConfig, SearchResults,
};
use crate::workspace_search::ContentSearchOutputMode;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

mod service;

pub use service::{
    RemoteCommandOutput, RemoteWorkspaceSearchProvider, RemoteWorkspaceSearchService,
    RemoteWorkspaceSearchStdioProtocol,
};

pub(crate) const REMOTE_OS_PROBES: &[&str] = &["uname -s", "sh -c 'uname -s 2>/dev/null'"];
pub(crate) const REMOTE_ARCHITECTURE_PROBES: &[&str] = &[
    "uname -m",
    "arch",
    "sh -c 'uname -m 2>/dev/null || arch 2>/dev/null'",
];

const REMOTE_FLASHGREP_INSTALL_DIR: &str = ".bitfun/bin";
const LINUX_X86_64_FLASHGREP_BUNDLES: &[&str] = &[
    "flashgrep-x86_64-unknown-linux-musl",
    "flashgrep-x86_64-unknown-linux-gnu",
];
const LINUX_AARCH64_FLASHGREP_BUNDLES: &[&str] = &[
    "flashgrep-aarch64-unknown-linux-musl",
    "flashgrep-aarch64-unknown-linux-gnu",
];

pub(crate) struct LocalFlashgrepBundle {
    pub(crate) binary_name: String,
    pub(crate) path: PathBuf,
    pub(crate) bytes: Vec<u8>,
    pub(crate) sha256: String,
}

pub(crate) fn build_remote_scope(
    repo_root: &str,
    search_path: Option<&Path>,
    globs: Vec<String>,
    file_types: Vec<String>,
    exclude_file_types: Vec<String>,
) -> Result<PathScope, String> {
    let repo_root = normalize_remote_workspace_path(repo_root);
    let roots = match search_path {
        Some(path) => {
            let normalized = normalize_remote_scope_path(&repo_root, path)?;
            if normalized == repo_root {
                Vec::new()
            } else {
                vec![PathBuf::from(normalized)]
            }
        }
        None => Vec::new(),
    };

    Ok(PathScope {
        roots,
        globs,
        iglobs: Vec::new(),
        type_add: Vec::new(),
        type_clear: Vec::new(),
        types: file_types,
        type_not: exclude_file_types,
    })
}

fn normalize_remote_scope_path(repo_root: &str, search_path: &Path) -> Result<String, String> {
    let raw_path = search_path.to_string_lossy();
    let normalized = if raw_path.starts_with('/') {
        normalize_remote_workspace_path(&raw_path)
    } else {
        join_remote_path(repo_root, &raw_path)
    };
    let repo_root_with_slash = format!("{}/", repo_root.trim_end_matches('/'));
    if normalized != repo_root && !normalized.starts_with(&repo_root_with_slash) {
        return Err(format!(
            "Remote search path is outside workspace root: {normalized}"
        ));
    }
    Ok(normalized)
}

pub(crate) fn remote_flashgrep_install_dir(repo_root: &str) -> String {
    join_remote_path(
        &normalize_remote_workspace_path(repo_root),
        REMOTE_FLASHGREP_INSTALL_DIR,
    )
}

pub(crate) fn remote_workspace_search_storage_root(repo_root: &str) -> String {
    join_remote_path(repo_root, ".bitfun/search/flashgrep-index")
}

pub(crate) fn looks_like_linux_workspace_root(path: &str) -> bool {
    path.starts_with('/') && !path.contains(':')
}

pub(crate) fn parse_remote_architecture_output(stdout: &str, stderr: &str) -> Option<String> {
    for stream in [stdout, stderr] {
        for line in stream.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let normalized = trimmed.to_ascii_lowercase();
            if normalized.contains("x86_64") || normalized.contains("amd64") {
                return Some("x86_64".to_string());
            }
            if normalized.contains("aarch64")
                || normalized.contains("arm64")
                || normalized.contains("armv8")
            {
                return Some("aarch64".to_string());
            }
        }
    }

    None
}

pub(crate) fn parse_remote_os_output(stdout: &str, stderr: &str) -> Option<String> {
    for stream in [stdout, stderr] {
        for line in stream.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let normalized = trimmed.to_ascii_lowercase();
            if normalized.contains("linux") {
                return Some("Linux".to_string());
            }
            if normalized.contains("darwin") || normalized.contains("macos") {
                return Some("Darwin".to_string());
            }
            if normalized.contains("windows")
                || normalized.contains("mingw")
                || normalized.contains("msys")
                || normalized.contains("cygwin")
            {
                return Some("Windows".to_string());
            }
        }
    }

    None
}

fn linux_flashgrep_bundle_names_for_arch(
    remote_arch: &str,
) -> Result<&'static [&'static str], String> {
    match remote_arch {
        "x86_64" | "amd64" => Ok(LINUX_X86_64_FLASHGREP_BUNDLES),
        "aarch64" | "arm64" => Ok(LINUX_AARCH64_FLASHGREP_BUNDLES),
        arch => Err(format!(
            "Remote workspace search does not support Linux architecture: {arch}"
        )),
    }
}

pub(crate) async fn local_flashgrep_bundle_for_arch(
    remote_arch: &str,
) -> Result<LocalFlashgrepBundle, String> {
    let bundled_binary_names = linux_flashgrep_bundle_names_for_arch(remote_arch)?;

    let (binary_name, path) = bundled_binary_names
        .iter()
        .find_map(|binary_name| {
            resolve_local_flashgrep_bundle(binary_name)
                .map(|path| ((*binary_name).to_string(), path))
        })
        .ok_or_else(|| {
            format!(
                "Bundled Linux flashgrep binary is missing. Expected one of: {}",
                bundled_binary_names
                    .iter()
                    .map(|name| format!("resources/flashgrep/{name}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
    let bytes = tokio::fs::read(&path).await.map_err(|error| {
        format!(
            "Failed to read bundled flashgrep binary {}: {error}",
            path.display()
        )
    })?;
    let sha256 = hex_encode(&Sha256::digest(&bytes));

    Ok(LocalFlashgrepBundle {
        binary_name,
        path,
        bytes,
        sha256,
    })
}

pub(crate) fn remote_stdio_search_mode(output_mode: ContentSearchOutputMode) -> SearchModeConfig {
    match output_mode {
        ContentSearchOutputMode::Content => SearchModeConfig::LineMatches,
        ContentSearchOutputMode::Count => SearchModeConfig::CountOnly,
        ContentSearchOutputMode::FilesWithMatches => SearchModeConfig::FilesWithMatches,
    }
}

pub(crate) fn should_retry_remote_scan_fallback_as_files_with_matches(
    backend: SearchBackend,
    primary_search_mode: SearchModeConfig,
    search_results: &SearchResults,
) -> bool {
    let primary_has_details = !search_results.hits.is_empty()
        || !search_results.file_counts.is_empty()
        || !search_results.file_match_counts.is_empty()
        || !search_results.matched_paths.is_empty();
    matches!(backend, SearchBackend::ScanFallback)
        && !primary_has_details
        && search_results.matched_lines > 0
        && !matches!(primary_search_mode, SearchModeConfig::FilesWithMatches)
}

pub(crate) fn join_remote_path(base: &str, child: &str) -> String {
    let base = normalize_remote_workspace_path(base);
    let child = child.trim_start_matches('/');
    if base == "/" {
        format!("/{child}")
    } else {
        format!("{base}/{child}")
    }
}

pub(crate) fn shell_escape(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '-' | '_' | ':' | '='))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn resolve_local_flashgrep_bundle(binary_name: &str) -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.join("../../../..");
    let mut candidates = vec![workspace_root.join("resources/flashgrep").join(binary_name)];

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            candidates.push(parent.join("resources/flashgrep").join(binary_name));
            candidates.push(parent.join("flashgrep").join(binary_name));
            candidates.push(parent.join("../Resources/flashgrep").join(binary_name));
            candidates.push(parent.join("../share/bitfun/flashgrep").join(binary_name));
            candidates.push(
                parent
                    .join("../share/com.bitfun.desktop/flashgrep")
                    .join(binary_name),
            );
        }
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.exists())
        .map(|candidate| candidate.canonicalize().unwrap_or(candidate))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_search::flashgrep::{
        FileCount, SearchBackend, SearchHit, SearchModeConfig, SearchResults,
    };
    use std::path::{Path, PathBuf};

    #[test]
    fn remote_workspace_search_paths_preserve_current_contract() {
        assert_eq!(
            remote_flashgrep_install_dir("/home/wgq/workspace/bot_detection"),
            "/home/wgq/workspace/bot_detection/.bitfun/bin"
        );
        assert_eq!(
            remote_workspace_search_storage_root("/home/wgq/workspace/bot_detection/"),
            "/home/wgq/workspace/bot_detection/.bitfun/search/flashgrep-index"
        );
        assert_eq!(join_remote_path("/", "tmp/file.txt"), "/tmp/file.txt");
        assert_eq!(
            join_remote_path("/home/user/repo/", "/src/lib.rs"),
            "/home/user/repo/src/lib.rs"
        );
        assert_eq!(
            shell_escape("/home/user/repo/file.txt"),
            "/home/user/repo/file.txt"
        );
        assert_eq!(
            shell_escape("/home/user/my repo/file.txt"),
            "'/home/user/my repo/file.txt'"
        );
    }

    #[test]
    fn remote_workspace_search_probe_parsers_preserve_current_contract() {
        assert_eq!(
            parse_remote_architecture_output("x86_64\n", ""),
            Some("x86_64".to_string())
        );
        assert_eq!(
            parse_remote_architecture_output("Welcome\nArchitecture: amd64\n", ""),
            Some("x86_64".to_string())
        );
        assert_eq!(
            parse_remote_architecture_output("", "machine: arm64\n"),
            Some("aarch64".to_string())
        );
        assert_eq!(
            parse_remote_os_output("Linux\n", ""),
            Some("Linux".to_string())
        );
        assert_eq!(
            parse_remote_os_output("Darwin Kernel Version\n", ""),
            Some("Darwin".to_string())
        );
        assert_eq!(
            parse_remote_os_output("Welcome\nOperating system: linux\n", ""),
            Some("Linux".to_string())
        );
        assert!(looks_like_linux_workspace_root(
            "/home/wgq/workspace/bot_detection"
        ));
        assert!(!looks_like_linux_workspace_root(
            "C:/Users/wgq/workspace/bot_detection"
        ));
    }

    #[tokio::test]
    async fn remote_workspace_search_bundle_rejects_unsupported_linux_arch() {
        let err = match local_flashgrep_bundle_for_arch("riscv64").await {
            Ok(_) => panic!("unsupported architecture should fail before bundle lookup"),
            Err(err) => err,
        };
        assert!(err.contains("does not support Linux architecture"));
    }

    #[test]
    fn remote_workspace_search_scope_preserves_current_contract() {
        let scope = build_remote_scope(
            "/home/user/repo/",
            Some(Path::new("src")),
            vec!["*.rs".to_string()],
            vec!["rust".to_string()],
            vec!["lock".to_string()],
        )
        .unwrap();
        assert_eq!(scope.roots, vec![PathBuf::from("/home/user/repo/src")]);
        assert_eq!(scope.globs, vec!["*.rs".to_string()]);
        assert_eq!(scope.types, vec!["rust".to_string()]);
        assert_eq!(scope.type_not, vec!["lock".to_string()]);

        let workspace_root_scope = build_remote_scope(
            "/home/user/repo",
            Some(Path::new("/home/user/repo")),
            vec![],
            vec![],
            vec![],
        )
        .unwrap();
        assert!(workspace_root_scope.roots.is_empty());

        let err = build_remote_scope(
            "/home/user/repo",
            Some(Path::new("/home/user/other")),
            vec![],
            vec![],
            vec![],
        )
        .unwrap_err();
        assert!(err.contains("outside workspace root"));
    }

    #[test]
    fn remote_workspace_search_mode_preserves_current_contract() {
        assert_eq!(
            remote_stdio_search_mode(ContentSearchOutputMode::Content),
            SearchModeConfig::LineMatches
        );
        assert_eq!(
            remote_stdio_search_mode(ContentSearchOutputMode::Count),
            SearchModeConfig::CountOnly
        );
        assert_eq!(
            remote_stdio_search_mode(ContentSearchOutputMode::FilesWithMatches),
            SearchModeConfig::FilesWithMatches
        );
    }

    #[test]
    fn remote_scan_fallback_retry_policy_preserves_current_contract() {
        let summary_only = search_results_with_counts(7, 12);

        assert!(should_retry_remote_scan_fallback_as_files_with_matches(
            SearchBackend::ScanFallback,
            SearchModeConfig::LineMatches,
            &summary_only,
        ));
        assert!(should_retry_remote_scan_fallback_as_files_with_matches(
            SearchBackend::ScanFallback,
            SearchModeConfig::CountOnly,
            &summary_only,
        ));

        let mut with_paths = search_results_with_counts(7, 12);
        with_paths
            .matched_paths
            .push("/repo/src/lib.rs".to_string());
        assert!(!should_retry_remote_scan_fallback_as_files_with_matches(
            SearchBackend::ScanFallback,
            SearchModeConfig::LineMatches,
            &with_paths,
        ));

        let mut with_file_counts = search_results_with_counts(7, 12);
        with_file_counts.file_counts.push(FileCount {
            path: "/repo/src/lib.rs".to_string(),
            matched_lines: 7,
        });
        assert!(!should_retry_remote_scan_fallback_as_files_with_matches(
            SearchBackend::ScanFallback,
            SearchModeConfig::LineMatches,
            &with_file_counts,
        ));

        let mut with_hits = search_results_with_counts(7, 12);
        with_hits.hits.push(SearchHit {
            path: "/repo/src/lib.rs".to_string(),
            matches: Vec::new(),
            lines: Vec::new(),
        });
        assert!(!should_retry_remote_scan_fallback_as_files_with_matches(
            SearchBackend::ScanFallback,
            SearchModeConfig::LineMatches,
            &with_hits,
        ));
        assert!(!should_retry_remote_scan_fallback_as_files_with_matches(
            SearchBackend::IndexedClean,
            SearchModeConfig::LineMatches,
            &summary_only,
        ));
        assert!(!should_retry_remote_scan_fallback_as_files_with_matches(
            SearchBackend::ScanFallback,
            SearchModeConfig::FilesWithMatches,
            &summary_only,
        ));
        assert!(!should_retry_remote_scan_fallback_as_files_with_matches(
            SearchBackend::ScanFallback,
            SearchModeConfig::LineMatches,
            &search_results_with_counts(0, 0),
        ));
    }

    fn search_results_with_counts(
        matched_lines: usize,
        matched_occurrences: usize,
    ) -> SearchResults {
        SearchResults {
            candidate_docs: 0,
            searches_with_match: 0,
            bytes_searched: 0,
            matched_lines,
            matched_occurrences,
            matched_paths: Vec::new(),
            file_counts: Vec::new(),
            file_match_counts: Vec::new(),
            line_matches: Vec::new(),
            hits: Vec::new(),
        }
    }

    #[test]
    fn preserves_supported_linux_flashgrep_bundle_order() {
        assert_eq!(
            linux_flashgrep_bundle_names_for_arch("amd64").unwrap(),
            &[
                "flashgrep-x86_64-unknown-linux-musl",
                "flashgrep-x86_64-unknown-linux-gnu"
            ]
        );
        assert_eq!(
            linux_flashgrep_bundle_names_for_arch("arm64").unwrap(),
            &[
                "flashgrep-aarch64-unknown-linux-musl",
                "flashgrep-aarch64-unknown-linux-gnu"
            ]
        );
    }
}
