use bitfun_services_core::session::SessionStorageLayout;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct TestTempDir {
    path: PathBuf,
}

impl TestTempDir {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("bitfun-session-layout-{name}-{nonce}"));
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

#[test]
fn session_layout_preserves_legacy_file_names() {
    let root = TestTempDir::new("paths");
    let layout = SessionStorageLayout::new(root.path().join("sessions"));

    assert_eq!(
        layout.index_path(),
        root.path().join("sessions").join("index.json")
    );
    assert_eq!(
        layout.metadata_path("session-1"),
        root.path()
            .join("sessions")
            .join("session-1")
            .join("metadata.json")
    );
    assert_eq!(
        layout.state_path("session-1"),
        root.path()
            .join("sessions")
            .join("session-1")
            .join("state.json")
    );
    assert_eq!(
        layout.prompt_cache_path("session-1"),
        root.path()
            .join("sessions")
            .join("session-1")
            .join("prompt_cache.json")
    );
    assert_eq!(
        layout.request_trace_path("session-1", 7),
        root.path()
            .join("sessions")
            .join("session-1")
            .join("request-traces")
            .join("request-000007.json")
    );
    assert_eq!(
        layout.turn_path("session-1", 7),
        root.path()
            .join("sessions")
            .join("session-1")
            .join("turns")
            .join("turn-0007.json")
    );
    assert_eq!(
        layout.context_snapshot_path("session-1", 2),
        root.path()
            .join("sessions")
            .join("session-1")
            .join("snapshots")
            .join("context-0002.json")
    );
    assert_eq!(
        layout.skill_agent_snapshot_path("session-1", 2),
        root.path()
            .join("sessions")
            .join("session-1")
            .join("snapshots")
            .join("skill-agent-0002.json")
    );
    assert_eq!(
        layout.skill_agent_baseline_override_path("session-1"),
        root.path()
            .join("sessions")
            .join("session-1")
            .join("snapshots")
            .join("skill-agent-baseline-override.json")
    );
    assert_eq!(
        layout.transcript_path("session-1"),
        root.path()
            .join("sessions")
            .join("session-1")
            .join("artifacts")
            .join("transcript.txt")
    );
    assert_eq!(
        layout.transcript_meta_path("session-1"),
        root.path()
            .join("sessions")
            .join("session-1")
            .join("artifacts")
            .join("transcript.meta.json")
    );
}

#[tokio::test]
async fn session_layout_ensures_target_directories() {
    let root = TestTempDir::new("ensure");
    let layout = SessionStorageLayout::new(root.path().join("sessions"));

    let session_dir = layout
        .ensure_session_dir("session-1")
        .await
        .expect("session dir should be created");
    let request_traces_dir = layout
        .ensure_request_traces_dir("session-1")
        .await
        .expect("request traces dir should be created");
    let turns_dir = layout
        .ensure_turns_dir("session-1")
        .await
        .expect("turns dir should be created");
    let snapshots_dir = layout
        .ensure_snapshots_dir("session-1")
        .await
        .expect("snapshots dir should be created");
    let artifacts_dir = layout
        .ensure_artifacts_dir("session-1")
        .await
        .expect("artifacts dir should be created");

    assert!(session_dir.exists());
    assert!(request_traces_dir.exists());
    assert!(turns_dir.exists());
    assert!(snapshots_dir.exists());
    assert!(artifacts_dir.exists());
}

#[tokio::test]
async fn session_layout_lists_indexed_turn_paths_in_numeric_order() {
    let root = TestTempDir::new("turn-list");
    let layout = SessionStorageLayout::new(root.path().join("sessions"));
    let turns_dir = layout
        .ensure_turns_dir("session-1")
        .await
        .expect("turns dir should be created");

    std::fs::write(turns_dir.join("turn-0010.json"), "{}").expect("turn file");
    std::fs::write(turns_dir.join("turn-0002.json"), "{}").expect("turn file");
    std::fs::write(turns_dir.join("turn-invalid.json"), "{}").expect("ignored file");
    std::fs::write(turns_dir.join("turn-0003.txt"), "{}").expect("ignored extension");
    std::fs::write(turns_dir.join("notes.json"), "{}").expect("ignored prefix");

    let indexed_paths = layout
        .list_indexed_turn_paths("session-1")
        .await
        .expect("turn paths should be listed");

    assert_eq!(
        indexed_paths
            .iter()
            .map(|(index, _)| *index)
            .collect::<Vec<_>>(),
        vec![2, 10]
    );
    assert_eq!(indexed_paths[0].1, layout.turn_path("session-1", 2));
    assert_eq!(indexed_paths[1].1, layout.turn_path("session-1", 10));
}

#[tokio::test]
async fn session_layout_returns_empty_turn_paths_when_turns_dir_is_missing() {
    let root = TestTempDir::new("missing-turn-list");
    let layout = SessionStorageLayout::new(root.path().join("sessions"));

    let indexed_paths = layout
        .list_indexed_turn_paths("session-1")
        .await
        .expect("missing turns dir should be empty");

    assert!(indexed_paths.is_empty());
}

#[tokio::test]
async fn session_layout_deletes_indexed_turn_paths_from_start_index() {
    let root = TestTempDir::new("turn-delete");
    let layout = SessionStorageLayout::new(root.path().join("sessions"));
    let turns_dir = layout
        .ensure_turns_dir("session-1")
        .await
        .expect("turns dir should be created");

    std::fs::write(layout.turn_path("session-1", 0), "{}").expect("turn file");
    std::fs::write(layout.turn_path("session-1", 1), "{}").expect("turn file");
    std::fs::write(layout.turn_path("session-1", 2), "{}").expect("turn file");
    std::fs::write(turns_dir.join("turn-invalid.json"), "{}").expect("ignored file");

    let deleted = layout
        .delete_indexed_turn_paths_from("session-1", 1)
        .await
        .expect("turn files should be deleted");

    assert_eq!(deleted, 2);
    assert!(layout.turn_path("session-1", 0).exists());
    assert!(!layout.turn_path("session-1", 1).exists());
    assert!(!layout.turn_path("session-1", 2).exists());
    assert!(turns_dir.join("turn-invalid.json").exists());
}
