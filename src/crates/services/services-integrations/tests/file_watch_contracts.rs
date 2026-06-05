#![cfg(feature = "file-watch")]

use bitfun_services_integrations::file_watch::{
    FileWatchEventKind, FileWatchService, FileWatcherConfig,
};

#[tokio::test]
async fn file_watch_preserves_missing_path_error() {
    let service = FileWatchService::new(FileWatcherConfig::default());

    let error = service
        .watch_path(
            "__bitfun_missing_watch_path_for_services_integrations_test__",
            None,
        )
        .await
        .expect_err("missing paths should keep the existing error contract");

    assert_eq!(error, "Path does not exist");
}

#[test]
fn file_watch_event_kind_serializes_snake_case() {
    let value = serde_json::to_value(FileWatchEventKind::Modify).expect("serialize event kind");

    assert_eq!(value, "modify");
}
