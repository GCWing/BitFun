use bitfun_services_core::workspace_instructions::read_workspace_instruction_files;
use std::fs;

fn instruction_names(
    files: &[bitfun_services_core::workspace_instructions::WorkspaceInstructionFile],
) -> Vec<&str> {
    files.iter().map(|file| file.name.as_str()).collect()
}

#[tokio::test]
async fn claude_project_files_and_unconditional_rules_have_deterministic_order() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::create_dir_all(temp.path().join(".claude/rules/nested")).expect("rules dir");
    fs::write(temp.path().join("AGENTS.md"), "bitfun rules\n").expect("agents");
    fs::write(temp.path().join("CLAUDE.md"), "root claude\n").expect("root claude");
    fs::write(temp.path().join(".claude/CLAUDE.md"), "fallback claude\n").expect("fallback claude");
    fs::write(temp.path().join("CLAUDE.local.md"), "local claude\n").expect("local claude");
    fs::write(temp.path().join(".claude/rules/z-last.md"), "last rule\n").expect("last rule");
    fs::write(
        temp.path().join(".claude/rules/nested/a-first.md"),
        "first rule\n",
    )
    .expect("first rule");
    fs::write(
        temp.path().join(".claude/rules/path-scoped.md"),
        "---\npaths:\n  - src/**/*.rs\n---\nconditional rule\n",
    )
    .expect("path scoped rule");

    let files = read_workspace_instruction_files(temp.path())
        .await
        .expect("instructions");

    assert_eq!(
        instruction_names(&files),
        vec![
            "AGENTS.md",
            "CLAUDE.md",
            "CLAUDE.local.md",
            ".claude/rules/nested/a-first.md",
            ".claude/rules/z-last.md",
        ]
    );
    assert_eq!(files[1].content, "root claude\n");
    assert!(!files.iter().any(|file| file.name == ".claude/CLAUDE.md"));
    assert!(!files.iter().any(|file| file.name.contains("path-scoped")));
}

#[tokio::test]
async fn claude_internal_imports_are_depth_first_deduplicated_and_workspace_contained() {
    let temp = tempfile::tempdir().expect("tempdir");
    let outside = tempfile::tempdir().expect("outside tempdir");
    fs::create_dir_all(temp.path().join("docs")).expect("docs dir");
    fs::write(outside.path().join("outside.md"), "must not load\n").expect("outside");
    fs::write(
        temp.path().join("CLAUDE.md"),
        "Project rules.\n@docs/base.md\n@../outside.md\n@~/private.md\n",
    )
    .expect("claude");
    fs::write(
        temp.path().join("docs/base.md"),
        "Base rules.\n@nested.md\n@nested.md\n",
    )
    .expect("base");
    fs::write(
        temp.path().join("docs/nested.md"),
        "Nested rules.\n@../CLAUDE.md\n",
    )
    .expect("nested");

    let files = read_workspace_instruction_files(temp.path())
        .await
        .expect("instructions");

    assert_eq!(
        instruction_names(&files),
        vec!["CLAUDE.md", "docs/base.md", "docs/nested.md"]
    );
    assert_eq!(
        files
            .iter()
            .filter(|file| file.name == "docs/nested.md")
            .count(),
        1
    );
}

