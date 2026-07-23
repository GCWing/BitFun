#![cfg(feature = "git")]

use bitfun_services_integrations::git::{
    build_git_changed_files_args, build_git_diff_args, parse_branch_line, parse_git_log_line,
    parse_name_status_output, parse_worktree_list, GitAuthor, GitChangedFile, GitChangedFileStatus,
    GitChangedFilesParams, GitCommandOutput, GitCommitParams, GitDiffParams, GitError, GitGraph,
    GitService, GitStatus, GitWorktreeInfo, GraphNode, GraphRef,
};
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn git_changed_file_status_preserves_snake_case_contract() {
    let status = serde_json::to_value(GitChangedFileStatus::Renamed).unwrap();
    assert_eq!(status, serde_json::json!("renamed"));

    let changed_file = GitChangedFile {
        path: "src/new.rs".to_string(),
        old_path: Some("src/old.rs".to_string()),
        status: GitChangedFileStatus::Renamed,
    };

    let value = serde_json::to_value(changed_file).unwrap();
    assert_eq!(value["old_path"], "src/old.rs");
    assert_eq!(value["status"], "renamed");
}

#[test]
fn git_name_status_parser_preserves_common_status_contract() {
    let files = parse_name_status_output(
        "M\tsrc/main.rs\nA\tsrc/new.rs\nD\tsrc/old.rs\nR100\tsrc/old_name.rs\tsrc/new_name.rs\nC087\tsrc/source.rs\tsrc/copy.rs\n",
    );

    assert_eq!(
        files,
        vec![
            GitChangedFile {
                path: "src/main.rs".to_string(),
                old_path: None,
                status: GitChangedFileStatus::Modified,
            },
            GitChangedFile {
                path: "src/new.rs".to_string(),
                old_path: None,
                status: GitChangedFileStatus::Added,
            },
            GitChangedFile {
                path: "src/old.rs".to_string(),
                old_path: None,
                status: GitChangedFileStatus::Deleted,
            },
            GitChangedFile {
                path: "src/new_name.rs".to_string(),
                old_path: Some("src/old_name.rs".to_string()),
                status: GitChangedFileStatus::Renamed,
            },
            GitChangedFile {
                path: "src/copy.rs".to_string(),
                old_path: Some("src/source.rs".to_string()),
                status: GitChangedFileStatus::Copied,
            },
        ],
    );
}

#[test]
fn git_command_output_preserves_raw_stream_contract() {
    let output = GitCommandOutput {
        stdout: "ok".to_string(),
        stderr: "warning".to_string(),
        exit_code: 1,
    };

    assert_eq!(output.stdout, "ok");
    assert_eq!(output.stderr, "warning");
    assert_eq!(output.exit_code, 1);
}

#[test]
fn git_text_parsers_preserve_branch_and_log_contracts() {
    assert_eq!(
        parse_git_log_line("abc123|BitFun|bitfun@example.com|2026-05-12|subject|body"),
        Some((
            "abc123".to_string(),
            "BitFun".to_string(),
            "bitfun@example.com".to_string(),
            "2026-05-12".to_string(),
            "subject|body".to_string(),
        ))
    );
    assert_eq!(parse_git_log_line("abc123|missing"), None);

    assert_eq!(
        parse_branch_line("* main"),
        Some(("main".to_string(), true))
    );
    assert_eq!(
        parse_branch_line("  feature/test"),
        Some(("feature/test".to_string(), false))
    );
    assert_eq!(
        parse_branch_line("detached"),
        Some(("detached".to_string(), false))
    );
    assert_eq!(parse_branch_line("  "), None);
}

