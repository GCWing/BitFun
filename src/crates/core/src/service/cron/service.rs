//! Scheduled job service.

use super::schedule::{
    compute_initial_next_run_at_ms, compute_next_run_after_ms, validate_schedule,
};
use super::store::CronJobStore;
use super::types::{
    CreateCronJobRequest, CronJob, CronJobPayload, CronJobRunStatus, CronSchedule,
    UpdateCronJobRequest, DEFAULT_RETRY_DELAY_MS,
};
use crate::agentic::coordination::{
    DialogQueuePriority, DialogScheduler, DialogSubmissionPolicy, DialogTriggerSource,
};
use crate::agentic::core::PromptEnvelope;
use crate::infrastructure::PathManager;
use crate::util::errors::{BitFunError, BitFunResult};
use chrono::{Local, SecondsFormat, TimeZone, Utc};
use log::{debug, info, warn};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::sync::{Mutex, Notify, RwLock};
use tokio::time::Duration;
use uuid::Uuid;

static GLOBAL_CRON_SERVICE: OnceLock<Arc<CronService>> = OnceLock::new();

pub struct CronService {
    scheduler: Arc<DialogScheduler>,
    store: Arc<CronJobStore>,
    jobs: Arc<RwLock<HashMap<String, CronJob>>>,
    mutation_lock: Arc<Mutex<()>>,
    wakeup: Arc<Notify>,
    runner_started: AtomicBool,
}

impl CronService {
    pub async fn new(
        path_manager: Arc<PathManager>,
        scheduler: Arc<DialogScheduler>,
    ) -> BitFunResult<Arc<Self>> {
        let store = Arc::new(CronJobStore::new(path_manager).await?);
        let loaded = store.load().await?;
        let current_ms = now_ms();

        let mut jobs = HashMap::new();
        let mut needs_save = false;

        for mut job in loaded.jobs {
            if jobs.contains_key(&job.id) {
                return Err(BitFunError::service(format!(
                    "Duplicate scheduled job id found in jobs.json: {}",
                    job.id
                )));
            }

            needs_save |= reconcile_loaded_job(&mut job, current_ms)?;
            jobs.insert(job.id.clone(), job);
        }

        let service = Arc::new(Self {
            scheduler,
            store,
            jobs: Arc::new(RwLock::new(jobs)),
            mutation_lock: Arc::new(Mutex::new(())),
            wakeup: Arc::new(Notify::new()),
            runner_started: AtomicBool::new(false),
        });

        if needs_save {
            service.persist_snapshot().await?;
        }

        Ok(service)
    }

    pub fn start(self: &Arc<Self>) {
        if self
            .runner_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let service = Arc::clone(self);
        tokio::spawn(async move {
            service.run_loop().await;
        });
    }

    pub async fn list_jobs(&self) -> Vec<CronJob> {
        let jobs = self.jobs.read().await;
        jobs.values().cloned().collect::<Vec<_>>()
    }

    pub async fn list_jobs_filtered(
        &self,
        workspace_path: Option<&str>,
        session_id: Option<&str>,
    ) -> Vec<CronJob> {
        let jobs = self.jobs.read().await;
        jobs.values()
            .filter(|job| {
                workspace_path
                    .map(|workspace_path| job.workspace_path == workspace_path)
                    .unwrap_or(true)
                    && session_id
                        .map(|session_id| job.session_id == session_id)
                        .unwrap_or(true)
            })
            .cloned()
            .collect::<Vec<_>>()
    }

    pub async fn get_job(&self, job_id: &str) -> Option<CronJob> {
        self.jobs.read().await.get(job_id).cloned()
    }

    pub async fn create_job(&self, request: CreateCronJobRequest) -> BitFunResult<CronJob> {
        let _guard = self.mutation_lock.lock().await;
        let mut jobs = self.jobs.write().await;
        let current_ms = now_ms();
        let schedule = materialize_schedule(request.schedule, current_ms);
        validate_request_fields(
            &request.name,
            &request.payload,
            &request.session_id,
            &request.workspace_path,
        )?;
        validate_schedule(&schedule, current_ms)?;

        let mut job = CronJob {
            id: format!("cron_{}", Uuid::new_v4().simple()),
            name: request.name.trim().to_string(),
            schedule,
            payload: request.payload,
            enabled: request.enabled,
            session_id: request.session_id.trim().to_string(),
            workspace_path: request.workspace_path.trim().to_string(),
            created_at_ms: current_ms,
            config_updated_at_ms: current_ms,
            updated_at_ms: current_ms,
            state: Default::default(),
        };

        if job.enabled {
            job.state.next_run_at_ms = compute_initial_next_run_at_ms(&job, current_ms)?;
        }

        jobs.insert(job.id.clone(), job.clone());
        self.persist_jobs_locked(&jobs).await?;
        drop(jobs);
        self.wakeup.notify_one();

        Ok(job)
    }

