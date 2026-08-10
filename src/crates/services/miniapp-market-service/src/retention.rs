use crate::artifacts::ArtifactStore;
use crate::db::Database;
use chrono::Utc;
use sqlx::Row;
use std::collections::BTreeSet;
use std::time::{Duration, UNIX_EPOCH};

const DRAFT_RETENTION_SECONDS: i64 = 7 * 24 * 60 * 60;
const CLOSED_RETENTION_SECONDS: i64 = 90 * 24 * 60 * 60;
const CLEANUP_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct RetentionResult {
    pub(crate) submissions_scrubbed: u64,
    pub(crate) packages_removed: u64,
    pub(crate) screenshots_removed: u64,
}

pub(crate) async fn cleanup_expired_submission_artifacts(
    database: &Database,
    artifacts: &ArtifactStore,
) -> anyhow::Result<RetentionResult> {
    let now = Utc::now().timestamp();
    cleanup_expired_submission_artifacts_at(database, artifacts, now).await
}

async fn cleanup_expired_submission_artifacts_at(
    database: &Database,
    artifacts: &ArtifactStore,
    now: i64,
) -> anyhow::Result<RetentionResult> {
    let draft_cutoff = now - DRAFT_RETENTION_SECONDS;
    let closed_cutoff = now - CLOSED_RETENTION_SECONDS;
    let mut transaction = database.pool().begin().await?;
    let candidates = sqlx::query(
        "SELECT id, package_sha256
         FROM submissions
         WHERE (status = 'draft' AND updated_at <= ?)
            OR (status IN ('rejected', 'withdrawn') AND updated_at <= ?)",
    )
    .bind(draft_cutoff)
    .bind(closed_cutoff)
    .fetch_all(&mut *transaction)
    .await?;

    let mut package_hashes = BTreeSet::new();
    let mut submission_ids = Vec::with_capacity(candidates.len());
    for row in candidates {
        submission_ids.push(row.get::<String, _>("id"));
        if let Ok(hash) = row.try_get::<String, _>("package_sha256") {
            package_hashes.insert(hash);
        }
    }

    let mut screenshot_hashes = BTreeSet::new();
    for submission_id in &submission_ids {
        let rows = sqlx::query(
            "SELECT sha256 FROM screenshots
             WHERE submission_id = ? AND release_id IS NULL",
        )
        .bind(submission_id)
        .fetch_all(&mut *transaction)
        .await?;
        screenshot_hashes.extend(rows.into_iter().map(|row| row.get::<String, _>("sha256")));
    }

    let mut scrubbed = 0_u64;
    for submission_id in &submission_ids {
        sqlx::query(
            "DELETE FROM screenshots
             WHERE submission_id = ? AND release_id IS NULL
               AND submission_id IN (
                 SELECT id FROM submissions
                 WHERE (status = 'draft' AND updated_at <= ?)
                    OR (status IN ('rejected', 'withdrawn') AND updated_at <= ?)
               )",
        )
        .bind(submission_id)
        .bind(draft_cutoff)
        .bind(closed_cutoff)
        .execute(&mut *transaction)
        .await?;
        let updated = sqlx::query(
            "UPDATE submissions
             SET package_sha256 = NULL, package_size = NULL
             WHERE id = ?
               AND ((status = 'draft' AND updated_at <= ?)
                 OR (status IN ('rejected', 'withdrawn') AND updated_at <= ?))",
        )
        .bind(submission_id)
        .bind(draft_cutoff)
        .bind(closed_cutoff)
        .execute(&mut *transaction)
        .await?;
        scrubbed += updated.rows_affected();
    }
    transaction.commit().await?;

    let mut result = RetentionResult {
        submissions_scrubbed: scrubbed,
        ..RetentionResult::default()
    };
    for hash in package_hashes {
        let references: i64 = sqlx::query_scalar(
            "SELECT
               (SELECT COUNT(*) FROM submissions WHERE package_sha256 = ?)
               + (SELECT COUNT(*) FROM releases WHERE package_sha256 = ?)",
        )
        .bind(&hash)
        .bind(&hash)
        .fetch_one(database.pool())
        .await?;
        if references == 0 && artifacts.remove_package_if_exists(&hash).await? {
            result.packages_removed += 1;
        }
    }
    for hash in screenshot_hashes {
        let references: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM screenshots WHERE sha256 = ?")
                .bind(&hash)
                .fetch_one(database.pool())
                .await?;
        if references == 0 && artifacts.remove_screenshot_if_exists(&hash).await? {
            result.screenshots_removed += 1;
        }
    }
    let artifact_cutoff = UNIX_EPOCH + Duration::from_secs(draft_cutoff.max(0) as u64);
    for hash in artifacts.package_hashes_older_than(artifact_cutoff).await? {
        let references: i64 = sqlx::query_scalar(
            "SELECT
               (SELECT COUNT(*) FROM submissions WHERE package_sha256 = ?)
               + (SELECT COUNT(*) FROM releases WHERE package_sha256 = ?)",
        )
        .bind(&hash)
        .bind(&hash)
        .fetch_one(database.pool())
        .await?;
        if references == 0 && artifacts.remove_package_if_exists(&hash).await? {
            result.packages_removed += 1;
        }
    }
    for hash in artifacts
        .screenshot_hashes_older_than(artifact_cutoff)
        .await?
    {
        let references: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM screenshots WHERE sha256 = ?")
                .bind(&hash)
                .fetch_one(database.pool())
                .await?;
        if references == 0 && artifacts.remove_screenshot_if_exists(&hash).await? {
            result.screenshots_removed += 1;
        }
    }
    Ok(result)
}

