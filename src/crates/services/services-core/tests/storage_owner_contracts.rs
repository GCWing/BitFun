use bitfun_services_core::persistence::{PersistenceService, StorageOptions};
use bitfun_services_core::storage_cleanup::{CleanupPolicy, CleanupRoots, CleanupService};
use bitfun_services_core::workspace_instructions::read_workspace_instruction_files;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DemoRecord {
    name: String,
    count: u32,
}

#[tokio::test]
async fn persistence_service_keeps_atomic_json_shape_and_backups() {
    let temp = tempfile::tempdir().expect("tempdir");
    let service = PersistenceService::new(temp.path().join("store"))
        .await
        .expect("service");

    service
        .save_json(
            "demo",
            &DemoRecord {
                name: "first".to_string(),
                count: 1,
            },
            StorageOptions::default(),
        )
        .await
        .expect("first save");
    service
        .save_json(
            "demo",
            &DemoRecord {
                name: "second".to_string(),
                count: 2,
            },
            StorageOptions::default(),
        )
        .await
        .expect("second save");

    let loaded: DemoRecord = service
        .load_json("demo")
        .await
        .expect("load")
        .expect("record");
    assert_eq!(
        loaded,
        DemoRecord {
            name: "second".to_string(),
            count: 2
        }
    );

    let backups = fs::read_dir(temp.path().join("store").join("backups"))
        .expect("backup dir")
        .count();
    assert_eq!(backups, 1);
    assert!(service.delete("demo").await.expect("delete"));
    assert!(service
        .load_json::<DemoRecord>("demo")
        .await
        .expect("load missing")
        .is_none());
}

#[tokio::test]
async fn cleanup_service_deletes_old_temp_and_log_files_without_product_paths() {
    let temp = tempfile::tempdir().expect("tempdir");
    let temp_dir = temp.path().join("temp");
    let logs_dir = temp.path().join("logs");
    let cache_dir = temp.path().join("cache");
    fs::create_dir_all(&temp_dir).expect("temp dir");
    fs::create_dir_all(&logs_dir).expect("logs dir");
    fs::create_dir_all(&cache_dir).expect("cache dir");

    let old_temp_file = temp_dir.join("old.tmp");
    let old_log_file = logs_dir.join("old.log");
    fs::write(&old_temp_file, "old temp").expect("old temp");
    fs::write(&old_log_file, "old log").expect("old log");

    let old_time = filetime::FileTime::from_system_time(
        SystemTime::now() - Duration::from_secs(60 * 60 * 24 * 10),
    );
    filetime::set_file_mtime(&old_temp_file, old_time).expect("mtime temp");
    filetime::set_file_mtime(&old_log_file, old_time).expect("mtime log");

    let service = CleanupService::new(
        CleanupRoots {
            temp_dir,
            logs_dir,
            cache_dir,
        },
        CleanupPolicy {
            temp_retention_days: 7,
            log_retention_days: 7,
            ..CleanupPolicy::default()
        },
    );

    let result = service.cleanup_all().await.expect("cleanup");
    assert_eq!(result.files_deleted, 2);
    assert!(!old_temp_file.exists());
    assert!(!old_log_file.exists());
}

#[tokio::test]
async fn cleanup_service_trims_oldest_cache_files_when_size_exceeds_policy() {
    let temp = tempfile::tempdir().expect("tempdir");
    let temp_dir = temp.path().join("temp");
    let logs_dir = temp.path().join("logs");
    let cache_dir = temp.path().join("cache");
    fs::create_dir_all(&temp_dir).expect("temp dir");
    fs::create_dir_all(&logs_dir).expect("logs dir");
    fs::create_dir_all(&cache_dir).expect("cache dir");

    let newest_file = cache_dir.join("newest.bin");
    let middle_file = cache_dir.join("middle.bin");
    let oldest_file = cache_dir.join("oldest.bin");
    let two_mb = vec![b'x'; 2 * 1_048_576];
    fs::write(&newest_file, &two_mb).expect("newest");
    fs::write(&middle_file, &two_mb).expect("middle");
    fs::write(&oldest_file, &two_mb).expect("oldest");

    let now = SystemTime::now();
    filetime::set_file_mtime(
        &newest_file,
        filetime::FileTime::from_system_time(now - Duration::from_secs(60)),
    )
    .expect("newest mtime");
    filetime::set_file_mtime(
        &middle_file,
        filetime::FileTime::from_system_time(now - Duration::from_secs(120)),
    )
    .expect("middle mtime");
    filetime::set_file_mtime(
        &oldest_file,
        filetime::FileTime::from_system_time(now - Duration::from_secs(180)),
    )
    .expect("oldest mtime");

    let service = CleanupService::new(
        CleanupRoots {
            temp_dir,
            logs_dir,
            cache_dir,
        },
        CleanupPolicy {
            max_cache_size_mb: 4,
            ..CleanupPolicy::default()
        },
    );

    let result = service.cleanup_all().await.expect("cleanup");

    assert_eq!(result.files_deleted, 1);
    assert_eq!(result.bytes_freed, two_mb.len() as u64);
    assert!(newest_file.exists());
    assert!(middle_file.exists());
    assert!(!oldest_file.exists());
    assert_eq!(result.categories.len(), 1);
    assert_eq!(result.categories[0].name, "Oversized Cache");
}