    pub async fn update_job(
        &self,
        job_id: &str,
        request: UpdateCronJobRequest,
    ) -> BitFunResult<CronJob> {
        let _guard = self.mutation_lock.lock().await;
        let mut jobs = self.jobs.write().await;
        let current_ms = now_ms();
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| BitFunError::NotFound(format!("Scheduled job not found: {}", job_id)))?;

        if let Some(name) = request.name {
            job.name = name.trim().to_string();
        }
        if let Some(payload) = request.payload {
            job.payload = payload;
        }
        if let Some(session_id) = request.session_id {
            job.session_id = session_id.trim().to_string();
        }
        if let Some(workspace_path) = request.workspace_path {
            job.workspace_path = workspace_path.trim().to_string();
        }
        if let Some(enabled) = request.enabled {
            job.enabled = enabled;
        }
        if let Some(schedule) = request.schedule {
            job.schedule = materialize_schedule(schedule, current_ms);
        }

        validate_request_fields(
            &job.name,
            &job.payload,
            &job.session_id,
            &job.workspace_path,
        )?;
        validate_schedule(&job.schedule, job.created_at_ms)?;

        job.config_updated_at_ms = current_ms;
        job.updated_at_ms = current_ms;
        job.state.pending_trigger_at_ms = None;
        job.state.retry_at_ms = None;

        if !job.enabled {
            job.state.next_run_at_ms = None;
        } else if job.state.active_turn_id.is_some() {
            if job.is_one_shot() {
                job.state.next_run_at_ms = None;
            } else {
                job.state.next_run_at_ms =
                    compute_next_run_after_ms(&job.schedule, job.created_at_ms, current_ms)?;
            }
        } else {
            job.state.next_run_at_ms = compute_initial_next_run_at_ms(job, current_ms)?;
        }

        let updated = job.clone();
        self.persist_jobs_locked(&jobs).await?;
        drop(jobs);
        self.wakeup.notify_one();

