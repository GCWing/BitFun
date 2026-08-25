#![cfg(feature = "local-storage")]

use bitfun_services_core::session::{SessionWriteLock, SessionWriteLockError};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::tempdir;

#[test]
fn one_session_can_have_only_one_writer() {
    let storage_root = tempdir().expect("session storage root");

    let first =
        SessionWriteLock::try_acquire(storage_root.path(), "shared-session").expect("first writer");
    let error = SessionWriteLock::try_acquire(storage_root.path(), "shared-session")
        .expect_err("second writer must fail immediately");

    assert!(matches!(error, SessionWriteLockError::InUse));
    assert_eq!(error.code(), "session_in_use");

    drop(first);
    SessionWriteLock::try_acquire(storage_root.path(), "shared-session")
        .expect("writer after release");
}

#[test]
fn different_sessions_in_one_workspace_are_independent() {
    let storage_root = tempdir().expect("session storage root");

    let _first = SessionWriteLock::try_acquire(storage_root.path(), "first-session")
        .expect("first session writer");
    let _second = SessionWriteLock::try_acquire(storage_root.path(), "second-session")
        .expect("second session writer");
}

#[test]
fn the_same_session_id_in_different_storage_roots_is_independent() {
    let first_storage_root = tempdir().expect("first session storage root");
    let second_storage_root = tempdir().expect("second session storage root");

    let _first = SessionWriteLock::try_acquire(first_storage_root.path(), "shared-session")
        .expect("first storage writer");
    let _second = SessionWriteLock::try_acquire(second_storage_root.path(), "shared-session")
        .expect("second storage writer");
}

#[test]
fn storage_path_aliases_resolve_to_the_same_writer() {
    let storage_root = tempdir().expect("session storage root");
    let alias = storage_root.path().join(".");

    let _first =
        SessionWriteLock::try_acquire(storage_root.path(), "shared-session").expect("first writer");
    let error = SessionWriteLock::try_acquire(&alias, "shared-session")
        .expect_err("path alias must identify the same session");

    assert_eq!(error.code(), "session_in_use");
}

#[test]
fn a_stale_lock_file_does_not_block_a_new_writer() {
    let project_root = tempdir().expect("project runtime root");
    let storage_root = project_root.path().join("sessions");

    let first =
        SessionWriteLock::try_acquire(&storage_root, "shared-session").expect("first writer");
    let lock_root = project_root.path().join(".session-write-locks");
    assert_eq!(
        std::fs::read_dir(&lock_root)
            .expect("read lock root")
            .count(),
        1
    );
    drop(first);

    assert_eq!(
        std::fs::read_dir(&lock_root)
            .expect("read lock root")
            .count(),
        1,
        "the lock file may remain after the OS lock is released"
    );
    SessionWriteLock::try_acquire(&storage_root, "shared-session")
        .expect("stale file must not imply ownership");
}

#[test]
fn persistence_operations_reuse_the_process_writer_without_releasing_it() {
    let storage_root = tempdir().expect("session storage root");
    let writer = SessionWriteLock::try_acquire(storage_root.path(), "shared-session")
        .expect("Session writer");

    let operation =
        SessionWriteLock::try_acquire_for_operation(storage_root.path(), "shared-session")
            .expect("same-process persistence operation");
    drop(operation);

    assert!(matches!(
        SessionWriteLock::try_acquire(storage_root.path(), "shared-session"),
        Err(SessionWriteLockError::InUse)
    ));
    drop(writer);
    SessionWriteLock::try_acquire(storage_root.path(), "shared-session")
        .expect("writer after the process writer releases");
}

#[test]
fn invalid_session_ids_fail_before_touching_the_lock_root() {
    let project_root = tempdir().expect("project runtime root");
    let storage_root = project_root.path().join("sessions");

    let error = SessionWriteLock::try_acquire(&storage_root, "bad\nid").expect_err("invalid id");

    assert_eq!(error.code(), "invalid_session_id");
    assert_eq!(
        std::fs::read_dir(project_root.path())
            .expect("read project runtime root")
            .count(),
        0
    );
}

#[tokio::test]
async fn a_timed_out_write_operation_releases_its_writer() {
    let storage_root = tempdir().expect("session storage root");

    let timed_out = tokio::time::timeout(Duration::from_millis(20), async {
        let _writer = SessionWriteLock::try_acquire(storage_root.path(), "timed-out-session")
            .expect("temporary writer");
        std::future::pending::<()>().await;
    })
    .await;
    assert!(timed_out.is_err());

    SessionWriteLock::try_acquire(storage_root.path(), "timed-out-session")
        .expect("writer after timeout cancellation");
}

#[test]
fn abnormal_process_exit_releases_the_writer() {
    let project_root = tempdir().expect("project runtime root");
    let storage_root = project_root.path().join("sessions");
    let ready_path = project_root.path().join("child-ready");
    let mut command = Command::new(std::env::current_exe().expect("current test executable"));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command
        .arg("--exact")
        .arg("abnormal_exit_child_holds_writer")
        .arg("--nocapture")
        .env("BITFUN_SESSION_WRITE_LOCK_CHILD", "1")
        .env("BITFUN_SESSION_WRITE_STORAGE_ROOT", &storage_root)
        .env("BITFUN_SESSION_WRITE_READY_PATH", &ready_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn writer child");

    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready_path.exists() && Instant::now() < deadline {
        if let Some(status) = child.try_wait().expect("poll writer child") {
            panic!("writer child exited before acquiring the lock: {status}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if !ready_path.exists() {
        let _ = child.kill();
        let _ = child.wait();
        panic!("writer child did not become ready");
    }
    let was_blocked = matches!(
        SessionWriteLock::try_acquire(&storage_root, "abnormal-exit-session"),
        Err(SessionWriteLockError::InUse)
    );

    child.kill().expect("terminate writer child");
    child.wait().expect("reap writer child");
    assert!(was_blocked, "child process must own the Session writer");
    SessionWriteLock::try_acquire(&storage_root, "abnormal-exit-session")
        .expect("writer after abnormal process exit");
}

#[test]
fn abnormal_exit_child_holds_writer() {
    if std::env::var_os("BITFUN_SESSION_WRITE_LOCK_CHILD").is_none() {
        return;
    }
    let storage_root = std::path::PathBuf::from(
        std::env::var_os("BITFUN_SESSION_WRITE_STORAGE_ROOT").expect("child storage root"),
    );
    let ready_path = std::path::PathBuf::from(
        std::env::var_os("BITFUN_SESSION_WRITE_READY_PATH").expect("child ready path"),
    );
    let _writer = SessionWriteLock::try_acquire(&storage_root, "abnormal-exit-session")
        .expect("child writer");
    std::fs::write(ready_path, b"ready").expect("publish child readiness");
    loop {
        std::thread::park();
    }
}