#[tokio::test]
async fn workspace_instruction_files_reads_agents_then_claude_and_skips_empty_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::write(temp.path().join("AGENTS.md"), "agent rules\n").expect("agents");
    fs::write(temp.path().join("CLAUDE.md"), "claude rules\n").expect("claude");

    let files = read_workspace_instruction_files(temp.path())
        .await
        .expect("instruction files");

    assert_eq!(files.len(), 2);
    assert_eq!(files[0].name, "AGENTS.md");
    assert_eq!(files[0].content, "agent rules\n");
    assert_eq!(files[1].name, "CLAUDE.md");
    assert_eq!(files[1].content, "claude rules\n");

    fs::write(temp.path().join("AGENTS.md"), "").expect("empty agents");
    let files = read_workspace_instruction_files(temp.path())
        .await
        .expect("instruction files");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "CLAUDE.md");
}

#[tokio::test]
async fn token_usage_service_persists_records_and_filters_subagents_by_default() {
    let temp = tempfile::tempdir().expect("tempdir");
    let service =
        bitfun_services_core::token_usage::TokenUsageService::new(temp.path().to_path_buf())
            .await
            .expect("service");

    service
        .record_usage(
            "model-config-a".to_string(),
            "model-a".to_string(),
            "session-a".to_string(),
            "turn-a".to_string(),
            100,
            40,
            Some(30),
            Some(json!({ "cacheCreationTokenCount": 12 })),
            false,
        )
        .await
        .expect("record main");
    service
        .record_usage(
            "model-config-a".to_string(),
            "model-a".to_string(),
            "session-a".to_string(),
            "turn-sub".to_string(),
            50,
            10,
            None,
            None,
            true,
        )
        .await
        .expect("record subagent");

    let summary = service
        .get_summary(bitfun_services_core::token_usage::TokenUsageQuery {
            model_id: Some("model-a".to_string()),
            session_id: None,
            time_range: bitfun_services_core::token_usage::TimeRange::All,
            limit: None,
            offset: None,
            include_subagent: false,
        })
        .await
        .expect("summary");

    assert_eq!(summary.record_count, 1);
    assert_eq!(summary.total_input, 100);
    assert_eq!(summary.total_cached, 30);
    assert_eq!(summary.total_cache_write, 12);

    let reloaded =
        bitfun_services_core::token_usage::TokenUsageService::new(temp.path().to_path_buf())
            .await
            .expect("reloaded");
    let stats = reloaded
        .get_model_stats("model-a")
        .await
        .expect("model stats");
    assert_eq!(stats.request_count, 2);
    assert_eq!(stats.total_input, 150);
}

#[tokio::test]
async fn token_usage_clear_does_not_replay_cached_record_batches() {
    let temp = tempfile::tempdir().expect("tempdir");
    let service =
        bitfun_services_core::token_usage::TokenUsageService::new(temp.path().to_path_buf())
            .await
            .expect("service");

    service
        .record_usage(
            "model-config-old".to_string(),
            "model-old".to_string(),
            "session-old".to_string(),
            "turn-old".to_string(),
            10,
            5,
            None,
            None,
            false,
        )
        .await
        .expect("record old usage");
    service.clear_all_stats().await.expect("clear usage");
    service
        .record_usage(
            "model-config-new".to_string(),
            "model-new".to_string(),
            "session-new".to_string(),
            "turn-new".to_string(),
            20,
            7,
            None,
            None,
            false,
        )
        .await
        .expect("record new usage");

    let summary = service
        .get_summary(bitfun_services_core::token_usage::TokenUsageQuery {
            model_id: None,
            session_id: None,
            time_range: bitfun_services_core::token_usage::TimeRange::All,
            limit: None,
            offset: None,
            include_subagent: true,
        })
        .await
        .expect("summary after clear");

    assert_eq!(summary.record_count, 1);
    assert_eq!(summary.total_input, 20);
    assert!(service.get_model_stats("model-old").await.is_none());
    assert_eq!(
        service
            .get_model_stats("model-new")
            .await
            .expect("new model stats")
            .request_count,
        1
    );
}

#[tokio::test]
async fn token_usage_all_range_ignores_non_date_record_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let service =
        bitfun_services_core::token_usage::TokenUsageService::new(temp.path().to_path_buf())
            .await
            .expect("service");

    service
        .record_usage(
            "model-config-a".to_string(),
            "model-a".to_string(),
            "session-a".to_string(),
            "turn-a".to_string(),
            10,
            1,
            None,
            None,
            false,
        )
        .await
        .expect("record usage");

    let records_dir = temp.path().join("records");
    fs::write(
        records_dir.join("manual-backup.json"),
        r#"{"records":[{"model_id":"model-b","session_id":"session-b","turn_id":"turn-b","timestamp":"2026-07-07T00:00:00Z","input_tokens":999,"output_tokens":1,"cached_tokens":0,"cached_tokens_available":false,"cache_write_tokens":0,"total_tokens":1000,"token_details":null,"is_subagent":false}]}"#,
    )
    .expect("write stray record file");

    let summary = service
        .get_summary(bitfun_services_core::token_usage::TokenUsageQuery {
            model_id: None,
            session_id: None,
            time_range: bitfun_services_core::token_usage::TimeRange::All,
            limit: None,
            offset: None,
            include_subagent: true,
        })
        .await
        .expect("summary");

    assert_eq!(summary.record_count, 1);
    assert_eq!(summary.total_input, 10);
}