        Ok(updated)
    }

    pub async fn set_job_enabled(&self, job_id: &str, enabled: bool) -> BitFunResult<CronJob> {
        self.update_job(
            job_id,
            UpdateCronJobRequest {
                enabled: Some(enabled),
                ..Default::default()
            },
        )
        .await
    }

    pub async fn delete_job(&self, job_id: &str) -> BitFunResult<bool> {
        let _guard = self.mutation_lock.lock().await;
        let mut jobs = self.jobs.write().await;
        let existed = jobs.remove(job_id).is_some();
        if existed {
            self.persist_jobs_locked(&jobs).await?;
            drop(jobs);
            self.wakeup.notify_one();
        }
        Ok(existed)
    }

    pub async fn run_job_now(&self, job_id: &str) -> BitFunResult<CronJob> {
        {
            let _guard = self.mutation_lock.lock().await;
            let mut jobs = self.jobs.write().await;
            let current_ms = now_ms();
            let job = jobs.get_mut(job_id).ok_or_else(|| {
                BitFunError::NotFound(format!("Scheduled job not found: {}", job_id))
            })?;

            if job.state.pending_trigger_at_ms.is_some() {
                job.state.coalesced_run_count = job.state.coalesced_run_count.saturating_add(1);
            }

            job.state.pending_trigger_at_ms = Some(current_ms);
            job.state.last_trigger_at_ms = Some(current_ms);
            job.state.retry_at_ms = None;
            job.updated_at_ms = current_ms;

            self.persist_jobs_locked(&jobs).await?;
            drop(jobs);
            self.wakeup.notify_one();
        }

        self.process_job(job_id).await?;
        self.get_job(job_id).await.ok_or_else(|| {
            BitFunError::NotFound(format!("Scheduled job not found after run: {}", job_id))
        })
    }

    pub async fn handle_turn_started(&self, turn_id: &str) -> BitFunResult<()> {
        self.handle_turn_state_change(turn_id, |job, now_ms| {
            job.state.last_run_status = Some(CronJobRunStatus::Running);
            job.state.last_run_started_at_ms = Some(now_ms);
            job.updated_at_ms = now_ms;
        })
        .await
    }

    pub async fn handle_turn_completed(&self, turn_id: &str, duration_ms: u64) -> BitFunResult<()> {
        self.handle_turn_state_change(turn_id, |job, now_ms| {
            job.state.active_turn_id = None;
            job.state.last_run_status = Some(CronJobRunStatus::Ok);
            job.state.last_error = None;
            job.state.last_duration_ms = Some(duration_ms);
            job.state.last_run_finished_at_ms = Some(now_ms);
            job.state.last_run_started_at_ms = Some(now_ms.saturating_sub(duration_ms as i64));
            job.state.consecutive_failures = 0;
            job.updated_at_ms = now_ms;
        })
        .await
    }

    pub async fn handle_turn_failed(&self, turn_id: &str, error: &str) -> BitFunResult<()> {
        self.handle_turn_state_change(turn_id, |job, now_ms| {
            job.state.active_turn_id = None;
            job.state.last_run_status = Some(CronJobRunStatus::Error);
            job.state.last_error = Some(error.to_string());
            job.state.last_run_finished_at_ms = Some(now_ms);
            job.state.consecutive_failures = job.state.consecutive_failures.saturating_add(1);
            job.updated_at_ms = now_ms;
        })
        .await
    }

    pub async fn handle_turn_cancelled(&self, turn_id: &str) -> BitFunResult<()> {
        self.handle_turn_state_change(turn_id, |job, now_ms| {
            job.state.active_turn_id = None;
            job.state.last_run_status = Some(CronJobRunStatus::Cancelled);
            job.state.last_error = None;
            job.state.last_run_finished_at_ms = Some(now_ms);
            job.updated_at_ms = now_ms;
        })
        .await
    }

    async fn handle_turn_state_change<F>(&self, turn_id: &str, update: F) -> BitFunResult<()>
    where
        F: FnOnce(&mut CronJob, i64),
    {
        let _guard = self.mutation_lock.lock().await;
        let mut jobs = self.jobs.write().await;
        let Some(job) = jobs
            .values_mut()
            .find(|job| job.state.active_turn_id.as_deref() == Some(turn_id))
        else {
            return Ok(());
        };

        update(job, now_ms());
        self.persist_jobs_locked(&jobs).await?;
        drop(jobs);
        self.wakeup.notify_one();
        Ok(())
    }

    async fn run_loop(self: Arc<Self>) {
        info!("Cron service loop started");

        loop {
            match self.next_wakeup_at().await {
                Some(next_wakeup_ms) => {
                    let current_ms = now_ms();
                    if next_wakeup_ms > current_ms {
                        let sleep_ms = (next_wakeup_ms - current_ms) as u64;
                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_millis(sleep_ms)) => {}
                            _ = self.wakeup.notified() => {
                                continue;
                            }
                        }
                    }
                }
                None => {
                    self.wakeup.notified().await;
                    continue;
                }
            }

            if let Err(error) = self.process_due_jobs().await {
                warn!("Failed to process due scheduled jobs: {}", error);
                tokio::time::sleep(Duration::from_millis(1_000)).await;
            }
        }
    }

    async fn next_wakeup_at(&self) -> Option<i64> {
        let jobs = self.jobs.read().await;
        jobs.values().filter_map(next_wakeup_for_job).min()
    }

    async fn process_due_jobs(&self) -> BitFunResult<()> {
        let current_ms = now_ms();
        let due_job_ids = {
            let jobs = self.jobs.read().await;
            let mut due = jobs
                .values()
                .filter_map(|job| {
                    let wake_at = next_wakeup_for_job(job)?;
                    (wake_at <= current_ms).then(|| (job.id.clone(), wake_at))
                })
                .collect::<Vec<_>>();
            due.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));
            due.into_iter()
                .map(|(job_id, _)| job_id)
                .collect::<Vec<_>>()
        };

        for job_id in due_job_ids {
            self.process_job(&job_id).await?;
        }

        Ok(())
    }

    async fn process_job(&self, job_id: &str) -> BitFunResult<()> {
        let _guard = self.mutation_lock.lock().await;
        let mut jobs = self.jobs.write().await;
        let current_ms = now_ms();

        let mut should_persist = false;
        let mut should_attempt_enqueue = false;
        let mut scheduled_at_ms = None;
        let mut enqueue_input = None;

        {
            let Some(job) = jobs.get_mut(job_id) else {
                return Ok(());
            };

            if !job.enabled && job.state.pending_trigger_at_ms.is_none() {
                return Ok(());
            }

            if let Some(next_run_at_ms) = job.state.next_run_at_ms {
                if next_run_at_ms <= current_ms {
                    if job.state.active_turn_id.is_some()
                        || job.state.pending_trigger_at_ms.is_some()
                    {
                        job.state.last_trigger_at_ms = Some(next_run_at_ms);
                        job.state.coalesced_run_count =
                            job.state.coalesced_run_count.saturating_add(1);
                        job.state.next_run_at_ms = compute_next_run_after_ms(
                            &job.schedule,
                            job.created_at_ms,
                            current_ms,
                        )?;
                        job.updated_at_ms = current_ms;
                        should_persist = true;
                    } else {
                        job.state.pending_trigger_at_ms = Some(next_run_at_ms);
                        job.state.last_trigger_at_ms = Some(next_run_at_ms);
                        job.state.retry_at_ms = None;
                        job.state.next_run_at_ms = compute_next_run_after_ms(
                            &job.schedule,
                            job.created_at_ms,
                            current_ms,
                        )?;
                        job.updated_at_ms = current_ms;
                        should_persist = true;
                    }
                }
            }

            if job.state.active_turn_id.is_none() && pending_is_due(job, current_ms) {
                let pending_trigger_at_ms = job.state.pending_trigger_at_ms.ok_or_else(|| {
                    BitFunError::service(format!(
                        "Scheduled job {} is missing pending trigger timestamp",
                        job.id
                    ))
                })?;

                let turn_id = format!("cronjob_{}_{}", job.id, pending_trigger_at_ms);
                scheduled_at_ms = Some(pending_trigger_at_ms);
                enqueue_input = Some(EnqueueInput {
                    turn_id,
                    session_id: job.session_id.clone(),
                    workspace_path: job.workspace_path.clone(),
                    user_input: format_scheduled_job_user_input(&job.payload.text, current_ms),
                });
                should_attempt_enqueue = true;
            }
        }

        if should_persist {
            self.persist_jobs_locked(&jobs).await?;
        }

        if !should_attempt_enqueue {
            return Ok(());
        }

        let enqueue_input = enqueue_input.ok_or_else(|| {
            BitFunError::service(format!(
                "Scheduled job {} is missing enqueue input after due calculation",
                job_id
            ))
        })?;
        let scheduled_at_ms = scheduled_at_ms.ok_or_else(|| {
            BitFunError::service(format!(
                "Scheduled job {} is missing scheduled timestamp after due calculation",
                job_id
            ))
        })?;

        let submit_result = self
            .scheduler
            .submit(
                enqueue_input.session_id.clone(),
                enqueue_input.user_input,
                None,
                Some(enqueue_input.turn_id.clone()),
                String::new(),
                Some(enqueue_input.workspace_path),
                scheduled_job_policy(),
                None,
            )
            .await;

        let now_after_submit = now_ms();
        let Some(job) = jobs.get_mut(job_id) else {
            return Ok(());
        };

        match submit_result {
            Ok(()) => {
                job.state.active_turn_id = Some(enqueue_input.turn_id);
                job.state.pending_trigger_at_ms = None;
                job.state.retry_at_ms = None;
                job.state.last_enqueued_at_ms = Some(now_after_submit);
                job.state.last_run_status = Some(CronJobRunStatus::Queued);
                job.state.last_error = None;
                job.updated_at_ms = now_after_submit;

                if job.is_one_shot() {
                    job.enabled = false;
                    job.state.next_run_at_ms = None;
                }

                debug!(
                    "Scheduled job enqueued: job_id={}, session_id={}, scheduled_at_ms={}",
                    job.id, job.session_id, scheduled_at_ms
                );
            }
            Err(error) => {
                job.state.last_run_status = Some(CronJobRunStatus::Error);
                job.state.last_error = Some(error.clone());
                job.state.last_run_finished_at_ms = Some(now_after_submit);
                job.state.retry_at_ms = Some(now_after_submit + DEFAULT_RETRY_DELAY_MS);
                job.state.consecutive_failures = job.state.consecutive_failures.saturating_add(1);
                job.updated_at_ms = now_after_submit;

                warn!(
                    "Failed to enqueue scheduled job: job_id={}, session_id={}, error={}",
                    job.id, job.session_id, error
                );
            }
        }

        self.persist_jobs_locked(&jobs).await?;
        drop(jobs);
        self.wakeup.notify_one();
        Ok(())
    }

    async fn persist_snapshot(&self) -> BitFunResult<()> {
        let jobs = self.jobs.read().await;
        self.persist_jobs_locked(&jobs).await
    }

    async fn persist_jobs_locked(&self, jobs: &HashMap<String, CronJob>) -> BitFunResult<()> {
        self.store
            .save_jobs(jobs.values().cloned().collect::<Vec<_>>())
            .await
    }
}

