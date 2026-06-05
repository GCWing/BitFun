//! jobs.json persistence wrapper.

use super::types::{
    CronJob, CronJobPayload, CronJobState, CronJobTarget, CronJobsFile, CronSchedule,
    CronWorkspaceRef, CRON_JOBS_VERSION,
};
use crate::infrastructure::storage::{PersistenceService, StorageOptions};
use crate::infrastructure::PathManager;
use crate::util::errors::{BitFunError, BitFunResult};
use log::{info, warn};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;

pub struct CronJobStore {
    persistence: PersistenceService,
    path_manager: Arc<PathManager>,
}

impl CronJobStore {
    pub async fn new(path_manager: Arc<PathManager>) -> BitFunResult<Self> {
        let cron_dir = path_manager.user_cron_dir();
        path_manager.ensure_dir(&cron_dir).await?;

        let persistence = PersistenceService::new(cron_dir).await?;

        Ok(Self {
            persistence,
            path_manager,
        })
    }

    pub fn jobs_file_path(&self) -> PathBuf {
        self.path_manager.cron_jobs_file()
    }

    pub async fn load(&self) -> BitFunResult<CronJobsFile> {
        let jobs_file_path = self.jobs_file_path();
        if !jobs_file_path.exists() {
            return Ok(CronJobsFile::default());
        }

        let content = fs::read_to_string(&jobs_file_path)
            .await
            .map_err(|error| BitFunError::service(format!("Failed to read file: {}", error)))?;

        match parse_jobs_file_content(&content, &jobs_file_path) {
            Ok(LoadJobsOutcome::Current(file)) => Ok(file),
            Ok(LoadJobsOutcome::Migrated(file)) => {
                info!(
                    "Migrated legacy cron jobs file to version {}: path={}",
                    CRON_JOBS_VERSION,
                    jobs_file_path.display()
                );
                self.save_jobs(file.jobs.clone()).await?;
                Ok(file)
            }
            Err(error) => {
                warn!(
                    "Failed to load cron jobs file; backing up and resetting to empty state: path={}, error={}",
                    jobs_file_path.display(),
                    error
                );
                self.backup_incompatible_jobs_file(&jobs_file_path).await?;
                self.save_jobs(Vec::new()).await?;
                Ok(CronJobsFile::default())
            }
        }
    }

