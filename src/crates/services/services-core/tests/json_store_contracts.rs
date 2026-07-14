use bitfun_services_core::json_store::{JsonFileStore, JsonFileStoreError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TestPayload {
    label: String,
    count: u32,
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
        let path = std::env::temp_dir().join(format!("bitfun-json-store-{name}-{nonce}"));
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