#[test]
fn git_diff_arg_builders_preserve_existing_command_contract() {
    let args = build_git_diff_args(&GitDiffParams {
        source: Some("main".to_string()),
        target: Some("feature".to_string()),
        files: Some(vec!["src/lib.rs".to_string(), "README.md".to_string()]),
        staged: Some(true),
        stat: Some(true),
        review_safe: Some(true),
    });
    assert_eq!(
        args,
        vec![
            "--literal-pathspecs",
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--find-renames",
            "--cached",
            "main..feature",
            "--stat",
            "--",
            "src/lib.rs",
            "README.md",
        ]
    );

    let target_only_args = build_git_diff_args(&GitDiffParams {
        source: None,
        target: Some("feature".to_string()),
        files: None,
        staged: None,
        stat: None,
        review_safe: None,
    });
    assert_eq!(target_only_args, vec!["diff"]);

    let changed_args = build_git_changed_files_args(&GitChangedFilesParams {
        source: None,
        target: Some("feature".to_string()),
        staged: Some(true),
        review_safe: Some(true),
    });
    assert_eq!(
        changed_args,
        vec![
            "--literal-pathspecs",
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--find-renames",
            "--name-status",
            "-z",
            "--cached",
            "feature",
        ]
    );
}

#[tokio::test]
async fn review_safe_workspace_diff_forces_rename_detection() {
    let repo_dir = TempRepoDir::new("review-workspace-rename");
    run_git(repo_dir.path(), &["init"]);
    run_git(repo_dir.path(), &["config", "user.name", "BitFun Tests"]);
    run_git(
        repo_dir.path(),
        &["config", "user.email", "tests@bitfun.dev"],
    );
    let original = (0..10)
        .map(|index| format!("stable line {index}\n"))
        .collect::<String>();
    fs::write(repo_dir.path().join("old.txt"), &original).unwrap();
    run_git(repo_dir.path(), &["add", "--", "old.txt"]);
    run_git(repo_dir.path(), &["commit", "-m", "base"]);
    run_git(repo_dir.path(), &["config", "diff.renames", "false"]);
    run_git(repo_dir.path(), &["mv", "old.txt", "new.txt"]);
    fs::write(
        repo_dir.path().join("new.txt"),
        original.replace("stable line 5", "edited line 5"),
    )
    .unwrap();

    let files = GitService::get_changed_files(
        repo_dir.path(),
        &GitChangedFilesParams {
            source: Some("HEAD".to_string()),
            review_safe: Some(true),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].status, GitChangedFileStatus::Renamed);
    assert_eq!(files[0].old_path.as_deref(), Some("old.txt"));
    assert_eq!(files[0].path, "new.txt");

    let diff = GitService::get_diff(
        repo_dir.path(),
        &GitDiffParams {
            source: Some("HEAD".to_string()),
            files: Some(vec!["old.txt".to_string(), "new.txt".to_string()]),
            review_safe: Some(true),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(diff.contains("rename from old.txt"));
    assert!(diff.contains("rename to new.txt"));
}

#[tokio::test]
async fn git_service_preserves_repository_status_contract() {
    let repo_dir = TempRepoDir::new("git-service-status");
    assert!(!GitService::is_repository(repo_dir.path()).await.unwrap());

    run_git(repo_dir.path(), &["init"]);
    fs::write(repo_dir.path().join("new-file.txt"), "hello\n").unwrap();

    assert!(GitService::is_repository(repo_dir.path()).await.unwrap());

    let status = GitService::get_status(repo_dir.path()).await.unwrap();
    assert!(status
        .untracked
        .iter()
        .any(|path| path == "new-file.txt" || path == "new-file.txt/"));
}

#[test]
fn git_name_status_parser_preserves_nul_delimited_unicode_and_rename_paths() {
    let files = parse_name_status_output("M\0src/中文.rs\0R100\0old name.rs\0new name.rs\0");
    assert_eq!(
        files,
        vec![
            GitChangedFile {
                path: "src/中文.rs".to_string(),
                old_path: None,
                status: GitChangedFileStatus::Modified,
            },
            GitChangedFile {
                path: "new name.rs".to_string(),
                old_path: Some("old name.rs".to_string()),
                status: GitChangedFileStatus::Renamed,
            },
        ]
    );
}

#[test]
fn git_status_json_preserves_conflict_contract() {
    let status = GitStatus {
        staged: Vec::new(),
        unstaged: Vec::new(),
        untracked: Vec::new(),
        conflicts: vec!["src/conflicted.rs".to_string()],
        current_branch: "main".to_string(),
        ahead: 0,
        behind: 0,
    };
    assert_eq!(
        serde_json::to_value(status).unwrap()["conflicts"],
        serde_json::json!(["src/conflicted.rs"])
    );
}

#[tokio::test]
async fn review_git_service_reads_exact_renamed_and_deleted_range_without_mutation() {
    let repo_dir = TempRepoDir::new("review-exact-range");
    run_git(repo_dir.path(), &["init"]);
    run_git(repo_dir.path(), &["config", "user.name", "BitFun Tests"]);
    run_git(
        repo_dir.path(),
        &["config", "user.email", "bitfun-tests@example.com"],
    );
    fs::write(
        repo_dir.path().join(".gitattributes"),
        "*.txt diff=reviewdriver\n",
    )
    .unwrap();
    fs::write(repo_dir.path().join("renamed.txt"), "before rename\n").unwrap();
    fs::write(repo_dir.path().join("deleted.txt"), "deleted content\n").unwrap();
    fs::write(
        repo_dir.path().join(" leading whitespace.txt"),
        "before whitespace\n",
    )
    .unwrap();
    run_git(repo_dir.path(), &["add", "."]);
    run_git(repo_dir.path(), &["commit", "-m", "base"]);
    let base = GitService::resolve_revision(repo_dir.path(), "HEAD")
        .await
        .unwrap();

    run_git(repo_dir.path(), &["mv", "renamed.txt", "moved.txt"]);
    run_git(repo_dir.path(), &["rm", "deleted.txt"]);
    fs::write(repo_dir.path().join("moved.txt"), "after rename\n").unwrap();
    fs::write(
        repo_dir.path().join(" leading whitespace.txt"),
        "after whitespace\n",
    )
    .unwrap();
    run_git(repo_dir.path(), &["add", "."]);
    run_git(repo_dir.path(), &["commit", "-m", "target"]);
    let head = GitService::resolve_revision(repo_dir.path(), "HEAD")
        .await
        .unwrap();

    run_git(
        repo_dir.path(),
        &["config", "diff.external", "bitfun-review-should-not-run"],
    );
    run_git(
        repo_dir.path(),
        &[
            "config",
            "diff.reviewdriver.textconv",
            "bitfun-review-should-not-run",
        ],
    );

    let diff = GitService::get_review_diff(
        repo_dir.path(),
        &base,
        &head,
        &[
            "renamed.txt".to_string(),
            "moved.txt".to_string(),
            "deleted.txt".to_string(),
            " leading whitespace.txt".to_string(),
        ],
    )
    .await
    .unwrap();
    assert!(diff.contains("moved.txt"));
    assert!(diff.contains("deleted.txt"));
    assert!(diff.contains("after rename"));
    assert!(diff.contains("after whitespace"));

    let generic_diff = GitService::get_diff(
        repo_dir.path(),
        &GitDiffParams {
            source: Some(base.clone()),
            target: Some(head.clone()),
            files: Some(vec!["moved.txt".to_string()]),
            staged: None,
            stat: None,
            review_safe: Some(true),
        },
    )
    .await
    .expect("generic read-only diff must ignore external diff and textconv helpers");
    assert!(generic_diff.contains("after rename"));

    let wildcard_diff =
        GitService::get_review_diff(repo_dir.path(), &base, &head, &["*.txt".to_string()])
            .await
            .unwrap();
    assert!(wildcard_diff.is_empty());

    let status = GitService::get_status(repo_dir.path()).await.unwrap();
    assert!(status.staged.is_empty());
    assert!(status.unstaged.is_empty());
    assert!(status.untracked.is_empty());
    assert!(status.conflicts.is_empty());
}

#[tokio::test]
async fn review_git_service_rejects_mutable_revisions_and_escaping_paths() {
    let repo_dir = TempRepoDir::new("review-input-boundary");

    let mutable_revision = GitService::get_review_diff(
        repo_dir.path(),
        "main",
        "feature",
        &["src/lib.rs".to_string()],
    )
    .await;
    assert!(mutable_revision.is_err());

    let escaping_path = GitService::get_review_diff(
        repo_dir.path(),
        &"1".repeat(40),
        &"2".repeat(40),
        &["../outside.rs".to_string()],
    )
    .await;
    assert!(escaping_path.is_err());

    let nul_path = GitService::get_review_diff(
        repo_dir.path(),
        &"1".repeat(40),
        &"2".repeat(40),
        &["src/nul\0path.rs".to_string()],
    )
    .await;
    assert!(
        matches!(nul_path, Err(GitError::InvalidPath(message)) if message.contains("workspace-relative"))
    );
}

struct TempRepoDir {
    path: std::path::PathBuf,
}

impl TempRepoDir {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "bitfun-services-integrations-{}-{}-{}",
            name,
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempRepoDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run_git(repo_dir: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(repo_dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn git_worktree_info_preserves_camel_case_contract() {
    let worktree = GitWorktreeInfo {
        path: "D:/workspace/BitFun-worktree".to_string(),
        branch: Some("feature/test".to_string()),
        head: "abc123".to_string(),
        is_main: false,
        is_locked: true,
        is_prunable: false,
    };

    let value = serde_json::to_value(worktree).unwrap();
    assert_eq!(value["isMain"], false);
    assert_eq!(value["isLocked"], true);
    assert_eq!(value["isPrunable"], false);
}

#[test]
fn git_worktree_parser_preserves_porcelain_contract() {
    let worktrees = parse_worktree_list(
        "worktree D:/workspace/BitFun\nHEAD abc123\nbranch refs/heads/main\n\nworktree D:/workspace/BitFun-feature\nHEAD def456\nbranch refs/heads/feature/test\nlocked\nprunable\n",
    );

    assert_eq!(worktrees.len(), 2);
    assert_eq!(worktrees[0].path, "D:/workspace/BitFun");
    assert_eq!(worktrees[0].branch.as_deref(), Some("main"));
    assert_eq!(worktrees[0].head, "abc123");
    assert!(worktrees[0].is_main);
    assert!(!worktrees[0].is_locked);
    assert_eq!(worktrees[1].branch.as_deref(), Some("feature/test"));
    assert!(worktrees[1].is_locked);
    assert!(worktrees[1].is_prunable);
}

#[test]
fn git_commit_params_preserves_no_verify_rename_contract() {
    let params = GitCommitParams {
        message: "test commit".to_string(),
        amend: Some(false),
        all: Some(true),
        no_verify: Some(true),
        author: Some(GitAuthor {
            name: "BitFun".to_string(),
            email: "bitfun@example.com".to_string(),
        }),
    };

    let value = serde_json::to_value(params).unwrap();
    assert_eq!(value["noVerify"], true);
    assert!(value.get("no_verify").is_none());
}

#[test]
fn git_graph_contract_preserves_camel_case_contract() {
    let graph = GitGraph {
        nodes: vec![GraphNode {
            hash: "abc123".to_string(),
            message: "initial".to_string(),
            full_message: "initial commit".to_string(),
            author_name: "BitFun".to_string(),
            author_email: "bitfun@example.com".to_string(),
            timestamp: 1_700_000_000,
            parents: Vec::new(),
            children: vec!["def456".to_string()],
            refs: vec![GraphRef {
                name: "main".to_string(),
                ref_type: "branch".to_string(),
                is_current: true,
                is_head: true,
            }],
            lane: 0,
            forking_lanes: Vec::new(),
            merging_lanes: Vec::new(),
            passing_lanes: Vec::new(),
        }],
        max_lane: 1,
        current_branch: Some("main".to_string()),
    };

    let value = serde_json::to_value(graph).unwrap();
    assert_eq!(value["maxLane"], 1);
    assert_eq!(value["currentBranch"], "main");
    assert_eq!(value["nodes"][0]["fullMessage"], "initial commit");
    assert_eq!(value["nodes"][0]["refs"][0]["refType"], "branch");
    assert_eq!(value["nodes"][0]["refs"][0]["isCurrent"], true);
}
