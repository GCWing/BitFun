use crate::artifacts::ArtifactStore;
use crate::db::Database;
use chrono::Utc;
use sqlx::Row;
use std::collections::BTreeSet;
use std::time::{Duration, UNIX_EPOCH};

const DRAFT_RETENTION_SECONDS: i64 = 7 * 24 * 60 * 60;
const CLOSED_RETENTION_SECONDS: i64 = 90 * 24 * 60 * 60;
const DOWNLOAD_DETAIL_RETENTION_SECONDS: i64 = 90 * 24 * 60 * 60;
const CLEANUP_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct RetentionResult {
    pub submissions_scrubbed: u64,
    pub packages_removed: u64,
    pub previews_removed: u64,
    pub temporary_files_removed: u64,
    pub download_rows_compacted: u64,
}

pub(crate) async fn cleanup(
    database: &Database,
    artifacts: &ArtifactStore,
) -> anyhow::Result<RetentionResult> {
    cleanup_at(database, artifacts, Utc::now().timestamp()).await
}

async fn cleanup_at(
    database: &Database,
    artifacts: &ArtifactStore,
    now: i64,
) -> anyhow::Result<RetentionResult> {
    // Keep content-addressed artifact writes plus their database attachment
    // atomic with respect to orphan cleanup. Without this lease, cleanup could
    // delete a reused hash after upload observes it but before the submission
    // row starts referencing it.
    let mutation_guard = artifacts.lock_mutations().await;
    let draft_cutoff = now - DRAFT_RETENTION_SECONDS;
    let closed_cutoff = now - CLOSED_RETENTION_SECONDS;
    let candidates = sqlx::query(
        "SELECT id, package_sha256, preview_sha256 FROM submissions
         WHERE (status = 'draft' AND updated_at <= ?)
            OR (status IN ('rejected', 'withdrawn') AND updated_at <= ?)",
    )
    .bind(draft_cutoff)
    .bind(closed_cutoff)
    .fetch_all(database.pool())
    .await?;
    let mut packages = BTreeSet::new();
    let mut previews = BTreeSet::new();
    let mut scrubbed = 0;
    for row in candidates {
        let submission_id: String = row.get("id");
        if let Ok(value) = row.try_get::<String, _>("package_sha256") {
            packages.insert(value);
        }
        if let Ok(value) = row.try_get::<String, _>("preview_sha256") {
            previews.insert(value);
        }
        scrubbed += sqlx::query(
            "UPDATE submissions SET
               package_meta_json = NULL,
               manifest_json = NULL,
               package_sha256 = NULL,
               package_size = NULL,
               preview_sha256 = NULL
             WHERE id = ? AND ((status = 'draft' AND updated_at <= ?)
                OR (status IN ('rejected', 'withdrawn') AND updated_at <= ?))",
        )
        .bind(submission_id)
        .bind(draft_cutoff)
        .bind(closed_cutoff)
        .execute(database.pool())
        .await?
        .rows_affected();
    }
    let mut result = RetentionResult {
        submissions_scrubbed: scrubbed,
        ..RetentionResult::default()
    };
    for hash in packages {
        if package_references(database, &hash).await? == 0
            && artifacts.remove_package(&mutation_guard, &hash).await?
        {
            result.packages_removed += 1;
        }
    }
    for hash in previews {
        if preview_references(database, &hash).await? == 0
            && artifacts.remove_preview(&mutation_guard, &hash).await?
        {
            result.previews_removed += 1;
        }
    }
    let cutoff = UNIX_EPOCH + Duration::from_secs(draft_cutoff.max(0) as u64);
    for hash in artifacts.package_hashes_older_than(cutoff).await? {
        if package_references(database, &hash).await? == 0
            && artifacts.remove_package(&mutation_guard, &hash).await?
        {
            result.packages_removed += 1;
        }
    }
    for hash in artifacts.preview_hashes_older_than(cutoff).await? {
        if preview_references(database, &hash).await? == 0
            && artifacts.remove_preview(&mutation_guard, &hash).await?
        {
            result.previews_removed += 1;
        }
    }
    result.temporary_files_removed = artifacts
        .remove_temporary_older_than(&mutation_guard, cutoff)
        .await?;
    result.download_rows_compacted = compact_download_rows(database, now).await?;
    Ok(result)
}

async fn compact_download_rows(database: &Database, now: i64) -> anyhow::Result<u64> {
    let cutoff = chrono::DateTime::from_timestamp(now - DOWNLOAD_DETAIL_RETENTION_SECONDS, 0)
        .ok_or_else(|| anyhow::anyhow!("download retention timestamp is invalid"))?
        .format("%Y-%m-%d")
        .to_string();
    let mut transaction = database.pool().begin().await?;
    sqlx::query(
        "UPDATE listings SET download_count = download_count + (
           SELECT COUNT(*) FROM download_days d
           WHERE d.listing_id = listings.id AND d.day < ?
         ) WHERE EXISTS (
           SELECT 1 FROM download_days d
           WHERE d.listing_id = listings.id AND d.day < ?
         )",
    )
    .bind(&cutoff)
    .bind(&cutoff)
    .execute(&mut *transaction)
    .await?;
    let removed = sqlx::query("DELETE FROM download_days WHERE day < ?")
        .bind(&cutoff)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    transaction.commit().await?;
    Ok(removed)
}

async fn package_references(database: &Database, hash: &str) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT (SELECT COUNT(*) FROM submissions WHERE package_sha256 = ?)
              + (SELECT COUNT(*) FROM releases WHERE package_sha256 = ?)",
    )
    .bind(hash)
    .bind(hash)
    .fetch_one(database.pool())
    .await?)
}

async fn preview_references(database: &Database, hash: &str) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "SELECT (SELECT COUNT(*) FROM submissions WHERE preview_sha256 = ?)
              + (SELECT COUNT(*) FROM releases WHERE preview_sha256 = ?)",
    )
    .bind(hash)
    .bind(hash)
    .fetch_one(database.pool())
    .await?)
}

pub(crate) fn spawn_cleanup_loop(database: Database, artifacts: ArtifactStore) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(CLEANUP_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        interval.tick().await;
        loop {
            interval.tick().await;
            match cleanup(&database, &artifacts).await {
                Ok(result)
                    if result.submissions_scrubbed > 0
                        || result.packages_removed > 0
                        || result.previews_removed > 0
                        || result.temporary_files_removed > 0
                        || result.download_rows_compacted > 0 =>
                {
                    tracing::info!(
                        submissions_scrubbed = result.submissions_scrubbed,
                        packages_removed = result.packages_removed,
                        previews_removed = result.previews_removed,
                        temporary_files_removed = result.temporary_files_removed,
                        download_rows_compacted = result.download_rows_compacted,
                        "Appearance market retention cleanup completed"
                    );
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::error!(error = %error, "Appearance market retention cleanup failed");
                }
            }
        }
    });
}
