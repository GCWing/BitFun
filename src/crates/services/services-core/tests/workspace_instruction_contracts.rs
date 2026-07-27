#![cfg(feature = "workspace-runtime")]

use bitfun_services_core::workspace::LocalWorkspaceFs;
use bitfun_services_core::workspace_instructions::read_workspace_instruction_files_with_fs;
use std::fs;

#[tokio::test]
async fn port_backed_instructions_honor_agents_override_and_keep_claude_context() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::write(temp.path().join("AGENTS.override.md"), "override rules\n").expect("override");
    fs::write(temp.path().join("AGENTS.md"), "base rules\n").expect("agents");
    fs::write(temp.path().join("CLAUDE.md"), "claude rules\n").expect("claude");
    let root = temp.path().to_string_lossy();

    let files = read_workspace_instruction_files_with_fs(&LocalWorkspaceFs, &root)
        .await
        .expect("instruction files");

    assert_eq!(files.len(), 2);
    assert_eq!(files[0].name, "AGENTS.override.md");
    assert_eq!(files[0].content, "override rules\n");
    assert_eq!(files[1].name, "CLAUDE.md");
    assert_eq!(files[1].content, "claude rules\n");

    fs::write(temp.path().join("AGENTS.override.md"), "").expect("empty override");
    let files = read_workspace_instruction_files_with_fs(&LocalWorkspaceFs, &root)
        .await
        .expect("empty override selection");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "CLAUDE.md");
}
