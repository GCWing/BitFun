use super::{
    changed_files, collect_worktree_patch, detect_verify_command, git_diff_base,
    needs_change_baseline, write_patch_to_path, ExecMode, TOOL_START_INPUT_PREVIEW_CHARS,
};
use serde_json::json;
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

fn git(workspace: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(workspace)
        .status()
        .expect("run git command");
    assert!(status.success(), "git {args:?} succeeds");
}

fn commit_fixture(workspace: &Path) {
    git(workspace, &["init", "-q"]);
    git(workspace, &["add", "."]);
    git(
        workspace,
        &[
            "-c",
            "user.name=BitFun Test",
            "-c",
            "user.email=bitfun-test@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ],
    );
}

#[test]
fn write_patch_to_path_creates_nested_parent_directories() {
    let temp = tempfile::tempdir().expect("tempdir");
    let patch_path = temp.path().join("parent/child/out.patch");
    write_patch_to_path(patch_path.to_str().expect("utf8 path"), "diff content")
        .expect("write patch");

    let written = std::fs::read_to_string(&patch_path).expect("read patch");
    assert_eq!(written, "diff content");
}

#[test]
fn verification_captures_a_change_baseline_without_patch_output() {
    assert!(needs_change_baseline(None, true));
    assert!(needs_change_baseline(Some("result.patch"), false));
    assert!(!needs_change_baseline(None, false));
}

#[test]
fn tool_input_preview_redacts_data_urls() {
    let preview = ExecMode::tool_input_preview(&json!({
        "image": {
            "data_url": "data:image/png;base64,abc",
            "name": "sample"
        }
    }));

    assert!(!preview.contains("data:image/png"));
    assert!(preview.contains("\"has_data_url\":true"));
    assert!(preview.contains("\"name\":\"sample\""));
}

#[test]
fn tool_input_preview_truncates_large_inputs() {
    let preview = ExecMode::tool_input_preview(&json!({
        "content": "x".repeat(TOOL_START_INPUT_PREVIEW_CHARS + 100)
    }));

    assert!(preview.ends_with("... [truncated]"));
    assert!(preview.len() < TOOL_START_INPUT_PREVIEW_CHARS + 100);
}

#[test]
fn automatic_verifier_ignores_project_wide_make_targets() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path();
    std::fs::write(workspace.join("go.mod"), "module example.com/fixture\n").expect("write go.mod");
    std::fs::create_dir_all(workspace.join("pkg")).expect("create package directory");
    let source = workspace.join("pkg/example.go");
    std::fs::write(&source, "package pkg\n\nfunc Value() int { return 1 }\n")
        .expect("write source");
    commit_fixture(workspace);
    std::fs::write(workspace.join("Makefile"), "test:\n\t@echo whole repo\n")
        .expect("write Makefile");
    std::fs::write(&source, "package pkg\n\nfunc Value() int { return 2 }\n")
        .expect("modify source");

    let command = detect_verify_command(workspace, Some("HEAD"), &BTreeSet::new());

    assert_eq!(
        command.as_deref(),
        Some("go vet -printf=false -composites=false -stdmethods=false './pkg'")
    );
}

#[test]
fn automatic_verifier_composes_go_and_typescript_checks() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path();
    std::fs::write(workspace.join("go.mod"), "module example.com/fixture\n").expect("write go.mod");
    std::fs::write(
        workspace.join("tsconfig.json"),
        r#"{"compilerOptions":{"noEmit":true},"include":["web"]}"#,
    )
    .expect("write tsconfig");
    std::fs::create_dir_all(workspace.join("pkg")).expect("create Go package");
    std::fs::create_dir_all(workspace.join("web")).expect("create web package");
    let go_source = workspace.join("pkg/example.go");
    let ts_source = workspace.join("web/example.ts");
    std::fs::write(&go_source, "package pkg\n\nfunc Value() int { return 1 }\n")
        .expect("write Go source");
    std::fs::write(&ts_source, "export const value: number = 1;\n")
        .expect("write TypeScript source");
    commit_fixture(workspace);
    std::fs::write(&go_source, "package pkg\n\nfunc Value() int { return 2 }\n")
        .expect("modify Go source");
    std::fs::write(&ts_source, "export const value: number = 2;\n")
        .expect("modify TypeScript source");

    let command =
        detect_verify_command(workspace, Some("HEAD"), &BTreeSet::new()).expect("mixed verifier");

    assert!(command.contains("go vet -printf=false -composites=false -stdmethods=false './pkg'"));
    assert!(command.contains("npx --no-install tsc --noEmit -p 'tsconfig.json'"));
}

