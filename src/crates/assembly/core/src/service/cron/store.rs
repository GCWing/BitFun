//! jobs.json persistence wrapper.

use super::types::{CronJob, CronJobsFile, CRON_JOBS_VERSION};
use crate::infrastructure::storage::{PersistenceService, StorageOptions};
use crate::infrastructure::PathManager;
use crate::util::errors::{OpenBitFunError, OpenBitFunResult};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;

pub(super) struct CronJobStore {
    persistence: PersistenceService,
    path_manager: Arc<PathManager>,
}

impl CronJobStore {
    pub(super) async fn new(path_manager: Arc<PathManager>) -> OpenBitFunResult<Self> {
        let cron_dir = path_manager.user_cron_dir();
        path_manager.ensure_dir(&cron_dir).await?;

        let persistence = PersistenceService::new(cron_dir).await?;

        Ok(Self {
            persistence,
            path_manager,
        })
    }

    fn jobs_file_path(&self) -> PathBuf {
        self.path_manager.cron_jobs_file()
    }

    pub(super) async fn load(&self) -> OpenBitFunResult<CronJobsFile> {
        let jobs_file_path = self.jobs_file_path();
        if !jobs_file_path.exists() {
            return Ok(CronJobsFile::default());
        }

        let content = fs::read_to_string(&jobs_file_path)
            .await
            .map_err(|error| OpenBitFunError::service(format!("Failed to read file: {}", error)))?;

        parse_jobs_file_content(&content, &jobs_file_path)
    }

    pub(super) async fn save_jobs(&self, jobs: Vec<CronJob>) -> OpenBitFunResult<()> {
        let mut jobs = jobs;
        jobs.sort_by(|left, right| {
            left.created_at_ms
                .cmp(&right.created_at_ms)
                .then_with(|| left.id.cmp(&right.id))
        });

        let data = CronJobsFile {
            version: CRON_JOBS_VERSION,
            jobs,
        };

        self.persistence
            .save_json("jobs", &data, StorageOptions::default())
            .await
    }
}

fn unsupported_jobs_file(jobs_file_path: &Path, detail: impl AsRef<str>) -> OpenBitFunError {
    OpenBitFunError::config(format!(
        "Unsupported cron jobs persistence format in {}: {}. The file was left unchanged; explicit data migration is required",
        jobs_file_path.display(),
        detail.as_ref()
    ))
}

fn parse_jobs_file_content(content: &str, jobs_file_path: &Path) -> OpenBitFunResult<CronJobsFile> {
    let value: serde_json::Value = serde_json::from_str(content)
        .map_err(|error| unsupported_jobs_file(jobs_file_path, error.to_string()))?;

    let version = value
        .get("version")
        .and_then(|value| value.as_u64())
        .ok_or_else(|| unsupported_jobs_file(jobs_file_path, "missing numeric version field"))?;

    if version != u64::from(CRON_JOBS_VERSION) {
        return Err(unsupported_jobs_file(
            jobs_file_path,
            format!("version {version} is not supported; expected {CRON_JOBS_VERSION}"),
        ));
    }

    serde_json::from_value(value).map_err(|error| {
        unsupported_jobs_file(
            jobs_file_path,
            format!("failed to deserialize version {CRON_JOBS_VERSION}: {error}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_current_jobs_file_keeps_current_format() {
        let content = r#"{
          "version": 2,
          "jobs": [
            {
              "id": "cron_1",
              "name": "daily",
              "schedule": { "kind": "cron", "expr": "0 8 * * *" },
              "payload": { "text": "hello" },
              "enabled": true,
              "target": {
                "kind": "session",
                "sessionId": "session_1",
                "workspace": { "workspacePath": "E:/workspace" }
              },
              "createdAtMs": 1,
              "configUpdatedAtMs": 2,
              "updatedAtMs": 3,
              "state": {}
            }
          ]
        }"#;

        let file = parse_jobs_file_content(content, Path::new("jobs.json")).expect("load");

        assert_eq!(file.version, CRON_JOBS_VERSION);
        assert_eq!(file.jobs.len(), 1);
        assert_eq!(file.jobs[0].session_id(), Some("session_1"));
    }

    #[test]
    fn parse_version_one_jobs_file_requires_explicit_migration() {
        let content = r#"{
          "version": 1,
          "jobs": [
            {
              "id": "cron_legacy",
              "name": "legacy",
              "schedule": { "kind": "cron", "expr": "0 8 * * *" },
              "payload": { "text": "hello" },
              "enabled": true,
              "sessionId": "session_legacy",
              "workspacePath": "E:/workspace",
              "createdAtMs": 10,
              "configUpdatedAtMs": 20,
              "updatedAtMs": 30,
              "state": {}
            }
          ]
        }"#;

        let error = parse_jobs_file_content(content, Path::new("jobs.json"))
            .expect_err("version one must not migrate during normal loading");

        assert!(error.to_string().contains("version 1 is not supported"));
        assert!(error.to_string().contains("left unchanged"));
        assert!(error
            .to_string()
            .contains("explicit data migration is required"));
    }

    #[test]
    fn parse_unknown_version_returns_error() {
        let content = r#"{
          "version": 99,
          "jobs": []
        }"#;

        let error = parse_jobs_file_content(content, Path::new("jobs.json"))
            .expect_err("unknown version should fail");

        assert!(error.to_string().contains("version 99 is not supported"));
        assert!(error.to_string().contains("left unchanged"));
    }
}
