//! jobs.json persistence wrapper.

use super::types::{CronJob, CronJobsFile, CRON_JOBS_VERSION};
use crate::infrastructure::storage::{PersistenceService, StorageOptions};
use crate::infrastructure::PathManager;
use crate::util::errors::{BitFunError, BitFunResult};
use std::sync::Arc;

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

    pub fn jobs_file_path(&self) -> std::path::PathBuf {
        self.path_manager.cron_jobs_file()
    }

    pub async fn load(&self) -> BitFunResult<CronJobsFile> {
        let data = self.persistence.load_json::<CronJobsFile>("jobs").await?;
        match data {
            Some(file) if file.version == CRON_JOBS_VERSION => Ok(file),
            Some(file) => Err(BitFunError::service(format!(
                "Unsupported cron jobs file version {} in {:?}",
                file.version,
                self.jobs_file_path()
            ))),
            None => Ok(CronJobsFile::default()),
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
}
