#![cfg(feature = "file-watch")]

use bitfun_services_integrations::file_watch::{
    FileWatchEventKind, FileWatchService, FileWatcherConfig,
};
use std::fs;
use std::time::Duration;

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

#[tokio::test]
async fn file_watch_publishes_debounced_batches_to_backend_subscribers() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut config = FileWatcherConfig::default();
    config.debounce_interval_ms = 40;
    config.ignore_hidden_files = false;
    let service = FileWatchService::new(config.clone());
    let mut events = service.subscribe();
    service
        .watch_path(temp.path().to_str().unwrap(), Some(config))
        .await
        .expect("watch temp directory");

    let file = temp.path().join("command.md");
    fs::write(&file, "first").expect("create watched file");
    fs::write(&file, "second").expect("modify watched file");

    let batch = tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .expect("watch batch timeout")
        .expect("watch broadcast remains open");
    assert!(batch
        .iter()
        .any(|event| event.path == file.to_string_lossy()));
}

#[tokio::test]
async fn a_narrow_duplicate_registration_does_not_downgrade_recursive_watch() {
    let temp = tempfile::tempdir().expect("tempdir");
    let nested = temp.path().join("nested");
    fs::create_dir_all(&nested).expect("nested directory");
    let mut recursive = FileWatcherConfig::default();
    recursive.debounce_interval_ms = 40;
    recursive.ignore_hidden_files = false;
    let service = FileWatchService::new(recursive.clone());
    let mut events = service.subscribe();
    service
        .watch_path(temp.path().to_str().unwrap(), Some(recursive.clone()))
        .await
        .expect("recursive watch");
    recursive.watch_recursively = false;
    service
        .watch_path(temp.path().to_str().unwrap(), Some(recursive))
        .await
        .expect("shared narrow watch");

    let file = nested.join("command.md");
    fs::write(&file, "created").expect("nested file");
    let batch = tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .expect("watch batch timeout")
        .expect("watch broadcast remains open");
    assert!(batch
        .iter()
        .any(|event| event.path == file.to_string_lossy()));
}

#[tokio::test]
async fn a_removed_root_does_not_block_registering_another_root() {
    let temp = tempfile::tempdir().expect("tempdir");
    let removed = temp.path().join("removed");
    let replacement = temp.path().join("replacement");
    fs::create_dir_all(&removed).expect("removed root");
    fs::create_dir_all(&replacement).expect("replacement root");
    let service = FileWatchService::new(FileWatcherConfig::default());
    service
        .watch_path(removed.to_str().unwrap(), None)
        .await
        .expect("first root");
    fs::remove_dir_all(&removed).expect("remove first root");

    service
        .watch_path(replacement.to_str().unwrap(), None)
        .await
        .expect("missing stale roots should be skipped during reconfiguration");
}

#[tokio::test]
async fn atomic_rename_keeps_the_non_temporary_destination_path() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut config = FileWatcherConfig::default();
    config.debounce_interval_ms = 40;
    config.ignore_hidden_files = false;
    let service = FileWatchService::new(config.clone());
    let mut events = service.subscribe();
    service
        .watch_path(temp.path().to_str().unwrap(), Some(config))
        .await
        .expect("watch temp directory");

    let temporary = temp.path().join("command.md.tmp");
    let destination = temp.path().join("command.md");
    fs::write(&temporary, "complete").expect("temporary file");
    fs::rename(&temporary, &destination).expect("atomic rename");

    let batch = tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .expect("watch batch timeout")
        .expect("watch broadcast remains open");
    assert!(batch
        .iter()
        .any(|event| event.path == destination.to_string_lossy()));
}
