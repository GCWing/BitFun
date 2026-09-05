use crate::{
    read_frame, write_frame, DiscoveryRecord, DiscoveryStore, RuntimeInstanceIdentity,
    RuntimeInstanceLock, RuntimeIpcFrame, RuntimeIpcIoError, RuntimeIpcOperation,
    MAX_REQUEST_FRAME_BYTES, PROTOCOL_VERSION,
};
use tempfile::tempdir;
use tokio::io::AsyncWriteExt;

#[test]
fn discovery_replacement_never_exposes_partial_json() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let runtime_root = tempdir().expect("runtime root");
    let workspace = tempdir().expect("workspace");
    let identity = RuntimeInstanceIdentity::for_workspace(
        workspace.path(),
        "openbitfun",
        "stable",
        "user-a",
        PROTOCOL_VERSION,
    )
    .expect("identity");
    let store = DiscoveryStore::new(runtime_root.path(), identity.clone());
    let record = |owner: usize| {
        DiscoveryRecord::new(
            identity.clone(),
            format!("endpoint-{owner}-{}", "x".repeat(8 * 1024)),
            42,
            format!("token-{owner}"),
            format!("owner-{owner}"),
        )
    };
    store.write(&record(0)).expect("initial discovery");

    let running = Arc::new(AtomicBool::new(true));
    let reader_running = running.clone();
    let reader_root = runtime_root.path().to_path_buf();
    let reader_identity = identity.clone();
    let reader = std::thread::spawn(move || {
        let reader_store = DiscoveryStore::new(&reader_root, reader_identity);
        while reader_running.load(Ordering::Acquire) {
            let observation = reader_store.read();
            #[cfg(windows)]
            match &observation {
                // ReplaceFileW keeps the target contents atomic but may make
                // the path briefly unavailable while a replacement commits.
                Ok(None) => continue,
                Err(crate::RuntimeIpcDiscoveryError::ReadDiscovery { source, .. })
                    if matches!(source.raw_os_error(), Some(32 | 33)) =>
                {
                    continue;
                }
                _ => {}
            }
            assert!(observation.expect("discovery remains valid").is_some());
        }
    });

    for owner in 1..=500 {
        store.write(&record(owner)).expect("replace discovery");
    }
    running.store(false, Ordering::Release);
    reader.join().expect("reader remains healthy");
    assert_eq!(store.read().expect("final discovery"), Some(record(500)));
}

#[cfg(windows)]
#[test]
fn sharing_violation_is_never_reported_as_missing_discovery() {
    use std::os::windows::fs::OpenOptionsExt;

    let runtime_root = tempdir().expect("runtime root");
    let workspace = tempdir().expect("workspace");
    let identity = RuntimeInstanceIdentity::for_workspace(
        workspace.path(),
        "openbitfun",
        "stable",
        "user-a",
        PROTOCOL_VERSION,
    )
    .expect("identity");
    let store = DiscoveryStore::new(runtime_root.path(), identity.clone());
    let record = DiscoveryRecord::new(
        identity.clone(),
        "local-endpoint".to_string(),
        42,
        "secret-token".to_string(),
        "owner".to_string(),
    );
    store.write(&record).expect("write discovery");
    let path = runtime_root
        .path()
        .join(format!("{}.json", identity.as_str()));
    let _exclusive_reader = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(path)
        .expect("hold discovery without sharing");

    assert!(matches!(
        store.read(),
        Err(crate::RuntimeIpcDiscoveryError::ReadDiscovery { source, .. })
            if matches!(source.raw_os_error(), Some(32 | 33))
    ));
}

#[test]
fn instance_identity_is_stable_and_scoped_to_runtime_facts() {
    let first_workspace = tempdir().expect("first workspace");
    let second_workspace = tempdir().expect("second workspace");

    let first = RuntimeInstanceIdentity::for_workspace(
        first_workspace.path(),
        "openbitfun",
        "stable",
        "user-a",
        PROTOCOL_VERSION,
    )
    .expect("first identity");
    let same = RuntimeInstanceIdentity::for_workspace(
        &first_workspace.path().join("."),
        "openbitfun",
        "stable",
        "user-a",
        PROTOCOL_VERSION,
    )
    .expect("same identity");
    let other_workspace = RuntimeInstanceIdentity::for_workspace(
        second_workspace.path(),
        "openbitfun",
        "stable",
        "user-a",
        PROTOCOL_VERSION,
    )
    .expect("other workspace identity");
    let other_user = RuntimeInstanceIdentity::for_workspace(
        first_workspace.path(),
        "openbitfun",
        "stable",
        "user-b",
        PROTOCOL_VERSION,
    )
    .expect("other user identity");

    assert_eq!(first, same);
    assert_ne!(first, other_workspace);
    assert_ne!(first, other_user);
    assert_eq!(first.as_str().len(), 64);
}