pub fn get_global_cron_service() -> Option<Arc<CronService>> {
    GLOBAL_CRON_SERVICE.get().cloned()
}

pub fn set_global_cron_service(service: Arc<CronService>) {
    let _ = GLOBAL_CRON_SERVICE.set(service);
}

fn reconcile_loaded_job(job: &mut CronJob, now_ms: i64) -> BitFunResult<bool> {
    let original = job.clone();

    validate_request_fields(
        &job.name,
        &job.payload,
        &job.session_id,
        &job.workspace_path,
    )?;
    validate_schedule(&job.schedule, job.created_at_ms)?;

    if job.updated_at_ms < job.created_at_ms {
        job.updated_at_ms = job.created_at_ms;
    }

    if let CronSchedule::Every { anchor_ms, .. } = &mut job.schedule {
        if anchor_ms.is_none() {
            *anchor_ms = Some(job.created_at_ms);
        }
    }

    if job.state.active_turn_id.is_some() {
        job.state.active_turn_id = None;
        job.state.pending_trigger_at_ms = None;
        job.state.retry_at_ms = None;
        job.state.last_run_status = Some(CronJobRunStatus::Error);
        job.state.last_error =
            Some("Application restarted before the scheduled job finished".to_string());
        job.state.last_run_finished_at_ms = Some(now_ms);
        job.state.consecutive_failures = job.state.consecutive_failures.saturating_add(1);
        job.updated_at_ms = now_ms;
    }

    if !job.enabled {
        job.state.next_run_at_ms = None;
        job.state.pending_trigger_at_ms = None;
        job.state.retry_at_ms = None;
    } else if job.state.pending_trigger_at_ms.is_some() {
        if job.state.retry_at_ms.is_none() {
            job.state.retry_at_ms = Some(now_ms);
        }
    } else if job.state.next_run_at_ms.is_none() {
        job.state.next_run_at_ms = compute_initial_next_run_at_ms(job, now_ms)?;
    }

    Ok(job != &original)
}