#[test]
fn automatic_verifier_checks_rust_test_targets_through_nearest_manifest() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path();
    std::fs::write(
        workspace.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::create_dir_all(workspace.join("src")).expect("create src");
    std::fs::create_dir_all(workspace.join("tests")).expect("create tests");
    std::fs::write(
        workspace.join("src/lib.rs"),
        "pub fn value() -> i32 { 1 }\n",
    )
    .expect("write lib");
    let test_source = workspace.join("tests/integration.rs");
    std::fs::write(&test_source, "#[test]\nfn works() {}\n").expect("write test");
    commit_fixture(workspace);
    std::fs::write(&test_source, "#[test]\nfn still_works() {}\n").expect("modify test");

    let command =
        detect_verify_command(workspace, Some("HEAD"), &BTreeSet::new()).expect("Rust verifier");
    assert!(command
        .contains("cargo check --manifest-path 'Cargo.toml' -p 'fixture' --message-format=short"));
    assert!(command.contains(
        "cargo check --manifest-path 'Cargo.toml' -p 'fixture' --test 'integration' --message-format=short"
    ));
}

#[test]
fn automatic_verifier_keeps_deleted_go_file_in_package_scope() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path();
    std::fs::write(workspace.join("go.mod"), "module example.com/fixture\n").expect("write go.mod");
    std::fs::create_dir_all(workspace.join("pkg")).expect("create package");
    let source = workspace.join("pkg/example.go");
    std::fs::write(&source, "package pkg\n").expect("write source");
    std::fs::write(workspace.join("pkg/keeper.go"), "package pkg\n").expect("write keeper");
    commit_fixture(workspace);
    std::fs::remove_file(&source).expect("delete source");

    assert!(changed_files(workspace, Some("HEAD"), &BTreeSet::new())
        .contains(&"pkg/example.go".to_string()));
    assert_eq!(
        detect_verify_command(workspace, Some("HEAD"), &BTreeSet::new()).as_deref(),
        Some("go vet -printf=false -composites=false -stdmethods=false './pkg'")
    );
}

#[test]
fn automatic_verifier_skips_a_removed_go_package() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path();
    std::fs::write(workspace.join("go.mod"), "module example.com/fixture\n").expect("write go.mod");
    std::fs::create_dir_all(workspace.join("removed")).expect("create package");
    let source = workspace.join("removed/example.go");
    std::fs::write(&source, "package removed\n").expect("write source");
    commit_fixture(workspace);
    std::fs::remove_file(&source).expect("delete source");

    assert_eq!(
        detect_verify_command(workspace, Some("HEAD"), &BTreeSet::new()),
        None,
        "a deleted package must not produce a go vet target that no longer exists"
    );
}

#[test]
fn nested_rust_test_helpers_do_not_become_integration_targets() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path();
    std::fs::write(
        workspace.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::create_dir_all(workspace.join("src")).expect("create src");
    std::fs::create_dir_all(workspace.join("tests/support")).expect("create test support");
    std::fs::write(workspace.join("src/lib.rs"), "pub fn value() {}\n").expect("write lib");
    let helper = workspace.join("tests/support/mod.rs");
    std::fs::write(&helper, "pub fn setup() {}\n").expect("write helper");
    commit_fixture(workspace);
    std::fs::write(&helper, "pub fn setup() { println!(\"ready\"); }\n").expect("modify helper");

    assert_eq!(
        detect_verify_command(workspace, Some("HEAD"), &BTreeSet::new()).as_deref(),
        Some("cargo check --manifest-path 'Cargo.toml' -p 'fixture' --message-format=short")
    );
}

#[test]
fn nested_go_module_runs_from_its_owner_and_quotes_package_path() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path();
    let module = workspace.join("nested");
    let package = module.join("pkg with space");
    std::fs::create_dir_all(&package).expect("create nested package");
    std::fs::write(module.join("go.mod"), "module example.com/nested\n")
        .expect("write nested go.mod");
    let source = package.join("example.go");
    std::fs::write(&source, "package example\n\nconst Value = 1\n").expect("write source");
    commit_fixture(workspace);
    std::fs::write(&source, "package example\n\nconst Value = 2\n").expect("modify source");

    assert_eq!(
        detect_verify_command(workspace, Some("HEAD"), &BTreeSet::new()).as_deref(),
        Some(
            "(cd 'nested' && go vet -printf=false -composites=false -stdmethods=false './pkg with space')"
        )
    );
}