pub(crate) fn spawn_cleanup_loop(database: Database, artifacts: ArtifactStore) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(CLEANUP_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        interval.tick().await;
        loop {
            interval.tick().await;
            match cleanup_expired_submission_artifacts(&database, &artifacts).await {
                Ok(result)
                    if result.submissions_scrubbed > 0
                        || result.packages_removed > 0
                        || result.screenshots_removed > 0 =>
                {
                    tracing::info!(
                        submissions_scrubbed = result.submissions_scrubbed,
                        packages_removed = result.packages_removed,
                        screenshots_removed = result.screenshots_removed,
                        "MiniApp market retention cleanup completed"
                    );
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::error!(
                        error = %error,
                        "MiniApp market retention cleanup failed"
                    );
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cleanup_scrubs_expired_closed_artifacts_but_keeps_approved_content() {
        let temporary = tempfile::tempdir().unwrap();
        let database = Database::open(&temporary.path().join("market.sqlite"))
            .await
            .unwrap();
        let artifacts = ArtifactStore::open(temporary.path().join("artifacts"))
            .await
            .unwrap();
        let old = 1_700_000_000_i64;
        sqlx::query(
            "INSERT INTO users(id, github_id, login, avatar_url, created_at, updated_at)
             VALUES(1, 1, 'owner', '', ?, ?)",
        )
        .bind(old)
        .bind(old)
        .execute(database.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO submissions(
               id, owner_user_id, slug, release_number, metadata_json, status,
               package_sha256, package_size, created_at, updated_at
             ) VALUES
               ('expired', 1, 'expired-app', 1, '{}', 'rejected', 'expiredhash', 4, ?, ?),
               ('active', 1, 'active-app', 1, '{}', 'approved', 'approvedhash', 4, ?, ?)",
        )
        .bind(old)
        .bind(old)
        .bind(old)
        .bind(old)
        .execute(database.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO listings(
               id, slug, owner_user_id, is_published, created_at, updated_at
             ) VALUES('listing', 'active-app', 1, 1, ?, ?)",
        )
        .bind(old)
        .bind(old)
        .execute(database.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO releases(
               id, listing_id, submission_id, release_number, metadata_json,
               package_sha256, package_size, review_bundle_hash, published_at
             ) VALUES('release', 'listing', 'active', 1, '{}',
                      'approvedhash', 4, 'bundle', ?)",
        )
        .bind(old)
        .execute(database.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO screenshots(
               id, submission_id, position, sha256, media_type, size_bytes,
               width, height, created_at
             ) VALUES('shot', 'expired', 0, 'expiredshot', 'image/webp', 4, 1, 1, ?)",
        )
        .bind(old)
        .execute(database.pool())
        .await
        .unwrap();
        artifacts.put_package("expiredhash", b"gone").await.unwrap();
        artifacts
            .put_package("approvedhash", b"keep")
            .await
            .unwrap();
        artifacts
            .put_screenshot("expiredshot", b"gone")
            .await
            .unwrap();

        let result = cleanup_expired_submission_artifacts_at(
            &database,
            &artifacts,
            old + CLOSED_RETENTION_SECONDS + 1,
        )
        .await
        .unwrap();

        assert_eq!(result.submissions_scrubbed, 1);
        assert_eq!(result.packages_removed, 1);
        assert_eq!(result.screenshots_removed, 1);
        assert!(!artifacts.package_path("expiredhash").exists());
        assert!(!artifacts.screenshot_path("expiredshot").exists());
        assert!(artifacts.package_path("approvedhash").exists());
        let approved_hash: String =
            sqlx::query_scalar("SELECT package_sha256 FROM submissions WHERE id = 'active'")
                .fetch_one(database.pool())
                .await
                .unwrap();
        assert_eq!(approved_hash, "approvedhash");
    }

    #[tokio::test]
    async fn cleanup_removes_unreferenced_overwritten_draft_fragments_after_seven_days() {
        let temporary = tempfile::tempdir().unwrap();
        let database = Database::open(&temporary.path().join("market.sqlite"))
            .await
            .unwrap();
        let artifacts = ArtifactStore::open(temporary.path().join("artifacts"))
            .await
            .unwrap();
        let package_hash = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
        let screenshot_hash = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
        artifacts
            .put_package(package_hash, b"overwritten")
            .await
            .unwrap();
        artifacts
            .put_screenshot(screenshot_hash, b"overwritten")
            .await
            .unwrap();

        let future = Utc::now().timestamp() + DRAFT_RETENTION_SECONDS + 2;
        let result = cleanup_expired_submission_artifacts_at(&database, &artifacts, future)
            .await
            .unwrap();

        assert_eq!(result.packages_removed, 1);
        assert_eq!(result.screenshots_removed, 1);
        assert!(!artifacts.package_path(package_hash).exists());
        assert!(!artifacts.screenshot_path(screenshot_hash).exists());
    }
}
