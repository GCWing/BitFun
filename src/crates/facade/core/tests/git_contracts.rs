use bitfun_core::service::git::{
    build_git_changed_files_args, build_git_diff_args, parse_branch_line, parse_git_log_line,
    GitChangedFileStatus, GitChangedFilesParams, GitCommandOutput, GitCommitParams, GitDiffParams,
    GitGraph, GitService, GitWorktreeInfo, GraphNode, GraphRef,
};

#[test]
fn git_contracts_remain_available_from_core_facade() {
    let status = serde_json::to_value(GitChangedFileStatus::Renamed).unwrap();
    assert_eq!(status, serde_json::json!("renamed"));

    let worktree = GitWorktreeInfo {
        path: "D:/workspace/BitFun-worktree".to_string(),
        branch: Some("feature/test".to_string()),
        head: "abc123".to_string(),
        is_main: false,
        is_locked: true,
        is_prunable: false,
    };
    let worktree_value = serde_json::to_value(worktree).unwrap();
    assert_eq!(worktree_value["isMain"], false);

    let commit_params = GitCommitParams {
        message: "test commit".to_string(),
        amend: Some(false),
        all: Some(true),
        no_verify: Some(true),
        author: None,
    };
    let commit_value = serde_json::to_value(commit_params).unwrap();
    assert_eq!(commit_value["noVerify"], true);
    assert!(commit_value.get("no_verify").is_none());

    let command_output = GitCommandOutput {
        stdout: "ok".to_string(),
        stderr: "warning".to_string(),
        exit_code: 1,
    };
    assert_eq!(command_output.exit_code, 1);

    assert_eq!(
        parse_git_log_line("abc123|BitFun|bitfun@example.com|2026-05-12|subject"),
        Some((
            "abc123".to_string(),
            "BitFun".to_string(),
            "bitfun@example.com".to_string(),
            "2026-05-12".to_string(),
            "subject".to_string(),
        ))
    );
    assert_eq!(
        parse_branch_line("* main"),
        Some(("main".to_string(), true))
    );
    assert_eq!(
        build_git_diff_args(&GitDiffParams {
            source: Some("main".to_string()),
            target: Some("feature".to_string()),
            files: None,
            staged: Some(false),
            stat: Some(true),
        }),
        vec!["diff", "main..feature", "--stat"]
    );
    assert_eq!(
        build_git_changed_files_args(&GitChangedFilesParams {
            source: None,
            target: Some("feature".to_string()),
            staged: Some(false),
        }),
        vec!["diff", "--name-status", "feature"]
    );
    let _service_size = std::mem::size_of::<GitService>();

    let graph = GitGraph {
        nodes: vec![GraphNode {
            hash: "abc123".to_string(),
            message: "initial".to_string(),
            full_message: "initial commit".to_string(),
            author_name: "BitFun".to_string(),
            author_email: "bitfun@example.com".to_string(),
            timestamp: 1_700_000_000,
            parents: Vec::new(),
            children: Vec::new(),
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
    let graph_value = serde_json::to_value(graph).unwrap();
    assert_eq!(graph_value["maxLane"], 1);
    assert_eq!(graph_value["nodes"][0]["refs"][0]["isHead"], true);
}