#[test]
fn nested_cargo_package_uses_its_own_manifest() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path();
    let package = workspace.join("tools/fixture");
    std::fs::create_dir_all(package.join("src")).expect("create nested crate");
    std::fs::write(
        package.join("Cargo.toml"),
        "[package]\nname = \"nested-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write nested manifest");
    let source = package.join("src/lib.rs");
    std::fs::write(&source, "pub const VALUE: i32 = 1;\n").expect("write source");
    commit_fixture(workspace);
    std::fs::write(&source, "pub const VALUE: i32 = 2;\n").expect("modify source");

    assert_eq!(
        detect_verify_command(workspace, Some("HEAD"), &BTreeSet::new()).as_deref(),
        Some(
            "cargo check --manifest-path 'tools/fixture/Cargo.toml' -p 'nested-fixture' --message-format=short"
        )
    );
}

#[test]
fn manifest_only_changes_still_select_a_scoped_verifier() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path();
    let go_module = workspace.join("go-service");
    let rust_package = workspace.join("rust-service");
    std::fs::create_dir_all(&go_module).expect("create Go module");
    std::fs::create_dir_all(rust_package.join("src")).expect("create Rust package");
    std::fs::write(go_module.join("go.mod"), "module example.com/service\n").expect("write go.mod");
    std::fs::write(
        rust_package.join("Cargo.toml"),
        "[package]\nname = \"rust-service\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(rust_package.join("src/lib.rs"), "pub fn value() {}\n")
        .expect("write Rust source");
    commit_fixture(workspace);
    std::fs::write(
        go_module.join("go.mod"),
        "module example.com/service\n\ngo 1.24\n",
    )
    .expect("modify go.mod");
    std::fs::write(
        rust_package.join("Cargo.toml"),
        "[package]\nname = \"rust-service\"\nversion = \"0.1.1\"\nedition = \"2021\"\n",
    )
    .expect("modify Cargo.toml");

    let command = detect_verify_command(workspace, Some("HEAD"), &BTreeSet::new())
        .expect("manifest verifiers");

    assert!(command.contains("(cd 'go-service' && go list -m all)"));
    assert!(command.contains(
        "cargo check --manifest-path 'rust-service/Cargo.toml' -p 'rust-service' --message-format=short"
    ));
}

#[test]
fn exported_patch_and_verifier_include_staged_and_untracked_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path();
    std::fs::write(workspace.join("tracked.py"), "value = 1\n").expect("write tracked");
    commit_fixture(workspace);
    std::fs::write(workspace.join("tracked.py"), "value = 2\n").expect("modify tracked");
    git(workspace, &["add", "tracked.py"]);
    std::fs::write(workspace.join("new.py"), "created = True\n").expect("write untracked");

    let changed = changed_files(workspace, Some("HEAD"), &BTreeSet::new());
    let patch =
        collect_worktree_patch(workspace, Some("HEAD"), &BTreeSet::new()).expect("collect patch");

    assert_eq!(
        changed,
        vec!["new.py".to_string(), "tracked.py".to_string()]
    );
    assert!(patch.contains("value = 2"));
    assert!(patch.contains("created = True"));
    assert!(patch.contains("new file mode"));
}

#[test]
fn exported_patch_uses_the_pre_exec_base_after_agent_commit() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path();
    git(workspace, &["init", "-q"]);
    let diff_base = git_diff_base(workspace).expect("empty-tree diff base");
    std::fs::write(workspace.join("committed.py"), "committed = True\n")
        .expect("write committed file");
    git(workspace, &["add", "committed.py"]);
    git(
        workspace,
        &[
            "-c",
            "user.name=BitFun Test",
            "-c",
            "user.email=bitfun-test@example.invalid",
            "commit",
            "-qm",
            "agent commit",
        ],
    );

    let changed = changed_files(workspace, Some(&diff_base), &BTreeSet::new());
    let patch = collect_worktree_patch(workspace, Some(&diff_base), &BTreeSet::new())
        .expect("collect committed patch");

    assert_eq!(changed, vec!["committed.py".to_string()]);
    assert!(patch.contains("committed = True"));
    assert!(patch.contains("new file mode"));
}

#[test]
fn patch_collection_excludes_files_untracked_before_exec_started() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path();
    std::fs::write(workspace.join("tracked.py"), "value = 1\n").expect("write tracked");
    commit_fixture(workspace);
    std::fs::write(workspace.join("existing-notes.py"), "private = True\n")
        .expect("write pre-existing untracked file");
    let initial_untracked = super::untracked_files(workspace);
    std::fs::write(workspace.join("created.py"), "created = True\n")
        .expect("write newly created file");

    let changed = changed_files(workspace, Some("HEAD"), &initial_untracked);
    let patch =
        collect_worktree_patch(workspace, Some("HEAD"), &initial_untracked).expect("collect patch");

    assert_eq!(changed, vec!["created.py".to_string()]);
    assert!(patch.contains("created = True"));
    assert!(!patch.contains("private = True"));
    assert!(!patch.contains("existing-notes.py"));
}
