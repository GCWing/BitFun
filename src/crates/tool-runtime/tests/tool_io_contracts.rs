use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tool_runtime::fs::{
    delete_local_path, edit_local_file, inspect_local_delete_target, write_local_file,
    DeleteLocalPathRequest, EditLocalFileRequest, LocalDeleteTarget, WriteLocalFileRequest,
    WriteLocalFileStatus,
};
use tool_runtime::search::glob_search::{execute_local_glob, LocalGlobRequest};

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
