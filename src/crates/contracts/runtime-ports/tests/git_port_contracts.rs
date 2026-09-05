#![cfg(feature = "git-port")]

use openbitfun_runtime_ports::{
    WorkspaceDiffContent, WorkspaceDiffFile, WorkspaceDiffFileStatus, WorkspaceDiffSnapshot,
};

#[test]
fn workspace_diff_snapshot_round_trips_stable_file_facts() {
    let snapshot = WorkspaceDiffSnapshot {
        files: vec![WorkspaceDiffFile {
            path: "src/main.rs".to_string(),
            old_path: None,
            status: WorkspaceDiffFileStatus::Modified,
            staged: true,
            unstaged: true,
            untracked: false,
            additions: 3,
            deletions: 1,
            content: WorkspaceDiffContent::Text {
                patch: "@@ -1 +1 @@\n-old\n+new\n".to_string(),
            },
        }],
        truncated: false,
    };

    let encoded = serde_json::to_value(&snapshot).expect("serialize workspace diff");
    assert_eq!(encoded["files"][0]["status"], "modified");
    assert_eq!(encoded["files"][0]["content"]["kind"], "text");

    let decoded: WorkspaceDiffSnapshot =
        serde_json::from_value(encoded).expect("deserialize workspace diff");
    assert_eq!(decoded, snapshot);
}