#[cfg(unix)]
#[test]
fn instance_identity_preserves_non_utf8_workspace_bytes() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let root = tempdir().expect("workspace root");
    let first_path = root.path().join(OsString::from_vec(vec![b'w', 0x80]));
    let second_path = root.path().join(OsString::from_vec(vec![b'w', 0x81]));
    std::fs::create_dir(&first_path).expect("first non-UTF-8 workspace");
    std::fs::create_dir(&second_path).expect("second non-UTF-8 workspace");

    let identity = |path| {
        RuntimeInstanceIdentity::for_workspace(
            path,
            "openbitfun",
            "stable",
            "user-a",
            PROTOCOL_VERSION,
        )
        .expect("workspace identity")
    };

    assert_ne!(identity(&first_path), identity(&second_path));
}

#[test]
fn discovery_is_owner_checked_and_instance_lock_is_exclusive() {
    let runtime_root = tempdir().expect("runtime root");
    let workspace = tempdir().expect("workspace");
    let identity = RuntimeInstanceIdentity::for_workspace(
        workspace.path(),
        "openbitfun",
        "stable",
        "user-a",
        PROTOCOL_VERSION,
    )
    .expect("identity");
    let store = DiscoveryStore::new(runtime_root.path(), identity.clone());
    let record = DiscoveryRecord::new(
        identity.clone(),
        "local-endpoint".to_string(),
        42,
        "secret-token".to_string(),
        "owner-one".to_string(),
    );

    let instance_lock = RuntimeInstanceLock::try_acquire(runtime_root.path(), &identity)
        .expect("first instance lock");
    assert!(RuntimeInstanceLock::try_acquire(runtime_root.path(), &identity).is_err());

    store.write(&record).expect("write discovery");
    assert_eq!(store.read().expect("read discovery"), Some(record.clone()));
    assert!(!format!("{record:?}").contains("secret-token"));

    let another_owner = DiscoveryRecord::new(
        identity,
        "local-endpoint".to_string(),
        42,
        "other-token".to_string(),
        "owner-two".to_string(),
    );
    assert!(!store
        .remove_if_owned(&another_owner)
        .expect("do not remove another owner's record"));
    assert!(store.remove_if_owned(&record).expect("remove owned record"));
    assert_eq!(store.read().expect("read removed discovery"), None);

    drop(instance_lock);
    RuntimeInstanceLock::try_acquire(runtime_root.path(), store.identity())
        .expect("lock released by RAII drop");
}

#[tokio::test]
async fn framing_round_trips_health_and_rejects_oversized_lengths() {
    let (mut writer, mut reader) = tokio::io::duplex(MAX_REQUEST_FRAME_BYTES + 16);
    let expected = RuntimeIpcFrame::Request {
        request_id: 9,
        operation: RuntimeIpcOperation::Health,
    };
    write_frame(&mut writer, &expected)
        .await
        .expect("write bounded frame");
    assert_eq!(
        read_frame(&mut reader).await.expect("read bounded frame"),
        expected
    );

    let (mut writer, mut reader) = tokio::io::duplex(8);
    writer
        .write_u32((MAX_REQUEST_FRAME_BYTES + 1) as u32)
        .await
        .expect("write oversized length prefix");
    let error = read_frame(&mut reader)
        .await
        .expect_err("reject oversized frame");
    assert!(matches!(error, RuntimeIpcIoError::FrameTooLarge { .. }));
}

#[tokio::test]
async fn request_framing_rejects_unknown_fields_inside_nested_dtos() {
    let value = serde_json::json!({
        "type": "request",
        "request_id": 1,
        "operation": {
            "operation": "list_sessions",
            "request": {
                "workspacePath": "workspace",
                "future_field": true
            }
        }
    });
    let bytes = serde_json::to_vec(&value).expect("serialize fixture");
    let (mut writer, mut reader) = tokio::io::duplex(bytes.len() + 4);
    writer
        .write_u32(bytes.len() as u32)
        .await
        .expect("write length");
    writer.write_all(&bytes).await.expect("write fixture");

    assert!(matches!(
        read_frame(&mut reader).await,
        Err(RuntimeIpcIoError::UnknownField { path }) if path.ends_with("future_field")
    ));
}