fn validate_request_fields(
    name: &str,
    payload: &CronJobPayload,
    session_id: &str,
    workspace_path: &str,
) -> BitFunResult<()> {
    if name.trim().is_empty() {
        return Err(BitFunError::validation(
            "Scheduled job name must not be empty",
        ));
    }
    if payload.text.trim().is_empty() {
        return Err(BitFunError::validation(
            "Scheduled job payload.text must not be empty",
        ));
    }
    if session_id.trim().is_empty() {
        return Err(BitFunError::validation(
            "Scheduled job sessionId must not be empty",
        ));
    }
    if workspace_path.trim().is_empty() {
        return Err(BitFunError::validation(
            "Scheduled job workspacePath must not be empty",
        ));
    }

    Ok(())
}

fn materialize_schedule(schedule: CronSchedule, anchor_ms: i64) -> CronSchedule {
    match schedule {
        CronSchedule::Every {
            every_ms,
            anchor_ms: None,
        } => CronSchedule::Every {
            every_ms,
            anchor_ms: Some(anchor_ms),
        },
        other => other,
    }
}

fn pending_is_due(job: &CronJob, now_ms: i64) -> bool {
    let Some(pending_trigger_at_ms) = job.state.pending_trigger_at_ms else {
        return false;
    };

    let retry_at_ms = job.state.retry_at_ms.unwrap_or(pending_trigger_at_ms);
    retry_at_ms <= now_ms
}

fn next_wakeup_for_job(job: &CronJob) -> Option<i64> {
    let schedule_wakeup = job.state.next_run_at_ms;
    let retry_wakeup = job
        .state
        .pending_trigger_at_ms
        .map(|pending_trigger_at_ms| job.state.retry_at_ms.unwrap_or(pending_trigger_at_ms));

    match (schedule_wakeup, retry_wakeup) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn format_scheduled_job_user_input(payload: &str, current_ms: i64) -> String {
    let current_time = Local
        .timestamp_millis_opt(current_ms)
        .single()
        .map(|datetime| datetime.to_rfc3339_opts(SecondsFormat::Secs, false))
        .unwrap_or_else(|| current_ms.to_string());

    let mut envelope = PromptEnvelope::new();
    envelope.push_system_reminder(format!(
        "This message was triggered by a scheduled job.\nCurrent time: {}",
        current_time
    ));
    envelope.push_user_query(payload.to_string());
    envelope.render()
}

fn scheduled_job_policy() -> DialogSubmissionPolicy {
    DialogSubmissionPolicy::new(
        DialogTriggerSource::ScheduledJob,
        DialogQueuePriority::Low,
        true,
    )
}

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

struct EnqueueInput {
    turn_id: String,
    session_id: String,
    workspace_path: String,
    user_input: String,
}
