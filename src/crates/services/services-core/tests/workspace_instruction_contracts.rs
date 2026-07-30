#![cfg(feature = "workspace-runtime")]

use bitfun_services_core::workspace::LocalWorkspaceFs;
use bitfun_services_core::workspace_instructions::{
    read_workspace_instruction_files, read_workspace_instruction_files_with_fs,
};
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

#[tokio::test]
async fn port_backed_and_local_instruction_resolution_have_identical_order_and_content() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::create_dir_all(temp.path().join(".claude/rules")).expect("rules dir");
    fs::create_dir_all(temp.path().join("docs")).expect("docs dir");
    fs::write(temp.path().join("AGENTS.md"), "agents\n").expect("agents");
    fs::write(temp.path().join("CLAUDE.md"), "claude\n@docs/imported.md\n").expect("claude");
    fs::write(temp.path().join("docs/imported.md"), "imported\n").expect("imported");
    fs::write(temp.path().join(".claude/rules/base.md"), "base rule\n").expect("rule");
    fs::write(
        temp.path().join("opencode.json"),
        r#"{"instructions":["docs/*.md"]}"#,
    )
    .expect("opencode config");
    let root = temp.path().to_string_lossy();

    let local = read_workspace_instruction_files(temp.path())
        .await
        .expect("local instructions");
    let port = read_workspace_instruction_files_with_fs(&LocalWorkspaceFs, &root)
        .await
        .expect("port instructions");

    assert_eq!(port, local);
}

#[cfg(unix)]
#[tokio::test]
async fn exact_instruction_paths_do_not_follow_symlinked_parent_directories() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().expect("tempdir");
    let outside = tempfile::tempdir().expect("outside tempdir");
    fs::write(outside.path().join("private.md"), "outside\n").expect("outside file");
    symlink(outside.path(), temp.path().join("linked")).expect("directory symlink");
    fs::write(
        temp.path().join("opencode.json"),
        r#"{"instructions":["linked/private.md"]}"#,
    )
    .expect("opencode config");
    let root = temp.path().to_string_lossy();

    let local = read_workspace_instruction_files(temp.path())
        .await
        .expect("local instructions");
    let port = read_workspace_instruction_files_with_fs(&LocalWorkspaceFs, &root)
        .await
        .expect("port instructions");

    assert!(local.is_empty());
    assert!(port.is_empty());
}
