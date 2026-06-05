use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tool_runtime::fs::read_file::{build_remote_read_command, parse_remote_read_output};
use tool_runtime::fs::{
    build_remote_delete_command, build_remote_list_commands, delete_local_path, edit_local_file,
    inspect_local_delete_target, parse_remote_list_entries, write_local_file,
    DeleteLocalPathRequest, EditLocalFileRequest, LocalDeleteTarget, WriteLocalFileRequest,
    WriteLocalFileStatus,
};
use tool_runtime::search::glob_search::{
    collect_remote_glob_matches, execute_local_glob, LocalGlobRequest,
};
use tool_runtime::search::grep_search::{
    apply_offset_and_limit, build_remote_grep_command, count_remote_grep_matches,
    relativize_result_text, render_remote_grep_result_text, OutputMode, RemoteGrepCommandRequest,
};
use tool_runtime::util::string::shell_single_quote;

fn make_temp_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("bitfun-tool-io-{name}-{unique}"));
    fs::create_dir_all(&dir).expect("temp dir should be created");
    dir
}

fn normalized(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[test]
fn write_local_file_reports_created_overwritten_and_identical_retry() {
    let root = make_temp_dir("write");
    let target = root.join("nested").join("file.txt");

    let created = write_local_file(WriteLocalFileRequest {
        logical_path: "nested/file.txt".to_string(),
        resolved_path: target.clone(),
        content: "hello\nworld\n".to_string(),
    })
    .expect("write should create file");

    assert_eq!(created.status, WriteLocalFileStatus::Created);
    assert_eq!(created.bytes_written, "hello\nworld\n".len());
    assert_eq!(created.lines_written, 2);
    assert_eq!(
        fs::read_to_string(&target).expect("file should exist"),
        "hello\nworld\n"
    );

    let identical = write_local_file(WriteLocalFileRequest {
        logical_path: "nested/file.txt".to_string(),
        resolved_path: target.clone(),
        content: "hello\nworld\n".to_string(),
    })
    .expect("identical retry should be successful and idempotent");

    assert_eq!(
        identical.status,
        WriteLocalFileStatus::AlreadyExistsSameContent
    );
    assert_eq!(identical.bytes_written, 0);
    assert_eq!(identical.lines_written, 0);

    let overwritten = write_local_file(WriteLocalFileRequest {
        logical_path: "nested/file.txt".to_string(),
        resolved_path: target.clone(),
        content: "replacement".to_string(),
    })
    .expect("write should overwrite file");

    assert_eq!(overwritten.status, WriteLocalFileStatus::Overwritten);
    assert_eq!(
        fs::read_to_string(&target).expect("file should exist"),
        "replacement"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn edit_local_file_writes_apply_edit_result() {
    let root = make_temp_dir("edit");
    let target = root.join("file.txt");
    fs::write(&target, "alpha\nbeta\n").expect("file should be written");

    let outcome = edit_local_file(EditLocalFileRequest {
        logical_path: "file.txt".to_string(),
        resolved_path: target.clone(),
        old_string: "beta".to_string(),
        new_string: "BETA".to_string(),
        replace_all: false,
    })
    .expect("edit should succeed");

    assert_eq!(outcome.match_count, 1);
    assert_eq!(outcome.edit_result.start_line, 2);
    assert_eq!(
        fs::read_to_string(&target).expect("file should exist"),
        "alpha\nBETA\n"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn delete_local_path_inspection_and_execution_preserve_recursive_guard_facts() {
    let root = make_temp_dir("delete");
    let dir = root.join("dir");
    fs::create_dir_all(&dir).expect("dir should be created");
    fs::write(dir.join("child.txt"), "child").expect("child should be written");

    let target = inspect_local_delete_target(&dir).expect("target should inspect");
    assert_eq!(
        target,
        LocalDeleteTarget {
            exists: true,
            is_directory: true,
            is_empty: false,
        }
    );

    let deleted = delete_local_path(DeleteLocalPathRequest {
        logical_path: "dir".to_string(),
        resolved_path: dir.clone(),
        recursive: true,
    })
    .expect("recursive delete should succeed");

    assert!(deleted.is_directory);
    assert!(deleted.recursive);
    assert!(!dir.exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn execute_local_glob_keeps_shallowest_matches() {
    let root = make_temp_dir("glob");
    fs::create_dir_all(root.join("src").join("deep")).expect("dirs should be created");
    fs::create_dir_all(root.join("tests")).expect("dirs should be created");
    fs::write(root.join("Cargo.toml"), "").expect("file should be written");
    fs::write(root.join("src").join("lib.rs"), "").expect("file should be written");
    fs::write(root.join("src").join("deep").join("mod.rs"), "").expect("file should be written");
    fs::write(root.join("tests").join("mod.rs"), "").expect("file should be written");

    let result = execute_local_glob(LocalGlobRequest {
        search_path: root.clone(),
        pattern: "**/*.rs".to_string(),
        limit: 2,
    })
    .expect("glob should succeed");

    let matches = result
        .matches
        .iter()
        .map(|path| normalized(path))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 2);
    assert!(matches.iter().any(|path| path.ends_with("/src/lib.rs")));
    assert!(matches.iter().any(|path| path.ends_with("/tests/mod.rs")));
    assert!(!matches
        .iter()
        .any(|path| path.ends_with("/src/deep/mod.rs")));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn remote_glob_stdout_is_normalized_and_limited_by_tool_runtime() {
    let matches =
        collect_remote_glob_matches("C:/repo", "./src/deep/mod.rs\nsrc/lib.rs\nREADME.md\n\n", 2)
            .into_iter()
            .map(|path| normalized(&path))
            .collect::<Vec<_>>();

    assert_eq!(matches, vec!["C:/repo/README.md", "C:/repo/src/lib.rs"]);
}

#[test]
fn shell_single_quote_preserves_existing_remote_escape_style() {
    assert_eq!(shell_single_quote("C:/repo/a'b"), "'C:/repo/a'\\''b'");
}

#[test]
fn remote_read_command_and_parser_preserve_existing_window_markers() {
    let command =
        build_remote_read_command("C:/repo/a'b.txt", 2, 3, 120, 1_000).expect("command builds");

    assert!(command.starts_with(
        "if [ ! -f 'C:/repo/a'\\''b.txt' ]; then exit 3; fi; awk -v start=2 -v end=4 -v max=120 -v budget=1000"
    ));
    assert!(command.contains("__BITFUN_TOTAL_LINES__="));
    assert!(command.contains("__BITFUN_HIT_TOTAL_CHAR_LIMIT__="));
    assert!(command.ends_with("'C:/repo/a'\\''b.txt'"));

    let result = parse_remote_read_output(
        "     2\talpha\n",
        "__BITFUN_TOTAL_LINES__=5\n__BITFUN_HIT_TOTAL_CHAR_LIMIT__=1\n",
        0,
        "C:/repo/a'b.txt",
        2,
    )
    .expect("remote output parses");

    assert_eq!(result.start_line, 2);
    assert_eq!(result.end_line, 2);
    assert_eq!(result.total_lines, 5);
    assert_eq!(result.content, "     2\talpha");
    assert!(result.hit_total_char_limit);
}

#[test]
fn remote_ls_command_plan_and_stdout_parser_preserve_existing_shape() {
    let plan = build_remote_list_commands("/repo/a'b", 10);

    assert_eq!(
        plan.scan_command,
        "find '/repo/a'\\''b' -maxdepth 1 -not -name '.*' -not -path '/repo/a'\\''b' | head -n 11 | sort"
    );
    assert_eq!(
        plan.listing_command,
        "ls -la --time-style=long-iso '/repo/a'\\''b' 2>/dev/null || ls -la '/repo/a'\\''b'"
    );

    let entries = parse_remote_list_entries("/repo/a'b/file.txt\n/repo/a'b/dir/\n\n");

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "file.txt");
    assert_eq!(entries[0].path, "/repo/a'b/file.txt");
    assert!(!entries[0].is_dir);
    assert_eq!(entries[1].name, "dir");
    assert_eq!(entries[1].path, "/repo/a'b/dir/");
    assert!(entries[1].is_dir);
}

#[test]
fn remote_delete_command_preserves_existing_recursive_flag_and_escaping() {
    assert_eq!(
        build_remote_delete_command("/repo/a'b.txt", false),
        "rm -f '/repo/a'\\''b.txt'"
    );
    assert_eq!(
        build_remote_delete_command("/repo/a'b", true),
        "rm -rf '/repo/a'\\''b'"
    );
}

#[test]
fn remote_grep_command_preserves_rg_fallback_filters_and_windowing() {
    let command = build_remote_grep_command(&RemoteGrepCommandRequest {
        pattern: "panic('x')".to_string(),
        path: "/repo/src app".to_string(),
        case_insensitive: true,
        output_mode: OutputMode::Content,
        show_line_numbers: true,
        context: Some(2),
        before_context: Some(1),
        after_context: Some(1),
        glob_patterns: vec!["*.rs".to_string(), "**/*.ts".to_string()],
        file_type: Some("rust".to_string()),
        head_limit: Some(7),
        offset: 3,
    });

    assert_eq!(
        command,
        "if command -v rg >/dev/null 2>&1; then rg --no-heading --hidden --max-columns 500 -i --line-number -C 2 --glob '*.rs' --glob '**/*.ts' --type 'rust' -e 'panic('\\''x'\\'')' '/repo/src app' 2>/dev/null | tail -n +4 | head -n 7; else grep -rni -e 'panic('\\''x'\\'')' '/repo/src app' 2>/dev/null | tail -n +4 | head -n 7; fi"
    );
}

#[test]
fn remote_grep_result_rendering_preserves_counts_and_display_paths() {
    let stdout = "/repo/src/main.rs:12:panic!(\"x\")\n/repo/src/lib.rs:3:pub fn lib() {}\n";

    assert_eq!(count_remote_grep_matches(stdout), 2);
    assert_eq!(
        render_remote_grep_result_text(stdout, "panic", Some("/repo")),
        "src/main.rs:12:panic!(\"x\")\nsrc/lib.rs:3:pub fn lib() {}"
    );
    assert_eq!(
        render_remote_grep_result_text("", "panic", Some("/repo")),
        "No matches found for pattern 'panic'"
    );
}

#[test]
fn grep_result_windowing_can_be_applied_outside_core() {
    let mut items = vec![
        "one".to_string(),
        "two".to_string(),
        "three".to_string(),
        "four".to_string(),
    ];

    apply_offset_and_limit(&mut items, 1, Some(2));
    assert_eq!(items, vec!["two", "three"]);

    let text = "C:/repo/src/main.rs:1:one\nC:/repo/src/lib.rs:2:two";
    assert_eq!(
        relativize_result_text(text, Some("C:/repo")),
        "src/main.rs:1:one\nsrc/lib.rs:2:two"
    );
}