    pub async fn save_jobs(&self, jobs: Vec<CronJob>) -> BitFunResult<()> {
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

    async fn backup_incompatible_jobs_file(&self, jobs_file_path: &Path) -> BitFunResult<()> {
        let backup_path = incompatible_backup_path(jobs_file_path);
        fs::rename(jobs_file_path, &backup_path)
            .await
            .map_err(|error| {
                BitFunError::service(format!(
                    "Failed to back up incompatible cron jobs file {} to {}: {}",
                    jobs_file_path.display(),
                    backup_path.display(),
                    error
                ))
            })?;
        info!(
            "Backed up incompatible cron jobs file: source={}, backup={}",
            jobs_file_path.display(),
            backup_path.display()
        );
        Ok(())
    }
}

#[derive(Debug)]
enum LoadJobsOutcome {
    Current(CronJobsFile),
    Migrated(CronJobsFile),
}

fn parse_jobs_file_content(content: &str, jobs_file_path: &Path) -> BitFunResult<LoadJobsOutcome> {
    let value: serde_json::Value = serde_json::from_str(content).map_err(|error| {
        BitFunError::service(format!(
            "Failed to parse cron jobs file {}: {}",
            jobs_file_path.display(),
            error
        ))
    })?;

    let version = value
        .get("version")
        .and_then(|value| value.as_u64())
        .ok_or_else(|| {
            BitFunError::service(format!(
                "Cron jobs file {} is missing a numeric version field",
                jobs_file_path.display()
            ))
        })?;

    if version == u64::from(CRON_JOBS_VERSION) {
        let file: CronJobsFile = serde_json::from_value(value).map_err(|error| {
            BitFunError::service(format!(
                "Failed to deserialize cron jobs file {} as version {}: {}",
                jobs_file_path.display(),
                CRON_JOBS_VERSION,
                error
            ))
        })?;
        return Ok(LoadJobsOutcome::Current(file));
    }

    if version == 1 {
        let legacy: LegacyCronJobsFileV1 = serde_json::from_value(value).map_err(|error| {
            BitFunError::service(format!(
                "Failed to deserialize legacy cron jobs file {}: {}",
                jobs_file_path.display(),
                error
            ))
        })?;
        return Ok(LoadJobsOutcome::Migrated(migrate_legacy_jobs_file(legacy)));
    }

    Err(BitFunError::service(format!(
        "Unsupported cron jobs file version {} in {}",
        version,
        jobs_file_path.display()
    )))
}

fn migrate_legacy_jobs_file(legacy: LegacyCronJobsFileV1) -> CronJobsFile {
    CronJobsFile {
        version: CRON_JOBS_VERSION,
        jobs: legacy.jobs.into_iter().map(migrate_legacy_job).collect(),
    }
}

fn migrate_legacy_job(legacy: LegacyCronJobV1) -> CronJob {
    CronJob {
        id: legacy.id,
        name: legacy.name,
        schedule: legacy.schedule,
        payload: legacy.payload,
        enabled: legacy.enabled,
        target: CronJobTarget::Session {
            session_id: legacy.session_id,
            workspace: CronWorkspaceRef {
                workspace_id: None,
                workspace_path: legacy.workspace_path,
                remote_connection_id: None,
                remote_ssh_host: None,
            },
        },
        created_at_ms: legacy.created_at_ms,
        config_updated_at_ms: legacy.config_updated_at_ms,
        updated_at_ms: legacy.updated_at_ms,
        state: legacy.state,
    }
}

fn incompatible_backup_path(jobs_file_path: &Path) -> PathBuf {
    let timestamp_ms = chrono::Utc::now().timestamp_millis();
    let file_name = jobs_file_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("jobs.json");
    jobs_file_path.with_file_name(format!("{}.incompatible.{}.bak", file_name, timestamp_ms))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyCronJobsFileV1 {
    #[serde(rename = "version")]
    _version: u32,
    jobs: Vec<LegacyCronJobV1>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyCronJobV1 {
    id: String,
    name: String,
    schedule: CronSchedule,
    payload: CronJobPayload,
    enabled: bool,
    session_id: String,
    workspace_path: String,
    created_at_ms: i64,
    config_updated_at_ms: i64,
    updated_at_ms: i64,
    #[serde(default)]
    state: CronJobState,
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

        let outcome = parse_jobs_file_content(content, Path::new("jobs.json")).expect("load");

        match outcome {
            LoadJobsOutcome::Current(file) => {
                assert_eq!(file.version, CRON_JOBS_VERSION);
                assert_eq!(file.jobs.len(), 1);
                assert_eq!(file.jobs[0].session_id(), Some("session_1"));
            }
            LoadJobsOutcome::Migrated(_) => panic!("expected current format"),
        }
    }

    #[test]
    fn parse_legacy_jobs_file_migrates_to_session_target() {
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

        let outcome = parse_jobs_file_content(content, Path::new("jobs.json")).expect("load");

        match outcome {
            LoadJobsOutcome::Migrated(file) => {
                assert_eq!(file.version, CRON_JOBS_VERSION);
                assert_eq!(file.jobs.len(), 1);
                let job = &file.jobs[0];
                assert_eq!(job.session_id(), Some("session_legacy"));
                assert_eq!(job.workspace().workspace_path, "E:/workspace");
            }
            LoadJobsOutcome::Current(_) => panic!("expected migrated format"),
        }
    }

    #[test]
    fn parse_unknown_version_returns_error() {
        let content = r#"{
          "version": 99,
          "jobs": []
        }"#;

        let error = parse_jobs_file_content(content, Path::new("jobs.json"))
            .expect_err("unknown version should fail");

        assert!(error
            .to_string()
            .contains("Unsupported cron jobs file version"));
    }
}