#[tokio::test]
async fn opencode_project_instructions_support_local_files_and_globs_only() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::create_dir_all(temp.path().join("docs/nested")).expect("docs dir");
    fs::create_dir_all(temp.path().join("more/nested")).expect("more dir");
    fs::create_dir_all(temp.path().join(".opencode")).expect("opencode dir");
    fs::write(temp.path().join("docs/a.md"), "a\n").expect("a");
    fs::write(temp.path().join("docs/b.md"), "b\n").expect("b");
    fs::write(temp.path().join("docs/nested/c.md"), "c\n").expect("c");
    fs::write(temp.path().join("exact.txt"), "exact\n").expect("exact");
    fs::write(temp.path().join("more/nested/rule.md"), "nested rule\n").expect("nested rule");
    fs::write(
        temp.path().join("opencode.jsonc"),
        r#"{
          // Project-local declarative context only.
          "instructions": [
            "docs/*.md",
            "exact.txt",
            "https://example.invalid/rules.md",
            "../outside.md",
            "docs/a.md",
          ],
        }"#,
    )
    .expect("root config");
    fs::write(
        temp.path().join(".opencode/opencode.json"),
        r#"{"instructions":["more/**/*.md","exact.txt"]}"#,
    )
    .expect("directory config");

    let files = read_workspace_instruction_files(temp.path())
        .await
        .expect("instructions");

    assert_eq!(
        instruction_names(&files),
        vec!["docs/a.md", "docs/b.md", "exact.txt", "more/nested/rule.md",]
    );
    assert!(!files.iter().any(|file| file.name.contains("outside")));
    assert!(!files.iter().any(|file| file.name.starts_with("http")));
}

#[tokio::test]
async fn broad_opencode_globs_skip_vcs_dependency_and_build_directories() {
    let temp = tempfile::tempdir().expect("tempdir");
    for directory in ["docs", ".git", "node_modules/pkg", "target/debug"] {
        fs::create_dir_all(temp.path().join(directory)).expect("instruction directory");
    }
    fs::write(temp.path().join("docs/visible.md"), "visible\n").expect("visible");
    fs::write(temp.path().join(".git/private.md"), "private\n").expect("git file");
    fs::write(
        temp.path().join("node_modules/pkg/dependency.md"),
        "dependency\n",
    )
    .expect("dependency file");
    fs::write(temp.path().join("target/debug/output.md"), "output\n").expect("build file");
    fs::write(
        temp.path().join("opencode.json"),
        r#"{"instructions":["**/*.md"]}"#,
    )
    .expect("config");

    let files = read_workspace_instruction_files(temp.path())
        .await
        .expect("instructions");

    assert_eq!(instruction_names(&files), vec!["docs/visible.md"]);
}

#[tokio::test]
async fn declarative_instruction_expansion_has_a_file_budget() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::create_dir_all(temp.path().join("docs")).expect("docs");
    for index in 0..260 {
        fs::write(
            temp.path().join(format!("docs/{index:03}.md")),
            format!("rule {index}\n"),
        )
        .expect("instruction file");
    }
    fs::write(
        temp.path().join("opencode.json"),
        r#"{"instructions":["docs/*.md"]}"#,
    )
    .expect("config");

    let files = read_workspace_instruction_files(temp.path())
        .await
        .expect("instructions");

    assert_eq!(files.len(), 256);
}

#[tokio::test]
async fn path_scoped_claude_rules_consume_the_shared_read_budget() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::create_dir_all(temp.path().join(".claude/rules")).expect("rules");
    for index in 0..260 {
        fs::write(
            temp.path().join(format!(".claude/rules/{index:03}.md")),
            "---\npaths:\n  - src/**/*.rs\n---\nconditional\n",
        )
        .expect("path-scoped rule");
    }
    fs::write(
        temp.path().join("exact.md"),
        "must stay outside the budget\n",
    )
    .expect("exact");
    fs::write(
        temp.path().join("opencode.json"),
        r#"{"instructions":["exact.md"]}"#,
    )
    .expect("config");

    let files = read_workspace_instruction_files(temp.path())
        .await
        .expect("instructions");

    assert!(files.is_empty());
}

#[tokio::test]
async fn oversized_declarative_instruction_files_are_ignored() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::write(
        temp.path().join("oversized.md"),
        vec![b'x'; 1024 * 1024 + 1],
    )
    .expect("oversized file");
    fs::write(
        temp.path().join("opencode.json"),
        r#"{"instructions":["oversized.md"]}"#,
    )
    .expect("config");

    let files = read_workspace_instruction_files(temp.path())
        .await
        .expect("instructions");

    assert!(!files.iter().any(|file| file.name == "oversized.md"));
}
