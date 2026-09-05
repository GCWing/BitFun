use openbitfun_services_core::json_store::{JsonFileStore, JsonFileStoreError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct TestPayload {
    label: String,
    count: u32,
}

#[tokio::test]
async fn locked_updates_merge_independent_read_modify_write_operations() {
    let root = TestTempDir::new("locked-update");
    let path = root.path().join("preferences.json");
    let first = JsonFileStore;
    let second = JsonFileStore;

    let first_update = first.update_locked(&path, TestPayload::default(), |payload| {
        payload.label = "preserved".to_string();
    });
    let second_update = second.update_locked(&path, TestPayload::default(), |payload| {
        payload.count = 7;
    });
    let (first_result, second_result) = tokio::join!(first_update, second_update);
    first_result.expect("first update");
    second_result.expect("second update");

    let loaded = JsonFileStore
        .read_locked_optional::<TestPayload>(&path)
        .await
        .expect("locked read")
        .expect("persisted payload");
    assert_eq!(
        loaded,
        TestPayload {
            label: "preserved".to_string(),
            count: 7,
        }
    );
}

struct TestTempDir {
    path: PathBuf,
}

impl TestTempDir {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("openbitfun-json-store-{name}-{nonce}"));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestTempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[tokio::test]
async fn json_store_returns_none_for_missing_file() {
    let root = TestTempDir::new("missing");
    let store = JsonFileStore;

    let value = store
        .read_optional::<TestPayload>(&root.path().join("missing.json"))
        .await
        .expect("missing file should not be an error");

    assert_eq!(value, None);
}

#[tokio::test]
async fn json_store_creates_parent_dirs_and_round_trips_payload() {
    let root = TestTempDir::new("round-trip");
    let store = JsonFileStore;
    let path = root.path().join("nested").join("payload.json");
    let payload = TestPayload {
        label: "session metadata".to_string(),
        count: 3,
    };

    store
        .write_atomic(&path, &payload)
        .await
        .expect("write should create parent dir");
    let loaded = store
        .read_optional::<TestPayload>(&path)
        .await
        .expect("written payload should be readable");

    assert_eq!(loaded, Some(payload));
}

#[cfg(windows)]
#[tokio::test]
async fn strict_atomic_write_supports_extended_length_windows_paths() {
    let root = TestTempDir::new("long-path");
    let segment = "snapshot-history-segment".repeat(5);
    let path = root
        .path()
        .join(&segment)
        .join(&segment)
        .join("session.json");
    assert!(path.to_string_lossy().len() > 260);
    let initial = TestPayload {
        label: "persisted".to_string(),
        count: 1,
    };
    let forward_slash_path = PathBuf::from(path.to_string_lossy().replace('\\', "/"));

    JsonFileStore
        .write_atomic_strict(&forward_slash_path, &initial)
        .await
        .expect("strict first write should normalize long forward-slash paths");

    let alias_anchor = path.parent().unwrap().join("alias-anchor");
    std::fs::create_dir_all(&alias_anchor).expect("create long-path alias anchor");
    let aliased_path = alias_anchor.join("..").join("session.json");
    let replacement = TestPayload {
        label: "replaced".to_string(),
        count: 2,
    };
    JsonFileStore
        .write_atomic_strict(&aliased_path, &replacement)
        .await
        .expect("strict replacement should normalize dot segments");

    assert_eq!(
        JsonFileStore
            .read_optional::<TestPayload>(&path)
            .await
            .expect("long-path payload should be readable"),
        Some(replacement)
    );
}

#[tokio::test]
async fn json_store_reports_no_parent_directory() {
    let store = JsonFileStore;

    let error = store
        .write_atomic(
            Path::new(""),
            &TestPayload {
                label: "rootless".to_string(),
                count: 1,
            },
        )
        .await
        .expect_err("empty path has no parent component");

    assert!(matches!(
        error,
        JsonFileStoreError::NoParentDirectory { .. }
    ));
}
